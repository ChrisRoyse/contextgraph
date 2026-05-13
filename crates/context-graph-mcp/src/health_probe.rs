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

use crate::daemon::supervised_task_names;
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
    pub restart_counts: BTreeMap<String, usize>,
    pub supervised_tasks: Vec<String>,
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
    let generated_at_unix_seconds = chrono::Utc::now().timestamp();
    let bytes = match std::fs::read(&status_file) {
        Ok(bytes) => bytes,
        Err(err) => {
            return not_ready(
                status_file,
                "MEJEPA_HEALTH_PROBE_SUPERVISOR_STATUS_MISSING",
                format!("supervisor status file is not readable: {err}"),
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
                status_file,
                "MEJEPA_HEALTH_PROBE_SUPERVISOR_STATUS_INVALID",
                format!("supervisor status file is not valid JSON: {err}"),
                BTreeMap::new(),
                Vec::new(),
                generated_at_unix_seconds,
            );
        }
    };

    let missing_tasks = supervised_task_names()
        .into_iter()
        .filter(|task| !snapshot.supervised_tasks.iter().any(|seen| seen == task))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if !missing_tasks.is_empty() {
        return not_ready(
            status_file,
            "MEJEPA_HEALTH_PROBE_SUPERVISED_TASK_MISSING",
            format!(
                "supervisor status is missing tasks: {}",
                missing_tasks.join(",")
            ),
            snapshot.restart_counts,
            snapshot.supervised_tasks,
            generated_at_unix_seconds,
        );
    }

    if snapshot.status != "running" {
        return not_ready(
            status_file,
            "MEJEPA_HEALTH_PROBE_SUPERVISOR_NOT_RUNNING",
            format!("supervisor status is {}", snapshot.status),
            snapshot.restart_counts,
            snapshot.supervised_tasks,
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
            status_file,
            "MEJEPA_HEALTH_PROBE_SUPERVISOR_FLAPPING",
            format!(
                "restart count exceeded {FLAPPING_THRESHOLD} in 5m window: {}",
                flapping.join(",")
            ),
            snapshot.restart_counts,
            snapshot.supervised_tasks,
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
        restart_counts: snapshot.restart_counts,
        supervised_tasks: snapshot.supervised_tasks,
        generated_at_unix_seconds,
    }
}

fn not_ready(
    status_file: PathBuf,
    error_code: &str,
    message: String,
    restart_counts: BTreeMap<String, usize>,
    supervised_tasks: Vec<String>,
    generated_at_unix_seconds: i64,
) -> HealthReadinessReport {
    HealthReadinessReport {
        ready: false,
        status: "not_ready".to_string(),
        error_code: Some(error_code.to_string()),
        message: Some(message),
        status_file,
        restart_counts,
        supervised_tasks,
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
            "pid": 12345,
            "supervised_tasks": supervised_task_names(),
            "restart_counts": restart_counts,
            "updated_at_unix_seconds": 1_778_000_000_i64,
        }))
        .expect("encode status");
        std::fs::write(&path, bytes).expect("write status");
        path
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
}
