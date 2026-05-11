use std::collections::{BTreeMap, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::future::Future;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use futures_util::FutureExt;
use rocksdb::{WriteOptions, DB};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

use crate::daemon_validate::DaemonPaths;

const RESTART_WINDOW: Duration = Duration::from_secs(5 * 60);
const FLAPPING_THRESHOLD: usize = 5;
const BACKOFF_CAP: Duration = Duration::from_secs(60);

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

#[derive(Debug, Clone)]
pub struct FlywheelDaemonConfig {
    pub paths: DaemonPaths,
    pub repo_root: PathBuf,
}

impl FlywheelDaemonConfig {
    pub fn from_paths(paths: DaemonPaths, repo_root: PathBuf) -> Self {
        Self { paths, repo_root }
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
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
            write_supervisor_status(&config, "stopped", &restart_state.restart_counts())?;
            return Ok(());
        }

        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_ok() && *shutdown_rx.borrow() {
                    info!("MEJEPA_DAEMON_SUPERVISOR_SHUTDOWN: stopping supervised flywheel tasks");
                    tasks.abort_all();
                    while tasks.join_next().await.is_some() {}
                    write_supervisor_status(&config, "stopped", &restart_state.restart_counts())?;
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
            run_self_optimization_scheduler(config, shutdown_rx).await
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
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    let mut ticker_counts = BTreeMap::<&'static str, u64>::new();
    let mut observe = tokio::time::interval(Duration::from_millis(250));
    let mut drift = tokio::time::interval(Duration::from_secs(60));
    let mut active_learning = tokio::time::interval(Duration::from_secs(600));
    let mut promote = tokio::time::interval(Duration::from_secs(3600));
    let mut continual_backprop = tokio::time::interval(Duration::from_secs(1800));
    let mut constellation = tokio::time::interval(Duration::from_secs(3600));
    loop {
        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_ok() && *shutdown_rx.borrow() {
                    return Ok(());
                }
            }
            _ = observe.tick() => write_scheduler_tick(&config, "observe", &mut ticker_counts)?,
            _ = drift.tick() => write_scheduler_tick(&config, "drift_check", &mut ticker_counts)?,
            _ = active_learning.tick() => write_scheduler_tick(&config, "active_learning", &mut ticker_counts)?,
            _ = promote.tick() => write_scheduler_tick(&config, "promote", &mut ticker_counts)?,
            _ = continual_backprop.tick() => write_scheduler_tick(&config, "continual_backprop", &mut ticker_counts)?,
            _ = constellation.tick() => write_scheduler_tick(&config, "constellation_freshness", &mut ticker_counts)?,
        }
    }
}

async fn run_nightly_gc_scheduler(
    config: FlywheelDaemonConfig,
    db: Arc<DB>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    loop {
        let delay = duration_until_next_local_hour(3)?;
        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_ok() && *shutdown_rx.borrow() {
                    return Ok(());
                }
            }
            _ = tokio::time::sleep(delay) => {
                let db = db.clone();
                let archive_root = config.paths.hygiene_archive_dir.clone();
                let report = tokio::task::spawn_blocking(move || -> Result<_> {
                    let runtime = context_graph_mejepa_hygiene::runtime_config(db, archive_root)?;
                    let env = context_graph_mejepa_hygiene::HygieneEnv::try_new(runtime)?;
                    Ok(context_graph_mejepa_hygiene::gc_run_nightly(&env)?)
                }).await.context("MEJEPA_GC_SCHEDULER_JOIN_FAILED")??;
                write_scheduler_report(&config, "nightly_gc", &report)?;
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
        let delay = Duration::from_secs(7 * 24 * 3600);
        tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_ok() && *shutdown_rx.borrow() {
                    return Ok(());
                }
            }
            _ = tokio::time::sleep(delay) => {
                let db = db.clone();
                let config_for_eval = config.clone();
                let report_summary = tokio::task::spawn_blocking(move || {
                    run_weekly_eval_once(&config_for_eval, db)
                })
                .await
                .context("MEJEPA_WEEKLY_EVAL_JOIN_FAILED")??;
                write_scheduler_report(&config, "weekly_eval", &report_summary)?;
            }
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
    let compiler = Arc::new(context_graph_mejepa::build_deterministic_compiler(
        config.repo_root.clone(),
        infer_store,
        calibration,
        context_graph_mejepa::MeJepaInferConfig::default(),
    )?);
    let eval_store = context_graph_mejepa::RocksDbEvalStore::new(db)?;
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
    write_weekly_eval_markdown(&markdown_path, &report, &report_hash)?;
    let json_path = report_dir.join("weekly.json");
    context_graph_mejepa::eval::report::write_json_0600(&json_path, &report)?;
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
        "updated_at_unix_seconds": chrono::Utc::now().timestamp(),
    }))
}

fn write_weekly_eval_markdown(
    path: &Path,
    report: &context_graph_mejepa::EvalReport,
    report_hash: &str,
) -> Result<()> {
    report.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
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
         ## Ship Gate Failures\n\n{}\n",
        report.report_date,
        report.holdout_count,
        report.ship_gate_passed,
        report_hash,
        report.q1_pass_rate,
        report.q2_report_correlation,
        report.q3_side_effect_agreement,
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

fn write_scheduler_tick(
    config: &FlywheelDaemonConfig,
    ticker: &'static str,
    counts: &mut BTreeMap<&'static str, u64>,
) -> Result<()> {
    *counts.entry(ticker).or_insert(0) += 1;
    write_scheduler_report(
        config,
        "self_optimization",
        &serde_json::json!({
            "last_ticker": ticker,
            "ticker_counts": counts,
            "updated_at_unix_seconds": chrono::Utc::now().timestamp(),
        }),
    )
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn supervisor_restart_flapping_writes_heal_report_source_of_truth() {
        let temp = tempfile::tempdir().expect("tempdir");
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
        let temp = tempfile::tempdir().expect("tempdir");
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
