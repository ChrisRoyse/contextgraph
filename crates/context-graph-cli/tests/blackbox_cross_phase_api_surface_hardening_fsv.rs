//! Cross-phase API surface hardening FSV tests.
//!
//! The source of truth for served HTTP routes is the real Phase 17 router, not
//! the command output alone. These tests execute the CLI, start the local HTTP
//! server, make real requests, and inspect the physical SQLite ledger.

use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::Duration;

use reqwest::Client;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_cross_phase_surface";
const API_KEY_ENV: &str = "BLACKBOX_CROSS_PHASE_API_KEY";
const API_KEY: &str = "cross-phase-api-key-000000000000000000000000";

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

fn api_surface(ledger: &Path) -> Value {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "api",
        "surface",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
    ]))
}

fn endpoint_key(endpoint: &Value) -> String {
    let method = endpoint["method"].as_str().expect("endpoint method");
    let path = endpoint["path"].as_str().expect("endpoint path");
    format!("{method} {path}")
}

fn endpoint_keys(value: &Value, field: &str) -> BTreeSet<String> {
    value[field]
        .as_array()
        .expect("endpoint array")
        .iter()
        .map(endpoint_key)
        .collect()
}

fn planned_endpoint<'a>(value: &'a Value, key: &str) -> &'a Value {
    value["planned_http_endpoints"]
        .as_array()
        .expect("planned endpoint array")
        .iter()
        .find(|endpoint| endpoint_key(endpoint) == key)
        .expect("planned endpoint exists")
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
        .expect("spawn cross-phase HTTP server");
    let guard = ServerGuard { child };
    let client = Client::new();
    let health_url = format!("http://{addr}/v1/health");
    for _ in 0..80 {
        match client.get(&health_url).send().await {
            Ok(response) if response.status().is_success() => return guard,
            _ => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    panic!("cross-phase HTTP server did not become healthy at {health_url}");
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

fn audit_rows(ledger: &Path) -> Vec<(i64, String, i64, Option<String>)> {
    let conn = Connection::open(ledger).expect("open SQLite ledger");
    let mut stmt = conn
        .prepare(
            "SELECT sequence,
                    json_extract(payload_json, '$.request.route_id'),
                    json_extract(payload_json, '$.response.status_code'),
                    json_extract(payload_json, '$.response.error_code')
             FROM ledger_records
             WHERE record_kind = 'http_request_audit'
             ORDER BY sequence",
        )
        .expect("prepare audit readback query");
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .expect("query audit rows");
    rows.map(|row| row.expect("decode audit row")).collect()
}

fn latest_audit_row(ledger: &Path) -> (i64, String, i64, Option<String>) {
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
        params![],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .expect("read latest audit row")
}

fn print_state(label: &str, ledger: &Path) {
    println!(
        "FSV_CROSS_PHASE {label} counts={:?} audit_rows={:?}",
        ledger_kind_counts(ledger),
        audit_rows(ledger)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api_surface_only_advertises_routes_that_the_http_router_serves() {
    let temp = TempDir::new().expect("tempdir");
    let ledger = temp.path().join("blackbox.sqlite");
    init_ledger(&ledger);

    let surface = api_surface(&ledger);
    let served = endpoint_keys(&surface, "served_http_endpoints");
    let endpoints = endpoint_keys(&surface, "endpoints");
    assert_eq!(endpoints, served);
    assert!(served.contains("GET /v1/health"));
    assert!(served.contains("GET /v1/dashboard/summary"));
    assert!(!served.contains("POST /v1/sdk/events"));
    assert!(!served.contains("POST /v1/replay"));

    let planned_sdk = planned_endpoint(&surface, "POST /v1/sdk/events");
    assert_eq!(planned_sdk["implemented"], false);
    assert_eq!(planned_sdk["expected_http_status_until_implemented"], 404);
    assert_eq!(
        planned_sdk["command_equivalent"],
        "blackbox sdk ingest --ledger PATH --input cloudevents.json"
    );
    let planned_replay = planned_endpoint(&surface, "POST /v1/replay");
    assert_eq!(planned_replay["implemented"], false);
    assert_eq!(
        planned_replay["expected_http_status_until_implemented"],
        404
    );

    println!(
        "FSV_CROSS_PHASE api_surface served={served:?} planned_sdk={planned_sdk} planned_replay={planned_replay}"
    );

    let addr = free_loopback_addr();
    let _server = start_server(&ledger, addr).await;
    let client = Client::new();

    print_state("happy_before_health", &ledger);
    let health = client
        .get(format!("http://{addr}/v1/health"))
        .send()
        .await
        .expect("send health request");
    assert_eq!(health.status(), 200);
    let health_json: Value = health.json().await.expect("health JSON");
    assert_eq!(health_json["status"], "ok");
    print_state("happy_after_health", &ledger);

    print_state("happy_before_run", &ledger);
    let audit_before_run = kind_count(&ledger, "http_request_audit");
    let run_response = client
        .post(format!("http://{addr}/v1/runs"))
        .bearer_auth(API_KEY)
        .header("X-Blackbox-Tenant", TENANT)
        .json(&json!({
            "tenant": TENANT,
            "task": "cross-phase API surface hardening run",
            "agent": "cross_phase_surface_agent",
            "model": "cross-phase-model",
            "task_kind": "api_surface_fsv",
            "risk_tier": "low"
        }))
        .send()
        .await
        .expect("send authenticated run request");
    assert_eq!(run_response.status(), 200);
    let run_json: Value = run_response.json().await.expect("run response JSON");
    assert!(!run_json["run_id"].as_str().expect("run_id").is_empty());
    print_state("happy_after_run", &ledger);
    assert_eq!(kind_count(&ledger, "agent_run"), 1);
    assert_eq!(
        kind_count(&ledger, "http_request_audit"),
        audit_before_run + 1
    );
    assert_eq!(
        latest_audit_row(&ledger),
        (2, "POST /v1/runs".to_string(), 200, None)
    );

    print_state("happy_before_summary", &ledger);
    let audit_before_summary = kind_count(&ledger, "http_request_audit");
    let summary = client
        .get(format!("http://{addr}/v1/dashboard/summary"))
        .bearer_auth(API_KEY)
        .header("X-Blackbox-Tenant", TENANT)
        .send()
        .await
        .expect("send authenticated summary request");
    assert_eq!(summary.status(), 200);
    let summary_json: Value = summary.json().await.expect("summary JSON");
    assert_eq!(summary_json["tenant_id"], TENANT);
    print_state("happy_after_summary", &ledger);
    assert_eq!(
        kind_count(&ledger, "http_request_audit"),
        audit_before_summary + 1
    );
    assert_eq!(
        latest_audit_row(&ledger),
        (3, "GET /v1/dashboard/summary".to_string(), 200, None)
    );

    print_state("edge_missing_auth_before", &ledger);
    let audit_before_missing_auth = kind_count(&ledger, "http_request_audit");
    let missing_auth = client
        .get(format!("http://{addr}/v1/dashboard/summary"))
        .send()
        .await
        .expect("send missing auth request");
    assert_eq!(missing_auth.status(), 401);
    let missing_auth_json: Value = missing_auth.json().await.expect("missing auth JSON");
    assert_eq!(missing_auth_json["error_code"], "HTTP_AUTH_MISSING");
    print_state("edge_missing_auth_after", &ledger);
    assert_eq!(
        kind_count(&ledger, "http_request_audit"),
        audit_before_missing_auth + 1
    );
    assert_eq!(
        latest_audit_row(&ledger),
        (
            4,
            "GET /v1/dashboard/summary".to_string(),
            401,
            Some("HTTP_AUTH_MISSING".to_string())
        )
    );

    print_state("edge_planned_sdk_before", &ledger);
    let before_sdk = ledger_kind_counts(&ledger);
    let sdk_response = client
        .post(format!("http://{addr}/v1/sdk/events"))
        .bearer_auth(API_KEY)
        .header("X-Blackbox-Tenant", TENANT)
        .json(&json!({"specversion": "1.0", "id": "not-routed", "source": "fsv"}))
        .send()
        .await
        .expect("send planned SDK HTTP request");
    assert_eq!(sdk_response.status(), 404);
    print_state("edge_planned_sdk_after", &ledger);
    assert_eq!(ledger_kind_counts(&ledger), before_sdk);

    print_state("edge_planned_replay_before", &ledger);
    let before_replay = ledger_kind_counts(&ledger);
    let replay_response = client
        .post(format!("http://{addr}/v1/replay"))
        .bearer_auth(API_KEY)
        .header("X-Blackbox-Tenant", TENANT)
        .json(&json!({"run_id": "not-routed"}))
        .send()
        .await
        .expect("send planned replay HTTP request");
    assert_eq!(replay_response.status(), 404);
    print_state("edge_planned_replay_after", &ledger);
    assert_eq!(ledger_kind_counts(&ledger), before_replay);

    println!(
        "FSV_CROSS_PHASE source_of_truth=sqlite:{}#ledger_records final_counts={:?} final_audit_rows={:?}",
        ledger.display(),
        ledger_kind_counts(&ledger),
        audit_rows(&ledger)
    );
}
