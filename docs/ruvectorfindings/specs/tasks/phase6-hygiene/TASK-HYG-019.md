# TASK-HYG-019 — gc_steps::step8_report writer (0o600 mode; pretty JSON)

```xml
<task_spec id="TASK-HYG-019" version="1.0">
<metadata>
  <title>gc_steps::step8_report — write GcReport to <runtime_root>/mejepa/gc-report-<YYYY-MM-DD>.json with mode 0o600 + pretty JSON</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>19</sequence>
  <implements>
    <requirement_ref>REQ-HYG-03</requirement_ref>
    <requirement_ref>NFR-HYG-06</requirement_ref>
    <requirement_ref>NFR-HYG-07</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Step 8: serialize GcReport to pretty JSON (2-space indent per NFR-HYG-07) and write to
`<runtime_root>/mejepa/gc-report-<YYYY-MM-DD>.json` with file mode 0o600 per SEC-MEJEPA-07
+ NFR-HYG-06. Always runs (even when GC is aborted at step 7) so the operator gets a record
of what happened. Also writes a copy into CF_MEJEPA_GC_HISTORY (last 365 entries; older
self-pruned by step 5-equivalent in future passes).
</context>

<input_context_files>
  <file purpose="step_spec">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="security">../../constitution.md#security_requirements</file>
  <file purpose="reports">crates/context-graph-mejepa-hygiene/src/reports.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-003 shipped</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/gc_steps/step8_report.rs`:
      * `pub const CF_MEJEPA_GC_HISTORY: &str = "CF_MEJEPA_GC_HISTORY";`
      * `pub fn run(env: &HygieneEnv, report: &mut GcReport) -> OpsResult<()>`:
        1. Set report.completed_at = Some(env.now()).
        2. Compute total_disk_used_gb_after via per-CF size sum (re-snapshot).
        3. Validate report (calls report.validate()).
        4. Serialize to pretty JSON (serde_json::to_string_pretty with 2-space indent).
        5. Build path: env.runtime_root.join("mejepa")
           .join(format!("gc-report-{}.json", report.started_at.date_naive())).
        6. Create parent dirs if missing.
        7. On Unix: open with `OpenOptions::new().mode(0o600).create(true).write(true)
           .truncate(true)`; on Windows: best-effort create + warn that 0o600 is Unix-only.
        8. Write JSON; fsync; close.
        9. Persist serialized GcReport bytes into CF_MEJEPA_GC_HISTORY keyed by gc_run_id bytes.
        10. Push GcStepReport { step_index: 8, step_name: "report",
            entries_processed: 1, bytes_freed: 0, ... }.
    - Unit / integration tests:
      * Run step 8 on synthetic GcReport; verify file exists at expected path with mode 0o600;
        re-read JSON parses back to identical GcReport.
      * Run step 8 on aborted GcReport (aborted=true, abort_reason set) → still writes file.
      * stat -c '%a' on file == 600.
      * CF_MEJEPA_GC_HISTORY contains the entry under gc_run_id key.
  </in_scope>
  <out_of_scope>
    - GC history pruning (would be a step-5-equivalent on CF_MEJEPA_GC_HISTORY itself; out of v1.0).
    - Operator UI to read reports.
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/gc_steps/step8_report.rs">
    pub const CF_MEJEPA_GC_HISTORY: &amp;str = "CF_MEJEPA_GC_HISTORY";

    pub fn run(env: &amp;crate::HygieneEnv, report: &amp;mut crate::reports::GcReport) -&gt; crate::error::OpsResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - File mode 0o600 on Unix (NFR-HYG-06 + SEC-MEJEPA-07).
    - JSON pretty-printed 2-space indent (NFR-HYG-07).
    - Always runs (idempotent — even on abort).
    - validate() called BEFORE serialize so corrupt reports surface early.
    - File ≤ 500 lines.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-hygiene -- step8_report_writes_0o600` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- step8_report_aborted_still_writes` passes.
    - `stat -c '%a' <test report>` returns 600.
    - JSON round-trip identical.
    - clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn run(env: &amp;HygieneEnv, report: &amp;mut GcReport) -&gt; OpsResult&lt;()&gt; {
    let started = std::time::Instant::now();
    report.completed_at = Some(env.now());
    report.total_disk_used_gb_after = compute_total_used_gb(env)?;
    report.validate()?;
    let json = serde_json::to_string_pretty(report)
        .map_err(|e| OpsError::internal_with_source("serialize", Box::new(e)))?;
    let date = report.started_at.date_naive().to_string();
    let dir = env.runtime_root.join("mejepa");
    std::fs::create_dir_all(&amp;dir)?;
    let path = dir.join(format!("gc-report-{}.json", date));
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new().mode(0o600).create(true).write(true).truncate(true).open(&amp;path)?;
        std::io::Write::write_all(&amp;mut f, json.as_bytes())?;
        f.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        tracing::warn!(target:"hygiene", "0o600 unsupported on this platform; writing best-effort");
        std::fs::write(&amp;path, &amp;json)?;
    }
    let cf = env.db.cf_handle(CF_MEJEPA_GC_HISTORY).ok_or_else(|| OpsError::internal("gc_hist_cf_missing"))?;
    env.db.put_cf(&amp;cf, report.gc_run_id.as_bytes(), json.as_bytes())?;
    report.step_reports.push(GcStepReport { step_index: 8, step_name: "report".into(),
        duration_ms: started.elapsed().as_millis() as u64,
        entries_processed: 1, bytes_freed: 0, errors_encountered: vec![] });
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/step8_report.rs">report writer 0o600</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/mod.rs">declare step8</file>
</files_to_modify>

<validation_criteria>
  <criterion>File written at expected path with mode 0o600</criterion>
  <criterion>Pretty JSON round-trip preserves GcReport</criterion>
  <criterion>Aborted reports still write</criterion>
  <criterion>CF_MEJEPA_GC_HISTORY contains entry under gc_run_id</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step8_report_writes_0o600</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step8_report_aborted_still_writes</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```
