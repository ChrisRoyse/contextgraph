use crate::error::MejepaInferError;

pub fn compute_ood_score(
    predicted: &[f32],
    target: &[f32],
    sigma_squared: f32,
) -> Result<f32, MejepaInferError> {
    if predicted.len() != target.len() {
        return Err(MejepaInferError::DimMismatch {
            expected: target.len(),
            actual: predicted.len(),
            context: "compute_ood_score predicted/target len mismatch".to_string(),
        });
    }
    if predicted.is_empty() {
        return Err(MejepaInferError::DimMismatch {
            expected: 1,
            actual: 0,
            context: "compute_ood_score requires non-empty vectors".to_string(),
        });
    }
    if !sigma_squared.is_finite() || sigma_squared <= 0.0 {
        return Err(MejepaInferError::InvalidInput {
            field: "sigma_squared".to_string(),
            detail: format!("sigma_squared must be finite and > 0; got {sigma_squared}"),
        });
    }
    let norm_sq = squared_l2(predicted, target)?;
    Ok(ood_score_from_norm_sq(norm_sq, sigma_squared))
}

pub fn ood_score_from_norm_sq(norm_sq: f32, sigma_squared: f32) -> f32 {
    if !norm_sq.is_finite() || !sigma_squared.is_finite() || sigma_squared <= 0.0 {
        return f32::NAN;
    }
    (1.0 - (-norm_sq / sigma_squared).exp()).clamp(0.0, 1.0)
}

pub fn tune_sigma_squared(
    norm_sq_per_example: &[f32],
    target_mean_ood: f32,
) -> Result<f32, MejepaInferError> {
    if norm_sq_per_example.is_empty() {
        return Err(MejepaInferError::DimMismatch {
            expected: 1,
            actual: 0,
            context: "tune_sigma_squared requires non-empty residual norms".to_string(),
        });
    }
    if !target_mean_ood.is_finite() || target_mean_ood <= 0.0 || target_mean_ood >= 1.0 {
        return Err(MejepaInferError::InvalidInput {
            field: "target_mean_ood".to_string(),
            detail: format!("target_mean_ood must be in (0, 1); got {target_mean_ood}"),
        });
    }
    let mut max_norm = 0.0f32;
    for (idx, norm_sq) in norm_sq_per_example.iter().enumerate() {
        if !norm_sq.is_finite() || *norm_sq < 0.0 {
            return Err(MejepaInferError::NanDetected {
                nan_source: "norm_sq_per_example".to_string(),
                detail: format!("norm_sq_per_example[{idx}] is invalid: {norm_sq}"),
            });
        }
        max_norm = max_norm.max(*norm_sq);
    }
    if max_norm == 0.0 {
        return Ok(1.0);
    }
    let mut lo = f32::MIN_POSITIVE;
    let mut hi = max_norm / (-(1.0 - target_mean_ood).ln()).max(1e-6);
    hi = hi.max(1e-6);
    while mean_score(norm_sq_per_example, hi) > target_mean_ood {
        hi *= 2.0;
        if !hi.is_finite() {
            return Err(MejepaInferError::NanDetected {
                nan_source: "sigma_squared".to_string(),
                detail: "sigma_squared search overflowed".to_string(),
            });
        }
    }
    for _ in 0..96 {
        let mid = lo + (hi - lo) * 0.5;
        if mean_score(norm_sq_per_example, mid) > target_mean_ood {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Ok(hi.max(f32::MIN_POSITIVE))
}

pub fn separation_auc(in_dist_scores: &[f32], ood_scores: &[f32]) -> Result<f32, MejepaInferError> {
    if in_dist_scores.is_empty() || ood_scores.is_empty() {
        return Err(MejepaInferError::DimMismatch {
            expected: 1,
            actual: 0,
            context: "separation_auc requires non-empty in-distribution and OOD scores".to_string(),
        });
    }
    let mut wins = 0.0f64;
    let mut total = 0.0f64;
    for (i, in_score) in in_dist_scores.iter().enumerate() {
        validate_score("in_dist_scores", i, *in_score)?;
        for (j, ood_score) in ood_scores.iter().enumerate() {
            validate_score("ood_scores", j, *ood_score)?;
            wins += if ood_score > in_score {
                1.0
            } else if (*ood_score - *in_score).abs() <= f32::EPSILON {
                0.5
            } else {
                0.0
            };
            total += 1.0;
        }
    }
    Ok((wins / total) as f32)
}

pub fn squared_l2(left: &[f32], right: &[f32]) -> Result<f32, MejepaInferError> {
    if left.len() != right.len() {
        return Err(MejepaInferError::DimMismatch {
            expected: right.len(),
            actual: left.len(),
            context: "squared_l2 len mismatch".to_string(),
        });
    }
    let mut total = 0.0f32;
    for (idx, (l, r)) in left.iter().zip(right.iter()).enumerate() {
        if !l.is_finite() || !r.is_finite() {
            return Err(MejepaInferError::NanDetected {
                nan_source: "squared_l2".to_string(),
                detail: format!("non-finite at dim {idx}: left={l} right={r}"),
            });
        }
        let diff = l - r;
        total += diff * diff;
    }
    Ok(total)
}

fn mean_score(norm_sq: &[f32], sigma_squared: f32) -> f32 {
    norm_sq
        .iter()
        .map(|value| ood_score_from_norm_sq(*value, sigma_squared))
        .sum::<f32>()
        / norm_sq.len() as f32
}

fn validate_score(name: &str, idx: usize, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(MejepaInferError::NanDetected {
            nan_source: name.to_string(),
            detail: format!("{name}[{idx}] must be finite and in [0, 1]; got {value}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ood_score_uses_expected_formula() {
        let score = compute_ood_score(&[1.0, 1.0], &[0.0, 1.0], 2.0).unwrap();
        let expected = 1.0 - (-0.5f32).exp();
        assert!((score - expected).abs() < 1e-6);
    }

    #[test]
    fn auc_is_pairwise_rank_probability() {
        let auc = separation_auc(&[0.1, 0.2], &[0.8, 0.9]).unwrap();
        assert_eq!(auc, 1.0);
    }

    #[test]
    fn sigma_tuning_hits_mean_budget() {
        let norms = [0.1, 0.2, 0.3, 0.4];
        let sigma = tune_sigma_squared(&norms, 0.30).unwrap();
        let mean = norms
            .iter()
            .map(|v| ood_score_from_norm_sq(*v, sigma))
            .sum::<f32>()
            / norms.len() as f32;
        assert!((mean - 0.30).abs() < 1e-3, "mean={mean}");
    }
}
