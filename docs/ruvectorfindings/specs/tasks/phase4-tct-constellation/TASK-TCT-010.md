# TASK-TCT-010 — ViolationRateAggregator: rolling-window constellation_violation_rate (cross-cutting with UTML)

```xml
<task_spec id="TASK-TCT-010" version="1.0">

<metadata>
  <title>ViolationRateAggregator in `crates/context-graph-mejepa-tct/src/rate_aggregator.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>10</sequence>
  <implements>
    <item ref="REQ-TCT-13">constellation_violation_rate over rolling window; Option::None when N&lt;30</item>
    <item ref="AC-TCT-06-01">Window default 1,000 calls or 24 hours, whichever is smaller</item>
    <item ref="AC-TCT-06-02">Rate &gt; 0.05 sustained 2 hours triggers Phase 5 Latent-Collapsing intervention (consumer-side)</item>
    <item ref="data_model:CF_MEJEPA_GUARD_DECISIONS">key=verdict_id; rolling iteration</item>
    <item ref="constitution:rocksdb_cfs">CF_ prefix; CF_MEJEPA_GUARD_DECISIONS naming</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TCT-006</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (rate_aggregator.rs)</estimated_files>
</metadata>

<context>
The constellation_violation_rate is consumed by SPEC-UTML compute_delta_xi as the third predictor-
side defense diagnostic per doc 11 §5.3, and by Phase 5 self-healing as the Latent-Collapsing
intervention trigger when sustained &gt; 5% over 2 hours. Reads CF_MEJEPA_GUARD_DECISIONS in reverse-
time order, filters to window, returns rejection rate or Option::None on insufficient samples
(&lt; 30).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (rocksdb_cfs naming)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TCT.md (REQ-TCT-13; AC-TCT-06-01..03; BR-TCT-07, 08; EC-TCT-11)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TCT.md (component_contracts ViolationRateAggregator.constellation_violation_rate)</file>
  <file purpose="upstream_rate_types">crates/context-graph-mejepa-tct/src/rate.rs (RollingWindow; TASK-TCT-002)</file>
  <file purpose="upstream_verdict">crates/context-graph-mejepa-tct/src/verdict.rs (VerdictGuardRejected; TASK-TCT-002)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TCT-002 complete; RollingWindow + VerdictGuardRejected exist</check>
  <check>TASK-TCT-006 complete; Gτ produces Verdict::GuardRejected</check>
  <check>`rocksdb`, `bincode`, `chrono` workspace deps</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa-tct/src/rate_aggregator.rs` with:
      - `pub const CF_MEJEPA_GUARD_DECISIONS: &str = "CF_MEJEPA_GUARD_DECISIONS";`
      - `pub const MIN_SAMPLES_FOR_RATE: usize = 30;`
      - `pub enum VerdictRecord { Approve(VerdictApproveSummary), GuardRejected(VerdictGuardRejected) }` (the persisted record kind; Approve summary is a small struct keyed by timestamp + verdict_id)
      - `pub struct VerdictApproveSummary { pub timestamp: SystemTime, pub constellation_version_id: [u8; 32] }`
      - `pub struct ViolationRateAggregator { db: Arc<rocksdb::DB> }`
      - `pub fn new(db: Arc<rocksdb::DB>) -> Result<Self, TctError>` validating CF exists
      - `pub fn record_verdict(&self, record: &VerdictRecord) -> Result<(), TctError>` — bincode-serialize + put_cf with key = `timestamp_be(8) || verdict_ulid(16)`
      - `pub fn constellation_violation_rate(&self, window: RollingWindow, now: SystemTime) -> Result<Option<f32>, TctError>` per TECH-TCT behavior
    - Window logic: read up to `window.window_size_calls` most-recent verdicts via reverse iterator on CF_MEJEPA_GUARD_DECISIONS; filter to `timestamp >= now - window.window_hours hours`; total = filtered.len(); if `total < MIN_SAMPLES_FOR_RATE` return Ok(None); rejected = filtered.filter(|v| matches!(v, VerdictRecord::GuardRejected(_))).count(); return Ok(Some(rejected as f32 / total as f32))
    - Read env var overrides `MEJEPA_TCT_RATE_WINDOW_SIZE` and `MEJEPA_TCT_RATE_WINDOW_HOURS` to populate RollingWindow defaults
    - Add integration tests:
      - `rate_zero_when_no_violations`: 100 Approve records → rate = 0.0
      - `rate_quarter_when_25_of_100_rejected`: 25 GuardRejected + 75 Approve → 0.25
      - `rate_returns_none_when_below_min_samples`: 29 records → Option::None
      - `rate_uses_smaller_of_calls_or_hours_window`: 5,000 records spanning 48 hours, window={1000, 24} → reads ≤ 1,000 within 24 hours
      - `rate_filters_by_window_hours`: 100 records distributed across 48 hours, window_hours=24, now-anchor → only last 24 hours considered
      - `rate_env_var_override_window_size`: MEJEPA_TCT_RATE_WINDOW_SIZE=500 → window.window_size_calls=500
      - `record_verdict_persists_with_canonical_key`: persist + reverse-iterate returns the record
  </in_scope>
  <out_of_scope>
    - Phase 5 self-healing trigger logic (consumes the rate; this task only produces it)
    - SPEC-UTML compute_delta_xi consumer wiring (consumes the rate; this task only produces it)
    - Constellation refit (Phase 5)
    - MCP wiring (TASK-TCT-011)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-tct/src/rate_aggregator.rs">
use std::sync::Arc;
use std::time::SystemTime;
use rocksdb::DB;
use crate::rate::RollingWindow;
use crate::verdict::VerdictGuardRejected;
use crate::error::TctError;

pub const CF_MEJEPA_GUARD_DECISIONS: &amp;str = "CF_MEJEPA_GUARD_DECISIONS";
pub const MIN_SAMPLES_FOR_RATE: usize = 30;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum VerdictRecord {
    Approve(VerdictApproveSummary),
    GuardRejected(VerdictGuardRejected),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VerdictApproveSummary {
    pub timestamp: SystemTime,
    pub constellation_version_id: [u8; 32],
}

pub struct ViolationRateAggregator {
    db: Arc&lt;DB&gt;,
}

impl ViolationRateAggregator {
    pub fn new(db: Arc&lt;DB&gt;) -&gt; Result&lt;Self, TctError&gt;;

    pub fn record_verdict(&amp;self, record: &amp;VerdictRecord)
        -&gt; Result&lt;(), TctError&gt;;

    pub fn constellation_violation_rate(
        &amp;self,
        window: RollingWindow,
        now: SystemTime,
    ) -&gt; Result&lt;Option&lt;f32&gt;, TctError&gt;;
}

pub fn read_window_config() -&gt; RollingWindow;
    </signature>
  </signatures>
  <constraints>
    - CF name MUST be `CF_MEJEPA_GUARD_DECISIONS` per constitution rocksdb_cfs naming
    - Key encoding: `timestamp_be(8) || ulid(16)` enables time-ordered reverse iteration
    - Window: smaller-of (size_calls, hours) controls effective filter
    - MIN_SAMPLES_FOR_RATE = 30; below → Option::None (NOT 0.0)
    - Returned rate always ∈ [0.0, 1.0]
    - Determinism: same DB state + same `now` → byte-equal rate
    - Read env var overrides MEJEPA_TCT_RATE_WINDOW_SIZE / MEJEPA_TCT_RATE_WINDOW_HOURS
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-tct` zero warnings
    - `cargo test -p context-graph-mejepa-tct rate_aggregator` shows ≥ 7 passing tests
    - `cargo clippy -p context-graph-mejepa-tct -- -D warnings` clean
    - Rate=0 with 100 approves
    - Rate=0.25 with 25/100 rejections
    - Insufficient samples returns None
    - Window filter respected
  </verification>
</definition_of_done>

<pseudo_code>
impl ViolationRateAggregator {
    pub fn record_verdict(&self, record: &VerdictRecord) -> Result<(), TctError> {
        let cf = self.db.cf_handle(CF_MEJEPA_GUARD_DECISIONS).unwrap();
        let timestamp = match record {
            VerdictRecord::Approve(a) => a.timestamp,
            VerdictRecord::GuardRejected(_) => SystemTime::now(),
        };
        let ts_secs = timestamp.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let mut key = Vec::with_capacity(24);
        key.extend_from_slice(&ts_secs.to_be_bytes());
        let ulid: [u8; 16] = ulid::Ulid::new().into();
        key.extend_from_slice(&ulid);
        let bytes = bincode::serialize(record)
            .map_err(|_| TctError::DimMismatch { expected: 1, actual: 0 })?;
        self.db.put_cf(cf, key, bytes)
            .map_err(|_| TctError::DimMismatch { expected: 1, actual: 0 })?;
        Ok(())
    }

    pub fn constellation_violation_rate(&self, window, now) -> Result<Option<f32>, TctError> {
        let cf = self.db.cf_handle(CF_MEJEPA_GUARD_DECISIONS).unwrap();
        let cutoff = now - std::time::Duration::from_secs(window.window_hours as u64 * 3600);
        let cutoff_secs = cutoff.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);

        let mut iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::End);
        let mut filtered: Vec<VerdictRecord> = Vec::new();
        let mut taken = 0usize;
        while taken < window.window_size_calls as usize {
            match iter.next() {
                Some(Ok((k, v))) => {
                    let ts_be: [u8; 8] = k[..8].try_into().unwrap_or([0u8; 8]);
                    let ts_secs = u64::from_be_bytes(ts_be);
                    if ts_secs < cutoff_secs { break; }
                    if let Ok(rec) = bincode::deserialize::<VerdictRecord>(&v) {
                        filtered.push(rec);
                        taken += 1;
                    }
                },
                _ => break,
            }
        }
        let total = filtered.len();
        if total < MIN_SAMPLES_FOR_RATE { return Ok(None); }
        let rejected = filtered.iter().filter(|r| matches!(r, VerdictRecord::GuardRejected(_))).count();
        Ok(Some(rejected as f32 / total as f32))
    }
}

pub fn read_window_config() -> RollingWindow {
    let size = std::env::var("MEJEPA_TCT_RATE_WINDOW_SIZE").ok()
        .and_then(|v| v.parse::<u32>().ok()).unwrap_or(1_000);
    let hours = std::env::var("MEJEPA_TCT_RATE_WINDOW_HOURS").ok()
        .and_then(|v| v.parse::<u32>().ok()).unwrap_or(24);
    RollingWindow { window_size_calls: size, window_hours: hours }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-tct/src/rate_aggregator.rs">VerdictRecord + ViolationRateAggregator + read_window_config + 7 integration tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-tct/src/lib.rs">add `pub mod rate_aggregator;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa-tct zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa-tct rate_aggregator shows ≥ 7 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa-tct -- -D warnings clean</criterion>
  <criterion>CF name == "CF_MEJEPA_GUARD_DECISIONS"</criterion>
  <criterion>Rate=0.0 with 100 approves</criterion>
  <criterion>Rate=0.25 with 25/100 rejections</criterion>
  <criterion>N&lt;30 returns Option::None</criterion>
  <criterion>Smaller-of window logic verified</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-tct</command>
  <command>cargo test -p context-graph-mejepa-tct rate_aggregator</command>
  <command>cargo clippy -p context-graph-mejepa-tct -- -D warnings</command>
</test_commands>

</task_spec>
```
