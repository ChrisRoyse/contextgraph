# TASK-PERF-003: Convert MetaCognitiveProvider to async

```xml
<task_spec id="TASK-PERF-003" version="1.0">
<metadata>
  <title>Convert MetaCognitiveProvider to async</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>8</sequence>
  <implements><requirement_ref>REQ-PERF-003</requirement_ref></implements>
  <depends_on>TASK-06</depends_on>
  <estimated_hours>1.5</estimated_hours>
</metadata>

<context>
MetaCognitiveProvider trait has sync methods that may block. Converting to async
ensures consistency with WorkspaceProvider and prevents future deadlock issues.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/gwt_traits.rs
</input_context_files>

<scope>
<in_scope>
- Add #[async_trait] to MetaCognitiveProvider trait
- Convert all methods to async fn
- Update Send + Sync bounds
</in_scope>
<out_of_scope>
- WorkspaceProvider (TASK-PERF-002)
- Implementation updates (TASK-PERF-004)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/handlers/gwt_traits.rs

/// Meta-cognitive provider trait.
///
/// All methods are async per AP-08.
#[async_trait]
pub trait MetaCognitiveProvider: Send + Sync {
    /// Get current acetylcholine level.
    async fn acetylcholine(&self) -> f32;

    /// Get monitoring frequency in Hz.
    async fn monitoring_frequency(&self) -> f32;

    /// Get recent coherence scores.
    async fn get_recent_scores(&self, count: usize) -> Vec<f32>;
}
```
</signatures>
<constraints>
- Trait MUST have #[async_trait] attribute
- All methods MUST be async fn
- Trait MUST require Send + Sync
</constraints>
<verification>
```bash
cargo check -p context-graph-mcp
grep -q "MetaCognitiveProvider" crates/context-graph-mcp/src/handlers/gwt_traits.rs
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
```
</test_commands>
</task_spec>
```
