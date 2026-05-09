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
