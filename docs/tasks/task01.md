# Task 01: Fix Test Suite Compilation Errors

## Metadata
- **Task ID**: TASK-GAP-001
- **Phase**: 1 (Foundation)
- **Priority**: Critical
- **Complexity**: Medium
- **Estimated Time**: 1-2 hours
- **Dependencies**: None

## Objective

Fix the MCP test suite compilation errors caused by imports referencing modules deleted in commit `fab0622`. The tests reference `MetaUtlTracker`, `gwt_providers`, and `gwt_traits` which no longer exist. This task unblocks `cargo test --no-run -p context-graph-mcp` from running.

## Input Context

Files to READ before starting:
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tests/mod.rs` - Main test file with broken imports (lines 700-704 and functions using deleted types)
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tests/task_emb_024_verification.rs` - Test file referencing MetaUtlTracker
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tests/manual_fsv_verification.rs` - Test file referencing MetaUtlTracker
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/core/mod.rs` - Check what types are actually exported

## Files to Create/Modify

**Files to Modify:**
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tests/mod.rs`
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tests/task_emb_024_verification.rs`
- `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tests/manual_fsv_verification.rs`

## Implementation Steps

### Step 1: Fix mod.rs imports and helper functions

In `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tests/mod.rs`:

1. Remove the broken imports (around lines 700-704):
```rust
// REMOVE these lines:
use crate::handlers::core::MetaUtlTracker;
use crate::handlers::gwt_providers::{
    GwtSystemProviderImpl, MetaCognitiveProviderImpl, WorkspaceProviderImpl,
};
use crate::handlers::gwt_traits::{GwtSystemProvider, MetaCognitiveProvider, WorkspaceProvider};
```

2. Remove the helper functions that depend on deleted types:
   - `create_test_handlers_with_warm_gwt()` (around lines 729-761)
   - `create_test_handlers_with_warm_gwt_rocksdb()` (around lines 771-811)
   - `create_test_handlers_with_all_components()` (around lines 826-893)

3. Also remove the imports that are only used by these functions:
```rust
// Remove if only used by deleted functions:
use parking_lot::RwLock as ParkingRwLock;
use tokio::sync::RwLock as TokioRwLock;
use context_graph_core::monitoring::{StubLayerStatusProvider, StubSystemMonitor};
use context_graph_core::{LayerStatusProvider, SystemMonitor};
```

### Step 2: Fix task_emb_024_verification.rs

In `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tests/task_emb_024_verification.rs`:

1. Remove the broken import (line 28):
```rust
// REMOVE:
use crate::handlers::core::MetaUtlTracker;
```

2. Find and remove or stub the function `create_handlers_with_tracker()` that uses `MetaUtlTracker`

3. For any tests that depend on this function, either:
   - Rewrite them to use `create_test_handlers()` instead
   - Or add `#[ignore]` with TODO comment:
```rust
#[tokio::test]
#[ignore = "TODO: MetaUtlTracker removed in fab0622 - restore when Meta-UTL system reimplemented"]
async fn test_requiring_tracker() {
    // Test body removed
}
```

### Step 3: Fix manual_fsv_verification.rs

In `/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tests/manual_fsv_verification.rs`:

1. Remove the broken import (line 21):
```rust
// REMOVE:
use crate::handlers::core::MetaUtlTracker;
```

2. Find all usages of `MetaUtlTracker::new()` and related tracker code
3. Update affected test functions to use `create_test_handlers()` instead
4. For tests that cannot be easily fixed, add `#[ignore]` with TODO comment

## Code/Content to Implement

### mod.rs - Lines to Remove (approximately 700-893)

```rust
// ============================================================================
// REMOVED: GWT Test Helpers - modules deleted in fab0622
// ============================================================================
// The following imports and functions were removed because the underlying
// modules (gwt_providers, gwt_traits, MetaUtlTracker) no longer exist after
// PRD v6 compliance refactor.
//
// Removed imports:
// - crate::handlers::core::MetaUtlTracker
// - crate::handlers::gwt_providers::*
// - crate::handlers::gwt_traits::*
//
// Removed functions:
// - create_test_handlers_with_warm_gwt()
// - create_test_handlers_with_warm_gwt_rocksdb()
// - create_test_handlers_with_all_components()
//
// Tests requiring these helpers should use create_test_handlers() or
// create_test_handlers_with_rocksdb() instead.
// ============================================================================
```

### Example ignored test template

```rust
#[tokio::test]
#[ignore = "TODO: MetaUtlTracker removed in fab0622 - restore when Meta-UTL system reimplemented"]
async fn test_meta_utl_tracker_verification() {
    // Original test depended on deleted MetaUtlTracker module
    // When Meta-UTL system is reimplemented, restore this test with new API
}
```

## Definition of Done

- [ ] `cargo test --no-run -p context-graph-mcp` completes without compilation errors
- [ ] No imports reference deleted modules (`MetaUtlTracker`, `gwt_providers`, `gwt_traits`)
- [ ] No helper functions reference deleted types
- [ ] Tests that cannot be easily migrated are marked with `#[ignore]` and TODO comment
- [ ] `cargo clippy -p context-graph-mcp -- -D warnings` passes without errors
- [ ] Existing test coverage preserved where possible (tests using `create_test_handlers()` still work)

## Verification

```bash
cd /home/cabdru/contextgraph

# Step 1: Verify tests compile
cargo test --no-run -p context-graph-mcp

# Step 2: Run clippy
cargo clippy -p context-graph-mcp -- -D warnings

# Step 3: Verify no references to deleted modules
grep -r "MetaUtlTracker" crates/context-graph-mcp/src/handlers/tests/
grep -r "gwt_providers" crates/context-graph-mcp/src/handlers/tests/
grep -r "gwt_traits" crates/context-graph-mcp/src/handlers/tests/
# Should return empty results

# Step 4: Run available tests to ensure nothing regressed
cargo test -p context-graph-mcp --lib
```
