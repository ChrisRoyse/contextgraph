use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use context_graph_mcp::daemon::supervised_task_names;
use context_graph_mcp::health_probe::{start_health_probe, HealthProbeConfig};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-f-health-probe-fsv";

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

    let all_boundary_passed = boundary_cases
        .iter()
        .all(|case| case["pass"].as_bool().unwrap_or(false));
    let artifact_path = PathBuf::from(FSV_ROOT).join("health_probe_fsv.json");
    let status_file_state = file_state(&status_path)?;
    let report = json!({
        "fsv_root": FSV_ROOT,
        "task_id": "TASK-OBS-014",
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_head_short(),
        "source_of_truth": {
            "status_file": status_path,
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
        "physical_artifacts": {
            "run_root": run_root,
            "status_file": status_file_state,
        },
        "all_passed": health_pass && ready_pass && all_boundary_passed,
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
    let bytes = serde_json::to_vec_pretty(&json!({
        "status": status,
        "pid": std::process::id(),
        "supervised_tasks": supervised_task_names(),
        "restart_counts": restart_counts,
        "updated_at_unix_seconds": chrono::Utc::now().timestamp(),
    }))?;
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
