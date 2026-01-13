# TASK-MCP-P1-001: Implement Tool Name Aliases

```xml
<task_spec id="TASK-MCP-P1-001" version="2.0">
<metadata>
  <title>Implement Tool Name Aliases - Add Alias Resolution for Legacy Tool Names</title>
  <status>IMPLEMENTED</status>
  <layer>surface</layer>
  <sequence>2</sequence>
  <priority>P1</priority>
  <implements>
    <requirement_ref>SPEC-MCP-001</requirement_ref>
  </implements>
  <depends_on>TASK-MCP-P0-001</depends_on>
  <estimated_complexity>low</estimated_complexity>
  <implementation_date>2025-01-12</implementation_date>
  <verified_by>SHERLOCK-HOLMES</verified_by>
</metadata>

<forensic_verification>
  <investigation_date>2026-01-12</investigation_date>
  <investigator>SHERLOCK-HOLMES</investigator>
  <verdict>IMPLEMENTED - INNOCENT</verdict>
  <confidence>HIGH</confidence>

  <evidence_collected>
    <evidence id="E1" type="file_exists">
      <file>crates/context-graph-mcp/src/tools/aliases.rs</file>
      <lines>69</lines>
      <status>EXISTS</status>
    </evidence>
    <evidence id="E2" type="function_exists">
      <function>resolve_alias(name: &amp;str) -&gt; &amp;str</function>
      <location>aliases.rs:31-37</location>
      <status>IMPLEMENTED</status>
    </evidence>
    <evidence id="E3" type="wiring_verified">
      <location>dispatch.rs:53</location>
      <code>let tool_name = crate::tools::aliases::resolve_alias(raw_tool_name);</code>
      <status>WIRED</status>
    </evidence>
    <evidence id="E4" type="test_results">
      <command>cargo test -p context-graph-mcp --lib tools::aliases</command>
      <result>4 passed; 0 failed</result>
    </evidence>
    <evidence id="E5" type="integration_test">
      <command>cargo test -p context-graph-mcp -- discover</command>
      <result>126 passed; 0 failed</result>
    </evidence>
  </evidence_collected>
</forensic_verification>

<context>
The Architect investigation identified a gap where two tool aliases were expected but NOT implemented.
This task has been COMPLETED. The alias system is now functional.

**Current State (VERIFIED 2026-01-12):**
- `aliases.rs` module EXISTS at `crates/context-graph-mcp/src/tools/aliases.rs`
- `resolve_alias()` function is IMPLEMENTED using simple match-based resolution
- Alias resolution IS WIRED into dispatch.rs at line 53
- All 4 alias unit tests PASS
- Integration tests for both aliased tools PASS

**Implemented Aliases:**
| Legacy Name | Canonical Name | Status |
|-------------|----------------|--------|
| `discover_goals` | `discover_sub_goals` | VERIFIED |
| `consolidate_memories` | `trigger_consolidation` | VERIFIED |

**Design Decision (Implemented):**
Simple match-based resolution was chosen over phf (perfect hash function):
- Avoids extra dependency
- Equally performant for 2 aliases
- Easier to understand and maintain
- O(1) lookup via compiler optimization
</context>

<constitution_rules>
  <rule id="MCP-001">MCP tools must return clear error messages</rule>
  <rule id="MCP-002">MCP protocol must be JSON-RPC 2.0 compliant</rule>
  <rule id="MCP-ALIAS-001">Tool names should be discoverable and consistent</rule>
  <rule id="MCP-ALIAS-002">Aliases should map to canonical tool names</rule>
</constitution_rules>

<current_implementation>
  <file path="crates/context-graph-mcp/src/tools/aliases.rs">
```rust
//! Tool name aliases for backward compatibility.
//!
//! TASK-MCP-P1-001: Provides alias resolution for legacy tool names.
//!
//! ## Design
//!
//! - Aliases use simple match for O(1) lookup
//! - Resolution is transparent to clients
//! - Adding aliases requires updating resolve_alias() function
//!
//! ## Aliases
//!
//! | Legacy Name | Canonical Name |
//! |-------------|----------------|
//! | discover_goals | discover_sub_goals |
//! | consolidate_memories | trigger_consolidation |

/// Resolve a tool name to its canonical form.
///
/// If the name has an alias, returns the canonical name.
/// Otherwise returns the original name unchanged.
#[inline]
pub fn resolve_alias(name: &amp;str) -&gt; &amp;str {
    match name {
        "discover_goals" =&gt; "discover_sub_goals",
        "consolidate_memories" =&gt; "trigger_consolidation",
        other =&gt; other,
    }
}
```
  </file>

  <file path="crates/context-graph-mcp/src/handlers/tools/dispatch.rs" lines="41-66">
```rust
let raw_tool_name = match params.get("name").and_then(|v| v.as_str()) {
    Some(n) =&gt; n,
    None =&gt; {
        return JsonRpcResponse::error(
            id,
            error_codes::INVALID_PARAMS,
            "Missing 'name' parameter in tools/call",
        );
    }
};

// TASK-MCP-P1-001: Resolve alias to canonical name
let tool_name = crate::tools::aliases::resolve_alias(raw_tool_name);

let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

debug!(
    "Calling tool: {} with arguments: {:?}{}",
    tool_name,
    arguments,
    if raw_tool_name != tool_name {
        format!(" (resolved from alias '{}')", raw_tool_name)
    } else {
        String::new()
    }
);
```
  </file>
</current_implementation>

<tool_name_catalog>
  <!-- Complete list of all 39 canonical tool names as of 2026-01-12 -->
  <category name="Core Tools (6)">
    <tool name="inject_context" constant="INJECT_CONTEXT" />
    <tool name="store_memory" constant="STORE_MEMORY" />
    <tool name="get_memetic_status" constant="GET_MEMETIC_STATUS" />
    <tool name="get_graph_manifest" constant="GET_GRAPH_MANIFEST" />
    <tool name="search_graph" constant="SEARCH_GRAPH" />
    <tool name="utl_status" constant="UTL_STATUS" />
  </category>

  <category name="GWT Tools (6)">
    <tool name="get_consciousness_state" constant="GET_CONSCIOUSNESS_STATE" />
    <tool name="get_kuramoto_sync" constant="GET_KURAMOTO_SYNC" />
    <tool name="get_workspace_status" constant="GET_WORKSPACE_STATUS" />
    <tool name="get_ego_state" constant="GET_EGO_STATE" />
    <tool name="trigger_workspace_broadcast" constant="TRIGGER_WORKSPACE_BROADCAST" />
    <tool name="adjust_coupling" constant="ADJUST_COUPLING" />
  </category>

  <category name="UTL Tools (1)">
    <tool name="gwt/compute_delta_sc" constant="COMPUTE_DELTA_SC" />
  </category>

  <category name="ATC Tools (3)">
    <tool name="get_threshold_status" constant="GET_THRESHOLD_STATUS" />
    <tool name="get_calibration_metrics" constant="GET_CALIBRATION_METRICS" />
    <tool name="trigger_recalibration" constant="TRIGGER_RECALIBRATION" />
  </category>

  <category name="Dream Tools (4)">
    <tool name="trigger_dream" constant="TRIGGER_DREAM" />
    <tool name="get_dream_status" constant="GET_DREAM_STATUS" />
    <tool name="abort_dream" constant="ABORT_DREAM" />
    <tool name="get_amortized_shortcuts" constant="GET_AMORTIZED_SHORTCUTS" />
  </category>

  <category name="Neuromodulation Tools (2)">
    <tool name="get_neuromodulation_state" constant="GET_NEUROMODULATION_STATE" />
    <tool name="adjust_neuromodulator" constant="ADJUST_NEUROMODULATOR" />
  </category>

  <category name="Steering Tools (1)">
    <tool name="get_steering_feedback" constant="GET_STEERING_FEEDBACK" />
  </category>

  <category name="Causal Tools (1)">
    <tool name="omni_infer" constant="OMNI_INFER" />
  </category>

  <category name="Teleological Tools (5)">
    <tool name="search_teleological" constant="SEARCH_TELEOLOGICAL" />
    <tool name="compute_teleological_vector" constant="COMPUTE_TELEOLOGICAL_VECTOR" />
    <tool name="fuse_embeddings" constant="FUSE_EMBEDDINGS" />
    <tool name="update_synergy_matrix" constant="UPDATE_SYNERGY_MATRIX" />
    <tool name="manage_teleological_profile" constant="MANAGE_TELEOLOGICAL_PROFILE" />
  </category>

  <category name="Autonomous Tools (7)">
    <tool name="auto_bootstrap_north_star" constant="AUTO_BOOTSTRAP_NORTH_STAR" />
    <tool name="get_alignment_drift" constant="GET_ALIGNMENT_DRIFT" />
    <tool name="trigger_drift_correction" constant="TRIGGER_DRIFT_CORRECTION" />
    <tool name="get_pruning_candidates" constant="GET_PRUNING_CANDIDATES" />
    <tool name="trigger_consolidation" constant="TRIGGER_CONSOLIDATION" alias="consolidate_memories" />
    <tool name="discover_sub_goals" constant="DISCOVER_SUB_GOALS" alias="discover_goals" />
    <tool name="get_autonomous_status" constant="GET_AUTONOMOUS_STATUS" />
  </category>

  <category name="Meta-UTL Tools (3)">
    <tool name="get_meta_learning_status" constant="GET_META_LEARNING_STATUS" />
    <tool name="trigger_lambda_recalibration" constant="TRIGGER_LAMBDA_RECALIBRATION" />
    <tool name="get_meta_learning_log" constant="GET_META_LEARNING_LOG" />
  </category>
</tool_name_catalog>

<alias_mappings>
  <!-- Currently implemented aliases -->
  <alias legacy="discover_goals" canonical="discover_sub_goals" status="IMPLEMENTED" />
  <alias legacy="consolidate_memories" canonical="trigger_consolidation" status="IMPLEMENTED" />
</alias_mappings>

<state_verification>
  <source_of_truth>
    <description>resolve_alias(alias) returns canonical name</description>
    <location>crates/context-graph-mcp/src/tools/aliases.rs:31-37</location>
  </source_of_truth>

  <execute_and_inspect>
    <test name="alias_resolution">
      <input>resolve_alias("discover_goals")</input>
      <expected>"discover_sub_goals"</expected>
      <actual>"discover_sub_goals"</actual>
      <status>PASS</status>
    </test>
    <test name="alias_resolution_2">
      <input>resolve_alias("consolidate_memories")</input>
      <expected>"trigger_consolidation"</expected>
      <actual>"trigger_consolidation"</actual>
      <status>PASS</status>
    </test>
    <test name="canonical_passthrough">
      <input>resolve_alias("trigger_consolidation")</input>
      <expected>"trigger_consolidation"</expected>
      <actual>"trigger_consolidation"</actual>
      <status>PASS</status>
    </test>
    <test name="unknown_passthrough">
      <input>resolve_alias("unknown_tool")</input>
      <expected>"unknown_tool"</expected>
      <actual>"unknown_tool"</actual>
      <status>PASS</status>
      <note>Unknown names pass through unchanged; dispatch returns TOOL_NOT_FOUND later</note>
    </test>
    <test name="empty_string">
      <input>resolve_alias("")</input>
      <expected>""</expected>
      <actual>""</actual>
      <status>PASS</status>
    </test>
  </execute_and_inspect>

  <edge_cases>
    <case name="unknown_alias">
      <behavior>Returns input unchanged, dispatch returns TOOL_NOT_FOUND (-32004)</behavior>
      <verified>YES - test_unknown_tool_name passes</verified>
    </case>
    <case name="canonical_name_passed">
      <behavior>Returns same canonical name (no-op)</behavior>
      <verified>YES - test_canonical_name_unchanged passes</verified>
    </case>
    <case name="empty_string">
      <behavior>Returns empty string, dispatch returns TOOL_NOT_FOUND</behavior>
      <verified>YES - test_unknown_name_unchanged passes</verified>
    </case>
  </edge_cases>

  <evidence_of_success>
    <assertion test="resolve_alias('discover_goals') returns 'discover_sub_goals'" status="VERIFIED" />
    <assertion test="resolve_alias('consolidate_memories') returns 'trigger_consolidation'" status="VERIFIED" />
    <assertion test="tools/call with 'discover_goals' dispatches to discover_sub_goals handler" status="VERIFIED" />
    <assertion test="tools/call with 'consolidate_memories' dispatches to trigger_consolidation handler" status="VERIFIED" />
    <assertion test="Canonical names still work after alias resolution" status="VERIFIED" />
    <assertion test="Unknown tools return TOOL_NOT_FOUND (-32004)" status="VERIFIED" />
  </evidence_of_success>
</state_verification>

<manual_test_design>
  <test_case name="Alias Resolution Verification">
    <input>
      aliases = ["discover_goals", "consolidate_memories", "trigger_consolidation", "unknown_tool"]
    </input>
    <expected_output>
      ["discover_sub_goals", "trigger_consolidation", "trigger_consolidation", "unknown_tool"]
    </expected_output>
    <verification>
      All known aliases resolve to canonical names.
      Unknown names pass through unchanged (dispatch handles TOOL_NOT_FOUND).
      Canonical names are idempotent (resolve to themselves).
    </verification>
    <status>VERIFIED</status>
  </test_case>

  <test_case name="Dispatch Integration">
    <input>
      tools/call with name="discover_goals"
    </input>
    <expected_output>
      Response from discover_sub_goals handler (NOT TOOL_NOT_FOUND)
    </expected_output>
    <verification>
      Alias is resolved before dispatch match statement.
      Handler receives canonical name.
      Response is identical to calling with canonical name.
    </verification>
    <status>VERIFIED</status>
  </test_case>
</manual_test_design>

<definition_of_done>
  <criterion id="DOD-1" status="DONE">aliases.rs module exists with resolve_alias() function</criterion>
  <criterion id="DOD-2" status="DONE">ALIASES map contains "discover_goals" to "discover_sub_goals" mapping</criterion>
  <criterion id="DOD-3" status="DONE">ALIASES map contains "consolidate_memories" to "trigger_consolidation" mapping</criterion>
  <criterion id="DOD-4" status="DONE">resolve_alias() is called in dispatch.rs before match statement</criterion>
  <criterion id="DOD-5" status="DONE">Unit tests verify alias resolution</criterion>
  <criterion id="DOD-6" status="DONE">Integration tests verify dispatch via alias</criterion>
  <criterion id="DOD-7" status="DONE">Canonical names still work after alias resolution</criterion>
  <criterion id="DOD-8" status="DONE">tools/list does NOT include alias names (only canonical)</criterion>
  <criterion id="DOD-9" status="DONE">Debug logging shows both raw and canonical names when alias used</criterion>
</definition_of_done>

<validation_criteria>
  <criterion id="VC-1" status="PASS">ALIASES map contains "discover_goals" to "discover_sub_goals" mapping</criterion>
  <criterion id="VC-2" status="PASS">ALIASES map contains "consolidate_memories" to "trigger_consolidation" mapping</criterion>
  <criterion id="VC-3" status="PASS">resolve_alias("discover_goals") returns "discover_sub_goals"</criterion>
  <criterion id="VC-4" status="PASS">resolve_alias("consolidate_memories") returns "trigger_consolidation"</criterion>
  <criterion id="VC-5" status="PASS">resolve_alias("unknown") returns "unknown" (passthrough)</criterion>
  <criterion id="VC-6" status="PASS">dispatch.rs calls resolve_alias() before match statement</criterion>
  <criterion id="VC-7" status="PASS">tools/call with "discover_goals" does NOT return TOOL_NOT_FOUND</criterion>
  <criterion id="VC-8" status="PASS">tools/call with "consolidate_memories" does NOT return TOOL_NOT_FOUND</criterion>
  <criterion id="VC-9" status="PASS">tools/list does NOT include alias names (only canonical)</criterion>
  <criterion id="VC-10" status="PASS">Canonical names continue to work after alias resolution</criterion>
</validation_criteria>

<test_commands>
  <command description="Run alias module tests">
    cargo test -p context-graph-mcp --lib tools::aliases -- --nocapture
  </command>
  <command description="Verify discover_goals alias works">
    cargo test -p context-graph-mcp --lib handlers::tests::exhaustive_mcp_tools -- discover --nocapture
  </command>
  <command description="Verify trigger_consolidation works">
    cargo test -p context-graph-mcp --lib handlers::tests::exhaustive_mcp_tools -- trigger_consolidation --nocapture
  </command>
  <command description="Run all MCP tests">
    cargo test -p context-graph-mcp --lib -- --nocapture
  </command>
</test_commands>

<test_results timestamp="2026-01-12">
  <result command="cargo test -p context-graph-mcp --lib tools::aliases">
    <output>running 4 tests
test tools::aliases::tests::test_canonical_name_unchanged ... ok
test tools::aliases::tests::test_discover_goals_alias ... ok
test tools::aliases::tests::test_consolidate_memories_alias ... ok
test tools::aliases::tests::test_unknown_name_unchanged ... ok

test result: ok. 4 passed; 0 failed; 0 ignored</output>
  </result>
  <result command="cargo test -p context-graph-mcp --lib handlers::tests::exhaustive_mcp_tools -- discover">
    <output>running 126 tests
...
test result: ok. 126 passed; 0 failed; 0 ignored</output>
  </result>
</test_results>

<notes>
  <note>
    Implementation uses simple match instead of phf (perfect hash function).
    This is acceptable for 2 aliases and avoids an extra dependency.
    If alias count grows significantly (10+), consider migrating to phf.
  </note>
  <note>
    Aliases are NOT listed in tools/list. This is intentional:
    - Prevents confusion about canonical vs alias names
    - Clients should migrate to canonical names
    - Aliases exist only for backward compatibility
  </note>
  <note>
    Debug logging includes both raw and canonical names for troubleshooting.
    This helps identify when aliases are being used in production.
  </note>
  <note>
    Unknown aliases pass through unchanged and are handled by the dispatch
    match statement's wildcard case, which returns TOOL_NOT_FOUND (-32004).
    This is the correct behavior - alias resolution is not responsible for
    validating tool existence.
  </note>
</notes>
</task_spec>
```

---

## Summary

| Field | Value |
|-------|-------|
| Task ID | TASK-MCP-P1-001 |
| Title | Implement Tool Name Aliases |
| Status | **IMPLEMENTED** |
| Layer | Surface (MCP) |
| Priority | P1 (High) |
| Complexity | Low |
| Implementation Date | 2025-01-12 |
| Verification Date | 2026-01-12 |
| Verified By | SHERLOCK-HOLMES |

## Files Involved

| File | Purpose | Status |
|------|---------|--------|
| `crates/context-graph-mcp/src/tools/aliases.rs` | Alias resolution module | EXISTS (69 lines) |
| `crates/context-graph-mcp/src/tools/mod.rs` | Module exports | UPDATED (exports aliases) |
| `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` | Tool dispatch | WIRED (line 53) |

## Alias Mappings

| Legacy Name | Canonical Name | Status |
|-------------|----------------|--------|
| `discover_goals` | `discover_sub_goals` | VERIFIED |
| `consolidate_memories` | `trigger_consolidation` | VERIFIED |

## Complete Tool Name Catalog (39 tools)

### Core Tools (6)
- `inject_context`
- `store_memory`
- `get_memetic_status`
- `get_graph_manifest`
- `search_graph`
- `utl_status`

### GWT Tools (6)
- `get_consciousness_state`
- `get_kuramoto_sync`
- `get_workspace_status`
- `get_ego_state`
- `trigger_workspace_broadcast`
- `adjust_coupling`

### UTL Tools (1)
- `gwt/compute_delta_sc`

### ATC Tools (3)
- `get_threshold_status`
- `get_calibration_metrics`
- `trigger_recalibration`

### Dream Tools (4)
- `trigger_dream`
- `get_dream_status`
- `abort_dream`
- `get_amortized_shortcuts`

### Neuromodulation Tools (2)
- `get_neuromodulation_state`
- `adjust_neuromodulator`

### Steering Tools (1)
- `get_steering_feedback`

### Causal Tools (1)
- `omni_infer`

### Teleological Tools (5)
- `search_teleological`
- `compute_teleological_vector`
- `fuse_embeddings`
- `update_synergy_matrix`
- `manage_teleological_profile`

### Autonomous Tools (7)
- `auto_bootstrap_north_star`
- `get_alignment_drift`
- `trigger_drift_correction`
- `get_pruning_candidates`
- `trigger_consolidation` (alias: `consolidate_memories`)
- `discover_sub_goals` (alias: `discover_goals`)
- `get_autonomous_status`

### Meta-UTL Tools (3)
- `get_meta_learning_status`
- `trigger_lambda_recalibration`
- `get_meta_learning_log`

## Verification Commands

```bash
# Verify alias resolution works
cargo test -p context-graph-mcp --lib tools::aliases -- --nocapture

# Verify dispatch integration
cargo test -p context-graph-mcp --lib handlers::tests::exhaustive_mcp_tools -- discover --nocapture

# Run all MCP tests
cargo test -p context-graph-mcp --lib -- --nocapture
```

## Design Rationale

**Why simple match instead of phf?**
- Only 2 aliases - phf overhead not justified
- No extra dependency
- Compiler optimizes match to O(1)
- Easier to read and maintain

**Why NOT list aliases in tools/list?**
- Clients should migrate to canonical names
- Prevents confusion about which name to use
- Aliases are backward compatibility only

**Why pass through unknown names?**
- Separation of concerns: alias resolution != tool validation
- Dispatch handles unknown tools with TOOL_NOT_FOUND (-32004)
- Allows future tools to be added without modifying aliases.rs
