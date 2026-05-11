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
- `crates/context-graph-mejepa/src/calibration.rs` ships `CalibrationStore` with `record_residual` + `quantile`. Conformal calibration SHIPPED.
- `crates/context-graph-mejepa/src/conformal.rs` ships the conformal set construction.
- No per-language stratification exists today; both files are language-agnostic.

**Decided scope for this task:**
1. Add `compute_conformal_coverage_per_language(holdout: &HoldoutDataset, calibration: &CalibrationStore) -> Result<BTreeMap<Language, ConformalHealthEntry>, EvalError>` to `crates/context-graph-mejepa/src/eval/conformal_health.rs` (new file).
2. For each `Language` in 11-variant enum: filter the calibration residuals to that language; compute the empirical coverage at the stated 90% level; produce `ConformalHealthEntry { stated: 0.90, empirical: <measured>, delta_pct: (empirical - stated) * 100, num_samples: <N>, passes_band: |delta_pct| <= 2.0 }`.
3. If any language has `num_samples < 30`, set `passes_band: None` (use `Option<bool>` if needed) and emit `tracing::warn!` with `code = "MEJEPA_EVAL_LANGUAGE_INSUFFICIENT_CALIBRATION_SAMPLES"`.
4. If any language fails the band, set the report flag `MEJEPA_EVAL_LANGUAGE_CONFORMAL_DRIFT` (a structured field in `EvalReport`).
5. The `±2 %` band is constitution-pinned via `ShipGateThresholds::conformal_band_pct` (default 2.0).

**FSV for this task:** integration test feeds a synthetic holdout with KNOWN per-language coverage drift (e.g., python: empirical 0.92, rust: empirical 0.87) → asserts `passes_band` correct per language → asserts the drift flag fires only on the failing language. Source of Truth: returned BTreeMap + flag readback.

---

# TASK-EVAL-005 — Per-language stratified conformal coverage health (±2% band check)

```xml
<task_spec id="TASK-EVAL-005" version="1.0">

<metadata>
  <title>Implement per-language conformal coverage health computation in `crates/context-graph-mejepa/src/eval/conformal_health.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>5</sequence>
  <implements>
    <item ref="REQ-EVAL-10">Per-language conformal coverage health with ±2% band</item>
    <item ref="ERR-EVAL-04">MEJEPA_EVAL_LANGUAGE_CONFORMAL_DRIFT</item>
    <item ref="AC-EVAL-02-05">conformal_coverage_health[ruby] outside [0.88, 0.92] fails ship gate</item>
    <item ref="component:EvalRunner.compute_conformal_coverage_per_language">empirical coverage = (count actual outcome ∈ predicted set) / N</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-004</task_ref><!-- run_holdout_eval scaffolding + per-panel pairs accumulator -->
    <task_ref>TASK-INFER-005</task_ref><!-- ConformalSet (stated alpha + outcome_set) populated by compile() -->
    <task_ref>TASK-INFER-010</task_ref><!-- CalibrationStore for per-language tau threshold -->
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (conformal_health.rs); 1 modified (runner.rs to wire step 5b)</estimated_files>
</metadata>

<context>
This task lands the per-language stratification of conformal-coverage health (Python bias mitigation
per doc 10 §C). For each language `l`, count the holdout examples where `actual_oracle_outcome ∈
predicted_conformal_set.outcome_set`, divide by N, compute `delta_pct = (empirical − stated) × 100`,
fail-band when |delta_pct| > 2.0. The ship gate later (TASK-EVAL-008) consumes
`ConformalHealthEntry.passes_band` per language. Cross-cutting note: TCT spec hosts Gτ threshold
calibration; SELF-HEALING hosts drift detection; this task hosts the per-language conformal
coverage band check as a static slice of the report.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (performance_budgets conformal_coverage_target ±2%)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-10 conformal coverage; AC-EVAL-02-05; ERR-EVAL-05)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (compute_conformal_coverage_per_language behavior; ConformalHealthEntry data model; MEJEPA_EVAL_LANGUAGE_CONFORMAL_DRIFT failure mode)</file>
  <file purpose="prior_task">TASK-EVAL-001..004</file>
  <file purpose="cross_domain">crates/context-graph-mejepa/src/eval/calibration_store.rs (TASK-INFER-010 stated alpha)</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-004 shipped: `run_holdout_eval` produces per-panel pairs vector with language + ConformalSet</check>
  <check>TASK-INFER-010 shipped: `CalibrationStore::stated_alpha_for_language(l) -> f32` available</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/eval/conformal_health.rs` with:
      - `pub fn compute_conformal_coverage_per_language(panels: &[HoldoutPanel], predictions: &[RealityPrediction], oracle_outcomes: &[OracleOutcome], stated_alpha_per_language: &BTreeMap<Language, f32>, config: &EvalConfig) -> Result<BTreeMap<Language, ConformalHealthEntry>, EvalError>`
      - For each language `l`:
        1. Filter pairs to language `l`
        2. If N < `config.min_samples_per_language` (default 30): skip language; do NOT create entry; emit `EvalError::EmptyHoldoutLanguage{language}` to log_context
        3. Stated coverage = `1 - stated_alpha_per_language[l]` (default 0.90 when α=0.10)
        4. Empirical coverage = `count(actual_outcome ∈ prediction.confidence.outcome_set) / N`
        5. `delta_pct = (empirical - stated) × 100`
        6. `passes_band = |delta_pct| <= config.thresholds.conformal_band_pct` (default 2.0)
        7. Construct `ConformalHealthEntry { stated, empirical, delta_pct, num_samples: N, passes_band }`
      - Return `BTreeMap<Language, ConformalHealthEntry>` (deterministic iter)
    - Update `crates/context-graph-mejepa/src/eval/runner.rs`:
      - Wire step 5b of `run_holdout_eval` to call `compute_conformal_coverage_per_language` and store in `EvalReportBuilder.conformal_coverage_health`
      - Wrap call site with `.map_err(|e| { e.log_context("eval::runner.conformal"); e })?`
    - Add unit + integration tests in `conformal_health.rs` `#[cfg(test)] mod tests` for:
      - `coverage_perfect_returns_band_pass` (every actual ∈ predicted set; empirical = 1.0)
      - `coverage_off_by_three_pct_fails_band` (3% gap)
      - `coverage_within_band_returns_pass` (1.5% gap)
      - `language_under_min_samples_skipped` (N=10 → no entry; no panic)
      - `delta_pct_sign_negative_when_under_covered` (e.g., empirical=0.85, stated=0.90 → delta=-5)
      - `bivariate_round_trip_serde` (ConformalHealthEntry serde round-trip)
  </in_scope>
  <out_of_scope>
    - Per-language OOD AUC + 4-week drift (TASK-EVAL-006)
    - Per-language Gτ pass rate (TASK-EVAL-007)
    - check_ship_gate consumption (TASK-EVAL-008)
    - 4-week conformal drift trend analysis (out of Phase 8 v1; tracked as future task once 4 weeks of CF_MEJEPA_EVAL_REPORTS exist)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/eval/conformal_health.rs">
use std::collections::BTreeMap;
use crate::types::{Language, OracleOutcome, RealityPrediction};
use super::config::EvalConfig;
use super::error::EvalError;
use super::types::ConformalHealthEntry;
use crate::eval::HoldoutPanel;

pub fn compute_conformal_coverage_per_language(
    panels: &[HoldoutPanel],
    predictions: &[RealityPrediction],
    oracle_outcomes: &[OracleOutcome],
    stated_alpha_per_language: &BTreeMap&lt;Language, f32&gt;,
    config: &EvalConfig,
) -&gt; Result&lt;BTreeMap&lt;Language, ConformalHealthEntry&gt;, EvalError&gt;;
    </signature>
  </signatures>
  <constraints>
    - Inputs `panels`, `predictions`, `oracle_outcomes` must have identical length; mismatch → `EvalError::NanDetected{field: "conformal_health_input_length_mismatch", cell: None}`
    - Empirical coverage clamped to [0, 1]; intermediate division by zero (N == 0) skipped via min_samples gate
    - Stated coverage validated finite + in [0, 1]; missing language in `stated_alpha_per_language` map → return `EvalError::MissingProvenance{field: format!("stated_alpha_for_{:?}", language)}`
    - `passes_band` strictly uses `<=` (boundary inclusive)
    - All floats validated finite at construction of `ConformalHealthEntry`
    - All `Err` returns wrapped with `.log_context("eval::conformal_health")`
    - File length budget: < 500 lines
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` clean
    - `cargo test -p context-graph-mejepa eval::conformal_health` shows ≥ 6 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
pub fn compute_conformal_coverage_per_language(...) -> Result<BTreeMap<Language, ConformalHealthEntry>, EvalError> {
    if panels.len() != predictions.len() || panels.len() != oracle_outcomes.len() {
        return Err(EvalError::NanDetected{field: "conformal_health_input_length_mismatch".into(), cell: None});
    }
    let mut by_lang: BTreeMap<Language, Vec<usize>> = BTreeMap::new();
    for (i, p) in panels.iter().enumerate() { by_lang.entry(p.language).or_default().push(i); }
    let mut out = BTreeMap::new();
    for (lang, indices) in by_lang {
        if indices.len() < config.min_samples_per_language { continue; }
        let stated_alpha = *stated_alpha_per_language.get(&lang)
            .ok_or_else(|| EvalError::MissingProvenance{ field: format!("stated_alpha_for_{:?}", lang) })?;
        let stated = 1.0 - stated_alpha;
        let covered = indices.iter().filter(|&&i| {
            predictions[i].confidence.outcome_set.contains(&oracle_outcomes[i])
        }).count();
        let empirical = covered as f32 / indices.len() as f32;
        let delta_pct = (empirical - stated) * 100.0;
        let passes_band = delta_pct.abs() <= config.thresholds.conformal_band_pct;
        out.insert(lang, ConformalHealthEntry { stated, empirical, delta_pct, num_samples: indices.len(), passes_band });
    }
    Ok(out)
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/eval/conformal_health.rs">compute_conformal_coverage_per_language + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/eval/runner.rs">wire step 5b in run_holdout_eval</file>
  <file path="crates/context-graph-mejepa/src/eval/mod.rs">declare `pub mod conformal_health;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build / test / clippy clean</criterion>
  <criterion>compute_conformal_coverage_per_language returns BTreeMap with deterministic iteration</criterion>
  <criterion>passes_band = false when |delta_pct| > 2.0; true when ≤ 2.0</criterion>
  <criterion>Languages with N < 30 are absent from the result map (not None — absent)</criterion>
  <criterion>Length-mismatch input returns EvalError::NanDetected</criterion>
  <criterion>Missing stated alpha returns EvalError::MissingProvenance</criterion>
  <criterion>Run_holdout_eval now populates EvalReport.conformal_coverage_health</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa eval::conformal_health</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
