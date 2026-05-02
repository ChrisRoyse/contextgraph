//! BlackBoxFix Phase 5 episode export FSV tests.
//!
//! These tests execute the real `context-graph blackbox` binary against real
//! SQLite ledgers, real Git repositories, and real exported files. Assertions
//! inspect the physical ledger and output artifacts directly.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase5_test";
const AGENT: &str = "agent_phase5_test";

struct Phase5Case {
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
    must_git(repo, &["config", "user.email", "phase5@example.test"]);
    must_git(repo, &["config", "user.name", "Phase5 Test"]);
    fs::write(repo.join("app.txt"), "phase5\n").expect("write tracked file");
    must_git(repo, &["add", "app.txt"]);
    must_git(repo, &["commit", "-m", "initial phase5 fixture"]);
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
        "phase5_real_tool",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--capability",
        "phase5.real_tool.write",
    ]))
}

fn new_phase5_case(task: &str) -> Phase5Case {
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
            "operation": "phase5-export",
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

    Phase5Case {
        _temp: temp,
        root,
        ledger,
        repo,
        run_id,
        tool_call_id,
    }
}

fn verifier_spec(case: &Phase5Case, expected: Value) -> Value {
    json!({
        "verifier_id": "git_branch_exists_v1",
        "run_id": case.run_id,
        "tool_call_id": case.tool_call_id,
        "source": { "repo": case.repo.display().to_string() },
        "expected": expected
    })
}

fn run_verify(case: &Phase5Case, spec: Value) -> Output {
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

fn export_episodes(case: &Phase5Case, out: &Path, force: bool) -> Output {
    let mut args = vec![
        "blackbox",
        "export",
        "episodes",
        "--ledger",
        case.ledger.to_str().expect("ledger path UTF-8"),
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

fn sha256_file(path: &Path) -> String {
    let bytes = fs::read(path).expect("read file for sha256");
    let digest = Sha256::digest(&bytes);
    format!("sha256:{digest:x}")
}

fn parse_jsonl(path: &Path) -> Vec<Value> {
    let text = fs::read_to_string(path).expect("read JSONL");
    text.lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("parse JSONL line"))
        .collect()
}

fn run_pass_and_failure(case: &Phase5Case) {
    let branch = must_git(&case.repo, &["branch", "--show-current"]);
    let branch = branch.trim();
    let pass = stdout_json(&run_verify(
        case,
        verifier_spec(case, json!({ "branch": branch })),
    ));
    assert_eq!(pass["verifier_status"], "passed");

    let fail = run_verify(
        case,
        verifier_spec(case, json!({ "branch": "missing-phase5-branch" })),
    );
    assert_eq!(stderr_json(&fail)["error_code"], "GIT_BRANCH_NOT_FOUND");
}

#[test]
fn phase5_export_episodes_writes_jsonl_manifest_and_eval_fixtures() {
    let case = new_phase5_case("phase5 episode export happy path");
    run_pass_and_failure(&case);
    let before_rows = ledger_row_count(&case.ledger);
    let out = case.root.join("episodes-export");

    let export = stdout_json(&export_episodes(&case, &out, false));
    let after_rows = ledger_row_count(&case.ledger);
    assert_eq!(
        before_rows, after_rows,
        "episode export must not mutate ledger"
    );
    assert_eq!(export["episodes_total"], 3);
    assert_eq!(export["counts"]["splits"]["passed"], 1);
    assert_eq!(export["counts"]["splits"]["failed"], 1);
    assert_eq!(export["counts"]["splits"]["incident"], 1);

    let episodes = out.join("episodes.jsonl");
    let manifest = out.join("manifest.json");
    let evals = out.join("evals");
    assert!(episodes.exists(), "episodes.jsonl must exist");
    assert!(manifest.exists(), "manifest.json must exist");
    assert!(evals.is_dir(), "evals directory must exist");

    let rows = parse_jsonl(&episodes);
    assert_eq!(rows.len(), 3);
    for row in &rows {
        let evidence_id = row["source"]["evidence_packet_id"]
            .as_str()
            .expect("row evidence id");
        assert!(
            evidence_packet_exists(&case.ledger, evidence_id),
            "episode evidence packet must exist in physical ledger: {evidence_id}"
        );
        assert!(row["source"]["ledger_sequence_range"].is_array());
    }
    assert!(rows
        .iter()
        .any(|row| row["record_kind"] == "incident_agent_episode"));

    let manifest_json: Value =
        serde_json::from_str(&fs::read_to_string(&manifest).expect("read manifest"))
            .expect("parse manifest");
    assert_eq!(manifest_json["status"], "ok");
    assert_eq!(
        manifest_json["format"],
        "blackbox_episodes_manifest_json_v1"
    );
    assert_eq!(
        manifest_json["output"]["episodes_sha256"],
        sha256_file(&episodes)
    );
    assert_eq!(manifest_json["output"]["readback"]["line_count"], 3);
    assert_eq!(manifest_json["evals"]["fixture_count"], 2);

    let eval_files = fs::read_dir(&evals)
        .expect("read eval dir")
        .map(|entry| entry.expect("eval entry").path())
        .collect::<Vec<_>>();
    assert_eq!(eval_files.len(), 2);
    for eval_file in eval_files {
        let fixture: Value =
            serde_json::from_str(&fs::read_to_string(&eval_file).expect("read eval fixture"))
                .expect("parse eval fixture");
        assert_eq!(fixture["format"], "blackbox_eval_fixture_json_v1");
        assert_eq!(fixture["expected"]["error_code"], "GIT_BRANCH_NOT_FOUND");
        let evidence_id = fixture["evidence_packet_id"]
            .as_str()
            .expect("fixture evidence id");
        assert!(evidence_packet_exists(&case.ledger, evidence_id));
    }

    let counts = ledger_kind_counts(&case.ledger);
    assert_eq!(counts.get("evidence_packet").copied().unwrap_or(0), 2);
    assert_eq!(counts.get("incident_packet").copied().unwrap_or(0), 1);
}

#[test]
fn phase5_existing_output_without_force_fails_before_writing() {
    let case = new_phase5_case("phase5 output exists edge");
    run_pass_and_failure(&case);
    let out = case.root.join("existing-export");
    fs::create_dir(&out).expect("create existing output dir");
    fs::write(out.join("marker.txt"), "keep").expect("write marker");
    let before = ledger_row_count(&case.ledger);

    let output = export_episodes(&case, &out, false);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "BBX_OUTPUT_EXISTS");
    assert_eq!(error["connector"], "fs_local_v1");
    assert_eq!(ledger_row_count(&case.ledger), before);
    assert!(out.join("marker.txt").exists());
    assert!(!out.join("episodes.jsonl").exists());
    assert!(!out.join("manifest.json").exists());
}

#[test]
fn phase5_hash_chain_break_blocks_export_before_files_are_written() {
    let case = new_phase5_case("phase5 tamper edge");
    run_pass_and_failure(&case);
    let out = case.root.join("tampered-export");
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

    let output = export_episodes(&case, &out, false);
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "BBX_HASH_CHAIN_BROKEN");
    assert_eq!(ledger_row_count(&case.ledger), before);
    assert!(!out.exists(), "tampered export must not create output dir");
}

#[test]
fn phase5_unverified_tool_call_blocks_export_before_files_are_written() {
    let case = new_phase5_case("phase5 missing verifier edge");
    let out = case.root.join("unverified-export");
    let before = ledger_row_count(&case.ledger);

    let output = export_episodes(&case, &out, false);
    let error = stderr_json(&output);
    assert_eq!(
        error["error_code"],
        "BBX_VERIFIER_MISSING_FOR_MUTATING_ACTION"
    );
    assert_eq!(ledger_row_count(&case.ledger), before);
    assert!(
        !out.exists(),
        "unverified export must not create output dir"
    );
}
