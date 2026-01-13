# TASK-MCP-005: Implement get_johari_classification tool

```xml
<task_spec id="TASK-MCP-005" version="1.0">
<metadata>
  <title>Implement get_johari_classification tool</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>31</sequence>
  <implements><requirement_ref>REQ-MCP-005</requirement_ref></implements>
  <depends_on>TASK-09</depends_on>
  <estimated_hours>3</estimated_hours>
</metadata>

<context>
The get_johari_classification tool returns the Johari quadrant for a query
based on surprise (deltaS) and confidence (deltaC) metrics. Uses the fixed
action mapping from TASK-UTL-001.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-utl/src/johari/retrieval/functions.rs (from TASK-UTL-001)
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 4.3.3)
</input_context_files>

<scope>
<in_scope>
- Define input/output schemas for get_johari_classification
- Implement handler calling UTL johari functions
- Return quadrant, deltaS, deltaC, suggested_action, explanation
</in_scope>
<out_of_scope>
- Johari calculation internals (in UTL crate)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/tools/schemas/johari.rs
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct JohariClassificationInput {
    /// Query to classify (1-4096 chars)
    #[schemars(length(min = 1, max = 4096))]
    pub query: String,

    /// Optional context nodes
    pub context_nodes: Option<Vec<uuid::Uuid>>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct JohariClassificationOutput {
    pub quadrant: JohariQuadrant,
    pub delta_s: f32,
    pub delta_c: f32,
    pub suggested_action: SuggestedAction,
    pub explanation: String,
}

// crates/context-graph-mcp/src/tools/handlers/johari.rs
pub async fn handle_get_johari_classification(
    input: JohariClassificationInput,
    graph: &ContextGraph,
) -> Result<JohariClassificationOutput, McpError>;
```
</signatures>
<constraints>
- suggested_action MUST use TASK-UTL-001 mapping
- delta_s and delta_c MUST be in [0.0, 1.0]
- explanation MUST describe why the quadrant was chosen
</constraints>
<verification>
```bash
cargo test -p context-graph-mcp johari_tool
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-mcp/src/tools/schemas/johari.rs
- crates/context-graph-mcp/src/tools/handlers/johari.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/tools/schemas/mod.rs
- crates/context-graph-mcp/src/tools/handlers/mod.rs
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp johari
```
</test_commands>
</task_spec>
```
