# TASK-INFER-005 — Conformal calibration: per-example non-conformity score + (1-α) quantile threshold

```xml
<task_spec id="TASK-INFER-005" version="1.0">

<metadata>
  <title>Conformal calibration in `crates/context-graph-mejepa/src/conformal.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>5</sequence>
  <implements>
    <item ref="REQ-INFER-06">Conformal calibration: per-example max-test L_∞ residual; (1-α) quantile τ; default α=0.10 → coverage 90%</item>
    <item ref="component:CalibrationStore.calibrate">behavior steps 1-7 in TECH-INFERENCE</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INFER-002</task_ref>
    <task_ref>TASK-INFER-010</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (conformal.rs)</estimated_files>
</metadata>

<context>
Per doc 09 §4.2 the non-conformity score is `s = max_test |predicted_test_pass[t] - actual_test_pass[t]|`,
the per-test L_∞ residual. The conformal threshold τ is the `⌈(N+1)(1-α)⌉/N`-quantile of sorted
calibration scores (split-conformal correction). At inference time, prediction set is
`{outcomes : non_conformity_score(outcome) ≤ τ}`. By construction, marginal coverage on calibration
split equals stated 1-α exactly; out-of-fold empirical coverage must measure within ±2%.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (conformal_coverage_target ±2%; performance_budgets)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INFERENCE.md (REQ-INFER-06; AC-INFER-02-01..03; EC-INFER-03)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (component_contracts CalibrationStore::calibrate; conformal_calibration_diagram)</file>
  <file purpose="upstream_types">crates/context-graph-mejepa/src/{types.rs, calibration_types.rs, error.rs} (TASK-INFER-001/002)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INFER-002 complete; CalibrationRecord + MejepaInferError exist</check>
  <check>TASK-INFER-010 complete; CalibrationStore exists for persistence</check>
  <check>Phase 0 calibration split available: ./data/mejepa-corpus/calibration with stratified by language; min 30 per (lang × mutation_category) per AC-INFER-02-03</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/conformal.rs` with:
      - `pub struct CalibrationExample { pub language: Language, pub predicted_test_pass: Vec<f32>, pub actual_test_pass: Vec<f32> }` (per-test predicted vs actual probabilities)
      - `pub fn non_conformity_score(predicted: &[f32], actual: &[f32]) -> Result<f32, MejepaInferError>` returning `max_t |predicted[t] - actual[t]|` after dim/finite check
      - `pub fn calibrate_tau(scores: &[f32], alpha: f32) -> Result<f32, MejepaInferError>` implementing split-conformal `idx = ceil((N+1)*(1-alpha))` clamped to N-1; returns scores[idx] after sort
      - `pub fn calibrate(examples: &[CalibrationExample], alpha: f32, min_samples_per_stratum: usize) -> Result<CalibrationRecord, MejepaInferError>` orchestrating per-example score → quantile → record (sigma_squared filled by TASK-INFER-006 in a separate calibrate_sigma() call invoked by the CalibrationStore aggregator)
      - Per-language stratum count check; if any language < min_samples_per_stratum return `ConformalInsufficientSamples { required, observed, language }`
      - Return CalibrationRecord with empirical_coverage on calibration set itself (sanity check; equals stated by construction = 1-alpha to within FP error)
    - Add unit tests:
      - `non_conformity_score_max_test_correct`: per-test residuals [0.1, 0.3, 0.05] → 0.3
      - `non_conformity_score_dim_mismatch_rejects`: predicted len 3 vs actual len 4 → DimMismatch
      - `non_conformity_score_nan_rejects`: NaN in either → NanDetected
      - `calibrate_tau_split_conformal_quantile`: scores [0.1, 0.2, 0.3, 0.4, 0.5], alpha=0.10 → tau = scores[ceil(6*0.9)-1] = scores[5-1] = 0.5 (or per formula); compute analytically and assert
      - `calibrate_alpha_out_of_range_rejects`: alpha=0.6 → DimMismatch-like error or dedicated
      - `calibrate_insufficient_per_language_samples`: 5 Python examples with min=10 → ConformalInsufficientSamples
      - `calibrate_sanity_coverage_equals_stated`: 100 examples, alpha=0.10 → empirical_coverage on the calibration set itself ∈ [0.88, 0.92]
      - `calibrate_returns_record_with_provenance`: corpus_sha + embedder_versions + frozen_at populated from inputs
    - Implement deterministic sort (stable, ordered, NaN-safe — reject NaN before sort)
  </in_scope>
  <out_of_scope>
    - σ² (sigma_squared) tuning — TASK-INFER-006 produces it; this task accepts it via input or fills with 1.0 placeholder (per CalibrationStore aggregator)
    - Persistence to RocksDB — TASK-INFER-010 owns CalibrationStore.persist
    - Outcome-set construction — TASK-INFER-007
    - Sliding-window recompute — TASK-INFER-010 (recompute_sliding_window)
    - Forward-pass over calibration corpus (computes predicted_test_pass) — that's TASK-INFER-013 CLI driver
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/conformal.rs">
use crate::types::Language;
use crate::error::MejepaInferError;
use crate::calibration_types::CalibrationRecord;

#[derive(Debug, Clone)]
pub struct CalibrationExample {
    pub language: Language,
    pub predicted_test_pass: Vec&lt;f32&gt;,
    pub actual_test_pass: Vec&lt;f32&gt;,
}

pub fn non_conformity_score(predicted: &amp;[f32], actual: &amp;[f32])
    -&gt; Result&lt;f32, MejepaInferError&gt;;

pub fn calibrate_tau(scores: &amp;[f32], alpha: f32)
    -&gt; Result&lt;f32, MejepaInferError&gt;;

pub fn calibrate(
    examples: &amp;[CalibrationExample],
    alpha: f32,
    min_samples_per_stratum: usize,
    sigma_squared: f32,
    corpus_sha: [u8; 32],
    embedder_versions: std::collections::BTreeMap&lt;crate::types::EmbedderId, String&gt;,
) -&gt; Result&lt;CalibrationRecord, MejepaInferError&gt;;
    </signature>
  </signatures>
  <constraints>
    - `non_conformity_score`: rejects len mismatch with DimMismatch; rejects any NaN/Inf with NanDetected; returns `max_t |a[t] - b[t]|` (L_∞ over per-test residuals)
    - `calibrate_tau`: panics-free; sorts scores ascending; rejects empty `scores` with ConformalInsufficientSamples { required: 1 }; rejects alpha outside [0.01, 0.50] (uses MejepaInferError::ConformalInsufficientSamples or dedicated DimMismatch — chosen variant: ConformalInsufficientSamples with required=u32::MAX placeholder + log)
    - Quantile index = `(((N + 1) as f32 * (1.0 - alpha)).ceil() as usize - 1).min(N - 1).max(0)`
    - `calibrate`: per-language stratum check happens BEFORE score sort; counts per `Language` enum
    - empirical_coverage_on_calibration: `count(score <= tau) / N`, must equal `(N - quantile_idx) / N` ≈ 1-alpha
    - Floats compared via `total_cmp` (NaN-safe)
    - Determinism: same inputs → same tau (sort stable; reduction order fixed)
    - Module is `pub` so TASK-INFER-007 can import `non_conformity_score`
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds
    - `cargo test -p context-graph-mejepa conformal` shows ≥ 8 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - non_conformity_score returns L_∞ residual
    - calibrate_tau_split_conformal_quantile assertion is analytically correct
    - calibrate_insufficient_per_language_samples returns ConformalInsufficientSamples with the under-represented language
    - calibrate_sanity_coverage_equals_stated empirical ∈ [0.88, 0.92]
  </verification>
</definition_of_done>

<pseudo_code>
pub fn non_conformity_score(predicted: &[f32], actual: &[f32]) -> Result<f32, MejepaInferError> {
    if predicted.len() != actual.len() {
        return Err(MejepaInferError::DimMismatch { expected: predicted.len(), actual: actual.len() });
    }
    if predicted.iter().any(|x| !x.is_finite()) || actual.iter().any(|x| !x.is_finite()) {
        return Err(MejepaInferError::NanDetected { field: "non_conformity_input".into(), witness_chain_offset: 0 });
    }
    let s = predicted.iter().zip(actual.iter())
        .map(|(p, a)| (p - a).abs())
        .fold(0.0_f32, f32::max);
    Ok(s)
}

pub fn calibrate_tau(scores: &[f32], alpha: f32) -> Result<f32, MejepaInferError> {
    if scores.is_empty() {
        return Err(MejepaInferError::ConformalInsufficientSamples { required: 1, observed: 0, language: Language::Python });
    }
    if !alpha.is_finite() || !(0.01..=0.50).contains(&alpha) {
        // bound check; we have no AlphaOutOfRange variant so reuse DimMismatch with sentinel? — use ConformalInsufficientSamples with required = usize::MAX as a sentinel
        return Err(MejepaInferError::ConformalInsufficientSamples { required: usize::MAX, observed: 0, language: Language::Python });
    }
    let mut sorted: Vec<f32> = scores.iter().copied().collect();
    if sorted.iter().any(|x| !x.is_finite()) {
        return Err(MejepaInferError::NanDetected { field: "scores".into(), witness_chain_offset: 0 });
    }
    sorted.sort_by(|a, b| a.total_cmp(b));
    let n = sorted.len();
    let target = ((n as f32 + 1.0) * (1.0 - alpha)).ceil() as usize;
    let idx = target.saturating_sub(1).min(n - 1);
    Ok(sorted[idx])
}

pub fn calibrate(examples, alpha, min_samples_per_stratum, sigma_squared, corpus_sha, embedder_versions)
    -> Result<CalibrationRecord, MejepaInferError>
{
    // Stratum check
    let mut counts: BTreeMap<Language, usize> = BTreeMap::new();
    for ex in examples { *counts.entry(ex.language).or_default() += 1; }
    for (lang, n) in &counts {
        if *n < min_samples_per_stratum {
            return Err(MejepaInferError::ConformalInsufficientSamples {
                required: min_samples_per_stratum, observed: *n, language: *lang,
            });
        }
    }
    // Scores
    let scores: Vec<f32> = examples.iter()
        .map(|e| non_conformity_score(&e.predicted_test_pass, &e.actual_test_pass))
        .collect::<Result<Vec<_>, _>>()?;
    let tau = calibrate_tau(&scores, alpha)?;
    let count_le = scores.iter().filter(|s| **s <= tau).count();
    let empirical_coverage = count_le as f32 / scores.len() as f32;
    Ok(CalibrationRecord {
        version: ulid::Ulid::new().to_string(),
        alpha, tau, sigma_squared,
        calibration_corpus_sha: corpus_sha,
        embedder_versions,
        frozen_at: chrono::Utc::now().timestamp(),
        num_samples: examples.len(),
        empirical_coverage,
    })
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/conformal.rs">non_conformity_score + calibrate_tau + calibrate + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/lib.rs">add `pub mod conformal;`</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">add `chrono` dep (for Utc timestamp)</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa conformal shows ≥ 8 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>non_conformity_score_max_test_correct passes</criterion>
  <criterion>calibrate_tau is the (⌈(N+1)(1-α)⌉/N)-quantile</criterion>
  <criterion>calibrate_insufficient_per_language_samples returns ConformalInsufficientSamples</criterion>
  <criterion>calibrate_sanity_coverage_equals_stated within [0.88, 0.92] for alpha=0.10</criterion>
  <criterion>NaN in inputs rejected with NanDetected</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa conformal</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
