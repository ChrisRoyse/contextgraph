# TASK-HYG-018 — gc_steps::step7_verify_integrity + atomic batch commit

```xml
<task_spec id="TASK-HYG-018" version="1.0">
<metadata>
  <title>gc_steps::step7_verify_integrity — verify_chain_integrity then atomic commit of staged steps 4+5 batch (or rollback on break)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>18</sequence>
  <implements>
    <requirement_ref>REQ-HYG-03</requirement_ref>
    <requirement_ref>BR-HYG-02</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-011</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Step 7: the integrity gate. Run verify_chain_integrity over the FULL chain (live + compressed
segments). On pass: commit the orchestrator's staged WriteBatch (containing steps 4 + 5
deletes); GC pass continues to step 8. On break: do NOT commit (drop the batch, RocksDB
rolls back automatically); set report.aborted=true + abort_reason="MEJEPA_HYG_GC_INTEGRITY_FAIL";
return OpsError::GcIntegrityFail. This is the structural fail-closed boundary per BR-HYG-02 +
NFR-HYG-04. Step 8 still runs (writes the abort report) before the orchestrator returns.
</context>

<input_context_files>
  <file purpose="step_spec">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="business_rule">../../functional/SPEC-HYGIENE.md#business_rules</file>
  <file purpose="verify_module">crates/context-graph-mejepa-hygiene/src/witness_compress.rs</file>
  <file purpose="error">crates/context-graph-mejepa-hygiene/src/error.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-011 shipped</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/gc_steps/step7_verify_integrity.rs`:
      * `pub fn run(env: &HygieneEnv, report: &mut GcReport, staged_batch: WriteBatch)
        -> OpsResult<()>`:
        1. Call `crate::witness_compress::verify_chain_integrity(env)`.
        2. If integrity.passed:
           - Commit staged_batch atomically: env.db.write(staged_batch).
           - Push GcStepReport { step_index: 7, step_name: "verify_integrity",
             entries_processed: integrity.entries_verified, bytes_freed: 0,
             errors_encountered: [], duration_ms: integrity.duration_ms }.
           - report.integrity_check_passed = true.
           - Ok(()).
        3. If !integrity.passed:
           - Drop staged_batch (RocksDB never sees these writes — rollback by drop).
           - Push GcStepReport with errors_encountered = ["MEJEPA_HYG_GC_INTEGRITY_FAIL"].
           - report.aborted = true;
             report.abort_reason = Some("MEJEPA_HYG_GC_INTEGRITY_FAIL".into()).
           - report.integrity_check_passed = false.
           - Build OpsError::GcIntegrityFail with break_offset + the prev_hashes from
             integrity.break_offset (need to re-read those entries to populate hex strings).
           - Return Err(opsError).
      * The signature takes `staged_batch` BY VALUE (consume + drop on rollback);
        on commit the batch is moved into env.db.write.
    - Unit / integration tests:
      * Healthy chain + non-empty staged batch → batch committed; entries deleted post-call;
        report.integrity_check_passed=true.
      * Inject break before step 7 → batch dropped; entries still present; report.aborted=true.
      * Verify both paths emit exactly one GcStepReport for step 7.
  </in_scope>
  <out_of_scope>
    - The verify body itself (TASK-HYG-011).
    - Step 8 (TASK-HYG-019).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/gc_steps/step7_verify_integrity.rs">
    pub fn run(
        env: &amp;crate::HygieneEnv,
        report: &amp;mut crate::reports::GcReport,
        staged_batch: rocksdb::WriteBatch,
    ) -&gt; crate::error::OpsResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - staged_batch consumed BY VALUE so rollback is automatic on early-return path
      (Rust drops it without commit).
    - GcIntegrityFail carries enough context (break_offset + both hashes) to point operator
      at the exact entry.
    - File ≤ 500 lines.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-hygiene -- step7_verify_passes_commits_batch` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- step7_verify_fails_drops_batch` passes
      (deletes are NOT applied; entries still readable).
    - clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn run(env: &amp;HygieneEnv, report: &amp;mut GcReport, staged_batch: WriteBatch) -&gt; OpsResult&lt;()&gt; {
    let integrity = crate::witness_compress::verify_chain_integrity(env)?;
    if integrity.passed {
        env.db.write(staged_batch).map_err(|e| OpsError::internal_with_source("commit", Box::new(e)))?;
        report.step_reports.push(GcStepReport { step_index: 7, step_name: "verify_integrity".into(),
            duration_ms: integrity.duration_ms, entries_processed: integrity.entries_verified,
            bytes_freed: 0, errors_encountered: vec![] });
        report.integrity_check_passed = true;
        return Ok(());
    }
    // FAIL — staged_batch is dropped here automatically (rollback)
    drop(staged_batch);
    report.step_reports.push(GcStepReport { step_index: 7, step_name: "verify_integrity".into(),
        duration_ms: integrity.duration_ms, entries_processed: integrity.entries_verified,
        bytes_freed: 0, errors_encountered: vec!["MEJEPA_HYG_GC_INTEGRITY_FAIL".into()] });
    report.integrity_check_passed = false;
    report.aborted = true;
    report.abort_reason = Some("MEJEPA_HYG_GC_INTEGRITY_FAIL".into());
    let break_offset = integrity.break_offset.unwrap_or(0);
    let (expected, actual) = read_break_hashes(env, break_offset).unwrap_or(([0u8;32], [0u8;32]));
    Err(OpsError { kind: OpsErrorKind::GcIntegrityFail {
        gc_run_id: report.gc_run_id, break_offset,
        expected_prev_hash_hex: hex::encode(expected),
        actual_prev_hash_hex: hex::encode(actual) } })
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/step7_verify_integrity.rs">verify + atomic commit/rollback</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/mod.rs">declare step7</file>
</files_to_modify>

<validation_criteria>
  <criterion>Healthy chain + staged batch → batch committed; entries deleted post-call</criterion>
  <criterion>Injected break → batch dropped; entries still present; report.aborted=true</criterion>
  <criterion>OpsError::GcIntegrityFail carries break_offset + both hashes</criterion>
  <criterion>Step 7 emits exactly one GcStepReport in both paths</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step7_verify_passes_commits_batch</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step7_verify_fails_drops_batch</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```
