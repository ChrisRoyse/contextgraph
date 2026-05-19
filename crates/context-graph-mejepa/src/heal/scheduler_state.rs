use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::compiler::{ColdCellMetric, MejepaStore};
use crate::conformal::CalibrationExample;
use crate::eval::{
    ActiveLearningKind, ActiveLearningQueueState, ActiveLearningRankBy, ProductionTelemetryWindow,
    RocksDbEvalStore, TRAINING_HOLDOUT_DRIFT_ALERT_CODE,
};
use crate::heal::calibration::continuous_calibration_if_due;
use crate::heal::cf::{decode_value, encode_value, CF_MEJEPA_DRIFT_WINDOW};
use crate::heal::dissipation::{detect_dissipation, DissipationConfig};
use crate::heal::dormancy::tick_per_head_dormancy;
use crate::heal::drift::{
    DriftDetector, DriftSample, DriftSeverity, NoopDriftSurface, SeverityTable,
};
use crate::heal::drift_attribution::{classify_drift_attribution, write_operator_alert};
use crate::heal::drift_per_cell::{
    detect_and_act_per_cell_drift, PerCellDriftConfig, PerCellIntervention,
};
use crate::heal::drift_surprise_weighted::surprise_weight_from_score;
use crate::heal::emergency_eviction::{
    classify_emergency_eviction, latest_emergency_eviction_decision,
    persist_emergency_eviction_decision, EmergencyEvictionAction, EmergencyEvictionDecision,
};
use crate::heal::errors::HealError;
use crate::heal::fisher::snapshot_per_head_fisher;
use crate::heal::full_retrain::full_retrain_if_due;
use crate::heal::integrity::{
    complete_witness_quarantine_repair, verify, ChainIntegrityChecker, WitnessChainAppender,
};
use crate::heal::lambda_ramp::tick_lambda_ramp;
use crate::heal::pipeline::{
    LinearHealPredictor, MeJepaHealConfig, SelfHealingPipeline, StatusChange,
};
use crate::heal::policy::{
    load_policy_record, persist_policy_record, policy_key, timestamped_policy_key,
};
use crate::heal::promote::{HealReport, HoldoutEval, ModeWinner, TriggerReason};
use crate::heal::promote_approval::{
    approved_promotions, mark_promotion_executed, queue_pending_retrain_request,
    PendingPromotionKind,
};
use crate::heal::regulate::{assess_substrate, regulate_substrate};
use crate::heal::store::HealRocksStore;
use crate::reward_signal_audit::{
    persist_signal_drop_log_entry, SignalDropLogEntry, SignalDropSeverity,
};
use crate::sampler_reward::{
    persist_sampler_reward_signal_readback, sampler_reward_from_prediction_outcome,
    sampler_reward_key,
};
use crate::store::RocksDbInferStore;
use crate::system_cost::{HealTickerTelemetrySnapshot, SystemCostCounters};
use crate::types::OracleOutcome;

pub const DEFAULT_OBSERVE_PERIOD: Duration = Duration::from_millis(250);
pub const DEFAULT_DRIFT_PERIOD: Duration = Duration::from_secs(60);
pub const DEFAULT_ACTIVE_LEARNING_PERIOD: Duration = Duration::from_secs(600);
pub const DEFAULT_PROMOTE_PERIOD: Duration = Duration::from_secs(3600);
pub const DEFAULT_CONTINUAL_BACKPROP_PERIOD: Duration = Duration::from_secs(1800);
pub const DEFAULT_CONSTELLATION_PERIOD: Duration = Duration::from_secs(3600);
pub const DEFAULT_EMERGENCY_EVICTION_PERIOD: Duration = Duration::from_secs(600);
pub const DEFAULT_TELEMETRY_FEEDBACK_PERIOD: Duration = Duration::from_secs(900);
pub const DEFAULT_EMERGENCY_EVICTION_THRESHOLD_BYTES: u64 = 110 * 1024 * 1024 * 1024;
pub const COLD_CELL_TARGETED_CORPUS_ABSTAINS_PER_WEEK: u32 = 100;

const TELEMETRY_CONFORMAL_TARGET_COVERAGE: f32 = 0.90;
const TELEMETRY_CONFORMAL_COVERAGE_MIN: f32 = 0.88;
const TELEMETRY_CONFORMAL_COVERAGE_MAX: f32 = 0.92;
const TELEMETRY_CONFORMAL_ECE_MAX: f32 = 0.02;
const TELEMETRY_ORACLE_AGREEMENT_MIN: f32 = 0.95;
pub const OPERATOR_OVERRIDE_RECALIBRATION_THRESHOLD: u64 = 100;

#[derive(Debug, Clone)]
pub struct SelfOptimConfig {
    pub observe_period: Duration,
    pub drift_period: Duration,
    pub active_learning_period: Duration,
    pub promote_period: Duration,
    pub continual_backprop_period: Duration,
    pub constellation_period: Duration,
    pub emergency_eviction_period: Duration,
    pub telemetry_feedback_period: Duration,
    pub status_path: PathBuf,
    pub hygiene_archive_root: PathBuf,
    pub witness_chain_path: PathBuf,
    pub test_outcome_root: PathBuf,
    pub emergency_eviction_threshold_bytes: u64,
}

impl Default for SelfOptimConfig {
    fn default() -> Self {
        Self {
            observe_period: DEFAULT_OBSERVE_PERIOD,
            drift_period: DEFAULT_DRIFT_PERIOD,
            active_learning_period: DEFAULT_ACTIVE_LEARNING_PERIOD,
            promote_period: DEFAULT_PROMOTE_PERIOD,
            continual_backprop_period: DEFAULT_CONTINUAL_BACKPROP_PERIOD,
            constellation_period: DEFAULT_CONSTELLATION_PERIOD,
            emergency_eviction_period: DEFAULT_EMERGENCY_EVICTION_PERIOD,
            telemetry_feedback_period: DEFAULT_TELEMETRY_FEEDBACK_PERIOD,
            status_path: PathBuf::from(
                "/mnt/d/contextgraph/state/schedulers/self_optimization_status.json",
            ),
            hygiene_archive_root: PathBuf::from("/mnt/d/contextgraph/archive/mejepa-hygiene"),
            witness_chain_path: PathBuf::from(
                "/mnt/d/contextgraph/storage/witness/self-optimization-witness-chain.bin",
            ),
            test_outcome_root: PathBuf::from("/mnt/d/contextgraph/runtime/test-outcomes"),
            emergency_eviction_threshold_bytes: DEFAULT_EMERGENCY_EVICTION_THRESHOLD_BYTES,
        }
    }
}

impl SelfOptimConfig {
    pub fn with_all_periods(mut self, period: Duration) -> Self {
        self.observe_period = period;
        self.drift_period = period;
        self.active_learning_period = period;
        self.promote_period = period;
        self.continual_backprop_period = period;
        self.constellation_period = period;
        self.emergency_eviction_period = period;
        self.telemetry_feedback_period = period;
        self
    }

    pub fn validate(&self) -> Result<(), HealError> {
        for (field, value) in [
            ("observe_period", self.observe_period),
            ("drift_period", self.drift_period),
            ("active_learning_period", self.active_learning_period),
            ("promote_period", self.promote_period),
            ("continual_backprop_period", self.continual_backprop_period),
            ("constellation_period", self.constellation_period),
            ("emergency_eviction_period", self.emergency_eviction_period),
            ("telemetry_feedback_period", self.telemetry_feedback_period),
        ] {
            if value.is_zero() {
                return Err(HealError::invalid(
                    format!("self_optim_config.{field}"),
                    "period must be greater than zero",
                ));
            }
        }
        if self.status_path.as_os_str().is_empty() {
            return Err(HealError::invalid(
                "self_optim_config.status_path",
                "status path must be non-empty",
            ));
        }
        if self.hygiene_archive_root.as_os_str().is_empty() {
            return Err(HealError::invalid(
                "self_optim_config.hygiene_archive_root",
                "hygiene archive root must be non-empty",
            ));
        }
        if self.witness_chain_path.as_os_str().is_empty() {
            return Err(HealError::invalid(
                "self_optim_config.witness_chain_path",
                "witness chain path must be non-empty",
            ));
        }
        if self.test_outcome_root.as_os_str().is_empty() {
            return Err(HealError::invalid(
                "self_optim_config.test_outcome_root",
                "test outcome root must be non-empty",
            ));
        }
        if self.emergency_eviction_threshold_bytes == 0 {
            return Err(HealError::invalid(
                "self_optim_config.emergency_eviction_threshold_bytes",
                "threshold must be greater than zero",
            ));
        }
        Ok(())
    }

    pub fn period_for(&self, tick: TickName) -> Duration {
        match tick {
            TickName::Observe => self.observe_period,
            TickName::DriftCheck => self.drift_period,
            TickName::ActiveLearning => self.active_learning_period,
            TickName::Promote => self.promote_period,
            TickName::ContinualBackprop => self.continual_backprop_period,
            TickName::ConstellationFreshness => self.constellation_period,
            TickName::EmergencyEviction => self.emergency_eviction_period,
            TickName::TelemetryFeedback => self.telemetry_feedback_period,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TickName {
    Observe,
    DriftCheck,
    ActiveLearning,
    Promote,
    ContinualBackprop,
    ConstellationFreshness,
    EmergencyEviction,
    TelemetryFeedback,
}

impl TickName {
    pub fn all() -> [Self; 8] {
        [
            Self::Observe,
            Self::DriftCheck,
            Self::ActiveLearning,
            Self::Promote,
            Self::ContinualBackprop,
            Self::ConstellationFreshness,
            Self::EmergencyEviction,
            Self::TelemetryFeedback,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "observe",
            Self::DriftCheck => "drift_check",
            Self::ActiveLearning => "active_learning",
            Self::Promote => "promote",
            Self::ContinualBackprop => "continual_backprop",
            Self::ConstellationFreshness => "constellation_freshness",
            Self::EmergencyEviction => "emergency_eviction",
            Self::TelemetryFeedback => "telemetry_feedback",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryFeedbackActionKind {
    QueueRecalibration,
    QueueFullRetrainCandidate,
    TrainingHoldoutDriftAlert,
}

impl TelemetryFeedbackActionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::QueueRecalibration => "queue_recalibration",
            Self::QueueFullRetrainCandidate => "queue_full_retrain_candidate",
            Self::TrainingHoldoutDriftAlert => "training_holdout_drift_alert",
        }
    }

    fn trigger_reason(self) -> TriggerReason {
        match self {
            Self::QueueRecalibration => TriggerReason::TelemetryConformalRecalibration,
            Self::QueueFullRetrainCandidate => TriggerReason::TelemetryFullRetrainCandidate,
            Self::TrainingHoldoutDriftAlert => TriggerReason::TrainingHoldoutDistributionDrift,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TelemetryFeedbackAction {
    pub kind: TelemetryFeedbackActionKind,
    pub window_id: String,
    pub captured_at_unix_ms: i64,
    pub reason: String,
    pub metric_name: String,
    pub metric_value: f32,
    pub threshold_min: Option<f32>,
    pub threshold_max: Option<f32>,
    pub source_of_truth_cf: String,
    pub heal_report_key_hex: String,
    pub pending_promotion_id: Option<String>,
    pub already_queued: bool,
    pub recorded_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OperatorOverrideCalibrationCounter {
    pub total_applied_count: u64,
    pub last_observed_sampler_counter: u64,
    pub overrides_consumed_since_calibration: u64,
    pub threshold: u64,
    pub recalibration_queued: bool,
    pub already_queued: bool,
    pub counter_reset_detected: bool,
    pub heal_report_key_hex: Option<String>,
    pub last_recalibration_total_applied_count: Option<u64>,
    pub last_recalibration_override_count: Option<u64>,
    pub source_of_truth_cfs: Vec<String>,
    pub recorded_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TestOutcomeEvent {
    event_id: String,
    ts: i64,
    session_id: String,
    tool_use_id: String,
    command: String,
    framework: String,
    test_id: String,
    outcome: String,
    duration_ms: Option<u64>,
    error_log: String,
    source: String,
    sequence: u64,
    line_no: u64,
    #[serde(default)]
    summary_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TestOutcomeBindingReport {
    source_root: PathBuf,
    scanned_files: usize,
    events_scanned: usize,
    events_bound: usize,
    events_missing_prediction: usize,
    events_skipped_non_hard_label: usize,
    bound_keys: Vec<String>,
    verification_keys: Vec<String>,
    sampler_reward_keys: Vec<String>,
    sampler_reward_statuses: Vec<String>,
    sampler_reward_write_dispositions: Vec<String>,
    signal_drop_event_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SchedulerPredictionVerificationRecord {
    schema_version: u32,
    verification_id: String,
    prediction_id: String,
    ts: i64,
    session_id: String,
    task_id: String,
    test_id: String,
    tool_use_id: String,
    command: String,
    observed_outcome: String,
    predicted_outcome: String,
    agreement: String,
    evidence_path: String,
    event_id: String,
    created_at_unix_ms: i64,
    source: String,
    source_prediction_cf: String,
    source_verification_cf: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActiveLearningFastPathRecord {
    task_id: String,
    prediction_id_hex: String,
    severity_score: f32,
    surprise_weight: u8,
    source_queue_cf: String,
    recorded_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ColdCellTargetRecord {
    cell_id: String,
    abstain_count: u32,
    threshold_per_week: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WitnessIntegrityTickRecord {
    observation_counter: u64,
    checked: bool,
    reason: String,
    entry_count: Option<u64>,
    recorded_at_unix_ms: i64,
    source_of_truth_cf: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TickRunRecord {
    pub ticker: String,
    pub ok: bool,
    pub action: String,
    pub details: serde_json::Value,
    pub duration_ms: u64,
    pub updated_at_unix_ms: i64,
    pub error_code: Option<String>,
    pub error: Option<String>,
}

impl TickRunRecord {
    fn success(tick: TickName, action: impl Into<String>, details: serde_json::Value) -> Self {
        Self {
            ticker: tick.as_str().to_string(),
            ok: true,
            action: action.into(),
            details,
            duration_ms: 0,
            updated_at_unix_ms: chrono::Utc::now().timestamp_millis(),
            error_code: None,
            error: None,
        }
    }

    fn failure(tick: TickName, err: &HealError) -> Self {
        Self {
            ticker: tick.as_str().to_string(),
            ok: false,
            action: "error".to_string(),
            details: json!({}),
            duration_ms: 0,
            updated_at_unix_ms: chrono::Utc::now().timestamp_millis(),
            error_code: Some(err.code().to_string()),
            error: Some(err.to_string()),
        }
    }

    fn with_duration(mut self, started: Instant) -> Self {
        self.duration_ms = started.elapsed().as_millis() as u64;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchedulerSourceOfTruth {
    pub status_path: PathBuf,
    pub db_column_families: Vec<String>,
    pub witness_chain_path: PathBuf,
    pub hygiene_archive_root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchedulerStatusSnapshot {
    pub status: String,
    pub started_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
    pub ticker_counts: BTreeMap<String, u64>,
    pub heal_ticker_telemetry_total: HealTickerTelemetrySnapshot,
    pub last_ticker: Option<String>,
    pub last_success: BTreeMap<String, TickRunRecord>,
    pub last_error: BTreeMap<String, TickRunRecord>,
    pub source_of_truth: SchedulerSourceOfTruth,
}

pub struct SchedulerState {
    config: SelfOptimConfig,
    db: Arc<DB>,
    storage: Arc<HealRocksStore>,
    eval_store: RocksDbEvalStore,
    pipeline: SelfHealingPipeline,
    system_cost_counters: Arc<SystemCostCounters>,
    started_at_unix_ms: i64,
    ticker_counts: BTreeMap<String, u64>,
    last_ticker: Option<String>,
    last_success: BTreeMap<String, TickRunRecord>,
    last_error: BTreeMap<String, TickRunRecord>,
}

impl SchedulerState {
    pub fn open(db: Arc<DB>, config: SelfOptimConfig) -> Result<Self, HealError> {
        Self::open_with_counters(db, config, Arc::new(SystemCostCounters::new()))
    }

    pub fn open_with_counters(
        db: Arc<DB>,
        config: SelfOptimConfig,
        system_cost_counters: Arc<SystemCostCounters>,
    ) -> Result<Self, HealError> {
        config.validate()?;
        let storage = HealRocksStore::from_db_with_system_cost_counters(
            db.clone(),
            Arc::clone(&system_cost_counters),
        )?;
        for cf in context_graph_mejepa_cf::all_hygiene_referenced_cfs() {
            if db.cf_handle(cf).is_none() {
                return Err(HealError::invalid(
                    "self_optim_scheduler.column_family",
                    format!("missing column family {cf}"),
                ));
            }
        }
        let eval_store = RocksDbEvalStore::new_with_system_cost_counters(
            db.clone(),
            Arc::clone(&system_cost_counters),
        )
        .map_err(map_eval_error)?;
        let witness_chain = WitnessChainAppender::new(config.witness_chain_path.clone())?;
        let integrity_checker = ChainIntegrityChecker::try_new(config.witness_chain_path.clone())?;
        let pipeline = SelfHealingPipeline::new(
            LinearHealPredictor::try_new(64)?,
            storage.clone(),
            witness_chain,
            integrity_checker,
            MeJepaHealConfig::default(),
        )?;
        let state = Self {
            config,
            db,
            storage,
            eval_store,
            pipeline,
            system_cost_counters,
            started_at_unix_ms: chrono::Utc::now().timestamp_millis(),
            ticker_counts: BTreeMap::new(),
            last_ticker: None,
            last_success: BTreeMap::new(),
            last_error: BTreeMap::new(),
        };
        state.write_status("running")?;
        Ok(state)
    }

    pub fn config(&self) -> &SelfOptimConfig {
        &self.config
    }

    pub fn status_path(&self) -> &Path {
        &self.config.status_path
    }

    pub fn mark_stopped(&mut self) -> Result<(), HealError> {
        self.write_status("stopped")
    }

    pub fn run_tick(&mut self, tick: TickName) -> Result<(), HealError> {
        let started = Instant::now();
        let result = self.dispatch_tick(tick);
        let record = match result {
            Ok(record) => {
                let record = record.with_duration(started);
                tracing::debug!(
                    target: "context_graph_mejepa::heal::scheduler",
                    ticker = tick.as_str(),
                    action = %record.action,
                    details = %record.details,
                    duration_ms = record.duration_ms,
                    "self-optimization scheduler tick completed"
                );
                record
            }
            Err(err) => {
                let record = TickRunRecord::failure(tick, &err).with_duration(started);
                tracing::error!(
                    target: "context_graph_mejepa::heal::scheduler",
                    error_code = err.code(),
                    ticker = tick.as_str(),
                    error = %err,
                    duration_ms = record.duration_ms,
                    "self-optimization scheduler tick failed"
                );
                record
            }
        };
        self.record_tick(tick, record)
    }

    pub fn snapshot(&self, status: impl Into<String>) -> SchedulerStatusSnapshot {
        SchedulerStatusSnapshot {
            status: status.into(),
            started_at_unix_ms: self.started_at_unix_ms,
            updated_at_unix_ms: chrono::Utc::now().timestamp_millis(),
            ticker_counts: self.ticker_counts.clone(),
            heal_ticker_telemetry_total: self
                .system_cost_counters
                .snapshot()
                .heal_ticker_telemetry_total,
            last_ticker: self.last_ticker.clone(),
            last_success: self.last_success.clone(),
            last_error: self.last_error.clone(),
            source_of_truth: SchedulerSourceOfTruth {
                status_path: self.config.status_path.clone(),
                db_column_families: context_graph_mejepa_cf::all_hygiene_referenced_cfs()
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                witness_chain_path: self.config.witness_chain_path.clone(),
                hygiene_archive_root: self.config.hygiene_archive_root.clone(),
            },
        }
    }

    fn dispatch_tick(&mut self, tick: TickName) -> Result<TickRunRecord, HealError> {
        match tick {
            TickName::Observe => self.tick_observe(),
            TickName::DriftCheck => self.tick_drift_check(),
            TickName::ActiveLearning => self.tick_active_learning(),
            TickName::Promote => self.tick_promote(),
            TickName::ContinualBackprop => self.tick_continual_backprop(),
            TickName::ConstellationFreshness => self.tick_constellation_freshness(),
            TickName::EmergencyEviction => self.tick_emergency_eviction(),
            TickName::TelemetryFeedback => self.tick_telemetry_feedback(),
        }
    }

    fn tick_observe(&mut self) -> Result<TickRunRecord, HealError> {
        let test_outcome_bindings =
            bind_test_outcomes(&self.config.test_outcome_root, self.storage.db())?;
        let witness_integrity = self.maybe_verify_witness_chain_integrity()?;
        let oracle_verdicts = self
            .storage
            .count_cf(context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS)?;
        let live_predictions = self
            .storage
            .count_cf(context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS)?;
        let shift_watermarks = self
            .storage
            .count_cf(context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK)?;
        let sampler_rewards = self
            .storage
            .count_cf(context_graph_mejepa_cf::CF_MEJEPA_SAMPLER_REWARDS)?;
        Ok(TickRunRecord::success(
            TickName::Observe,
            "observed_sources_inspected",
            json!({
                "oracle_verdict_rows": oracle_verdicts,
                "live_prediction_rows": live_predictions,
                "shift_watermark_rows": shift_watermarks,
                "sampler_reward_rows": sampler_rewards,
                "test_outcome_bindings": test_outcome_bindings,
                "witness_integrity": witness_integrity,
                "source_of_truth_cfs": [
                    context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS,
                    context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
                    context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
                    context_graph_mejepa_cf::CF_MEJEPA_SAMPLER_REWARDS,
                    context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS
                ],
            }),
        ))
    }

    fn tick_drift_check(&mut self) -> Result<TickRunRecord, HealError> {
        let values = self.storage.scan_cf_values(CF_MEJEPA_DRIFT_WINDOW)?;
        let mut detector = DriftDetector::try_new(0.90, SeverityTable::default())?;
        for bytes in values
            .iter()
            .rev()
            .take(crate::heal::drift::DEFAULT_DRIFT_WINDOW_SIZE)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            let sample: DriftSample = decode_value(bytes)?;
            detector.window.push(sample);
        }
        let sample_count = detector.window.len();
        let severity = detector.detect_drift(self.storage.as_ref(), &NoopDriftSurface)?;
        let per_cell = detect_and_act_per_cell_drift(
            self.db.clone(),
            self.storage.clone(),
            &mut self.pipeline.lora_refresher,
            &PerCellDriftConfig::default(),
        )?;
        let catastrophic_cell_count = per_cell
            .cells
            .values()
            .filter(|decision| {
                decision.intervention == PerCellIntervention::QueueFullRetrainApproval
            })
            .count();
        let lora_cell_count = per_cell
            .cells
            .values()
            .filter(|decision| decision.intervention == PerCellIntervention::QueueLoraRefresh)
            .count();
        let promotion_action =
            if severity == DriftSeverity::Catastrophic || catastrophic_cell_count > 0 {
                let coverage = detector.last_empirical_coverage.unwrap_or(0.0);
                let promotion_id = queue_pending_retrain_request(
                    self.storage.as_ref(),
                    PendingPromotionKind::CatastrophicFullRetrainRequired {
                        cell_key: "global_or_per_cell".to_string(),
                        metric_value: coverage,
                    },
                    TriggerReason::DriftCatastrophic,
                    "catastrophic global/per-cell drift requires operator approval",
                )?;
                json!({
                    "kind": "catastrophic_promotion_approval_queued",
                    "promotion_id": promotion_id,
                    "coverage": coverage,
                    "sample_count": sample_count,
                })
            } else if matches!(severity, DriftSeverity::Soft | DriftSeverity::Hard)
                || lora_cell_count > 0
            {
                let abc_report = self
                    .pipeline
                    .trigger_abc_for_current_drift(TriggerReason::DriftHard)?;
                json!({
                    "kind": "lora_refresh_and_abc_promoted",
                    "reason": "soft_or_hard_drift",
                    "sample_count": sample_count,
                    "heal_report": abc_report,
                })
            } else {
                json!({"kind": "none"})
            };
        let drift_alert = self.write_drift_alert_if_needed(
            severity,
            catastrophic_cell_count,
            lora_cell_count,
            detector.last_empirical_coverage,
        )?;
        Ok(TickRunRecord::success(
            TickName::DriftCheck,
            "drift_window_evaluated",
            json!({
                "source_of_truth_cf": CF_MEJEPA_DRIFT_WINDOW,
                "sample_count": sample_count,
                "severity": severity,
                "empirical_coverage": detector.last_empirical_coverage,
                "per_cell": per_cell,
                "lora_cell_count": lora_cell_count,
                "catastrophic_cell_count": catastrophic_cell_count,
                "promotion_action": promotion_action,
                "operator_alert": drift_alert,
            }),
        ))
    }

    fn tick_active_learning(&self) -> Result<TickRunRecord, HealError> {
        let mut queue = match self.eval_store.load_queue().map_err(map_eval_error)? {
            Some(queue) => queue,
            None => ActiveLearningQueueState::new(16).map_err(map_eval_error)?,
        };
        let cold_cell_targets = self.enqueue_cold_cell_targets(&mut queue)?;
        if cold_cell_targets.is_empty() && queue.entries.is_empty() {
            return Ok(TickRunRecord::success(
                TickName::ActiveLearning,
                "active_learning_queue_absent",
                json!({
                    "source_of_truth_cf": context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
                    "cold_cell_metric_cf": context_graph_mejepa_cf::CF_MEJEPA_COLD_CELL_METRICS,
                    "cold_cell_targets": [],
                    "queued_count": 0,
                    "top_tasks": [],
                }),
            ));
        }
        if !cold_cell_targets.is_empty() {
            self.eval_store
                .persist_queue(&queue)
                .map_err(map_eval_error)?;
        }
        // REQ-FLYWHEEL-11 / TASK-RWD-211: fast-track agent surprise and
        // novel-pattern rows before generic uncertainty backlog.
        let selected = queue
            .ranked_entries(ActiveLearningRankBy::SchedulerPriority)
            .into_iter()
            .take(10)
            .collect::<Vec<_>>();
        let mut fast_path_records = Vec::new();
        for entry in &selected {
            if let ActiveLearningKind::AgentSurprise {
                prediction_id,
                severity_score,
            } = &entry.kind
            {
                let record = ActiveLearningFastPathRecord {
                    task_id: entry.task_id.0.clone(),
                    prediction_id_hex: hex::encode(prediction_id.0),
                    severity_score: *severity_score,
                    surprise_weight: surprise_weight_from_score(*severity_score),
                    source_queue_cf: context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE
                        .to_string(),
                    recorded_at_unix_ms: chrono::Utc::now().timestamp_millis(),
                };
                let key = policy_key(&[
                    "phase_e",
                    "active-learning-fast-path",
                    &format!(
                        "{:020}-{}-{}",
                        record.recorded_at_unix_ms, record.task_id, record.prediction_id_hex
                    ),
                ])?;
                persist_policy_record(self.storage.as_ref(), &key, &record)?;
                fast_path_records.push(record);
            }
        }
        let top_tasks = selected
            .into_iter()
            .map(|entry| {
                let surprise_weight = match &entry.kind {
                    ActiveLearningKind::AgentSurprise { severity_score, .. } => {
                        surprise_weight_from_score(*severity_score)
                    }
                    _ => 1,
                };
                json!({
                    "task_id": entry.task_id.0,
                    "score": entry.score,
                    "curiosity_score": entry.curiosity_score,
                    "surprise_weight": surprise_weight,
                    "reason": entry.reason,
                    "ood_score": entry.ood_score,
                    "outcome_set_len": entry.outcome_set_len,
                    "kind": active_learning_kind_summary(&entry.kind),
                })
            })
            .collect::<Vec<_>>();
        Ok(TickRunRecord::success(
            TickName::ActiveLearning,
            "active_learning_queue_ranked",
            json!({
                "source_of_truth_cf": context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
                "cold_cell_metric_cf": context_graph_mejepa_cf::CF_MEJEPA_COLD_CELL_METRICS,
                "cold_cell_targets": cold_cell_targets,
                "queued_count": queue.entries.len(),
                "evicted_count": queue.evicted.len(),
                "ood_escalation_count": queue.ood_escalations.len(),
                "fast_path_records": fast_path_records,
                "top_tasks": top_tasks,
            }),
        ))
    }

    fn enqueue_cold_cell_targets(
        &self,
        queue: &mut ActiveLearningQueueState,
    ) -> Result<Vec<ColdCellTargetRecord>, HealError> {
        let now = chrono::Utc::now().timestamp_millis();
        let cutoff = now - 7 * 24 * 60 * 60 * 1000;
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        for value in self
            .storage
            .scan_cf_values(context_graph_mejepa_cf::CF_MEJEPA_COLD_CELL_METRICS)?
        {
            let metric: ColdCellMetric = bincode::deserialize(&value)?;
            metric.validate().map_err(|err| {
                HealError::invalid("cold_cell_metric", format!("{}: {err}", err.code()))
            })?;
            if metric.created_at_unix_ms < cutoff {
                continue;
            }
            let count = counts.entry(metric.cell_id).or_insert(0);
            *count = count.saturating_add(1);
        }
        let mut targets = Vec::new();
        for (cell_id, abstain_count) in counts {
            if abstain_count <= COLD_CELL_TARGETED_CORPUS_ABSTAINS_PER_WEEK {
                continue;
            }
            queue
                .enqueue_cold_cell_targeted_corpus(cell_id.clone(), abstain_count)
                .map_err(map_eval_error)?;
            targets.push(ColdCellTargetRecord {
                cell_id,
                abstain_count,
                threshold_per_week: COLD_CELL_TARGETED_CORPUS_ABSTAINS_PER_WEEK,
            });
        }
        Ok(targets)
    }

    fn tick_promote(&mut self) -> Result<TickRunRecord, HealError> {
        let mut approved = approved_promotions(self.storage.as_ref())?;
        approved.sort_by_key(|record| record.updated_at_unix_ms);
        let mut witness_repair = None;
        let mut catastrophic_promotion = None;
        if let Some(record) = approved.into_iter().rev().find(|record| {
            !matches!(
                record.kind,
                PendingPromotionKind::DynamicEmbedderPromotion { .. }
            )
        }) {
            match &record.kind {
                PendingPromotionKind::WitnessChainRepairRequired { .. } => {
                    let cleared = complete_witness_quarantine_repair(
                        self.storage.as_ref(),
                        &self.pipeline.integrity_checker,
                        &record.promotion_id,
                    )?;
                    witness_repair = Some(json!({
                        "approval": record,
                        "quarantine": cleared,
                    }));
                }
                PendingPromotionKind::CatastrophicFullRetrainRequired { .. }
                | PendingPromotionKind::CatastrophicAbcCandidate { .. } => {
                    let report = self
                        .pipeline
                        .trigger_abc_for_current_drift(TriggerReason::DriftCatastrophic)?;
                    let executed =
                        mark_promotion_executed(self.storage.as_ref(), &record.promotion_id)?;
                    catastrophic_promotion = Some(json!({
                        "approval": executed,
                        "heal_report": report,
                    }));
                }
                PendingPromotionKind::DynamicEmbedderPromotion { .. } => {}
            }
        }
        let calibration = continuous_calibration_if_due(&mut self.pipeline)?;
        let retrain = full_retrain_if_due(&mut self.pipeline)?;
        Ok(TickRunRecord::success(
            TickName::Promote,
            "promotion_gates_checked",
            json!({
                "witness_repair": witness_repair,
                "catastrophic_promotion": catastrophic_promotion,
                "calibration_record": calibration,
                "full_retrain_report": retrain,
                "source_of_truth_cfs": [
                    context_graph_mejepa_cf::CF_MEJEPA_CALIBRATION_HISTORY,
                    context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_POINTERS,
                    context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS
                ],
            }),
        ))
    }

    fn tick_continual_backprop(&mut self) -> Result<TickRunRecord, HealError> {
        let window = self.pipeline.read_train_cert_window()?;
        let breakdown = assess_substrate(&window);
        let report = regulate_substrate(&mut self.pipeline, &breakdown)?;
        let dissipation = detect_dissipation(self.storage.as_ref(), &DissipationConfig::default())?;
        let lambda_ramp = tick_lambda_ramp(self.storage.as_ref())?;
        let per_head_fisher = snapshot_per_head_fisher(
            self.storage.as_ref(),
            &self.pipeline.ewc.fisher_matrix.diagonal,
            self.pipeline.ewc.fisher_matrix.rank,
            self.pipeline.ewc.fisher_matrix.step_count,
        )?;
        let per_head_dormancy = tick_per_head_dormancy(&mut self.pipeline)?;
        Ok(TickRunRecord::success(
            TickName::ContinualBackprop,
            "continual_backprop_regulation_checked",
            json!({
                "source_of_truth_cfs": [
                    context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS,
                    context_graph_mejepa_cf::CF_MEJEPA_WEIGHT_BLOBS,
                    context_graph_mejepa_cf::CF_MEJEPA_PLASTICITY_HISTORY
                ],
                "train_cert_source_step_count": window.source_step_count,
                "substrate_breakdown": breakdown,
                "regulation_report": report,
                "dissipation_report": dissipation,
                "lambda_ramp_report": lambda_ramp,
                "per_head_fisher_report": per_head_fisher,
                "per_head_dormancy_report": per_head_dormancy,
                "predictor_plasticity_state": &self.pipeline.plasticity_state,
            }),
        ))
    }

    fn tick_constellation_freshness(&self) -> Result<TickRunRecord, HealError> {
        let store = context_graph_mejepa_tct::ConstellationStore::new(self.db.clone())
            .map_err(map_tct_error)?;
        let count = store.count_constellations().map_err(map_tct_error)?;
        if count == 0 {
            return Ok(TickRunRecord::success(
                TickName::ConstellationFreshness,
                "constellation_store_empty",
                json!({
                    "source_of_truth_cf": context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION,
                    "constellation_count": 0,
                }),
            ));
        }
        let latest_version = store.latest_version().map_err(map_tct_error)?;
        let constellation = store
            .load_without_runtime_checks(latest_version)
            .map_err(map_tct_error)?;
        let (max_age_days, allow_stale) =
            context_graph_mejepa_tct::read_freshness_config().map_err(map_tct_error)?;
        let train_cert_count = self
            .storage
            .count_cf(context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)?;
        let latest_telemetry = latest_production_telemetry(self.storage.as_ref())?;
        let now = SystemTime::now();
        let recent_refresh_log = store
            .load_refresh_log_entries(10_000)
            .map_err(map_tct_error)?;
        let overrides = context_graph_mejepa_tct::overrides_from_refresh_log(
            latest_version,
            &recent_refresh_log,
        );
        let mut cells =
            context_graph_mejepa_tct::materialize_constellation_cells(&constellation, &overrides)
                .map_err(map_tct_error)?;
        let train_cert_count_u32 = u32::try_from(train_cert_count).unwrap_or(u32::MAX);
        let tau_m_drift_pct_rolling = latest_telemetry
            .as_ref()
            .map(|telemetry| {
                if telemetry.gtau_pass_rate < 0.97 {
                    ((0.97 - telemetry.gtau_pass_rate) / 0.97 * 100.0).max(0.0)
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        for cell in &mut cells {
            cell.examples_since_last_refresh = train_cert_count_u32;
            cell.tau_m_drift_pct_rolling = tau_m_drift_pct_rolling;
            cell.validate().map_err(map_tct_error)?;
        }
        let refresh_config = context_graph_mejepa_tct::RefreshPolicyConfig {
            max_age_days,
            ..context_graph_mejepa_tct::RefreshPolicyConfig::default()
        };
        let audit = context_graph_mejepa_tct::build_freshness_audit(
            latest_version,
            &cells,
            now,
            refresh_config,
        )
        .map_err(map_tct_error)?;
        let refresh_report_count = store.count_refresh_reports().map_err(map_tct_error)?;
        let refresh_log_count_before = store.count_refresh_log_entries().map_err(map_tct_error)?;
        let mut persisted_refresh_actions = 0usize;
        let mut refresh_failed_actions = 0usize;
        let mut shrunk_cell_refresh_actions = 0usize;
        for row in audit.rows.iter().filter(|row| row.decision.is_refit()) {
            let status = if train_cert_count == 0 {
                refresh_failed_actions += 1;
                context_graph_mejepa_tct::RefreshActionStatus::RefreshFailed
            } else {
                context_graph_mejepa_tct::RefreshActionStatus::RefitSucceeded
            };
            if matches!(
                row.decision,
                context_graph_mejepa_tct::RefreshDecision::Refit {
                    shrunk_cell: true,
                    ..
                }
            ) {
                shrunk_cell_refresh_actions += 1;
            }
            let detail = match status {
                context_graph_mejepa_tct::RefreshActionStatus::RefreshFailed => {
                    "REFRESH_FAILED: no train-cert rows available for centroid refit".to_string()
                }
                context_graph_mejepa_tct::RefreshActionStatus::RefitSucceeded => {
                    "REFIT_SUCCEEDED: scheduler advanced per-cell freshness after policy match"
                        .to_string()
                }
                _ => "constellation freshness scheduler action".to_string(),
            };
            let entry = context_graph_mejepa_tct::ConstellationRefreshLogEntry::try_new(
                context_graph_mejepa_tct::ConstellationRefreshLogEntryInput {
                    constellation_version_id: latest_version,
                    cell: row.cell,
                    decision: row.decision.clone(),
                    status,
                    generated_at: now,
                    after_last_refresh_ts: (status
                        == context_graph_mejepa_tct::RefreshActionStatus::RefitSucceeded)
                        .then_some(now),
                    operator_id: None,
                    detail,
                },
            )
            .map_err(map_tct_error)?;
            store
                .persist_refresh_log_entry(&entry)
                .map_err(map_tct_error)?;
            persisted_refresh_actions += 1;
        }
        let refresh_log_count_after = store.count_refresh_log_entries().map_err(map_tct_error)?;
        let refresh_policy = if audit.refit_required_count == 0 {
            json!({
                "action": "none",
                "total_cells": audit.total_cells,
                "skip_count": audit.skip_count,
            })
        } else {
            let record = json!({
                "action": "constellation_freshness_tick",
                "constellation_version_hex": hex::encode(latest_version),
                "total_cells": audit.total_cells,
                "refit_required_count": audit.refit_required_count,
                "persisted_refresh_actions": persisted_refresh_actions,
                "refresh_failed_actions": refresh_failed_actions,
                "shrunk_cell_refresh_actions": shrunk_cell_refresh_actions,
                "histogram": audit.histogram,
                "source_of_truth_cfs": [
                    context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION,
                    context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION_REFRESH_LOG,
                    context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION_REFRESH_REPORTS,
                    context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS,
                    context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY
                ],
                "generated_at_unix_ms": chrono::Utc::now().timestamp_millis(),
            });
            let key = timestamped_policy_key("constellation_refresh")?;
            persist_policy_record(self.storage.as_ref(), &key, &record)?;
            record
        };
        Ok(TickRunRecord::success(
            TickName::ConstellationFreshness,
            "constellation_freshness_verified",
            json!({
                "source_of_truth_cf": context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION,
                "constellation_count": count,
                "latest_version_hex": hex::encode(latest_version),
                "max_age_days": max_age_days,
                "allow_stale": allow_stale,
                "refresh_report_count": refresh_report_count,
                "refresh_log_count_before": refresh_log_count_before,
                "refresh_log_count_after": refresh_log_count_after,
                "train_cert_count": train_cert_count,
                "tau_m_drift_pct_rolling": tau_m_drift_pct_rolling,
                "latest_production_telemetry": latest_telemetry,
                "freshness_audit_summary": {
                    "total_cells": audit.total_cells,
                    "refit_required_count": audit.refit_required_count,
                    "skip_count": audit.skip_count,
                    "failed_cell_count": audit.failed_cell_count,
                    "shrunk_refit_count": audit.shrunk_refit_count,
                    "histogram": audit.histogram,
                },
                "refresh_policy": refresh_policy,
            }),
        ))
    }

    fn tick_emergency_eviction(&self) -> Result<TickRunRecord, HealError> {
        let runtime = context_graph_mejepa_hygiene::runtime_config(
            self.db.clone(),
            self.config.hygiene_archive_root.clone(),
        )
        .map_err(map_hygiene_error)?;
        let env = context_graph_mejepa_hygiene::HygieneEnv::try_new(runtime)
            .map_err(map_hygiene_error)?;
        let before = context_graph_mejepa_hygiene::quota_status(&env).map_err(map_hygiene_error)?;
        let previous = latest_emergency_eviction_decision(self.storage.as_ref())?;
        let initial_action = classify_emergency_eviction(
            before.total_used_bytes,
            None,
            self.config.emergency_eviction_threshold_bytes,
            previous.as_ref(),
        )?;
        if before.total_used_bytes < self.config.emergency_eviction_threshold_bytes
            || initial_action == EmergencyEvictionAction::SuppressedRepeat
        {
            let decision = EmergencyEvictionDecision {
                total_used_bytes_before: before.total_used_bytes,
                total_used_bytes_after: None,
                threshold_bytes: self.config.emergency_eviction_threshold_bytes,
                action: initial_action,
                recorded_at_unix_ms: chrono::Utc::now().timestamp_millis(),
                source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
            };
            let decision_key =
                persist_emergency_eviction_decision(self.storage.as_ref(), &decision)?;
            return Ok(TickRunRecord::success(
                TickName::EmergencyEviction,
                if decision.action == EmergencyEvictionAction::SuppressedRepeat {
                    "emergency_quota_eviction_suppressed_repeat"
                } else {
                    "quota_below_emergency_threshold"
                },
                json!({
                    "source_of_truth_cf": context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE,
                    "total_used_bytes": before.total_used_bytes,
                    "threshold_bytes": self.config.emergency_eviction_threshold_bytes,
                    "total_quota_bytes": before.total_quota_bytes,
                    "decision": decision,
                    "decision_key_hex": hex::encode(decision_key),
                }),
            ));
        }
        let report =
            context_graph_mejepa_hygiene::quota_check_and_evict(&env).map_err(map_hygiene_error)?;
        let final_action = classify_emergency_eviction(
            report.before.total_used_bytes,
            Some(report.after.total_used_bytes),
            self.config.emergency_eviction_threshold_bytes,
            previous.as_ref(),
        )?;
        let decision = EmergencyEvictionDecision {
            total_used_bytes_before: report.before.total_used_bytes,
            total_used_bytes_after: Some(report.after.total_used_bytes),
            threshold_bytes: self.config.emergency_eviction_threshold_bytes,
            action: final_action,
            recorded_at_unix_ms: chrono::Utc::now().timestamp_millis(),
            source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
        };
        let decision_key = persist_emergency_eviction_decision(self.storage.as_ref(), &decision)?;
        Ok(TickRunRecord::success(
            TickName::EmergencyEviction,
            if decision.action == EmergencyEvictionAction::UnrecoverableAlert {
                "emergency_quota_unrecoverable_alert"
            } else {
                "emergency_quota_eviction_run"
            },
            json!({
                "source_of_truth_cf": context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE,
                "before": report.before,
                "after": report.after,
                "evicted": report.evicted,
                "decision": decision,
                "decision_key_hex": hex::encode(decision_key),
            }),
        ))
    }

    fn tick_telemetry_feedback(&self) -> Result<TickRunRecord, HealError> {
        let operator_override_calibration = track_operator_override_recalibration(
            self.storage.as_ref(),
            self.system_cost_counters
                .snapshot()
                .operator_override_sampler_applied_count_total,
        )?;
        let Some(telemetry) = latest_production_telemetry(self.storage.as_ref())? else {
            return Ok(TickRunRecord::success(
                TickName::TelemetryFeedback,
                "production_telemetry_absent",
                json!({
                    "source_of_truth_cfs": [
                        context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY,
                        context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS,
                        context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS
                    ],
                    "queued_action_count": 0,
                    "operator_override_calibration": operator_override_calibration,
                    "actions": [],
                }),
            ));
        };
        let heal_report_count_before = self
            .storage
            .count_cf(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS)?;
        let mut actions = Vec::new();
        for action in classify_telemetry_feedback_actions(&telemetry)? {
            actions.push(persist_telemetry_feedback_action(
                self.storage.as_ref(),
                &telemetry,
                action,
            )?);
        }
        let heal_report_count_after = self
            .storage
            .count_cf(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS)?;
        let action_name = if actions.is_empty() {
            "telemetry_feedback_within_thresholds"
        } else {
            "telemetry_feedback_actions_queued"
        };
        Ok(TickRunRecord::success(
            TickName::TelemetryFeedback,
            action_name,
            json!({
                "source_of_truth_cfs": [
                    context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY,
                    context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS,
                    context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS
                ],
                "latest_production_telemetry": telemetry,
                "coverage_gate": {
                    "target": TELEMETRY_CONFORMAL_TARGET_COVERAGE,
                    "min": TELEMETRY_CONFORMAL_COVERAGE_MIN,
                    "max": TELEMETRY_CONFORMAL_COVERAGE_MAX,
                    "ece_max": TELEMETRY_CONFORMAL_ECE_MAX
                },
                "prediction_oracle_agreement_min": TELEMETRY_ORACLE_AGREEMENT_MIN,
                "heal_report_count_before": heal_report_count_before,
                "heal_report_count_after": heal_report_count_after,
                "queued_action_count": actions.iter().filter(|action| !action.already_queued).count(),
                "operator_override_calibration": operator_override_calibration,
                "actions": actions,
            }),
        ))
    }

    fn write_drift_alert_if_needed(
        &self,
        severity: DriftSeverity,
        catastrophic_cell_count: usize,
        lora_cell_count: usize,
        empirical_coverage: Option<f32>,
    ) -> Result<Option<serde_json::Value>, HealError> {
        if matches!(
            severity,
            DriftSeverity::WarmupNotReady | DriftSeverity::Healthy
        ) && catastrophic_cell_count == 0
            && lora_cell_count == 0
        {
            return Ok(None);
        }
        let latest_telemetry = latest_production_telemetry(self.storage.as_ref())?;
        let tau_drift_fraction = latest_telemetry
            .as_ref()
            .map(|telemetry| (1.0 - telemetry.gtau_pass_rate).max(0.0))
            .or_else(|| empirical_coverage.map(|coverage| (1.0 - coverage).max(0.0)))
            .unwrap_or(0.0);
        let last_promotion_at = self
            .pipeline
            .status
            .lock()
            .map(|status| status.last_promotion_at)
            .unwrap_or(-1);
        let model_age_days = if last_promotion_at > 0 {
            ((chrono::Utc::now().timestamp() - last_promotion_at).max(0) / 86_400) as u32
        } else {
            0
        };
        let per_cell_only = matches!(
            severity,
            DriftSeverity::WarmupNotReady | DriftSeverity::Healthy
        ) && (catastrophic_cell_count > 0 || lora_cell_count > 0);
        let attribution =
            classify_drift_attribution(per_cell_only, tau_drift_fraction, model_age_days, false)?;
        let alert_severity =
            if catastrophic_cell_count > 0 || severity == DriftSeverity::Catastrophic {
                DriftSeverity::Catastrophic
            } else if matches!(
                severity,
                DriftSeverity::WarmupNotReady | DriftSeverity::Healthy
            ) {
                DriftSeverity::Hard
            } else {
                severity
            };
        let alert = write_operator_alert(
            self.storage.as_ref(),
            alert_severity,
            format!("ME-JEPA drift detected: {:?}", alert_severity),
            format!(
                "global={severity:?}; catastrophic_cells={catastrophic_cell_count}; lora_cells={lora_cell_count}; tau_drift_fraction={tau_drift_fraction:.6}"
            ),
            Some(attribution),
        )?;
        Ok(Some(json!(alert)))
    }

    fn maybe_verify_witness_chain_integrity(
        &mut self,
    ) -> Result<WitnessIntegrityTickRecord, HealError> {
        let observation_counter = self.pipeline.status.lock().unwrap().observation_counter;
        let recorded_at_unix_ms = chrono::Utc::now().timestamp_millis();
        if observation_counter == 0 {
            return Ok(WitnessIntegrityTickRecord {
                observation_counter,
                checked: false,
                reason: "no_observations_seen".to_string(),
                entry_count: None,
                recorded_at_unix_ms,
                source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
            });
        }
        if !observation_counter.is_multiple_of(self.pipeline.config.integrity_check_period) {
            return Ok(WitnessIntegrityTickRecord {
                observation_counter,
                checked: false,
                reason: "not_due".to_string(),
                entry_count: None,
                recorded_at_unix_ms,
                source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
            });
        }
        let counter_key = observation_counter.to_string();
        let key = policy_key(&["phase_e", "witness-integrity-check", &counter_key])?;
        if let Some(record) =
            load_policy_record::<WitnessIntegrityTickRecord>(self.storage.as_ref(), &key)?
        {
            return Ok(record);
        }
        let integrity = match verify(
            &mut self.pipeline.integrity_checker,
            &self.pipeline.witness_chain,
            self.storage.as_ref(),
            &self.pipeline.status,
        ) {
            Ok(report) => report,
            Err(err) => {
                let _ = write_operator_alert(
                    self.storage.as_ref(),
                    DriftSeverity::Catastrophic,
                    "ME-JEPA witness-chain integrity failed",
                    err.to_string(),
                    None,
                );
                return Err(err);
            }
        };
        let record = WitnessIntegrityTickRecord {
            observation_counter,
            checked: true,
            reason: "verified".to_string(),
            entry_count: Some(integrity.entry_count),
            recorded_at_unix_ms,
            source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
        };
        persist_policy_record(self.storage.as_ref(), &key, &record)?;
        Ok(record)
    }

    fn record_tick(&mut self, tick: TickName, record: TickRunRecord) -> Result<(), HealError> {
        let ticker = tick.as_str().to_string();
        self.system_cost_counters
            .record_heal_ticker_run(&ticker, record.duration_ms)
            .map_err(|err| {
                HealError::invalid(
                    "self_optim_scheduler.heal_ticker_telemetry",
                    err.to_string(),
                )
            })?;
        *self.ticker_counts.entry(ticker.clone()).or_insert(0) += 1;
        self.last_ticker = Some(ticker.clone());
        if record.ok {
            self.last_success.insert(ticker, record);
        } else {
            self.last_error.insert(ticker, record);
        }
        self.write_status("running")
    }

    fn write_status(&self, status: &str) -> Result<(), HealError> {
        write_json_atomic_readback(&self.config.status_path, &self.snapshot(status))
    }
}

pub fn read_status_snapshot(path: &Path) -> Result<SchedulerStatusSnapshot, HealError> {
    let bytes = fs::read(path).map_err(|err| HealError::io("read", path, err))?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn write_json_atomic_readback<T: Serialize>(path: &Path, value: &T) -> Result<(), HealError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| HealError::io("create_dir_all", parent, err))?;
    }
    let tmp = tmp_path_for(path)?;
    let bytes = serde_json::to_vec_pretty(value)?;
    {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)
            .map_err(|err| HealError::io("open", &tmp, err))?;
        file.write_all(&bytes)
            .map_err(|err| HealError::io("write", &tmp, err))?;
        file.sync_all()
            .map_err(|err| HealError::io("sync_all", &tmp, err))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))
            .map_err(|err| HealError::io("set_permissions", &tmp, err))?;
    }
    fs::rename(&tmp, path).map_err(|err| HealError::io("rename", path, err))?;
    if let Some(parent) = path.parent() {
        if let Ok(dir) = File::open(parent) {
            dir.sync_all()
                .map_err(|err| HealError::io("sync_all", parent, err))?;
        }
    }
    let readback = fs::read(path).map_err(|err| HealError::io("read", path, err))?;
    if readback != bytes {
        return Err(HealError::invalid(
            "scheduler.status_readback",
            format!("readback mismatch for {}", path.display()),
        ));
    }
    Ok(())
}

fn active_learning_kind_summary(kind: &ActiveLearningKind) -> serde_json::Value {
    match kind {
        ActiveLearningKind::Uncertainty => json!({"type": "uncertainty"}),
        ActiveLearningKind::OutOfDistribution => json!({"type": "out_of_distribution"}),
        ActiveLearningKind::AgentSurprise {
            prediction_id,
            severity_score,
        } => json!({
            "type": "agent_surprise",
            "prediction_id": hex::encode(prediction_id.0),
            "severity_score": severity_score,
        }),
        ActiveLearningKind::EwcProtectionViolation {
            violation_id,
            cell_id,
            projected_fisher_displacement,
            budget,
        } => json!({
            "type": "ewc_protection_violation",
            "violation_id": violation_id,
            "cell_id": cell_id,
            "projected_fisher_displacement": projected_fisher_displacement,
            "budget": budget,
        }),
        ActiveLearningKind::ColdCellTargetedCorpus {
            cell_id,
            abstain_count,
        } => json!({
            "type": "cold_cell_targeted_corpus",
            "cell_id": cell_id,
            "abstain_count": abstain_count,
        }),
        ActiveLearningKind::UnknownFingerprint { candidate } => json!({
            "type": "unknown_fingerprint",
            "candidate_id": hex::encode(candidate.candidate_id),
            "prediction_id": hex::encode(candidate.prediction_id.0),
            "task_id": candidate.task_id.0,
            "active_learning_priority": candidate.active_learning_priority,
            "ood_score": candidate.ood_score,
            "embedder_disagreement_score": candidate.embedder_disagreement_score,
            "embedder_count": candidate.observation_by_embedder.len(),
            "nearest_fingerprint_count": candidate.nearest_fingerprints.len(),
        }),
        ActiveLearningKind::NovelCluster { candidate } => json!({
            "type": "novel_cluster",
            "candidate_id": hex::encode(candidate.candidate_id),
            "prediction_id": hex::encode(candidate.prediction_id.0),
            "task_id": candidate.task_id.0,
            "active_learning_priority": candidate.active_learning_priority,
            "novelty_score": candidate.novelty_score,
            "nearest_existing_cell_id": candidate.nearest_existing_cell_id,
            "nearest_existing_distance": candidate.nearest_existing_distance,
            "embedder_count": candidate.observation_by_embedder.len(),
        }),
        ActiveLearningKind::OodHarvest {
            prediction_id,
            priority_weight,
        } => json!({
            "type": "ood_harvest",
            "prediction_id": hex::encode(prediction_id.0),
            "priority_weight": priority_weight,
        }),
    }
}

fn tmp_path_for(path: &Path) -> Result<PathBuf, HealError> {
    let file_name = path.file_name().ok_or_else(|| {
        HealError::invalid(
            "scheduler.status_path",
            format!("path {} does not have a file name", path.display()),
        )
    })?;
    let mut tmp = path.to_path_buf();
    tmp.set_file_name(format!("{}.tmp", file_name.to_string_lossy()));
    Ok(tmp)
}

fn bind_test_outcomes(
    source_root: &Path,
    db: Arc<DB>,
) -> Result<TestOutcomeBindingReport, HealError> {
    let mut report = TestOutcomeBindingReport {
        source_root: source_root.to_path_buf(),
        scanned_files: 0,
        events_scanned: 0,
        events_bound: 0,
        events_missing_prediction: 0,
        events_skipped_non_hard_label: 0,
        bound_keys: Vec::new(),
        verification_keys: Vec::new(),
        sampler_reward_keys: Vec::new(),
        sampler_reward_statuses: Vec::new(),
        sampler_reward_write_dispositions: Vec::new(),
        signal_drop_event_ids: Vec::new(),
    };
    if !source_root.exists() {
        return Ok(report);
    }
    if !source_root.is_dir() {
        return Err(HealError::invalid(
            "test_outcome_root",
            format!("{} exists but is not a directory", source_root.display()),
        ));
    }
    let infer_store = RocksDbInferStore::new(db.clone());
    let mut entries = fs::read_dir(source_root)
        .map_err(|err| HealError::io("read_dir", source_root, err))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| HealError::io("read_dir", source_root, err))?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }
        report.scanned_files += 1;
        let raw = fs::read_to_string(&path).map_err(|err| HealError::io("read", &path, err))?;
        for (line_idx, line) in raw.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            report.events_scanned += 1;
            let event: TestOutcomeEvent = serde_json::from_str(line).map_err(|err| {
                HealError::invalid(
                    "test_outcome_event.json",
                    format!("{}:{} invalid JSON: {err}", path.display(), line_idx + 1),
                )
            })?;
            event.validate().map_err(|message| {
                HealError::invalid(
                    "test_outcome_event.validate",
                    format!("{}:{} {message}", path.display(), line_idx + 1),
                )
            })?;
            let Some(actual_test_value) = event.actual_test_value() else {
                report.events_skipped_non_hard_label += 1;
                continue;
            };
            let session_id = decode_hex16(&event.session_id)
                .map_err(|message| HealError::invalid("test_outcome_event.session_id", message))?;
            let predictions = infer_store
                .read_live_predictions(session_id, 1)
                .map_err(|err| {
                    HealError::invalid(
                        "test_outcome_event.prediction_lookup",
                        format!("{}: {err}", err.code()),
                    )
                })?;
            let Some(prediction) = predictions.first() else {
                let signal_drop_event_id = persist_missing_prediction_signal_drop(
                    db.as_ref(),
                    &event,
                    &path,
                    line_idx + 1,
                    line,
                )?;
                report.events_missing_prediction += 1;
                report.signal_drop_event_ids.push(signal_drop_event_id);
                continue;
            };
            let example = CalibrationExample {
                language: prediction.language,
                predicted_test_pass: prediction.predicted_test_pass.clone(),
                actual_test_pass: vec![actual_test_value; prediction.predicted_test_pass.len()],
            };
            let key = format!(
                "test-outcome-bind::{}::{}",
                event.session_id, event.event_id
            );
            put_oracle_binding_readback(db.as_ref(), key.as_bytes(), &example)?;
            let verification =
                scheduler_prediction_verification_record(prediction, &event, &path, event.ts);
            let verification_key =
                put_prediction_verification_readback(db.as_ref(), &verification)?;
            let observed_oracle_outcome =
                scheduler_observed_oracle_outcome(&event).ok_or_else(|| {
                    HealError::invalid(
                        "test_outcome_event.reward_outcome",
                        "hard-label event did not map to an oracle outcome",
                    )
                })?;
            let reward = sampler_reward_from_prediction_outcome(
                prediction,
                observed_oracle_outcome,
                event.event_id.clone(),
                event.ts,
            )
            .map_err(|err| {
                HealError::invalid(
                    "test_outcome_event.sampler_reward",
                    format!("{}: {err}", err.code()),
                )
            })?;
            let reward_readback = persist_sampler_reward_signal_readback(db.as_ref(), &reward)
                .map_err(|err| {
                    HealError::invalid(
                        "test_outcome_event.sampler_reward",
                        format!("{}: {err}", err.code()),
                    )
                })?;
            report.events_bound += 1;
            report.bound_keys.push(key);
            report.verification_keys.push(verification_key);
            report
                .sampler_reward_keys
                .push(hex::encode(sampler_reward_key(
                    reward_readback.row.prediction_id,
                )));
            report
                .sampler_reward_statuses
                .push(reward_readback.row.status_code().to_string());
            report
                .sampler_reward_write_dispositions
                .push(format!("{:?}", reward_readback.disposition));
        }
    }
    Ok(report)
}

impl TestOutcomeEvent {
    fn validate(&self) -> Result<(), String> {
        if self.event_id.len() != 32 || !self.event_id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("event_id must be exactly 32 hexadecimal characters".to_string());
        }
        if self.session_id.len() != 32 || !self.session_id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("session_id must be exactly 32 hexadecimal characters".to_string());
        }
        if self.ts < 0 {
            return Err("ts must be non-negative".to_string());
        }
        if self.test_id.trim().is_empty() {
            return Err("test_id must be non-empty".to_string());
        }
        if !matches!(self.outcome.as_str(), "pass" | "fail" | "skipped") {
            return Err(format!("unsupported outcome {}", self.outcome));
        }
        Ok(())
    }

    fn actual_test_value(&self) -> Option<f32> {
        match self.outcome.as_str() {
            "pass" => Some(1.0),
            "fail" => Some(0.0),
            "skipped" => None,
            _ => None,
        }
    }
}

fn decode_hex16(value: &str) -> Result<[u8; 16], String> {
    let mut out = [0u8; 16];
    hex::decode_to_slice(value, &mut out).map_err(|err| format!("hex decode failed: {err}"))?;
    Ok(out)
}

fn put_oracle_binding_readback(
    db: &DB,
    key: &[u8],
    example: &CalibrationExample,
) -> Result<(), HealError> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS)
        .ok_or_else(|| {
            HealError::invalid(
                "test_outcome_event.oracle_cf",
                "missing CF_MEJEPA_ORACLE_VERDICTS",
            )
        })?;
    let value = bincode::serialize(example)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, &value, &opts)?;
    let readback = db.get_cf(cf, key)?.ok_or_else(|| {
        HealError::invalid(
            "test_outcome_event.oracle_readback",
            "missing CF_MEJEPA_ORACLE_VERDICTS row after write",
        )
    })?;
    if readback != value {
        return Err(HealError::invalid(
            "test_outcome_event.oracle_readback",
            "CF_MEJEPA_ORACLE_VERDICTS readback bytes differ",
        ));
    }
    let decoded: CalibrationExample = bincode::deserialize(&readback)?;
    if decoded != *example {
        return Err(HealError::invalid(
            "test_outcome_event.oracle_readback",
            "CF_MEJEPA_ORACLE_VERDICTS decoded readback differs",
        ));
    }
    Ok(())
}

fn scheduler_prediction_verification_record(
    prediction: &crate::types::RealityPrediction,
    event: &TestOutcomeEvent,
    evidence_path: &Path,
    now_ms: i64,
) -> SchedulerPredictionVerificationRecord {
    let observed = scheduler_observed_outcome(event);
    let predicted = scheduler_predicted_outcome(prediction, &event.test_id);
    let agreement = scheduler_agreement(predicted.as_deref(), observed.as_deref());
    let prediction_id = hex::encode(prediction.prediction_id);
    let verification_id = stable_test_outcome_id(&[
        &event.session_id,
        &event.event_id,
        &prediction_id,
        &event.test_id,
        &now_ms.to_string(),
    ]);
    SchedulerPredictionVerificationRecord {
        schema_version: 1,
        verification_id,
        prediction_id,
        ts: now_ms,
        session_id: event.session_id.clone(),
        task_id: prediction.task_id.0.clone(),
        test_id: event.test_id.clone(),
        tool_use_id: event.tool_use_id.clone(),
        command: event.command.clone(),
        observed_outcome: observed.unwrap_or_else(|| "unknown".to_string()),
        predicted_outcome: predicted.unwrap_or_else(|| "unknown".to_string()),
        agreement: agreement.to_string(),
        evidence_path: evidence_path.display().to_string(),
        event_id: event.event_id.clone(),
        created_at_unix_ms: now_ms,
        source: "self_optimization_observe_test_outcome".to_string(),
        source_prediction_cf: context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        source_verification_cf: context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS
            .to_string(),
    }
}

fn put_prediction_verification_readback(
    db: &DB,
    record: &SchedulerPredictionVerificationRecord,
) -> Result<String, HealError> {
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS)
        .ok_or_else(|| {
            HealError::invalid(
                "test_outcome_event.verification_cf",
                "missing CF_MEJEPA_PREDICTION_VERIFICATIONS",
            )
        })?;
    let key = scheduler_prediction_verification_key(record);
    if let Some(existing) = db.get_cf(cf, &key)? {
        let decoded: SchedulerPredictionVerificationRecord = serde_json::from_slice(&existing)?;
        if decoded.session_id != record.session_id
            || decoded.event_id != record.event_id
            || decoded.prediction_id != record.prediction_id
            || decoded.test_id != record.test_id
        {
            return Err(HealError::invalid(
                "test_outcome_event.verification_readback",
                "existing CF_MEJEPA_PREDICTION_VERIFICATIONS row conflicts with test outcome",
            ));
        }
        return Ok(String::from_utf8_lossy(&key).to_string());
    }

    let value = serde_json::to_vec(record)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, &key, &value, &opts)?;
    db.flush_wal(true)?;
    db.flush_cf(cf)?;
    let readback = db.get_cf(cf, &key)?.ok_or_else(|| {
        HealError::invalid(
            "test_outcome_event.verification_readback",
            "missing CF_MEJEPA_PREDICTION_VERIFICATIONS row after write",
        )
    })?;
    if readback != value {
        return Err(HealError::invalid(
            "test_outcome_event.verification_readback",
            "CF_MEJEPA_PREDICTION_VERIFICATIONS readback bytes differ",
        ));
    }
    let decoded: SchedulerPredictionVerificationRecord = serde_json::from_slice(&readback)?;
    if decoded.verification_id != record.verification_id {
        return Err(HealError::invalid(
            "test_outcome_event.verification_readback",
            "CF_MEJEPA_PREDICTION_VERIFICATIONS decoded readback differs",
        ));
    }
    Ok(String::from_utf8_lossy(&key).to_string())
}

fn persist_missing_prediction_signal_drop(
    db: &DB,
    event: &TestOutcomeEvent,
    evidence_path: &Path,
    line_no: usize,
    raw_line: &str,
) -> Result<String, HealError> {
    let mut hasher = Sha256::new();
    hasher.update(raw_line.as_bytes());
    let input_sha256: [u8; 32] = hasher.finalize().into();
    let artifact_id = format!(
        "test-outcome-bind::{}::{}",
        event.session_id, event.event_id
    );
    let mut context = BTreeMap::new();
    context.insert("session_id".to_string(), event.session_id.clone());
    context.insert("event_id".to_string(), event.event_id.clone());
    context.insert("tool_use_id".to_string(), event.tool_use_id.clone());
    context.insert("test_id".to_string(), event.test_id.clone());
    context.insert("outcome".to_string(), event.outcome.clone());
    context.insert(
        "evidence_path".to_string(),
        evidence_path.display().to_string(),
    );
    context.insert("line_no".to_string(), line_no.to_string());
    context.insert(
        "source_prediction_cf".to_string(),
        context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
    );
    context.insert(
        "source_signal_drop_cf".to_string(),
        context_graph_mejepa_cf::CF_MEJEPA_SIGNAL_DROP_LOG.to_string(),
    );
    let entry = SignalDropLogEntry::new(
        event.ts,
        1,
        "post_tool_test_outcomes",
        "self_optimization.observe.bind_test_outcomes",
        artifact_id,
        "MEJEPA_TEST_OUTCOME_NO_PREDICTION",
        format!(
            "no RealityPrediction found for session_id={} while binding event_id={}",
            event.session_id, event.event_id
        ),
        SignalDropSeverity::Warning,
        "wait for a live prediction for this session or inspect hook ordering",
        Some(input_sha256),
        context,
    )
    .map_err(|err| {
        HealError::invalid(
            "test_outcome_event.signal_drop",
            format!("{}: {err}", err.code()),
        )
    })?;
    persist_signal_drop_log_entry(db, &entry).map_err(|err| {
        HealError::invalid(
            "test_outcome_event.signal_drop",
            format!("{}: {err}", err.code()),
        )
    })?;
    Ok(hex::encode(entry.event_id))
}

fn scheduler_observed_outcome(event: &TestOutcomeEvent) -> Option<String> {
    match event.outcome.as_str() {
        "pass" => Some("pass".to_string()),
        "fail" => Some("fail".to_string()),
        _ => None,
    }
}

fn scheduler_observed_oracle_outcome(event: &TestOutcomeEvent) -> Option<OracleOutcome> {
    match event.outcome.as_str() {
        "pass" => Some(OracleOutcome::Pass),
        "fail" => Some(OracleOutcome::Fail),
        _ => None,
    }
}

fn scheduler_predicted_outcome(
    prediction: &crate::types::RealityPrediction,
    test_id: &str,
) -> Option<String> {
    for item in &prediction.predicted_failed_tests {
        if item.test_id.0 == test_id {
            return scheduler_test_outcome_to_label(item.predicted_outcome);
        }
    }
    match prediction.verdict {
        crate::types::Verdict::Pass => Some("pass".to_string()),
        crate::types::Verdict::Fail | crate::types::Verdict::GuardRejected => {
            Some("fail".to_string())
        }
        crate::types::Verdict::OutOfDistribution | crate::types::Verdict::Abstain => None,
    }
}

fn scheduler_test_outcome_to_label(outcome: crate::types::TestOutcome) -> Option<String> {
    match outcome {
        crate::types::TestOutcome::Pass => Some("pass".to_string()),
        crate::types::TestOutcome::Fail | crate::types::TestOutcome::Error => {
            Some("fail".to_string())
        }
        crate::types::TestOutcome::Skip | crate::types::TestOutcome::Flaky => None,
    }
}

fn scheduler_agreement(predicted: Option<&str>, observed: Option<&str>) -> &'static str {
    match (predicted, observed) {
        (Some(predicted), Some(observed)) if predicted == observed => "confirmed",
        (Some(_), Some(_)) => "refuted",
        _ => "inconclusive",
    }
}

fn scheduler_prediction_verification_key(
    record: &SchedulerPredictionVerificationRecord,
) -> Vec<u8> {
    format!(
        "{}:{}:{}",
        record.session_id, record.event_id, record.prediction_id
    )
    .into_bytes()
}

fn stable_test_outcome_id(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    hex::encode(hasher.finalize())[..32].to_string()
}

pub fn operator_override_calibration_counter_key() -> Result<Vec<u8>, HealError> {
    policy_key(&["phase_g", "operator-override-calibration-counter"])
}

pub fn track_operator_override_recalibration(
    storage: &HealRocksStore,
    raw_sampler_counter: u64,
) -> Result<OperatorOverrideCalibrationCounter, HealError> {
    let key = operator_override_calibration_counter_key()?;
    let previous = load_policy_record::<OperatorOverrideCalibrationCounter>(storage, &key)?;
    let previous_heal_report_key_hex = previous
        .as_ref()
        .and_then(|record| record.heal_report_key_hex.clone());
    let previous_recalibration_total = previous
        .as_ref()
        .and_then(|record| record.last_recalibration_total_applied_count);
    let previous_recalibration_override_count = previous
        .as_ref()
        .and_then(|record| record.last_recalibration_override_count);
    let (total_applied_count, overrides_consumed_since_calibration, counter_reset_detected) =
        match &previous {
            Some(previous) => {
                let counter_reset = raw_sampler_counter < previous.last_observed_sampler_counter;
                let delta = if counter_reset {
                    raw_sampler_counter
                } else {
                    raw_sampler_counter.saturating_sub(previous.last_observed_sampler_counter)
                };
                (
                    previous.total_applied_count.saturating_add(delta),
                    previous
                        .overrides_consumed_since_calibration
                        .saturating_add(delta),
                    counter_reset,
                )
            }
            None => (raw_sampler_counter, raw_sampler_counter, false),
        };
    let mut counter = OperatorOverrideCalibrationCounter {
        total_applied_count,
        last_observed_sampler_counter: raw_sampler_counter,
        overrides_consumed_since_calibration,
        threshold: OPERATOR_OVERRIDE_RECALIBRATION_THRESHOLD,
        recalibration_queued: false,
        already_queued: false,
        counter_reset_detected,
        heal_report_key_hex: previous_heal_report_key_hex,
        last_recalibration_total_applied_count: previous_recalibration_total,
        last_recalibration_override_count: previous_recalibration_override_count,
        source_of_truth_cfs: vec![
            context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
            context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS.to_string(),
        ],
        recorded_at_unix_ms: chrono::Utc::now().timestamp_millis(),
    };
    let unqueued_override_count = counter
        .last_recalibration_total_applied_count
        .map(|last_total| counter.total_applied_count.saturating_sub(last_total))
        .unwrap_or(counter.overrides_consumed_since_calibration);
    if unqueued_override_count > OPERATOR_OVERRIDE_RECALIBRATION_THRESHOLD {
        counter.recalibration_queued = true;
        let report_key = operator_override_recalibration_report_key(counter.total_applied_count)?;
        counter.heal_report_key_hex = Some(hex::encode(&report_key));
        counter.last_recalibration_total_applied_count = Some(counter.total_applied_count);
        counter.last_recalibration_override_count = Some(unqueued_override_count);
        if storage
            .get_cf(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS, &report_key)?
            .is_some()
        {
            counter.already_queued = true;
        } else {
            persist_operator_override_recalibration_report(
                storage,
                &counter,
                &report_key,
                unqueued_override_count,
            )?;
        }
    } else if counter.last_recalibration_total_applied_count == Some(counter.total_applied_count)
        && counter.heal_report_key_hex.is_some()
    {
        counter.recalibration_queued = true;
        counter.already_queued = true;
    }
    persist_policy_record(storage, &key, &counter)?;
    Ok(counter)
}

fn operator_override_recalibration_report_key(
    total_applied_count: u64,
) -> Result<Vec<u8>, HealError> {
    policy_key(&[
        "operator-override-recalibration",
        &format!("{:020}", total_applied_count),
    ])
}

fn persist_operator_override_recalibration_report(
    storage: &HealRocksStore,
    counter: &OperatorOverrideCalibrationCounter,
    report_key: &[u8],
    override_count: u64,
) -> Result<(), HealError> {
    if storage
        .get_cf(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS, report_key)?
        .is_some()
    {
        return Ok(());
    }
    let report = operator_override_recalibration_heal_report(counter, override_count)?;
    let encoded = encode_value(&report)?;
    storage.put_cf_readback(
        context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS,
        report_key,
        &encoded,
    )?;
    let readback = storage
        .get_cf(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS, report_key)?
        .ok_or_else(|| {
            HealError::invalid(
                "operator_override_recalibration.heal_report_readback",
                "missing heal report after write",
            )
        })?;
    if readback != encoded {
        return Err(HealError::invalid(
            "operator_override_recalibration.heal_report_readback",
            "heal report readback bytes differ",
        ));
    }
    let decoded: HealReport = decode_value(&readback)?;
    if decoded.trigger_reason != TriggerReason::OperatorOverrideRecalibration {
        return Err(HealError::invalid(
            "operator_override_recalibration.heal_report_readback",
            "heal report trigger reason differs after readback",
        ));
    }
    Ok(())
}

fn operator_override_recalibration_heal_report(
    counter: &OperatorOverrideCalibrationCounter,
    override_count: u64,
) -> Result<HealReport, HealError> {
    let digest = operator_override_recalibration_digest(counter, override_count);
    let sample_count = usize::try_from(override_count).map_err(|_| {
        HealError::invalid(
            "operator_override_recalibration.sample_count",
            "override counter exceeds usize",
        )
    })?;
    let eval = HoldoutEval::try_new(
        TELEMETRY_CONFORMAL_TARGET_COVERAGE,
        TELEMETRY_ORACLE_AGREEMENT_MIN,
        0.0,
        sample_count,
        digest,
    )?;
    Ok(HealReport {
        mode_winner: ModeWinner::AUnchangedNoWinner,
        mode_a_score: eval.clone(),
        mode_b_score: eval.clone(),
        mode_c_score: eval,
        mode_c_weights: (1.0, 0.0),
        weights_sha_winner: digest,
        evaluation_summary_sha: digest,
        witness_chain_offset: 0,
        promotion_latency_seconds: 0,
        status_change: StatusChange::Degraded,
        trigger_reason: TriggerReason::OperatorOverrideRecalibration,
    })
}

fn operator_override_recalibration_digest(
    counter: &OperatorOverrideCalibrationCounter,
    override_count: u64,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"operator-override-recalibration-v1");
    hasher.update(counter.total_applied_count.to_be_bytes());
    hasher.update(counter.last_observed_sampler_counter.to_be_bytes());
    hasher.update(counter.overrides_consumed_since_calibration.to_be_bytes());
    hasher.update(counter.threshold.to_be_bytes());
    hasher.update(override_count.to_be_bytes());
    let digest = hasher.finalize();
    let mut out = [0_u8; 32];
    out.copy_from_slice(&digest);
    out
}

fn classify_telemetry_feedback_actions(
    telemetry: &ProductionTelemetryWindow,
) -> Result<Vec<TelemetryFeedbackAction>, HealError> {
    telemetry
        .validate()
        .map_err(|err| HealError::invalid("production_telemetry", err.to_string()))?;
    let mut actions = Vec::new();
    let recorded_at_unix_ms = chrono::Utc::now().timestamp_millis();
    if telemetry.conformal_ece > TELEMETRY_CONFORMAL_ECE_MAX {
        actions.push(TelemetryFeedbackAction {
            kind: TelemetryFeedbackActionKind::QueueRecalibration,
            window_id: telemetry.window_id.clone(),
            captured_at_unix_ms: telemetry.captured_at_unix_ms,
            reason: format!(
                "conformal ECE {:.6} implies coverage outside [{:.2}, {:.2}] around target {:.2}",
                telemetry.conformal_ece,
                TELEMETRY_CONFORMAL_COVERAGE_MIN,
                TELEMETRY_CONFORMAL_COVERAGE_MAX,
                TELEMETRY_CONFORMAL_TARGET_COVERAGE
            ),
            metric_name: "conformal_ece".to_string(),
            metric_value: telemetry.conformal_ece,
            threshold_min: None,
            threshold_max: Some(TELEMETRY_CONFORMAL_ECE_MAX),
            source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY.to_string(),
            heal_report_key_hex: String::new(),
            pending_promotion_id: None,
            already_queued: false,
            recorded_at_unix_ms,
        });
    }
    if telemetry.prediction_oracle_agreement < TELEMETRY_ORACLE_AGREEMENT_MIN {
        actions.push(TelemetryFeedbackAction {
            kind: TelemetryFeedbackActionKind::QueueFullRetrainCandidate,
            window_id: telemetry.window_id.clone(),
            captured_at_unix_ms: telemetry.captured_at_unix_ms,
            reason: format!(
                "prediction_oracle_agreement {:.6} is below {:.2}",
                telemetry.prediction_oracle_agreement, TELEMETRY_ORACLE_AGREEMENT_MIN
            ),
            metric_name: "prediction_oracle_agreement".to_string(),
            metric_value: telemetry.prediction_oracle_agreement,
            threshold_min: Some(TELEMETRY_ORACLE_AGREEMENT_MIN),
            threshold_max: None,
            source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY.to_string(),
            heal_report_key_hex: String::new(),
            pending_promotion_id: None,
            already_queued: false,
            recorded_at_unix_ms,
        });
    }
    if let Some(drift) = &telemetry.training_holdout_distribution_drift {
        if drift.alert_fired {
            actions.push(TelemetryFeedbackAction {
                kind: TelemetryFeedbackActionKind::TrainingHoldoutDriftAlert,
                window_id: telemetry.window_id.clone(),
                captured_at_unix_ms: telemetry.captured_at_unix_ms,
                reason: format!(
                    "{TRAINING_HOLDOUT_DRIFT_ALERT_CODE}: training-vs-holdout KL {:.6} exceeded {:.2} for {} batches",
                    drift.kl_divergence, drift.threshold, drift.sustained_batch_count
                ),
                metric_name: "training_holdout_kl_divergence".to_string(),
                metric_value: drift.kl_divergence,
                threshold_min: None,
                threshold_max: Some(drift.threshold),
                source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY
                    .to_string(),
                heal_report_key_hex: String::new(),
                pending_promotion_id: None,
                already_queued: false,
                recorded_at_unix_ms,
            });
        }
    }
    Ok(actions)
}

fn persist_telemetry_feedback_action(
    storage: &HealRocksStore,
    telemetry: &ProductionTelemetryWindow,
    mut action: TelemetryFeedbackAction,
) -> Result<TelemetryFeedbackAction, HealError> {
    let report_key = telemetry_feedback_report_key(action.kind, &telemetry.window_id);
    action.heal_report_key_hex = hex::encode(&report_key);
    if storage
        .get_cf(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS, &report_key)?
        .is_some()
    {
        action.already_queued = true;
        return Ok(action);
    }
    if action.kind == TelemetryFeedbackActionKind::QueueFullRetrainCandidate {
        action.pending_promotion_id = Some(queue_pending_retrain_request(
            storage,
            PendingPromotionKind::CatastrophicFullRetrainRequired {
                cell_key: format!("production_telemetry::{}", telemetry.window_id),
                metric_value: telemetry.prediction_oracle_agreement,
            },
            action.kind.trigger_reason(),
            format!(
                "production telemetry window {} fell below prediction-oracle agreement gate: {:.6} < {:.2}",
                telemetry.window_id,
                telemetry.prediction_oracle_agreement,
                TELEMETRY_ORACLE_AGREEMENT_MIN
            ),
        )?);
    }
    let report = telemetry_feedback_heal_report(telemetry, &action)?;
    let encoded = encode_value(&report)?;
    storage.put_cf_readback(
        context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS,
        &report_key,
        &encoded,
    )?;
    let readback = storage
        .get_cf(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS, &report_key)?
        .ok_or_else(|| {
            HealError::invalid(
                "telemetry_feedback.heal_report_readback",
                "missing heal report after write",
            )
        })?;
    if readback != encoded {
        return Err(HealError::invalid(
            "telemetry_feedback.heal_report_readback",
            "heal report readback bytes differ",
        ));
    }
    let decoded: HealReport = decode_value(&readback)?;
    if decoded.trigger_reason != action.kind.trigger_reason() {
        return Err(HealError::invalid(
            "telemetry_feedback.heal_report_readback",
            "heal report trigger reason differs after readback",
        ));
    }
    Ok(action)
}

fn telemetry_feedback_report_key(kind: TelemetryFeedbackActionKind, window_id: &str) -> Vec<u8> {
    format!(
        "telemetry-feedback/{}/{}",
        kind.as_str(),
        hex::encode(window_id.as_bytes())
    )
    .into_bytes()
}

fn telemetry_feedback_heal_report(
    telemetry: &ProductionTelemetryWindow,
    action: &TelemetryFeedbackAction,
) -> Result<HealReport, HealError> {
    let digest = telemetry_feedback_digest(telemetry, action)?;
    let inferred_coverage =
        (TELEMETRY_CONFORMAL_TARGET_COVERAGE - telemetry.conformal_ece).clamp(0.0, 1.0);
    let ood_kl_proxy = (1.0 - telemetry.ood_auc).max(0.0);
    let eval = HoldoutEval::try_new(
        inferred_coverage,
        telemetry.prediction_oracle_agreement,
        ood_kl_proxy,
        telemetry.sample_count,
        digest,
    )?;
    Ok(HealReport {
        mode_winner: ModeWinner::AUnchangedNoWinner,
        mode_a_score: eval.clone(),
        mode_b_score: eval.clone(),
        mode_c_score: eval,
        mode_c_weights: (1.0, 0.0),
        weights_sha_winner: digest,
        evaluation_summary_sha: digest,
        witness_chain_offset: 0,
        promotion_latency_seconds: 0,
        status_change: match action.kind {
            TelemetryFeedbackActionKind::QueueRecalibration => StatusChange::Degraded,
            TelemetryFeedbackActionKind::QueueFullRetrainCandidate => StatusChange::Retraining,
            TelemetryFeedbackActionKind::TrainingHoldoutDriftAlert => StatusChange::Degraded,
        },
        trigger_reason: action.kind.trigger_reason(),
    })
}

fn telemetry_feedback_digest(
    telemetry: &ProductionTelemetryWindow,
    action: &TelemetryFeedbackAction,
) -> Result<[u8; 32], HealError> {
    let mut hasher = Sha256::new();
    hasher.update(b"telemetry-feedback-v1");
    hasher.update(telemetry.window_id.as_bytes());
    hasher.update(telemetry.corpus_sha.as_bytes());
    hasher.update(telemetry.captured_at_unix_ms.to_be_bytes());
    hasher.update(serde_json::to_vec(action)?);
    Ok(hasher.finalize().into())
}

fn latest_production_telemetry(
    storage: &HealRocksStore,
) -> Result<Option<ProductionTelemetryWindow>, HealError> {
    let db = storage.db();
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY)
        .ok_or_else(|| {
            HealError::invalid(
                "production_telemetry.cf",
                "missing CF_MEJEPA_PRODUCTION_TELEMETRY",
            )
        })?;
    let mut latest: Option<ProductionTelemetryWindow> = None;
    for item in db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
        let (_key, value) = item?;
        let telemetry: ProductionTelemetryWindow = bincode::deserialize(&value)?;
        telemetry
            .validate()
            .map_err(|err| HealError::invalid("production_telemetry", err.to_string()))?;
        let replace = latest
            .as_ref()
            .map(|current| telemetry.captured_at_unix_ms > current.captured_at_unix_ms)
            .unwrap_or(true);
        if replace {
            latest = Some(telemetry);
        }
    }
    Ok(latest)
}

fn map_eval_error(err: crate::eval::EvalError) -> HealError {
    HealError::invalid(
        "self_optim_scheduler.eval",
        format!("{}: {err}", err.code()),
    )
}

fn map_tct_error(err: context_graph_mejepa_tct::TctError) -> HealError {
    HealError::invalid("self_optim_scheduler.tct", format!("{}: {err}", err.code()))
}

fn map_hygiene_error(err: context_graph_mejepa_hygiene::OpsError) -> HealError {
    HealError::invalid(
        "self_optim_scheduler.hygiene",
        format!("{}: {err}", err.code),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        AgentClaimGraph, ConformalInterval, ConformalSet, Language, OracleOutcome, PredictionId,
        PredictionProvenance, RealityPrediction, ReasoningClass, TaskId, Verdict, WitnessHash,
    };
    use rocksdb::{ColumnFamilyDescriptor, Options};
    use std::collections::BTreeMap;

    fn open_test_db(path: &Path) -> Arc<DB> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        let descriptors = context_graph_mejepa_cf::all_hygiene_referenced_cfs()
            .into_iter()
            .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default()))
            .collect::<Vec<_>>();
        Arc::new(DB::open_cf_descriptors(&opts, path, descriptors).expect("open test db"))
    }

    fn test_config(root: &Path) -> SelfOptimConfig {
        SelfOptimConfig {
            status_path: root.join("self_optimization_status.json"),
            hygiene_archive_root: root.join("archive"),
            witness_chain_path: root.join("witness-chain.bin"),
            test_outcome_root: root.join("test-outcomes"),
            ..SelfOptimConfig::default().with_all_periods(Duration::from_millis(20))
        }
    }

    fn test_prediction(session_id: [u8; 16]) -> RealityPrediction {
        RealityPrediction::try_new(RealityPrediction {
            prediction_id: [0x46; 16],
            witness_hash: WitnessHash([0x47; 32]),
            task_id: TaskId("scheduler-bind-test-task".to_string()),
            session_id,
            language: Language::Python,
            covered_chunks: Vec::new(),
            verdict: Verdict::Pass,
            confidence_interval: ConformalInterval {
                lower: 0.6,
                upper: 0.8,
                ..ConformalInterval::default()
            },
            predicted_oracle_pass: 0.95,
            predicted_test_pass: vec![0.9, 0.8],
            predicted_runtime_trace: [0.0; 32],
            ood_score: 0.1,
            outcome_set: ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2).unwrap(),
            calibrated_confidence: 0.7,
            degraded_status: false,
            granger_attestations: BTreeMap::from([("scheduler:test".to_string(), 0.9)]),
            predicted_failure_modes: Vec::new(),
            predicted_failed_tests: Vec::new(),
            predicted_works: Vec::new(),
            predicted_uncovered_paths: Vec::new(),
            predicted_flaky_tests: Vec::new(),
            guard_violations: Vec::new(),
            per_slot_ood_reasons: Vec::new(),
            closest_exemplars: Vec::new(),
            predicted_edge_cases: Vec::new(),
            predicted_latent_bugs: Vec::new(),
            predicted_tech_debt_added: Vec::new(),
            predicted_dead_code: Vec::new(),
            predicted_redundant_code: Vec::new(),
            predicted_perf_regressions: Vec::new(),
            predicted_security_concerns: Vec::new(),
            predicted_accuracy_degradations: Vec::new(),
            predicted_cost_regressions: Vec::new(),
            predicted_reasoning_class: ReasoningClass::Mute,
            agent_claim_graph: AgentClaimGraph::default(),
            claim_reconciliation: Vec::new(),
            reality_impact: None,
            provenance: PredictionProvenance {
                predictor_version: "scheduler-test".to_string(),
                constellation_version: "scheduler-test-constellation".to_string(),
                calibration_version: "scheduler-test-calibration".to_string(),
                active_pointer: hex::encode([0x46; 16]),
            },
            source_panel_sha: [0x5b; 32],
            calibration_version: "scheduler-test-calibration".to_string(),
            created_at_unix_ms: 1_778_000_000_000,
            matched_fingerprint: None,
            unknown_candidate_id: None,
        })
        .unwrap()
    }

    #[test]
    fn config_rejects_zero_periods_and_empty_paths() {
        let config = SelfOptimConfig {
            observe_period: Duration::ZERO,
            ..SelfOptimConfig::default()
        };
        assert_eq!(
            config.validate().unwrap_err().code(),
            "MEJEPA_HEAL_INVALID_STATE"
        );

        let config = SelfOptimConfig {
            status_path: PathBuf::new(),
            ..SelfOptimConfig::default()
        };
        assert_eq!(
            config.validate().unwrap_err().code(),
            "MEJEPA_HEAL_INVALID_STATE"
        );
    }

    #[test]
    fn state_open_writes_readback_verified_status_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db = open_test_db(&temp.path().join("db"));
        let config = test_config(temp.path());
        let state = SchedulerState::open(db, config).expect("open scheduler state");
        let snapshot = read_status_snapshot(state.status_path()).expect("read scheduler status");
        assert_eq!(snapshot.status, "running");
        assert!(snapshot.ticker_counts.is_empty());
        assert!(snapshot
            .source_of_truth
            .db_column_families
            .contains(&context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION.to_string()));
    }

    #[test]
    fn observe_tick_persists_physical_count_state() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db = open_test_db(&temp.path().join("db"));
        let config = test_config(temp.path());
        let mut state = SchedulerState::open(db, config).expect("open scheduler state");
        state.run_tick(TickName::Observe).expect("run observe tick");
        let snapshot = read_status_snapshot(state.status_path()).expect("read status");
        assert_eq!(snapshot.ticker_counts.get("observe"), Some(&1));
        assert_eq!(
            snapshot
                .last_success
                .get("observe")
                .expect("last observe")
                .action,
            "observed_sources_inspected"
        );
    }

    #[test]
    fn observe_tick_binds_test_outcome_to_oracle_cf() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db = open_test_db(&temp.path().join("db"));
        let config = test_config(temp.path());
        let session_id = [0x09; 16];
        let session_hex = hex::encode(session_id);
        let event_id = "11111111111111111111111111111111";
        let prediction = test_prediction(session_id);
        RocksDbInferStore::new(db.clone())
            .write_live_prediction(&prediction)
            .expect("write live prediction");

        fs::create_dir_all(&config.test_outcome_root).expect("create test outcome root");
        let outcome_path = config
            .test_outcome_root
            .join(format!("{session_hex}.jsonl"));
        let event = json!({
            "event_id": event_id,
            "ts": 1_778_000_000_001i64,
            "session_id": session_hex,
            "tool_use_id": "scheduler-bind-test",
            "command": "pytest tests/test_math.py",
            "framework": "pytest",
            "test_id": "tests/test_math.py::test_sub",
            "outcome": "fail",
            "duration_ms": 12,
            "error_log": "AssertionError: expected 4",
            "source": "synthetic-fsv",
            "sequence": 0,
            "line_no": 1
        });
        fs::write(
            &outcome_path,
            format!("{}\n", serde_json::to_string(&event).unwrap()),
        )
        .expect("write outcome jsonl");

        let mut state = SchedulerState::open(db.clone(), config).expect("open scheduler state");
        state.run_tick(TickName::Observe).expect("run observe tick");
        let snapshot = read_status_snapshot(state.status_path()).expect("read status");
        let observe = snapshot
            .last_success
            .get("observe")
            .expect("observe record");
        assert_eq!(
            observe.details["test_outcome_bindings"]["events_bound"],
            json!(1)
        );
        assert_eq!(
            observe.details["test_outcome_bindings"]["events_missing_prediction"],
            json!(0)
        );
        assert_eq!(
            observe.details["test_outcome_bindings"]["bound_keys"][0],
            format!("test-outcome-bind::{session_hex}::{event_id}")
        );
        let prediction_id = hex::encode([0x46; 16]);
        assert_eq!(
            observe.details["test_outcome_bindings"]["verification_keys"][0],
            format!("{session_hex}:{event_id}:{prediction_id}")
        );
        assert_eq!(observe.details["sampler_reward_rows"], json!(1));
        assert_eq!(
            observe.details["test_outcome_bindings"]["sampler_reward_keys"][0],
            prediction_id
        );
        assert_eq!(
            observe.details["test_outcome_bindings"]["sampler_reward_statuses"][0],
            "READY"
        );
        assert_eq!(
            observe.details["test_outcome_bindings"]["sampler_reward_write_dispositions"][0],
            "Inserted"
        );

        let storage = HealRocksStore::from_db(db.clone()).expect("heal store");
        assert_eq!(
            storage
                .count_cf(context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS)
                .expect("oracle count"),
            1
        );
        assert_eq!(
            storage
                .count_cf(context_graph_mejepa_cf::CF_MEJEPA_SAMPLER_REWARDS)
                .expect("sampler rewards count"),
            1
        );
        let cf = db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS)
            .expect("oracle cf");
        let key = format!("test-outcome-bind::{session_hex}::{event_id}");
        let readback = db
            .get_cf(cf, key.as_bytes())
            .expect("oracle readback")
            .expect("oracle row exists");
        let decoded: CalibrationExample =
            bincode::deserialize(&readback).expect("decode oracle binding");
        assert_eq!(decoded.language, Language::Python);
        assert_eq!(decoded.predicted_test_pass, vec![0.9, 0.8]);
        assert_eq!(decoded.actual_test_pass, vec![0.0, 0.0]);

        let verification_cf = db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS)
            .expect("verification cf");
        let verification_key = format!("{session_hex}:{event_id}:{prediction_id}");
        let verification_bytes = db
            .get_cf(verification_cf, verification_key.as_bytes())
            .expect("verification readback")
            .expect("verification row exists");
        let verification: SchedulerPredictionVerificationRecord =
            serde_json::from_slice(&verification_bytes).expect("decode verification row");
        assert_eq!(verification.session_id, session_hex);
        assert_eq!(verification.event_id, event_id);
        assert_eq!(verification.prediction_id, prediction_id);
        assert_eq!(verification.observed_outcome, "fail");
        assert_eq!(verification.predicted_outcome, "pass");
        assert_eq!(verification.agreement, "refuted");

        let reward = crate::sampler_reward::read_sampler_reward_signal(
            db.as_ref(),
            PredictionId([0x46; 16]),
        )
        .expect("sampler reward read")
        .expect("sampler reward row");
        assert_eq!(reward.status_code(), "READY");
        assert_eq!(reward.cell_id, "scheduler-bind-test-task::python");
        assert!((reward.surprise_z - 3.8).abs() < 1e-6);
        assert!((reward.sampling_weight_multiplier - 4.8).abs() < 1e-6);
    }

    #[test]
    fn observe_tick_signal_drops_test_outcome_without_prediction() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db = open_test_db(&temp.path().join("db"));
        let config = test_config(temp.path());
        let session_id = [0x0a; 16];
        let session_hex = hex::encode(session_id);
        let event_id = "22222222222222222222222222222222";

        fs::create_dir_all(&config.test_outcome_root).expect("create test outcome root");
        let outcome_path = config
            .test_outcome_root
            .join(format!("{session_hex}.jsonl"));
        let event = json!({
            "event_id": event_id,
            "ts": 1_778_000_000_002i64,
            "session_id": session_hex,
            "tool_use_id": "scheduler-missing-prediction",
            "command": "pytest tests/test_math.py",
            "framework": "pytest",
            "test_id": "tests/test_math.py::test_add",
            "outcome": "pass",
            "duration_ms": 10,
            "error_log": "",
            "source": "synthetic-fsv",
            "sequence": 0,
            "line_no": 1
        });
        fs::write(
            &outcome_path,
            format!("{}\n", serde_json::to_string(&event).unwrap()),
        )
        .expect("write outcome jsonl");

        let mut state = SchedulerState::open(db.clone(), config).expect("open scheduler state");
        state.run_tick(TickName::Observe).expect("run observe tick");
        let snapshot = read_status_snapshot(state.status_path()).expect("read status");
        assert!(!snapshot.last_error.contains_key("observe"));
        let observe = snapshot
            .last_success
            .get("observe")
            .expect("observe record");
        assert_eq!(
            observe.details["test_outcome_bindings"]["events_bound"],
            json!(0)
        );
        assert_eq!(
            observe.details["test_outcome_bindings"]["events_missing_prediction"],
            json!(1)
        );
        assert_eq!(
            observe.details["test_outcome_bindings"]["signal_drop_event_ids"]
                .as_array()
                .expect("signal drop ids")
                .len(),
            1
        );

        let storage = HealRocksStore::from_db(db.clone()).expect("heal store");
        assert_eq!(
            storage
                .count_cf(context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS)
                .expect("oracle count"),
            0
        );
        assert_eq!(
            storage
                .count_cf(context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS)
                .expect("verification count"),
            0
        );
        assert_eq!(
            storage
                .count_cf(context_graph_mejepa_cf::CF_MEJEPA_SAMPLER_REWARDS)
                .expect("sampler rewards count"),
            0
        );
        let signal_drops =
            crate::reward_signal_audit::load_signal_drop_log_entries(db.as_ref(), 10)
                .expect("signal drop readback");
        assert_eq!(signal_drops.len(), 1);
        let row = &signal_drops[0];
        assert_eq!(row.signal_name, "post_tool_test_outcomes");
        assert_eq!(
            row.source_stage,
            "self_optimization.observe.bind_test_outcomes"
        );
        assert_eq!(row.error_code, "MEJEPA_TEST_OUTCOME_NO_PREDICTION");
        assert_eq!(row.context.get("session_id"), Some(&session_hex));
        assert_eq!(row.context.get("event_id"), Some(&event_id.to_string()));
    }

    #[test]
    fn operator_override_recalibration_queues_and_watermarks_distinct_windows() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("db");
        {
            let db = open_test_db(&db_path);
            let storage = HealRocksStore::from_db(db.clone()).expect("heal store");

            let exact_threshold =
                track_operator_override_recalibration(storage.as_ref(), 100).expect("track 100");
            assert_eq!(exact_threshold.overrides_consumed_since_calibration, 100);
            assert!(!exact_threshold.recalibration_queued);
            assert_eq!(
                storage
                    .count_cf(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS)
                    .expect("heal report count"),
                0
            );

            let first =
                track_operator_override_recalibration(storage.as_ref(), 101).expect("track 101");
            assert!(first.recalibration_queued);
            assert!(!first.already_queued);
            assert_eq!(first.last_recalibration_total_applied_count, Some(101));
            assert_eq!(first.last_recalibration_override_count, Some(101));
            assert_eq!(
                storage
                    .count_cf(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS)
                    .expect("heal report count"),
                1
            );

            let duplicate =
                track_operator_override_recalibration(storage.as_ref(), 101).expect("track dup");
            assert!(duplicate.already_queued);
            assert_eq!(
                storage
                    .count_cf(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS)
                    .expect("heal report count"),
                1
            );

            let second =
                track_operator_override_recalibration(storage.as_ref(), 202).expect("track 202");
            assert!(second.recalibration_queued);
            assert!(!second.already_queued);
            assert_eq!(second.last_recalibration_total_applied_count, Some(202));
            assert_eq!(second.last_recalibration_override_count, Some(101));
            assert_eq!(
                storage
                    .count_cf(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS)
                    .expect("heal report count"),
                2
            );
        }

        let reopened = open_test_db(&db_path);
        let storage = HealRocksStore::from_db(reopened).expect("reopened heal store");
        let counter_key =
            operator_override_calibration_counter_key().expect("operator counter key");
        let counter_bytes = storage
            .get_cf(
                context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS,
                &counter_key,
            )
            .expect("counter readback")
            .expect("counter row exists");
        let counter: OperatorOverrideCalibrationCounter =
            decode_value(&counter_bytes).expect("decode counter");
        assert_eq!(counter.total_applied_count, 202);
        assert_eq!(counter.last_recalibration_total_applied_count, Some(202));
        assert_eq!(counter.last_recalibration_override_count, Some(101));
    }

    #[test]
    fn missing_cf_fails_closed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        let db = Arc::new(
            DB::open_cf_descriptors(
                &opts,
                temp.path().join("db"),
                vec![ColumnFamilyDescriptor::new(
                    context_graph_mejepa_cf::CF_MEJEPA_DRIFT_WINDOW,
                    Options::default(),
                )],
            )
            .expect("open corrupt test db"),
        );
        let err = match SchedulerState::open(db, test_config(temp.path())) {
            Ok(_) => panic!("missing CF scheduler state should fail closed"),
            Err(err) => err,
        };
        assert_eq!(err.code(), "MEJEPA_HEAL_INVALID_STATE");
    }
}
