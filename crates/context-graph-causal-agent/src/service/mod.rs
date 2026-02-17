//! Background Causal Discovery Service.
//!
//! Coordinates the causal discovery pipeline:
//! 1. Periodically scans memories for candidate pairs
//! 2. Analyzes candidates using the local LLM
//! 3. Activates E5 embeddings for confirmed relationships
//! 4. Persists CausalRelationship records to RocksDB
//! 5. Emits audit records for provenance
//! 6. Persists cursor for restart resumption
//! 7. Adapts interval based on discovery rate

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::causal::CausalGraph;
use context_graph_core::traits::TeleologicalMemoryStore;
use context_graph_core::types::audit::{AuditOperation, AuditRecord};
use context_graph_core::types::CausalRelationship;
use context_graph_embeddings::models::CausalModel;

use crate::activator::{ActivatorConfig, E5EmbedderActivator};
use crate::error::{CausalAgentError, CausalAgentResult};
use crate::llm::{CausalDiscoveryLLM, LlmConfig};
use crate::scanner::{MemoryScanner, ScannerConfig};
use crate::types::{CausalCandidate, MemoryForAnalysis};

// ============================================================================
// CURSOR KEY
// ============================================================================

const CURSOR_KEY: &str = "causal_discovery_cursor";

// ============================================================================
// DISCOVERY CURSOR
// ============================================================================

/// Persisted cursor for the background discovery loop.
///
/// Serialized as JSON to CF_SYSTEM via `store_processing_cursor`.
/// On parse failure, the loop starts fresh (no error).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscoveryCursor {
    /// Timestamp of the last processed memory.
    pub last_timestamp: Option<DateTime<Utc>>,
    /// ID of the last processed fingerprint.
    pub last_fingerprint_id: Option<Uuid>,
    /// Number of completed cycles.
    pub cycles_completed: u64,
    /// Total relationships discovered across all cycles.
    pub total_relationships: u64,
}

// ============================================================================
// CYCLE METRICS
// ============================================================================

/// Metrics from a single background discovery cycle.
#[derive(Debug, Clone, Default)]
pub struct CycleMetrics {
    /// Which cycle number this was.
    pub cycle_number: u64,
    /// How many memories were harvested from the store.
    pub memories_harvested: usize,
    /// How many relationships were discovered (confirmed by LLM).
    pub relationships_discovered: usize,
    /// How many relationships were rejected by LLM.
    pub relationships_rejected: usize,
    /// Total wall-clock duration of the cycle.
    pub total_duration: Duration,
}

// ============================================================================
// CONFIG
// ============================================================================

/// Configuration for the Causal Discovery Service.
#[derive(Debug, Clone)]
pub struct CausalDiscoveryConfig {
    /// Initial interval between discovery cycles.
    pub interval: Duration,

    /// Maximum pairs to analyze per cycle.
    pub batch_size: usize,

    /// Minimum LLM confidence to accept a relationship.
    pub min_confidence: f32,

    /// Whether to skip already-analyzed pairs.
    pub skip_analyzed: bool,

    /// Whether background discovery is enabled.
    pub enable_background: bool,

    /// Minimum interval (adaptive floor).
    pub min_interval: Duration,

    /// Maximum interval (adaptive ceiling).
    pub max_interval: Duration,

    /// Max consecutive errors before long pause.
    pub max_consecutive_errors: u32,

    /// LLM configuration (includes model path).
    pub llm_config: LlmConfig,

    /// Scanner configuration.
    pub scanner_config: ScannerConfig,

    /// Activator configuration.
    pub activator_config: ActivatorConfig,
}

impl Default for CausalDiscoveryConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(120), // 2 minutes
            batch_size: 100,
            min_confidence: 0.7,
            skip_analyzed: true,
            enable_background: false, // Explicit opt-in
            min_interval: Duration::from_secs(30),
            max_interval: Duration::from_secs(600),
            max_consecutive_errors: 3,
            llm_config: LlmConfig::default(),
            scanner_config: ScannerConfig::default(),
            activator_config: ActivatorConfig::default(),
        }
    }
}

impl CausalDiscoveryConfig {
    /// Load config overrides from environment variables.
    pub fn with_env_overrides(mut self) -> Self {
        if let Ok(val) = std::env::var("CAUSAL_DISCOVERY_ENABLED") {
            self.enable_background = val == "true" || val == "1";
        }
        if let Ok(val) = std::env::var("CAUSAL_DISCOVERY_INTERVAL_SECS") {
            if let Ok(secs) = val.parse::<u64>() {
                self.interval = Duration::from_secs(secs);
            }
        }
        if let Ok(val) = std::env::var("CAUSAL_DISCOVERY_BATCH_SIZE") {
            if let Ok(size) = val.parse::<usize>() {
                self.batch_size = size;
            }
        }
        if let Ok(val) = std::env::var("CAUSAL_DISCOVERY_MIN_CONFIDENCE") {
            if let Ok(conf) = val.parse::<f32>() {
                self.min_confidence = conf;
            }
        }
        self
    }
}

// Re-export shared type from core.
pub use context_graph_core::types::DiscoveryCycleResult;

pub use context_graph_core::types::ServiceStatus;

// ============================================================================
// SERVICE
// ============================================================================

/// Background service for causal discovery.
pub struct CausalDiscoveryService {
    config: CausalDiscoveryConfig,
    llm: Arc<CausalDiscoveryLLM>,
    scanner: RwLock<MemoryScanner>,
    activator: Arc<E5EmbedderActivator>,
    causal_graph: Arc<RwLock<CausalGraph>>,
    running: AtomicBool,
    status: RwLock<ServiceStatus>,
    last_result: RwLock<Option<DiscoveryCycleResult>>,
    shutdown_tx: RwLock<Option<mpsc::Sender<()>>>,
    /// CRIT-05 FIX: Store background task JoinHandle so panics are not silently lost.
    join_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
}

impl CausalDiscoveryService {
    /// Create a new service with the given configuration (no model).
    #[deprecated(
        since = "0.1.0",
        note = "Use with_models() for production. This constructor creates E5EmbedderActivator without CausalModel."
    )]
    pub async fn new(config: CausalDiscoveryConfig) -> CausalAgentResult<Self> {
        let llm = CausalDiscoveryLLM::with_config(config.llm_config.clone())?;
        let llm = Arc::new(llm);
        let causal_graph = Arc::new(RwLock::new(CausalGraph::new()));
        let scanner = MemoryScanner::with_config(config.scanner_config.clone());
        let activator_config = ActivatorConfig {
            min_confidence: config.min_confidence,
            ..config.activator_config.clone()
        };
        let activator = E5EmbedderActivator::with_config(causal_graph.clone(), activator_config);
        let activator = Arc::new(activator);

        Ok(Self {
            config,
            llm,
            scanner: RwLock::new(scanner),
            activator,
            causal_graph,
            running: AtomicBool::new(false),
            status: RwLock::new(ServiceStatus::Stopped),
            last_result: RwLock::new(None),
            shutdown_tx: RwLock::new(None),
            join_handle: RwLock::new(None),
        })
    }

    /// Create a new service with models for production use.
    pub fn with_models(
        shared_llm: Arc<CausalDiscoveryLLM>,
        causal_model: Arc<CausalModel>,
        config: CausalDiscoveryConfig,
    ) -> Self {
        let causal_graph = Arc::new(RwLock::new(CausalGraph::new()));
        let scanner = MemoryScanner::with_config(config.scanner_config.clone());
        let activator_config = ActivatorConfig {
            min_confidence: config.min_confidence,
            ..config.activator_config.clone()
        };
        let activator = E5EmbedderActivator::with_model(
            causal_graph.clone(),
            causal_model,
            activator_config,
        );
        let activator = Arc::new(activator);

        Self {
            config,
            llm: shared_llm,
            scanner: RwLock::new(scanner),
            activator,
            causal_graph,
            running: AtomicBool::new(false),
            status: RwLock::new(ServiceStatus::Stopped),
            last_result: RwLock::new(None),
            shutdown_tx: RwLock::new(None),
            join_handle: RwLock::new(None),
        }
    }

    /// Load the LLM model.
    pub async fn load_model(&self) -> CausalAgentResult<()> {
        info!("Loading LLM model for causal discovery");
        self.llm.load().await
    }

    /// Unload the LLM model.
    pub async fn unload_model(&self) -> CausalAgentResult<()> {
        info!("Unloading LLM model");
        self.llm.unload().await
    }

    /// Check if the LLM model is loaded.
    pub fn is_model_loaded(&self) -> bool {
        self.llm.is_loaded()
    }

    // ========================================================================
    // DISCOVERY CYCLE (existing, now accepts optional store for persistence)
    // ========================================================================

    /// Run a single discovery cycle.
    ///
    /// When `store` is `Some`, confirmed relationships are persisted to RocksDB.
    /// When `None`, only the in-memory CausalGraph is updated (test/on-demand mode).
    pub async fn run_discovery_cycle(
        &self,
        memories: &[MemoryForAnalysis],
        store: Option<&Arc<dyn TeleologicalMemoryStore>>,
    ) -> CausalAgentResult<DiscoveryCycleResult> {
        let started_at = Utc::now();
        let start = Instant::now();

        let mut result = DiscoveryCycleResult {
            started_at,
            ..Default::default()
        };

        // Ensure model is loaded
        if !self.llm.is_loaded() {
            self.llm.load().await?;
        }

        // 1. Find candidate pairs
        let candidates = {
            let mut scanner = self.scanner.write();
            match scanner.find_candidates(memories) {
                Ok(c) => c,
                Err(CausalAgentError::NoCandidatesFound) => {
                    info!("No candidate pairs found, skipping cycle");
                    result.completed_at = Utc::now();
                    result.duration = start.elapsed();
                    return Ok(result);
                }
                Err(e) => return Err(e),
            }
        };

        result.candidates_found = candidates.len();
        info!(
            candidates = candidates.len(),
            batch_size = self.config.batch_size,
            "Found candidate pairs"
        );

        // 2. Analyze candidates with LLM and persist
        let batch: Vec<_> = candidates.iter().take(self.config.batch_size).collect();

        for candidate in batch {
            match self.process_candidate_with_store(candidate, store).await {
                Ok(confirmed) => {
                    if confirmed {
                        result.relationships_confirmed += 1;
                        result.embeddings_generated += 2; // cause + effect
                        result.edges_created += 1;
                    } else {
                        result.relationships_rejected += 1;
                    }
                }
                Err(e) => {
                    warn!(
                        cause = %candidate.cause_memory_id,
                        effect = %candidate.effect_memory_id,
                        error = %e,
                        "Failed to process candidate"
                    );
                    result.errors += 1;
                    result.error_messages.push(e.to_string());
                }
            }
        }

        result.completed_at = Utc::now();
        result.duration = start.elapsed();

        // Store result
        *self.last_result.write() = Some(result.clone());

        info!(
            confirmed = result.relationships_confirmed,
            rejected = result.relationships_rejected,
            errors = result.errors,
            duration_ms = result.duration.as_millis(),
            "Discovery cycle complete"
        );

        Ok(result)
    }

    /// Process a single candidate pair, optionally persisting to RocksDB.
    async fn process_candidate_with_store(
        &self,
        candidate: &CausalCandidate,
        store: Option<&Arc<dyn TeleologicalMemoryStore>>,
    ) -> CausalAgentResult<bool> {
        debug!(
            cause = %candidate.cause_memory_id,
            effect = %candidate.effect_memory_id,
            initial_score = candidate.initial_score,
            "Processing candidate"
        );

        // Analyze with LLM
        let analysis = self
            .llm
            .analyze_causal_relationship(&candidate.cause_content, &candidate.effect_content)
            .await?;

        if !analysis.has_causal_link || analysis.confidence < self.config.min_confidence {
            debug!(
                has_link = analysis.has_causal_link,
                confidence = analysis.confidence,
                "Rejected candidate"
            );
            return Ok(false);
        }

        // Activate E5 embedding - returns (cause_vec, effect_vec)
        let (cause_vec, effect_vec) = self
            .activator
            .activate_relationship(
                candidate.cause_memory_id,
                candidate.effect_memory_id,
                &candidate.cause_content,
                &candidate.effect_content,
                &analysis,
            )
            .await?;

        // Persist to RocksDB if store is provided
        if let Some(store) = store {
            let mechanism_type = analysis
                .mechanism_type
                .as_ref()
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(|| "direct".to_string());

            // Build E1 semantic embedding for fallback search.
            // Use a zero vector since E1 embedding is generated separately
            // during store_causal_relationship by the storage layer.
            let e1_semantic = vec![0.0f32; 1024];

            let relationship = CausalRelationship::new(
                candidate.cause_content.chars().take(500).collect::<String>(),
                candidate.effect_content.chars().take(500).collect::<String>(),
                analysis.mechanism.clone(),
                cause_vec,
                effect_vec,
                e1_semantic,
                format!("{}\n---\n{}", candidate.cause_content, candidate.effect_content),
                candidate.cause_memory_id,
                analysis.confidence,
                mechanism_type,
            );

            match store.store_causal_relationship(&relationship).await {
                Ok(id) => {
                    info!(
                        id = %id,
                        cause = %candidate.cause_memory_id,
                        effect = %candidate.effect_memory_id,
                        confidence = analysis.confidence,
                        "Persisted CausalRelationship to RocksDB"
                    );
                }
                Err(e) => {
                    error!(
                        cause = %candidate.cause_memory_id,
                        effect = %candidate.effect_memory_id,
                        error = %e,
                        "Failed to persist CausalRelationship to RocksDB"
                    );
                    return Err(CausalAgentError::StorageError {
                        message: format!("Failed to persist relationship: {e}"),
                    });
                }
            }
        }

        Ok(true)
    }

    // ========================================================================
    // BACKGROUND LOOP
    // ========================================================================

    /// Start the background discovery loop.
    ///
    /// # Breaking change
    /// Now requires a `store` parameter to fetch memories and persist results.
    pub async fn start(
        self: Arc<Self>,
        store: Arc<dyn TeleologicalMemoryStore>,
    ) -> CausalAgentResult<()> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(CausalAgentError::ServiceAlreadyRunning);
        }

        *self.status.write() = ServiceStatus::Starting;

        info!(
            interval_secs = self.config.interval.as_secs(),
            batch_size = self.config.batch_size,
            enable_background = self.config.enable_background,
            "Starting causal discovery service"
        );

        if !self.config.enable_background {
            info!("Background discovery disabled (CAUSAL_DISCOVERY_ENABLED != true). Loop will not run.");
            *self.status.write() = ServiceStatus::Running;
            self.running.store(false, Ordering::SeqCst);
            *self.status.write() = ServiceStatus::Stopped;
            return Ok(());
        }

        // Create shutdown channel
        let (tx, mut rx) = mpsc::channel(1);
        *self.shutdown_tx.write() = Some(tx);

        // Load model
        self.llm.load().await?;

        *self.status.write() = ServiceStatus::Running;

        // Load cursor from store
        let initial_cursor = self.load_cursor(&store).await;

        // CRIT-05 FIX: Store JoinHandle so panics are observable and stop() can await.
        let service = self.clone();
        let handle = tokio::spawn(async move {
            let mut cursor = initial_cursor;
            let mut consecutive_errors: u32 = 0;
            let mut current_interval = service.config.interval;

            loop {
                tokio::select! {
                    _ = rx.recv() => {
                        info!("Received shutdown signal");
                        break;
                    }
                    _ = tokio::time::sleep(current_interval) => {
                        match service.run_background_tick(&store, &mut cursor).await {
                            Ok(metrics) => {
                                consecutive_errors = 0;
                                current_interval = service.compute_next_interval(&metrics);

                                // Non-fatal cursor save per ARCH-PROV-01
                                if let Err(e) = service.save_cursor(&store, &cursor).await {
                                    warn!(error = %e, "Cursor save failed (non-fatal)");
                                }

                                info!(
                                    cycle = metrics.cycle_number,
                                    harvested = metrics.memories_harvested,
                                    discovered = metrics.relationships_discovered,
                                    rejected = metrics.relationships_rejected,
                                    duration_ms = metrics.total_duration.as_millis(),
                                    next_interval_s = current_interval.as_secs(),
                                    "Causal discovery cycle complete"
                                );
                            }
                            Err(e) => {
                                consecutive_errors += 1;
                                error!(
                                    error = %e,
                                    consecutive = consecutive_errors,
                                    "Causal discovery cycle failed"
                                );

                                if consecutive_errors >= service.config.max_consecutive_errors {
                                    error!(
                                        max = service.config.max_consecutive_errors,
                                        "Too many consecutive failures, pausing for 1 hour"
                                    );
                                    current_interval = Duration::from_secs(3600);
                                } else {
                                    // Exponential backoff: 60s, 120s, 240s...
                                    current_interval = Duration::from_secs(
                                        60 * 2u64.pow(consecutive_errors - 1)
                                    );
                                }
                            }
                        }
                    }
                }
            }

            *service.status.write() = ServiceStatus::Stopped;
            service.running.store(false, Ordering::SeqCst);
        });
        *self.join_handle.write() = Some(handle);

        Ok(())
    }

    /// Execute one tick of the background discovery loop.
    async fn run_background_tick(
        &self,
        store: &Arc<dyn TeleologicalMemoryStore>,
        cursor: &mut DiscoveryCursor,
    ) -> CausalAgentResult<CycleMetrics> {
        let cycle_start = Instant::now();
        let cycle_number = cursor.cycles_completed + 1;

        // Phase 1: Harvest memories from store
        let memories = self.harvest_memories(store).await?;
        if memories.is_empty() {
            cursor.cycles_completed = cycle_number;
            return Ok(CycleMetrics {
                cycle_number,
                memories_harvested: 0,
                total_duration: cycle_start.elapsed(),
                ..Default::default()
            });
        }

        // Phase 2+3+4: Scan + Analyze + Activate + Persist
        let result = self
            .run_discovery_cycle(&memories, Some(store))
            .await?;

        // Phase 5: Emit audit record (non-fatal per ARCH-PROV-01)
        let audit_params = serde_json::json!({
            "cycle": cycle_number,
            "discovered": result.relationships_confirmed,
            "rejected": result.relationships_rejected,
            "harvested": memories.len(),
            "duration_ms": cycle_start.elapsed().as_millis() as u64,
        });
        let audit_record = AuditRecord::new(
            AuditOperation::RelationshipDiscovered {
                relationship_type: "causal".to_string(),
                confidence: if result.relationships_confirmed > 0 {
                    // Average confidence not available here; use 1.0 as "cycle completed"
                    1.0
                } else {
                    0.0
                },
            },
            Uuid::nil(), // targets the CF, not a single entity
        )
        .with_rationale("background_discovery_loop")
        .with_parameters(audit_params);

        if let Err(e) = store.append_audit_record(&audit_record).await {
            warn!(error = %e, "Audit write failed (non-fatal per ARCH-PROV-01)");
        }

        // Update cursor
        if let Some(last) = memories.last() {
            cursor.last_timestamp = Some(last.created_at);
            cursor.last_fingerprint_id = Some(last.id);
        }
        cursor.cycles_completed = cycle_number;
        cursor.total_relationships += result.relationships_confirmed as u64;

        Ok(CycleMetrics {
            cycle_number,
            memories_harvested: memories.len(),
            relationships_discovered: result.relationships_confirmed,
            relationships_rejected: result.relationships_rejected,
            total_duration: cycle_start.elapsed(),
        })
    }

    // ========================================================================
    // HARVEST MEMORIES
    // ========================================================================

    /// Fetch memories from the store for causal discovery.
    async fn harvest_memories(
        &self,
        store: &Arc<dyn TeleologicalMemoryStore>,
    ) -> CausalAgentResult<Vec<MemoryForAnalysis>> {
        let fingerprints = store
            .scan_fingerprints_for_clustering(Some(self.config.batch_size))
            .await
            .map_err(|e| CausalAgentError::StorageError {
                message: format!("scan_fingerprints_for_clustering failed: {e}"),
            })?;

        let mut memories = Vec::with_capacity(fingerprints.len());
        for (id, embeddings) in fingerprints {
            let content = match store.get_content(id).await {
                Ok(Some(c)) if !c.is_empty() => c,
                Ok(Some(_)) => {
                    warn!(id = %id, "Fingerprint has empty content, skipping");
                    continue;
                }
                Ok(None) => {
                    warn!(id = %id, "Fingerprint has no content, skipping");
                    continue;
                }
                Err(e) => {
                    warn!(id = %id, error = %e, "Failed to get content, skipping");
                    continue;
                }
            };

            // Get created_at from source metadata if available
            let created_at = match store.get_source_metadata(id).await {
                Ok(Some(meta)) => meta.created_at.unwrap_or_else(Utc::now),
                _ => Utc::now(),
            };

            // LOW-7 FIX: Validate embeddings[0] exists and has correct E1 dimension (1024D).
            // Previously assumed index 0 was E1 without bounds check — empty vec would panic.
            if embeddings.is_empty() {
                warn!(id = %id, "Fingerprint has no embeddings, skipping");
                continue;
            }
            if embeddings[0].len() != 1024 {
                warn!(
                    id = %id,
                    dim = embeddings[0].len(),
                    "Fingerprint E1 embedding has wrong dimension (expected 1024), skipping"
                );
                continue;
            }

            memories.push(MemoryForAnalysis {
                id,
                content,
                e1_embedding: embeddings[0].clone(), // E1 is index 0 (1024D)
                created_at,
                session_id: None,
            });
        }

        info!(count = memories.len(), "Harvested memories for causal discovery");
        Ok(memories)
    }

    // ========================================================================
    // ADAPTIVE INTERVAL
    // ========================================================================

    /// Compute the next sleep interval based on discovery metrics.
    pub fn compute_next_interval(&self, metrics: &CycleMetrics) -> Duration {
        let raw = if metrics.memories_harvested == 0 {
            Duration::from_secs(600) // 10 min: nothing to process
        } else if metrics.relationships_discovered == 0 {
            Duration::from_secs(300) // 5 min: content but no causation
        } else if metrics.relationships_discovered <= 5 {
            Duration::from_secs(120) // 2 min: moderate discovery
        } else {
            Duration::from_secs(30) // 30s: heavy causal content
        };
        raw.max(self.config.min_interval)
            .min(self.config.max_interval)
    }

    // ========================================================================
    // CURSOR PERSISTENCE
    // ========================================================================

    /// Save the cursor to the store.
    async fn save_cursor(
        &self,
        store: &Arc<dyn TeleologicalMemoryStore>,
        cursor: &DiscoveryCursor,
    ) -> CausalAgentResult<()> {
        let json = serde_json::to_vec(cursor).map_err(|e| CausalAgentError::ParseError {
            message: format!("Failed to serialize cursor: {e}"),
        })?;
        store
            .store_processing_cursor(CURSOR_KEY, &json)
            .await
            .map_err(|e| CausalAgentError::StorageError {
                message: format!("Failed to store cursor: {e}"),
            })?;
        Ok(())
    }

    /// Load the cursor from the store.
    async fn load_cursor(
        &self,
        store: &Arc<dyn TeleologicalMemoryStore>,
    ) -> DiscoveryCursor {
        match store.get_processing_cursor(CURSOR_KEY).await {
            Ok(Some(bytes)) => {
                match serde_json::from_slice::<DiscoveryCursor>(&bytes) {
                    Ok(cursor) => {
                        info!(
                            cycles = cursor.cycles_completed,
                            relationships = cursor.total_relationships,
                            "Loaded cursor from store"
                        );
                        cursor
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to parse cursor, starting fresh");
                        DiscoveryCursor::default()
                    }
                }
            }
            Ok(None) => {
                info!("No cursor found, starting fresh");
                DiscoveryCursor::default()
            }
            Err(e) => {
                warn!(error = %e, "Failed to load cursor, starting fresh");
                DiscoveryCursor::default()
            }
        }
    }

    // ========================================================================
    // STOP / STATUS / ACCESSORS
    // ========================================================================

    /// Stop the background discovery loop.
    pub async fn stop(&self) -> CausalAgentResult<()> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        *self.status.write() = ServiceStatus::Stopping;
        info!("Stopping causal discovery service");

        // CRIT-04 FIX: Clone sender out of parking_lot scope before .await
        let tx_clone = {
            let shutdown = self.shutdown_tx.read();
            shutdown.as_ref().cloned()
        };
        if let Some(tx) = tx_clone {
            let _ = tx.send(()).await;
        }

        // Unload model
        self.llm.unload().await?;

        // CRIT-05 FIX: Await JoinHandle with timeout
        let handle = self.join_handle.write().take();
        if let Some(handle) = handle {
            match tokio::time::timeout(Duration::from_secs(10), handle).await {
                Ok(Ok(())) => info!("Causal discovery background task completed normally"),
                Ok(Err(e)) => error!("Causal discovery background task panicked: {:?}", e),
                Err(_) => error!("Causal discovery background task did not shut down within 10 seconds"),
            }
        }

        Ok(())
    }

    pub fn status(&self) -> ServiceStatus {
        *self.status.read()
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn last_result(&self) -> Option<DiscoveryCycleResult> {
        self.last_result.read().clone()
    }

    pub fn causal_graph(&self) -> &Arc<RwLock<CausalGraph>> {
        &self.causal_graph
    }

    pub fn config(&self) -> &CausalDiscoveryConfig {
        &self.config
    }

    pub fn estimated_vram_mb(&self) -> usize {
        self.llm.estimated_vram_mb()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> CausalDiscoveryConfig {
        let mut workspace_root = std::env::current_dir().unwrap();
        while !workspace_root.join("Cargo.toml").exists()
            || !workspace_root.join("models").exists()
        {
            if !workspace_root.pop() {
                workspace_root = std::path::PathBuf::from(".");
                break;
            }
        }

        let model_dir = workspace_root.join("models/hermes-2-pro");

        let llm_config = crate::llm::LlmConfig {
            model_path: model_dir.join("Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf"),
            causal_grammar_path: model_dir.join("causal_analysis.gbnf"),
            graph_grammar_path: model_dir.join("graph_relationship.gbnf"),
            validation_grammar_path: model_dir.join("validation.gbnf"),
            ..Default::default()
        };

        CausalDiscoveryConfig {
            interval: Duration::from_secs(1),
            batch_size: 10,
            min_confidence: 0.6,
            llm_config,
            ..Default::default()
        }
    }

    fn create_test_memory(id: u128, content: &str, hours_ago: i64) -> MemoryForAnalysis {
        use chrono::TimeDelta;

        MemoryForAnalysis {
            id: Uuid::from_u128(id),
            content: content.to_string(),
            created_at: Utc::now() - TimeDelta::hours(hours_ago),
            session_id: Some("test-session".to_string()),
            e1_embedding: vec![0.7; 1024],
        }
    }

    #[tokio::test]
    async fn test_service_creation() {
        let config = create_test_config();
        #[allow(deprecated)]
        let service = CausalDiscoveryService::new(config).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_discovery_cycle() {
        let config = create_test_config();

        if !config.llm_config.model_path.exists() {
            eprintln!(
                "Skipping test_discovery_cycle: model not found at {:?}",
                config.llm_config.model_path
            );
            return;
        }

        #[allow(deprecated)]
        let service = CausalDiscoveryService::new(config).await.unwrap();
        service.load_model().await.unwrap();

        let memories = vec![
            create_test_memory(1, "Because of the error, the system crashed", 2),
            create_test_memory(2, "Therefore, users were affected", 1),
        ];

        let result = service.run_discovery_cycle(&memories, None).await.unwrap();
        assert!(result.errors == 0);
    }

    #[tokio::test]
    async fn test_service_status() {
        let config = create_test_config();
        #[allow(deprecated)]
        let service = CausalDiscoveryService::new(config).await.unwrap();

        assert_eq!(service.status(), ServiceStatus::Stopped);
        assert!(!service.is_running());
    }

    #[allow(deprecated)]
    fn default_service() -> CausalDiscoveryService {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            CausalDiscoveryService::new(CausalDiscoveryConfig::default()).await.unwrap()
        })
    }

    #[test]
    fn test_adaptive_interval_empty() {
        let service = default_service();
        let metrics = CycleMetrics {
            memories_harvested: 0,
            ..Default::default()
        };
        assert_eq!(service.compute_next_interval(&metrics), Duration::from_secs(600));
    }

    #[test]
    fn test_adaptive_interval_no_discovery() {
        let service = default_service();
        let metrics = CycleMetrics {
            memories_harvested: 50,
            relationships_discovered: 0,
            ..Default::default()
        };
        assert_eq!(service.compute_next_interval(&metrics), Duration::from_secs(300));
    }

    #[test]
    fn test_adaptive_interval_heavy() {
        let service = default_service();
        let metrics = CycleMetrics {
            memories_harvested: 50,
            relationships_discovered: 10,
            ..Default::default()
        };
        assert_eq!(service.compute_next_interval(&metrics), Duration::from_secs(30));
    }

    #[test]
    fn test_cursor_serialization() {
        let cursor = DiscoveryCursor {
            last_timestamp: Some(Utc::now()),
            last_fingerprint_id: Some(Uuid::new_v4()),
            cycles_completed: 42,
            total_relationships: 100,
        };
        let json = serde_json::to_vec(&cursor).unwrap();
        let restored: DiscoveryCursor = serde_json::from_slice(&json).unwrap();
        assert_eq!(restored.cycles_completed, 42);
        assert_eq!(restored.total_relationships, 100);
    }

    #[test]
    fn test_config_env_overrides() {
        std::env::set_var("CAUSAL_DISCOVERY_ENABLED", "true");
        std::env::set_var("CAUSAL_DISCOVERY_INTERVAL_SECS", "60");
        std::env::set_var("CAUSAL_DISCOVERY_BATCH_SIZE", "200");

        let config = CausalDiscoveryConfig::default().with_env_overrides();
        assert!(config.enable_background);
        assert_eq!(config.interval, Duration::from_secs(60));
        assert_eq!(config.batch_size, 200);

        std::env::remove_var("CAUSAL_DISCOVERY_ENABLED");
        std::env::remove_var("CAUSAL_DISCOVERY_INTERVAL_SECS");
        std::env::remove_var("CAUSAL_DISCOVERY_BATCH_SIZE");
    }
}
