# TASK-PERF-001: Add async-trait to MCP crate

```xml
<task_spec id="TASK-PERF-001" version="1.0">
<metadata>
  <title>Add async-trait to MCP crate</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>6</sequence>
  <implements><requirement_ref>REQ-PERF-001</requirement_ref></implements>
  <depends_on></depends_on>
  <estimated_hours>0.5</estimated_hours>
</metadata>

<context>
The MCP crate needs async trait support to convert synchronous provider traits to async.
This eliminates block_on() calls that cause deadlocks in single-threaded runtimes.
Constitution: AP-08 prohibits sync I/O in async context.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-mcp/Cargo.toml
</input_context_files>

<scope>
<in_scope>
- Add async-trait = "0.1" dependency to context-graph-mcp
- Verify dependency resolves correctly
</in_scope>
<out_of_scope>
- Trait conversion (TASK-PERF-002, TASK-PERF-003)
- Implementation updates (TASK-PERF-004)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```toml
# crates/context-graph-mcp/Cargo.toml
[dependencies]
async-trait = "0.1"
```
</signatures>
<constraints>
- Version must be "0.1" (stable async-trait API)
- Must not introduce conflicting dependency versions
</constraints>
<verification>
```bash
cargo check -p context-graph-mcp
cargo tree -p context-graph-mcp | grep async-trait
```
</verification>
</definition_of_done>

<files_to_create>
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/Cargo.toml
</files_to_modify>

<test_commands>
```bash
cargo check -p context-graph-mcp
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Why async-trait?

Rust's native async traits (stabilized in 1.75) have limitations with dyn trait objects.
async-trait provides:
- `#[async_trait]` macro for trait definitions
- Box<dyn Future> under the hood
- Compatibility with tokio and other runtimes

### Usage Pattern

```rust
use async_trait::async_trait;

#[async_trait]
pub trait WorkspaceProvider: Send + Sync {
    async fn get_active_memory(&self) -> Option<Uuid>;
}
```
