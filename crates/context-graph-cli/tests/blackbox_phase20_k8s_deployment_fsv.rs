//! BlackBoxFix Phase 20 Kubernetes Deployment snapshot FSV tests.
//!
//! These tests use physical kubectl-style JSON snapshot files and a real
//! SQLite ledger. They run the real CLI, then query the ledger directly to
//! prove verifier evidence was persisted.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase20_k8s_test";
const AGENT: &str = "agent_phase20_k8s_test";
const DEPLOYMENT: &str = "payments-api";
const NAMESPACE: &str = "prod";
const CONTAINER: &str = "api";
const IMAGE_REPOSITORY: &str = "registry.example.com/payments-api:v1.2.3";
const GOOD_DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BAD_DIGEST: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

struct Phase20Case {
    _temp: TempDir,
    root: PathBuf,
    ledger: PathBuf,
    run_id: String,
    tool_call_id: String,
    happy_snapshot: PathBuf,
    wrong_digest_snapshot: PathBuf,
    invalid_json_snapshot: PathBuf,
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

fn run_context_graph(args: &[&str]) -> Output {
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

fn init_ledger(ledger: &Path) {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "init",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
    ]));
}

fn record_run(ledger: &Path) -> Value {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "record",
        "run",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--task",
        "phase20 Kubernetes deployment source-of-truth test",
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--risk-tier",
        "high",
    ]))
}

fn record_tool_call(ledger: &Path, run_id: &str, input: &Path) -> Value {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "record",
        "tool-call",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--run-id",
        run_id,
        "--tool",
        "kubectl_apply_deployment",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--capability",
        "k8s.deployment.apply",
    ]))
}

fn deployment_snapshot(image_digest: &str) -> Value {
    json!({
        "apiVersion": "apps/v1",
        "kind": "DeploymentList",
        "metadata": {
            "resourceVersion": "4242"
        },
        "items": [{
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": {
                "name": DEPLOYMENT,
                "namespace": NAMESPACE,
                "uid": "11111111-2222-3333-4444-555555555555",
                "resourceVersion": "4243",
                "generation": 7
            },
            "spec": {
                "replicas": 3,
                "selector": {
                    "matchLabels": { "app": DEPLOYMENT }
                },
                "template": {
                    "metadata": {
                        "labels": { "app": DEPLOYMENT }
                    },
                    "spec": {
                        "containers": [{
                            "name": CONTAINER,
                            "image": format!("{IMAGE_REPOSITORY}@{image_digest}")
                        }]
                    }
                }
            },
            "status": {
                "observedGeneration": 7,
                "replicas": 3,
                "updatedReplicas": 3,
                "readyReplicas": 3,
                "availableReplicas": 3,
                "unavailableReplicas": 0,
                "conditions": [
                    {
                        "type": "Available",
                        "status": "True",
                        "reason": "MinimumReplicasAvailable",
                        "message": "Deployment has minimum availability."
                    },
                    {
                        "type": "Progressing",
                        "status": "True",
                        "reason": "NewReplicaSetAvailable",
                        "message": "ReplicaSet has successfully progressed."
                    }
                ]
            }
        }]
    })
}

fn new_phase20_case() -> Phase20Case {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();
    let ledger = root.join("blackbox.sqlite");
    let happy_snapshot = root.join("deployment-happy.json");
    let wrong_digest_snapshot = root.join("deployment-wrong-digest.json");
    let invalid_json_snapshot = root.join("deployment-invalid.json");

    write_json(&happy_snapshot, &deployment_snapshot(GOOD_DIGEST));
    write_json(&wrong_digest_snapshot, &deployment_snapshot(BAD_DIGEST));
    fs::write(&invalid_json_snapshot, "{not kubernetes json").expect("write invalid JSON");

    init_ledger(&ledger);
    let tool_input = root.join("tool-input.json");
    write_json(
        &tool_input,
        &json!({
            "operation": "kubectl apply deployment",
            "namespace": NAMESPACE,
            "deployment": DEPLOYMENT,
            "snapshot": happy_snapshot,
        }),
    );
    let run = record_run(&ledger);
    let run_id = run["run_id"].as_str().expect("run id").to_string();
    let tool = record_tool_call(&ledger, &run_id, &tool_input);
    let tool_call_id = tool["tool_call_id"]
        .as_str()
        .expect("tool call id")
        .to_string();

    Phase20Case {
        _temp: temp,
        root,
        ledger,
        run_id,
        tool_call_id,
        happy_snapshot,
        wrong_digest_snapshot,
        invalid_json_snapshot,
    }
}

fn verifier_spec(case: &Phase20Case, verifier_id: &str, source: Value, expected: Value) -> Value {
    json!({
        "verifier_id": verifier_id,
        "run_id": case.run_id,
        "tool_call_id": case.tool_call_id,
        "source": source,
        "expected": expected
    })
}

fn k8s_source(path: &Path) -> Value {
    json!({
        "snapshot": path,
        "snapshot_format": "kubectl-deployment-json"
    })
}

fn deployment_expected() -> Value {
    json!({
        "namespace": NAMESPACE,
        "deployment": DEPLOYMENT,
    })
}

fn run_verify(case: &Phase20Case, spec: Value) -> Output {
    let spec_path = case.root.join("verify-spec.json");
    write_json(&spec_path, &spec);
    run_context_graph(&[
        "blackbox",
        "verify",
        "--ledger",
        case.ledger.to_str().expect("ledger path UTF-8"),
        "--input",
        spec_path.to_str().expect("spec path UTF-8"),
        "--tenant",
        TENANT,
        "--agent",
        AGENT,
    ])
}

fn ledger_kind_counts(ledger: &Path) -> BTreeMap<String, i64> {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let mut stmt = conn
        .prepare(
            "SELECT record_kind, COUNT(*)
             FROM ledger_records
             GROUP BY record_kind
             ORDER BY record_kind",
        )
        .expect("prepare record kind count");
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("query record kind count");
    rows.map(|row| row.expect("record kind count row"))
        .collect()
}

fn latest_payload(ledger: &Path, record_kind: &str) -> Value {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let payload: String = conn
        .query_row(
            "SELECT payload_json
             FROM ledger_records
             WHERE record_kind = ?1
             ORDER BY sequence DESC
             LIMIT 1",
            params![record_kind],
            |row| row.get(0),
        )
        .expect("read latest ledger payload");
    serde_json::from_str(&payload).expect("ledger payload JSON")
}

fn kind_count(ledger: &Path, kind: &str) -> i64 {
    let conn = Connection::open(ledger).expect("open physical ledger");
    conn.query_row(
        "SELECT COUNT(*) FROM ledger_records WHERE record_kind = ?1",
        params![kind],
        |row| row.get(0),
    )
    .expect("kind count")
}

fn assert_edge(
    case: &Phase20Case,
    verifier_id: &str,
    source: Value,
    expected: Value,
    status: &str,
    error_code: &str,
    readback_delta: i64,
    incident_delta: i64,
) -> Value {
    let before = ledger_kind_counts(&case.ledger);
    println!("FSV20 EDGE {error_code} BEFORE={before:?}");
    let output = run_verify(case, verifier_spec(case, verifier_id, source, expected));
    assert_eq!(stderr_json(&output)["error_code"], error_code);
    let after = ledger_kind_counts(&case.ledger);
    println!("FSV20 EDGE {error_code} AFTER={after:?}");
    assert_eq!(
        after.get("expected_delta").copied().unwrap_or(0),
        before.get("expected_delta").copied().unwrap_or(0) + 1
    );
    assert_eq!(
        after.get("readback_snapshot").copied().unwrap_or(0),
        before.get("readback_snapshot").copied().unwrap_or(0) + readback_delta
    );
    assert_eq!(
        after.get("verifier_result").copied().unwrap_or(0),
        before.get("verifier_result").copied().unwrap_or(0) + 1
    );
    assert_eq!(
        after.get("evidence_packet").copied().unwrap_or(0),
        before.get("evidence_packet").copied().unwrap_or(0) + 1
    );
    assert_eq!(
        after.get("incident_packet").copied().unwrap_or(0),
        before.get("incident_packet").copied().unwrap_or(0) + incident_delta
    );
    let payload = latest_payload(&case.ledger, "verifier_result");
    assert_eq!(payload["verifier_id"], verifier_id);
    assert_eq!(payload["status"], status);
    assert_eq!(payload["error_code"], error_code);
    payload
}

#[test]
fn phase20_k8s_deployment_snapshot_verifies_physical_state_and_edges() {
    let case = new_phase20_case();
    println!(
        "FSV20 source_of_truth=sqlite:{}#ledger_records",
        case.ledger.display()
    );

    let list = stdout_json(&run_context_graph(&["blackbox", "connector", "list"]));
    assert!(list["connectors"]
        .as_array()
        .expect("connectors")
        .iter()
        .any(|item| item["connector_id"] == "k8s_deployment_snapshot_v1"));

    let connector = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "k8s_deployment_snapshot_v1",
        "--source",
        case.happy_snapshot.to_str().expect("snapshot path UTF-8"),
    ]));
    assert_eq!(connector["connector"], "k8s_deployment_snapshot_v1");
    assert_eq!(connector["readback"]["normalized"]["deployments_total"], 1);
    assert_eq!(
        connector["readback"]["normalized"]["deployments"][0]["metadata"]["name"],
        DEPLOYMENT
    );

    let before_happy = ledger_kind_counts(&case.ledger);
    println!("FSV20 HAPPY BEFORE={before_happy:?}");

    let image = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "k8s_deployment_image_digest_v1",
            k8s_source(&case.happy_snapshot),
            {
                let mut expected = deployment_expected();
                expected["container"] = json!(CONTAINER);
                expected["image_repository"] = json!(IMAGE_REPOSITORY);
                expected["image_digest"] = json!(GOOD_DIGEST);
                expected
            },
        ),
    ));
    assert_eq!(image["verifier_status"], "passed");
    assert_eq!(image["actual_readback"]["actual_image_digest"], GOOD_DIGEST);

    let replicas = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "k8s_replicas_match_v1",
            k8s_source(&case.happy_snapshot),
            {
                let mut expected = deployment_expected();
                expected["spec_replicas"] = json!(3);
                expected["status_replicas"] = json!(3);
                expected["updated_replicas"] = json!(3);
                expected["ready_replicas"] = json!(3);
                expected["available_replicas"] = json!(3);
                expected["unavailable_replicas"] = json!(0);
                expected
            },
        ),
    ));
    assert_eq!(replicas["verifier_status"], "passed");
    assert_eq!(
        replicas["actual_readback"]["actual"]["available_replicas"],
        3
    );

    let rollout = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "k8s_rollout_completed_v1",
            k8s_source(&case.happy_snapshot),
            {
                let mut expected = deployment_expected();
                expected["rollout_status"] = json!("completed");
                expected
            },
        ),
    ));
    assert_eq!(rollout["verifier_status"], "passed");
    assert_eq!(rollout["actual_readback"]["rollout_completed"], true);

    let after_happy = ledger_kind_counts(&case.ledger);
    println!("FSV20 HAPPY AFTER={after_happy:?}");
    assert_eq!(kind_count(&case.ledger, "agent_run"), 1);
    assert_eq!(kind_count(&case.ledger, "tool_call"), 1);
    assert_eq!(kind_count(&case.ledger, "expected_delta"), 3);
    assert_eq!(kind_count(&case.ledger, "readback_snapshot"), 3);
    assert_eq!(kind_count(&case.ledger, "verifier_result"), 3);
    assert_eq!(kind_count(&case.ledger, "evidence_packet"), 3);
    assert_eq!(kind_count(&case.ledger, "incident_packet"), 0);
    assert_eq!(
        latest_payload(&case.ledger, "readback_snapshot")["source_of_truth"]["kind"],
        "k8s_deployment_snapshot_v1"
    );

    let missing = assert_edge(
        &case,
        "k8s_deployment_image_digest_v1",
        k8s_source(&case.happy_snapshot),
        json!({
            "namespace": NAMESPACE,
            "deployment": "orders-api",
            "container": CONTAINER,
            "image_digest": GOOD_DIGEST
        }),
        "failed",
        "K8S_DEPLOYMENT_NOT_FOUND",
        1,
        1,
    );
    assert_eq!(missing["actual_readback"]["deployment_found"], false);

    let wrong_digest = assert_edge(
        &case,
        "k8s_deployment_image_digest_v1",
        k8s_source(&case.wrong_digest_snapshot),
        {
            let mut expected = deployment_expected();
            expected["container"] = json!(CONTAINER);
            expected["image_digest"] = json!(GOOD_DIGEST);
            expected
        },
        "failed",
        "K8S_IMAGE_DIGEST_MISMATCH",
        1,
        1,
    );
    assert_eq!(
        wrong_digest["actual_readback"]["actual_image_digest"],
        BAD_DIGEST
    );

    let invalid_json = assert_edge(
        &case,
        "k8s_rollout_completed_v1",
        k8s_source(&case.invalid_json_snapshot),
        {
            let mut expected = deployment_expected();
            expected["rollout_status"] = json!("completed");
            expected
        },
        "blocked",
        "K8S_SNAPSHOT_JSON_INVALID",
        1,
        0,
    );
    assert_eq!(invalid_json["status"], "blocked");

    let final_counts = ledger_kind_counts(&case.ledger);
    println!("FSV20 FINAL physical counts: {final_counts:?}");
    assert_eq!(final_counts.get("expected_delta").copied().unwrap_or(0), 6);
    assert_eq!(
        final_counts.get("readback_snapshot").copied().unwrap_or(0),
        6
    );
    assert_eq!(final_counts.get("verifier_result").copied().unwrap_or(0), 6);
    assert_eq!(final_counts.get("evidence_packet").copied().unwrap_or(0), 6);
    assert_eq!(final_counts.get("incident_packet").copied().unwrap_or(0), 2);
}
