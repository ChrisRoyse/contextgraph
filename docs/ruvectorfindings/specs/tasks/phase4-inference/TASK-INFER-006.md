# TASK-INFER-006 — OOD detection: ood_score formula + σ² tuning

```xml
<task_spec id="TASK-INFER-006" version="1.0">

<metadata>
  <title>OOD detection in `crates/context-graph-mejepa/src/ood.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>6</sequence>
  <implements>
    <item ref="REQ-INFER-07">ood_score(panel) = 1 - exp(-‖g_φ - f_ξ‖² / σ²); σ² tuned so calibration < 0.30; AUC > 0.85</item>
    <item ref="REQ-INFER-09">Gate 1 OOD short-circuit at score > 0.5</item>
    <item ref="component:compute_ood_score">finite + clamp + Tensor norm</item>
    <item ref="component:CalibrationStore.calibrate">sigma_squared tuning step (5)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INFER-002</task_ref>
    <task_ref>TASK-INFER-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (ood.rs)</estimated_files>
</metadata>

<context>
Per doc 09 §4.3 the OOD score is `1 - exp(-‖g_φ(panel) - f_ξ(panel)‖² / σ²)` where σ² is tuned on
the calibration set such that the mean calibration ood_score < 0.30. AUC between the in-distribution
mutation corpus and a synthetic OOD set (random AST diffs from a held-out repo) must exceed 0.85
per Phase 4 DoD. compile() returns OodRefuse on score > 0.5; verify() converts to EscalateToHuman.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (anti_patterns "no silent OOD fallback"; ood_separation_target AUC > 0.85)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INFERENCE.md (REQ-INFER-07; AC-INFER-01-03; AC-INFER-02-01; EC-INFER-04 boundary 0.5; BR-INFER-02)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (component_contracts compute_ood_score; ood_diagram)</file>
  <file purpose="upstream_types">crates/context-graph-mejepa/src/{types.rs, error.rs} (TASK-INFER-001/002)</file>
  <file purpose="conformal">crates/context-graph-mejepa/src/conformal.rs (TASK-INFER-005 — calibrate plumbing accepts sigma_squared)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INFER-002 complete; MejepaInferError variants exist</check>
  <check>TASK-INFER-005 complete; calibrate() accepts sigma_squared</check>
  <check>candle-core ≥ 0.9.x available for `Tensor::norm` / `Tensor::sub` (or use Vec&lt;f32&gt; ops if Phase 2 predictor returns Vec rather than Tensor)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/ood.rs` with:
      - `pub fn compute_ood_score(predicted: &[f32], target: &[f32], sigma_squared: f32) -> Result<f32, MejepaInferError>` — element-wise diff → squared L2 norm → `1 - exp(-norm_sq / sigma_squared)` clamped to [0, 1]
      - `pub fn tune_sigma_squared(calibration_diffs: &[f32], target_mean_ood: f32) -> Result<f32, MejepaInferError>` — grid search σ² candidates (e.g., 100 log-spaced points between 0.01 and 100.0) such that mean(ood_score on calibration) is the largest σ² with mean ≤ target_mean_ood (default 0.30); fail-closed if no σ² achieves the target
      - `pub fn separation_auc(in_dist_scores: &[f32], ood_scores: &[f32]) -> Result<f32, MejepaInferError>` — Mann-Whitney U / Wilcoxon AUC between two distributions; deterministic
    - Calibration_diffs is a flat Vec<f32> of `‖predicted - target‖²` per calibration example (forward-pass produced)
    - Boundary handling per EC-INFER-04: strict `>` 0.5 enforced (0.5 itself does NOT trigger refusal); document in tests
    - Add unit tests:
      - `compute_ood_score_zero_diff_returns_zero`: predicted == target → norm_sq=0 → 1 - exp(0) = 0
      - `compute_ood_score_dim_mismatch`: rejects with DimMismatch
      - `compute_ood_score_nan_input_rejects`: NaN in input → NanDetected
      - `compute_ood_score_clamped_above_zero_below_one`: large diff → asymptotes to 1.0; never exceeds 1.0
      - `compute_ood_score_at_exact_05_does_not_trigger_refusal_check`: score == 0.5 boundary case
      - `tune_sigma_squared_picks_largest_under_target`: synthetic diffs → known σ² grid → analytic check
      - `tune_sigma_squared_fails_closed_when_no_candidate`: target=0.001, all diffs huge → returns ConformalInsufficientSamples or DimMismatch sentinel
      - `separation_auc_perfect_separation_returns_one`: in-dist all 0.1, OOD all 0.9 → AUC = 1.0
      - `separation_auc_overlap_returns_half`: identical distributions → AUC = 0.5
      - `separation_auc_meets_phase4_dod_target_synthetic`: synthetic in-dist ~0.1±0.05, OOD ~0.7±0.1 → AUC > 0.85
    - Hook into `conformal::calibrate(...)` so that the CalibrationStore aggregator (TASK-INFER-010) can call `tune_sigma_squared` after τ is picked and persist the resulting CalibrationRecord with sigma_squared filled
  </in_scope>
  <out_of_scope>
    - Real predictor + frozen target forward passes — TASK-INFER-003 owns those calls; this task only consumes pre-computed `predicted`/`target` byte slices
    - Refusal logic — TASK-INFER-003 (OodRefuse Err) and TASK-INFER-004 (EscalateToHuman conversion)
    - σ² persistence in CF_MEJEPA_CALIBRATION_HISTORY — TASK-INFER-010
    - σ² staleness check (BR-INFER-04) — TASK-INFER-010
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/ood.rs">
use crate::error::MejepaInferError;

pub fn compute_ood_score(predicted: &amp;[f32], target: &amp;[f32], sigma_squared: f32)
    -&gt; Result&lt;f32, MejepaInferError&gt;;

pub fn tune_sigma_squared(calibration_norm_squared: &amp;[f32], target_mean_ood: f32)
    -&gt; Result&lt;f32, MejepaInferError&gt;;

pub fn separation_auc(in_dist_scores: &amp;[f32], ood_scores: &amp;[f32])
    -&gt; Result&lt;f32, MejepaInferError&gt;;

pub const OOD_REFUSE_THRESHOLD: f32 = 0.5;
pub const TARGET_MEAN_OOD_ON_CALIBRATION: f32 = 0.30;
    </signature>
  </signatures>
  <constraints>
    - `compute_ood_score`: predicted.len() == target.len() else DimMismatch
    - any NaN/Inf in predicted/target → NanDetected
    - sigma_squared > 0 else DimMismatch (sentinel) or NanDetected with field "sigma_squared"
    - Result clamped to [0.0, 1.0]; documented (constitution: "no unwrap_or on confidence intervals")
    - Determinism: pairwise summation of squared diffs in fixed order; SIMD only when len multiple of 8 (per implementation_notes)
    - `tune_sigma_squared`: grid candidates `σ² = 10^x` for x ∈ logspace(-2, 2, 100); pick the largest σ² where mean(ood across calibration) ≤ target_mean_ood; if none qualifies (all σ² produce mean > target), return Err with sentinel
    - `separation_auc`: Mann-Whitney U computed via merged sort with rank assignment; ties broken with average rank; result ∈ [0.0, 1.0]
    - Public API documented with `///` rustdoc comments per file_organization rule
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds
    - `cargo test -p context-graph-mejepa ood` shows ≥ 10 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - All boundary tests pass (zero diff, dim mismatch, NaN, clamp)
    - separation_auc returns 1.0 on perfect separation and 0.5 on identical distributions
    - tune_sigma_squared picks largest σ² satisfying target
  </verification>
</definition_of_done>

<pseudo_code>
pub fn compute_ood_score(predicted: &[f32], target: &[f32], sigma_squared: f32) -> Result<f32, MejepaInferError> {
    if predicted.len() != target.len() {
        return Err(MejepaInferError::DimMismatch { expected: predicted.len(), actual: target.len() });
    }
    if !sigma_squared.is_finite() || sigma_squared <= 0.0 {
        return Err(MejepaInferError::NanDetected { field: "sigma_squared".into(), witness_chain_offset: 0 });
    }
    let mut norm_sq = 0.0_f64;
    for (p, t) in predicted.iter().zip(target.iter()) {
        if !p.is_finite() || !t.is_finite() {
            return Err(MejepaInferError::NanDetected { field: "ood_input".into(), witness_chain_offset: 0 });
        }
        let d = (*p - *t) as f64;
        norm_sq += d * d;
    }
    let score = 1.0_f64 - (-norm_sq / sigma_squared as f64).exp();
    let score = score.clamp(0.0, 1.0) as f32;
    Ok(score)
}

pub fn tune_sigma_squared(calibration_norm_squared: &[f32], target: f32) -> Result<f32, MejepaInferError> {
    if calibration_norm_squared.is_empty() {
        return Err(MejepaInferError::ConformalInsufficientSamples { required: 1, observed: 0, language: Language::Python });
    }
    let mut best: Option<f32> = None;
    for x in 0..100 {
        let exp = -2.0 + 4.0 * (x as f32 / 99.0); // -2..=2 logspace
        let sigma2 = 10.0_f32.powf(exp);
        let mean_score: f32 = calibration_norm_squared.iter()
            .map(|n| 1.0 - (-n / sigma2).exp())
            .sum::<f32>() / calibration_norm_squared.len() as f32;
        if mean_score <= target {
            best = Some(match best { Some(b) => b.max(sigma2), None => sigma2 });
        }
    }
    best.ok_or(MejepaInferError::NanDetected { field: "sigma_tune_no_candidate".into(), witness_chain_offset: 0 })
}

pub fn separation_auc(in_dist: &[f32], ood: &[f32]) -> Result<f32, MejepaInferError> {
    if in_dist.is_empty() || ood.is_empty() {
        return Err(MejepaInferError::DimMismatch { expected: 1, actual: 0 });
    }
    // Mann-Whitney U: rank sort merged, AUC = (U1) / (n1 * n2)
    // ... canonical implementation with average ranking on ties ...
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/ood.rs">compute_ood_score + tune_sigma_squared + separation_auc + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/lib.rs">add `pub mod ood;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa ood shows ≥ 10 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>compute_ood_score(predicted=target) == 0</criterion>
  <criterion>compute_ood_score result ∈ [0,1] always</criterion>
  <criterion>NaN/Inf rejected with NanDetected</criterion>
  <criterion>tune_sigma_squared picks largest σ² satisfying mean ≤ 0.30</criterion>
  <criterion>separation_auc on perfect separation = 1.0</criterion>
  <criterion>separation_auc on identical distributions = 0.5</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa ood</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
