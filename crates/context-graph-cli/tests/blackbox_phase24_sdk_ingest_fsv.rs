//! BlackBoxFix Phase 24 SDK ingestion FSV tests.
//!
//! These tests execute the real CLI against real CloudEvents JSON files and a
//! real SQLite ledger, then inspect `ledger_records` directly.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase24_sdk_test";
const AGENT: &str = "agent_phase24_sdk_test";
const SOURCE: &str = "urn:contextgraph:phase24-sdk-test";
const TRACEPARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

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

fn ingest(ledger: &Path, input: &Path) -> Output {
    run_context_graph(&[
        "blackbox",
        "sdk",
        "ingest",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--input",
        input.to_str().expect("input path UTF-8"),
    ])
}

fn cloud_event(id: &str, event_type: &str, data: Value) -> Value {
    json!({
        "specversion": "1.0",
        "id": id,
        "source": SOURCE,
        "type": event_type,
        "datacontenttype": "application/json",
        "subject": "phase24-sdk-fsv",
        "time": "2026-05-03T12:00:00Z",
        "traceparent": TRACEPARENT,
        "data": data,
    })
}

fn sdk_metadata() -> Value {
    json!({
        "name": "rust-sdk",
        "version": "0.24.0"
    })
}

fn run_event(id: &str) -> Value {
    cloud_event(
        id,
        "dev.contextgraph.blackbox.agent_run.v1",
        json!({
            "tenant": TENANT,
            "agent": AGENT,
            "model": "phase24-model",
            "task": "phase24 SDK ingest known happy path",
            "task_kind": "sdk-ingest-fsv",
            "risk_tier": "high",
            "sdk": sdk_metadata(),
        }),
    )
}

fn ledger_row_count(ledger: &Path) -> i64 {
    let conn = Connection::open(ledger).expect("open physical ledger");
    conn.query_row("SELECT COUNT(*) FROM ledger_records", [], |row| row.get(0))
        .expect("count physical ledger rows")
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

fn payload_by_sdk_event_id(ledger: &Path, event_id: &str) -> Value {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let payload: String = conn
        .query_row(
            "SELECT payload_json
             FROM ledger_records
             WHERE json_extract(payload_json, '$.sdk_event.id') = ?1
             ORDER BY sequence DESC
             LIMIT 1",
            params![event_id],
            |row| row.get(0),
        )
        .expect("read SDK event payload from physical ledger");
    serde_json::from_str(&payload).expect("payload JSON")
}

fn create_run_from_sdk_event(root: &Path, ledger: &Path, event_id: &str) -> String {
    let path = root.join(format!("{event_id}.json"));
    write_json(&path, &run_event(event_id));
    let output = stdout_json(&ingest(ledger, &path));
    output["records"][0]["identifiers"]["run_id"]
        .as_str()
        .expect("SDK run id in ingest readback")
        .to_string()
}

#[test]
fn phase24_sdk_ingest_happy_path_persists_cloudevents_with_physical_readback() {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path();
    let ledger = root.join("phase24-sdk.sqlite");
    let run_path = root.join("run-event.json");
    let batch_path = root.join("child-events.json");
    init_ledger(&ledger);
    write_json(&run_path, &run_event("phase24-run-001"));

    let connector = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "blackbox_sdk_ingest_v1",
        "--source",
        run_path.to_str().expect("event path UTF-8"),
    ]));
    assert_eq!(connector["status"], "ok");
    assert_eq!(connector["readback"]["event_count"], 1);
    assert_eq!(
        connector["readback"]["events"][0]["type"],
        "dev.contextgraph.blackbox.agent_run.v1"
    );

    let before = ledger_kind_counts(&ledger);
    println!(
        "FSV24 HAPPY BEFORE source_of_truth=sqlite:{}#ledger_records input=file:{} COUNTS={before:?}",
        ledger.display(),
        run_path.display()
    );
    let run_ingest = stdout_json(&ingest(&ledger, &run_path));
    let run_id = run_ingest["records"][0]["identifiers"]["run_id"]
        .as_str()
        .expect("run id")
        .to_string();
    let run_payload = payload_by_sdk_event_id(&ledger, "phase24-run-001");
    println!(
        "FSV24 HAPPY RUN_READBACK run_id={} sdk_event={} runtime={}",
        run_id, run_payload["sdk_event"], run_payload["runtime"]
    );
    assert_eq!(run_payload["sdk_event"]["id"], "phase24-run-001");
    assert_eq!(run_payload["sdk_event"]["source"], SOURCE);
    assert_eq!(run_payload["runtime"]["framework"], "sdk:rust-sdk");
    assert_eq!(run_payload["runtime"]["sdk"]["version"], "0.24.0");

    let child_events = json!([
        cloud_event(
            "phase24-step-001",
            "dev.contextgraph.blackbox.agent_step.v1",
            json!({
                "tenant": TENANT,
                "agent": AGENT,
                "model": "phase24-model",
                "run_id": run_id,
                "step_kind": "analysis",
                "summary": "phase24 SDK step persisted",
                "context": { "expected_math": "2+2=4", "observed": 4 },
                "sdk": sdk_metadata(),
            })
        ),
        cloud_event(
            "phase24-decision-001",
            "dev.contextgraph.blackbox.model_decision.v1",
            json!({
                "tenant": TENANT,
                "agent": AGENT,
                "model": "phase24-model",
                "run_id": run_id,
                "tool_name": "phase24_apply_patch",
                "capability_id": "phase24.sdk.apply_patch",
                "confidence": 0.91,
                "input": {
                    "policy_context": { "risk": "known synthetic high" },
                    "operation": "choose verified patch",
                    "api_key": "phase24-redaction-secret"
                },
                "sdk": sdk_metadata(),
            })
        ),
        cloud_event(
            "phase24-tool-001",
            "dev.contextgraph.blackbox.tool_call.v1",
            json!({
                "tenant": TENANT,
                "agent": AGENT,
                "model": "phase24-model",
                "run_id": run_id,
                "tool_name": "phase24_apply_patch",
                "capability_id": "phase24.sdk.apply_patch",
                "input": {
                    "operation": "write expected file",
                    "api_key": "phase24-tool-secret"
                },
                "mutating": true,
                "expected_delta": {
                    "files": [{ "path": "phase24.txt", "sha256": "sha256:known" }],
                    "secret_token": "phase24-delta-secret"
                },
                "status": "ok",
                "external_request_id": "phase24-ext-001",
                "sdk": sdk_metadata(),
            })
        )
    ]);
    write_json(&batch_path, &child_events);

    let batch_ingest = stdout_json(&ingest(&ledger, &batch_path));
    assert_eq!(batch_ingest["event_count"], 3);
    let after = ledger_kind_counts(&ledger);
    println!(
        "FSV24 HAPPY AFTER COUNTS={after:?} RECORDS={}",
        batch_ingest["records"]
    );
    assert_eq!(after.get("agent_run"), Some(&1));
    assert_eq!(after.get("agent_step"), Some(&1));
    assert_eq!(after.get("model_decision"), Some(&1));
    assert_eq!(after.get("tool_call"), Some(&1));
    assert_eq!(ledger_row_count(&ledger), 4);

    let tool_payload = payload_by_sdk_event_id(&ledger, "phase24-tool-001");
    assert_eq!(tool_payload["mutating"], true);
    assert_eq!(tool_payload["sdk_event"]["traceparent"], TRACEPARENT);
    assert!(tool_payload["input_redacted"]["api_key"]
        .as_str()
        .expect("redacted api key")
        .starts_with("[REDACTED:sha256:"));
    assert!(tool_payload["expected_delta_redacted"]["secret_token"]
        .as_str()
        .expect("redacted expected delta token")
        .starts_with("[REDACTED:sha256:"));

    let surface = stdout_json(&run_context_graph(&["blackbox", "api", "surface"]));
    assert_eq!(
        surface["phase24_sdk_ingest"]["connector"],
        "blackbox_sdk_ingest_v1"
    );
    let schema = stdout_json(&run_context_graph(&["blackbox", "sdk", "schema"]));
    assert_eq!(schema["connector"], "blackbox_sdk_ingest_v1");
}

#[test]
fn phase24_duplicate_source_id_fails_without_mutating_ledger() {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path();
    let ledger = root.join("duplicate.sqlite");
    let event_path = root.join("duplicate-run.json");
    init_ledger(&ledger);
    write_json(&event_path, &run_event("phase24-duplicate-001"));
    stdout_json(&ingest(&ledger, &event_path));
    let before_count = ledger_row_count(&ledger);
    let before_counts = ledger_kind_counts(&ledger);
    println!("FSV24 EDGE duplicate BEFORE_ROWS={before_count} BEFORE_COUNTS={before_counts:?}");

    let output = ingest(&ledger, &event_path);
    let error = stderr_json(&output);
    let after_count = ledger_row_count(&ledger);
    let after_counts = ledger_kind_counts(&ledger);
    println!(
        "FSV24 EDGE duplicate AFTER_ROWS={after_count} AFTER_COUNTS={after_counts:?} ERROR={error}"
    );
    assert_eq!(error["error_code"], "SDK_EVENT_DUPLICATE");
    assert_eq!(after_count, before_count);
    assert_eq!(after_counts, before_counts);
}

#[test]
fn phase24_mutating_tool_without_expected_delta_is_all_or_nothing() {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path();
    let ledger = root.join("missing-delta.sqlite");
    let batch_path = root.join("missing-delta-batch.json");
    init_ledger(&ledger);
    let run_id = create_run_from_sdk_event(root, &ledger, "phase24-run-for-missing-delta");
    let before_count = ledger_row_count(&ledger);
    let before_counts = ledger_kind_counts(&ledger);
    let batch = json!([
        cloud_event(
            "phase24-valid-step-before-invalid-tool",
            "dev.contextgraph.blackbox.agent_step.v1",
            json!({
                "tenant": TENANT,
                "agent": AGENT,
                "run_id": run_id,
                "step_kind": "will-not-persist",
                "summary": "this valid event must roll back with the invalid batch",
                "context": { "should_persist": false },
                "sdk": sdk_metadata(),
            })
        ),
        cloud_event(
            "phase24-invalid-tool-missing-delta",
            "dev.contextgraph.blackbox.tool_call.v1",
            json!({
                "tenant": TENANT,
                "agent": AGENT,
                "run_id": run_id,
                "tool_name": "phase24_mutating_without_delta",
                "capability_id": "phase24.sdk.mutate",
                "input": { "operation": "mutate without expected delta" },
                "mutating": true,
                "status": "ok",
                "sdk": sdk_metadata(),
            })
        )
    ]);
    write_json(&batch_path, &batch);
    println!("FSV24 EDGE missing_delta BEFORE_ROWS={before_count} BEFORE_COUNTS={before_counts:?}");

    let output = ingest(&ledger, &batch_path);
    let error = stderr_json(&output);
    let after_count = ledger_row_count(&ledger);
    let after_counts = ledger_kind_counts(&ledger);
    println!("FSV24 EDGE missing_delta AFTER_ROWS={after_count} AFTER_COUNTS={after_counts:?} ERROR={error}");
    assert_eq!(error["error_code"], "SDK_EXPECTED_DELTA_REQUIRED");
    assert_eq!(after_count, before_count);
    assert_eq!(after_counts, before_counts);
    assert_eq!(
        after_counts.get("agent_step").copied().unwrap_or(0),
        0,
        "valid earlier batch event must not persist when later event is invalid"
    );
}

#[test]
fn phase24_empty_batch_and_invalid_traceparent_fail_before_ledger_mutation() {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path();
    let ledger = root.join("invalid-input.sqlite");
    let empty_path = root.join("empty.json");
    let invalid_trace_path = root.join("invalid-trace.json");
    init_ledger(&ledger);
    write_json(&empty_path, &json!([]));
    let before_empty = ledger_row_count(&ledger);
    println!("FSV24 EDGE empty BEFORE_ROWS={before_empty}");
    let empty_error = stderr_json(&ingest(&ledger, &empty_path));
    let after_empty = ledger_row_count(&ledger);
    println!("FSV24 EDGE empty AFTER_ROWS={after_empty} ERROR={empty_error}");
    assert_eq!(empty_error["error_code"], "SDK_EVENT_BATCH_EMPTY");
    assert_eq!(after_empty, before_empty);

    let mut invalid_trace = run_event("phase24-invalid-traceparent");
    invalid_trace["traceparent"] =
        Value::String("00-00000000000000000000000000000000-00f067aa0ba902b7-01".to_string());
    write_json(&invalid_trace_path, &invalid_trace);
    let before_trace = ledger_row_count(&ledger);
    println!("FSV24 EDGE invalid_trace BEFORE_ROWS={before_trace}");
    let trace_error = stderr_json(&ingest(&ledger, &invalid_trace_path));
    let after_trace = ledger_row_count(&ledger);
    println!("FSV24 EDGE invalid_trace AFTER_ROWS={after_trace} ERROR={trace_error}");
    assert_eq!(trace_error["error_code"], "SDK_TRACEPARENT_INVALID");
    assert_eq!(after_trace, before_trace);
}
