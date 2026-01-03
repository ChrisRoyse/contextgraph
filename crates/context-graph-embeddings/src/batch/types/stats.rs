//! Batch queue statistics and summary types.
//!
//! This module provides statistics tracking for batch queue operations,
//! using atomics for thread-safe concurrent updates.

use std::sync::atomic::{AtomicU64, Ordering};

/// Queue statistics for monitoring and debugging.
///
/// All fields use atomics for thread-safe concurrent updates.
///
/// # Example
///
/// ```
/// # use context_graph_embeddings::batch::BatchQueueStats;
/// let stats = BatchQueueStats::default();
/// stats.record_request();
/// stats.record_batch(10, 5000); // 10 items, 5ms wait
///
/// let summary = stats.summary();
/// assert_eq!(summary.batches_processed, 1);
/// assert_eq!(summary.requests_received, 1);
/// ```
#[derive(Debug, Default)]
pub struct BatchQueueStats {
    /// Total requests received.
    pub requests_received: AtomicU64,

    /// Total batches processed.
    pub batches_processed: AtomicU64,

    /// Total requests completed successfully.
    pub requests_completed: AtomicU64,

    /// Total requests that failed.
    pub requests_failed: AtomicU64,

    /// Cumulative wait time in microseconds.
    pub total_wait_time_us: AtomicU64,

    /// Running sum for average batch size calculation.
    /// Stored as (sum * 1000) to preserve precision.
    batch_size_sum: AtomicU64,
}

impl Clone for BatchQueueStats {
    fn clone(&self) -> Self {
        Self {
            requests_received: AtomicU64::new(self.requests_received.load(Ordering::Relaxed)),
            batches_processed: AtomicU64::new(self.batches_processed.load(Ordering::Relaxed)),
            requests_completed: AtomicU64::new(self.requests_completed.load(Ordering::Relaxed)),
            requests_failed: AtomicU64::new(self.requests_failed.load(Ordering::Relaxed)),
            total_wait_time_us: AtomicU64::new(self.total_wait_time_us.load(Ordering::Relaxed)),
            batch_size_sum: AtomicU64::new(self.batch_size_sum.load(Ordering::Relaxed)),
        }
    }
}

impl BatchQueueStats {
    /// Record a new request received.
    #[inline]
    pub fn record_request(&self) {
        self.requests_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a batch processed.
    ///
    /// # Arguments
    /// * `size` - Number of requests in the batch
    /// * `wait_time_us` - Average wait time in microseconds
    #[inline]
    pub fn record_batch(&self, size: usize, wait_time_us: u64) {
        self.batches_processed.fetch_add(1, Ordering::Relaxed);
        self.batch_size_sum
            .fetch_add(size as u64, Ordering::Relaxed);
        self.total_wait_time_us
            .fetch_add(wait_time_us, Ordering::Relaxed);
    }

    /// Record a request completion.
    ///
    /// # Arguments
    /// * `success` - Whether the request completed successfully
    #[inline]
    pub fn record_completion(&self, success: bool) {
        if success {
            self.requests_completed.fetch_add(1, Ordering::Relaxed);
        } else {
            self.requests_failed.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Get a summary snapshot of current statistics.
    #[must_use]
    pub fn summary(&self) -> BatchQueueSummary {
        let batches = self.batches_processed.load(Ordering::Relaxed);
        let size_sum = self.batch_size_sum.load(Ordering::Relaxed);
        let wait_sum = self.total_wait_time_us.load(Ordering::Relaxed);

        BatchQueueSummary {
            requests_received: self.requests_received.load(Ordering::Relaxed),
            batches_processed: batches,
            requests_completed: self.requests_completed.load(Ordering::Relaxed),
            requests_failed: self.requests_failed.load(Ordering::Relaxed),
            avg_batch_size: if batches > 0 {
                (size_sum as f64) / (batches as f64)
            } else {
                0.0
            },
            avg_wait_time_us: if batches > 0 { wait_sum / batches } else { 0 },
        }
    }

    /// Reset all statistics to zero.
    pub fn reset(&self) {
        self.requests_received.store(0, Ordering::Relaxed);
        self.batches_processed.store(0, Ordering::Relaxed);
        self.requests_completed.store(0, Ordering::Relaxed);
        self.requests_failed.store(0, Ordering::Relaxed);
        self.total_wait_time_us.store(0, Ordering::Relaxed);
        self.batch_size_sum.store(0, Ordering::Relaxed);
    }
}

/// Summary snapshot of queue statistics.
///
/// This is a non-atomic copy for reporting purposes.
#[derive(Debug, Clone, PartialEq)]
pub struct BatchQueueSummary {
    /// Total requests received.
    pub requests_received: u64,

    /// Total batches processed.
    pub batches_processed: u64,

    /// Total requests completed successfully.
    pub requests_completed: u64,

    /// Total requests that failed.
    pub requests_failed: u64,

    /// Average batch size (floating point for precision).
    pub avg_batch_size: f64,

    /// Average wait time in microseconds.
    pub avg_wait_time_us: u64,
}
