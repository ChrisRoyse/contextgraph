# TASK-HEAL-016 — mcp__cgreality__mejepa_heal_status MCP tool (read-only HealStatus snapshot)

```xml
<task_spec id="TASK-HEAL-016" version="1.0">

<metadata>
  <title>Read-only MCP tool surface for HealStatus snapshot in `crates/context-graph-mcp/src/handlers/tools/mejepa/heal_status.rs`</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>16</sequence>
  <implements>
    <item ref="REQ-HEAL-17">mcp__cgreality__mejepa_heal_status MCP tool exposing HealStatus { drift_severity, last_promotion_at, last_calibration_at, last_integrity_check_at, observation_counter, ewc_lambda, fisher_rank } — read-only, no params, no side effects</item>
    <item ref="TC-HEAL-15">Operator queries heal status; sees current drift severity + counters</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-008</task_ref>
    <task_ref>TASK-HEAL-009</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 created (crates/context-graph-mcp/src/handlers/tools/mejepa/heal_status.rs); 2 modified (mod.rs registry; protocol.rs tool list)</estimated_files>
</metadata>

<context>
Surface 1 of 4. Wires the read-only `HealStatus` snapshot into the `mcp__cgreality__*` namespace
so Claude Code (and any MCP client) can poll the heal-cycle state for operator inspection.
Read-only, zero-parameter, side-effect-free; the body acquires a brief `pipeline.status` mutex
read, copies the snapshot, releases the mutex, and serializes to JSON. No `additionalProperties`
on the schema (additionalProperties: false per technical spec api_contracts).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<security_requirements>` SEC-MEJEPA-04 operator-can-read-but-not-mutate; MCP tool surface convention)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-17; TC-HEAL-15)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (api_contracts mejepa_heal_status endpoint; data_models HealStatus 11 fields)</file>
  <file purpose="prior_task">TASK-HEAL-008 (SelfHealingPipeline.status: Arc&lt;Mutex&lt;HealStatus&gt;&gt;)</file>
  <file purpose="prior_task">TASK-HEAL-009 (active_weights_sha after promotion)</file>
  <file purpose="mcp_handler_pattern">crates/context-graph-mcp/src/handlers/tools/ (existing tool handler pattern: schema_definition + handle_call)</file>
  <file purpose="mcp_protocol">crates/context-graph-mcp/src/protocol.rs (tool registry list + JSON-RPC dispatch)</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-008 SelfHealingPipeline.status (Arc&lt;Mutex&lt;HealStatus&gt;&gt;) accessible from MCP handler context</check>
  <check>TASK-HEAL-009 active_weights_sha + active_calibration_version + active_constellation_version populated in HealStatus</check>
  <check>MCP server has a SelfHealingPipeline accessor (Arc&lt;SelfHealingPipeline&gt; or Arc&lt;Mutex&lt;SelfHealingPipeline&gt;&gt;) in its dispatch state — wired by parent MCP main</check>
  <check>HealStatus is Serialize per `<schema_constraints>` (additionalProperties: false on emitted JSON)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mcp/src/handlers/tools/mejepa/heal_status.rs`:
      - `pub fn schema() -> serde_json::Value` returning the JSON schema (zero-input; output object with 11 named fields per data_models HealStatus); `additionalProperties: false`; required: ALL 11 fields
      - `pub async fn handle_call(state: &McpState, params: serde_json::Value) -> Result<serde_json::Value, McpError>` — body:
        1. Reject if `params` is non-empty (the schema declares zero inputs); return McpError::InvalidArgument { reason: "mejepa_heal_status takes no parameters" }
        2. Acquire read lock on `state.heal_pipeline.status` (`pipeline.status.lock().unwrap()` → MutexGuard&lt;HealStatus&gt;)
        3. Copy snapshot: `let snapshot: HealStatus = (*guard).clone();` (HealStatus is Clone per data_models)
        4. Drop the guard immediately (release the mutex; do NOT hold during serialization)
        5. Serialize via `serde_json::to_value(&snapshot)?` and return as the success body
      - `pub fn tool_definition() -> ToolDefinition` returning `{ name: "mejepa_heal_status", description: "...", input_schema: schema() }` for tool-list registration
    - Modify `crates/context-graph-mcp/src/handlers/tools/mejepa/mod.rs`:
      - Add `pub mod heal_status;`
      - Re-export `heal_status::handle_call as handle_mejepa_heal_status` and `heal_status::tool_definition as mejepa_heal_status_tool_definition`
    - Modify `crates/context-graph-mcp/src/protocol.rs` (or wherever the tool registry lives):
      - Register `"mejepa_heal_status"` in the tool dispatch match arm → `mejepa::heal_status::handle_call(state, params).await`
      - Append `mejepa_heal_status_tool_definition()` to the tool-list response
    - Tests:
      - `schema_returns_zero_input_object_with_11_required_fields`
      - `schema_has_additional_properties_false`
      - `handle_call_returns_full_healstatus_snapshot_as_json`
      - `handle_call_does_not_block_observe_concurrent` (concurrent observe() acquires the same mutex; verify both calls eventually succeed; `handle_call` releases promptly)
      - `handle_call_rejects_non_empty_params`
      - `handle_call_serializes_drift_severity_as_string_enum`
      - `handle_call_returns_zero_observation_counter_at_startup`
      - `handle_call_reflects_observation_counter_after_observe_calls` (call observe() N times; assert handle_call returns counter=N)
      - `handle_call_reflects_status_change_paused_after_integrity_violation` (force IntegrityViolation; assert status_change=paused)
      - `handle_call_reflects_status_change_retraining_during_abc` (mid-A/B/C; assert status_change=retraining)
      - `tool_definition_name_is_mejepa_heal_status`
      - End-to-end via JSON-RPC over stdio: spawn the MCP server, send `{"method":"tools/call","params":{"name":"mejepa_heal_status","arguments":{}}}`, assert response is HealStatus JSON
    - FSV evidence emission for TASK-HEAL-019: `HealStatusEvidence { snapshot_at: i64, snapshot_json: serde_json::Value, snapshot_size_bytes: usize, mutex_acquisition_us: u64 }`
  </in_scope>
  <out_of_scope>
    - mejepa_rollback_to (TASK-HEAL-017 is the operator-mutating sibling)
    - HealStatus mutation (read-only by contract)
    - HealStatus serialization format itself (defined in TASK-HEAL-004 via Serialize derive)
    - Authentication / operator-role gating (this tool is read-only and safe for ANY caller per SEC-MEJEPA-04 spirit)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/heal_status.rs">
use crate::handlers::tools::{ToolDefinition, McpState, McpError};
use serde_json::Value;
use context_graph_mejepa::heal::status::HealStatus;

pub fn schema() -&gt; serde_json::Value;

pub async fn handle_call(
    state: &amp;McpState,
    params: serde_json::Value,
) -&gt; Result&lt;serde_json::Value, McpError&gt;;

pub fn tool_definition() -&gt; ToolDefinition;
    </signature>
  </signatures>
  <constraints>
    - Read-only: NEVER acquires `pipeline.status.lock().unwrap_mut()` or any write surface
    - Mutex held for &lt; 1 ms (clone-and-drop pattern); MUST NOT hold across `await` boundaries
    - additionalProperties: false on schema (per technical spec `<schema_constraints>`)
    - All 11 HealStatus fields present in schema; required: all 11 (no optional fields on the snapshot)
    - Zero-parameter input schema: `{ "type": "object", "properties": {}, "additionalProperties": false }`
    - Tool name MUST be exactly `mejepa_heal_status` (no `mcp__cgreality__` prefix in the tool registry; the prefix is added by the MCP runtime when the tool is exposed to clients)
    - File &lt; 500 lines (this is a thin wrapper; expected ~150 lines)
    - tracing::debug! on entry; tracing::debug! on exit with snapshot summary (drift_severity + observation_counter)
    - No mock data in tests; real SelfHealingPipeline (constructed from synthetic but real RocksDB tempdir + real witness chain)
  </constraints>
  <verification>
    - `cargo test -p context-graph-mcp handlers::tools::mejepa::heal_status` shows ≥ 12 passing tests
    - `cargo clippy -p context-graph-mcp -- -D warnings` clean
    - JSON-RPC over stdio end-to-end test returns valid HealStatus JSON (all 11 fields)
    - Schema validates against the response (every required field present)
    - Concurrent observe() + handle_call do not deadlock (mutex released promptly)
    - Non-empty params rejected with InvalidArgument
    - status_change reflects current pipeline state (active / retraining / degraded / paused)
  </verification>
</definition_of_done>

<pseudo_code>
pub async fn handle_call(state: &McpState, params: serde_json::Value) -> Result<serde_json::Value, McpError> {
    if !params.is_null() && params.as_object().map_or(false, |o| !o.is_empty()) {
        return Err(McpError::InvalidArgument { reason: "mejepa_heal_status takes no parameters".into() });
    }
    let snapshot: HealStatus = {
        let guard = state.heal_pipeline.status.lock()
            .map_err(|e| McpError::Internal { reason: format!("status mutex poisoned: {}", e) })?;
        (*guard).clone()
    }; // guard dropped here; mutex released
    let value = serde_json::to_value(&snapshot)
        .map_err(|e| McpError::Internal { reason: format!("HealStatus serialize: {}", e) })?;
    tracing::debug!(drift_severity = ?snapshot.drift_severity, counter = snapshot.observation_counter, "mejepa_heal_status served");
    Ok(value)
}

pub fn schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false,
    })
}

pub fn tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: "mejepa_heal_status".into(),
        description: "Returns a read-only snapshot of the SelfHealingPipeline HealStatus (drift_severity, last_promotion_at, observation_counter, ewc_lambda, fisher_rank, ...). Zero parameters. No side effects.".into(),
        input_schema: schema(),
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/heal_status.rs">schema + handle_call + tool_definition + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/mod.rs">add `pub mod heal_status;` + re-exports</file>
  <file path="crates/context-graph-mcp/src/protocol.rs">register mejepa_heal_status in the tool dispatch table + tool list</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo test -p context-graph-mcp handlers::tools::mejepa::heal_status shows ≥ 12 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mcp -- -D warnings clean</criterion>
  <criterion>Schema has additionalProperties: false + zero input properties</criterion>
  <criterion>JSON-RPC end-to-end returns 11-field HealStatus JSON</criterion>
  <criterion>Mutex released before `await` (no deadlock with concurrent observe())</criterion>
  <criterion>Non-empty params → InvalidArgument</criterion>
  <criterion>status_change reflects current pipeline state (active / retraining / degraded / paused)</criterion>
  <criterion>Tool name exactly "mejepa_heal_status" in the registry</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mcp</command>
  <command>cargo test -p context-graph-mcp handlers::tools::mejepa::heal_status</command>
  <command>cargo clippy -p context-graph-mcp -- -D warnings</command>
</test_commands>

</task_spec>
```
