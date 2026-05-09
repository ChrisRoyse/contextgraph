# TASK-HYG-013 — gc_steps::step2_compact (parallel compact_range_cf per ME-JEPA CF)

```xml
<task_spec id="TASK-HYG-013" version="1.0">
<metadata>
  <title>gc_steps::step2_compact — parallel compact_range_cf per ME-JEPA CF; reclaim freed bytes</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>13</sequence>
  <implements>
    <requirement_ref>REQ-HYG-03</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Step 2: trigger RocksDB compaction on every ME-JEPA CF in parallel via tokio::task::spawn_blocking
(RocksDB compactions are thread-safe across distinct CFs, per <note category="performance">).
This is the single biggest contributor to NFR-HYG-01 (60-min p95) on 100 GB-scale CFs. Computes
bytes_freed as `(size_before - size_after).max(0)` summed across CFs.
</context>

<input_context_files>
  <file purpose="step_spec">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="performance_note">../../technical/TECH-HYGIENE.md#implementation_notes</file>
  <file purpose="reports">crates/context-graph-mejepa-hygiene/src/reports.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-003 shipped</check>
  <check>tokio in workspace deps</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/gc_steps/step2_compact.rs`:
      * `pub fn run(env: &HygieneEnv, report: &mut GcReport) -> OpsResult<()>`:
        1. List of ME-JEPA CFs from env (CF_MEJEPA_* prefix); cached in HygieneEnv as
           `mejepa_cfs(&self) -> Vec<String>`.
        2. For each CF: capture size_before via `db.get_property_cf_int("rocksdb.total-sst-files-size")`.
        3. Spawn parallel tokio blocking tasks: `db.compact_range_cf(&cf, None, None)`.
        4. After all complete: capture size_after; bytes_freed_per_cf = max(0, size_before - size_after).
        5. Sum into GcStepReport; push to report.step_reports.
      * Add `HygieneEnv::mejepa_cfs(&self) -> Vec<String>` returning the registered ME-JEPA
        CFs (CF_MEJEPA_PANELS, CF_MEJEPA_PANEL_META, CF_MEJEPA_WEIGHT_BLOBS,
        CF_MEJEPA_TRAIN_CERTS, CF_MEJEPA_ORACLE_VERDICTS, CF_MEJEPA_MUTATION_CORPUS,
        CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_SHIFT_WATERMARK, CF_MEJEPA_QUOTA_STATE,
        CF_MEJEPA_WITNESS_SEGMENT_META, CF_MEJEPA_GC_HISTORY).
    - Unit / integration test:
      * Synthetic CFs with delete tombstones; step2 reduces total-sst-files-size.
      * Parallel compaction completes for 11 CFs; total wall-clock < sum of individual durations.
  </in_scope>
  <out_of_scope>
    - CF schema definitions (owned by other phases / tasks).
    - Eviction (TASK-HYG-014).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/gc_steps/step2_compact.rs">
    pub fn run(env: &amp;crate::HygieneEnv, report: &amp;mut crate::reports::GcReport) -&gt; crate::error::OpsResult&lt;()&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-hygiene/src/lib.rs">
    impl HygieneEnv { pub fn mejepa_cfs(&amp;self) -&gt; Vec&lt;String&gt;; }
    </signature>
  </signatures>

  <constraints>
    - Parallel compactions via tokio::task::spawn_blocking (NOT cargo-bench-style threads).
    - Must work in non-async caller via tokio::runtime::Runtime::block_on at boundary.
    - bytes_freed clamped to ≥ 0 (RocksDB can occasionally report increase due to L0 churn).
    - Step does not abort on individual CF compaction failure (logs + continues).
    - File ≤ 500 lines.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- step2_compact_reduces_size` passes.
    - clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn run(env: &amp;HygieneEnv, report: &amp;mut GcReport) -&gt; OpsResult&lt;()&gt; {
    let started = std::time::Instant::now();
    let cfs = env.mejepa_cfs();
    let mut total_freed = 0i64;
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build()
        .map_err(|e| OpsError::internal_with_source("rt", Box::new(e)))?;
    let results: Vec&lt;_&gt; = rt.block_on(async {
        let mut handles = Vec::new();
        for cf_name in &amp;cfs {
            let db = Arc::clone(&amp;env.db);
            let name = cf_name.clone();
            handles.push(tokio::task::spawn_blocking(move || {
                let cf = db.cf_handle(&amp;name).ok_or("cf_missing")?;
                let before = db.property_int_value_cf(&amp;cf, "rocksdb.total-sst-files-size")
                    .ok().flatten().unwrap_or(0);
                db.compact_range_cf(&amp;cf, None::&lt;&amp;[u8]&gt;, None::&lt;&amp;[u8]&gt;);
                let after = db.property_int_value_cf(&amp;cf, "rocksdb.total-sst-files-size")
                    .ok().flatten().unwrap_or(0);
                Ok::&lt;_, String&gt;((name, before, after))
            }));
        }
        let mut out = Vec::new();
        for h in handles { out.push(h.await); }
        out
    });
    let mut errs = Vec::new();
    for r in results {
        match r { Ok(Ok((_, before, after))) =&gt; total_freed += (before - after).max(0) as i64,
                  Ok(Err(e)) =&gt; errs.push(e),
                  Err(e) =&gt; errs.push(e.to_string()) };
    }
    report.step_reports.push(GcStepReport { step_index: 2, step_name: "compact".into(),
        duration_ms: started.elapsed().as_millis() as u64,
        entries_processed: cfs.len() as u64, bytes_freed: total_freed,
        errors_encountered: errs });
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/step2_compact.rs">parallel compact_range_cf</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/lib.rs">add HygieneEnv::mejepa_cfs() helper</file>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/mod.rs">declare step2_compact</file>
</files_to_modify>

<validation_criteria>
  <criterion>11 ME-JEPA CFs compacted in parallel</criterion>
  <criterion>bytes_freed reflects total-sst-files-size reduction (≥ 0)</criterion>
  <criterion>Per-CF failure logged but does not abort the step</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step2_compact_reduces_size</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```
