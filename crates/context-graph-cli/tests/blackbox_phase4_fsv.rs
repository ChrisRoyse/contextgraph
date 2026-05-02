//! BlackBoxFix Phase 4 FSV tests.
//!
//! These tests execute the real `context-graph blackbox` binary against real
//! SQLite ledgers and real Git repositories. Assertions read the physical
//! ledger and exported files directly; no mock evidence is accepted.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase4_test";
const AGENT: &str = "agent_phase4_test";

struct Phase4Case {
    _temp: TempDir,
    root: PathBuf,
    ledger: PathBuf,
    repo: PathBuf,
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
    must_git(repo, &["config", "user.email", "phase4@example.test"]);
    must_git(repo, &["config", "user.name", "Phase4 Test"]);
    fs::write(repo.join("app.txt"), "phase4\n").expect("write tracked file");
    must_git(repo, &["add", "app.txt"]);
    must_git(repo, &["commit", "-m", "initial phase4 fixture"]);
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
        "phase4_real_tool",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--capability",
        "phase4.real_tool.write",
    ]))
}

fn new_phase4_case(task: &str) -> Phase4Case {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();
    let ledger = root.join("blackbox.sqlite");
    let repo = root.join("repo");
    let tool_input = root.join("tool-input.json");

    init_ledger(&ledger);
    create_git_repo(&repo);
    write_json(
        &tool_input,
        &json!({
            "operation": "phase4-evidence-read",
            "target": repo.join("app.txt").display().to_string()
        }),
    );
    let run = record_run(&ledger, task);
    let run_id = run["run_id"].as_str().expect("run id").to_string();
    let tool = record_tool_call(&ledger, &run_id, &tool_input);
    let tool_call_id = tool["tool_call_id"]
        .as_str()
        .expect("tool call id")
        .to_string();

    Phase4Case {
        _temp: temp,
        root,
        ledger,
        repo,
        run_id,
        tool_call_id,
    }
}

fn verifier_spec(case: &Phase4Case, verifier_id: &str, expected: Value) -> Value {
    json!({
        "verifier_id": verifier_id,
        "run_id": case.run_id,
        "tool_call_id": case.tool_call_id,
        "source": { "repo": case.repo.display().to_string() },
        "expected": expected
    })
}

fn run_verify(case: &Phase4Case, spec: Value) -> Output {
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
            [record_kind],
            |row| row.get(0),
        )
        .expect("read latest ledger payload");
    serde_json::from_str(&payload).expect("ledger payload JSON")
}

fn ledger_row_count(ledger: &Path) -> i64 {
    let conn = Connection::open(ledger).expect("open physical ledger");
    conn.query_row("SELECT COUNT(*) FROM ledger_records", [], |row| row.get(0))
        .expect("count physical ledger rows")
}

fn assert_kind_count(ledger: &Path, kind: &str, expected: i64) {
    let counts = ledger_kind_counts(ledger);
    assert_eq!(
        counts.get(kind).copied().unwrap_or(0),
        expected,
        "ledger count for {kind}: {counts:?}"
    );
}

fn assert_evidence_refs_match_physical_rows(ledger: &Path, evidence: &Value) {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let refs = evidence["record_refs"]
        .as_array()
        .expect("evidence record_refs array");
    assert!(!refs.is_empty(), "evidence must contain physical row refs");
    for record_ref in refs {
        let sequence = record_ref["sequence"].as_i64().expect("ref sequence");
        let (record_kind, payload_sha256, record_sha256): (String, String, String) = conn
            .query_row(
                "SELECT record_kind, payload_sha256, record_sha256
                 FROM ledger_records
                 WHERE sequence = ?1",
                params![sequence],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read referenced physical row");
        assert_eq!(record_ref["record_kind"], record_kind);
        assert_eq!(record_ref["payload_sha256"], payload_sha256);
        assert_eq!(record_ref["record_sha256"], record_sha256);
    }
}

fn replay(case: &Phase4Case, run_id: &str) -> Output {
    run_context_graph(&[
        "blackbox",
        "replay",
        "--ledger",
        case.ledger.to_str().expect("ledger path UTF-8"),
        "--run-id",
        run_id,
    ])
}

fn export_evidence(case: &Phase4Case, run_id: &str, out: &Path) -> Output {
    run_context_graph(&[
        "blackbox",
        "export",
        "evidence",
        "--ledger",
        case.ledger.to_str().expect("ledger path UTF-8"),
        "--run-id",
        run_id,
        "--out",
        out.to_str().expect("out path UTF-8"),
    ])
}

#[test]
fn phase4_passed_verifier_writes_evidence_and_replay_is_read_only() {
    let case = new_phase4_case("phase4 evidence packet happy path");
    let branch = must_git(&case.repo, &["branch", "--show-current"]);
    let branch = branch.trim();

    let output = stdout_json(&run_verify(
        &case,
        verifier_spec(&case, "git_branch_exists_v1", json!({ "branch": branch })),
    ));
    assert_eq!(output["verifier_status"], "passed");
    assert!(output["evidence_packet_sequence"].as_i64().unwrap() > 0);
    assert!(output["incident_packet_sequence"].is_null());

    assert_kind_count(&case.ledger, "agent_run", 1);
    assert_kind_count(&case.ledger, "tool_call", 1);
    assert_kind_count(&case.ledger, "expected_delta", 1);
    assert_kind_count(&case.ledger, "readback_snapshot", 1);
    assert_kind_count(&case.ledger, "verifier_result", 1);
    assert_kind_count(&case.ledger, "evidence_packet", 1);
    assert_kind_count(&case.ledger, "incident_packet", 0);

    let evidence = latest_payload(&case.ledger, "evidence_packet");
    assert_eq!(evidence["verifier_status"], "passed");
    assert_eq!(evidence["record_kind"], "evidence_packet");
    assert_eq!(evidence["source_of_truth"][0]["kind"], "git_local_v1");
    assert_evidence_refs_match_physical_rows(&case.ledger, &evidence);

    let before = ledger_row_count(&case.ledger);
    let replay = stdout_json(&replay(&case, &case.run_id));
    let after = ledger_row_count(&case.ledger);
    assert_eq!(before, after, "forensic replay must not mutate ledger");
    assert_eq!(replay["report"]["mode"], "forensic");
    assert_eq!(replay["report"]["mutates_source_of_truth"], false);
    assert_eq!(
        replay["report"]["evidence_hash_verification"]["status"],
        "passed"
    );
    assert_eq!(replay["report"]["summary"]["kinds"]["evidence_packet"], 1);
}

#[test]
fn phase4_failed_verifier_writes_evidence_and_incident_packet() {
    let case = new_phase4_case("phase4 incident packet failure path");

    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "git_branch_exists_v1",
            json!({ "branch": "missing-phase4-branch" }),
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], "GIT_BRANCH_NOT_FOUND");

    assert_kind_count(&case.ledger, "expected_delta", 1);
    assert_kind_count(&case.ledger, "readback_snapshot", 1);
    assert_kind_count(&case.ledger, "verifier_result", 1);
    assert_kind_count(&case.ledger, "evidence_packet", 1);
    assert_kind_count(&case.ledger, "incident_packet", 1);

    let evidence = latest_payload(&case.ledger, "evidence_packet");
    assert_eq!(evidence["verifier_status"], "failed");
    assert_evidence_refs_match_physical_rows(&case.ledger, &evidence);

    let incident = latest_payload(&case.ledger, "incident_packet");
    assert_eq!(incident["trigger"], "verifier_failed");
    assert_eq!(incident["severity"], "high");
    assert_eq!(incident["error_code"], "GIT_BRANCH_NOT_FOUND");
    assert_eq!(
        incident["evidence_packet_id"],
        evidence["evidence_packet_id"]
    );

    let replay = stdout_json(&replay(&case, &case.run_id));
    assert_eq!(replay["report"]["summary"]["kinds"]["incident_packet"], 1);
    assert_eq!(
        replay["report"]["summary"]["first_failure"]["error_code"],
        "GIT_BRANCH_NOT_FOUND"
    );
    assert_eq!(replay["report"]["incidents"].as_array().unwrap().len(), 1);
}

#[test]
fn phase4_export_evidence_writes_json_file_and_readback_matches() {
    let case = new_phase4_case("phase4 evidence export happy path");
    let branch = must_git(&case.repo, &["branch", "--show-current"]);
    let branch = branch.trim();
    stdout_json(&run_verify(
        &case,
        verifier_spec(&case, "git_branch_exists_v1", json!({ "branch": branch })),
    ));

    let out = case.root.join("evidence-report.json");
    let before = ledger_row_count(&case.ledger);
    let export = stdout_json(&export_evidence(&case, &case.run_id, &out));
    let after = ledger_row_count(&case.ledger);
    assert_eq!(before, after, "export must not mutate ledger");
    assert!(out.exists(), "export file must exist on disk");

    let bytes = fs::read(&out).expect("read exported evidence file");
    assert_eq!(export["size_bytes"], bytes.len());
    let parsed: Value = serde_json::from_slice(&bytes).expect("export JSON parses");
    assert_eq!(parsed["format"], "blackbox_evidence_report_json_v1");
    assert_eq!(parsed["pdf_ready"], true);
    assert_eq!(parsed["report"]["summary"]["kinds"]["evidence_packet"], 1);
    assert_eq!(
        parsed["report"]["evidence_hash_verification"]["status"],
        "passed"
    );
}

#[test]
fn phase4_edge_cases_fail_closed_without_mutating_or_hiding_state() {
    let case = new_phase4_case("phase4 replay/export edge cases");
    let before = ledger_row_count(&case.ledger);

    let replay_missing = replay(&case, "run_missing_phase4");
    assert_eq!(
        stderr_json(&replay_missing)["error_code"],
        "BBX_RECORD_NOT_FOUND"
    );
    assert_eq!(ledger_row_count(&case.ledger), before);

    let missing_out = case.root.join("missing-run-export.json");
    let export_missing = export_evidence(&case, "run_missing_phase4", &missing_out);
    assert_eq!(
        stderr_json(&export_missing)["error_code"],
        "BBX_RECORD_NOT_FOUND"
    );
    assert!(!missing_out.exists());
    assert_eq!(ledger_row_count(&case.ledger), before);

    let out_dir = case.root.join("export-dir");
    fs::create_dir(&out_dir).expect("create output directory edge fixture");
    let export_to_dir = export_evidence(&case, &case.run_id, &out_dir);
    let error = stderr_json(&export_to_dir);
    assert_eq!(error["error_code"], "BBX_EXPORT_PATH_INVALID");
    assert_eq!(error["field_path"], "--out");
    assert_eq!(ledger_row_count(&case.ledger), before);
}
