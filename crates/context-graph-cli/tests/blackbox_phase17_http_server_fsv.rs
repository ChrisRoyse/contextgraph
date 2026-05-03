//! BlackBoxFix Phase 17 HTTP server FSV tests.
//!
//! These tests execute the real `context-graph blackbox server serve` binary,
//! send real HTTP requests, and then inspect the physical SQLite ledger
//! directly. No mocked transport, database, or source-of-truth layer is used.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::Duration;

use reqwest::Client;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase17_test";
const OTHER_TENANT: &str = "tenant_phase17_other";
const AGENT: &str = "agent_phase17_test";
const API_KEY_ENV: &str = "BLACKBOX_PHASE17_API_KEY";
const API_KEY: &str = "phase17-test-api-key-000000000000000000000000";

struct ServerGuard {
    child: Child,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
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

fn init_ledger(ledger: &Path) {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "init",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
    ]));
}

fn free_loopback_addr() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind free loopback port");
    let addr = listener.local_addr().expect("read local addr");
    drop(listener);
    addr
}

async fn start_server(ledger: &Path, addr: SocketAddr) -> ServerGuard {
    let child = Command::new(context_graph_bin())
        .args([
            "blackbox",
            "server",
            "serve",
            "--ledger",
            ledger.to_str().expect("ledger path UTF-8"),
            "--bind",
            &addr.to_string(),
            "--api-key-env",
            API_KEY_ENV,
        ])
        .env(API_KEY_ENV, API_KEY)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn phase17 HTTP server");
    let guard = ServerGuard { child };
    let client = Client::new();
    let health_url = format!("http://{addr}/v1/health");
    for _ in 0..80 {
        match client.get(&health_url).send().await {
            Ok(response) if response.status().is_success() => return guard,
            _ => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    panic!("phase17 HTTP server did not become healthy at {health_url}");
}

fn ledger_kind_counts(ledger: &Path) -> BTreeMap<String, i64> {
    let conn = Connection::open(ledger).expect("open SQLite ledger");
    let mut stmt = conn
        .prepare(
            "SELECT record_kind, COUNT(*)
             FROM ledger_records
             GROUP BY record_kind
             ORDER BY record_kind",
        )
        .expect("prepare kind count query");
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("query kind counts");
    rows.map(|row| row.expect("decode kind count"))
        .collect::<BTreeMap<_, _>>()
}

fn kind_count(ledger: &Path, kind: &str) -> i64 {
    *ledger_kind_counts(ledger).get(kind).unwrap_or(&0)
}

fn latest_payload(ledger: &Path, kind: &str) -> Value {
    let conn = Connection::open(ledger).expect("open SQLite ledger");
    let payload: String = conn
        .query_row(
            "SELECT payload_json
             FROM ledger_records
             WHERE record_kind = ?1
             ORDER BY sequence DESC
             LIMIT 1",
            params![kind],
            |row| row.get(0),
        )
        .expect("read latest payload");
    serde_json::from_str(&payload).expect("payload JSON parses")
}

fn latest_audit_tuple(ledger: &Path) -> (i64, String, i64, Option<String>) {
    let conn = Connection::open(ledger).expect("open SQLite ledger");
    conn.query_row(
        "SELECT sequence,
                json_extract(payload_json, '$.request.route_id'),
                json_extract(payload_json, '$.response.status_code'),
                json_extract(payload_json, '$.response.error_code')
         FROM ledger_records
         WHERE record_kind = 'http_request_audit'
         ORDER BY sequence DESC
         LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .expect("read latest audit tuple")
}

fn print_state(label: &str, ledger: &Path) {
    println!("FSV17 {label} counts={:?}", ledger_kind_counts(ledger));
    if kind_count(ledger, "http_request_audit") > 0 {
        println!(
            "FSV17 {label} latest_audit={:?}",
            latest_audit_tuple(ledger)
        );
    }
}

async fn authed_post(client: &Client, url: &str, body: Value) -> reqwest::Response {
    client
        .post(url)
        .bearer_auth(API_KEY)
        .header("X-Blackbox-Tenant", TENANT)
        .json(&body)
        .send()
        .await
        .expect("send authenticated POST")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn phase17_http_server_persists_and_audits_real_ledger_state() {
    let temp = TempDir::new().expect("tempdir");
    let ledger = temp.path().join("blackbox.sqlite");
    init_ledger(&ledger);
    let addr = free_loopback_addr();
    let _server = start_server(&ledger, addr).await;
    let client = Client::new();

    print_state("happy_before_run", &ledger);
    let run_response = authed_post(
        &client,
        &format!("http://{addr}/v1/runs"),
        json!({
            "tenant": TENANT,
            "task": "phase17 HTTP run",
            "agent": AGENT,
            "model": "phase17-model",
            "task_kind": "http_agent_task",
            "risk_tier": "low"
        }),
    )
    .await;
    assert_eq!(run_response.status(), 200);
    let run_json: Value = run_response.json().await.expect("run response JSON");
    let run_id = run_json["run_id"].as_str().expect("run_id").to_string();
    print_state("happy_after_run", &ledger);
    assert_eq!(kind_count(&ledger, "agent_run"), 1);
    assert_eq!(kind_count(&ledger, "http_request_audit"), 1);
    let run_payload = latest_payload(&ledger, "agent_run");
    assert_eq!(run_payload["runtime"]["framework"], "http_server");
    assert_eq!(run_payload["task"]["requested_outcome"], "phase17 HTTP run");
    assert_eq!(latest_audit_tuple(&ledger).1, "POST /v1/runs");

    print_state("happy_before_step", &ledger);
    let step_response = authed_post(
        &client,
        &format!("http://{addr}/v1/runs/{run_id}/steps"),
        json!({
            "tenant": TENANT,
            "summary": "phase17 recorded real HTTP step",
            "step_kind": "http_step",
            "agent": AGENT,
            "model": "phase17-model",
            "context": {"source": "phase17_fsv"}
        }),
    )
    .await;
    assert_eq!(step_response.status(), 200);
    print_state("happy_after_step", &ledger);
    assert_eq!(kind_count(&ledger, "agent_step"), 1);
    assert_eq!(kind_count(&ledger, "http_request_audit"), 2);

    print_state("happy_before_tool", &ledger);
    let tool_response = authed_post(
        &client,
        &format!("http://{addr}/v1/tool-calls"),
        json!({
            "tenant": TENANT,
            "run_id": run_id,
            "tool": "phase17.synthetic.write",
            "input": {
                "path": "app.txt",
                "api_key": "phase17-secret-value-that-must-be-redacted"
            },
            "agent": AGENT,
            "model": "phase17-model",
            "capability": "local.tool",
            "status": "ok",
            "external_request_id": "phase17-ext-001"
        }),
    )
    .await;
    assert_eq!(tool_response.status(), 200);
    print_state("happy_after_tool", &ledger);
    assert_eq!(kind_count(&ledger, "tool_call"), 1);
    assert_eq!(kind_count(&ledger, "http_request_audit"), 3);
    let tool_payload = latest_payload(&ledger, "tool_call");
    assert_eq!(tool_payload["input_redacted"]["path"], "app.txt");
    assert!(tool_payload["input_redacted"]["api_key"]
        .as_str()
        .expect("redacted api_key")
        .starts_with("[REDACTED:sha256:"));

    let summary_before = kind_count(&ledger, "http_request_audit");
    let summary_response = client
        .get(format!("http://{addr}/v1/dashboard/summary"))
        .bearer_auth(API_KEY)
        .header("X-Blackbox-Tenant", TENANT)
        .send()
        .await
        .expect("send dashboard request");
    assert_eq!(summary_response.status(), 200);
    let summary: Value = summary_response.json().await.expect("summary JSON");
    print_state("happy_after_summary", &ledger);
    assert_eq!(summary["tenant_id"], TENANT);
    assert_eq!(summary["data"]["counts"]["record_kinds"]["agent_run"], 1);
    assert_eq!(summary["data"]["counts"]["record_kinds"]["tool_call"], 1);
    assert_eq!(
        kind_count(&ledger, "http_request_audit"),
        summary_before + 1
    );

    let auth_before = ledger_kind_counts(&ledger);
    print_state("edge_invalid_auth_before", &ledger);
    let invalid_auth = client
        .post(format!("http://{addr}/v1/runs"))
        .bearer_auth("wrong-phase17-api-key-00000000000000000000")
        .header("X-Blackbox-Tenant", TENANT)
        .json(&json!({
            "tenant": TENANT,
            "task": "must not persist",
            "agent": AGENT,
            "model": null,
            "task_kind": "http_agent_task",
            "risk_tier": "low"
        }))
        .send()
        .await
        .expect("send invalid auth request");
    assert_eq!(invalid_auth.status(), 401);
    let invalid_auth_json: Value = invalid_auth.json().await.expect("invalid auth JSON");
    assert_eq!(invalid_auth_json["error_code"], "HTTP_AUTH_INVALID");
    print_state("edge_invalid_auth_after", &ledger);
    assert_eq!(kind_count(&ledger, "agent_run"), auth_before["agent_run"]);
    assert_eq!(
        kind_count(&ledger, "http_request_audit"),
        auth_before["http_request_audit"] + 1
    );
    assert_eq!(latest_audit_tuple(&ledger).2, 401);
    assert_eq!(
        latest_audit_tuple(&ledger).3.as_deref(),
        Some("HTTP_AUTH_INVALID")
    );

    let mismatch_before = ledger_kind_counts(&ledger);
    print_state("edge_tenant_mismatch_before", &ledger);
    let tenant_mismatch = authed_post(
        &client,
        &format!("http://{addr}/v1/runs"),
        json!({
            "tenant": OTHER_TENANT,
            "task": "must not persist",
            "agent": AGENT,
            "model": null,
            "task_kind": "http_agent_task",
            "risk_tier": "low"
        }),
    )
    .await;
    assert_eq!(tenant_mismatch.status(), 400);
    let tenant_mismatch_json: Value = tenant_mismatch.json().await.expect("tenant mismatch JSON");
    assert_eq!(tenant_mismatch_json["error_code"], "HTTP_TENANT_MISMATCH");
    print_state("edge_tenant_mismatch_after", &ledger);
    assert_eq!(
        kind_count(&ledger, "agent_run"),
        mismatch_before["agent_run"]
    );
    assert_eq!(
        kind_count(&ledger, "http_request_audit"),
        mismatch_before["http_request_audit"] + 1
    );

    let large_before = ledger_kind_counts(&ledger);
    print_state("edge_large_body_before", &ledger);
    let large_task = "x".repeat(HTTP_TEST_LARGE_BODY_BYTES);
    let large_body = format!(
        r#"{{"tenant":"{TENANT}","task":"{large_task}","agent":"{AGENT}","model":null,"task_kind":"http_agent_task","risk_tier":"low"}}"#
    );
    let large_response = client
        .post(format!("http://{addr}/v1/runs"))
        .bearer_auth(API_KEY)
        .header("X-Blackbox-Tenant", TENANT)
        .header("Content-Type", "application/json")
        .body(large_body)
        .send()
        .await
        .expect("send oversized body");
    assert_eq!(large_response.status(), 413);
    let large_json: Value = large_response.json().await.expect("large body JSON");
    assert_eq!(large_json["error_code"], "HTTP_BODY_TOO_LARGE");
    print_state("edge_large_body_after", &ledger);
    assert_eq!(kind_count(&ledger, "agent_run"), large_before["agent_run"]);
    assert_eq!(
        kind_count(&ledger, "http_request_audit"),
        large_before["http_request_audit"] + 1
    );

    println!(
        "FSV17 source_of_truth=sqlite:{}#ledger_records final_counts={:?}",
        ledger.display(),
        ledger_kind_counts(&ledger)
    );
}

const HTTP_TEST_LARGE_BODY_BYTES: usize = 70 * 1024;
