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

## 📋 2026-05-10 STATE AUDIT

**Verified by agent-212-task-updater against working tree HEAD.**

- `crates/context-graph-mejepa/src/eval/ood_health.rs` does NOT exist. Accurate on "create".
- **OOD score source**: `crates/context-graph-mejepa/src/ood.rs` is **SHIPPED**. Verify the per-panel OOD score field is exposed on the predictor output before computing AUC.
- AUC implementation — use a fail-closed deterministic computation; do NOT pull a numerical lib that silently NaN-coerces. Pair-wise Mann-Whitney U or trapezoidal AUC over sorted scores per language.
- 4-week drift detection — depends on the CF_MEJEPA_EVAL_REPORTS history (TASK-EVAL-003). Confirm rolling history retrieval helper lands in storage.rs.
- Use `crate::types::Language` (11 variants) — NOT `mejepa-train::eval::Lang`.

---

# TASK-EVAL-006 — Per-language OOD AUC + 4-week drift detection + per-language stratification of OOD calibration

```xml
<task_spec id="TASK-EVAL-006" version="1.0">

<metadata>
  <title>Implement per-language OOD AUC + 4-week drift detection in `crates/context-graph-mejepa/src/eval/ood_health.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>6</sequence>
  <implements>
    <item ref="REQ-EVAL-10">Per-language OOD AUC + 4-week rolling-window drift</item>
    <item ref="ERR-EVAL-04">MEJEPA_EVAL_LANGUAGE_OOD_DRIFT</item>
    <item ref="component:EvalRunner.compute_ood_auc_per_language">trapezoidal ROC AUC; clamp [0, 1]</item>
    <item ref="AC-EVAL-02-04">ood_calibration_health[c] = 0.78 fails ship gate (< 0.85)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-004</task_ref>
    <task_ref>TASK-INFER-006</task_ref><!-- OodDetector available -->
    <task_ref>TASK-CORPUS-***</task_ref><!-- synthetic OOD panels per language from SPEC-CORPUS held-out repo -->
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (ood_health.rs); 1 modified (runner.rs)</estimated_files>
</metadata>

<context>
This task lands the per-language OOD calibration health computation (AUC ≥ 0.85 ship-gate condition v
per REQ-EVAL-03) AND the 4-week rolling-window drift detection. Per-language stratification is
required because a Python-trained model evaluating Rust must produce HIGH OOD scores (refuse), not
confident wrong predictions — Python bias mitigation per doc 10 §C. The 4-week trend reads prior
weekly reports from CF_MEJEPA_EVAL_REPORTS.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (performance_budgets ood_separation_target AUC > 0.85)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-10 per-language OOD; AC-EVAL-02-04; ERR-EVAL-04)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (compute_ood_auc_per_language behavior; trapezoidal ROC AUC)</file>
  <file purpose="prior_task">TASK-EVAL-001..004</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-004 shipped</check>
  <check>TASK-INFER-006 shipped: `OodDetector::ood_score(&Panel) -> f32` available</check>
  <check>SPEC-CORPUS exposes synthetic OOD panels per language at `./data/mejepa-corpus/synthetic_ood/<language>/`</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/eval/ood_health.rs` with:
      - `pub fn compute_ood_auc_per_language(language: Language, in_dist_panels: &[Panel], ood_panels: &[Panel], ood_detector: &OodDetector, config: &EvalConfig) -> Result<Option<f32>, EvalError>`
        - Validate `in_dist_panels.len() >= 50`; else return `Ok(None)` and log `InsufficientSamples`
        - For each panel compute `ood_score = ood_detector.ood_score(&panel)?`
        - Build score-label vector: in-dist → label 1; ood → label 0
        - Sort by score descending
        - Compute ROC AUC via trapezoidal integration over thresholds at each unique score
        - Validate finite; clamp [0, 1]
      - `pub fn detect_ood_drift_per_language(current_auc_per_language: &BTreeMap<Language, f32>, prior_history: &[(NaiveDate, BTreeMap<Language, f32>)], config: &EvalConfig) -> Result<Vec<EvalError>, EvalError>`
        - For each language present in current AND in oldest of `config.ood_drift_window_weeks` history snapshots
        - If `delta = (oldest_auc - current_auc) > config.ood_drift_threshold` (default 0.10)
        - Emit `EvalError::LanguageOodDrift{language, prior_auc: oldest_auc, current_auc, delta, window_weeks: config.ood_drift_window_weeks}` to result Vec (informational; not propagated)
      - `pub fn load_ood_history_from_storage(storage: &RocksDbStore, weeks: usize) -> Result<Vec<(NaiveDate, BTreeMap<Language, f32>)>, EvalError>`
        - Prefix-scan CF_MEJEPA_EVAL_REPORTS in date order; deserialize each report; extract `ood_calibration_health` snapshot
        - Return last `weeks` entries (default 4)
    - Update `crates/context-graph-mejepa/src/eval/runner.rs`:
      - Wire step 5a of `run_holdout_eval` to call `compute_ood_auc_per_language` per language and store in `EvalReportBuilder.ood_calibration_health`
      - Wire `load_ood_history_from_storage` and `detect_ood_drift_per_language` to populate `ood_calibration_health_history` field
      - Add returned drift errors to `ship_gate_failures` (informational; consumed by TASK-EVAL-008 ship-gate logic)
    - Add unit + integration tests in `ood_health.rs` `#[cfg(test)] mod tests` for:
      - `auc_perfect_separation_returns_1` (in-dist all score 0.1, OOD all score 0.9)
      - `auc_random_returns_0_5` (uniform random scores)
      - `auc_under_50_in_dist_returns_none` (graceful)
      - `drift_detected_when_delta_exceeds_0_10` (oldest=0.95, current=0.83 → delta=0.12 → flag)
      - `drift_not_flagged_when_within_threshold` (delta=0.05 → no flag)
      - `auc_handles_nan_score_via_error`
      - `load_ood_history_returns_last_4_weeks` (synthesize 5 weekly reports; expect 4)
  </in_scope>
  <out_of_scope>
    - Per-language conformal coverage (TASK-EVAL-005)
    - Per-language Gτ pass rate (TASK-EVAL-007)
    - check_ship_gate consumption (TASK-EVAL-008)
    - Synthetic OOD panel generation (owned by SPEC-CORPUS; consumed read-only here)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/eval/ood_health.rs">
use std::collections::BTreeMap;
use chrono::NaiveDate;
use crate::types::Language;
use super::config::EvalConfig;
use super::error::EvalError;

pub fn compute_ood_auc_per_language(
    language: Language,
    in_dist_panels: &amp;[Panel],
    ood_panels: &amp;[Panel],
    ood_detector: &amp;OodDetector,
    config: &amp;EvalConfig,
) -&gt; Result&lt;Option&lt;f32&gt;, EvalError&gt;;

pub fn detect_ood_drift_per_language(
    current: &amp;BTreeMap&lt;Language, f32&gt;,
    history: &amp;[(NaiveDate, BTreeMap&lt;Language, f32&gt;)],
    config: &amp;EvalConfig,
) -&gt; Result&lt;Vec&lt;EvalError&gt;, EvalError&gt;;

pub fn load_ood_history_from_storage(
    storage: &amp;RocksDbStore,
    weeks: usize,
) -&gt; Result&lt;Vec&lt;(NaiveDate, BTreeMap&lt;Language, f32&gt;)&gt;, EvalError&gt;;
    </signature>
  </signatures>
  <constraints>
    - AUC computation uses trapezoidal integration on sorted-by-score thresholds; deterministic via stable sort
    - Reject NaN ood_score via `EvalError::NanDetected{field: "ood_score", cell: Some(format!("{:?}", language))}`
    - When in_dist_panels < 50, return Ok(None) — informational, not error
    - When ood_panels < 50, return Ok(None) with log warning
    - 4-week drift uses OLDEST entry in window vs CURRENT (per doc 10 §C "drift over rolling window")
    - All `Err` returns wrapped with `.log_context("eval::ood_health")`
    - File length budget: < 500 lines
  </constraints>
  <verification>
    - `cargo build/test/clippy` clean
    - ≥ 7 passing tests
    - AUC monotone: perfect → 1.0; random → ≈ 0.5; reversed → ≈ 0.0 (clamped)
  </verification>
</definition_of_done>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/eval/ood_health.rs">compute_ood_auc_per_language + detect_ood_drift_per_language + load_ood_history_from_storage + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/eval/runner.rs">wire step 5a in run_holdout_eval</file>
  <file path="crates/context-graph-mejepa/src/eval/mod.rs">declare `pub mod ood_health;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build / test / clippy clean</criterion>
  <criterion>AUC = 1.0 on perfect separation</criterion>
  <criterion>AUC = 0.5 (±0.02) on random</criterion>
  <criterion>Returns Ok(None) when N < 50</criterion>
  <criterion>Drift detected when delta > 0.10</criterion>
  <criterion>load_ood_history_from_storage returns at most `weeks` entries in date order</criterion>
  <criterion>run_holdout_eval populates EvalReport.ood_calibration_health AND ood_calibration_health_history</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa eval::ood_health</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
