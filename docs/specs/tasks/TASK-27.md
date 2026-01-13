# TASK-MCP-001: Implement epistemic_action tool schema

```xml
<task_spec id="TASK-MCP-001" version="1.0">
<metadata>
  <title>Implement epistemic_action tool schema</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>27</sequence>
  <implements><requirement_ref>REQ-MCP-001</requirement_ref></implements>
  <depends_on></depends_on>
  <estimated_hours>2</estimated_hours>
</metadata>

<context>
The epistemic_action MCP tool allows clients to perform epistemic actions on the GWT
workspace to update uncertainty and knowledge states. This task defines the JSON schema
for input/output validation.
</context>

<input_context_files>
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 4.3.1)
</input_context_files>

<scope>
<in_scope>
- Create tool schema definition file
- Define input schema with action_type, target, confidence, rationale, context
- Define output schema with success, action_id, workspace_state, error
- Add validation constraints (min/max lengths, enums)
</in_scope>
<out_of_scope>
- Handler implementation (TASK-MCP-002)
- Tool registration (TASK-MCP-015)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/tools/schemas/epistemic_action.rs
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Input schema for epistemic_action tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct EpistemicActionInput {
    /// Type of epistemic action to perform
    pub action_type: EpistemicActionType,

    /// Target concept or proposition (1-4096 chars)
    #[schemars(length(min = 1, max = 4096))]
    pub target: String,

    /// Confidence level (0.0-1.0, default 0.5)
    #[serde(default = "default_confidence")]
    pub confidence: f32,

    /// Rationale for action (required, 1-1024 chars)
    #[schemars(length(min = 1, max = 1024))]
    pub rationale: String,

    /// Optional context
    pub context: Option<EpistemicContext>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EpistemicActionType {
    Assert,
    Retract,
    Query,
    Hypothesize,
    Verify,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct EpistemicContext {
    pub source_nodes: Option<Vec<uuid::Uuid>>,
    pub uncertainty_type: Option<UncertaintyType>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UncertaintyType {
    Epistemic,
    Aleatory,
    Mixed,
}

/// Output schema for epistemic_action tool.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct EpistemicActionOutput {
    pub success: bool,
    pub action_id: Option<uuid::Uuid>,
    pub workspace_state: Option<WorkspaceStateChange>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WorkspaceStateChange {
    pub uncertainty_delta: f32,
    pub coherence_impact: f32,
    pub triggered_dream: bool,
}
```
</signatures>
<constraints>
- target MUST be 1-4096 characters
- rationale MUST be 1-1024 characters (required per PRD 0.3)
- confidence MUST be in [0.0, 1.0]
- action_type MUST be one of enum variants
</constraints>
<verification>
```bash
cargo check -p context-graph-mcp
cargo test -p context-graph-mcp epistemic_action_schema
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-mcp/src/tools/schemas/mod.rs
- crates/context-graph-mcp/src/tools/schemas/epistemic_action.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/tools/mod.rs (add schemas module)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp schemas
```
</test_commands>
</task_spec>
```
