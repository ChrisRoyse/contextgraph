use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use context_graph_mcp::daemon::{
    start_supervised_flywheel_tasks, supervised_task_health_path, supervised_task_names,
    FlywheelDaemonConfig, SupervisedTaskHealthRecord, SupervisedTaskHealthRegistrySnapshot,
};
use context_graph_mcp::daemon_validate::validate_daemon_root;
use context_graph_mcp::health_probe::{start_health_probe, HealthProbeConfig};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/phase-f-health-probe-fsv";

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> Result<()> {
    let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let run_root =
        PathBuf::from(FSV_ROOT).join(format!("run-{}-{}", started_at_unix_ms, std::process::id()));
    let scheduler_state_dir = run_root.join("state/schedulers");
    fs::create_dir_all(&scheduler_state_dir)?;
    let config = HealthProbeConfig::new(
        "127.0.0.1:0"
            .parse::<SocketAddr>()
            .context("parse health probe bind addr")?,
        scheduler_state_dir.clone(),
    );
    let probe = start_health_probe(config).await?;
    let status_path = scheduler_state_dir.join("flywheel_supervisor_status.json");
    let task_health_path = supervised_task_health_path(&scheduler_state_dir);

    let health = http_get(probe.local_addr, "/health")?;
    let health_pass = health.status == 200 && health.body["status"] == "ok";

    write_status(
        &status_path,
        "running",
        BTreeMap::from([
            ("shift_subscriber".to_string(), 0),
            ("self_optimization_scheduler".to_string(), 5),
            ("nightly_gc_scheduler".to_string(), 0),
            ("weekly_eval_scheduler".to_string(), 0),
        ]),
    )?;
    let ready = http_get(probe.local_addr, "/ready")?;
    let ready_pass = ready.status == 200 && ready.body["ready"] == true;

    let mut boundary_cases = Vec::new();
    let mut stale_tasks = healthy_task_health()?;
    let stale_now = chrono::Utc::now().timestamp();
    let stale_subscriber_row = stale_tasks
        .get_mut("shift_subscriber")
        .context("shift_subscriber task health row exists")?;
    stale_subscriber_row.started_at_unix_seconds = stale_now.saturating_sub(60);
    stale_subscriber_row.last_heartbeat_unix_seconds = stale_now.saturating_sub(31);
    write_task_health(&task_health_path, stale_tasks)?;
    let stale_subscriber = http_get(probe.local_addr, "/ready")?;
    boundary_cases.push(json!({
        "case": "shift_subscriber_stale_heartbeat_returns_503",
        "before": {"task": "shift_subscriber", "last_heartbeat_age_seconds": 31},
        "trigger": "GET /ready",
        "expected": "HTTP 503 and MEJEPA_HEALTH_PROBE_TASK_HEARTBEAT_STALE",
        "actual": {"status": stale_subscriber.status, "body": stale_subscriber.body},
        "pass": stale_subscriber.status == 503
            && stale_subscriber.body["errorCode"] == "MEJEPA_HEALTH_PROBE_TASK_HEARTBEAT_STALE",
    }));

    write_status(
        &status_path,
        "running",
        BTreeMap::from([
            ("shift_subscriber".to_string(), 0),
            ("self_optimization_scheduler".to_string(), 5),
            ("nightly_gc_scheduler".to_string(), 0),
            ("weekly_eval_scheduler".to_string(), 0),
        ]),
    )?;
    let mut restarting_tasks = healthy_task_health()?;
    let subscriber = restarting_tasks
        .get_mut("shift_subscriber")
        .context("shift_subscriber task health row exists")?;
    subscriber.status = "restarting".to_string();
    subscriber.last_exit_status = Some("panic".to_string());
    subscriber.last_error_code = Some("MEJEPA_DAEMON_TASK_PANIC".to_string());
    subscriber.last_error = Some("synthetic subscriber panic".to_string());
    write_task_health(&task_health_path, restarting_tasks)?;
    let restarting_subscriber = http_get(probe.local_addr, "/ready")?;
    boundary_cases.push(json!({
        "case": "shift_subscriber_restarting_returns_503",
        "before": {"task": "shift_subscriber", "status": "restarting"},
        "trigger": "GET /ready",
        "expected": "HTTP 503 and MEJEPA_HEALTH_PROBE_TASK_UNHEALTHY",
        "actual": {"status": restarting_subscriber.status, "body": restarting_subscriber.body},
        "pass": restarting_subscriber.status == 503
            && restarting_subscriber.body["errorCode"] == "MEJEPA_HEALTH_PROBE_TASK_UNHEALTHY",
    }));

    write_status(
        &status_path,
        "running",
        BTreeMap::from([("self_optimization_scheduler".to_string(), 6)]),
    )?;
    let flapping = http_get(probe.local_addr, "/ready")?;
    boundary_cases.push(json!({
        "case": "scheduler_flapping_returns_503",
        "before": {"restart_count": 6},
        "trigger": "GET /ready",
        "expected": "HTTP 503 and MEJEPA_HEALTH_PROBE_SUPERVISOR_FLAPPING",
        "actual": {"status": flapping.status, "body": flapping.body},
        "pass": flapping.status == 503
            && flapping.body["errorCode"] == "MEJEPA_HEALTH_PROBE_SUPERVISOR_FLAPPING",
    }));

    write_status(&status_path, "restarting", BTreeMap::new())?;
    let restarting = http_get(probe.local_addr, "/ready")?;
    boundary_cases.push(json!({
        "case": "supervisor_restarting_returns_503",
        "before": {"status": "restarting"},
        "trigger": "GET /ready",
        "expected": "HTTP 503 and MEJEPA_HEALTH_PROBE_SUPERVISOR_NOT_RUNNING",
        "actual": {"status": restarting.status, "body": restarting.body},
        "pass": restarting.status == 503
            && restarting.body["errorCode"] == "MEJEPA_HEALTH_PROBE_SUPERVISOR_NOT_RUNNING",
    }));

    write_status_with_pid(
        &status_path,
        "running",
        std::process::id().saturating_add(1),
        BTreeMap::new(),
    )?;
    let pid_mismatch = http_get(probe.local_addr, "/ready")?;
    boundary_cases.push(json!({
        "case": "supervisor_pid_mismatch_returns_503",
        "before": {"pid": "not current process"},
        "trigger": "GET /ready",
        "expected": "HTTP 503 and MEJEPA_HEALTH_PROBE_SUPERVISOR_PID_MISMATCH",
        "actual": {"status": pid_mismatch.status, "body": pid_mismatch.body},
        "pass": pid_mismatch.status == 503
            && pid_mismatch.body["errorCode"] == "MEJEPA_HEALTH_PROBE_SUPERVISOR_PID_MISMATCH",
    }));

    fs::remove_file(&status_path)?;
    let missing = http_get(probe.local_addr, "/ready")?;
    boundary_cases.push(json!({
        "case": "missing_supervisor_status_returns_503",
        "before": {"status_file_exists": false},
        "trigger": "GET /ready",
        "expected": "HTTP 503 and MEJEPA_HEALTH_PROBE_SUPERVISOR_STATUS_MISSING",
        "actual": {"status": missing.status, "body": missing.body},
        "pass": missing.status == 503
            && missing.body["errorCode"] == "MEJEPA_HEALTH_PROBE_SUPERVISOR_STATUS_MISSING",
    }));

    write_synced(&status_path, b"{not-valid-json")?;
    let malformed = http_get(probe.local_addr, "/ready")?;
    boundary_cases.push(json!({
        "case": "malformed_supervisor_status_returns_503",
        "before": {"status_file": "malformed json"},
        "trigger": "GET /ready",
        "expected": "HTTP 503 and MEJEPA_HEALTH_PROBE_SUPERVISOR_STATUS_INVALID",
        "actual": {"status": malformed.status, "body": malformed.body},
        "pass": malformed.status == 503
            && malformed.body["errorCode"] == "MEJEPA_HEALTH_PROBE_SUPERVISOR_STATUS_INVALID",
    }));

    let live_daemon_case = live_daemon_shift_subscriber_health_case(&run_root).await?;
    let all_boundary_passed = boundary_cases
        .iter()
        .all(|case| case["pass"].as_bool().unwrap_or(false));
    let live_daemon_passed = live_daemon_case["pass"].as_bool().unwrap_or(false);
    let artifact_path = PathBuf::from(FSV_ROOT).join("health_probe_fsv.json");
    let status_file_state = file_state(&status_path)?;
    let report = json!({
        "fsv_root": FSV_ROOT,
        "task_id": "TASK-OBS-014",
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_head_short(),
        "source_of_truth": {
            "status_file": status_path,
            "task_health_file": task_health_path,
            "http_probe": format!("http://{}", probe.local_addr),
        },
        "happy_path": [
            {
                "case": "health_process_alive",
                "sot": "process + HTTP listener",
                "before": null,
                "trigger": "GET /health",
                "after": health.body,
                "expected": "HTTP 200 status=ok",
                "actual": {"status": health.status, "body": health.body},
                "pass": health_pass,
            },
            {
                "case": "ready_running_supervisor",
                "sot": status_path,
                "before": "running supervisor status file with restart counts <= 5",
                "trigger": "GET /ready",
                "after": ready.body,
                "expected": "HTTP 200 ready=true",
                "actual": {"status": ready.status, "body": ready.body},
                "pass": ready_pass,
            }
        ],
        "boundary_cases": boundary_cases,
        "live_daemon_case": live_daemon_case,
        "physical_artifacts": {
            "run_root": run_root,
            "status_file": status_file_state,
            "task_health_file": file_state(&task_health_path)?,
        },
        "all_passed": health_pass && ready_pass && all_boundary_passed && live_daemon_passed,
        "readback_equal": true,
    });
    write_json_artifact(&artifact_path, &report)?;
    let artifact_sha256 = sha256_file(&artifact_path)?;
    probe.shutdown().await?;

    if !report["all_passed"].as_bool().unwrap_or(false) {
        anyhow::bail!(
            "MEJEPA_HEALTH_PROBE_FSV_FAILED: {}",
            artifact_path.display()
        );
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "artifact_path": artifact_path,
            "artifact_sha256": artifact_sha256,
            "all_passed": true,
            "readback_equal": true,
        }))?
    );
    Ok(())
}

async fn live_daemon_shift_subscriber_health_case(run_root: &Path) -> Result<Value> {
    let daemon_root = run_root.join("live-daemon-root");
    fs::create_dir_all(&daemon_root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&daemon_root, fs::Permissions::from_mode(0o700))?;
    }
    let paths = validate_daemon_root(&daemon_root)?;
    let repo_root = daemon_root.join("repo");
    fs::create_dir_all(&repo_root)?;
    let db = context_graph_mejepa_hygiene::open_hygiene_rocksdb(&paths.storage_db)?;
    let config = FlywheelDaemonConfig::from_paths(paths.clone(), repo_root);
    let mut daemon = start_supervised_flywheel_tasks(config, db.clone())?;
    let probe = start_health_probe(HealthProbeConfig::new(
        "127.0.0.1:0".parse::<SocketAddr>()?,
        paths.scheduler_state_dir.clone(),
    ))
    .await?;

    let ready_before =
        wait_for_ready_state(probe.local_addr, true, Duration::from_secs(20)).await?;
    fs::remove_dir_all(&paths.shift_log_dir)?;
    let not_ready_after_subscriber_kill =
        wait_for_ready_state(probe.local_addr, false, Duration::from_secs(30)).await?;
    fs::create_dir_all(&paths.shift_log_dir)?;
    let ready_after_recovery =
        wait_for_ready_state(probe.local_addr, true, Duration::from_secs(30)).await?;

    let status_file = paths
        .scheduler_state_dir
        .join("flywheel_supervisor_status.json");
    let task_health_file = supervised_task_health_path(&paths.scheduler_state_dir);
    let status_file_state = file_state(&status_file)?;
    let task_health_file_state = file_state(&task_health_file)?;
    probe.shutdown().await?;
    daemon.shutdown().await?;
    drop(db);

    let killed_error = not_ready_after_subscriber_kill.body["errorCode"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let pass = ready_before.status == 200
        && ready_before.body["ready"] == true
        && not_ready_after_subscriber_kill.status == 503
        && matches!(
            killed_error.as_str(),
            "MEJEPA_HEALTH_PROBE_TASK_UNHEALTHY"
                | "MEJEPA_HEALTH_PROBE_SUPERVISOR_NOT_RUNNING"
                | "MEJEPA_HEALTH_PROBE_SUPERVISOR_FLAPPING"
        )
        && ready_after_recovery.status == 200
        && ready_after_recovery.body["ready"] == true;

    Ok(json!({
        "case": "live_daemon_shift_subscriber_removed_then_recovers",
        "sot": {
            "supervisor_status_file": status_file,
            "task_health_file": task_health_file,
            "shift_log_dir": paths.shift_log_dir,
        },
        "before": {"ready": ready_before.body, "status": ready_before.status},
        "trigger": "remove runtime/cgreality-shift-log while the real supervised shift_subscriber task is running, then recreate it",
        "during": {
            "status": not_ready_after_subscriber_kill.status,
            "body": not_ready_after_subscriber_kill.body,
        },
        "after": {"ready": ready_after_recovery.body, "status": ready_after_recovery.status},
        "physical_artifacts": {
            "status_file": status_file_state,
            "task_health_file": task_health_file_state,
        },
        "expected": "ready 200 before, 503 after subscriber task failure, 200 after heartbeat recovery",
        "pass": pass,
    }))
}

async fn wait_for_ready_state(
    addr: SocketAddr,
    expected_ready: bool,
    timeout: Duration,
) -> Result<HttpResponse> {
    let started = Instant::now();
    let expected_status = if expected_ready { 200 } else { 503 };
    let mut last_error: Option<String>;
    loop {
        match http_get(addr, "/ready") {
            Ok(response)
                if response.status == expected_status
                    && response.body["ready"].as_bool() == Some(expected_ready) =>
            {
                return Ok(response);
            }
            Ok(response) => {
                last_error = Some(format!(
                    "status={} body={}",
                    response.status,
                    serde_json::to_string(&response.body)?
                ));
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }
        if started.elapsed() >= timeout {
            anyhow::bail!(
                "MEJEPA_HEALTH_PROBE_READY_WAIT_TIMEOUT: expected_ready={expected_ready} timeout={timeout:?} last={}",
                last_error.unwrap_or_else(|| "no response".to_string())
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

struct HttpResponse {
    status: u16,
    body: Value,
}

fn http_get(addr: SocketAddr, path: &str) -> Result<HttpResponse> {
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))
        .with_context(|| format!("connect to {addr}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes())?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw)?;
    let status = raw
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("MEJEPA_HEALTH_PROBE_HTTP_STATUS_MISSING: {raw}"))?
        .parse::<u16>()?;
    let body_raw = raw
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .ok_or_else(|| anyhow::anyhow!("MEJEPA_HEALTH_PROBE_HTTP_BODY_MISSING: {raw}"))?;
    let body = serde_json::from_str(body_raw)?;
    Ok(HttpResponse { status, body })
}

fn write_status(path: &Path, status: &str, restart_counts: BTreeMap<String, usize>) -> Result<()> {
    write_status_with_pid(path, status, std::process::id(), restart_counts)
}

fn write_status_with_pid(
    path: &Path,
    status: &str,
    pid: u32,
    restart_counts: BTreeMap<String, usize>,
) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(&json!({
        "status": status,
        "pid": pid,
        "supervised_tasks": supervised_task_names(),
        "restart_counts": restart_counts,
        "updated_at_unix_seconds": chrono::Utc::now().timestamp(),
    }))?;
    write_synced(path, &bytes)?;
    let health_path = path
        .parent()
        .map(supervised_task_health_path)
        .context("supervisor status path has parent")?;
    write_task_health(&health_path, healthy_task_health()?)
}

fn healthy_task_health() -> Result<BTreeMap<String, SupervisedTaskHealthRecord>> {
    let now = chrono::Utc::now().timestamp();
    Ok(supervised_task_names()
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
                    last_heartbeat_unix_seconds: now,
                    last_restart_unix_seconds: None,
                    restart_count_5m: 0,
                    last_exit_status: None,
                    last_error_code: None,
                    last_error: None,
                },
            )
        })
        .collect())
}

fn write_task_health(
    path: &Path,
    tasks: BTreeMap<String, SupervisedTaskHealthRecord>,
) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    let bytes = serde_json::to_vec_pretty(&SupervisedTaskHealthRegistrySnapshot {
        schema_version: 1,
        pid: std::process::id(),
        updated_at_unix_seconds: now,
        heartbeat_timeout_seconds: 30,
        tasks,
    })?;
    write_synced(path, &bytes)
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    let readback = fs::read(path)?;
    if readback != bytes {
        anyhow::bail!(
            "MEJEPA_HEALTH_PROBE_FILE_READBACK_MISMATCH: {}",
            path.display()
        );
    }
    Ok(())
}

fn write_json_artifact(path: &Path, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    write_synced(path, &bytes)
}

fn file_state(path: &Path) -> Result<Value> {
    let bytes = fs::read(path)?;
    Ok(json!({
        "path": path,
        "bytes": bytes.len(),
        "sha256": sha256_bytes(&bytes),
    }))
}

fn sha256_file(path: &Path) -> Result<String> {
    Ok(sha256_bytes(&fs::read(path)?))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn git_head_short() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}
