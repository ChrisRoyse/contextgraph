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
