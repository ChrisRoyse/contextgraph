# TASK-HYG-016 — gc_steps::step5_prune_calibration

```xml
<task_spec id="TASK-HYG-016" version="1.0">
<metadata>
  <title>gc_steps::step5_prune_calibration — keep last 30 calibration snapshots; stage deletes in shared batch</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>16</sequence>
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
Step 5: list calibration snapshots in CF_MEJEPA_CALIBRATION_HISTORY; sort by frozen_at desc;
keep newest 30; stage deletes for the rest in the orchestrator's shared WriteBatch (committed
only on step 7 integrity success). Simpler than step 4 — no tagged-release shield since
calibration snapshots are not gold-tier.
</context>

<input_context_files>
  <file purpose="step_spec">../../technical/TECH-HYGIENE.md#component_contracts</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-003 shipped</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/gc_steps/step5_prune_calibration.rs`:
      * `pub const CF_MEJEPA_CALIBRATION_HISTORY: &str = "CF_MEJEPA_CALIBRATION_HISTORY";`
      * `pub struct CalibrationSnapshotMeta { pub snapshot_id: uuid::Uuid, pub frozen_at_ts: u64, pub size_bytes: u64 }`
      * `pub fn run(env: &HygieneEnv, report: &mut GcReport, staged_batch: &mut WriteBatch)
        -> OpsResult<()>`:
        1. Iterate CF_MEJEPA_CALIBRATION_HISTORY; deserialize CalibrationSnapshotMeta entries.
        2. Sort by frozen_at_ts desc.
        3. Skip newest 30; stage deletes for the rest (and any sibling data CF).
        4. Build GcStepReport.
    - Unit tests:
      * 50 snapshots: 30 newest kept; 20 staged for delete.
      * 0 snapshots: zero counts.
      * 25 snapshots: all kept.
  </in_scope>
  <out_of_scope>
    - Calibration snapshot construction (Phase 4 SPEC-INFERENCE).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/gc_steps/step5_prune_calibration.rs">
    pub const CF_MEJEPA_CALIBRATION_HISTORY: &amp;str = "CF_MEJEPA_CALIBRATION_HISTORY";

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct CalibrationSnapshotMeta {
        pub snapshot_id: uuid::Uuid,
        pub frozen_at_ts: u64,
        pub size_bytes: u64,
    }

    pub fn run(
        env: &amp;crate::HygieneEnv,
        report: &amp;mut crate::reports::GcReport,
        staged_batch: &amp;mut rocksdb::WriteBatch,
    ) -&gt; crate::error::OpsResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - Stages in caller's batch; does not commit.
    - File ≤ 500 lines.
    - clippy clean.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-hygiene -- step5_prune_keeps_last_30` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- step5_prune_under_30_keeps_all` passes.
    - clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn run(env: &amp;HygieneEnv, report: &amp;mut GcReport, batch: &amp;mut WriteBatch) -&gt; OpsResult&lt;()&gt; {
    let started = std::time::Instant::now();
    let cf = env.db.cf_handle(CF_MEJEPA_CALIBRATION_HISTORY)
        .ok_or_else(|| OpsError::internal("calib_cf_missing"))?;
    let mut all = read_all_calibration_snapshots(env, &amp;cf)?;
    all.sort_by_key(|m| std::cmp::Reverse(m.frozen_at_ts));
    let mut bytes_freed = 0i64;
    let mut entries_processed = 0u64;
    for snap in all.iter().skip(30) {
        batch.delete_cf(&amp;cf, snap.snapshot_id.as_bytes());
        bytes_freed += snap.size_bytes as i64;
        entries_processed += 1;
    }
    report.step_reports.push(GcStepReport { step_index: 5, step_name: "prune_calibration".into(),
        duration_ms: started.elapsed().as_millis() as u64,
        entries_processed, bytes_freed, errors_encountered: vec![] });
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/step5_prune_calibration.rs">prune to last 30</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/mod.rs">declare step5</file>
</files_to_modify>

<validation_criteria>
  <criterion>50 snapshots: 30 kept, 20 staged for delete</criterion>
  <criterion>25 snapshots: all kept</criterion>
  <criterion>Empty CF: zero counts</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step5_prune_keeps_last_30</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step5_prune_under_30_keeps_all</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```
