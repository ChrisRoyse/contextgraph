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

## 📋 2026-05-10 STATE AUDIT (agent-211-task-updater)

**Audit pass against current tree on 2026-05-10:**

- ✅ **Crate `context-graph-mejepa-shift-subscriber` does NOT exist yet** — `LatencyProfiler` lands in the same not-yet-existing crate.
- ⚠ **`hdrhistogram` workspace dep status: verify before assuming.** Search `Cargo.toml` for `hdrhistogram` — if absent, this task adds it. If a different histogram primitive (e.g. an internal bucketed counter) is already present, prefer using that. As of 2026-05-10 there's no clear evidence the dep is present in the workspace; treat as "add if missing".
- ✅ `LiveSubscriberStatus` (declared in TASK-SHIFT-002) is extended here with a `latency_profile: LatencySnapshot` field — confirm the field addition does not collide with another `latency_*` field from TASK-MCP-005 (PENDING).
- ✅ `process_shift` instrumentation tracks `Instant::now()` deltas at 4 stages (panel / predict / observe / persist) — these stages all exist in TASK-SHIFT-007's body.
- No drift in `<files_to_create>` paths.

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
