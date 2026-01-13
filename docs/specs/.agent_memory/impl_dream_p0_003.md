# Implementation Summary: TASK-DREAM-P0-003

## Task: Complete Amortized Edge Creation

**Status**: COMPLETED
**Date**: 2026-01-12
**Implementation Time**: ~15 minutes

## Changes Made

### 1. New Types Added to `amortized.rs`

#### `ShortcutEdge` struct
```rust
pub struct ShortcutEdge {
    pub source: Uuid,
    pub target: Uuid,
    pub weight: f32,
    pub confidence: f32,
    pub is_shortcut: bool,      // Always true
    pub original_path: Vec<Uuid>,
}
```

#### `EdgeCreator` trait
```rust
pub trait EdgeCreator: Send + Sync {
    fn create_edge(&self, edge: &ShortcutEdge) -> CoreResult<bool>;
}
```

#### `NullEdgeCreator` (backward compatibility)
```rust
pub struct NullEdgeCreator;

impl EdgeCreator for NullEdgeCreator {
    fn create_edge(&self, edge: &ShortcutEdge) -> CoreResult<bool> {
        debug!("NullEdgeCreator: would create edge...");
        Ok(true)
    }
}
```

### 2. `AmortizedLearner` Modifications

Added new field:
```rust
edge_creator: Option<Arc<dyn EdgeCreator>>
```

Added new methods:
- `set_edge_creator(creator: Arc<dyn EdgeCreator>)` - Sets the edge creator
- `with_edge_creator(creator) -> Self` - Constructor with creator

Updated `create_shortcut()`:
- Removed TODO stub and "Agent 2" comments
- Creates `ShortcutEdge::from_candidate()`
- Calls `edge_creator.create_edge()` when creator is set
- Properly handles creator returning `Ok(false)` (edge exists)
- Propagates errors from creator

Added manual `Debug` impl (because `Arc<dyn EdgeCreator>` is not Debug).

### 3. Re-exports in `mod.rs`

```rust
pub use amortized::{
    AmortizedLearner, EdgeCreator, NullEdgeCreator, PathSignature, ShortcutCandidate, ShortcutEdge,
};
```

## Tests Added

12 new tests for EdgeCreator functionality:
- `test_shortcut_edge_from_candidate`
- `test_shortcut_edge_is_shortcut_always_true`
- `test_null_edge_creator`
- `test_set_edge_creator`
- `test_with_edge_creator_constructor`
- `test_shortcut_creation_calls_creator`
- `test_create_shortcut_without_creator`
- `test_create_shortcut_creator_returns_false`
- `test_create_shortcut_creator_error_propagates`
- `test_quality_gate_enforcement_with_creator`
- `test_multiple_shortcuts_with_creator`

Also includes `RecordingEdgeCreator` helper for testing (records all created edges).

## Verification

```bash
# Tests pass
cargo test -p context-graph-core --lib dream::amortized -- --nocapture
# 22 passed; 0 failed

# No TODO markers
grep -r "TODO\|Agent 2" crates/context-graph-core/src/dream/amortized.rs
# No matches

# All dream tests pass
cargo test -p context-graph-core --lib dream
# 228 passed; 0 failed
```

## Constitution Compliance

- **DREAM-005**: Amortized shortcuts for 3+ hop paths traversed 5+ times
  - Quality gate enforces: `hops >= 3`, `traversals >= 5`, `confidence >= 0.7`
  - ShortcutEdge.is_shortcut is always `true`
  - ShortcutEdge.original_path contains full path

## Files Modified

1. `crates/context-graph-core/src/dream/amortized.rs` - Main implementation
2. `crates/context-graph-core/src/dream/mod.rs` - Added re-exports

## Key Design Decisions

1. **Dependency Injection**: EdgeCreator is injected via `set_edge_creator()` or constructor, allowing different implementations for graph stores.

2. **Backward Compatibility**: Without a creator, shortcuts are tracked internally but not persisted (existing behavior preserved).

3. **Error Handling**: Errors from EdgeCreator propagate up (no silent failures). `Ok(false)` means edge exists (counter not incremented).

4. **Manual Debug Impl**: Required because `Arc<dyn EdgeCreator>` doesn't implement Debug.
