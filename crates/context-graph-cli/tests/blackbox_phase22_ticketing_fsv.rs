//! BlackBoxFix Phase 22 ticketing source-of-truth FSV tests.
//!
//! These tests use physical Jira, Linear, and ServiceNow API JSON artifact
//! files plus a real SQLite ledger. They run the real CLI, then query SQLite
//! directly to prove verifier evidence and incidents were persisted.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase22_ticketing_test";
const AGENT: &str = "agent_phase22_ticketing_test";
const JIRA_KEY: &str = "BBX-22";
const LINEAR_IDENTIFIER: &str = "ENG-22";
const SERVICENOW_NUMBER: &str = "INC0000022";
const EVIDENCE_URL: &str = "https://evidence.example.com/bbx/phase22";

struct Phase22Case {
    _temp: TempDir,
    root: PathBuf,
    ledger: PathBuf,
    run_id: String,
    tool_call_id: String,
    jira_happy: PathBuf,
    jira_missing: PathBuf,
    linear_happy: PathBuf,
    linear_wrong_state: PathBuf,
    servicenow_happy: PathBuf,
    servicenow_invalid: PathBuf,
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
        "phase22 ticketing source-of-truth test",
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
        "ticketing_close_change_request",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--capability",
        "ticketing.change.close",
    ]))
}

fn jira_issue(key: &str, status: &str) -> Value {
    json!({
        "id": "10022",
        "key": key,
        "self": format!("https://example.atlassian.net/rest/api/3/issue/{key}"),
        "fields": {
            "summary": "Phase 22 ticketing verification",
            "status": {
                "id": "10000",
                "name": status
            },
            "updated": "2026-05-03T12:00:00.000+0000",
            "assignee": {
                "accountId": "jira-user-22",
                "displayName": "Phase 22 User"
            },
            "description": {
                "type": "doc",
                "version": 1,
                "content": [{
                    "type": "paragraph",
                    "content": [{
                        "type": "text",
                        "text": format!("Verified evidence packet {EVIDENCE_URL}")
                    }]
                }]
            }
        }
    })
}

fn linear_issue(identifier: &str, state: &str) -> Value {
    json!({
        "data": {
            "issue": {
                "id": "lin-phase22-issue",
                "identifier": identifier,
                "title": "Phase 22 ticketing verification",
                "state": {
                    "id": "state-phase22",
                    "name": state
                },
                "url": format!("https://linear.app/contextgraph/issue/{identifier}"),
                "updatedAt": "2026-05-03T12:00:00.000Z",
                "assignee": {
                    "id": "linear-user-22",
                    "name": "Phase 22 User"
                },
                "description": format!("Verified evidence packet {EVIDENCE_URL}")
            }
        }
    })
}

fn servicenow_record(number: &str, state_display: &str) -> Value {
    json!({
        "result": {
            "sys_id": "sn-phase22-record",
            "number": number,
            "short_description": "Phase 22 ticketing verification",
            "state": {
                "value": "6",
                "display_value": state_display
            },
            "sys_updated_on": "2026-05-03 12:00:00",
            "assigned_to": {
                "value": "servicenow-user-22",
                "display_value": "Phase 22 User"
            },
            "comments": format!("Verified evidence packet {EVIDENCE_URL}")
        }
    })
}

fn new_phase22_case() -> Phase22Case {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();
    let ledger = root.join("blackbox.sqlite");
    let jira_happy = root.join("jira-happy.json");
    let jira_missing = root.join("jira-missing.json");
    let linear_happy = root.join("linear-happy.json");
    let linear_wrong_state = root.join("linear-wrong-state.json");
    let servicenow_happy = root.join("servicenow-happy.json");
    let servicenow_invalid = root.join("servicenow-invalid.json");

    write_json(&jira_happy, &jira_issue(JIRA_KEY, "Done"));
    write_json(&jira_missing, &jira_issue("BBX-404", "Done"));
    write_json(&linear_happy, &linear_issue(LINEAR_IDENTIFIER, "Done"));
    write_json(
        &linear_wrong_state,
        &linear_issue(LINEAR_IDENTIFIER, "In Progress"),
    );
    write_json(
        &servicenow_happy,
        &servicenow_record(SERVICENOW_NUMBER, "Resolved"),
    );
    fs::write(&servicenow_invalid, "{not servicenow json").expect("write invalid JSON");

    init_ledger(&ledger);
    let tool_input = root.join("tool-input.json");
    write_json(
        &tool_input,
        &json!({
            "operation": "close customer-facing change request",
            "jira_ticket": jira_happy,
            "linear_ticket": linear_happy,
            "servicenow_ticket": servicenow_happy
        }),
    );
    let run = record_run(&ledger);
    let run_id = run["run_id"].as_str().expect("run id").to_string();
    let tool = record_tool_call(&ledger, &run_id, &tool_input);
    let tool_call_id = tool["tool_call_id"]
        .as_str()
        .expect("tool call id")
        .to_string();

    Phase22Case {
        _temp: temp,
        root,
        ledger,
        run_id,
        tool_call_id,
        jira_happy,
        jira_missing,
        linear_happy,
        linear_wrong_state,
        servicenow_happy,
        servicenow_invalid,
    }
}

fn verifier_spec(case: &Phase22Case, verifier_id: &str, source: Value, expected: Value) -> Value {
    json!({
        "verifier_id": verifier_id,
        "run_id": case.run_id,
        "tool_call_id": case.tool_call_id,
        "source": source,
        "expected": expected
    })
}

fn jira_source(path: &Path) -> Value {
    json!({
        "ticket": path,
        "ticket_format": "jira-rest-api-issue-json"
    })
}

fn linear_source(path: &Path) -> Value {
    json!({
        "ticket": path,
        "ticket_format": "linear-graphql-issue-json"
    })
}

fn servicenow_source(path: &Path) -> Value {
    json!({
        "ticket": path,
        "ticket_format": "servicenow-table-api-record-json"
    })
}

fn jira_expected() -> Value {
    json!({
        "issue_key": JIRA_KEY,
        "status": "Done",
        "evidence_url": EVIDENCE_URL
    })
}

fn linear_expected() -> Value {
    json!({
        "issue_identifier": LINEAR_IDENTIFIER,
        "state": "Done",
        "evidence_url": EVIDENCE_URL
    })
}

fn servicenow_expected() -> Value {
    json!({
        "number": SERVICENOW_NUMBER,
        "state": "Resolved",
        "evidence_url": EVIDENCE_URL
    })
}

fn run_verify(case: &Phase22Case, spec: Value) -> Output {
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
    case: &Phase22Case,
    verifier_id: &str,
    source: Value,
    expected: Value,
    status: &str,
    error_code: &str,
    readback_delta: i64,
    incident_delta: i64,
) -> Value {
    let before = ledger_kind_counts(&case.ledger);
    println!("FSV22 EDGE {error_code} BEFORE={before:?}");
    let output = run_verify(case, verifier_spec(case, verifier_id, source, expected));
    assert_eq!(stderr_json(&output)["error_code"], error_code);
    let after = ledger_kind_counts(&case.ledger);
    println!("FSV22 EDGE {error_code} AFTER={after:?}");
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
fn phase22_ticketing_artifacts_verify_physical_state_and_edges() {
    let case = new_phase22_case();
    println!(
        "FSV22 source_of_truth=sqlite:{}#ledger_records",
        case.ledger.display()
    );

    let list = stdout_json(&run_context_graph(&["blackbox", "connector", "list"]));
    assert!(list["connectors"]
        .as_array()
        .expect("connectors")
        .iter()
        .any(|item| item["connector_id"] == "ticketing_artifact_v1"));

    let connector = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "ticketing_artifact_v1",
        "--source",
        case.jira_happy.to_str().expect("Jira artifact path UTF-8"),
    ]));
    assert_eq!(connector["connector"], "ticketing_artifact_v1");
    assert_eq!(connector["readback"]["normalized"]["provider"], "jira");
    assert_eq!(connector["readback"]["normalized"]["tickets_total"], 1);
    assert_eq!(
        connector["readback"]["normalized"]["tickets"][0]["ticket_key"],
        JIRA_KEY
    );

    let before_happy = ledger_kind_counts(&case.ledger);
    println!("FSV22 HAPPY BEFORE={before_happy:?}");

    let jira = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "jira_ticket_state_v1",
            jira_source(&case.jira_happy),
            jira_expected(),
        ),
    ));
    assert_eq!(jira["verifier_status"], "passed");
    assert_eq!(jira["actual_readback"]["actual_status"], "Done");
    assert_eq!(jira["actual_readback"]["evidence_found"], true);

    let linear = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "linear_ticket_state_v1",
            linear_source(&case.linear_happy),
            linear_expected(),
        ),
    ));
    assert_eq!(linear["verifier_status"], "passed");
    assert_eq!(linear["actual_readback"]["actual_status"], "Done");
    assert_eq!(linear["actual_readback"]["evidence_found"], true);

    let servicenow = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "servicenow_ticket_state_v1",
            servicenow_source(&case.servicenow_happy),
            servicenow_expected(),
        ),
    ));
    assert_eq!(servicenow["verifier_status"], "passed");
    assert_eq!(servicenow["actual_readback"]["actual_status"], "Resolved");
    assert_eq!(servicenow["actual_readback"]["evidence_found"], true);

    let after_happy = ledger_kind_counts(&case.ledger);
    println!("FSV22 HAPPY AFTER={after_happy:?}");
    assert_eq!(kind_count(&case.ledger, "agent_run"), 1);
    assert_eq!(kind_count(&case.ledger, "tool_call"), 1);
    assert_eq!(kind_count(&case.ledger, "expected_delta"), 3);
    assert_eq!(kind_count(&case.ledger, "readback_snapshot"), 3);
    assert_eq!(kind_count(&case.ledger, "verifier_result"), 3);
    assert_eq!(kind_count(&case.ledger, "evidence_packet"), 3);
    assert_eq!(kind_count(&case.ledger, "incident_packet"), 0);
    assert_eq!(
        latest_payload(&case.ledger, "readback_snapshot")["source_of_truth"]["kind"],
        "ticketing_artifact_v1"
    );

    let missing = assert_edge(
        &case,
        "jira_ticket_state_v1",
        jira_source(&case.jira_missing),
        jira_expected(),
        "failed",
        "TICKETING_TICKET_NOT_FOUND",
        1,
        1,
    );
    assert_eq!(missing["actual_readback"]["ticket_found"], false);
    assert_eq!(
        missing["actual_readback"]["ticket_keys"],
        json!(["BBX-404"])
    );

    let wrong_state = assert_edge(
        &case,
        "linear_ticket_state_v1",
        linear_source(&case.linear_wrong_state),
        linear_expected(),
        "failed",
        "TICKETING_TICKET_STATUS_MISMATCH",
        1,
        1,
    );
    assert_eq!(
        wrong_state["actual_readback"]["actual_status"],
        "In Progress"
    );

    let invalid_json = assert_edge(
        &case,
        "servicenow_ticket_state_v1",
        servicenow_source(&case.servicenow_invalid),
        servicenow_expected(),
        "blocked",
        "TICKETING_ARTIFACT_JSON_INVALID",
        1,
        0,
    );
    assert_eq!(invalid_json["status"], "blocked");

    let final_counts = ledger_kind_counts(&case.ledger);
    println!("FSV22 FINAL physical counts: {final_counts:?}");
    assert_eq!(final_counts.get("expected_delta").copied().unwrap_or(0), 6);
    assert_eq!(
        final_counts.get("readback_snapshot").copied().unwrap_or(0),
        6
    );
    assert_eq!(final_counts.get("verifier_result").copied().unwrap_or(0), 6);
    assert_eq!(final_counts.get("evidence_packet").copied().unwrap_or(0), 6);
    assert_eq!(final_counts.get("incident_packet").copied().unwrap_or(0), 2);
}
