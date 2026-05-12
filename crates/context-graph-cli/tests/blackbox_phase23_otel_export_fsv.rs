//! BlackBoxFix Phase 23 OpenTelemetry export FSV tests.
//!
//! These tests execute the real CLI against a real SQLite ledger and then
//! inspect the physical ledger plus exported OTLP JSONL files directly.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase23_otel_test";
const AGENT: &str = "agent_phase23_otel_test";

struct Phase23Case {
    _temp: TempDir,
    root: PathBuf,
    ledger: PathBuf,
    run_id: String,
    tool_call_id: String,
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
        "phase23_otel_export_fixture",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--capability",
        "phase23.telemetry.export",
    ]))
}

fn new_phase23_case(task: &str) -> Phase23Case {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();
    let ledger = root.join("blackbox.sqlite");
    let tool_input = root.join("tool-input.json");
    write_json(
        &tool_input,
        &json!({
            "operation": "phase23-opentelemetry-export",
            "expected_signals": ["traces", "logs", "metrics"]
        }),
    );

    init_ledger(&ledger);
    let run = record_run(&ledger, task);
    let run_id = run["run_id"].as_str().expect("run id").to_string();
    let tool = record_tool_call(&ledger, &run_id, &tool_input);
    let tool_call_id = tool["tool_call_id"]
        .as_str()
        .expect("tool call id")
        .to_string();

    Phase23Case {
        _temp: temp,
        root,
        ledger,
        run_id,
        tool_call_id,
    }
}

fn verifier_spec(case: &Phase23Case, command: Value, artifacts: Value) -> Value {
    json!({
        "verifier_id": "cmd_exit_zero_v1",
        "run_id": case.run_id,
        "tool_call_id": case.tool_call_id,
        "source": { "cwd": case.root },
        "expected": {
            "command": command,
            "artifacts": artifacts
        }
    })
}

fn run_verify(case: &Phase23Case, spec: Value, name: &str) -> Output {
    let spec_path = case.root.join(format!("{name}-verify-spec.json"));
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

fn add_pass_and_failure_evidence(case: &Phase23Case) {
    let pass = stdout_json(&run_verify(
        case,
        verifier_spec(
            case,
            json!(["sh", "-c", "printf phase23-otel > cmd-output.txt"]),
            json!(["cmd-output.txt"]),
        ),
        "pass",
    ));
    assert_eq!(pass["verifier_status"], "passed");
    assert_eq!(
        fs::read_to_string(case.root.join("cmd-output.txt")).expect("read pass artifact"),
        "phase23-otel"
    );

    let failure = run_verify(
        case,
        verifier_spec(
            case,
            json!(["sh", "-c", "printf failed > failed-output.txt; exit 7"]),
            json!(["failed-output.txt"]),
        ),
        "fail",
    );
    assert_eq!(stderr_json(&failure)["error_code"], "CMD_EXIT_NON_ZERO");
    assert_eq!(
        fs::read_to_string(case.root.join("failed-output.txt")).expect("read failed artifact"),
        "failed"
    );
}

fn export_otel(ledger: &Path, out: &Path, force: bool) -> Output {
    let mut args = vec![
        "blackbox",
        "export",
        "otel",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--out",
        out.to_str().expect("out path UTF-8"),
    ];
    if force {
        args.push("--force");
    }
    run_context_graph(&args)
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

fn latest_verifier_payload(ledger: &Path) -> Value {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let payload: String = conn
        .query_row(
            "SELECT payload_json
             FROM ledger_records
             WHERE record_kind = 'verifier_result'
             ORDER BY sequence DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("read latest verifier payload");
    serde_json::from_str(&payload).expect("verifier payload JSON")
}

fn sha256_file(path: &Path) -> String {
    let bytes = fs::read(path).expect("read file for sha256");
    let digest = Sha256::digest(&bytes);
    format!("sha256:{digest:x}")
}

fn parse_json_file(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).expect("read JSON file")).expect("parse JSON")
}

fn parse_single_jsonl(path: &Path) -> Value {
    let text = fs::read_to_string(path).expect("read JSONL file");
    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(
        lines.len(),
        1,
        "{} must contain one OTLP JSON line",
        path.display()
    );
    serde_json::from_str(lines[0]).expect("parse OTLP JSONL line")
}

fn file_entry<'a>(manifest: &'a Value, signal: &str) -> &'a Value {
    manifest["files"]
        .as_array()
        .expect("manifest files")
        .iter()
        .find(|entry| entry["signal"] == signal)
        .expect("manifest signal file")
}

fn assert_otel_export_files(out: &Path, expected_spans: i64) -> Value {
    let traces_path = out.join("traces.jsonl");
    let logs_path = out.join("logs.jsonl");
    let metrics_path = out.join("metrics.jsonl");
    let manifest_path = out.join("manifest.json");
    for path in [&traces_path, &logs_path, &metrics_path, &manifest_path] {
        assert!(path.exists(), "{} must exist", path.display());
    }

    let traces = parse_single_jsonl(&traces_path);
    let logs = parse_single_jsonl(&logs_path);
    let metrics = parse_single_jsonl(&metrics_path);
    let manifest = parse_json_file(&manifest_path);

    let spans = traces
        .pointer("/resourceSpans/0/scopeSpans/0/spans")
        .and_then(Value::as_array)
        .expect("trace spans");
    let log_records = logs
        .pointer("/resourceLogs/0/scopeLogs/0/logRecords")
        .and_then(Value::as_array)
        .expect("log records");
    let metric_rows = metrics
        .pointer("/resourceMetrics/0/scopeMetrics/0/metrics")
        .and_then(Value::as_array)
        .expect("metrics");
    assert_eq!(spans.len() as i64, expected_spans);
    assert!(
        log_records.len() >= 3,
        "failed verifier, incident packet, and summary log must be exported"
    );
    assert_eq!(metric_rows.len(), 4);
    assert_eq!(manifest["format"], "blackbox_otel_export_manifest_json_v1");
    assert_eq!(manifest["counts"]["spans_total"], expected_spans);
    assert_eq!(manifest["counts"]["metrics_total"], 4);
    assert_eq!(
        file_entry(&manifest, "traces")["sha256"],
        sha256_file(&traces_path)
    );
    assert_eq!(
        file_entry(&manifest, "logs")["sha256"],
        sha256_file(&logs_path)
    );
    assert_eq!(
        file_entry(&manifest, "metrics")["sha256"],
        sha256_file(&metrics_path)
    );
    assert_eq!(
        file_entry(&manifest, "traces")["item_count"],
        expected_spans
    );
    assert!(
        spans[0]["attributes"]
            .as_array()
            .expect("span attributes")
            .iter()
            .any(|attr| attr["key"] == "blackbox.record.kind")
    );
    assert!(
        traces["resourceSpans"][0]["resource"]["attributes"]
            .as_array()
            .expect("resource attributes")
            .iter()
            .any(|attr| {
                attr["key"] == "service.name" && attr["value"]["stringValue"] == "blackboxfix"
            })
    );
    manifest
}

#[test]
fn phase23_otel_export_writes_trace_log_metric_files_with_physical_readback() {
    let case = new_phase23_case("phase23 OpenTelemetry export happy path");
    add_pass_and_failure_evidence(&case);
    let out = case.root.join("otel-export");
    let before_rows = ledger_row_count(&case.ledger);
    let before_counts = ledger_kind_counts(&case.ledger);
    println!(
        "FSV23 HAPPY source_of_truth=sqlite:{}#ledger_records output=file:{} BEFORE_ROWS={before_rows} BEFORE_COUNTS={before_counts:?}",
        case.ledger.display(),
        out.display()
    );
    assert_eq!(latest_verifier_payload(&case.ledger)["status"], "failed");
    assert_eq!(
        latest_verifier_payload(&case.ledger)["error_code"],
        "CMD_EXIT_NON_ZERO"
    );

    let export = stdout_json(&export_otel(&case.ledger, &out, false));
    let after_rows = ledger_row_count(&case.ledger);
    let after_counts = ledger_kind_counts(&case.ledger);
    println!(
        "FSV23 HAPPY AFTER_ROWS={after_rows} AFTER_COUNTS={after_counts:?} EXPORT_COUNTS={}",
        export["counts"]
    );
    assert_eq!(
        before_rows, after_rows,
        "OTEL export must not mutate ledger"
    );
    assert_eq!(before_counts, after_counts);
    assert_eq!(export["connector"], "blackbox_otel_export_v1");

    let manifest = assert_otel_export_files(&out, after_rows);
    assert_eq!(manifest["source_ledger"]["ledger_record_count"], after_rows);
    assert_eq!(
        manifest["counts"]["record_kind_counts"]["verifier_result"],
        2
    );
    assert_eq!(manifest["counts"]["verifier_status_counts"]["passed"], 1);
    assert_eq!(manifest["counts"]["verifier_status_counts"]["failed"], 1);

    let connector = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "blackbox_otel_export_v1",
        "--source",
        out.to_str().expect("export path UTF-8"),
    ]));
    assert_eq!(connector["status"], "ok");
    assert_eq!(
        connector["readback"]["files"]["traces"]["item_count"],
        after_rows
    );
    assert_eq!(connector["readback"]["files"]["metrics"]["item_count"], 4);

    let surface = stdout_json(&run_context_graph(&["blackbox", "api", "surface"]));
    assert_eq!(
        surface["phase23_opentelemetry_export"]["connector"],
        "blackbox_otel_export_v1"
    );
}

#[test]
fn phase23_empty_ledger_fails_before_export_files_are_written() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("empty.sqlite");
    let out = temp.path().join("otel-empty");
    init_ledger(&ledger);
    let before = ledger_kind_counts(&ledger);
    println!(
        "FSV23 EDGE empty BEFORE={before:?} OUT_EXISTS={}",
        out.exists()
    );

    let output = export_otel(&ledger, &out, false);
    let error = stderr_json(&output);
    let after = ledger_kind_counts(&ledger);
    println!(
        "FSV23 EDGE empty AFTER={after:?} OUT_EXISTS={}",
        out.exists()
    );
    assert_eq!(error["error_code"], "OTEL_EXPORT_EMPTY_LEDGER");
    assert_eq!(before, after);
    assert!(
        !out.exists(),
        "empty ledger export must not create output dir"
    );
}

#[test]
fn phase23_existing_output_without_force_preserves_prior_export() {
    let case = new_phase23_case("phase23 existing output edge");
    add_pass_and_failure_evidence(&case);
    let out = case.root.join("otel-existing");
    stdout_json(&export_otel(&case.ledger, &out, false));
    let before_rows = ledger_row_count(&case.ledger);
    let before_counts = ledger_kind_counts(&case.ledger);
    let before_manifest_sha = sha256_file(&out.join("manifest.json"));
    println!(
        "FSV23 EDGE existing BEFORE_ROWS={before_rows} BEFORE_COUNTS={before_counts:?} MANIFEST_SHA={before_manifest_sha}"
    );

    let output = export_otel(&case.ledger, &out, false);
    let error = stderr_json(&output);
    let after_rows = ledger_row_count(&case.ledger);
    let after_counts = ledger_kind_counts(&case.ledger);
    let after_manifest_sha = sha256_file(&out.join("manifest.json"));
    println!(
        "FSV23 EDGE existing AFTER_ROWS={after_rows} AFTER_COUNTS={after_counts:?} MANIFEST_SHA={after_manifest_sha}"
    );
    assert_eq!(error["error_code"], "OTEL_OUTPUT_EXISTS");
    assert_eq!(before_rows, after_rows);
    assert_eq!(before_counts, after_counts);
    assert_eq!(before_manifest_sha, after_manifest_sha);
    assert_otel_export_files(&out, after_rows);
}

#[test]
fn phase23_hash_chain_break_blocks_export_before_files_are_written() {
    let case = new_phase23_case("phase23 tampered ledger edge");
    let out = case.root.join("otel-tampered");
    let before_rows = ledger_row_count(&case.ledger);
    let before_counts = ledger_kind_counts(&case.ledger);
    println!(
        "FSV23 EDGE tamper BEFORE_ROWS={before_rows} BEFORE_COUNTS={before_counts:?} OUT_EXISTS={}",
        out.exists()
    );
    {
        let conn = Connection::open(&case.ledger).expect("open physical ledger");
        conn.execute_batch(
            "DROP TRIGGER ledger_records_no_update;
             UPDATE ledger_records
             SET payload_json = '{\"record_kind\":\"agent_run\",\"tampered\":true}'
             WHERE sequence = 1;
             CREATE TRIGGER ledger_records_no_update
             BEFORE UPDATE ON ledger_records
             BEGIN
                 SELECT RAISE(ABORT, 'ledger_records is append-only; updates are forbidden');
             END;",
        )
        .expect("tamper ledger while restoring trigger");
    }

    let output = export_otel(&case.ledger, &out, false);
    let error = stderr_json(&output);
    let after_rows = ledger_row_count(&case.ledger);
    let after_count: i64 = {
        let conn = Connection::open(&case.ledger).expect("open physical ledger");
        conn.query_row(
            "SELECT COUNT(*) FROM ledger_records WHERE json_extract(payload_json, '$.tampered') = 1",
            params![],
            |row| row.get(0),
        )
        .expect("count tampered physical rows")
    };
    println!(
        "FSV23 EDGE tamper AFTER_ROWS={after_rows} TAMPERED_ROWS={after_count} OUT_EXISTS={}",
        out.exists()
    );
    assert_eq!(error["error_code"], "BBX_HASH_CHAIN_BROKEN");
    assert_eq!(before_rows, after_rows);
    assert_eq!(after_count, 1);
    assert!(
        !out.exists(),
        "tampered ledger export must not create output dir"
    );
}
