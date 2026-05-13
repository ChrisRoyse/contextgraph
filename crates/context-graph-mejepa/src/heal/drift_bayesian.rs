use serde::{Deserialize, Serialize};

use crate::heal::drift::DriftSeverity;
use crate::heal::errors::HealError;

const JEFFREYS_ALPHA: f32 = 0.5;
const JEFFREYS_BETA: f32 = 0.5;
const Z_95: f32 = 1.959_964;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BetaPosteriorDecision {
    pub successes: u64,
    pub failures: u64,
    pub sample_count: u64,
    pub alpha: f32,
    pub beta: f32,
    pub posterior_mean: f32,
    pub lower_95: f32,
    pub upper_95: f32,
    pub threshold: f32,
    pub severity: DriftSeverity,
    pub fires: bool,
}

impl BetaPosteriorDecision {
    pub fn drift_detected(&self) -> bool {
        matches!(
            self.severity,
            DriftSeverity::Soft | DriftSeverity::Hard | DriftSeverity::Catastrophic
        )
    }
}

pub fn jeffreys_posterior_decision(
    successes: u64,
    failures: u64,
    threshold: f32,
) -> Result<BetaPosteriorDecision, HealError> {
    if successes == 0 && failures == 0 {
        return Err(HealError::invalid(
            "bayesian_drift.sample_count",
            "at least one Bernoulli observation is required",
        ));
    }
    if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
        return Err(HealError::invalid(
            "bayesian_drift.threshold",
            format!("threshold must be finite in [0,1], got {threshold}"),
        ));
    }
    let alpha = successes as f32 + JEFFREYS_ALPHA;
    let beta = failures as f32 + JEFFREYS_BETA;
    let total = alpha + beta;
    let mean = alpha / total;
    let variance = (alpha * beta) / (total * total * (total + 1.0));
    let radius = Z_95 * variance.sqrt();
    let lower = (mean - radius).clamp(0.0, 1.0);
    let upper = (mean + radius).clamp(0.0, 1.0);
    let severity = if upper < threshold - 0.10 {
        DriftSeverity::Catastrophic
    } else if upper < threshold - 0.04 {
        DriftSeverity::Hard
    } else if upper < threshold {
        DriftSeverity::Soft
    } else {
        DriftSeverity::Healthy
    };
    let fires = matches!(
        severity,
        DriftSeverity::Soft | DriftSeverity::Hard | DriftSeverity::Catastrophic
    );
    Ok(BetaPosteriorDecision {
        successes,
        failures,
        sample_count: successes + failures,
        alpha,
        beta,
        posterior_mean: mean,
        lower_95: lower,
        upper_95: upper,
        threshold,
        severity,
        fires,
    })
}

pub type BayesianDriftDecision = BetaPosteriorDecision;

pub fn jeffreys_posterior_below_threshold(
    successes: u64,
    trials: u64,
    threshold: f32,
    confidence: f32,
) -> Result<BayesianDriftDecision, HealError> {
    if trials == 0 {
        return Err(HealError::invalid(
            "bayesian_drift.trials",
            "trials must be greater than zero",
        ));
    }
    if successes > trials {
        return Err(HealError::invalid(
            "bayesian_drift.successes",
            format!("successes {successes} exceeds trials {trials}"),
        ));
    }
    if !confidence.is_finite() || !(0.5..1.0).contains(&confidence) {
        return Err(HealError::invalid(
            "bayesian_drift.confidence",
            format!("confidence must be finite in [0.5,1), got {confidence}"),
        ));
    }
    jeffreys_posterior_decision(successes, trials - successes, threshold)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jeffreys_posterior_flags_small_window_below_threshold() {
        let decision = jeffreys_posterior_decision(38, 12, 0.90).unwrap();
        assert!(decision.drift_detected());
        assert_eq!(decision.sample_count, 50);
        assert!(decision.upper_95 < 0.90);
    }

    #[test]
    fn jeffreys_posterior_rejects_empty_window() {
        assert_eq!(
            jeffreys_posterior_decision(0, 0, 0.9).unwrap_err().code(),
            "MEJEPA_HEAL_INVALID_STATE"
        );
    }
}
