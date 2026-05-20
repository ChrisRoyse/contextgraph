//! aiwonder GPU telemetry hardening FSV.
//!
//! The CUDA driver memory readback is authoritative for budget gates. The
//! `nvidia-smi` side channel is diagnostic only; budget decisions use direct
//! CUDA-driver memory readback.

use context_graph_mejepa_embedders::{
    nvidia_smi_status, nvidia_smi_total_mb_from_command, query_vram_budget, VramBudget,
};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

const FSV_DIR: &str = "/zfs/archive/contextgraph/fsv/contextgraph-mejepa-vram-aiwonder-fsv";
const EVIDENCE_PATH: &str =
    "/zfs/archive/contextgraph/fsv/contextgraph-mejepa-vram-aiwonder-fsv/vram-aiwonder-telemetry-evidence.json";
const SYSTEM_SPECS: &str = "/home/cabdru/contextgraph/docs2/aiwonder.md";

fn main() -> FsvResult<()> {
    let root = PathBuf::from(FSV_DIR);
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;

    let specs = fs::read_to_string(SYSTEM_SPECS)?;
    require("system specs contain RTX 5090", specs.contains("RTX 5090"))?;
    require("system specs contain CUDA 13.2", specs.contains("13.2"))?;

    let real_budget = query_vram_budget(VramBudget {
        required_bytes: 1 << 30,
        device_ordinal: 0,
    })?;
    require(
        "CUDA driver budget readback is authoritative",
        real_budget.telemetry_source == "cuda_driver_cuMemGetInfo_v2",
    )?;
    require(
        "real budget report has non-zero total bytes",
        real_budget.total_bytes > 0 && real_budget.free_bytes > 0,
    )?;

    let edge_cases = vec![
        edge_missing_command(&root)?,
        edge_unsupported_nvidia_smi_sentinel(&root)?,
        edge_malformed_output(&root)?,
        edge_valid_output(&root)?,
    ];
    let evidence = json!({
        "source_of_truth": {
            "system_specs": SYSTEM_SPECS,
            "evidence": EVIDENCE_PATH,
            "cuda_driver_report": "query_vram_budget(VramBudget { required_bytes: 1GiB, device_ordinal: 0 })",
            "nvidia_smi_probe": "nvidia_smi_total_mb_from_command"
        },
        "web_research": [
            {
                "source": "aiwonder hardware profile",
                "url": "docs2/aiwonder.md",
                "applied": "validate the live aiwonder RTX 5090 / CUDA 13.2 context without restoring the removed private hardware dump"
            },
            {
                "source": "NVIDIA CUDA 13.2 compute capabilities",
                "url": "https://docs.nvidia.com/cuda/archive/13.2.0/cuda-programming-guide/05-appendices/compute-capabilities.html",
                "applied": "validate compute-capability context through CUDA/driver telemetry"
            },
            {
                "source": "NVIDIA CUDA GPU compute capability table",
                "url": "https://developer.nvidia.com/cuda/gpus",
                "applied": "RTX 5090 is a compute capability 12.0 Blackwell GPU"
            }
        ],
        "happy_path": {
            "before": {
                "required_bytes": 1_u64 << 30,
                "device_ordinal": 0,
                "system_specs_mentions_rtx5090": true,
                "system_specs_mentions_cuda_13_2": true
            },
            "after": {
                "report": real_budget,
                "passes": true
            }
        },
        "edge_cases": edge_cases
    });
    write_json_0600(EVIDENCE_PATH, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(EVIDENCE_PATH)?)?;
    require(
        "evidence readback real budget total",
        readback["happy_path"]["after"]["report"]["total_bytes"].as_u64()
            == evidence["happy_path"]["after"]["report"]["total_bytes"].as_u64(),
    )?;
    require(
        "all edge cases have expected outcome",
        readback["edge_cases"]
            .as_array()
            .expect("edge array")
            .iter()
            .all(|case| case["observed_expected_outcome"] == true),
    )?;
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn edge_missing_command(root: &Path) -> FsvResult<Value> {
    let missing = root.join("missing-nvidia-smi");
    let before = json!({
        "command_path": missing,
        "command_exists": missing.exists()
    });
    let result = nvidia_smi_total_mb_from_command(&missing, 0);
    let status = nvidia_smi_status(&result);
    let after = json!({
        "result": result_json(&result),
        "status": status
    });
    Ok(json!({
        "name": "missing_nvidia_smi_command_is_diagnostic_only",
        "before": before,
        "after": after,
        "observed_expected_outcome": result.is_err() && status.starts_with("diagnostic_unavailable_not_authoritative")
    }))
}

fn edge_unsupported_nvidia_smi_sentinel(root: &Path) -> FsvResult<Value> {
    let command = write_probe_script(root, "nvidia-smi-unsupported", "N/A\n")?;
    let before = json!({
        "command_path": command,
        "stdout": "N/A"
    });
    let result = nvidia_smi_total_mb_from_command(&command, 0);
    let status = nvidia_smi_status(&result);
    let after = json!({
        "result": result_json(&result),
        "status": status
    });
    Ok(json!({
        "name": "unsupported_memory_total_is_non_authoritative",
        "before": before,
        "after": after,
        "observed_expected_outcome": result.as_ref().unwrap_err().contains("unsupported memory.total row")
            && status.starts_with("diagnostic_unavailable_not_authoritative")
    }))
}

fn edge_malformed_output(root: &Path) -> FsvResult<Value> {
    let command = write_probe_script(root, "nvidia-smi-malformed", "32607 MiB\n")?;
    let before = json!({
        "command_path": command,
        "stdout": "32607 MiB"
    });
    let result = nvidia_smi_total_mb_from_command(&command, 0);
    let after = json!({
        "result": result_json(&result),
        "status": nvidia_smi_status(&result)
    });
    Ok(json!({
        "name": "malformed_memory_total_fails_closed",
        "before": before,
        "after": after,
        "observed_expected_outcome": result.as_ref().unwrap_err().contains("failed to parse nvidia-smi memory.total row")
    }))
}

fn edge_valid_output(root: &Path) -> FsvResult<Value> {
    let command = write_probe_script(root, "nvidia-smi-valid", "32607\n")?;
    let before = json!({
        "command_path": command,
        "stdout": "32607"
    });
    let result = nvidia_smi_total_mb_from_command(&command, 0);
    let after = json!({
        "result": result_json(&result),
        "status": nvidia_smi_status(&result)
    });
    Ok(json!({
        "name": "valid_memory_total_is_recorded_as_diagnostic",
        "before": before,
        "after": after,
        "observed_expected_outcome": result == Ok(32607)
    }))
}

fn write_probe_script(root: &Path, name: &str, stdout: &str) -> FsvResult<PathBuf> {
    let path = root.join(name);
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o700)
        .open(&path)?;
    writeln!(file, "#!/usr/bin/env bash")?;
    writeln!(file, "printf '%b' {:?}", stdout)?;
    drop(file);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
    Ok(path)
}

fn result_json(result: &Result<u64, String>) -> Value {
    match result {
        Ok(value) => json!({"ok": true, "total_mb": value}),
        Err(message) => json!({"ok": false, "error": message}),
    }
}

fn write_json_0600(path: impl AsRef<Path>, value: &Value) -> FsvResult<()> {
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    let readback = fs::read(path)?;
    require("evidence byte readback", readback == bytes)?;
    require(
        "evidence permissions",
        fs::metadata(path)?.permissions().mode() & 0o777 == 0o600,
    )?;
    Ok(())
}

fn require(name: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("assertion failed: {name}").into())
    }
}
