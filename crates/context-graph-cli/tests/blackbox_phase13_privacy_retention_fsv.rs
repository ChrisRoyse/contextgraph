//! BlackBoxFix Phase 13 privacy/retention FSV tests.
//!
//! These tests execute the real `context-graph blackbox` binary against a real
//! SQLite ledger. Assertions inspect the physical source of truth directly:
//! `ledger_records` enterprise authorization, retention_decision, and
//! evidence_deletion rows.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase13_test";
const AGENT: &str = "agent_phase13_test";
const PRIVACY_PRINCIPAL: &str = "user_privacy";
const VIEWER_PRINCIPAL: &str = "user_viewer";
const SUBJECT: &str = "subject.phase13@example.test";

struct Phase13Case {
    _temp: TempDir,
    root: PathBuf,
    ledger: PathBuf,
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

fn sha256_text(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    format!("sha256:{digest:x}")
}

fn enterprise_config() -> Value {
    json!({
        "tenant_id": TENANT,
        "sso": {
            "protocol": "oidc",
            "issuer": "https://idp.phase13.example.test/tenant_phase13_test",
            "client_id": "blackboxfix-phase13",
            "jwks_sha256": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "required_claims": ["iss", "sub", "aud", "exp"]
        },
        "rbac": {
            "roles": [
                {
                    "role_id": "privacy_admin",
                    "permissions": ["privacy.review", "privacy.erase", "dashboard.read"]
                },
                {
                    "role_id": "viewer",
                    "permissions": ["dashboard.read"]
                }
            ]
        },
        "retention": {
            "default_class": "standard",
            "classes": [
                { "class_id": "standard", "retain_days": 365, "legal_hold": false },
                { "class_id": "incident", "retain_days": 2555, "legal_hold": true }
            ]
        },
        "deployment": {
            "mode": "private_vpc",
            "region": "us-test-1",
            "data_residency": "us",
            "kms_key_id": "kms_phase13_tenant_key_001",
            "siem_format": "rfc5424_jsonl"
        }
    })
}

fn enterprise_directory() -> Value {
    json!({
        "tenant_id": TENANT,
        "schemas": [
            "urn:ietf:params:scim:schemas:core:2.0:User",
            "urn:ietf:params:scim:schemas:core:2.0:Group"
        ],
        "users": [
            {
                "id": PRIVACY_PRINCIPAL,
                "userName": "privacy.phase13@example.test",
                "active": true,
                "groups": ["group_privacy"]
            },
            {
                "id": VIEWER_PRINCIPAL,
                "userName": "viewer.phase13@example.test",
                "active": true,
                "groups": ["group_viewers"]
            }
        ],
        "groups": [
            {
                "id": "group_privacy",
                "displayName": "Privacy",
                "roles": ["privacy_admin"]
            },
            {
                "id": "group_viewers",
                "displayName": "Viewers",
                "roles": ["viewer"]
            }
        ]
    })
}

fn new_case() -> Phase13Case {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();
    let ledger = root.join("blackbox.sqlite");
    let init = stdout_json(&run_context_graph(&[
        "blackbox",
        "init",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
    ]));
    assert_eq!(init["schema_version"], 13);

    let config_path = root.join("enterprise-config.json");
    let directory_path = root.join("enterprise-directory.json");
    write_json(&config_path, &enterprise_config());
    write_json(&directory_path, &enterprise_directory());
    stdout_json(&run_context_graph(&[
        "blackbox",
        "enterprise",
        "configure",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--input",
        config_path.to_str().expect("config path UTF-8"),
        "--tenant",
        TENANT,
        "--agent",
        AGENT,
    ]));
    stdout_json(&run_context_graph(&[
        "blackbox",
        "enterprise",
        "sync-directory",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--input",
        directory_path.to_str().expect("directory path UTF-8"),
        "--tenant",
        TENANT,
        "--agent",
        AGENT,
    ]));

    Phase13Case {
        _temp: temp,
        root,
        ledger,
    }
}

fn record_run(ledger: &Path, task: &str) -> String {
    let output = stdout_json(&run_context_graph(&[
        "blackbox",
        "record",
        "run",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--task",
        task,
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--risk-tier",
        "medium",
    ]));
    output["run_id"].as_str().expect("run_id").to_string()
}

fn record_tool_call(ledger: &Path, root: &Path, run_id: &str) -> String {
    let input = root.join(format!("{run_id}-tool-input.json"));
    write_json(
        &input,
        &json!({
            "operation": "phase13 incident fixture",
            "expected_effect": "create failed verifier evidence for privacy erasure lock"
        }),
    );
    let output = stdout_json(&run_context_graph(&[
        "blackbox",
        "record",
        "tool-call",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--run-id",
        run_id,
        "--tool",
        "phase13_real_tool",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--capability",
        "phase13.real_tool",
    ]));
    output["tool_call_id"]
        .as_str()
        .expect("tool_call_id")
        .to_string()
}

fn verify_cmd_failure(ledger: &Path, root: &Path, run_id: &str, tool_call_id: &str) -> Output {
    let spec = root.join(format!("{run_id}-{tool_call_id}-cmd-fail.json"));
    write_json(
        &spec,
        &json!({
            "verifier_id": "cmd_exit_zero_v1",
            "run_id": run_id,
            "tool_call_id": tool_call_id,
            "source": { "cwd": root },
            "expected": {
                "command": ["sh", "-c", "exit 42"],
                "artifacts": []
            }
        }),
    );
    run_context_graph(&[
        "blackbox",
        "verify",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--input",
        spec.to_str().expect("spec path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
    ])
}

fn evaluate_retention(
    ledger: &Path,
    run_id: &str,
    principal: &str,
    retention_class: &str,
    data_subject: &str,
    purpose: &str,
) -> Output {
    run_context_graph(&[
        "blackbox",
        "privacy",
        "evaluate-retention",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--tenant",
        TENANT,
        "--run-id",
        run_id,
        "--principal-id",
        principal,
        "--retention-class",
        retention_class,
        "--data-subject",
        data_subject,
        "--purpose",
        purpose,
        "--agent",
        AGENT,
    ])
}

fn request_erasure(
    ledger: &Path,
    run_id: &str,
    principal: &str,
    retention_class: &str,
    data_subject: &str,
    reason: &str,
) -> Output {
    run_context_graph(&[
        "blackbox",
        "privacy",
        "request-erasure",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--tenant",
        TENANT,
        "--run-id",
        run_id,
        "--principal-id",
        principal,
        "--retention-class",
        retention_class,
        "--data-subject",
        data_subject,
        "--reason",
        reason,
        "--agent",
        AGENT,
    ])
}

fn kind_counts(ledger: &Path) -> BTreeMap<String, i64> {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let mut stmt = conn
        .prepare(
            "SELECT record_kind, COUNT(*)
             FROM ledger_records
             GROUP BY record_kind
             ORDER BY record_kind",
        )
        .expect("prepare kind counts");
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("query kind counts");
    rows.map(|row| row.expect("kind count row")).collect()
}

fn record_kind_count(ledger: &Path, record_kind: &str) -> i64 {
    let conn = Connection::open(ledger).expect("open physical ledger");
    conn.query_row(
        "SELECT COUNT(*) FROM ledger_records WHERE record_kind = ?1",
        params![record_kind],
        |row| row.get(0),
    )
    .expect("count record kind")
}

fn latest_payload_for_run(ledger: &Path, record_kind: &str, run_id: &str) -> Value {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let payload: String = conn
        .query_row(
            "SELECT payload_json
             FROM ledger_records
             WHERE record_kind = ?1 AND run_id = ?2
             ORDER BY sequence DESC
             LIMIT 1",
            params![record_kind, run_id],
            |row| row.get(0),
        )
        .expect("read latest payload");
    serde_json::from_str(&payload).expect("payload JSON")
}

fn all_payload_text(ledger: &Path) -> String {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let mut stmt = conn
        .prepare("SELECT payload_json FROM ledger_records ORDER BY sequence")
        .expect("prepare payload scan");
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query payloads");
    rows.map(|row| row.expect("payload row"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn phase13_privacy_retention_persists_happy_paths_and_edges() {
    let case = new_case();
    println!(
        "FSV13 source_of_truth=sqlite:{}#ledger_records",
        case.ledger.display()
    );
    println!("FSV13 fixture_root={}", case.root.display());
    println!("FSV13 BEFORE counts: {:?}", kind_counts(&case.ledger));

    let retain_run = record_run(&case.ledger, "phase13 retention happy path");
    let retain = stdout_json(&evaluate_retention(
        &case.ledger,
        &retain_run,
        PRIVACY_PRINCIPAL,
        "standard",
        SUBJECT,
        "privacy-review-case-001",
    ));
    let retain_stored = latest_payload_for_run(&case.ledger, "retention_decision", &retain_run);
    println!("FSV13 retention AFTER stored={}", retain_stored);
    assert_eq!(retain["readback"]["status"], "passed");
    assert_eq!(
        retain_stored["privacy_decision_id"],
        retain["privacy_decision_id"]
    );
    assert_eq!(retain_stored["decision"]["status"], "retained");
    assert_eq!(
        retain_stored["request"]["data_subject_sha256"],
        sha256_text(SUBJECT)
    );
    assert_eq!(
        retain_stored["request"]["data_subject_storage"],
        "sha256_only_raw_value_not_persisted"
    );
    assert_eq!(retain_stored["retention_policy"]["retain_days"], 365);

    let approved_erasure_run = record_run(&case.ledger, "phase13 approved erasure path");
    let approved = stdout_json(&request_erasure(
        &case.ledger,
        &approved_erasure_run,
        PRIVACY_PRINCIPAL,
        "standard",
        SUBJECT,
        "privacy-erasure-case-002",
    ));
    let approved_stored =
        latest_payload_for_run(&case.ledger, "evidence_deletion", &approved_erasure_run);
    println!("FSV13 approved-erasure AFTER stored={}", approved_stored);
    assert_eq!(approved["readback"]["status"], "passed");
    assert_eq!(approved_stored["decision"]["status"], "approved");
    assert_eq!(
        approved_stored["decision"]["reason_code"],
        "ERASURE_APPROVED_BY_POLICY"
    );
    assert_eq!(approved_stored["decision"]["deletes_ledger_rows"], false);

    let legal_hold_run = record_run(&case.ledger, "phase13 legal hold denial edge");
    let legal_hold_before = kind_counts(&case.ledger);
    let legal_hold = stdout_json(&request_erasure(
        &case.ledger,
        &legal_hold_run,
        PRIVACY_PRINCIPAL,
        "incident",
        SUBJECT,
        "privacy-erasure-case-003",
    ));
    let legal_hold_stored =
        latest_payload_for_run(&case.ledger, "evidence_deletion", &legal_hold_run);
    println!(
        "FSV13 legal-hold BEFORE counts={:?} AFTER stored={}",
        legal_hold_before, legal_hold_stored
    );
    assert_eq!(legal_hold["readback"]["status"], "passed");
    assert_eq!(legal_hold_stored["decision"]["status"], "denied");
    assert_eq!(
        legal_hold_stored["decision"]["reason_code"],
        "LEGAL_HOLD_ACTIVE"
    );
    assert_eq!(legal_hold_stored["retention_policy"]["legal_hold"], true);

    let incident_run = record_run(&case.ledger, "phase13 incident erasure denial edge");
    let incident_tool_call = record_tool_call(&case.ledger, &case.root, &incident_run);
    let incident_verify_error = stderr_json(&verify_cmd_failure(
        &case.ledger,
        &case.root,
        &incident_run,
        &incident_tool_call,
    ));
    assert_eq!(incident_verify_error["error_code"], "CMD_EXIT_NON_ZERO");
    let incident_before = kind_counts(&case.ledger);
    let incident = stdout_json(&request_erasure(
        &case.ledger,
        &incident_run,
        PRIVACY_PRINCIPAL,
        "standard",
        SUBJECT,
        "privacy-erasure-case-004",
    ));
    let incident_stored = latest_payload_for_run(&case.ledger, "evidence_deletion", &incident_run);
    println!(
        "FSV13 incident BEFORE counts={:?} VERIFY error={} AFTER stored={}",
        incident_before, incident_verify_error, incident_stored
    );
    assert_eq!(incident["readback"]["status"], "passed");
    assert_eq!(incident_stored["decision"]["status"], "denied");
    assert_eq!(
        incident_stored["decision"]["reason_code"],
        "INCIDENT_EVIDENCE_LOCKED"
    );
    assert_eq!(incident_stored["decision"]["incident_packets_total"], 1);

    let missing_class_run = record_run(&case.ledger, "phase13 missing retention class edge");
    let missing_before_privacy = record_kind_count(&case.ledger, "retention_decision");
    let missing_before_access = record_kind_count(&case.ledger, "enterprise_access_decision");
    let missing_error = stderr_json(&evaluate_retention(
        &case.ledger,
        &missing_class_run,
        PRIVACY_PRINCIPAL,
        "missing_class",
        SUBJECT,
        "privacy-review-case-005",
    ));
    let missing_after_privacy = record_kind_count(&case.ledger, "retention_decision");
    let missing_after_access = record_kind_count(&case.ledger, "enterprise_access_decision");
    println!(
        "FSV13 missing-class BEFORE privacy={} access={} AFTER privacy={} access={} error={}",
        missing_before_privacy,
        missing_before_access,
        missing_after_privacy,
        missing_after_access,
        missing_error
    );
    assert_eq!(
        missing_error["error_code"],
        "PRIVACY_RETENTION_CLASS_NOT_FOUND"
    );
    assert_eq!(missing_after_privacy, missing_before_privacy);
    assert_eq!(missing_after_access, missing_before_access + 1);

    let unauthorized_run = record_run(&case.ledger, "phase13 unauthorized erasure edge");
    let unauthorized_before_deletions = record_kind_count(&case.ledger, "evidence_deletion");
    let unauthorized_before_access = record_kind_count(&case.ledger, "enterprise_access_decision");
    let unauthorized_error = stderr_json(&request_erasure(
        &case.ledger,
        &unauthorized_run,
        VIEWER_PRINCIPAL,
        "standard",
        SUBJECT,
        "privacy-erasure-case-006",
    ));
    let unauthorized_after_deletions = record_kind_count(&case.ledger, "evidence_deletion");
    let unauthorized_after_access = record_kind_count(&case.ledger, "enterprise_access_decision");
    println!(
        "FSV13 unauthorized BEFORE deletions={} access={} AFTER deletions={} access={} error={}",
        unauthorized_before_deletions,
        unauthorized_before_access,
        unauthorized_after_deletions,
        unauthorized_after_access,
        unauthorized_error
    );
    assert_eq!(unauthorized_error["error_code"], "PRIVACY_RBAC_DENIED");
    assert_eq!(unauthorized_after_deletions, unauthorized_before_deletions);
    assert_eq!(unauthorized_after_access, unauthorized_before_access + 1);

    let missing_run_before = kind_counts(&case.ledger);
    let missing_run_error = stderr_json(&request_erasure(
        &case.ledger,
        "run_missing_phase13",
        PRIVACY_PRINCIPAL,
        "standard",
        SUBJECT,
        "privacy-erasure-case-007",
    ));
    let missing_run_after = kind_counts(&case.ledger);
    println!(
        "FSV13 missing-run BEFORE counts={:?} AFTER counts={:?} error={}",
        missing_run_before, missing_run_after, missing_run_error
    );
    assert_eq!(missing_run_error["error_code"], "PRIVACY_RUN_NOT_FOUND");
    assert_eq!(missing_run_after, missing_run_before);

    let empty_subject_before = kind_counts(&case.ledger);
    let empty_subject_error = stderr_json(&evaluate_retention(
        &case.ledger,
        &retain_run,
        PRIVACY_PRINCIPAL,
        "standard",
        "",
        "privacy-review-case-008",
    ));
    let empty_subject_after = kind_counts(&case.ledger);
    println!(
        "FSV13 empty-subject BEFORE counts={:?} AFTER counts={:?} error={}",
        empty_subject_before, empty_subject_after, empty_subject_error
    );
    assert_eq!(empty_subject_error["error_code"], "PRIVACY_REQUEST_INVALID");
    assert_eq!(empty_subject_after, empty_subject_before);

    let payload_text = all_payload_text(&case.ledger);
    assert!(
        !payload_text.contains(SUBJECT),
        "raw data subject must not be persisted in ledger payload_json"
    );
    assert!(payload_text.contains(&sha256_text(SUBJECT)));

    let connector = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "blackbox_privacy_v1",
        "--source",
        case.ledger.to_str().expect("ledger path UTF-8"),
    ]));
    assert_eq!(connector["connector"], "blackbox_privacy_v1");
    assert_eq!(
        connector["readback"]["retention_decisions_total"],
        record_kind_count(&case.ledger, "retention_decision")
    );
    assert_eq!(
        connector["readback"]["evidence_deletions_total"],
        record_kind_count(&case.ledger, "evidence_deletion")
    );

    let final_counts = kind_counts(&case.ledger);
    println!("FSV13 FINAL physical counts: {:?}", final_counts);
    assert_eq!(final_counts.get("enterprise_tenant_config"), Some(&1));
    assert_eq!(final_counts.get("enterprise_directory_snapshot"), Some(&1));
    assert_eq!(final_counts.get("agent_run"), Some(&6));
    assert_eq!(final_counts.get("tool_call"), Some(&1));
    assert_eq!(final_counts.get("expected_delta"), Some(&1));
    assert_eq!(final_counts.get("readback_snapshot"), Some(&1));
    assert_eq!(final_counts.get("verifier_result"), Some(&1));
    assert_eq!(final_counts.get("evidence_packet"), Some(&1));
    assert_eq!(final_counts.get("incident_packet"), Some(&1));
    assert_eq!(final_counts.get("enterprise_access_decision"), Some(&6));
    assert_eq!(final_counts.get("retention_decision"), Some(&1));
    assert_eq!(final_counts.get("evidence_deletion"), Some(&3));
}
