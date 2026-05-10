use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::heal::cf::{
    ActivePointerValue, CF_MEJEPA_ACTIVE_POINTERS, encode_active_pointer_key,
    encode_calibration_history_key, encode_value,
};
use crate::heal::errors::HealError;
use crate::heal::pipeline::SelfHealingPipeline;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HealCalibrationRecord {
    pub tau: f32,
    pub sigma_squared: f32,
    pub frozen_at: i64,
    pub corpus_sha: [u8; 32],
    pub embedder_versions_sha: [u8; 32],
    pub num_samples: usize,
    pub calibration_version: String,
}

impl HealCalibrationRecord {
    pub fn try_new(
        tau: f32,
        sigma_squared: f32,
        frozen_at: i64,
        corpus_sha: [u8; 32],
        embedder_versions_sha: [u8; 32],
        num_samples: usize,
    ) -> Result<Self, HealError> {
        if !tau.is_finite() || !(0.0..=1.0).contains(&tau) {
            return Err(HealError::invalid("calibration_record.tau", "tau must be in [0,1]"));
        }
        if !sigma_squared.is_finite() || sigma_squared <= 0.0 {
            return Err(HealError::invalid(
                "calibration_record.sigma_squared",
                "sigma_squared must be positive",
            ));
        }
        if num_samples == 0 {
            return Err(HealError::invalid(
                "calibration_record.num_samples",
                "num_samples must be > 0",
            ));
        }
        let calibration_version = format!("heal-calib-{}-{num_samples}", frozen_at);
        Ok(Self {
            tau,
            sigma_squared,
            frozen_at,
            corpus_sha,
            embedder_versions_sha,
            num_samples,
            calibration_version,
        })
    }
}

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
    let record = HealCalibrationRecord::try_new(
        tau,
        sigma_squared,
        chrono::Utc::now().timestamp(),
        pipeline.corpus_sha,
        pipeline.embedder_versions_sha,
        writer.sliding_window.len(),
    )?;
    let bytes = encode_value(&record)?;
    pipeline.storage.put_cf_readback(
        crate::heal::cf::CF_MEJEPA_CALIBRATION_HISTORY,
        &encode_calibration_history_key(record.frozen_at),
        &bytes,
    )?;
    let active = ActivePointerValue::try_new(
        record.calibration_version.as_bytes().to_vec(),
        record.frozen_at,
    )?;
    pipeline.storage.put_cf_readback(
        CF_MEJEPA_ACTIVE_POINTERS,
        &encode_active_pointer_key("active_calibration")?,
        &encode_value(&active)?,
    )?;
    writer.last_record = Some(record.clone());
    if let Ok(mut status) = pipeline.status.lock() {
        status.last_calibration_at = record.frozen_at;
        status.active_calibration_version = record.calibration_version.clone();
    }
    Ok(Some(record))
}

pub fn compute_conformal_tau(scores: &[f32], alpha: f32) -> Result<f32, HealError> {
    if scores.is_empty() {
        return Err(HealError::ConformalInsufficientSamples {
            observed: 0,
            required: 1,
        });
    }
    if !alpha.is_finite() || alpha <= 0.0 || alpha >= 1.0 {
        return Err(HealError::invalid("calibration.alpha", "alpha must be in (0,1)"));
    }
    let mut sorted = scores.to_vec();
    if sorted.iter().any(|value| !value.is_finite() || !(0.0..=1.0).contains(value)) {
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
