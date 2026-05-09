# TASK-INFER-012 — mcp__cgreality__mejepa_predict_latest MCP tool

```xml
<task_spec id="TASK-INFER-012" version="1.0">

<metadata>
  <title>mcp__cgreality__mejepa_predict_latest handler in `crates/context-graph-mcp/src/handlers/tools/mejepa.rs`</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>12</sequence>
  <implements>
    <item ref="REQ-INFER-19">CF_MEJEPA_LIVE_PREDICTIONS read; mejepa_predict_latest exposes recent verdicts</item>
    <item ref="component:mcp_endpoint">mejepa_predict_latest API contract</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INFER-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>0 created (extends mejepa.rs from TASK-INFER-011)</estimated_files>
</metadata>

<context>
This task adds the second MCP tool: `mcp__cgreality__mejepa_predict_latest(session_id, limit)`
returning the most recent N RealityPrediction records from CF_MEJEPA_LIVE_PREDICTIONS for a given
session_id. Used by operators to inspect recent verdicts and by the Phase 5 self-healing rolling
window to read predictions for empirical-coverage computation.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (mcp_tool_namespace_lock; rocksdb_cfs)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INFERENCE.md (REQ-INFER-19; US-INFER-04)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (api_contracts mcp__cgreality__mejepa_predict_latest)</file>
  <file purpose="upstream_handler">crates/context-graph-mcp/src/handlers/tools/mejepa.rs (TASK-INFER-011 — schema patterns + dispatch)</file>
  <file purpose="upstream_compiler">crates/context-graph-mejepa/src/compiler.rs (MejepaStore trait with read_recent_predictions; TASK-INFER-003)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INFER-011 complete; mejepa.rs handler scaffold exists</check>
  <check>TASK-INFER-003 declares MejepaStore::write_live_prediction; this task adds read_live_predictions</check>
</prerequisites>

<scope>
  <in_scope>
    - Add `read_live_predictions(&self, session_id: [u8; 16], limit: u32) -> Result<Vec<RealityPrediction>, MejepaInferError>` method to `MejepaStore` trait in `crates/context-graph-mejepa/src/compiler.rs`
    - Add concrete impl on `RocksDbStore` (in `crates/context-graph-mejepa/src/store.rs` if not present, or extend existing): reverse-iterate CF_MEJEPA_LIVE_PREDICTIONS with prefix=session_id; deserialize bincode; collect up to `limit`
    - Add `pub const TOOL_NAME_PREDICT_LATEST: &str = "mcp__cgreality__mejepa_predict_latest";` to mejepa.rs
    - Add `schema_predict_latest()` returning JSON schema for input `{ session_id: hex string 32 chars, limit: u32 (default 10, max 1000) }`
    - Add `pub async fn handle_mejepa_predict_latest(args, store: &dyn MejepaStore) -> Result<Value, JsonRpcError>` — parse session_id (hex), bound limit ∈ [1, 1000], call store.read_live_predictions, serialize as `{ predictions: [...] }`
    - Register handler in dispatch table
    - Add integration tests:
      - `mejepa_predict_latest_returns_recent`: synthesize 5 predictions for session A → response.predictions.len() == 5
      - `mejepa_predict_latest_limit_enforced`: insert 100, request limit=10 → 10
      - `mejepa_predict_latest_unknown_session_returns_empty`: returns `{ predictions: [] }` (NOT error)
      - `mejepa_predict_latest_rejects_invalid_hex_session_id`: 31-char or non-hex → invalid_params
      - `mejepa_predict_latest_limit_exceeds_max_clamped`: limit=2000 → response capped at 1000
      - `mejepa_predict_latest_rejects_extra_fields`: additionalProperties: false
  </in_scope>
  <out_of_scope>
    - CF_MEJEPA_LIVE_PREDICTIONS column-family registration (Phase 6 SPEC-HYGIENE) — this task assumes CF exists
    - Phase 6 retention/tier-compress — out of scope
    - Operator override of session_id (not supported in v1)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa.rs">
pub const TOOL_NAME_PREDICT_LATEST: &amp;str = "mcp__cgreality__mejepa_predict_latest";

pub fn schema_predict_latest() -&gt; serde_json::Value;

pub async fn handle_mejepa_predict_latest(
    args: serde_json::Value,
    store: &amp;dyn context_graph_mejepa::compiler::MejepaStore,
) -&gt; Result&lt;serde_json::Value, JsonRpcError&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa/src/compiler.rs">
pub trait MejepaStore: Send + Sync {
    // ... existing methods ...
    fn read_live_predictions(
        &amp;self,
        session_id: [u8; 16],
        limit: u32,
    ) -&gt; Result&lt;Vec&lt;crate::types::RealityPrediction&gt;, crate::error::MejepaInferError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - Tool name namespace-locked to `mcp__cgreality__mejepa_predict_latest`
    - additionalProperties: false on JSON schema
    - limit clamped to [1, 1000]; default 10
    - session_id parsed as exactly 32 hex chars (16 bytes)
    - Empty result is success (not error)
    - Iteration uses RocksDB prefix iterator with prefix=session_id BE-encoded for time order
    - Determinism: byte-equal response for same DB state + same query
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mcp` succeeds
    - `cargo test -p context-graph-mcp mejepa_predict_latest` shows ≥ 6 passing tests
    - `cargo clippy -p context-graph-mcp -- -D warnings` clean
    - All boundary tests pass (limit clamp, empty session, invalid hex)
  </verification>
</definition_of_done>

<pseudo_code>
pub async fn handle_mejepa_predict_latest(args, store) -> Result<Value, JsonRpcError> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Req {
        session_id: String,
        #[serde(default = "default_limit")]
        limit: u32,
    }
    fn default_limit() -> u32 { 10 }

    let req: Req = serde_json::from_value(args)
        .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;

    if req.session_id.len() != 32 || !req.session_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(JsonRpcError::invalid_params("session_id must be 32 hex chars"));
    }
    let mut sid = [0u8; 16];
    hex::decode_to_slice(&req.session_id, &mut sid)
        .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;

    let limit = req.limit.clamp(1, 1000);
    let preds = store.read_live_predictions(sid, limit)
        .map_err(|e| JsonRpcError::application(e.code(), e.to_string()))?;
    Ok(serde_json::json!({ "predictions": preds }))
}
</pseudo_code>

<files_to_create>
  <!-- none; extends mejepa.rs from TASK-INFER-011 -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa.rs">add TOOL_NAME_PREDICT_LATEST + schema + handler + 6 tests</file>
  <file path="crates/context-graph-mcp/src/handlers/tools/mod.rs">register handler in dispatch table</file>
  <file path="crates/context-graph-mejepa/src/compiler.rs">add `read_live_predictions` method to `MejepaStore` trait</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mcp zero warnings</criterion>
  <criterion>cargo test -p context-graph-mcp mejepa_predict_latest shows ≥ 6 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mcp -- -D warnings clean</criterion>
  <criterion>limit clamped to [1, 1000]</criterion>
  <criterion>session_id 32-hex validation</criterion>
  <criterion>Empty result returns success not error</criterion>
  <criterion>additionalProperties: false enforced</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mcp</command>
  <command>cargo test -p context-graph-mcp mejepa_predict_latest</command>
  <command>cargo clippy -p context-graph-mcp -- -D warnings</command>
</test_commands>

</task_spec>
```
