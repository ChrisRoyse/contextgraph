# TASK-PERF-004: Remove block_on from gwt_providers

```xml
<task_spec id="TASK-PERF-004" version="1.0">
<metadata>
  <title>Remove block_on from gwt_providers</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>16</sequence>
  <implements><requirement_ref>REQ-PERF-004</requirement_ref></implements>
  <depends_on>TASK-07</depends_on>
  <estimated_hours>2</estimated_hours>
</metadata>

<context>
The gwt_providers implementation uses futures::executor::block_on() to bridge
sync trait methods with async internals. This causes deadlocks on single-threaded
tokio runtime. Now that traits are async (TASK-PERF-002), implementations can use
.await directly, eliminating the deadlock.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/gwt_providers.rs
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/gwt_traits.rs (from TASK-PERF-002)
</input_context_files>

<scope>
<in_scope>
- Remove all block_on() calls from WorkspaceProviderImpl
- Remove all block_on() calls from MetaCognitiveProviderImpl
- Replace std::sync::RwLock with tokio::sync::RwLock
- Add #[async_trait] to impl blocks
- Update internal locks to use .await instead of .lock()
</in_scope>
<out_of_scope>
- Trait definitions (TASK-PERF-002, TASK-PERF-003)
- Other providers in different modules
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/handlers/gwt_providers.rs
use crate::handlers::gwt_traits::{WorkspaceProvider, MetaCognitiveProvider};
use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;
use std::sync::Arc;

/// Async workspace provider implementation.
///
/// Uses tokio::sync::RwLock instead of std::sync::RwLock.
/// No block_on() calls - all methods are properly async.
pub struct WorkspaceProviderImpl {
    workspace: Arc<RwLock<GwtWorkspace>>,
}

#[async_trait]
impl WorkspaceProvider for WorkspaceProviderImpl {
    async fn get_active_memory(&self) -> Option<Uuid> {
        let workspace = self.workspace.read().await;
        workspace.active_memory()
    }

    async fn is_broadcasting(&self) -> bool {
        let workspace = self.workspace.read().await;
        workspace.is_broadcasting()
    }

    async fn has_conflict(&self) -> bool {
        let workspace = self.workspace.read().await;
        workspace.has_conflict()
    }

    async fn get_conflict_details(&self) -> Option<String> {
        let workspace = self.workspace.read().await;
        workspace.conflict_details()
    }

    async fn coherence_threshold(&self) -> f32 {
        let workspace = self.workspace.read().await;
        workspace.coherence_threshold()
    }
}

/// Async meta-cognitive provider implementation.
pub struct MetaCognitiveProviderImpl {
    meta_cognitive: Arc<RwLock<MetaCognitiveState>>,
}

#[async_trait]
impl MetaCognitiveProvider for MetaCognitiveProviderImpl {
    async fn acetylcholine(&self) -> f32 {
        let state = self.meta_cognitive.read().await;
        state.acetylcholine()
    }

    async fn monitoring_frequency(&self) -> f32 {
        let state = self.meta_cognitive.read().await;
        state.monitoring_frequency()
    }

    async fn get_recent_scores(&self, count: usize) -> Vec<f32> {
        let state = self.meta_cognitive.read().await;
        state.recent_scores(count)
    }
}
```
</signatures>
<constraints>
- MUST NOT contain any block_on() calls
- MUST NOT contain any futures::executor imports
- MUST use tokio::sync::RwLock, not std::sync::RwLock
- All lock acquisitions MUST use .await
</constraints>
<verification>
```bash
cargo check -p context-graph-mcp
# Verify no block_on
grep -r "block_on" crates/context-graph-mcp/src/handlers/ && exit 1 || echo "OK"
# Verify no futures::executor
grep -r "futures::executor" crates/context-graph-mcp/src/handlers/ && exit 1 || echo "OK"
```
</verification>
</definition_of_done>

<files_to_create>
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/handlers/gwt_providers.rs
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp
cargo clippy -p context-graph-mcp -- -D warnings
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Deadlock Prevention

The key change is from:
```rust
// BEFORE - DEADLOCK on single-threaded runtime
fn get_active_memory(&self) -> Option<Uuid> {
    futures::executor::block_on(async {
        self.workspace.read().await.active_memory()
    })
}
```

To:
```rust
// AFTER - No deadlock
async fn get_active_memory(&self) -> Option<Uuid> {
    self.workspace.read().await.active_memory()
}
```

### Why tokio::sync::RwLock?

- `std::sync::RwLock` blocks the thread
- `tokio::sync::RwLock` yields to the runtime
- In async context, always prefer async-aware locks

### Call Site Updates

All callers of these providers need to add `.await`:
```rust
// Before
let memory = provider.get_active_memory();

// After
let memory = provider.get_active_memory().await;
```
