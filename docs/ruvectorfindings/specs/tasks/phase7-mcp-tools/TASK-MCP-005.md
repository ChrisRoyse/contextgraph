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
