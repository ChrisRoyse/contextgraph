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

## 📋 2026-05-11 STATE AUDIT (sharpened against verified codebase)

> **Codebase reality table:** see `_index.md` in this directory.

**Confirmed 2026-05-11:**
- `crates/context-graph-mejepa-tct/src/gtau.rs::ConstellationGuard::gtau_test` SHIPPED (Phase 4b).
- The TCT spec hosts threshold calibration; this Phase 8 task is READ-ONLY against the SHIPPED guard.

**Decided scope for this task:**
1. New file `crates/context-graph-mejepa/src/eval/gtau_health.rs`.
2. `compute_gtau_pass_rate_per_language(holdout: &HoldoutDataset, guard: &ConstellationGuard) -> Result<BTreeMap<Language, f32>, EvalError>` — for each panel call `guard.gtau_test(predicted_panel)`; group pass/fail by `language`; return pass-rate fraction `[0, 1]`.
3. Per-language Gτ thresholds are owned by TCT spec; this task READS the per-language calibration from the TCT constellation store and reports it in the `EvalReport`.
4. Each language with pass-rate < 0.95 fails `EvalReport.gtau_pass_rate` aggregation; ship gate respects per-language constraint via TASK-EVAL-008's 8-condition AND.
5. NaN inputs trigger `EvalError::PanelMaterializationFailed`.

**FSV for this task:** integration test feeds a synthetic holdout with KNOWN Gτ outcomes (mock the guard with deterministic pass/fail per language for testing only — the GUARD itself is not mocked; what's mocked is the synthetic input) → asserts pass-rate per language matches expected. Source of Truth: returned BTreeMap.

---

# TASK-EVAL-007 — Per-language Gτ pass rate (read-only consumption of TCT calibrated thresholds)

```xml
<task_spec id="TASK-EVAL-007" version="1.0">

<metadata>
  <title>Implement per-language Gτ pass-rate computation in `crates/context-graph-mejepa/src/eval/gtau_health.rs` (read-only consumer of SPEC-TCT)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>7</sequence>
  <implements>
    <item ref="REQ-EVAL-02">gtau_pass_rate field per language</item>
    <item ref="REQ-EVAL-10">Per-language stratification of Gτ thresholds (cross-cutting with TCT)</item>
    <item ref="component:EvalRunner.run_holdout_eval step 5c">gtau_pass_rate[l] = fraction Approve</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-004</task_ref>
    <task_ref>TASK-TCT-***</task_ref><!-- ConstellationGuard::gtau_test available -->
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 created (gtau_health.rs); 1 modified (runner.rs)</estimated_files>
</metadata>

<context>
This task lands the per-language Gτ pass-rate metric — fraction of holdout panels for which
`ConstellationGuard::gtau_test(panel)` returns Approve. **Cross-cutting boundary:** TCT spec hosts
the actual Gτ threshold calibration AND the per-language stratification of those thresholds
(per-cell math: 8 mutation × 13 entity × 11 languages × 21 embedders); this task only READS the
result and rolls it up per language. When SPEC-TCT not yet shipped, `ConstellationGuard` is a
mocked-always-Approve (per phase4-inference _index.md note); this task's pass-rate will be 1.0
across all languages and surface a `provenance_warnings: ["constellation_unavailable"]` flag in
EvalReport (added in TASK-EVAL-015 weekly report writer).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (tct_invariants rule 3 "Gτ test is deterministic O(N · d_max), no learned model"; rule 5 per-cell math)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-02 gtau_pass_rate; REQ-EVAL-10 per-language stratification)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (run_holdout_eval step 5c; ConstellationGuard::gtau_test)</file>
  <file purpose="prior_task">TASK-EVAL-001..004</file>
  <file purpose="cross_domain_authority">docs/ruvectorfindings/specs/functional/SPEC-TCT.md (Gτ threshold calibration owner)</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-004 shipped</check>
  <check>SPEC-TCT (or its Phase 4b mock) exposes `ConstellationGuard::gtau_test(&Panel) -> Result<GtauVerdict, _>` where `GtauVerdict = enum { Approve, Reject{violating_embedders, violating_chunks} }`</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/eval/gtau_health.rs` with:
      - `pub fn compute_gtau_pass_rate_per_language(panels: &[Panel], gtau_checker: &ConstellationGuard, config: &EvalConfig) -> Result<BTreeMap<Language, f32>, EvalError>`
        - For each language `l`:
          1. Filter panels to language `l`
          2. If N < `config.min_samples_per_language`, skip (entry absent from result map)
          3. Count Approve verdicts via `gtau_checker.gtau_test(panel)`
          4. `pass_rate = approve_count / N`; clamp [0, 1]
          5. Insert into result map
        - Return `BTreeMap<Language, f32>`
      - `pub fn gtau_constellation_unavailable_warning(gtau_checker: &ConstellationGuard) -> Option<&'static str>` — returns `Some("constellation_unavailable")` when checker is the MockedAlwaysApprove placeholder; else None. Used by TASK-EVAL-015 to populate `provenance_warnings`.
    - Update `crates/context-graph-mejepa/src/eval/runner.rs`:
      - Wire step 5c of `run_holdout_eval` to call `compute_gtau_pass_rate_per_language` and store in `EvalReportBuilder.gtau_pass_rate`
    - Add unit tests in `gtau_health.rs` `#[cfg(test)] mod tests` for:
      - `gtau_pass_rate_all_approve_returns_1` (mock checker always Approve)
      - `gtau_pass_rate_half_reject_returns_0_5`
      - `gtau_pass_rate_under_min_samples_skips_language`
      - `gtau_pass_rate_returns_btree_with_deterministic_iter`
      - `gtau_constellation_unavailable_warning_returns_some_when_mocked`
  </in_scope>
  <out_of_scope>
    - Gτ threshold calibration (SPEC-TCT owns)
    - Per-cell Gτ math (8 × 13 × 11 × 21 = 24,024 cells per constitution; SPEC-TCT owns)
    - Bayesian shrinkage of Gτ centroids when N < 50 (SPEC-TCT owns)
    - check_ship_gate consumption (TASK-EVAL-008)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/eval/gtau_health.rs">
use std::collections::BTreeMap;
use crate::types::Language;
use super::config::EvalConfig;
use super::error::EvalError;

pub fn compute_gtau_pass_rate_per_language(
    panels: &amp;[Panel],
    gtau_checker: &amp;ConstellationGuard,
    config: &amp;EvalConfig,
) -&gt; Result&lt;BTreeMap&lt;Language, f32&gt;, EvalError&gt;;

pub fn gtau_constellation_unavailable_warning(gtau_checker: &amp;ConstellationGuard) -&gt; Option&lt;&amp;'static str&gt;;
    </signature>
  </signatures>
  <constraints>
    - Strictly read-only (does not mutate constellation, does not write to CF_MEJEPA_CONSTELLATION)
    - Languages with N < min_samples_per_language are absent from result map (NOT 0.0); ship gate later (TASK-EVAL-008) skips absent entries
    - All Pearson-style errors propagate via `.log_context("eval::gtau_health")`
    - File length budget: < 200 lines
  </constraints>
  <verification>
    - `cargo build/test/clippy` clean
    - ≥ 5 passing tests
    - run_holdout_eval populates EvalReport.gtau_pass_rate
  </verification>
</definition_of_done>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/eval/gtau_health.rs">compute_gtau_pass_rate_per_language + gtau_constellation_unavailable_warning + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/eval/runner.rs">wire step 5c</file>
  <file path="crates/context-graph-mejepa/src/eval/mod.rs">declare `pub mod gtau_health;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build / test / clippy clean</criterion>
  <criterion>compute_gtau_pass_rate_per_language returns BTreeMap with deterministic iteration</criterion>
  <criterion>Pass rate clamped [0, 1]</criterion>
  <criterion>Languages under min_samples absent from map</criterion>
  <criterion>gtau_constellation_unavailable_warning returns Some when mock; None when real</criterion>
  <criterion>run_holdout_eval populates EvalReport.gtau_pass_rate</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa eval::gtau_health</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
