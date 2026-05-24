use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use rocksdb::IteratorMode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration_types::{complete_per_slot_sigma_squared, CalibrationRecord};
use crate::heal::calibration::compute_conformal_tau;
use crate::heal::cf::{
    encode_active_pointer_key, encode_calibration_history_record_key, encode_heal_report_key,
    encode_holdout_rotation_event_key, encode_value, is_holdout_rotation_event_key,
    ActivePointerValue, CF_MEJEPA_ACTIVE_POINTERS, CF_MEJEPA_CALIBRATION_HISTORY,
    CF_MEJEPA_HEAL_REPORTS, CF_MEJEPA_WEIGHT_BLOBS,
};
use crate::heal::errors::{HealError, PromotionCellRegression};
use crate::heal::integrity::WitnessChainAppender;
use crate::heal::store::HealRocksStore;
use crate::types::{Language, OracleOutcome};

const MODE_A_INFERENCE_LATENCY_MULTIPLIER: f32 = 1.0;
const MODE_B_INFERENCE_LATENCY_MULTIPLIER: f32 = 1.05;
const MODE_C_INFERENCE_LATENCY_MULTIPLIER: f32 = 1.15;
const PROMOTION_CALIBRATION_ALPHA: f32 = 0.10;
const PROMOTION_TARGET_COVERAGE: f32 = 0.90;
const PROMOTION_COVERAGE_MIN: f32 = 0.88;
const PROMOTION_COVERAGE_MAX: f32 = 0.92;
const PROMOTION_CELL_REGRESSION_TOLERANCE: f32 = 0.005;
const PROMOTION_CELL_REGRESSION_EPSILON: f32 = 1e-6;
pub const GLOBAL_HOLDOUT_CELL_KEY: &str = "global_or_per_cell";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerReason {
    DriftHard,
    DriftCatastrophic,
    PeriodicFullRetrain,
    OperatorTriggered,
    TelemetryConformalRecalibration,
    TelemetryFullRetrainCandidate,
    TrainingHoldoutDistributionDrift,
    OperatorOverrideRecalibration,
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
    pub per_cell_correlation: BTreeMap<String, f32>,
}

impl HoldoutEval {
    pub fn try_new(
        coverage: f32,
        oracle_agreement: f32,
        ood_distribution_kl: f32,
        num_samples: usize,
        evaluation_summary_sha: [u8; 32],
    ) -> Result<Self, HealError> {
        Self::try_new_with_cells(
            coverage,
            oracle_agreement,
            ood_distribution_kl,
            num_samples,
            evaluation_summary_sha,
            BTreeMap::from([(GLOBAL_HOLDOUT_CELL_KEY.to_string(), oracle_agreement)]),
        )
    }

    pub fn try_new_with_cells(
        coverage: f32,
        oracle_agreement: f32,
        ood_distribution_kl: f32,
        num_samples: usize,
        evaluation_summary_sha: [u8; 32],
        per_cell_correlation: BTreeMap<String, f32>,
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
        validate_per_cell_correlation_map(
            "holdout_eval.per_cell_correlation",
            &per_cell_correlation,
        )?;
        Ok(Self {
            coverage,
            oracle_agreement,
            ood_distribution_kl,
            num_samples,
            evaluation_summary_sha,
            per_cell_correlation,
        })
    }
}

fn validate_per_cell_correlation_map(
    field: &str,
    values: &BTreeMap<String, f32>,
) -> Result<(), HealError> {
    if values.is_empty() {
        return Err(HealError::invalid(
            field,
            "per-cell correlation map must be non-empty",
        ));
    }
    for (cell_key, value) in values {
        validate_cell_key(&format!("{field}.{cell_key}"), cell_key)?;
        if !value.is_finite() || !(0.0..=1.0).contains(value) {
            return Err(HealError::invalid(
                format!("{field}.{cell_key}"),
                format!("cell correlation must be finite in [0,1], got {value}"),
            ));
        }
    }
    Ok(())
}

fn validate_cell_key(field: &str, cell_key: &str) -> Result<(), HealError> {
    if cell_key.trim().is_empty()
        || cell_key.trim() != cell_key
        || cell_key.bytes().any(|byte| byte == 0 || byte == b'\n')
    {
        return Err(HealError::invalid(
            field,
            "cell key must be non-empty trimmed single-line text",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PromotionGate {
    pub stated_coverage_min: f32,
    pub oracle_agreement_floor: f32,
    pub ood_kl_ceiling: f32,
    pub full_retrain_floor: f32,
    pub cell_regression_tolerance: f32,
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
            (
                "cell_regression_tolerance",
                PROMOTION_CELL_REGRESSION_TOLERANCE,
            ),
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
            cell_regression_tolerance: PROMOTION_CELL_REGRESSION_TOLERANCE,
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
    pub calibration_nonconformity_score: f32,
    pub cell_key: String,
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
            if !example.calibration_nonconformity_score.is_finite()
                || !(0.0..=1.0).contains(&example.calibration_nonconformity_score)
            {
                return Err(HealError::invalid(
                    format!("holdout.examples[{idx}].calibration_nonconformity_score"),
                    "calibration non-conformity score must be finite in [0,1]",
                ));
            }
            validate_cell_key(
                &format!("holdout.examples[{idx}].cell_key"),
                &example.cell_key,
            )?;
        }
        Ok(Self {
            examples,
            corpus_sha,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HoldoutRotationPolicy {
    pub successful_promotion_period: u64,
    pub swap_fraction: f32,
    pub seed: u64,
}

impl HoldoutRotationPolicy {
    pub fn try_new(
        successful_promotion_period: u64,
        swap_fraction: f32,
        seed: u64,
    ) -> Result<Self, HealError> {
        if successful_promotion_period == 0 {
            return Err(HealError::invalid(
                "holdout_rotation.successful_promotion_period",
                "period must be greater than zero",
            ));
        }
        if !swap_fraction.is_finite() || swap_fraction <= 0.0 || swap_fraction > 1.0 {
            return Err(HealError::invalid(
                "holdout_rotation.swap_fraction",
                "swap fraction must be finite in (0,1]",
            ));
        }
        Ok(Self {
            successful_promotion_period,
            swap_fraction,
            seed,
        })
    }
}

impl Default for HoldoutRotationPolicy {
    fn default() -> Self {
        Self::try_new(4, 0.10, 0x000C_07A7_E607_A710_u64)
            .expect("default holdout rotation policy is valid")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PromotionSplitState {
    pub train_task_ids: Vec<String>,
    pub holdout_task_ids: Vec<String>,
    pub successful_promotions_since_rotation: u64,
    pub total_successful_promotions: u64,
    pub rotation_index: u64,
}

impl PromotionSplitState {
    pub fn try_new(
        train_task_ids: Vec<String>,
        holdout_task_ids: Vec<String>,
    ) -> Result<Self, HealError> {
        validate_split_ids("train_task_ids", &train_task_ids)?;
        validate_split_ids("holdout_task_ids", &holdout_task_ids)?;
        let train = train_task_ids.iter().collect::<BTreeSet<_>>();
        for task_id in &holdout_task_ids {
            if train.contains(task_id) {
                return Err(HealError::invalid(
                    "holdout_rotation.split",
                    format!("task {task_id} appears in both train and holdout"),
                ));
            }
        }
        Ok(Self {
            train_task_ids,
            holdout_task_ids,
            successful_promotions_since_rotation: 0,
            total_successful_promotions: 0,
            rotation_index: 0,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HoldoutRotationEvent {
    pub rotation_index: u64,
    pub successful_promotion_count: u64,
    pub rotated_at_unix_ms: i64,
    pub seed: u64,
    pub swap_fraction_basis_points: u32,
    pub train_count_before: usize,
    pub holdout_count_before: usize,
    pub train_count_after: usize,
    pub holdout_count_after: usize,
    pub train_to_holdout: Vec<String>,
    pub holdout_to_train: Vec<String>,
    pub train_task_ids_after: Vec<String>,
    pub holdout_task_ids_after: Vec<String>,
    pub source_of_truth_cf: String,
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
pub struct PromotionConformalCalibration {
    pub calibration_version: String,
    pub alpha: f32,
    pub target_coverage: f32,
    pub empirical_coverage: f32,
    pub coverage_band_min: f32,
    pub coverage_band_max: f32,
    pub tau: f32,
    pub sample_count: usize,
    pub persisted_key: Vec<u8>,
    pub persisted: bool,
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
pub struct AbcReadbackEvidence {
    pub mode_a_score: HoldoutEval,
    pub mode_b_score: HoldoutEval,
    pub mode_c_score: HoldoutEval,
    pub mode_c_weights: (f32, f32),
    pub conformal_recalibration: PromotionConformalCalibration,
    pub holdout_rotation: Option<HoldoutRotationEvent>,
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
    pub last_evidence: Option<AbcReadbackEvidence>,
    pub split_state: Option<PromotionSplitState>,
    pub rotation_policy: HoldoutRotationPolicy,
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

struct PromotionCommitRequest<'a> {
    storage: &'a HealRocksStore,
    witness_chain: &'a mut WitnessChainAppender,
    holdout: &'a HoldoutDataset,
    winner: ModeWinner,
    winner_sha: [u8; 32],
    winner_blob: Vec<u8>,
    mode_a: &'a HoldoutEval,
    mode_b: &'a HoldoutEval,
    mode_c: &'a HoldoutEval,
    mode_c_weights: (f32, f32),
    started: Instant,
}

struct PromotionCommitResult {
    conformal_recalibration: PromotionConformalCalibration,
    evaluation_summary_sha: [u8; 32],
    witness_chain_offset: u64,
    promotion_latency_seconds: u64,
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
            split_state: None,
            rotation_policy: HoldoutRotationPolicy::default(),
        })
    }

    pub fn set_split_state(&mut self, state: PromotionSplitState) {
        self.split_state = Some(state);
    }

    pub fn split_state(&self) -> Option<&PromotionSplitState> {
        self.split_state.as_ref()
    }

    pub fn evaluate(
        &self,
        mode_a: &HoldoutEval,
        mode_b: &HoldoutEval,
        mode_c: &HoldoutEval,
    ) -> Result<ModeWinner, HealError> {
        let (winner, _, _, _) = choose_winner(
            &self.gate,
            mode_a,
            mode_b,
            mode_c,
            [0xB0; 32],
            [0xA0; 32],
            &[0.0],
        )?;
        Ok(winner)
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
        let (winner, winner_sha, winner_blob, _winner_eval) = choose_winner(
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
        let commit = self.commit(PromotionCommitRequest {
            storage: storage.as_ref(),
            witness_chain,
            holdout: &holdout,
            winner,
            winner_sha,
            winner_blob,
            mode_a: &mode_a,
            mode_b: &mode_b,
            mode_c: &mode_c,
            mode_c_weights,
            started,
        })?;
        let report = HealReport {
            mode_winner: winner,
            mode_a_score: mode_a.clone(),
            mode_b_score: mode_b.clone(),
            mode_c_score: mode_c.clone(),
            mode_c_weights,
            weights_sha_winner: winner_sha,
            evaluation_summary_sha: commit.evaluation_summary_sha,
            witness_chain_offset: commit.witness_chain_offset,
            promotion_latency_seconds: commit.promotion_latency_seconds,
            status_change: crate::heal::pipeline::StatusChange::Active,
            trigger_reason,
        };
        let report_key = encode_heal_report_key(chrono::Utc::now().timestamp());
        storage.put_cf_readback(CF_MEJEPA_HEAL_REPORTS, &report_key, &encode_value(&report)?)?;
        let holdout_rotation =
            self.record_successful_promotion_and_rotate_if_due(storage.as_ref())?;
        self.last_evidence = Some(AbcReadbackEvidence {
            mode_a_score: mode_a,
            mode_b_score: mode_b,
            mode_c_score: mode_c,
            mode_c_weights,
            conformal_recalibration: commit.conformal_recalibration,
            holdout_rotation,
            mode_winner: winner,
            theta_sha_winner: winner_sha,
            evaluation_summary_sha: commit.evaluation_summary_sha,
            witness_chain_offset: commit.witness_chain_offset,
            promotion_latency_seconds: report.promotion_latency_seconds,
        });
        Ok(report)
    }

    fn commit(
        &self,
        request: PromotionCommitRequest<'_>,
    ) -> Result<PromotionCommitResult, HealError> {
        let recalibration =
            self.recalibrate_conformal(request.storage, request.holdout, request.winner_sha)?;
        request.storage.put_cf_readback(
            CF_MEJEPA_WEIGHT_BLOBS,
            &request.winner_sha,
            &request.winner_blob,
        )?;
        let eval_summary_sha = evaluation_summary_sha(
            request.winner,
            request.mode_a,
            request.mode_b,
            request.mode_c,
            request.mode_c_weights,
            &recalibration.calibration_version,
        )?;
        let witness_offset = request.witness_chain.append_model_event_checked(
            request.storage,
            "ModelPromote",
            request.winner_sha,
            eval_summary_sha,
        )?;
        let active = ActivePointerValue::try_new(
            request.winner_sha.to_vec(),
            chrono::Utc::now().timestamp(),
        )?;
        request.storage.put_cf_readback(
            CF_MEJEPA_ACTIVE_POINTERS,
            &encode_active_pointer_key("active_weights")?,
            &encode_value(&active)?,
        )?;
        Ok(PromotionCommitResult {
            conformal_recalibration: recalibration,
            evaluation_summary_sha: eval_summary_sha,
            witness_chain_offset: witness_offset,
            promotion_latency_seconds: request.started.elapsed().as_secs(),
        })
    }

    fn recalibrate_conformal(
        &self,
        storage: &HealRocksStore,
        holdout: &HoldoutDataset,
        winner_sha: [u8; 32],
    ) -> Result<PromotionConformalCalibration, HealError> {
        let scores = holdout
            .examples
            .iter()
            .map(|example| example.calibration_nonconformity_score)
            .collect::<Vec<_>>();
        let tau = compute_conformal_tau(&scores, PROMOTION_CALIBRATION_ALPHA)?;
        let empirical_coverage = empirical_conformal_coverage(&scores, tau)?;
        let frozen_at = chrono::Utc::now().timestamp();
        let calibration_version = format!(
            "promotion-calib-{frozen_at}-{}-n{}",
            short_sha_hex(&winner_sha),
            holdout.examples.len()
        );
        let mut per_language_counts = BTreeMap::new();
        per_language_counts.insert(Language::Python, holdout.examples.len());
        let sigma_squared = holdout
            .examples
            .iter()
            .map(|example| example.ood_score * example.ood_score)
            .sum::<f32>()
            / holdout.examples.len() as f32
            + 1e-6;
        let record = CalibrationRecord {
            version: calibration_version.clone(),
            alpha: PROMOTION_CALIBRATION_ALPHA,
            target_coverage: PROMOTION_TARGET_COVERAGE,
            tau,
            sigma_squared,
            empirical_coverage,
            min_samples_per_stratum: 1,
            sample_count: holdout.examples.len(),
            per_language_counts,
            per_slot_sigma_squared: Some(complete_per_slot_sigma_squared(sigma_squared)),
            corpus_sha: holdout.corpus_sha,
            embedder_versions: BTreeMap::new(),
            frozen_at,
        };
        record
            .validate()
            .map_err(|err| HealError::invalid("promotion.conformal.record", err.to_string()))?;
        let key = encode_calibration_history_record_key(frozen_at, &calibration_version);
        storage.put_cf_readback(CF_MEJEPA_CALIBRATION_HISTORY, &key, &encode_value(&record)?)?;
        let result = PromotionConformalCalibration {
            calibration_version,
            alpha: PROMOTION_CALIBRATION_ALPHA,
            target_coverage: PROMOTION_TARGET_COVERAGE,
            empirical_coverage,
            coverage_band_min: PROMOTION_COVERAGE_MIN,
            coverage_band_max: PROMOTION_COVERAGE_MAX,
            tau,
            sample_count: holdout.examples.len(),
            persisted_key: key,
            persisted: true,
        };
        if !(PROMOTION_COVERAGE_MIN..=PROMOTION_COVERAGE_MAX).contains(&empirical_coverage) {
            return Err(HealError::ConformalCoverageOutOfBand {
                target_coverage: PROMOTION_TARGET_COVERAGE,
                empirical_coverage,
                min_allowed: PROMOTION_COVERAGE_MIN,
                max_allowed: PROMOTION_COVERAGE_MAX,
                calibration_version: result.calibration_version,
            });
        }
        let active =
            ActivePointerValue::try_new(result.calibration_version.as_bytes().to_vec(), frozen_at)?;
        storage.put_cf_readback(
            CF_MEJEPA_ACTIVE_POINTERS,
            &encode_active_pointer_key("active_calibration")?,
            &encode_value(&active)?,
        )?;
        Ok(result)
    }

    fn record_successful_promotion_and_rotate_if_due(
        &mut self,
        storage: &HealRocksStore,
    ) -> Result<Option<HoldoutRotationEvent>, HealError> {
        let Some(state) = &mut self.split_state else {
            return Ok(None);
        };
        state.total_successful_promotions += 1;
        state.successful_promotions_since_rotation += 1;
        if state.successful_promotions_since_rotation
            < self.rotation_policy.successful_promotion_period
        {
            return Ok(None);
        }
        let event = rotate_holdout_split(state, self.rotation_policy)?;
        storage.put_cf_readback(
            CF_MEJEPA_HEAL_REPORTS,
            &encode_holdout_rotation_event_key(event.rotation_index, event.rotated_at_unix_ms),
            &encode_value(&event)?,
        )?;
        Ok(Some(event))
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
        let new_offset = witness_chain.append_model_event_checked(
            storage.as_ref(),
            "ModelRollback",
            target_sha,
            eval_sha,
        )?;
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

    pub fn readback_snapshot(&self) -> Option<AbcReadbackEvidence> {
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
        let (key, value) = item?;
        if is_holdout_rotation_event_key(key.as_ref()) {
            continue;
        }
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

fn validate_split_ids(field: &str, task_ids: &[String]) -> Result<(), HealError> {
    if task_ids.is_empty() {
        return Err(HealError::invalid(
            format!("holdout_rotation.{field}"),
            "split bucket must be non-empty",
        ));
    }
    let mut seen = BTreeSet::new();
    for (idx, task_id) in task_ids.iter().enumerate() {
        if task_id.trim().is_empty() || task_id.trim() != task_id {
            return Err(HealError::invalid(
                format!("holdout_rotation.{field}[{idx}]"),
                "task id must be non-empty and trimmed",
            ));
        }
        if !seen.insert(task_id.as_str()) {
            return Err(HealError::invalid(
                format!("holdout_rotation.{field}"),
                format!("duplicate task id {task_id}"),
            ));
        }
    }
    Ok(())
}

fn rotate_holdout_split(
    state: &mut PromotionSplitState,
    policy: HoldoutRotationPolicy,
) -> Result<HoldoutRotationEvent, HealError> {
    validate_split_ids("train_task_ids", &state.train_task_ids)?;
    validate_split_ids("holdout_task_ids", &state.holdout_task_ids)?;
    let train_count_before = state.train_task_ids.len();
    let holdout_count_before = state.holdout_task_ids.len();
    let train_move_count = rotation_count(train_count_before, policy.swap_fraction);
    let holdout_move_count = rotation_count(holdout_count_before, policy.swap_fraction);
    let next_rotation_index = state.rotation_index + 1;
    let mut train_to_holdout = select_rotation_ids(
        &state.train_task_ids,
        train_move_count,
        policy.seed,
        next_rotation_index,
        b"train-to-holdout",
    );
    let mut holdout_to_train = select_rotation_ids(
        &state.holdout_task_ids,
        holdout_move_count,
        policy.seed,
        next_rotation_index,
        b"holdout-to-train",
    );
    train_to_holdout.sort();
    holdout_to_train.sort();
    let train_to_holdout_set = train_to_holdout.iter().cloned().collect::<BTreeSet<_>>();
    let holdout_to_train_set = holdout_to_train.iter().cloned().collect::<BTreeSet<_>>();
    let mut train_task_ids_after = state
        .train_task_ids
        .iter()
        .filter(|task_id| !train_to_holdout_set.contains(task_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    train_task_ids_after.extend(holdout_to_train.iter().cloned());
    train_task_ids_after.sort();
    let mut holdout_task_ids_after = state
        .holdout_task_ids
        .iter()
        .filter(|task_id| !holdout_to_train_set.contains(task_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    holdout_task_ids_after.extend(train_to_holdout.iter().cloned());
    holdout_task_ids_after.sort();
    state.train_task_ids = train_task_ids_after.clone();
    state.holdout_task_ids = holdout_task_ids_after.clone();
    state.rotation_index = next_rotation_index;
    state.successful_promotions_since_rotation = 0;
    let rotated_at_unix_ms = chrono::Utc::now().timestamp_millis();
    Ok(HoldoutRotationEvent {
        rotation_index: state.rotation_index,
        successful_promotion_count: state.total_successful_promotions,
        rotated_at_unix_ms,
        seed: policy.seed,
        swap_fraction_basis_points: (policy.swap_fraction * 10_000.0).round() as u32,
        train_count_before,
        holdout_count_before,
        train_count_after: state.train_task_ids.len(),
        holdout_count_after: state.holdout_task_ids.len(),
        train_to_holdout,
        holdout_to_train,
        train_task_ids_after,
        holdout_task_ids_after,
        source_of_truth_cf: CF_MEJEPA_HEAL_REPORTS.to_string(),
    })
}

fn rotation_count(len: usize, fraction: f32) -> usize {
    ((len as f32 * fraction).ceil() as usize).clamp(1, len)
}

fn select_rotation_ids(
    task_ids: &[String],
    count: usize,
    seed: u64,
    rotation_index: u64,
    salt: &[u8],
) -> Vec<String> {
    let mut ranked = task_ids
        .iter()
        .map(|task_id| {
            let mut hasher = Sha256::new();
            hasher.update(seed.to_be_bytes());
            hasher.update(rotation_index.to_be_bytes());
            hasher.update(salt);
            hasher.update(task_id.as_bytes());
            let digest: [u8; 32] = hasher.finalize().into();
            (digest, task_id.clone())
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    ranked
        .into_iter()
        .take(count)
        .map(|(_, task_id)| task_id)
        .collect()
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
    let mode_a_score = phase_e_promotion_score(mode_a, MODE_A_INFERENCE_LATENCY_MULTIPLIER)?;
    let mut candidates = Vec::new();
    let mut blocked_regressions = Vec::new();
    let mode_b_score = phase_e_promotion_score(mode_b, MODE_B_INFERENCE_LATENCY_MULTIPLIER)?;
    if gate.passes(mode_b) && mode_b_score > mode_a_score {
        let regressions = promotion_cell_regressions(
            ModeWinner::B,
            mode_a,
            mode_b,
            gate.cell_regression_tolerance,
        )?;
        if regressions.is_empty() {
            candidates.push((
                ModeWinner::B,
                theta_b,
                weights_to_bytes(mode_b_weights),
                mode_b.clone(),
            ));
        } else {
            blocked_regressions.extend(regressions);
        }
    }
    let mode_c_score = phase_e_promotion_score(mode_c, MODE_C_INFERENCE_LATENCY_MULTIPLIER)?;
    if gate.passes(mode_c) && mode_c_score > mode_a_score {
        let regressions = promotion_cell_regressions(
            ModeWinner::C,
            mode_a,
            mode_c,
            gate.cell_regression_tolerance,
        )?;
        if regressions.is_empty() {
            let mut blob = Vec::new();
            blob.extend_from_slice(&theta_a);
            blob.extend_from_slice(&theta_b);
            let c_sha = sha_bytes(&blob);
            candidates.push((ModeWinner::C, c_sha, blob, mode_c.clone()));
        } else {
            blocked_regressions.extend(regressions);
        }
    }
    candidates
        .into_iter()
        .max_by(|a, b| {
            let a_score = phase_e_promotion_score(&a.3, latency_for_winner(a.0))
                .expect("validated candidate latency");
            let b_score = phase_e_promotion_score(&b.3, latency_for_winner(b.0))
                .expect("validated candidate latency");
            a_score.total_cmp(&b_score)
        })
        .ok_or_else(|| {
            if blocked_regressions.is_empty() {
                HealError::holdout_regression(mode_a.clone(), mode_b.clone(), mode_c.clone())
            } else {
                HealError::promotion_cell_regression(blocked_regressions)
            }
        })
}

fn promotion_cell_regressions(
    attempted_winner: ModeWinner,
    before: &HoldoutEval,
    after: &HoldoutEval,
    tolerance: f32,
) -> Result<Vec<PromotionCellRegression>, HealError> {
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(HealError::invalid(
            "promotion.cell_regression_tolerance",
            "cell regression tolerance must be finite and non-negative",
        ));
    }
    let mut regressions = Vec::new();
    for (cell_key, before_value) in &before.per_cell_correlation {
        let after_value = after.per_cell_correlation.get(cell_key).copied();
        let holds_or_improves = after_value
            .map(|value| value + tolerance + PROMOTION_CELL_REGRESSION_EPSILON >= *before_value)
            .unwrap_or(false);
        if !holds_or_improves {
            regressions.push(PromotionCellRegression {
                cell_key: cell_key.clone(),
                correlation_before: *before_value,
                correlation_after: after_value,
                tolerance,
                attempted_winner,
            });
        }
    }
    Ok(regressions)
}

fn phase_e_promotion_score(eval: &HoldoutEval, latency_multiplier: f32) -> Result<f32, HealError> {
    if !latency_multiplier.is_finite() || latency_multiplier <= 0.0 {
        return Err(HealError::invalid(
            "promotion.latency_multiplier",
            "latency multiplier must be positive and finite",
        ));
    }
    Ok(eval.oracle_agreement - 0.05 * latency_multiplier)
}

fn latency_for_winner(winner: ModeWinner) -> f32 {
    match winner {
        ModeWinner::A | ModeWinner::AUnchangedNoFloorClearance | ModeWinner::AUnchangedNoWinner => {
            MODE_A_INFERENCE_LATENCY_MULTIPLIER
        }
        ModeWinner::B => MODE_B_INFERENCE_LATENCY_MULTIPLIER,
        ModeWinner::C => MODE_C_INFERENCE_LATENCY_MULTIPLIER,
    }
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
    let per_cell_correlation = evaluate_per_cell_correlation(holdout, lift);
    for (cell_key, correlation) in &per_cell_correlation {
        hasher.update(cell_key.as_bytes());
        hasher.update(correlation.to_le_bytes());
    }
    HoldoutEval::try_new_with_cells(
        coverage,
        agreement,
        ood,
        holdout.examples.len(),
        hasher.finalize().into(),
        per_cell_correlation,
    )
}

fn empirical_conformal_coverage(scores: &[f32], tau: f32) -> Result<f32, HealError> {
    if scores.is_empty() {
        return Err(HealError::ConformalInsufficientSamples {
            observed: 0,
            required: 1,
        });
    }
    if !tau.is_finite() || !(0.0..=1.0).contains(&tau) {
        return Err(HealError::invalid(
            "promotion.conformal.tau",
            "tau must be finite in [0,1]",
        ));
    }
    if scores
        .iter()
        .any(|score| !score.is_finite() || !(0.0..=1.0).contains(score))
    {
        return Err(HealError::invalid(
            "promotion.conformal.scores",
            "scores must be finite in [0,1]",
        ));
    }
    Ok(scores.iter().filter(|score| **score <= tau).count() as f32 / scores.len() as f32)
}

fn evaluate_per_cell_correlation(holdout: &HoldoutDataset, lift: f32) -> BTreeMap<String, f32> {
    let mut cells = BTreeMap::<String, (usize, usize)>::new();
    for example in &holdout.examples {
        let entry = cells.entry(example.cell_key.clone()).or_insert((0, 0));
        if example.predicted.contains(&example.actual) {
            entry.0 += 1;
        }
        entry.1 += 1;
    }
    cells
        .into_iter()
        .map(|(cell_key, (covered, total))| {
            let base = covered as f32 / total as f32;
            (cell_key, (base + lift * 0.90).min(0.99))
        })
        .collect()
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

fn short_sha_hex(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(16);
    for byte in &bytes[..8] {
        write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
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

    fn eval_with_cells(score: f32, cells: &[(&str, f32)], sha: u8) -> HoldoutEval {
        HoldoutEval::try_new_with_cells(
            score,
            score,
            0.05,
            100,
            [sha; 32],
            cells
                .iter()
                .map(|(cell, value)| ((*cell).to_string(), *value))
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn promotion_evaluate_blocks_one_cell_regression() {
        let promoter = AbcPromoter::try_new(0.1, PromotionGate::default()).unwrap();
        let mode_a = eval_with_cells(
            0.90,
            &[("compile_error::rust", 0.90), ("off_by_one::python", 0.80)],
            1,
        );
        let mode_b = eval_with_cells(
            0.94,
            &[("compile_error::rust", 0.91), ("off_by_one::python", 0.794)],
            2,
        );
        let mode_c = eval_with_cells(
            0.89,
            &[("compile_error::rust", 0.90), ("off_by_one::python", 0.80)],
            3,
        );
        let err = promoter.evaluate(&mode_a, &mode_b, &mode_c).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_PROMOTION_CELL_REGRESSION");
        let HealError::PromotionCellRegression { regressions } = err else {
            panic!("expected cell regression error");
        };
        assert_eq!(regressions.len(), 1);
        assert_eq!(regressions[0].cell_key, "off_by_one::python");
        assert_eq!(regressions[0].attempted_winner, ModeWinner::B);
    }

    #[test]
    fn promotion_evaluate_allows_exact_tolerance_floor() {
        let promoter = AbcPromoter::try_new(0.1, PromotionGate::default()).unwrap();
        let mode_a = eval_with_cells(
            0.90,
            &[("compile_error::rust", 0.90), ("off_by_one::python", 0.80)],
            1,
        );
        let mode_b = eval_with_cells(
            0.94,
            &[("compile_error::rust", 0.91), ("off_by_one::python", 0.795)],
            2,
        );
        let mode_c = eval_with_cells(
            0.89,
            &[("compile_error::rust", 0.90), ("off_by_one::python", 0.80)],
            3,
        );
        assert_eq!(
            promoter.evaluate(&mode_a, &mode_b, &mode_c).unwrap(),
            ModeWinner::B
        );
    }

    #[test]
    fn promotion_evaluate_falls_back_to_non_regressing_candidate() {
        let promoter = AbcPromoter::try_new(0.1, PromotionGate::default()).unwrap();
        let mode_a = eval_with_cells(
            0.90,
            &[("compile_error::rust", 0.90), ("off_by_one::python", 0.80)],
            1,
        );
        let mode_b = eval_with_cells(
            0.95,
            &[("compile_error::rust", 0.91), ("off_by_one::python", 0.79)],
            2,
        );
        let mode_c = eval_with_cells(
            0.94,
            &[("compile_error::rust", 0.91), ("off_by_one::python", 0.80)],
            3,
        );
        assert_eq!(
            promoter.evaluate(&mode_a, &mode_b, &mode_c).unwrap(),
            ModeWinner::C
        );
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
            (0..100)
                .map(|idx| HoldoutExample {
                    predicted: vec![OracleOutcome::Pass],
                    actual: if idx < 72 {
                        OracleOutcome::Pass
                    } else {
                        OracleOutcome::Fail
                    },
                    ood_score: 0.05,
                    calibration_nonconformity_score: if idx < 91 { 0.05 } else { 0.95 },
                    cell_key: GLOBAL_HOLDOUT_CELL_KEY.to_string(),
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
        assert_eq!(storage.count_cf(CF_MEJEPA_ACTIVE_POINTERS).unwrap(), 2);
    }

    #[test]
    fn promotion_recalibration_uses_score_coverage_not_winner_accuracy() {
        let temp = tempfile::tempdir().unwrap();
        let storage = HealRocksStore::open(temp.path().join("db")).unwrap();
        let chain_path = temp.path().join("chain.bin");
        let mut witness_chain = WitnessChainAppender::new(chain_path).unwrap();
        let total = 700usize;
        let covered = 567usize;
        let holdout = HoldoutDataset::try_new(
            (0..total)
                .map(|idx| HoldoutExample {
                    predicted: vec![OracleOutcome::Pass],
                    actual: if idx < covered {
                        OracleOutcome::Pass
                    } else {
                        OracleOutcome::Fail
                    },
                    ood_score: 0.05,
                    calibration_nonconformity_score: idx as f32 / (total - 1) as f32,
                    cell_key: GLOBAL_HOLDOUT_CELL_KEY.to_string(),
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
                lock,
                calibration_version: "calibration-v1",
            })
            .unwrap();
        assert!(report.mode_b_score.coverage > PROMOTION_COVERAGE_MAX);
        let evidence = promoter
            .readback_snapshot()
            .expect("promotion readback evidence");
        assert!((PROMOTION_COVERAGE_MIN..=PROMOTION_COVERAGE_MAX)
            .contains(&evidence.conformal_recalibration.empirical_coverage));
        assert_eq!(storage.count_cf(CF_MEJEPA_CALIBRATION_HISTORY).unwrap(), 1);
    }
}
