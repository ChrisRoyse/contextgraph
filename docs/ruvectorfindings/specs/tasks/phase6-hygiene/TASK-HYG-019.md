# TASK-HYG-019 — gc_steps::step8_report writer (0o600 mode; pretty JSON)

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

`CF_MEJEPA_GC_HISTORY` is NOT yet declared in heal/cf.rs — task is its first declarer (correct). Task body accurate.

```xml
<task_spec id="TASK-HYG-019" version="1.0">
<metadata>
  <title>gc_steps::step8_report — write GcReport to <runtime_root>/mejepa/gc-report-<YYYY-MM-DD>.json with mode 0o600 + pretty JSON</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>19</sequence>
  <implements>
    <requirement_ref>REQ-HYG-03</requirement_ref>
    <requirement_ref>NFR-HYG-06</requirement_ref>
    <requirement_ref>NFR-HYG-07</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-HYG-003</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 file</estimated_files>
</metadata>

<context>
Step 8: serialize GcReport to pretty JSON (2-space indent per NFR-HYG-07) and write to
`<runtime_root>/mejepa/gc-report-<YYYY-MM-DD>.json` with file mode 0o600 per SEC-MEJEPA-07
+ NFR-HYG-06. Always runs (even when GC is aborted at step 7) so the operator gets a record
of what happened. Also writes a copy into CF_MEJEPA_GC_HISTORY (last 365 entries; older
self-pruned by step 5-equivalent in future passes).
</context>

<input_context_files>
  <file purpose="step_spec">../../technical/TECH-HYGIENE.md#component_contracts</file>
  <file purpose="security">../../constitution.md#security_requirements</file>
  <file purpose="reports">crates/context-graph-mejepa-hygiene/src/reports.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-HYG-003 shipped</check>
</prerequisites>

<scope>
  <in_scope>
    - CREATE `src/gc_steps/step8_report.rs`:
      * `pub const CF_MEJEPA_GC_HISTORY: &str = "CF_MEJEPA_GC_HISTORY";`
      * `pub fn run(env: &HygieneEnv, report: &mut GcReport) -> OpsResult<()>`:
        1. Set report.completed_at = Some(env.now()).
        2. Compute total_disk_used_gb_after via per-CF size sum (re-snapshot).
        3. Validate report (calls report.validate()).
        4. Serialize to pretty JSON (serde_json::to_string_pretty with 2-space indent).
        5. Build path: env.runtime_root.join("mejepa")
           .join(format!("gc-report-{}.json", report.started_at.date_naive())).
        6. Create parent dirs if missing.
        7. On Unix: open with `OpenOptions::new().mode(0o600).create(true).write(true)
           .truncate(true)`; on Windows: best-effort create + warn that 0o600 is Unix-only.
        8. Write JSON; fsync; close.
        9. Persist serialized GcReport bytes into CF_MEJEPA_GC_HISTORY keyed by gc_run_id bytes.
        10. Push GcStepReport { step_index: 8, step_name: "report",
            entries_processed: 1, bytes_freed: 0, ... }.
    - Unit / integration tests:
      * Run step 8 on synthetic GcReport; verify file exists at expected path with mode 0o600;
        re-read JSON parses back to identical GcReport.
      * Run step 8 on aborted GcReport (aborted=true, abort_reason set) → still writes file.
      * stat -c '%a' on file == 600.
      * CF_MEJEPA_GC_HISTORY contains the entry under gc_run_id key.
  </in_scope>
  <out_of_scope>
    - GC history pruning (would be a step-5-equivalent on CF_MEJEPA_GC_HISTORY itself; out of v1.0).
    - Operator UI to read reports.
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-hygiene/src/gc_steps/step8_report.rs">
    pub const CF_MEJEPA_GC_HISTORY: &amp;str = "CF_MEJEPA_GC_HISTORY";

    pub fn run(env: &amp;crate::HygieneEnv, report: &amp;mut crate::reports::GcReport) -&gt; crate::error::OpsResult&lt;()&gt;;
    </signature>
  </signatures>

  <constraints>
    - File mode 0o600 on Unix (NFR-HYG-06 + SEC-MEJEPA-07).
    - JSON pretty-printed 2-space indent (NFR-HYG-07).
    - Always runs (idempotent — even on abort).
    - validate() called BEFORE serialize so corrupt reports surface early.
    - File ≤ 500 lines.
  </constraints>

  <verification>
    - `cargo test -p context-graph-mejepa-hygiene -- step8_report_writes_0o600` passes.
    - `cargo test -p context-graph-mejepa-hygiene -- step8_report_aborted_still_writes` passes.
    - `stat -c '%a' <test report>` returns 600.
    - JSON round-trip identical.
    - clippy clean.
  </verification>
</definition_of_done>

<pseudo_code>
pub fn run(env: &amp;HygieneEnv, report: &amp;mut GcReport) -&gt; OpsResult&lt;()&gt; {
    let started = std::time::Instant::now();
    report.completed_at = Some(env.now());
    report.total_disk_used_gb_after = compute_total_used_gb(env)?;
    report.validate()?;
    let json = serde_json::to_string_pretty(report)
        .map_err(|e| OpsError::internal_with_source("serialize", Box::new(e)))?;
    let date = report.started_at.date_naive().to_string();
    let dir = env.runtime_root.join("mejepa");
    std::fs::create_dir_all(&amp;dir)?;
    let path = dir.join(format!("gc-report-{}.json", date));
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new().mode(0o600).create(true).write(true).truncate(true).open(&amp;path)?;
        std::io::Write::write_all(&amp;mut f, json.as_bytes())?;
        f.sync_all()?;
    }
    #[cfg(not(unix))]
    {
        tracing::warn!(target:"hygiene", "0o600 unsupported on this platform; writing best-effort");
        std::fs::write(&amp;path, &amp;json)?;
    }
    let cf = env.db.cf_handle(CF_MEJEPA_GC_HISTORY).ok_or_else(|| OpsError::internal("gc_hist_cf_missing"))?;
    env.db.put_cf(&amp;cf, report.gc_run_id.as_bytes(), json.as_bytes())?;
    report.step_reports.push(GcStepReport { step_index: 8, step_name: "report".into(),
        duration_ms: started.elapsed().as_millis() as u64,
        entries_processed: 1, bytes_freed: 0, errors_encountered: vec![] });
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/step8_report.rs">report writer 0o600</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-hygiene/src/gc_steps/mod.rs">declare step8</file>
</files_to_modify>

<validation_criteria>
  <criterion>File written at expected path with mode 0o600</criterion>
  <criterion>Pretty JSON round-trip preserves GcReport</criterion>
  <criterion>Aborted reports still write</criterion>
  <criterion>CF_MEJEPA_GC_HISTORY contains entry under gc_run_id</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-hygiene</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step8_report_writes_0o600</command>
  <command>cargo test -p context-graph-mejepa-hygiene -- step8_report_aborted_still_writes</command>
  <command>cargo clippy -p context-graph-mejepa-hygiene -- -D warnings</command>
</test_commands>
</task_spec>
```
