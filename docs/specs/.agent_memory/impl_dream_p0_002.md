# TASK-DREAM-P0-002: Integrate MemoryStore with NREM Replay

**Implementation Status**: COMPLETED
**Implementation Date**: 2026-01-12
**Agent**: Implementation Coder

## Summary

Successfully integrated MemoryStore with NREM Replay by implementing the `MemoryProvider` trait and modifying `NremPhase` to use dependency injection for memory retrieval.

## Changes Made

### 1. `MemoryProvider` Trait (nrem.rs:63-92)

Defined a new trait for memory retrieval:

```rust
pub trait MemoryProvider: Send + Sync + std::fmt::Debug {
    fn get_recent_memories(&self, limit: usize, recency_bias: f32) -> Vec<(Uuid, u64, f32)>;
    fn get_edges_for_memories(&self, memory_ids: &[Uuid]) -> Vec<(Uuid, Uuid, f32)>;
}
```

### 2. `NullMemoryProvider` Struct (nrem.rs:95-119)

Created backward-compatible implementation that returns empty vectors:

```rust
#[derive(Debug, Clone, Default)]
pub struct NullMemoryProvider;

impl MemoryProvider for NullMemoryProvider {
    fn get_recent_memories(&self, _limit: usize, _recency_bias: f32) -> Vec<(Uuid, u64, f32)> {
        Vec::new()
    }

    fn get_edges_for_memories(&self, _memory_ids: &[Uuid]) -> Vec<(Uuid, Uuid, f32)> {
        Vec::new()
    }
}
```

### 3. Modified `NremPhase` Struct (nrem.rs:138-158)

Added `memory_provider` field:

```rust
pub struct NremPhase {
    // ... existing fields ...
    memory_provider: Option<Arc<dyn MemoryProvider>>,
}
```

### 4. New Methods Added

- `set_memory_provider(&mut self, provider: Arc<dyn MemoryProvider>)` - Inject memory provider
- `clear_memory_provider(&mut self)` - Clear provider (revert to empty data)
- `has_memory_provider(&self) -> bool` - Check if provider is set

### 5. Modified `process()` Method (nrem.rs:280-453)

Changed from hardcoded empty vectors to provider calls:

```rust
// Step 1: Get memories from provider (or empty if no provider)
let memories = match &self.memory_provider {
    Some(provider) => provider.get_recent_memories(100, self.recency_bias),
    None => Vec::new(),
};

// Step 3: Get edges for selected memories from provider
let edges = match &self.memory_provider {
    Some(provider) => provider.get_edges_for_memories(&selected_memory_ids),
    None => Vec::new(),
};
```

### 6. Updated Re-exports (mod.rs:76)

```rust
pub use nrem::{MemoryProvider, NremPhase, NremReport, NullMemoryProvider};
```

## Tests Added

| Test Name | Purpose |
|-----------|---------|
| `test_null_memory_provider` | Verify NullMemoryProvider returns empty |
| `test_memory_provider_trait` | Test trait implementation with TestMemoryProvider |
| `test_set_memory_provider` | Verify provider injection and clearing |
| `test_nrem_with_provider` | Integration test with 3 memories and 2 edges |
| `test_nrem_with_provider_processes_edges` | Verify Hebbian strengthening with high phi |
| `test_nrem_without_provider_backward_compat` | Backward compatibility when no provider |
| `test_nrem_provider_edge_filtering` | Verify edges filtered to selected memories |
| `test_nrem_phase_clone_with_provider` | Test Clone with provider |
| `test_nrem_phase_debug_with_provider` | Test Debug trait with provider |

## Constitution Compliance

- **DREAM-001**: Hebbian replay formula (dw = eta * phi_i * phi_j) still applied via HebbianEngine
- **AP-35**: No stub returns when provider is set - real data flows through
- **AP-36**: nrem.rs TODO stubs replaced with provider calls

## Verification Results

```bash
# Tests passed: 17/17
cargo test -p context-graph-core --lib dream::nrem -- --nocapture

# Vec::new() locations verified:
# - NullMemoryProvider (by design)
# - None branches in process() (backward compatibility)
```

## Files Modified

1. `/home/cabdru/contextgraph/crates/context-graph-core/src/dream/nrem.rs`
   - Added MemoryProvider trait
   - Added NullMemoryProvider struct
   - Modified NremPhase struct with memory_provider field
   - Added set_memory_provider(), clear_memory_provider(), has_memory_provider()
   - Modified process() to use provider
   - Added 9 new tests

2. `/home/cabdru/contextgraph/crates/context-graph-core/src/dream/mod.rs`
   - Added re-exports for MemoryProvider and NullMemoryProvider

## Usage Example

```rust
use context_graph_core::dream::{NremPhase, MemoryProvider};

// Create NREM phase
let mut phase = NremPhase::new();

// Inject memory provider when graph store is available
phase.set_memory_provider(Arc::new(my_memory_store));

// Process will now use real memories and edges
let report = phase.process(&interrupt_flag, &mut amortizer).await?;
println!("Replayed {} memories", report.memories_replayed);
```

## Next Steps (Out of Scope)

- Implement a real `MemoryStore` that implements `MemoryProvider`
- Connect to graph store for persistent memory retrieval
- Persist edge weight updates from HebbianEngine back to graph store
