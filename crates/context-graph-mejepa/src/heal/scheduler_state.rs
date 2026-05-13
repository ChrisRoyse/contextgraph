use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::compiler::MejepaStore;
use crate::conformal::CalibrationExample;
use crate::eval::{ActiveLearningKind, ProductionTelemetryWindow, RocksDbEvalStore};
use crate::heal::calibration::continuous_calibration_if_due;
use crate::heal::cf::{decode_value, CF_MEJEPA_DRIFT_WINDOW};
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
use crate::heal::pipeline::{LinearHealPredictor, MeJepaHealConfig, SelfHealingPipeline};
use crate::heal::policy::{
    load_policy_record, persist_policy_record, policy_key, timestamped_policy_key,
};
use crate::heal::promote::TriggerReason;
use crate::heal::promote_approval::{
    approved_promotions, mark_promotion_executed, queue_pending_retrain_request,
    PendingPromotionKind,
};
use crate::heal::regulate::{assess_substrate, regulate_substrate};
use crate::heal::store::HealRocksStore;
use crate::store::RocksDbInferStore;
use crate::system_cost::{HealTickerTelemetrySnapshot, SystemCostCounters};

pub const DEFAULT_OBSERVE_PERIOD: Duration = Duration::from_millis(250);
pub const DEFAULT_DRIFT_PERIOD: Duration = Duration::from_secs(60);
pub const DEFAULT_ACTIVE_LEARNING_PERIOD: Duration = Duration::from_secs(600);
pub const DEFAULT_PROMOTE_PERIOD: Duration = Duration::from_secs(3600);
pub const DEFAULT_CONTINUAL_BACKPROP_PERIOD: Duration = Duration::from_secs(1800);
pub const DEFAULT_CONSTELLATION_PERIOD: Duration = Duration::from_secs(3600);
pub const DEFAULT_EMERGENCY_EVICTION_PERIOD: Duration = Duration::from_secs(600);
pub const DEFAULT_EMERGENCY_EVICTION_THRESHOLD_BYTES: u64 = 110 * 1024 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct SelfOptimConfig {
    pub observe_period: Duration,
    pub drift_period: Duration,
    pub active_learning_period: Duration,
    pub promote_period: Duration,
    pub continual_backprop_period: Duration,
    pub constellation_period: Duration,
    pub emergency_eviction_period: Duration,
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
}

impl TickName {
    pub fn all() -> [Self; 7] {
        [
            Self::Observe,
            Self::DriftCheck,
            Self::ActiveLearning,
            Self::Promote,
            Self::ContinualBackprop,
            Self::ConstellationFreshness,
            Self::EmergencyEviction,
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
        }
    }
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
    events_skipped_non_hard_label: usize,
    bound_keys: Vec<String>,
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
        Ok(TickRunRecord::success(
            TickName::Observe,
            "observed_sources_inspected",
            json!({
                "oracle_verdict_rows": oracle_verdicts,
                "live_prediction_rows": live_predictions,
                "shift_watermark_rows": shift_watermarks,
                "test_outcome_bindings": test_outcome_bindings,
                "witness_integrity": witness_integrity,
                "source_of_truth_cfs": [
                    context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS,
                    context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
                    context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK,
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
        let queue = self.eval_store.load_queue().map_err(map_eval_error)?;
        let Some(queue) = queue else {
            return Ok(TickRunRecord::success(
                TickName::ActiveLearning,
                "active_learning_queue_absent",
                json!({
                    "source_of_truth_cf": context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
                    "queued_count": 0,
                    "top_tasks": [],
                }),
            ));
        };
        let mut top_tasks = queue.entries.values().collect::<Vec<_>>();
        // REQ-FLYWHEEL-11: AgentSurprise → OutOfDistribution → Uncertainty,
        // with score as the within-kind tiebreaker. This makes the scheduler
        // drain agent-flagged surprises before any uncertainty backlog.
        top_tasks.sort_by(|left, right| {
            left.kind
                .fast_track_rank()
                .cmp(&right.kind.fast_track_rank())
                .then_with(|| right.score.total_cmp(&left.score))
        });
        let selected = top_tasks.into_iter().take(10).collect::<Vec<_>>();
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
                    "surprise_weight": surprise_weight,
                    "reason": entry.reason,
                    "ood_score": entry.ood_score,
                    "outcome_set_len": entry.outcome_set_len,
                    "kind": entry.kind,
                })
            })
            .collect::<Vec<_>>();
        Ok(TickRunRecord::success(
            TickName::ActiveLearning,
            "active_learning_queue_ranked",
            json!({
                "source_of_truth_cf": context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
                "queued_count": queue.entries.len(),
                "evicted_count": queue.evicted.len(),
                "ood_escalation_count": queue.ood_escalations.len(),
                "fast_path_records": fast_path_records,
                "top_tasks": top_tasks,
            }),
        ))
    }

    fn tick_promote(&mut self) -> Result<TickRunRecord, HealError> {
        let mut approved = approved_promotions(self.storage.as_ref())?;
        approved.sort_by_key(|record| record.updated_at_unix_ms);
        let mut witness_repair = None;
        let mut catastrophic_promotion = None;
        if let Some(record) = approved.pop() {
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
                    context_graph_mejepa_cf::CF_MEJEPA_WEIGHT_BLOBS
                ],
                "train_cert_source_step_count": window.source_step_count,
                "substrate_breakdown": breakdown,
                "regulation_report": report,
                "dissipation_report": dissipation,
                "lambda_ramp_report": lambda_ramp,
                "per_head_fisher_report": per_head_fisher,
                "per_head_dormancy_report": per_head_dormancy,
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
        let mut refresh_reasons = Vec::new();
        match constellation.check_freshness(max_age_days, allow_stale) {
            Ok(()) => {}
            Err(context_graph_mejepa_tct::TctError::StaleConstellation {
                frozen_at_iso,
                age_days,
                max_age_days,
            }) => {
                refresh_reasons.push(json!({
                    "kind": "stale_constellation",
                    "frozen_at_iso": frozen_at_iso,
                    "age_days": age_days,
                    "max_age_days": max_age_days,
                }));
            }
            Err(err) => return Err(map_tct_error(err)),
        }
        let train_cert_count = self
            .storage
            .count_cf(context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)?;
        let refresh_report_count = store.count_refresh_reports().map_err(map_tct_error)?;
        if train_cert_count >= 100 && refresh_report_count == 0 {
            refresh_reasons.push(json!({
                "kind": "new_training_examples_without_refresh_report",
                "train_cert_count": train_cert_count,
                "threshold": 100,
            }));
        }
        let latest_telemetry = latest_production_telemetry(self.storage.as_ref())?;
        if let Some(telemetry) = &latest_telemetry {
            if telemetry.gtau_pass_rate < 0.97 {
                refresh_reasons.push(json!({
                    "kind": "gtau_pass_rate_drift",
                    "gtau_pass_rate": telemetry.gtau_pass_rate,
                    "threshold": 0.97,
                    "window_id": telemetry.window_id,
                }));
            }
        }
        let refresh_policy = if refresh_reasons.is_empty() {
            json!({"action": "none", "reasons": []})
        } else {
            let record = json!({
                "action": "queue_constellation_refresh",
                "constellation_version_hex": hex::encode(latest_version),
                "reasons": refresh_reasons,
                "source_of_truth_cfs": [
                    context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION,
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
                "train_cert_count": train_cert_count,
                "latest_production_telemetry": latest_telemetry,
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
        events_skipped_non_hard_label: 0,
        bound_keys: Vec::new(),
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
            let prediction = predictions.first().ok_or_else(|| {
                HealError::invalid(
                    "test_outcome_event.prediction_lookup",
                    format!(
                        "no RealityPrediction found for session_id={} while binding event_id={}",
                        event.session_id, event.event_id
                    ),
                )
            })?;
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
            report.events_bound += 1;
            report.bound_keys.push(key);
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
        AgentClaimGraph, ConformalInterval, ConformalSet, Language, OracleOutcome,
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
            guard_violations: Vec::new(),
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
            observe.details["test_outcome_bindings"]["bound_keys"][0],
            format!("test-outcome-bind::{session_hex}::{event_id}")
        );

        let storage = HealRocksStore::from_db(db.clone()).expect("heal store");
        assert_eq!(
            storage
                .count_cf(context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS)
                .expect("oracle count"),
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
