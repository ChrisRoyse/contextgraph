# TASK-PERF-002: Convert WorkspaceProvider to async

```xml
<task_spec id="TASK-PERF-002" version="1.0">
<metadata>
  <title>Convert WorkspaceProvider to async</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>7</sequence>
  <implements><requirement_ref>REQ-PERF-002</requirement_ref></implements>
  <depends_on>TASK-06</depends_on>
  <estimated_hours>2</estimated_hours>
</metadata>

<context>
WorkspaceProvider trait currently has sync methods that internally call block_on(),
causing deadlocks on single-threaded tokio runtime. Converting to async traits
eliminates this deadlock risk per AP-08.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/gwt_traits.rs (existing trait)
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 4.1)
</input_context_files>

<scope>
<in_scope>
- Add #[async_trait] to WorkspaceProvider trait
- Convert all methods to async fn
- Update Send + Sync bounds
- Add documentation explaining async requirement
</in_scope>
<out_of_scope>
- Implementation updates (TASK-PERF-004)
- MetaCognitiveProvider (TASK-PERF-003)
- Call site updates
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/handlers/gwt_traits.rs
use async_trait::async_trait;
use uuid::Uuid;

/// Workspace provider trait for GWT integration.
///
/// All methods are async to prevent deadlock with single-threaded runtimes.
/// Constitution: AP-08 ("No sync I/O in async context")
#[async_trait]
pub trait WorkspaceProvider: Send + Sync {
    /// Get the currently active memory UUID.
    async fn get_active_memory(&self) -> Option<Uuid>;

    /// Check if workspace is currently broadcasting.
    async fn is_broadcasting(&self) -> bool;

    /// Check if workspace has conflicts.
    async fn has_conflict(&self) -> bool;

    /// Get conflict details if any.
    async fn get_conflict_details(&self) -> Option<String>;

    /// Get the coherence threshold.
    async fn coherence_threshold(&self) -> f32;
}
```
</signatures>
<constraints>
- Trait MUST have #[async_trait] attribute
- All methods MUST be async fn (no blocking)
- Trait MUST require Send + Sync for thread safety
- Documentation MUST reference AP-08
</constraints>
<verification>
```bash
cargo check -p context-graph-mcp
# Verify async-trait usage
grep -q "#\[async_trait\]" crates/context-graph-mcp/src/handlers/gwt_traits.rs
```
</verification>
</definition_of_done>

<files_to_create>
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/handlers/gwt_traits.rs
</files_to_modify>

<test_commands>
```bash
cargo check -p context-graph-mcp
cargo doc -p context-graph-mcp --no-deps
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Migration Strategy

The trait change is a breaking API change. Implementations must be updated:

Before:
```rust
impl WorkspaceProvider for MyProvider {
    fn get_active_memory(&self) -> Option<Uuid> {
        // sync implementation
    }
}
```

After:
```rust
#[async_trait]
impl WorkspaceProvider for MyProvider {
    async fn get_active_memory(&self) -> Option<Uuid> {
        // async implementation
    }
}
```

### Compatibility Notes

- Existing tests will need `.await` at call sites
- Mock implementations need `#[async_trait]` attribute
- Any code using `dyn WorkspaceProvider` continues to work
