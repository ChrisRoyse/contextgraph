use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::calibration_types::{complete_per_slot_sigma_squared, CalibrationRecord};
use crate::error::MejepaInferError;
use crate::types::{ConformalSet, EmbedderId, Language, OracleOutcome, TaskContext};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationExample {
    pub language: Language,
    pub predicted_test_pass: Vec<f32>,
    pub actual_test_pass: Vec<f32>,
}

// #623: `ColdStartConformalPolicy` struct and `cold_start_conformal_policy`
// function deleted as dead public API with contradictory invariant. The
// function returned `alpha = 1.0 - coverage_target` which, for
// `oracle_outcome_count < 100`, was `0.0` — and `calibrate_tau` below
// rejects `alpha <= 0.0`. Workspace grep showed zero callers outside
// this file's now-deleted unit tests. If a cold-start contract is
// needed later, file a new issue and reintroduce as a tagged enum
// (e.g. `ConformalPolicyDecision::ColdStart { ... } |
// ::Calibrated { alpha, tau }`) so the type system prevents
// `alpha=0.0` from reaching `calibrate_tau`.

pub fn non_conformity_score(
    predicted_test_pass: &[f32],
    actual_test_pass: &[f32],
) -> Result<f32, MejepaInferError> {
    if predicted_test_pass.len() != actual_test_pass.len() {
        return Err(MejepaInferError::DimMismatch {
            expected: actual_test_pass.len(),
            actual: predicted_test_pass.len(),
            context: "non_conformity_score predicted/actual len mismatch".to_string(),
        });
    }
    if predicted_test_pass.is_empty() {
        return Err(MejepaInferError::DimMismatch {
            expected: 1,
            actual: 0,
            context: "non_conformity_score requires at least one test".to_string(),
        });
    }
    let mut total = 0.0f32;
    for (idx, (predicted, actual)) in predicted_test_pass
        .iter()
        .zip(actual_test_pass.iter())
        .enumerate()
    {
        validate_probability(&format!("predicted_test_pass[{idx}]"), *predicted)?;
        validate_probability(&format!("actual_test_pass[{idx}]"), *actual)?;
        total += (predicted - actual).abs();
    }
    Ok((total / predicted_test_pass.len() as f32).clamp(0.0, 1.0))
}

pub fn calibrate_tau(scores: &[f32], alpha: f32) -> Result<f32, MejepaInferError> {
    if scores.is_empty() {
        return Err(MejepaInferError::ConformalInsufficientSamples {
            language: None,
            expected: 1,
            actual: 0,
        });
    }
    if !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
        return Err(MejepaInferError::InvalidInput {
            field: "alpha".to_string(),
            detail: format!("alpha must be finite and in (0, 1); got {alpha}"),
        });
    }
    let mut sorted = scores.to_vec();
    for (idx, score) in sorted.iter().enumerate() {
        if !score.is_finite() {
            return Err(MejepaInferError::NanDetected {
                nan_source: "calibration_score".to_string(),
                detail: format!("score[{idx}] is non-finite: {score}"),
            });
        }
    }
    sorted.sort_by(|a, b| a.total_cmp(b));
    let raw_idx = (((sorted.len() + 1) as f32) * (1.0 - alpha)).ceil() as isize - 1;
    let idx = raw_idx.clamp(0, sorted.len() as isize - 1) as usize;
    Ok(sorted[idx].clamp(0.0, 1.0))
}

pub fn calibrate(
    examples: &[CalibrationExample],
    alpha: f32,
    min_samples_per_stratum: usize,
    sigma_squared: f32,
    corpus_sha: [u8; 32],
    embedder_versions: BTreeMap<EmbedderId, String>,
) -> Result<CalibrationRecord, MejepaInferError> {
    if examples.is_empty() {
        return Err(MejepaInferError::ConformalInsufficientSamples {
            language: None,
            expected: 1,
            actual: 0,
        });
    }
    if min_samples_per_stratum == 0 {
        return Err(MejepaInferError::InvalidInput {
            field: "min_samples_per_stratum".to_string(),
            detail: "min_samples_per_stratum must be >= 1".to_string(),
        });
    }
    if !sigma_squared.is_finite() || sigma_squared <= 0.0 {
        return Err(MejepaInferError::InvalidInput {
            field: "sigma_squared".to_string(),
            detail: format!("sigma_squared must be finite and > 0; got {sigma_squared}"),
        });
    }
    let mut counts: BTreeMap<Language, usize> = BTreeMap::new();
    let mut scores = Vec::with_capacity(examples.len());
    for example in examples {
        *counts.entry(example.language).or_default() += 1;
        scores.push(non_conformity_score(
            &example.predicted_test_pass,
            &example.actual_test_pass,
        )?);
    }
    for (language, count) in &counts {
        if *count < min_samples_per_stratum {
            return Err(MejepaInferError::ConformalInsufficientSamples {
                language: Some(format!("{language:?}")),
                expected: min_samples_per_stratum,
                actual: *count,
            });
        }
    }
    let tau = calibrate_tau(&scores, alpha)?;
    let covered = scores.iter().filter(|score| **score <= tau).count();
    let empirical_coverage = covered as f32 / scores.len() as f32;
    let frozen_at = chrono::Utc::now().timestamp();
    let version = calibration_version(frozen_at, &scores, corpus_sha);
    let per_slot_sigma_squared = Some(complete_per_slot_sigma_squared(sigma_squared));
    let record = CalibrationRecord {
        version,
        alpha,
        target_coverage: 1.0 - alpha,
        tau,
        sigma_squared,
        empirical_coverage,
        min_samples_per_stratum,
        sample_count: examples.len(),
        per_language_counts: counts,
        per_slot_sigma_squared,
        corpus_sha,
        embedder_versions,
        frozen_at,
    };
    record.validate()?;
    Ok(record)
}

pub fn enumerate_candidate_outcomes(test_count: usize) -> Vec<(OracleOutcome, Vec<f32>)> {
    vec![
        (OracleOutcome::Pass, vec![1.0; test_count]),
        (OracleOutcome::Fail, vec![0.0; test_count]),
        (OracleOutcome::OutOfDistribution, vec![0.5; test_count]),
        (OracleOutcome::Abstain, vec![0.5; test_count]),
    ]
}

pub fn build_outcome_set(
    per_test_probs: &[f32],
    _context: &TaskContext,
    alpha: f32,
    tau: f32,
    multiplier: f32,
) -> Result<Vec<OracleOutcome>, MejepaInferError> {
    if per_test_probs.is_empty() {
        return Err(MejepaInferError::DimMismatch {
            expected: 1,
            actual: 0,
            context: "build_outcome_set requires at least one test probability".to_string(),
        });
    }
    for (idx, value) in per_test_probs.iter().enumerate() {
        validate_probability(&format!("per_test_probs[{idx}]"), *value)?;
    }
    if !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
        return Err(MejepaInferError::InvalidInput {
            field: "alpha".to_string(),
            detail: format!("alpha must be finite and in (0, 1); got {alpha}"),
        });
    }
    if !tau.is_finite() || !(0.0..=1.0).contains(&tau) {
        return Err(MejepaInferError::InvalidInput {
            field: "tau".to_string(),
            detail: format!("tau must be finite and in [0, 1]; got {tau}"),
        });
    }
    if !multiplier.is_finite() {
        return Err(MejepaInferError::NanDetected {
            nan_source: "multiplier".to_string(),
            detail: format!("multiplier is non-finite: {multiplier}"),
        });
    }
    let m = multiplier.clamp(0.10, 0.95);
    let tau_eff = (tau / m).clamp(0.0, 1.0);
    let mut scored = Vec::with_capacity(4);
    for (outcome, target) in enumerate_candidate_outcomes(per_test_probs.len()) {
        let score = non_conformity_score(per_test_probs, &target)?;
        scored.push((outcome, score));
    }
    scored.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    let kept = scored
        .iter()
        .filter(|(_, score)| *score <= tau_eff)
        .map(|(outcome, _)| *outcome)
        .collect::<Vec<_>>();
    if kept.is_empty() {
        return Err(MejepaInferError::OodRefuse {
            ood_score: 1.0,
            threshold: tau_eff,
            reason: "conformal outcome set is empty; refusing to invent a closest fallback"
                .to_string(),
        });
    }
    Ok(kept)
}

pub fn conformal_set(
    outcomes: Vec<OracleOutcome>,
    alpha: f32,
    tau: f32,
) -> Result<ConformalSet, MejepaInferError> {
    ConformalSet::try_new(outcomes, alpha, tau)
}

pub fn shannon_entropy_of_outcome_set(outcomes: &[OracleOutcome]) -> f32 {
    let unique = outcomes.iter().copied().collect::<BTreeSet<_>>();
    let n = unique.len() as f32;
    if n <= 1.0 {
        0.0
    } else {
        n.log2()
    }
}

fn calibration_version(frozen_at: i64, scores: &[f32], corpus_sha: [u8; 32]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(frozen_at.to_be_bytes());
    hasher.update(corpus_sha);
    for score in scores {
        hasher.update(score.to_le_bytes());
    }
    let digest = hasher.finalize();
    format!("infer-cal-{frozen_at}-{}", hex::encode(&digest[..8]))
}

fn validate_probability(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(MejepaInferError::NanDetected {
            nan_source: field.to_string(),
            detail: format!("{field} must be finite and in [0, 1]; got {value}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TaskEnvironment, TaskId, TestId};
    use std::path::PathBuf;

    fn context() -> TaskContext {
        TaskContext {
            task_id: TaskId("task".to_string()),
            session_id: [1; 16],
            language: Language::Python,
            problem_statement: "scenario: approve".to_string(),
            tests: vec![TestId("test".to_string())],
            environment: TaskEnvironment {
                repo_root: PathBuf::from("."),
                python_version: Some("3.11".to_string()),
                os: "linux".to_string(),
            },
            claim_graph: None,
            skill_citations: vec![],
        }
    }

    #[test]
    fn tau_uses_conformal_quantile() {
        let tau = calibrate_tau(&[0.1, 0.2, 0.3, 0.4], 0.10).unwrap();
        assert_eq!(tau, 0.4);
    }

    // #623: `cold_start_policy_widens_until_enough_oracle_outcomes_exist`
    // test deleted alongside the `cold_start_conformal_policy` function.

    #[test]
    fn sharp_prediction_returns_pass_only() {
        let outcomes = build_outcome_set(&[0.99, 0.98], &context(), 0.10, 0.10, 0.95).unwrap();
        assert_eq!(outcomes, vec![OracleOutcome::Pass]);
    }

    #[test]
    fn uncertain_prediction_returns_ood_and_abstain() {
        let outcomes = build_outcome_set(&[0.50, 0.50], &context(), 0.10, 0.10, 0.95).unwrap();
        assert_eq!(
            outcomes,
            vec![OracleOutcome::OutOfDistribution, OracleOutcome::Abstain]
        );
    }

    #[test]
    fn empty_candidate_result_errors_not_fallback() {
        let err = build_outcome_set(&[0.25], &context(), 0.10, 0.01, 0.95).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INFER_OOD_REFUSE");
    }

    #[test]
    fn entropy_matches_log2_unique_outcomes() {
        let entropy = shannon_entropy_of_outcome_set(&[
            OracleOutcome::Pass,
            OracleOutcome::Fail,
            OracleOutcome::OutOfDistribution,
            OracleOutcome::Abstain,
        ]);
        assert!((entropy - 2.0).abs() < 1e-6);
    }
}
