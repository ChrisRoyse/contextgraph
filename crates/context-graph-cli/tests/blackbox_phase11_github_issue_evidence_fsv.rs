//! BlackBoxFix Phase 11 GitHub issue evidence-link FSV tests.
//!
//! These tests do not mock ticket state. They discover a real public GitHub
//! issue containing a real URL, run the real CLI verifier, and query the
//! physical SQLite ledger to verify persisted evidence.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use rusqlite::Connection;
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase11_github_issue_test";
const AGENT: &str = "agent_phase11_github_issue_test";

struct Phase11Fixture {
    owner: String,
    repo: String,
    issue_number: u64,
    issue_state: String,
    evidence_url: String,
    pr_number: u64,
    pr_state: String,
}

struct Phase11Case {
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

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn gh_auth_token() -> Option<String> {
    let output = Command::new("gh").args(["auth", "token"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn github_token() -> Option<String> {
    static TOKEN: OnceLock<Option<String>> = OnceLock::new();
    TOKEN
        .get_or_init(|| {
            ["BBX_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"]
                .into_iter()
                .find_map(non_empty_env)
                .or_else(gh_auth_token)
        })
        .clone()
}

fn github_auth_state() -> &'static str {
    if github_token().is_some() {
        "authenticated"
    } else {
        "unauthenticated"
    }
}

fn context_graph_command(args: &[&str]) -> Command {
    let mut command = Command::new(context_graph_bin());
    command.args(args);
    if let Some(token) = github_token() {
        command.env("GITHUB_TOKEN", token);
    }
    command
}

fn run_context_graph(args: &[&str]) -> Output {
    context_graph_command(args)
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
    let locator = std::env::var("BBX_GITHUB_TICKET_TEST_REPO")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("BBX_GITHUB_TEST_REPO")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "actions/checkout".to_string());
    let parts = locator.trim().split('/').collect::<Vec<_>>();
    assert_eq!(
        parts.len(),
        2,
        "BBX_GITHUB_TICKET_TEST_REPO must be OWNER/REPO, found {locator:?}"
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
        .arg("User-Agent: contextgraph-blackbox-phase11-tests");

    if let Some(token) = github_token() {
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
        "GitHub source-of-truth curl failed\npath={path_and_query}\nauth={}\nstatus={:?}\nstdout={}\nstderr={}\nhint=set BBX_GITHUB_TOKEN/GITHUB_TOKEN/GH_TOKEN or run `gh auth login` before live GitHub FSV tests; do not treat this as verifier success",
        github_auth_state(),
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("GitHub source-of-truth JSON")
}

fn extract_first_url(text: &str) -> Option<String> {
    let mut best_start = None;
    for marker in ["https://", "http://"] {
        if let Some(index) = text.find(marker) {
            best_start = Some(best_start.map_or(index, |current: usize| current.min(index)));
        }
    }
    let start = best_start?;
    let tail = &text[start..];
    let end = tail
        .find(|ch: char| {
            ch.is_whitespace() || matches!(ch, ')' | ']' | '}' | '>' | '"' | '\'' | '`' | '|')
        })
        .unwrap_or(tail.len());
    let url = tail[..end]
        .trim_end_matches(['.', ',', ';', ':', ')', ']'])
        .to_string();
    if url.starts_with("http://") || url.starts_with("https://") {
        Some(url)
    } else {
        None
    }
}

fn discover_fixture() -> Phase11Fixture {
    let (owner, repo) = github_repo();
    let pulls = github_api_json(&format!(
        "repos/{owner}/{repo}/pulls?state=open&per_page=10"
    ));
    let pr = pulls
        .as_array()
        .and_then(|items| items.first())
        .expect("fixture repository must expose at least one open PR");
    let pr_number = pr["number"].as_u64().expect("fixture PR number");
    let pr_state = pr["state"].as_str().expect("fixture PR state").to_string();

    let issues = github_api_json(&format!(
        "repos/{owner}/{repo}/issues?state=open&per_page=100"
    ));
    let issue = issues
        .as_array()
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("pull_request").is_none()
                    && item
                        .get("body")
                        .and_then(Value::as_str)
                        .and_then(extract_first_url)
                        .is_some()
            })
        })
        .expect(
            "fixture repository must expose an open issue body containing an http(s) evidence URL",
        );
    let issue_number = issue["number"].as_u64().expect("fixture issue number");
    let issue_state = issue["state"]
        .as_str()
        .expect("fixture issue state")
        .to_string();
    let body = issue["body"].as_str().expect("fixture issue body");
    let evidence_url = extract_first_url(body).expect("fixture evidence URL");

    let direct_issue = github_api_json(&format!("repos/{owner}/{repo}/issues/{issue_number}"));
    assert_eq!(direct_issue["number"], issue_number);
    assert!(
        direct_issue["body"]
            .as_str()
            .expect("direct issue body")
            .contains(&evidence_url),
        "direct GitHub source-of-truth read must contain discovered evidence URL"
    );

    Phase11Fixture {
        owner,
        repo,
        issue_number,
        issue_state,
        evidence_url,
        pr_number,
        pr_state,
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

fn record_run(ledger: &Path) -> Value {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "record",
        "run",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--task",
        "phase11 live GitHub issue evidence-link source-of-truth test",
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--risk-tier",
        "medium",
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
        "phase11_github_issue_evidence_read",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--capability",
        "phase11.github.issue.evidence.read",
    ]))
}

fn new_phase11_case(fixture: &Phase11Fixture) -> Phase11Case {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();
    let ledger = root.join("blackbox.sqlite");
    let tool_input = root.join("tool-input.json");

    init_ledger(&ledger);
    write_json(
        &tool_input,
        &json!({
            "operation": "phase11-live-github-issue-evidence-read",
            "repo": format!("{}/{}", fixture.owner, fixture.repo),
            "issue_number": fixture.issue_number,
            "evidence_url": fixture.evidence_url
        }),
    );
    let run = record_run(&ledger);
    let run_id = run["run_id"].as_str().expect("run id").to_string();
    let tool = record_tool_call(&ledger, &run_id, &tool_input);
    let tool_call_id = tool["tool_call_id"]
        .as_str()
        .expect("tool call id")
        .to_string();

    Phase11Case {
        _temp: temp,
        root,
        ledger,
        run_id,
        tool_call_id,
    }
}

fn verifier_spec(case: &Phase11Case, verifier_id: &str, source: Value, expected: Value) -> Value {
    json!({
        "verifier_id": verifier_id,
        "run_id": case.run_id,
        "tool_call_id": case.tool_call_id,
        "source": source,
        "expected": expected
    })
}

fn source(fixture: &Phase11Fixture) -> Value {
    json!({
        "owner": fixture.owner,
        "repo": fixture.repo,
    })
}

fn run_verify(case: &Phase11Case, spec: Value) -> Output {
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

fn assert_edge(
    case: &Phase11Case,
    fixture: &Phase11Fixture,
    expected: Value,
    status: &str,
    error_code: &str,
    readback_delta: i64,
    incident_delta: i64,
) -> Value {
    assert_edge_with_source(
        case,
        source(fixture),
        expected,
        status,
        error_code,
        readback_delta,
        incident_delta,
    )
}

fn assert_edge_with_source(
    case: &Phase11Case,
    source_value: Value,
    expected: Value,
    status: &str,
    error_code: &str,
    readback_delta: i64,
    incident_delta: i64,
) -> Value {
    let before = ledger_kind_counts(&case.ledger);
    let output = run_verify(
        case,
        verifier_spec(
            case,
            "github_issue_evidence_link_v1",
            source_value,
            expected,
        ),
    );
    assert_eq!(stderr_json(&output)["error_code"], error_code);
    let after = ledger_kind_counts(&case.ledger);
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
    assert_latest_verifier(
        &case.ledger,
        "github_issue_evidence_link_v1",
        status,
        Some(error_code),
    )
}

#[test]
fn phase11_live_github_issue_evidence_link_fsv() {
    let fixture = discover_fixture();
    let case = new_phase11_case(&fixture);
    let repo_locator = format!("{}/{}", fixture.owner, fixture.repo);

    let list = stdout_json(&run_context_graph(&["blackbox", "connector", "list"]));
    assert!(list["connectors"]
        .as_array()
        .expect("connectors")
        .iter()
        .any(|item| {
            item["connector_id"] == "github_v1"
                && item["readback_specs"]
                    .as_array()
                    .expect("github readback specs")
                    .iter()
                    .any(|spec| spec == "github_issue_evidence_link_v1")
        }));

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

    let happy = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "github_issue_evidence_link_v1",
            source(&fixture),
            json!({
                "issue_number": fixture.issue_number,
                "evidence_url": fixture.evidence_url,
                "link_scope": "body_or_comments"
            }),
        ),
    ));
    assert_eq!(happy["verifier_status"], "passed");
    assert_eq!(happy["actual_readback"]["evidence_link_present"], true);
    assert!(
        happy["actual_readback"]["matched_count"]
            .as_u64()
            .expect("matched_count")
            > 0
    );

    assert_kind_count(&case.ledger, "agent_run", 1);
    assert_kind_count(&case.ledger, "tool_call", 1);
    assert_kind_count(&case.ledger, "expected_delta", 1);
    assert_kind_count(&case.ledger, "readback_snapshot", 1);
    assert_kind_count(&case.ledger, "verifier_result", 1);
    assert_kind_count(&case.ledger, "evidence_packet", 1);
    assert_kind_count(&case.ledger, "incident_packet", 0);
    let readback = latest_payload(&case.ledger, "readback_snapshot");
    assert_eq!(readback["source_of_truth"]["kind"], "github_v1");
    assert_eq!(
        readback["observed_state"]["observed"]["evidence_url"],
        fixture.evidence_url
    );

    let missing_issue = assert_edge(
        &case,
        &fixture,
        json!({
            "issue_number": 999999999u64,
            "evidence_url": fixture.evidence_url,
            "link_scope": "body_or_comments"
        }),
        "failed",
        "GITHUB_ISSUE_NOT_FOUND",
        1,
        1,
    );
    assert_eq!(missing_issue["actual_readback"]["exists"], false);

    let wrong_link = assert_edge(
        &case,
        &fixture,
        json!({
            "issue_number": fixture.issue_number,
            "evidence_url": "https://example.invalid/blackboxfix/phase11/not-present",
            "link_scope": "body_or_comments"
        }),
        "failed",
        "GITHUB_ISSUE_EVIDENCE_LINK_NOT_FOUND",
        1,
        1,
    );
    assert_eq!(
        wrong_link["actual_readback"]["evidence_link_present"],
        false
    );

    let issue_is_pr = assert_edge(
        &case,
        &fixture,
        json!({
            "issue_number": fixture.pr_number,
            "evidence_url": fixture.evidence_url,
            "link_scope": "body_or_comments"
        }),
        "failed",
        "GITHUB_ISSUE_IS_PULL_REQUEST",
        1,
        1,
    );
    assert_eq!(issue_is_pr["actual_readback"]["is_pull_request"], true);
    assert_eq!(fixture.pr_state, "open");
    assert_eq!(fixture.issue_state, "open");

    assert_edge(
        &case,
        &fixture,
        json!({
            "issue_number": fixture.issue_number,
            "evidence_url": "not-a-url",
            "link_scope": "body_or_comments"
        }),
        "blocked",
        "GITHUB_ISSUE_EVIDENCE_URL_INVALID",
        0,
        0,
    );

    assert_edge(
        &case,
        &fixture,
        json!({
            "issue_number": fixture.issue_number,
            "evidence_url": fixture.evidence_url,
            "link_scope": "timeline"
        }),
        "blocked",
        "GITHUB_ISSUE_EVIDENCE_SCOPE_INVALID",
        0,
        0,
    );

    assert_edge(
        &case,
        &fixture,
        json!({
            "issue_number": 0,
            "evidence_url": fixture.evidence_url,
            "link_scope": "body_or_comments"
        }),
        "blocked",
        "GITHUB_ISSUE_NUMBER_INVALID",
        0,
        0,
    );

    assert_edge_with_source(
        &case,
        json!({
            "owner": format!("{}/{}", fixture.owner, fixture.repo),
            "repo": "checkout?fragment=true",
        }),
        json!({
            "issue_number": fixture.issue_number,
            "evidence_url": fixture.evidence_url,
            "link_scope": "body_or_comments"
        }),
        "blocked",
        "GITHUB_REPO_LOCATOR_INVALID",
        0,
        0,
    );

    assert_kind_count(&case.ledger, "expected_delta", 8);
    assert_kind_count(&case.ledger, "readback_snapshot", 4);
    assert_kind_count(&case.ledger, "verifier_result", 8);
    assert_kind_count(&case.ledger, "evidence_packet", 8);
    assert_kind_count(&case.ledger, "incident_packet", 3);
}
