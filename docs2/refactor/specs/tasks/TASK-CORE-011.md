# TASK-CORE-011: GPU Memory Management Infrastructure

## Metadata

| Field | Value |
|-------|-------|
| **Task ID** | TASK-CORE-011 |
| **Title** | GPU Memory Management Infrastructure |
| **Status** | :yellow_circle: in_progress (infrastructure exists, needs slot management) |
| **Layer** | Foundation |
| **Sequence** | 11 |
| **Complexity** | High |

## Implements

- **ARCH-08**: CUDA GPU is Required for Production
- **AP-007**: No CPU fallback in production builds
- **Performance Budget**: GPU memory usage < 8GB for 13 models

---

## CRITICAL CONTEXT FOR IMPLEMENTING AGENT

### What Already Exists (DO NOT RECREATE)

The following GPU infrastructure already exists in the codebase:

| File | What It Contains | Status |
|------|------------------|--------|
| `crates/context-graph-embeddings/src/gpu/memory/pool.rs` | `GpuMemoryPool` (thread-safe) | EXISTS |
| `crates/context-graph-embeddings/src/gpu/memory/tracker.rs` | `VramTracker` (allocation tracking) | EXISTS |
| `crates/context-graph-embeddings/src/gpu/memory/budget.rs` | `MemoryBudget` (RTX 5090 config) | EXISTS |
| `crates/context-graph-embeddings/src/gpu/memory/error.rs` | `MemoryError` enum | EXISTS |
| `crates/context-graph-embeddings/src/gpu/memory/stats.rs` | `MemoryStats` struct | EXISTS |
| `crates/context-graph-embeddings/src/gpu/device/core.rs` | GPU singleton with fail-fast | EXISTS |
| `crates/context-graph-cuda/src/error.rs` | `CudaError` enum | EXISTS |
| `crates/context-graph-embeddings/src/warm/cuda_alloc/allocator.rs` | `WarmCudaAllocator` | EXISTS |
| `crates/context-graph-embeddings/src/warm/cuda_alloc/allocator_cuda.rs` | CUDA FFI implementation | EXISTS |

### What This Task MUST Add

1. **Model Slot Management**: Manage 13 embedding model slots within VRAM budget
2. **Memory Pressure Detection**: `MemoryPressure` enum with thresholds
3. **Eviction Policy**: When >95% VRAM used, evict least-recently-used model
4. **8GB Budget Enforcement**: Strict enforcement of <8GB for 13 models
5. **Integration Tests**: Verify all 13 slots can be allocated within budget

---

## Dependencies

| Task | Reason | Status |
|------|--------|--------|
| None | Foundation task | Ready |

**Note**: This task has NO upstream dependencies but blocks TASK-CORE-012 (Embedding Model Loading).

---

## Objective

Extend the existing GPU memory management infrastructure to support:
1. **Model slot management** for exactly 13 embedding models (E1-E13)
2. **Memory pressure detection** with configurable thresholds
3. **Eviction policy** for memory-constrained scenarios
4. **Budget enforcement** of <8GB total for 13 models

**Target Hardware**: RTX 5090 (32GB VRAM), CUDA 13.1, Compute Capability 12.0

---

## Scope

### In Scope

1. Add `ModelSlotManager` struct for tracking 13 model slots
2. Add `MemoryPressure` enum (Low/Medium/High/Critical)
3. Add `pressure_level()` method to `GpuMemoryPool`
4. Add eviction callbacks for memory pressure > 95%
5. Add budget enforcement (reject allocations exceeding 8GB)
6. Integration tests with synthetic allocations

### Out of Scope

- Model loading (TASK-CORE-012)
- Quantization (TASK-CORE-013)
- Specific embedding model implementations

---

## Definition of Done

### 1. Add Memory Pressure Detection

**File**: `crates/context-graph-embeddings/src/gpu/memory/pressure.rs` (NEW)

```rust
/// Memory pressure levels for eviction decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemoryPressure {
    Low,      // <50% used
    Medium,   // 50-80% used
    High,     // 80-95% used
    Critical, // >95% used - triggers eviction
}

impl MemoryPressure {
    /// Calculate pressure level from utilization percentage
    ///
    /// Note: Uses f32 to match MemoryStats::utilization_percent()
    pub fn from_utilization(percent: f32) -> Self {
        match percent {
            p if p < 50.0 => MemoryPressure::Low,
            p if p < 80.0 => MemoryPressure::Medium,
            p if p < 95.0 => MemoryPressure::High,
            _ => MemoryPressure::Critical,
        }
    }

    /// Whether eviction should be triggered
    pub fn should_evict(&self) -> bool {
        *self == MemoryPressure::Critical
    }
}
```

### 2. Add Model Slot Manager

**File**: `crates/context-graph-embeddings/src/gpu/memory/slots.rs` (NEW)

```rust
use context_graph_core::teleological::embedder::Embedder;
use std::collections::HashMap;
use std::time::Instant;

/// Budget for 13 models: 8GB total
pub const MODEL_BUDGET_BYTES: usize = 8 * 1024 * 1024 * 1024;

/// Individual model slot with tracking metadata
#[derive(Debug)]
pub struct ModelSlot {
    /// Which embedder this slot belongs to
    pub embedder: Embedder,
    /// Allocated bytes for this model
    pub size_bytes: usize,
    /// When this slot was last accessed
    pub last_accessed: Instant,
    /// Whether model weights are currently loaded
    pub loaded: bool,
}

/// Manages 13 model slots within GPU memory budget
#[derive(Debug)]
pub struct ModelSlotManager {
    /// Slots indexed by embedder
    slots: HashMap<Embedder, ModelSlot>,
    /// Total allocated across all slots
    total_allocated: usize,
    /// Maximum allowed (8GB)
    budget: usize,
}

impl ModelSlotManager {
    /// Create manager with default 8GB budget
    pub fn new() -> Self {
        Self::with_budget(MODEL_BUDGET_BYTES)
    }

    /// Create manager with custom budget
    pub fn with_budget(budget: usize) -> Self {
        Self {
            slots: HashMap::with_capacity(13),
            total_allocated: 0,
            budget,
        }
    }

    /// Allocate a slot for an embedder
    ///
    /// # Errors
    /// Returns `MemoryError::OutOfMemory` if allocation would exceed budget
    pub fn allocate_slot(&mut self, embedder: Embedder, size_bytes: usize)
        -> Result<(), super::error::MemoryError>
    {
        // Check budget
        if self.total_allocated + size_bytes > self.budget {
            return Err(super::error::MemoryError::OutOfMemory {
                requested: size_bytes,
                available: self.budget.saturating_sub(self.total_allocated),
            });
        }

        let slot = ModelSlot {
            embedder,
            size_bytes,
            last_accessed: Instant::now(),
            loaded: true,
        };

        self.slots.insert(embedder, slot);
        self.total_allocated += size_bytes;

        tracing::debug!(
            embedder = ?embedder,
            size_bytes = size_bytes,
            total_allocated = self.total_allocated,
            budget = self.budget,
            "Allocated model slot"
        );

        Ok(())
    }

    /// Deallocate a slot
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

    /// Mark a slot as accessed (updates LRU timestamp)
    pub fn touch(&mut self, embedder: &Embedder) {
        if let Some(slot) = self.slots.get_mut(embedder) {
            slot.last_accessed = Instant::now();
        }
    }

    /// Get the least recently used slot for eviction
    pub fn get_lru_candidate(&self) -> Option<Embedder> {
        self.slots
            .values()
            .filter(|s| s.loaded)
            .min_by_key(|s| s.last_accessed)
            .map(|s| s.embedder)
    }

    /// Current memory pressure level
    pub fn pressure_level(&self) -> super::pressure::MemoryPressure {
        let utilization = (self.total_allocated as f32 / self.budget as f32) * 100.0;
        super::pressure::MemoryPressure::from_utilization(utilization)
    }

    /// Available bytes before hitting budget
    pub fn available(&self) -> usize {
        self.budget.saturating_sub(self.total_allocated)
    }

    /// Total allocated bytes
    pub fn allocated(&self) -> usize {
        self.total_allocated
    }

    /// Number of loaded slots
    pub fn loaded_count(&self) -> usize {
        self.slots.values().filter(|s| s.loaded).count()
    }

    /// Check if specific embedder has a slot
    pub fn has_slot(&self, embedder: &Embedder) -> bool {
        self.slots.contains_key(embedder)
    }
}

impl Default for ModelSlotManager {
    fn default() -> Self {
        Self::new()
    }
}
```

### 3. Extend GpuMemoryPool

**File**: `crates/context-graph-embeddings/src/gpu/memory/pool.rs` (MODIFY)

Add these methods to the existing `GpuMemoryPool`:

```rust
/// Get current memory pressure level
pub fn pressure_level(&self) -> MemoryPressure {
    let stats = self.stats();
    let utilization = stats.utilization_percent();
    MemoryPressure::from_utilization(utilization)
}

/// Check if eviction should be triggered
pub fn should_evict(&self) -> bool {
    self.pressure_level().should_evict()
}
```

### 4. Update Memory Module Exports

**File**: `crates/context-graph-embeddings/src/gpu/memory/mod.rs` (MODIFY)

```rust
mod budget;
mod error;
mod pool;
mod pressure;  // NEW
mod slots;     // NEW
mod stats;
mod tracker;

pub use budget::MemoryBudget;
pub use error::MemoryError;
pub use pool::GpuMemoryPool;
pub use pressure::MemoryPressure;  // NEW
pub use slots::{ModelSlot, ModelSlotManager, MODEL_BUDGET_BYTES};  // NEW
pub use stats::MemoryStats;
pub use tracker::VramTracker;
```

---

## Constraints

| Constraint | Target | Verification |
|------------|--------|--------------|
| Total GPU memory for 13 models | < 8GB | Unit test |
| Allocation latency | < 1ms | Benchmark |
| Deallocation latency | < 0.5ms | Benchmark |
| Fail-fast startup check | < 100ms | Integration test |

---

## Verification Checklist

### Unit Tests

- [ ] `MemoryPressure::from_utilization()` returns correct levels at thresholds
- [ ] `ModelSlotManager::allocate_slot()` fails when budget exceeded
- [ ] `ModelSlotManager::get_lru_candidate()` returns oldest slot
- [ ] `GpuMemoryPool::pressure_level()` reflects utilization
- [ ] All 13 embedder slots can be allocated within 8GB budget

### Integration Tests

- [ ] Create `ModelSlotManager` and allocate all 13 slots
- [ ] Verify LRU eviction works correctly
- [ ] Verify memory pressure triggers eviction callback
- [ ] Test with real CUDA allocations (feature-gated)

---

## Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `crates/context-graph-embeddings/src/gpu/memory/pressure.rs` | CREATE | Memory pressure enum |
| `crates/context-graph-embeddings/src/gpu/memory/slots.rs` | CREATE | Model slot manager |
| `crates/context-graph-embeddings/src/gpu/memory/mod.rs` | MODIFY | Export new types |
| `crates/context-graph-embeddings/src/gpu/memory/pool.rs` | MODIFY | Add pressure methods |
| `crates/context-graph-embeddings/tests/gpu_memory_slots_test.rs` | CREATE | Integration tests |

---

## Full State Verification Requirements

After implementing the logic, you MUST perform Full State Verification.

### Source of Truth

The source of truth for this task is the `ModelSlotManager` internal state:
- `slots: HashMap<Embedder, ModelSlot>`
- `total_allocated: usize`
- `budget: usize`

### Verification Protocol

1. **Execute & Inspect**: After each allocation, read the internal state and print:
   - Current `total_allocated` value
   - Number of slots in `slots` HashMap
   - Current `pressure_level()`

2. **Boundary & Edge Case Audit**: Test these 3 cases with printed before/after state:

   **Edge Case 1: Budget Boundary**
   ```rust
   // BEFORE state
   let mut manager = ModelSlotManager::with_budget(1000);
   println!("BEFORE: allocated={}, available={}", manager.allocated(), manager.available());

   // Action: Allocate exactly at budget
   manager.allocate_slot(Embedder::Semantic, 1000).unwrap();

   // AFTER state
   println!("AFTER: allocated={}, available={}", manager.allocated(), manager.available());
   assert_eq!(manager.allocated(), 1000);
   assert_eq!(manager.available(), 0);
   ```

   **Edge Case 2: Over-Budget Rejection**
   ```rust
   // BEFORE state
   let mut manager = ModelSlotManager::with_budget(1000);
   manager.allocate_slot(Embedder::Semantic, 900).unwrap();
   println!("BEFORE: allocated={}", manager.allocated());

   // Action: Try to exceed budget
   let result = manager.allocate_slot(Embedder::TemporalRecent, 200);

   // AFTER state (should be unchanged)
   println!("AFTER: allocated={}, result={:?}", manager.allocated(), result);
   assert!(result.is_err());
   assert_eq!(manager.allocated(), 900); // Unchanged
   ```

   **Edge Case 3: LRU Eviction Selection**
   ```rust
   use std::thread::sleep;
   use std::time::Duration;

   let mut manager = ModelSlotManager::with_budget(10000);

   // Allocate in order
   manager.allocate_slot(Embedder::Semantic, 1000).unwrap();
   sleep(Duration::from_millis(10));
   manager.allocate_slot(Embedder::TemporalRecent, 1000).unwrap();
   sleep(Duration::from_millis(10));
   manager.allocate_slot(Embedder::TemporalPeriodic, 1000).unwrap();

   // Touch Semantic to make it recent
   manager.touch(&Embedder::Semantic);

   // LRU should be TemporalRecent (oldest untouched)
   let lru = manager.get_lru_candidate();
   println!("LRU candidate: {:?}", lru);
   assert_eq!(lru, Some(Embedder::TemporalRecent));
   ```

3. **Evidence of Success**: The test output must show:
   ```
   [PASS] Edge Case 1: Budget boundary respected
   [PASS] Edge Case 2: Over-budget allocation rejected
   [PASS] Edge Case 3: LRU candidate correctly identified
   ```

---

## Manual Testing Protocol

### Test 1: Allocate All 13 Embedders

**Input**: Allocate slots for all 13 embedders with realistic sizes:
```rust
use context_graph_core::teleological::embedder::Embedder;

// Approximate sizes for quantized models (PQ-8, Float8, etc.)
// NOTE: Use Embedder::all() to iterate - these are the CORRECT variant names
const MODEL_SIZES: [(Embedder, usize); 13] = [
    (Embedder::Semantic, 200_000_000),         // E1: 200MB (1024D e5-large)
    (Embedder::TemporalRecent, 100_000_000),   // E2: 100MB (512D)
    (Embedder::TemporalPeriodic, 100_000_000), // E3: 100MB (512D)
    (Embedder::TemporalPositional, 100_000_000), // E4: 100MB (512D)
    (Embedder::Causal, 150_000_000),           // E5: 150MB (768D Longformer)
    (Embedder::Sparse, 50_000_000),            // E6: 50MB sparse SPLADE
    (Embedder::Code, 300_000_000),             // E7: 300MB (1536D Qodo)
    (Embedder::Graph, 75_000_000),             // E8: 75MB (384D MiniLM)
    (Embedder::Hdc, 50_000_000),               // E9: 50MB (1024D projected)
    (Embedder::Multimodal, 150_000_000),       // E10: 150MB (768D CLIP)
    (Embedder::Entity, 75_000_000),            // E11: 75MB (384D MiniLM)
    (Embedder::LateInteraction, 200_000_000),  // E12: 200MB (128D/token ColBERT)
    (Embedder::KeywordSplade, 50_000_000),     // E13: 50MB sparse SPLADE v3
];
// Total: ~1.6GB - well within 8GB budget
```

**Expected Output**:
- All 13 slots allocated successfully
- `total_allocated` = sum of all sizes
- `pressure_level()` = `MemoryPressure::Low` (< 50% of 8GB)

**Verification**: Query `manager.loaded_count()` equals 13

### Test 2: Memory Pressure Threshold

**Input**: Allocate until pressure reaches Critical
```rust
let mut manager = ModelSlotManager::with_budget(1000);
manager.allocate_slot(Embedder::Semantic, 960).unwrap(); // 96%
```

**Expected Output**:
- `pressure_level()` returns `MemoryPressure::Critical`
- `should_evict()` returns `true`

### Test 3: Real CUDA Allocation (GPU Test)

**Prerequisite**: Run with `--features cuda` on machine with GPU

```rust
#[test]
#[cfg(feature = "cuda")]
fn test_real_cuda_allocation() {
    use crate::gpu::{init_gpu, GpuMemoryPool};

    // Initialize real GPU
    let device = init_gpu().expect("GPU required for this test");

    // Create pool with real VRAM
    let pool = GpuMemoryPool::rtx_5090();

    // Allocate test block
    pool.allocate("test_model", 100_000_000).expect("allocation failed");

    // Verify
    assert!(pool.stats().allocated_bytes >= 100_000_000);
    println!("CUDA allocation verified: {} bytes", pool.stats().allocated_bytes);
}
```

---

## Error Handling Requirements

### NO Fallbacks or Workarounds

- If GPU unavailable: **FAIL with exit code 101**
- If allocation fails: **FAIL with `MemoryError::OutOfMemory`**
- If budget exceeded: **FAIL with `MemoryError::OutOfMemory`**

### Error Logging

All errors must include:
1. Error code (e.g., `EMB-E003`)
2. Requested bytes
3. Available bytes
4. Current allocations
5. Suggested remediation

Example:
```
[EMB-E003] GPU memory allocation failed
  Requested: 500,000,000 bytes
  Available: 100,000,000 bytes
  Total Budget: 8,589,934,592 bytes (8GB)
  Currently Allocated: 8,489,934,592 bytes
  Loaded Models: 12/13
  Suggestion: Evict least-recently-used model or reduce model sizes
```

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| OOM during operation | Medium | High | Memory pressure monitoring + LRU eviction |
| Budget calculation errors | Low | High | Extensive unit tests with boundary values |
| Thread safety issues | Medium | High | Use `Arc<RwLock<ModelSlotManager>>` |
| LRU timestamp precision | Low | Low | Use `Instant::now()` (monotonic) |

---

## Traceability

| Requirement | Source | Line |
|-------------|--------|------|
| ARCH-08 | `constitution.yaml` | 113-118 |
| AP-007 | `constitution.yaml` | 198-199 |
| GPU Budget 8GB | `constitution.yaml` | 239 |
| 13 Embedders | `constitution.yaml` | 322-325, 588-706 |
| RTX 5090 Target | `constitution.yaml` | 33, 901 |

---

## Notes for Implementing Agent

1. **Do NOT recreate existing infrastructure** - `GpuMemoryPool`, `VramTracker`, `MemoryError` already exist
2. **The `cuda` feature must be enabled** - Build with `cargo build --features cuda`
3. **Tests must pass without GPU** for CI - Use `#[cfg(feature = "cuda")]` for GPU-dependent tests
4. **The 8GB budget is for model weights only** - Working memory and activations have separate budgets
5. **All 13 embedders are defined in** `context_graph_core::teleological::embedder::Embedder`
