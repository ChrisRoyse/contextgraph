use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures_util::FutureExt;
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

use crate::daemon_validate::DaemonPaths;
use context_graph_mejepa::toolchain_detect::{
    audit_required_toolchains, default_enabled_label_toolchains, ToolchainAuditReport,
    ToolchainBinary, ToolchainMissingDiagnostic,
};
use context_graph_mejepa::Language;
const RESTART_WINDOW: Duration = Duration::from_secs(5 * 60);
const FLAPPING_THRESHOLD: usize = 5;
const BACKOFF_CAP: Duration = Duration::from_secs(60);
const SUPERVISOR_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);
const DAEMON_CONFIG_ROOT_ENV: &str = "MEJEPA_CONFIG_ROOT";
const DAEMON_CONFIG_FILE: &str = "daemon.toml";

type TaskRunner = Arc<
    dyn Fn(
            SupervisedTaskKind,
            FlywheelDaemonConfig,
            Arc<DB>,
            watch::Receiver<bool>,
        ) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>
        + Send
        + Sync,
>;

#[derive(Debug, Clone, Serialize)]
struct WeeklyOperationalSummary {
    agent_feedback_rows: usize,
    drift_window_rows: usize,
    drift_history_rows: usize,
    active_learning_queue_rows: usize,
    ood_escalation_rows: usize,
}

#[derive(Debug, Clone)]
pub struct FlywheelDaemonConfig {
    pub paths: DaemonPaths,
    pub repo_root: PathBuf,
    nightly_gc_interval_override: Option<Duration>,
    weekly_eval_interval_override: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StartupToolchainGateReport {
    pub schema_version: u32,
    pub config_path: Option<String>,
    pub config_source: String,
    pub enabled_languages: Vec<Language>,
    pub required_count: usize,
    pub path_entries_checked: usize,
    pub resolved_count: usize,
    pub missing_count: usize,
    pub all_available: bool,
    pub no_rocksdb_opened: bool,
    pub missing: Vec<ToolchainMissingDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StartupToolchainGateError {
    pub error_code: &'static str,
    pub detail: String,
    pub report: StartupToolchainGateReport,
}

impl StartupToolchainGateError {
    pub fn structured_json(&self) -> serde_json::Value {
        serde_json::json!({
            "error_code": self.error_code,
            "detail": self.detail,
            "toolchain_audit": self.report,
        })
    }
}

impl std::fmt::Display for StartupToolchainGateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.error_code, self.detail)
    }
}

impl std::error::Error for StartupToolchainGateError {}

#[derive(Debug, Deserialize)]
struct OperatorDaemonToml {
    enabled_languages: Option<Vec<Language>>,
    daemon: Option<OperatorDaemonSection>,
}

#[derive(Debug, Deserialize)]
struct OperatorDaemonSection {
    enabled_languages: Option<Vec<Language>>,
}

impl FlywheelDaemonConfig {
    pub fn from_paths(paths: DaemonPaths, repo_root: PathBuf) -> Self {
        Self {
            paths,
            repo_root,
            nightly_gc_interval_override: None,
            weekly_eval_interval_override: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_test_scheduler_periods(
        mut self,
        nightly_gc_period: Duration,
        weekly_eval_period: Duration,
    ) -> Result<Self> {
        validate_scheduler_period("nightly_gc_period", nightly_gc_period)?;
        validate_scheduler_period("weekly_eval_period", weekly_eval_period)?;
        self.nightly_gc_interval_override = Some(nightly_gc_period);
        self.weekly_eval_interval_override = Some(weekly_eval_period);
        Ok(self)
    }
}

fn validate_scheduler_period(field: &str, period: Duration) -> Result<()> {
    if period.is_zero() {
        return Err(anyhow::anyhow!(
            "MEJEPA_DAEMON_SCHEDULER_PERIOD_INVALID: {field} must be greater than zero"
        ));
    }
    Ok(())
}

pub fn default_daemon_config_path() -> Option<PathBuf> {
    if let Ok(root) = std::env::var(DAEMON_CONFIG_ROOT_ENV) {
        return Some(PathBuf::from(root).join(DAEMON_CONFIG_FILE));
    }
    dirs::home_dir().map(|home| home.join(".config/mejepa").join(DAEMON_CONFIG_FILE))
}

pub fn default_daemon_enabled_label_languages() -> Vec<Language> {
    vec![Language::Python]
}

pub fn required_toolchains_for_languages(enabled_languages: &[Language]) -> Vec<ToolchainBinary> {
    let enabled = enabled_languages.iter().copied().collect::<BTreeSet<_>>();
    default_enabled_label_toolchains()
        .into_iter()
        .filter(|toolchain| enabled.contains(&toolchain.language))
        .collect()
}

pub fn load_enabled_label_languages_from_daemon_config(
    config_path: Option<&Path>,
) -> std::result::Result<(Option<PathBuf>, String, Vec<Language>), StartupToolchainGateError> {
    let Some(path) = config_path
        .map(Path::to_path_buf)
        .or_else(default_daemon_config_path)
    else {
        return Ok((
            None,
            "default_python_no_config_path".to_string(),
            default_daemon_enabled_label_languages(),
        ));
    };
    if !path.exists() {
        return Ok((
            Some(path),
            "default_python_config_missing".to_string(),
            default_daemon_enabled_label_languages(),
        ));
    }
    let contents = fs::read_to_string(&path).map_err(|err| {
        startup_toolchain_config_error(
            Some(path.clone()),
            "MEJEPA_LABEL_TOOLCHAIN_CONFIG_READ_FAILED",
            format!("failed to read daemon toolchain config: {err}"),
        )
    })?;
    let parsed: OperatorDaemonToml = toml::from_str(&contents).map_err(|err| {
        startup_toolchain_config_error(
            Some(path.clone()),
            "MEJEPA_LABEL_TOOLCHAIN_CONFIG_INVALID",
            format!("failed to parse daemon toolchain config: {err}"),
        )
    })?;
    let configured = parsed
        .daemon
        .and_then(|section| section.enabled_languages)
        .or(parsed.enabled_languages);
    let Some(languages) = configured else {
        return Ok((
            Some(path),
            "default_python_config_no_enabled_languages".to_string(),
            default_daemon_enabled_label_languages(),
        ));
    };
    normalize_enabled_languages(Some(path), languages)
}

pub fn enforce_startup_toolchain_gate(
    config_path: Option<&Path>,
    path_env: Option<&str>,
) -> std::result::Result<StartupToolchainGateReport, StartupToolchainGateError> {
    let (resolved_config_path, config_source, enabled_languages) =
        load_enabled_label_languages_from_daemon_config(config_path)?;
    let required = required_toolchains_for_languages(&enabled_languages);
    let audit = audit_required_toolchains(&required, path_env).map_err(|err| {
        startup_toolchain_config_error(
            resolved_config_path.clone(),
            "MEJEPA_LABEL_TOOLCHAIN_CONFIG_INVALID",
            err.to_string(),
        )
    })?;
    let report = startup_toolchain_report(
        resolved_config_path.as_deref(),
        config_source,
        enabled_languages,
        &audit,
    );
    if report.all_available {
        return Ok(report);
    }
    Err(StartupToolchainGateError {
        error_code: "MEJEPA_LABEL_TOOLCHAIN_MISSING",
        detail: "daemon startup refused before opening RocksDB because one or more configured label toolchains are missing".to_string(),
        report,
    })
}

fn normalize_enabled_languages(
    path: Option<PathBuf>,
    languages: Vec<Language>,
) -> std::result::Result<(Option<PathBuf>, String, Vec<Language>), StartupToolchainGateError> {
    let deduped = languages.into_iter().collect::<BTreeSet<_>>();
    if deduped.is_empty() {
        return Err(startup_toolchain_config_error(
            path,
            "MEJEPA_LABEL_TOOLCHAIN_CONFIG_INVALID",
            "enabled_languages must contain at least one language".to_string(),
        ));
    }
    Ok((
        path,
        "daemon_toml_enabled_languages".to_string(),
        deduped.into_iter().collect(),
    ))
}

fn startup_toolchain_report(
    config_path: Option<&Path>,
    config_source: String,
    enabled_languages: Vec<Language>,
    audit: &ToolchainAuditReport,
) -> StartupToolchainGateReport {
    StartupToolchainGateReport {
        schema_version: 1,
        config_path: config_path.map(|path| path.display().to_string()),
        config_source,
        enabled_languages,
        required_count: audit.required_count,
        path_entries_checked: audit.path_entries_checked,
        resolved_count: audit.resolved.len(),
        missing_count: audit.missing.len(),
        all_available: audit.all_available,
        no_rocksdb_opened: true,
        missing: audit.missing.clone(),
    }
}

fn startup_toolchain_config_error(
    path: Option<PathBuf>,
    error_code: &'static str,
    detail: String,
) -> StartupToolchainGateError {
    StartupToolchainGateError {
        error_code,
        detail,
        report: StartupToolchainGateReport {
            schema_version: 1,
            config_path: path.map(|path| path.display().to_string()),
            config_source: "daemon_toml_invalid".to_string(),
            enabled_languages: Vec::new(),
            required_count: 0,
            path_entries_checked: 0,
            resolved_count: 0,
            missing_count: 0,
            all_available: false,
            no_rocksdb_opened: true,
            missing: Vec::new(),
        },
    }
}

pub struct FlywheelDaemonHandle {
    pub shutdown_tx: watch::Sender<bool>,
    pub join_handle: tokio::task::JoinHandle<Result<()>>,
}

impl FlywheelDaemonHandle {
    pub async fn shutdown(&mut self) -> Result<()> {
        let _ = self.shutdown_tx.send(true);
        match (&mut self.join_handle).await {
            Ok(result) => result,
            Err(err) => Err(anyhow::anyhow!(
                "MEJEPA_DAEMON_SUPERVISOR_JOIN_FAILED: {err}"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisedTaskKind {
    ShiftSubscriber,
    SelfOptimizationScheduler,
    NightlyGcScheduler,
    WeeklyEvalScheduler,
}

impl SupervisedTaskKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::ShiftSubscriber => "shift_subscriber",
            Self::SelfOptimizationScheduler => "self_optimization_scheduler",
            Self::NightlyGcScheduler => "nightly_gc_scheduler",
            Self::WeeklyEvalScheduler => "weekly_eval_scheduler",
        }
    }
}

pub fn supervised_task_names() -> Vec<&'static str> {
    all_task_kinds().iter().map(|kind| kind.name()).collect()
}

fn all_task_kinds() -> [SupervisedTaskKind; 4] {
    [
        SupervisedTaskKind::ShiftSubscriber,
        SupervisedTaskKind::SelfOptimizationScheduler,
        SupervisedTaskKind::NightlyGcScheduler,
        SupervisedTaskKind::WeeklyEvalScheduler,
    ]
}

fn flywheel_job_run_source_of_truth() -> serde_json::Value {
    serde_json::json!({
        "column_family": context_graph_mejepa_cf::CF_MEJEPA_FLYWHEEL_JOB_RUNS,
        "status_file": "state/schedulers/flywheel_supervisor_status.json"
    })
}

fn flywheel_run_id(kind: SupervisedTaskKind, event: &str) -> String {
    let nanos = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| chrono::Utc::now().timestamp_micros() * 1000);
    format!("{}:{event}:{}:{nanos}", kind.name(), std::process::id())
}

fn error_code_from_message(message: &str) -> String {
    message
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .find(|token| token.starts_with("MEJEPA_") && token.len() > "MEJEPA_".len())
        .unwrap_or("MEJEPA_FLYWHEEL_JOB_FAILED")
        .to_string()
}

pub fn record_flywheel_job_run(db: &DB, record: &FlywheelJobRunRecord) -> Result<String> {
    record.validate()?;
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_FLYWHEEL_JOB_RUNS)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "MEJEPA_CF_MISSING: {}; storage opener must register flywheel job-run source of truth",
                context_graph_mejepa_cf::CF_MEJEPA_FLYWHEEL_JOB_RUNS
            )
        })?;
    let key = record.run_id.as_bytes();
    let value = serde_json::to_vec(record)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, &value, &opts)?;
    let readback = db.get_cf(cf, key)?.ok_or_else(|| {
        anyhow::anyhow!(
            "MEJEPA_FLYWHEEL_JOB_RUN_READBACK_MISSING: {}",
            record.run_id
        )
    })?;
    if readback != value {
        return Err(anyhow::anyhow!(
            "MEJEPA_FLYWHEEL_JOB_RUN_READBACK_MISMATCH: {}",
            record.run_id
        ));
    }
    let decoded: FlywheelJobRunRecord = serde_json::from_slice(&readback)?;
    if decoded != *record {
        return Err(anyhow::anyhow!(
            "MEJEPA_FLYWHEEL_JOB_RUN_DECODE_MISMATCH: {}",
            record.run_id
        ));
    }
    Ok(hex::encode(key))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlywheelJobRunRecord {
    pub schema_version: u32,
    pub run_id: String,
    pub task: String,
    pub event: String,
    pub status: String,
    pub pid: u32,
    pub started_unix_ms: i64,
    pub completed_unix_ms: Option<i64>,
    pub duration_ms: Option<u64>,
    pub restart_count_5m: Option<usize>,
    pub source_of_truth: serde_json::Value,
    pub details: serde_json::Value,
    pub error_code: Option<String>,
    pub error: Option<String>,
}

impl FlywheelJobRunRecord {
    fn started(kind: SupervisedTaskKind, event: &str, run_id: String) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            schema_version: 1,
            run_id,
            task: kind.name().to_string(),
            event: event.to_string(),
            status: "started".to_string(),
            pid: std::process::id(),
            started_unix_ms: now,
            completed_unix_ms: None,
            duration_ms: None,
            restart_count_5m: None,
            source_of_truth: flywheel_job_run_source_of_truth(),
            details: serde_json::json!({}),
            error_code: None,
            error: None,
        }
    }

    fn completed(mut self, details: serde_json::Value) -> Self {
        let completed = chrono::Utc::now().timestamp_millis();
        self.status = "completed".to_string();
        self.completed_unix_ms = Some(completed);
        self.duration_ms = Some((completed - self.started_unix_ms).max(0) as u64);
        self.details = details;
        self
    }

    fn failed(mut self, error: &anyhow::Error) -> Self {
        let completed = chrono::Utc::now().timestamp_millis();
        let message = error.to_string();
        self.status = "error".to_string();
        self.completed_unix_ms = Some(completed);
        self.duration_ms = Some((completed - self.started_unix_ms).max(0) as u64);
        self.error_code = Some(error_code_from_message(&message));
        self.error = Some(message);
        self
    }

    fn task_exit(kind: SupervisedTaskKind, exit: &TaskExit, restart: RestartDecision) -> Self {
        let mut record = Self {
            schema_version: 1,
            run_id: flywheel_run_id(kind, "task_exit"),
            task: kind.name().to_string(),
            event: "task_exit".to_string(),
            status: "completed".to_string(),
            pid: std::process::id(),
            started_unix_ms: exit.started_unix_seconds * 1000,
            completed_unix_ms: Some(exit.completed_unix_seconds * 1000),
            duration_ms: Some(
                (exit.completed_unix_seconds - exit.started_unix_seconds).max(0) as u64 * 1000,
            ),
            restart_count_5m: Some(restart.count_in_window),
            source_of_truth: flywheel_job_run_source_of_truth(),
            details: serde_json::json!({
                "backoff_ms": restart.backoff.as_millis() as u64,
                "flapping": restart.flapping
            }),
            error_code: None,
            error: None,
        };
        match &exit.status {
            TaskExitStatus::Completed => {}
            TaskExitStatus::Error(detail) => {
                record.status = "error".to_string();
                record.error_code = Some(error_code_from_message(detail));
                record.error = Some(detail.clone());
            }
            TaskExitStatus::Panic(detail) => {
                record.status = "panic".to_string();
                record.error_code = Some("MEJEPA_DAEMON_TASK_PANIC".to_string());
                record.error = Some(detail.clone());
            }
        }
        record
    }

    fn validate(&self) -> Result<()> {
        if self.schema_version != 1 {
            return Err(anyhow::anyhow!(
                "MEJEPA_FLYWHEEL_JOB_RUN_SCHEMA_UNSUPPORTED: {}",
                self.schema_version
            ));
        }
        for (field, value) in [
            ("run_id", self.run_id.as_str()),
            ("task", self.task.as_str()),
            ("event", self.event.as_str()),
            ("status", self.status.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(anyhow::anyhow!(
                    "MEJEPA_FLYWHEEL_JOB_RUN_INVALID: {field} must be non-empty"
                ));
            }
        }
        if !matches!(
            self.status.as_str(),
            "started" | "completed" | "error" | "panic"
        ) {
            return Err(anyhow::anyhow!(
                "MEJEPA_FLYWHEEL_JOB_RUN_INVALID: unsupported status {}",
                self.status
            ));
        }
        if matches!(self.status.as_str(), "error" | "panic") && self.error_code.is_none() {
            return Err(anyhow::anyhow!(
                "MEJEPA_FLYWHEEL_JOB_RUN_INVALID: failed records require error_code"
            ));
        }
        if let Some(completed) = self.completed_unix_ms {
            if completed < self.started_unix_ms {
                return Err(anyhow::anyhow!(
                    "MEJEPA_FLYWHEEL_JOB_RUN_INVALID: completed_unix_ms precedes started_unix_ms"
                ));
            }
        }
        Ok(())
    }
}

pub fn start_supervised_flywheel_tasks(
    config: FlywheelDaemonConfig,
    db: Arc<DB>,
) -> Result<FlywheelDaemonHandle> {
    ensure_mejepa_column_families(&db)?;
    append_daemon_log(
        &config.paths.stdout_log,
        "validated D-drive root, starting supervised tasks",
    )?;
    write_supervisor_status(&config, "starting", &BTreeMap::new())?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let join_handle = tokio::spawn(supervise(config, db, shutdown_rx));
    Ok(FlywheelDaemonHandle {
        shutdown_tx,
        join_handle,
    })
}

async fn supervise(
    config: FlywheelDaemonConfig,
    db: Arc<DB>,
    shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    supervise_with_runner(config, db, shutdown_rx, production_task_runner()).await
}

fn production_task_runner() -> TaskRunner {
    Arc::new(|kind, config, db, shutdown_rx| Box::pin(run_task(kind, config, db, shutdown_rx)))
}

async fn supervise_with_runner(
    config: FlywheelDaemonConfig,
    db: Arc<DB>,
    mut shutdown_rx: watch::Receiver<bool>,
    runner: TaskRunner,
) -> Result<()> {
    let mut tasks = JoinSet::new();
    let mut restart_state = RestartState::default();
    for kind in all_task_kinds() {
        spawn_supervised_task(
            &mut tasks,
            kind,
            config.clone(),
            db.clone(),
            shutdown_rx.clone(),
            runner.clone(),
        );
    }
    write_supervisor_status(&config, "running", &restart_state.restart_counts())?;

    loop {
        if *shutdown_rx.borrow() {
            info!("MEJEPA_DAEMON_SUPERVISOR_SHUTDOWN: stopping supervised flywheel tasks");
            stop_supervised_tasks(&mut tasks, &config, &restart_state).await?;
            return Ok(());
        }

        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_ok() && *shutdown_rx.borrow() {
                    info!("MEJEPA_DAEMON_SUPERVISOR_SHUTDOWN: stopping supervised flywheel tasks");
                    stop_supervised_tasks(&mut tasks, &config, &restart_state).await?;
                    return Ok(());
                }
            }
            joined = tasks.join_next(), if !tasks.is_empty() => {
                let Some(joined) = joined else {
                    return Err(anyhow::anyhow!("MEJEPA_DAEMON_SUPERVISOR_EMPTY: all supervised tasks exited"));
                };
                let exit = joined.map_err(|err| {
                    anyhow::anyhow!("MEJEPA_DAEMON_TASK_JOIN_FAILED: {err}")
                })?;
                if *shutdown_rx.borrow() {
                    continue;
                }
                let restart = restart_state.record(exit.kind);
                log_task_exit(&exit, restart);
                record_flywheel_job_run(
                    &db,
                    &FlywheelJobRunRecord::task_exit(exit.kind, &exit, restart),
                )?;
                if restart.flapping {
                    record_scheduler_flapping(&db, exit.kind, restart.count_in_window)?;
                }
                write_supervisor_status(&config, "restarting", &restart_state.restart_counts())?;
                tokio::select! {
                    _ = tokio::time::sleep(restart.backoff) => {}
                    changed = shutdown_rx.changed() => {
                        if changed.is_ok() && *shutdown_rx.borrow() {
                            continue;
                        }
                    }
                }
                if !*shutdown_rx.borrow() {
                    spawn_supervised_task(
                        &mut tasks,
                        exit.kind,
                        config.clone(),
                        db.clone(),
                        shutdown_rx.clone(),
                        runner.clone(),
                    );
                    write_supervisor_status(&config, "running", &restart_state.restart_counts())?;
                }
            }
        }
    }
}

async fn stop_supervised_tasks(
    tasks: &mut JoinSet<TaskExit>,
    config: &FlywheelDaemonConfig,
    restart_state: &RestartState,
) -> Result<()> {
    let timeout = tokio::time::sleep(SUPERVISOR_SHUTDOWN_TIMEOUT);
    tokio::pin!(timeout);
    while !tasks.is_empty() {
        tokio::select! {
            joined = tasks.join_next() => {
                match joined {
                    Some(Ok(exit)) => {
                        if let Some(detail) = task_exit_detail(&exit.status) {
                            warn!(
                                task = exit.kind.name(),
                                detail = %detail,
                                "MEJEPA_DAEMON_TASK_STOPPED_WITH_ERROR_DURING_SHUTDOWN"
                            );
                        }
                    }
                    Some(Err(err)) => {
                        warn!(
                            error = %err,
                            "MEJEPA_DAEMON_TASK_JOIN_ERROR_DURING_SHUTDOWN"
                        );
                    }
                    None => break,
                }
            }
            _ = &mut timeout => {
                return Err(anyhow::anyhow!(
                    "MEJEPA_DAEMON_SUPERVISOR_SHUTDOWN_TIMEOUT: {} tasks still running after {:?}",
                    tasks.len(),
                    SUPERVISOR_SHUTDOWN_TIMEOUT
                ));
            }
        }
    }
    write_supervisor_status(config, "stopped", &restart_state.restart_counts())?;
    Ok(())
}

fn task_exit_detail(status: &TaskExitStatus) -> Option<&str> {
    match status {
        TaskExitStatus::Completed => None,
        TaskExitStatus::Error(detail) | TaskExitStatus::Panic(detail) => Some(detail.as_str()),
    }
}

fn spawn_supervised_task(
    tasks: &mut JoinSet<TaskExit>,
    kind: SupervisedTaskKind,
    config: FlywheelDaemonConfig,
    db: Arc<DB>,
    shutdown_rx: watch::Receiver<bool>,
    runner: TaskRunner,
) {
    tasks.spawn(async move {
        let started = chrono::Utc::now().timestamp();
        let run_id = flywheel_run_id(kind, "task_started");
        if let Err(err) = record_flywheel_job_run(
            &db,
            &FlywheelJobRunRecord::started(kind, "task_started", run_id),
        ) {
            return TaskExit::errored(
                kind,
                started,
                format!("MEJEPA_FLYWHEEL_JOB_RUN_LEDGER_WRITE_FAILED: {err}"),
            );
        }
        let result = std::panic::AssertUnwindSafe(runner(kind, config, db, shutdown_rx))
            .catch_unwind()
            .await;
        match result {
            Ok(Ok(())) => TaskExit::completed(kind, started),
            Ok(Err(err)) => TaskExit::errored(kind, started, err.to_string()),
            Err(payload) => TaskExit::panicked(kind, started, panic_payload(payload)),
        }
    });
}

async fn run_task(
    kind: SupervisedTaskKind,
    config: FlywheelDaemonConfig,
    db: Arc<DB>,
    shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    match kind {
        SupervisedTaskKind::ShiftSubscriber => run_shift_subscriber(config, db, shutdown_rx).await,
        SupervisedTaskKind::SelfOptimizationScheduler => {
            run_self_optimization_scheduler(config, db, shutdown_rx).await
        }
        SupervisedTaskKind::NightlyGcScheduler => {
            run_nightly_gc_scheduler(config, db, shutdown_rx).await
        }
        SupervisedTaskKind::WeeklyEvalScheduler => {
            run_weekly_eval_scheduler(config, db, shutdown_rx).await
        }
    }
}

async fn run_shift_subscriber(
    config: FlywheelDaemonConfig,
    db: Arc<DB>,
    shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    let subscriber_config = context_graph_mejepa_shift_subscriber::MeJepaShiftSubscriberConfig {
        infer_db_path: config.paths.storage_db.clone(),
        panel_db_path: config.paths.panel_db.clone(),
        repo_root: config.repo_root,
        shift_log_dir: config.paths.shift_log_dir,
        l_step_observe_threshold: 0.05,
        max_concurrent_shifts: 1,
        lag_alert_threshold_shifts: 500,
        lag_alert_sustain_seconds: 60,
        tail_poll_interval_ms: 250,
    };
    let subscriber = context_graph_mejepa_shift_subscriber::ShiftSubscriber::open_with_db(
        subscriber_config,
        db,
    )?;
    subscriber.run_until_shutdown(shutdown_rx).await?;
    Ok(())
}

async fn run_self_optimization_scheduler(
    config: FlywheelDaemonConfig,
    db: Arc<DB>,
    shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    let scheduler_config = self_optimization_scheduler_config(&config);
    context_graph_mejepa::heal::run_self_optimization_scheduler(db, scheduler_config, shutdown_rx)
        .await?;
    Ok(())
}

fn self_optimization_scheduler_config(
    config: &FlywheelDaemonConfig,
) -> context_graph_mejepa::heal::SelfOptimConfig {
    context_graph_mejepa::heal::SelfOptimConfig {
        status_path: config
            .paths
            .scheduler_state_dir
            .join("self_optimization_status.json"),
        hygiene_archive_root: config.paths.hygiene_archive_dir.clone(),
        witness_chain_path: config
            .paths
            .hygiene_archive_dir
            .join("self-optimization-witness-chain.bin"),
        ..context_graph_mejepa::heal::SelfOptimConfig::default()
    }
}

async fn run_nightly_gc_scheduler(
    config: FlywheelDaemonConfig,
    db: Arc<DB>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    loop {
        let delay = next_nightly_gc_delay(&config)?;
        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_ok() && *shutdown_rx.borrow() {
                    return Ok(());
                }
            }
            _ = tokio::time::sleep(delay) => {
                run_nightly_gc_job_once(&config, db.clone()).await?;
            }
        }
    }
}

async fn run_weekly_eval_scheduler(
    config: FlywheelDaemonConfig,
    db: Arc<DB>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    loop {
        let delay = next_weekly_eval_delay(&config)?;
        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_ok() && *shutdown_rx.borrow() {
                    return Ok(());
                }
            }
            _ = tokio::time::sleep(delay) => {
                run_weekly_eval_job_once(&config, db.clone()).await?;
            }
        }
    }
}

fn next_nightly_gc_delay(config: &FlywheelDaemonConfig) -> Result<Duration> {
    match config.nightly_gc_interval_override {
        Some(period) => {
            validate_scheduler_period("nightly_gc_interval_override", period)?;
            Ok(period)
        }
        None => duration_until_next_local_hour(3),
    }
}

fn next_weekly_eval_delay(config: &FlywheelDaemonConfig) -> Result<Duration> {
    match config.weekly_eval_interval_override {
        Some(period) => {
            validate_scheduler_period("weekly_eval_interval_override", period)?;
            Ok(period)
        }
        None => Ok(Duration::from_secs(7 * 24 * 3600)),
    }
}

pub async fn run_nightly_gc_job_once(
    config: &FlywheelDaemonConfig,
    db: Arc<DB>,
) -> Result<serde_json::Value> {
    let started = FlywheelJobRunRecord::started(
        SupervisedTaskKind::NightlyGcScheduler,
        "scheduled_run",
        flywheel_run_id(SupervisedTaskKind::NightlyGcScheduler, "scheduled_run"),
    );
    record_flywheel_job_run(db.as_ref(), &started)?;
    let db_for_gc = db.clone();
    let archive_root = config.paths.hygiene_archive_dir.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<_> {
        let runtime = context_graph_mejepa_hygiene::runtime_config(db_for_gc, archive_root)?;
        let env = context_graph_mejepa_hygiene::HygieneEnv::try_new(runtime)?;
        Ok(context_graph_mejepa_hygiene::gc_run_nightly(&env)?)
    })
    .await
    .context("MEJEPA_GC_SCHEDULER_JOIN_FAILED")?;
    match result {
        Ok(report) => {
            write_scheduler_report(config, "nightly_gc", &report)?;
            let details = serde_json::to_value(&report)?;
            record_flywheel_job_run(db.as_ref(), &started.completed(details.clone()))?;
            Ok(details)
        }
        Err(err) => {
            record_flywheel_job_run(db.as_ref(), &started.failed(&err))?;
            Err(err)
        }
    }
}

pub async fn run_weekly_eval_job_once(
    config: &FlywheelDaemonConfig,
    db: Arc<DB>,
) -> Result<serde_json::Value> {
    let started = FlywheelJobRunRecord::started(
        SupervisedTaskKind::WeeklyEvalScheduler,
        "scheduled_run",
        flywheel_run_id(SupervisedTaskKind::WeeklyEvalScheduler, "scheduled_run"),
    );
    record_flywheel_job_run(db.as_ref(), &started)?;
    let db_for_eval = db.clone();
    let config_for_eval = config.clone();
    let result =
        tokio::task::spawn_blocking(move || run_weekly_eval_once(&config_for_eval, db_for_eval))
            .await
            .context("MEJEPA_WEEKLY_EVAL_JOIN_FAILED")?;
    match result {
        Ok(summary) => {
            write_scheduler_report(config, "weekly_eval", &summary)?;
            record_flywheel_job_run(db.as_ref(), &started.completed(summary.clone()))?;
            Ok(summary)
        }
        Err(err) => {
            record_flywheel_job_run(db.as_ref(), &started.failed(&err))?;
            Err(err)
        }
    }
}

fn duration_until_next_local_hour(hour: u32) -> Result<Duration> {
    use chrono::{Datelike, Local, TimeZone};
    let now = Local::now();
    let today = Local
        .with_ymd_and_hms(now.year(), now.month(), now.day(), hour, 0, 0)
        .single()
        .ok_or_else(|| anyhow::anyhow!("MEJEPA_SCHEDULER_TIME_INVALID"))?;
    let next = if today > now {
        today
    } else {
        today + chrono::Duration::days(1)
    };
    Ok((next - now).to_std()?)
}

fn run_weekly_eval_once(config: &FlywheelDaemonConfig, db: Arc<DB>) -> Result<serde_json::Value> {
    let report_date = chrono::Local::now().date_naive().to_string();
    let holdout_path = config
        .paths
        .root
        .join("state/gold-labels/weekly_holdout_panels.json");
    let holdout_file = File::open(&holdout_path).with_context(|| {
        format!(
            "MEJEPA_EVAL_HOLDOUT_UNAVAILABLE: {}",
            holdout_path.display()
        )
    })?;
    let holdout: Vec<context_graph_mejepa::HoldoutPanel> = serde_json::from_reader(holdout_file)
        .with_context(|| {
            format!(
                "MEJEPA_EVAL_HOLDOUT_INVALID_JSON: {}",
                holdout_path.display()
            )
        })?;
    if holdout.is_empty() {
        return Err(anyhow::anyhow!(
            "MEJEPA_EVAL_EMPTY_HOLDOUT: {} contained zero panels",
            holdout_path.display()
        ));
    }

    let calibration = context_graph_mejepa::CalibrationStore::new(
        db.clone(),
        context_graph_mejepa::INFER_DEFAULT_MAX_CALIBRATION_AGE_DAYS,
    )?;
    let active_calibration = calibration.load_active()?;
    let infer_store = Arc::new(context_graph_mejepa::RocksDbInferStore::new(db.clone()));
    let eval_infer_config = context_graph_mejepa::MeJepaInferConfig {
        // Evaluation must measure reject/OOD behavior across the full holdout.
        // The production verify path still refuses OOD samples; this compiler
        // config prevents the report job from crashing before it can record the
        // OOD score, GTau result, and active-learning escalation evidence.
        ood_refuse_threshold: 1.0,
        ..context_graph_mejepa::MeJepaInferConfig::default()
    };
    let compiler = Arc::new(context_graph_mejepa::build_deterministic_compiler(
        config.repo_root.clone(),
        infer_store,
        calibration,
        eval_infer_config,
    )?);
    let eval_store = context_graph_mejepa::RocksDbEvalStore::new(db.clone())?;
    let provenance = context_graph_mejepa::EvalProvenance {
        corpus_sha: context_graph_mejepa::corpus_sha_from_holdout(&holdout),
        eval_code_version: env!("CARGO_PKG_VERSION").to_string(),
        calibration_version: active_calibration.version.clone(),
        generated_by: "context-graph-mcp-daemon-weekly-eval".to_string(),
    };
    let runner = context_graph_mejepa::EvalRunner::new(
        compiler,
        eval_store.clone(),
        context_graph_mejepa::EvalConfig::default(),
    )?;
    let report = runner.run_holdout_eval(&holdout, &[], report_date.clone(), provenance)?;
    let latest = eval_store.load_latest_report()?.ok_or_else(|| {
        anyhow::anyhow!(
            "MEJEPA_EVAL_REPORT_READBACK_MISSING: latest report missing after weekly eval"
        )
    })?;
    let report_hash = report.determinism_hash()?;
    if latest.determinism_hash()? != report_hash {
        return Err(anyhow::anyhow!(
            "MEJEPA_EVAL_REPORT_READBACK_MISMATCH: latest report hash differs after weekly eval"
        ));
    }

    let report_dir = config.paths.root.join("exports/eval").join(&report_date);
    let markdown_path = report_dir.join("weekly.md");
    let operational = weekly_operational_summary(db.as_ref())?;
    write_weekly_eval_markdown(&markdown_path, &report, &report_hash, &operational)?;
    let json_path = report_dir.join("weekly.json");
    context_graph_mejepa::eval::report::write_json_0600(&json_path, &report)?;
    let pairwise_dir = config.paths.root.join("exports/pairwise-mi");
    let pairwise_csv_path = pairwise_dir.join(format!("{report_date}.csv"));
    let pairwise_mi_heatmap = if pairwise_csv_path.is_file() {
        let pairwise_markdown_path = pairwise_dir.join(format!("{report_date}.md"));
        let pairwise_png_path = pairwise_dir.join(format!("{report_date}.png"));
        let render = context_graph_mejepa::eval::render_pairwise_mi_heatmap(
            &pairwise_csv_path,
            &pairwise_markdown_path,
            &pairwise_png_path,
        )
        .with_context(|| {
            format!(
                "MEJEPA_PAIRWISE_MI_HEATMAP_RENDER_FAILED: {}",
                pairwise_csv_path.display()
            )
        })?;
        Some(serde_json::json!({
            "source_csv_path": pairwise_csv_path,
            "markdown_path": pairwise_markdown_path,
            "png_path": pairwise_png_path,
            "slot_count": render.slot_count,
            "max_off_diagonal": render.max_off_diagonal,
            "readback_equal": render.readback_equal,
        }))
    } else {
        None
    };
    Ok(serde_json::json!({
        "status": "completed",
        "report_date": report_date,
        "holdout_count": report.holdout_count,
        "ship_gate_passed": report.ship_gate_passed,
        "ship_gate_failures": report.ship_gate_failures,
        "report_hash": report_hash,
        "source_of_truth": "CF_MEJEPA_EVAL_REPORTS",
        "holdout_path": holdout_path,
        "weekly_markdown_path": markdown_path,
        "weekly_json_path": json_path,
        "pairwise_mi_heatmap": pairwise_mi_heatmap,
        "operational_summary": operational,
        "updated_at_unix_seconds": chrono::Utc::now().timestamp(),
    }))
}

fn write_weekly_eval_markdown(
    path: &Path,
    report: &context_graph_mejepa::EvalReport,
    report_hash: &str,
    operational: &WeeklyOperationalSummary,
) -> Result<()> {
    report.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let per_cell_state_transfer = if report.per_cell_state_transfer.is_empty() {
        "- none".to_string()
    } else {
        report
            .per_cell_state_transfer
            .iter()
            .map(|(cell, diagnostic)| match diagnostic {
                Some(diagnostic) => format!(
                    "- {cell}: transfer_score={:.6}, wasserstein_1={:.6}, performance_deploy={:.6}, candidate={}",
                    diagnostic.transfer_score,
                    diagnostic.wasserstein_1,
                    diagnostic.performance_deploy,
                    if diagnostic.transfer_score < 0.80 {
                        "fine_tune"
                    } else {
                        "hold"
                    }
                ),
                None => format!("- {cell}: unavailable"),
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let per_cell_ship_gate = if report.per_cell_correlation.is_empty() {
        "- none".to_string()
    } else {
        report
            .per_cell_correlation
            .iter()
            .map(|(cell, correlation)| match correlation {
                Some(value) if *value >= 0.95 => {
                    format!("- {cell}: pass correlation={value:.6}")
                }
                Some(value) => format!("- {cell}: fail correlation={value:.6} target=0.950000"),
                None => format!("- {cell}: insufficient_samples"),
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let regression_checks = if report.regression_checks.is_empty() {
        "- none".to_string()
    } else {
        report
            .regression_checks
            .iter()
            .map(|check| {
                format!(
                    "- {}: previous={:.6}, current={:.6}, drop={:.6}, passed={}",
                    check.name, check.previous, check.current, check.drop, check.passed
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let per_prediction_class_calibration = if report.per_prediction_class_calibration.is_empty() {
        "- none".to_string()
    } else {
        report
            .per_prediction_class_calibration
            .values()
            .map(|calibration| {
                format!(
                    "- {}: ece={:.6}, samples={}, mean_confidence={:.6}, empirical_accuracy={:.6}, within_tolerance={}",
                    calibration.class_name,
                    calibration.expected_calibration_error,
                    calibration.sample_count,
                    calibration.mean_confidence,
                    calibration.empirical_accuracy,
                    calibration.within_tolerance
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let catastrophic_events = report
        .ship_gate_failures
        .iter()
        .filter(|failure| failure.to_ascii_lowercase().contains("catastrophic"))
        .map(|failure| format!("- {failure}"))
        .collect::<Vec<_>>();
    let catastrophic_events = if catastrophic_events.is_empty() {
        "- none".to_string()
    } else {
        catastrophic_events.join("\n")
    };
    let bytes = format!(
        "# ME-JEPA Weekly Evaluation\n\n\
         - report_date: {}\n\
         - holdout_count: {}\n\
         - ship_gate_passed: {}\n\
         - report_hash: {}\n\
         - source_of_truth: CF_MEJEPA_EVAL_REPORTS\n\
         - q1_pass_rate: {:.6}\n\
         - q2_report_correlation: {:?}\n\
         - q3_side_effect_agreement: {:?}\n\n\
         ## Per-Prediction-Class Calibration\n\n{}\n\n\
         ## Catastrophic Events\n\n{}\n\n\
         ## Per-Cell Ship Gate Status\n\n{}\n\n\
         ## Active-Learning Queue Summary\n\n\
         - queued_count: {}\n\
         - evicted_count: {}\n\
         - ood_escalation_count: {}\n\
         - active_learning_queue_rows: {}\n\
         - ood_escalation_rows: {}\n\n\
         ## Agent-Feedback Summary\n\n\
         - feedback_rows: {}\n\
         - source_of_truth: CF_MEJEPA_AGENT_FEEDBACK\n\n\
         ## Drift Events Of The Week\n\n\
         - drift_window_rows: {}\n\
         - drift_history_rows: {}\n\n{}\n\n\
         ## Promotions / Rollbacks Of The Week\n\n\
         - source_of_truth: CF_MEJEPA_HEAL_REPORTS / CF_MEJEPA_MODEL_PROMOTIONS\n\
         - see `mcp__cgreality__mejepa_weekly_eval_dashboard` for current row counts and readback hashes\n\n\
         ## Storage Utilization\n\n\
         - storage source: /mnt/d/contextgraph runtime root\n\
         - detailed slice readback: `mcp__cgreality__mejepa_quota_status`\n\n\
         ## Per-Cell State Transfer T\n\n{}\n\n\
         ## Ship Gate Failures\n\n{}\n",
        report.report_date,
        report.holdout_count,
        report.ship_gate_passed,
        report_hash,
        report.q1_pass_rate,
        report.q2_report_correlation,
        report.q3_side_effect_agreement,
        per_prediction_class_calibration,
        catastrophic_events,
        per_cell_ship_gate,
        report.active_learning.queued_count,
        report.active_learning.evicted_count,
        report.active_learning.ood_escalation_count,
        operational.active_learning_queue_rows,
        operational.ood_escalation_rows,
        operational.agent_feedback_rows,
        operational.drift_window_rows,
        operational.drift_history_rows,
        regression_checks,
        per_cell_state_transfer,
        if report.ship_gate_failures.is_empty() {
            "- none".to_string()
        } else {
            report
                .ship_gate_failures
                .iter()
                .map(|failure| format!("- {failure}"))
                .collect::<Vec<_>>()
                .join("\n")
        }
    )
    .into_bytes();
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    let readback = fs::read(path)?;
    if readback != bytes {
        return Err(anyhow::anyhow!(
            "MEJEPA_WEEKLY_EVAL_REPORT_READBACK_MISMATCH: {}",
            path.display()
        ));
    }
    Ok(())
}

fn weekly_operational_summary(db: &DB) -> Result<WeeklyOperationalSummary> {
    Ok(WeeklyOperationalSummary {
        agent_feedback_rows: weekly_count_cf(
            db,
            context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK,
        )?,
        drift_window_rows: weekly_count_cf(db, context_graph_mejepa_cf::CF_MEJEPA_DRIFT_WINDOW)?,
        drift_history_rows: weekly_count_cf(db, context_graph_mejepa_cf::CF_MEJEPA_DRIFT_HISTORY)?,
        active_learning_queue_rows: weekly_count_cf(
            db,
            context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
        )?,
        ood_escalation_rows: weekly_count_cf(
            db,
            context_graph_mejepa_cf::CF_MEJEPA_OOD_ESCALATIONS,
        )?,
    })
}

fn weekly_count_cf(db: &DB, cf_name: &str) -> Result<usize> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| anyhow::anyhow!("MEJEPA_WEEKLY_REPORT_CF_MISSING: {cf_name}"))?;
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        item?;
        count += 1;
    }
    Ok(count)
}

#[derive(Debug)]
struct TaskExit {
    kind: SupervisedTaskKind,
    started_unix_seconds: i64,
    completed_unix_seconds: i64,
    status: TaskExitStatus,
}

impl TaskExit {
    fn completed(kind: SupervisedTaskKind, started_unix_seconds: i64) -> Self {
        Self {
            kind,
            started_unix_seconds,
            completed_unix_seconds: chrono::Utc::now().timestamp(),
            status: TaskExitStatus::Completed,
        }
    }

    fn errored(kind: SupervisedTaskKind, started_unix_seconds: i64, detail: String) -> Self {
        Self {
            kind,
            started_unix_seconds,
            completed_unix_seconds: chrono::Utc::now().timestamp(),
            status: TaskExitStatus::Error(detail),
        }
    }

    fn panicked(kind: SupervisedTaskKind, started_unix_seconds: i64, detail: String) -> Self {
        Self {
            kind,
            started_unix_seconds,
            completed_unix_seconds: chrono::Utc::now().timestamp(),
            status: TaskExitStatus::Panic(detail),
        }
    }
}

#[derive(Debug)]
enum TaskExitStatus {
    Completed,
    Error(String),
    Panic(String),
}

#[derive(Default)]
struct RestartState {
    restarts: BTreeMap<SupervisedTaskKind, VecDeque<Instant>>,
}

impl RestartState {
    fn record(&mut self, kind: SupervisedTaskKind) -> RestartDecision {
        let now = Instant::now();
        let restarts = self.restarts.entry(kind).or_default();
        restarts.push_back(now);
        while restarts
            .front()
            .is_some_and(|started| now.duration_since(*started) > RESTART_WINDOW)
        {
            restarts.pop_front();
        }
        let count = restarts.len();
        let shift = count.saturating_sub(1).min(9);
        let backoff = Duration::from_millis(100)
            .saturating_mul(1_u32 << shift)
            .min(BACKOFF_CAP);
        RestartDecision {
            count_in_window: count,
            backoff,
            flapping: count > FLAPPING_THRESHOLD,
        }
    }

    fn restart_counts(&self) -> BTreeMap<&'static str, usize> {
        self.restarts
            .iter()
            .map(|(kind, restarts)| (kind.name(), restarts.len()))
            .collect()
    }
}

#[derive(Clone, Copy)]
struct RestartDecision {
    count_in_window: usize,
    backoff: Duration,
    flapping: bool,
}

fn log_task_exit(exit: &TaskExit, restart: RestartDecision) {
    match &exit.status {
        TaskExitStatus::Completed => warn!(
            error_code = "MEJEPA_DAEMON_TASK_EXITED",
            task = exit.kind.name(),
            started_unix_seconds = exit.started_unix_seconds,
            completed_unix_seconds = exit.completed_unix_seconds,
            restart_count_5m = restart.count_in_window,
            backoff_ms = restart.backoff.as_millis() as u64,
            "supervised flywheel task exited; restarting"
        ),
        TaskExitStatus::Error(detail) => error!(
            error_code = "MEJEPA_DAEMON_TASK_ERROR",
            task = exit.kind.name(),
            started_unix_seconds = exit.started_unix_seconds,
            completed_unix_seconds = exit.completed_unix_seconds,
            restart_count_5m = restart.count_in_window,
            backoff_ms = restart.backoff.as_millis() as u64,
            detail = %detail,
            "supervised flywheel task returned an error; restarting"
        ),
        TaskExitStatus::Panic(detail) => error!(
            error_code = "MEJEPA_DAEMON_TASK_PANIC",
            task = exit.kind.name(),
            started_unix_seconds = exit.started_unix_seconds,
            completed_unix_seconds = exit.completed_unix_seconds,
            restart_count_5m = restart.count_in_window,
            backoff_ms = restart.backoff.as_millis() as u64,
            detail = %detail,
            "supervised flywheel task panicked; restarting"
        ),
    }
}

fn record_scheduler_flapping(
    db: &DB,
    kind: SupervisedTaskKind,
    restart_count: usize,
) -> Result<()> {
    use context_graph_mejepa::heal::pipeline::StatusChange;
    use context_graph_mejepa::heal::promote::{HealReport, HoldoutEval, ModeWinner, TriggerReason};
    let summary = format!("MEJEPA_SCHEDULER_FLAPPING:{}:{restart_count}", kind.name());
    let mut hasher = Sha256::new();
    hasher.update(summary.as_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    let eval = HoldoutEval::try_new(0.0, 0.0, 0.0, 1, digest)?;
    let report = HealReport {
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
        trigger_reason: TriggerReason::DriftCatastrophic,
    };
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS)
        .ok_or_else(|| anyhow::anyhow!("missing CF_MEJEPA_HEAL_REPORTS"))?;
    let key = format!(
        "scheduler-flapping:{}:{}",
        kind.name(),
        chrono::Utc::now().timestamp_millis()
    );
    let value = bincode::serialize(&report)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key.as_bytes(), &value, &opts)?;
    let readback = db
        .get_cf(cf, key.as_bytes())?
        .ok_or_else(|| anyhow::anyhow!("MEJEPA_SCHEDULER_FLAPPING_READBACK_MISSING"))?;
    if readback != value {
        return Err(anyhow::anyhow!(
            "MEJEPA_SCHEDULER_FLAPPING_READBACK_MISMATCH"
        ));
    }
    error!(
        error_code = "MEJEPA_SCHEDULER_FLAPPING",
        severity = "Catastrophic",
        task = kind.name(),
        restart_count_5m = restart_count,
        cf = context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS,
        key = %key,
        "supervised scheduler task is flapping; catastrophic heal report persisted"
    );
    Ok(())
}

pub fn ensure_mejepa_column_families(db: &DB) -> Result<()> {
    for cf in context_graph_mejepa_cf::all_hygiene_referenced_cfs() {
        if db.cf_handle(cf).is_none() {
            return Err(anyhow::anyhow!(
                "MEJEPA_CF_MISSING: {cf}; storage opener must register all ME-JEPA column families before daemon startup"
            ));
        }
    }
    Ok(())
}

fn write_scheduler_report(
    config: &FlywheelDaemonConfig,
    name: &str,
    value: &impl Serialize,
) -> Result<()> {
    let path = config
        .paths
        .scheduler_state_dir
        .join(format!("{name}_status.json"));
    write_json_atomic(&path, value)
}

fn write_supervisor_status(
    config: &FlywheelDaemonConfig,
    status: &str,
    restart_counts: &BTreeMap<&'static str, usize>,
) -> Result<()> {
    write_json_atomic(
        &config
            .paths
            .scheduler_state_dir
            .join("flywheel_supervisor_status.json"),
        &serde_json::json!({
            "status": status,
            "pid": std::process::id(),
            "supervised_tasks": supervised_task_names(),
            "restart_counts": restart_counts,
            "updated_at_unix_seconds": chrono::Utc::now().timestamp(),
        }),
    )
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value)?;
    {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    if let Some(parent) = path.parent() {
        if let Ok(dir) = File::open(parent) {
            dir.sync_all()?;
        }
    }
    let readback = fs::read(path)?;
    if readback != bytes {
        return Err(anyhow::anyhow!(
            "MEJEPA_DAEMON_STATE_READBACK_MISMATCH: {}",
            path.display()
        ));
    }
    Ok(())
}

fn append_daemon_log(path: &PathBuf, line: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(
        file,
        "{} {}",
        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        line
    )?;
    file.flush()?;
    Ok(())
}

fn panic_payload(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn private_tempdir() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("tempdir");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700))
                .expect("set private tempdir permissions");
        }
        temp
    }

    fn open_mejepa_test_db(path: &Path) -> DB {
        let mut opts = rocksdb::Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        let descriptors = context_graph_mejepa_cf::all_hygiene_referenced_cfs()
            .into_iter()
            .map(|cf| rocksdb::ColumnFamilyDescriptor::new(cf, rocksdb::Options::default()))
            .collect::<Vec<_>>();
        DB::open_cf_descriptors(&opts, path, descriptors).expect("open db")
    }

    #[test]
    fn supervised_task_name_contract_is_stable() {
        assert_eq!(
            supervised_task_names(),
            vec![
                "shift_subscriber",
                "self_optimization_scheduler",
                "nightly_gc_scheduler",
                "weekly_eval_scheduler"
            ]
        );
    }

    #[test]
    fn flapping_report_round_trips_through_heal_reports_cf() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db = open_mejepa_test_db(temp.path());
        ensure_mejepa_column_families(&db).expect("ensure cfs");
        record_scheduler_flapping(&db, SupervisedTaskKind::ShiftSubscriber, 6)
            .expect("write flapping report");
        let cf = db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS)
            .expect("cf");
        let count = db.iterator_cf(cf, rocksdb::IteratorMode::Start).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn flywheel_job_run_record_round_trips_through_source_of_truth_cf() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db = open_mejepa_test_db(temp.path());
        ensure_mejepa_column_families(&db).expect("ensure cfs");
        let record = FlywheelJobRunRecord::started(
            SupervisedTaskKind::WeeklyEvalScheduler,
            "scheduled_run",
            flywheel_run_id(SupervisedTaskKind::WeeklyEvalScheduler, "unit_test"),
        )
        .completed(serde_json::json!({"weekly_json_path": "/mnt/d/contextgraph/exports/eval/unit/weekly.json"}));
        let key_hex = record_flywheel_job_run(&db, &record).expect("record job run");
        let key = hex::decode(key_hex).expect("decode key");
        let cf = db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_FLYWHEEL_JOB_RUNS)
            .expect("job run cf");
        let readback = db.get_cf(cf, &key).expect("read cf").expect("row");
        let decoded: FlywheelJobRunRecord = serde_json::from_slice(&readback).expect("decode row");
        assert_eq!(decoded, record);
    }

    #[test]
    fn scheduler_period_override_rejects_zero_duration() {
        let temp = private_tempdir();
        let paths =
            crate::daemon_validate::validate_d_drive_root(temp.path()).expect("valid daemon root");
        let config = FlywheelDaemonConfig::from_paths(paths, temp.path().to_path_buf());
        let err = config
            .with_test_scheduler_periods(Duration::ZERO, Duration::from_secs(1))
            .expect_err("zero period must fail closed");
        assert!(err
            .to_string()
            .contains("MEJEPA_DAEMON_SCHEDULER_PERIOD_INVALID"));
    }

    #[test]
    fn self_optimization_scheduler_config_uses_daemon_source_of_truth_paths() {
        let temp = private_tempdir();
        let paths =
            crate::daemon_validate::validate_d_drive_root(temp.path()).expect("valid daemon root");
        let config = FlywheelDaemonConfig::from_paths(paths.clone(), temp.path().to_path_buf());
        let scheduler_config = self_optimization_scheduler_config(&config);

        assert_eq!(
            scheduler_config.status_path,
            paths
                .scheduler_state_dir
                .join("self_optimization_status.json")
        );
        assert_eq!(
            scheduler_config.hygiene_archive_root,
            paths.hygiene_archive_dir
        );
        assert_eq!(
            scheduler_config.witness_chain_path,
            paths
                .hygiene_archive_dir
                .join("self-optimization-witness-chain.bin")
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn supervisor_restart_flapping_writes_heal_report_source_of_truth() {
        let temp = private_tempdir();
        let paths =
            crate::daemon_validate::validate_d_drive_root(temp.path()).expect("valid daemon root");
        let db = Arc::new(open_mejepa_test_db(&paths.storage_db));
        let config = FlywheelDaemonConfig::from_paths(paths.clone(), temp.path().to_path_buf());
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let runner: TaskRunner = Arc::new(|kind, _config, _db, mut shutdown_rx| {
            Box::pin(async move {
                if kind == SupervisedTaskKind::ShiftSubscriber {
                    panic!("synthetic supervisor panic for FSV");
                }
                loop {
                    if *shutdown_rx.borrow() {
                        return Ok(());
                    }
                    if shutdown_rx.changed().await.is_err() {
                        return Ok(());
                    }
                }
            })
        });

        let supervisor = tokio::spawn(supervise_with_runner(
            config,
            db.clone(),
            shutdown_rx,
            runner,
        ));
        let cf = db
            .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS)
            .expect("heal reports cf");
        let mut heal_report_count = 0usize;
        for _ in 0..120 {
            heal_report_count = db.iterator_cf(cf, rocksdb::IteratorMode::Start).count();
            if heal_report_count > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let running_status_path = paths
            .scheduler_state_dir
            .join("flywheel_supervisor_status.json");
        let running_status = fs::read_to_string(&running_status_path).expect("running status");
        println!("FSV_SUPERVISOR_RUNNING_STATUS={running_status}");
        println!("FSV_HEAL_REPORT_COUNT={heal_report_count}");
        assert!(
            heal_report_count > 0,
            "MEJEPA_SCHEDULER_FLAPPING heal report missing from source-of-truth CF"
        );

        shutdown_tx.send(true).expect("send shutdown");
        let result = tokio::time::timeout(Duration::from_secs(10), supervisor)
            .await
            .expect("supervisor shutdown timeout")
            .expect("join supervisor");
        result.expect("supervisor result");

        let stopped_status = fs::read_to_string(&running_status_path).expect("stopped status");
        println!("FSV_SUPERVISOR_STOPPED_STATUS={stopped_status}");
        assert!(stopped_status.contains("\"status\": \"stopped\""));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn self_optimization_scheduler_panic_is_caught_by_supervisor_wrapper() {
        let temp = private_tempdir();
        let paths =
            crate::daemon_validate::validate_d_drive_root(temp.path()).expect("valid daemon root");
        let db = Arc::new(open_mejepa_test_db(&paths.storage_db));
        let config = FlywheelDaemonConfig::from_paths(paths, temp.path().to_path_buf());
        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let runner: TaskRunner = Arc::new(|kind, _config, _db, _shutdown_rx| {
            Box::pin(async move {
                if kind == SupervisedTaskKind::SelfOptimizationScheduler {
                    panic!("synthetic scheduler panic for FSV");
                }
                Ok(())
            })
        });
        let mut tasks = JoinSet::new();
        spawn_supervised_task(
            &mut tasks,
            SupervisedTaskKind::SelfOptimizationScheduler,
            config,
            db,
            shutdown_rx,
            runner,
        );
        let exit = tasks
            .join_next()
            .await
            .expect("task exit")
            .expect("task join");
        assert_eq!(exit.kind, SupervisedTaskKind::SelfOptimizationScheduler);
        match exit.status {
            TaskExitStatus::Panic(detail) => {
                assert!(detail.contains("synthetic scheduler panic for FSV"));
            }
            _ => panic!("expected panic task exit"),
        }
    }
}
