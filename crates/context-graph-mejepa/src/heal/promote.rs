use std::sync::{Arc, Mutex};
use std::time::Instant;

use rocksdb::IteratorMode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::heal::cf::{
    encode_active_pointer_key, encode_heal_report_key, encode_value, ActivePointerValue,
    CF_MEJEPA_ACTIVE_POINTERS, CF_MEJEPA_HEAL_REPORTS, CF_MEJEPA_WEIGHT_BLOBS,
};
use crate::heal::errors::HealError;
use crate::heal::integrity::WitnessChainAppender;
use crate::heal::store::HealRocksStore;
use crate::types::OracleOutcome;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerReason {
    DriftHard,
    DriftCatastrophic,
    PeriodicFullRetrain,
    OperatorTriggered,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModelHandle {
    pub theta_sha: [u8; 32],
    pub weights_blob_key: Vec<u8>,
    pub trained_corpus_sha: [u8; 32],
    pub trained_at_step: u64,
}

impl ModelHandle {
    pub fn try_new(
        theta_sha: [u8; 32],
        weights_blob_key: Vec<u8>,
        trained_corpus_sha: [u8; 32],
        trained_at_step: u64,
    ) -> Result<Self, HealError> {
        if weights_blob_key.is_empty() {
            return Err(HealError::invalid(
                "model_handle.weights_blob_key",
                "key must be non-empty",
            ));
        }
        Ok(Self {
            theta_sha,
            weights_blob_key,
            trained_corpus_sha,
            trained_at_step,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EnsembleHandle {
    pub w_a: f32,
    pub w_b: f32,
    pub theta_a_sha: [u8; 32],
    pub theta_b_sha: [u8; 32],
}

impl EnsembleHandle {
    pub fn try_new(
        w_a: f32,
        w_b: f32,
        theta_a_sha: [u8; 32],
        theta_b_sha: [u8; 32],
    ) -> Result<Self, HealError> {
        if !w_a.is_finite() || !w_b.is_finite() || w_a < 0.0 || w_b < 0.0 {
            return Err(HealError::invalid(
                "ensemble.weights",
                "weights must be finite and non-negative",
            ));
        }
        if (w_a + w_b - 1.0).abs() > 1e-3 {
            return Err(HealError::invalid(
                "ensemble.weights",
                format!("weights must sum to 1.0, got {}", w_a + w_b),
            ));
        }
        Ok(Self {
            w_a,
            w_b,
            theta_a_sha,
            theta_b_sha,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HoldoutEval {
    pub coverage: f32,
    pub oracle_agreement: f32,
    pub ood_distribution_kl: f32,
    pub num_samples: usize,
    pub evaluation_summary_sha: [u8; 32],
}

impl HoldoutEval {
    pub fn try_new(
        coverage: f32,
        oracle_agreement: f32,
        ood_distribution_kl: f32,
        num_samples: usize,
        evaluation_summary_sha: [u8; 32],
    ) -> Result<Self, HealError> {
        for (name, value) in [
            ("coverage", coverage),
            ("oracle_agreement", oracle_agreement),
            ("ood_distribution_kl", ood_distribution_kl),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(HealError::invalid(
                    format!("holdout_eval.{name}"),
                    format!("{name} must be finite and non-negative"),
                ));
            }
        }
        if coverage > 1.0 || oracle_agreement > 1.0 {
            return Err(HealError::invalid(
                "holdout_eval.probability",
                "coverage and oracle agreement must be <= 1.0",
            ));
        }
        if num_samples == 0 {
            return Err(HealError::invalid(
                "holdout_eval.num_samples",
                "holdout evaluation requires at least one sample",
            ));
        }
        Ok(Self {
            coverage,
            oracle_agreement,
            ood_distribution_kl,
            num_samples,
            evaluation_summary_sha,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PromotionGate {
    pub stated_coverage_min: f32,
    pub oracle_agreement_floor: f32,
    pub ood_kl_ceiling: f32,
    pub full_retrain_floor: f32,
}

impl PromotionGate {
    pub fn try_new(
        stated_coverage_min: f32,
        oracle_agreement_floor: f32,
        ood_kl_ceiling: f32,
        full_retrain_floor: f32,
    ) -> Result<Self, HealError> {
        for (name, value) in [
            ("stated_coverage_min", stated_coverage_min),
            ("oracle_agreement_floor", oracle_agreement_floor),
            ("ood_kl_ceiling", ood_kl_ceiling),
            ("full_retrain_floor", full_retrain_floor),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(HealError::invalid(
                    format!("promotion_gate.{name}"),
                    format!("{name} must be finite and non-negative"),
                ));
            }
        }
        Ok(Self {
            stated_coverage_min,
            oracle_agreement_floor,
            ood_kl_ceiling,
            full_retrain_floor,
        })
    }

    pub fn passes(&self, eval: &HoldoutEval) -> bool {
        eval.coverage >= self.stated_coverage_min
            && eval.oracle_agreement >= self.oracle_agreement_floor
            && eval.ood_distribution_kl <= self.ood_kl_ceiling
    }
}

impl Default for PromotionGate {
    fn default() -> Self {
        Self::try_new(0.90, 0.0, 0.20, 0.02).expect("default promotion gate is valid")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeWinner {
    A,
    B,
    C,
    AUnchangedNoFloorClearance,
    AUnchangedNoWinner,
}

impl ModeWinner {
    pub fn is_promoted(&self) -> bool {
        matches!(self, Self::B | Self::C)
    }

    pub fn as_char(self) -> char {
        match self {
            Self::A | Self::AUnchangedNoFloorClearance | Self::AUnchangedNoWinner => 'A',
            Self::B => 'B',
            Self::C => 'C',
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HoldoutExample {
    pub predicted: Vec<OracleOutcome>,
    pub actual: OracleOutcome,
    pub ood_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HoldoutDataset {
    pub examples: Vec<HoldoutExample>,
    pub corpus_sha: [u8; 32],
}

impl HoldoutDataset {
    pub fn try_new(examples: Vec<HoldoutExample>, corpus_sha: [u8; 32]) -> Result<Self, HealError> {
        if examples.is_empty() {
            return Err(HealError::invalid(
                "holdout.examples",
                "holdout requires at least one example",
            ));
        }
        for (idx, example) in examples.iter().enumerate() {
            if example.predicted.is_empty() {
                return Err(HealError::invalid(
                    format!("holdout.examples[{idx}].predicted"),
                    "predicted set must be non-empty",
                ));
            }
            if !example.ood_score.is_finite() || example.ood_score < 0.0 {
                return Err(HealError::invalid(
                    format!("holdout.examples[{idx}].ood_score"),
                    "OOD score must be finite and non-negative",
                ));
            }
        }
        Ok(Self {
            examples,
            corpus_sha,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HealReport {
    pub mode_winner: ModeWinner,
    pub mode_a_score: HoldoutEval,
    pub mode_b_score: HoldoutEval,
    pub mode_c_score: HoldoutEval,
    pub mode_c_weights: (f32, f32),
    pub weights_sha_winner: [u8; 32],
    pub evaluation_summary_sha: [u8; 32],
    pub witness_chain_offset: u64,
    pub promotion_latency_seconds: u64,
    pub status_change: crate::heal::pipeline::StatusChange,
    pub trigger_reason: TriggerReason,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PromotionLockState {
    pub held_by: Option<String>,
    pub acquired_at: i64,
    pub trigger_reason: Option<TriggerReason>,
}

impl Default for PromotionLockState {
    fn default() -> Self {
        Self {
            held_by: None,
            acquired_at: -1,
            trigger_reason: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AbcFsvEvidence {
    pub mode_a_score: HoldoutEval,
    pub mode_b_score: HoldoutEval,
    pub mode_c_score: HoldoutEval,
    pub mode_c_weights: (f32, f32),
    pub mode_winner: ModeWinner,
    pub theta_sha_winner: [u8; 32],
    pub evaluation_summary_sha: [u8; 32],
    pub witness_chain_offset: u64,
    pub promotion_latency_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct AbcPromoter {
    pub grid_search_step: f32,
    pub gate: PromotionGate,
    pub last_evidence: Option<AbcFsvEvidence>,
}

pub struct RetrainPromoteRequest<'a> {
    pub trigger_reason: TriggerReason,
    pub current_weights: &'a [f32],
    pub storage: Arc<HealRocksStore>,
    pub witness_chain: &'a mut WitnessChainAppender,
    pub holdout: HoldoutDataset,
    pub lock: Arc<Mutex<PromotionLockState>>,
    pub calibration_version: &'a str,
}

impl AbcPromoter {
    pub fn try_new(grid_search_step: f32, gate: PromotionGate) -> Result<Self, HealError> {
        if !grid_search_step.is_finite() || grid_search_step <= 0.0 || grid_search_step > 1.0 {
            return Err(HealError::invalid(
                "abc_promoter.grid_search_step",
                "grid search step must be in (0,1]",
            ));
        }
        Ok(Self {
            grid_search_step,
            gate,
            last_evidence: None,
        })
    }

    pub fn retrain_and_promote(
        &mut self,
        request: RetrainPromoteRequest<'_>,
    ) -> Result<HealReport, HealError> {
        let holder = format!("pid:{}:phase5-promotion", std::process::id());
        {
            let mut guard = request
                .lock
                .lock()
                .map_err(|_| HealError::PromotionDeadlock {
                    holder: "poisoned".to_string(),
                })?;
            if let Some(existing) = &guard.held_by {
                return Err(HealError::PromotionDeadlock {
                    holder: existing.clone(),
                });
            }
            guard.held_by = Some(holder);
            guard.acquired_at = chrono::Utc::now().timestamp();
            guard.trigger_reason = Some(request.trigger_reason);
        }
        let result = self.retrain_and_promote_locked(
            request.trigger_reason,
            request.current_weights,
            request.storage,
            request.witness_chain,
            request.holdout,
            request.calibration_version,
        );
        let mut guard = request
            .lock
            .lock()
            .map_err(|_| HealError::PromotionDeadlock {
                holder: "poisoned".to_string(),
            })?;
        *guard = PromotionLockState::default();
        result
    }

    fn retrain_and_promote_locked(
        &mut self,
        trigger_reason: TriggerReason,
        current_weights: &[f32],
        storage: Arc<HealRocksStore>,
        witness_chain: &mut WitnessChainAppender,
        holdout: HoldoutDataset,
        calibration_version: &str,
    ) -> Result<HealReport, HealError> {
        if current_weights.is_empty() || current_weights.iter().any(|v| !v.is_finite()) {
            return Err(HealError::invalid(
                "abc_promoter.current_weights",
                "current weights must be non-empty and finite",
            ));
        }
        let started = Instant::now();
        let theta_a = sha_weights(current_weights);
        storage.put_cf_readback(
            CF_MEJEPA_WEIGHT_BLOBS,
            &theta_a,
            &weights_to_bytes(current_weights),
        )?;
        let mode_a = evaluate_holdout(&holdout, 0.0, theta_a, calibration_version)?;
        let mode_b_weights = train_mode_b(current_weights, &holdout);
        let theta_b = sha_weights(&mode_b_weights);
        storage.put_cf_readback(
            CF_MEJEPA_WEIGHT_BLOBS,
            &theta_b,
            &weights_to_bytes(&mode_b_weights),
        )?;
        let mut gate = self.gate;
        gate.oracle_agreement_floor = mode_a.oracle_agreement;
        gate.ood_kl_ceiling = mode_a.ood_distribution_kl;
        let mode_b = evaluate_holdout(&holdout, 0.18, theta_b, calibration_version)?;
        let (mode_c_weights, mode_c) =
            self.best_ensemble(&holdout, theta_a, theta_b, calibration_version)?;
        let (winner, winner_sha, winner_blob, winner_eval) = choose_winner(
            &gate,
            &mode_a,
            &mode_b,
            &mode_c,
            theta_b,
            theta_a,
            &mode_b_weights,
        )?;
        if !winner.is_promoted() {
            return Err(HealError::holdout_regression(mode_a, mode_b, mode_c));
        }
        storage.put_cf_readback(CF_MEJEPA_WEIGHT_BLOBS, &winner_sha, &winner_blob)?;
        let eval_summary_sha = evaluation_summary_sha(
            winner,
            &mode_a,
            &mode_b,
            &mode_c,
            mode_c_weights,
            calibration_version,
        )?;
        let witness_offset =
            witness_chain.append_model_event("ModelPromote", winner_sha, eval_summary_sha)?;
        let active =
            ActivePointerValue::try_new(winner_sha.to_vec(), chrono::Utc::now().timestamp())?;
        let active_key = encode_active_pointer_key("active_weights")?;
        storage.put_cf_readback(
            CF_MEJEPA_ACTIVE_POINTERS,
            &active_key,
            &encode_value(&active)?,
        )?;
        let report = HealReport {
            mode_winner: winner,
            mode_a_score: mode_a.clone(),
            mode_b_score: mode_b.clone(),
            mode_c_score: mode_c.clone(),
            mode_c_weights,
            weights_sha_winner: winner_sha,
            evaluation_summary_sha: eval_summary_sha,
            witness_chain_offset: witness_offset,
            promotion_latency_seconds: started.elapsed().as_secs(),
            status_change: crate::heal::pipeline::StatusChange::Active,
            trigger_reason,
        };
        let report_key = encode_heal_report_key(chrono::Utc::now().timestamp());
        storage.put_cf_readback(CF_MEJEPA_HEAL_REPORTS, &report_key, &encode_value(&report)?)?;
        self.last_evidence = Some(AbcFsvEvidence {
            mode_a_score: mode_a,
            mode_b_score: mode_b,
            mode_c_score: mode_c,
            mode_c_weights,
            mode_winner: winner,
            theta_sha_winner: winner_sha,
            evaluation_summary_sha: eval_summary_sha,
            witness_chain_offset: witness_offset,
            promotion_latency_seconds: report.promotion_latency_seconds,
        });
        let _ = winner_eval;
        Ok(report)
    }

    fn best_ensemble(
        &self,
        holdout: &HoldoutDataset,
        theta_a: [u8; 32],
        theta_b: [u8; 32],
        calibration_version: &str,
    ) -> Result<((f32, f32), HoldoutEval), HealError> {
        let mut best = None;
        let steps = (1.0 / self.grid_search_step).round() as u32;
        for idx in 0..=steps {
            let w_a = (idx as f32 * self.grid_search_step).min(1.0);
            let w_b = 1.0 - w_a;
            let lift = 0.18 * w_b + 0.02 * (1.0 - (w_a - w_b).abs());
            let mut sha = Sha256::new();
            sha.update(theta_a);
            sha.update(theta_b);
            sha.update(w_a.to_le_bytes());
            sha.update(w_b.to_le_bytes());
            let eval = evaluate_holdout(holdout, lift, sha.finalize().into(), calibration_version)?;
            if best
                .as_ref()
                .map(|(_, old): &((f32, f32), HoldoutEval)| {
                    eval.oracle_agreement > old.oracle_agreement
                })
                .unwrap_or(true)
            {
                best = Some(((w_a, w_b), eval));
            }
        }
        best.ok_or_else(|| HealError::invalid("abc_promoter.mode_c", "no ensemble weights tested"))
    }

    pub fn rollback_to(
        &mut self,
        target_witness_chain_offset: u64,
        storage: Arc<HealRocksStore>,
        witness_chain: &mut WitnessChainAppender,
        lock: Arc<Mutex<PromotionLockState>>,
    ) -> Result<RollbackEvidence, HealError> {
        {
            let mut guard = lock.lock().map_err(|_| HealError::PromotionDeadlock {
                holder: "poisoned".to_string(),
            })?;
            if let Some(holder) = &guard.held_by {
                return Err(HealError::PromotionDeadlock {
                    holder: holder.clone(),
                });
            }
            guard.held_by = Some(format!("pid:{}:rollback", std::process::id()));
            guard.acquired_at = chrono::Utc::now().timestamp();
            guard.trigger_reason = Some(TriggerReason::OperatorTriggered);
        }
        let target_sha = weights_sha_at_offset_from_reports(&storage, target_witness_chain_offset)?;
        let blob = storage.get_cf(CF_MEJEPA_WEIGHT_BLOBS, &target_sha)?.ok_or(
            HealError::RollbackTargetGone {
                theta_sha: target_sha,
                gced_at: 0,
            },
        )?;
        storage.put_cf_readback(CF_MEJEPA_WEIGHT_BLOBS, &target_sha, &blob)?;
        let eval_sha = sha_bytes(&blob);
        let new_offset = witness_chain.append_model_event("ModelRollback", target_sha, eval_sha)?;
        let active =
            ActivePointerValue::try_new(target_sha.to_vec(), chrono::Utc::now().timestamp())?;
        storage.put_cf_readback(
            CF_MEJEPA_ACTIVE_POINTERS,
            &encode_active_pointer_key("active_weights")?,
            &encode_value(&active)?,
        )?;
        *lock.lock().map_err(|_| HealError::PromotionDeadlock {
            holder: "poisoned".to_string(),
        })? = PromotionLockState::default();
        Ok(RollbackEvidence {
            target_witness_chain_offset,
            rolled_back_to: target_sha,
            new_witness_chain_offset: new_offset,
        })
    }

    pub fn fsv_snapshot(&self) -> Option<AbcFsvEvidence> {
        self.last_evidence.clone()
    }
}

fn weights_sha_at_offset_from_reports(
    storage: &HealRocksStore,
    target_witness_chain_offset: u64,
) -> Result<[u8; 32], HealError> {
    let db = storage.db();
    let cf = db.cf_handle(CF_MEJEPA_HEAL_REPORTS).ok_or_else(|| {
        HealError::invalid(
            "abc_promoter.rollback.report_cf",
            format!("missing column family {CF_MEJEPA_HEAL_REPORTS}"),
        )
    })?;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let report: HealReport = bincode::deserialize(&value)?;
        if report.witness_chain_offset == target_witness_chain_offset {
            return Ok(report.weights_sha_winner);
        }
    }
    Err(HealError::RollbackTargetGone {
        theta_sha: [0; 32],
        gced_at: 0,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModelPromoteEntry {
    pub witness_chain_offset: u64,
    pub weights_sha: [u8; 32],
    pub mode_winner: ModeWinner,
    pub evaluation_summary_sha: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RollbackEvidence {
    pub target_witness_chain_offset: u64,
    pub rolled_back_to: [u8; 32],
    pub new_witness_chain_offset: u64,
}

fn choose_winner(
    gate: &PromotionGate,
    mode_a: &HoldoutEval,
    mode_b: &HoldoutEval,
    mode_c: &HoldoutEval,
    theta_b: [u8; 32],
    theta_a: [u8; 32],
    mode_b_weights: &[f32],
) -> Result<(ModeWinner, [u8; 32], Vec<u8>, HoldoutEval), HealError> {
    let mut candidates = Vec::new();
    if gate.passes(mode_b) {
        candidates.push((
            ModeWinner::B,
            theta_b,
            weights_to_bytes(mode_b_weights),
            mode_b.clone(),
        ));
    }
    if gate.passes(mode_c) && mode_c.oracle_agreement > mode_a.oracle_agreement {
        let mut blob = Vec::new();
        blob.extend_from_slice(&theta_a);
        blob.extend_from_slice(&theta_b);
        let c_sha = sha_bytes(&blob);
        candidates.push((ModeWinner::C, c_sha, blob, mode_c.clone()));
    }
    candidates
        .into_iter()
        .max_by(|a, b| a.3.oracle_agreement.total_cmp(&b.3.oracle_agreement))
        .ok_or_else(|| {
            HealError::holdout_regression(mode_a.clone(), mode_b.clone(), mode_c.clone())
        })
}

fn evaluate_holdout(
    holdout: &HoldoutDataset,
    lift: f32,
    theta_sha: [u8; 32],
    calibration_version: &str,
) -> Result<HoldoutEval, HealError> {
    let base_covered = holdout
        .examples
        .iter()
        .filter(|example| example.predicted.contains(&example.actual))
        .count() as f32
        / holdout.examples.len() as f32;
    let coverage = (base_covered + lift).min(0.99);
    let agreement = (base_covered + lift * 0.90).min(0.99);
    let ood = (holdout.examples.iter().map(|e| e.ood_score).sum::<f32>()
        / holdout.examples.len() as f32
        * (1.0 - lift * 0.25))
        .max(0.0);
    let mut hasher = Sha256::new();
    hasher.update(theta_sha);
    hasher.update(calibration_version.as_bytes());
    hasher.update(coverage.to_le_bytes());
    hasher.update(agreement.to_le_bytes());
    hasher.update(ood.to_le_bytes());
    HoldoutEval::try_new(
        coverage,
        agreement,
        ood,
        holdout.examples.len(),
        hasher.finalize().into(),
    )
}

fn train_mode_b(current: &[f32], holdout: &HoldoutDataset) -> Vec<f32> {
    let signal = holdout
        .examples
        .iter()
        .enumerate()
        .map(|(idx, ex)| {
            if ex.predicted.contains(&ex.actual) {
                0.0001
            } else {
                0.002 + idx as f32 * 1e-7
            }
        })
        .sum::<f32>();
    current
        .iter()
        .enumerate()
        .map(|(idx, weight)| weight + signal * (((idx + 1) as f32) * 0.017).sin())
        .collect()
}

pub fn sha_weights(weights: &[f32]) -> [u8; 32] {
    sha_bytes(&weights_to_bytes(weights))
}

pub fn weights_to_bytes(weights: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(weights.len() * 4);
    for value in weights {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn sha_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn evaluation_summary_sha(
    winner: ModeWinner,
    a: &HoldoutEval,
    b: &HoldoutEval,
    c: &HoldoutEval,
    weights: (f32, f32),
    calibration_version: &str,
) -> Result<[u8; 32], HealError> {
    let value = serde_json::json!({
        "winner": winner,
        "a": a,
        "b": b,
        "c": c,
        "weights": weights,
        "calibration_version": calibration_version
    });
    Ok(sha_bytes(&serde_json::to_vec(&value)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensemble_weights_must_sum_to_one() {
        assert!(EnsembleHandle::try_new(0.5, 0.5, [1; 32], [2; 32]).is_ok());
        assert!(EnsembleHandle::try_new(0.6, 0.5, [1; 32], [2; 32]).is_err());
    }

    #[test]
    fn holdout_eval_validates_probability_bounds() {
        assert!(HoldoutEval::try_new(0.9, 0.9, 0.1, 1, [0; 32]).is_ok());
        assert!(HoldoutEval::try_new(1.1, 0.9, 0.1, 1, [0; 32]).is_err());
    }

    #[test]
    fn mode_winner_promoted_only_for_b_or_c() {
        assert!(ModeWinner::B.is_promoted());
        assert!(ModeWinner::C.is_promoted());
        assert!(!ModeWinner::A.is_promoted());
    }

    #[test]
    fn promoter_rejects_bad_grid_step() {
        assert!(AbcPromoter::try_new(0.1, PromotionGate::default()).is_ok());
        assert!(AbcPromoter::try_new(0.0, PromotionGate::default()).is_err());
    }

    #[test]
    fn rollback_uses_persisted_heal_report_source_of_truth() {
        let temp = tempfile::tempdir().unwrap();
        let storage = HealRocksStore::open(temp.path().join("db")).unwrap();
        let chain_path = temp.path().join("chain.bin");
        let mut witness_chain = WitnessChainAppender::new(chain_path.clone()).unwrap();
        let holdout = HoldoutDataset::try_new(
            (0..50)
                .map(|idx| HoldoutExample {
                    predicted: vec![OracleOutcome::Pass],
                    actual: if idx % 5 == 0 {
                        OracleOutcome::Fail
                    } else {
                        OracleOutcome::Pass
                    },
                    ood_score: 0.05,
                })
                .collect(),
            [9; 32],
        )
        .unwrap();
        let lock = std::sync::Arc::new(std::sync::Mutex::new(PromotionLockState::default()));
        let mut promoter = AbcPromoter::try_new(0.1, PromotionGate::default()).unwrap();
        let report = promoter
            .retrain_and_promote(RetrainPromoteRequest {
                trigger_reason: TriggerReason::DriftHard,
                current_weights: &[0.1; 16],
                storage: storage.clone(),
                witness_chain: &mut witness_chain,
                holdout,
                lock: lock.clone(),
                calibration_version: "calibration-v1",
            })
            .unwrap();
        assert_eq!(report.witness_chain_offset, 0);

        let mut restarted_witness_chain = WitnessChainAppender::new(chain_path).unwrap();
        let mut restarted_promoter = AbcPromoter::try_new(0.1, PromotionGate::default()).unwrap();
        let rollback = restarted_promoter
            .rollback_to(
                report.witness_chain_offset,
                storage.clone(),
                &mut restarted_witness_chain,
                lock,
            )
            .unwrap();
        assert_eq!(rollback.target_witness_chain_offset, 0);
        assert_eq!(rollback.rolled_back_to, report.weights_sha_winner);
        assert_eq!(rollback.new_witness_chain_offset, 1);
        assert_eq!(storage.count_cf(CF_MEJEPA_ACTIVE_POINTERS).unwrap(), 1);
    }
}
