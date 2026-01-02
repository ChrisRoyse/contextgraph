//! GPU memory management for RTX 5090 32GB VRAM.
//!
//! # Design
//!
//! Memory management strategy for 32GB VRAM:
//!
//! | Pool | Size | Purpose |
//! |------|------|---------|
//! | Model Weights | 16GB | Pretrained model parameters |
//! | Activation Cache | 8GB | Intermediate activations |
//! | Working Memory | 6GB | Batch processing buffers |
//! | Reserved | 2GB | System overhead, fragmentation |
//!
//! # Usage
//!
//! ```rust,ignore
//! use context_graph_embeddings::gpu::{GpuMemoryPool, VramTracker};
//!
//! let mut tracker = VramTracker::new(32 * 1024 * 1024 * 1024);
//! tracker.allocate("model_weights", 16 * 1024 * 1024 * 1024)?;
//! println!("Available: {} GB", tracker.available() / (1024 * 1024 * 1024));
//! ```

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Memory statistics for monitoring.
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    /// Total VRAM capacity in bytes.
    pub total_bytes: usize,
    /// Currently allocated bytes.
    pub allocated_bytes: usize,
    /// Peak allocation (high water mark).
    pub peak_bytes: usize,
    /// Number of allocations.
    pub allocation_count: usize,
    /// Number of deallocations.
    pub deallocation_count: usize,
}

impl MemoryStats {
    /// Get available memory in bytes.
    pub fn available(&self) -> usize {
        self.total_bytes.saturating_sub(self.allocated_bytes)
    }

    /// Get memory utilization as percentage.
    pub fn utilization_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.allocated_bytes as f32 / self.total_bytes as f32) * 100.0
        }
    }
}

/// VRAM allocation tracker.
///
/// Tracks named allocations for debugging and monitoring.
#[derive(Debug)]
pub struct VramTracker {
    /// Total VRAM capacity.
    total_bytes: usize,
    /// Named allocations.
    allocations: HashMap<String, usize>,
    /// Statistics.
    stats: MemoryStats,
}

impl VramTracker {
    /// Create a new VRAM tracker with given capacity.
    ///
    /// # Arguments
    ///
    /// * `total_bytes` - Total VRAM capacity (e.g., 32GB for RTX 5090)
    pub fn new(total_bytes: usize) -> Self {
        Self {
            total_bytes,
            allocations: HashMap::new(),
            stats: MemoryStats {
                total_bytes,
                ..Default::default()
            },
        }
    }

    /// Create tracker for RTX 5090 (32GB).
    pub fn rtx_5090() -> Self {
        Self::new(32 * 1024 * 1024 * 1024)
    }

    /// Allocate memory with a name for tracking.
    ///
    /// # Arguments
    ///
    /// * `name` - Identifier for the allocation (e.g., "semantic_model")
    /// * `bytes` - Number of bytes to allocate
    ///
    /// # Returns
    ///
    /// Ok if allocation succeeded, Err if insufficient memory.
    pub fn allocate(&mut self, name: &str, bytes: usize) -> Result<(), MemoryError> {
        if self.stats.available() < bytes {
            return Err(MemoryError::OutOfMemory {
                requested: bytes,
                available: self.stats.available(),
            });
        }

        self.allocations.insert(name.to_string(), bytes);
        self.stats.allocated_bytes += bytes;
        self.stats.allocation_count += 1;
        self.stats.peak_bytes = self.stats.peak_bytes.max(self.stats.allocated_bytes);

        tracing::debug!(
            "GPU allocated '{}': {} bytes ({:.1}% used)",
            name,
            bytes,
            self.stats.utilization_percent()
        );

        Ok(())
    }

    /// Deallocate memory by name.
    ///
    /// # Returns
    ///
    /// Number of bytes freed, or 0 if name not found.
    pub fn deallocate(&mut self, name: &str) -> usize {
        if let Some(bytes) = self.allocations.remove(name) {
            self.stats.allocated_bytes = self.stats.allocated_bytes.saturating_sub(bytes);
            self.stats.deallocation_count += 1;

            tracing::debug!(
                "GPU deallocated '{}': {} bytes ({:.1}% used)",
                name,
                bytes,
                self.stats.utilization_percent()
            );

            bytes
        } else {
            0
        }
    }

    /// Get current memory statistics.
    pub fn stats(&self) -> &MemoryStats {
        &self.stats
    }

    /// Get available memory in bytes.
    pub fn available(&self) -> usize {
        self.stats.available()
    }

    /// List all allocations.
    pub fn allocations(&self) -> &HashMap<String, usize> {
        &self.allocations
    }

    /// Check if an allocation exists.
    pub fn has_allocation(&self, name: &str) -> bool {
        self.allocations.contains_key(name)
    }

    /// Get size of a specific allocation.
    pub fn allocation_size(&self, name: &str) -> Option<usize> {
        self.allocations.get(name).copied()
    }
}

/// Thread-safe GPU memory pool.
///
/// Wraps VramTracker with Arc<RwLock> for concurrent access.
#[derive(Debug, Clone)]
pub struct GpuMemoryPool {
    inner: Arc<RwLock<VramTracker>>,
}

impl GpuMemoryPool {
    /// Create a new GPU memory pool.
    pub fn new(total_bytes: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(VramTracker::new(total_bytes))),
        }
    }

    /// Create pool for RTX 5090 (32GB).
    pub fn rtx_5090() -> Self {
        Self::new(32 * 1024 * 1024 * 1024)
    }

    /// Allocate memory.
    pub fn allocate(&self, name: &str, bytes: usize) -> Result<(), MemoryError> {
        self.inner.write().map_err(|_| MemoryError::LockPoisoned)?.allocate(name, bytes)
    }

    /// Deallocate memory.
    pub fn deallocate(&self, name: &str) -> usize {
        self.inner.write().ok().map(|mut t| t.deallocate(name)).unwrap_or(0)
    }

    /// Get current statistics.
    pub fn stats(&self) -> MemoryStats {
        self.inner.read().ok().map(|t| t.stats().clone()).unwrap_or_default()
    }

    /// Get available memory.
    pub fn available(&self) -> usize {
        self.inner.read().ok().map(|t| t.available()).unwrap_or(0)
    }
}

impl Default for GpuMemoryPool {
    fn default() -> Self {
        Self::rtx_5090()
    }
}

/// Memory allocation errors.
#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    /// Insufficient VRAM for requested allocation.
    #[error("Out of GPU memory: requested {requested} bytes, available {available} bytes")]
    OutOfMemory { requested: usize, available: usize },

    /// Lock poisoned (thread panic while holding lock).
    #[error("Memory pool lock poisoned")]
    LockPoisoned,
}

/// Reserved memory pools with fixed budgets.
#[derive(Debug, Clone)]
pub struct MemoryBudget {
    /// Model weights budget (e.g., 16GB).
    pub model_weights: usize,
    /// Activation cache budget (e.g., 8GB).
    pub activation_cache: usize,
    /// Working memory budget (e.g., 6GB).
    pub working_memory: usize,
    /// Reserved for system overhead (e.g., 2GB).
    pub reserved: usize,
}

impl MemoryBudget {
    /// Default budget for RTX 5090 32GB.
    pub fn rtx_5090() -> Self {
        const GB: usize = 1024 * 1024 * 1024;
        Self {
            model_weights: 16 * GB,
            activation_cache: 8 * GB,
            working_memory: 6 * GB,
            reserved: 2 * GB,
        }
    }

    /// Total budgeted memory.
    pub fn total(&self) -> usize {
        self.model_weights + self.activation_cache + self.working_memory + self.reserved
    }
}

impl Default for MemoryBudget {
    fn default() -> Self {
        Self::rtx_5090()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_stats() {
        let stats = MemoryStats {
            total_bytes: 1000,
            allocated_bytes: 250,
            ..Default::default()
        };

        assert_eq!(stats.available(), 750);
        assert!((stats.utilization_percent() - 25.0).abs() < 0.1);
    }

    #[test]
    fn test_vram_tracker() {
        let mut tracker = VramTracker::new(1000);

        // Allocate
        assert!(tracker.allocate("test1", 400).is_ok());
        assert_eq!(tracker.available(), 600);

        // Allocate more
        assert!(tracker.allocate("test2", 300).is_ok());
        assert_eq!(tracker.available(), 300);

        // Fail on over-allocation
        assert!(tracker.allocate("test3", 500).is_err());

        // Deallocate
        assert_eq!(tracker.deallocate("test1"), 400);
        assert_eq!(tracker.available(), 700);
    }

    #[test]
    fn test_memory_pool_thread_safe() {
        let pool = GpuMemoryPool::new(1000);

        // Clone for concurrent access
        let pool2 = pool.clone();

        assert!(pool.allocate("a", 300).is_ok());
        assert!(pool2.allocate("b", 300).is_ok());
        assert_eq!(pool.available(), 400);
        assert_eq!(pool2.available(), 400);
    }

    #[test]
    fn test_memory_budget() {
        let budget = MemoryBudget::rtx_5090();
        assert_eq!(budget.total(), 32 * 1024 * 1024 * 1024);
    }
}
