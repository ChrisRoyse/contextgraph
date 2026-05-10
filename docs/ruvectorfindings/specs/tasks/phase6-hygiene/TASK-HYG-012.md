# TASK-HYG-012 — gc_steps::step1_demote::run

## 🔍 AI-Agent Implementation Mandate (read FIRST — fresh-context onboarding)

> **You are an AI coding assistant inheriting this task with no prior context. This block gives you everything you need.**

### 1. Project state pointers — read these before touching code

- **Live shipping status table:** `docs/ruvectorfindings/specs/_README.md` Domain map (one row per phase, current as of 2026-05-10).
- **Per-phase deliverable status:** `docs/ruvectorfindings/specs/implementation-plan.md` "Per-phase deliverable table".
- **Constitution (immutable rules):** `docs/ruvectorfindings/specs/constitution.md`.
- **This phase's status snapshot:** the `_index.md` in this same directory.
- **Recent activity:** `git log --oneline -30` shows the last 30 commits.
- **Architecture intent:** `docs/ruvectorfindings/{09,10,11,12,13,14}*.md` (read 09 master plan + 10 pivot first).
- **Project rules:** `CLAUDE.md` at workspace root.

### 2. ⚠ ASSUME THIS TASK IS WRONG — audit before you implement

This task body was authored 2026-05-09. Before implementing, audit it against the current codebase:

1. Do the file paths it references still exist? `find` / `ls` them.
2. Do the type signatures it specifies still match what's in tree? `grep` for the type names in `crates/`.
3. Has the crate it tells you to "create" already been created? Check `crates/`. If it exists, STOP — read what's there, reconcile against the task, and update the task itself before implementing more.
4. Does the spec it cites (e.g., `TECH-<DOMAIN>.md`) still describe the same data shapes? Re-read those specs.

If you find drift, document it in your handoff (`memory/handoff/`) and either (a) update the task to match reality before continuing, or (b) escalate to the operator if scope changed materially.

### 3. Hard rules — ZERO tolerance, fail-closed

1. **No backwards compatibility.** Schema changes wipe + recreate; never write migration shims. Synthetic data only.
2. **No mock data in tests.** Use real types, real corpus rows, real RocksDB CFs, real file SHAs. Tests must FAIL when the system is broken — never write a test that passes against a broken state.
3. **Fail closed with structured error codes** (e.g., `MEJEPA_*`, `UTML_*`). No `unwrap_or(default)` on confidence / quota / correlation / signature / chain-integrity paths. No silent fallback. Every error has `.code()` returning `SCREAMING_SNAKE_CASE`.
4. **Robust error logging.** Every fail-path emits `tracing::error!` with the structured code + actionable context (which file, which CF, which byte offset, which step). When something breaks, the log must say exactly what failed and how to fix it.
5. **No comments unless they encode non-obvious WHY.** Don't explain WHAT — well-named identifiers do that.
6. **Files under 500 lines** (split if exceeded).
7. **Provenance comment** at top of every new `.rs` file: `// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).` Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.

### 4. Mandatory Full State Verification (FSV) — every task ships with it

Do not rely on return values alone. After the logic runs, you MUST:

1. **Define the Source of Truth.** State explicitly where the final result lives: a RocksDB column family, a `/tmp/contextgraph-*-fsv/*.json` file, a witness-chain segment, a tier-compressed panel blob, a CF watermark row, a tensor allocator's `live_tensors` map, etc.
2. **Execute & Inspect.** Run the logic, then perform a SEPARATE READ on the Source of Truth to verify the data was processed correctly. Cite the readback command (e.g., `cargo run --release -p <crate> --example <fsv_example>` or a direct CF dump).
3. **Boundary & Edge Case Audit.** Manually simulate AT LEAST 3 edge cases — empty inputs, maximum limits, invalid formats, NaN/Inf in floats, control chars in strings, byte offsets at file boundary, etc. For each, print the system state BEFORE and AFTER the action.
4. **Evidence of Success.** Provide a log showing the actual data residing in the system after execution. Cite the log path in the task's `<verification>` block.

### 5. Trigger → outcome verification (mandatory database / artifact check)

> Every triggered event exists to produce a specific intended outcome. The trigger is observable; the outcome is queryable. If your task writes anything to a persistent surface, you MUST query that surface AFTER running and confirm the expected data is there. Do not trust return values alone.

Synthetic protocol: pick an input where you KNOW the expected output. Run it. Manually check the source of truth for the expected output (e.g., open RocksDB and dump the CF, read the FSV JSON, replay the witness chain). Document the trigger → outcome chain in the FSV evidence. Examples:

- Trigger: `compute_l_step({delta_p:0, delta_k:1, delta_omega:1, delta_xi:1})` → Outcome: returns `0.0` exactly (multiplicative gate; any factor at zero zeros the product). FSV: dump training cert, confirm `l_step == 0.0`.
- Trigger: insert `ShiftEntry { shift_id: "01J0123456789ABCDEFGHIJK", ... }` → Outcome: row in `CF_MEJEPA_SHIFT_WATERMARK` with that exact key. FSV: open RocksDB, `cf.get(b"01J0123456789ABCDEFGHIJK")` returns Some.

### 6. Occam's razor

The simplest implementation that satisfies the contract is the correct one. Do NOT add features, abstractions, or fallbacks beyond the task scope. A bug fix doesn't need surrounding cleanup. Three similar lines beats a premature abstraction. If the task says "create struct X with 5 fields", create exactly that — not a builder, not a derive macro family, not a generic over a placeholder.

### 7. Subagent / verification expectations

After implementation:
- Run `cargo build --release -p <crate>` AND `cargo test -p <crate>` AND `cargo clippy -p <crate> -- -D warnings`.
- Spawn the `code-simplifier` subagent (or equivalent) on your changes; apply BLOCKER fixes.
- Write a final SMAP report at `memory/reports/agent-{N}-{role}--final-report.md`.
- Touch the corresponding row in `tasks/_traceability.md` if the REQ→TASK mapping changed.

---

## 📋 2026-05-10 STATE AUDIT — clean

Step 1 wraps tier_demote + reads CF_MEJEPA_PANEL_META (declared in TASK-HYG-007, pending). No code in tree yet. Task body accurate.

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
