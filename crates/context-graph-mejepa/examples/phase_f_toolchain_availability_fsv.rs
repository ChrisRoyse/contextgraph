use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use context_graph_mejepa::toolchain_detect::{
    audit_required_toolchains, default_enabled_label_toolchains, enforce_required_toolchains,
    ToolchainAuditError, ToolchainBinary,
};
use context_graph_mejepa::Language;
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-f-toolchain-availability-fsv";
const REPORT_FILE: &str = "toolchain_availability_fsv.json";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let fsv_root = PathBuf::from(FSV_ROOT);
    fs::create_dir_all(&fsv_root)?;
    let run_root = fsv_root.join(format!("run-{}-{}", started_at_unix_ms, std::process::id()));
    fs::create_dir_all(&run_root)?;

    let tool_dir = run_root.join("fake-path-bin");
    fs::create_dir_all(&tool_dir)?;
    let default_required = default_enabled_label_toolchains();
    create_fake_binaries(&tool_dir, &default_required)?;
    let path_env = tool_dir.display().to_string();

    let cf_counts_before = json!({"toolchainAuditReportFiles": 0});
    let report = enforce_required_toolchains(&default_required, Some(&path_env))?;
    let audit_path = run_root.join("startup-toolchain-audit.json");
    write_json_readback(&audit_path, &report)?;
    let audit_readback: serde_json::Value = serde_json::from_slice(&fs::read(&audit_path)?)?;
    let expected_required = default_required.len();
    let happy_pass = report.all_available
        && report.no_partial_labels_persisted
        && report.required_count == expected_required
        && report.path_entries_checked == 1
        && report.resolved.len() == expected_required
        && audit_readback["allAvailable"] == json!(true);

    let boundary_cases = vec![
        missing_pyright_fails_closed(&run_root)?,
        empty_path_fails_closed()?,
        invalid_requirement_fails_closed()?,
        missing_after_resolved_tool_writes_no_partial_labels(&run_root)?,
    ];
    let all_boundaries_pass = boundary_cases
        .iter()
        .all(|case| case["pass"].as_bool().unwrap_or(false));

    let evidence_path = fsv_root.join(REPORT_FILE);
    let evidence = json!({
        "fsv_root": FSV_ROOT,
        "task_id": "TASK-PREDICT-LABEL-010",
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_head_short()?,
        "run_root": run_root,
        "happy_path": [{
            "case": "default_enabled_label_toolchains_available",
            "sot": audit_path,
            "before": cf_counts_before["toolchainAuditReportFiles"],
            "trigger": "cargo run --release -p context-graph-mejepa --example phase_f_toolchain_availability_fsv",
            "after": 1,
            "expected": {
                "all_available": true,
                "required_count": expected_required,
                "path_entries_checked": 1,
                "no_partial_labels_persisted": true
            },
            "actual": {
                "all_available": report.all_available,
                "required_count": report.required_count,
                "path_entries_checked": report.path_entries_checked,
                "resolved_count": report.resolved.len(),
                "no_partial_labels_persisted": report.no_partial_labels_persisted
            },
            "pass": happy_pass,
            "evidence_path": audit_path
        }],
        "boundary_cases": boundary_cases,
        "all_passed": happy_pass && all_boundaries_pass,
        "cf_counts_before": cf_counts_before,
        "cf_counts_after": {"toolchainAuditReportFiles": 1},
        "readback_equal": audit_readback["allAvailable"] == json!(true)
            && audit_readback["requiredCount"] == json!(expected_required)
            && audit_readback["pathEntriesChecked"] == json!(1),
        "physical_artifacts": {
            "startup_audit": artifact_metadata(&audit_path)?,
            "fsv_artifact": {
                "path": evidence_path.display().to_string(),
                "will_exist_after_write": true
            }
        }
    });

    if !evidence["all_passed"].as_bool().unwrap_or(false) {
        return Err("TASK-PREDICT-LABEL-010 FSV evidence did not pass".into());
    }
    write_json_readback(&evidence_path, &evidence)?;
    let final_readback: serde_json::Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    if final_readback["all_passed"] != json!(true)
        || final_readback["readback_equal"] != json!(true)
    {
        return Err("toolchain availability FSV artifact readback failed".into());
    }
    println!("{}", serde_json::to_string_pretty(&final_readback)?);
    Ok(())
}

fn missing_pyright_fails_closed(
    run_root: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let dir = run_root.join("missing-pyright-bin");
    fs::create_dir_all(&dir)?;
    create_fake_binary(&dir, "ruff")?;
    let required = vec![
        ToolchainBinary::new(
            "ruff",
            Language::Python,
            "python_static_analysis",
            "install ruff",
        ),
        ToolchainBinary::new(
            "pyright",
            Language::Python,
            "python_type_graph",
            "install pyright",
        ),
    ];
    let path_env = dir.display().to_string();
    let audit = audit_required_toolchains(&required, Some(&path_env))?;
    let err = enforce_required_toolchains(&required, Some(&path_env)).unwrap_err();
    let error_code = error_code(&err);
    Ok(json!({
        "case": "missing_required_pyright",
        "sot": "ToolchainAuditReport",
        "before": {"labelsPersisted": 0},
        "after": {"labelsPersisted": 0, "missing": audit.missing.len()},
        "expected": "MEJEPA_LABEL_TOOLCHAIN_MISSING",
        "actual": error_code,
        "pass": error_code == "MEJEPA_LABEL_TOOLCHAIN_MISSING"
            && !audit.all_available
            && !audit.no_partial_labels_persisted
            && audit.missing[0].path_entries_checked == 1
            && audit.missing[0].binary == "pyright"
    }))
}

fn empty_path_fails_closed() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let required = vec![ToolchainBinary::new(
        "pyright",
        Language::Python,
        "python_type_graph",
        "install pyright",
    )];
    let err = enforce_required_toolchains(&required, None).unwrap_err();
    let error_code = error_code(&err);
    Ok(json!({
        "case": "empty_path_env",
        "sot": "PATH",
        "expected": "MEJEPA_LABEL_TOOLCHAIN_MISSING",
        "actual": error_code,
        "pass": error_code == "MEJEPA_LABEL_TOOLCHAIN_MISSING"
    }))
}

fn invalid_requirement_fails_closed() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let required = vec![ToolchainBinary::new(
        "",
        Language::Python,
        "python_type_graph",
        "install pyright",
    )];
    let err = audit_required_toolchains(&required, Some("/usr/bin")).unwrap_err();
    let error_code = error_code(&err);
    Ok(json!({
        "case": "invalid_empty_binary",
        "sot": "enabled label-toolchain registry",
        "expected": "MEJEPA_LABEL_TOOLCHAIN_INVALID_REQUIREMENT",
        "actual": error_code,
        "pass": error_code == "MEJEPA_LABEL_TOOLCHAIN_INVALID_REQUIREMENT"
    }))
}

fn missing_after_resolved_tool_writes_no_partial_labels(
    run_root: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let dir = run_root.join("partial-path-bin");
    fs::create_dir_all(&dir)?;
    create_fake_binary(&dir, "ruff")?;
    let required = vec![
        ToolchainBinary::new(
            "ruff",
            Language::Python,
            "python_static_analysis",
            "install ruff",
        ),
        ToolchainBinary::new(
            "clippy-driver",
            Language::Rust,
            "rust_static_analysis",
            "install clippy",
        ),
    ];
    let path_env = dir.display().to_string();
    let audit = audit_required_toolchains(&required, Some(&path_env))?;
    Ok(json!({
        "case": "first_missing_after_one_resolved",
        "sot": "ToolchainAuditReport.noPartialLabelsPersisted",
        "expected": false,
        "actual": audit.no_partial_labels_persisted,
        "pass": audit.resolved.len() == 1
            && audit.missing.len() == 1
            && audit.path_entries_checked == 1
            && !audit.no_partial_labels_persisted
            && audit.missing[0].binary == "clippy-driver"
    }))
}

fn create_fake_binaries(
    dir: &Path,
    required: &[ToolchainBinary],
) -> Result<(), Box<dyn std::error::Error>> {
    for tool in required {
        create_fake_binary(dir, &tool.binary)?;
    }
    Ok(())
}

fn create_fake_binary(dir: &Path, binary: &str) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(dir.join(binary), b"#!/bin/sh\necho ok\n")?;
    Ok(())
}

fn error_code(err: &ToolchainAuditError) -> &'static str {
    match err {
        ToolchainAuditError::InvalidRequirement { .. } => {
            "MEJEPA_LABEL_TOOLCHAIN_INVALID_REQUIREMENT"
        }
        ToolchainAuditError::Missing(_) => "MEJEPA_LABEL_TOOLCHAIN_MISSING",
    }
}

fn write_json_readback<T: serde::Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let readback = fs::read(path)?;
    if readback != bytes {
        return Err(format!("readback mismatch for {}", path.display()).into());
    }
    Ok(())
}

fn artifact_metadata(path: &Path) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    Ok(json!({
        "path": path,
        "bytes": bytes.len(),
        "sha256": hex::encode(Sha256::digest(&bytes))
    }))
}

fn git_head_short() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err("git rev-parse --short HEAD failed".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
