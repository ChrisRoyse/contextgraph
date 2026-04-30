//! Phase 5 DynamicJEPA CLI full-state verification.
//!
//! This test uses the real CLI binary, real RocksDB databases, and checked-in
//! deterministic fixture data. It verifies bindings and trajectories by reading
//! decoded source-of-truth rows from RocksDB after each command.

use std::collections::BTreeMap;
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

fn inspect_cf(db: &str, cf: &str, limit: &str) -> Value {
    run_ok(&[
        "dynamicjepa",
        "inspect-cf",
        "--db",
        db,
        "--cf",
        cf,
        "--limit",
        limit,
        "--json",
    ])
}

fn write_first_gridworld_trajectory(root: &Path, temp: &Path) -> PathBuf {
    let corpus = fs::read_to_string(
        root.join("configs/dynamicjepa/verification/gridworld_5x5_corpus.jsonl"),
    )
    .expect("read gridworld corpus");
    let subset = corpus.lines().take(8).collect::<Vec<_>>().join("\n") + "\n";
    let path = temp.join("gridworld_first_trajectory_phase5.jsonl");
    fs::write(&path, subset).expect("write gridworld subset fixture");
    path
}

fn run_phase4_happy(db: &str, domain_file: &str, domain: &str, fixture: &str) -> Value {
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
    ])
}

fn transition_timestamp_map(db: &str) -> BTreeMap<String, i64> {
    let rows = inspect_cf(db, "dj_transitions", "100");
    rows["decoded_records"]["rows"]
        .as_array()
        .expect("transition rows")
        .iter()
        .map(|row| {
            (
                row["decoded"]["transition_id"]
                    .as_str()
                    .expect("transition id")
                    .to_string(),
                row["decoded"]["timestamp_ms"]
                    .as_i64()
                    .expect("transition timestamp"),
            )
        })
        .collect()
}

fn trajectory_transition_timestamps(db: &str, trajectory: &Value) -> Vec<i64> {
    let by_id = transition_timestamp_map(db);
    trajectory["ordered_transition_ids"]
        .as_array()
        .expect("ordered transition ids")
        .iter()
        .map(|id| {
            let id = id.as_str().expect("transition id string");
            *by_id
                .get(id)
                .unwrap_or_else(|| panic!("transition {id} missing from inspected CF"))
        })
        .collect()
}

#[test]
fn dynamicjepa_phase5_cli_fsv_real_rocksdb() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let counter_db_path = temp.path().join("dynamicjepa_phase5_counter_rocksdb");
    let grid_db_path = temp.path().join("dynamicjepa_phase5_grid_rocksdb");
    let missing_panel_db_path = temp.path().join("dynamicjepa_phase5_missing_panel_rocksdb");
    let counter_db = counter_db_path.display().to_string();
    let grid_db = grid_db_path.display().to_string();
    let missing_panel_db = missing_panel_db_path.display().to_string();
    let root = repo_root();
    let evidence_dir = root.join("tmp/5090jepa_evidence/phase5");
    fs::create_dir_all(&evidence_dir).expect("create Phase 5 evidence dir");

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

    let mut evidence = Vec::new();

    let counter_initial = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    assert_eq!(count(&counter_initial, "dj_trajectories"), 0);
    evidence.push(json!({"step": "counter_initial_counts", "output": counter_initial}));

    let counter_materialize = run_phase4_happy(
        &counter_db,
        &counter_domain,
        "counter_world",
        &counter_happy,
    );
    assert_eq!(count(&counter_materialize, "dj_latent_panels"), 3);
    evidence.push(json!({"step": "counter_phase4_happy", "output": counter_materialize}));

    let unsupported_policy_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    let unsupported_policy_err = run_err(&[
        "dynamicjepa",
        "compile-trajectories",
        "--db",
        &counter_db,
        "--domain",
        "counter_world",
        "--policy",
        "unsupported_policy",
        "--json",
    ]);
    let unsupported_policy_after = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    assert_eq!(
        delta(
            &unsupported_policy_before,
            &unsupported_policy_after,
            "dj_trajectories"
        ),
        0
    );
    assert_eq!(
        delta(
            &unsupported_policy_before,
            &unsupported_policy_after,
            "dj_audit_log"
        ),
        0
    );
    evidence.push(json!({"step": "edge_unsupported_policy", "before": unsupported_policy_before, "error": unsupported_policy_err, "after": unsupported_policy_after}));

    let counter_compile = run_ok(&[
        "dynamicjepa",
        "compile-trajectories",
        "--db",
        &counter_db,
        "--domain",
        "counter_world",
        "--policy",
        "by_domain_session",
        "--json",
    ]);
    assert_eq!(count(&counter_compile, "dj_trajectories"), 1);
    assert_eq!(counter_compile["summary"]["trajectory_count"], 1);
    assert_eq!(counter_compile["count_delta"]["dj_trajectories"], 1);
    assert_eq!(counter_compile["count_delta"]["dj_audit_log"], 1);
    let counter_trajectory_id = counter_compile["created_ids"]["trajectory_ids"][0]
        .as_str()
        .expect("counter trajectory id")
        .to_string();
    evidence.push(json!({"step": "counter_compile_trajectories", "output": counter_compile}));

    let counter_get = run_ok(&[
        "dynamicjepa",
        "get-trajectory",
        "--db",
        &counter_db,
        "--id",
        &counter_trajectory_id,
        "--json",
    ]);
    let counter_trajectory = &counter_get["decoded_records"]["trajectory"];
    assert_eq!(
        counter_trajectory["ordered_transition_ids"]
            .as_array()
            .expect("counter transitions")
            .len(),
        3
    );
    assert_eq!(
        counter_trajectory["ordered_panel_ids"]
            .as_array()
            .expect("counter panels")
            .len(),
        3
    );
    assert_eq!(
        trajectory_transition_timestamps(&counter_db, counter_trajectory),
        vec![1700000000000, 1700000001000, 1700000002000]
    );
    evidence.push(json!({"step": "counter_get_trajectory", "output": counter_get}));

    let duplicate_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    let duplicate_err = run_err(&[
        "dynamicjepa",
        "compile-trajectories",
        "--db",
        &counter_db,
        "--domain",
        "counter_world",
        "--policy",
        "by_domain_session",
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
        delta(&duplicate_before, &duplicate_after, "dj_trajectories"),
        0
    );
    assert_eq!(
        delta(&duplicate_before, &duplicate_after, "dj_audit_log"),
        0
    );
    evidence.push(json!({"step": "edge_duplicate_compile", "before": duplicate_before, "error": duplicate_err, "after": duplicate_after}));

    let raw_rows = inspect_cf(&counter_db, "dj_raw_events", "10");
    let raw_key = raw_rows["decoded_records"]["rows"][0]["key_hex"]
        .as_str()
        .expect("raw key hex")
        .to_string();
    let trajectory_rows = inspect_cf(&counter_db, "dj_trajectories", "10");
    let trajectory_key = trajectory_rows["decoded_records"]["rows"][0]["key_hex"]
        .as_str()
        .expect("trajectory key hex")
        .to_string();

    let missing_binding_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    let missing_binding_err = run_err(&[
        "dynamicjepa",
        "bind",
        "--db",
        &counter_db,
        "--left-cf",
        "dj_raw_events",
        "--left-key",
        &raw_key,
        "--right-cf",
        "dj_trajectories",
        "--right-key",
        "00000000000000000000000000000000",
        "--method",
        "explicit_mapping",
        "--json",
    ]);
    let missing_binding_after = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    assert_eq!(
        delta(
            &missing_binding_before,
            &missing_binding_after,
            "dj_bindings"
        ),
        0
    );
    assert_eq!(
        delta(
            &missing_binding_before,
            &missing_binding_after,
            "dj_bindings_by_entity"
        ),
        0
    );
    assert_eq!(
        delta(
            &missing_binding_before,
            &missing_binding_after,
            "dj_audit_log"
        ),
        0
    );
    evidence.push(json!({"step": "edge_missing_binding_evidence", "before": missing_binding_before, "error": missing_binding_err, "after": missing_binding_after}));

    let bind_ok = run_ok(&[
        "dynamicjepa",
        "bind",
        "--db",
        &counter_db,
        "--left-cf",
        "dj_raw_events",
        "--left-key",
        &raw_key,
        "--right-cf",
        "dj_trajectories",
        "--right-key",
        &trajectory_key,
        "--method",
        "explicit_mapping",
        "--json",
    ]);
    assert_eq!(count(&bind_ok, "dj_bindings"), 1);
    assert_eq!(count(&bind_ok, "dj_bindings_by_entity"), 1);
    assert_eq!(bind_ok["count_delta"]["dj_bindings"], 1);
    assert_eq!(bind_ok["count_delta"]["dj_bindings_by_entity"], 1);
    assert_eq!(bind_ok["count_delta"]["dj_audit_log"], 1);
    evidence.push(json!({"step": "counter_bind_raw_to_trajectory", "output": bind_ok}));

    let id_equality_bind = run_ok(&[
        "dynamicjepa",
        "bind",
        "--db",
        &counter_db,
        "--left-cf",
        "dj_raw_events",
        "--left-key",
        &raw_key,
        "--right-cf",
        "dj_raw_events",
        "--right-key",
        &raw_key,
        "--method",
        "id_equality",
        "--json",
    ]);
    assert_eq!(count(&id_equality_bind, "dj_bindings"), 2);
    assert_eq!(count(&id_equality_bind, "dj_bindings_by_entity"), 2);
    assert_eq!(id_equality_bind["count_delta"]["dj_bindings"], 1);
    assert_eq!(id_equality_bind["count_delta"]["dj_bindings_by_entity"], 1);
    assert_eq!(id_equality_bind["count_delta"]["dj_audit_log"], 1);
    assert_eq!(
        id_equality_bind["decoded_records"]["binding"]["method"],
        "IdEquality"
    );
    evidence
        .push(json!({"step": "counter_bind_raw_to_raw_id_equality", "output": id_equality_bind}));

    let list_bindings = run_ok(&[
        "dynamicjepa",
        "list-bindings",
        "--db",
        &counter_db,
        "--entity",
        &raw_key,
        "--json",
    ]);
    assert_eq!(
        list_bindings["decoded_records"]["bindings"]
            .as_array()
            .expect("bindings")
            .len(),
        2
    );
    evidence.push(json!({"step": "counter_list_bindings", "output": list_bindings}));

    run_ok(&[
        "dynamicjepa",
        "register-domain",
        "--db",
        &missing_panel_db,
        "--file",
        &counter_domain,
        "--json",
    ]);
    run_ok(&[
        "dynamicjepa",
        "ingest-event",
        "--db",
        &missing_panel_db,
        "--domain",
        "counter_world",
        "--adapter",
        "json_event",
        "--file",
        &counter_happy,
        "--json",
    ]);
    let missing_panel_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &missing_panel_db,
        "--json",
    ]);
    let missing_panel_err = run_err(&[
        "dynamicjepa",
        "compile-trajectories",
        "--db",
        &missing_panel_db,
        "--domain",
        "counter_world",
        "--policy",
        "by_domain_session",
        "--json",
    ]);
    let missing_panel_after = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &missing_panel_db,
        "--json",
    ]);
    assert_eq!(
        delta(
            &missing_panel_before,
            &missing_panel_after,
            "dj_trajectories"
        ),
        0
    );
    assert_eq!(
        delta(&missing_panel_before, &missing_panel_after, "dj_audit_log"),
        0
    );
    evidence.push(json!({"step": "edge_missing_panel_source", "before": missing_panel_before, "error": missing_panel_err, "after": missing_panel_after}));

    let grid_initial = run_ok(&["dynamicjepa", "inspect-counts", "--db", &grid_db, "--json"]);
    assert_eq!(count(&grid_initial, "dj_trajectories"), 0);
    evidence.push(json!({"step": "grid_initial_counts", "output": grid_initial}));

    let grid_materialize = run_phase4_happy(&grid_db, &grid_domain, "gridworld_5x5", &grid_happy);
    assert_eq!(count(&grid_materialize, "dj_latent_panels"), 8);
    evidence.push(json!({"step": "grid_phase4_happy", "output": grid_materialize}));

    let grid_compile = run_ok(&[
        "dynamicjepa",
        "compile-trajectories",
        "--db",
        &grid_db,
        "--domain",
        "gridworld_5x5",
        "--policy",
        "by_domain_session",
        "--json",
    ]);
    assert_eq!(count(&grid_compile, "dj_trajectories"), 1);
    let grid_trajectory_id = grid_compile["created_ids"]["trajectory_ids"][0]
        .as_str()
        .expect("grid trajectory id")
        .to_string();
    evidence.push(json!({"step": "grid_compile_trajectories", "output": grid_compile}));

    let grid_get = run_ok(&[
        "dynamicjepa",
        "get-trajectory",
        "--db",
        &grid_db,
        "--id",
        &grid_trajectory_id,
        "--json",
    ]);
    let grid_trajectory = &grid_get["decoded_records"]["trajectory"];
    assert_eq!(
        grid_trajectory["ordered_transition_ids"]
            .as_array()
            .expect("grid transitions")
            .len(),
        8
    );
    assert_eq!(
        trajectory_transition_timestamps(&grid_db, grid_trajectory),
        vec![0, 1, 2, 3, 4, 5, 6, 7]
    );
    assert!(
        grid_trajectory["entity_refs"]
            .as_array()
            .expect("entity refs")
            .iter()
            .any(|value| value
                .as_str()
                .expect("entity ref string")
                .starts_with("trajectory_id=")),
        "grid trajectory must carry the source trajectory_id entity ref: {grid_trajectory:#}"
    );
    evidence.push(json!({"step": "grid_get_trajectory", "output": grid_get}));

    let grid_list = run_ok(&[
        "dynamicjepa",
        "list-trajectories",
        "--db",
        &grid_db,
        "--domain",
        "gridworld_5x5",
        "--json",
    ]);
    assert_eq!(grid_list["decoded_records"]["total_for_domain"], 1);
    evidence.push(json!({"step": "grid_list_trajectories", "output": grid_list}));

    let counter_final = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    let grid_final = run_ok(&["dynamicjepa", "inspect-counts", "--db", &grid_db, "--json"]);
    evidence.push(json!({"step": "counter_final_counts", "output": counter_final}));
    evidence.push(json!({"step": "grid_final_counts", "output": grid_final}));

    fs::write(
        evidence_dir.join("cli_phase5_fsv.json"),
        serde_json::to_vec_pretty(&json!({
            "source_of_truth": {
                "physical_dbs": {
                    "counter_world": counter_db_path,
                    "gridworld_5x5": grid_db_path,
                    "missing_panel_edge": missing_panel_db_path
                },
                "dynamicjepa_cfs": [
                    "dj_raw_events",
                    "dj_transitions",
                    "dj_latent_panels",
                    "dj_bindings",
                    "dj_bindings_by_entity",
                    "dj_trajectories",
                    "dj_audit_log"
                ],
                "read_paths": [
                    "dynamicjepa inspect-counts",
                    "dynamicjepa inspect-cf",
                    "dynamicjepa get-trajectory",
                    "dynamicjepa list-trajectories",
                    "dynamicjepa list-bindings"
                ]
            },
            "steps": evidence
        }))
        .expect("serialize evidence"),
    )
    .expect("write Phase 5 evidence log");
}
