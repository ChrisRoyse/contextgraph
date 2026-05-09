# TASK-HYG-008 — quota::snapshot + degradation_for + score_for_eviction

```xml
<task_spec id="TASK-HYG-008" version="1.0">
<metadata>
  <title>quota::snapshot + degradation_for + score_for_eviction (recency exp decay; uplift × freq × recency)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <requirement_ref>REQ-HYG-04</requirement_ref>
    <requirement_ref>REQ-HYG-10</requirement_ref>
    <requirement_ref>REQ-HYG-05</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Pure-math quota layer. `snapshot` reads cached per-CF size estimates and per-CF Unrecoverable
flags from CF_MEJEPA_QUOTA_STATE and assembles a QuotaStatus that the MCP `mejepa_quota_status`
tool serializes. `degradation_for` maps `(used, budget, unrecoverable)` → DegradationStatus
(Healthy &lt; 80%, Approaching 80-95%, OverBudget 95-100%, Unrecoverable post-failed-GC, sticky).
`score_for_eviction` is the deterministic ranking function for both nightly bottom-1% and
emergency eviction (`uplift × access_frequency × recency_factor`; recency_factor =
exp(-recency_seconds / 30d)). Latency target &lt; 50 ms p50 (NFR-HYG-03) — read cache, not full CF.
</context>

<input_context_files>
  <file purpose="quota_component">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="degradation_thresholds">../../functional/SPEC-HYGIENE.md#requirements</file>
  <file purpose="reports_types">crates/context-graph-mejepa-hygiene/src/reports.rs</file>
  <file purpose="quota_types">crates/context-graph-mejepa-hygiene/src/quota_types.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-003 shipped (QuotaStatus + CategoryStatus + DegradationStatus exist)</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/quota.rs` with:
      * `pub fn snapshot(env: &HygieneEnv) -> OpsResult<QuotaStatus>`:
        For each Category: read cached size from CF_MEJEPA_QUOTA_STATE entry "size_<slug>"
        OR fall back to `db.get_property_cf_int("rocksdb.estimate-num-keys")` + sum-of-value-sizes
        (cached at end of each GC pass per <note category="performance">). Compute degradation
        via `degradation_for`. Build CategoryStatus with tier_distribution (read from
        CF_MEJEPA_PANEL_META aggregated counts; empty for non-tiered categories).
        Assemble QuotaStatus; latency &lt; 50 ms p50.
      * `pub fn degradation_for(used: u64, budget: u64, unrecoverable: bool) -> DegradationStatus`:
        if unrecoverable: Unrecoverable
        else if used >= budget: OverBudget
        else if used * 100 >= budget * 95: OverBudget    (95-100% bucket merged with OverBudget per spec table)
        else if used * 100 >= budget * 80: Approaching
        else: Healthy
      * `pub fn score_for_eviction(uplift: f32, access_freq: f32, recency_seconds: u64) -> f32`:
        const SECONDS_PER_30_DAYS: f64 = 30.0 * 86400.0;
        let recency_factor = (-(recency_seconds as f64) / SECONDS_PER_30_DAYS).exp() as f32;
        uplift * access_freq * recency_factor
        Pure function; no I/O. Lowest score evicted first.
      * `pub fn read_unrecoverable_set(env: &HygieneEnv) -> OpsResult<HashSet<Category>>`:
        Read CF_MEJEPA_QUOTA_STATE; collect Categories with unrecoverable_since_ts: Some(_).
      * `pub fn write_unrecoverable(env: &HygieneEnv, category: Category, since_ts: u64,
        last_gc_outcome: &str) -> OpsResult<()>`: write to CF_MEJEPA_QUOTA_STATE.
    - DEFINE `pub const CF_MEJEPA_QUOTA_STATE: &str = "CF_MEJEPA_QUOTA_STATE";`
    - Unit tests:
      * degradation_for boundaries: 79% Healthy, 80% Approaching, 94% Approaching, 95% OverBudget,
        100% OverBudget, anything + unrecoverable=true → Unrecoverable.
      * score_for_eviction monotonic: doubling uplift doubles score; exp decay matches reference
        (recency=0 → factor 1.0; recency=30d → factor 1/e ≈ 0.368).
      * snapshot with empty CFs returns 9 Healthy CategoryStatus entries; total_disk_used_gb = 0.
  </in_scope>
  <out_of_scope>
    - Eviction itself (TASK-HYG-009 + TASK-HYG-014).
    - GC step 3 sweeping (TASK-HYG-014).
    - MCP shim (TASK-HYG-023).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/quota.rs">
    pub const CF_MEJEPA_QUOTA_STATE: &amp;str = "CF_MEJEPA_QUOTA_STATE";

    pub fn snapshot(env: &amp;crate::HygieneEnv) -&gt; crate::error::OpsResult&lt;crate::quota_types::QuotaStatus&gt;;
    pub fn degradation_for(used: u64, budget: u64, unrecoverable: bool) -&gt; crate::quota_types::DegradationStatus;
    pub fn score_for_eviction(uplift: f32, access_freq: f32, recency_seconds: u64) -&gt; f32;
    pub fn read_unrecoverable_set(env: &amp;crate::HygieneEnv) -&gt; crate::error::OpsResult&lt;std::collections::HashSet&lt;crate::categories::Category&gt;&gt;;
    pub fn write_unrecoverable(env: &amp;crate::HygieneEnv, category: crate::categories::Category, since_ts: u64, last_gc_outcome: &amp;str) -&gt; crate::error::OpsResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - degradation_for is pure (no I/O); thresholds are integer ratios (used*100 / budget) to avoid float drift.
    - score_for_eviction is deterministic given inputs.
    - snapshot has p50 &lt; 50 ms target; uses cached values, not full CF scan.
    - No unwrap_or on confidence; no silent fallback.
    - File ≤ 500 lines.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- degradation_for_thresholds` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- score_for_eviction_monotonic` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- snapshot_empty_returns_9_healthy` passes.
    - clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn degradation_for(used: u64, budget: u64, unrecoverable: bool) -&gt; DegradationStatus {
    if unrecoverable { return DegradationStatus::Unrecoverable; }
    if budget == 0 { return DegradationStatus::OverBudget; } // defensive
    if used &gt;= budget { return DegradationStatus::OverBudget; }
    let pct = (used as u128 * 100) / budget as u128;
    if pct &gt;= 95 { DegradationStatus::OverBudget }
    else if pct &gt;= 80 { DegradationStatus::Approaching }
    else { DegradationStatus::Healthy }
}

pub fn score_for_eviction(uplift: f32, access_freq: f32, recency_seconds: u64) -&gt; f32 {
    const SECONDS_PER_30_DAYS: f64 = 30.0 * 86400.0;
    let recency_factor = (-(recency_seconds as f64) / SECONDS_PER_30_DAYS).exp() as f32;
    uplift * access_freq * recency_factor
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/quota.rs">snapshot + degradation_for + score_for_eviction + CF state helpers</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/lib.rs">module already declared in TASK-HYG-001; this populates body</file>
  <file path="crates/context-graph-mejepa-hygiene/src/tests.rs">add quota math tests</file>
</files_to_modify>

<validation_criteria>
  <criterion>degradation_for: 79%→Healthy, 80%→Approaching, 94%→Approaching, 95%→OverBudget, 100%→OverBudget</criterion>
  <criterion>score_for_eviction(1,1,0) ≈ 1.0; score(1,1,30d) ≈ 1/e ≈ 0.368</criterion>
  <criterion>snapshot on empty DB returns 9 Healthy entries totaling 0 GB</criterion>
  <criterion>read_unrecoverable_set + write_unrecoverable round-trip</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- degradation_for_thresholds</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- score_for_eviction_monotonic</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- snapshot_empty_returns_9_healthy</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```
