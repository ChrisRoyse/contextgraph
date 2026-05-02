//! BlackBoxFix Phase 1 FSV tests.
//!
//! These tests execute the real `context-graph blackbox` binary against real
//! temporary SQLite ledgers, then independently read the physical
//! `ledger_records` table as the source of truth.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::Value;
use tempfile::TempDir;

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

fn init_ledger(ledger: &Path) {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "init",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
    ]));
}

fn record_run(ledger: &Path, task: &str) -> Value {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "record",
        "run",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--task",
        task,
        "--agent",
        "agent_phase1_test",
        "--tenant",
        "tenant_phase1_test",
        "--risk-tier",
        "low",
    ]))
}

fn record_tool_call(ledger: &Path, run_id: &str, input: &Path) -> Output {
    run_context_graph(&[
        "blackbox",
        "record",
        "tool-call",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--run-id",
        run_id,
        "--tool",
        "phase1_real_tool",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        "agent_phase1_test",
        "--tenant",
        "tenant_phase1_test",
        "--capability",
        "phase1.real_tool.write",
    ])
}

fn ledger_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM ledger_records", [], |row| row.get(0))
        .expect("count ledger rows")
}

#[test]
fn phase1_happy_path_persists_and_inspects_physical_rows() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    let input = temp.path().join("tool-input.json");
    fs::write(
        &input,
        r#"{"file":"Cargo.toml","operation":"inspect","expected_delta":{"records_added":0},"api_key":"phase1-secret"}"#,
    )
    .expect("write real input file");

    init_ledger(&ledger);
    let run = record_run(&ledger, "phase1 happy path source-of-truth persistence");
    let run_id = run["run_id"].as_str().expect("run id");
    let tool = stdout_json(&record_tool_call(&ledger, run_id, &input));
    assert_eq!(tool["record_kind"], "tool_call");

    let inspect = stdout_json(&run_context_graph(&[
        "blackbox",
        "inspect",
        "ledger",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--run-id",
        run_id,
    ]));
    assert_eq!(inspect["integrity"]["status"], "passed");
    assert_eq!(inspect["records"].as_array().expect("records").len(), 2);

    let conn = Connection::open(&ledger).expect("open physical ledger");
    assert_eq!(ledger_count(&conn), 2);

    let (record_kind, payload_json, redaction_json, record_hash): (String, String, String, String) =
        conn.query_row(
            "SELECT record_kind, payload_json, redaction_json, record_sha256
             FROM ledger_records
             WHERE sequence = 2",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read physical tool-call row");
    assert_eq!(record_kind, "tool_call");
    assert!(record_hash.starts_with("sha256:"));

    let payload: Value = serde_json::from_str(&payload_json).expect("payload JSON");
    assert_eq!(payload["tool_name"], "phase1_real_tool");
    assert_eq!(payload["input_redacted"]["file"], "Cargo.toml");
    assert!(payload["input_redacted"]["api_key"]
        .as_str()
        .expect("redacted api_key")
        .starts_with("[REDACTED:sha256:"));
    assert_ne!(
        payload_json,
        fs::read_to_string(&input).expect("input readback")
    );

    let redaction: Value = serde_json::from_str(&redaction_json).expect("redaction JSON");
    assert_eq!(redaction["policy_id"], "redact_v1");
    assert_eq!(redaction["redacted_fields"][0], "$.api_key");
}

#[test]
fn phase1_redaction_violation_blocks_persistence() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    let input = temp.path().join("bad-tool-input.json");
    fs::write(
        &input,
        r#"{"note":"Bearer should-not-be-persisted","operation":"write"}"#,
    )
    .expect("write real input file");

    init_ledger(&ledger);
    let run = record_run(&ledger, "phase1 redaction failure source-of-truth test");
    let run_id = run["run_id"].as_str().expect("run id");
    let conn = Connection::open(&ledger).expect("open physical ledger");
    assert_eq!(ledger_count(&conn), 1);

    let output = record_tool_call(&ledger, run_id, &input);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "BBX_REDACTION_VIOLATION");
    assert_eq!(
        ledger_count(&conn),
        1,
        "no row may be inserted after redaction failure"
    );
}

#[test]
fn phase1_case_insensitive_secret_prefix_blocks_persistence() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    let input = temp.path().join("bad-tool-input.json");
    fs::write(
        &input,
        r#"{"note":"SK-SHOULD-NOT-BE-PERSISTED","operation":"write"}"#,
    )
    .expect("write real input file");

    init_ledger(&ledger);
    let run = record_run(
        &ledger,
        "phase1 case-insensitive secret source-of-truth test",
    );
    let run_id = run["run_id"].as_str().expect("run id");
    let conn = Connection::open(&ledger).expect("open physical ledger");
    assert_eq!(ledger_count(&conn), 1);

    let output = record_tool_call(&ledger, run_id, &input);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "BBX_REDACTION_VIOLATION");
    assert_eq!(error["field_path"], "$.note");
    assert_eq!(
        ledger_count(&conn),
        1,
        "no row may be inserted after redaction failure"
    );
}

#[test]
fn phase1_cross_tenant_tool_call_is_blocked() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    let input = temp.path().join("tool-input.json");
    fs::write(
        &input,
        r#"{"operation":"inspect","api_key":"phase1-secret"}"#,
    )
    .expect("write real input file");

    init_ledger(&ledger);
    let run = record_run(&ledger, "phase1 tenant isolation source-of-truth test");
    let run_id = run["run_id"].as_str().expect("run id");
    let conn = Connection::open(&ledger).expect("open physical ledger");
    assert_eq!(ledger_count(&conn), 1);

    let output = run_context_graph(&[
        "blackbox",
        "record",
        "tool-call",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--run-id",
        run_id,
        "--tool",
        "phase1_real_tool",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        "agent_phase1_test",
        "--tenant",
        "tenant_other",
        "--capability",
        "phase1.real_tool.write",
    ]);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "BBX_TENANT_MISMATCH");
    assert_eq!(error["field_path"], "ledger_records.tenant_id");
    assert_eq!(
        ledger_count(&conn),
        1,
        "cross-tenant tool-call must not be persisted"
    );
}

#[test]
fn phase1_missing_parent_run_blocks_tool_call() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    let input = temp.path().join("tool-input.json");
    fs::write(&input, r#"{"operation":"inspect"}"#).expect("write real input file");

    init_ledger(&ledger);
    let output = run_context_graph(&[
        "blackbox",
        "record",
        "tool-call",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--run-id",
        "run_missing",
        "--tool",
        "phase1_real_tool",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        "agent_phase1_test",
        "--tenant",
        "tenant_phase1_test",
        "--capability",
        "phase1.real_tool.write",
    ]);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "BBX_RECORD_NOT_FOUND");
    assert_eq!(error["field_path"], "--run-id");
    assert!(error["source_of_truth"]
        .as_str()
        .expect("source of truth")
        .contains("blackbox.sqlite#ledger_records"));

    let conn = Connection::open(&ledger).expect("open physical ledger");
    assert_eq!(
        ledger_count(&conn),
        0,
        "missing parent run must not create a tool-call row"
    );
}

#[test]
fn phase1_invalid_risk_tier_blocks_persistence() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    init_ledger(&ledger);

    let output = run_context_graph(&[
        "blackbox",
        "record",
        "run",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--task",
        "phase1 invalid risk tier source-of-truth test",
        "--agent",
        "agent_phase1_test",
        "--tenant",
        "tenant_phase1_test",
        "--risk-tier",
        "critical",
    ]);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "BBX_INVALID_INPUT");
    assert_eq!(error["field_path"], "--risk-tier");
    assert!(error["source_of_truth"]
        .as_str()
        .expect("source of truth")
        .contains("blackbox.sqlite#ledger_records"));

    let conn = Connection::open(&ledger).expect("open physical ledger");
    assert_eq!(
        ledger_count(&conn),
        0,
        "invalid run input must not create a ledger row"
    );
}

#[test]
fn phase1_hash_chain_detection_catches_payload_tampering() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    let input = temp.path().join("tool-input.json");
    fs::write(&input, r#"{"file":"Cargo.toml","operation":"inspect"}"#)
        .expect("write real input file");

    init_ledger(&ledger);
    let run = record_run(&ledger, "phase1 tamper detection source-of-truth test");
    let run_id = run["run_id"].as_str().expect("run id");
    stdout_json(&record_tool_call(&ledger, run_id, &input));

    let conn = Connection::open(&ledger).expect("open physical ledger");
    assert_eq!(ledger_count(&conn), 2);
    conn.execute("DROP TRIGGER ledger_records_no_update", [])
        .expect("drop update trigger to simulate privileged tamper");
    conn.execute(
        "UPDATE ledger_records SET payload_json = ?1 WHERE sequence = 2",
        params![r#"{"record_kind":"tool_call","tampered":true}"#],
    )
    .expect("tamper physical payload");
    conn.execute_batch(
        "CREATE TRIGGER ledger_records_no_update
             BEFORE UPDATE ON ledger_records
             BEGIN
                 SELECT RAISE(ABORT, 'ledger_records is append-only; updates are forbidden');
             END;",
    )
    .expect("restore update trigger");

    let output = run_context_graph(&[
        "blackbox",
        "inspect",
        "ledger",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
    ]);
    assert_eq!(output.status.code(), Some(2));
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "BBX_HASH_CHAIN_BROKEN");
}

#[test]
fn phase1_append_only_trigger_blocks_direct_update() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    init_ledger(&ledger);
    record_run(&ledger, "phase1 append-only trigger source-of-truth test");

    let conn = Connection::open(&ledger).expect("open physical ledger");
    let result = conn.execute(
        "UPDATE ledger_records SET record_kind = 'tampered' WHERE sequence = 1",
        [],
    );
    assert!(result.is_err(), "append-only trigger must reject UPDATE");

    let kind: String = conn
        .query_row(
            "SELECT record_kind FROM ledger_records WHERE sequence = 1",
            [],
            |row| row.get(0),
        )
        .expect("read physical row after blocked update");
    assert_eq!(kind, "agent_run");
}

#[test]
fn phase1_schema_validation_rejects_noop_append_only_trigger() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    init_ledger(&ledger);
    record_run(
        &ledger,
        "phase1 no-op trigger schema validation source-of-truth test",
    );

    let conn = Connection::open(&ledger).expect("open physical ledger");
    conn.execute("DROP TRIGGER ledger_records_no_update", [])
        .expect("drop update trigger to simulate schema drift");
    conn.execute_batch(
        "CREATE TRIGGER ledger_records_no_update
             BEFORE UPDATE ON ledger_records
             BEGIN
                 SELECT 1;
             END;",
    )
    .expect("install no-op trigger with the expected name");

    let output = run_context_graph(&[
        "blackbox",
        "inspect",
        "ledger",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
    ]);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "BBX_SCHEMA_VALIDATION_FAILED");
    assert_eq!(
        error["field_path"],
        "schema.triggers.ledger_records_no_update.sql"
    );

    conn.execute(
        "UPDATE ledger_records SET record_kind = 'tampered' WHERE sequence = 1",
        [],
    )
    .expect("the no-op trigger demonstrates the physical append-only guard is broken");
    let kind: String = conn
        .query_row(
            "SELECT record_kind FROM ledger_records WHERE sequence = 1",
            [],
            |row| row.get(0),
        )
        .expect("read physical row after no-op trigger update");
    assert_eq!(kind, "tampered");
}

#[test]
fn phase1_init_does_not_repair_missing_append_only_trigger() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    init_ledger(&ledger);

    let conn = Connection::open(&ledger).expect("open physical ledger");
    conn.execute("DROP TRIGGER ledger_records_no_update", [])
        .expect("drop update trigger to simulate schema drift");

    let output = run_context_graph(&[
        "blackbox",
        "init",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
    ]);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "BBX_SCHEMA_VALIDATION_FAILED");
    assert_eq!(
        error["field_path"],
        "schema.triggers.ledger_records_no_update"
    );

    let trigger_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'trigger' AND name = 'ledger_records_no_update'",
            [],
            |row| row.get(0),
        )
        .expect("read physical trigger state");
    assert_eq!(
        trigger_count, 0,
        "init must not silently recreate drifted triggers"
    );
}
