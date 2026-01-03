//! Batch queue for collecting and organizing embedding requests.
//!
//! This module provides the `BatchQueue` type which manages pending
//! requests for a single model, implementing batching logic based on
//! size and timeout thresholds.

use std::collections::VecDeque;

use crate::config::BatchConfig;
use crate::error::EmbeddingError;
use crate::types::ModelId;

use super::batch::Batch;
use super::request::BatchRequest;
use super::stats::{BatchQueueStats, BatchQueueSummary};

/// Queue of pending requests for a single model.
///
/// Each model has its own BatchQueue, allowing independent batching
/// and timeout behavior per model.
///
/// # Thread Safety
///
/// This struct is NOT Send/Sync due to the oneshot channels.
/// Wrap in Arc<Mutex<>> or use within a single task.
///
/// # Example
///
/// ```
/// # use context_graph_embeddings::batch::{BatchQueue, BatchRequest};
/// # use context_graph_embeddings::config::BatchConfig;
/// # use context_graph_embeddings::types::{ModelId, ModelInput};
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = BatchConfig::default();
/// let mut queue = BatchQueue::new(ModelId::Semantic, config);
///
/// // Add requests
/// let input = ModelInput::text("Test input")?;
/// let (request, _rx) = BatchRequest::new(input, ModelId::Semantic);
/// queue.push(request);
///
/// assert_eq!(queue.len(), 1);
///
/// // Drain batch for processing
/// if let Some(batch) = queue.drain_batch() {
///     assert_eq!(batch.len(), 1);
///     batch.fail("doc test cleanup");
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct BatchQueue {
    /// Pending requests ordered by submission time.
    requests: VecDeque<BatchRequest>,

    /// Configuration for batching behavior.
    config: BatchConfig,

    /// Model this queue serves.
    model_id: ModelId,

    /// Statistics.
    stats: BatchQueueStats,
}

impl BatchQueue {
    /// Create a new batch queue for a specific model.
    ///
    /// # Arguments
    /// * `model_id` - The model this queue serves
    /// * `config` - Batching configuration
    #[must_use]
    pub fn new(model_id: ModelId, config: BatchConfig) -> Self {
        Self {
            requests: VecDeque::new(),
            config,
            model_id,
            stats: BatchQueueStats::default(),
        }
    }

    /// Add request to queue.
    ///
    /// The request will be batched with others and processed
    /// when `should_flush()` returns true.
    pub fn push(&mut self, request: BatchRequest) {
        self.stats.record_request();
        self.requests.push_back(request);
    }

    /// Check if queue should be flushed (batch ready).
    ///
    /// Returns true if:
    /// - Queue has reached max_batch_size, OR
    /// - Oldest request has waited >= max_wait_ms
    ///
    /// Returns false if queue is empty.
    #[must_use]
    pub fn should_flush(&self) -> bool {
        if self.requests.is_empty() {
            return false;
        }

        // Flush if reached max batch size
        if self.requests.len() >= self.config.max_batch_size {
            return true;
        }

        // Flush if oldest request waited too long
        if let Some(oldest) = self.requests.front() {
            if oldest.elapsed().as_millis() as u64 >= self.config.max_wait_ms {
                return true;
            }
        }

        false
    }

    /// Extract a batch of requests for processing.
    ///
    /// Drains up to `max_batch_size` requests from the queue.
    /// If `sort_by_length` is enabled, sorts by estimated token count
    /// for padding efficiency.
    ///
    /// # Returns
    /// `Some(Batch)` if there are requests to process, `None` if queue is empty.
    pub fn drain_batch(&mut self) -> Option<Batch> {
        if self.requests.is_empty() {
            return None;
        }

        let batch_size = self.requests.len().min(self.config.max_batch_size);
        let mut batch = Batch::new(self.model_id);

        // Drain requests
        let mut requests: Vec<BatchRequest> = self.requests.drain(..batch_size).collect();

        // Calculate average wait time before moving requests
        let avg_wait_us = if !requests.is_empty() {
            let total_wait: u64 = requests
                .iter()
                .map(|r| r.elapsed().as_micros() as u64)
                .sum();
            total_wait / requests.len() as u64
        } else {
            0
        };

        // Optionally sort by length for padding efficiency
        if self.config.sort_by_length {
            requests.sort_by_key(|r| r.estimated_tokens());
        }

        // Add requests to batch
        for request in requests {
            batch.add(request);
        }

        // Update statistics
        self.stats.record_batch(batch.len(), avg_wait_us);

        Some(batch)
    }

    /// Number of pending requests.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// Check if queue is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// Oldest request wait time.
    ///
    /// # Returns
    /// `Some(Duration)` if queue is not empty, `None` if empty.
    #[must_use]
    pub fn oldest_wait_time(&self) -> Option<std::time::Duration> {
        self.requests.front().map(|r| r.elapsed())
    }

    /// Clear all pending requests with error.
    ///
    /// Sends a BatchError to all pending request channels and clears the queue.
    /// Used for graceful shutdown or error recovery.
    ///
    /// # Arguments
    /// * `message` - The error message to send to all pending requests
    pub fn cancel_all(&mut self, message: impl Into<String>) {
        let msg = message.into();
        for request in self.requests.drain(..) {
            // Ignore send errors (receiver may have dropped)
            let _ = request.response_tx.send(Err(EmbeddingError::BatchError {
                message: msg.clone(),
            }));
            self.stats.record_completion(false);
        }
    }

    /// Get the model this queue serves.
    #[inline]
    #[must_use]
    pub fn model_id(&self) -> ModelId {
        self.model_id
    }

    /// Get the current configuration.
    #[inline]
    #[must_use]
    pub fn config(&self) -> &BatchConfig {
        &self.config
    }

    /// Get queue statistics.
    #[inline]
    #[must_use]
    pub fn stats(&self) -> &BatchQueueStats {
        &self.stats
    }

    /// Get a summary of queue statistics.
    #[must_use]
    pub fn stats_summary(&self) -> BatchQueueSummary {
        self.stats.summary()
    }
}
