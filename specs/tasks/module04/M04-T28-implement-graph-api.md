---
id: "M04-T28"
title: "Implement GPU Memory Manager"
description: |
  Implement GpuMemoryManager for VRAM budget tracking and allocation.
  Target: 24GB RTX 5090 with budget:
  - FAISS index: 8GB
  - Hyperbolic coords: 2.5GB
  - Entailment cones: 2.7GB
  - Working memory: 10.8GB
  Methods: allocate(bytes), free(allocation), available(), used(), budget().
  Returns error if allocation exceeds budget.
layer: "surface"
status: "pending"
priority: "high"
estimated_hours: 3
sequence: 35
depends_on:
  - "M04-T27"
spec_refs:
  - "TECH-GRAPH-004 Section 10"
  - "NFR-KG-001"
files_to_create:
  - path: "crates/context-graph-graph/src/index/gpu_memory.rs"
    description: "GPU memory manager implementation"
files_to_modify:
  - path: "crates/context-graph-graph/src/index/mod.rs"
    description: "Add gpu_memory module"
test_file: "crates/context-graph-graph/tests/gpu_memory_tests.rs"
---

## Context

The Knowledge Graph requires careful GPU memory management to operate within the 24GB VRAM budget of the RTX 5090. The GpuMemoryManager provides a centralized system for tracking allocations, preventing oversubscription, and enabling memory pressure-aware operations. This is critical for preventing OOM conditions when working with 10M+ vectors.

Memory budget breakdown:
- **FAISS Index** (8GB): IVF16384,PQ64x8 index for 10M x 1536D vectors
- **Hyperbolic Coords** (2.5GB): 10M x 64 floats = 2.56GB
- **Entailment Cones** (2.7GB): 10M x 68 floats = 2.72GB
- **Working Memory** (10.8GB): Batch operations, intermediate results

## Scope

### In Scope
- GpuMemoryManager struct with allocation tracking
- AllocationHandle for RAII memory management
- Budget enforcement with category support
- Statistics and monitoring
- Thread-safe operations (Arc<Mutex<>>)

### Out of Scope
- Actual CUDA memory allocation (cudaMalloc) - uses placeholder
- Memory defragmentation
- Multi-GPU support
- Async allocation

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/index/gpu_memory.rs

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::{GraphError, GraphResult};

/// Memory categories for budget allocation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryCategory {
    /// FAISS index structures
    FaissIndex,
    /// Poincare point coordinates
    HyperbolicCoords,
    /// Entailment cone data
    EntailmentCones,
    /// Temporary working memory
    WorkingMemory,
    /// Uncategorized allocations
    Other,
}

impl MemoryCategory {
    /// Get default budget for this category (in bytes)
    pub fn default_budget(&self) -> usize {
        match self {
            MemoryCategory::FaissIndex => 8 * 1024 * 1024 * 1024,      // 8GB
            MemoryCategory::HyperbolicCoords => 2560 * 1024 * 1024,   // 2.5GB
            MemoryCategory::EntailmentCones => 2764 * 1024 * 1024,    // 2.7GB
            MemoryCategory::WorkingMemory => 10854 * 1024 * 1024,     // 10.8GB
            MemoryCategory::Other => 512 * 1024 * 1024,               // 512MB
        }
    }
}

/// Handle to allocated GPU memory
///
/// When dropped, automatically frees the allocation.
/// Must not outlive the GpuMemoryManager.
#[derive(Debug)]
pub struct AllocationHandle {
    id: u64,
    size: usize,
    category: MemoryCategory,
    manager: Arc<Mutex<ManagerInner>>,
}

impl AllocationHandle {
    /// Get allocation size in bytes
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get allocation category
    pub fn category(&self) -> MemoryCategory {
        self.category
    }

    /// Get allocation ID
    pub fn id(&self) -> u64 {
        self.id
    }
}

impl Drop for AllocationHandle {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.manager.lock() {
            inner.free(self.id);
        }
    }
}

/// Configuration for GPU memory manager
#[derive(Debug, Clone)]
pub struct GpuMemoryConfig {
    /// Total VRAM budget in bytes (default: 24GB)
    pub total_budget: usize,

    /// Per-category budget overrides
    pub category_budgets: HashMap<MemoryCategory, usize>,

    /// Allow over-allocation (for testing only)
    pub allow_overallocation: bool,

    /// Low memory threshold for warnings (fraction)
    pub low_memory_threshold: f32,
}

impl Default for GpuMemoryConfig {
    fn default() -> Self {
        Self {
            total_budget: 24 * 1024 * 1024 * 1024,  // 24GB
            category_budgets: HashMap::new(),
            allow_overallocation: false,
            low_memory_threshold: 0.9,
        }
    }
}

impl GpuMemoryConfig {
    /// Create config for RTX 5090 (24GB)
    pub fn rtx_5090() -> Self {
        Self::default()
    }

    /// Create config for smaller GPU
    pub fn with_budget(total_bytes: usize) -> Self {
        Self {
            total_budget: total_bytes,
            ..Default::default()
        }
    }

    /// Set category budget
    pub fn category_budget(mut self, category: MemoryCategory, bytes: usize) -> Self {
        self.category_budgets.insert(category, bytes);
        self
    }
}

/// Statistics about GPU memory usage
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    /// Total bytes allocated
    pub total_allocated: usize,

    /// Total budget
    pub total_budget: usize,

    /// Number of active allocations
    pub allocation_count: usize,

    /// Peak memory usage
    pub peak_usage: usize,

    /// Per-category usage
    pub category_usage: HashMap<MemoryCategory, usize>,

    /// Per-category budget
    pub category_budget: HashMap<MemoryCategory, usize>,
}

impl MemoryStats {
    /// Get usage percentage
    pub fn usage_percent(&self) -> f32 {
        if self.total_budget > 0 {
            (self.total_allocated as f32 / self.total_budget as f32) * 100.0
        } else {
            0.0
        }
    }

    /// Check if memory is low
    pub fn is_low_memory(&self, threshold: f32) -> bool {
        self.usage_percent() / 100.0 > threshold
    }

    /// Get available bytes
    pub fn available(&self) -> usize {
        self.total_budget.saturating_sub(self.total_allocated)
    }
}

/// Internal manager state
struct ManagerInner {
    config: GpuMemoryConfig,
    allocations: HashMap<u64, (usize, MemoryCategory)>,
    category_usage: HashMap<MemoryCategory, usize>,
    total_allocated: usize,
    peak_usage: usize,
    next_id: u64,
}

impl ManagerInner {
    fn new(config: GpuMemoryConfig) -> Self {
        Self {
            config,
            allocations: HashMap::new(),
            category_usage: HashMap::new(),
            total_allocated: 0,
            peak_usage: 0,
            next_id: 0,
        }
    }

    fn allocate(&mut self, size: usize, category: MemoryCategory) -> Result<u64, GraphError> {
        // Check total budget
        let new_total = self.total_allocated + size;
        if !self.config.allow_overallocation && new_total > self.config.total_budget {
            return Err(GraphError::GpuResourceAllocation(format!(
                "Allocation of {} bytes would exceed budget ({} / {} used)",
                size, self.total_allocated, self.config.total_budget
            )));
        }

        // Check category budget
        let category_budget = self.config.category_budgets
            .get(&category)
            .copied()
            .unwrap_or_else(|| category.default_budget());

        let current_category_usage = self.category_usage.get(&category).copied().unwrap_or(0);
        if !self.config.allow_overallocation && current_category_usage + size > category_budget {
            return Err(GraphError::GpuResourceAllocation(format!(
                "Allocation of {} bytes in {:?} would exceed category budget ({} / {} used)",
                size, category, current_category_usage, category_budget
            )));
        }

        // Perform allocation
        let id = self.next_id;
        self.next_id += 1;

        self.allocations.insert(id, (size, category));
        *self.category_usage.entry(category).or_insert(0) += size;
        self.total_allocated += size;
        self.peak_usage = self.peak_usage.max(self.total_allocated);

        Ok(id)
    }

    fn free(&mut self, id: u64) {
        if let Some((size, category)) = self.allocations.remove(&id) {
            self.total_allocated = self.total_allocated.saturating_sub(size);
            if let Some(usage) = self.category_usage.get_mut(&category) {
                *usage = usage.saturating_sub(size);
            }
        }
    }

    fn stats(&self) -> MemoryStats {
        MemoryStats {
            total_allocated: self.total_allocated,
            total_budget: self.config.total_budget,
            allocation_count: self.allocations.len(),
            peak_usage: self.peak_usage,
            category_usage: self.category_usage.clone(),
            category_budget: [
                MemoryCategory::FaissIndex,
                MemoryCategory::HyperbolicCoords,
                MemoryCategory::EntailmentCones,
                MemoryCategory::WorkingMemory,
                MemoryCategory::Other,
            ].iter()
                .map(|&cat| {
                    let budget = self.config.category_budgets
                        .get(&cat)
                        .copied()
                        .unwrap_or_else(|| cat.default_budget());
                    (cat, budget)
                })
                .collect(),
        }
    }
}

/// GPU memory manager for VRAM budget tracking
///
/// Provides centralized allocation tracking to prevent OOM conditions.
/// Thread-safe for use across async tasks.
///
/// # Example
/// ```rust
/// let manager = GpuMemoryManager::new(GpuMemoryConfig::rtx_5090())?;
///
/// // Allocate memory
/// let handle = manager.allocate(1024 * 1024, MemoryCategory::WorkingMemory)?;
///
/// // Check stats
/// let stats = manager.stats();
/// println!("Using {} of {} bytes", stats.total_allocated, stats.total_budget);
///
/// // Memory freed automatically when handle is dropped
/// drop(handle);
/// ```
#[derive(Clone)]
pub struct GpuMemoryManager {
    inner: Arc<Mutex<ManagerInner>>,
}

impl GpuMemoryManager {
    /// Create new memory manager with given configuration
    pub fn new(config: GpuMemoryConfig) -> GraphResult<Self> {
        Ok(Self {
            inner: Arc::new(Mutex::new(ManagerInner::new(config))),
        })
    }

    /// Create manager for RTX 5090 (24GB)
    pub fn rtx_5090() -> GraphResult<Self> {
        Self::new(GpuMemoryConfig::rtx_5090())
    }

    /// Allocate GPU memory
    ///
    /// Returns an AllocationHandle that frees memory when dropped.
    ///
    /// # Arguments
    /// * `size` - Bytes to allocate
    /// * `category` - Memory category for budget tracking
    ///
    /// # Errors
    /// * `GpuResourceAllocation` if allocation exceeds budget
    pub fn allocate(
        &self,
        size: usize,
        category: MemoryCategory,
    ) -> GraphResult<AllocationHandle> {
        let id = self.inner.lock()
            .map_err(|_| GraphError::GpuResourceAllocation("Lock poisoned".into()))?
            .allocate(size, category)?;

        Ok(AllocationHandle {
            id,
            size,
            category,
            manager: self.inner.clone(),
        })
    }

    /// Get available memory in bytes
    pub fn available(&self) -> usize {
        self.inner.lock()
            .map(|inner| inner.config.total_budget.saturating_sub(inner.total_allocated))
            .unwrap_or(0)
    }

    /// Get used memory in bytes
    pub fn used(&self) -> usize {
        self.inner.lock()
            .map(|inner| inner.total_allocated)
            .unwrap_or(0)
    }

    /// Get total budget in bytes
    pub fn budget(&self) -> usize {
        self.inner.lock()
            .map(|inner| inner.config.total_budget)
            .unwrap_or(0)
    }

    /// Get memory statistics
    pub fn stats(&self) -> MemoryStats {
        self.inner.lock()
            .map(|inner| inner.stats())
            .unwrap_or_default()
    }

    /// Check if low memory condition
    pub fn is_low_memory(&self) -> bool {
        self.inner.lock()
            .map(|inner| {
                let threshold = inner.config.low_memory_threshold;
                let usage = inner.total_allocated as f32 / inner.config.total_budget as f32;
                usage > threshold
            })
            .unwrap_or(false)
    }

    /// Get available memory in specific category
    pub fn category_available(&self, category: MemoryCategory) -> usize {
        self.inner.lock()
            .map(|inner| {
                let budget = inner.config.category_budgets
                    .get(&category)
                    .copied()
                    .unwrap_or_else(|| category.default_budget());
                let used = inner.category_usage.get(&category).copied().unwrap_or(0);
                budget.saturating_sub(used)
            })
            .unwrap_or(0)
    }

    /// Try to allocate, returning None if insufficient memory
    pub fn try_allocate(
        &self,
        size: usize,
        category: MemoryCategory,
    ) -> Option<AllocationHandle> {
        self.allocate(size, category).ok()
    }
}

impl std::fmt::Debug for GpuMemoryManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stats = self.stats();
        f.debug_struct("GpuMemoryManager")
            .field("used", &stats.total_allocated)
            .field("budget", &stats.total_budget)
            .field("allocations", &stats.allocation_count)
            .finish()
    }
}
```

### Constraints
- Thread-safe via Arc<Mutex<>>
- RAII handles for automatic deallocation
- Budget enforcement per category
- RTX 5090 default: 24GB total budget
- Must prevent OOM by rejecting over-budget allocations

### Acceptance Criteria
- [ ] GpuMemoryManager struct with budget tracking
- [ ] allocate() reserves memory and returns handle
- [ ] free() releases memory back to pool (via Drop)
- [ ] available() returns remaining budget
- [ ] Returns GpuResourceAllocation error if over budget
- [ ] Thread-safe (Arc<Mutex<>> or similar)
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Memory Budget Visualization
```
RTX 5090 24GB VRAM:
+------------------+
| FAISS Index 8GB  |
+------------------+
| Hyperbolic 2.5GB |
+------------------+
| Cones 2.7GB      |
+------------------+
| Working 10.8GB   |
+------------------+
```

### Thread Safety Model
```
GpuMemoryManager (Clone)
    |
    v
Arc<Mutex<ManagerInner>>
    |
    +-- allocations: HashMap<u64, (usize, MemoryCategory)>
    +-- total_allocated: usize
    +-- category_usage: HashMap<MemoryCategory, usize>
```

### RAII Pattern
```rust
{
    let handle = manager.allocate(1_000_000, MemoryCategory::WorkingMemory)?;
    // ... use GPU memory ...
}  // handle dropped, memory freed automatically
```

### Edge Cases
- Zero-size allocation: Allowed (no-op effectively)
- Very large allocation: Reject if exceeds budget
- Concurrent allocations: Protected by mutex
- Drop during panic: Safe due to RAII
- Manager dropped while handles exist: Handles have Arc to inner

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph gpu_memory
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] Budget limits enforced
- [ ] Memory freed on handle drop
- [ ] Thread-safe access
- [ ] Stats accurate after operations

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocation_and_free() {
        let manager = GpuMemoryManager::new(
            GpuMemoryConfig::with_budget(1024 * 1024)  // 1MB
        ).unwrap();

        // Initial state
        assert_eq!(manager.used(), 0);
        assert_eq!(manager.available(), 1024 * 1024);

        // Allocate
        let handle = manager.allocate(512 * 1024, MemoryCategory::WorkingMemory).unwrap();
        assert_eq!(manager.used(), 512 * 1024);
        assert_eq!(handle.size(), 512 * 1024);

        // Free via drop
        drop(handle);
        assert_eq!(manager.used(), 0);
    }

    #[test]
    fn test_budget_enforcement() {
        let manager = GpuMemoryManager::new(
            GpuMemoryConfig::with_budget(1024)  // 1KB
        ).unwrap();

        // Allocation within budget
        let _h1 = manager.allocate(512, MemoryCategory::WorkingMemory).unwrap();

        // Allocation exceeding budget
        let result = manager.allocate(1024, MemoryCategory::WorkingMemory);
        assert!(result.is_err());
    }

    #[test]
    fn test_category_budget() {
        let config = GpuMemoryConfig::default()
            .category_budget(MemoryCategory::FaissIndex, 1024);

        let manager = GpuMemoryManager::new(config).unwrap();

        // Within category budget
        let _h1 = manager.allocate(512, MemoryCategory::FaissIndex).unwrap();

        // Exceeds category budget
        let result = manager.allocate(1024, MemoryCategory::FaissIndex);
        assert!(result.is_err());

        // Different category still works
        let _h2 = manager.allocate(1024, MemoryCategory::WorkingMemory).unwrap();
    }

    #[test]
    fn test_stats() {
        let manager = GpuMemoryManager::new(
            GpuMemoryConfig::with_budget(1024 * 1024)
        ).unwrap();

        let _h1 = manager.allocate(100_000, MemoryCategory::FaissIndex).unwrap();
        let _h2 = manager.allocate(200_000, MemoryCategory::WorkingMemory).unwrap();

        let stats = manager.stats();
        assert_eq!(stats.total_allocated, 300_000);
        assert_eq!(stats.allocation_count, 2);
        assert!(stats.category_usage.get(&MemoryCategory::FaissIndex) == Some(&100_000));
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;

        let manager = GpuMemoryManager::new(
            GpuMemoryConfig::with_budget(1024 * 1024 * 100)
        ).unwrap();

        let handles: Vec<_> = (0..10).map(|_| {
            let m = manager.clone();
            thread::spawn(move || {
                let _h = m.allocate(1024 * 1024, MemoryCategory::WorkingMemory).unwrap();
                thread::sleep(std::time::Duration::from_millis(10));
            })
        }).collect();

        for h in handles {
            h.join().unwrap();
        }

        // All allocations freed after threads complete
        assert_eq!(manager.used(), 0);
    }

    #[test]
    fn test_rtx_5090_config() {
        let manager = GpuMemoryManager::rtx_5090().unwrap();

        assert_eq!(manager.budget(), 24 * 1024 * 1024 * 1024);  // 24GB
    }

    #[test]
    fn test_try_allocate() {
        let manager = GpuMemoryManager::new(
            GpuMemoryConfig::with_budget(1024)
        ).unwrap();

        // Success
        let h = manager.try_allocate(512, MemoryCategory::WorkingMemory);
        assert!(h.is_some());

        // Failure (returns None, not error)
        let h2 = manager.try_allocate(1024, MemoryCategory::WorkingMemory);
        assert!(h2.is_none());
    }
}
```
