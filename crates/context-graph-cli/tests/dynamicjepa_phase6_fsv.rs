//! Phase 6 DynamicJEPA CLI full-state verification.
//!
//! This test uses the real CLI binary, real RocksDB databases, and deterministic
//! synthetic fixture data through the actual adapter, panel, trajectory, and
//! dataset compiler paths. Dataset success is verified by reading
//! dj_dataset_shards and inspecting linked panel/action/state/outcome rows.

use std::fs;
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

fn write_first_gridworld_trajectory(root: &Path, temp: &Path) -> PathBuf {
    let corpus = fs::read_to_string(
        root.join("configs/dynamicjepa/verification/gridworld_5x5_corpus.jsonl"),
    )
    .expect("read gridworld corpus");
    let subset = corpus.lines().take(8).collect::<Vec<_>>().join("\n") + "\n";
    let path = temp.join("gridworld_first_trajectory_phase6.jsonl");
    fs::write(&path, subset).expect("write gridworld subset fixture");
    path
}

fn write_counter_insufficient_negatives(temp: &Path) -> PathBuf {
    let path = temp.join("counter_insufficient_negatives_phase6.jsonl");
    fs::write(
        &path,
        [
            r#"{"state":{"counter":0},"action":{"delta":1},"outcome":{"next_counter":1},"ts":1700000000000}"#,
            r#"{"state":{"counter":1},"action":{"delta":0},"outcome":{"next_counter":1},"ts":1700000001000}"#,
            r#"{"state":{"counter":2},"action":{"delta":-1},"outcome":{"next_counter":1},"ts":1700000002000}"#,
        ]
        .join("\n")
            + "\n",
    )
    .expect("write insufficient-negative fixture");
    path
}

fn run_phase5_happy(db: &str, domain_file: &str, domain: &str, fixture: &str) -> Value {
    run_ok(&[
        "dynamicjepa",
        "register-domain",
        "--db",
        db,
        "--file",
        domain_file,
        "--json",
    ]);
    run_ok(&[
        "dynamicjepa",
        "ingest-event",
        "--db",
        db,
        "--domain",
        domain,
        "--adapter",
        "json_event",
        "--file",
        fixture,
        "--json",
    ]);
    run_ok(&[
        "dynamicjepa",
        "materialize-panel",
        "--db",
        db,
        "--all-pending",
        "--domain",
        domain,
        "--json",
    ]);
    run_ok(&[
        "dynamicjepa",
        "compile-trajectories",
        "--db",
        db,
        "--domain",
        domain,
        "--policy",
        "by_domain_session",
        "--json",
    ])
}

fn compile_dataset(db: &str, domain: &str, split: &str) -> Value {
    run_ok(&[
        "dynamicjepa",
        "compile-dataset",
        "--db",
        db,
        "--domain",
        domain,
        "--policy",
        "one_step",
        "--split",
        split,
        "--json",
    ])
}

fn inspect_dataset(db: &str, dataset_id: &str, shard_id: &str, row: &str) -> Value {
    run_ok(&[
        "dynamicjepa",
        "inspect-dataset",
        "--db",
        db,
        "--dataset-id",
        dataset_id,
        "--shard-id",
        shard_id,
        "--row",
        row,
        "--json",
    ])
}

#[test]
fn dynamicjepa_phase6_cli_fsv_real_rocksdb() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let root = repo_root();
    let evidence_dir = root.join("tmp/5090jepa_evidence/phase6");
    fs::create_dir_all(&evidence_dir).expect("create Phase 6 evidence dir");

    let counter_db_path = temp.path().join("dynamicjepa_phase6_counter_rocksdb");
    let no_traj_db_path = temp.path().join("dynamicjepa_phase6_no_traj_rocksdb");
    let bad_negative_db_path = temp.path().join("dynamicjepa_phase6_bad_negative_rocksdb");
    let grid_db_path = temp.path().join("dynamicjepa_phase6_grid_rocksdb");
    let counter_db = counter_db_path.display().to_string();
    let no_traj_db = no_traj_db_path.display().to_string();
    let bad_negative_db = bad_negative_db_path.display().to_string();
    let grid_db = grid_db_path.display().to_string();

    let counter_domain = root
        .join("configs/dynamicjepa/domain_packs/counter_world.v1.toml")
        .display()
        .to_string();
    let counter_happy = root
        .join("configs/dynamicjepa/verification/counter_world_happy_path.jsonl")
        .display()
        .to_string();
    let grid_domain = root
        .join("configs/dynamicjepa/domain_packs/gridworld_5x5.v1.toml")
        .display()
        .to_string();
    let grid_happy = write_first_gridworld_trajectory(&root, temp.path())
        .display()
        .to_string();
    let bad_negative_fixture = write_counter_insufficient_negatives(temp.path())
        .display()
        .to_string();

    let mut evidence = Vec::new();

    let counter_initial = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    assert_eq!(count(&counter_initial, "dj_dataset_shards"), 0);
    evidence.push(json!({"step": "counter_initial_counts", "output": counter_initial}));

    let counter_phase5 = run_phase5_happy(
        &counter_db,
        &counter_domain,
        "counter_world",
        &counter_happy,
    );
    assert_eq!(count(&counter_phase5, "dj_trajectories"), 1);
    assert_eq!(count(&counter_phase5, "dj_latent_panels"), 3);
    evidence.push(json!({"step": "counter_phase5_happy", "output": counter_phase5}));

    let train = compile_dataset(&counter_db, "counter_world", "train");
    assert_eq!(train["summary"]["row_count"], 1);
    assert_eq!(train["summary"]["all_split_row_counts"]["train"], 1);
    assert_eq!(train["summary"]["all_split_row_counts"]["val"], 1);
    assert_eq!(train["count_delta"]["dj_dataset_shards"], 1);
    assert_eq!(train["count_delta"]["dj_audit_log"], 1);
    let dataset_id = train["created_ids"]["dataset_id"]
        .as_str()
        .expect("dataset id")
        .to_string();
    let train_shard_id = train["created_ids"]["shard_id"]
        .as_str()
        .expect("train shard id")
        .to_string();
    evidence.push(json!({"step": "counter_compile_train", "output": train}));

    let train_get = run_ok(&[
        "dynamicjepa",
        "get-dataset-shard",
        "--db",
        &counter_db,
        "--dataset-id",
        &dataset_id,
        "--shard-id",
        &train_shard_id,
        "--json",
    ]);
    assert_eq!(
        train_get["decoded_records"]["dataset_shard"]["row_count"],
        1
    );
    evidence.push(json!({"step": "counter_get_train_shard", "output": train_get}));

    let train_row = inspect_dataset(&counter_db, &dataset_id, &train_shard_id, "0");
    assert_eq!(
        train_row["decoded_records"]["row"]["source_hash_checks"]["input_equal"],
        true
    );
    assert_eq!(
        train_row["decoded_records"]["row"]["source_hash_checks"]["target_equal"],
        true
    );
    assert_eq!(
        train_row["decoded_records"]["row"]["input_state_fields"]["counter"]["I64"],
        0
    );
    assert_eq!(
        train_row["decoded_records"]["row"]["target_outcome_fields"]["next_counter"]["I64"],
        3
    );
    evidence.push(json!({"step": "counter_inspect_train_row_0", "output": train_row}));

    let val = compile_dataset(&counter_db, "counter_world", "val");
    assert_eq!(val["summary"]["row_count"], 1);
    assert_eq!(val["created_ids"]["dataset_id"], dataset_id);
    assert_eq!(val["count_delta"]["dj_dataset_shards"], 1);
    let val_shard_id = val["created_ids"]["shard_id"]
        .as_str()
        .expect("val shard id")
        .to_string();
    evidence.push(json!({"step": "counter_compile_val", "output": val}));

    let val_row = inspect_dataset(&counter_db, &dataset_id, &val_shard_id, "0");
    assert_eq!(
        val_row["decoded_records"]["row"]["input_state_fields"]["counter"]["I64"],
        1
    );
    assert_eq!(
        val_row["decoded_records"]["row"]["target_outcome_fields"]["next_counter"]["I64"],
        4
    );
    evidence.push(json!({"step": "counter_inspect_val_row_0", "output": val_row}));

    let duplicate_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    let duplicate_err = run_err(&[
        "dynamicjepa",
        "compile-dataset",
        "--db",
        &counter_db,
        "--domain",
        "counter_world",
        "--policy",
        "one_step",
        "--split",
        "train",
        "--json",
    ]);
    let duplicate_after = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    assert_eq!(
        delta(&duplicate_before, &duplicate_after, "dj_dataset_shards"),
        0
    );
    assert_eq!(
        delta(&duplicate_before, &duplicate_after, "dj_audit_log"),
        0
    );
    evidence.push(json!({"step": "edge_duplicate_dataset_split", "before": duplicate_before, "error": duplicate_err, "after": duplicate_after}));

    let invalid_policy_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    let invalid_policy_err = run_err(&[
        "dynamicjepa",
        "compile-dataset",
        "--db",
        &counter_db,
        "--domain",
        "counter_world",
        "--policy",
        "n_step",
        "--split",
        "test",
        "--json",
    ]);
    let invalid_policy_after = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    assert_eq!(
        invalid_policy_err["field"], "compile-dataset.policy",
        "invalid policy must fail at the declared invalid field"
    );
    assert_eq!(
        delta(
            &invalid_policy_before,
            &invalid_policy_after,
            "dj_dataset_shards"
        ),
        0
    );
    assert_eq!(
        delta(
            &invalid_policy_before,
            &invalid_policy_after,
            "dj_audit_log"
        ),
        0
    );
    evidence.push(json!({"step": "edge_invalid_dataset_policy", "before": invalid_policy_before, "error": invalid_policy_err, "after": invalid_policy_after}));

    run_ok(&[
        "dynamicjepa",
        "register-domain",
        "--db",
        &no_traj_db,
        "--file",
        &counter_domain,
        "--json",
    ]);
    let no_traj_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &no_traj_db,
        "--json",
    ]);
    let no_traj_err = run_err(&[
        "dynamicjepa",
        "compile-dataset",
        "--db",
        &no_traj_db,
        "--domain",
        "counter_world",
        "--policy",
        "one_step",
        "--split",
        "train",
        "--json",
    ]);
    let no_traj_after = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &no_traj_db,
        "--json",
    ]);
    assert_eq!(no_traj_err["field"], "compile-dataset.trajectories");
    assert_eq!(
        delta(&no_traj_before, &no_traj_after, "dj_dataset_shards"),
        0
    );
    assert_eq!(delta(&no_traj_before, &no_traj_after, "dj_audit_log"), 0);
    evidence.push(json!({"step": "edge_no_trajectories", "before": no_traj_before, "error": no_traj_err, "after": no_traj_after}));

    run_phase5_happy(
        &bad_negative_db,
        &counter_domain,
        "counter_world",
        &bad_negative_fixture,
    );
    let bad_negative_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &bad_negative_db,
        "--json",
    ]);
    let bad_negative_err = run_err(&[
        "dynamicjepa",
        "compile-dataset",
        "--db",
        &bad_negative_db,
        "--domain",
        "counter_world",
        "--policy",
        "one_step",
        "--split",
        "train",
        "--json",
    ]);
    let bad_negative_after = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &bad_negative_db,
        "--json",
    ]);
    assert_eq!(bad_negative_err["error_code"], "DATASET_LEAKAGE_DETECTED");
    assert!(bad_negative_err["error_message"]
        .as_str()
        .expect("error message")
        .contains("insufficient negatives"));
    assert_eq!(
        delta(
            &bad_negative_before,
            &bad_negative_after,
            "dj_dataset_shards"
        ),
        0
    );
    assert_eq!(
        delta(&bad_negative_before, &bad_negative_after, "dj_audit_log"),
        0
    );
    evidence.push(json!({"step": "edge_insufficient_negatives", "before": bad_negative_before, "error": bad_negative_err, "after": bad_negative_after}));

    let grid_phase5 = run_phase5_happy(&grid_db, &grid_domain, "gridworld_5x5", &grid_happy);
    assert_eq!(count(&grid_phase5, "dj_trajectories"), 1);
    assert_eq!(count(&grid_phase5, "dj_latent_panels"), 8);
    evidence.push(json!({"step": "grid_phase5_happy_one_trajectory", "output": grid_phase5}));

    let grid_train = compile_dataset(&grid_db, "gridworld_5x5", "train");
    assert_eq!(grid_train["summary"]["row_count"], 7);
    assert_eq!(grid_train["summary"]["input_dim"], 31);
    assert_eq!(grid_train["summary"]["target_dim"], 31);
    assert_eq!(grid_train["summary"]["action_dim"], 5);
    assert_eq!(grid_train["count_delta"]["dj_dataset_shards"], 1);
    let grid_dataset_id = grid_train["created_ids"]["dataset_id"]
        .as_str()
        .expect("grid dataset id")
        .to_string();
    let grid_shard_id = grid_train["created_ids"]["shard_id"]
        .as_str()
        .expect("grid shard id")
        .to_string();
    evidence.push(json!({"step": "grid_compile_train_one_trajectory", "output": grid_train}));

    let grid_row = inspect_dataset(&grid_db, &grid_dataset_id, &grid_shard_id, "0");
    assert_eq!(
        grid_row["decoded_records"]["row"]["source_hash_checks"]["input_equal"],
        true
    );
    assert_eq!(
        grid_row["decoded_records"]["row"]["source_hash_checks"]["target_equal"],
        true
    );
    assert_eq!(
        grid_row["decoded_records"]["row"]["input_action_fields"]["move"]["Categorical"]["variant"]
            .as_str()
            .is_some(),
        true
    );
    evidence.push(json!({"step": "grid_inspect_train_row_0", "output": grid_row}));

    let counter_final = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    let grid_final = run_ok(&["dynamicjepa", "inspect-counts", "--db", &grid_db, "--json"]);
    assert_eq!(count(&counter_final, "dj_dataset_shards"), 2);
    assert_eq!(count(&grid_final, "dj_dataset_shards"), 1);
    evidence.push(json!({"step": "counter_final_counts", "output": counter_final}));
    evidence.push(json!({"step": "grid_final_counts", "output": grid_final}));

    fs::write(
        evidence_dir.join("cli_phase6_fsv.json"),
        serde_json::to_vec_pretty(&json!({
            "source_of_truth": {
                "physical_dbs": {
                    "counter_world": counter_db_path,
                    "gridworld_5x5": grid_db_path,
                    "no_trajectories_edge": no_traj_db_path,
                    "insufficient_negatives_edge": bad_negative_db_path
                },
                "dynamicjepa_cfs": [
                    "dj_trajectories",
                    "dj_latent_panels",
                    "dj_actions",
                    "dj_outcomes",
                    "dj_dataset_shards",
                    "dj_audit_log"
                ],
                "read_paths": [
                    "dynamicjepa inspect-counts",
                    "dynamicjepa inspect-cf",
                    "dynamicjepa get-dataset-shard",
                    "dynamicjepa inspect-dataset"
                ]
            },
            "steps": evidence
        }))
        .expect("serialize evidence"),
    )
    .expect("write Phase 6 evidence log");
}
