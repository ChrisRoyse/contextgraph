//! BlackBoxFix Phase 19 executable Full State Verification protocol tests.
//!
//! These tests execute the real CLI against a real SQLite ledger and real
//! package/artifact files. Assertions inspect SQLite and the filesystem
//! directly; no database, filesystem, or command source is mocked.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase19_test";
const AGENT: &str = "agent_phase19_test";

struct Phase19Case {
    _temp: TempDir,
    root: PathBuf,
    ledger: PathBuf,
    run_id: String,
    pre_state: Value,
    post_state: Value,
    delta: Value,
    log_path: PathBuf,
}

fn context_graph_bin() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_context-graph") {
        return PathBuf::from(path);
    }
    let current = std::env::current_exe().expect("current test executable path");
    current
        .parent()
        .and_then(|deps| deps.parent())
        .map(|target_debug| target_debug.join("context-graph"))
        .expect("target/debug path")
}

fn run_context_graph(args: Vec<String>) -> Output {
    Command::new(context_graph_bin())
        .args(args)
        .output()
        .expect("spawn context-graph binary")
}

fn stdout_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "command failed\nstatus={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("stdout JSON")
}

fn stderr_json(output: &Output) -> Value {
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stderr).expect("stderr JSON")
}

fn write_json(path: &Path, value: &Value) {
    fs::write(
        path,
        serde_json::to_vec_pretty(value).expect("serialize JSON"),
    )
    .expect("write JSON file");
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read JSON")).expect("parse JSON")
}

fn sha256_file(path: &Path) -> String {
    let bytes = fs::read(path).expect("read file for sha256");
    let digest = Sha256::digest(&bytes);
    format!("sha256:{digest:x}")
}

fn file_bytes(path: &Path) -> u64 {
    fs::metadata(path).expect("stat file").len()
}

fn source_of_truth(ledger: &Path) -> String {
    format!("sqlite:{}#ledger_records", ledger.display())
}

fn kind_counts(ledger: &Path) -> BTreeMap<String, i64> {
    let conn = Connection::open(ledger).expect("open ledger");
    let mut stmt = conn
        .prepare("SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind")
        .expect("prepare kind counts");
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("query kind counts");
    rows.map(|row| row.expect("decode kind count")).collect()
}

fn latest_payload(ledger: &Path, record_kind: &str) -> Value {
    let conn = Connection::open(ledger).expect("open ledger");
    let payload: String = conn
        .query_row(
            "SELECT payload_json FROM ledger_records
             WHERE record_kind = ?1
             ORDER BY sequence DESC LIMIT 1",
            params![record_kind],
            |row| row.get(0),
        )
        .expect("latest payload");
    serde_json::from_str(&payload).expect("payload JSON")
}

fn kind_count(ledger: &Path, record_kind: &str) -> i64 {
    let conn = Connection::open(ledger).expect("open ledger");
    conn.query_row(
        "SELECT COUNT(*) FROM ledger_records WHERE record_kind = ?1",
        params![record_kind],
        |row| row.get(0),
    )
    .expect("kind count")
}

fn ledger_state(ledger: &Path) -> Value {
    let conn = Connection::open(ledger).expect("open ledger");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM ledger_records", [], |row| row.get(0))
        .expect("record count");
    let tip: String = conn
        .query_row(
            "SELECT value FROM blackbox_metadata WHERE key = 'ledger_tip_sha256'",
            [],
            |row| row.get(0),
        )
        .expect("ledger tip");
    let mut object = Map::new();
    object.insert("ledger_record_count".to_string(), json!(count));
    object.insert("record_kind_counts".to_string(), json!(kind_counts(ledger)));
    if !tip.is_empty() {
        object.insert("ledger_tip_sha256".to_string(), json!(tip));
    }
    Value::Object(object)
}

fn delta(pre: &Value, post: &Value) -> Value {
    let pre_count = pre["ledger_record_count"].as_i64().unwrap();
    let post_count = post["ledger_record_count"].as_i64().unwrap();
    let pre_kinds = pre["record_kind_counts"].as_object().unwrap();
    let post_kinds = post["record_kind_counts"].as_object().unwrap();
    let mut keys = std::collections::BTreeSet::new();
    for key in pre_kinds.keys() {
        keys.insert(key.clone());
    }
    for key in post_kinds.keys() {
        keys.insert(key.clone());
    }
    let mut kind_delta = Map::new();
    for key in keys {
        let pre_value = pre_kinds.get(&key).and_then(Value::as_i64).unwrap_or(0);
        let post_value = post_kinds.get(&key).and_then(Value::as_i64).unwrap_or(0);
        let diff = post_value - pre_value;
        if diff != 0 {
            kind_delta.insert(key, json!(diff));
        }
    }
    json!({
        "ledger_record_count": post_count - pre_count,
        "record_kind_counts": kind_delta,
    })
}

fn zero_delta() -> Value {
    json!({
        "ledger_record_count": 0,
        "record_kind_counts": {},
    })
}

fn edge_case(
    category: &str,
    name: &str,
    command: &str,
    exit_code: i32,
    error_code: &str,
    state: &Value,
) -> Value {
    json!({
        "category": category,
        "name": name,
        "command": command,
        "exit_code": exit_code,
        "error_code": error_code,
        "pre_state": state,
        "post_state": state,
        "expected_delta": zero_delta(),
        "actual_delta": zero_delta(),
    })
}

fn build_evidence(
    case: &Phase19Case,
    edges: Vec<Value>,
    artifact_sha256: String,
    post_state_override: Option<Value>,
) -> Value {
    let post_state = post_state_override.unwrap_or_else(|| case.post_state.clone());
    let observed_delta = if post_state == case.post_state {
        case.delta.clone()
    } else {
        delta(&case.pre_state, &post_state)
    };
    json!({
        "format": "blackbox_full_state_verification_evidence_json_v1",
        "phase": 19,
        "tenant_id": TENANT,
        "run_id": case.run_id,
        "source_of_truth": source_of_truth(&case.ledger),
        "trigger": {
            "command": "context-graph blackbox record run && context-graph blackbox record tool-call",
            "exit_code": 0,
            "stdout": "two append-only ledger rows were created and then read back with SQLite",
            "stderr": "",
        },
        "pre_state": case.pre_state,
        "post_state": post_state,
        "expected_delta": observed_delta,
        "actual_delta": observed_delta,
        "read_queries": [
            {
                "source_of_truth": source_of_truth(&case.ledger),
                "query": "SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind",
                "output": format!("{:?}", kind_counts(&case.ledger)),
            },
            {
                "source_of_truth": source_of_truth(&case.ledger),
                "query": "SELECT value FROM blackbox_metadata WHERE key='ledger_tip_sha256'",
                "output": case.post_state["ledger_tip_sha256"].as_str().unwrap_or(""),
            }
        ],
        "artifacts": [
            {
                "path": case.log_path,
                "sha256": artifact_sha256,
                "bytes": file_bytes(&case.log_path),
            }
        ],
        "edge_cases": edges,
    })
}

fn fsv_certify_args(ledger: &Path, evidence: &Path, out: &Path, run_id: &str) -> Vec<String> {
    vec![
        "blackbox".to_string(),
        "fsv".to_string(),
        "certify".to_string(),
        "--ledger".to_string(),
        ledger.display().to_string(),
        "--tenant".to_string(),
        TENANT.to_string(),
        "--phase".to_string(),
        "19".to_string(),
        "--run-id".to_string(),
        run_id.to_string(),
        "--evidence".to_string(),
        evidence.display().to_string(),
        "--out".to_string(),
        out.display().to_string(),
        "--agent".to_string(),
        AGENT.to_string(),
    ]
}

fn setup_case() -> Phase19Case {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();
    let ledger = root.join("blackbox.sqlite");
    let init = stdout_json(&run_context_graph(vec![
        "blackbox".to_string(),
        "init".to_string(),
        "--ledger".to_string(),
        ledger.display().to_string(),
    ]));
    assert_eq!(init["schema_version"], 13);
    let pre_state = ledger_state(&ledger);

    let run = stdout_json(&run_context_graph(vec![
        "blackbox".to_string(),
        "record".to_string(),
        "run".to_string(),
        "--ledger".to_string(),
        ledger.display().to_string(),
        "--tenant".to_string(),
        TENANT.to_string(),
        "--task".to_string(),
        "phase19 executable FSV protocol proof".to_string(),
        "--agent".to_string(),
        AGENT.to_string(),
        "--risk-tier".to_string(),
        "medium".to_string(),
    ]));
    let run_id = run["run_id"].as_str().unwrap().to_string();
    let input_path = root.join("tool-input.json");
    write_json(
        &input_path,
        &json!({
            "operation": "phase19_append_only_source_read",
            "expected": "one tool_call row",
        }),
    );
    let tool = stdout_json(&run_context_graph(vec![
        "blackbox".to_string(),
        "record".to_string(),
        "tool-call".to_string(),
        "--ledger".to_string(),
        ledger.display().to_string(),
        "--run-id".to_string(),
        run_id.clone(),
        "--tenant".to_string(),
        TENANT.to_string(),
        "--tool".to_string(),
        "sqlite3".to_string(),
        "--capability".to_string(),
        "local.sqlite.read".to_string(),
        "--input".to_string(),
        input_path.display().to_string(),
        "--agent".to_string(),
        AGENT.to_string(),
    ]));
    assert_eq!(tool["record_kind"], "tool_call");
    let post_state = ledger_state(&ledger);
    let delta = delta(&pre_state, &post_state);
    assert_eq!(delta["ledger_record_count"], 2);

    let log_path = root.join("fsv.log");
    fs::write(
        &log_path,
        format!(
            "SOURCE_OF_TRUTH={}\nPRE={}\nPOST={}\nDELTA={}\n",
            source_of_truth(&ledger),
            pre_state,
            post_state,
            delta
        ),
    )
    .expect("write FSV log");

    Phase19Case {
        _temp: temp,
        root,
        ledger,
        run_id,
        pre_state,
        post_state,
        delta,
        log_path,
    }
}

fn verify_manifest_files(out: &Path) {
    let manifest = read_json(&out.join("manifest.json"));
    assert_eq!(
        manifest["format"],
        "blackbox_full_state_verification_manifest_json_v1"
    );
    for file in manifest["files"].as_array().unwrap() {
        let rel = file["path"].as_str().unwrap();
        let path = out.join(rel);
        assert!(path.exists(), "manifest file missing: {}", path.display());
        assert_eq!(file["sha256"].as_str().unwrap(), sha256_file(&path));
        assert_eq!(file["bytes"].as_u64().unwrap(), file_bytes(&path));
    }
}

#[test]
fn phase19_fsv_protocol_certifies_physical_evidence_and_edges() {
    let case = setup_case();
    println!(
        "FSV19 source_of_truth={} initial_counts={:?}",
        source_of_truth(&case.ledger),
        kind_counts(&case.ledger)
    );

    let missing_out = case.root.join("missing-evidence-package");
    let missing_evidence = case.root.join("missing-evidence.json");
    let missing_command =
        fsv_certify_args(&case.ledger, &missing_evidence, &missing_out, &case.run_id).join(" ");
    let before_missing = ledger_state(&case.ledger);
    let missing_output = run_context_graph(fsv_certify_args(
        &case.ledger,
        &missing_evidence,
        &missing_out,
        &case.run_id,
    ));
    let missing_error = stderr_json(&missing_output);
    let after_missing = ledger_state(&case.ledger);
    assert_eq!(missing_error["error_code"], "BBX_IO_ERROR");
    assert_eq!(before_missing, after_missing);
    assert!(!missing_out.exists());
    assert_eq!(kind_count(&case.ledger, "full_state_verification"), 0);
    println!(
        "FSV19 missing-target BEFORE={} AFTER={} ERROR={}",
        before_missing, after_missing, missing_error
    );

    let mut wrong_post = case.post_state.clone();
    wrong_post["ledger_tip_sha256"] =
        json!("sha256:0000000000000000000000000000000000000000000000000000000000000000");
    let wrong_evidence = case.root.join("wrong-post-evidence.json");
    let wrong_edges = vec![
        edge_case(
            "missing_target_state",
            "missing evidence file",
            &missing_command,
            missing_output.status.code().unwrap_or(1),
            "BBX_IO_ERROR",
            &case.post_state,
        ),
        edge_case(
            "wrong_target_state",
            "post state tip mismatch",
            "context-graph blackbox fsv certify --evidence wrong-post-evidence.json",
            1,
            "FSV_POST_STATE_MISMATCH",
            &case.post_state,
        ),
        edge_case(
            "permission_or_schema_failure",
            "artifact hash mismatch",
            "context-graph blackbox fsv certify --evidence bad-artifact-evidence.json",
            1,
            "FSV_ARTIFACT_HASH_MISMATCH",
            &case.post_state,
        ),
    ];
    write_json(
        &wrong_evidence,
        &build_evidence(
            &case,
            wrong_edges.clone(),
            sha256_file(&case.log_path),
            Some(wrong_post),
        ),
    );
    let wrong_out = case.root.join("wrong-post-package");
    let before_wrong = ledger_state(&case.ledger);
    let wrong_output = run_context_graph(fsv_certify_args(
        &case.ledger,
        &wrong_evidence,
        &wrong_out,
        &case.run_id,
    ));
    let wrong_error = stderr_json(&wrong_output);
    let after_wrong = ledger_state(&case.ledger);
    assert_eq!(wrong_error["error_code"], "FSV_POST_STATE_MISMATCH");
    assert_eq!(before_wrong, after_wrong);
    assert!(!wrong_out.exists());
    assert_eq!(kind_count(&case.ledger, "full_state_verification"), 0);
    println!(
        "FSV19 wrong-target BEFORE={} AFTER={} ERROR={}",
        before_wrong, after_wrong, wrong_error
    );

    let bad_artifact_evidence = case.root.join("bad-artifact-evidence.json");
    write_json(
        &bad_artifact_evidence,
        &build_evidence(
            &case,
            wrong_edges,
            "sha256:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
            None,
        ),
    );
    let bad_artifact_out = case.root.join("bad-artifact-package");
    let before_bad_artifact = ledger_state(&case.ledger);
    let bad_artifact_output = run_context_graph(fsv_certify_args(
        &case.ledger,
        &bad_artifact_evidence,
        &bad_artifact_out,
        &case.run_id,
    ));
    let bad_artifact_error = stderr_json(&bad_artifact_output);
    let after_bad_artifact = ledger_state(&case.ledger);
    assert_eq!(
        bad_artifact_error["error_code"],
        "FSV_ARTIFACT_HASH_MISMATCH"
    );
    assert_eq!(before_bad_artifact, after_bad_artifact);
    assert!(!bad_artifact_out.exists());
    assert_eq!(kind_count(&case.ledger, "full_state_verification"), 0);
    println!(
        "FSV19 schema-failure BEFORE={} AFTER={} ERROR={}",
        before_bad_artifact, after_bad_artifact, bad_artifact_error
    );

    fs::write(
        &case.log_path,
        format!(
            "SOURCE_OF_TRUTH={}\nPRE={}\nPOST={}\nDELTA={}\nMISSING={}\nWRONG={}\nBAD_ARTIFACT={}\n",
            source_of_truth(&case.ledger),
            case.pre_state,
            case.post_state,
            case.delta,
            missing_error,
            wrong_error,
            bad_artifact_error,
        ),
    )
    .expect("rewrite FSV log with edge evidence");
    let final_edges = vec![
        edge_case(
            "missing_target_state",
            "missing evidence file",
            &missing_command,
            missing_output.status.code().unwrap_or(1),
            "BBX_IO_ERROR",
            &case.post_state,
        ),
        edge_case(
            "wrong_target_state",
            "post state tip mismatch",
            "context-graph blackbox fsv certify --evidence wrong-post-evidence.json",
            wrong_output.status.code().unwrap_or(1),
            "FSV_POST_STATE_MISMATCH",
            &case.post_state,
        ),
        edge_case(
            "permission_or_schema_failure",
            "artifact hash mismatch",
            "context-graph blackbox fsv certify --evidence bad-artifact-evidence.json",
            bad_artifact_output.status.code().unwrap_or(1),
            "FSV_ARTIFACT_HASH_MISMATCH",
            &case.post_state,
        ),
    ];
    let happy_evidence = case.root.join("happy-evidence.json");
    write_json(
        &happy_evidence,
        &build_evidence(&case, final_edges, sha256_file(&case.log_path), None),
    );
    let happy_out = case.root.join("happy-fsv-package");
    let before_happy = ledger_state(&case.ledger);
    let happy = stdout_json(&run_context_graph(fsv_certify_args(
        &case.ledger,
        &happy_evidence,
        &happy_out,
        &case.run_id,
    )));
    let after_happy = ledger_state(&case.ledger);
    assert_eq!(happy["status"], "ok");
    assert_eq!(happy["connector"], "blackbox_fsv_protocol_v1");
    assert_eq!(kind_count(&case.ledger, "full_state_verification"), 1);
    assert_eq!(
        after_happy["ledger_record_count"].as_i64().unwrap()
            - before_happy["ledger_record_count"].as_i64().unwrap(),
        1
    );
    verify_manifest_files(&happy_out);
    let payload = latest_payload(&case.ledger, "full_state_verification");
    assert_eq!(payload["status"], "passed");
    assert_eq!(payload["phase"], 19);
    assert_eq!(payload["fsv_protocol"]["edge_case_count"], 3);
    assert_eq!(payload["fsv_protocol"]["artifact_count"], 1);
    assert_eq!(
        payload["fsv_protocol"]["sqlite_integrity_check"],
        json!(["ok"])
    );
    println!(
        "FSV19 happy BEFORE={} AFTER={} STORED={}",
        before_happy, after_happy, payload
    );

    let connector = stdout_json(&run_context_graph(vec![
        "blackbox".to_string(),
        "connector".to_string(),
        "test".to_string(),
        "--connector".to_string(),
        "blackbox_fsv_protocol_v1".to_string(),
        "--source".to_string(),
        case.ledger.display().to_string(),
    ]));
    assert_eq!(
        connector["readback"]["full_state_verification_records_total"],
        1
    );
    assert_eq!(
        connector["readback"]["sqlite_integrity_check"],
        json!(["ok"])
    );

    let before_exists = ledger_state(&case.ledger);
    let exists_output = run_context_graph(fsv_certify_args(
        &case.ledger,
        &happy_evidence,
        &happy_out,
        &case.run_id,
    ));
    let exists_error = stderr_json(&exists_output);
    let after_exists = ledger_state(&case.ledger);
    assert_eq!(exists_error["error_code"], "BBX_OUTPUT_EXISTS");
    assert_eq!(before_exists, after_exists);
    assert_eq!(kind_count(&case.ledger, "full_state_verification"), 1);
    println!(
        "FSV19 output-exists BEFORE={} AFTER={} ERROR={}",
        before_exists, after_exists, exists_error
    );
}
