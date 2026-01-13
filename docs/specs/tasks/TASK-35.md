# TASK-MCP-009: Implement trigger_dream tool

```xml
<task_spec id="TASK-MCP-009" version="1.0">
<metadata>
  <title>Implement trigger_dream tool</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>35</sequence>
  <implements><requirement_ref>REQ-MCP-009</requirement_ref></implements>
  <depends_on>TASK-24</depends_on> <!-- TASK-24 = TASK-DREAM-003 (DreamEventListener to TriggerManager) -->
  <estimated_hours>3</estimated_hours>
</metadata>

<context>
The trigger_dream tool allows manual triggering of dream consolidation.
Uses the TriggerManager with Manual trigger reason.
</context>

<scope>
<in_scope>
- Define input/output schemas
- Implement handler that calls TriggerManager.set_manual_trigger()
- Check GPU eligibility before triggering
- Return trigger status and reason
</in_scope>
<out_of_scope>
- TriggerManager implementation (TASK-21)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/tools/schemas/dream.rs
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TriggerDreamInput {
    /// Rationale for manual trigger (required)
    #[schemars(length(min = 1, max = 1024))]
    pub rationale: String,

    /// Force trigger even if GPU busy (not recommended)
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TriggerDreamOutput {
    pub success: bool,
    pub trigger_accepted: bool,
    pub reason: Option<String>,
    pub gpu_utilization: Option<f32>,
    pub error: Option<String>,
}

pub async fn handle_trigger_dream(
    input: TriggerDreamInput,
    trigger_manager: &mut TriggerManager<impl GpuMonitor>,
) -> Result<TriggerDreamOutput, McpError>;
```
</signatures>
<constraints>
- MUST check GPU eligibility unless force=true
- MUST log rationale for audit
- MUST respect trigger cooldown
</constraints>
<verification>
```bash
cargo test -p context-graph-mcp trigger_dream_tool
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-mcp/src/tools/schemas/dream.rs
- crates/context-graph-mcp/src/tools/handlers/dream.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/tools/schemas/mod.rs
- crates/context-graph-mcp/src/tools/handlers/mod.rs
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp dream_tool
```
</test_commands>
</task_spec>
```
