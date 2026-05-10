# TASK-HYG-016 — gc_steps::step5_prune_calibration

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

Task body declares `pub const CF_MEJEPA_CALIBRATION_HISTORY: &str = "CF_MEJEPA_CALIBRATION_HISTORY";` (line 59). BUT this CF is ALREADY declared in `crates/context-graph-mejepa/src/heal/cf.rs:16`. The hygiene crate must import the existing const (`use context_graph_mejepa::heal::cf::CF_MEJEPA_CALIBRATION_HISTORY;`) rather than re-`pub const`. Adjust the implementation accordingly. Also: heal/cf.rs already wires a `RetentionPolicy` for this CF (lines 206-212) — verify the task's "keep last 30" semantics align with or override what heal already defines, and reconcile in one place to avoid double-pruning.

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
