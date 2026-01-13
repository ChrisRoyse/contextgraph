# TASK-MCP-003: Implement merge_concepts tool schema

```xml
<task_spec id="TASK-MCP-003" version="1.0">
<metadata>
  <title>Implement merge_concepts tool schema</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>29</sequence>
  <implements><requirement_ref>REQ-MCP-003</requirement_ref></implements>
  <depends_on></depends_on>
  <estimated_hours>2</estimated_hours>
</metadata>

<context>
The merge_concepts tool allows merging two or more related concepts into a unified node.
Includes 30-day reversal capability via reversal_hash.
</context>

<input_context_files>
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 4.3.2)
</input_context_files>

<scope>
<in_scope>
- Define input schema with source_ids, target_name, merge_strategy, rationale
- Define output schema with success, merged_id, reversal_hash, error
- Add validation constraints
</in_scope>
<out_of_scope>
- Handler implementation (TASK-MCP-004)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/tools/schemas/merge_concepts.rs
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Input schema for merge_concepts tool.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct MergeConceptsInput {
    /// UUIDs of concepts to merge (2-10)
    #[schemars(length(min = 2, max = 10))]
    pub source_ids: Vec<uuid::Uuid>,

    /// Name for the merged concept (1-256 chars)
    #[schemars(length(min = 1, max = 256))]
    pub target_name: String,

    /// Strategy for merging (default: union)
    #[serde(default)]
    pub merge_strategy: MergeStrategy,

    /// Rationale for merge (required, 1-1024 chars)
    #[schemars(length(min = 1, max = 1024))]
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    #[default]
    Union,
    Intersection,
    WeightedAverage,
}

/// Output schema for merge_concepts tool.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MergeConceptsOutput {
    pub success: bool,
    pub merged_id: Option<uuid::Uuid>,
    /// Hash for 30-day undo capability
    pub reversal_hash: Option<String>,
    pub error: Option<String>,
}
```
</signatures>
<constraints>
- source_ids MUST have 2-10 UUIDs
- target_name MUST be 1-256 characters
- rationale MUST be 1-1024 characters (required)
- merge_strategy MUST default to union
</constraints>
<verification>
```bash
cargo test -p context-graph-mcp merge_concepts_schema
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-mcp/src/tools/schemas/merge_concepts.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/tools/schemas/mod.rs
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp schemas
```
</test_commands>
</task_spec>
```
