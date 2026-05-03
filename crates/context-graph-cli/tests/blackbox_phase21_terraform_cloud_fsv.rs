//! BlackBoxFix Phase 21 Terraform and cloud artifact FSV tests.
//!
//! These tests use physical Terraform JSON, cloud inventory JSON, cloud audit
//! JSONL artifacts, and a real SQLite ledger. They run the real CLI, then query
//! the ledger directly to prove verifier evidence was persisted.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase21_tf_cloud_test";
const AGENT: &str = "agent_phase21_tf_cloud_test";
const BUCKET_ADDRESS: &str = "aws_s3_bucket.audit_logs";
const BUCKET_NAME: &str = "phase21-audit-logs";
const BUCKET_ARN: &str = "arn:aws:s3:::phase21-audit-logs";
const REGION: &str = "us-east-1";
const PRINCIPAL_ID: &str = "arn:aws:iam::123456789012:role/contextgraph-phase21";
const AUDIT_ACTION: &str = "s3:PutBucketVersioning";

struct Phase21Case {
    _temp: TempDir,
    root: PathBuf,
    ledger: PathBuf,
    run_id: String,
    tool_call_id: String,
    plan_happy: PathBuf,
    plan_unapproved: PathBuf,
    state_happy: PathBuf,
    inventory_happy: PathBuf,
    audit_happy: PathBuf,
    audit_invalid: PathBuf,
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
        "phase21 Terraform and cloud source-of-truth test",
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
        "terraform_apply_cloud_resource",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--capability",
        "terraform.cloud.apply",
    ]))
}

fn tf_plan(include_unapproved_delete: bool) -> Value {
    let mut changes = vec![
        json!({
            "address": BUCKET_ADDRESS,
            "mode": "managed",
            "type": "aws_s3_bucket",
            "name": "audit_logs",
            "provider_name": "registry.terraform.io/hashicorp/aws",
            "change": {
                "actions": ["update"],
                "before": {
                    "bucket": BUCKET_NAME,
                    "versioning": [{ "enabled": false }]
                },
                "after": {
                    "bucket": BUCKET_NAME,
                    "versioning": [{ "enabled": true }]
                }
            }
        }),
        json!({
            "address": "aws_iam_role.contextgraph",
            "mode": "managed",
            "type": "aws_iam_role",
            "name": "contextgraph",
            "provider_name": "registry.terraform.io/hashicorp/aws",
            "change": {
                "actions": ["no-op"],
                "before": { "name": "contextgraph-phase21" },
                "after": { "name": "contextgraph-phase21" }
            }
        }),
    ];
    if include_unapproved_delete {
        changes.push(json!({
            "address": "aws_db_instance.prod",
            "mode": "managed",
            "type": "aws_db_instance",
            "name": "prod",
            "provider_name": "registry.terraform.io/hashicorp/aws",
            "change": {
                "actions": ["delete"],
                "before": { "identifier": "prod" },
                "after": null
            }
        }));
    }
    json!({
        "format_version": "1.0",
        "terraform_version": "1.7.5",
        "resource_changes": changes,
        "output_changes": {
            "bucket_name": {
                "actions": ["update"],
                "before": "old-phase21-audit-logs",
                "after": BUCKET_NAME
            }
        }
    })
}

fn tf_state() -> Value {
    json!({
        "format_version": "1.0",
        "terraform_version": "1.7.5",
        "values": {
            "root_module": {
                "resources": [
                    {
                        "address": BUCKET_ADDRESS,
                        "mode": "managed",
                        "type": "aws_s3_bucket",
                        "name": "audit_logs",
                        "provider_name": "registry.terraform.io/hashicorp/aws",
                        "values": {
                            "bucket": BUCKET_NAME,
                            "arn": BUCKET_ARN,
                            "versioning": [{ "enabled": true }],
                            "tags": { "Environment": "prod", "Owner": "contextgraph" }
                        },
                        "sensitive_values": {}
                    },
                    {
                        "address": "aws_iam_role.contextgraph",
                        "mode": "managed",
                        "type": "aws_iam_role",
                        "name": "contextgraph",
                        "provider_name": "registry.terraform.io/hashicorp/aws",
                        "values": {
                            "name": "contextgraph-phase21",
                            "arn": PRINCIPAL_ID
                        },
                        "sensitive_values": {}
                    }
                ]
            }
        }
    })
}

fn cloud_inventory() -> Value {
    json!({
        "format": "blackbox-cloud-inventory-json-v1",
        "provider": "aws",
        "generated_at": "2026-05-03T12:00:00Z",
        "resources": [{
            "provider": "aws",
            "resource_type": "aws_s3_bucket",
            "resource_id": BUCKET_ARN,
            "region": REGION,
            "attributes": {
                "bucket": BUCKET_NAME,
                "versioning": "enabled",
                "encryption": "aws:kms"
            }
        }]
    })
}

fn cloud_audit_event() -> Value {
    json!({
        "format": "blackbox-cloud-audit-event-json-v1",
        "provider": "aws",
        "event_id": "evt-phase21-0001",
        "principal_id": PRINCIPAL_ID,
        "action": AUDIT_ACTION,
        "resource_id": BUCKET_ARN,
        "event_time": "2026-05-03T12:03:00Z",
        "outcome": "success",
        "region": REGION
    })
}

fn new_phase21_case() -> Phase21Case {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();
    let ledger = root.join("blackbox.sqlite");
    let plan_happy = root.join("tfplan-happy.json");
    let plan_unapproved = root.join("tfplan-unapproved.json");
    let state_happy = root.join("tfstate-happy.json");
    let inventory_happy = root.join("cloud-inventory.json");
    let audit_happy = root.join("cloud-audit.jsonl");
    let audit_invalid = root.join("cloud-audit-invalid.jsonl");

    write_json(&plan_happy, &tf_plan(false));
    write_json(&plan_unapproved, &tf_plan(true));
    write_json(&state_happy, &tf_state());
    write_json(&inventory_happy, &cloud_inventory());
    fs::write(
        &audit_happy,
        format!(
            "{}\n",
            serde_json::to_string(&cloud_audit_event()).expect("audit event JSON")
        ),
    )
    .expect("write audit JSONL");
    fs::write(&audit_invalid, "{not valid audit json}\n").expect("write invalid audit JSONL");

    init_ledger(&ledger);
    let tool_input = root.join("tool-input.json");
    write_json(
        &tool_input,
        &json!({
            "operation": "terraform apply",
            "plan": plan_happy,
            "state": state_happy,
            "inventory": inventory_happy,
            "audit_log": audit_happy,
        }),
    );
    let run = record_run(&ledger);
    let run_id = run["run_id"].as_str().expect("run id").to_string();
    let tool = record_tool_call(&ledger, &run_id, &tool_input);
    let tool_call_id = tool["tool_call_id"]
        .as_str()
        .expect("tool call id")
        .to_string();

    Phase21Case {
        _temp: temp,
        root,
        ledger,
        run_id,
        tool_call_id,
        plan_happy,
        plan_unapproved,
        state_happy,
        inventory_happy,
        audit_happy,
        audit_invalid,
    }
}

fn verifier_spec(case: &Phase21Case, verifier_id: &str, source: Value, expected: Value) -> Value {
    json!({
        "verifier_id": verifier_id,
        "run_id": case.run_id,
        "tool_call_id": case.tool_call_id,
        "source": source,
        "expected": expected
    })
}

fn plan_source(path: &Path) -> Value {
    json!({
        "plan": path,
        "plan_format": "terraform-show-json"
    })
}

fn state_source(path: &Path) -> Value {
    json!({
        "state": path,
        "state_format": "terraform-show-json"
    })
}

fn inventory_source(path: &Path) -> Value {
    json!({
        "inventory": path,
        "inventory_format": "blackbox-cloud-inventory-json-v1"
    })
}

fn audit_source(path: &Path) -> Value {
    json!({
        "audit_log": path,
        "audit_log_format": "blackbox-cloud-audit-jsonl-v1"
    })
}

fn plan_expected() -> Value {
    json!({
        "approved_changes": [{
            "address": BUCKET_ADDRESS,
            "actions": ["update"]
        }],
        "approved_output_changes": [{
            "name": "bucket_name",
            "actions": ["update"]
        }]
    })
}

fn cloud_resource_expected() -> Value {
    json!({
        "provider": "aws",
        "resource_type": "aws_s3_bucket",
        "resource_id": BUCKET_ARN,
        "region": REGION,
        "required_attributes": {
            "bucket": BUCKET_NAME,
            "versioning": "enabled"
        }
    })
}

fn audit_expected() -> Value {
    json!({
        "provider": "aws",
        "principal_id": PRINCIPAL_ID,
        "action": AUDIT_ACTION,
        "resource_id": BUCKET_ARN,
        "outcome": "success",
        "region": REGION
    })
}

fn run_verify(case: &Phase21Case, spec: Value) -> Output {
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
    case: &Phase21Case,
    verifier_id: &str,
    source: Value,
    expected: Value,
    status: &str,
    error_code: &str,
    readback_delta: i64,
    incident_delta: i64,
) -> Value {
    let before = ledger_kind_counts(&case.ledger);
    println!("FSV21 EDGE {error_code} BEFORE={before:?}");
    let output = run_verify(case, verifier_spec(case, verifier_id, source, expected));
    assert_eq!(stderr_json(&output)["error_code"], error_code);
    let after = ledger_kind_counts(&case.ledger);
    println!("FSV21 EDGE {error_code} AFTER={after:?}");
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
fn phase21_terraform_and_cloud_artifacts_verify_physical_state_and_edges() {
    let case = new_phase21_case();
    println!(
        "FSV21 source_of_truth=sqlite:{}#ledger_records",
        case.ledger.display()
    );

    let list = stdout_json(&run_context_graph(&["blackbox", "connector", "list"]));
    assert!(list["connectors"]
        .as_array()
        .expect("connectors")
        .iter()
        .any(|item| item["connector_id"] == "terraform_cloud_artifact_v1"));

    let connector = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "terraform_cloud_artifact_v1",
        "--source",
        case.plan_happy.to_str().expect("plan path UTF-8"),
    ]));
    assert_eq!(connector["connector"], "terraform_cloud_artifact_v1");
    assert_eq!(
        connector["readback"]["normalized"]["changed_resources_total"],
        1
    );
    assert_eq!(
        connector["readback"]["normalized"]["changed_outputs_total"],
        1
    );

    let before_happy = ledger_kind_counts(&case.ledger);
    println!("FSV21 HAPPY BEFORE={before_happy:?}");

    let plan = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "tf_plan_only_approved_changes_v1",
            plan_source(&case.plan_happy),
            plan_expected(),
        ),
    ));
    assert_eq!(plan["verifier_status"], "passed");
    assert_eq!(plan["actual_readback"]["unapproved_changes"], json!([]));

    let state = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "tf_state_attribute_v1",
            state_source(&case.state_happy),
            json!({
                "resource_address": BUCKET_ADDRESS,
                "attribute_path": ["versioning", 0, "enabled"],
                "expected_value": true
            }),
        ),
    ));
    assert_eq!(state["verifier_status"], "passed");
    assert_eq!(state["actual_readback"]["actual_value"], true);

    let cloud_resource = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "cloud_resource_present_v1",
            inventory_source(&case.inventory_happy),
            cloud_resource_expected(),
        ),
    ));
    assert_eq!(cloud_resource["verifier_status"], "passed");
    assert_eq!(
        cloud_resource["actual_readback"]["resource"]["resource_id"],
        BUCKET_ARN
    );

    let audit = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "cloud_audit_principal_action_v1",
            audit_source(&case.audit_happy),
            audit_expected(),
        ),
    ));
    assert_eq!(audit["verifier_status"], "passed");
    assert_eq!(audit["actual_readback"]["matched_count"], 1);

    let after_happy = ledger_kind_counts(&case.ledger);
    println!("FSV21 HAPPY AFTER={after_happy:?}");
    assert_eq!(kind_count(&case.ledger, "agent_run"), 1);
    assert_eq!(kind_count(&case.ledger, "tool_call"), 1);
    assert_eq!(kind_count(&case.ledger, "expected_delta"), 4);
    assert_eq!(kind_count(&case.ledger, "readback_snapshot"), 4);
    assert_eq!(kind_count(&case.ledger, "verifier_result"), 4);
    assert_eq!(kind_count(&case.ledger, "evidence_packet"), 4);
    assert_eq!(kind_count(&case.ledger, "incident_packet"), 0);
    assert_eq!(
        latest_payload(&case.ledger, "readback_snapshot")["source_of_truth"]["kind"],
        "terraform_cloud_artifact_v1"
    );

    let unapproved = assert_edge(
        &case,
        "tf_plan_only_approved_changes_v1",
        plan_source(&case.plan_unapproved),
        plan_expected(),
        "failed",
        "TF_PLAN_UNAPPROVED_CHANGES",
        1,
        1,
    );
    assert_eq!(
        unapproved["actual_readback"]["unapproved_changes"][0]["address"],
        "aws_db_instance.prod"
    );

    let state_mismatch = assert_edge(
        &case,
        "tf_state_attribute_v1",
        state_source(&case.state_happy),
        json!({
            "resource_address": BUCKET_ADDRESS,
            "attribute_path": ["versioning", 0, "enabled"],
            "expected_value": false
        }),
        "failed",
        "TF_STATE_ATTRIBUTE_MISMATCH",
        1,
        1,
    );
    assert_eq!(state_mismatch["actual_readback"]["actual_value"], true);

    let invalid_audit = assert_edge(
        &case,
        "cloud_audit_principal_action_v1",
        audit_source(&case.audit_invalid),
        audit_expected(),
        "blocked",
        "CLOUD_AUDIT_JSONL_INVALID",
        1,
        0,
    );
    assert_eq!(invalid_audit["status"], "blocked");

    let final_counts = ledger_kind_counts(&case.ledger);
    println!("FSV21 FINAL physical counts: {final_counts:?}");
    assert_eq!(final_counts.get("expected_delta").copied().unwrap_or(0), 7);
    assert_eq!(
        final_counts.get("readback_snapshot").copied().unwrap_or(0),
        7
    );
    assert_eq!(final_counts.get("verifier_result").copied().unwrap_or(0), 7);
    assert_eq!(final_counts.get("evidence_packet").copied().unwrap_or(0), 7);
    assert_eq!(final_counts.get("incident_packet").copied().unwrap_or(0), 2);
}
