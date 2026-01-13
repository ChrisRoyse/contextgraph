# TASK-MCP-002: Implement epistemic_action handler

```xml
<task_spec id="TASK-MCP-002" version="1.0">
<metadata>
  <title>Implement epistemic_action handler</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>28</sequence>
  <implements><requirement_ref>REQ-MCP-002</requirement_ref></implements>
  <depends_on>TASK-27</depends_on>
  <estimated_hours>4</estimated_hours>
</metadata>

<context>
The epistemic_action handler processes epistemic actions on the GWT workspace.
It validates input, executes the action, and returns workspace state changes.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/schemas/epistemic_action.rs (from TASK-MCP-001)
</input_context_files>

<scope>
<in_scope>
- Create handler function for epistemic_action
- Implement each action type (assert, retract, query, hypothesize, verify)
- Calculate workspace state changes
- Handle errors with descriptive messages
- Log all actions for audit trail
</in_scope>
<out_of_scope>
- Schema definition (TASK-MCP-001)
- Tool registration (TASK-MCP-015)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/tools/handlers/epistemic_action.rs
use crate::tools::schemas::epistemic_action::*;

/// Handle epistemic_action tool call.
pub async fn handle_epistemic_action(
    input: EpistemicActionInput,
    workspace: &mut GwtWorkspace,
) -> Result<EpistemicActionOutput, McpError> {
    // Validate input
    validate_epistemic_input(&input)?;

    // Execute action based on type
    let result = match input.action_type {
        EpistemicActionType::Assert => execute_assert(input, workspace).await,
        EpistemicActionType::Retract => execute_retract(input, workspace).await,
        EpistemicActionType::Query => execute_query(input, workspace).await,
        EpistemicActionType::Hypothesize => execute_hypothesize(input, workspace).await,
        EpistemicActionType::Verify => execute_verify(input, workspace).await,
    }?;

    Ok(result)
}

async fn execute_assert(input: EpistemicActionInput, workspace: &mut GwtWorkspace)
    -> Result<EpistemicActionOutput, McpError>;
async fn execute_retract(input: EpistemicActionInput, workspace: &mut GwtWorkspace)
    -> Result<EpistemicActionOutput, McpError>;
async fn execute_query(input: EpistemicActionInput, workspace: &mut GwtWorkspace)
    -> Result<EpistemicActionOutput, McpError>;
async fn execute_hypothesize(input: EpistemicActionInput, workspace: &mut GwtWorkspace)
    -> Result<EpistemicActionOutput, McpError>;
async fn execute_verify(input: EpistemicActionInput, workspace: &mut GwtWorkspace)
    -> Result<EpistemicActionOutput, McpError>;
```
</signatures>
<constraints>
- All actions MUST be logged with rationale
- Errors MUST include descriptive messages
- Handler MUST return EpistemicActionOutput
- workspace_state MUST reflect actual changes
</constraints>
<verification>
```bash
cargo test -p context-graph-mcp epistemic_handler
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-mcp/src/tools/handlers/epistemic_action.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/tools/handlers/mod.rs (add epistemic_action module)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp handlers
```
</test_commands>
</task_spec>
```
