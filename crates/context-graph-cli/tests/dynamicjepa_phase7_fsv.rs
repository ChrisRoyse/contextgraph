//! Phase 7 DynamicJEPA CLI full-state verification.
//!
//! This test uses the real CLI binary, real RocksDB databases, deterministic
//! synthetic domain-valid JSONL events, CUDA training, and real artifact files.
//! Success is verified by reading the persisted training/artifact source of
//! truth and recomputing hashes from disk.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn context_graph_bin() -> &'static str {
    env!("CARGO_BIN_EXE_context-graph")
}

fn run_cli(args: &[&str]) -> (bool, Value) {
    let output = Command::new(context_graph_bin())
        .args(args)
        .env("CONTEXT_GRAPH_ENV", "development")
        .output()
        .expect("execute context-graph binary");
    let stdout = String::from_utf8(output.stdout).expect("CLI stdout is UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("CLI stderr is UTF-8");
    assert!(
        !stdout.trim().is_empty(),
        "CLI produced empty stdout; stderr={stderr}"
    );
    let value: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("stdout must be one JSON object: {err}\nstdout={stdout}"));
    (output.status.success(), value)
}

fn run_ok(args: &[&str]) -> Value {
    let (success, value) = run_cli(args);
    assert!(
        success,
        "command expected success but failed:\n{}",
        serde_json::to_string_pretty(&value).expect("pretty JSON")
    );
    assert_eq!(value["status"], "ok");
    value
}

fn run_err(args: &[&str]) -> Value {
    let (success, value) = run_cli(args);
    assert!(
        !success,
        "command expected failure but succeeded:\n{}",
        serde_json::to_string_pretty(&value).expect("pretty JSON")
    );
    assert_eq!(value["status"], "error");
    value
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(
        &fs::read(path).unwrap_or_else(|err| panic!("read JSON {}: {err}", path.display())),
    )
    .unwrap_or_else(|err| panic!("parse JSON {}: {err}", path.display()))
}

fn metric_f64(value: &Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("metric {key} missing in {value:#}"))
}

fn assert_close(lhs: f64, rhs: f64, label: &str) {
    assert!(
        (lhs - rhs).abs() <= 1.0e-12,
        "{label} mismatch: lhs={lhs} rhs={rhs}"
    );
}

fn counts(value: &Value) -> &Value {
    value
        .get("counts")
        .or_else(|| value.get("after_counts"))
        .expect("counts or after_counts")
}

fn count(value: &Value, cf: &str) -> u64 {
    counts(value)[cf]
        .as_u64()
        .unwrap_or_else(|| panic!("count for {cf} missing in {value:#}"))
}

fn delta(before: &Value, after: &Value, cf: &str) -> i64 {
    count(after, cf) as i64 - count(before, cf) as i64
}

fn write_counter_fixture(temp: &Path) -> PathBuf {
    let mut lines = Vec::new();
    for counter in 0..60 {
        lines.push(format!(
            r#"{{"state":{{"counter":{counter}}},"action":{{"delta":1}},"outcome":{{"next_counter":{}}},"ts":{}}}"#,
            counter + 1,
            1_700_000_000_000i64 + counter as i64 * 1_000
        ));
    }
    let path = temp.join("counter_world_phase7_generated_60.jsonl");
    fs::write(&path, lines.join("\n") + "\n").expect("write counter fixture");
    path
}

fn write_cpu_config(root: &Path, temp: &Path) -> PathBuf {
    let config =
        fs::read_to_string(root.join("configs/dynamicjepa/verification/tiny_train_config.json"))
            .expect("read tiny config");
    let mut value: Value = serde_json::from_str(&config).expect("parse tiny config");
    value["schedule"]["device"] = json!("cpu");
    let path = temp.join("tiny_train_config_cpu_invalid.json");
    fs::write(
        &path,
        serde_json::to_vec(&value).expect("serialize invalid config"),
    )
    .expect("write invalid config");
    path
}

fn corrupt_model_file(artifact_root: &str) {
    let path = Path::new(artifact_root).join("model.safetensors");
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap_or_else(|err| panic!("open model for corruption {}: {err}", path.display()));
    file.seek(SeekFrom::Start(64))
        .expect("seek into model.safetensors");
    let mut byte = [0u8; 1];
    file.read_exact(&mut byte)
        .expect("read model byte before corruption");
    file.seek(SeekFrom::Start(64))
        .expect("seek back before corruption");
    file.write_all(&[byte[0] ^ 0xff])
        .expect("write corrupted model byte");
    file.flush().expect("flush corrupted model byte");
}

fn run_phase6_counter_data(db: &str, root: &Path, fixture: &str) -> (String, Vec<Value>) {
    let counter_domain = root
        .join("configs/dynamicjepa/domain_packs/counter_world.v1.toml")
        .display()
        .to_string();
    let mut evidence = Vec::new();
    evidence.push(json!({"step": "initial_counts", "output": run_ok(&[
        "dynamicjepa", "inspect-counts", "--db", db, "--allow-missing", "--json"
    ])}));
    evidence.push(json!({"step": "register_domain", "output": run_ok(&[
        "dynamicjepa", "register-domain", "--db", db, "--file", &counter_domain, "--json"
    ])}));
    evidence.push(json!({"step": "ingest_events", "output": run_ok(&[
        "dynamicjepa", "ingest-event", "--db", db, "--domain", "counter_world",
        "--adapter", "json_event", "--file", fixture, "--json"
    ])}));
    evidence.push(json!({"step": "materialize_panels", "output": run_ok(&[
        "dynamicjepa", "materialize-panel", "--db", db, "--all-pending",
        "--domain", "counter_world", "--json"
    ])}));
    evidence.push(json!({"step": "compile_trajectories", "output": run_ok(&[
        "dynamicjepa", "compile-trajectories", "--db", db, "--domain", "counter_world",
        "--policy", "by_domain_session", "--json"
    ])}));
    let train = run_ok(&[
        "dynamicjepa",
        "compile-dataset",
        "--db",
        db,
        "--domain",
        "counter_world",
        "--policy",
        "one_step",
        "--split",
        "train",
        "--json",
    ]);
    let dataset_id = train["created_ids"]["dataset_id"]
        .as_str()
        .expect("dataset id")
        .to_string();
    assert_eq!(train["summary"]["row_count"], 41);
    evidence.push(json!({"step": "compile_dataset_train", "output": train}));
    let val = run_ok(&[
        "dynamicjepa",
        "compile-dataset",
        "--db",
        db,
        "--domain",
        "counter_world",
        "--policy",
        "one_step",
        "--split",
        "val",
        "--json",
    ]);
    assert_eq!(val["created_ids"]["dataset_id"], dataset_id);
    assert_eq!(val["summary"]["row_count"], 12);
    evidence.push(json!({"step": "compile_dataset_val", "output": val}));
    let test = run_ok(&[
        "dynamicjepa",
        "compile-dataset",
        "--db",
        db,
        "--domain",
        "counter_world",
        "--policy",
        "one_step",
        "--split",
        "test",
        "--json",
    ]);
    assert_eq!(test["created_ids"]["dataset_id"], dataset_id);
    assert_eq!(test["summary"]["row_count"], 6);
    evidence.push(json!({"step": "compile_dataset_test", "output": test}));
    (dataset_id, evidence)
}

#[test]
fn dynamicjepa_phase7_cli_fsv_real_cuda_artifacts() {
    let root = repo_root();
    let evidence_dir = root.join("tmp/5090jepa_evidence/phase7");
    fs::create_dir_all(&evidence_dir).expect("create Phase 7 evidence dir");
    let run_root = evidence_dir.join("cli_phase7_fsv_runtime");
    if run_root.exists() {
        fs::remove_dir_all(&run_root).expect("remove previous Phase 7 runtime evidence");
    }
    fs::create_dir_all(&run_root).expect("create Phase 7 runtime evidence dir");

    let db_path = run_root.join("dynamicjepa_phase7_counter_rocksdb");
    let missing_db_path = run_root.join("dynamicjepa_phase7_missing_rocksdb");
    let artifact_root_path = run_root.join("dynamicjepa_phase7_artifacts");
    let bad_artifact_root_path = run_root.join("dynamicjepa_phase7_bad_artifacts");
    let db = db_path.display().to_string();
    let missing_db = missing_db_path.display().to_string();
    let artifact_root = artifact_root_path.display().to_string();
    let bad_artifact_root = bad_artifact_root_path.display().to_string();
    let config = root
        .join("configs/dynamicjepa/verification/tiny_train_config.json")
        .display()
        .to_string();
    let cpu_config = write_cpu_config(&root, &run_root).display().to_string();
    let fixture = write_counter_fixture(&run_root).display().to_string();

    let mut evidence = Vec::new();
    let (dataset_id, setup_evidence) = run_phase6_counter_data(&db, &root, &fixture);
    evidence.extend(setup_evidence);

    let before_train = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    let train = run_ok(&[
        "dynamicjepa",
        "train",
        "--db",
        &db,
        "--dataset-id",
        &dataset_id,
        "--config",
        &config,
        "--artifact-root",
        &artifact_root,
        "--json",
    ]);
    let after_train = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(delta(&before_train, &after_train, "dj_training_runs"), 1);
    assert_eq!(delta(&before_train, &after_train, "dj_model_artifacts"), 1);
    assert_eq!(count(&after_train, "dj_predictions"), 0);
    assert_eq!(
        train["decoded_records"]["training_run"]["status"],
        "Completed"
    );
    assert_eq!(train["decoded_records"]["artifact"]["status"], "Active");
    assert_eq!(train["summary"]["row_count_by_split"]["train"], 41);
    assert_eq!(train["summary"]["row_count_by_split"]["val"], 12);
    assert_eq!(train["summary"]["row_count_by_split"]["test"], 6);
    assert_eq!(
        train["summary"]["metrics"]["parameter_count_trainable"],
        180.0
    );
    assert!(
        train["summary"]["metrics"]["val_latent_mse"]
            .as_f64()
            .expect("val mse")
            <= 0.005
    );
    assert!(
        train["summary"]["metrics"]["val_latent_cosine"]
            .as_f64()
            .expect("val cosine")
            >= 0.99
    );
    assert!(
        train["summary"]["metrics"]["vicreg_variance_per_dim_min"]
            .as_f64()
            .expect("variance min")
            >= 0.5
    );
    assert!(
        train["summary"]["metrics"]["shuffled_target_cosine"]
            .as_f64()
            .expect("centered shuffled target cosine")
            <= 0.10
    );
    assert!(
        train["summary"]["metrics"]["vicreg_covariance_off_diag_rms"]
            .as_f64()
            .expect("covariance off diagonal rms")
            <= 0.75
    );
    let training_run_id = train["created_ids"]["training_run_id"]
        .as_str()
        .expect("training run id")
        .to_string();
    let artifact_id = train["created_ids"]["artifact_id"]
        .as_str()
        .expect("artifact id")
        .to_string();
    let artifact_root = train["summary"]["artifact_root"]
        .as_str()
        .expect("artifact root")
        .to_string();
    let artifact_files = train["artifact_hashes"]
        .as_array()
        .expect("artifact hashes");
    assert_eq!(artifact_files.len(), 5);
    assert!(artifact_files
        .iter()
        .all(|file| file["equal"].as_bool() == Some(true)));
    evidence.push(json!({"step": "happy_train", "before": before_train, "output": train, "after": after_train}));

    let get_training = run_ok(&[
        "dynamicjepa",
        "get-training-run",
        "--db",
        &db,
        "--id",
        &training_run_id,
        "--json",
    ]);
    assert_eq!(
        get_training["decoded_records"]["training_run"]["artifact_ids"][0],
        artifact_id
    );
    evidence.push(json!({"step": "happy_get_training_run", "output": get_training}));

    let get_artifact = run_ok(&[
        "dynamicjepa",
        "get-artifact",
        "--db",
        &db,
        "--id",
        &artifact_id,
        "--verify-files",
        "--json",
    ]);
    assert!(get_artifact["artifact_hashes"]
        .as_array()
        .expect("verified artifact hashes")
        .iter()
        .all(|file| file["equal"].as_bool() == Some(true)));
    evidence.push(json!({"step": "happy_get_artifact_verify", "output": get_artifact}));

    let artifact_metrics = read_json(&Path::new(&artifact_root).join("metrics.json"));
    let evaluation_report = read_json(&Path::new(&artifact_root).join("evaluation_report.json"));
    for key in [
        "val_latent_mse",
        "val_latent_cosine",
        "shuffled_target_cosine",
        "vicreg_variance_per_dim_min",
        "vicreg_covariance_off_diag_rms",
        "vicreg_covariance_loss_scale",
    ] {
        assert_close(
            metric_f64(&artifact_metrics, key),
            metric_f64(&train["summary"]["metrics"], key),
            key,
        );
    }
    assert_close(
        evaluation_report["shuffled_target_baseline"]["cosine"]
            .as_f64()
            .expect("evaluation shuffled target baseline"),
        metric_f64(&artifact_metrics, "shuffled_target_cosine"),
        "evaluation_report.shuffled_target_baseline.cosine",
    );
    assert_close(
        evaluation_report["vicreg_covariance_off_diag_rms"]
            .as_f64()
            .expect("evaluation covariance rms"),
        metric_f64(&artifact_metrics, "vicreg_covariance_off_diag_rms"),
        "evaluation_report.vicreg_covariance_off_diag_rms",
    );
    evidence.push(json!({
        "step": "happy_artifact_file_readback",
        "source_of_truth": {
            "metrics_json": Path::new(&artifact_root).join("metrics.json"),
            "evaluation_report_json": Path::new(&artifact_root).join("evaluation_report.json")
        },
        "metrics": artifact_metrics,
        "evaluation_report_excerpt": {
            "shuffled_target_baseline": evaluation_report["shuffled_target_baseline"],
            "vicreg_variance_per_dim_min": evaluation_report["vicreg_variance_per_dim_min"],
            "vicreg_covariance_off_diag_rms": evaluation_report["vicreg_covariance_off_diag_rms"],
            "collapse_diagnostics": evaluation_report["collapse_diagnostics"]
        }
    }));

    let missing_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &missing_db,
        "--allow-missing",
        "--json",
    ]);
    let missing_err = run_err(&[
        "dynamicjepa",
        "train",
        "--db",
        &missing_db,
        "--dataset-id",
        "00000000-0000-0000-0000-000000000123",
        "--config",
        &config,
        "--artifact-root",
        &bad_artifact_root,
        "--json",
    ]);
    let missing_after = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &missing_db,
        "--allow-missing",
        "--json",
    ]);
    assert_eq!(missing_err["error_code"], "SOURCE_OF_TRUTH_MISSING");
    assert_eq!(
        delta(&missing_before, &missing_after, "dj_training_runs"),
        0
    );
    assert_eq!(
        delta(&missing_before, &missing_after, "dj_model_artifacts"),
        0
    );
    assert_eq!(delta(&missing_before, &missing_after, "dj_audit_log"), 0);
    evidence.push(json!({"step": "edge_missing_dataset", "before": missing_before, "error": missing_err, "after": missing_after}));

    let cpu_before = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    let cpu_err = run_err(&[
        "dynamicjepa",
        "train",
        "--db",
        &db,
        "--dataset-id",
        &dataset_id,
        "--config",
        &cpu_config,
        "--artifact-root",
        &bad_artifact_root,
        "--json",
    ]);
    let cpu_after = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(cpu_err["error_code"], "TRAINING_FAILED");
    assert!(cpu_err["error_message"]
        .as_str()
        .expect("cpu error")
        .contains("unsupported training device"));
    assert_eq!(delta(&cpu_before, &cpu_after, "dj_training_runs"), 0);
    assert_eq!(delta(&cpu_before, &cpu_after, "dj_model_artifacts"), 0);
    assert_eq!(delta(&cpu_before, &cpu_after, "dj_audit_log"), 0);
    evidence.push(json!({"step": "edge_invalid_cpu_config", "before": cpu_before, "error": cpu_err, "after": cpu_after}));

    let corrupt_before = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    corrupt_model_file(&artifact_root);
    let corrupt_err = run_err(&[
        "dynamicjepa",
        "get-artifact",
        "--db",
        &db,
        "--id",
        &artifact_id,
        "--verify-files",
        "--json",
    ]);
    let corrupt_after = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(corrupt_err["error_code"], "ARTIFACT_HASH_MISMATCH");
    assert_eq!(corrupt_err["wrote_prediction"], false);
    assert_eq!(count(&corrupt_before, "dj_predictions"), 0);
    assert_eq!(count(&corrupt_after, "dj_predictions"), 0);
    assert_eq!(
        delta(&corrupt_before, &corrupt_after, "dj_training_runs"),
        0
    );
    assert_eq!(
        delta(&corrupt_before, &corrupt_after, "dj_model_artifacts"),
        0
    );
    evidence.push(json!({"step": "edge_corrupt_artifact", "before": corrupt_before, "error": corrupt_err, "after": corrupt_after}));

    fs::write(
        evidence_dir.join("cli_phase7_fsv.json"),
        serde_json::to_vec_pretty(&json!({
            "source_of_truth": {
                "physical_dbs": {
                    "happy_path": db_path,
                    "missing_dataset_edge": missing_db_path
                },
                "artifact_root": artifact_root,
                "dynamicjepa_cfs": [
                    "dj_dataset_shards",
                    "dj_training_runs",
                    "dj_model_artifacts",
                    "dj_predictions",
                    "dj_audit_log"
                ],
                "read_paths": [
                    "dynamicjepa inspect-counts",
                    "dynamicjepa get-training-run",
                    "dynamicjepa get-artifact --verify-files",
                    "artifact_root/metrics.json",
                    "artifact_root/evaluation_report.json"
                ]
            },
            "steps": evidence
        }))
        .expect("serialize evidence"),
    )
    .expect("write Phase 7 evidence log");
}
