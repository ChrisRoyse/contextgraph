# TASK-HEAL-017 — mcp__cgreality__mejepa_rollback_to MCP tool (operator-only)

```xml
<task_spec id="TASK-HEAL-017" version="1.0">

<metadata>
  <title>Operator-only MCP tool surface for rollback in `crates/context-graph-mcp/src/handlers/tools/mejepa/rollback_to.rs`</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>17</sequence>
  <implements>
    <item ref="REQ-HEAL-18">mcp__cgreality__mejepa_rollback_to(target_witness_chain_offset: u64) — operator-issued rollback to prior promotion's weights_sha; appends ModelRollback witness entry; updates active-weight pointer</item>
    <item ref="EC-HEAL-14">target weights_sha already GC'd → MEJEPA_HEAL_ROLLBACK_TARGET_GONE</item>
    <item ref="ERR-HEAL-11">MEJEPA_HEAL_ROLLBACK_TARGET_GONE</item>
    <item ref="TC-HEAL-16">Operator manual rollback flow</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-010</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 created (crates/context-graph-mcp/src/handlers/tools/mejepa/rollback_to.rs); 2 modified (mod.rs registry; protocol.rs tool list)</estimated_files>
</metadata>

<context>
Surface 2 of 4. Wires the operator-only `AbcPromoter::rollback_to` (TASK-HEAL-010) into the
`mcp__cgreality__*` namespace. This is a MUTATING tool: it appends a ModelRollback witness entry
and rewinds the active-weights pointer to a prior promotion. Because this is the only path that
mutates active weights without going through A/B/C retrain, it MUST gate on operator role per
`<security_requirements>` SEC-MEJEPA-04 (READ for operator + scheduler; WRITE-rollback for operator
only). The body validates `target_witness_chain_offset`, calls `AbcPromoter::rollback_to`, and
serializes the resulting HealReport-derived response.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<security_requirements>` SEC-MEJEPA-04 operator-only WRITE-rollback; SEC-MEJEPA-03 ed25519 signing on every promotion-class entry)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-18; TC-HEAL-16; EC-HEAL-14; ERR-HEAL-11)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (api_contracts mejepa_rollback_to endpoint; component_contracts AbcPromoter::rollback_to 8-step body)</file>
  <file purpose="prior_task">TASK-HEAL-010 (AbcPromoter::rollback_to body + RollbackEvidence + ModelRollback witness entry)</file>
  <file purpose="mcp_handler_pattern">crates/context-graph-mcp/src/handlers/tools/ (existing tool handler pattern: schema_definition + handle_call + role gate)</file>
  <file purpose="mcp_protocol">crates/context-graph-mcp/src/protocol.rs (tool registry list + JSON-RPC dispatch + caller_role surfacing)</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-010 AbcPromoter::rollback_to body shipped + RollbackEvidence struct exposed</check>
  <check>MCP server exposes `state.caller_role` (Operator | Scheduler | Read-only) in McpState — sourced from CLI flag or env per `<security_requirements>`; defaults to ReadOnly for safety</check>
  <check>MCP server has Arc&lt;SelfHealingPipeline&gt; (or Arc&lt;Mutex&lt;...&gt;&gt;) accessible to handler</check>
  <check>WitnessChain ModelRollback variant supported (TASK-HEAL-010 prerequisite)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mcp/src/handlers/tools/mejepa/rollback_to.rs`:
      - `pub fn schema() -> serde_json::Value` returning JSON schema:
        - input: `{ type: "object", properties: { target_witness_chain_offset: { type: "integer", minimum: 0 } }, required: ["target_witness_chain_offset"], additionalProperties: false }`
        - output: `{ type: "object", properties: { rolled_back_to: { type: "string", description: "hex-encoded weights_sha (32 bytes / 64 hex chars)" }, new_witness_chain_offset: { type: "integer", minimum: 0 }, hot_swap_latency_ms: { type: "integer", minimum: 0 } }, required: ["rolled_back_to", "new_witness_chain_offset", "hot_swap_latency_ms"], additionalProperties: false }`
      - `pub async fn handle_call(state: &McpState, params: serde_json::Value) -> Result<serde_json::Value, McpError>` — body:
        1. **Operator-role gate**: assert `state.caller_role == CallerRole::Operator`; else return `Err(McpError::PermissionDenied { reason: "mejepa_rollback_to requires Operator role per SEC-MEJEPA-04" })`
        2. **Parse params**: deserialize `{ target_witness_chain_offset: u64 }`; reject missing / negative / non-integer with InvalidArgument
        3. **Bounds check**: assert `target_witness_chain_offset < witness_chain.current_offset()`; else return InvalidArgument with reason "target offset beyond chain length"
        4. **Invoke**: `state.heal_pipeline.abc_promoter.rollback_to(target_witness_chain_offset, state.predictor.clone(), state.storage.clone(), state.witness_chain.clone(), state.heal_pipeline.promotion_lock.clone())?`
        5. **Map errors** to McpError variants:
           - HealError::RollbackTargetGone { theta_sha, gced_at } → McpError::Custom { code: "MEJEPA_HEAL_ROLLBACK_TARGET_GONE", message, payload }
           - HealError::PromotionDeadlock { holder } → McpError::Custom { code: "MEJEPA_HEAL_PROMOTION_DEADLOCK", message, payload }
           - HealError::IntegrityViolation { kind } → McpError::Custom { code: "MEJEPA_OBSERVE_INTEGRITY_VIOLATION", message, payload }
           - Other HealError variants → McpError::Custom with `.code()` mapping per constitution failure_taxonomy
        6. **Build response**: `{ rolled_back_to: hex(report.weights_sha_winner), new_witness_chain_offset: report.witness_chain_offset, hot_swap_latency_ms: report.hot_swap_latency_ms }`
        7. **Audit log**: emit SMAP journal entry at `memory/journal/agent-{N}-mejepa-mcp--rollback-issued-{timestamp}.md` with operator action + target offset + new_offset (per CLAUDE.md §6 SMAP frontmatter); emit `tracing::warn!` so it's visible in MCP server logs
        8. Return success body
      - `pub fn tool_definition() -> ToolDefinition` returning `{ name: "mejepa_rollback_to", description: "OPERATOR-ONLY. Roll back active weights to a prior ModelPromote witness entry...", input_schema: schema() }`
    - Modify `crates/context-graph-mcp/src/handlers/tools/mejepa/mod.rs`:
      - Add `pub mod rollback_to;`
      - Re-export `rollback_to::handle_call as handle_mejepa_rollback_to` and `rollback_to::tool_definition as mejepa_rollback_to_tool_definition`
    - Modify `crates/context-graph-mcp/src/protocol.rs`:
      - Register `"mejepa_rollback_to"` in the tool dispatch match arm
      - Append `mejepa_rollback_to_tool_definition()` to the tool-list response
      - Mark the tool with the `caller_role: Operator` flag in any tool-level metadata used for audit
    - Tests:
      - `schema_input_requires_target_witness_chain_offset`
      - `schema_input_rejects_negative_offset` (minimum: 0)
      - `schema_input_rejects_additional_properties`
      - `handle_call_rejects_non_operator_role` (caller_role=ReadOnly → PermissionDenied)
      - `handle_call_rejects_missing_offset_param` (InvalidArgument)
      - `handle_call_rejects_offset_beyond_chain_length`
      - `handle_call_succeeds_with_valid_modelpromote_offset` (round-trip rollback)
      - `handle_call_returns_rollback_target_gone_when_blob_gced` (EC-HEAL-14 → MEJEPA_HEAL_ROLLBACK_TARGET_GONE)
      - `handle_call_returns_promotion_deadlock_when_lock_held`
      - `handle_call_returns_integrity_violation_when_offset_not_modelpromote`
      - `handle_call_writes_smap_journal_entry_on_success`
      - `handle_call_response_has_hex_encoded_weights_sha_64_chars`
      - `tool_definition_name_is_mejepa_rollback_to`
      - End-to-end via JSON-RPC: send `{"method":"tools/call","params":{"name":"mejepa_rollback_to","arguments":{"target_witness_chain_offset":42}}}` with caller_role=Operator → success response
    - FSV evidence emission for TASK-HEAL-019: `RollbackToolEvidence { caller_role, target_witness_chain_offset, new_witness_chain_offset, rolled_back_to_sha_hex, hot_swap_latency_ms, smap_journal_path }`
  </in_scope>
  <out_of_scope>
    - The rollback business logic itself (TASK-HEAL-010 is the implementation; this task is the MCP wrapper)
    - HealStatus snapshot (TASK-HEAL-016 is the read-only sibling)
    - Operator-role authentication mechanism itself (out of scope; assumed propagated via state.caller_role)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/rollback_to.rs">
use crate::handlers::tools::{ToolDefinition, McpState, McpError, CallerRole};
use serde_json::Value;
use context_graph_mejepa::heal::errors::HealError;

#[derive(Debug, Clone, Deserialize)]
pub struct RollbackToParams {
    pub target_witness_chain_offset: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RollbackToResponse {
    pub rolled_back_to: String,            // hex-encoded weights_sha (64 chars)
    pub new_witness_chain_offset: u64,
    pub hot_swap_latency_ms: u64,
}

pub fn schema() -&gt; serde_json::Value;

pub async fn handle_call(
    state: &amp;McpState,
    params: serde_json::Value,
) -&gt; Result&lt;serde_json::Value, McpError&gt;;

pub fn tool_definition() -&gt; ToolDefinition;
    </signature>
  </signatures>
  <constraints>
    - OPERATOR-ROLE-GATED: caller_role MUST equal `CallerRole::Operator`; any other role → PermissionDenied (per SEC-MEJEPA-04)
    - Schema input: `target_witness_chain_offset` is u64 (minimum: 0), required, additionalProperties: false
    - Schema output: 3 fields (rolled_back_to as 64-char hex string, new_witness_chain_offset, hot_swap_latency_ms), additionalProperties: false
    - HealError → McpError mapping uses `.code()` from constitution failure_taxonomy SCREAMING_SNAKE registry
    - SMAP journal entry on success per CLAUDE.md §6 (namespace=journal, agent_id, timestamp, status=resolved, tags=mejepa,heal,rollback,operator)
    - tracing::warn! on every rollback invocation (operator-mutating action; high-visibility logging)
    - Tool name MUST be exactly `mejepa_rollback_to` in the registry
    - File &lt; 500 lines (thin wrapper; expected ~250 lines including tests)
    - No mock data in tests; real SelfHealingPipeline + real witness chain + real ed25519 keypair from tempfile keyring
    - Audit-log path for rollback action MUST appear in tracing output AND in the SMAP journal entry
  </constraints>
  <verification>
    - `cargo test -p context-graph-mcp handlers::tools::mejepa::rollback_to` shows ≥ 13 passing tests
    - `cargo clippy -p context-graph-mcp -- -D warnings` clean
    - Non-Operator caller → PermissionDenied (ALL non-Operator roles tested)
    - Missing target_witness_chain_offset → InvalidArgument (NOT silently default to 0)
    - additionalProperties on input → InvalidArgument
    - Valid ModelPromote offset → Operator-role caller gets a 3-field success response
    - GC'd blob → MEJEPA_HEAL_ROLLBACK_TARGET_GONE
    - Concurrent A/B/C in flight → MEJEPA_HEAL_PROMOTION_DEADLOCK
    - Non-ModelPromote offset → MEJEPA_OBSERVE_INTEGRITY_VIOLATION
    - SMAP journal entry written on success with full frontmatter
    - JSON-RPC end-to-end test with Operator caller succeeds
  </verification>
</definition_of_done>

<pseudo_code>
pub async fn handle_call(state: &McpState, params: serde_json::Value) -> Result<serde_json::Value, McpError> {
    if state.caller_role != CallerRole::Operator {
        tracing::warn!(caller_role = ?state.caller_role, "mejepa_rollback_to PermissionDenied");
        return Err(McpError::PermissionDenied { reason: "mejepa_rollback_to requires Operator role per SEC-MEJEPA-04".into() });
    }
    let parsed: RollbackToParams = serde_json::from_value(params)
        .map_err(|e| McpError::InvalidArgument { reason: format!("expected target_witness_chain_offset: u64; {}", e) })?;
    let chain_len = state.witness_chain.current_offset();
    if parsed.target_witness_chain_offset >= chain_len {
        return Err(McpError::InvalidArgument { reason: format!("target offset {} beyond chain length {}", parsed.target_witness_chain_offset, chain_len) });
    }
    tracing::warn!(target_offset = parsed.target_witness_chain_offset, "OPERATOR mejepa_rollback_to issued");
    let mut promoter = state.heal_pipeline.abc_promoter.lock().unwrap();
    let report_result = promoter.rollback_to(
        parsed.target_witness_chain_offset,
        state.predictor.clone(),
        state.storage.clone(),
        state.witness_chain.clone(),
        state.heal_pipeline.promotion_lock.clone(),
    );
    drop(promoter);
    let report = report_result.map_err(|e| heal_error_to_mcp(e))?;
    write_smap_journal_rollback(&parsed, &report)?;
    let response = RollbackToResponse {
        rolled_back_to: hex_encode(&report.weights_sha_winner),
        new_witness_chain_offset: report.witness_chain_offset,
        hot_swap_latency_ms: report.promotion_latency_seconds * 1000, // hot_swap is fast; latency_seconds is the rollback wallclock
    };
    Ok(serde_json::to_value(&response)?)
}

fn heal_error_to_mcp(e: HealError) -> McpError {
    let code = e.code(); // SCREAMING_SNAKE per constitution
    match e {
        HealError::RollbackTargetGone { theta_sha, gced_at } => McpError::Custom {
            code: code.into(),
            message: format!("rollback target weights_sha {} GC'd at {}", hex_encode(&theta_sha), gced_at),
            payload: serde_json::json!({ "theta_sha": hex_encode(&theta_sha), "gced_at": gced_at }),
        },
        HealError::PromotionDeadlock { holder } => McpError::Custom {
            code: code.into(),
            message: format!("promotion in flight; holder={}", holder),
            payload: serde_json::json!({ "holder": holder }),
        },
        HealError::IntegrityViolation { kind } => McpError::Custom {
            code: code.into(),
            message: format!("witness chain integrity violation: {:?}", kind),
            payload: serde_json::to_value(&kind).unwrap_or_default(),
        },
        other => McpError::Custom {
            code: other.code().into(),
            message: format!("{:?}", other),
            payload: serde_json::Value::Null,
        },
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/rollback_to.rs">RollbackToParams + RollbackToResponse + schema + handle_call + heal_error_to_mcp + tool_definition + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/mod.rs">add `pub mod rollback_to;` + re-exports</file>
  <file path="crates/context-graph-mcp/src/protocol.rs">register mejepa_rollback_to in the tool dispatch table + tool list (Operator-only metadata)</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo test -p context-graph-mcp handlers::tools::mejepa::rollback_to shows ≥ 13 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mcp -- -D warnings clean</criterion>
  <criterion>Non-Operator role → PermissionDenied (every non-Operator variant tested)</criterion>
  <criterion>Schema input requires target_witness_chain_offset (no silent default)</criterion>
  <criterion>additionalProperties: false on both input and output schemas</criterion>
  <criterion>GC'd blob → MEJEPA_HEAL_ROLLBACK_TARGET_GONE</criterion>
  <criterion>Concurrent A/B/C → MEJEPA_HEAL_PROMOTION_DEADLOCK</criterion>
  <criterion>Non-ModelPromote offset → MEJEPA_OBSERVE_INTEGRITY_VIOLATION</criterion>
  <criterion>Successful rollback → 3-field response (hex weights_sha, new_offset, latency_ms)</criterion>
  <criterion>SMAP journal entry written + tracing::warn! emitted on every rollback invocation</criterion>
  <criterion>Tool name exactly "mejepa_rollback_to"; Operator-only metadata in tool list</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mcp</command>
  <command>cargo test -p context-graph-mcp handlers::tools::mejepa::rollback_to</command>
  <command>cargo clippy -p context-graph-mcp -- -D warnings</command>
</test_commands>

</task_spec>
```
