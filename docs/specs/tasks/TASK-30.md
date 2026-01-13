# TASK-MCP-004: Implement merge_concepts handler

```xml
<task_spec id="TASK-MCP-004" version="1.0">
<metadata>
  <title>Implement merge_concepts handler</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>30</sequence>
  <implements><requirement_ref>REQ-MCP-004</requirement_ref></implements>
  <depends_on>TASK-29</depends_on>
  <estimated_hours>6</estimated_hours>
</metadata>

<context>
The merge_concepts handler merges concept nodes using the specified strategy.
It must store reversal information for 30-day undo capability.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/schemas/merge_concepts.rs (from TASK-MCP-003)
</input_context_files>

<scope>
<in_scope>
- Implement handler for merge_concepts
- Implement union, intersection, weighted_average merge strategies
- Generate reversal_hash for undo capability
- Store reversal information for 30 days
- Handle missing source nodes gracefully
</in_scope>
<out_of_scope>
- Schema definition (TASK-MCP-003)
- Reversal expiration cleanup
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/tools/handlers/merge_concepts.rs
use crate::tools::schemas::merge_concepts::*;

/// Handle merge_concepts tool call.
pub async fn handle_merge_concepts(
    input: MergeConceptsInput,
    graph: &mut ContextGraph,
) -> Result<MergeConceptsOutput, McpError>;

/// Merge embeddings using union strategy.
fn merge_union(embeddings: Vec<Vec<f32>>) -> Vec<f32>;

/// Merge embeddings using intersection strategy.
fn merge_intersection(embeddings: Vec<Vec<f32>>) -> Vec<f32>;

/// Merge embeddings using weighted average.
fn merge_weighted_average(embeddings: Vec<Vec<f32>>, weights: Option<&[f32]>) -> Vec<f32>;

/// Generate reversal hash and store reversal data.
fn create_reversal_record(
    source_ids: &[uuid::Uuid],
    merged_id: uuid::Uuid,
    original_data: Vec<NodeData>,
) -> String;
```
</signatures>
<constraints>
- All source nodes MUST exist or return error
- reversal_hash MUST be unique
- Reversal data MUST be stored for 30 days
- Merged node MUST inherit edges from sources
</constraints>
<verification>
```bash
cargo test -p context-graph-mcp merge_handler
cargo test -p context-graph-mcp test_merge_strategies
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-mcp/src/tools/handlers/merge_concepts.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/tools/handlers/mod.rs
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp handlers
```
</test_commands>
</task_spec>
```
