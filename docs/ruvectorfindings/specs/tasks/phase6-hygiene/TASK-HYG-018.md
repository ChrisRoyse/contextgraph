# TASK-HYG-018 — gc_steps::step7_verify_integrity + atomic batch commit

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

Atomic batch commit composes steps 4+5 from prior tasks. No code in tree yet. Task body accurate.

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
