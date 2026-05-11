## 🔍 AI-Agent Implementation Mandate (read FIRST — fresh-context onboarding)

> **You are an AI coding assistant inheriting this task with no prior context. This block gives you everything you need.**

### 1. Project state pointers — read these before touching code

- **Live shipping status table:** `docs/ruvectorfindings/specs/_README.md` Domain map (one row per phase, current as of 2026-05-10).
- **Per-phase deliverable status:** `docs/ruvectorfindings/specs/implementation-plan.md` "Per-phase deliverable table".
- **Constitution (immutable rules):** `docs/ruvectorfindings/specs/constitution.md`.
- **This phase's status snapshot:** the `_index.md` in this same directory.
- **Recent activity:** `git log --oneline -30` shows the last 30 commits.
- **Architecture intent:** `docs/ruvectorfindings/{09,10,11,12,13,14}*.md` (read 09 master plan + 10 pivot first).
- **Project rules:** `CLAUDE.md` at workspace root.

### 2. ⚠ ASSUME THIS TASK IS WRONG — audit before you implement

This task body was authored 2026-05-09. Before implementing, audit it against the current codebase:

1. Do the file paths it references still exist? `find` / `ls` them.
2. Do the type signatures it specifies still match what's in tree? `grep` for the type names in `crates/`.
3. Has the crate it tells you to "create" already been created? Check `crates/`. If it exists, STOP — read what's there, reconcile against the task, and update the task itself before implementing more.
4. Does the spec it cites (e.g., `TECH-<DOMAIN>.md`) still describe the same data shapes? Re-read those specs.

If you find drift, document it in your handoff (`memory/handoff/`) and either (a) update the task to match reality before continuing, or (b) escalate to the operator if scope changed materially.

### 3. Hard rules — ZERO tolerance, fail-closed

1. **No backwards compatibility.** Schema changes wipe + recreate; never write migration shims. Synthetic data only.
2. **No mock data in tests.** Use real types, real corpus rows, real RocksDB CFs, real file SHAs. Tests must FAIL when the system is broken — never write a test that passes against a broken state.
3. **Fail closed with structured error codes** (e.g., `MEJEPA_*`, `UTML_*`). No `unwrap_or(default)` on confidence / quota / correlation / signature / chain-integrity paths. No silent fallback. Every error has `.code()` returning `SCREAMING_SNAKE_CASE`.
4. **Robust error logging.** Every fail-path emits `tracing::error!` with the structured code + actionable context (which file, which CF, which byte offset, which step). When something breaks, the log must say exactly what failed and how to fix it.
5. **No comments unless they encode non-obvious WHY.** Don't explain WHAT — well-named identifiers do that.
6. **Files under 500 lines** (split if exceeded).
7. **Provenance comment** at top of every new `.rs` file: `// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).` Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.

### 4. Mandatory Full State Verification (FSV) — every task ships with it

Do not rely on return values alone. After the logic runs, you MUST:

1. **Define the Source of Truth.** State explicitly where the final result lives: a RocksDB column family, a `/tmp/contextgraph-*-fsv/*.json` file, a witness-chain segment, a tier-compressed panel blob, a CF watermark row, a tensor allocator's `live_tensors` map, etc.
2. **Execute & Inspect.** Run the logic, then perform a SEPARATE READ on the Source of Truth to verify the data was processed correctly. Cite the readback command (e.g., `cargo run --release -p <crate> --example <fsv_example>` or a direct CF dump).
3. **Boundary & Edge Case Audit.** Manually simulate AT LEAST 3 edge cases — empty inputs, maximum limits, invalid formats, NaN/Inf in floats, control chars in strings, byte offsets at file boundary, etc. For each, print the system state BEFORE and AFTER the action.
4. **Evidence of Success.** Provide a log showing the actual data residing in the system after execution. Cite the log path in the task's `<verification>` block.

### 5. Trigger → outcome verification (mandatory database / artifact check)

> Every triggered event exists to produce a specific intended outcome. The trigger is observable; the outcome is queryable. If your task writes anything to a persistent surface, you MUST query that surface AFTER running and confirm the expected data is there. Do not trust return values alone.

Synthetic protocol: pick an input where you KNOW the expected output. Run it. Manually check the source of truth for the expected output (e.g., open RocksDB and dump the CF, read the FSV JSON, replay the witness chain). Document the trigger → outcome chain in the FSV evidence. Examples:

- Trigger: `compute_l_step({delta_p:0, delta_k:1, delta_omega:1, delta_xi:1})` → Outcome: returns `0.0` exactly (multiplicative gate; any factor at zero zeros the product). FSV: dump training cert, confirm `l_step == 0.0`.
- Trigger: insert `ShiftEntry { shift_id: "01J0123456789ABCDEFGHIJK", ... }` → Outcome: row in `CF_MEJEPA_SHIFT_WATERMARK` with that exact key. FSV: open RocksDB, `cf.get(b"01J0123456789ABCDEFGHIJK")` returns Some.

### 6. Occam's razor

The simplest implementation that satisfies the contract is the correct one. Do NOT add features, abstractions, or fallbacks beyond the task scope. A bug fix doesn't need surrounding cleanup. Three similar lines beats a premature abstraction. If the task says "create struct X with 5 fields", create exactly that — not a builder, not a derive macro family, not a generic over a placeholder.

### 7. Subagent / verification expectations

After implementation:
- Run `cargo build --release -p <crate>` AND `cargo test -p <crate>` AND `cargo clippy -p <crate> -- -D warnings`.
- Spawn the `code-simplifier` subagent (or equivalent) on your changes; apply BLOCKER fixes.
- Write a final SMAP report at `memory/reports/agent-{N}-{role}--final-report.md`.
- Touch the corresponding row in `tasks/_traceability.md` if the REQ→TASK mapping changed.

---

## 📋 2026-05-11 STATE AUDIT (sharpened against verified codebase)

> **Codebase reality table:** see `_index.md` in this directory.

**This task creates a NEW MCP handler.** Today no `mejepa_observe_shift` const, schema, dispatcher arm, or handler exists.

**Drift from original task body:**
- File path `crates/context-graph-mcp/src/handlers/tools/mejepa/observe_shift.rs` is wrong — the sub-module tree was never created.
- Place the new handler at the bottom of `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs` (currently 838 lines; if your addition pushes the file past 1500, split in a follow-up).
- The `Args` DTO must match the inline pattern at `mejepa_tools.rs:32-69`: `#[derive(Debug, Deserialize)]` `#[serde(rename_all = "camelCase", deny_unknown_fields)]`.

**Decided scope for this task:**
1. Add `ObserveShiftArgs { shift_id: String }` DTO (validate `^01J[0-9A-F]{20}$` regex inline before dispatch).
2. Add `pub(crate) async fn call_mejepa_observe_shift(&self, id, args) -> JsonRpcResponse` impl on `Handlers`. Body: locate the in-memory `SubscriberIndex` (DashMap<ShiftId, (SessionId, ByteOffset, Option<AttemptId>)>) via the subscriber Arc (sister TASK-SHIFT-007 publishes); if shift_id not found → return `{"shift_found": false, "replay_outcome": null}` (NEVER errors per AC-SHIFT-06-02); if found → invoke `subscriber.process_shift(replayed_entry)` with `replay: true`; the watermark writer enforces `ReplayWatermarkPolicy::DoNotAdvance` so the watermark never rolls back.
3. Use the `MEJEPA_OBSERVE_SHIFT` constant added by TASK-MCP-001.
4. SessionId contract: WatermarkRecord uses `[u8; 16]` everywhere — see TASK-SHIFT-002.
5. Per-step training cert from replay carries `replay: true` flag (cross-cutting with SPEC-TRAINING).

**Cross-phase dep:** sister TASK-SHIFT-007 publishes `ShiftSubscriber::process_shift`; sister TASK-SHIFT-011 enforces EWC++ replay tolerance. Stub locally if SHIFT not landed.

**FSV for this task:** integration test invokes the handler 10 times on the same `shift_id`, asserts (a) `CF_MEJEPA_LIVE_PREDICTIONS` has exactly one row for the resulting `(session_id, witness_chain_offset)` key (Source of Truth: separate read on the CF after the call), (b) watermark CF row unchanged, (c) per-step training certs record 10 `replay: true` entries.

---

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
