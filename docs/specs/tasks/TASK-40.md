# TASK-MCP-014: Implement set_coupling_strength tool

```xml
<task_spec id="TASK-MCP-014" version="1.0">
<metadata>
  <title>Implement set_coupling_strength tool</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>40</sequence>
  <implements><requirement_ref>REQ-MCP-014</requirement_ref></implements>
  <depends_on>TASK-12</depends_on> <!-- TASK-12 = TASK-GWT-003 (KuramotoStepper lifecycle) -->
  <estimated_hours>2</estimated_hours>
</metadata>

<context>
The set_coupling_strength tool allows runtime adjustment of Kuramoto coupling.
Useful for experimentation and tuning coherence dynamics.
</context>

<scope>
<in_scope>
- Define input/output schemas
- Implement handler updating KuramotoNetwork coupling
- Validate coupling is in reasonable range [0, 1]
</in_scope>
<out_of_scope>
- KuramotoNetwork implementation (TASK-11)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/tools/schemas/kuramoto.rs (append)
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SetCouplingStrengthInput {
    /// New coupling strength [0.0, 1.0]
    pub coupling: f32,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct SetCouplingStrengthOutput {
    pub success: bool,
    pub previous_coupling: f32,
    pub new_coupling: f32,
}

pub async fn handle_set_coupling_strength(
    input: SetCouplingStrengthInput,
    server: &mut McpServer,
) -> Result<SetCouplingStrengthOutput, McpError>;
```
</signatures>
<constraints>
- coupling MUST be in [0.0, 1.0]
- MUST return previous value
</constraints>
<verification>
```bash
cargo test -p context-graph-mcp coupling_tool
```
</verification>
</definition_of_done>

<files_to_create>
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/tools/schemas/kuramoto.rs
- crates/context-graph-mcp/src/tools/handlers/kuramoto.rs
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp kuramoto
```
</test_commands>
</task_spec>
```
