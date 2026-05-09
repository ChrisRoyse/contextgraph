# TASK-HYG-012 — gc_steps::step1_demote::run

```xml
<task_spec id="TASK-HYG-012" version="1.0">
<metadata>
  <title>gc_steps::step1_demote — scan CF_MEJEPA_PANEL_META; demote entries whose decayed freq dropped below current tier's lower bound</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>12</sequence>
  <implements>
    <requirement_ref>REQ-HYG-03</requirement_ref>
    <requirement_ref>REQ-HYG-02</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-003</task_ref>
    <task_ref>TASK-HYG-007</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Step 1 of the 8-step nightly GC pass: iterate CF_MEJEPA_PANEL_META, compute decayed access
frequency vs current tier's threshold_lower; if below, call tier_demote on the entry. Builds a
GcStepReport with entries_processed + bytes_freed (deltas across tier transitions) +
errors_encountered (TierDecompressFail entries are logged but don't abort the step).
</context>

<input_context_files>
  <file purpose="step_spec">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="tier_ops">crates/context-graph-mejepa-hygiene/src/tier_ops.rs</file>
  <file purpose="reports">crates/context-graph-mejepa-hygiene/src/reports.rs</file>
  <file purpose="diagram">../../technical/TECH-HYGIENE.md#architecture_diagram</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-003 shipped (GcStepReport)</check>
  <check>TASK-HYG-007 shipped (tier_demote + TierLockMap)</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/gc_steps/mod.rs` with `pub mod step1_demote;` ... `pub mod step8_report;`
    - CREATE `src/gc_steps/step1_demote.rs`:
      * `pub fn run(env: &HygieneEnv, locks: &TierLockMap, report: &mut GcReport)
        -> OpsResult<()>`:
        1. Started timer.
        2. Iterate CF_MEJEPA_PANEL_META keys.
        3. For each entry: deserialize EntryAccessFrequency; compute decayed freq via
           `decayed_frequency(meta.frequency, meta.last_read_ts_ns, now)`.
        4. current_tier = tier_for_frequency(meta.frequency); new_tier = tier_for_frequency(decayed).
        5. If new_tier &lt; current_tier (colder): call tier_demote(env, locks, &entry_id);
           on TierDecompressFail: append code to errors_encountered; continue (do not abort).
        6. Accumulate entries_processed (count of items examined) + bytes_freed
           (sum of size_before - size_after over successful demotions).
        7. Build GcStepReport { step_index: 1, step_name: "demote", duration_ms,
           entries_processed, bytes_freed, errors_encountered } and push into report.step_reports.
    - Unit / integration test:
      * 100 synthetic entries with varying ages; after step1 run the entries whose
        freq dropped past their lower bound are demoted; tier_distribution shifts.
      * Inject one corrupt entry → errors_encountered contains MEJEPA_HYG_TIER_DECOMPRESS_FAIL;
        step does not abort.
  </in_scope>
  <out_of_scope>
    - Other 7 steps (TASK-HYG-013..019).
    - Orchestrator (TASK-HYG-020).
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/gc_steps/step1_demote.rs">
    pub fn run(
        env: &amp;crate::HygieneEnv,
        locks: &amp;crate::tier_ops::TierLockMap,
        report: &amp;mut crate::reports::GcReport,
    ) -&gt; crate::error::OpsResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - Step 1 NEVER aborts on individual entry failure (logs + continues per <note category="reliability">).
    - bytes_freed is computed from CF size_before/size_after of the entry blob (not panel meta).
    - File ≤ 500 lines.
    - clippy clean.
  </constraints>

  <verification>
    - `cargo build --release -p context-graph-mejepa-hygiene` succeeds.
    - `cargo test -p context-graph-mejepa-hygiene -- step1_demote_decay_demotes` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- step1_demote_corrupt_does_not_abort` passes.
    - clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn run(env: &amp;HygieneEnv, locks: &amp;TierLockMap, report: &amp;mut GcReport) -&gt; OpsResult&lt;()&gt; {
    let started = std::time::Instant::now();
    let mut entries_processed = 0u64;
    let mut bytes_freed = 0i64;
    let mut errors = Vec::new();
    let cf = env.db.cf_handle(CF_MEJEPA_PANEL_META)
        .ok_or_else(|| OpsError::internal("meta_cf_missing"))?;
    let now = env.now().timestamp_nanos_opt().unwrap_or(0) as u64;
    for kv in env.db.iterator_cf(&amp;cf, IteratorMode::Start) {
        let (key, raw) = kv.map_err(|e| OpsError::internal_with_source("iter", Box::new(e)))?;
        let meta: EntryAccessFrequency = match serde_json::from_slice(&amp;raw) { Ok(m) =&gt; m,
            Err(e) =&gt; { errors.push(format!("meta_parse:{}", e)); continue; } };
        entries_processed += 1;
        let cur = tier_for_frequency(meta.frequency);
        let decayed = match decayed_frequency(meta.frequency, meta.last_read_ts_ns, now) {
            Ok(v) =&gt; v, Err(e) =&gt; { errors.push(e.code().into()); continue; } };
        let new = tier_for_frequency(decayed);
        if new &lt; cur {
            let size_before = env.db.get_cf(&amp;meta.entry_id_resolve_cf(env)?, &amp;meta.entry_id.key)
                .unwrap_or(None).map(|b| b.len() as i64).unwrap_or(0);
            match tier_demote(env, locks, &amp;meta.entry_id) {
                Ok(()) =&gt; {
                    let size_after = env.db.get_cf(&amp;cf_handle, &amp;meta.entry_id.key)
                        .unwrap_or(None).map(|b| b.len() as i64).unwrap_or(0);
                    bytes_freed += size_before - size_after;
                }
                Err(e) =&gt; { errors.push(e.code().into()); }
            }
        }
    }
    report.step_reports.push(GcStepReport {
        step_index: 1, step_name: "demote".into(),
        duration_ms: started.elapsed().as_millis() as u64,
        entries_processed, bytes_freed, errors_encountered: errors });
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/mod.rs">module declarations for 8 steps</file>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/step1_demote.rs">demote sweep</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/lib.rs">module already declared in TASK-HYG-001; populate via gc_steps/mod.rs</file>
</files_to_modify>

<validation_criteria>
  <criterion>100 synthetic entries: those whose decayed freq drops below current threshold are demoted</criterion>
  <criterion>tier_distribution shifts after step1 run</criterion>
  <criterion>Corrupt entry → error logged in step report; step does not abort</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step1_demote_decay_demotes</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step1_demote_corrupt_does_not_abort</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```
