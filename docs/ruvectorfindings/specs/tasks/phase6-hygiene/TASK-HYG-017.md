# TASK-HYG-017 — gc_steps::step6_compress_witness wrapper

```xml
<task_spec id="TASK-HYG-017" version="1.0">
<metadata>
  <title>gc_steps::step6_compress_witness — wrap witness_compress::compress_segments(threshold=env.witness_threshold_days)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>17</sequence>
  <implements>
    <requirement_ref>REQ-HYG-04</requirement_ref>
    <requirement_ref>REQ-HYG-03</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-010</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Step 6: thin wrapper that invokes witness_compress::compress_segments with the configured
threshold (env.witness_threshold_days, default 90). Merges the returned CompressReport's
counts into the GcStepReport. EC-HYG-04 disk-full propagates as a non-aborting error code in
errors_encountered (the live chain remains unchanged on rollback, so step 7 integrity verify
will still pass).
</context>

<input_context_files>
  <file purpose="step_spec">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="compress_module">crates/context-graph-mejepa-hygiene/src/witness_compress.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-010 shipped</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/gc_steps/step6_compress_witness.rs`:
      * `pub fn run(env: &HygieneEnv, report: &mut GcReport) -> OpsResult<()>`:
        1. threshold = chrono::Duration::days(env.witness_threshold_days as i64).
        2. Call `crate::witness_compress::compress_segments(env, threshold)`.
        3. On success: merge CompressReport into GcStepReport
           (entries_processed = entries_compressed; bytes_freed = bytes_before - bytes_after).
        4. On QuotaExceeded: append code to errors_encountered; do NOT abort
           (live chain unchanged; step 7 will still pass).
        5. On MerkleVerifyFail (post-compression integrity check): append code +
           propagate the error so the orchestrator sees it.
        6. Push GcStepReport.
    - Unit / integration tests:
      * Healthy 2048-entry chain → 2 segments compressed; bytes_freed > 0.
      * Disk-full simulation → step does not abort; QuotaExceeded in errors_encountered.
      * Empty witness chain → step succeeds with zero counts.
  </in_scope>
  <out_of_scope>
    - Compression body (TASK-HYG-010).
    - Verification (TASK-HYG-011 / step 7).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/gc_steps/step6_compress_witness.rs">
    pub fn run(env: &amp;crate::HygieneEnv, report: &amp;mut crate::reports::GcReport) -&gt; crate::error::OpsResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - Disk-full / partial-segment / threshold-too-small are all non-aborting (errors_encountered).
    - MerkleVerifyFail at post-compression check IS aborting (returns OpsError to orchestrator).
    - File ≤ 500 lines.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-hygiene -- step6_compress_witness_2048` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- step6_compress_witness_empty_chain` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- step6_compress_witness_disk_full` passes.
    - clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn run(env: &amp;HygieneEnv, report: &amp;mut GcReport) -&gt; OpsResult&lt;()&gt; {
    let started = std::time::Instant::now();
    let mut errs = Vec::new();
    let threshold = chrono::Duration::days(env.witness_threshold_days as i64);
    let r = match crate::witness_compress::compress_segments(env, threshold) {
        Ok(r) =&gt; r,
        Err(e) =&gt; match e.kind {
            OpsErrorKind::QuotaExceeded { .. } =&gt; {
                errs.push(e.code().into());
                report.step_reports.push(GcStepReport { step_index: 6,
                    step_name: "compress_witness".into(),
                    duration_ms: started.elapsed().as_millis() as u64,
                    entries_processed: 0, bytes_freed: 0, errors_encountered: errs });
                return Ok(());
            }
            _ =&gt; return Err(e),
        },
    };
    report.step_reports.push(GcStepReport { step_index: 6, step_name: "compress_witness".into(),
        duration_ms: started.elapsed().as_millis() as u64,
        entries_processed: r.entries_compressed,
        bytes_freed: (r.bytes_before as i64) - (r.bytes_after as i64),
        errors_encountered: errs });
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/step6_compress_witness.rs">wrapper around compress_segments</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/mod.rs">declare step6</file>
</files_to_modify>

<validation_criteria>
  <criterion>2048-entry chain compressed → 2 segments; bytes_freed positive</criterion>
  <criterion>Disk-full → QuotaExceeded in errors_encountered, step does not abort</criterion>
  <criterion>Empty chain → zero counts; step succeeds</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step6_compress_witness</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```
