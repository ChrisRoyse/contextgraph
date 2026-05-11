use super::{non_empty, validate_finite, validate_unit, UtmlError, UtmlErrorCode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DivergenceMetric {
    Wasserstein1,
    MmdRbf,
    SymmetricKl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateTransferDiagnostic {
    pub metric: DivergenceMetric,
    pub divergence: f32,
    pub lambda: f32,
    pub transfer_score: f32,
    pub source_count: usize,
    pub target_count: usize,
}

pub fn compute_state_transfer(
    source_distribution: &[f32],
    target_distribution: &[f32],
    metric: DivergenceMetric,
    lambda: f32,
) -> Result<StateTransferDiagnostic, UtmlError> {
    validate_distribution("source_distribution", source_distribution)?;
    validate_distribution("target_distribution", target_distribution)?;
    if !lambda.is_finite() || lambda <= 0.0 {
        return Err(UtmlError::new(
            UtmlErrorCode::OutOfRange,
            format!("state transfer lambda must be finite and > 0; got {lambda}"),
        ));
    }
    let divergence = match metric {
        DivergenceMetric::Wasserstein1 => wasserstein_1(source_distribution, target_distribution)?,
        DivergenceMetric::MmdRbf => mmd_rbf(source_distribution, target_distribution)?,
        DivergenceMetric::SymmetricKl => symmetric_kl(source_distribution, target_distribution)?,
    };
    validate_finite("state_transfer.divergence", divergence)?;
    let transfer_score = (-divergence / lambda).exp().clamp(0.0, 1.0);
    validate_unit("state_transfer.transfer_score", transfer_score)?;
    Ok(StateTransferDiagnostic {
        metric,
        divergence,
        lambda,
        transfer_score,
        source_count: source_distribution.len(),
        target_count: target_distribution.len(),
    })
}

pub fn performance_deploy(
    empirical_performance: f32,
    diagnostic: &StateTransferDiagnostic,
) -> Result<f32, UtmlError> {
    validate_unit("empirical_performance", empirical_performance)?;
    validate_unit("state_transfer.transfer_score", diagnostic.transfer_score)?;
    Ok((empirical_performance * diagnostic.transfer_score).clamp(0.0, 1.0))
}

fn wasserstein_1(a: &[f32], b: &[f32]) -> Result<f32, UtmlError> {
    if a.len() != b.len() {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            format!(
                "wasserstein_1 requires equal lengths; got {} and {}",
                a.len(),
                b.len()
            ),
        ));
    }
    let mut left = a.to_vec();
    let mut right = b.to_vec();
    left.sort_by(f32::total_cmp);
    right.sort_by(f32::total_cmp);
    Ok(left
        .iter()
        .zip(&right)
        .map(|(x, y)| (x - y).abs())
        .sum::<f32>()
        / left.len() as f32)
}

fn mmd_rbf(a: &[f32], b: &[f32]) -> Result<f32, UtmlError> {
    let gamma = 1.0f32;
    let aa = kernel_mean(a, a, gamma)?;
    let bb = kernel_mean(b, b, gamma)?;
    let ab = kernel_mean(a, b, gamma)?;
    Ok((aa + bb - 2.0 * ab).max(0.0).sqrt())
}

fn symmetric_kl(a: &[f32], b: &[f32]) -> Result<f32, UtmlError> {
    if a.len() != b.len() {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            format!(
                "symmetric_kl requires equal lengths; got {} and {}",
                a.len(),
                b.len()
            ),
        ));
    }
    let eps = 1e-6f32;
    let sum_a = a.iter().map(|v| v.max(eps)).sum::<f32>();
    let sum_b = b.iter().map(|v| v.max(eps)).sum::<f32>();
    let mut kl_ab = 0.0f32;
    let mut kl_ba = 0.0f32;
    for (x, y) in a.iter().zip(b) {
        let px = x.max(eps) / sum_a;
        let py = y.max(eps) / sum_b;
        kl_ab += px * (px / py).ln();
        kl_ba += py * (py / px).ln();
    }
    Ok((kl_ab + kl_ba) / 2.0)
}

fn kernel_mean(a: &[f32], b: &[f32], gamma: f32) -> Result<f32, UtmlError> {
    let mut sum = 0.0f32;
    let mut count = 0usize;
    for x in a {
        for y in b {
            sum += (-gamma * (x - y).powi(2)).exp();
            count += 1;
        }
    }
    if count == 0 {
        return Err(UtmlError::new(
            UtmlErrorCode::EmptyInput,
            "kernel_mean received no pairs",
        ));
    }
    Ok(sum / count as f32)
}

fn validate_distribution(name: &str, values: &[f32]) -> Result<(), UtmlError> {
    non_empty(name, values)?;
    for (idx, value) in values.iter().enumerate() {
        validate_finite(&format!("{name}[{idx}]"), *value)?;
    }
    Ok(())
}
