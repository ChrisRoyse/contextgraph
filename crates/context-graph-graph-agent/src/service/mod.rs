//! Graph discovery service orchestrating the discovery pipeline.
//!
//! The service combines:
//! - Scanner: Finds candidate memory pairs
//! - LLM: Analyzes relationships
//! - Activator: Stores confirmed relationships
//!
//! # Pipeline Flow
//!
//! ```text
//! Memories -> Scanner -> Candidates -> LLM Analysis -> Activator -> Graph Edges
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use context_graph_causal_agent::CausalDiscoveryLLM;
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use context_graph_embeddings::models::GraphModel;

use crate::activator::{ActivatorConfig, E8Activator, GraphStorage};
use crate::error::{GraphAgentError, GraphAgentResult};
use crate::llm::GraphRelationshipLLM;
use crate::scanner::{MemoryScanner, ScannerConfig};
use crate::types::MemoryForGraphAnalysis;

/// Configuration for the graph discovery service.
#[derive(Debug, Clone)]
pub struct GraphDiscoveryConfig {
    /// Interval between discovery cycles (background mode).
    pub interval: Duration,

    /// Maximum candidates to process per cycle.
    pub batch_size: usize,

    /// Minimum confidence for relationships.
    pub min_confidence: f32,

    /// Scanner configuration.
    pub scanner_config: ScannerConfig,

    /// Activator configuration.
    pub activator_config: ActivatorConfig,

    /// Whether to skip already analyzed pairs.
    pub skip_analyzed: bool,
}

impl Default for GraphDiscoveryConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(3600), // 1 hour
            batch_size: 50,
            min_confidence: 0.7,
            scanner_config: ScannerConfig::default(),
            activator_config: ActivatorConfig::default(),
            skip_analyzed: true,
        }
    }
}

// Re-export shared type from core.
pub use context_graph_core::types::DiscoveryCycleResult;

pub use context_graph_core::types::ServiceStatus;

/// Graph relationship discovery service.
///
/// Orchestrates the discovery pipeline:
/// 1. Scans memories for candidate pairs
/// 2. Analyzes pairs with LLM for relationships
/// 3. Stores confirmed relationships via activator
pub struct GraphDiscoveryService {
    config: GraphDiscoveryConfig,
    llm: Arc<GraphRelationshipLLM>,
    scanner: RwLock<MemoryScanner>,
    activator: Arc<E8Activator>,
    running: AtomicBool,
    status: RwLock<ServiceStatus>,
    last_result: RwLock<Option<DiscoveryCycleResult>>,
    shutdown_tx: RwLock<Option<mpsc::Sender<()>>>,
    /// CRIT-05 FIX: Store background task JoinHandle so panics are not silently lost
    /// and stop() can await clean shutdown instead of spin-looping.
    join_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
}

impl GraphDiscoveryService {
    /// Create a new graph discovery service without a model.
    ///
    /// # Arguments
    /// * `shared_llm` - Shared CausalDiscoveryLLM from causal-agent
    ///
    /// # Warning
    ///
    /// This constructor creates an E8Activator WITHOUT a GraphModel.
    /// Embedding operations will fail. Use `with_models()` for production deployments.
    #[allow(deprecated)]
    pub fn new(shared_llm: Arc<CausalDiscoveryLLM>) -> Self {
        Self::with_config(shared_llm, GraphDiscoveryConfig::default())
    }

    /// Create with custom configuration but no model.
    ///
    /// # Warning
    ///
    /// This constructor creates an E8Activator WITHOUT a GraphModel.
    /// Embedding operations will fail. Use `with_models()` for production deployments.
    #[deprecated(
        since = "0.1.0",
        note = "Use with_models() for production. This constructor creates E8Activator without GraphModel."
    )]
    pub fn with_config(shared_llm: Arc<CausalDiscoveryLLM>, config: GraphDiscoveryConfig) -> Self {
        let graph_llm = Arc::new(GraphRelationshipLLM::new(shared_llm));
        let activator = Arc::new(E8Activator::with_config(config.activator_config.clone()));
        let scanner = MemoryScanner::with_config(config.scanner_config.clone());

        Self {
            config,
            llm: graph_llm,
            scanner: RwLock::new(scanner),
            activator,
            running: AtomicBool::new(false),
            status: RwLock::new(ServiceStatus::Stopped),
            last_result: RwLock::new(None),
            shutdown_tx: RwLock::new(None),
            join_handle: RwLock::new(None),
        }
    }

    /// Create with all models for production use.
    ///
    /// This is the recommended constructor for production deployments.
    /// It properly injects the GraphModel into E8Activator for real embeddings.
    ///
    /// # Arguments
    ///
    /// * `shared_llm` - Shared CausalDiscoveryLLM from causal-agent (for relationship classification)
    /// * `graph_model` - GraphModel for E8 asymmetric embeddings (1024D, ~1.3GB VRAM)
    /// * `config` - Service configuration
    ///
    /// # Example
    ///
    /// ```ignore
    /// let graph_model = Arc::new(GraphModel::new(path, config)?);
    /// graph_model.load().await?;
    /// let service = GraphDiscoveryService::with_models(shared_llm, graph_model, config);
    /// ```
    pub fn with_models(
        shared_llm: Arc<CausalDiscoveryLLM>,
        graph_model: Arc<GraphModel>,
        config: GraphDiscoveryConfig,
    ) -> Self {
        let graph_llm = Arc::new(GraphRelationshipLLM::new(shared_llm));
        let activator = Arc::new(E8Activator::with_model(
            graph_model,
            config.activator_config.clone(),
        ));
        let scanner = MemoryScanner::with_config(config.scanner_config.clone());

        Self {
            config,
            llm: graph_llm,
            scanner: RwLock::new(scanner),
            activator,
            running: AtomicBool::new(false),
            status: RwLock::new(ServiceStatus::Stopped),
            last_result: RwLock::new(None),
            shutdown_tx: RwLock::new(None),
            join_handle: RwLock::new(None),
        }
    }

    /// Check if the background loop has been started.
    ///
    /// LOW-4 Note: The background loop is a stub — it sleeps and logs
    /// "NOT IMPLEMENTED." No discovery cycles actually execute. This method
    /// returns `true` when the loop task has been spawned (for lifecycle
    /// management by `start()`/`stop()`), NOT when real discovery is happening.
    /// Use `discover_graph_relationships` MCP tool for on-demand analysis.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get current service status.
    pub fn status(&self) -> ServiceStatus {
        *self.status.read()
    }

    /// Get the last discovery cycle result.
    pub fn last_result(&self) -> Option<DiscoveryCycleResult> {
        self.last_result.read().clone()
    }

    /// Get the graph storage.
    pub fn graph(&self) -> Arc<RwLock<GraphStorage>> {
        self.activator.graph()
    }

    /// Get access to the underlying LLM for direct analysis.
    ///
    /// This allows tools like validate_graph_link to analyze single pairs
    /// without going through the full discovery cycle.
    pub fn llm(&self) -> &Arc<GraphRelationshipLLM> {
        &self.llm
    }

    /// Run a single discovery cycle manually.
    ///
    /// # Arguments
    /// * `memories` - Memories to analyze for relationships
    pub async fn run_discovery_cycle(
        &self,
        memories: &[MemoryForGraphAnalysis],
    ) -> GraphAgentResult<DiscoveryCycleResult> {
        let started_at = Utc::now();
        let mut relationships_confirmed = 0;
        let mut relationships_rejected = 0;
        let mut errors = 0;
        let mut error_messages = Vec::new();

        info!(
            memory_count = memories.len(),
            "Starting graph discovery cycle"
        );

        // Step 1: Find candidate pairs
        let candidates = {
            let mut scanner = self.scanner.write();
            scanner.find_candidates(memories)?
        };
        let candidates_found = candidates.len();

        debug!(candidates_found, "Found candidate pairs");

        if candidates.is_empty() {
            let completed_at = Utc::now();
            let duration = (completed_at - started_at)
                .to_std()
                .unwrap_or(Duration::ZERO);

            let result = DiscoveryCycleResult {
                started_at,
                completed_at,
                duration,
                candidates_found: 0,
                relationships_confirmed: 0,
                relationships_rejected: 0,
                embeddings_generated: 0,
                edges_created: 0,
                errors: 0,
                error_messages: Vec::new(),
            };

            *self.last_result.write() = Some(result.clone());
            return Ok(result);
        }

        // Step 2: Analyze candidates with LLM (batch processing)
        let batch_size = self.config.batch_size.min(candidates.len());
        let batch = &candidates[..batch_size];

        for candidate in batch {
            // Analyze relationship
            let analysis = match self
                .llm
                .analyze_relationship(&candidate.memory_a_content, &candidate.memory_b_content)
                .await
            {
                Ok(result) => result,
                Err(e) => {
                    warn!(
                        memory_a = %candidate.memory_a_id,
                        memory_b = %candidate.memory_b_id,
                        error = %e,
                        "Failed to analyze candidate"
                    );
                    errors += 1;
                    error_messages.push(format!(
                        "Analysis failed for {}-{}: {}",
                        candidate.memory_a_id, candidate.memory_b_id, e
                    ));
                    continue;
                }
            };

            // Mark as analyzed
            if self.config.skip_analyzed {
                let mut scanner = self.scanner.write();
                scanner.mark_analyzed(candidate.memory_a_id, candidate.memory_b_id);
            }

            // Check if relationship was confirmed
            if !analysis.has_connection || analysis.confidence < self.config.min_confidence {
                relationships_rejected += 1;
                debug!(
                    memory_a = %candidate.memory_a_id,
                    memory_b = %candidate.memory_b_id,
                    has_connection = analysis.has_connection,
                    confidence = analysis.confidence,
                    "Relationship rejected"
                );
                continue;
            }

            relationships_confirmed += 1;

            // Step 3: Activate the relationship
            match self
                .activator
                .activate_relationship(
                    candidate.memory_a_id,
                    candidate.memory_b_id,
                    &candidate.memory_a_content,
                    &candidate.memory_b_content,
                    &analysis,
                )
                .await
            {
                Ok(_) => {
                    info!(
                        memory_a = %candidate.memory_a_id,
                        memory_b = %candidate.memory_b_id,
                        relationship_type = ?analysis.relationship_type,
                        direction = ?analysis.direction,
                        confidence = analysis.confidence,
                        "Relationship activated"
                    );
                }
                Err(e) => {
                    warn!(
                        memory_a = %candidate.memory_a_id,
                        memory_b = %candidate.memory_b_id,
                        error = %e,
                        "Failed to activate relationship"
                    );
                    errors += 1;
                    error_messages.push(format!(
                        "Activation failed for {}-{}: {}",
                        candidate.memory_a_id, candidate.memory_b_id, e
                    ));
                }
            }
        }

        let completed_at = Utc::now();
        let duration = (completed_at - started_at)
            .to_std()
            .unwrap_or(Duration::ZERO);

        let stats = self.activator.stats();

        let result = DiscoveryCycleResult {
            started_at,
            completed_at,
            duration,
            candidates_found,
            relationships_confirmed,
            relationships_rejected,
            embeddings_generated: stats.embeddings_generated,
            edges_created: stats.edges_created,
            errors,
            error_messages,
        };

        info!(
            candidates_found,
            relationships_confirmed,
            relationships_rejected,
            edges_created = stats.edges_created,
            duration_ms = duration.as_millis(),
            "Graph discovery cycle complete"
        );

        *self.last_result.write() = Some(result.clone());
        Ok(result)
    }

    /// Start the background discovery loop.
    pub async fn start(self: Arc<Self>) -> GraphAgentResult<()> {
        if self.is_running() {
            return Err(GraphAgentError::ServiceAlreadyRunning);
        }

        *self.status.write() = ServiceStatus::Starting;

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        *self.shutdown_tx.write() = Some(shutdown_tx);

        self.running.store(true, Ordering::SeqCst);
        *self.status.write() = ServiceStatus::Running;

        info!(
            interval_secs = self.config.interval.as_secs(),
            "Starting graph discovery background loop"
        );

        let service = Arc::clone(&self);

        // CRIT-05 FIX: Store JoinHandle so panics are observable and stop() can await.
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("Graph discovery service received shutdown signal");
                        break;
                    }
                    _ = tokio::time::sleep(service.config.interval) => {
                        // CRIT-02 FIX: Emit warn! instead of silent debug! so operators
                        // know discovery is NOT running. The background loop is a facade:
                        // it never loads memories or calls run_discovery_cycle().
                        warn!(
                            "Graph discovery cycle tick: NOT IMPLEMENTED. \
                             Background loop is running but no discovery occurs. \
                             Use discover_graph_relationships MCP tool for on-demand analysis."
                        );
                    }
                }
            }

            service.running.store(false, Ordering::SeqCst);
            *service.status.write() = ServiceStatus::Stopped;
        });
        *self.join_handle.write() = Some(handle);

        Ok(())
    }

    /// Stop the background discovery loop.
    pub async fn stop(&self) -> GraphAgentResult<()> {
        if !self.is_running() {
            return Ok(());
        }

        *self.status.write() = ServiceStatus::Stopping;

        let tx = self.shutdown_tx.write().take();
        if let Some(tx) = tx {
            let _ = tx.send(()).await;
        }

        // CRIT-05 + HIGH-12 FIX: Replace infinite spin-loop with awaiting the
        // JoinHandle with a timeout. This detects panics and avoids busy-waiting.
        let handle = self.join_handle.write().take();
        if let Some(handle) = handle {
            match tokio::time::timeout(Duration::from_secs(10), handle).await {
                Ok(Ok(())) => info!("Graph discovery background task completed normally"),
                Ok(Err(e)) => tracing::error!("Graph discovery background task panicked: {:?}", e),
                Err(_) => tracing::error!("Graph discovery background task did not shut down within 10 seconds"),
            }
        }

        info!("Graph discovery service stopped");
        Ok(())
    }

    /// Get scanner statistics.
    pub fn scanner_analyzed_count(&self) -> usize {
        self.scanner.read().analyzed_count()
    }

    /// Reset scanner's analyzed pairs tracking.
    pub fn reset_scanner(&self) {
        self.scanner.write().clear_analyzed();
    }

    /// Get activator statistics.
    pub fn activator_stats(&self) -> crate::activator::ActivationStats {
        self.activator.stats()
    }

    /// Reset activator statistics.
    pub fn reset_activator_stats(&self) {
        self.activator.reset_stats();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = GraphDiscoveryConfig::default();
        assert_eq!(config.interval, Duration::from_secs(3600));
        assert_eq!(config.batch_size, 50);
        assert!((config.min_confidence - 0.7).abs() < 0.01);
    }

    // Integration tests would require a loaded LLM
    // which we skip in unit tests
}
