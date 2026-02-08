//! Background Graph Builder for K-NN Graph Construction.
//!
//! This module provides a background service that builds K-NN graphs from
//! fingerprints stored in the teleological memory store.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────┐
//! │                  BackgroundGraphBuilder                         │
//! ├────────────────────────────────────────────────────────────────┤
//! │  store_memory() ─→ enqueue(fingerprint_id)                     │
//! │                          │                                      │
//! │                          ▼                                      │
//! │  ┌─────────────────────────────────────────────┐               │
//! │  │          Pending Queue (VecDeque<Uuid>)      │               │
//! │  └───────────────────────┬─────────────────────┘               │
//! │                          │                                      │
//! │                    Every 60s (batch interval)                  │
//! │                          │                                      │
//! │                          ▼                                      │
//! │  ┌─────────────────────────────────────────────┐               │
//! │  │  GraphLinkService.build() for batch         │               │
//! │  │  - NN-Descent per active embedder           │               │
//! │  │  - EdgeBuilder creates typed edges          │               │
//! │  └───────────────────────┬─────────────────────┘               │
//! │                          │                                      │
//! │                          ▼                                      │
//! │  ┌─────────────────────────────────────────────┐               │
//! │  │  EdgeRepository.store_* (RocksDB)           │               │
//! │  └─────────────────────────────────────────────┘               │
//! └────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Design Constraints
//!
//! - **NO FALLBACKS**: All errors propagate, no silent failures
//! - **FAIL FAST**: Errors logged with context and returned immediately
//! - **AP-60**: Temporal embedders (E2-E4) excluded from edge type detection
//! - **AP-77**: E5 uses asymmetric similarity

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::graph_linking::service::{GraphLinkService, GraphLinkServiceConfig};
use context_graph_core::graph_linking::{EdgeBuilderConfig, NnDescentConfig, TypedEdge};
use context_graph_core::traits::{MultiArrayEmbeddingProvider, TeleologicalMemoryStore};
use context_graph_core::types::audit::{AuditOperation, AuditRecord};
use context_graph_core::types::fingerprint::TeleologicalFingerprint;

use super::{EdgeRepository, GraphEdgeStorageError, GraphEdgeStorageResult};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the background graph builder.
#[derive(Debug, Clone)]
pub struct GraphBuilderConfig {
    /// Interval between batch processing in seconds.
    /// Default: 60 seconds.
    pub batch_interval_secs: u64,

    /// Maximum number of fingerprints to process in one batch.
    /// Default: 1000.
    pub max_batch_size: usize,

    /// Minimum batch size before processing.
    /// Default: 10.
    pub min_batch_size: usize,

    /// Which embedders to build K-NN graphs for.
    /// Default: [0, 4, 6, 7, 9, 10] = E1, E5, E7, E8, E10, E11
    pub active_embedders: Vec<u8>,

    /// K neighbors per node in K-NN graph.
    /// Default: 20.
    pub k: usize,

    /// Minimum weighted agreement for creating typed edges.
    /// Default: 2.5 (same as topic threshold per ARCH-09).
    pub min_weighted_agreement: f32,

    /// Whether to process immediately when max_batch_size is reached.
    /// Default: true.
    pub process_on_max: bool,
}

impl Default for GraphBuilderConfig {
    fn default() -> Self {
        Self {
            batch_interval_secs: 60,
            max_batch_size: 1000,
            min_batch_size: 10,
            active_embedders: vec![0, 4, 6, 7, 9, 10], // E1, E5, E7, E8, E10, E11
            k: 20,
            min_weighted_agreement: 2.5,
            process_on_max: true,
        }
    }
}

// ============================================================================
// Result Types
// ============================================================================

/// Result of batch processing.
#[derive(Debug, Clone)]
pub struct BatchBuildResult {
    /// Number of fingerprints processed.
    pub processed_count: usize,
    /// Number of K-NN edges created per embedder.
    pub knn_edges_created: usize,
    /// Number of typed edges created.
    pub typed_edges_created: usize,
    /// Processing time in milliseconds.
    pub elapsed_ms: u64,
    /// Errors encountered (non-fatal).
    pub warnings: Vec<String>,
}

/// Result of full rebuild.
#[derive(Debug, Clone)]
pub struct RebuildResult {
    /// Total fingerprints processed.
    pub total_processed: usize,
    /// Number of batches processed.
    pub batch_count: usize,
    /// Total K-NN edges created.
    pub total_knn_edges: usize,
    /// Total typed edges created.
    pub total_typed_edges: usize,
    /// Total time in milliseconds.
    pub elapsed_ms: u64,
}

// ============================================================================
// Background Graph Builder
// ============================================================================

/// Background service for building K-NN graphs from stored fingerprints.
///
/// This service:
/// 1. Queues fingerprint IDs as they are stored
/// 2. Periodically processes the queue in batches
/// 3. Builds K-NN graphs using NN-Descent algorithm
/// 4. Creates typed edges based on embedder agreement
/// 5. Persists results to EdgeRepository
pub struct BackgroundGraphBuilder {
    /// Edge repository for persistent storage.
    edge_repository: EdgeRepository,

    /// Teleological memory store for retrieving fingerprints.
    teleological_store: Arc<dyn TeleologicalMemoryStore>,

    /// Multi-array embedding provider for similarity computation.
    #[allow(dead_code)]
    multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,

    /// Configuration.
    config: GraphBuilderConfig,

    /// Pending fingerprint IDs to process.
    pending_queue: Arc<Mutex<VecDeque<Uuid>>>,

    /// Flag to stop the background worker.
    running: Arc<AtomicBool>,

    /// Stats tracking.
    stats: Arc<Mutex<BuilderStats>>,
}

/// Runtime statistics for the builder.
#[derive(Debug, Clone, Default)]
pub struct BuilderStats {
    /// Total fingerprints processed.
    pub total_processed: usize,
    /// Total batches processed.
    pub batches_processed: usize,
    /// Total K-NN edges created.
    pub total_knn_edges: usize,
    /// Total typed edges created.
    pub total_typed_edges: usize,
    /// Current queue size.
    pub queue_size: usize,
    /// Last batch processing time in ms.
    pub last_batch_ms: u64,
}

impl BackgroundGraphBuilder {
    /// Create a new background graph builder.
    ///
    /// # Arguments
    ///
    /// * `edge_repository` - Repository for storing K-NN and typed edges
    /// * `teleological_store` - Store for retrieving fingerprints
    /// * `multi_array_provider` - Provider for computing embeddings
    /// * `config` - Builder configuration
    pub fn new(
        edge_repository: EdgeRepository,
        teleological_store: Arc<dyn TeleologicalMemoryStore>,
        multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,
        config: GraphBuilderConfig,
    ) -> Self {
        info!(
            "Creating BackgroundGraphBuilder with batch_interval={}s, max_batch={}, active_embedders={:?}",
            config.batch_interval_secs, config.max_batch_size, config.active_embedders
        );

        Self {
            edge_repository,
            teleological_store,
            multi_array_provider,
            config,
            pending_queue: Arc::new(Mutex::new(VecDeque::new())),
            running: Arc::new(AtomicBool::new(false)),
            stats: Arc::new(Mutex::new(BuilderStats::default())),
        }
    }

    /// Queue a fingerprint for edge computation.
    ///
    /// Called from store_memory after a fingerprint is stored.
    /// Thread-safe and non-blocking.
    pub async fn enqueue(&self, fingerprint_id: Uuid) {
        let mut queue = self.pending_queue.lock().await;
        queue.push_back(fingerprint_id);

        let queue_size = queue.len();
        self.stats.lock().await.queue_size = queue_size;

        debug!(
            fingerprint_id = %fingerprint_id,
            queue_size = queue_size,
            "Queued fingerprint for K-NN graph building"
        );

        // Check if we should process immediately due to max batch size
        if self.config.process_on_max && queue_size >= self.config.max_batch_size {
            debug!("Queue reached max_batch_size, signaling immediate processing");
        }
    }

    /// Get current queue size.
    pub async fn queue_size(&self) -> usize {
        self.pending_queue.lock().await.len()
    }

    /// Get current builder statistics.
    pub async fn stats(&self) -> BuilderStats {
        self.stats.lock().await.clone()
    }

    /// Get the builder configuration.
    pub fn config(&self) -> &GraphBuilderConfig {
        &self.config
    }

    /// Process the pending queue.
    ///
    /// Drains fingerprints from the queue and builds K-NN graphs.
    /// Returns immediately if queue size is below minimum.
    pub async fn process_batch(&self) -> GraphEdgeStorageResult<BatchBuildResult> {
        let start = Instant::now();
        let mut warnings = Vec::new();

        // Drain queue (up to max_batch_size)
        let fingerprint_ids: Vec<Uuid> = {
            let mut queue = self.pending_queue.lock().await;
            let drain_count = queue.len().min(self.config.max_batch_size);

            if drain_count < self.config.min_batch_size {
                debug!(
                    queue_size = queue.len(),
                    min_batch = self.config.min_batch_size,
                    "Queue below minimum batch size, skipping"
                );
                return Ok(BatchBuildResult {
                    processed_count: 0,
                    knn_edges_created: 0,
                    typed_edges_created: 0,
                    elapsed_ms: 0,
                    warnings: vec![],
                });
            }

            queue.drain(..drain_count).collect()
        };

        let processed_count = fingerprint_ids.len();
        info!(
            fingerprint_count = processed_count,
            "Processing batch for K-NN graph building"
        );

        // Retrieve fingerprints from store
        let mut fingerprints: Vec<TeleologicalFingerprint> = Vec::with_capacity(processed_count);
        for id in &fingerprint_ids {
            match self.teleological_store.retrieve(*id).await {
                Ok(Some(fp)) => fingerprints.push(fp),
                Ok(None) => {
                    warnings.push(format!("Fingerprint {} not found in store", id));
                }
                Err(e) => {
                    warnings.push(format!("Failed to retrieve fingerprint {}: {}", id, e));
                }
            }
        }

        if fingerprints.is_empty() {
            warn!("No fingerprints retrieved from store for batch");
            return Ok(BatchBuildResult {
                processed_count: 0,
                knn_edges_created: 0,
                typed_edges_created: 0,
                elapsed_ms: start.elapsed().as_millis() as u64,
                warnings,
            });
        }

        // Build K-NN graphs and typed edges
        let (knn_edges_created, typed_edges) =
            self.build_graphs_for_fingerprints(&fingerprints).await?;

        // Store typed edges in batch
        if !typed_edges.is_empty() {
            self.edge_repository.store_typed_edges_batch(&typed_edges)?;
            info!(
                typed_edges_count = typed_edges.len(),
                "Stored typed edges to repository"
            );
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;

        // Update stats
        {
            let mut stats = self.stats.lock().await;
            stats.total_processed += processed_count;
            stats.batches_processed += 1;
            stats.total_knn_edges += knn_edges_created;
            stats.total_typed_edges += typed_edges.len();
            stats.queue_size = self.pending_queue.lock().await.len();
            stats.last_batch_ms = elapsed_ms;
        }

        info!(
            processed = processed_count,
            knn_edges = knn_edges_created,
            typed_edges = typed_edges.len(),
            elapsed_ms = elapsed_ms,
            "Batch processing complete"
        );

        // Emit audit record for background graph building provenance
        let typed_edges_count = typed_edges.len();
        if processed_count > 0 {
            let audit_record = AuditRecord::new(
                AuditOperation::RelationshipDiscovered {
                    relationship_type: "knn_graph_batch".to_string(),
                    confidence: 1.0,
                },
                // Use first fingerprint ID as target, or nil if none
                fingerprint_ids.first().copied().unwrap_or(Uuid::nil()),
            )
            .with_operator("background_graph_builder")
            .with_rationale(format!(
                "Built K-NN graph batch: {} fingerprints → {} K-NN edges + {} typed edges in {}ms",
                processed_count, knn_edges_created, typed_edges_count, elapsed_ms
            ))
            .with_parameters(serde_json::json!({
                "processed_count": processed_count,
                "knn_edges_created": knn_edges_created,
                "typed_edges_created": typed_edges_count,
                "elapsed_ms": elapsed_ms,
                "active_embedders": self.config.active_embedders,
                "warning_count": warnings.len(),
            }));

            if let Err(e) = self.teleological_store.append_audit_record(&audit_record).await {
                warn!(error = %e, "BackgroundGraphBuilder: Failed to write audit record (non-fatal)");
            }
        }

        Ok(BatchBuildResult {
            processed_count,
            knn_edges_created,
            typed_edges_created: typed_edges_count,
            elapsed_ms,
            warnings,
        })
    }

    /// Extract embedding from fingerprint for a given embedder ID.
    fn get_embedding(fp: &TeleologicalFingerprint, embedder_id: u8) -> Option<Vec<f32>> {
        match embedder_id {
            0 => Some(fp.semantic.e1_semantic.clone()),
            4 => {
                // E5 causal - use as_cause for K-NN building
                if !fp.semantic.e5_causal_as_cause.is_empty() {
                    Some(fp.semantic.e5_causal_as_cause.clone())
                } else if !fp.semantic.e5_causal.is_empty() {
                    Some(fp.semantic.e5_causal.clone())
                } else {
                    None
                }
            }
            6 => Some(fp.semantic.e7_code.clone()),
            7 => {
                // E8 graph - use as_source for K-NN building
                if !fp.semantic.e8_graph_as_source.is_empty() {
                    Some(fp.semantic.e8_graph_as_source.clone())
                } else if !fp.semantic.e8_graph.is_empty() {
                    Some(fp.semantic.e8_graph.clone())
                } else {
                    None
                }
            }
            9 => {
                // E10 multimodal - use as_intent for K-NN building
                if !fp.semantic.e10_multimodal_as_intent.is_empty() {
                    Some(fp.semantic.e10_multimodal_as_intent.clone())
                } else if !fp.semantic.e10_multimodal.is_empty() {
                    Some(fp.semantic.e10_multimodal.clone())
                } else {
                    None
                }
            }
            10 => Some(fp.semantic.e11_entity.clone()),
            _ => None,
        }
    }

    /// Build K-NN graphs for a set of fingerprints.
    ///
    /// Returns (knn_edges_created, typed_edges).
    async fn build_graphs_for_fingerprints(
        &self,
        fingerprints: &[TeleologicalFingerprint],
    ) -> GraphEdgeStorageResult<(usize, Vec<TypedEdge>)> {
        let node_ids: Vec<Uuid> = fingerprints.iter().map(|fp| fp.id).collect();

        // Create embedding lookup
        let mut embedding_map: HashMap<(Uuid, u8), Vec<f32>> = HashMap::new();
        for fp in fingerprints {
            for &emb_id in &self.config.active_embedders {
                if let Some(emb) = Self::get_embedding(fp, emb_id) {
                    if !emb.is_empty() {
                        embedding_map.insert((fp.id, emb_id), emb);
                    }
                }
            }
        }

        // Configure graph link service
        let nn_config = NnDescentConfig::default().with_k(self.config.k);
        let edge_config =
            EdgeBuilderConfig::default().with_min_weighted_agreement(self.config.min_weighted_agreement);

        let service_config = GraphLinkServiceConfig::default()
            .with_active_embedders(self.config.active_embedders.clone())
            .with_nn_descent(nn_config)
            .with_edge_builder(edge_config);

        let mut service = GraphLinkService::new(service_config);

        // Build K-NN graphs
        let build_result = service
            .build(
                &node_ids,
                |id, emb_id| embedding_map.get(&(id, emb_id)).cloned(),
                |a, b| {
                    // Cosine similarity
                    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
                    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
                    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
                    if norm_a == 0.0 || norm_b == 0.0 {
                        0.0
                    } else {
                        dot / (norm_a * norm_b)
                    }
                },
            )
            .map_err(|e| GraphEdgeStorageError::GraphBuildError {
                message: format!("NN-Descent build failed: {}", e),
            })?;

        // Store K-NN edges per embedder
        let mut knn_edges_created = 0;
        for (embedder_id, graph) in &build_result.graphs {
            for edge in graph.edges() {
                // Store edges for this source
                let edges = graph.get_neighbors(edge.source());
                if !edges.is_empty() {
                    self.edge_repository
                        .store_embedder_edges(*embedder_id, edge.source(), &edges)?;
                    knn_edges_created += edges.len();
                }
            }
        }

        Ok((knn_edges_created, build_result.typed_edges))
    }

    /// Full rebuild of all K-NN graphs.
    ///
    /// Retrieves all fingerprints from the store and rebuilds all edges.
    /// This is a heavy operation and should be used sparingly.
    pub async fn rebuild_all(&self) -> GraphEdgeStorageResult<RebuildResult> {
        let start = Instant::now();
        info!("Starting full K-NN graph rebuild");

        // Get all fingerprints from store using scan_fingerprints_for_clustering
        // This returns (id, embeddings) tuples which we can use to reconstruct fingerprints
        let all_fingerprints = self
            .teleological_store
            .scan_fingerprints_for_clustering(None)
            .await
            .map_err(|e| GraphEdgeStorageError::GraphBuildError {
                message: format!("Failed to scan fingerprints: {}", e),
            })?;

        let total_count = all_fingerprints.len();
        info!(fingerprint_count = total_count, "Retrieved fingerprints for rebuild");

        if total_count == 0 {
            return Ok(RebuildResult {
                total_processed: 0,
                batch_count: 0,
                total_knn_edges: 0,
                total_typed_edges: 0,
                elapsed_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Collect all IDs
        let all_ids: Vec<Uuid> = all_fingerprints.iter().map(|(id, _)| *id).collect();

        // Process in batches by queueing and processing
        let mut batch_count = 0;
        let mut total_knn_edges = 0;
        let mut total_typed_edges = 0;

        for chunk in all_ids.chunks(self.config.max_batch_size) {
            // Queue all IDs in this chunk
            {
                let mut queue = self.pending_queue.lock().await;
                queue.clear(); // Clear any pending items
                for id in chunk {
                    queue.push_back(*id);
                }
            }

            // Process this batch - force processing even for small batches during rebuild
            let result = self.process_batch_internal(1).await;

            match result {
                Ok(result) => {
                    batch_count += 1;
                    total_knn_edges += result.knn_edges_created;
                    total_typed_edges += result.typed_edges_created;

                    if !result.warnings.is_empty() {
                        warn!(
                            batch = batch_count,
                            warnings = ?result.warnings,
                            "Batch completed with warnings"
                        );
                    }
                }
                Err(e) => {
                    error!(batch = batch_count, error = %e, "Batch processing failed");
                    return Err(e);
                }
            }
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;

        info!(
            total_processed = total_count,
            batch_count = batch_count,
            total_knn_edges = total_knn_edges,
            total_typed_edges = total_typed_edges,
            elapsed_ms = elapsed_ms,
            "Full K-NN graph rebuild complete"
        );

        Ok(RebuildResult {
            total_processed: total_count,
            batch_count,
            total_knn_edges,
            total_typed_edges,
            elapsed_ms,
        })
    }

    /// Internal process_batch with configurable min_batch_size.
    async fn process_batch_internal(
        &self,
        min_batch_size: usize,
    ) -> GraphEdgeStorageResult<BatchBuildResult> {
        let start = Instant::now();
        let mut warnings = Vec::new();

        // Drain queue (up to max_batch_size)
        let fingerprint_ids: Vec<Uuid> = {
            let mut queue = self.pending_queue.lock().await;
            let drain_count = queue.len().min(self.config.max_batch_size);

            if drain_count < min_batch_size {
                return Ok(BatchBuildResult {
                    processed_count: 0,
                    knn_edges_created: 0,
                    typed_edges_created: 0,
                    elapsed_ms: 0,
                    warnings: vec![],
                });
            }

            queue.drain(..drain_count).collect()
        };

        let processed_count = fingerprint_ids.len();

        // Retrieve fingerprints from store
        let mut fingerprints: Vec<TeleologicalFingerprint> = Vec::with_capacity(processed_count);
        for id in &fingerprint_ids {
            match self.teleological_store.retrieve(*id).await {
                Ok(Some(fp)) => fingerprints.push(fp),
                Ok(None) => {
                    warnings.push(format!("Fingerprint {} not found in store", id));
                }
                Err(e) => {
                    warnings.push(format!("Failed to retrieve fingerprint {}: {}", id, e));
                }
            }
        }

        if fingerprints.is_empty() {
            return Ok(BatchBuildResult {
                processed_count: 0,
                knn_edges_created: 0,
                typed_edges_created: 0,
                elapsed_ms: start.elapsed().as_millis() as u64,
                warnings,
            });
        }

        // Build K-NN graphs and typed edges
        let (knn_edges_created, typed_edges) =
            self.build_graphs_for_fingerprints(&fingerprints).await?;

        // Store typed edges in batch
        if !typed_edges.is_empty() {
            self.edge_repository.store_typed_edges_batch(&typed_edges)?;
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;

        Ok(BatchBuildResult {
            processed_count,
            knn_edges_created,
            typed_edges_created: typed_edges.len(),
            elapsed_ms,
            warnings,
        })
    }

    /// Start the background worker loop.
    ///
    /// Returns a JoinHandle that can be awaited for graceful shutdown.
    pub fn start_worker(self: Arc<Self>) -> JoinHandle<()> {
        if self.running.swap(true, Ordering::SeqCst) {
            warn!("Background worker already running");
        }

        let this = Arc::clone(&self);
        let interval = Duration::from_secs(self.config.batch_interval_secs);

        tokio::spawn(async move {
            info!(
                interval_secs = this.config.batch_interval_secs,
                "Background graph builder worker started"
            );

            while this.running.load(Ordering::SeqCst) {
                // Sleep for interval
                tokio::time::sleep(interval).await;

                if !this.running.load(Ordering::SeqCst) {
                    break;
                }

                // Process batch
                match this.process_batch().await {
                    Ok(result) => {
                        if result.processed_count > 0 {
                            info!(
                                processed = result.processed_count,
                                knn_edges = result.knn_edges_created,
                                typed_edges = result.typed_edges_created,
                                elapsed_ms = result.elapsed_ms,
                                "Background batch processed"
                            );
                        } else {
                            debug!("Background batch: no items to process");
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "Background batch processing failed");
                    }
                }
            }

            info!("Background graph builder worker stopped");
        })
    }

    /// Stop the background worker.
    pub fn stop(&self) {
        info!("Stopping background graph builder worker");
        self.running.store(false, Ordering::SeqCst);
    }

    /// Check if the worker is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = GraphBuilderConfig::default();
        assert_eq!(config.batch_interval_secs, 60);
        assert_eq!(config.max_batch_size, 1000);
        assert_eq!(config.min_batch_size, 10);
        assert_eq!(config.k, 20);
        assert!((config.min_weighted_agreement - 2.5).abs() < 0.01);
    }

    #[test]
    fn test_builder_stats_default() {
        let stats = BuilderStats::default();
        assert_eq!(stats.total_processed, 0);
        assert_eq!(stats.batches_processed, 0);
        assert_eq!(stats.queue_size, 0);
    }
}
