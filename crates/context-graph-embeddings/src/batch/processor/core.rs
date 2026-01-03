//! BatchProcessor core implementation.
//!
//! Contains the main BatchProcessor struct and construction/lifecycle methods.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Notify, RwLock, Semaphore};
use tokio::task::JoinHandle;

use crate::error::EmbeddingResult;
use crate::models::ModelRegistry;
use crate::types::ModelId;

use crate::batch::{BatchQueue, BatchRequest};

use super::config::BatchProcessorConfig;
use super::stats::{BatchProcessorStats, BatchProcessorStatsInternal};
use super::worker;

// ============================================================================
// BATCH PROCESSOR
// ============================================================================

/// Multi-model batch processor with dynamic batching.
///
/// Manages per-model queues and worker tasks that process embedding requests
/// in optimal batch sizes for GPU efficiency.
///
/// # Thread Safety
/// All operations are thread-safe. Internal state uses Arc<RwLock<>>.
///
/// # Lifecycle
/// 1. Create with `new()` - starts worker task
/// 2. Submit requests with `submit()` or `submit_batch()`
/// 3. Shutdown with `shutdown()` - waits for in-flight batches
///
/// # Example
///
/// ```
/// use context_graph_embeddings::batch::BatchProcessorConfig;
///
/// // Configure batch processing
/// let config = BatchProcessorConfig::default();
///
/// // Verify default settings
/// assert!(config.max_concurrent_batches > 0);
/// assert!(config.request_buffer_size > 0);
/// config.validate().expect("Default config should be valid");
/// ```
pub struct BatchProcessor {
    /// Model registry for accessing loaded models.
    #[allow(dead_code)]
    pub(crate) registry: Arc<ModelRegistry>,

    /// Per-model queues protected by RwLock.
    pub(crate) queues: Arc<RwLock<HashMap<ModelId, BatchQueue>>>,

    /// Configuration.
    pub(crate) config: BatchProcessorConfig,

    /// Channel for submitting requests to the worker.
    pub(crate) request_tx: mpsc::Sender<BatchRequest>,

    /// Worker task handle.
    pub(crate) worker_handle: Option<JoinHandle<()>>,

    /// Shutdown signal.
    pub(crate) shutdown_notify: Arc<Notify>,

    /// Running state.
    pub(crate) is_running: Arc<AtomicBool>,

    /// Statistics.
    pub(crate) stats: Arc<BatchProcessorStatsInternal>,

    /// Semaphore for limiting concurrent batches.
    pub(crate) batch_semaphore: Arc<Semaphore>,
}

impl BatchProcessor {
    /// Create a new BatchProcessor and start the worker task.
    ///
    /// # Arguments
    /// * `registry` - Model registry for accessing models
    /// * `config` - Processor configuration
    ///
    /// # Errors
    /// * `EmbeddingError::ConfigError` if config is invalid
    pub async fn new(
        registry: Arc<ModelRegistry>,
        config: BatchProcessorConfig,
    ) -> EmbeddingResult<Self> {
        // Validate config - FAIL FAST
        config.validate()?;

        // Create per-model queues for all 12 models
        let mut queues = HashMap::new();
        for model_id in ModelId::all() {
            queues.insert(
                *model_id,
                BatchQueue::new(*model_id, config.batch_config.clone()),
            );
        }
        let queues = Arc::new(RwLock::new(queues));

        // Create channels
        let (request_tx, request_rx) = mpsc::channel(config.request_buffer_size);

        // Create synchronization primitives
        let shutdown_notify = Arc::new(Notify::new());
        let is_running = Arc::new(AtomicBool::new(true));
        let stats = Arc::new(BatchProcessorStatsInternal::default());
        let batch_semaphore = Arc::new(Semaphore::new(config.max_concurrent_batches));

        // Clone for worker
        let worker_queues = queues.clone();
        let worker_registry = registry.clone();
        let worker_shutdown = shutdown_notify.clone();
        let worker_running = is_running.clone();
        let worker_stats = stats.clone();
        let worker_semaphore = batch_semaphore.clone();
        let poll_interval = Duration::from_millis(config.poll_interval_ms);

        // Spawn worker task
        let worker_handle = tokio::spawn(async move {
            worker::worker_loop(
                worker_queues,
                worker_registry,
                request_rx,
                worker_shutdown,
                worker_running,
                worker_stats,
                worker_semaphore,
                poll_interval,
            )
            .await;
        });

        Ok(Self {
            registry,
            queues,
            config,
            request_tx,
            worker_handle: Some(worker_handle),
            shutdown_notify,
            is_running,
            stats,
            batch_semaphore,
        })
    }

    // ========================================================================
    // QUERY METHODS
    // ========================================================================

    /// Get current queue depth for a model.
    pub async fn queue_depth(&self, model_id: ModelId) -> usize {
        let queues_guard = self.queues.read().await;
        queues_guard.get(&model_id).map(|q| q.len()).unwrap_or(0)
    }

    /// Get total queue depth across all models.
    pub async fn total_queue_depth(&self) -> usize {
        let queues_guard = self.queues.read().await;
        queues_guard.values().map(|q| q.len()).sum()
    }

    /// Get current statistics snapshot.
    pub async fn stats(&self) -> BatchProcessorStats {
        let queue_depth = self.total_queue_depth().await;
        let active = self.config.max_concurrent_batches - self.batch_semaphore.available_permits();

        let mut snapshot = self.stats.snapshot();
        snapshot.current_queue_depth = queue_depth;
        snapshot.active_batches = active;
        snapshot
    }

    /// Check if processor is running.
    #[inline]
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Get the processor configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &BatchProcessorConfig {
        &self.config
    }

    // ========================================================================
    // LIFECYCLE METHODS
    // ========================================================================

    /// Graceful shutdown - waits for in-flight batches.
    ///
    /// After calling shutdown:
    /// 1. No new requests are accepted
    /// 2. All queued requests are processed
    /// 3. All in-flight batches complete
    /// 4. Worker task terminates
    pub async fn shutdown(&mut self) {
        // Signal shutdown
        self.is_running.store(false, Ordering::Relaxed);
        self.shutdown_notify.notify_one();

        // Wait for worker to finish
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.await;
        }
    }
}

// SAFETY: All internal state protected by Arc<RwLock<>> or channels
unsafe impl Send for BatchProcessor {}
unsafe impl Sync for BatchProcessor {}

impl Drop for BatchProcessor {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        self.shutdown_notify.notify_one();
    }
}
