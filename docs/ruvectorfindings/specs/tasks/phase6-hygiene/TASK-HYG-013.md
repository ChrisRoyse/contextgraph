# TASK-HYG-013 — gc_steps::step2_compact (parallel compact_range_cf per ME-JEPA CF)

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

## 📋 2026-05-10 STATE AUDIT — flagged

Task body's CF list (lines 49-52) cites `CF_MEJEPA_PANELS`, `CF_MEJEPA_PANEL_META`, `CF_MEJEPA_WEIGHT_BLOBS`, `CF_MEJEPA_TRAIN_CERTS`, `CF_MEJEPA_ORACLE_VERDICTS`, `CF_MEJEPA_MUTATION_CORPUS`, `CF_MEJEPA_LIVE_PREDICTIONS`, `CF_MEJEPA_SHIFT_WATERMARK`, `CF_MEJEPA_QUOTA_STATE`, `CF_MEJEPA_WITNESS_SEGMENT_META`, `CF_MEJEPA_GC_HISTORY`. Of these, `CF_MEJEPA_WEIGHT_BLOBS`, `CF_MEJEPA_TRAIN_CERTS`, and `CF_MEJEPA_SHIFT_WATERMARK` ARE ALREADY declared in `crates/context-graph-mejepa/src/heal/cf.rs`. The hygiene crate must import these existing consts (or re-derive the canonical CF-name strings) and not re-`pub const` them. Also missing from the task list (consider adding): `CF_MEJEPA_FISHER_SNAPSHOTS`, `CF_MEJEPA_DRIFT_WINDOW`, `CF_MEJEPA_DRIFT_HISTORY`, `CF_MEJEPA_HEAL_REPORTS`, `CF_MEJEPA_DISTILL_STEPS`, `CF_MEJEPA_ACTIVE_POINTERS`, `CF_MEJEPA_CALIBRATION_HISTORY` — all present in heal/cf.rs and should also be subject to compact_range_cf if they accumulate tombstones.

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
