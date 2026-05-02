//! BlackBoxFix Phase 7 dashboard FSV tests.
//!
//! These tests execute the real `context-graph blackbox` binary against real
//! SQLite ledgers, real Git repositories, and real generated dashboard files.
//! Assertions inspect the physical ledger and dashboard API files directly.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase7_test";
const AGENT: &str = "agent_phase7_test";

struct Phase7Case {
    _temp: TempDir,
    root: PathBuf,
    ledger: PathBuf,
    repo: PathBuf,
    passed_run_id: String,
    passed_tool_call_id: String,
    failed_run_id: String,
    failed_tool_call_id: String,
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

fn must_git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout={}\nstderr={}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git stdout UTF-8")
}

fn create_git_repo(repo: &Path) {
    let output = Command::new("git")
        .arg("init")
        .arg(repo)
        .output()
        .expect("spawn git init");
    assert!(
        output.status.success(),
        "git init failed\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    must_git(repo, &["config", "user.email", "phase7@example.test"]);
    must_git(repo, &["config", "user.name", "Phase7 Test"]);
    fs::write(repo.join("app.txt"), "phase7\n").expect("write tracked file");
    must_git(repo, &["add", "app.txt"]);
    must_git(repo, &["commit", "-m", "initial phase7 fixture"]);
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
        "low",
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
        "phase7_real_tool",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--capability",
        "phase7.real_tool.write",
    ]))
}

fn new_phase7_case() -> Phase7Case {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();
    let ledger = root.join("blackbox.sqlite");
    let repo = root.join("repo");
    let passed_input = root.join("passed-tool-input.json");
    let failed_input = root.join("failed-tool-input.json");

    init_ledger(&ledger);
    create_git_repo(&repo);
    write_json(
        &passed_input,
        &json!({
            "operation": "phase7-dashboard-pass",
            "target": repo.join("app.txt").display().to_string()
        }),
    );
    write_json(
        &failed_input,
        &json!({
            "operation": "phase7-dashboard-fail",
            "target": repo.join("app.txt").display().to_string()
        }),
    );

    let passed_run = record_run(&ledger, "phase7 dashboard passed run");
    let passed_run_id = passed_run["run_id"]
        .as_str()
        .expect("passed run id")
        .to_string();
    let passed_tool = record_tool_call(&ledger, &passed_run_id, &passed_input);
    let passed_tool_call_id = passed_tool["tool_call_id"]
        .as_str()
        .expect("passed tool call id")
        .to_string();

    let failed_run = record_run(&ledger, "phase7 dashboard failed run");
    let failed_run_id = failed_run["run_id"]
        .as_str()
        .expect("failed run id")
        .to_string();
    let failed_tool = record_tool_call(&ledger, &failed_run_id, &failed_input);
    let failed_tool_call_id = failed_tool["tool_call_id"]
        .as_str()
        .expect("failed tool call id")
        .to_string();

    Phase7Case {
        _temp: temp,
        root,
        ledger,
        repo,
        passed_run_id,
        passed_tool_call_id,
        failed_run_id,
        failed_tool_call_id,
    }
}

fn verifier_spec(case: &Phase7Case, run_id: &str, tool_call_id: &str, expected: Value) -> Value {
    json!({
        "verifier_id": "git_branch_exists_v1",
        "run_id": run_id,
        "tool_call_id": tool_call_id,
        "source": { "repo": case.repo.display().to_string() },
        "expected": expected
    })
}

fn run_verify(case: &Phase7Case, spec: Value, file_name: &str) -> Output {
    let spec_path = case.root.join(file_name);
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

fn run_passed_and_failed_verifiers(case: &Phase7Case) {
    let branch = must_git(&case.repo, &["branch", "--show-current"]);
    let branch = branch.trim();
    let pass = stdout_json(&run_verify(
        case,
        verifier_spec(
            case,
            &case.passed_run_id,
            &case.passed_tool_call_id,
            json!({ "branch": branch }),
        ),
        "passed-verify-spec.json",
    ));
    assert_eq!(pass["verifier_status"], "passed");

    let fail = run_verify(
        case,
        verifier_spec(
            case,
            &case.failed_run_id,
            &case.failed_tool_call_id,
            json!({ "branch": "missing-phase7-branch" }),
        ),
        "failed-verify-spec.json",
    );
    assert_eq!(stderr_json(&fail)["error_code"], "GIT_BRANCH_NOT_FOUND");
}

fn dashboard_build(ledger: &Path, out: &Path, force: bool) -> Output {
    let mut args = vec![
        "blackbox",
        "dashboard",
        "build",
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

fn dashboard_api(ledger: &Path, view: &str, run_id: Option<&str>) -> Output {
    let mut args = vec![
        "blackbox",
        "dashboard",
        "api",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--view",
        view,
    ];
    if let Some(run_id) = run_id {
        args.push("--run-id");
        args.push(run_id);
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

fn verifier_status_counts_from_db(ledger: &Path) -> BTreeMap<String, i64> {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let mut stmt = conn
        .prepare(
            "SELECT json_extract(payload_json, '$.status'), COUNT(*)
             FROM ledger_records
             WHERE record_kind = 'verifier_result'
             GROUP BY json_extract(payload_json, '$.status')
             ORDER BY 1",
        )
        .expect("prepare verifier status count");
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("query verifier status count");
    rows.map(|row| row.expect("verifier status count row"))
        .collect()
}

fn evidence_packet_exists(ledger: &Path, evidence_packet_id: &str) -> bool {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM ledger_records
             WHERE record_kind = 'evidence_packet'
               AND json_extract(payload_json, '$.evidence_packet_id') = ?1",
            params![evidence_packet_id],
            |row| row.get(0),
        )
        .expect("count evidence packet id");
    count == 1
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).expect("read JSON")).expect("parse JSON")
}

fn sha256_file(path: &Path) -> String {
    let bytes = fs::read(path).expect("read file for sha256");
    let digest = Sha256::digest(&bytes);
    format!("sha256:{digest:x}")
}

#[test]
fn phase7_dashboard_build_writes_api_manifest_html_and_timelines() {
    let case = new_phase7_case();
    run_passed_and_failed_verifiers(&case);
    let before_rows = ledger_row_count(&case.ledger);
    let direct_counts = ledger_kind_counts(&case.ledger);
    let direct_status_counts = verifier_status_counts_from_db(&case.ledger);
    let out = case.root.join("dashboard");

    let build = stdout_json(&dashboard_build(&case.ledger, &out, false));
    let after_rows = ledger_row_count(&case.ledger);
    assert_eq!(
        before_rows, after_rows,
        "dashboard build must not mutate ledger"
    );
    assert_eq!(build["runs_total"], 2);
    assert_eq!(build["evidence_packets_total"], 2);
    assert_eq!(build["incident_packets_total"], 1);

    let index = out.join("index.html");
    let manifest_path = out.join("manifest.json");
    let summary_path = out.join("api/summary.json");
    let runs_path = out.join("api/runs.json");
    let evidence_path = out.join("api/evidence.json");
    let verifiers_path = out.join("api/verifier-status.json");
    let errors_path = out.join("api/errors.json");
    let exports_path = out.join("api/exports.json");
    for path in [
        &index,
        &manifest_path,
        &summary_path,
        &runs_path,
        &evidence_path,
        &verifiers_path,
        &errors_path,
        &exports_path,
    ] {
        assert!(
            path.exists(),
            "dashboard file must exist: {}",
            path.display()
        );
    }

    let summary = read_json(&summary_path);
    assert_eq!(summary["format"], "blackbox_dashboard_summary_json_v1");
    assert_eq!(
        summary["counts"]["record_kinds"]["agent_run"],
        direct_counts["agent_run"]
    );
    assert_eq!(
        summary["counts"]["record_kinds"]["evidence_packet"],
        direct_counts["evidence_packet"]
    );
    assert_eq!(
        summary["counts"]["record_kinds"]["incident_packet"],
        direct_counts["incident_packet"]
    );
    assert_eq!(
        summary["source_of_truth_coverage"]["tool_calls_without_evidence"],
        0
    );
    assert_eq!(
        summary["evidence_hash_verification"]["checked_packets"],
        direct_counts["evidence_packet"]
    );

    let runs = read_json(&runs_path);
    let run_rows = runs["runs"].as_array().expect("runs array");
    assert_eq!(run_rows.len(), 2);
    assert!(run_rows
        .iter()
        .any(|run| run["run_id"] == case.passed_run_id && run["status"] == "passed"));
    assert!(run_rows
        .iter()
        .any(|run| run["run_id"] == case.failed_run_id && run["status"] == "failed"));
    for run in run_rows {
        let timeline = out.join(run["timeline_api"].as_str().expect("timeline api path"));
        assert!(timeline.exists(), "timeline file must exist");
        let timeline_json = read_json(&timeline);
        assert_eq!(
            timeline_json["format"],
            "blackbox_dashboard_timeline_json_v1"
        );
        assert!(
            timeline_json["report"]["timeline"]
                .as_array()
                .unwrap()
                .len()
                >= 5
        );
    }

    let evidence = read_json(&evidence_path);
    let packets = evidence["evidence_packets"]
        .as_array()
        .expect("evidence packets array");
    assert_eq!(packets.len(), 2);
    for packet in packets {
        let evidence_id = packet["evidence_packet_id"].as_str().expect("evidence id");
        assert!(
            evidence_packet_exists(&case.ledger, evidence_id),
            "dashboard evidence packet must exist in physical ledger: {evidence_id}"
        );
        assert!(packet["source_of_truth"].is_array());
        assert!(packet["record_refs"].is_array());
    }

    let verifiers = read_json(&verifiers_path);
    assert_eq!(
        verifiers["counts"]["by_status"]["passed"],
        direct_status_counts["passed"]
    );
    assert_eq!(
        verifiers["counts"]["by_status"]["failed"],
        direct_status_counts["failed"]
    );
    assert_eq!(verifiers["verifier_results"].as_array().unwrap().len(), 2);

    let errors = read_json(&errors_path);
    let error_codes = errors["error_codes"].as_array().expect("error codes array");
    assert!(error_codes
        .iter()
        .any(|error| error["error_code"] == "GIT_BRANCH_NOT_FOUND" && error["count"] == 2));

    let exports = read_json(&exports_path);
    assert_eq!(
        exports["exports"]["evidence_by_run"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let manifest = read_json(&manifest_path);
    assert_eq!(manifest["format"], "blackbox_dashboard_manifest_json_v1");
    for file in manifest["files"].as_array().expect("manifest files array") {
        let rel = file["path"].as_str().expect("file path");
        let path = out.join(rel);
        assert_eq!(file["sha256"], sha256_file(&path), "manifest hash mismatch");
        assert_eq!(
            file["bytes"],
            fs::metadata(&path).expect("stat dashboard file").len()
        );
    }

    let html = fs::read_to_string(&index).expect("read dashboard HTML");
    assert!(html.contains("Source of truth:"));
    assert!(html.contains("Timeline"));
    assert!(html.contains("id=\"timeline-kind\""));
    assert!(html.contains("Verifier Status"));
    assert!(html.contains("Evidence"));
    assert!(html.contains("Error Codes"));
    assert!(html.contains("Exports"));

    let timeline_api = stdout_json(&dashboard_api(
        &case.ledger,
        "timeline",
        Some(&case.failed_run_id),
    ));
    assert_eq!(
        timeline_api["data"]["report"]["summary"]["verifier_statuses"]["failed"],
        1
    );
}

#[test]
fn phase7_existing_dashboard_output_without_force_fails_before_writing() {
    let case = new_phase7_case();
    run_passed_and_failed_verifiers(&case);
    let out = case.root.join("existing-dashboard");
    fs::create_dir(&out).expect("create existing dashboard dir");
    fs::write(out.join("marker.txt"), "keep").expect("write marker");
    let before = ledger_row_count(&case.ledger);

    let output = dashboard_build(&case.ledger, &out, false);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "BBX_OUTPUT_EXISTS");
    assert_eq!(error["connector"], "fs_local_v1");
    assert_eq!(ledger_row_count(&case.ledger), before);
    assert!(out.join("marker.txt").exists());
    assert!(!out.join("manifest.json").exists());
    assert!(!out.join("api/summary.json").exists());
}

#[test]
fn phase7_empty_ledger_blocks_dashboard_before_files_are_written() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("empty.sqlite");
    let out = temp.path().join("empty-dashboard");
    init_ledger(&ledger);
    let before = ledger_row_count(&ledger);

    let output = dashboard_build(&ledger, &out, false);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "DASHBOARD_NO_RUNS");
    assert_eq!(ledger_row_count(&ledger), before);
    assert!(
        !out.exists(),
        "empty dashboard build must not create output dir"
    );
}

#[test]
fn phase7_hash_chain_break_blocks_dashboard_before_files_are_written() {
    let case = new_phase7_case();
    run_passed_and_failed_verifiers(&case);
    let out = case.root.join("tampered-dashboard");
    let before = ledger_row_count(&case.ledger);
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

    let output = dashboard_build(&case.ledger, &out, false);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "BBX_HASH_CHAIN_BROKEN");
    assert_eq!(ledger_row_count(&case.ledger), before);
    assert!(
        !out.exists(),
        "tampered dashboard must not create output dir"
    );
}

#[test]
fn phase7_missing_timeline_run_id_fails_without_mutating_ledger() {
    let case = new_phase7_case();
    run_passed_and_failed_verifiers(&case);
    let before = ledger_row_count(&case.ledger);

    let output = dashboard_api(&case.ledger, "timeline", Some("run_missing_phase7"));
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "BBX_RECORD_NOT_FOUND");
    assert_eq!(ledger_row_count(&case.ledger), before);
}
