# TASK-HYG-015 — gc_steps::step4_prune_checkpoints + tagged-release shield

```xml
<task_spec id="TASK-HYG-015" version="1.0">
<metadata>
  <title>gc_steps::step4_prune_checkpoints — keep last 10 + tagged_release; refuse tagged with MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>15</sequence>
  <implements>
    <requirement_ref>REQ-HYG-09</requirement_ref>
    <requirement_ref>REQ-HYG-03</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Step 4: list checkpoints in CF_MEJEPA_WEIGHT_BLOBS metadata; sort by created_at desc; keep
newest 10 + all tagged_release=true. Stage deletes in a write_batch (committed only after
step 7 integrity verification per NFR-HYG-04 atomic). Any tagged_release that races into the
prune set is RESPECTED — log WARN with code MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED; never abort
the step (per BR-HYG-05).
</context>

<input_context_files>
  <file purpose="step_spec">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="business_rule">../../functional/SPEC-HYGIENE.md#business_rules</file>
  <file purpose="error_state">../../functional/SPEC-HYGIENE.md#error_states</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-003 shipped</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/gc_steps/step4_prune_checkpoints.rs`:
      * `pub struct CheckpointMeta { pub theta_sha: [u8;32], pub created_at_ts: u64, pub tagged_release: bool, pub tagged_at_ts: Option<u64>, pub size_bytes: u64 }`
      * `pub fn run(env: &HygieneEnv, report: &mut GcReport, staged_batch: &mut WriteBatch)
        -> OpsResult<()>`:
        1. List all checkpoints from CF_MEJEPA_WEIGHT_BLOBS_META; deserialize CheckpointMeta.
        2. Sort by created_at_ts descending.
        3. Determine prune set: skip newest 10; skip all tagged_release=true.
        4. For each prune candidate:
           - If tagged_release==true (defensive double-check):
             append "MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED" to errors_encountered;
             tracing::warn!(target:"hygiene", checkpoint_id, theta_sha, tagged_at_ts);
             continue (do not stage delete).
           - Else: stage delete in `staged_batch` for both blob CF and meta CF.
             Sum size_bytes into bytes_freed.
        5. Build GcStepReport; push.
    - The `staged_batch` is owned by the orchestrator (TASK-HYG-020); step 4 + step 5
      append to it; step 7 commits or drops it.
    - Unit / integration tests:
      * 20 checkpoints (5 tagged, 15 untagged): 10 newest + 5 tagged kept (15); 5 oldest untagged staged.
      * If a tagged_release somehow appears in the prune set (test injection): error logged; step continues.
      * 0 checkpoints: bytes_freed=0; entries_processed=0.
  </in_scope>
  <out_of_scope>
    - Tagging API (operator runbook out of scope per spec).
    - Step 7 commit (TASK-HYG-018).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/gc_steps/step4_prune_checkpoints.rs">
    pub const CF_MEJEPA_WEIGHT_BLOBS: &amp;str = "CF_MEJEPA_WEIGHT_BLOBS";
    pub const CF_MEJEPA_WEIGHT_BLOBS_META: &amp;str = "CF_MEJEPA_WEIGHT_BLOBS_META";

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct CheckpointMeta {
        pub theta_sha: [u8; 32],
        pub created_at_ts: u64,
        pub tagged_release: bool,
        pub tagged_at_ts: Option&lt;u64&gt;,
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
    - tagged_release is checked twice: once in candidate filter, once defensively per-entry
      (race-safe per BR-HYG-05).
    - Refused tagged prune does NOT abort the step; just logs + appends to errors_encountered.
    - Stages deletes in caller's WriteBatch; does not commit on its own.
    - File ≤ 500 lines.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- step4_prune_keeps_10_plus_tagged` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- step4_prune_refuses_tagged_continues` passes.
    - clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn run(env: &amp;HygieneEnv, report: &amp;mut GcReport, batch: &amp;mut WriteBatch) -&gt; OpsResult&lt;()&gt; {
    let started = std::time::Instant::now();
    let cf_meta = env.db.cf_handle(CF_MEJEPA_WEIGHT_BLOBS_META)
        .ok_or_else(|| OpsError::internal("ckpt_meta_cf_missing"))?;
    let cf_blob = env.db.cf_handle(CF_MEJEPA_WEIGHT_BLOBS)
        .ok_or_else(|| OpsError::internal("ckpt_blob_cf_missing"))?;
    let mut all = read_all_checkpoints(env, &amp;cf_meta)?;
    all.sort_by_key(|m| std::cmp::Reverse(m.created_at_ts));
    let mut bytes_freed = 0i64;
    let mut errs = Vec::new();
    let mut entries_processed = 0u64;
    for (i, ckpt) in all.iter().enumerate() {
        if i &lt; 10 || ckpt.tagged_release { continue; }
        // Defensive double-check (race-safe)
        if ckpt.tagged_release {
            errs.push("MEJEPA_HYG_CHECKPOINT_PRUNE_REFUSED".into());
            tracing::warn!(target:"hygiene", theta_sha=?hex::encode(ckpt.theta_sha),
                tagged_at_ts=?ckpt.tagged_at_ts, "refused tagged-release prune");
            continue;
        }
        batch.delete_cf(&amp;cf_blob, &amp;ckpt.theta_sha);
        batch.delete_cf(&amp;cf_meta, &amp;ckpt.theta_sha);
        bytes_freed += ckpt.size_bytes as i64;
        entries_processed += 1;
    }
    report.step_reports.push(GcStepReport { step_index: 4, step_name: "prune_checkpoints".into(),
        duration_ms: started.elapsed().as_millis() as u64,
        entries_processed, bytes_freed, errors_encountered: errs });
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/step4_prune_checkpoints.rs">checkpoint pruning + tagged-release shield</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/mod.rs">declare step4</file>
</files_to_modify>

<validation_criteria>
  <criterion>20 ckpts (5 tagged, 15 untagged): 10 newest + 5 tagged kept</criterion>
  <criterion>Tagged-release double-check: refused logs warn + step continues</criterion>
  <criterion>Empty checkpoint list: zero counts; no error</criterion>
  <criterion>Deletes are STAGED in batch (not yet committed)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step4_prune_keeps_10_plus_tagged</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step4_prune_refuses_tagged_continues</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```
