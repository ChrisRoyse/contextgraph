use std::collections::BTreeSet;

use crate::heal::errors::HealError;
use crate::heal::plasticity::{GsnrDistribution, PlasticityConfig, PlasticityRuntimeState};

#[derive(Debug, Clone)]
pub(crate) struct PlasticityMetrics {
    pub(crate) sample_count: usize,
    pub(crate) gradient_sample_count: usize,
    pub(crate) dormancy_fraction: f32,
    pub(crate) dormant_unit_count: usize,
    pub(crate) dormant_units: Vec<usize>,
    pub(crate) grad_cov_effective_rank: f32,
    pub(crate) grad_cov_rank_threshold: f32,
    pub(crate) gsnr_distribution: GsnrDistribution,
    pub(crate) utilities: Vec<(usize, f32)>,
}

#[derive(Debug, Clone)]
pub(crate) struct SelectedReinit {
    pub(crate) reinit_units: Vec<usize>,
    pub(crate) ewc_blocked_reinit_budget: usize,
}

pub(crate) fn measure_plasticity(
    activation_window: &[Vec<f32>],
    gradient_window: &[Vec<f32>],
    weights: &[f32],
    config: PlasticityConfig,
) -> Result<PlasticityMetrics, HealError> {
    let parameter_count = weights.len();
    if parameter_count == 0 {
        return Err(HealError::invalid(
            "plasticity.parameter_count",
            "parameter_count must be > 0",
        ));
    }
    let activation_stats =
        activation_means_and_dormant_units(activation_window, parameter_count, config)?;
    let gradient_stats = gradient_rank_and_gsnr(gradient_window, parameter_count)?;
    let utilities = unit_utilities(
        &activation_stats.mean_abs_activation,
        &gradient_stats.gradient_rms,
        weights,
    );
    Ok(PlasticityMetrics {
        sample_count: activation_window.len(),
        gradient_sample_count: gradient_window.len(),
        dormancy_fraction: activation_stats.dormancy_fraction,
        dormant_unit_count: activation_stats.dormant_units.len(),
        dormant_units: activation_stats.dormant_units,
        grad_cov_effective_rank: gradient_stats.effective_rank,
        grad_cov_rank_threshold: config.rank_threshold(parameter_count),
        gsnr_distribution: gradient_stats.gsnr_distribution,
        utilities,
    })
}

pub(crate) fn select_reinit_candidates(
    metrics: &PlasticityMetrics,
    protected_units: &[usize],
    reinit_rate: f32,
    config: PlasticityConfig,
) -> SelectedReinit {
    let max_budget =
        ((metrics.utilities.len() as f32 * config.max_reinit_rate).ceil() as usize).max(1);
    let budget =
        ((metrics.utilities.len() as f32 * reinit_rate).ceil() as usize).clamp(1, max_budget);
    let protected = protected_units.iter().copied().collect::<BTreeSet<_>>();
    let dormant = metrics
        .dormant_units
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let rank_trigger = metrics.grad_cov_effective_rank < metrics.grad_cov_rank_threshold;
    let mut reinit_units = Vec::new();
    let mut ewc_blocked_reinit_budget = 0usize;
    for (unit, _utility) in &metrics.utilities {
        if !rank_trigger && !dormant.contains(unit) {
            continue;
        }
        if protected.contains(unit) {
            ewc_blocked_reinit_budget += 1;
            continue;
        }
        reinit_units.push(*unit);
        if reinit_units.len() == budget {
            break;
        }
    }
    SelectedReinit {
        reinit_units,
        ewc_blocked_reinit_budget,
    }
}

pub(crate) fn push_gradient_window(
    state: &mut PlasticityRuntimeState,
    gradient: &[f32],
    max_rows: usize,
) {
    state.gradient_window.push(gradient.to_vec());
    if state.gradient_window.len() > max_rows {
        let remove_count = state.gradient_window.len() - max_rows;
        state.gradient_window.drain(0..remove_count);
    }
}

pub(crate) fn validate_gradient(gradient: &[f32], parameter_count: usize) -> Result<(), HealError> {
    if gradient.len() != parameter_count {
        return Err(HealError::invalid(
            "plasticity.gradient",
            format!(
                "gradient len {} != parameter count {parameter_count}",
                gradient.len()
            ),
        ));
    }
    if gradient.iter().any(|value| !value.is_finite()) {
        return Err(HealError::BatchNan {
            component: "plasticity.gradient".to_string(),
            witness_chain_offset: 0,
        });
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct ActivationStats {
    mean_abs_activation: Vec<f32>,
    dormant_units: Vec<usize>,
    dormancy_fraction: f32,
}

fn activation_means_and_dormant_units(
    activation_window: &[Vec<f32>],
    parameter_count: usize,
    config: PlasticityConfig,
) -> Result<ActivationStats, HealError> {
    if activation_window.is_empty() {
        return Ok(ActivationStats {
            mean_abs_activation: vec![1.0; parameter_count],
            dormant_units: Vec::new(),
            dormancy_fraction: 0.0,
        });
    }
    let mut mean_abs_activation = vec![0.0f32; parameter_count];
    let mut near_zero_counts = vec![0usize; parameter_count];
    for (row_idx, row) in activation_window.iter().enumerate() {
        if row.is_empty() {
            return Err(HealError::invalid(
                "plasticity.activation_window",
                format!("activation row {row_idx} is empty"),
            ));
        }
        // Activation samples can be embedder-width while the predictor is wider.
        // Project deterministically so the predictor-wide CBP gate remains total.
        for unit in 0..parameter_count {
            let value = row[unit % row.len()];
            if !value.is_finite() {
                return Err(HealError::BatchNan {
                    component: format!("plasticity.activation_window[{row_idx}][{unit}]"),
                    witness_chain_offset: 0,
                });
            }
            let abs = value.abs();
            mean_abs_activation[unit] += abs;
            if abs <= config.activation_threshold {
                near_zero_counts[unit] += 1;
            }
        }
    }
    let sample_count = activation_window.len();
    for value in &mut mean_abs_activation {
        *value /= sample_count as f32;
    }
    let required_near_zero_count =
        ((sample_count as f32 * config.dormant_batch_fraction).ceil() as usize).max(1);
    let dormant_units = near_zero_counts
        .iter()
        .enumerate()
        .filter_map(|(idx, count)| (*count >= required_near_zero_count).then_some(idx))
        .collect::<Vec<_>>();
    let dormancy_fraction = dormant_units.len() as f32 / parameter_count as f32;
    Ok(ActivationStats {
        mean_abs_activation,
        dormant_units,
        dormancy_fraction,
    })
}

#[derive(Debug, Clone)]
struct GradientStats {
    effective_rank: f32,
    gradient_rms: Vec<f32>,
    gsnr_distribution: GsnrDistribution,
}

fn gradient_rank_and_gsnr(
    gradient_window: &[Vec<f32>],
    parameter_count: usize,
) -> Result<GradientStats, HealError> {
    if gradient_window.is_empty() {
        return Ok(GradientStats {
            effective_rank: parameter_count as f32,
            gradient_rms: vec![1.0; parameter_count],
            gsnr_distribution: zero_gsnr_distribution(),
        });
    }
    let mut mean = vec![0.0f32; parameter_count];
    let mut mean_sq = vec![0.0f32; parameter_count];
    for (row_idx, row) in gradient_window.iter().enumerate() {
        validate_gradient(row, parameter_count).map_err(|err| {
            HealError::invalid(
                "plasticity.gradient_window",
                format!("gradient row {row_idx} invalid: {err}"),
            )
        })?;
        for (idx, value) in row.iter().enumerate() {
            mean[idx] += *value;
            mean_sq[idx] += *value * *value;
        }
    }
    let sample_count = gradient_window.len() as f32;
    let mut variances = Vec::with_capacity(parameter_count);
    let mut gradient_rms = Vec::with_capacity(parameter_count);
    let mut gsnr = Vec::with_capacity(parameter_count);
    for idx in 0..parameter_count {
        mean[idx] /= sample_count;
        mean_sq[idx] /= sample_count;
        let variance = (mean_sq[idx] - mean[idx] * mean[idx]).max(0.0);
        variances.push(variance);
        gradient_rms.push(mean_sq[idx].sqrt());
        let signal = mean[idx] * mean[idx];
        let ratio = if variance <= 1e-12 {
            if signal <= 1e-12 {
                0.0
            } else {
                1.0e9
            }
        } else {
            (signal / variance).min(1.0e9)
        };
        gsnr.push(ratio);
    }
    let var_sum = variances.iter().sum::<f32>();
    let var_sq_sum = variances.iter().map(|value| value * value).sum::<f32>();
    let effective_rank = if var_sum <= 1e-12 || var_sq_sum <= 1e-12 {
        0.0
    } else {
        (var_sum * var_sum / var_sq_sum).min(parameter_count as f32)
    };
    Ok(GradientStats {
        effective_rank,
        gradient_rms,
        gsnr_distribution: gsnr_distribution(gsnr),
    })
}

fn unit_utilities(
    mean_abs_activation: &[f32],
    gradient_rms: &[f32],
    weights: &[f32],
) -> Vec<(usize, f32)> {
    let mut utilities = (0..weights.len())
        .map(|idx| {
            let utility =
                mean_abs_activation[idx] + gradient_rms[idx] * 0.1 + weights[idx].abs() * 0.01;
            (idx, utility)
        })
        .collect::<Vec<_>>();
    utilities.sort_by(|left, right| {
        left.1
            .partial_cmp(&right.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    utilities
}

fn gsnr_distribution(mut values: Vec<f32>) -> GsnrDistribution {
    if values.is_empty() {
        return zero_gsnr_distribution();
    }
    values.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let p50 = percentile(&values, 0.50);
    let p90 = percentile(&values, 0.90);
    GsnrDistribution {
        min: values[0],
        p50,
        p90,
        max: values[values.len() - 1],
        low_signal_count: values.iter().filter(|value| **value < 1.0).count(),
    }
}

fn percentile(values: &[f32], p: f32) -> f32 {
    let idx = ((values.len().saturating_sub(1)) as f32 * p).round() as usize;
    values[idx.min(values.len().saturating_sub(1))]
}

fn zero_gsnr_distribution() -> GsnrDistribution {
    GsnrDistribution {
        min: 0.0,
        p50: 0.0,
        p90: 0.0,
        max: 0.0,
        low_signal_count: 0,
    }
}
