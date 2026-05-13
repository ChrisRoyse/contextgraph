use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::daemon::{
    supervised_task_health_path, supervised_task_names, SupervisedTaskHealthRecord,
    SupervisedTaskHealthRegistrySnapshot,
};
use crate::daemon_validate::DaemonPaths;

const DEFAULT_HEALTH_PROBE_BIND: &str = "127.0.0.1";
const DEFAULT_HEALTH_PROBE_PORT: u16 = 9111;
const FLAPPING_THRESHOLD: usize = 5;

#[derive(Debug, Clone)]
pub struct HealthProbeConfig {
    pub bind_addr: SocketAddr,
    pub scheduler_state_dir: PathBuf,
}

impl HealthProbeConfig {
    pub fn new(bind_addr: SocketAddr, scheduler_state_dir: PathBuf) -> Self {
        Self {
            bind_addr,
            scheduler_state_dir,
        }
    }
}

pub fn config_from_env(paths: &DaemonPaths) -> Result<Option<HealthProbeConfig>> {
    let enabled = match std::env::var("CONTEXT_GRAPH_HEALTH_PROBE") {
        Ok(value) => parse_env_bool("CONTEXT_GRAPH_HEALTH_PROBE", &value)?,
        Err(std::env::VarError::NotPresent) => false,
        Err(err) => {
            anyhow::bail!("MEJEPA_HEALTH_PROBE_CONFIG_INVALID: CONTEXT_GRAPH_HEALTH_PROBE is not valid Unicode: {err}")
        }
    };
    let port_from_env = std::env::var("CONTEXT_GRAPH_HEALTH_PROBE_PORT").ok();
    if !enabled && port_from_env.is_none() {
        return Ok(None);
    }

    let bind = std::env::var("CONTEXT_GRAPH_HEALTH_PROBE_BIND")
        .unwrap_or_else(|_| DEFAULT_HEALTH_PROBE_BIND.to_string());
    let port = match port_from_env {
        Some(raw) => parse_port("CONTEXT_GRAPH_HEALTH_PROBE_PORT", &raw)?,
        None => DEFAULT_HEALTH_PROBE_PORT,
    };
    let bind_addr = format!("{bind}:{port}").parse().with_context(|| {
        format!(
            "MEJEPA_HEALTH_PROBE_BIND_INVALID: bind={bind:?} port={port}; expected IPv4 host and port"
        )
    })?;
    Ok(Some(HealthProbeConfig::new(
        bind_addr,
        paths.scheduler_state_dir.clone(),
    )))
}

fn parse_env_bool(name: &str, raw: &str) -> Result<bool> {
    match raw.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => anyhow::bail!(
            "MEJEPA_HEALTH_PROBE_CONFIG_INVALID: {name} must be one of true,false,1,0,yes,no,on,off"
        ),
    }
}

fn parse_port(name: &str, raw: &str) -> Result<u16> {
    let port = raw.parse::<u16>().with_context(|| {
        format!("MEJEPA_HEALTH_PROBE_PORT_INVALID: {name} must be a port number")
    })?;
    if port == 0 {
        anyhow::bail!("MEJEPA_HEALTH_PROBE_PORT_INVALID: {name} must be in range 1-65535");
    }
    Ok(port)
}

pub struct HealthProbeHandle {
    pub local_addr: SocketAddr,
    shutdown_tx: watch::Sender<bool>,
    pub join_handle: tokio::task::JoinHandle<Result<()>>,
}

impl HealthProbeHandle {
    pub async fn shutdown(self) -> Result<()> {
        let _ = self.shutdown_tx.send(true);
        self.join_handle
            .await
            .context("MEJEPA_HEALTH_PROBE_JOIN_FAILED")?
    }
}

#[derive(Debug, Clone)]
struct HealthProbeState {
    scheduler_state_dir: PathBuf,
}

pub async fn start_health_probe(config: HealthProbeConfig) -> Result<HealthProbeHandle> {
    let listener = TcpListener::bind(config.bind_addr)
        .await
        .with_context(|| format!("MEJEPA_HEALTH_PROBE_BIND_FAILED: {}", config.bind_addr))?;
    let local_addr = listener
        .local_addr()
        .context("MEJEPA_HEALTH_PROBE_LOCAL_ADDR_FAILED")?;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let join_handle = tokio::spawn(run_bound_health_probe(listener, config, shutdown_rx));
    Ok(HealthProbeHandle {
        local_addr,
        shutdown_tx,
        join_handle,
    })
}

async fn run_bound_health_probe(
    listener: TcpListener,
    config: HealthProbeConfig,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .with_state(HealthProbeState {
            scheduler_state_dir: config.scheduler_state_dir,
        });
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            while !*shutdown_rx.borrow() {
                if shutdown_rx.changed().await.is_err() {
                    break;
                }
            }
        })
        .await
        .context("MEJEPA_HEALTH_PROBE_SERVER_FAILED")
}

async fn health() -> Response {
    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "processAlive": true,
            "pid": std::process::id(),
            "endpoint": "/health",
        })),
    )
        .into_response()
}

async fn ready(State(state): State<HealthProbeState>) -> Response {
    let report = evaluate_readiness(&state.scheduler_state_dir);
    let status = if report.ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(report)).into_response()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HealthReadinessReport {
    pub ready: bool,
    pub status: String,
    pub error_code: Option<String>,
    pub message: Option<String>,
    pub status_file: PathBuf,
    pub task_health_file: PathBuf,
    pub restart_counts: BTreeMap<String, usize>,
    pub supervised_tasks: Vec<String>,
    pub task_health: BTreeMap<String, SupervisedTaskHealthRecord>,
    pub unhealthy_tasks: Vec<String>,
    pub generated_at_unix_seconds: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SupervisorStatusSnapshot {
    status: String,
    pid: u32,
    supervised_tasks: Vec<String>,
    restart_counts: BTreeMap<String, usize>,
    updated_at_unix_seconds: i64,
}

pub fn evaluate_readiness(scheduler_state_dir: &Path) -> HealthReadinessReport {
    let status_file = scheduler_state_dir.join("flywheel_supervisor_status.json");
    let task_health_file = supervised_task_health_path(scheduler_state_dir);
    let generated_at_unix_seconds = chrono::Utc::now().timestamp();
    let bytes = match std::fs::read(&status_file) {
        Ok(bytes) => bytes,
        Err(err) => {
            return not_ready(
                (status_file, task_health_file),
                "MEJEPA_HEALTH_PROBE_SUPERVISOR_STATUS_MISSING",
                format!("supervisor status file is not readable: {err}"),
                BTreeMap::new(),
                Vec::new(),
                BTreeMap::new(),
                Vec::new(),
                generated_at_unix_seconds,
            );
        }
    };
    let snapshot: SupervisorStatusSnapshot = match serde_json::from_slice(&bytes) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            return not_ready(
                (status_file, task_health_file),
                "MEJEPA_HEALTH_PROBE_SUPERVISOR_STATUS_INVALID",
                format!("supervisor status file is not valid JSON: {err}"),
                BTreeMap::new(),
                Vec::new(),
                BTreeMap::new(),
                Vec::new(),
                generated_at_unix_seconds,
            );
        }
    };

    let current_pid = std::process::id();
    if snapshot.pid != current_pid {
        return not_ready(
            (status_file, task_health_file),
            "MEJEPA_HEALTH_PROBE_SUPERVISOR_PID_MISMATCH",
            format!(
                "supervisor status pid {} does not match current daemon pid {}",
                snapshot.pid, current_pid
            ),
            snapshot.restart_counts,
            snapshot.supervised_tasks,
            BTreeMap::new(),
            Vec::new(),
            generated_at_unix_seconds,
        );
    }

    let missing_tasks = supervised_task_names()
        .into_iter()
        .filter(|task| !snapshot.supervised_tasks.iter().any(|seen| seen == task))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if !missing_tasks.is_empty() {
        return not_ready(
            (status_file, task_health_file),
            "MEJEPA_HEALTH_PROBE_SUPERVISED_TASK_MISSING",
            format!(
                "supervisor status is missing tasks: {}",
                missing_tasks.join(",")
            ),
            snapshot.restart_counts,
            snapshot.supervised_tasks,
            BTreeMap::new(),
            missing_tasks,
            generated_at_unix_seconds,
        );
    }

    if snapshot.status != "running" {
        return not_ready(
            (status_file, task_health_file),
            "MEJEPA_HEALTH_PROBE_SUPERVISOR_NOT_RUNNING",
            format!("supervisor status is {}", snapshot.status),
            snapshot.restart_counts,
            snapshot.supervised_tasks,
            BTreeMap::new(),
            Vec::new(),
            generated_at_unix_seconds,
        );
    }

    let flapping = snapshot
        .restart_counts
        .iter()
        .filter(|(_, count)| **count > FLAPPING_THRESHOLD)
        .map(|(task, count)| format!("{task}:{count}"))
        .collect::<Vec<_>>();
    if !flapping.is_empty() {
        return not_ready(
            (status_file, task_health_file),
            "MEJEPA_HEALTH_PROBE_SUPERVISOR_FLAPPING",
            format!(
                "restart count exceeded {FLAPPING_THRESHOLD} in 5m window: {}",
                flapping.join(",")
            ),
            snapshot.restart_counts,
            snapshot.supervised_tasks,
            BTreeMap::new(),
            flapping,
            generated_at_unix_seconds,
        );
    }

    let task_health_snapshot = match read_task_health_registry(&task_health_file) {
        Ok(snapshot) => snapshot,
        Err((error_code, message)) => {
            return not_ready(
                (status_file, task_health_file),
                error_code,
                message,
                snapshot.restart_counts,
                snapshot.supervised_tasks,
                BTreeMap::new(),
                Vec::new(),
                generated_at_unix_seconds,
            );
        }
    };
    if let Err((error_code, message, unhealthy_tasks)) =
        validate_task_health_registry(&task_health_snapshot, generated_at_unix_seconds)
    {
        return not_ready(
            (status_file, task_health_file),
            error_code,
            message,
            snapshot.restart_counts,
            snapshot.supervised_tasks,
            task_health_snapshot.tasks,
            unhealthy_tasks,
            generated_at_unix_seconds,
        );
    }

    HealthReadinessReport {
        ready: true,
        status: "ready".to_string(),
        error_code: None,
        message: Some(format!(
            "supervisor pid {} updated at {}",
            snapshot.pid, snapshot.updated_at_unix_seconds
        )),
        status_file,
        task_health_file,
        restart_counts: snapshot.restart_counts,
        supervised_tasks: snapshot.supervised_tasks,
        task_health: task_health_snapshot.tasks,
        unhealthy_tasks: Vec::new(),
        generated_at_unix_seconds,
    }
}

fn read_task_health_registry(
    task_health_file: &Path,
) -> std::result::Result<SupervisedTaskHealthRegistrySnapshot, (&'static str, String)> {
    let bytes = std::fs::read(task_health_file).map_err(|err| {
        (
            "MEJEPA_HEALTH_PROBE_TASK_HEALTH_MISSING",
            format!("supervised task health registry is not readable: {err}"),
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|err| {
        (
            "MEJEPA_HEALTH_PROBE_TASK_HEALTH_INVALID",
            format!("supervised task health registry is not valid JSON: {err}"),
        )
    })
}

fn validate_task_health_registry(
    snapshot: &SupervisedTaskHealthRegistrySnapshot,
    now_unix_seconds: i64,
) -> std::result::Result<(), (&'static str, String, Vec<String>)> {
    if snapshot.schema_version != 1 {
        return Err((
            "MEJEPA_HEALTH_PROBE_TASK_HEALTH_INVALID",
            format!(
                "supervised task health schema version {} is unsupported",
                snapshot.schema_version
            ),
            Vec::new(),
        ));
    }
    let current_pid = std::process::id();
    if snapshot.pid != current_pid {
        return Err((
            "MEJEPA_HEALTH_PROBE_TASK_HEALTH_PID_MISMATCH",
            format!(
                "task health registry pid {} does not match current daemon pid {}",
                snapshot.pid, current_pid
            ),
            Vec::new(),
        ));
    }
    if snapshot.heartbeat_timeout_seconds == 0 {
        return Err((
            "MEJEPA_HEALTH_PROBE_TASK_HEALTH_INVALID",
            "task health heartbeat_timeout_seconds must be greater than zero".to_string(),
            Vec::new(),
        ));
    }

    let expected = supervised_task_names()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let missing = expected
        .iter()
        .filter(|task| !snapshot.tasks.contains_key(*task))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err((
            "MEJEPA_HEALTH_PROBE_TASK_HEALTH_MISSING_TASK",
            format!(
                "task health registry is missing tasks: {}",
                missing.join(",")
            ),
            missing,
        ));
    }
    let unknown = snapshot
        .tasks
        .keys()
        .filter(|task| !expected.iter().any(|expected| expected == *task))
        .cloned()
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        return Err((
            "MEJEPA_HEALTH_PROBE_TASK_HEALTH_UNKNOWN_TASK",
            format!(
                "task health registry includes unknown tasks: {}",
                unknown.join(",")
            ),
            unknown,
        ));
    }

    let invalid = expected
        .iter()
        .filter_map(|task| {
            let record = snapshot.tasks.get(task)?;
            if record.task != *task {
                Some(format!("{task}:task_field_mismatch:{}", record.task))
            } else if record.pid != current_pid {
                Some(format!("{task}:pid_mismatch:{}", record.pid))
            } else if record.last_heartbeat_unix_seconds < record.started_at_unix_seconds {
                Some(format!("{task}:heartbeat_before_start"))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if !invalid.is_empty() {
        return Err((
            "MEJEPA_HEALTH_PROBE_TASK_HEALTH_INVALID",
            format!(
                "task health registry has invalid task rows: {}",
                invalid.join(",")
            ),
            invalid,
        ));
    }

    let unhealthy = expected
        .iter()
        .filter_map(|task| {
            let record = snapshot.tasks.get(task)?;
            if record.status != "healthy" {
                Some(format!("{task}:{}", record.status))
            } else if record.heartbeat_count == 0 {
                Some(format!("{task}:no_heartbeat"))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if !unhealthy.is_empty() {
        return Err((
            "MEJEPA_HEALTH_PROBE_TASK_UNHEALTHY",
            format!(
                "one or more supervised tasks are not healthy: {}",
                unhealthy.join(",")
            ),
            unhealthy,
        ));
    }

    let timeout = snapshot.heartbeat_timeout_seconds as i64;
    let stale = expected
        .iter()
        .filter_map(|task| {
            let record = snapshot.tasks.get(task)?;
            let age = now_unix_seconds.saturating_sub(record.last_heartbeat_unix_seconds);
            if age > timeout {
                Some(format!("{task}:{age}s"))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if !stale.is_empty() {
        return Err((
            "MEJEPA_HEALTH_PROBE_TASK_HEARTBEAT_STALE",
            format!(
                "supervised task heartbeat exceeded {}s: {}",
                snapshot.heartbeat_timeout_seconds,
                stale.join(",")
            ),
            stale,
        ));
    }

    Ok(())
}

fn not_ready(
    files: (PathBuf, PathBuf),
    error_code: &str,
    message: String,
    restart_counts: BTreeMap<String, usize>,
    supervised_tasks: Vec<String>,
    task_health: BTreeMap<String, SupervisedTaskHealthRecord>,
    unhealthy_tasks: Vec<String>,
    generated_at_unix_seconds: i64,
) -> HealthReadinessReport {
    let (status_file, task_health_file) = files;
    HealthReadinessReport {
        ready: false,
        status: "not_ready".to_string(),
        error_code: Some(error_code.to_string()),
        message: Some(message),
        status_file,
        task_health_file,
        restart_counts,
        supervised_tasks,
        task_health,
        unhealthy_tasks,
        generated_at_unix_seconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_status(dir: &Path, status: &str, restart_counts: BTreeMap<String, usize>) -> PathBuf {
        std::fs::create_dir_all(dir).expect("create scheduler dir");
        let path = dir.join("flywheel_supervisor_status.json");
        let bytes = serde_json::to_vec_pretty(&json!({
            "status": status,
            "pid": std::process::id(),
            "supervised_tasks": supervised_task_names(),
            "restart_counts": restart_counts,
            "updated_at_unix_seconds": 1_778_000_000_i64,
        }))
        .expect("encode status");
        std::fs::write(&path, bytes).expect("write status");
        write_task_health(dir, healthy_task_health(1_778_000_000_i64));
        path
    }

    fn healthy_task_health(now: i64) -> BTreeMap<String, SupervisedTaskHealthRecord> {
        supervised_task_names()
            .into_iter()
            .map(|task| {
                (
                    task.to_string(),
                    SupervisedTaskHealthRecord {
                        task: task.to_string(),
                        status: "healthy".to_string(),
                        pid: std::process::id(),
                        generation: 1,
                        heartbeat_count: 1,
                        started_at_unix_seconds: now,
                        last_heartbeat_unix_seconds: chrono::Utc::now().timestamp(),
                        last_restart_unix_seconds: None,
                        restart_count_5m: 0,
                        last_exit_status: None,
                        last_error_code: None,
                        last_error: None,
                    },
                )
            })
            .collect()
    }

    fn write_task_health(dir: &Path, tasks: BTreeMap<String, SupervisedTaskHealthRecord>) {
        std::fs::create_dir_all(dir).expect("create scheduler dir");
        let path = supervised_task_health_path(dir);
        let bytes = serde_json::to_vec_pretty(&SupervisedTaskHealthRegistrySnapshot {
            schema_version: 1,
            pid: std::process::id(),
            updated_at_unix_seconds: chrono::Utc::now().timestamp(),
            heartbeat_timeout_seconds: 30,
            tasks,
        })
        .expect("encode task health");
        std::fs::write(path, bytes).expect("write task health");
    }

    #[test]
    fn readiness_accepts_running_supervisor_with_restart_counts_under_threshold() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_status(
            temp.path(),
            "running",
            BTreeMap::from([("self_optimization_scheduler".to_string(), 5)]),
        );
        let report = evaluate_readiness(temp.path());
        assert!(report.ready);
        assert_eq!(report.status, "ready");
    }

    #[test]
    fn readiness_rejects_scheduler_flapping() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_status(
            temp.path(),
            "running",
            BTreeMap::from([("self_optimization_scheduler".to_string(), 6)]),
        );
        let report = evaluate_readiness(temp.path());
        assert!(!report.ready);
        assert_eq!(
            report.error_code.as_deref(),
            Some("MEJEPA_HEALTH_PROBE_SUPERVISOR_FLAPPING")
        );
    }

    #[test]
    fn readiness_rejects_missing_status_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let report = evaluate_readiness(temp.path());
        assert!(!report.ready);
        assert_eq!(
            report.error_code.as_deref(),
            Some("MEJEPA_HEALTH_PROBE_SUPERVISOR_STATUS_MISSING")
        );
    }

    #[test]
    fn readiness_rejects_pid_mismatch() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path()).expect("create scheduler dir");
        let path = temp.path().join("flywheel_supervisor_status.json");
        let bytes = serde_json::to_vec_pretty(&json!({
            "status": "running",
            "pid": std::process::id().saturating_add(1),
            "supervised_tasks": supervised_task_names(),
            "restart_counts": {},
            "updated_at_unix_seconds": 1_778_000_000_i64,
        }))
        .expect("encode status");
        std::fs::write(path, bytes).expect("write status");
        write_task_health(temp.path(), healthy_task_health(1_778_000_000_i64));
        let report = evaluate_readiness(temp.path());
        assert!(!report.ready);
        assert_eq!(
            report.error_code.as_deref(),
            Some("MEJEPA_HEALTH_PROBE_SUPERVISOR_PID_MISMATCH")
        );
    }

    #[test]
    fn readiness_rejects_missing_task_health_registry() {
        let temp = tempfile::tempdir().expect("tempdir");
        let status_path = write_status(temp.path(), "running", BTreeMap::new());
        std::fs::remove_file(supervised_task_health_path(temp.path())).expect("remove registry");
        assert!(status_path.is_file());
        let report = evaluate_readiness(temp.path());
        assert!(!report.ready);
        assert_eq!(
            report.error_code.as_deref(),
            Some("MEJEPA_HEALTH_PROBE_TASK_HEALTH_MISSING")
        );
    }

    #[test]
    fn readiness_rejects_stale_shift_subscriber_heartbeat() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_status(temp.path(), "running", BTreeMap::new());
        let mut tasks = healthy_task_health(1_778_000_000_i64);
        let subscriber = tasks.get_mut("shift_subscriber").expect("subscriber");
        subscriber.last_heartbeat_unix_seconds = chrono::Utc::now().timestamp().saturating_sub(31);
        write_task_health(temp.path(), tasks);
        let report = evaluate_readiness(temp.path());
        assert!(!report.ready);
        assert_eq!(
            report.error_code.as_deref(),
            Some("MEJEPA_HEALTH_PROBE_TASK_HEARTBEAT_STALE")
        );
        assert!(report
            .unhealthy_tasks
            .iter()
            .any(|task| task.starts_with("shift_subscriber:")));
    }

    #[test]
    fn readiness_rejects_restarting_shift_subscriber() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_status(temp.path(), "running", BTreeMap::new());
        let mut tasks = healthy_task_health(1_778_000_000_i64);
        let subscriber = tasks.get_mut("shift_subscriber").expect("subscriber");
        subscriber.status = "restarting".to_string();
        subscriber.last_exit_status = Some("panic".to_string());
        subscriber.last_error_code = Some("MEJEPA_DAEMON_TASK_PANIC".to_string());
        subscriber.last_error = Some("synthetic subscriber panic".to_string());
        write_task_health(temp.path(), tasks);
        let report = evaluate_readiness(temp.path());
        assert!(!report.ready);
        assert_eq!(
            report.error_code.as_deref(),
            Some("MEJEPA_HEALTH_PROBE_TASK_UNHEALTHY")
        );
        assert!(report
            .unhealthy_tasks
            .iter()
            .any(|task| task == "shift_subscriber:restarting"));
    }
}
