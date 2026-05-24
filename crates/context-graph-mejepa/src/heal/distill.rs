use std::sync::Arc;

use context_graph_mejepa_instruments::Panel;
use serde::{Deserialize, Serialize};

use crate::heal::cf::{encode_distill_step_key, encode_value, CF_MEJEPA_DISTILL_STEPS};
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
            return Err(HealError::invalid(
                "distiller.ema_tau",
                "tau must be in (0,1)",
            ));
        }
        if !signal_clarity_threshold.is_finite() || !(0.0..=1.0).contains(&signal_clarity_threshold)
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
    // #705: the previous `panel.data().iter().take(256).sum() / 256.0`
    // averaged the first 256 floats of the flat 5,120-d panel buffer,
    // which spans E_AST + first 32 dims of E_CFG. That cross-embedder
    // aggregate violated CLAUDE.md §6.2 and doc 01 §1.5ter and was being
    // added to every per-embedder InfoNCE loss term. Removed entirely:
    // the per-embedder `student_score` is already a per-embedder quantity
    // and the panel-wide cross-slot average had no semantic meaning.
    // (Per-embedder slot lookup deferred to a follow-up that adds a
    // `slot: InstrumentSlot` field on `EmbedderHandle`.)
    let _ = panel; // explicitly mark panel as not consumed in the loss
    let mut total_loss = 0.0f32;
    let mut total_student_score = 0.0f32;
    let mut pre_norm = 0.0f32;
    let mut post_norm = 0.0f32;
    for embedder in &mut *active_embedders {
        if embedder.weights.is_empty() || embedder.weights.iter().any(|v| !v.is_finite()) {
            return Err(HealError::invalid(
                "distill.embedder.weights",
                format!("embedder {} has invalid weights", embedder.embedder_id),
            ));
        }
        let student_score = embedder.weights.iter().sum::<f32>() / embedder.weights.len() as f32;
        let loss = info_nce_loss(
            student_score,
            teacher_score,
            distiller.info_nce_temperature,
        );
        let after_grad = embedder
            .weights
            .iter()
            .map(|w| w - 0.01 * loss * signal_clarity)
            .collect::<Vec<_>>();
        pre_norm += l2(&embedder.weights);
        ema_update_in_place(&mut embedder.weights, &after_grad, distiller.ema_tau)?;
        post_norm += l2(&embedder.weights);
        total_loss += loss;
        total_student_score += student_score;
    }
    let frozen_at = chrono::Utc::now().timestamp();
    let step = DistillStep::Applied {
        teacher_score,
        // #705: was `student_score: panel_mean` (cross-embedder leak via a
        // mislabeled field). Now the average of per-embedder student
        // scores — slot-preserving because each student_score depended
        // only on its own embedder's weights.
        student_score: total_student_score / active_embedders.len() as f32,
        info_nce_loss: total_loss / active_embedders.len() as f32,
        ema_norm_pre: pre_norm,
        ema_norm_post: post_norm,
        signal_clarity,
        ema_tau: distiller.ema_tau,
        frozen_at,
    };
    let key = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    storage.put_cf_readback(
        CF_MEJEPA_DISTILL_STEPS,
        &encode_distill_step_key(key),
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
        return Err(HealError::invalid(
            "distill.ema_tau",
            "tau must be in (0,1)",
        ));
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

    /// #705 regression: with the cross-embedder `panel_mean` term removed
    /// from the InfoNCE loss, swapping the panel content of slots OTHER
    /// than what the embedder reads (i.e., perturbing dims in the flat
    /// buffer that no embedder consumed) MUST NOT change the emitted
    /// `info_nce_loss`. Previously the first 256 flat-buffer dims were
    /// averaged into every embedder's loss, so any panel mutation would
    /// shift the loss. After the fix the loss depends only on
    /// `embedder.weights` and `oracle_outcome`.
    #[test]
    fn heal_distill_loss_is_invariant_under_unrelated_panel_perturbations(
    ) -> Result<(), HealError> {
        use context_graph_mejepa_instruments::{InstrumentSlot, PanelBuilder};
        use tempfile::TempDir;

        fn panel_with_e_ast_pattern(scale: f32) -> Panel {
            let mut builder = PanelBuilder::new();
            for slot in InstrumentSlot::all() {
                let v: Vec<f32> = (0..slot.dim())
                    .map(|i| (i as f32) * 0.001 * scale)
                    .collect();
                builder.set_slot(slot, &v).expect("set slot");
            }
            builder.build().expect("panel builds")
        }

        let temp = TempDir::new().unwrap();
        let storage_a = HealRocksStore::open(temp.path().join("db_a")).unwrap();
        let storage_b = HealRocksStore::open(temp.path().join("db_b")).unwrap();

        let mut distiller_a = OnlineDistiller::default();
        let mut distiller_b = OnlineDistiller::default();
        let mut embedders_a = vec![EmbedderHandle {
            embedder_id: 7,
            weights: vec![0.1, 0.2, 0.3, 0.4],
        }];
        let mut embedders_b = embedders_a.clone();
        let oracle = OracleHandle {
            pass_score: 0.9,
            fail_score: 0.1,
        };

        // Two panels with very different content: only embedder.weights +
        // oracle determine the loss. Pre-#705, the flat panel.data().take(256)
        // mean leaked into the loss and the two runs would differ.
        let panel_a = panel_with_e_ast_pattern(1.0);
        let panel_b = panel_with_e_ast_pattern(50.0);
        let signal_clarity = 0.8_f32;

        let step_a = consume(
            &mut distiller_a,
            &panel_a,
            &OracleOutcome::Pass,
            signal_clarity,
            &mut embedders_a,
            &oracle,
            storage_a.clone(),
        )?;
        let step_b = consume(
            &mut distiller_b,
            &panel_b,
            &OracleOutcome::Pass,
            signal_clarity,
            &mut embedders_b,
            &oracle,
            storage_b.clone(),
        )?;

        match (step_a, step_b) {
            (
                DistillStep::Applied {
                    info_nce_loss: loss_a,
                    student_score: ss_a,
                    ..
                },
                DistillStep::Applied {
                    info_nce_loss: loss_b,
                    student_score: ss_b,
                    ..
                },
            ) => {
                assert!(
                    (loss_a - loss_b).abs() < 1e-6,
                    "info_nce_loss must be invariant under unrelated panel \
                     perturbations (#705); got loss_a={loss_a} loss_b={loss_b}"
                );
                assert!(
                    (ss_a - ss_b).abs() < 1e-6,
                    "student_score must be invariant under unrelated panel \
                     perturbations (#705); got ss_a={ss_a} ss_b={ss_b}"
                );
            }
            other => panic!("expected DistillStep::Applied pair, got {other:?}"),
        }
        Ok(())
    }
}
