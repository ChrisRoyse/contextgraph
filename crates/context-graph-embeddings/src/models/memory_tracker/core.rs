//! Core MemoryTracker implementation.
//!
//! This module contains the `MemoryTracker` struct which tracks memory usage
//! across loaded models and enforces a total memory budget.

use std::collections::HashMap;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::types::ModelId;

/// Tracks memory usage across loaded models.
///
/// Maintains a per-model allocation map and enforces a total memory budget.
/// All allocations are checked against the budget before proceeding.
///
/// # Thread Safety
///
/// This struct is NOT thread-safe by itself. The `ModelRegistry` wraps it
/// in `RwLock<MemoryTracker>` for thread-safe access.
///
/// # Example
///
/// ```rust
/// use context_graph_embeddings::MemoryTracker;
/// use context_graph_embeddings::types::ModelId;
/// use context_graph_embeddings::error::EmbeddingResult;
///
/// fn main() -> EmbeddingResult<()> {
///     let mut tracker = MemoryTracker::new(32_000_000_000); // 32GB budget
///
///     // Check before allocating
///     if tracker.can_allocate(1_400_000_000) {
///         tracker.allocate(ModelId::Semantic, 1_400_000_000)?;
///     }
///
///     // Deallocate when unloading
///     let freed = tracker.deallocate(ModelId::Semantic)?;
///     Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct MemoryTracker {
    /// Current total memory usage in bytes
    current_bytes: usize,
    /// Maximum allowed memory in bytes
    budget_bytes: usize,
    /// Per-model memory allocations
    allocations: HashMap<ModelId, usize>,
}

impl MemoryTracker {
    /// Create a new memory tracker with the given budget.
    ///
    /// # Arguments
    /// * `budget_bytes` - Maximum allowed memory in bytes
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::MemoryTracker;
    ///
    /// let tracker = MemoryTracker::new(32_000_000_000); // 32GB
    /// assert_eq!(tracker.remaining(), 32_000_000_000);
    /// ```
    pub fn new(budget_bytes: usize) -> Self {
        Self {
            current_bytes: 0,
            budget_bytes,
            allocations: HashMap::new(),
        }
    }

    /// Check if allocation is possible within budget.
    ///
    /// Does NOT perform the allocation, only checks if it would succeed.
    ///
    /// # Arguments
    /// * `bytes` - Number of bytes to allocate
    ///
    /// # Returns
    /// `true` if allocation would succeed, `false` otherwise
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::MemoryTracker;
    /// use context_graph_embeddings::types::ModelId;
    /// use context_graph_embeddings::error::EmbeddingResult;
    ///
    /// fn main() -> EmbeddingResult<()> {
    ///     let mut tracker = MemoryTracker::new(2_000_000_000); // 2GB budget
    ///     let memory_estimate = 500_000_000; // 500MB
    ///     let model_id = ModelId::Semantic;
    ///
    ///     if tracker.can_allocate(memory_estimate) {
    ///         tracker.allocate(model_id, memory_estimate)?;
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub fn can_allocate(&self, bytes: usize) -> bool {
        self.current_bytes.saturating_add(bytes) <= self.budget_bytes
    }

    /// Allocate memory for a model.
    ///
    /// # Arguments
    /// * `model_id` - The model being loaded
    /// * `bytes` - Memory required in bytes
    ///
    /// # Returns
    /// - `Ok(())` if allocation successful
    /// - `Err(EmbeddingError::MemoryBudgetExceeded)` if budget exceeded
    /// - `Err(EmbeddingError::ModelAlreadyLoaded)` if model already allocated
    ///
    /// # Errors
    ///
    /// Returns `MemoryBudgetExceeded` with full context including:
    /// - Requested amount
    /// - Current usage
    /// - Budget
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::MemoryTracker;
    /// use context_graph_embeddings::types::ModelId;
    /// use context_graph_embeddings::error::EmbeddingResult;
    ///
    /// fn main() -> EmbeddingResult<()> {
    ///     let mut tracker = MemoryTracker::new(2_000_000_000); // 2GB budget
    ///     tracker.allocate(ModelId::Semantic, 1_400_000_000)?;
    ///     assert_eq!(tracker.current_usage(), 1_400_000_000);
    ///     Ok(())
    /// }
    /// ```
    pub fn allocate(&mut self, model_id: ModelId, bytes: usize) -> EmbeddingResult<()> {
        // FAIL FAST: Check if model already allocated
        if self.allocations.contains_key(&model_id) {
            return Err(EmbeddingError::ModelAlreadyLoaded { model_id });
        }

        // FAIL FAST: Check budget before allocation
        let new_total = self.current_bytes.saturating_add(bytes);
        if new_total > self.budget_bytes {
            return Err(EmbeddingError::MemoryBudgetExceeded {
                requested_bytes: bytes,
                available_bytes: self.remaining(),
                budget_bytes: self.budget_bytes,
            });
        }

        // Perform allocation
        self.allocations.insert(model_id, bytes);
        self.current_bytes = new_total;

        tracing::debug!(
            model_id = ?model_id,
            allocated_bytes = bytes,
            total_bytes = self.current_bytes,
            remaining_bytes = self.remaining(),
            "Memory allocated for model"
        );

        Ok(())
    }

    /// Deallocate memory when model unloaded.
    ///
    /// # Arguments
    /// * `model_id` - The model being unloaded
    ///
    /// # Returns
    /// - `Ok(bytes)` - The number of bytes freed
    /// - `Err(EmbeddingError::ModelNotLoaded)` if model not allocated
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_embeddings::MemoryTracker;
    /// use context_graph_embeddings::types::ModelId;
    /// use context_graph_embeddings::error::EmbeddingResult;
    ///
    /// fn main() -> EmbeddingResult<()> {
    ///     let mut tracker = MemoryTracker::new(2_000_000_000); // 2GB budget
    ///     tracker.allocate(ModelId::Semantic, 1_400_000_000)?;
    ///     let freed = tracker.deallocate(ModelId::Semantic)?;
    ///     assert_eq!(freed, 1_400_000_000);
    ///     Ok(())
    /// }
    /// ```
    pub fn deallocate(&mut self, model_id: ModelId) -> EmbeddingResult<usize> {
        // FAIL FAST: Model must be allocated
        let bytes = self
            .allocations
            .remove(&model_id)
            .ok_or_else(|| EmbeddingError::ModelNotLoaded { model_id })?;

        self.current_bytes = self.current_bytes.saturating_sub(bytes);

        tracing::debug!(
            model_id = ?model_id,
            freed_bytes = bytes,
            remaining_total = self.current_bytes,
            "Memory deallocated for model"
        );

        Ok(bytes)
    }

    /// Get current total memory usage in bytes.
    ///
    /// # Returns
    /// Total bytes currently allocated across all models.
    #[inline]
    pub fn current_usage(&self) -> usize {
        self.current_bytes
    }

    /// Get remaining budget in bytes.
    ///
    /// # Returns
    /// Bytes available for allocation.
    #[inline]
    pub fn remaining(&self) -> usize {
        self.budget_bytes.saturating_sub(self.current_bytes)
    }

    /// Get the configured budget in bytes.
    ///
    /// # Returns
    /// Total memory budget.
    #[inline]
    pub fn budget(&self) -> usize {
        self.budget_bytes
    }

    /// Get memory allocated for a specific model.
    ///
    /// # Arguments
    /// * `model_id` - The model to query
    ///
    /// # Returns
    /// Bytes allocated, or 0 if not loaded.
    #[inline]
    pub fn allocation_for(&self, model_id: ModelId) -> usize {
        self.allocations.get(&model_id).copied().unwrap_or(0)
    }

    /// Get the number of models with allocations.
    #[inline]
    pub fn allocation_count(&self) -> usize {
        self.allocations.len()
    }

    /// Check if a model has memory allocated.
    #[inline]
    pub fn is_allocated(&self, model_id: ModelId) -> bool {
        self.allocations.contains_key(&model_id)
    }

    /// Get all model IDs with allocations.
    pub fn allocated_models(&self) -> Vec<ModelId> {
        self.allocations.keys().copied().collect()
    }
}
