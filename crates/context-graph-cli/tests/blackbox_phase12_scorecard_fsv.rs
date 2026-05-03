//! BlackBoxFix Phase 12 scorecard FSV tests.
//!
//! These tests execute the real `context-graph blackbox` binary against a real
//! SQLite ledger and a real local Git repository. Assertions inspect the
//! physical ledger source of truth directly: `ledger_records` scorecard rows.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase12_test";
const AGENT: &str = "agent_phase12_test";

struct Phase12Case {
    _temp: TempDir,
    root: PathBuf,
    ledger: PathBuf,
    repo: PathBuf,
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

fn init_git_repo(repo: &Path) {
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
    must_git(repo, &["config", "user.email", "phase12@example.test"]);
    must_git(repo, &["config", "user.name", "Phase12 Test"]);
    fs::write(repo.join("app.txt"), "phase12\n").expect("write tracked file");
    must_git(repo, &["add", "app.txt"]);
    must_git(repo, &["commit", "-m", "initial phase12 fixture"]);
}

fn new_case() -> Phase12Case {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();
    let ledger = root.join("blackbox.sqlite");
    let repo = root.join("repo");
    stdout_json(&run_context_graph(&[
        "blackbox",
        "init",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
    ]));
    init_git_repo(&repo);
    Phase12Case {
        _temp: temp,
        root,
        ledger,
        repo,
    }
}

fn record_run(ledger: &Path, task: &str, risk_tier: &str) -> String {
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
        risk_tier,
    ]));
    output["run_id"].as_str().expect("run_id").to_string()
}

fn record_tool_call(ledger: &Path, root: &Path, run_id: &str, label: &str) -> String {
    let input = root.join(format!("{label}-tool-input.json"));
    write_json(
        &input,
        &json!({
            "operation": label,
            "expected_effect": "phase12 scorecard fixture"
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
        "phase12_real_tool",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--capability",
        "local.git.apply_patch",
    ]));
    output["tool_call_id"]
        .as_str()
        .expect("tool_call_id")
        .to_string()
}

fn policy_allow_git_apply_patch(ledger: &Path, run_id: &str) {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "policy",
        "evaluate",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--mode",
        "bounded_autonomous",
        "--tool",
        "verified_git_apply_patch",
        "--capability",
        "local.git.apply_patch",
        "--run-id",
        run_id,
        "--mutating",
        "--verifier-id",
        "git_file_diff_matches_patch_v1",
        "--source-scope",
        "git:/phase12/repo#working_tree",
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
    ]));
}

fn verify_working_tree_clean(
    ledger: &Path,
    root: &Path,
    repo: &Path,
    run_id: &str,
    tool_call_id: &str,
    clean: bool,
) -> Output {
    let spec = root.join(format!("{run_id}-{tool_call_id}-verify.json"));
    write_json(
        &spec,
        &json!({
            "verifier_id": "git_working_tree_clean_v1",
            "run_id": run_id,
            "tool_call_id": tool_call_id,
            "source": { "repo": repo },
            "expected": { "clean": clean }
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

fn score_run(ledger: &Path, run_id: &str) -> Value {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "score",
        "run",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--run-id",
        run_id,
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
    ]))
}

fn kind_counts(ledger: &Path) -> BTreeMap<String, i64> {
    let conn = Connection::open(ledger).expect("open ledger");
    let mut stmt = conn
        .prepare("SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind")
        .expect("prepare kind count query");
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("query kind counts");
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .expect("decode kind counts")
}

fn latest_scorecard_payload(ledger: &Path, run_id: &str) -> Value {
    let conn = Connection::open(ledger).expect("open ledger");
    let payload_json: String = conn
        .query_row(
            "SELECT payload_json
             FROM ledger_records
             WHERE record_kind = 'scorecard' AND run_id = ?1
             ORDER BY sequence DESC
             LIMIT 1",
            params![run_id],
            |row| row.get(0),
        )
        .expect("read latest scorecard payload");
    serde_json::from_str(&payload_json).expect("scorecard payload JSON")
}

fn scorecard_count(ledger: &Path) -> i64 {
    let conn = Connection::open(ledger).expect("open ledger");
    conn.query_row(
        "SELECT COUNT(*) FROM ledger_records WHERE record_kind = 'scorecard'",
        [],
        |row| row.get(0),
    )
    .expect("count scorecards")
}

#[test]
fn phase12_scorecard_persists_happy_path_and_edges() {
    let case = new_case();
    println!(
        "FSV12 source_of_truth=sqlite:{}#ledger_records",
        case.ledger.display()
    );
    println!(
        "FSV12 git_source_before={}",
        must_git(
            &case.repo,
            &["status", "--porcelain=v1", "--untracked-files=all"]
        )
    );
    println!("FSV12 BEFORE counts: {:?}", kind_counts(&case.ledger));

    let happy_run = record_run(
        &case.ledger,
        "phase12 verified scorecard happy path",
        "medium",
    );
    policy_allow_git_apply_patch(&case.ledger, &happy_run);
    let happy_tool = record_tool_call(&case.ledger, &case.root, &happy_run, "happy");
    stdout_json(&verify_working_tree_clean(
        &case.ledger,
        &case.root,
        &case.repo,
        &happy_run,
        &happy_tool,
        true,
    ));
    let happy_score = score_run(&case.ledger, &happy_run);
    let happy_stored = latest_scorecard_payload(&case.ledger, &happy_run);
    println!("FSV12 happy AFTER counts: {:?}", kind_counts(&case.ledger));
    println!("FSV12 happy STORED scorecard: {}", happy_stored);
    assert_eq!(happy_stored["scorecard_id"], happy_score["scorecard_id"]);
    assert_eq!(happy_stored["score"], happy_score["score"]);
    assert_eq!(happy_stored["score"]["trust_level"], "verified_with_policy");
    assert_eq!(happy_stored["score"]["correctness"], 1.0);
    assert_eq!(happy_stored["score"]["verification_confidence"], 0.9725);
    assert_eq!(happy_stored["score"]["risk"], 0.58);
    assert_eq!(happy_stored["score"]["training_value"], 0.2);
    assert_eq!(
        happy_stored["score_inputs"]["policy_decision_counts"]["allowed"],
        1
    );
    assert_eq!(happy_stored["score_inputs"]["tool_calls_with_evidence"], 1);
    assert_eq!(happy_score["readback"]["status"], "passed");

    let no_tool_before = kind_counts(&case.ledger);
    let no_tool_run = record_run(&case.ledger, "phase12 no tool-call edge", "low");
    let no_tool_score = score_run(&case.ledger, &no_tool_run);
    let no_tool_stored = latest_scorecard_payload(&case.ledger, &no_tool_run);
    println!(
        "FSV12 no-tool BEFORE counts={:?} AFTER score={}",
        no_tool_before, no_tool_stored
    );
    assert_eq!(
        no_tool_stored["scorecard_id"],
        no_tool_score["scorecard_id"]
    );
    assert_eq!(no_tool_stored["score"]["trust_level"], "recorded");
    assert!(no_tool_stored["score"]["correctness"].is_null());
    assert_eq!(no_tool_stored["score"]["verification_confidence"], 0.0);
    assert_eq!(no_tool_stored["score"]["risk"], 0.2);
    assert!(no_tool_stored["score_inputs"]["evidence_coverage_ratio"].is_null());

    let unverified_before = kind_counts(&case.ledger);
    let unverified_run = record_run(&case.ledger, "phase12 unverified tool-call edge", "high");
    record_tool_call(&case.ledger, &case.root, &unverified_run, "unverified");
    let unverified_score = score_run(&case.ledger, &unverified_run);
    let unverified_stored = latest_scorecard_payload(&case.ledger, &unverified_run);
    println!(
        "FSV12 unverified BEFORE counts={:?} AFTER score={}",
        unverified_before, unverified_stored
    );
    assert_eq!(
        unverified_stored["scorecard_id"],
        unverified_score["scorecard_id"]
    );
    assert_eq!(unverified_stored["score"]["trust_level"], "recorded");
    assert!(unverified_stored["score"]["correctness"].is_null());
    assert_eq!(unverified_stored["score"]["risk"], 1.0);
    assert_eq!(
        unverified_stored["score_inputs"]["evidence_coverage_ratio"],
        0.0
    );
    assert_eq!(
        unverified_stored["score_inputs"]["policy_compliance"],
        "policy_missing"
    );

    fs::write(case.repo.join("dirty.txt"), "uncommitted\n").expect("write dirty git file");
    let failed_before = kind_counts(&case.ledger);
    let failed_run = record_run(&case.ledger, "phase12 failed verifier incident edge", "low");
    let failed_tool = record_tool_call(&case.ledger, &case.root, &failed_run, "failed");
    let failed_verify_error = stderr_json(&verify_working_tree_clean(
        &case.ledger,
        &case.root,
        &case.repo,
        &failed_run,
        &failed_tool,
        true,
    ));
    assert_eq!(failed_verify_error["error_code"], "GIT_WORKING_TREE_DIRTY");
    let failed_score = score_run(&case.ledger, &failed_run);
    let failed_stored = latest_scorecard_payload(&case.ledger, &failed_run);
    println!(
        "FSV12 failed-verifier BEFORE counts={:?} VERIFY error={} AFTER score={}",
        failed_before, failed_verify_error, failed_stored
    );
    assert_eq!(failed_stored["scorecard_id"], failed_score["scorecard_id"]);
    assert_eq!(failed_stored["score"]["trust_level"], "incident");
    assert_eq!(failed_stored["score"]["correctness"], 0.0);
    assert_eq!(failed_stored["score"]["risk"], 0.5);
    assert_eq!(failed_stored["score"]["training_value"], 0.55);
    assert_eq!(failed_stored["score_inputs"]["incident_packets_total"], 1);
    assert_eq!(
        failed_stored["score_inputs"]["verifier_status_counts"]["failed"],
        1
    );

    let before_missing = scorecard_count(&case.ledger);
    let missing_error = stderr_json(&run_context_graph(&[
        "blackbox",
        "score",
        "run",
        "--ledger",
        case.ledger.to_str().expect("ledger path UTF-8"),
        "--run-id",
        "run_missing_phase12",
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
    ]));
    let after_missing = scorecard_count(&case.ledger);
    println!(
        "FSV12 missing-run BEFORE scorecards={} AFTER scorecards={} error={}",
        before_missing, after_missing, missing_error
    );
    assert_eq!(missing_error["error_code"], "SCORECARD_RUN_NOT_FOUND");
    assert_eq!(before_missing, after_missing);

    let final_counts = kind_counts(&case.ledger);
    println!("FSV12 FINAL physical counts: {:?}", final_counts);
    assert_eq!(final_counts.get("agent_run").copied().unwrap_or(0), 4);
    assert_eq!(final_counts.get("policy_decision").copied().unwrap_or(0), 1);
    assert_eq!(final_counts.get("tool_call").copied().unwrap_or(0), 3);
    assert_eq!(final_counts.get("expected_delta").copied().unwrap_or(0), 2);
    assert_eq!(
        final_counts.get("readback_snapshot").copied().unwrap_or(0),
        2
    );
    assert_eq!(final_counts.get("verifier_result").copied().unwrap_or(0), 2);
    assert_eq!(final_counts.get("evidence_packet").copied().unwrap_or(0), 2);
    assert_eq!(final_counts.get("incident_packet").copied().unwrap_or(0), 1);
    assert_eq!(final_counts.get("scorecard").copied().unwrap_or(0), 4);
}
