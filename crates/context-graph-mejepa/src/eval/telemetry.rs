use super::error::{EvalError, EvalErrorCode};
use crate::system_cost::HealTickerTelemetrySnapshot;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const TRAINING_HOLDOUT_DRIFT_ALERT_CODE: &str = "MEJEPA_TRAINING_HOLDOUT_DRIFT";
pub const TRAINING_HOLDOUT_DRIFT_KL_THRESHOLD: f32 = 0.5;
pub const TRAINING_HOLDOUT_DRIFT_REQUIRED_BATCHES: u64 = 100;

const TRAINING_HOLDOUT_DISTRIBUTION_EPSILON: f64 = 1e-9;
const DISTRIBUTION_SUM_TOLERANCE: f32 = 0.001;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LatencyTelemetry {
    pub p50_ms: f32,
    pub p95_ms: f32,
    pub p99_ms: f32,
    pub max_ms: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionTelemetryWindow {
    pub window_id: String,
    pub captured_at_unix_ms: i64,
    pub corpus_sha: String,
    pub sample_count: usize,
    pub conformal_ece: f32,
    pub ood_auc: f32,
    pub gtau_pass_rate: f32,
    pub prediction_oracle_agreement: f32,
    pub ship_gate_target: f32,
    pub ship_gate_passed: bool,
    pub latency: LatencyTelemetry,
    pub vram_free_bytes: u64,
    pub vram_total_bytes: u64,
    pub vram_required_bytes: u64,
    pub heal_ticker_telemetry_total: HealTickerTelemetrySnapshot,
    pub training_holdout_distribution_drift: Option<TrainingHoldoutDistributionDrift>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingHoldoutDistributionDrift {
    pub kl_divergence: f32,
    pub threshold: f32,
    pub sustained_batch_count: u64,
    pub required_sustained_batches: u64,
    pub alert_fired: bool,
    pub alert_code: Option<String>,
    pub training_distribution: BTreeMap<String, f32>,
    pub holdout_distribution: BTreeMap<String, f32>,
}

impl ProductionTelemetryWindow {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.window_id.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "production telemetry window_id must be non-empty",
            ));
        }
        if self.window_id.chars().any(char::is_control) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "production telemetry window_id must be single-line text",
            ));
        }
        if self.corpus_sha.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "production telemetry corpus_sha must be non-empty",
            ));
        }
        if self.sample_count == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "production telemetry sample_count must be greater than zero",
            ));
        }
        validate_unit("conformal_ece", self.conformal_ece)?;
        validate_unit("ood_auc", self.ood_auc)?;
        validate_unit("gtau_pass_rate", self.gtau_pass_rate)?;
        validate_unit(
            "prediction_oracle_agreement",
            self.prediction_oracle_agreement,
        )?;
        validate_unit("ship_gate_target", self.ship_gate_target)?;
        let computed_ship_gate_passed = self.prediction_oracle_agreement >= self.ship_gate_target;
        if self.ship_gate_passed != computed_ship_gate_passed {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "production telemetry ship_gate_passed={} contradicts prediction_oracle_agreement={} and ship_gate_target={}",
                    self.ship_gate_passed, self.prediction_oracle_agreement, self.ship_gate_target
                ),
            ));
        }
        self.latency.validate()?;
        if self.vram_total_bytes == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "production telemetry vram_total_bytes must be greater than zero",
            ));
        }
        if self.vram_required_bytes == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "production telemetry vram_required_bytes must be greater than zero",
            ));
        }
        if self.vram_free_bytes > self.vram_total_bytes {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "production telemetry vram_free_bytes {} exceeds total {}",
                    self.vram_free_bytes, self.vram_total_bytes
                ),
            ));
        }
        if self.vram_required_bytes > self.vram_total_bytes {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "production telemetry vram_required_bytes {} exceeds total {}",
                    self.vram_required_bytes, self.vram_total_bytes
                ),
            ));
        }
        if let Some(drift) = &self.training_holdout_distribution_drift {
            drift.validate()?;
        }
        if self.source.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "production telemetry source must be non-empty",
            ));
        }
        Ok(())
    }
}

impl TrainingHoldoutDistributionDrift {
    pub fn validate(&self) -> Result<(), EvalError> {
        if !self.kl_divergence.is_finite() || self.kl_divergence < 0.0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "training_holdout_distribution_drift.kl_divergence must be finite and non-negative; got {}",
                    self.kl_divergence
                ),
            ));
        }
        if !self.threshold.is_finite() || self.threshold < 0.0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "training_holdout_distribution_drift.threshold must be finite and non-negative; got {}",
                    self.threshold
                ),
            ));
        }
        if self.required_sustained_batches == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "training_holdout_distribution_drift.required_sustained_batches must be greater than zero",
            ));
        }
        validate_probability_distribution(
            "training_holdout_distribution_drift.training_distribution",
            &self.training_distribution,
        )?;
        validate_probability_distribution(
            "training_holdout_distribution_drift.holdout_distribution",
            &self.holdout_distribution,
        )?;
        if self.training_distribution.keys().collect::<Vec<_>>()
            != self.holdout_distribution.keys().collect::<Vec<_>>()
        {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "training_holdout_distribution_drift distributions must use identical cell support",
            ));
        }
        let expected_alert = self.kl_divergence > self.threshold
            && self.sustained_batch_count >= self.required_sustained_batches;
        if self.alert_fired != expected_alert {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "training_holdout_distribution_drift.alert_fired={} contradicts KL={} threshold={} sustained={} required={}",
                    self.alert_fired,
                    self.kl_divergence,
                    self.threshold,
                    self.sustained_batch_count,
                    self.required_sustained_batches
                ),
            ));
        }
        match (self.alert_fired, self.alert_code.as_deref()) {
            (true, Some(TRAINING_HOLDOUT_DRIFT_ALERT_CODE)) => Ok(()),
            (true, Some(code)) => Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "training_holdout_distribution_drift.alert_code must be {TRAINING_HOLDOUT_DRIFT_ALERT_CODE}; got {code}"
                ),
            )),
            (true, None) => Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "training_holdout_distribution_drift.alert_code is required when alert_fired=true",
            )),
            (false, None) => Ok(()),
            (false, Some(code)) => Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "training_holdout_distribution_drift.alert_code must be absent when alert_fired=false; got {code}"
                ),
            )),
        }
    }
}

pub fn training_holdout_distribution_drift(
    training_counts: &BTreeMap<String, u64>,
    holdout_counts: &BTreeMap<String, u64>,
    sustained_batch_count: u64,
) -> Result<TrainingHoldoutDistributionDrift, EvalError> {
    validate_count_distribution("training_counts", training_counts)?;
    validate_count_distribution("holdout_counts", holdout_counts)?;

    let support = training_counts
        .keys()
        .chain(holdout_counts.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let training_distribution = normalized_distribution(training_counts, &support)?;
    let holdout_distribution = normalized_distribution(holdout_counts, &support)?;
    let mut kl_divergence = 0.0_f64;
    for cell in &support {
        let p = f64::from(*training_distribution.get(cell).ok_or_else(|| {
            EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("missing training distribution cell {cell}"),
            )
        })?);
        let q = f64::from(*holdout_distribution.get(cell).ok_or_else(|| {
            EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("missing holdout distribution cell {cell}"),
            )
        })?);
        kl_divergence += p * (p / q).ln();
    }
    let kl_divergence = kl_divergence.max(0.0) as f32;
    let alert_fired = kl_divergence > TRAINING_HOLDOUT_DRIFT_KL_THRESHOLD
        && sustained_batch_count >= TRAINING_HOLDOUT_DRIFT_REQUIRED_BATCHES;
    let drift = TrainingHoldoutDistributionDrift {
        kl_divergence,
        threshold: TRAINING_HOLDOUT_DRIFT_KL_THRESHOLD,
        sustained_batch_count,
        required_sustained_batches: TRAINING_HOLDOUT_DRIFT_REQUIRED_BATCHES,
        alert_fired,
        alert_code: if alert_fired {
            Some(TRAINING_HOLDOUT_DRIFT_ALERT_CODE.to_string())
        } else {
            None
        },
        training_distribution,
        holdout_distribution,
    };
    drift.validate()?;
    Ok(drift)
}

impl LatencyTelemetry {
    pub fn validate(&self) -> Result<(), EvalError> {
        for (name, value) in [
            ("latency.p50_ms", self.p50_ms),
            ("latency.p95_ms", self.p95_ms),
            ("latency.p99_ms", self.p99_ms),
            ("latency.max_ms", self.max_ms),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!("{name} must be finite and non-negative; got {value}"),
                ));
            }
        }
        if !(self.p50_ms <= self.p95_ms && self.p95_ms <= self.p99_ms && self.p99_ms <= self.max_ms)
        {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "latency percentiles must be monotonic; got p50={} p95={} p99={} max={}",
                    self.p50_ms, self.p95_ms, self.p99_ms, self.max_ms
                ),
            ));
        }
        Ok(())
    }
}

fn validate_unit(name: &str, value: f32) -> Result<(), EvalError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{name} must be finite and in [0,1]; got {value}"),
        ));
    }
    Ok(())
}

fn validate_count_distribution(
    name: &str,
    counts: &BTreeMap<String, u64>,
) -> Result<(), EvalError> {
    if counts.is_empty() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{name} must contain at least one cell"),
        ));
    }
    for (cell, count) in counts {
        validate_cell_key(name, cell)?;
        if *count == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("{name}.{cell} count must be greater than zero"),
            ));
        }
    }
    Ok(())
}

fn normalized_distribution(
    counts: &BTreeMap<String, u64>,
    support: &BTreeSet<String>,
) -> Result<BTreeMap<String, f32>, EvalError> {
    let total = counts.values().try_fold(0_u64, |acc, count| {
        acc.checked_add(*count).ok_or_else(|| {
            EvalError::new(
                EvalErrorCode::InvalidInput,
                "distribution count total overflowed u64",
            )
        })
    })?;
    if total == 0 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "distribution count total must be greater than zero",
        ));
    }
    let denominator = total as f64 + TRAINING_HOLDOUT_DISTRIBUTION_EPSILON * support.len() as f64;
    Ok(support
        .iter()
        .map(|cell| {
            let count = *counts.get(cell).unwrap_or(&0) as f64;
            (
                cell.clone(),
                ((count + TRAINING_HOLDOUT_DISTRIBUTION_EPSILON) / denominator) as f32,
            )
        })
        .collect())
}

fn validate_probability_distribution(
    name: &str,
    distribution: &BTreeMap<String, f32>,
) -> Result<(), EvalError> {
    if distribution.is_empty() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{name} must contain at least one cell"),
        ));
    }
    let mut sum = 0.0_f32;
    for (cell, probability) in distribution {
        validate_cell_key(name, cell)?;
        if !probability.is_finite() || !(0.0..=1.0).contains(probability) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("{name}.{cell} must be finite and in [0,1]; got {probability}"),
            ));
        }
        sum += *probability;
    }
    if (sum - 1.0).abs() > DISTRIBUTION_SUM_TOLERANCE {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{name} probabilities must sum to 1.0; got {sum}"),
        ));
    }
    Ok(())
}

fn validate_cell_key(name: &str, cell: &str) -> Result<(), EvalError> {
    if cell.trim().is_empty() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{name} contains an empty cell key"),
        ));
    }
    if cell.chars().any(char::is_control) {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{name}.{cell} must be single-line text"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_cell_counts(left: u64, right: u64) -> BTreeMap<String, u64> {
        BTreeMap::from([
            ("known_good::python".to_string(), left),
            ("compile_error::python".to_string(), right),
        ])
    }

    #[test]
    fn matched_training_holdout_distribution_does_not_alert() {
        let drift = training_holdout_distribution_drift(
            &two_cell_counts(50, 50),
            &two_cell_counts(50, 50),
            100,
        )
        .expect("compute drift");
        assert!(drift.kl_divergence < 0.000001);
        assert!(!drift.alert_fired);
        assert_eq!(drift.alert_code, None);
    }

    #[test]
    fn skewed_training_holdout_distribution_alerts_after_required_batches() {
        let drift = training_holdout_distribution_drift(
            &two_cell_counts(99, 1),
            &two_cell_counts(50, 50),
            TRAINING_HOLDOUT_DRIFT_REQUIRED_BATCHES,
        )
        .expect("compute drift");
        assert!(drift.kl_divergence > TRAINING_HOLDOUT_DRIFT_KL_THRESHOLD);
        assert!(drift.alert_fired);
        assert_eq!(
            drift.alert_code.as_deref(),
            Some(TRAINING_HOLDOUT_DRIFT_ALERT_CODE)
        );
    }

    #[test]
    fn skewed_training_holdout_distribution_waits_for_sustain_window() {
        let drift = training_holdout_distribution_drift(
            &two_cell_counts(99, 1),
            &two_cell_counts(50, 50),
            TRAINING_HOLDOUT_DRIFT_REQUIRED_BATCHES - 1,
        )
        .expect("compute drift");
        assert!(drift.kl_divergence > TRAINING_HOLDOUT_DRIFT_KL_THRESHOLD);
        assert!(!drift.alert_fired);
        assert_eq!(drift.alert_code, None);
    }

    #[test]
    fn empty_training_holdout_distribution_fails_closed() {
        let err = training_holdout_distribution_drift(
            &BTreeMap::new(),
            &two_cell_counts(50, 50),
            TRAINING_HOLDOUT_DRIFT_REQUIRED_BATCHES,
        )
        .expect_err("empty distribution must fail");
        assert_eq!(err.code(), "MEJEPA_EVAL_INVALID_INPUT");
    }
}
