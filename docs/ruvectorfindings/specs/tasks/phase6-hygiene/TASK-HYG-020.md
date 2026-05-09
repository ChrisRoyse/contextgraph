# TASK-HYG-020 — gc::gc_run_nightly orchestrator + flock + Phase 5 mid-promotion guard

```xml
<task_spec id="TASK-HYG-020" version="1.0">
<metadata>
  <title>gc::gc_run_nightly orchestrator — global flock + Phase 5 retrain mid-promotion guard + sequenced 8-step pipeline + atomic batch staging across steps 4+5+7</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>20</sequence>
  <implements>
    <requirement_ref>REQ-HYG-03</requirement_ref>
    <requirement_ref>BR-HYG-07</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-012</task_ref>
    <task_ref>TASK-HYG-013</task_ref>
    <task_ref>TASK-HYG-014</task_ref>
    <task_ref>TASK-HYG-015</task_ref>
    <task_ref>TASK-HYG-016</task_ref>
    <task_ref>TASK-HYG-017</task_ref>
    <task_ref>TASK-HYG-018</task_ref>
    <task_ref>TASK-HYG-019</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Top-level orchestrator: acquires `~/.cache/cgreality/mejepa-gc.lock` flock; checks Phase 5
retrain mid-promotion (deferral path per BR-HYG-07); executes 8 steps in order; manages the
shared WriteBatch staging across steps 4+5+7; on step 7 abort, runs step 8 anyway to write the
abort report; releases flock.
</context>

<input_context_files>
  <file purpose="orchestrator_spec">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="diagram">../../technical/TECH-HYGIENE.md#architecture_diagram</file>
  <file purpose="business_rule">../../functional/SPEC-HYGIENE.md#business_rules</file>
  <file purpose="all_steps">crates/context-graph-mejepa-hygiene/src/gc_steps/</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-012..019 shipped</check>
  <check>fs2 dep added (flock)</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/gc.rs`:
      * `pub fn gc_run_nightly(env: &HygieneEnv) -> OpsResult<GcReport>`:
        1. Build initial GcReport: gc_run_id = uuid::v4(), started_at = env.now(),
           total_disk_used_gb_before = compute_total_used_gb, deferred=false, aborted=false,
           step_reports=[], integrity_check_passed=false.
        2. Acquire flock at `~/.cache/cgreality/mejepa-gc.lock`. If contended:
           return OpsError::Internal { reason: "gc_already_running" }.
        3. Check Phase 5 retrain mid-promotion:
           - If env.self_healing_handle.is_some()
             AND env.self_healing_handle.as_ref().unwrap().is_retrain_in_progress():
             - Set report.deferred=true; deferred_reason="retrain_mid_promotion".
             - Run step 8 to write report; return Ok(report).
           - Else: continue.
        4. Build TierLockMap.
        5. Run step1_demote(env, &locks, &mut report).
        6. Run step2_compact(env, &mut report).
        7. Run step3_evict(env, &mut report).
        8. Build staged_batch: WriteBatch::default().
        9. Run step4_prune_checkpoints(env, &mut report, &mut staged_batch).
        10. Run step5_prune_calibration(env, &mut report, &mut staged_batch).
        11. Run step6_compress_witness(env, &mut report).
        12. Run step7_verify_integrity(env, &mut report, staged_batch) — consumes batch by value.
            On Err(OpsError::GcIntegrityFail): step8 still runs, then return Err.
        13. Run step8_report(env, &mut report).
        14. Release flock (drop guard).
        15. Return Ok(report).
    - Lock file directory created if missing (`mkdir -p ~/.cache/cgreality`).
    - On flock contention: short retry once (50ms), then fail-closed — better behavior than
      blocking forever.
    - Unit / integration tests:
      * Healthy run: 8 steps execute; report.completed_at set; integrity_check_passed=true.
      * Phase 5 deferred: only step 8 runs; deferred=true.
      * Concurrent gc_run_nightly: second invocation returns Internal "gc_already_running".
      * Step 7 break: step 8 still writes file; return Err(GcIntegrityFail).
  </in_scope>
  <out_of_scope>
    - cron / systemd integration (operator runbook).
    - tokio_cron_scheduler wiring (out of scope for v1.0; manual invocation via MCP).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/gc.rs">
    pub fn gc_run_nightly(env: &amp;crate::HygieneEnv) -&gt; crate::error::OpsResult&lt;crate::reports::GcReport&gt;;
    </signature>
  </signatures>

  <constraints>
    - Global GC flock at `~/.cache/cgreality/mejepa-gc.lock` per <note category="locking">.
    - Step 8 ALWAYS runs (even on abort) per <note category="reliability">.
    - Phase 5 retrain check is non-aborting — just sets deferred + writes report + returns.
    - File ≤ 500 lines.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-hygiene -- gc_run_nightly_healthy` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- gc_run_nightly_phase5_deferred` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- gc_run_nightly_concurrent_blocks` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- gc_run_nightly_step7_abort_writes_report` passes.
    - clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn gc_run_nightly(env: &amp;HygieneEnv) -&gt; OpsResult&lt;GcReport&gt; {
    let mut report = GcReport {
        gc_run_id: Uuid::new_v4(), started_at: env.now(), completed_at: None,
        deferred: false, deferred_reason: None, aborted: false, abort_reason: None,
        step_reports: vec![],
        total_disk_used_gb_before: compute_total_used_gb(env)?,
        total_disk_used_gb_after: 0.0, integrity_check_passed: false };
    let lock_path = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("cgreality/mejepa-gc.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap())?;
    let lock_file = std::fs::OpenOptions::new().read(true).write(true).create(true).open(&amp;lock_path)?;
    if fs2::FileExt::try_lock_exclusive(&amp;lock_file).is_err() {
        std::thread::sleep(std::time::Duration::from_millis(50));
        if fs2::FileExt::try_lock_exclusive(&amp;lock_file).is_err() {
            return Err(OpsError::internal("gc_already_running"));
        }
    }

    if let Some(h) = &amp;env.self_healing_handle {
        if h.is_retrain_in_progress() {
            report.deferred = true;
            report.deferred_reason = Some("retrain_mid_promotion".into());
            crate::gc_steps::step8_report::run(env, &amp;mut report)?;
            return Ok(report);
        }
    }

    let locks = crate::tier_ops::TierLockMap::new();
    crate::gc_steps::step1_demote::run(env, &amp;locks, &amp;mut report)?;
    crate::gc_steps::step2_compact::run(env, &amp;mut report)?;
    crate::gc_steps::step3_evict::run(env, &amp;mut report)?;
    let mut staged = WriteBatch::default();
    crate::gc_steps::step4_prune_checkpoints::run(env, &amp;mut report, &amp;mut staged)?;
    crate::gc_steps::step5_prune_calibration::run(env, &amp;mut report, &amp;mut staged)?;
    crate::gc_steps::step6_compress_witness::run(env, &amp;mut report)?;
    let step7_result = crate::gc_steps::step7_verify_integrity::run(env, &amp;mut report, staged);
    crate::gc_steps::step8_report::run(env, &amp;mut report)?;
    step7_result?;
    Ok(report)
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/gc.rs">gc_run_nightly orchestrator</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/lib.rs">add `pub mod gc;` (already declared in TASK-HYG-001)</file>
  <file path="crates/context-graph-mejepa-hygiene/Cargo.toml">add fs2 + dirs dep if not present</file>
</files_to_modify>

<validation_criteria>
  <criterion>Healthy run: 8 step_reports + completed_at set + integrity_check_passed=true</criterion>
  <criterion>Phase 5 retrain deferred: only step 8 runs; deferred=true</criterion>
  <criterion>Concurrent invocation: second returns Internal "gc_already_running"</criterion>
  <criterion>Step 7 break: step 8 still writes file; return Err(GcIntegrityFail)</criterion>
  <criterion>Lock released cleanly on all paths</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- gc_run_nightly_healthy</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- gc_run_nightly_phase5_deferred</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- gc_run_nightly_concurrent_blocks</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- gc_run_nightly_step7_abort_writes_report</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```
