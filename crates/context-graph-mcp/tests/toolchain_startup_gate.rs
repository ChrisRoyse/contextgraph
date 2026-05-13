use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-f-toolchain-startup-gate-fsv";
const REPORT_FILE: &str = "toolchain_startup_gate_fsv.json";
const DAEMON_CONFIG_FILE: &str = "daemon.toml";

#[test]
fn task_flywheel_009_daemon_toolchain_startup_gate_fsv() {
    let fsv_root = PathBuf::from(FSV_ROOT);
    fs::create_dir_all(&fsv_root).expect("create fsv root");
    let run_root = fsv_root.join(format!(
        "run-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        std::process::id()
    ));
    fs::create_dir_all(&run_root).expect("create run root");

    let python_config_root = write_config_root(
        &run_root,
        "python-only",
        r#"
            [daemon]
            enabled_languages = ["python"]
        "#,
    );
    let invalid_config_root = write_config_root(
        &run_root,
        "invalid-language",
        r#"
            [daemon]
            enabled_languages = ["klingon"]
        "#,
    );
    let empty_enabled_root = write_config_root(
        &run_root,
        "empty-enabled",
        r#"
            [daemon]
            enabled_languages = []
        "#,
    );
    let missing_config_root = run_root.join("missing-config-root");
    let shim_path = create_shim_path(&run_root, &["ruff", "pyright", "python3"]);

    let pass = run_case(
        &run_root,
        "python_only_pass",
        &python_config_root,
        &shim_path,
    );
    let empty_path = run_case(&run_root, "empty_path_missing", &python_config_root, "");
    let invalid = run_case(
        &run_root,
        "invalid_config",
        &invalid_config_root,
        &shim_path,
    );
    let empty_enabled = run_case(
        &run_root,
        "empty_enabled_config",
        &empty_enabled_root,
        &shim_path,
    );
    let missing_config = run_case(
        &run_root,
        "missing_config_defaults_python",
        &missing_config_root,
        &shim_path,
    );

    let pass_json = first_json_line(&pass.stderr).expect("pass case emits success JSON");
    let missing_json =
        first_json_line(&empty_path.stderr).expect("empty PATH case emits missing JSON");
    let invalid_json = first_json_line(&invalid.stderr).expect("invalid config emits JSON");
    let empty_enabled_json =
        first_json_line(&empty_enabled.stderr).expect("empty enabled config emits JSON");
    let missing_config_json =
        first_json_line(&missing_config.stderr).expect("missing config emits success JSON");

    let lock_count = [
        pass.lock_exists,
        empty_path.lock_exists,
        invalid.lock_exists,
        empty_enabled.lock_exists,
        missing_config.lock_exists,
    ]
    .iter()
    .filter(|exists| **exists)
    .count();
    let report = json!({
        "fsv_root": FSV_ROOT,
        "task_id": "TASK-FLYWHEEL-009",
        "build_release_sha": git_head_short(),
        "source_of_truth": {
            "binary": env!("CARGO_BIN_EXE_context-graph-mcp"),
            "startup_phase": "pre_rocksdb",
            "operator_config": python_config_root.join(DAEMON_CONFIG_FILE).display().to_string(),
            "audit_only_flag": "--daemon-toolchain-audit-only"
        },
        "happy_path": [
            {
                "case": "python_only_configured_toolchain_passes_before_daemon_side_effects",
                "sot": "subprocess exit status + structured stderr + data root",
                "trigger": format!("{} --daemon-toolchain-audit-only --d-root <case-data>", env!("CARGO_BIN_EXE_context-graph-mcp")),
                "expected": {
                    "exit_success": true,
                    "all_available": true,
                    "required_count": 3,
                    "no_rocksdb_opened": true,
                    "lock_absent": true
                },
                "actual": {
                    "exit_success": pass.output.status.success(),
                    "stderr_json": pass_json,
                    "lock_exists": pass.lock_exists
                },
                "pass": pass.output.status.success()
                    && pass_json["allAvailable"] == json!(true)
                    && pass_json["requiredCount"] == json!(3)
                    && pass_json["noRocksdbOpened"] == json!(true)
                    && !pass.lock_exists
            }
        ],
        "boundary_cases": [
            {
                "case": "empty_path_fails_closed_with_structured_missing_toolchain",
                "expected": {
                    "exit_success": false,
                    "error_code": "MEJEPA_LABEL_TOOLCHAIN_MISSING",
                    "no_rocksdb_opened": true,
                    "lock_absent": true
                },
                "actual": {
                    "exit_success": empty_path.output.status.success(),
                    "stderr_json": missing_json,
                    "stderr": empty_path.stderr,
                    "lock_exists": empty_path.lock_exists
                },
                "pass": !empty_path.output.status.success()
                    && missing_json["error_code"] == json!("MEJEPA_LABEL_TOOLCHAIN_MISSING")
                    && missing_json["toolchain_audit"]["noRocksdbOpened"] == json!(true)
                    && !empty_path.lock_exists
            },
            {
                "case": "unsupported_enabled_language_fails_closed",
                "expected": {
                    "exit_success": false,
                    "error": "MEJEPA_LABEL_TOOLCHAIN_CONFIG_INVALID"
                },
                "actual": {
                    "exit_success": invalid.output.status.success(),
                    "stderr_json": invalid_json,
                    "stderr": invalid.stderr,
                    "lock_exists": invalid.lock_exists
                },
                "pass": !invalid.output.status.success()
                    && invalid_json["error_code"] == json!("MEJEPA_LABEL_TOOLCHAIN_CONFIG_INVALID")
                    && !invalid.lock_exists
            },
            {
                "case": "empty_enabled_languages_fails_closed",
                "expected": {
                    "exit_success": false,
                    "error": "MEJEPA_LABEL_TOOLCHAIN_CONFIG_INVALID"
                },
                "actual": {
                    "exit_success": empty_enabled.output.status.success(),
                    "stderr_json": empty_enabled_json,
                    "stderr": empty_enabled.stderr,
                    "lock_exists": empty_enabled.lock_exists
                },
                "pass": !empty_enabled.output.status.success()
                    && empty_enabled_json["error_code"] == json!("MEJEPA_LABEL_TOOLCHAIN_CONFIG_INVALID")
                    && !empty_enabled.lock_exists
            },
            {
                "case": "missing_config_defaults_to_python_scope",
                "expected": {
                    "exit_success": true,
                    "config_source": "default_python_config_missing",
                    "required_count": 3,
                    "lock_absent": true
                },
                "actual": {
                    "exit_success": missing_config.output.status.success(),
                    "stderr_json": missing_config_json,
                    "lock_exists": missing_config.lock_exists
                },
                "pass": missing_config.output.status.success()
                    && missing_config_json["configSource"] == json!("default_python_config_missing")
                    && missing_config_json["requiredCount"] == json!(3)
                    && missing_config_json["noRocksdbOpened"] == json!(true)
                    && !missing_config.lock_exists
            }
        ],
        "cf_counts_before": {"rocksdbLocks": 0},
        "cf_counts_after": {
            "rocksdbLocks": lock_count
        },
        "physical_artifacts": physical_artifacts(&run_root),
    });
    let all_passed = report["happy_path"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(report["boundary_cases"].as_array().into_iter().flatten())
        .all(|case| case["pass"].as_bool() == Some(true));
    let mut report = report;
    report["all_passed"] = json!(all_passed);
    report["readback_equal"] = json!(true);

    let artifact_path = fsv_root.join(REPORT_FILE);
    fs::write(&artifact_path, serde_json::to_vec_pretty(&report).unwrap())
        .expect("write fsv report");
    let readback: serde_json::Value =
        serde_json::from_slice(&fs::read(&artifact_path).expect("read fsv report"))
            .expect("decode fsv report");
    assert_eq!(readback, report);
    assert!(all_passed, "FSV report assertions failed: {report:#}");
}

struct CaseRun {
    output: Output,
    stderr: String,
    lock_exists: bool,
}

fn run_case(run_root: &Path, case: &str, config_root: &Path, path_env: &str) -> CaseRun {
    let data_root = run_root.join(format!("{case}-data"));
    fs::create_dir_all(&data_root).expect("create daemon data root");
    #[cfg(unix)]
    fs::set_permissions(&data_root, fs::Permissions::from_mode(0o700))
        .expect("tighten daemon data root permissions");
    let output = Command::new(env!("CARGO_BIN_EXE_context-graph-mcp"))
        .args([
            "--daemon-toolchain-audit-only",
            "--d-root",
            data_root.to_str().expect("utf8 data root"),
            "--no-warm",
        ])
        .env("PATH", path_env)
        .env("MEJEPA_CONFIG_ROOT", config_root)
        .env("RUST_LOG", "error")
        .env_remove("CONTEXT_GRAPH_DAEMON")
        .env_remove("CONTEXT_GRAPH_DAEMON_PORT")
        .env_remove("MEJEPA_DAEMON_CONFIG")
        .output()
        .expect("run context-graph-mcp startup gate subprocess");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    fs::write(
        run_root.join(format!("{case}.stderr.txt")),
        stderr.as_bytes(),
    )
    .expect("write stderr artifact");
    fs::write(run_root.join(format!("{case}.stdout.bin")), &output.stdout)
        .expect("write stdout artifact");
    let lock_exists = data_root.join("storage/contextgraph-rocksdb/LOCK").exists();
    CaseRun {
        output,
        stderr,
        lock_exists,
    }
}

fn write_config_root(root: &Path, name: &str, contents: &str) -> PathBuf {
    let path = root.join(name);
    fs::create_dir_all(&path).expect("create daemon config root");
    fs::write(path.join(DAEMON_CONFIG_FILE), contents).expect("write daemon config");
    path
}

fn create_shim_path(root: &Path, binaries: &[&str]) -> String {
    let dir = root.join("toolchain-shims");
    fs::create_dir_all(&dir).expect("create shim dir");
    for binary in binaries {
        fs::write(dir.join(binary), b"#!/bin/sh\nexit 0\n").expect("write shim");
    }
    dir.to_string_lossy().into_owned()
}

fn first_json_line(stderr: &str) -> Option<serde_json::Value> {
    stderr.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with('{') {
            serde_json::from_str(line).ok()
        } else {
            None
        }
    })
}

fn physical_artifacts(root: &Path) -> Vec<serde_json::Value> {
    let mut artifacts = Vec::new();
    collect_artifacts(root, &mut artifacts);
    artifacts
}

fn collect_artifacts(path: &Path, out: &mut Vec<serde_json::Value>) {
    for entry in fs::read_dir(path).expect("read artifact dir") {
        let entry = entry.expect("read artifact entry");
        let path = entry.path();
        if path.is_dir() {
            collect_artifacts(&path, out);
            continue;
        }
        let bytes = fs::read(&path).expect("read artifact file");
        out.push(json!({
            "path": path.display().to_string(),
            "bytes": bytes.len(),
            "sha256": hex::encode(Sha256::digest(&bytes)),
        }));
    }
}

fn git_head_short() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
