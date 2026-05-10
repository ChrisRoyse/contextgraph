# TASK-HYG-017 — gc_steps::step6_compress_witness wrapper

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

Thin wrapper around witness_compress::compress_segments (TASK-HYG-010, pending). Task body accurate.

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
