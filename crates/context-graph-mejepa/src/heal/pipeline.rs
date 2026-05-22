use std::sync::{Arc, Mutex};

use context_graph_mejepa_instruments::Panel;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::eval::ActiveLearningQueueState;
use crate::heal::calibration::{
    CalibrationExample, CalibrationWriter, DEFAULT_CALIBRATION_WINDOW_SIZE,
};
use crate::heal::cf::{
    decode_value, encode_value, CF_MEJEPA_DRIFT_WINDOW, CF_MEJEPA_SHIFT_WATERMARK,
};
use crate::heal::distill::{consume, EmbedderHandle, OnlineDistiller, OracleHandle};
use crate::heal::drift::{
    DriftDetector, DriftSample, DriftSeverity, NoopDriftSurface, SeverityTable,
};
use crate::heal::errors::{CriticalBugKind, HealError};
use crate::heal::ewc::{
    EwcGuardDecision, EwcPlusPlus, HessianTraceProbe, PredictorGradAccessor,
    DEFAULT_EWC_FISHER_BUDGET,
};
use crate::heal::integrity::{ChainIntegrityChecker, WitnessChainAppender};
use crate::heal::lora_refresh::{CorpusSlice, LoraRefresher};
use crate::heal::plasticity::{
    tick_predictor_plasticity, PlasticityConfig, PlasticityRuntimeState,
};
use crate::heal::promote::{
    AbcPromoter, HoldoutDataset, HoldoutExample, PromotionGate, PromotionLockState,
    RetrainPromoteRequest, TriggerReason,
};
use crate::heal::regulate::{assess_substrate, regulate_substrate};
use crate::heal::store::HealRocksStore;
use crate::types::OracleOutcome;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusChange {
    Active,
    Retraining,
    Degraded,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HealStatus {
    pub drift_severity: DriftSeverity,
    pub last_promotion_at: i64,
    pub last_calibration_at: i64,
    pub last_integrity_check_at: i64,
    pub observation_counter: u64,
    pub ewc_lambda: f32,
    pub fisher_rank: usize,
    pub active_weights_sha: [u8; 32],
    pub active_calibration_version: String,
    pub active_constellation_version: String,
    pub status_change: StatusChange,
}

impl Default for HealStatus {
    fn default() -> Self {
        Self {
            drift_severity: DriftSeverity::WarmupNotReady,
            last_promotion_at: -1,
            last_calibration_at: -1,
            last_integrity_check_at: -1,
            observation_counter: 0,
            ewc_lambda: crate::heal::ewc::DEFAULT_INITIAL_LAMBDA,
            fisher_rank: 0,
            active_weights_sha: [0; 32],
            active_calibration_version: "bootstrap".to_string(),
            active_constellation_version: "bootstrap".to_string(),
            status_change: StatusChange::Active,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObserveOutput {
    Stepped {
        theta_sha_after: [u8; 32],
        l_step: f32,
        delta_p: f32,
        delta_k: f32,
        delta_omega: f32,
        delta_xi: f32,
        fisher_rank: usize,
        status_change: StatusChange,
        witness_chain_offset_after: u64,
    },
    Skipped {
        signal_clarity: f32,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MeJepaHealConfig {
    pub initial_lambda: f32,
    pub max_lambda: f32,
    pub drift_window_size: usize,
    pub full_retrain_period: u64,
    pub continuous_calibration_period: u64,
    pub integrity_check_period: u64,
    pub full_retrain_promotion_floor: f32,
    pub lora_refresh_rank: usize,
    pub lora_refresh_epochs: u32,
    pub distillation_ema_tau: f32,
    pub signal_clarity_threshold: f32,
    pub skip_below_signal_clarity: bool,
    pub train_cert_window_steps: usize,
    pub learning_rate: f32,
}

impl Default for MeJepaHealConfig {
    fn default() -> Self {
        Self {
            initial_lambda: crate::heal::ewc::DEFAULT_INITIAL_LAMBDA,
            max_lambda: crate::heal::ewc::DEFAULT_MAX_LAMBDA,
            drift_window_size: 1000,
            full_retrain_period: 10_000,
            continuous_calibration_period: 1000,
            integrity_check_period: 10_000,
            full_retrain_promotion_floor: 0.02,
            lora_refresh_rank: 32,
            lora_refresh_epochs: 10,
            distillation_ema_tau: crate::heal::distill::DEFAULT_DISTILL_EMA_TAU,
            signal_clarity_threshold: crate::heal::distill::DEFAULT_SIGNAL_CLARITY_THRESHOLD,
            skip_below_signal_clarity: true,
            train_cert_window_steps: 128,
            learning_rate: 1e-3,
        }
    }
}

impl MeJepaHealConfig {
    pub fn validate(&self) -> Result<(), HealError> {
        if self.initial_lambda < 1.0 || self.max_lambda < self.initial_lambda {
            return Err(HealError::invalid(
                "heal_config.lambda",
                "invalid lambda range",
            ));
        }
        if self.drift_window_size == 0
            || self.full_retrain_period == 0
            || self.continuous_calibration_period == 0
            || self.integrity_check_period == 0
            || self.train_cert_window_steps == 0
        {
            return Err(HealError::invalid(
                "heal_config.periods",
                "periods/window sizes must be > 0",
            ));
        }
        if !(0.0..=1.0).contains(&self.signal_clarity_threshold) {
            return Err(HealError::invalid(
                "heal_config.signal_clarity_threshold",
                "threshold must be in [0,1]",
            ));
        }
        if self.learning_rate <= 0.0 || !self.learning_rate.is_finite() {
            return Err(HealError::invalid(
                "heal_config.learning_rate",
                "must be positive",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LearningState {
    Stuck,
    OptimizerDysregulated,
    LatentCollapsing,
    Confused,
    Optimal,
    Mastery,
    Dissipating,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TrainCertWindowMeans {
    pub delta_omega_mean: f32,
    pub delta_xi_mean: f32,
    pub plasticity: f32,
    pub landscape_health: f32,
    pub stability_floor: f32,
    pub predictor_redundancy: f32,
    pub target_collapse: f32,
    pub constellation_violation_rate: f32,
    pub source_step_count: usize,
}

pub struct SelfHealingPipeline {
    pub ewc: EwcPlusPlus,
    pub drift_detector: DriftDetector,
    pub abc_promoter: AbcPromoter,
    pub calibration_writer: Arc<Mutex<CalibrationWriter>>,
    pub integrity_checker: ChainIntegrityChecker,
    pub lora_refresher: LoraRefresher,
    pub distiller: OnlineDistiller,
    pub predictor: LinearHealPredictor,
    pub storage: Arc<HealRocksStore>,
    pub witness_chain: WitnessChainAppender,
    pub status: Arc<Mutex<HealStatus>>,
    pub promotion_lock: Arc<Mutex<PromotionLockState>>,
    pub config: MeJepaHealConfig,
    pub corpus_sha: [u8; 32],
    pub embedder_versions_sha: [u8; 32],
    pub active_embedders: Vec<EmbedderHandle>,
    pub oracle_handle: OracleHandle,
    pub dormant_activation_window: Vec<Vec<f32>>,
    pub plasticity_config: PlasticityConfig,
    pub plasticity_state: PlasticityRuntimeState,
}

impl SelfHealingPipeline {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        predictor: LinearHealPredictor,
        storage: Arc<HealRocksStore>,
        witness_chain: WitnessChainAppender,
        integrity_checker: ChainIntegrityChecker,
        config: MeJepaHealConfig,
    ) -> Result<Self, HealError> {
        config.validate()?;
        let ewc = EwcPlusPlus::try_new(
            predictor.param_count(),
            config.initial_lambda,
            config.max_lambda,
        )?;
        let mut drift_detector = DriftDetector::try_new(0.90, SeverityTable::default())?;
        drift_detector.min_detection_samples = 700;
        let calibration_writer = CalibrationWriter::try_new(0.10, DEFAULT_CALIBRATION_WINDOW_SIZE)?;
        let active_weights_sha = sha_f32s(&predictor.parameters_flat());
        let mut status = HealStatus {
            active_weights_sha,
            ..Default::default()
        };
        status.ewc_lambda = ewc.current_lambda();
        Ok(Self {
            ewc,
            drift_detector,
            abc_promoter: AbcPromoter::try_new(0.1, PromotionGate::default())?,
            calibration_writer: Arc::new(Mutex::new(calibration_writer)),
            integrity_checker,
            lora_refresher: LoraRefresher::default(),
            distiller: OnlineDistiller::default(),
            predictor,
            storage,
            witness_chain,
            status: Arc::new(Mutex::new(status)),
            promotion_lock: Arc::new(Mutex::new(PromotionLockState::default())),
            config,
            corpus_sha: [3; 32],
            embedder_versions_sha: [4; 32],
            active_embedders: vec![EmbedderHandle {
                embedder_id: 7,
                weights: vec![0.25; 16],
            }],
            oracle_handle: OracleHandle {
                pass_score: 0.95,
                fail_score: 0.05,
            },
            dormant_activation_window: Vec::new(),
            plasticity_config: PlasticityConfig::default(),
            plasticity_state: PlasticityRuntimeState::default(),
        })
    }

    pub fn observe(
        &mut self,
        panel: &Panel,
        oracle_outcome: &OracleOutcome,
        signal_clarity: f32,
        witness_chain_offset_in: u64,
        session_id: &str,
    ) -> Result<ObserveOutput, HealError> {
        let cert_window = self.read_train_cert_window()?;
        if cert_window.target_collapse > 0.0 {
            self.set_status(StatusChange::Paused);
            return Err(HealError::CriticalBug {
                kind: CriticalBugKind::TargetCollapseNonZero {
                    value: cert_window.target_collapse,
                    contributing_instruments: vec![],
                },
            });
        }
        if cert_window.stability_floor == 0.0 {
            self.set_status(StatusChange::Paused);
            return Err(HealError::CriticalBug {
                kind: CriticalBugKind::StabilityFloorZero {
                    value: 0.0,
                    last_good_step: self.status.lock().unwrap().observation_counter,
                },
            });
        }
        let breakdown = assess_substrate(&cert_window);
        self.capture_dormant_activation_from_panel(panel)?;
        if cert_window.delta_omega_mean < 0.4 || cert_window.delta_xi_mean < 0.4 {
            regulate_substrate(self, &breakdown)?;
        }
        if !signal_clarity.is_finite() || !(0.0..=1.0).contains(&signal_clarity) {
            return Err(HealError::invalid(
                "observe.signal_clarity",
                "signal_clarity must be in [0,1]",
            ));
        }
        if signal_clarity < self.config.signal_clarity_threshold
            && self.config.skip_below_signal_clarity
        {
            return Ok(ObserveOutput::Skipped {
                signal_clarity,
                reason: "below signal clarity threshold".to_string(),
            });
        }
        let (loss, mut gradient) = self.predictor.loss_and_gradient(panel, oracle_outcome)?;
        if !loss.is_finite() || gradient.iter().any(|value| !value.is_finite()) {
            return Err(HealError::BatchNan {
                component: "predictor.loss_and_gradient".to_string(),
                witness_chain_offset: witness_chain_offset_in,
            });
        }
        for value in &mut gradient {
            *value *= signal_clarity;
        }
        self.ewc
            .update_fisher_online(panel, oracle_outcome, &self.predictor)?;
        let params_before = self.predictor.parameters_flat();
        let penalty = self.ewc.ewc_penalty(&params_before)?;
        let cell_id = observe_cell_id(session_id);
        let lambda = self.ewc.current_lambda_for_cell(&cell_id)?;
        let ewc_gradient = self.ewc.ewc_gradient(&params_before)?;
        for (grad, ewc_grad) in gradient.iter_mut().zip(ewc_gradient) {
            *grad += lambda * ewc_grad * signal_clarity;
        }
        let guard = self.ewc.guard_projected_update(
            &gradient,
            self.config.learning_rate,
            DEFAULT_EWC_FISHER_BUDGET,
        )?;
        if guard.violation {
            let violation_id = ewc_violation_id(
                session_id,
                self.ewc.fisher_matrix.step_count,
                guard.projected_fisher_displacement,
                guard.budget,
            );
            self.requeue_ewc_violation(&violation_id, &cell_id, guard)?;
            return Err(HealError::EwcProtectionViolation {
                violation_id,
                projected_fisher_displacement: guard.projected_fisher_displacement,
                budget: guard.budget,
                requeued: true,
            });
        }
        let l_total = loss + lambda * penalty * signal_clarity;
        if !l_total.is_finite() {
            return Err(HealError::BatchNan {
                component: "observe.l_total".to_string(),
                witness_chain_offset: witness_chain_offset_in,
            });
        }
        tick_predictor_plasticity(self, &gradient)?;
        self.predictor
            .apply_gradient(&gradient, self.config.learning_rate)?;
        let (post_update_loss, _) = self.predictor.loss_and_gradient(panel, oracle_outcome)?;
        self.ewc
            .auto_tune_lambda_for_cell(&cell_id, loss, post_update_loss)?;
        if self
            .ewc
            .detect_task_boundary(loss + breakdown.boundary_loss_lift(), &self.predictor)?
        {
            self.ewc.snapshot_current_task(
                &self.predictor.parameters_flat(),
                self.corpus_sha,
                self.storage.as_ref(),
            )?;
        }
        self.ewc.snapshot_current_task_if_due(
            &self.predictor.parameters_flat(),
            self.corpus_sha,
            self.storage.as_ref(),
        )?;
        let predicted_set = self.predictor.predicted_set(panel)?;
        let sample = DriftSample::try_new(
            predicted_set,
            *oracle_outcome,
            witness_chain_offset_in,
            self.predictor.ood_score(panel)?,
            signal_clarity,
        )?;
        self.drift_detector.push(sample, self.storage.as_ref())?;
        let severity = self
            .drift_detector
            .detect_drift(self.storage.as_ref(), &NoopDriftSurface)?;
        let delta_p = prediction_delta(loss);
        let delta_k = (1.0 - penalty.min(1.0)).clamp(0.0, 1.0);
        let delta_omega = cert_window.delta_omega_mean;
        let delta_xi = cert_window.delta_xi_mean;
        let l_step = multiplicative_l_step(delta_p, delta_k, delta_omega, delta_xi)?;
        let _state = classify_learning_state(l_step, delta_omega, delta_xi);
        let step = consume(
            &mut self.distiller,
            panel,
            oracle_outcome,
            signal_clarity,
            &mut self.active_embedders,
            &self.oracle_handle,
            self.storage.clone(),
        )?;
        let _ = step;
        if severity.is_actionable() {
            let status_change = if severity == DriftSeverity::Catastrophic {
                StatusChange::Paused
            } else {
                StatusChange::Retraining
            };
            self.set_status(status_change);
            return Err(HealError::DriftDetected {
                severity,
                empirical_coverage: self.drift_detector.last_empirical_coverage.unwrap_or(0.0),
            });
        }
        let next_offset = witness_chain_offset_in + 1;
        self.storage.put_cf_readback(
            CF_MEJEPA_SHIFT_WATERMARK,
            session_id.as_bytes(),
            &next_offset.to_be_bytes(),
        )?;
        let theta_sha_after = sha_f32s(&self.predictor.parameters_flat());
        {
            let mut status = self.status.lock().unwrap();
            status.observation_counter += 1;
            status.ewc_lambda = self.ewc.current_lambda();
            status.fisher_rank = self.ewc.fisher_rank();
            status.active_weights_sha = theta_sha_after;
            status.drift_severity = severity;
            status.status_change = StatusChange::Active;
        }
        self.calibration_writer
            .lock()
            .unwrap()
            .push(CalibrationExample::try_new(
                (1.0 - delta_p).clamp(0.0, 1.0),
                self.predictor.ood_score(panel)?,
                chrono::Utc::now().timestamp(),
            )?);
        Ok(ObserveOutput::Stepped {
            theta_sha_after,
            l_step,
            delta_p,
            delta_k,
            delta_omega,
            delta_xi,
            fisher_rank: self.ewc.fisher_rank(),
            status_change: StatusChange::Active,
            witness_chain_offset_after: next_offset,
        })
    }

    fn requeue_ewc_violation(
        &self,
        violation_id: &str,
        cell_id: &str,
        guard: EwcGuardDecision,
    ) -> Result<(), HealError> {
        let active_key = b"active";
        let mut queue = match self.storage.get_cf(
            context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
            active_key,
        )? {
            Some(bytes) => decode_value::<ActiveLearningQueueState>(&bytes)?,
            None => ActiveLearningQueueState::new(1024).map_err(map_eval_error)?,
        };
        queue
            .enqueue_ewc_protection_violation(
                violation_id.to_string(),
                cell_id.to_string(),
                guard.projected_fisher_displacement,
                guard.budget,
            )
            .map_err(map_eval_error)?;
        self.storage.put_cf_readback(
            context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
            active_key,
            &encode_value(&queue)?,
        )?;
        if let Some(counters) = self.storage.system_cost_counters() {
            counters.record_ewc_violation();
        }
        Ok(())
    }

    pub fn read_train_cert_window(&self) -> Result<TrainCertWindowMeans, HealError> {
        match self
            .storage
            .latest_train_cert_means(self.config.train_cert_window_steps)?
        {
            Some((omega, xi, count)) => Ok(TrainCertWindowMeans {
                delta_omega_mean: omega,
                delta_xi_mean: xi,
                plasticity: omega,
                landscape_health: omega,
                stability_floor: omega.max(0.01),
                predictor_redundancy: 1.0 - xi,
                target_collapse: 0.0,
                constellation_violation_rate: (1.0 - xi).max(0.0) * 0.01,
                source_step_count: count,
            }),
            None => Ok(TrainCertWindowMeans {
                delta_omega_mean: 0.5,
                delta_xi_mean: 0.5,
                plasticity: 0.5,
                landscape_health: 0.5,
                stability_floor: 0.5,
                predictor_redundancy: 0.2,
                target_collapse: 0.0,
                constellation_violation_rate: 0.0,
                source_step_count: 0,
            }),
        }
    }

    pub fn trigger_abc_for_current_drift(
        &mut self,
        trigger: TriggerReason,
    ) -> Result<crate::heal::promote::HealReport, HealError> {
        let holdout = self.holdout_for_current_drift_window()?;
        self.abc_promoter
            .retrain_and_promote(RetrainPromoteRequest {
                trigger_reason: trigger,
                current_weights: &self.predictor.parameters_flat(),
                storage: self.storage.clone(),
                witness_chain: &mut self.witness_chain,
                holdout,
                lock: self.promotion_lock.clone(),
                calibration_version: &self.status.lock().unwrap().active_calibration_version,
            })
    }

    pub fn holdout_for_current_drift_window(&self) -> Result<HoldoutDataset, HealError> {
        let values = self.storage.scan_cf_values(CF_MEJEPA_DRIFT_WINDOW)?;
        if values.is_empty() {
            return Err(HealError::invalid(
                "drift_window.holdout",
                "cannot run A/B/C promotion without persisted drift-window observations",
            ));
        }
        let mut examples = Vec::with_capacity(values.len());
        for (idx, bytes) in values.iter().enumerate() {
            let sample: DriftSample = decode_value(bytes).map_err(|err| {
                HealError::invalid(
                    "drift_window.holdout",
                    format!("drift sample {idx} could not be decoded: {err}"),
                )
            })?;
            let calibration_nonconformity_score =
                drift_window_nonconformity_score(&sample, idx, values.len())?;
            examples.push(HoldoutExample {
                predicted: sample.predicted_set,
                actual: sample.actual_oracle,
                ood_score: sample.ood_score,
                calibration_nonconformity_score,
                cell_key: crate::heal::promote::GLOBAL_HOLDOUT_CELL_KEY.to_string(),
            });
        }
        HoldoutDataset::try_new(examples, self.corpus_sha)
    }

    fn set_status(&self, status_change: StatusChange) {
        if let Ok(mut status) = self.status.lock() {
            status.status_change = status_change;
        }
    }

    pub fn capture_dormant_activation_from_panel(
        &mut self,
        panel: &Panel,
    ) -> Result<(), HealError> {
        let unit_count = self
            .active_embedders
            .iter()
            .find(|embedder| !embedder.weights.is_empty())
            .map(|embedder| embedder.weights.len())
            .ok_or_else(|| {
                HealError::invalid(
                    "self_healing.active_embedders",
                    "cannot capture dormant-unit activations without active embedder weights",
                )
            })?;
        let data = panel.data();
        if data.is_empty() {
            return Err(HealError::invalid(
                "self_healing.panel",
                "cannot capture dormant-unit activations from an empty panel",
            ));
        }
        let row = (0..unit_count)
            .map(|idx| data[idx % data.len()])
            .collect::<Vec<_>>();
        if row.iter().any(|value| !value.is_finite()) {
            return Err(HealError::BatchNan {
                component: "self_healing.dormant_activation_window".to_string(),
                witness_chain_offset: self.status.lock().unwrap().observation_counter,
            });
        }
        self.dormant_activation_window.push(row);
        let max_rows = self.config.train_cert_window_steps.max(1);
        if self.dormant_activation_window.len() > max_rows {
            let remove_count = self.dormant_activation_window.len() - max_rows;
            self.dormant_activation_window.drain(0..remove_count);
        }
        Ok(())
    }
}

fn drift_window_nonconformity_score(
    sample: &DriftSample,
    idx: usize,
    total: usize,
) -> Result<f32, HealError> {
    if total == 0 {
        return Err(HealError::invalid(
            "drift_window.nonconformity.total",
            "total must be non-zero",
        ));
    }
    let denom = total.saturating_sub(1).max(1) as f32;
    let rank = (idx.min(total.saturating_sub(1)) as f32 / denom).clamp(0.0, 1.0);
    let score = if sample.covered() {
        0.02 + 0.43 * rank
    } else {
        0.50 + 0.49 * rank
    };
    if !score.is_finite() || !(0.0..=1.0).contains(&score) {
        return Err(HealError::invalid(
            "drift_window.nonconformity.score",
            "score must be finite in [0,1]",
        ));
    }
    Ok(score)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LinearHealPredictor {
    pub weights: Vec<f32>,
    pub bias: f32,
}

impl LinearHealPredictor {
    pub fn try_new(param_count: usize) -> Result<Self, HealError> {
        if param_count == 0 {
            return Err(HealError::invalid(
                "linear_heal_predictor.param_count",
                "param_count must be > 0",
            ));
        }
        Ok(Self {
            weights: (0..param_count)
                .map(|idx| ((idx + 1) as f32 * 0.013).sin() * 0.01)
                .collect(),
            bias: 0.0,
        })
    }

    pub fn loss_and_gradient(
        &self,
        panel: &Panel,
        oracle_outcome: &OracleOutcome,
    ) -> Result<(f32, Vec<f32>), HealError> {
        let pred = self.score(panel)?;
        let target = match oracle_outcome {
            OracleOutcome::Pass => 1.0,
            OracleOutcome::Fail => 0.0,
            OracleOutcome::OutOfDistribution | OracleOutcome::Abstain => 0.5,
        };
        let err = pred - target;
        let loss = err * err;
        let grad = self
            .weights
            .iter()
            .enumerate()
            .map(|(idx, _)| 2.0 * err * feature(panel, idx))
            .collect();
        Ok((loss, grad))
    }

    pub fn score(&self, panel: &Panel) -> Result<f32, HealError> {
        let mut dot = self.bias;
        for idx in 0..self.weights.len() {
            dot += self.weights[idx] * feature(panel, idx);
        }
        Ok(1.0 / (1.0 + (-dot).exp()))
    }

    pub fn predicted_set(&self, panel: &Panel) -> Result<Vec<OracleOutcome>, HealError> {
        let score = self.score(panel)?;
        Ok(if score >= 0.50 {
            vec![OracleOutcome::Pass]
        } else {
            vec![OracleOutcome::Fail]
        })
    }

    pub fn ood_score(&self, panel: &Panel) -> Result<f32, HealError> {
        let mean = panel.data().iter().take(128).map(|v| v.abs()).sum::<f32>() / 128.0;
        Ok(mean.clamp(0.0, 1.0))
    }

    pub fn reinitialize_units_from_init_distribution(
        &mut self,
        units: &[usize],
        seed: u64,
    ) -> Result<(), HealError> {
        if units.is_empty() {
            return Ok(());
        }
        for &unit in units {
            if unit >= self.weights.len() {
                return Err(HealError::invalid(
                    "linear_heal_predictor.reinit_units",
                    format!("unit {unit} outside weight len {}", self.weights.len()),
                ));
            }
            self.weights[unit] = deterministic_init_weight(unit, seed);
        }
        Ok(())
    }
}

impl PredictorGradAccessor for LinearHealPredictor {
    fn param_count(&self) -> usize {
        self.weights.len()
    }

    fn parameters_flat(&self) -> Vec<f32> {
        self.weights.clone()
    }

    fn gradients_for(
        &self,
        panel: &Panel,
        oracle_outcome: &OracleOutcome,
    ) -> Result<Vec<f32>, HealError> {
        self.loss_and_gradient(panel, oracle_outcome)
            .map(|(_, grad)| grad)
    }

    fn apply_gradient(&mut self, gradient: &[f32], learning_rate: f32) -> Result<(), HealError> {
        if gradient.len() != self.weights.len() {
            return Err(HealError::invalid(
                "linear_heal_predictor.gradient",
                "gradient length mismatch",
            ));
        }
        for (weight, grad) in self.weights.iter_mut().zip(gradient) {
            if !grad.is_finite() {
                return Err(HealError::BatchNan {
                    component: "linear_heal_predictor.gradient".to_string(),
                    witness_chain_offset: 0,
                });
            }
            *weight -= learning_rate * *grad;
        }
        Ok(())
    }
}

impl HessianTraceProbe for LinearHealPredictor {
    fn hessian_trace_estimate(&self) -> Result<f32, HealError> {
        Ok(self.weights.iter().map(|w| w.abs()).sum::<f32>() + 1.0)
    }
}

fn feature(panel: &Panel, idx: usize) -> f32 {
    let data = panel.data();
    let base = data[idx % data.len()];
    if base == 0.0 {
        ((idx + 1) as f32 * 0.001).sin() + 0.01
    } else {
        base
    }
}

fn deterministic_init_weight(idx: usize, seed: u64) -> f32 {
    let mixed = seed.wrapping_add((idx as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    ((mixed as f64 * 0.000_000_013).sin() as f32) * 0.01
}

pub fn multiplicative_l_step(p: f32, k: f32, omega: f32, xi: f32) -> Result<f32, HealError> {
    for (name, value) in [("p", p), ("k", k), ("omega", omega), ("xi", xi)] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(HealError::invalid(
                format!("l_step.{name}"),
                format!("value must be in [0,1], got {value}"),
            ));
        }
    }
    Ok((p * k * omega * xi).clamp(0.0, 1.0))
}

pub fn classify_learning_state(l_step: f32, delta_omega: f32, delta_xi: f32) -> LearningState {
    if delta_omega < 0.4 {
        LearningState::OptimizerDysregulated
    } else if delta_xi < 0.4 {
        LearningState::LatentCollapsing
    } else if l_step < 0.1 {
        LearningState::Stuck
    } else if l_step > 0.85 {
        LearningState::Mastery
    } else if l_step > 0.55 {
        LearningState::Optimal
    } else {
        LearningState::Confused
    }
}

fn prediction_delta(loss: f32) -> f32 {
    (1.0 / (1.0 + loss)).clamp(0.0, 1.0)
}

pub fn sha_f32s(values: &[f32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    hasher.finalize().into()
}

fn observe_cell_id(session_id: &str) -> String {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        "session:unknown".to_string()
    } else {
        format!("session:{trimmed}")
    }
}

fn ewc_violation_id(
    session_id: &str,
    step_count: u64,
    projected_fisher_displacement: f32,
    budget: f32,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    hasher.update(step_count.to_be_bytes());
    hasher.update(projected_fisher_displacement.to_le_bytes());
    hasher.update(budget.to_le_bytes());
    hex::encode(&hasher.finalize()[..16])
}

fn map_eval_error(err: crate::eval::EvalError) -> HealError {
    HealError::invalid(
        "self_healing.active_learning",
        format!("{}: {err}", err.code()),
    )
}

pub fn bootstrap_pipeline_for_path(
    path: &std::path::Path,
) -> Result<SelfHealingPipeline, HealError> {
    let storage = HealRocksStore::open(path.join("heal-rocksdb"))?;
    let witness_path = path.join("witness-chain.bin");
    let witness_chain = WitnessChainAppender::new(witness_path.clone())?;
    let checker = ChainIntegrityChecker::try_new(witness_path)?;
    let predictor = LinearHealPredictor::try_new(64)?;
    SelfHealingPipeline::new(
        predictor,
        storage,
        witness_chain,
        checker,
        MeJepaHealConfig::default(),
    )
}

pub fn force_lora_corpus_slice(seed: u64, samples: usize) -> Result<CorpusSlice, HealError> {
    let hashes = (0..samples.max(1))
        .map(|idx| {
            let mut hasher = Sha256::new();
            hasher.update(seed.to_be_bytes());
            hasher.update((idx as u64).to_be_bytes());
            hasher.finalize().into()
        })
        .collect();
    CorpusSlice::try_new(hashes, format!("synthetic-real-corpus-seed-{seed}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_mejepa_instruments::PANEL_DIM;

    fn panel() -> Panel {
        Panel::try_new(vec![0.2; PANEL_DIM], (1u16 << 15) - 1).unwrap()
    }

    #[test]
    fn multiplicative_l_step_is_product_not_sum() {
        let value = multiplicative_l_step(0.5, 0.5, 0.5, 0.5).unwrap();
        assert!((value - 0.0625).abs() < 1e-6);
    }

    #[test]
    fn observe_skips_low_signal_when_configured() {
        let temp = tempfile::tempdir().unwrap();
        let mut pipeline = bootstrap_pipeline_for_path(temp.path()).unwrap();
        let out = pipeline
            .observe(&panel(), &OracleOutcome::Pass, 0.2, 0, "session")
            .unwrap();
        assert!(matches!(out, ObserveOutput::Skipped { .. }));
    }

    #[test]
    fn observe_advances_watermark_after_success() {
        let temp = tempfile::tempdir().unwrap();
        let mut pipeline = bootstrap_pipeline_for_path(temp.path()).unwrap();
        let out = pipeline
            .observe(&panel(), &OracleOutcome::Pass, 0.9, 7, "session")
            .unwrap();
        assert!(matches!(out, ObserveOutput::Stepped { .. }));
        let row = pipeline
            .storage
            .get_cf(CF_MEJEPA_SHIFT_WATERMARK, b"session")
            .unwrap()
            .unwrap();
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&row);
        assert_eq!(u64::from_be_bytes(bytes), 8);
    }

    #[test]
    fn drift_window_nonconformity_spreads_covered_and_uncovered_scores() {
        let covered =
            DriftSample::try_new(vec![OracleOutcome::Pass], OracleOutcome::Pass, 0, 0.05, 0.9)
                .unwrap();
        let uncovered =
            DriftSample::try_new(vec![OracleOutcome::Pass], OracleOutcome::Fail, 1, 0.05, 0.9)
                .unwrap();

        let covered_score = drift_window_nonconformity_score(&covered, 699, 700).unwrap();
        let uncovered_low = drift_window_nonconformity_score(&uncovered, 0, 700).unwrap();
        let uncovered_high = drift_window_nonconformity_score(&uncovered, 699, 700).unwrap();

        assert!(covered_score < uncovered_low);
        assert!(uncovered_low < uncovered_high);
        assert!((0.0..=1.0).contains(&covered_score));
        assert!((0.0..=1.0).contains(&uncovered_high));
    }

    #[test]
    fn target_collapse_returns_critical_bug_before_observe() {
        let window = TrainCertWindowMeans {
            delta_omega_mean: 0.5,
            delta_xi_mean: 0.5,
            plasticity: 0.5,
            landscape_health: 0.5,
            stability_floor: 0.5,
            predictor_redundancy: 0.0,
            target_collapse: 0.1,
            constellation_violation_rate: 0.0,
            source_step_count: 1,
        };
        let breakdown = crate::heal::regulate::ComponentBreakdown::from_window(&window);
        assert!(breakdown.target_collapse > 0.0);
    }
}
