# TASK-HEAL-002 — Drift data models (DriftDetector, DriftSample, SeverityTable, DriftSeverity)

```xml
<task_spec id="TASK-HEAL-002" version="1.0">

<metadata>
  <title>Drift-detection data models in `crates/context-graph-mejepa/src/heal/drift.rs` (struct/enum definitions only; method bodies in TASK-HEAL-007)</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>2</sequence>
  <implements>
    <item ref="REQ-HEAL-03">DriftDetector struct + RingBuffer&lt;DriftSample, 1000&gt; + severity table</item>
    <item ref="data_model:DriftDetector">window + stated_coverage + severity_thresholds + hysteresis_windows + last_severity + severity_history</item>
    <item ref="data_model:DriftSample">predicted_set + actual_oracle + witness_chain_offset + ood_score + signal_clarity</item>
    <item ref="data_model:SeverityTable">healthy_min/max + soft_min + hard_min defaults</item>
    <item ref="data_model:DriftSeverity">enum {Healthy, Soft, Hard, Catastrophic, WarmupNotReady}</item>
    <item ref="EC-HEAL-03">WarmupNotReady when window.len() &lt; 1000</item>
    <item ref="EC-HEAL-04">3-window hysteresis on adjacent transitions</item>
  </implements>
  <depends_on><!-- foundation; cross-domain shipped --></depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (src/heal/drift.rs)</estimated_files>
</metadata>

<context>
Second foundation task. Lands the public data model contract for drift detection: a deterministic
ring-buffer of (predicted_set, actual_oracle) samples whose empirical conformal coverage maps to a
4-band severity table (Healthy 88-92% / Soft 85-88% / Hard 80-85% / Catastrophic <80%) with a
3-window hysteresis. Method bodies (push, detect_drift, hysteresis evaluation, persistence) are
deferred to TASK-HEAL-007. This task ships only struct/enum definitions, derives, validating
constructors, and unit tests for invariants (band ordering, window-size cap, hysteresis defaults,
WarmupNotReady gating).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<failure_taxonomy>` MEJEPA_OBSERVE_DRIFT_DETECTED; `<performance_budgets>` conformal_coverage_target ±2%)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-03, AC-HEAL-02-01, AC-HEAL-02-04 catastrophic; EC-HEAL-03 warmup; EC-HEAL-04 hysteresis)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (data_models block: DriftDetector, DriftSample, SeverityTable; drift_severity_diagram)</file>
  <file purpose="cross_domain_inference">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (RealityPrediction.confidence.outcome_set type alignment) — read-only, no import</file>
  <file purpose="naming_conventions">docs/ruvectorfindings/specs/constitution.md#coding_standards</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase5-self-healing/TASK-HEAL-001.md (heal/mod.rs module scaffold)</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-001 complete (heal/mod.rs exists with `pub mod drift;`)</check>
  <check>`serde = "1"`, `bincode = "1"` workspace deps present</check>
</prerequisites>

<scope>
  <in_scope>
    - Fill `crates/context-graph-mejepa/src/heal/drift.rs` with:
      - `DriftSeverity` enum: 5 variants `{Healthy, Soft, Hard, Catastrophic, WarmupNotReady}` with `#[serde(rename_all = "snake_case")]`. Implement `band_index(&self) -> Option<u8>` returning `Some(0..=3)` for Healthy/Soft/Hard/Catastrophic and `None` for WarmupNotReady. Implement `is_actionable(&self) -> bool` returning true for Hard/Catastrophic.
      - `OracleOutcomeRef` opaque newtype `pub struct OracleOutcomeRef(pub [u8; 32])` — placeholder reference into CF_MEJEPA_ORACLE_VERDICTS pending the full OracleOutcome type re-export wiring (the actual OracleOutcome lives in context-graph-mejepa-instruments per Phase 1; this newtype isolates the drift module from cross-crate churn). Add `#[serde(transparent)]`.
      - `DriftSample { predicted_set: Vec<OracleOutcomeRef>, actual_oracle: OracleOutcomeRef, witness_chain_offset: u64, ood_score: f32, signal_clarity: f32 }` with `try_new` validating finite ood_score ∈ [0,1] and signal_clarity ∈ [0,1].
      - `SeverityTable { healthy_min: f32, healthy_max: f32, soft_min: f32, hard_min: f32 }` with `Default::default()` returning `(0.88, 0.92, 0.85, 0.80)` and `try_new` validating monotonic ordering `0.0 <= hard_min <= soft_min <= healthy_min <= healthy_max <= 1.0`.
      - `pub const DEFAULT_DRIFT_WINDOW_SIZE: usize = 1000;`
      - `pub const DEFAULT_HYSTERESIS_WINDOWS: usize = 3;`
      - `pub const DEFAULT_STATED_COVERAGE: f32 = 0.90;`
      - `RingBuffer<T, const N: usize> { data: Vec<T>, head: usize, len: usize }` (FIFO ring, capacity N): `try_new()` (asserts N > 0), `push(item: T) -> Option<T>` (returns evicted oldest when full), `len(&self) -> usize`, `iter(&self) -> impl Iterator<Item = &T>`, `is_full(&self) -> bool`. Generic over T because SeverityHistoryEntry also uses ring; keep the buffer length type-level via a const generic AND an explicit `pub fn capacity(&self) -> usize { N }`.
      - `SeverityHistoryEntry { timestamp_seconds: i64, severity: DriftSeverity, empirical_coverage: f32, window_start_offset: u64, window_end_offset: u64 }` with `try_new` validating empirical_coverage finite ∈ [0,1].
      - `DriftDetector { window: RingBuffer<DriftSample, 1000>, stated_coverage: f32, severity_thresholds: SeverityTable, hysteresis_windows: usize, last_severity: Option<DriftSeverity>, severity_history: Vec<SeverityHistoryEntry>, consecutive_at_new_severity: u32 }` with `try_new(stated_coverage: f32, severity_thresholds: SeverityTable) -> Result<Self, &'static str>`. The `severity_history` is bounded (cap 1024 — Phase 6 GC trims; spec calls "bounded; persisted").
    - Module-level test set in `#[cfg(test)] mod tests`:
      - `severity_band_index_returns_none_for_warmup`
      - `severity_band_index_monotone` (Healthy=0 < Soft=1 < Hard=2 < Catastrophic=3)
      - `severity_is_actionable_only_hard_or_catastrophic`
      - `severity_table_default_matches_spec` (0.88, 0.92, 0.85, 0.80)
      - `severity_table_rejects_non_monotonic` (e.g., hard_min > soft_min)
      - `severity_table_rejects_out_of_range` (negative or >1)
      - `drift_sample_rejects_nan_ood_score`
      - `drift_sample_rejects_signal_clarity_above_one`
      - `ring_buffer_push_evicts_oldest_at_capacity`
      - `ring_buffer_iter_yields_in_insertion_order`
      - `ring_buffer_is_full_after_n_pushes`
      - `drift_detector_constructor_clamps_coverage`
      - `drift_detector_rejects_coverage_above_one`
      - `drift_detector_default_hysteresis_windows_three`
      - `default_constants_match_spec` (1000, 3, 0.90)
      - `serde_round_trip_drift_sample`
  </in_scope>
  <out_of_scope>
    - push() persistence to CF_MEJEPA_DRIFT_WINDOW (TASK-HEAL-007)
    - detect_drift() empirical_coverage computation (TASK-HEAL-007)
    - 3-window hysteresis evaluation logic (TASK-HEAL-007)
    - SeverityHistoryEntry persistence to CF_MEJEPA_DRIFT_HISTORY (TASK-HEAL-007)
    - WarmupNotReady gating logic in detect_drift body (TASK-HEAL-007)
    - The actual OracleOutcome type (lives in context-graph-mejepa-instruments; will be wired to OracleOutcomeRef via TASK-HEAL-007 or a small bridge layer)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/drift.rs">
use serde::{Deserialize, Serialize};

pub const DEFAULT_DRIFT_WINDOW_SIZE: usize = 1000;
pub const DEFAULT_HYSTERESIS_WINDOWS: usize = 3;
pub const DEFAULT_STATED_COVERAGE: f32 = 0.90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftSeverity {
    Healthy,
    Soft,
    Hard,
    Catastrophic,
    WarmupNotReady,
}

impl DriftSeverity {
    pub fn band_index(&amp;self) -&gt; Option&lt;u8&gt;;
    pub fn is_actionable(&amp;self) -&gt; bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OracleOutcomeRef(pub [u8; 32]);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriftSample {
    pub predicted_set: Vec&lt;OracleOutcomeRef&gt;,
    pub actual_oracle: OracleOutcomeRef,
    pub witness_chain_offset: u64,
    pub ood_score: f32,
    pub signal_clarity: f32,
}

impl DriftSample {
    pub fn try_new(predicted_set: Vec&lt;OracleOutcomeRef&gt;,
                   actual_oracle: OracleOutcomeRef,
                   witness_chain_offset: u64,
                   ood_score: f32,
                   signal_clarity: f32) -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct SeverityTable {
    pub healthy_min: f32,
    pub healthy_max: f32,
    pub soft_min: f32,
    pub hard_min: f32,
}

impl Default for SeverityTable { fn default() -&gt; Self; }
impl SeverityTable {
    pub fn try_new(healthy_min: f32, healthy_max: f32, soft_min: f32, hard_min: f32)
        -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RingBuffer&lt;T, const N: usize&gt; {
    pub data: Vec&lt;T&gt;,
    pub head: usize,
    pub len: usize,
}

impl&lt;T: Clone, const N: usize&gt; RingBuffer&lt;T, N&gt; {
    pub fn try_new() -&gt; Result&lt;Self, &amp;'static str&gt;;
    pub fn push(&amp;mut self, item: T) -&gt; Option&lt;T&gt;;
    pub fn len(&amp;self) -&gt; usize;
    pub fn is_full(&amp;self) -&gt; bool;
    pub fn capacity(&amp;self) -&gt; usize;
    pub fn iter(&amp;self) -&gt; impl Iterator&lt;Item = &amp;T&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeverityHistoryEntry {
    pub timestamp_seconds: i64,
    pub severity: DriftSeverity,
    pub empirical_coverage: f32,
    pub window_start_offset: u64,
    pub window_end_offset: u64,
}

impl SeverityHistoryEntry {
    pub fn try_new(timestamp_seconds: i64, severity: DriftSeverity,
                   empirical_coverage: f32, window_start_offset: u64,
                   window_end_offset: u64) -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DriftDetector {
    pub window: RingBuffer&lt;DriftSample, 1000&gt;,
    pub stated_coverage: f32,
    pub severity_thresholds: SeverityTable,
    pub hysteresis_windows: usize,
    pub last_severity: Option&lt;DriftSeverity&gt;,
    pub severity_history: Vec&lt;SeverityHistoryEntry&gt;,
    pub consecutive_at_new_severity: u32,
}

impl DriftDetector {
    pub fn try_new(stated_coverage: f32, severity_thresholds: SeverityTable)
        -&gt; Result&lt;Self, &amp;'static str&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - File &lt; 500 lines per constitution
    - Constants exactly: DEFAULT_DRIFT_WINDOW_SIZE=1000, DEFAULT_HYSTERESIS_WINDOWS=3, DEFAULT_STATED_COVERAGE=0.90
    - SeverityTable defaults exactly: healthy=[0.88, 0.92], soft_min=0.85, hard_min=0.80
    - Monotonic SeverityTable invariant: `0.0 &lt;= hard_min &lt;= soft_min &lt;= healthy_min &lt;= healthy_max &lt;= 1.0`
    - RingBuffer is the bare data shape; persistence wired in TASK-HEAL-007
    - `severity_history` cap 1024 (constant `pub const SEVERITY_HISTORY_CAP: usize = 1024;`)
    - All `f32` fields validated finite at constructor
    - `signal_clarity ∈ [0,1]`, `ood_score ∈ [0,1]`, `empirical_coverage ∈ [0,1]`, `stated_coverage ∈ (0,1)`
    - All public types `Debug + Clone + Serialize + Deserialize + PartialEq` (RingBuffer where `T: Clone`)
    - NO mock data in tests; deterministic synthetic OracleOutcomeRef SHAs only
    - File begins with `// Drift detection data shapes; method bodies in TASK-HEAL-007 (push, detect_drift, hysteresis evaluation).`
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds with zero warnings
    - `cargo test -p context-graph-mejepa heal::drift` shows ≥ 16 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - DriftSeverity::band_index returns None for WarmupNotReady, Some(0..=3) otherwise
    - SeverityTable::default() == SeverityTable { 0.88, 0.92, 0.85, 0.80 }
    - SeverityTable::try_new(hard=0.9, soft=0.5, ...) returns Err (non-monotonic)
    - RingBuffer::push at capacity 1000 returns Some(evicted) on the 1001st push
  </verification>
</definition_of_done>

<pseudo_code>
impl DriftSeverity {
    pub fn band_index(&self) -> Option<u8> {
        match self {
            DriftSeverity::Healthy => Some(0),
            DriftSeverity::Soft => Some(1),
            DriftSeverity::Hard => Some(2),
            DriftSeverity::Catastrophic => Some(3),
            DriftSeverity::WarmupNotReady => None,
        }
    }
    pub fn is_actionable(&self) -> bool {
        matches!(self, DriftSeverity::Hard | DriftSeverity::Catastrophic)
    }
}

impl SeverityTable {
    pub fn try_new(healthy_min, healthy_max, soft_min, hard_min) -> Result<Self, &'static str> {
        for x in [healthy_min, healthy_max, soft_min, hard_min] {
            if !x.is_finite() || x < 0.0 || x > 1.0 { return Err("threshold out of [0,1] or non-finite"); }
        }
        if !(hard_min <= soft_min && soft_min <= healthy_min && healthy_min <= healthy_max) {
            return Err("non-monotonic severity table: require hard_min <= soft_min <= healthy_min <= healthy_max");
        }
        Ok(Self { healthy_min, healthy_max, soft_min, hard_min })
    }
}

impl<T: Clone, const N: usize> RingBuffer<T, N> {
    pub fn try_new() -> Result<Self, &'static str> {
        if N == 0 { return Err("ring buffer capacity must be > 0"); }
        Ok(Self { data: Vec::with_capacity(N), head: 0, len: 0 })
    }
    pub fn push(&mut self, item: T) -> Option<T> {
        if self.len < N {
            self.data.push(item);
            self.len += 1;
            None
        } else {
            // Evict oldest (at index `head`); store new at the same slot; advance head.
            let evicted = std::mem::replace(&mut self.data[self.head], item);
            self.head = (self.head + 1) % N;
            Some(evicted)
        }
    }
}

impl DriftDetector {
    pub fn try_new(stated_coverage, severity_thresholds) -> Result<Self, &'static str> {
        if !stated_coverage.is_finite() || stated_coverage <= 0.0 || stated_coverage >= 1.0 {
            return Err("stated_coverage must be in (0, 1) and finite");
        }
        Ok(Self {
            window: RingBuffer::try_new()?,
            stated_coverage,
            severity_thresholds,
            hysteresis_windows: DEFAULT_HYSTERESIS_WINDOWS,
            last_severity: None,
            severity_history: Vec::new(),
            consecutive_at_new_severity: 0,
        })
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/heal/drift.rs">DriftSeverity + DriftSample + SeverityTable + RingBuffer + SeverityHistoryEntry + DriftDetector + OracleOutcomeRef + DEFAULT_* constants + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/heal/mod.rs">replace placeholder `pub mod drift;` line (already declared by TASK-HEAL-001) — no change needed if the module already declares; otherwise no-op</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa heal::drift shows ≥ 16 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>DriftSeverity has exactly 5 variants in canonical order</criterion>
  <criterion>SeverityTable::default() defaults exactly (0.88, 0.92, 0.85, 0.80)</criterion>
  <criterion>SeverityTable::try_new rejects non-monotonic ordering</criterion>
  <criterion>SeverityTable::try_new rejects values outside [0,1]</criterion>
  <criterion>RingBuffer&lt;_, 1000&gt;::push at capacity returns Some(evicted)</criterion>
  <criterion>DriftSample::try_new rejects ood_score &gt; 1.0 or non-finite</criterion>
  <criterion>DriftDetector::try_new rejects stated_coverage outside (0,1)</criterion>
  <criterion>SEVERITY_HISTORY_CAP == 1024</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::drift</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
