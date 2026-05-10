use std::sync::Arc;

use context_graph_mejepa_instruments::Panel;
use serde::{Deserialize, Serialize};

use crate::heal::cf::{CF_MEJEPA_HEAL_REPORTS, encode_heal_report_key, encode_value};
use crate::heal::errors::HealError;
use crate::heal::store::HealRocksStore;
use crate::types::OracleOutcome;

pub const DEFAULT_DISTILL_EMA_TAU: f32 = 0.996;
pub const DEFAULT_DISTILL_INFO_NCE_TEMPERATURE: f32 = 0.07;
pub const DEFAULT_SIGNAL_CLARITY_THRESHOLD: f32 = 0.5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OnlineDistiller {
    pub distill_handle_sha: [u8; 32],
    pub ema_tau: f32,
    pub quality_gate: bool,
    pub signal_clarity_threshold: f32,
    pub info_nce_temperature: f32,
}

impl OnlineDistiller {
    pub fn try_new(
        distill_handle_sha: [u8; 32],
        ema_tau: f32,
        quality_gate: bool,
        signal_clarity_threshold: f32,
        info_nce_temperature: f32,
    ) -> Result<Self, HealError> {
        if !ema_tau.is_finite() || ema_tau <= 0.0 || ema_tau >= 1.0 {
            return Err(HealError::invalid("distiller.ema_tau", "tau must be in (0,1)"));
        }
        if !signal_clarity_threshold.is_finite()
            || !(0.0..=1.0).contains(&signal_clarity_threshold)
        {
            return Err(HealError::invalid(
                "distiller.signal_clarity_threshold",
                "threshold must be in [0,1]",
            ));
        }
        if !info_nce_temperature.is_finite() || info_nce_temperature <= 0.0 {
            return Err(HealError::invalid(
                "distiller.info_nce_temperature",
                "temperature must be positive",
            ));
        }
        Ok(Self {
            distill_handle_sha,
            ema_tau,
            quality_gate,
            signal_clarity_threshold,
            info_nce_temperature,
        })
    }
}

impl Default for OnlineDistiller {
    fn default() -> Self {
        Self::try_new(
            [9; 32],
            DEFAULT_DISTILL_EMA_TAU,
            true,
            DEFAULT_SIGNAL_CLARITY_THRESHOLD,
            DEFAULT_DISTILL_INFO_NCE_TEMPERATURE,
        )
        .expect("default distiller is valid")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DistillStep {
    Skipped {
        signal_clarity: f32,
        reason: String,
    },
    Applied {
        teacher_score: f32,
        student_score: f32,
        info_nce_loss: f32,
        ema_norm_pre: f32,
        ema_norm_post: f32,
        signal_clarity: f32,
        ema_tau: f32,
        frozen_at: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EmbedderHandle {
    pub embedder_id: u32,
    pub weights: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OracleHandle {
    pub pass_score: f32,
    pub fail_score: f32,
}

impl OracleHandle {
    pub fn score(&self, outcome: &OracleOutcome) -> f32 {
        match outcome {
            OracleOutcome::Pass => self.pass_score,
            OracleOutcome::Fail => self.fail_score,
            OracleOutcome::OutOfDistribution | OracleOutcome::Abstain => 0.5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DistillEvidence {
    pub steps: Vec<DistillStep>,
    pub ema_tau: f32,
    pub skipped_count: usize,
    pub applied_count: usize,
}

pub fn consume(
    distiller: &mut OnlineDistiller,
    panel: &Panel,
    oracle_outcome: &OracleOutcome,
    signal_clarity: f32,
    active_embedders: &mut [EmbedderHandle],
    oracle_handle: &OracleHandle,
    storage: Arc<HealRocksStore>,
) -> Result<DistillStep, HealError> {
    if !signal_clarity.is_finite() || !(0.0..=1.0).contains(&signal_clarity) {
        return Err(HealError::invalid(
            "distill.signal_clarity",
            "signal_clarity must be in [0,1]",
        ));
    }
    if distiller.quality_gate && signal_clarity < distiller.signal_clarity_threshold {
        return Ok(DistillStep::Skipped {
            signal_clarity,
            reason: "below signal clarity threshold".to_string(),
        });
    }
    if active_embedders.is_empty() {
        return Err(HealError::invalid(
            "distill.active_embedders",
            "at least one embedder handle is required",
        ));
    }
    let teacher_score = oracle_handle.score(oracle_outcome);
    let panel_mean = panel.data().iter().take(256).sum::<f32>() / 256.0;
    let mut total_loss = 0.0f32;
    let mut pre_norm = 0.0f32;
    let mut post_norm = 0.0f32;
    for embedder in active_embedders {
        if embedder.weights.is_empty() || embedder.weights.iter().any(|v| !v.is_finite()) {
            return Err(HealError::invalid(
                "distill.embedder.weights",
                format!("embedder {} has invalid weights", embedder.embedder_id),
            ));
        }
        let student_score = embedder.weights.iter().sum::<f32>() / embedder.weights.len() as f32;
        let loss = info_nce_loss(student_score + panel_mean, teacher_score, distiller.info_nce_temperature);
        let after_grad = embedder
            .weights
            .iter()
            .map(|w| w - 0.01 * loss * signal_clarity)
            .collect::<Vec<_>>();
        pre_norm += l2(&embedder.weights);
        ema_update_in_place(&mut embedder.weights, &after_grad, distiller.ema_tau)?;
        post_norm += l2(&embedder.weights);
        total_loss += loss;
    }
    let frozen_at = chrono::Utc::now().timestamp();
    let step = DistillStep::Applied {
        teacher_score,
        student_score: panel_mean,
        info_nce_loss: total_loss / active_embedders.len() as f32,
        ema_norm_pre: pre_norm,
        ema_norm_post: post_norm,
        signal_clarity,
        ema_tau: distiller.ema_tau,
        frozen_at,
    };
    storage.put_cf_readback(
        CF_MEJEPA_HEAL_REPORTS,
        &encode_heal_report_key(frozen_at),
        &encode_value(&step)?,
    )?;
    Ok(step)
}

pub fn info_nce_loss(student: f32, teacher: f32, temperature: f32) -> f32 {
    let pos = (student * teacher / temperature).exp();
    let neg = ((1.0 - student) * teacher / temperature).exp();
    -(pos / (pos + neg)).max(1e-12).ln()
}

pub fn ema_update_in_place(
    w_student: &mut [f32],
    w_after_grad: &[f32],
    tau: f32,
) -> Result<(), HealError> {
    if w_student.len() != w_after_grad.len() || w_student.is_empty() {
        return Err(HealError::invalid(
            "distill.ema",
            "EMA vectors must be same non-empty length",
        ));
    }
    if !tau.is_finite() || tau <= 0.0 || tau >= 1.0 {
        return Err(HealError::invalid("distill.ema_tau", "tau must be in (0,1)"));
    }
    for (student, after) in w_student.iter_mut().zip(w_after_grad) {
        if !student.is_finite() || !after.is_finite() {
            return Err(HealError::BatchNan {
                component: "distill.ema".to_string(),
                witness_chain_offset: 0,
            });
        }
        *student = tau * *student + (1.0 - tau) * *after;
    }
    Ok(())
}

fn l2(values: &[f32]) -> f32 {
    values.iter().map(|v| v * v).sum::<f32>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn online_distiller_rejects_tau_one() {
        assert!(OnlineDistiller::try_new([0; 32], 1.0, true, 0.5, 0.07).is_err());
    }

    #[test]
    fn ema_update_uses_polyak_form() {
        let mut w = vec![1.0, 3.0];
        ema_update_in_place(&mut w, &[3.0, 7.0], 0.5).unwrap();
        assert_eq!(w, vec![2.0, 5.0]);
    }
}
