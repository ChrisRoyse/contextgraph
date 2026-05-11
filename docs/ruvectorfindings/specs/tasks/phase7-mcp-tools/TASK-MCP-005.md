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

**This task creates a NEW MCP handler.** Today no `mejepa_subscriber_status` const, schema, dispatcher arm, or handler exists.

**Drift from original task body:**
- File path `crates/context-graph-mcp/src/handlers/tools/mejepa/subscriber_status.rs` is wrong — the sub-module tree was never created.
- Place the new handler in `mejepa_tools.rs` next to `call_mejepa_predict_latest`.

**Decided scope for this task:**
1. Add `SubscriberStatusArgs` DTO (empty struct with `#[serde(deny_unknown_fields)]`).
2. Add `pub(crate) async fn call_mejepa_subscriber_status(&self, id, _args) -> JsonRpcResponse` on `Handlers`. Body: snapshot atomics from `SubscriberMetrics` (sister TASK-SHIFT-002 type), read CF_MEJEPA_SHIFT_WATERMARK to compose `last_watermark_per_session: HashMap<String, String>` (key is hex-encoded SessionId), measure RSS via `procfs::process::Process::myself()?.statm()?.resident * page_size` (add `procfs` to crate `[dependencies]` in `Cargo.toml`), package as `LiveSubscriberStatus` (sister TASK-SHIFT-002).
3. Use the `MEJEPA_SUBSCRIBER_STATUS` constant added by TASK-MCP-001.
4. NEVER returns stale-but-cached state — `task_alive_since: None` when supervisor detected panic; `last_panic: Some(PanicInfo)` populated.
5. Latency p50 < 50 ms target (in-memory atomics + single CF prefix iter).

**Cross-phase dep:** sister TASK-SHIFT-002 (`SubscriberMetrics`, `LiveSubscriberStatus`, `PanicInfo`); sister TASK-SHIFT-008 (watermark reader); sister TASK-SHIFT-010 (subscriber Arc + JoinSet supervisor). Stub locally if SHIFT not landed.

**FSV for this task:** integration test (a) runs the subscriber for 1,000 synthetic shifts → invokes handler → asserts every documented field populated and finite (Source of Truth: response JSON + separate atomic readback); (b) injects a panic via test hook → asserts `task_alive_since: None` plus `last_panic: Some(_)`; (c) measures latency on 100 sequential calls → asserts p50 < 50 ms.

---

<task_spec id="TASK-MCP-005" version="1.0">
<metadata>
  <title>Logic — `mejepa_subscriber_status` snapshot handler reading SubscriberMetrics + watermark CF + cache hit rate</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>5</sequence>
  <implements>
    <item ref="REQ-SHIFT-07">`mejepa_subscriber_status` registered + responds; `additionalProperties: false`</item>
    <item ref="REQ-SHIFT-12">Status snapshot fields: per-session watermarks, lag, alert state, error rate, cache hit rate, RSS, alive_since, dropped count, last panic</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#api_contracts[mejepa_subscriber_status]">request (empty) / response (LiveSubscriberStatus)</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#component_contracts[MejepaSubscriberStatusHandler]">behaviour: snapshot atomics → CF read → package</item>
    <item ref="TC-SHIFT-12">task-dead reports honestly (`task_alive_since: None`, `last_panic: Some(...)`)</item>
    <item ref="NFR-SHIFT-04">p50 &lt; 50 ms (in-memory atomics + one CF read)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-MCP-001</task_ref>
    <!-- Cross-phase: SubscriberMetrics types + LagMonitor watermark CF -->
    <task_ref>TASK-SHIFT-002</task_ref>
    <task_ref>TASK-SHIFT-008</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
A status snapshot for operators. Reads atomic counters from `Arc<SubscriberMetrics>` (sister TASK-SHIFT-002) — lag, lag_alert_state, hits/misses → hit_rate, dropped_l_step_count, RSS via procfs, task_alive_since, last_panic — then point-reads `CF_MEJEPA_SHIFT_WATERMARK` for per-session watermarks (sister TASK-SHIFT-008's `WatermarkWriter::read_all_watermarks`). The snapshot is fully atomic: every counter is loaded once with `Ordering::Acquire`, no log scans, no reconstruction. `task_alive_since: None` ⇒ subscriber Tokio task dead — REPORT HONESTLY (do NOT return stale-but-cached state per REQ-SHIFT-12).

The `mejepa_subscriber_status` tool takes NO parameters. The empty-object schema is the only valid input; any extra field is rejected.
</context>

<input_context_files>
  <file purpose="api_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#api_contracts[mejepa_subscriber_status]</file>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#component_contracts[MejepaSubscriberStatusHandler]</file>
  <file purpose="status_data_model">docs/ruvectorfindings/specs/functional/SPEC-SHIFT-SUBSCRIBER.md (SubscriberStatus entity)</file>
  <file purpose="metrics_owner">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-002.md</file>
  <file purpose="watermark_writer_owner">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-008.md</file>
  <file purpose="prior_skeleton">docs/ruvectorfindings/specs/tasks/phase7-mcp-tools/TASK-MCP-001.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-MCP-001 complete: skeleton + SubscriberStatusArgs + stub handle()</check>
  <check>TASK-SHIFT-002 complete: `SubscriberMetrics`, `LagAlertState`, `PanicInfo`, `SubscriberStatus`/`LiveSubscriberStatus` carrier types shipped</check>
  <check>TASK-SHIFT-008 complete: `WatermarkWriter::read_all_watermarks` + `WatermarkRecord` shipped</check>
  <check>The MCP context exposes `Arc<SubscriberMetrics>`, `Arc<WatermarkReader>` (existing accessors)</check>
</prerequisites>

<scope>
  <in_scope>
    - Replace stub body in `crates/context-graph-mcp/src/handlers/tools/mejepa/subscriber_status.rs`
    - Read atomic counters in one snapshot pass (single function `metrics_snapshot()`):
      - `lag: u64` (current global lag = sum of per-session lag)
      - `lag_alert_active: bool` (derived from atomic `LagAlertState` enum)
      - `error_rate_per_1000: f32` ∈ [0,1] (rolling-1000-shift error rate from CircularBuffer)
      - `instrument_cache_hit_rate: f32` ∈ [0,1] (`hits / (hits + misses)`; if hits+misses == 0, return 0.0 — finite per `<error_handling>`)
      - `instrument_cache_capacity: usize`, `instrument_cache_entries: usize`
      - `rss_bytes: u64` via `procfs::process::Process::myself()?.statm()?.resident * page_size` on Linux; on non-Linux, return `0` and log warn
      - `task_alive_since: Option<i64>` (None ⇒ task dead OR not yet started)
      - `dropped_l_step_below_threshold_count: u64`
      - `last_panic: Option<PanicInfo>` (None until first panic)
    - Read `CF_MEJEPA_SHIFT_WATERMARK` via `WatermarkReader::read_all_watermarks()` for per-session watermarks; derive `last_watermark_per_session: HashMap<SessionId, ShiftId>`
    - Build canonical JSON response using sorted keys (BTreeMap) for deterministic diff across replays
    - Single empty-args validation (deny extra fields); return structured error `MEJEPA_SUBSCRIBER_STATUS_UNEXPECTED_ARG` if any field is provided
    - Latency budget per NFR-SHIFT-04: p50 &lt; 50 ms (1,000 calls in test, recorded p50)
    - Unit tests: 6 tests including a panic-injection test asserting `task_alive_since: None, last_panic: Some(...)` per TC-SHIFT-12
  </in_scope>
  <out_of_scope>
    - The metrics writer side (sister TASK-SHIFT-002, TASK-SHIFT-008, TASK-SHIFT-009)
    - The lag-alert state machine itself (TASK-SHIFT-009)
    - FSV roundtrip (TASK-MCP-007)
    - Latency export (TASK-SHIFT-012)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/subscriber_status.rs">
      use std::collections::BTreeMap;

      pub async fn handle(
          args: serde_json::Value,
          ctx: &amp;crate::handlers::McpContext,
      ) -&gt; Result&lt;serde_json::Value, crate::error::McpError&gt;;

      fn metrics_snapshot(metrics: &amp;crate::handlers::tools::mejepa::traits::SubscriberMetrics) -&gt; AtomicSnapshot;
      fn rss_bytes_or_warn() -&gt; u64;
      fn finite_hit_rate(hits: u64, misses: u64) -&gt; f32;

      struct AtomicSnapshot {
          lag: u64,
          lag_alert_active: bool,
          error_rate_per_1000: f32,
          cache_hit_rate: f32,
          cache_capacity: usize,
          cache_entries: usize,
          rss_bytes: u64,
          task_alive_since: Option&lt;i64&gt;,
          dropped_l_step_count: u64,
          last_panic: Option&lt;crate::handlers::tools::mejepa::traits::PanicInfo&gt;,
      }
    </signature>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/traits.rs">
      // Add to traits.rs (extends prior tasks):
      pub use crate::handlers::tools::mejepa::external::SubscriberMetrics;
      pub use crate::handlers::tools::mejepa::external::PanicInfo;
      pub use crate::handlers::tools::mejepa::external::WatermarkRecord;
      pub use crate::handlers::tools::mejepa::external::WatermarkReader;
    </signature>
  </signatures>

  <constraints>
    - Empty-object schema strictly enforced; any provided field rejected with `MEJEPA_SUBSCRIBER_STATUS_UNEXPECTED_ARG`
    - All atomic loads use `Ordering::Acquire` (or `Relaxed` for monotonic counters where staleness is acceptable, documented per field)
    - `finite_hit_rate(0, 0) == 0.0` (NOT NaN); response field is always finite per `<error_handling>` rule on no `unwrap_or` defaults
    - Output JSON keys sorted lexicographically (BTreeMap) — deterministic diff across replays per TECH-SHIFT-SUBSCRIBER `<implementation_notes>` performance row
    - RSS read on non-Linux falls back to `0` with `tracing::warn!` once (not per-request) — use `Once` to gate the warn
    - The handler reads `CF_MEJEPA_SHIFT_WATERMARK` ONCE per call; do NOT iterate per-session in a loop with separate point reads
    - `task_alive_since: None` is reported AS-IS — the handler does NOT attempt to "infer alive" from staleness
    - Latency budget asserted in test: 1,000 calls; p50 &lt; 50 ms; record histogram with `std::time::Instant`
    - This task MUST NOT modify metrics writer code (sister phase)
    - File length under 500 lines
  </constraints>

  <verification>
    - Test 1 (happy path): mock SubscriberMetrics with hits=70, misses=30, lag=42, alert=Idle, dropped=5, alive_since=Some(1700000000); response field-by-field matches input; cache_hit_rate == 0.7
    - Test 2 (zero-traffic edge): hits=0, misses=0 → cache_hit_rate == 0.0 (NOT NaN); JSON deserialize confirms finite
    - Test 3 (panic injected): metrics has `last_panic: Some(PanicInfo{message,backtrace,captured_at,last_in_flight_shift})`, alive_since: None → response carries both per TC-SHIFT-12
    - Test 4 (per-session watermarks): mock WatermarkReader returns 3 records; response `last_watermark_per_session` is a 3-entry HashMap with sorted-deterministic JSON
    - Test 5 (unexpected arg): `{ "extra": 1 }` rejected with `MEJEPA_SUBSCRIBER_STATUS_UNEXPECTED_ARG`
    - Test 6 (latency): 1,000 sequential calls; p50 &lt; 50 ms (recorded via `std::time::Instant`); fail loudly if p50 ≥ 50 ms
    - `cargo build --release -p context-graph-mcp` clean
    - `cargo clippy -p context-graph-mcp -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
subscriber_status::handle(args, ctx):
  // Strict empty-arg validation
  let parsed: SubscriberStatusArgs = schema_validator::parse_strict(args)?;
  let _ = parsed; // unit struct; no fields

  let snap = metrics_snapshot(&ctx.subscriber_metrics);

  // CF read for watermarks (point read on prefix `wm:`)
  let watermark_records = ctx.watermark_reader.read_all_watermarks()
    .map_err(|e| McpError::structured("MEJEPA_SUBSCRIBER_STATUS_WATERMARK_READ_FAILED", json!({"reason": format!("{e}")})))?;
  let mut last_watermark_per_session: BTreeMap<String, String> = BTreeMap::new();
  for r in watermark_records {
    last_watermark_per_session.insert(r.session_id, r.last_consumed_shift_id);
  }

  // Build response with sorted keys
  Ok(serde_json::json!({
    "lag":                              snap.lag,
    "lag_alert_active":                 snap.lag_alert_active,
    "error_rate_per_1000":              snap.error_rate_per_1000,
    "instrument_cache_hit_rate":        snap.cache_hit_rate,
    "instrument_cache_capacity":        snap.cache_capacity,
    "instrument_cache_entries":         snap.cache_entries,
    "last_panic":                       snap.last_panic,
    "last_watermark_per_session":       last_watermark_per_session,
    "rss_bytes":                        snap.rss_bytes,
    "task_alive_since":                 snap.task_alive_since,
    "dropped_l_step_below_threshold_count": snap.dropped_l_step_count,
  }))

finite_hit_rate(hits, misses):
  let total = hits.saturating_add(misses);
  if total == 0 { 0.0 } else { (hits as f64 / total as f64) as f32 }
</pseudo_code>

<files_to_create>
  <!-- (none — modifies existing files only) -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/subscriber_status.rs">replace stub with real handler + AtomicSnapshot + 6 unit tests</file>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/traits.rs">re-export `SubscriberMetrics`, `PanicInfo`, `WatermarkRecord`, `WatermarkReader` from sister phase carrier types</file>
  <file path="crates/context-graph-mcp/Cargo.toml">add `procfs` workspace dep (Linux-gated) if not present</file>
</files_to_modify>

<validation_criteria>
  <criterion>All 6 unit tests pass</criterion>
  <criterion>Latency p50 &lt; 50 ms across 1,000 calls</criterion>
  <criterion>Zero-traffic cache_hit_rate is 0.0 (finite, not NaN)</criterion>
  <criterion>Panic-injection test: alive_since: None, last_panic: Some populated</criterion>
  <criterion>Empty-arg schema strict; any extra field rejected</criterion>
  <criterion>Response JSON keys deterministically sorted</criterion>
  <criterion>No file exceeds 500 lines</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mcp</command>
  <command>cargo test -p context-graph-mcp mejepa::subscriber_status</command>
  <command>cargo clippy -p context-graph-mcp -- -D warnings</command>
</test_commands>
</task_spec>
