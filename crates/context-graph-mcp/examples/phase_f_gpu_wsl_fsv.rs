use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use context_graph_mcp::telemetry::gpu_wsl::{default_query, nvidia_smi_query_with_command};
use context_graph_mejepa_embedders::{query_vram_budget, VramBudget};
use serde_json::{json, Value};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-f-gpu-wsl-telemetry-fsv";
const REPORT_FILE: &str = "gpu_wsl_telemetry_fsv.json";
const GIB: u64 = 1024 * 1024 * 1024;

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> FsvResult<()> {
    let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let fsv_root = PathBuf::from(FSV_ROOT);
    fs::create_dir_all(&fsv_root)?;
    let run_root = fsv_root.join(format!("run-{}-{}", started_at_unix_ms, std::process::id()));
    fs::create_dir_all(&run_root)?;
    fs::set_permissions(&run_root, fs::Permissions::from_mode(0o700))?;

    let valid_command = write_probe_script(
        &run_root,
        "nvidia-smi-valid",
        "printf 'NVIDIA RTX 5090, 32607, 30000, 2607\\n'\n",
    )?;
    let cuda_budget = query_vram_budget(VramBudget {
        required_bytes: GIB,
        device_ordinal: 0,
    })?;
    let diagnostic = nvidia_smi_query_with_command(&valid_command, default_query()).await?;
    let happy_pass = cuda_budget.total_bytes > 0
        && cuda_budget.free_bytes > 0
        && diagnostic.fields["memory.total"].value == Some(32607.0)
        && diagnostic.unavailable_fields.is_empty();

    let boundary_cases = vec![
        boundary_unsupported_wsl_sentinel(&run_root).await?,
        boundary_transient_retry(&run_root).await?,
        boundary_malformed_output(&run_root).await?,
        boundary_bad_numeric(&run_root).await?,
    ];
    let all_boundaries_pass = boundary_cases
        .iter()
        .all(|case| case["pass"].as_bool().unwrap_or(false));
    let script_artifacts = list_physical_files(&run_root)?;
    let evidence = json!({
        "fsv_root": FSV_ROOT,
        "task_id": "TASK-OBS-008",
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_head_short()?,
        "run_root": run_root,
        "happy_path": [{
            "case": "cuda_driver_authoritative_with_nvidia_smi_diagnostic",
            "sot": "CUDA driver cuMemGetInfo_v2 plus diagnostic command script readback",
            "before": {
                "required_bytes": GIB,
                "device_ordinal": 0,
                "diagnostic_command": valid_command
            },
            "trigger": "cargo run --release -p context-graph-mcp --example phase_f_gpu_wsl_fsv",
            "after": {
                "cuda_driver_budget": cuda_budget,
                "nvidia_smi_diagnostic": diagnostic,
                "nvidia_smi_authoritative": false
            },
            "expected": {
                "cuda_total_bytes_nonzero": true,
                "nvidia_smi_authoritative": false,
                "memory.total": 32607,
                "unavailable_fields": []
            },
            "actual": happy_pass,
            "pass": happy_pass,
            "evidence_path": run_root.join("nvidia-smi-valid")
        }],
        "boundary_cases": boundary_cases,
        "all_passed": happy_pass && all_boundaries_pass,
        "readback_equal": true,
        "physical_artifacts": {
            "scripts": script_artifacts
        }
    });
    if !evidence["all_passed"].as_bool().unwrap_or(false) {
        return Err("TASK-OBS-008 FSV evidence did not pass".into());
    }
    let evidence_path = fsv_root.join(REPORT_FILE);
    write_json_0600(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require(
        "evidence readback all_passed",
        readback["all_passed"].as_bool() == Some(true),
    )?;
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

async fn boundary_unsupported_wsl_sentinel(run_root: &Path) -> FsvResult<Value> {
    let command = write_probe_script(
        run_root,
        "nvidia-smi-wsl-na",
        "printf 'NVIDIA RTX 5090, N/A, [N/A], 10\\n'\n",
    )?;
    let diagnostic = nvidia_smi_query_with_command(&command, default_query()).await?;
    let pass = diagnostic.fields["memory.total"].unavailable
        && diagnostic.fields["memory.total"].value.is_none()
        && diagnostic.fields["memory.free"].unavailable
        && diagnostic
            .unavailable_fields
            .contains(&"memory.total".to_string());
    Ok(json!({
        "case": "nvidia_smi_na_reports_unavailable_true",
        "before": {"stdout": "NVIDIA RTX 5090, N/A, [N/A], 10", "command": command},
        "after": diagnostic,
        "expected": {"memory.total.unavailable": true, "memory.total.value": null},
        "actual": pass,
        "pass": pass
    }))
}

async fn boundary_transient_retry(run_root: &Path) -> FsvResult<Value> {
    let state = run_root.join("transient-count.txt");
    let body = format!(
        "state={:?}\n\
         count=0\n\
         if [ -f \"$state\" ]; then count=$(cat \"$state\"); fi\n\
         count=$((count + 1))\n\
         printf '%s' \"$count\" > \"$state\"\n\
         if [ \"$count\" -lt 3 ]; then sleep 3; fi\n\
         printf 'NVIDIA RTX 5090, 32607, 30000, 2607\\n'\n",
        state
    );
    let command = write_probe_script(run_root, "nvidia-smi-transient", &body)?;
    let diagnostic = nvidia_smi_query_with_command(&command, default_query()).await?;
    let pass = diagnostic.fields["memory.total"].value == Some(32607.0)
        && diagnostic.attempt_count == 3
        && diagnostic.unavailable_fields.is_empty();
    Ok(json!({
        "case": "transient_timeouts_are_retried",
        "before": {"script": command, "expected_timeouts_before_success": 2},
        "after": diagnostic,
        "expected": {"attempt_count": 3, "memory.total": 32607},
        "actual": pass,
        "pass": pass
    }))
}

async fn boundary_malformed_output(run_root: &Path) -> FsvResult<Value> {
    let command = write_probe_script(
        run_root,
        "nvidia-smi-malformed",
        "printf 'NVIDIA RTX 5090, 32607\\n'\n",
    )?;
    let err = nvidia_smi_query_with_command(&command, default_query())
        .await
        .unwrap_err();
    let pass = err.code == "MEJEPA_GPU_WSL_MALFORMED_OUTPUT";
    Ok(json!({
        "case": "malformed_column_count_fails_closed",
        "before": {"stdout": "NVIDIA RTX 5090, 32607", "command": command},
        "after": err,
        "expected": "MEJEPA_GPU_WSL_MALFORMED_OUTPUT",
        "actual": pass,
        "pass": pass
    }))
}

async fn boundary_bad_numeric(run_root: &Path) -> FsvResult<Value> {
    let command = write_probe_script(
        run_root,
        "nvidia-smi-bad-numeric",
        "printf 'NVIDIA RTX 5090, thirty-two, 30000, 2607\\n'\n",
    )?;
    let err = nvidia_smi_query_with_command(&command, default_query())
        .await
        .unwrap_err();
    let pass = err.code == "MEJEPA_GPU_WSL_INVALID_NUMERIC_FIELD";
    Ok(json!({
        "case": "bad_numeric_field_fails_closed",
        "before": {"stdout": "NVIDIA RTX 5090, thirty-two, 30000, 2607", "command": command},
        "after": err,
        "expected": "MEJEPA_GPU_WSL_INVALID_NUMERIC_FIELD",
        "actual": pass,
        "pass": pass
    }))
}

fn write_probe_script(root: &Path, name: &str, body: &str) -> FsvResult<PathBuf> {
    let path = root.join(name);
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o700)
        .open(&path)?;
    writeln!(file, "#!/usr/bin/env bash")?;
    write!(file, "{body}")?;
    file.sync_all()?;
    drop(file);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
    Ok(path)
}

fn write_json_0600(path: &Path, value: &Value) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    let readback = fs::read(path)?;
    require("evidence byte readback", readback == bytes)?;
    require(
        "evidence mode 0600",
        fs::metadata(path)?.permissions().mode() & 0o777 == 0o600,
    )
}

fn list_physical_files(root: &Path) -> FsvResult<Vec<Value>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            out.push(json!({
                "path": path,
                "bytes": fs::metadata(entry.path())?.len()
            }));
        }
    }
    out.sort_by_key(|value| value["path"].to_string());
    Ok(out)
}

fn git_head_short() -> FsvResult<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err("git rev-parse --short HEAD failed".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn require(name: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("assertion failed: {name}").into())
    }
}
