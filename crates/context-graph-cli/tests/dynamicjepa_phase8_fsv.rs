//! Phase 8 DynamicJEPA CLI full-state verification.
//!
//! This test uses the real CLI binary, real RocksDB, real CUDA training,
//! registered artifact files, and decoded source-of-truth reads for prediction,
//! planning, guard decisions, and surprise events.

use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Map, Value};

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

fn write_counter_fixture(temp: &Path) -> PathBuf {
    let mut lines = Vec::new();
    for counter in 0..60 {
        lines.push(format!(
            r#"{{"state":{{"counter":{counter}}},"action":{{"delta":1}},"outcome":{{"next_counter":{}}},"ts":{}}}"#,
            counter + 1,
            1_700_000_000_000i64 + counter as i64 * 1_000
        ));
    }
    let path = temp.join("counter_world_phase8_generated_60.jsonl");
    fs::write(&path, lines.join("\n") + "\n").expect("write counter fixture");
    path
}

fn write_high_counter_fixture(temp: &Path) -> PathBuf {
    let path = temp.join("counter_world_phase8_high_boundary.jsonl");
    fs::write(
        &path,
        r#"{"state":{"counter":64},"action":{"delta":0},"outcome":{"next_counter":64},"ts":1700000999000}"#,
    )
    .expect("write high counter fixture");
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

fn run_phase6_counter_data(
    db: &str,
    root: &Path,
    fixture: &str,
) -> (String, String, String, Vec<Value>) {
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
    let train_shard_id = train["created_ids"]["shard_id"]
        .as_str()
        .expect("train shard id")
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
    let val_shard_id = val["created_ids"]["shard_id"]
        .as_str()
        .expect("val shard id")
        .to_string();
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
    (dataset_id, train_shard_id, val_shard_id, evidence)
}

fn inspect_dataset_row(db: &str, dataset_id: &str, shard_id: &str, row: &str) -> Value {
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

fn get_panel(db: &str, panel_id: &str) -> Value {
    run_ok(&[
        "dynamicjepa",
        "get-panel",
        "--db",
        db,
        "--panel-id",
        panel_id,
        "--include-readings",
        "--json",
    ])
}

fn action_delta(action: &Value) -> i64 {
    action["fields"]["delta"]["I64"]
        .as_i64()
        .unwrap_or_else(|| panic!("candidate action missing delta field: {action:#}"))
}

fn string_array(value: &Value, field: &str) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{field} must be an array: {value:#}"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("{field} item must be string: {item:#}"))
                .to_string()
        })
        .collect()
}

fn uuid_key_hex(id: &str) -> String {
    id.chars().filter(|ch| *ch != '-').collect()
}

fn inspect_cf_key(db: &str, cf: &str, id: &str) -> Value {
    let key_hex = uuid_key_hex(id);
    run_ok(&[
        "dynamicjepa",
        "inspect-cf",
        "--db",
        db,
        "--cf",
        cf,
        "--key-hex",
        &key_hex,
        "--json",
    ])
}

fn decoded_cf_key_row(value: &Value) -> &Value {
    &value["decoded_records"]["row"]["decoded"]
}

fn ldb_count(db: &str, cf: &str) -> u64 {
    let output = Command::new("/usr/bin/ldb")
        .args([
            format!("--db={db}"),
            format!("--column_family={cf}"),
            "dump".to_string(),
            "--count_only".to_string(),
        ])
        .output()
        .unwrap_or_else(|err| panic!("run ldb count for {cf}: {err}"));
    let stdout = String::from_utf8(output.stdout).expect("ldb stdout is UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("ldb stderr is UTF-8");
    assert!(
        output.status.success(),
        "ldb count failed for {cf}: stdout={stdout}\nstderr={stderr}"
    );
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("Keys in range:"))
        .unwrap_or_else(|| panic!("ldb count output missing key count for {cf}: {stdout}"))
        .trim()
        .parse::<u64>()
        .unwrap_or_else(|err| panic!("parse ldb count for {cf}: {err}; stdout={stdout}"))
}

#[test]
fn dynamicjepa_phase8_cli_fsv_real_prediction_plan_surprise() {
    let root = repo_root();
    let evidence_dir = root.join("tmp/5090jepa_evidence/phase8");
    fs::create_dir_all(&evidence_dir).expect("create Phase 8 evidence dir");
    let run_root = evidence_dir.join("cli_phase8_fsv_runtime");
    if run_root.exists() {
        fs::remove_dir_all(&run_root).expect("remove previous Phase 8 runtime evidence");
    }
    fs::create_dir_all(&run_root).expect("create Phase 8 runtime evidence dir");

    let db_path = run_root.join("dynamicjepa_phase8_counter_rocksdb");
    let missing_db_path = run_root.join("dynamicjepa_phase8_missing_rocksdb");
    let artifact_root_path = run_root.join("dynamicjepa_phase8_artifacts");
    let db = db_path.display().to_string();
    let missing_db = missing_db_path.display().to_string();
    let artifact_root = artifact_root_path.display().to_string();
    let config = root
        .join("configs/dynamicjepa/verification/tiny_train_config.json")
        .display()
        .to_string();
    let fixture = write_counter_fixture(&run_root).display().to_string();
    let high_fixture = write_high_counter_fixture(&run_root).display().to_string();
    let skill_id = "10000000-0000-0000-0000-000000000001";

    let mut evidence = Vec::new();
    let (dataset_id, train_shard_id, val_shard_id, setup_evidence) =
        run_phase6_counter_data(&db, &root, &fixture);
    evidence.extend(setup_evidence);

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
    let artifact_id = train["created_ids"]["artifact_id"]
        .as_str()
        .expect("artifact id")
        .to_string();
    let artifact_root = train["summary"]["artifact_root"]
        .as_str()
        .expect("artifact root")
        .to_string();
    assert!(train["artifact_hashes"]
        .as_array()
        .expect("artifact hashes")
        .iter()
        .all(|file| file["equal"].as_bool() == Some(true)));
    evidence.push(json!({"step": "train", "output": train}));

    let row0 = inspect_dataset_row(&db, &dataset_id, &val_shard_id, "0");
    let input_panel_id = row0["decoded_records"]["row"]["input_panel_id"]
        .as_str()
        .expect("input panel id")
        .to_string();
    let action_id = row0["decoded_records"]["row"]["input_action_id"]
        .as_str()
        .expect("action id")
        .to_string();
    let target_panel_id = row0["decoded_records"]["row"]["target_panel_id"]
        .as_str()
        .expect("target panel id")
        .to_string();
    let target_panel = get_panel(&db, &target_panel_id);
    let target_outcome_id = target_panel["decoded_records"]["panel"]["outcome_id"]
        .as_str()
        .expect("target outcome id")
        .to_string();
    let plan_row = inspect_dataset_row(&db, &dataset_id, &train_shard_id, "0");
    let plan_input_panel_id = plan_row["decoded_records"]["row"]["input_panel_id"]
        .as_str()
        .expect("plan input panel id")
        .to_string();
    evidence.push(
        json!({"step": "inspect_prediction_inputs", "row": row0, "target_panel": target_panel, "plan_row": plan_row}),
    );

    let before_predict = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    let predict = run_ok(&[
        "dynamicjepa",
        "predict",
        "--db",
        &db,
        "--artifact-id",
        &artifact_id,
        "--panel-id",
        &input_panel_id,
        "--action-id",
        &action_id,
        "--json",
    ]);
    let after_predict = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(delta(&before_predict, &after_predict, "dj_predictions"), 1);
    assert_eq!(delta(&before_predict, &after_predict, "dj_audit_log"), 1);
    let prediction_id = predict["created_ids"]["prediction_id"]
        .as_str()
        .expect("prediction id")
        .to_string();
    assert_eq!(
        predict["decoded_records"]["prediction"]["model_artifact_id"],
        artifact_id
    );
    assert_eq!(
        predict["decoded_records"]["prediction"]["input_panel_id"],
        input_panel_id
    );
    assert_eq!(
        predict["decoded_records"]["prediction"]["candidate_action_id"],
        action_id
    );
    evidence.push(json!({"step": "happy_predict", "before": before_predict, "output": predict, "after": after_predict}));

    let get_prediction = run_ok(&[
        "dynamicjepa",
        "get-prediction",
        "--db",
        &db,
        "--id",
        &prediction_id,
        "--json",
    ]);
    assert_eq!(
        get_prediction["decoded_records"]["prediction"]["prediction_id"],
        prediction_id
    );
    evidence.push(json!({"step": "get_prediction_readback", "output": get_prediction}));

    let prediction_cf_key = inspect_cf_key(&db, "dj_predictions", &prediction_id);
    let prediction_cf_record = decoded_cf_key_row(&prediction_cf_key);
    assert_eq!(
        prediction_cf_record["prediction_id"].as_str(),
        Some(prediction_id.as_str())
    );
    assert_eq!(
        prediction_cf_record["model_artifact_id"].as_str(),
        Some(artifact_id.as_str())
    );
    assert_eq!(
        prediction_cf_record["input_panel_id"].as_str(),
        Some(input_panel_id.as_str())
    );
    assert_eq!(
        prediction_cf_record["candidate_action_id"].as_str(),
        Some(action_id.as_str())
    );
    assert_eq!(
        prediction_cf_key["decoded_records"]["row"]["key_hex"].as_str(),
        Some(uuid_key_hex(&prediction_id).as_str())
    );
    evidence
        .push(json!({"step": "inspect_prediction_cf_key_readback", "output": prediction_cf_key}));

    let before_plan = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    let plan = run_ok(&[
        "dynamicjepa",
        "plan",
        "--db",
        &db,
        "--artifact-id",
        &artifact_id,
        "--panel-id",
        &plan_input_panel_id,
        "--skill-id",
        skill_id,
        "--json",
    ]);
    let after_plan = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(delta(&before_plan, &after_plan, "dj_actions"), 5);
    assert_eq!(delta(&before_plan, &after_plan, "dj_predictions"), 5);
    assert_eq!(delta(&before_plan, &after_plan, "dj_guard_decisions"), 5);
    assert_eq!(delta(&before_plan, &after_plan, "dj_plan_traces"), 1);
    assert_eq!(delta(&before_plan, &after_plan, "dj_skill_policies"), 1);
    assert_eq!(plan["summary"]["candidate_count"], 5);
    assert_eq!(plan["summary"]["prediction_count"], 5);
    assert_eq!(plan["summary"]["guard_decision_count"], 5);
    assert_eq!(plan["summary"]["status"], "Selected");
    assert_eq!(
        plan["summary"]["utility_scores"],
        json!([-2.0, -1.0, 0.0, 1.0, 2.0])
    );
    assert_eq!(
        plan["decoded_records"]["plan_trace"]["selected_action_id"],
        plan["created_ids"]["selected_action_id"]
    );
    let selected_action_id = plan["created_ids"]["selected_action_id"]
        .as_str()
        .expect("selected action id")
        .to_string();
    let selected_action = plan["decoded_records"]["candidate_actions"]
        .as_array()
        .expect("candidate actions")
        .iter()
        .find(|action| action["action_id"].as_str() == Some(selected_action_id.as_str()))
        .unwrap_or_else(|| panic!("selected action {selected_action_id} missing from candidates"));
    assert_eq!(action_delta(selected_action), 2);
    assert_eq!(selected_action["action_origin"], "Hypothetical");
    for action in plan["decoded_records"]["candidate_actions"]
        .as_array()
        .expect("candidate actions")
    {
        assert_eq!(action["action_origin"], "Hypothetical");
    }
    let plan_trace_id = plan["created_ids"]["plan_trace_id"]
        .as_str()
        .expect("plan trace id")
        .to_string();
    evidence.push(
        json!({"step": "happy_plan", "before": before_plan, "output": plan, "after": after_plan}),
    );

    let get_plan = run_ok(&[
        "dynamicjepa",
        "get-plan-trace",
        "--db",
        &db,
        "--id",
        &plan_trace_id,
        "--include-predictions",
        "--include-guards",
        "--json",
    ]);
    assert_eq!(
        get_plan["decoded_records"]["plan_trace"]["plan_trace_id"],
        plan_trace_id
    );
    assert_eq!(
        get_plan["decoded_records"]["predictions"]
            .as_array()
            .expect("plan predictions")
            .len(),
        5
    );
    assert_eq!(
        get_plan["decoded_records"]["guard_decisions"]
            .as_array()
            .expect("plan guards")
            .len(),
        5
    );
    evidence.push(json!({"step": "get_plan_trace_readback", "output": get_plan}));

    let candidate_action_ids = string_array(
        &plan["created_ids"]["candidate_action_ids"],
        "candidate_action_ids",
    );
    let plan_prediction_ids =
        string_array(&plan["created_ids"]["prediction_ids"], "prediction_ids");
    let guard_decision_ids = string_array(
        &plan["created_ids"]["guard_decision_ids"],
        "guard_decision_ids",
    );
    let plan_trace_cf_key = inspect_cf_key(&db, "dj_plan_traces", &plan_trace_id);
    let plan_trace_cf_record = decoded_cf_key_row(&plan_trace_cf_key);
    assert_eq!(
        plan_trace_cf_record["plan_trace_id"].as_str(),
        Some(plan_trace_id.as_str())
    );
    assert_eq!(
        string_array(
            &plan_trace_cf_record["candidate_action_ids"],
            "plan_trace.candidate_action_ids"
        ),
        candidate_action_ids
    );
    assert_eq!(
        string_array(
            &plan_trace_cf_record["prediction_ids"],
            "plan_trace.prediction_ids"
        ),
        plan_prediction_ids
    );
    assert_eq!(
        string_array(
            &plan_trace_cf_record["guard_decision_ids"],
            "plan_trace.guard_decision_ids"
        ),
        guard_decision_ids
    );
    assert_eq!(
        plan_trace_cf_record["selected_action_id"].as_str(),
        Some(selected_action_id.as_str())
    );

    let action_cf_key_readbacks = candidate_action_ids
        .iter()
        .map(|id| inspect_cf_key(&db, "dj_actions", id))
        .collect::<Vec<_>>();
    let persisted_deltas = action_cf_key_readbacks
        .iter()
        .map(|row| action_delta(decoded_cf_key_row(row)))
        .collect::<Vec<_>>();
    assert_eq!(persisted_deltas, vec![-2, -1, 0, 1, 2]);
    for (idx, row) in action_cf_key_readbacks.iter().enumerate() {
        let decoded = decoded_cf_key_row(row);
        assert_eq!(
            decoded["action_id"].as_str(),
            Some(candidate_action_ids[idx].as_str())
        );
        assert_eq!(decoded["action_origin"].as_str(), Some("Hypothetical"));
    }

    let plan_prediction_cf_key_readbacks = plan_prediction_ids
        .iter()
        .map(|id| inspect_cf_key(&db, "dj_predictions", id))
        .collect::<Vec<_>>();
    for (idx, row) in plan_prediction_cf_key_readbacks.iter().enumerate() {
        let decoded = decoded_cf_key_row(row);
        assert_eq!(
            decoded["prediction_id"].as_str(),
            Some(plan_prediction_ids[idx].as_str())
        );
        assert_eq!(
            decoded["input_panel_id"].as_str(),
            Some(plan_input_panel_id.as_str())
        );
        assert_eq!(
            decoded["candidate_action_id"].as_str(),
            Some(candidate_action_ids[idx].as_str())
        );
        assert_eq!(
            decoded["header"]["source_run_id"].as_str(),
            Some(plan_trace_id.as_str())
        );
    }

    let guard_cf_key_readbacks = guard_decision_ids
        .iter()
        .map(|id| inspect_cf_key(&db, "dj_guard_decisions", id))
        .collect::<Vec<_>>();
    for row in &guard_cf_key_readbacks {
        let decoded = decoded_cf_key_row(row);
        assert_eq!(
            decoded["plan_trace_id"].as_str(),
            Some(plan_trace_id.as_str())
        );
        assert_eq!(
            decoded["header"]["source_run_id"].as_str(),
            Some(plan_trace_id.as_str())
        );
        let guard_action_id = decoded["candidate_action_id"]
            .as_str()
            .expect("guard candidate_action_id");
        assert!(
            candidate_action_ids.iter().any(|id| id == guard_action_id),
            "guard action {guard_action_id} must reference a persisted candidate"
        );
    }

    let skill_policy_cf_key = inspect_cf_key(&db, "dj_skill_policies", skill_id);
    assert_eq!(
        decoded_cf_key_row(&skill_policy_cf_key)["skill_id"].as_str(),
        Some(skill_id)
    );
    evidence.push(json!({
        "step": "inspect_plan_cf_key_readbacks",
        "plan_trace": plan_trace_cf_key,
        "candidate_actions": action_cf_key_readbacks,
        "predictions": plan_prediction_cf_key_readbacks,
        "guard_decisions": guard_cf_key_readbacks,
        "skill_policy": skill_policy_cf_key
    }));

    let before_surprise_none = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    let surprise_none = run_ok(&[
        "dynamicjepa",
        "record-surprise",
        "--db",
        &db,
        "--prediction-id",
        &prediction_id,
        "--observed-outcome-id",
        &target_outcome_id,
        "--observed-panel-id",
        &target_panel_id,
        "--json",
    ]);
    let after_surprise_none = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(surprise_none["summary"]["wrote_surprise"], false);
    assert_eq!(
        delta(
            &before_surprise_none,
            &after_surprise_none,
            "dj_surprise_events"
        ),
        0
    );
    evidence.push(json!({"step": "happy_record_surprise_no_row", "before": before_surprise_none, "output": surprise_none, "after": after_surprise_none}));

    let high_ingest = run_ok(&[
        "dynamicjepa",
        "ingest-event",
        "--db",
        &db,
        "--domain",
        "counter_world",
        "--adapter",
        "json_event",
        "--file",
        &high_fixture,
        "--json",
    ]);
    let high_panel_out = run_ok(&[
        "dynamicjepa",
        "materialize-panel",
        "--db",
        &db,
        "--all-pending",
        "--domain",
        "counter_world",
        "--json",
    ]);
    let high_panel_id = high_panel_out["created_ids"]["panel_ids"][0]
        .as_str()
        .expect("high panel id")
        .to_string();
    let high_panel = get_panel(&db, &high_panel_id);
    let high_outcome_id = high_panel["decoded_records"]["panel"]["outcome_id"]
        .as_str()
        .expect("high outcome id")
        .to_string();
    evidence.push(json!({"step": "edge_setup_high_boundary_panel", "ingest": high_ingest, "materialize": high_panel_out, "panel": high_panel}));

    let before_mismatched_surprise =
        run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    let mismatched_surprise = run_err(&[
        "dynamicjepa",
        "record-surprise",
        "--db",
        &db,
        "--prediction-id",
        &prediction_id,
        "--observed-outcome-id",
        &high_outcome_id,
        "--observed-panel-id",
        &target_panel_id,
        "--json",
    ]);
    let after_mismatched_surprise =
        run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(mismatched_surprise["error_code"], "VALIDATION");
    assert_eq!(
        mismatched_surprise["field"],
        "record-surprise.observed_outcome_id"
    );
    assert_eq!(
        delta(
            &before_mismatched_surprise,
            &after_mismatched_surprise,
            "dj_surprise_events"
        ),
        0
    );
    assert_eq!(
        delta(
            &before_mismatched_surprise,
            &after_mismatched_surprise,
            "dj_audit_log"
        ),
        0
    );
    evidence.push(json!({"step": "edge_mismatched_outcome_panel_surprise", "before": before_mismatched_surprise, "error": mismatched_surprise, "after": after_mismatched_surprise}));

    let before_guard_edge = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    let guard_plan = run_ok(&[
        "dynamicjepa",
        "plan",
        "--db",
        &db,
        "--artifact-id",
        &artifact_id,
        "--panel-id",
        &high_panel_id,
        "--skill-id",
        skill_id,
        "--json",
    ]);
    let after_guard_edge = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(
        delta(&before_guard_edge, &after_guard_edge, "dj_actions"),
        5
    );
    assert_eq!(
        delta(&before_guard_edge, &after_guard_edge, "dj_guard_decisions"),
        5
    );
    let rejected = guard_plan["decoded_records"]["guard_decisions"]
        .as_array()
        .expect("guard decisions")
        .iter()
        .filter(|guard| guard["decision"].get("Reject").is_some())
        .count();
    assert!(rejected >= 2, "expected rejected bounds_check decisions");
    evidence.push(json!({"step": "edge_guard_rejection_evidence", "before": before_guard_edge, "output": guard_plan, "after": after_guard_edge}));

    let far_row = inspect_dataset_row(&db, &dataset_id, &train_shard_id, "0");
    let far_target_panel_id = far_row["decoded_records"]["row"]["target_panel_id"]
        .as_str()
        .expect("far target panel id")
        .to_string();
    let far_target_panel = get_panel(&db, &far_target_panel_id);
    let far_target_outcome_id = far_target_panel["decoded_records"]["panel"]["outcome_id"]
        .as_str()
        .expect("far target outcome id")
        .to_string();
    let before_surprise = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    let forced_surprise = run_ok(&[
        "dynamicjepa",
        "record-surprise",
        "--db",
        &db,
        "--prediction-id",
        &prediction_id,
        "--observed-outcome-id",
        &far_target_outcome_id,
        "--observed-panel-id",
        &far_target_panel_id,
        "--json",
    ]);
    let after_surprise = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(forced_surprise["summary"]["wrote_surprise"], true);
    assert_eq!(
        delta(&before_surprise, &after_surprise, "dj_surprise_events"),
        1
    );
    assert_eq!(delta(&before_surprise, &after_surprise, "dj_audit_log"), 1);
    let surprise_id = forced_surprise["created_ids"]["surprise_event_id"]
        .as_str()
        .expect("surprise event id")
        .to_string();
    assert_eq!(
        forced_surprise["decoded_records"]["surprise_event"]["prediction_id"],
        prediction_id
    );
    assert_eq!(
        forced_surprise["decoded_records"]["surprise_event"]["observed_panel_id"],
        far_target_panel_id
    );
    evidence.push(json!({"step": "edge_forced_surprise_event", "before": before_surprise, "output": forced_surprise, "after": after_surprise}));

    let get_surprise = run_ok(&[
        "dynamicjepa",
        "get-surprise",
        "--db",
        &db,
        "--id",
        &surprise_id,
        "--json",
    ]);
    assert_eq!(
        get_surprise["decoded_records"]["surprise_event"]["surprise_event_id"],
        surprise_id
    );
    evidence.push(json!({"step": "get_surprise_readback", "output": get_surprise}));

    let surprise_cf_key = inspect_cf_key(&db, "dj_surprise_events", &surprise_id);
    let surprise_cf_record = decoded_cf_key_row(&surprise_cf_key);
    assert_eq!(
        surprise_cf_record["surprise_event_id"].as_str(),
        Some(surprise_id.as_str())
    );
    assert_eq!(
        surprise_cf_record["prediction_id"].as_str(),
        Some(prediction_id.as_str())
    );
    assert_eq!(
        surprise_cf_record["observed_outcome_id"].as_str(),
        Some(far_target_outcome_id.as_str())
    );
    assert_eq!(
        surprise_cf_record["observed_panel_id"].as_str(),
        Some(far_target_panel_id.as_str())
    );
    evidence.push(json!({"step": "inspect_surprise_cf_key_readback", "output": surprise_cf_key}));

    let missing_before = run_ok(&[
        "dynamicjepa",
        "inspect-counts",
        "--db",
        &missing_db,
        "--allow-missing",
        "--json",
    ]);
    let missing_predict = run_err(&[
        "dynamicjepa",
        "predict",
        "--db",
        &missing_db,
        "--artifact-id",
        "00000000-0000-0000-0000-000000000123",
        "--panel-id",
        &input_panel_id,
        "--action-id",
        &action_id,
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
    assert_eq!(missing_predict["error_code"], "SOURCE_OF_TRUTH_MISSING");
    assert_eq!(delta(&missing_before, &missing_after, "dj_predictions"), 0);
    assert_eq!(delta(&missing_before, &missing_after, "dj_audit_log"), 0);
    evidence.push(json!({"step": "edge_missing_artifact_predict", "before": missing_before, "error": missing_predict, "after": missing_after}));

    let corrupt_before = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    corrupt_model_file(&artifact_root);
    let corrupt_predict = run_err(&[
        "dynamicjepa",
        "predict",
        "--db",
        &db,
        "--artifact-id",
        &artifact_id,
        "--panel-id",
        &high_panel_id,
        "--action-id",
        &action_id,
        "--json",
    ]);
    let corrupt_after = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(corrupt_predict["error_code"], "ARTIFACT_HASH_MISMATCH");
    assert_eq!(corrupt_predict["wrote_prediction"], false);
    assert_eq!(delta(&corrupt_before, &corrupt_after, "dj_predictions"), 0);
    assert_eq!(delta(&corrupt_before, &corrupt_after, "dj_audit_log"), 0);
    evidence.push(json!({"step": "edge_corrupt_artifact_predict", "before": corrupt_before, "error": corrupt_predict, "after": corrupt_after}));

    let corrupt_plan_before = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    let corrupt_plan = run_err(&[
        "dynamicjepa",
        "plan",
        "--db",
        &db,
        "--artifact-id",
        &artifact_id,
        "--panel-id",
        &input_panel_id,
        "--skill-id",
        skill_id,
        "--json",
    ]);
    let corrupt_plan_after = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(corrupt_plan["error_code"], "ARTIFACT_HASH_MISMATCH");
    assert_eq!(corrupt_plan["wrote_plan_trace"], false);
    assert_eq!(
        delta(&corrupt_plan_before, &corrupt_plan_after, "dj_actions"),
        0
    );
    assert_eq!(
        delta(&corrupt_plan_before, &corrupt_plan_after, "dj_predictions"),
        0
    );
    assert_eq!(
        delta(
            &corrupt_plan_before,
            &corrupt_plan_after,
            "dj_guard_decisions"
        ),
        0
    );
    assert_eq!(
        delta(&corrupt_plan_before, &corrupt_plan_after, "dj_plan_traces"),
        0
    );
    assert_eq!(
        delta(&corrupt_plan_before, &corrupt_plan_after, "dj_audit_log"),
        0
    );
    evidence.push(json!({"step": "edge_corrupt_artifact_plan", "before": corrupt_plan_before, "error": corrupt_plan, "after": corrupt_plan_after}));

    let corrupt_surprise_before = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    let corrupt_surprise = run_err(&[
        "dynamicjepa",
        "record-surprise",
        "--db",
        &db,
        "--prediction-id",
        &prediction_id,
        "--observed-outcome-id",
        &target_outcome_id,
        "--observed-panel-id",
        &target_panel_id,
        "--json",
    ]);
    let corrupt_surprise_after = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    assert_eq!(corrupt_surprise["error_code"], "ARTIFACT_HASH_MISMATCH");
    assert_eq!(corrupt_surprise["wrote_surprise"], false);
    assert_eq!(
        delta(
            &corrupt_surprise_before,
            &corrupt_surprise_after,
            "dj_surprise_events"
        ),
        0
    );
    assert_eq!(
        delta(
            &corrupt_surprise_before,
            &corrupt_surprise_after,
            "dj_audit_log"
        ),
        0
    );
    evidence.push(json!({"step": "edge_corrupt_artifact_record_surprise", "before": corrupt_surprise_before, "error": corrupt_surprise, "after": corrupt_surprise_after}));

    let final_counts = run_ok(&["dynamicjepa", "inspect-counts", "--db", &db, "--json"]);
    let physical_count_cfs = [
        "dj_actions",
        "dj_predictions",
        "dj_skill_policies",
        "dj_guard_decisions",
        "dj_plan_traces",
        "dj_surprise_events",
        "dj_audit_log",
        "dj_model_artifacts",
        "dj_latent_panels",
        "dj_outcomes",
    ];
    let mut physical_counts = Map::new();
    for cf in physical_count_cfs {
        let physical_count = ldb_count(&db, cf);
        assert_eq!(
            physical_count,
            count(&final_counts, cf),
            "physical ldb count for {cf} must match inspect-counts"
        );
        physical_counts.insert(cf.to_string(), json!(physical_count));
    }
    evidence.push(json!({
        "step": "physical_ldb_count_proof",
        "inspect_counts": final_counts,
        "ldb_counts": physical_counts
    }));

    fs::write(
        evidence_dir.join("cli_phase8_fsv.json"),
        serde_json::to_vec_pretty(&json!({
            "source_of_truth": {
                "physical_dbs": {
                    "happy_and_edges": db_path,
                    "missing_artifact_edge": missing_db_path
                },
                "artifact_root": artifact_root,
                "dynamicjepa_cfs": [
                    "dj_actions",
                    "dj_predictions",
                    "dj_skill_policies",
                    "dj_guard_decisions",
                    "dj_plan_traces",
                    "dj_surprise_events",
                    "dj_audit_log"
                ],
                "read_paths": [
                    "dynamicjepa get-prediction",
                    "dynamicjepa get-plan-trace --include-predictions --include-guards",
                    "dynamicjepa get-surprise",
                    "dynamicjepa inspect-counts",
                    "dynamicjepa inspect-cf",
                    "dynamicjepa inspect-cf --key-hex",
                    "ldb dump --count_only"
                ]
            },
            "known_inputs": {
                "input_panel_id": input_panel_id,
                "action_id": action_id,
                "target_panel_id": target_panel_id,
                "target_outcome_id": target_outcome_id,
                "high_panel_id": high_panel_id,
                "high_outcome_id": high_outcome_id
            },
            "steps": evidence
        }))
        .expect("serialize evidence"),
    )
    .expect("write Phase 8 evidence log");
}
