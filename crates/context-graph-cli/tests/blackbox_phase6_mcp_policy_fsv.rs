//! BlackBoxFix Phase 6 MCP gateway + policy FSV tests.
//!
//! These tests execute the real `context-graph blackbox` binary as an MCP
//! JSON-RPC stdio server against real SQLite ledgers and real Git repositories.
//! Assertions inspect the physical ledger and Git worktree directly.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase6_test";
const AGENT: &str = "agent_phase6_test";

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
    must_git(repo, &["config", "user.email", "phase6@example.test"]);
    must_git(repo, &["config", "user.name", "Phase6 Test"]);
    fs::write(repo.join("app.txt"), "phase6\n").expect("write tracked file");
    must_git(repo, &["add", "app.txt"]);
    must_git(repo, &["commit", "-m", "initial phase6 fixture"]);
}

fn make_patch(repo: &Path) -> String {
    fs::write(repo.join("app.txt"), "phase6\nverified gateway\n").expect("edit tracked file");
    let patch = must_git(
        repo,
        &["diff", "--no-ext-diff", "--no-color", "--", "app.txt"],
    );
    assert!(patch.contains("+verified gateway"));
    must_git(repo, &["restore", "app.txt"]);
    assert_eq!(
        must_git(
            repo,
            &["diff", "--no-ext-diff", "--no-color", "--", "app.txt"]
        ),
        ""
    );
    patch
}

fn run_mcp_gateway(ledger: &Path, mode: &str, messages: &[Value]) -> Vec<Value> {
    let mut child = Command::new(context_graph_bin())
        .args([
            "blackbox",
            "mcp",
            "gateway",
            "--ledger",
            ledger.to_str().expect("ledger path UTF-8"),
            "--policy-mode",
            mode,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn MCP gateway");
    {
        let stdin = child.stdin.as_mut().expect("MCP stdin");
        for message in messages {
            writeln!(
                stdin,
                "{}",
                serde_json::to_string(message).expect("JSON message")
            )
            .expect("write MCP message");
        }
    }
    let output = child.wait_with_output().expect("wait MCP gateway");
    assert!(
        output.status.success(),
        "MCP gateway failed\nstatus={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("MCP stdout UTF-8")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("MCP response JSON"))
        .collect()
}

fn mcp_tool_result(response: &Value) -> &Value {
    response
        .pointer("/result/structuredContent")
        .expect("MCP structuredContent")
}

fn mcp_start_run(ledger: &Path) -> String {
    let responses = run_mcp_gateway(
        ledger,
        "bounded_autonomous",
        &[
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            json!({
                "jsonrpc":"2.0",
                "id":3,
                "method":"tools/call",
                "params":{
                    "name":"blackbox_start_run",
                    "arguments":{
                        "task":"phase6 MCP FSV run",
                        "agent":AGENT,
                        "tenant":TENANT,
                        "risk_tier":"low"
                    }
                }
            }),
        ],
    );
    let tools = responses[1]["result"]["tools"]
        .as_array()
        .expect("tools array");
    assert!(tools
        .iter()
        .any(|tool| tool["name"] == "verified_git_apply_patch"));
    mcp_tool_result(&responses[2])["run_id"]
        .as_str()
        .expect("run_id")
        .to_string()
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
        .expect("prepare kind count");
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("query kind count");
    rows.map(|row| row.expect("kind row")).collect()
}

fn ledger_row_count(ledger: &Path) -> i64 {
    let conn = Connection::open(ledger).expect("open physical ledger");
    conn.query_row("SELECT COUNT(*) FROM ledger_records", [], |row| row.get(0))
        .expect("count physical rows")
}

fn policy_statuses(ledger: &Path) -> Vec<(String, String)> {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let mut stmt = conn
        .prepare(
            "SELECT json_extract(payload_json, '$.status'),
                    coalesce(json_extract(payload_json, '$.error_code'), '')
             FROM ledger_records
             WHERE record_kind = 'policy_decision'
             ORDER BY sequence",
        )
        .expect("prepare policy query");
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .expect("query policy rows");
    rows.map(|row| row.expect("policy row")).collect()
}

fn evidence_count_for_tool(ledger: &Path, tool_call_id: &str) -> i64 {
    let conn = Connection::open(ledger).expect("open physical ledger");
    conn.query_row(
        "SELECT COUNT(*)
         FROM ledger_records
         WHERE record_kind = 'evidence_packet'
           AND json_extract(payload_json, '$.tool_call_id') = ?1",
        params![tool_call_id],
        |row| row.get(0),
    )
    .expect("count evidence rows")
}

#[test]
fn phase6_mcp_gateway_apply_patch_records_policy_and_evidence() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    let repo = temp.path().join("repo");
    init_ledger(&ledger);
    create_git_repo(&repo);
    let patch = make_patch(&repo);
    let run_id = mcp_start_run(&ledger);
    let before_rows = ledger_row_count(&ledger);

    let responses = run_mcp_gateway(
        &ledger,
        "bounded_autonomous",
        &[json!({
            "jsonrpc":"2.0",
            "id":10,
            "method":"tools/call",
            "params":{
                "name":"verified_git_apply_patch",
                "arguments":{
                    "run_id":run_id,
                    "repo":repo.display().to_string(),
                    "path":"app.txt",
                    "patch":patch,
                    "tenant":TENANT,
                    "agent":AGENT
                }
            }
        })],
    );
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["result"]["isError"], false);
    let result = mcp_tool_result(&responses[0]);
    let tool_call_id = result["tool_call"]["tool_call_id"]
        .as_str()
        .expect("tool call id");
    assert_eq!(result["verification"]["verifier_status"], "passed");
    assert_eq!(evidence_count_for_tool(&ledger, tool_call_id), 1);

    let actual_diff = must_git(
        &repo,
        &["diff", "--no-ext-diff", "--no-color", "--", "app.txt"],
    );
    assert_eq!(
        actual_diff,
        result["verification"]["actual_readback"]["patch"]
    );
    assert_eq!(
        result["tool_call"]["redaction"]["redacted_fields"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert!(ledger_row_count(&ledger) > before_rows);

    let counts = ledger_kind_counts(&ledger);
    assert_eq!(counts.get("agent_run"), Some(&1));
    assert_eq!(counts.get("agent_step"), Some(&1));
    assert_eq!(counts.get("model_decision"), Some(&1));
    assert_eq!(counts.get("policy_decision"), Some(&1));
    assert_eq!(counts.get("tool_call"), Some(&1));
    assert_eq!(counts.get("expected_delta"), Some(&1));
    assert_eq!(counts.get("readback_snapshot"), Some(&1));
    assert_eq!(counts.get("verifier_result"), Some(&1));
    assert_eq!(counts.get("evidence_packet"), Some(&1));
    assert_eq!(
        policy_statuses(&ledger),
        vec![("allowed".into(), "".into())]
    );
}

#[test]
fn phase6_blocked_policy_persists_denial_and_does_not_mutate_git() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    let repo = temp.path().join("repo");
    init_ledger(&ledger);
    create_git_repo(&repo);
    let patch = make_patch(&repo);
    let run_id = mcp_start_run(&ledger);

    let responses = run_mcp_gateway(
        &ledger,
        "blocked",
        &[json!({
            "jsonrpc":"2.0",
            "id":11,
            "method":"tools/call",
            "params":{
                "name":"verified_git_apply_patch",
                "arguments":{
                    "run_id":run_id,
                    "repo":repo.display().to_string(),
                    "path":"app.txt",
                    "patch":patch,
                    "tenant":TENANT,
                    "agent":AGENT
                }
            }
        })],
    );
    assert_eq!(responses[0]["result"]["isError"], true);
    assert_eq!(
        mcp_tool_result(&responses[0])["error_code"],
        "POLICY_MODE_BLOCKED"
    );
    assert_eq!(
        must_git(
            &repo,
            &["diff", "--no-ext-diff", "--no-color", "--", "app.txt"]
        ),
        ""
    );

    let counts = ledger_kind_counts(&ledger);
    assert_eq!(counts.get("agent_run"), Some(&1));
    assert_eq!(counts.get("agent_step"), Some(&1));
    assert_eq!(counts.get("model_decision"), Some(&1));
    assert_eq!(counts.get("policy_decision"), Some(&1));
    assert_eq!(counts.get("tool_call"), None);
    assert_eq!(counts.get("evidence_packet"), None);
    assert_eq!(
        policy_statuses(&ledger),
        vec![("denied".into(), "POLICY_MODE_BLOCKED".into())]
    );
}

#[test]
fn phase6_unverified_mutating_policy_is_denied_in_bounded_mode() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    init_ledger(&ledger);
    let run_id = mcp_start_run(&ledger);

    let responses = run_mcp_gateway(
        &ledger,
        "bounded_autonomous",
        &[json!({
            "jsonrpc":"2.0",
            "id":12,
            "method":"tools/call",
            "params":{
                "name":"blackbox_policy_evaluate",
                "arguments":{
                    "run_id":run_id,
                    "tool_name":"unverified_mutator",
                    "capability_id":"local.git.apply_patch",
                    "mutating":true,
                    "tenant":TENANT,
                    "agent":AGENT
                }
            }
        })],
    );
    assert_eq!(responses[0]["result"]["isError"], false);
    let result = mcp_tool_result(&responses[0]);
    assert_eq!(result["allowed"], false);
    assert_eq!(result["error_code"], "POLICY_VERIFIER_REQUIRED");
    assert_eq!(
        policy_statuses(&ledger),
        vec![("denied".into(), "POLICY_VERIFIER_REQUIRED".into())]
    );
    let counts = ledger_kind_counts(&ledger);
    assert_eq!(counts.get("policy_decision"), Some(&1));
    assert_eq!(counts.get("tool_call"), None);
}

#[test]
fn phase6_contextgraph_agentops_demo_capability_is_registered_and_allowed_with_verifier() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    init_ledger(&ledger);
    let run_id = mcp_start_run(&ledger);
    let ledger_str = ledger.to_str().expect("ledger path UTF-8");

    let registry = stdout_json(&run_context_graph(&[
        "blackbox", "policy", "registry", "--ledger", ledger_str,
    ]));
    let capabilities = registry["capabilities"]
        .as_array()
        .expect("capabilities array");
    let demo_capability = capabilities
        .iter()
        .find(|item| item["capability_id"] == "blackboxfix.demo.full_agentops_dynamicjepa")
        .expect("ContextGraph demo capability is registered");
    assert_eq!(
        demo_capability["tool_name"],
        "run_dynamicjepa_blackboxfix_pipeline"
    );
    assert_eq!(demo_capability["mutating"], true);
    assert_eq!(demo_capability["verifier_id"], "cmd_exit_zero_v1");
    assert_eq!(demo_capability["source_kind"], "cmd_local_v1");

    let before_rows = ledger_row_count(&ledger);
    let decision = stdout_json(&run_context_graph(&[
        "blackbox",
        "policy",
        "evaluate",
        "--ledger",
        ledger_str,
        "--mode",
        "bounded_autonomous",
        "--tool",
        "run_dynamicjepa_blackboxfix_pipeline",
        "--capability",
        "blackboxfix.demo.full_agentops_dynamicjepa",
        "--run-id",
        run_id.as_str(),
        "--mutating",
        "--verifier-id",
        "cmd_exit_zero_v1",
        "--source-scope",
        "urn:contextgraph:dynamicjepa:counter_world_full_agentops",
        "--tenant",
        TENANT,
        "--agent",
        AGENT,
    ]));
    assert_eq!(decision["allowed"], true);
    assert_eq!(decision["decision"], "allowed");
    assert!(decision["error_code"].is_null());
    assert_eq!(ledger_row_count(&ledger), before_rows + 1);
    assert_eq!(
        policy_statuses(&ledger),
        vec![("allowed".into(), "".into())]
    );
}

#[test]
fn phase6_invalid_mcp_arguments_fail_before_writing_rows() {
    let temp = TempDir::new().expect("temp dir");
    let ledger = temp.path().join("blackbox.sqlite");
    let repo = temp.path().join("repo");
    init_ledger(&ledger);
    create_git_repo(&repo);
    let run_id = mcp_start_run(&ledger);
    let before = ledger_row_count(&ledger);

    let responses = run_mcp_gateway(
        &ledger,
        "bounded_autonomous",
        &[json!({
            "jsonrpc":"2.0",
            "id":13,
            "method":"tools/call",
            "params":{
                "name":"verified_git_apply_patch",
                "arguments":{
                    "run_id":run_id,
                    "repo":repo.display().to_string(),
                    "path":"app.txt",
                    "tenant":TENANT,
                    "agent":AGENT
                }
            }
        })],
    );
    assert_eq!(responses[0]["result"]["isError"], true);
    assert_eq!(
        mcp_tool_result(&responses[0])["error_code"],
        "BBX_MCP_SCHEMA_INVALID"
    );
    assert_eq!(ledger_row_count(&ledger), before);
    assert_eq!(
        must_git(
            &repo,
            &["diff", "--no-ext-diff", "--no-color", "--", "app.txt"]
        ),
        ""
    );
}
