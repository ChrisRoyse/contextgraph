<task_spec id="TASK-MCP-004" version="1.0">
<metadata>
  <title>Logic — `mejepa_observe_shift` idempotent replay handler invoking `ShiftSubscriber::process_shift` via channel</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>4</sequence>
  <implements>
    <item ref="REQ-SHIFT-07">`mejepa_observe_shift` registered + responds; `additionalProperties: false`</item>
    <item ref="REQ-SHIFT-11">Idempotent shift replay; no watermark rollback; `replay: true` in train cert</item>
    <item ref="BR-SHIFT-08">Replay past-watermark shifts safely; idempotent overwrite</item>
    <item ref="EC-SHIFT-12">Replay-on-replay still produces exactly-one CF row</item>
    <item ref="TC-SHIFT-11">10 concurrent invocations on same shift_id → one row, ten `replay:true` certs</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#api_contracts[mejepa_observe_shift]">request/response wire contract; ULID regex</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#component_contracts[MejepaObserveShiftHandler]">behaviour: locate → dispatch → replay-policy: do-not-advance</item>
  </implements>
  <depends_on>
    <task_ref>TASK-MCP-001</task_ref>
    <!-- Cross-phase: subscriber + replay-aware process_shift -->
    <task_ref>TASK-SHIFT-007</task_ref>
    <task_ref>TASK-SHIFT-011</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
The `mejepa_observe_shift` MCP tool is the operator-facing replay surface. Operators invoke it by `shift_id` (ULID) for drift-injection drills, observe()-incident debugging, and idempotency verification. The handler:

1. Validates the ULID regex `^01J[0-9A-F]{20}$` at the wire boundary.
2. Locates the shift in the in-memory `SubscriberIndex` (DashMap shift_id → (session_id, byte_offset, attempt_id)) maintained by sister TASK-SHIFT-007 / TASK-SHIFT-011.
3. If found, dispatches a replay request to the subscriber via a `tokio::sync::mpsc::Sender<ReplayRequest>` channel — this avoids racing with the main `run()` loop's `&amp;mut self` borrow and preserves outer-shift serialization (REQ-SHIFT-17).
4. Awaits the `ReplayResponse` over a oneshot channel; the response carries the `ShiftOutcome` from `process_shift(replay: true)`.
5. The subscriber-side handler refuses to advance the watermark (per `ReplayWatermarkPolicy::DoNotAdvance` from sister TASK-SHIFT-011).
6. Not-found → `{ shift_found: false, replay_outcome: null }` per AC-SHIFT-06-02 (NEVER errors).

Idempotency is structural: writes to CF_MEJEPA_LIVE_PREDICTIONS overwrite atomically on the same `(session_id, witness_chain_offset)` key. Concurrent invocations of this tool on the same shift_id all converge to ONE row.
</context>

<input_context_files>
  <file purpose="api_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#api_contracts[mejepa_observe_shift]</file>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#component_contracts[MejepaObserveShiftHandler]</file>
  <file purpose="idempotency_invariant">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#implementation_notes (idempotency row)</file>
  <file purpose="replay_policy">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-011.md</file>
  <file purpose="prior_skeleton">docs/ruvectorfindings/specs/tasks/phase7-mcp-tools/TASK-MCP-001.md</file>
  <file purpose="subscriber_index_owner">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-007.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-MCP-001 complete: skeleton + ObserveShiftArgs + stub handle()</check>
  <check>TASK-SHIFT-007 complete: `SubscriberIndex` (DashMap shift_id → coordinates) + `ShiftOutcome` carrier exist</check>
  <check>TASK-SHIFT-011 complete: `ReplayWatermarkPolicy::DoNotAdvance` variant + replay-aware `process_shift(.., replay: true)` overload + `ReplayRequest`/`ReplayResponse` channel types</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace stub body in `crates/context-graph-mcp/src/handlers/tools/mejepa/observe_shift.rs`
    - ULID regex validation: compile once via `once_cell::sync::Lazy<regex::Regex>` at module load; pattern exactly `^01J[0-9A-F]{20}$`
    - Lookup in `ctx.subscriber_index.get(&amp;shift_id)` (Arc&lt;DashMap&gt; on McpContext)
    - Dispatch via `ctx.replay_tx.send(ReplayRequest{ shift_id, response_tx })` then `response_rx.await`
    - Wrap response: `{ shift_found: bool, replay_outcome: Option<ShiftOutcome> }`; `replay_outcome` is `null` when `shift_found: false`
    - Timeout on the channel await: 30 seconds (matches NFR-SHIFT-02 p99 budget); on timeout return structured error `MEJEPA_OBSERVE_SHIFT_TIMEOUT`
    - Subscriber-down case: if `replay_tx.send` fails (channel closed because subscriber Tokio task dead), return structured error `MEJEPA_SHIFT_SUBSCRIBER_NOT_RUNNING` (handlers must surface, not swallow, the dead-subscriber state)
    - Unit tests: 6 tests with mock `replay_tx`/`SubscriberIndex` impls — happy path, not-found, malformed ULID, channel-closed, timeout, idempotent N-replay aggregation
  </in_scope>
  <out_of_scope>
    - The subscriber-side `process_shift(replay: true)` body (TASK-SHIFT-007 + TASK-SHIFT-011)
    - The `ReplayWatermarkPolicy::DoNotAdvance` enforcement (TASK-SHIFT-011)
    - FSV roundtrip (TASK-MCP-007)
    - Drift-injection drill orchestration (Phase 5 SELF-HEALING)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/observe_shift.rs">
      use once_cell::sync::Lazy;
      use regex::Regex;
      use tokio::sync::oneshot;
      use std::time::Duration;

      static ULID_RE: Lazy&lt;Regex&gt; = Lazy::new(|| Regex::new(r"^01J[0-9A-F]{20}$").expect("static regex"));

      pub async fn handle(
          args: serde_json::Value,
          ctx: &amp;crate::handlers::McpContext,
      ) -&gt; Result&lt;serde_json::Value, crate::error::McpError&gt;;

      // Carriers re-exported from sister phase (NOT defined here):
      //   crate::handlers::tools::mejepa::traits::ReplayRequest
      //   crate::handlers::tools::mejepa::traits::ReplayResponse
      //   crate::handlers::tools::mejepa::traits::ShiftOutcome  (re-exported from sister TASK-SHIFT-002)

      const REPLAY_TIMEOUT: Duration = Duration::from_secs(30);
    </signature>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/traits.rs">
      // Add to traits.rs (extends prior tasks):
      pub struct ReplayRequest {
          pub shift_id: String,
          pub response_tx: tokio::sync::oneshot::Sender&lt;Result&lt;ShiftOutcome, McpError&gt;&gt;,
      }

      // ShiftOutcome re-exported from sister phase carrier types (TASK-SHIFT-002)
      pub use crate::handlers::tools::mejepa::external::ShiftOutcome;
    </signature>
  </signatures>

  <constraints>
    - All input parsing routed through SHIPPED `reality_loop::schema_validator`; deny_unknown_fields enforced both at DTO and JSON-Schema layer
    - ULID regex compiled exactly once via `Lazy` (no re-compile per request)
    - Lookup is O(1) on `SubscriberIndex` (DashMap point read); do NOT iterate
    - Channel send + oneshot await pattern: do NOT hold the DashMap entry guard across the await (clone the coordinates before await; release guard immediately)
    - Timeout MUST be enforced via `tokio::time::timeout(REPLAY_TIMEOUT, response_rx)`; on timeout return `MEJEPA_OBSERVE_SHIFT_TIMEOUT` with `shift_id` payload — do NOT silently retry
    - Channel-closed (subscriber dead) MUST surface `MEJEPA_SHIFT_SUBSCRIBER_NOT_RUNNING`; do NOT respond with `shift_found: false` (which would mask a real outage)
    - Not-found is NOT an error (`{shift_found: false, replay_outcome: null}` per AC-SHIFT-06-02)
    - ULID regex failure rejects with `MEJEPA_OBSERVE_SHIFT_MALFORMED_ID` (structured) — never silently truncate or canonicalize
    - This task MUST NOT modify the writer side (CF_MEJEPA_LIVE_PREDICTIONS overwrite atomicity is sister TASK-SHIFT-007's invariant)
    - Idempotency proof is structural — repeated calls of `mejepa_observe_shift` MUST result in N `replay:true` certs and 1 row in CF_MEJEPA_LIVE_PREDICTIONS; the test asserts this end-to-end
    - File length under 500 lines
  </constraints>

  <verification>
    - Test 1 (happy path): mock SubscriberIndex contains the shift_id; mock replay_tx routes to a mock subscriber that returns Ok(ShiftOutcome); response carries `shift_found: true, replay_outcome: { ... }`
    - Test 2 (not-found): SubscriberIndex empty; response `{ shift_found: false, replay_outcome: null }`; no error
    - Test 3 (malformed ULID): `shift_id: "01jBADULID"` → `MEJEPA_OBSERVE_SHIFT_MALFORMED_ID` structured error
    - Test 4 (channel closed): replay_tx is a dropped sender → `MEJEPA_SHIFT_SUBSCRIBER_NOT_RUNNING`
    - Test 5 (timeout): mock subscriber holds the response_tx without sending; advance tokio time past 30 s → `MEJEPA_OBSERVE_SHIFT_TIMEOUT` (use `tokio::time::pause()` for deterministic test)
    - Test 6 (idempotency): drive 10 concurrent `handle()` calls with same shift_id; assert mock subscriber invoked 10× with `replay: true`; assert mock LivePredictionWriter shows 1 row at `(session_id, witness_chain_offset)` (overwrites are byte-equal); assert mock TrainCertWriter shows 10 certs all `replay: true` per TC-SHIFT-11
    - Test 7 (unknown field): args with `evil:1` rejected by deny_unknown_fields
    - `cargo build --release -p context-graph-mcp` clean
    - `cargo clippy -p context-graph-mcp -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
observe_shift::handle(args, ctx):
  let parsed: ObserveShiftArgs = schema_validator::parse_strict(args)?;
  if !ULID_RE.is_match(&parsed.shift_id) {
    return Err(McpError::structured("MEJEPA_OBSERVE_SHIFT_MALFORMED_ID", json!({"shift_id": parsed.shift_id})));
  }
  let coords = match ctx.subscriber_index.get(&parsed.shift_id) {
    Some(entry) => entry.value().clone(),  // (session_id, byte_offset, Option<attempt_id>)
    None => return Ok(serde_json::json!({"shift_found": false, "replay_outcome": null})),
  };
  let (response_tx, response_rx) = tokio::sync::oneshot::channel();
  let req = ReplayRequest{ shift_id: parsed.shift_id.clone(), response_tx };
  if ctx.replay_tx.send(req).await.is_err() {
    return Err(McpError::structured("MEJEPA_SHIFT_SUBSCRIBER_NOT_RUNNING", json!({"shift_id": parsed.shift_id})));
  }
  match tokio::time::timeout(REPLAY_TIMEOUT, response_rx).await {
    Ok(Ok(Ok(outcome))) => Ok(serde_json::json!({"shift_found": true, "replay_outcome": outcome})),
    Ok(Ok(Err(e)))     => Err(e),  // subscriber-side error already structured
    Ok(Err(_recv_err)) => Err(McpError::structured("MEJEPA_SHIFT_SUBSCRIBER_NOT_RUNNING", json!({"shift_id": parsed.shift_id, "reason": "response channel dropped"}))),
    Err(_timeout)      => Err(McpError::structured("MEJEPA_OBSERVE_SHIFT_TIMEOUT", json!({"shift_id": parsed.shift_id, "timeout_secs": REPLAY_TIMEOUT.as_secs()}))),
  }
</pseudo_code>

<files_to_create>
  <!-- (none — modifies existing files only) -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/observe_shift.rs">replace stub with real handler + ULID regex + 7 unit tests</file>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/traits.rs">add `ReplayRequest` + re-export `ShiftOutcome` from sister phase</file>
  <file path="crates/context-graph-mcp/Cargo.toml">add `regex` + `once_cell` workspace deps if not present</file>
</files_to_modify>

<validation_criteria>
  <criterion>All 7 unit tests pass</criterion>
  <criterion>Idempotency: 10 concurrent calls → 1 row, 10 `replay:true` certs (TC-SHIFT-11)</criterion>
  <criterion>Channel-closed surfaces `MEJEPA_SHIFT_SUBSCRIBER_NOT_RUNNING` (subscriber-down is observable)</criterion>
  <criterion>Timeout surfaces `MEJEPA_OBSERVE_SHIFT_TIMEOUT`</criterion>
  <criterion>Not-found is NOT an error (returns `shift_found: false`)</criterion>
  <criterion>ULID regex compiled once via Lazy</criterion>
  <criterion>No DashMap entry guard held across await</criterion>
  <criterion>No file exceeds 500 lines</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mcp</command>
  <command>cargo test -p context-graph-mcp mejepa::observe_shift</command>
  <command>cargo clippy -p context-graph-mcp -- -D warnings</command>
</test_commands>
</task_spec>
