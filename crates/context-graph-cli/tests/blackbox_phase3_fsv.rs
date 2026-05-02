//! BlackBoxFix Phase 3 FSV tests.
//!
//! These tests use live GitHub and GitHub Actions source-of-truth data. They do
//! not mock the GitHub API: the CLI reads GitHub via its connector, and the test
//! independently discovers fixture state with `curl`, then reads the physical
//! SQLite ledger as the durable source of truth.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::Connection;
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase3_test";
const AGENT: &str = "agent_phase3_test";

struct Phase3Fixture {
    owner: String,
    repo: String,
    default_branch: String,
    default_branch_sha: String,
    pr_number: u64,
    pr_state: String,
    pr_head_sha: String,
    issue_number: u64,
    issue_state: String,
    workflow_run_id: u64,
    workflow_status: String,
    workflow_conclusion: String,
    workflow_head_sha: String,
    check_suite_count: u64,
    check_suite_conclusion: String,
}

struct Phase3Case {
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

fn github_repo() -> (String, String) {
    let locator = std::env::var("BBX_GITHUB_TEST_REPO")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "actions/checkout".to_string());
    let parts = locator.trim().split('/').collect::<Vec<_>>();
    assert_eq!(
        parts.len(),
        2,
        "BBX_GITHUB_TEST_REPO must be OWNER/REPO, found {locator:?}"
    );
    (parts[0].to_string(), parts[1].to_string())
}

fn github_api_json(path_and_query: &str) -> Value {
    let mut command = Command::new("curl");
    command
        .arg("-fsSL")
        .arg("-H")
        .arg("Accept: application/vnd.github+json")
        .arg("-H")
        .arg("X-GitHub-Api-Version: 2022-11-28")
        .arg("-H")
        .arg("User-Agent: contextgraph-blackbox-phase3-tests");

    if let Some(token) = std::env::var("BBX_GITHUB_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("GITHUB_TOKEN")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
    {
        command
            .arg("-H")
            .arg(format!("Authorization: Bearer {token}"));
    }

    let output = command
        .arg(format!("https://api.github.com/{path_and_query}"))
        .output()
        .expect("spawn curl for live GitHub source-of-truth read");
    assert!(
        output.status.success(),
        "GitHub source-of-truth curl failed\npath={path_and_query}\nstatus={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("GitHub source-of-truth JSON")
}

fn discover_fixture() -> Phase3Fixture {
    let (owner, repo) = github_repo();
    let repo_body = github_api_json(&format!("repos/{owner}/{repo}"));
    let default_branch = repo_body["default_branch"]
        .as_str()
        .expect("fixture repository default_branch")
        .to_string();
    let default_branch_body = github_api_json(&format!(
        "repos/{owner}/{repo}/commits/heads/{default_branch}"
    ));
    let default_branch_sha = default_branch_body["sha"]
        .as_str()
        .expect("fixture default branch sha")
        .to_string();

    let pulls = github_api_json(&format!(
        "repos/{owner}/{repo}/pulls?state=open&per_page=10"
    ));
    let pr = pulls
        .as_array()
        .and_then(|items| items.first())
        .expect("fixture repository must expose at least one open PR");
    let pr_number = pr["number"].as_u64().expect("fixture PR number");
    let pr_state = pr["state"].as_str().expect("fixture PR state").to_string();
    let pr_head_sha = pr["head"]["sha"]
        .as_str()
        .expect("fixture PR head sha")
        .to_string();

    let issues = github_api_json(&format!(
        "repos/{owner}/{repo}/issues?state=open&per_page=100"
    ));
    let issue = issues
        .as_array()
        .and_then(|items| items.iter().find(|item| item.get("pull_request").is_none()))
        .expect("fixture repository must expose at least one open issue that is not a PR");
    let issue_number = issue["number"].as_u64().expect("fixture issue number");
    let issue_state = issue["state"]
        .as_str()
        .expect("fixture issue state")
        .to_string();

    let runs = github_api_json(&format!(
        "repos/{owner}/{repo}/actions/runs?status=success&per_page=5"
    ));
    let mut selected_run = None;
    let workflow_runs = runs["workflow_runs"]
        .as_array()
        .expect("fixture workflow_runs array");
    for run in workflow_runs {
        let head_sha = run["head_sha"].as_str().expect("fixture workflow head sha");
        let suites = github_api_json(&format!(
            "repos/{owner}/{repo}/commits/{head_sha}/check-suites?per_page=100"
        ));
        let suite_items = suites["check_suites"]
            .as_array()
            .expect("fixture check_suites array");
        let total_count = suites["total_count"]
            .as_u64()
            .unwrap_or(suite_items.len() as u64);
        if total_count > 0
            && suite_items
                .iter()
                .all(|suite| suite["conclusion"].as_str() == Some("success"))
        {
            selected_run = Some((run.clone(), total_count));
            break;
        }
    }
    let (run, check_suite_count) =
        selected_run.expect("fixture repository must have a successful run with successful checks");

    Phase3Fixture {
        owner,
        repo,
        default_branch,
        default_branch_sha,
        pr_number,
        pr_state,
        pr_head_sha,
        issue_number,
        issue_state,
        workflow_run_id: run["id"].as_u64().expect("fixture workflow run id"),
        workflow_status: run["status"]
            .as_str()
            .expect("fixture workflow status")
            .to_string(),
        workflow_conclusion: run["conclusion"]
            .as_str()
            .expect("fixture workflow conclusion")
            .to_string(),
        workflow_head_sha: run["head_sha"]
            .as_str()
            .expect("fixture workflow head sha")
            .to_string(),
        check_suite_count,
        check_suite_conclusion: "success".to_string(),
    }
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
        "phase3_live_github_tool",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--capability",
        "phase3.github.read",
    ]))
}

fn new_phase3_case(fixture: &Phase3Fixture) -> Phase3Case {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();
    let ledger = root.join("blackbox.sqlite");
    let tool_input = root.join("tool-input.json");

    init_ledger(&ledger);
    write_json(
        &tool_input,
        &json!({
            "operation": "phase3-live-github-read",
            "repo": format!("{}/{}", fixture.owner, fixture.repo),
            "default_branch": fixture.default_branch,
            "workflow_head_sha": fixture.workflow_head_sha,
        }),
    );
    let run = record_run(&ledger, "phase3 live GitHub/GHA source-of-truth test");
    let run_id = run["run_id"].as_str().expect("run id").to_string();
    let tool = record_tool_call(&ledger, &run_id, &tool_input);
    let tool_call_id = tool["tool_call_id"]
        .as_str()
        .expect("tool call id")
        .to_string();

    Phase3Case {
        _temp: temp,
        root,
        ledger,
        run_id,
        tool_call_id,
    }
}

fn verifier_spec(case: &Phase3Case, verifier_id: &str, source: Value, expected: Value) -> Value {
    json!({
        "verifier_id": verifier_id,
        "run_id": case.run_id,
        "tool_call_id": case.tool_call_id,
        "source": source,
        "expected": expected
    })
}

fn source(fixture: &Phase3Fixture) -> Value {
    json!({
        "owner": fixture.owner,
        "repo": fixture.repo,
    })
}

fn run_verify(case: &Phase3Case, spec: Value) -> Output {
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

fn assert_kind_count(ledger: &Path, kind: &str, expected: i64) {
    let counts = ledger_kind_counts(ledger);
    assert_eq!(
        counts.get(kind).copied().unwrap_or(0),
        expected,
        "ledger count for {kind}: {counts:?}"
    );
}

fn assert_latest_verifier(
    ledger: &Path,
    verifier_id: &str,
    status: &str,
    error_code: Option<&str>,
) -> Value {
    let payload = latest_payload(ledger, "verifier_result");
    assert_eq!(payload["verifier_id"], verifier_id);
    assert_eq!(payload["status"], status);
    if let Some(error_code) = error_code {
        assert_eq!(payload["error_code"], error_code);
    } else {
        assert!(payload["error_code"].is_null());
    }
    payload
}

fn assert_failed_edge(
    case: &Phase3Case,
    fixture: &Phase3Fixture,
    verifier_id: &str,
    expected: Value,
    error_code: &str,
) -> Value {
    let before = ledger_kind_counts(&case.ledger);
    let output = run_verify(
        case,
        verifier_spec(case, verifier_id, source(fixture), expected),
    );
    assert_eq!(stderr_json(&output)["error_code"], error_code);
    let after = ledger_kind_counts(&case.ledger);
    assert_eq!(
        after.get("expected_delta").copied().unwrap_or(0),
        before.get("expected_delta").copied().unwrap_or(0) + 1,
        "expected_delta count must increase for {verifier_id}"
    );
    assert_eq!(
        after.get("readback_snapshot").copied().unwrap_or(0),
        before.get("readback_snapshot").copied().unwrap_or(0) + 1,
        "readback_snapshot count must increase for failed readback edge {verifier_id}"
    );
    assert_eq!(
        after.get("verifier_result").copied().unwrap_or(0),
        before.get("verifier_result").copied().unwrap_or(0) + 1,
        "verifier_result count must increase for {verifier_id}"
    );
    assert_latest_verifier(&case.ledger, verifier_id, "failed", Some(error_code))
}

fn assert_blocked_edge(
    case: &Phase3Case,
    fixture: &Phase3Fixture,
    verifier_id: &str,
    source_override: Option<Value>,
    expected: Value,
    error_code: &str,
) -> Value {
    let before = ledger_kind_counts(&case.ledger);
    let source_value = source_override.unwrap_or_else(|| source(fixture));
    let output = run_verify(
        case,
        verifier_spec(case, verifier_id, source_value, expected),
    );
    assert_eq!(stderr_json(&output)["error_code"], error_code);
    let after = ledger_kind_counts(&case.ledger);
    assert_eq!(
        after.get("expected_delta").copied().unwrap_or(0),
        before.get("expected_delta").copied().unwrap_or(0) + 1,
        "expected_delta count must increase for blocked {verifier_id}"
    );
    assert_eq!(
        after.get("readback_snapshot").copied().unwrap_or(0),
        before.get("readback_snapshot").copied().unwrap_or(0),
        "blocked schema/source edges must not claim source-of-truth readback for {verifier_id}"
    );
    assert_eq!(
        after.get("verifier_result").copied().unwrap_or(0),
        before.get("verifier_result").copied().unwrap_or(0) + 1,
        "verifier_result count must increase for blocked {verifier_id}"
    );
    assert_latest_verifier(&case.ledger, verifier_id, "blocked", Some(error_code))
}

fn opposite_state(state: &str) -> &'static str {
    if state == "open" {
        "closed"
    } else {
        "open"
    }
}

fn wrong_conclusion(conclusion: &str) -> &'static str {
    if conclusion == "success" {
        "failure"
    } else {
        "success"
    }
}

#[test]
fn phase3_live_github_and_actions_fsv() {
    let fixture = discover_fixture();
    let case = new_phase3_case(&fixture);
    let repo_locator = format!("{}/{}", fixture.owner, fixture.repo);

    let list = stdout_json(&run_context_graph(&["blackbox", "connector", "list"]));
    assert!(list["connectors"]
        .as_array()
        .expect("connectors")
        .iter()
        .any(|item| item["connector_id"] == "github_v1"));
    assert!(list["connectors"]
        .as_array()
        .expect("connectors")
        .iter()
        .any(|item| item["connector_id"] == "gha_v1"));

    let github = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "github_v1",
        "--source",
        &repo_locator,
    ]));
    assert_eq!(github["readback"]["repository"]["full_name"], repo_locator);

    let gha = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "gha_v1",
        "--source",
        &repo_locator,
    ]));
    assert!(
        gha["readback"]["total_count"]
            .as_u64()
            .expect("workflow run count")
            > 0
    );

    let pr_output = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "github_pr_state_v1",
            source(&fixture),
            json!({ "pr_number": fixture.pr_number, "state": fixture.pr_state }),
        ),
    ));
    assert_eq!(pr_output["verifier_status"], "passed");
    let pr_payload = assert_latest_verifier(&case.ledger, "github_pr_state_v1", "passed", None);
    assert_eq!(pr_payload["actual_readback"]["state"], fixture.pr_state);
    assert_eq!(
        pr_payload["actual_readback"]["head_sha"],
        fixture.pr_head_sha
    );

    let branch_output = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "github_branch_head_matches_commit_v1",
            source(&fixture),
            json!({
                "branch": fixture.default_branch,
                "commit": fixture.default_branch_sha
            }),
        ),
    ));
    assert_eq!(branch_output["verifier_status"], "passed");
    let branch_payload = assert_latest_verifier(
        &case.ledger,
        "github_branch_head_matches_commit_v1",
        "passed",
        None,
    );
    assert_eq!(
        branch_payload["actual_readback"]["sha"],
        fixture.default_branch_sha
    );

    let issue_output = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "github_issue_state_changed_v1",
            source(&fixture),
            json!({ "issue_number": fixture.issue_number, "state": fixture.issue_state }),
        ),
    ));
    assert_eq!(issue_output["verifier_status"], "passed");
    let issue_payload = assert_latest_verifier(
        &case.ledger,
        "github_issue_state_changed_v1",
        "passed",
        None,
    );
    assert_eq!(
        issue_payload["actual_readback"]["state"],
        fixture.issue_state
    );
    assert_eq!(issue_payload["actual_readback"]["is_pull_request"], false);

    let checks_output = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "github_check_suite_passed_v1",
            source(&fixture),
            json!({
                "ref": fixture.workflow_head_sha,
                "conclusion": fixture.check_suite_conclusion
            }),
        ),
    ));
    assert_eq!(checks_output["verifier_status"], "passed");
    let checks_payload =
        assert_latest_verifier(&case.ledger, "github_check_suite_passed_v1", "passed", None);
    assert_eq!(
        checks_payload["actual_readback"]["total_count"],
        fixture.check_suite_count
    );
    assert_eq!(checks_payload["actual_readback"]["mismatched_count"], 0);

    let gha_output = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "gha_workflow_run_completed_v1",
            source(&fixture),
            json!({
                "run_id": fixture.workflow_run_id,
                "status": fixture.workflow_status,
                "conclusion": fixture.workflow_conclusion
            }),
        ),
    ));
    assert_eq!(gha_output["verifier_status"], "passed");
    let gha_payload = assert_latest_verifier(
        &case.ledger,
        "gha_workflow_run_completed_v1",
        "passed",
        None,
    );
    assert_eq!(
        gha_payload["actual_readback"]["status"],
        fixture.workflow_status
    );
    assert_eq!(
        gha_payload["actual_readback"]["conclusion"],
        fixture.workflow_conclusion
    );

    assert_kind_count(&case.ledger, "agent_run", 1);
    assert_kind_count(&case.ledger, "tool_call", 1);
    assert_kind_count(&case.ledger, "expected_delta", 5);
    assert_kind_count(&case.ledger, "readback_snapshot", 5);
    assert_kind_count(&case.ledger, "verifier_result", 5);
    let readback = latest_payload(&case.ledger, "readback_snapshot");
    assert_eq!(readback["source_of_truth"]["kind"], "gha_v1");

    let missing_pr = assert_failed_edge(
        &case,
        &fixture,
        "github_pr_state_v1",
        json!({ "pr_number": 999999999, "state": "open" }),
        "GITHUB_PR_NOT_FOUND",
    );
    assert_eq!(missing_pr["actual_readback"]["exists"], false);

    let pr_wrong_state = assert_failed_edge(
        &case,
        &fixture,
        "github_pr_state_v1",
        json!({
            "pr_number": fixture.pr_number,
            "state": opposite_state(&fixture.pr_state)
        }),
        "GITHUB_PR_STATE_MISMATCH",
    );
    assert_eq!(pr_wrong_state["actual_readback"]["state"], fixture.pr_state);

    assert_blocked_edge(
        &case,
        &fixture,
        "github_pr_state_v1",
        None,
        json!({ "pr_number": "not-an-integer", "state": "open" }),
        "VERIFIER_SPEC_FIELD_TYPE",
    );

    let missing_branch = assert_failed_edge(
        &case,
        &fixture,
        "github_branch_head_matches_commit_v1",
        json!({
            "branch": "bbx-missing-phase3-branch-0000",
            "commit": "0000000000000000000000000000000000000000"
        }),
        "GITHUB_BRANCH_NOT_FOUND",
    );
    assert_eq!(missing_branch["actual_readback"]["exists"], false);
    assert_eq!(missing_branch["actual_readback"]["http_status"], 422);

    let stale_branch = assert_failed_edge(
        &case,
        &fixture,
        "github_branch_head_matches_commit_v1",
        json!({
            "branch": fixture.default_branch,
            "commit": "0000000000000000000000000000000000000000"
        }),
        "GITHUB_BRANCH_HEAD_MISMATCH",
    );
    assert_eq!(
        stale_branch["actual_readback"]["sha"],
        fixture.default_branch_sha
    );

    assert_blocked_edge(
        &case,
        &fixture,
        "github_branch_head_matches_commit_v1",
        Some(json!({ "owner": 123, "repo": fixture.repo })),
        json!({ "branch": fixture.default_branch, "commit": fixture.default_branch_sha }),
        "VERIFIER_SPEC_FIELD_TYPE",
    );

    let missing_issue = assert_failed_edge(
        &case,
        &fixture,
        "github_issue_state_changed_v1",
        json!({ "issue_number": 999999999, "state": "open" }),
        "GITHUB_ISSUE_NOT_FOUND",
    );
    assert_eq!(missing_issue["actual_readback"]["exists"], false);

    let issue_wrong_state = assert_failed_edge(
        &case,
        &fixture,
        "github_issue_state_changed_v1",
        json!({
            "issue_number": fixture.issue_number,
            "state": opposite_state(&fixture.issue_state)
        }),
        "GITHUB_ISSUE_STATE_MISMATCH",
    );
    assert_eq!(
        issue_wrong_state["actual_readback"]["state"],
        fixture.issue_state
    );

    let issue_is_pr = assert_failed_edge(
        &case,
        &fixture,
        "github_issue_state_changed_v1",
        json!({ "issue_number": fixture.pr_number, "state": fixture.pr_state }),
        "GITHUB_ISSUE_IS_PULL_REQUEST",
    );
    assert_eq!(issue_is_pr["actual_readback"]["is_pull_request"], true);

    let missing_checks = assert_failed_edge(
        &case,
        &fixture,
        "github_check_suite_passed_v1",
        json!({
            "ref": "0000000000000000000000000000000000000000",
            "conclusion": "success"
        }),
        "GITHUB_CHECK_SUITE_NOT_FOUND",
    );
    assert_eq!(missing_checks["actual_readback"]["http_status"], 422);

    let checks_wrong_conclusion = assert_failed_edge(
        &case,
        &fixture,
        "github_check_suite_passed_v1",
        json!({
            "ref": fixture.workflow_head_sha,
            "conclusion": wrong_conclusion(&fixture.check_suite_conclusion)
        }),
        "GITHUB_CHECK_SUITE_CONCLUSION_MISMATCH",
    );
    assert!(
        checks_wrong_conclusion["actual_readback"]["mismatched_count"]
            .as_u64()
            .expect("mismatched_count")
            > 0
    );

    assert_blocked_edge(
        &case,
        &fixture,
        "github_check_suite_passed_v1",
        None,
        json!({ "ref": 42, "conclusion": "success" }),
        "VERIFIER_SPEC_FIELD_TYPE",
    );

    let missing_run = assert_failed_edge(
        &case,
        &fixture,
        "gha_workflow_run_completed_v1",
        json!({
            "run_id": 999999999999999999u64,
            "status": "completed",
            "conclusion": "success"
        }),
        "GHA_WORKFLOW_RUN_NOT_FOUND",
    );
    assert_eq!(missing_run["actual_readback"]["exists"], false);

    let run_wrong_conclusion = assert_failed_edge(
        &case,
        &fixture,
        "gha_workflow_run_completed_v1",
        json!({
            "run_id": fixture.workflow_run_id,
            "status": fixture.workflow_status,
            "conclusion": wrong_conclusion(&fixture.workflow_conclusion)
        }),
        "GHA_WORKFLOW_RUN_CONCLUSION_MISMATCH",
    );
    assert_eq!(
        run_wrong_conclusion["actual_readback"]["conclusion"],
        fixture.workflow_conclusion
    );

    assert_blocked_edge(
        &case,
        &fixture,
        "gha_workflow_run_completed_v1",
        None,
        json!({ "run_id": "not-an-integer", "status": "completed", "conclusion": "success" }),
        "VERIFIER_SPEC_FIELD_TYPE",
    );

    assert_kind_count(&case.ledger, "expected_delta", 20);
    assert_kind_count(&case.ledger, "readback_snapshot", 16);
    assert_kind_count(&case.ledger, "verifier_result", 20);
}
