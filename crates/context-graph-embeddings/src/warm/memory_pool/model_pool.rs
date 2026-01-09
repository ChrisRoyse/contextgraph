//! Non-evictable memory pool for model weights.
//!
//! This module provides the `ModelMemoryPool` which holds permanent VRAM
//! allocations for model weights. These allocations are protected from
//! memory pressure and cannot be evicted.

use super::types::ModelAllocation;
use crate::warm::error::{WarmError, WarmResult};

/// Non-evictable memory pool for model weights.
///
/// This pool holds permanent VRAM allocations for model weights.
/// Allocations in this pool are protected from memory pressure
/// and cannot be evicted.
///
/// # Invariants
///
/// - `allocated_bytes <= capacity_bytes` (enforced by allocation methods)
/// - No duplicate `model_id` entries in `allocations`
#[derive(Debug, Clone)]
pub struct ModelMemoryPool {
    /// Total capacity in bytes (e.g., 24GB for RTX 5090).
    capacity_bytes: usize,
    /// Currently allocated bytes across all models.
    allocated_bytes: usize,
    /// Tracking information for each model allocation.
    allocations: Vec<ModelAllocation>,
}

impl ModelMemoryPool {
    /// Create a new model memory pool with the specified capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity_bytes` - Maximum bytes that can be allocated for model weights
    #[must_use]
    pub fn new(capacity_bytes: usize) -> Self {
        Self {
            capacity_bytes,
            allocated_bytes: 0,
            allocations: Vec::new(),
        }
    }

    /// Get the total capacity of this pool in bytes.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity_bytes
    }

    /// Get the currently allocated bytes.
    #[must_use]
    pub fn allocated(&self) -> usize {
        self.allocated_bytes
    }

    /// Get the available bytes for new allocations.
    #[must_use]
    pub fn available(&self) -> usize {
        self.capacity_bytes.saturating_sub(self.allocated_bytes)
    }

    /// Check if a model is already allocated.
    #[must_use]
    pub fn contains(&self, model_id: &str) -> bool {
        self.allocations.iter().any(|a| a.model_id == model_id)
    }

    /// Get allocation info for a specific model.
    #[must_use]
    pub fn get(&self, model_id: &str) -> Option<&ModelAllocation> {
        self.allocations.iter().find(|a| a.model_id == model_id)
    }

    /// Get all current allocations.
    #[must_use]
    pub fn allocations(&self) -> &[ModelAllocation] {
        &self.allocations
    }

    /// Allocate memory for a model.
    ///
    /// # Arguments
    ///
    /// * `model_id` - Unique identifier for the model
    /// * `size_bytes` - Size of allocation in bytes
    /// * `vram_ptr` - Raw VRAM pointer from CUDA
    ///
    /// # Errors
    ///
    /// Returns `WarmError::ModelAlreadyRegistered` if model already allocated.
    /// Returns `WarmError::VramAllocationFailed` if allocation exceeds capacity.
    pub fn allocate(&mut self, model_id: &str, size_bytes: usize, vram_ptr: u64) -> WarmResult<()> {
        // Check for duplicate allocation
        if self.contains(model_id) {
            return Err(WarmError::ModelAlreadyRegistered {
                model_id: model_id.to_string(),
            });
        }

        // Check capacity - NO FALLBACKS
        if self.allocated_bytes.saturating_add(size_bytes) > self.capacity_bytes {
            return Err(WarmError::VramAllocationFailed {
                requested_bytes: size_bytes,
                available_bytes: self.available(),
                error: format!(
                    "Model pool capacity exhausted: {} bytes requested, {} bytes available",
                    size_bytes,
                    self.available()
                ),
            });
        }

        // Record allocation
        self.allocations.push(ModelAllocation::new(
            model_id.to_string(),
            vram_ptr,
            size_bytes,
        ));
        self.allocated_bytes = self.allocated_bytes.saturating_add(size_bytes);

        Ok(())
    }

    /// Free a model allocation.
    ///
    /// # Arguments
    ///
    /// * `model_id` - Identifier of model to free
    ///
    /// # Errors
    ///
    /// Returns `WarmError::ModelNotRegistered` if model not found.
    pub fn free(&mut self, model_id: &str) -> WarmResult<usize> {
        // Find the allocation
        let idx = self
            .allocations
            .iter()
            .position(|a| a.model_id == model_id)
            .ok_or_else(|| WarmError::ModelNotRegistered {
                model_id: model_id.to_string(),
            })?;

        // Remove and update accounting
        let allocation = self.allocations.remove(idx);
        self.allocated_bytes = self.allocated_bytes.saturating_sub(allocation.size_bytes);

        Ok(allocation.size_bytes)
    }
}
