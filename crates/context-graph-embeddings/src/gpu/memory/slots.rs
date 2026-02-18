//! Model slot management for the 13 embedding models.
//!
//! Manages GPU memory allocation for exactly 13 embedding model slots (E1-E13)
//! within the 8GB budget constraint (per constitution.yaml ARCH-08).
//!
//! # Budget
//!
//! Total: 8GB for all 13 models (quantized)
//! Per-model estimates (from task spec):
//! - E1 Semantic: ~200MB
//! - E2-E4 Temporal: ~100MB each
//! - E5 Causal: ~150MB
//! - E6 Sparse: ~50MB
//! - E7 Code: ~300MB
//! - E8 Graph: ~75MB
//! - E9 HDC: ~50MB
//! - E10 Multimodal: ~150MB
//! - E11 Entity: ~75MB
//! - E12 LateInteraction: ~200MB
//! - E13 KeywordSplade: ~50MB
//!   Total: ~1.6GB with quantization
//!
//! # LRU Eviction
//!
//! When memory pressure reaches Critical (>95%), the least-recently-used
//! model slot is evicted to make room for new allocations.

// This module is prepared for future GPU memory management but not yet integrated.
// Suppress dead_code warnings until integration is complete.
#![allow(dead_code)]

use context_graph_core::teleological::embedder::Embedder;
use std::collections::HashMap;
use std::time::Instant;

use super::error::MemoryError;
use super::pressure::MemoryPressure;

/// Budget for 13 models: 8GB total.
///
/// From constitution.yaml: GPU memory usage < 8GB for 13 models
pub const MODEL_BUDGET_BYTES: usize = 8 * 1024 * 1024 * 1024;

/// Individual model slot with tracking metadata.
#[derive(Debug)]
pub struct ModelSlot {
    /// Which embedder this slot belongs to.
    pub embedder: Embedder,
    /// Allocated bytes for this model.
    pub size_bytes: usize,
    /// When this slot was last accessed (for LRU eviction).
    pub last_accessed: Instant,
    /// Whether model weights are currently loaded in GPU memory.
    pub loaded: bool,
}

impl ModelSlot {
    /// Create a new model slot.
    fn new(embedder: Embedder, size_bytes: usize) -> Self {
        Self {
            embedder,
            size_bytes,
            last_accessed: Instant::now(),
            loaded: true,
        }
    }

    /// Update the last accessed timestamp.
    fn touch(&mut self) {
        self.last_accessed = Instant::now();
    }
}

/// Manages 13 model slots within GPU memory budget.
///
/// Provides allocation, deallocation, and LRU eviction for the
/// 13 embedding models in the teleological array system.
#[derive(Debug)]
pub struct ModelSlotManager {
    /// Slots indexed by embedder.
    slots: HashMap<Embedder, ModelSlot>,
    /// Total allocated across all slots.
    total_allocated: usize,
    /// Maximum allowed (default: 8GB).
    budget: usize,
}

impl ModelSlotManager {
    /// Create manager with default 8GB budget.
    pub fn new() -> Self {
        Self::with_budget(MODEL_BUDGET_BYTES)
    }

    /// Create manager with custom budget for testing.
    ///
    /// # Arguments
    ///
    /// * `budget` - Maximum bytes allowed for all model slots
    pub fn with_budget(budget: usize) -> Self {
        Self {
            slots: HashMap::with_capacity(Embedder::COUNT),
            total_allocated: 0,
            budget,
        }
    }

    /// Allocate a slot for an embedder.
    ///
    /// # Arguments
    ///
    /// * `embedder` - The embedder to allocate slot for
    /// * `size_bytes` - Number of bytes required for this model
    ///
    /// # Errors
    ///
    /// Returns `MemoryError::OutOfMemory` if allocation would exceed budget.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_embeddings::gpu::ModelSlotManager;
    /// use context_graph_core::teleological::embedder::Embedder;
    ///
    /// let mut manager = ModelSlotManager::with_budget(1000);
    /// assert!(manager.allocate_slot(Embedder::Semantic, 500).is_ok());
    /// assert_eq!(manager.allocated(), 500);
    /// ```
    pub fn allocate_slot(
        &mut self,
        embedder: Embedder,
        size_bytes: usize,
    ) -> Result<(), MemoryError> {
        // Check if slot already exists - update it
        if let Some(existing) = self.slots.get(&embedder) {
            let old_size = existing.size_bytes;
            let size_diff = size_bytes.saturating_sub(old_size);

            // Check if new allocation fits
            if self.total_allocated + size_diff > self.budget {
                tracing::error!(
                    embedder = ?embedder,
                    requested = size_bytes,
                    current = old_size,
                    available = self.available(),
                    budget = self.budget,
                    "[EMB-E003] GPU memory allocation failed: would exceed budget"
                );
                return Err(MemoryError::OutOfMemory {
                    requested: size_bytes,
                    available: self.available() + old_size,
                });
            }

            // Update total allocated
            self.total_allocated = self.total_allocated.saturating_sub(old_size) + size_bytes;
        } else {
            // New slot - check budget
            if self.total_allocated + size_bytes > self.budget {
                tracing::error!(
                    embedder = ?embedder,
                    requested = size_bytes,
                    available = self.available(),
                    budget = self.budget,
                    total_allocated = self.total_allocated,
                    loaded_models = self.loaded_count(),
                    "[EMB-E003] GPU memory allocation failed: would exceed budget. \
                     Suggestion: Evict least-recently-used model or reduce model sizes"
                );
                return Err(MemoryError::OutOfMemory {
                    requested: size_bytes,
                    available: self.available(),
                });
            }

            self.total_allocated += size_bytes;
        }

        let slot = ModelSlot::new(embedder, size_bytes);
        self.slots.insert(embedder, slot);

        tracing::debug!(
            embedder = ?embedder,
            size_bytes = size_bytes,
            total_allocated = self.total_allocated,
            budget = self.budget,
            utilization_pct = format!("{:.1}%", self.utilization_percent()),
            "Allocated model slot"
        );

        Ok(())
    }

    /// Deallocate a slot and free its memory.
    ///
    /// # Returns
    ///
    /// Number of bytes freed, or 0 if embedder had no slot.
    pub fn deallocate_slot(&mut self, embedder: &Embedder) -> usize {
        if let Some(slot) = self.slots.remove(embedder) {
            self.total_allocated = self.total_allocated.saturating_sub(slot.size_bytes);
            tracing::debug!(
                embedder = ?embedder,
                freed_bytes = slot.size_bytes,
                total_allocated = self.total_allocated,
                "Deallocated model slot"
            );
            slot.size_bytes
        } else {
            0
        }
    }

    /// Mark a slot as accessed (updates LRU timestamp).
    ///
    /// Call this whenever a model is used for inference to
    /// update its position in the LRU order.
    pub fn touch(&mut self, embedder: &Embedder) {
        if let Some(slot) = self.slots.get_mut(embedder) {
            slot.touch();
            tracing::trace!(embedder = ?embedder, "Model slot touched");
        }
    }

    /// Get the least recently used slot for eviction.
    ///
    /// Only considers slots that are currently loaded.
    ///
    /// # Returns
    ///
    /// The embedder of the least recently used slot, or None if no slots are loaded.
    pub fn get_lru_candidate(&self) -> Option<Embedder> {
        self.slots
            .values()
            .filter(|s| s.loaded)
            .min_by_key(|s| s.last_accessed)
            .map(|s| s.embedder)
    }

    /// Evict the least recently used model to free memory.
    ///
    /// # Returns
    ///
    /// The embedder that was evicted and bytes freed, or None if nothing to evict.
    pub fn evict_lru(&mut self) -> Option<(Embedder, usize)> {
        if let Some(embedder) = self.get_lru_candidate() {
            let freed = self.deallocate_slot(&embedder);
            tracing::info!(
                embedder = ?embedder,
                freed_bytes = freed,
                "Evicted LRU model due to memory pressure"
            );
            Some((embedder, freed))
        } else {
            None
        }
    }

    /// Current memory pressure level.
    pub fn pressure_level(&self) -> MemoryPressure {
        MemoryPressure::from_bytes(self.total_allocated, self.budget)
    }

    /// Get utilization as a percentage.
    pub fn utilization_percent(&self) -> f32 {
        if self.budget == 0 {
            return 100.0;
        }
        (self.total_allocated as f32 / self.budget as f32) * 100.0
    }

    /// Available bytes before hitting budget.
    pub fn available(&self) -> usize {
        self.budget.saturating_sub(self.total_allocated)
    }

    /// Total allocated bytes.
    pub fn allocated(&self) -> usize {
        self.total_allocated
    }

    /// Total budget bytes.
    pub fn budget(&self) -> usize {
        self.budget
    }

    /// Number of loaded slots.
    pub fn loaded_count(&self) -> usize {
        self.slots.values().filter(|s| s.loaded).count()
    }

    /// Total number of slots (loaded or not).
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Check if specific embedder has a slot.
    pub fn has_slot(&self, embedder: &Embedder) -> bool {
        self.slots.contains_key(embedder)
    }

    /// Get the slot for a specific embedder.
    pub fn get_slot(&self, embedder: &Embedder) -> Option<&ModelSlot> {
        self.slots.get(embedder)
    }

    /// Get all embedders that have slots allocated.
    pub fn allocated_embedders(&self) -> impl Iterator<Item = Embedder> + '_ {
        self.slots.keys().copied()
    }

    /// Try to allocate with automatic LRU eviction if needed.
    ///
    /// Will evict models until there's enough space or no more models to evict.
    ///
    /// # Arguments
    ///
    /// * `embedder` - The embedder to allocate slot for
    /// * `size_bytes` - Number of bytes required
    ///
    /// # Returns
    ///
    /// Ok with list of evicted embedders, or Err if still can't fit.
    pub fn allocate_with_eviction(
        &mut self,
        embedder: Embedder,
        size_bytes: usize,
    ) -> Result<Vec<Embedder>, MemoryError> {
        let mut evicted = Vec::new();

        // Try direct allocation first
        if self.allocate_slot(embedder, size_bytes).is_ok() {
            return Ok(evicted);
        }

        // Evict until we have enough space
        while self.available() < size_bytes {
            if let Some((evicted_embedder, _)) = self.evict_lru() {
                evicted.push(evicted_embedder);
            } else {
                // No more models to evict
                tracing::error!(
                    embedder = ?embedder,
                    requested = size_bytes,
                    available = self.available(),
                    evicted_count = evicted.len(),
                    "[EMB-E003] Cannot allocate even after evicting all models"
                );
                return Err(MemoryError::OutOfMemory {
                    requested: size_bytes,
                    available: self.available(),
                });
            }
        }

        // Now try allocation again
        self.allocate_slot(embedder, size_bytes)?;
        Ok(evicted)
    }
}

impl Default for ModelSlotManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_model_budget_constant() {
        assert_eq!(MODEL_BUDGET_BYTES, 8 * 1024 * 1024 * 1024);
        println!("[PASS] MODEL_BUDGET_BYTES = 8GB");
    }

    #[test]
    fn test_default_manager_has_8gb_budget() {
        let manager = ModelSlotManager::new();
        assert_eq!(manager.budget(), MODEL_BUDGET_BYTES);
        assert_eq!(manager.allocated(), 0);
        assert_eq!(manager.available(), MODEL_BUDGET_BYTES);
        println!("[PASS] Default ModelSlotManager has 8GB budget");
    }

    #[test]
    fn test_allocate_slot_success() {
        let mut manager = ModelSlotManager::with_budget(1000);
        assert!(manager.allocate_slot(Embedder::Semantic, 500).is_ok());
        assert_eq!(manager.allocated(), 500);
        assert_eq!(manager.available(), 500);
        assert!(manager.has_slot(&Embedder::Semantic));
        println!("[PASS] allocate_slot() works correctly");
    }

    #[test]
    fn test_allocate_slot_fails_over_budget() {
        let mut manager = ModelSlotManager::with_budget(1000);
        let result = manager.allocate_slot(Embedder::Semantic, 1001);
        assert!(result.is_err());
        assert_eq!(manager.allocated(), 0);
        println!("[PASS] allocate_slot() fails when over budget");
    }

    #[test]
    fn test_edge_case_exact_budget_boundary() {
        // Edge Case 1: Budget Boundary
        let mut manager = ModelSlotManager::with_budget(1000);
        println!(
            "BEFORE: allocated={}, available={}",
            manager.allocated(),
            manager.available()
        );

        // Action: Allocate exactly at budget
        let result = manager.allocate_slot(Embedder::Semantic, 1000);
        assert!(result.is_ok());

        // AFTER state
        println!(
            "AFTER: allocated={}, available={}",
            manager.allocated(),
            manager.available()
        );
        assert_eq!(manager.allocated(), 1000);
        assert_eq!(manager.available(), 0);
        println!("[PASS] Edge Case 1: Budget boundary respected");
    }

    #[test]
    fn test_edge_case_over_budget_rejection() {
        // Edge Case 2: Over-Budget Rejection
        let mut manager = ModelSlotManager::with_budget(1000);
        manager
            .allocate_slot(Embedder::Semantic, 900)
            .expect("initial allocation should succeed");
        println!("BEFORE: allocated={}", manager.allocated());

        // Action: Try to exceed budget
        let result = manager.allocate_slot(Embedder::TemporalRecent, 200);

        // AFTER state (should be unchanged)
        println!(
            "AFTER: allocated={}, result={:?}",
            manager.allocated(),
            result
        );
        assert!(result.is_err());
        assert_eq!(manager.allocated(), 900); // Unchanged
        println!("[PASS] Edge Case 2: Over-budget allocation rejected");
    }

    #[test]
    fn test_edge_case_lru_eviction_selection() {
        // Edge Case 3: LRU Eviction Selection
        let mut manager = ModelSlotManager::with_budget(10000);

        // Allocate in order
        manager
            .allocate_slot(Embedder::Semantic, 1000)
            .expect("allocate E1");
        sleep(Duration::from_millis(10));
        manager
            .allocate_slot(Embedder::TemporalRecent, 1000)
            .expect("allocate E2");
        sleep(Duration::from_millis(10));
        manager
            .allocate_slot(Embedder::TemporalPeriodic, 1000)
            .expect("allocate E3");

        // Touch Semantic to make it recent
        manager.touch(&Embedder::Semantic);

        // LRU should be TemporalRecent (oldest untouched)
        let lru = manager.get_lru_candidate();
        println!("LRU candidate: {:?}", lru);
        assert_eq!(lru, Some(Embedder::TemporalRecent));
        println!("[PASS] Edge Case 3: LRU candidate correctly identified");
    }

    #[test]
    fn test_deallocate_slot() {
        let mut manager = ModelSlotManager::with_budget(1000);
        manager.allocate_slot(Embedder::Semantic, 500).unwrap();
        assert_eq!(manager.loaded_count(), 1);

        let freed = manager.deallocate_slot(&Embedder::Semantic);
        assert_eq!(freed, 500);
        assert_eq!(manager.allocated(), 0);
        assert_eq!(manager.loaded_count(), 0);
        assert!(!manager.has_slot(&Embedder::Semantic));
        println!("[PASS] deallocate_slot() frees memory correctly");
    }

    #[test]
    fn test_deallocate_nonexistent() {
        let mut manager = ModelSlotManager::with_budget(1000);
        let freed = manager.deallocate_slot(&Embedder::Semantic);
        assert_eq!(freed, 0);
        println!("[PASS] deallocate_slot() returns 0 for nonexistent slot");
    }

    #[test]
    fn test_pressure_level() {
        let mut manager = ModelSlotManager::with_budget(1000);

        // Low pressure (<50%)
        manager.allocate_slot(Embedder::Semantic, 400).unwrap();
        assert_eq!(manager.pressure_level(), MemoryPressure::Low);

        // Medium pressure (50-80%)
        manager
            .allocate_slot(Embedder::TemporalRecent, 200)
            .unwrap();
        assert_eq!(manager.pressure_level(), MemoryPressure::Medium);

        // High pressure (80-95%)
        manager
            .allocate_slot(Embedder::TemporalPeriodic, 250)
            .unwrap();
        assert_eq!(manager.pressure_level(), MemoryPressure::High);

        // Critical pressure (>95%)
        manager.allocate_slot(Embedder::Causal, 110).unwrap();
        assert_eq!(manager.pressure_level(), MemoryPressure::Critical);
        assert!(manager.pressure_level().should_evict());

        println!("[PASS] pressure_level() reflects utilization correctly");
    }

    #[test]
    fn test_evict_lru() {
        let mut manager = ModelSlotManager::with_budget(1000);
        manager.allocate_slot(Embedder::Semantic, 500).unwrap();
        sleep(Duration::from_millis(10));
        manager
            .allocate_slot(Embedder::TemporalRecent, 400)
            .unwrap();

        let evicted = manager.evict_lru();
        assert_eq!(evicted, Some((Embedder::Semantic, 500)));
        assert_eq!(manager.allocated(), 400);
        assert!(!manager.has_slot(&Embedder::Semantic));
        assert!(manager.has_slot(&Embedder::TemporalRecent));
        println!("[PASS] evict_lru() evicts oldest slot");
    }

    #[test]
    fn test_allocate_with_eviction() {
        let mut manager = ModelSlotManager::with_budget(1000);
        manager.allocate_slot(Embedder::Semantic, 600).unwrap();
        sleep(Duration::from_millis(10));
        manager
            .allocate_slot(Embedder::TemporalRecent, 300)
            .unwrap();

        // Try to allocate 500 more (need to evict)
        let result = manager.allocate_with_eviction(Embedder::Code, 500);
        assert!(result.is_ok());
        let evicted = result.unwrap();
        assert!(evicted.contains(&Embedder::Semantic));
        assert!(manager.has_slot(&Embedder::Code));
        println!("[PASS] allocate_with_eviction() auto-evicts as needed");
    }

    #[test]
    fn test_allocate_all_13_embedders() {
        // Test 1 from task spec: Allocate all 13 embedders
        let model_sizes: [(Embedder, usize); 13] = [
            (Embedder::Semantic, 200_000_000),           // E1: 200MB
            (Embedder::TemporalRecent, 100_000_000),     // E2: 100MB
            (Embedder::TemporalPeriodic, 100_000_000),   // E3: 100MB
            (Embedder::TemporalPositional, 100_000_000), // E4: 100MB
            (Embedder::Causal, 150_000_000),             // E5: 150MB
            (Embedder::Sparse, 50_000_000),              // E6: 50MB
            (Embedder::Code, 300_000_000),               // E7: 300MB
            (Embedder::Graph, 75_000_000),           // E8: 75MB (was Graph, renamed)
            (Embedder::Hdc, 50_000_000),                 // E9: 50MB
            (Embedder::Multimodal, 150_000_000),         // E10: 150MB
            (Embedder::Entity, 75_000_000),              // E11: 75MB
            (Embedder::LateInteraction, 200_000_000),    // E12: 200MB
            (Embedder::KeywordSplade, 50_000_000),       // E13: 50MB
        ];
        // Total: ~1.6GB

        let mut manager = ModelSlotManager::new(); // 8GB budget
        let mut total_expected = 0usize;

        for (embedder, size) in model_sizes.iter() {
            let result = manager.allocate_slot(*embedder, *size);
            assert!(
                result.is_ok(),
                "Failed to allocate {:?}: {:?}",
                embedder,
                result
            );
            total_expected += size;
        }

        assert_eq!(manager.loaded_count(), 13);
        assert_eq!(manager.allocated(), total_expected);
        assert_eq!(manager.pressure_level(), MemoryPressure::Low);

        println!(
            "Total allocated: {} bytes ({:.2}GB)",
            manager.allocated(),
            manager.allocated() as f64 / (1024.0 * 1024.0 * 1024.0)
        );
        println!("Pressure level: {:?}", manager.pressure_level());
        println!("[PASS] All 13 embedders allocated within 8GB budget");
    }

    #[test]
    fn test_memory_pressure_at_96_percent() {
        // Test 2 from task spec: Verify Critical pressure at 96%
        let mut manager = ModelSlotManager::with_budget(1000);
        manager.allocate_slot(Embedder::Semantic, 960).unwrap(); // 96%

        assert_eq!(manager.pressure_level(), MemoryPressure::Critical);
        assert!(manager.pressure_level().should_evict());
        println!("[PASS] pressure_level() returns Critical at 96% utilization");
    }

    #[test]
    fn test_update_existing_slot() {
        let mut manager = ModelSlotManager::with_budget(1000);
        manager.allocate_slot(Embedder::Semantic, 300).unwrap();
        assert_eq!(manager.allocated(), 300);

        // Update to larger size
        manager.allocate_slot(Embedder::Semantic, 500).unwrap();
        assert_eq!(manager.allocated(), 500);

        // Update to smaller size
        manager.allocate_slot(Embedder::Semantic, 200).unwrap();
        assert_eq!(manager.allocated(), 200);

        println!("[PASS] Updating existing slot adjusts allocation correctly");
    }

    #[test]
    fn test_slot_count_vs_loaded_count() {
        let mut manager = ModelSlotManager::with_budget(10000);
        manager.allocate_slot(Embedder::Semantic, 1000).unwrap();
        manager.allocate_slot(Embedder::Code, 1000).unwrap();

        assert_eq!(manager.slot_count(), 2);
        assert_eq!(manager.loaded_count(), 2);

        manager.deallocate_slot(&Embedder::Semantic);
        assert_eq!(manager.slot_count(), 1);
        assert_eq!(manager.loaded_count(), 1);

        println!("[PASS] slot_count() and loaded_count() track correctly");
    }

    #[test]
    fn test_allocated_embedders_iterator() {
        let mut manager = ModelSlotManager::with_budget(10000);
        manager.allocate_slot(Embedder::Semantic, 1000).unwrap();
        manager.allocate_slot(Embedder::Code, 1000).unwrap();
        manager.allocate_slot(Embedder::Causal, 1000).unwrap();

        let embedders: Vec<_> = manager.allocated_embedders().collect();
        assert_eq!(embedders.len(), 3);
        assert!(embedders.contains(&Embedder::Semantic));
        assert!(embedders.contains(&Embedder::Code));
        assert!(embedders.contains(&Embedder::Causal));

        println!("[PASS] allocated_embedders() returns correct iterator");
    }
}
