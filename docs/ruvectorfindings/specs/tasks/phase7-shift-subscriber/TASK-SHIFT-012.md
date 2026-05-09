<task_spec id="TASK-SHIFT-012" version="1.0">
<metadata>
  <title>Logic — Per-shift latency profiling (p50 < 5s cached, p99 < 30s cold)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>12</sequence>
  <implements>
    <item ref="REQ-SHIFT-16">Performance budgets enforced + measured</item>
    <item ref="NFR-SHIFT-01">p50 < 5 s</item>
    <item ref="NFR-SHIFT-02">p99 < 30 s</item>
    <item ref="AC-SHIFT-02-01">cold path &lt; 30s</item>
    <item ref="AC-SHIFT-02-02">warm path &lt; 5s</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (latency.rs) + tests</estimated_files>
</metadata>

<context>
Adds per-shift latency profiling with p50/p95/p99 percentile estimation, breakdown by cold/warm cache, exposed through `mejepa_subscriber_status` (sister phase). Uses HDR-histogram-style bucketing with bounded memory (no unbounded vector growth). Latency tracked separately for: (a) panel materialization, (b) predict() forward, (c) observe() update, (d) RocksDB persistence. Total per-shift wall time is the sum.
</context>

<input_context_files>
  <file purpose="performance_budgets">docs/ruvectorfindings/specs/constitution.md#performance_budgets</file>
  <file purpose="latency_breakdown">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#implementation_notes (performance row)</file>
  <file purpose="metrics_skeleton">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-002.md</file>
  <file purpose="run_loop">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-010.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-010 complete</check>
  <check>`hdrhistogram` workspace dep available (or equivalent bounded-memory percentile estimator)</check>
</prerequisites>

<scope>
  <in_scope>
    - `crates/context-graph-mejepa-shift-subscriber/src/latency.rs` — `LatencyProfiler` with 4 sub-histograms (panel, predict, observe, persist)
    - Each histogram: `hdrhistogram::Histogram<u64>` with range [1us, 60s], precision 3
    - Public API: `record_panel(duration_ms)`, `record_predict(...)`, `record_observe(...)`, `record_persist(...)`, `snapshot() -> LatencySnapshot { p50, p95, p99, max, count }` per stage
    - Cold-vs-warm separation: panel-stage histogram has 2 sub-histograms; selector by `cache_hits == 15` (all warm) → warm; else → cold
    - Wire calls into `process_shift` body (TASK-SHIFT-007 / TASK-SHIFT-011): `Instant::now()` before each stage, record on each stage completion
    - `LiveSubscriberStatus` (TASK-SHIFT-002) gains `latency_profile: LatencySnapshot` field
    - Tests: feed 1,000 synthetic shifts with controlled durations; assert reported p50/p99 within 5% of ground truth
    - Tests: warm/cold separation: 700 warm + 300 cold shifts → warm.p50 &lt; cold.p50
  </in_scope>
  <out_of_scope>
    - Latency export to OpenTelemetry/Prometheus (out of scope — operator polls via mejepa_subscriber_status)
    - Tier-compression of historical latency data (SPEC-HYGIENE)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/latency.rs">
      use hdrhistogram::Histogram;
      use parking_lot::RwLock;
      use serde::{Serialize, Deserialize};

      pub struct LatencyProfiler {
          panel_warm: RwLock&lt;Histogram&lt;u64&gt;&gt;,
          panel_cold: RwLock&lt;Histogram&lt;u64&gt;&gt;,
          predict: RwLock&lt;Histogram&lt;u64&gt;&gt;,
          observe: RwLock&lt;Histogram&lt;u64&gt;&gt;,
          persist: RwLock&lt;Histogram&lt;u64&gt;&gt;,
      }

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct StageLatency {
          pub p50_us: u64,
          pub p95_us: u64,
          pub p99_us: u64,
          pub max_us: u64,
          pub count: u64,
      }

      #[derive(Debug, Clone, Serialize, Deserialize)]
      pub struct LatencySnapshot {
          pub panel_warm: StageLatency,
          pub panel_cold: StageLatency,
          pub predict: StageLatency,
          pub observe: StageLatency,
          pub persist: StageLatency,
          pub overall_p50_ms: u64,
          pub overall_p99_ms: u64,
      }

      impl LatencyProfiler {
          pub fn new() -&gt; Result&lt;Self, hdrhistogram::CreationError&gt;;
          pub fn record_panel(&amp;self, duration_us: u64, all_warm: bool);
          pub fn record_predict(&amp;self, duration_us: u64);
          pub fn record_observe(&amp;self, duration_us: u64);
          pub fn record_persist(&amp;self, duration_us: u64);
          pub fn snapshot(&amp;self) -&gt; LatencySnapshot;
      }
    </signature>
  </signatures>

  <constraints>
    - HDR-histogram range [1, 60_000_000] microseconds (1 us – 60 s); precision 3 significant figures
    - Bounded memory: each histogram ≤ 1 MB independent of input count
    - `record_*` calls are lock-free for the common case (write-uncontended)
    - `snapshot()` p50/p95/p99 query latency &lt; 1 ms
    - File length under 500 lines
  </constraints>

  <verification>
    - 1,000-shift synthetic test: reported p50/p95/p99 within 5% of ground truth (Wilcoxon rank-sum)
    - Warm/cold separation: 700/300 split → warm.p50 strictly less than cold.p50
    - Snapshot serializes deterministically
    - `cargo clippy` clean
  </verification>
</definition_of_done>

<pseudo_code>
record_panel(duration_us, all_warm):
  let h = if all_warm { &self.panel_warm } else { &self.panel_cold };
  h.write().record(duration_us).ok();   -- silently drops out-of-range; we intentionally ignore overflows

snapshot():
  StageLatency{
    p50_us: h.value_at_quantile(0.50),
    p95_us: h.value_at_quantile(0.95),
    p99_us: h.value_at_quantile(0.99),
    max_us: h.max(),
    count: h.len() as u64,
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/latency.rs">LatencyProfiler + StageLatency + LatencySnapshot</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/tests/latency_drills.rs">1,000-shift accuracy + warm/cold separation</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/audit.rs">add `latency_profile: LatencySnapshot` to LiveSubscriberStatus</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/process.rs">instrument process_shift with Instant timers + record_* calls</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/lib.rs">`pub mod latency; pub use latency::*;`</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/Cargo.toml">add `hdrhistogram` workspace dep</file>
</files_to_modify>

<validation_criteria>
  <criterion>p50/p95/p99 reported within 5% of ground truth on synthetic distribution</criterion>
  <criterion>Warm vs cold separation: warm.p50 &lt; cold.p50</criterion>
  <criterion>Memory bounded: 100K records fit in &lt; 1 MB per histogram</criterion>
  <criterion>Snapshot latency &lt; 1 ms</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber latency::</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber --test latency_drills</command>
  <command>cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings</command>
</test_commands>
</task_spec>
