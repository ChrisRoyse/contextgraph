# TASK-MCP-012: Implement get_identity_continuity tool

```xml
<task_spec id="TASK-MCP-012" version="1.0">
<metadata>
  <title>Implement get_identity_continuity tool</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>38</sequence>
  <implements><requirement_ref>REQ-MCP-012</requirement_ref></implements>
  <depends_on>TASK-21</depends_on> <!-- TASK-21 = TASK-IDENTITY-003 (TriggerManager IC checking) -->
  <estimated_hours>2</estimated_hours>
</metadata>

<context>
The get_identity_continuity tool exposes the current IC value and crisis status.
</context>

<scope>
<in_scope>
- Define input/output schemas
- Implement handler querying TriggerManager
- Return ic_value, threshold, is_crisis, last_trigger_reason
</in_scope>
<out_of_scope>
- TriggerManager implementation (TASK-21)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/tools/schemas/identity.rs
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GetIdentityContinuityInput {}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GetIdentityContinuityOutput {
    pub ic_value: Option<f32>,
    pub threshold: f32,
    pub is_crisis: bool,
    pub last_trigger_reason: Option<String>,
}

pub async fn handle_get_identity_continuity(
    trigger_manager: &TriggerManager<impl GpuMonitor>,
) -> Result<GetIdentityContinuityOutput, McpError>;
```
</signatures>
<constraints>
- ic_value may be None if not yet calculated
- is_crisis MUST be ic_value < threshold
</constraints>
<verification>
```bash
cargo test -p context-graph-mcp identity_tool
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-mcp/src/tools/schemas/identity.rs
- crates/context-graph-mcp/src/tools/handlers/identity.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/tools/schemas/mod.rs
- crates/context-graph-mcp/src/tools/handlers/mod.rs
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp identity
```
</test_commands>
</task_spec>
```
