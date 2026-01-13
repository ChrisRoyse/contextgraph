# TASK-MCP-013: Implement get_kuramoto_state tool

```xml
<task_spec id="TASK-MCP-013" version="1.0">
<metadata>
  <title>Implement get_kuramoto_state tool</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>39</sequence>
  <implements><requirement_ref>REQ-MCP-013</requirement_ref></implements>
  <depends_on>TASK-12</depends_on> <!-- TASK-12 = TASK-GWT-003 (KuramotoStepper lifecycle) -->
  <estimated_hours>2</estimated_hours>
</metadata>

<context>
The get_kuramoto_state tool exposes detailed Kuramoto network state
including phases, frequencies, and coupling.
</context>

<scope>
<in_scope>
- Define input/output schemas
- Implement handler querying KuramotoStepper
- Return phases, frequencies, coupling, order_parameter, mean_phase
</in_scope>
<out_of_scope>
- KuramotoStepper implementation (TASK-12)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/tools/schemas/kuramoto.rs
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GetKuramotoStateInput {}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GetKuramotoStateOutput {
    pub is_running: bool,
    pub phases: Vec<f32>,
    pub frequencies: Vec<f32>,
    pub coupling: f32,
    pub order_parameter: f32,
    pub mean_phase: f32,
}

pub async fn handle_get_kuramoto_state(
    server: &McpServer,
) -> Result<GetKuramotoStateOutput, McpError>;
```
</signatures>
<constraints>
- phases MUST have exactly 13 elements
- frequencies MUST match constitution values
</constraints>
<verification>
```bash
cargo test -p context-graph-mcp kuramoto_tool
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-mcp/src/tools/schemas/kuramoto.rs
- crates/context-graph-mcp/src/tools/handlers/kuramoto.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/tools/schemas/mod.rs
- crates/context-graph-mcp/src/tools/handlers/mod.rs
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp kuramoto
```
</test_commands>
</task_spec>
```
