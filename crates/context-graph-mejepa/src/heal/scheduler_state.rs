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
use crate::eval::RocksDbEvalStore;
use crate::heal::calibration::continuous_calibration_if_due;
use crate::heal::cf::{decode_value, CF_MEJEPA_DRIFT_WINDOW};
use crate::heal::drift::{DriftDetector, DriftSample, NoopDriftSurface, SeverityTable};
use crate::heal::errors::HealError;
use crate::heal::full_retrain::full_retrain_if_due;
use crate::heal::integrity::{ChainIntegrityChecker, WitnessChainAppender};
use crate::heal::pipeline::{LinearHealPredictor, MeJepaHealConfig, SelfHealingPipeline};
use crate::heal::regulate::{assess_substrate, regulate_substrate};
use crate::heal::store::HealRocksStore;
use crate::store::RocksDbInferStore;

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
    started_at_unix_ms: i64,
    ticker_counts: BTreeMap<String, u64>,
    last_ticker: Option<String>,
    last_success: BTreeMap<String, TickRunRecord>,
    last_error: BTreeMap<String, TickRunRecord>,
}

impl SchedulerState {
    pub fn open(db: Arc<DB>, config: SelfOptimConfig) -> Result<Self, HealError> {
        config.validate()?;
        let storage = HealRocksStore::from_db(db.clone())?;
        for cf in context_graph_mejepa_cf::all_hygiene_referenced_cfs() {
            if db.cf_handle(cf).is_none() {
                return Err(HealError::invalid(
                    "self_optim_scheduler.column_family",
                    format!("missing column family {cf}"),
                ));
            }
        }
        let eval_store = RocksDbEvalStore::new(db.clone()).map_err(map_eval_error)?;
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

    fn tick_observe(&self) -> Result<TickRunRecord, HealError> {
        let test_outcome_bindings =
            bind_test_outcomes(&self.config.test_outcome_root, self.storage.db())?;
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
                "source_of_truth_cfs": [
                    context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS,
                    context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS,
                    context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK
                ],
            }),
        ))
    }

    fn tick_drift_check(&self) -> Result<TickRunRecord, HealError> {
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
        Ok(TickRunRecord::success(
            TickName::DriftCheck,
            "drift_window_evaluated",
            json!({
                "source_of_truth_cf": CF_MEJEPA_DRIFT_WINDOW,
                "sample_count": sample_count,
                "severity": severity,
                "empirical_coverage": detector.last_empirical_coverage,
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
        let top_tasks = top_tasks
            .into_iter()
            .take(10)
            .map(|entry| {
                json!({
                    "task_id": entry.task_id.0,
                    "score": entry.score,
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
                "top_tasks": top_tasks,
            }),
        ))
    }

    fn tick_promote(&mut self) -> Result<TickRunRecord, HealError> {
        let calibration = continuous_calibration_if_due(&mut self.pipeline)?;
        let retrain = full_retrain_if_due(&mut self.pipeline)?;
        Ok(TickRunRecord::success(
            TickName::Promote,
            "promotion_gates_checked",
            json!({
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
        constellation
            .check_freshness(max_age_days, allow_stale)
            .map_err(map_tct_error)?;
        Ok(TickRunRecord::success(
            TickName::ConstellationFreshness,
            "constellation_freshness_verified",
            json!({
                "source_of_truth_cf": context_graph_mejepa_cf::CF_MEJEPA_CONSTELLATION,
                "constellation_count": count,
                "latest_version_hex": hex::encode(latest_version),
                "max_age_days": max_age_days,
                "allow_stale": allow_stale,
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
        if before.total_used_bytes <= self.config.emergency_eviction_threshold_bytes {
            return Ok(TickRunRecord::success(
                TickName::EmergencyEviction,
                "quota_below_emergency_threshold",
                json!({
                    "source_of_truth_cf": context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE,
                    "total_used_bytes": before.total_used_bytes,
                    "threshold_bytes": self.config.emergency_eviction_threshold_bytes,
                    "total_quota_bytes": before.total_quota_bytes,
                }),
            ));
        }
        let report =
            context_graph_mejepa_hygiene::quota_check_and_evict(&env).map_err(map_hygiene_error)?;
        Ok(TickRunRecord::success(
            TickName::EmergencyEviction,
            "emergency_quota_eviction_run",
            json!({
                "source_of_truth_cf": context_graph_mejepa_cf::CF_MEJEPA_QUOTA_STATE,
                "before": report.before,
                "after": report.after,
                "evicted": report.evicted,
            }),
        ))
    }

    fn record_tick(&mut self, tick: TickName, record: TickRunRecord) -> Result<(), HealError> {
        let ticker = tick.as_str().to_string();
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
    use crate::types::{ConformalSet, Language, OracleOutcome, RealityPrediction, TaskId};
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
            task_id: TaskId("scheduler-bind-test-task".to_string()),
            session_id,
            language: Language::Python,
            predicted_oracle_pass: 0.95,
            predicted_test_pass: vec![0.9, 0.8],
            predicted_runtime_trace: [0.0; 32],
            ood_score: 0.1,
            outcome_set: ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2).unwrap(),
            calibrated_confidence: 0.7,
            degraded_status: false,
            granger_attestations: BTreeMap::from([("scheduler:test".to_string(), 0.9)]),
            source_panel_sha: [0x5b; 32],
            calibration_version: "scheduler-test-calibration".to_string(),
            created_at_unix_ms: 1_778_000_000_000,
        })
        .unwrap()
    }

    #[test]
    fn config_rejects_zero_periods_and_empty_paths() {
        let mut config = SelfOptimConfig::default();
        config.observe_period = Duration::ZERO;
        assert_eq!(
            config.validate().unwrap_err().code(),
            "MEJEPA_HEAL_INVALID_STATE"
        );

        let mut config = SelfOptimConfig::default();
        config.status_path = PathBuf::new();
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
