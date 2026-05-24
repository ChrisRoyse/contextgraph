use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::calibration_types::{complete_per_slot_sigma_squared, CalibrationRecord};
use crate::heal::cf::{
    encode_active_pointer_key, encode_calibration_history_record_key, encode_value,
    ActivePointerValue, CF_MEJEPA_ACTIVE_POINTERS,
};
use crate::heal::errors::HealError;
use crate::heal::pipeline::SelfHealingPipeline;
use crate::types::Language;

pub const DEFAULT_CALIBRATION_ALPHA: f32 = 0.10;
pub const DEFAULT_CALIBRATION_WINDOW_SIZE: usize = 1000;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationExample {
    pub non_conformity_score: f32,
    pub ood_score: f32,
    pub frozen_at: i64,
}

impl CalibrationExample {
    pub fn try_new(
        non_conformity_score: f32,
        ood_score: f32,
        frozen_at: i64,
    ) -> Result<Self, HealError> {
        if !non_conformity_score.is_finite() || !(0.0..=1.0).contains(&non_conformity_score) {
            return Err(HealError::invalid(
                "calibration_example.non_conformity_score",
                "score must be finite in [0,1]",
            ));
        }
        if !ood_score.is_finite() || ood_score < 0.0 {
            return Err(HealError::invalid(
                "calibration_example.ood_score",
                "OOD score must be finite and non-negative",
            ));
        }
        Ok(Self {
            non_conformity_score,
            ood_score,
            frozen_at,
        })
    }
}

pub type HealCalibrationRecord = CalibrationRecord;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationWriter {
    pub sliding_window: VecDeque<CalibrationExample>,
    pub alpha: f32,
    pub last_record: Option<HealCalibrationRecord>,
    pub capacity: usize,
}

impl CalibrationWriter {
    pub fn try_new(alpha: f32, capacity: usize) -> Result<Self, HealError> {
        if !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
            return Err(HealError::invalid(
                "calibration_writer.alpha",
                "alpha must be in (0,1)",
            ));
        }
        if capacity == 0 {
            return Err(HealError::invalid(
                "calibration_writer.capacity",
                "capacity must be > 0",
            ));
        }
        Ok(Self {
            sliding_window: VecDeque::with_capacity(capacity),
            alpha,
            last_record: None,
            capacity,
        })
    }

    pub fn push(&mut self, example: CalibrationExample) {
        if self.sliding_window.len() == self.capacity {
            self.sliding_window.pop_front();
        }
        self.sliding_window.push_back(example);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationEvidence {
    pub observed: usize,
    pub required: usize,
    pub tau: Option<f32>,
    pub sigma_squared: Option<f32>,
    pub persisted: bool,
}

pub fn continuous_calibration_if_due(
    pipeline: &mut SelfHealingPipeline,
) -> Result<Option<HealCalibrationRecord>, HealError> {
    let counter = pipeline.status.lock().unwrap().observation_counter;
    if counter == 0 || !counter.is_multiple_of(pipeline.config.continuous_calibration_period) {
        return Ok(None);
    }
    let mut writer = pipeline.calibration_writer.lock().unwrap();
    if writer.sliding_window.len() < DEFAULT_CALIBRATION_WINDOW_SIZE {
        if let Ok(mut status) = pipeline.status.lock() {
            status.status_change = crate::heal::pipeline::StatusChange::Degraded;
        }
        return Err(HealError::ConformalInsufficientSamples {
            observed: writer.sliding_window.len(),
            required: DEFAULT_CALIBRATION_WINDOW_SIZE,
        });
    }
    let scores = writer
        .sliding_window
        .iter()
        .map(|example| example.non_conformity_score)
        .collect::<Vec<_>>();
    let ood = writer
        .sliding_window
        .iter()
        .map(|example| example.ood_score)
        .collect::<Vec<_>>();
    let tau = compute_conformal_tau(&scores, writer.alpha)?;
    let sigma_squared = binary_search_sigma_squared(&ood, &scores, 0.30, 64)?;
    let frozen_at = chrono::Utc::now().timestamp();
    let mut per_language_counts = BTreeMap::new();
    per_language_counts.insert(Language::Python, writer.sliding_window.len());
    let empirical_coverage =
        scores.iter().filter(|score| **score <= tau).count() as f32 / scores.len() as f32;
    let record = HealCalibrationRecord {
        version: format!("heal-calib-{frozen_at}-{}", writer.sliding_window.len()),
        alpha: writer.alpha,
        target_coverage: 1.0 - writer.alpha,
        tau,
        sigma_squared,
        empirical_coverage,
        min_samples_per_stratum: 1,
        sample_count: writer.sliding_window.len(),
        per_language_counts,
        per_slot_sigma_squared: Some(complete_per_slot_sigma_squared(sigma_squared)),
        corpus_sha: pipeline.corpus_sha,
        embedder_versions: BTreeMap::new(),
        frozen_at,
    };
    validate_heal_calibration_record(&record)?;
    let bytes = encode_value(&record)?;
    pipeline.storage.put_cf_readback(
        crate::heal::cf::CF_MEJEPA_CALIBRATION_HISTORY,
        &encode_calibration_history_record_key(record.frozen_at, &record.version),
        &bytes,
    )?;
    let active = ActivePointerValue::try_new(record.version.as_bytes().to_vec(), record.frozen_at)?;
    pipeline.storage.put_cf_readback(
        CF_MEJEPA_ACTIVE_POINTERS,
        &encode_active_pointer_key("active_calibration")?,
        &encode_value(&active)?,
    )?;
    writer.last_record = Some(record.clone());
    if let Ok(mut status) = pipeline.status.lock() {
        status.last_calibration_at = record.frozen_at;
        status.active_calibration_version = record.version.clone();
    }
    Ok(Some(record))
}

fn validate_heal_calibration_record(record: &CalibrationRecord) -> Result<(), HealError> {
    record
        .validate()
        .map_err(|err| HealError::invalid("calibration_record", err.to_string()))
}

pub fn compute_conformal_tau(scores: &[f32], alpha: f32) -> Result<f32, HealError> {
    if scores.is_empty() {
        return Err(HealError::ConformalInsufficientSamples {
            observed: 0,
            required: 1,
        });
    }
    if !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
        return Err(HealError::invalid(
            "calibration.alpha",
            "alpha must be in (0,1)",
        ));
    }
    let mut sorted = scores.to_vec();
    if sorted
        .iter()
        .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
    {
        return Err(HealError::invalid(
            "calibration.scores",
            "scores must be finite in [0,1]",
        ));
    }
    sorted.sort_by(f32::total_cmp);
    let n = sorted.len();
    let rank = (((n + 1) as f32) * (1.0 - alpha)).ceil() as usize;
    let idx = rank.saturating_sub(1).min(n - 1);
    Ok(sorted[idx])
}

pub fn binary_search_sigma_squared(
    ood_scores: &[f32],
    non_conformity_scores: &[f32],
    target_fraction: f32,
    max_iter: u32,
) -> Result<f32, HealError> {
    if ood_scores.is_empty() || ood_scores.len() != non_conformity_scores.len() {
        return Err(HealError::invalid(
            "calibration.ood_scores",
            "OOD and non-conformity vectors must be same non-empty length",
        ));
    }
    if !target_fraction.is_finite() || !(0.0..=1.0).contains(&target_fraction) {
        return Err(HealError::invalid(
            "calibration.target_fraction",
            "target_fraction must be in [0,1]",
        ));
    }
    let mut lo = 1e-6f32;
    let mut hi = 1_000.0f32;
    for _ in 0..max_iter.max(1) {
        let mid = (lo + hi) * 0.5;
        let frac = ood_scores
            .iter()
            .zip(non_conformity_scores)
            .filter(|(ood, score)| {
                ood.is_finite() && score.is_finite() && (**ood / mid + **score) < target_fraction
            })
            .count() as f32
            / ood_scores.len() as f32;
        if frac >= 0.90 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Ok(hi.max(1e-6))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_writer_rejects_alpha_zero_or_one() {
        assert!(CalibrationWriter::try_new(0.0, 1000).is_err());
        assert!(CalibrationWriter::try_new(1.0, 1000).is_err());
        assert!(CalibrationWriter::try_new(0.1, 1000).is_ok());
    }

    #[test]
    fn compute_tau_uses_conformal_quantile() {
        let scores = (0..100).map(|i| i as f32 / 100.0).collect::<Vec<_>>();
        let tau = compute_conformal_tau(&scores, 0.10).unwrap();
        assert!((tau - 0.9).abs() <= 0.02);
    }
}
