//! Phase 4 DynamicJEPA CLI full-state verification.
//!
//! This test uses the real CLI binary, real RocksDB databases, and checked-in
//! deterministic fixture data. It verifies the physical DynamicJEPA source of
//! truth through inspect-counts, inspect-cf, get-panel, and
//! list-instrument-readings instead of trusting command return values alone.

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
    let path = temp.join("gridworld_first_trajectory.jsonl");
    fs::write(&path, subset).expect("write gridworld subset fixture");
    path
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

fn assert_any_adapter_failed(inspected: &Value, field_path: &str) {
    let rows = inspected["decoded_records"]["rows"]
        .as_array()
        .expect("inspect rows");
    assert!(
        rows.iter().any(|row| {
            row["decoded"]["status"] == "Failed" && row["decoded"]["field_path"] == field_path
        }),
        "expected failed adapter row for {field_path}, got {inspected:#}"
    );
}

#[test]
fn dynamicjepa_phase4_cli_fsv_real_rocksdb() {
    let temp = tempfile::tempdir().expect("create temp dir");
    let counter_db_path = temp.path().join("dynamicjepa_phase4_counter_rocksdb");
    let grid_db_path = temp.path().join("dynamicjepa_phase4_grid_rocksdb");
    let counter_db = counter_db_path.display().to_string();
    let grid_db = grid_db_path.display().to_string();
    let root = repo_root();
    let evidence_dir = root.join("tmp/5090jepa_evidence/phase4");
    fs::create_dir_all(&evidence_dir).expect("create Phase 4 evidence dir");

    let counter_domain = root
        .join("configs/dynamicjepa/domain_packs/counter_world.v1.toml")
        .display()
        .to_string();
    let counter_happy = root
        .join("configs/dynamicjepa/verification/counter_world_happy_path.jsonl")
        .display()
        .to_string();
    let counter_empty = root
        .join("configs/dynamicjepa/verification/counter_world_edge_empty_input.jsonl")
        .display()
        .to_string();
    let counter_invalid = root
        .join("configs/dynamicjepa/verification/counter_world_edge_invalid_action.jsonl")
        .display()
        .to_string();
    let counter_max = root
        .join("configs/dynamicjepa/verification/counter_world_edge_max_panel.jsonl")
        .display()
        .to_string();

    let grid_domain = root
        .join("configs/dynamicjepa/domain_packs/gridworld_5x5.v1.toml")
        .display()
        .to_string();
    let grid_happy = write_first_gridworld_trajectory(&root, temp.path())
        .display()
        .to_string();
    let grid_empty = root
        .join("configs/dynamicjepa/verification/gridworld_5x5_edge_empty.jsonl")
        .display()
        .to_string();
    let grid_invalid = root
        .join("configs/dynamicjepa/verification/gridworld_5x5_edge_invalid_action.jsonl")
        .display()
        .to_string();
    let grid_oob = root
        .join("configs/dynamicjepa/verification/gridworld_5x5_edge_oob_state.jsonl")
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
    assert_eq!(count(&counter_initial, "dj_raw_events"), 0);
    evidence.push(json!({"step": "counter_initial_counts", "output": counter_initial}));

    let counter_register = run_ok(&[
        "dynamicjepa",
        "register-domain",
        "--db",
        &counter_db,
        "--file",
        &counter_domain,
        "--json",
    ]);
    evidence.push(json!({"step": "counter_register_domain", "output": counter_register}));

    let counter_ingest = run_ok(&[
        "dynamicjepa",
        "ingest-event",
        "--db",
        &counter_db,
        "--domain",
        "counter_world",
        "--adapter",
        "json_event",
        "--file",
        &counter_happy,
        "--json",
    ]);
    assert_eq!(count(&counter_ingest, "dj_raw_events"), 3);
    assert_eq!(count(&counter_ingest, "dj_adapter_runs"), 3);
    assert_eq!(count(&counter_ingest, "dj_normalized_states"), 3);
    assert_eq!(count(&counter_ingest, "dj_actions"), 3);
    assert_eq!(count(&counter_ingest, "dj_outcomes"), 3);
    assert_eq!(count(&counter_ingest, "dj_transitions"), 3);
    evidence.push(json!({"step": "counter_ingest_happy", "output": counter_ingest}));

    let adapter_rows = inspect_cf(&counter_db, "dj_adapter_runs", "10");
    assert!(
        adapter_rows["decoded_records"]["rows"]
            .as_array()
            .expect("adapter rows")
            .iter()
            .all(|row| row["decoded"]["status"] == "Completed"),
        "all counter happy adapter runs must be completed"
    );
    evidence.push(json!({"step": "counter_inspect_adapter_runs", "output": adapter_rows}));

    let counter_materialize = run_ok(&[
        "dynamicjepa",
        "materialize-panel",
        "--db",
        &counter_db,
        "--all-pending",
        "--domain",
        "counter_world",
        "--json",
    ]);
    assert_eq!(count(&counter_materialize, "dj_instrument_readings"), 6);
    assert_eq!(count(&counter_materialize, "dj_latent_panels"), 3);
    assert_eq!(
        counter_materialize["summary"]["materialized"]
            .as_u64()
            .expect("materialized count"),
        3
    );
    evidence.push(json!({"step": "counter_materialize_panels", "output": counter_materialize}));

    let panel_id = counter_materialize["created_ids"]["panel_ids"][0]
        .as_str()
        .expect("panel id")
        .to_string();
    let event_id = counter_materialize["created_ids"]["event_ids"][0]
        .as_str()
        .expect("event id")
        .to_string();
    let get_panel = run_ok(&[
        "dynamicjepa",
        "get-panel",
        "--db",
        &counter_db,
        "--panel-id",
        &panel_id,
        "--include-readings",
        "--json",
    ]);
    assert_eq!(
        get_panel["decoded_records"]["panel"]["ordered_slots"]
            .as_array()
            .expect("panel slots")
            .len(),
        2
    );
    assert_eq!(
        get_panel["decoded_records"]["instrument_readings"]
            .as_array()
            .expect("panel readings")
            .len(),
        2
    );
    evidence.push(json!({"step": "counter_get_panel", "output": get_panel}));

    let list_readings = run_ok(&[
        "dynamicjepa",
        "list-instrument-readings",
        "--db",
        &counter_db,
        "--event-id",
        &event_id,
        "--json",
    ]);
    assert_eq!(
        list_readings["decoded_records"]["instrument_readings"]
            .as_array()
            .expect("event readings")
            .len(),
        2
    );
    evidence.push(json!({"step": "counter_list_readings", "output": list_readings}));

    let before_empty = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    let empty_err = run_err(&[
        "dynamicjepa",
        "ingest-event",
        "--db",
        &counter_db,
        "--domain",
        "counter_world",
        "--adapter",
        "json_event",
        "--file",
        &counter_empty,
        "--json",
    ]);
    let after_empty = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    assert_eq!(delta(&before_empty, &after_empty, "dj_raw_events"), 0);
    assert_eq!(delta(&before_empty, &after_empty, "dj_adapter_runs"), 0);
    assert_eq!(delta(&before_empty, &after_empty, "dj_audit_log"), 1);
    evidence.push(json!({"step": "counter_edge_empty", "before": before_empty, "error": empty_err, "after": after_empty}));

    let before_invalid = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    let invalid_err = run_err(&[
        "dynamicjepa",
        "ingest-event",
        "--db",
        &counter_db,
        "--domain",
        "counter_world",
        "--adapter",
        "json_event",
        "--file",
        &counter_invalid,
        "--json",
    ]);
    let after_invalid = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    assert_eq!(delta(&before_invalid, &after_invalid, "dj_raw_events"), 1);
    assert_eq!(delta(&before_invalid, &after_invalid, "dj_adapter_runs"), 1);
    assert_eq!(
        delta(&before_invalid, &after_invalid, "dj_normalized_states"),
        0
    );
    assert_eq!(delta(&before_invalid, &after_invalid, "dj_transitions"), 0);
    let invalid_adapters = inspect_cf(&counter_db, "dj_adapter_runs", "20");
    assert_any_adapter_failed(&invalid_adapters, "action.delta");
    evidence.push(json!({"step": "counter_edge_invalid_action", "before": before_invalid, "error": invalid_err, "after": after_invalid, "adapter_rows": invalid_adapters}));

    let before_max = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    let max_err = run_err(&[
        "dynamicjepa",
        "ingest-event",
        "--db",
        &counter_db,
        "--domain",
        "counter_world",
        "--adapter",
        "json_event",
        "--file",
        &counter_max,
        "--json",
    ]);
    let after_max = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &counter_db,
        "--json",
    ]);
    assert_eq!(delta(&before_max, &after_max, "dj_raw_events"), 1);
    assert_eq!(delta(&before_max, &after_max, "dj_adapter_runs"), 1);
    assert_eq!(delta(&before_max, &after_max, "dj_outcomes"), 0);
    assert_eq!(delta(&before_max, &after_max, "dj_transitions"), 0);
    let max_adapters = inspect_cf(&counter_db, "dj_adapter_runs", "20");
    assert_any_adapter_failed(&max_adapters, "outcome.next_counter");
    evidence.push(json!({"step": "counter_edge_max_panel", "before": before_max, "error": max_err, "after": after_max, "adapter_rows": max_adapters}));

    let grid_initial = run_ok(&["dynamicjepa", "inspect-counts", "--db", &grid_db, "--json"]);
    assert_eq!(count(&grid_initial, "dj_raw_events"), 0);
    evidence.push(json!({"step": "grid_initial_counts", "output": grid_initial}));

    let grid_register = run_ok(&[
        "dynamicjepa",
        "register-domain",
        "--db",
        &grid_db,
        "--file",
        &grid_domain,
        "--json",
    ]);
    evidence.push(json!({"step": "grid_register_domain", "output": grid_register}));

    let grid_ingest = run_ok(&[
        "dynamicjepa",
        "ingest-event",
        "--db",
        &grid_db,
        "--domain",
        "gridworld_5x5",
        "--adapter",
        "json_event",
        "--file",
        &grid_happy,
        "--json",
    ]);
    assert_eq!(count(&grid_ingest, "dj_raw_events"), 8);
    assert_eq!(count(&grid_ingest, "dj_adapter_runs"), 8);
    assert_eq!(count(&grid_ingest, "dj_normalized_states"), 8);
    assert_eq!(count(&grid_ingest, "dj_actions"), 8);
    assert_eq!(count(&grid_ingest, "dj_outcomes"), 8);
    assert_eq!(count(&grid_ingest, "dj_transitions"), 8);
    evidence.push(json!({"step": "grid_ingest_happy", "output": grid_ingest}));

    let grid_materialize = run_ok(&[
        "dynamicjepa",
        "materialize-panel",
        "--db",
        &grid_db,
        "--all-pending",
        "--domain",
        "gridworld_5x5",
        "--json",
    ]);
    assert_eq!(count(&grid_materialize, "dj_instrument_readings"), 24);
    assert_eq!(count(&grid_materialize, "dj_latent_panels"), 8);
    assert_eq!(
        grid_materialize["summary"]["materialized"]
            .as_u64()
            .expect("grid materialized count"),
        8
    );
    evidence.push(json!({"step": "grid_materialize_panels", "output": grid_materialize}));

    let before_grid_empty = run_ok(&["dynamicjepa", "inspect-counts", "--db", &grid_db, "--json"]);
    let grid_empty_err = run_err(&[
        "dynamicjepa",
        "ingest-event",
        "--db",
        &grid_db,
        "--domain",
        "gridworld_5x5",
        "--adapter",
        "json_event",
        "--file",
        &grid_empty,
        "--json",
    ]);
    let after_grid_empty = run_ok(&["dynamicjepa", "inspect-counts", "--db", &grid_db, "--json"]);
    assert_eq!(
        delta(&before_grid_empty, &after_grid_empty, "dj_raw_events"),
        0
    );
    assert_eq!(
        delta(&before_grid_empty, &after_grid_empty, "dj_adapter_runs"),
        0
    );
    assert_eq!(
        delta(&before_grid_empty, &after_grid_empty, "dj_audit_log"),
        1
    );
    evidence.push(json!({"step": "grid_edge_empty", "before": before_grid_empty, "error": grid_empty_err, "after": after_grid_empty}));

    let before_grid_invalid =
        run_ok(&["dynamicjepa", "inspect-counts", "--db", &grid_db, "--json"]);
    let grid_invalid_err = run_err(&[
        "dynamicjepa",
        "ingest-event",
        "--db",
        &grid_db,
        "--domain",
        "gridworld_5x5",
        "--adapter",
        "json_event",
        "--file",
        &grid_invalid,
        "--json",
    ]);
    let after_grid_invalid = run_ok(&["dynamicjepa", "inspect-counts", "--db", &grid_db, "--json"]);
    assert_eq!(
        delta(&before_grid_invalid, &after_grid_invalid, "dj_raw_events"),
        1
    );
    assert_eq!(
        delta(&before_grid_invalid, &after_grid_invalid, "dj_adapter_runs"),
        1
    );
    assert_eq!(
        delta(
            &before_grid_invalid,
            &after_grid_invalid,
            "dj_normalized_states"
        ),
        0
    );
    let grid_invalid_adapters = inspect_cf(&grid_db, "dj_adapter_runs", "20");
    assert_any_adapter_failed(&grid_invalid_adapters, "action.move");
    evidence.push(json!({"step": "grid_edge_invalid_action", "before": before_grid_invalid, "error": grid_invalid_err, "after": after_grid_invalid, "adapter_rows": grid_invalid_adapters}));

    let before_grid_oob = run_ok(&["dynamicjepa", "inspect-counts", "--db", &grid_db, "--json"]);
    let grid_oob_err = run_err(&[
        "dynamicjepa",
        "ingest-event",
        "--db",
        &grid_db,
        "--domain",
        "gridworld_5x5",
        "--adapter",
        "json_event",
        "--file",
        &grid_oob,
        "--json",
    ]);
    let after_grid_oob = run_ok(&["dynamicjepa", "inspect-counts", "--db", &grid_db, "--json"]);
    assert_eq!(delta(&before_grid_oob, &after_grid_oob, "dj_raw_events"), 1);
    assert_eq!(
        delta(&before_grid_oob, &after_grid_oob, "dj_adapter_runs"),
        1
    );
    assert_eq!(
        delta(&before_grid_oob, &after_grid_oob, "dj_normalized_states"),
        0
    );
    let grid_oob_adapters = inspect_cf(&grid_db, "dj_adapter_runs", "20");
    assert_any_adapter_failed(&grid_oob_adapters, "state.x");
    evidence.push(json!({"step": "grid_edge_oob_state", "before": before_grid_oob, "error": grid_oob_err, "after": after_grid_oob, "adapter_rows": grid_oob_adapters}));

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
        evidence_dir.join("cli_phase4_fsv.json"),
        serde_json::to_vec_pretty(&json!({
            "source_of_truth": {
                "physical_dbs": {
                    "counter_world": counter_db_path,
                    "gridworld_5x5": grid_db_path
                },
                "dynamicjepa_cfs": [
                    "dj_raw_events",
                    "dj_adapter_runs",
                    "dj_normalized_states",
                    "dj_actions",
                    "dj_outcomes",
                    "dj_transitions",
                    "dj_instrument_readings",
                    "dj_latent_panels",
                    "dj_audit_log"
                ],
                "read_paths": [
                    "dynamicjepa inspect-counts",
                    "dynamicjepa inspect-cf",
                    "dynamicjepa get-panel",
                    "dynamicjepa list-instrument-readings"
                ]
            },
            "steps": evidence
        }))
        .expect("serialize evidence"),
    )
    .expect("write Phase 4 evidence log");
}
