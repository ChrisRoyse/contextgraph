# TASK-CORE-001: Remove Protocol Constants

```xml
<task_spec id="TASK-CORE-001" version="1.0">
<metadata>
  <title>Remove Protocol Constants for Deprecated North Star System</title>
  <status>todo</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <requirement_ref>REQ-NORTHSTAR-REMOVE-01</requirement_ref>
  </implements>
  <depends_on><!-- None - first task --></depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_days>0.5</estimated_days>
</metadata>

<context>
First task in the North Star removal effort. Protocol constants must be removed before
dispatch routes and handlers that reference them. This is the foundation for eliminating
the broken single-embedding goal system that cannot be meaningfully compared to
13-embedder teleological arrays.
</context>

<objective>
Remove the deprecated North Star protocol constants from the MCP protocol definition
while adding a deprecation error code for graceful migration.
</objective>

<rationale>
The manual North Star system uses single 1024D embeddings for goals, but memories
are stored as 13-embedder teleological arrays. Comparing these is "apples-to-oranges"
and produces meaningless alignment scores. By removing the protocol constants first,
we ensure no new code can reference these deprecated endpoints.
</rationale>

<input_context_files>
  <file purpose="protocol_definition">crates/context-graph-mcp/src/protocol.rs</file>
  <file purpose="current_usage">crates/context-graph-mcp/src/handlers/core.rs</file>
  <file purpose="architecture_rationale">docs2/refactor/05-NORTH-STAR-REMOVAL.md</file>
</input_context_files>

<prerequisites>
  <check>Git working directory clean or changes stashed</check>
  <check>Codebase compiles before starting</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Remove PURPOSE_NORTH_STAR_ALIGNMENT constant</item>
    <item>Remove NORTH_STAR_UPDATE constant</item>
    <item>Add DEPRECATED_METHOD error code</item>
    <item>Keep PURPOSE_DRIFT_CHECK (will be refactored in TASK-LOGIC-010)</item>
  </in_scope>
  <out_of_scope>
    <item>Handler implementations (TASK-CORE-009)</item>
    <item>Dispatch route removal (requires handler removal first)</item>
    <item>GoalNode structure changes (TASK-CORE-005)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/protocol.rs">
      // These constants MUST NOT exist:
      // pub const PURPOSE_NORTH_STAR_ALIGNMENT: &str = "...";
      // pub const NORTH_STAR_UPDATE: &str = "...";

      // This constant MUST exist:
      pub const DEPRECATED_METHOD: i32 = -32601;

      // This constant MUST be preserved:
      pub const PURPOSE_DRIFT_CHECK: &str = "purpose/drift_check";
    </signature>
  </signatures>

  <constraints>
    <constraint>No references to removed constants in codebase</constraint>
    <constraint>DEPRECATED_METHOD follows JSON-RPC error code conventions</constraint>
    <constraint>PURPOSE_DRIFT_CHECK preserved for later refactoring</constraint>
    <constraint>Code compiles after changes (may have unused handler warnings)</constraint>
  </constraints>

  <verification>
    <command>cargo check -p context-graph-mcp</command>
    <command>rg "PURPOSE_NORTH_STAR_ALIGNMENT" --type rust</command>
    <command>rg "NORTH_STAR_UPDATE" --type rust</command>
    <command>rg "DEPRECATED_METHOD" --type rust crates/context-graph-mcp/src/protocol.rs</command>
  </verification>
</definition_of_done>

<pseudo_code>
In protocol.rs:
  1. Locate protocol constants section
  2. Remove: pub const PURPOSE_NORTH_STAR_ALIGNMENT
  3. Remove: pub const NORTH_STAR_UPDATE
  4. Add: pub const DEPRECATED_METHOD: i32 = -32601
  5. Verify PURPOSE_DRIFT_CHECK is preserved
  6. Add comment explaining deprecation

Note: Compile will produce warnings for now-unused imports/handlers.
These warnings are expected and will be resolved in subsequent tasks.
</pseudo_code>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/protocol.rs">
    Remove deprecated constants, add DEPRECATED_METHOD error code
  </file>
</files_to_modify>

<files_to_create>
  <!-- None -->
</files_to_create>

<validation_criteria>
  <criterion>rg "PURPOSE_NORTH_STAR_ALIGNMENT" returns no results</criterion>
  <criterion>rg "NORTH_STAR_UPDATE" returns no results</criterion>
  <criterion>DEPRECATED_METHOD constant exists with value -32601</criterion>
  <criterion>PURPOSE_DRIFT_CHECK constant is preserved</criterion>
  <criterion>cargo check passes (warnings acceptable)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo check -p context-graph-mcp 2>&amp;1 | head -50</command>
  <command>rg "PURPOSE_NORTH_STAR" --type rust</command>
  <command>rg "NORTH_STAR_UPDATE" --type rust</command>
</test_commands>

<rollback_plan>
  Git revert to previous commit if issues arise.
</rollback_plan>
</task_spec>
```
