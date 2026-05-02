//! BlackBoxFix Phase 2 FSV tests.
//!
//! These tests execute the real `context-graph blackbox` binary against real
//! Git repositories, real filesystem artifacts, real commands, and then read
//! the physical SQLite ledger as the source of truth.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase2_test";
const AGENT: &str = "agent_phase2_test";

struct Phase2Case {
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
    must_git(repo, &["config", "user.email", "phase2@example.test"]);
    must_git(repo, &["config", "user.name", "Phase2 Test"]);
    fs::write(repo.join("app.txt"), "alpha\n").expect("write tracked file");
    must_git(repo, &["add", "app.txt"]);
    must_git(repo, &["commit", "-m", "initial phase2 fixture"]);
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
        "phase2_real_tool",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--capability",
        "phase2.real_tool.write",
    ]))
}

fn new_phase2_case(task: &str) -> Phase2Case {
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
            "operation": "phase2-real-write",
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

    Phase2Case {
        _temp: temp,
        root,
        ledger,
        repo,
        run_id,
        tool_call_id,
    }
}

fn verifier_spec(case: &Phase2Case, verifier_id: &str, source: Value, expected: Value) -> Value {
    json!({
        "verifier_id": verifier_id,
        "run_id": case.run_id,
        "tool_call_id": case.tool_call_id,
        "source": source,
        "expected": expected
    })
}

fn run_verify(case: &Phase2Case, spec: Value) -> Output {
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
        .expect("read latest verifier_result");
    serde_json::from_str(&payload).expect("verifier payload JSON")
}

fn latest_readback_payload(ledger: &Path) -> Value {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let payload: String = conn
        .query_row(
            "SELECT payload_json
             FROM ledger_records
             WHERE record_kind = 'readback_snapshot'
             ORDER BY sequence DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("read latest readback_snapshot");
    serde_json::from_str(&payload).expect("readback payload JSON")
}

fn assert_latest_verifier(
    ledger: &Path,
    verifier_id: &str,
    status: &str,
    error_code: Option<&str>,
) -> Value {
    let payload = latest_verifier_payload(ledger);
    assert_eq!(payload["verifier_id"], verifier_id);
    assert_eq!(payload["status"], status);
    if let Some(error_code) = error_code {
        assert_eq!(payload["error_code"], error_code);
    } else {
        assert!(payload["error_code"].is_null());
    }
    payload
}

fn assert_kind_count(ledger: &Path, kind: &str, expected: i64) {
    let counts = ledger_kind_counts(ledger);
    assert_eq!(
        counts.get(kind).copied().unwrap_or(0),
        expected,
        "ledger count for {kind}: {counts:?}"
    );
}

#[test]
fn phase2_connectors_read_real_local_sources() {
    let case = new_phase2_case("phase2 connector source-of-truth test");
    let file = case.root.join("fs-real.txt");
    fs::write(&file, "phase2 filesystem readback\n").expect("write fs source");

    let list = stdout_json(&run_context_graph(&["blackbox", "connector", "list"]));
    assert!(list["connectors"]
        .as_array()
        .expect("connectors")
        .iter()
        .any(|item| item["connector_id"] == "git_local_v1"));
    assert!(list["connectors"]
        .as_array()
        .expect("connectors")
        .iter()
        .any(|item| item["connector_id"] == "fs_local_v1"));
    assert!(list["connectors"]
        .as_array()
        .expect("connectors")
        .iter()
        .any(|item| item["connector_id"] == "cmd_local_v1"));

    let git = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "git_local_v1",
        "--source",
        case.repo.to_str().expect("repo path UTF-8"),
    ]));
    assert_eq!(
        git["readback"]["head"],
        must_git(&case.repo, &["rev-parse", "HEAD"]).trim()
    );

    let fs_read = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "fs_local_v1",
        "--source",
        file.to_str().expect("file path UTF-8"),
    ]));
    assert_eq!(fs_read["readback"]["size_bytes"], 27);

    let cmd = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "cmd_local_v1",
        "--source",
        case.root.to_str().expect("root path UTF-8"),
        "--command",
        "sh",
        "-c",
        "printf connector-phase2",
    ]));
    assert_eq!(cmd["readback"]["exit_code"], 0);
    assert_eq!(cmd["readback"]["stdout"], "connector-phase2");
}

#[test]
fn phase2_happy_path_verifiers_persist_physical_readbacks() {
    let case = new_phase2_case("phase2 happy path source-of-truth test");
    let branch = must_git(&case.repo, &["branch", "--show-current"]);
    let branch = branch.trim();
    let head = must_git(&case.repo, &["rev-parse", "HEAD"]);
    let head = head.trim();

    let branch_output = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "git_branch_exists_v1",
            json!({ "repo": case.repo.display().to_string() }),
            json!({ "branch": branch }),
        ),
    ));
    assert_eq!(branch_output["verifier_status"], "passed");
    let branch_payload =
        assert_latest_verifier(&case.ledger, "git_branch_exists_v1", "passed", None);
    assert_eq!(branch_payload["actual_readback"]["branch"], branch);
    assert_eq!(branch_payload["actual_readback"]["commit"], head);

    let commit_output = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "git_commit_exists_v1",
            json!({ "repo": case.repo.display().to_string() }),
            json!({ "commit": head }),
        ),
    ));
    assert_eq!(commit_output["verifier_status"], "passed");
    let commit_payload =
        assert_latest_verifier(&case.ledger, "git_commit_exists_v1", "passed", None);
    assert_eq!(commit_payload["actual_readback"]["commit"], head);
    assert_eq!(commit_payload["actual_readback"]["object_type"], "commit");

    let clean_output = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "git_working_tree_clean_v1",
            json!({ "repo": case.repo.display().to_string() }),
            json!({ "clean": true }),
        ),
    ));
    assert_eq!(clean_output["verifier_status"], "passed");
    let clean_payload =
        assert_latest_verifier(&case.ledger, "git_working_tree_clean_v1", "passed", None);
    assert_eq!(clean_payload["actual_readback"]["status_porcelain_v1"], "");
    assert_eq!(
        must_git(
            &case.repo,
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        ""
    );

    fs::write(case.repo.join("app.txt"), "alpha\nbeta\n").expect("modify tracked file");
    let patch = must_git(
        &case.repo,
        &["diff", "--no-ext-diff", "--no-color", "--", "app.txt"],
    );
    assert!(patch.contains("+beta"));
    let diff_output = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "git_file_diff_matches_patch_v1",
            json!({ "repo": case.repo.display().to_string() }),
            json!({ "path": "app.txt", "patch": patch }),
        ),
    ));
    assert_eq!(diff_output["verifier_status"], "passed");
    let diff_payload = assert_latest_verifier(
        &case.ledger,
        "git_file_diff_matches_patch_v1",
        "passed",
        None,
    );
    assert_eq!(
        diff_payload["actual_readback"]["patch"],
        must_git(
            &case.repo,
            &["diff", "--no-ext-diff", "--no-color", "--", "app.txt"]
        )
    );

    let artifact = case.root.join("cmd-output.txt");
    let cmd_output = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "cmd_exit_zero_v1",
            json!({ "cwd": case.root.display().to_string() }),
            json!({
                "command": ["sh", "-c", "printf phase2-command > cmd-output.txt"],
                "artifacts": ["cmd-output.txt"]
            }),
        ),
    ));
    assert_eq!(cmd_output["verifier_status"], "passed");
    assert_eq!(
        fs::read_to_string(&artifact).expect("artifact readback"),
        "phase2-command"
    );
    let cmd_payload = assert_latest_verifier(&case.ledger, "cmd_exit_zero_v1", "passed", None);
    assert_eq!(cmd_payload["actual_readback"]["exit_code"], 0);
    assert_eq!(
        cmd_payload["actual_readback"]["artifacts"][0]["exists"],
        true
    );

    assert_kind_count(&case.ledger, "agent_run", 1);
    assert_kind_count(&case.ledger, "tool_call", 1);
    assert_kind_count(&case.ledger, "expected_delta", 5);
    assert_kind_count(&case.ledger, "readback_snapshot", 5);
    assert_kind_count(&case.ledger, "verifier_result", 5);
    let readback = latest_readback_payload(&case.ledger);
    assert_eq!(readback["source_of_truth"]["kind"], "cmd_local_v1");
}

#[test]
fn phase2_branch_edge_missing_branch_persists_failed_result() {
    let case = new_phase2_case("branch missing source-of-truth test");
    let before = ledger_kind_counts(&case.ledger);
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "git_branch_exists_v1",
            json!({ "repo": case.repo.display().to_string() }),
            json!({ "branch": "missing-phase2-branch" }),
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], "GIT_BRANCH_NOT_FOUND");
    let after = ledger_kind_counts(&case.ledger);
    assert_eq!(before.get("verifier_result"), None);
    assert_eq!(after.get("expected_delta").copied(), Some(1));
    assert_eq!(after.get("readback_snapshot").copied(), Some(1));
    assert_latest_verifier(
        &case.ledger,
        "git_branch_exists_v1",
        "failed",
        Some("GIT_BRANCH_NOT_FOUND"),
    );
}

#[test]
fn phase2_branch_edge_non_git_repo_is_blocked_with_evidence() {
    let case = new_phase2_case("branch non-git source-of-truth test");
    let non_git = case.root.join("non-git");
    fs::create_dir(&non_git).expect("create non-git dir");
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "git_branch_exists_v1",
            json!({ "repo": non_git.display().to_string() }),
            json!({ "branch": "main" }),
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], "GIT_REPO_NOT_FOUND");
    assert_latest_verifier(
        &case.ledger,
        "git_branch_exists_v1",
        "blocked",
        Some("GIT_REPO_NOT_FOUND"),
    );
}

#[test]
fn phase2_branch_edge_missing_expected_field_is_blocked_after_expected_delta() {
    let case = new_phase2_case("branch schema source-of-truth test");
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "git_branch_exists_v1",
            json!({ "repo": case.repo.display().to_string() }),
            json!({}),
        ),
    );
    assert_eq!(
        stderr_json(&output)["error_code"],
        "VERIFIER_SPEC_MISSING_FIELD"
    );
    assert_kind_count(&case.ledger, "expected_delta", 1);
    assert_kind_count(&case.ledger, "readback_snapshot", 0);
    assert_latest_verifier(
        &case.ledger,
        "git_branch_exists_v1",
        "blocked",
        Some("VERIFIER_SPEC_MISSING_FIELD"),
    );
}

#[test]
fn phase2_commit_edge_missing_commit_persists_failed_result() {
    let case = new_phase2_case("commit missing source-of-truth test");
    let fake = "0000000000000000000000000000000000000000";
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "git_commit_exists_v1",
            json!({ "repo": case.repo.display().to_string() }),
            json!({ "commit": fake }),
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], "GIT_COMMIT_NOT_FOUND");
    let payload = assert_latest_verifier(
        &case.ledger,
        "git_commit_exists_v1",
        "failed",
        Some("GIT_COMMIT_NOT_FOUND"),
    );
    assert_eq!(payload["actual_readback"]["commit"], fake);
}

#[test]
fn phase2_commit_edge_existing_non_commit_object_fails() {
    let case = new_phase2_case("commit wrong-object source-of-truth test");
    let tree = must_git(&case.repo, &["rev-parse", "HEAD^{tree}"]);
    let tree = tree.trim();
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "git_commit_exists_v1",
            json!({ "repo": case.repo.display().to_string() }),
            json!({ "commit": tree }),
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], "GIT_COMMIT_NOT_FOUND");
    let payload = assert_latest_verifier(
        &case.ledger,
        "git_commit_exists_v1",
        "failed",
        Some("GIT_COMMIT_NOT_FOUND"),
    );
    assert_eq!(payload["actual_readback"]["exists"], false);
}

#[test]
fn phase2_commit_edge_non_git_repo_is_blocked() {
    let case = new_phase2_case("commit non-git source-of-truth test");
    let non_git = case.root.join("non-git");
    fs::create_dir(&non_git).expect("create non-git dir");
    let head = must_git(&case.repo, &["rev-parse", "HEAD"]);
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "git_commit_exists_v1",
            json!({ "repo": non_git.display().to_string() }),
            json!({ "commit": head.trim() }),
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], "GIT_REPO_NOT_FOUND");
    assert_latest_verifier(
        &case.ledger,
        "git_commit_exists_v1",
        "blocked",
        Some("GIT_REPO_NOT_FOUND"),
    );
}

#[test]
fn phase2_diff_edge_missing_path_fails_against_empty_physical_diff() {
    let case = new_phase2_case("diff missing path source-of-truth test");
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "git_file_diff_matches_patch_v1",
            json!({ "repo": case.repo.display().to_string() }),
            json!({ "path": "missing.txt", "patch": "not-the-physical-diff\n" }),
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], "GIT_FILE_DIFF_MISMATCH");
    let payload = assert_latest_verifier(
        &case.ledger,
        "git_file_diff_matches_patch_v1",
        "failed",
        Some("GIT_FILE_DIFF_MISMATCH"),
    );
    assert_eq!(payload["actual_readback"]["patch"], "");
}

#[test]
fn phase2_diff_edge_wrong_patch_fails_with_actual_patch_in_ledger() {
    let case = new_phase2_case("diff wrong patch source-of-truth test");
    fs::write(case.repo.join("app.txt"), "alpha\nchanged\n").expect("modify tracked file");
    let actual = must_git(
        &case.repo,
        &["diff", "--no-ext-diff", "--no-color", "--", "app.txt"],
    );
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "git_file_diff_matches_patch_v1",
            json!({ "repo": case.repo.display().to_string() }),
            json!({ "path": "app.txt", "patch": "wrong patch\n" }),
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], "GIT_FILE_DIFF_MISMATCH");
    let payload = assert_latest_verifier(
        &case.ledger,
        "git_file_diff_matches_patch_v1",
        "failed",
        Some("GIT_FILE_DIFF_MISMATCH"),
    );
    assert_eq!(payload["actual_readback"]["patch"], actual);
}

#[test]
fn phase2_diff_edge_non_git_repo_is_blocked() {
    let case = new_phase2_case("diff non-git source-of-truth test");
    let non_git = case.root.join("non-git");
    fs::create_dir(&non_git).expect("create non-git dir");
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "git_file_diff_matches_patch_v1",
            json!({ "repo": non_git.display().to_string() }),
            json!({ "path": "app.txt", "patch": "" }),
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], "GIT_REPO_NOT_FOUND");
    assert_latest_verifier(
        &case.ledger,
        "git_file_diff_matches_patch_v1",
        "blocked",
        Some("GIT_REPO_NOT_FOUND"),
    );
}

#[test]
fn phase2_clean_edge_tracked_dirty_file_fails() {
    let case = new_phase2_case("clean dirty tracked source-of-truth test");
    fs::write(case.repo.join("app.txt"), "alpha\ndirty\n").expect("dirty tracked file");
    let physical_status = must_git(
        &case.repo,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );
    assert!(physical_status.contains(" M app.txt"));
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "git_working_tree_clean_v1",
            json!({ "repo": case.repo.display().to_string() }),
            json!({ "clean": true }),
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], "GIT_WORKING_TREE_DIRTY");
    let payload = assert_latest_verifier(
        &case.ledger,
        "git_working_tree_clean_v1",
        "failed",
        Some("GIT_WORKING_TREE_DIRTY"),
    );
    assert_eq!(
        payload["actual_readback"]["status_porcelain_v1"],
        physical_status
    );
}

#[test]
fn phase2_clean_edge_untracked_file_fails() {
    let case = new_phase2_case("clean untracked source-of-truth test");
    fs::write(case.repo.join("untracked.txt"), "untracked\n").expect("write untracked file");
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "git_working_tree_clean_v1",
            json!({ "repo": case.repo.display().to_string() }),
            json!({ "clean": true }),
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], "GIT_WORKING_TREE_DIRTY");
    let payload = assert_latest_verifier(
        &case.ledger,
        "git_working_tree_clean_v1",
        "failed",
        Some("GIT_WORKING_TREE_DIRTY"),
    );
    assert!(payload["actual_readback"]["status_porcelain_v1"]
        .as_str()
        .expect("status")
        .contains("?? untracked.txt"));
}

#[test]
fn phase2_clean_edge_non_git_repo_is_blocked() {
    let case = new_phase2_case("clean non-git source-of-truth test");
    let non_git = case.root.join("non-git");
    fs::create_dir(&non_git).expect("create non-git dir");
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "git_working_tree_clean_v1",
            json!({ "repo": non_git.display().to_string() }),
            json!({ "clean": true }),
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], "GIT_REPO_NOT_FOUND");
    assert_latest_verifier(
        &case.ledger,
        "git_working_tree_clean_v1",
        "blocked",
        Some("GIT_REPO_NOT_FOUND"),
    );
}

#[test]
fn phase2_cmd_edge_nonzero_exit_fails_with_exit_code() {
    let case = new_phase2_case("cmd nonzero source-of-truth test");
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "cmd_exit_zero_v1",
            json!({ "cwd": case.root.display().to_string() }),
            json!({ "command": ["sh", "-c", "exit 7"], "artifacts": [] }),
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], "CMD_EXIT_NON_ZERO");
    let payload = assert_latest_verifier(
        &case.ledger,
        "cmd_exit_zero_v1",
        "failed",
        Some("CMD_EXIT_NON_ZERO"),
    );
    assert_eq!(payload["actual_readback"]["exit_code"], 7);
}

#[test]
fn phase2_cmd_edge_missing_artifact_fails_after_zero_exit() {
    let case = new_phase2_case("cmd missing artifact source-of-truth test");
    let artifact = case.root.join("missing-artifact.txt");
    assert!(!artifact.exists());
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "cmd_exit_zero_v1",
            json!({ "cwd": case.root.display().to_string() }),
            json!({ "command": ["sh", "-c", "printf no-artifact"], "artifacts": ["missing-artifact.txt"] }),
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], "CMD_ARTIFACT_MISSING");
    assert!(!artifact.exists());
    let payload = assert_latest_verifier(
        &case.ledger,
        "cmd_exit_zero_v1",
        "failed",
        Some("CMD_ARTIFACT_MISSING"),
    );
    assert_eq!(payload["actual_readback"]["artifacts"][0]["exists"], false);
}

#[test]
fn phase2_cmd_edge_missing_executable_is_blocked() {
    let case = new_phase2_case("cmd missing executable source-of-truth test");
    let missing = case.root.join("does-not-exist");
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "cmd_exit_zero_v1",
            json!({ "cwd": case.root.display().to_string() }),
            json!({ "command": [missing.display().to_string()], "artifacts": [] }),
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], "CMD_EXEC_FAILED");
    assert_latest_verifier(
        &case.ledger,
        "cmd_exit_zero_v1",
        "blocked",
        Some("CMD_EXEC_FAILED"),
    );
}

#[test]
fn phase2_cmd_schema_invalid_argv_item_blocks_before_command_execution() {
    let case = new_phase2_case("cmd invalid argv schema source-of-truth test");
    let artifact = case.root.join("should-not-exist.txt");
    let output = run_verify(
        &case,
        verifier_spec(
            &case,
            "cmd_exit_zero_v1",
            json!({ "cwd": case.root.display().to_string() }),
            json!({
                "command": ["sh", 7, "-c", "printf bad > should-not-exist.txt"],
                "artifacts": ["should-not-exist.txt"]
            }),
        ),
    );
    let error = stderr_json(&output);
    assert_eq!(error["error_code"], "VERIFIER_SPEC_FIELD_TYPE");
    assert_eq!(error["field_path"], "$.expected.command[1]");
    assert!(!artifact.exists(), "invalid argv must not execute command");
    assert_kind_count(&case.ledger, "expected_delta", 1);
    assert_kind_count(&case.ledger, "readback_snapshot", 0);
    assert_latest_verifier(
        &case.ledger,
        "cmd_exit_zero_v1",
        "blocked",
        Some("VERIFIER_SPEC_FIELD_TYPE"),
    );
}

#[test]
fn phase2_schema_v2_init_wipes_v1_ledger_instead_of_migrating_silently() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");

    let conn = Connection::open(&ledger).expect("open v1 ledger");
    conn.execute_batch(
        "CREATE TABLE blackbox_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO blackbox_metadata(key, value) VALUES ('schema_version', '1');
         CREATE TABLE ledger_records(sequence INTEGER PRIMARY KEY, payload_json TEXT NOT NULL);
         INSERT INTO ledger_records(sequence, payload_json) VALUES (1, '{\"legacy\":true}');",
    )
    .expect("create physical v1 ledger");
    drop(conn);

    let init = stdout_json(&run_context_graph(&[
        "blackbox",
        "init",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
    ]));
    assert_eq!(init["schema_version"], 2);
    assert_eq!(init["wiped_existing_ledger"], true);

    let conn = Connection::open(&ledger).expect("open physical v2 ledger");
    let version: String = conn
        .query_row(
            "SELECT value FROM blackbox_metadata WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .expect("read schema version");
    assert_eq!(version, "2");
    let record_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM ledger_records", [], |row| row.get(0))
        .expect("count physical v2 rows");
    assert_eq!(record_count, 0);

    let legacy_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name = 'ledger_records'",
            params![],
            |row| row.get(0),
        )
        .expect("read ledger_records table state");
    assert_eq!(legacy_rows, 1);
}
