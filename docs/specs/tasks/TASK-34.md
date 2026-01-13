# TASK-MCP-008: Implement get_coherence_state tool

```xml
<task_spec id="TASK-MCP-008" version="1.0">
<metadata>
  <title>Implement get_coherence_state tool</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>34</sequence>
  <implements><requirement_ref>REQ-MCP-008</requirement_ref></implements>
  <depends_on>TASK-12</depends_on> <!-- TASK-12 = TASK-GWT-003 (KuramotoStepper lifecycle) -->
  <estimated_hours>3</estimated_hours>
</metadata>

<context>
The get_coherence_state tool exposes GWT workspace coherence metrics including
Kuramoto order parameter and coherence threshold status.
</context>

<scope>
<in_scope>
- Define input/output schemas for get_coherence_state
- Implement handler that queries KuramotoStepper
- Return order_parameter, coherence_level, is_broadcasting, conflict_status
</in_scope>
<out_of_scope>
- KuramotoStepper implementation (TASK-12)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/tools/schemas/coherence.rs
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GetCoherenceStateInput {
    /// Include detailed oscillator phases
    #[serde(default)]
    pub include_phases: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GetCoherenceStateOutput {
    /// Kuramoto order parameter r(t) in [0, 1]
    pub order_parameter: f32,
    /// Overall coherence level
    pub coherence_level: CoherenceLevel,
    /// Whether workspace is broadcasting
    pub is_broadcasting: bool,
    /// Current conflict status
    pub has_conflict: bool,
    /// Optional oscillator phases
    pub phases: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoherenceLevel {
    High,      // r > 0.8
    Medium,    // 0.5 <= r <= 0.8
    Low,       // r < 0.5
}

// Handler
pub async fn handle_get_coherence_state(
    input: GetCoherenceStateInput,
    server: &McpServer,
) -> Result<GetCoherenceStateOutput, McpError>;
```
</signatures>
<constraints>
- order_parameter MUST be from live KuramotoStepper
- coherence_level MUST match defined thresholds
</constraints>
<verification>
```bash
cargo test -p context-graph-mcp coherence_tool
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-mcp/src/tools/schemas/coherence.rs
- crates/context-graph-mcp/src/tools/handlers/coherence.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/tools/schemas/mod.rs
- crates/context-graph-mcp/src/tools/handlers/mod.rs
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp coherence
```
</test_commands>
</task_spec>
```
