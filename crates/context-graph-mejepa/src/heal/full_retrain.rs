use serde::{Deserialize, Serialize};

use crate::heal::errors::HealError;
use crate::heal::pipeline::SelfHealingPipeline;
use crate::heal::promote::{HealReport, PromotionGate, TriggerReason};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CorpusSnapshot {
    pub corpus_sha: [u8; 32],
    pub frozen_at: i64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FullRetrainEvidence {
    pub due: bool,
    pub observation_counter: u64,
    pub snapshot: Option<CorpusSnapshot>,
    pub report_mode_winner: Option<crate::heal::promote::ModeWinner>,
    pub promoted: bool,
}

pub fn full_retrain_if_due(
    pipeline: &mut SelfHealingPipeline,
) -> Result<Option<HealReport>, HealError> {
    let counter = pipeline.status.lock().unwrap().observation_counter;
    if counter == 0 || !counter.is_multiple_of(pipeline.config.full_retrain_period) {
        return Ok(None);
    }
    let _snapshot = snapshot_corpus_for_full_retrain(pipeline.corpus_sha)?;
    let mut original_gate = pipeline.abc_promoter.gate;
    pipeline.abc_promoter.gate = override_promotion_gate_with_full_retrain_floor(
        &original_gate,
        pipeline.config.full_retrain_promotion_floor,
    );
    let report = pipeline.trigger_abc_for_current_drift(TriggerReason::PeriodicFullRetrain)?;
    original_gate.oracle_agreement_floor = report.mode_a_score.oracle_agreement;
    pipeline.abc_promoter.gate = original_gate;
    Ok(Some(report))
}

pub fn override_promotion_gate_with_full_retrain_floor(
    base_gate: &PromotionGate,
    floor: f32,
) -> PromotionGate {
    let mut gate = *base_gate;
    gate.oracle_agreement_floor += floor;
    gate
}

pub fn snapshot_corpus_for_full_retrain(
    corpus_sha_now: [u8; 32],
) -> Result<CorpusSnapshot, HealError> {
    if corpus_sha_now == [0; 32] {
        return Err(HealError::invalid(
            "full_retrain.corpus_sha",
            "corpus sha cannot be zero",
        ));
    }
    Ok(CorpusSnapshot {
        corpus_sha: corpus_sha_now,
        frozen_at: chrono::Utc::now().timestamp(),
        source: "CF_MEJEPA_MUTATION_CORPUS snapshot".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_retrain_floor_raises_oracle_floor() {
        let base = PromotionGate::default();
        let raised = override_promotion_gate_with_full_retrain_floor(&base, 0.02);
        assert!(
            (raised.oracle_agreement_floor - (base.oracle_agreement_floor + 0.02)).abs() < 1e-6
        );
    }
}
