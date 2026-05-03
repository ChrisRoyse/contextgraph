//! BlackBoxFix Phase 9 GitLab CI FSV tests.
//!
//! These tests use live GitLab source-of-truth data. They do not mock the
//! GitLab API: the test independently discovers fixture state with `curl`,
//! then the CLI reads GitLab through `gitlab_ci_v1`, and the assertions read
//! the physical SQLite ledger as durable proof.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::Connection;
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase9_gitlab_test";
const AGENT: &str = "agent_phase9_gitlab_test";

struct GitlabFixture {
    project: String,
    project_id: u64,
    pipeline_id: u64,
    pipeline_status: String,
    pipeline_ref: String,
    pipeline_sha: String,
    jobs_total: usize,
}

struct Phase9Case {
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

fn gitlab_project() -> String {
    std::env::var("BBX_GITLAB_TEST_PROJECT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "gitlab-org/gitlab-runner".to_string())
}

fn gitlab_project_api_path(project: &str) -> String {
    project.trim_matches('/').replace('/', "%2F")
}

fn gitlab_api_json(path_and_query: &str) -> Value {
    let mut command = Command::new("curl");
    command
        .arg("-fsSL")
        .arg("--header")
        .arg("Accept: application/json")
        .arg("--header")
        .arg("User-Agent: contextgraph-blackbox-phase9-tests");

    if let Some(token) = std::env::var("BBX_GITLAB_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("GITLAB_TOKEN")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
    {
        command
            .arg("--header")
            .arg(format!("PRIVATE-TOKEN: {token}"));
    }

    let output = command
        .arg(format!("https://gitlab.com/api/v4/{path_and_query}"))
        .output()
        .expect("spawn curl for live GitLab source-of-truth read");
    assert!(
        output.status.success(),
        "GitLab source-of-truth curl failed\npath={path_and_query}\nstatus={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("GitLab source-of-truth JSON")
}

fn discover_fixture() -> GitlabFixture {
    let project = gitlab_project();
    let encoded = gitlab_project_api_path(&project);
    let project_body = gitlab_api_json(&format!("projects/{encoded}"));
    let project_id = project_body["id"].as_u64().expect("fixture project id");
    assert_eq!(
        project_body["path_with_namespace"]
            .as_str()
            .expect("fixture project path"),
        project
    );

    let pipelines = gitlab_api_json(&format!(
        "projects/{encoded}/pipelines?status=success&per_page=20"
    ));
    for pipeline in pipelines.as_array().expect("fixture pipelines array") {
        let pipeline_id = pipeline["id"].as_u64().expect("fixture pipeline id");
        let jobs = gitlab_api_json(&format!(
            "projects/{encoded}/pipelines/{pipeline_id}/jobs?per_page=100&include_retried=false"
        ));
        let job_items = jobs.as_array().expect("fixture jobs array");
        if !job_items.is_empty()
            && job_items.len() < 100
            && job_items
                .iter()
                .all(|job| job["status"].as_str() == Some("success"))
        {
            return GitlabFixture {
                project,
                project_id,
                pipeline_id,
                pipeline_status: pipeline["status"]
                    .as_str()
                    .expect("fixture pipeline status")
                    .to_string(),
                pipeline_ref: pipeline["ref"]
                    .as_str()
                    .expect("fixture pipeline ref")
                    .to_string(),
                pipeline_sha: pipeline["sha"]
                    .as_str()
                    .expect("fixture pipeline sha")
                    .to_string(),
                jobs_total: job_items.len(),
            };
        }
    }

    panic!(
        "fixture project {project} must expose a successful pipeline with a non-empty first page of successful jobs"
    );
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
        "phase9_gitlab_ci_read",
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
        "--capability",
        "phase9.gitlab_ci.read",
    ]))
}

fn new_phase9_case(fixture: &GitlabFixture) -> Phase9Case {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();
    let ledger = root.join("blackbox.sqlite");
    let tool_input = root.join("tool-input.json");

    init_ledger(&ledger);
    write_json(
        &tool_input,
        &json!({
            "operation": "phase9-live-gitlab-ci-read",
            "project": fixture.project,
            "project_id": fixture.project_id,
            "pipeline_id": fixture.pipeline_id,
        }),
    );
    let run = record_run(&ledger, "phase9 live GitLab CI source-of-truth test");
    let run_id = run["run_id"].as_str().expect("run id").to_string();
    let tool = record_tool_call(&ledger, &run_id, &tool_input);
    let tool_call_id = tool["tool_call_id"]
        .as_str()
        .expect("tool call id")
        .to_string();

    Phase9Case {
        _temp: temp,
        root,
        ledger,
        run_id,
        tool_call_id,
    }
}

fn verifier_spec(case: &Phase9Case, verifier_id: &str, source: Value, expected: Value) -> Value {
    json!({
        "verifier_id": verifier_id,
        "run_id": case.run_id,
        "tool_call_id": case.tool_call_id,
        "source": source,
        "expected": expected
    })
}

fn source(fixture: &GitlabFixture) -> Value {
    json!({ "project": fixture.project })
}

fn run_verify(case: &Phase9Case, spec: Value) -> Output {
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

fn pipeline_expected(fixture: &GitlabFixture) -> Value {
    json!({
        "pipeline_id": fixture.pipeline_id,
        "status": fixture.pipeline_status,
        "ref": fixture.pipeline_ref,
        "sha": fixture.pipeline_sha,
    })
}

fn jobs_expected(fixture: &GitlabFixture) -> Value {
    json!({
        "pipeline_id": fixture.pipeline_id,
        "allowed_statuses": ["success"],
        "allow_failure_failed_ok": false,
        "minimum_jobs": fixture.jobs_total,
    })
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
    case: &Phase9Case,
    fixture: &GitlabFixture,
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
        before.get("expected_delta").copied().unwrap_or(0) + 1
    );
    assert_eq!(
        after.get("readback_snapshot").copied().unwrap_or(0),
        before.get("readback_snapshot").copied().unwrap_or(0) + 1
    );
    assert_eq!(
        after.get("verifier_result").copied().unwrap_or(0),
        before.get("verifier_result").copied().unwrap_or(0) + 1
    );
    assert_latest_verifier(&case.ledger, verifier_id, "failed", Some(error_code))
}

fn assert_blocked_edge(
    case: &Phase9Case,
    fixture: &GitlabFixture,
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
        before.get("expected_delta").copied().unwrap_or(0) + 1
    );
    assert_eq!(
        after.get("readback_snapshot").copied().unwrap_or(0),
        before.get("readback_snapshot").copied().unwrap_or(0)
    );
    assert_eq!(
        after.get("verifier_result").copied().unwrap_or(0),
        before.get("verifier_result").copied().unwrap_or(0) + 1
    );
    assert_latest_verifier(&case.ledger, verifier_id, "blocked", Some(error_code))
}

#[test]
fn phase9_live_gitlab_ci_fsv() {
    let fixture = discover_fixture();
    let case = new_phase9_case(&fixture);

    let list = stdout_json(&run_context_graph(&["blackbox", "connector", "list"]));
    assert!(list["connectors"]
        .as_array()
        .expect("connectors")
        .iter()
        .any(|item| item["connector_id"] == "gitlab_ci_v1"));

    let connector = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "gitlab_ci_v1",
        "--source",
        &fixture.project,
    ]));
    assert_eq!(
        connector["readback"]["project"]["path_with_namespace"],
        fixture.project
    );
    assert_eq!(connector["readback"]["project"]["id"], fixture.project_id);

    let pipeline_output = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "gitlab_pipeline_status_v1",
            source(&fixture),
            pipeline_expected(&fixture),
        ),
    ));
    assert_eq!(pipeline_output["verifier_status"], "passed");
    let pipeline_payload =
        assert_latest_verifier(&case.ledger, "gitlab_pipeline_status_v1", "passed", None);
    assert_eq!(
        pipeline_payload["actual_readback"]["status"],
        fixture.pipeline_status
    );
    assert_eq!(
        pipeline_payload["actual_readback"]["sha"],
        fixture.pipeline_sha
    );

    let jobs_output = stdout_json(&run_verify(
        &case,
        verifier_spec(
            &case,
            "gitlab_pipeline_jobs_completed_v1",
            source(&fixture),
            jobs_expected(&fixture),
        ),
    ));
    assert_eq!(jobs_output["verifier_status"], "passed");
    let jobs_payload = assert_latest_verifier(
        &case.ledger,
        "gitlab_pipeline_jobs_completed_v1",
        "passed",
        None,
    );
    assert_eq!(
        jobs_payload["actual_readback"]["jobs_total"],
        fixture.jobs_total
    );
    assert_eq!(jobs_payload["actual_readback"]["mismatched_count"], 0);

    assert_kind_count(&case.ledger, "agent_run", 1);
    assert_kind_count(&case.ledger, "tool_call", 1);
    assert_kind_count(&case.ledger, "expected_delta", 2);
    assert_kind_count(&case.ledger, "readback_snapshot", 2);
    assert_kind_count(&case.ledger, "verifier_result", 2);
    assert_kind_count(&case.ledger, "evidence_packet", 2);
    assert_kind_count(&case.ledger, "incident_packet", 0);
    let readback = latest_payload(&case.ledger, "readback_snapshot");
    assert_eq!(readback["source_of_truth"]["kind"], "gitlab_ci_v1");

    let missing_pipeline = assert_failed_edge(
        &case,
        &fixture,
        "gitlab_pipeline_status_v1",
        json!({
            "pipeline_id": 9_999_999_999u64,
            "status": "success",
            "ref": fixture.pipeline_ref,
            "sha": fixture.pipeline_sha,
        }),
        "GITLAB_PIPELINE_NOT_FOUND",
    );
    assert_eq!(missing_pipeline["actual_readback"]["exists"], false);

    let wrong_status = assert_failed_edge(
        &case,
        &fixture,
        "gitlab_pipeline_status_v1",
        json!({
            "pipeline_id": fixture.pipeline_id,
            "status": "failed",
            "ref": fixture.pipeline_ref,
            "sha": fixture.pipeline_sha,
        }),
        "GITLAB_PIPELINE_STATUS_MISMATCH",
    );
    assert_eq!(
        wrong_status["actual_readback"]["status"],
        fixture.pipeline_status
    );

    let wrong_ref = assert_failed_edge(
        &case,
        &fixture,
        "gitlab_pipeline_status_v1",
        json!({
            "pipeline_id": fixture.pipeline_id,
            "status": fixture.pipeline_status,
            "ref": "bbx-phase9-wrong-ref",
            "sha": fixture.pipeline_sha,
        }),
        "GITLAB_PIPELINE_REF_MISMATCH",
    );
    assert_eq!(wrong_ref["actual_readback"]["ref"], fixture.pipeline_ref);

    assert_blocked_edge(
        &case,
        &fixture,
        "gitlab_pipeline_status_v1",
        json!({
            "pipeline_id": "not-an-integer",
            "status": "success",
            "ref": fixture.pipeline_ref,
            "sha": fixture.pipeline_sha,
        }),
        "VERIFIER_SPEC_FIELD_TYPE",
    );

    let missing_jobs = assert_failed_edge(
        &case,
        &fixture,
        "gitlab_pipeline_jobs_completed_v1",
        json!({
            "pipeline_id": 9_999_999_999u64,
            "allowed_statuses": ["success"],
            "allow_failure_failed_ok": false,
            "minimum_jobs": 1,
        }),
        "GITLAB_PIPELINE_JOBS_NOT_FOUND",
    );
    assert_eq!(missing_jobs["actual_readback"]["exists"], false);

    let wrong_job_status = assert_failed_edge(
        &case,
        &fixture,
        "gitlab_pipeline_jobs_completed_v1",
        json!({
            "pipeline_id": fixture.pipeline_id,
            "allowed_statuses": ["failed"],
            "allow_failure_failed_ok": false,
            "minimum_jobs": fixture.jobs_total,
        }),
        "GITLAB_PIPELINE_JOBS_STATUS_MISMATCH",
    );
    assert!(
        wrong_job_status["actual_readback"]["mismatched_count"]
            .as_u64()
            .expect("mismatched_count")
            > 0
    );

    let missing_minimum = assert_failed_edge(
        &case,
        &fixture,
        "gitlab_pipeline_jobs_completed_v1",
        json!({
            "pipeline_id": fixture.pipeline_id,
            "allowed_statuses": ["success"],
            "allow_failure_failed_ok": false,
            "minimum_jobs": fixture.jobs_total + 1,
        }),
        "GITLAB_PIPELINE_JOBS_MISSING",
    );
    assert_eq!(
        missing_minimum["actual_readback"]["jobs_total"],
        fixture.jobs_total
    );

    assert_blocked_edge(
        &case,
        &fixture,
        "gitlab_pipeline_jobs_completed_v1",
        json!({
            "pipeline_id": fixture.pipeline_id,
            "allowed_statuses": ["done"],
            "allow_failure_failed_ok": false,
            "minimum_jobs": fixture.jobs_total,
        }),
        "GITLAB_JOB_STATUS_INVALID",
    );

    assert_blocked_edge(
        &case,
        &fixture,
        "gitlab_pipeline_jobs_completed_v1",
        json!({
            "pipeline_id": fixture.pipeline_id,
            "allowed_statuses": ["success"],
            "allow_failure_failed_ok": false,
            "minimum_jobs": 0,
        }),
        "GITLAB_MINIMUM_JOBS_INVALID",
    );

    assert_kind_count(&case.ledger, "expected_delta", 11);
    assert_kind_count(&case.ledger, "readback_snapshot", 8);
    assert_kind_count(&case.ledger, "verifier_result", 11);
    assert_kind_count(&case.ledger, "evidence_packet", 11);
    assert_kind_count(&case.ledger, "incident_packet", 6);
}
