//! BlackBoxFix Phase 14 release-gate FSV tests.
//!
//! These tests execute the real `context-graph blackbox` binary against a real
//! SQLite ledger and a real local Git repository. Assertions inspect the
//! physical source of truth directly: `ledger_records` release_gate rows.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase14_test";
const AGENT: &str = "agent_phase14_test";
const RELEASE_PRINCIPAL: &str = "user_release";
const VIEWER_PRINCIPAL: &str = "user_viewer";
const SUBJECT: &str = "subject.phase14@example.test";

struct Phase14Case {
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
    must_git(repo, &["config", "user.email", "phase14@example.test"]);
    must_git(repo, &["config", "user.name", "Phase14 Test"]);
    fs::write(repo.join("app.txt"), "phase14\n").expect("write tracked file");
    must_git(repo, &["add", "app.txt"]);
    must_git(repo, &["commit", "-m", "initial phase14 fixture"]);
}

fn enterprise_config() -> Value {
    json!({
        "tenant_id": TENANT,
        "sso": {
            "protocol": "oidc",
            "issuer": "https://idp.phase14.example.test/tenant_phase14_test",
            "client_id": "blackboxfix-phase14",
            "jwks_sha256": "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
            "required_claims": ["iss", "sub", "aud", "exp"]
        },
        "rbac": {
            "roles": [
                {
                    "role_id": "release_admin",
                    "permissions": ["release.approve", "privacy.review", "privacy.erase", "dashboard.read"]
                },
                {
                    "role_id": "viewer",
                    "permissions": ["dashboard.read"]
                }
            ]
        },
        "retention": {
            "default_class": "standard",
            "classes": [
                { "class_id": "standard", "retain_days": 365, "legal_hold": false },
                { "class_id": "incident", "retain_days": 2555, "legal_hold": true }
            ]
        },
        "deployment": {
            "mode": "private_vpc",
            "region": "us-test-1",
            "data_residency": "us",
            "kms_key_id": "kms_phase14_tenant_key_001",
            "siem_format": "rfc5424_jsonl"
        }
    })
}

fn enterprise_directory() -> Value {
    json!({
        "tenant_id": TENANT,
        "schemas": [
            "urn:ietf:params:scim:schemas:core:2.0:User",
            "urn:ietf:params:scim:schemas:core:2.0:Group"
        ],
        "users": [
            {
                "id": RELEASE_PRINCIPAL,
                "userName": "release.phase14@example.test",
                "active": true,
                "groups": ["group_release"]
            },
            {
                "id": VIEWER_PRINCIPAL,
                "userName": "viewer.phase14@example.test",
                "active": true,
                "groups": ["group_viewers"]
            }
        ],
        "groups": [
            {
                "id": "group_release",
                "displayName": "Release",
                "roles": ["release_admin"]
            },
            {
                "id": "group_viewers",
                "displayName": "Viewers",
                "roles": ["viewer"]
            }
        ]
    })
}

fn new_case() -> Phase14Case {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();
    let ledger = root.join("blackbox.sqlite");
    let repo = root.join("repo");
    let init = stdout_json(&run_context_graph(&[
        "blackbox",
        "init",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
    ]));
    assert_eq!(init["schema_version"], 9);
    init_git_repo(&repo);

    let config_path = root.join("enterprise-config.json");
    let directory_path = root.join("enterprise-directory.json");
    write_json(&config_path, &enterprise_config());
    write_json(&directory_path, &enterprise_directory());
    stdout_json(&run_context_graph(&[
        "blackbox",
        "enterprise",
        "configure",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--input",
        config_path.to_str().expect("config path UTF-8"),
        "--tenant",
        TENANT,
        "--agent",
        AGENT,
    ]));
    stdout_json(&run_context_graph(&[
        "blackbox",
        "enterprise",
        "sync-directory",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--input",
        directory_path.to_str().expect("directory path UTF-8"),
        "--tenant",
        TENANT,
        "--agent",
        AGENT,
    ]));

    Phase14Case {
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
            "expected_effect": "phase14 release-gate fixture"
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
        "phase14_real_tool",
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
        "git:/phase14/repo#working_tree",
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

fn evaluate_retention(ledger: &Path, run_id: &str) -> Value {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "privacy",
        "evaluate-retention",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--tenant",
        TENANT,
        "--run-id",
        run_id,
        "--principal-id",
        RELEASE_PRINCIPAL,
        "--retention-class",
        "standard",
        "--data-subject",
        SUBJECT,
        "--purpose",
        "phase14-release-readiness",
        "--agent",
        AGENT,
    ]))
}

fn gate_evaluate(
    ledger: &Path,
    run_id: &str,
    principal: &str,
    max_risk: &str,
    require_policy: bool,
    require_privacy: bool,
) -> Output {
    let mut args = vec![
        "blackbox",
        "gate",
        "evaluate",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--tenant",
        TENANT,
        "--run-id",
        run_id,
        "--principal-id",
        principal,
        "--environment",
        "production",
        "--min-trust-level",
        "verified_with_policy",
        "--max-risk",
        max_risk,
        "--agent",
        AGENT,
    ];
    if require_policy {
        args.push("--require-policy");
    }
    if require_privacy {
        args.push("--require-privacy");
    }
    run_context_graph(&args)
}

fn verified_run(case: &Phase14Case, task: &str) -> String {
    let run_id = record_run(&case.ledger, task, "medium");
    policy_allow_git_apply_patch(&case.ledger, &run_id);
    let tool_call_id = record_tool_call(&case.ledger, &case.root, &run_id, task);
    stdout_json(&verify_working_tree_clean(
        &case.ledger,
        &case.root,
        &case.repo,
        &run_id,
        &tool_call_id,
        true,
    ));
    let score = score_run(&case.ledger, &run_id);
    assert_eq!(score["score"]["trust_level"], "verified_with_policy");
    evaluate_retention(&case.ledger, &run_id);
    run_id
}

fn failed_incident_run(case: &Phase14Case, task: &str) -> String {
    fs::write(case.repo.join("app.txt"), "phase14\nmodified\n").expect("dirty tracked file");
    let run_id = record_run(&case.ledger, task, "medium");
    policy_allow_git_apply_patch(&case.ledger, &run_id);
    let tool_call_id = record_tool_call(&case.ledger, &case.root, &run_id, task);
    let verify_error = stderr_json(&verify_working_tree_clean(
        &case.ledger,
        &case.root,
        &case.repo,
        &run_id,
        &tool_call_id,
        true,
    ));
    assert_eq!(verify_error["error_code"], "GIT_WORKING_TREE_DIRTY");
    let score = score_run(&case.ledger, &run_id);
    assert_eq!(score["score"]["trust_level"], "incident");
    evaluate_retention(&case.ledger, &run_id);
    run_id
}

fn kind_counts(ledger: &Path) -> BTreeMap<String, i64> {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let mut stmt = conn
        .prepare(
            "SELECT record_kind, COUNT(*)
             FROM ledger_records
             GROUP BY record_kind
             ORDER BY record_kind",
        )
        .expect("prepare kind counts");
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("query kind counts");
    rows.map(|row| row.expect("kind count row")).collect()
}

fn record_kind_count(ledger: &Path, record_kind: &str) -> i64 {
    let conn = Connection::open(ledger).expect("open physical ledger");
    conn.query_row(
        "SELECT COUNT(*) FROM ledger_records WHERE record_kind = ?1",
        params![record_kind],
        |row| row.get(0),
    )
    .expect("count record kind")
}

fn latest_gate_payload(ledger: &Path, run_id: &str) -> Value {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let payload: String = conn
        .query_row(
            "SELECT payload_json
             FROM ledger_records
             WHERE record_kind = 'release_gate' AND run_id = ?1
             ORDER BY sequence DESC
             LIMIT 1",
            params![run_id],
            |row| row.get(0),
        )
        .expect("read latest release gate");
    serde_json::from_str(&payload).expect("release gate payload JSON")
}

fn latest_gate_failed_reason_codes(ledger: &Path, run_id: &str) -> Vec<String> {
    latest_gate_payload(ledger, run_id)["decision"]["failed_checks"]
        .as_array()
        .expect("failed checks")
        .iter()
        .map(|check| {
            check["reason_code"]
                .as_str()
                .expect("reason code")
                .to_string()
        })
        .collect()
}

#[test]
fn phase14_release_gate_persists_approved_and_blocked_decisions() {
    let case = new_case();
    println!(
        "FSV14 source_of_truth=sqlite:{}#ledger_records",
        case.ledger.display()
    );
    println!(
        "FSV14 git_source_before={}",
        must_git(
            &case.repo,
            &["status", "--porcelain=v1", "--untracked-files=all"]
        )
    );
    println!("FSV14 BEFORE counts: {:?}", kind_counts(&case.ledger));

    let happy_run = verified_run(&case, "phase14 verified release happy path");
    let approved = stdout_json(&gate_evaluate(
        &case.ledger,
        &happy_run,
        RELEASE_PRINCIPAL,
        "0.7",
        true,
        true,
    ));
    let approved_stored = latest_gate_payload(&case.ledger, &happy_run);
    println!("FSV14 approved AFTER stored={}", approved_stored);
    assert_eq!(approved["readback"]["status"], "passed");
    assert_eq!(
        approved_stored["release_gate_id"],
        approved["release_gate_id"]
    );
    assert_eq!(approved_stored["decision"]["status"], "approved");
    assert_eq!(
        approved_stored["decision"]["reason_code"],
        "RELEASE_GATE_APPROVED"
    );
    assert!(approved_stored["decision"]["failed_checks"]
        .as_array()
        .expect("failed checks")
        .is_empty());

    let missing_score_run = record_run(&case.ledger, "phase14 missing score edge", "medium");
    let missing_before = kind_counts(&case.ledger);
    let missing_error = stderr_json(&gate_evaluate(
        &case.ledger,
        &missing_score_run,
        RELEASE_PRINCIPAL,
        "0.7",
        true,
        true,
    ));
    let missing_after = kind_counts(&case.ledger);
    let missing_gate = latest_gate_payload(&case.ledger, &missing_score_run);
    println!(
        "FSV14 missing-score BEFORE={:?} AFTER={:?} error={} stored={}",
        missing_before, missing_after, missing_error, missing_gate
    );
    assert_eq!(missing_error["error_code"], "RELEASE_GATE_BLOCKED");
    assert_eq!(missing_gate["decision"]["status"], "blocked");
    assert!(
        latest_gate_failed_reason_codes(&case.ledger, &missing_score_run)
            .contains(&"RELEASE_GATE_SCORECARD_REQUIRED".to_string())
    );
    assert_eq!(
        missing_after.get("release_gate").copied().unwrap_or(0),
        missing_before.get("release_gate").copied().unwrap_or(0) + 1
    );

    let risk_before = record_kind_count(&case.ledger, "release_gate");
    let risk_error = stderr_json(&gate_evaluate(
        &case.ledger,
        &happy_run,
        RELEASE_PRINCIPAL,
        "0.1",
        true,
        true,
    ));
    let risk_after = record_kind_count(&case.ledger, "release_gate");
    let risk_gate = latest_gate_payload(&case.ledger, &happy_run);
    println!(
        "FSV14 risk-threshold BEFORE gates={} AFTER gates={} error={} stored={}",
        risk_before, risk_after, risk_error, risk_gate
    );
    assert_eq!(risk_error["error_code"], "RELEASE_GATE_BLOCKED");
    assert_eq!(risk_after, risk_before + 1);
    assert!(latest_gate_failed_reason_codes(&case.ledger, &happy_run)
        .contains(&"RELEASE_GATE_RISK_TOO_HIGH".to_string()));

    let unauthorized_run = verified_run(&case, "phase14 unauthorized edge");
    let unauthorized_before_gate = record_kind_count(&case.ledger, "release_gate");
    let unauthorized_before_access = record_kind_count(&case.ledger, "enterprise_access_decision");
    let unauthorized_error = stderr_json(&gate_evaluate(
        &case.ledger,
        &unauthorized_run,
        VIEWER_PRINCIPAL,
        "0.7",
        true,
        true,
    ));
    let unauthorized_after_gate = record_kind_count(&case.ledger, "release_gate");
    let unauthorized_after_access = record_kind_count(&case.ledger, "enterprise_access_decision");
    println!(
        "FSV14 unauthorized BEFORE gate={} access={} AFTER gate={} access={} error={}",
        unauthorized_before_gate,
        unauthorized_before_access,
        unauthorized_after_gate,
        unauthorized_after_access,
        unauthorized_error
    );
    assert_eq!(unauthorized_error["error_code"], "RELEASE_GATE_RBAC_DENIED");
    assert_eq!(unauthorized_after_gate, unauthorized_before_gate);
    assert_eq!(unauthorized_after_access, unauthorized_before_access + 1);

    let incident_run = failed_incident_run(&case, "phase14 incident gate edge");
    let incident_error = stderr_json(&gate_evaluate(
        &case.ledger,
        &incident_run,
        RELEASE_PRINCIPAL,
        "0.7",
        true,
        true,
    ));
    let incident_gate = latest_gate_payload(&case.ledger, &incident_run);
    println!(
        "FSV14 incident error={} stored={}",
        incident_error, incident_gate
    );
    assert_eq!(incident_error["error_code"], "RELEASE_GATE_BLOCKED");
    let incident_reasons = latest_gate_failed_reason_codes(&case.ledger, &incident_run);
    assert!(incident_reasons.contains(&"RELEASE_GATE_INCIDENT_OPEN".to_string()));
    assert!(incident_reasons.contains(&"RELEASE_GATE_VERIFIER_NOT_CLEAN".to_string()));

    let invalid_before = kind_counts(&case.ledger);
    let invalid_error = stderr_json(&gate_evaluate(
        &case.ledger,
        &happy_run,
        RELEASE_PRINCIPAL,
        "1.5",
        true,
        true,
    ));
    let invalid_after = kind_counts(&case.ledger);
    println!(
        "FSV14 invalid-threshold BEFORE={:?} AFTER={:?} error={}",
        invalid_before, invalid_after, invalid_error
    );
    assert_eq!(
        invalid_error["error_code"],
        "RELEASE_GATE_THRESHOLD_INVALID"
    );
    assert_eq!(invalid_after, invalid_before);

    let connector = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "blackbox_release_gate_v1",
        "--source",
        case.ledger.to_str().expect("ledger path UTF-8"),
    ]));
    assert_eq!(connector["connector"], "blackbox_release_gate_v1");
    assert_eq!(
        connector["readback"]["release_gate_records_total"],
        record_kind_count(&case.ledger, "release_gate")
    );

    let final_counts = kind_counts(&case.ledger);
    println!("FSV14 FINAL physical counts: {:?}", final_counts);
    assert_eq!(final_counts.get("enterprise_tenant_config"), Some(&1));
    assert_eq!(final_counts.get("enterprise_directory_snapshot"), Some(&1));
    assert_eq!(final_counts.get("agent_run"), Some(&4));
    assert_eq!(final_counts.get("scorecard"), Some(&3));
    assert_eq!(final_counts.get("retention_decision"), Some(&3));
    assert_eq!(final_counts.get("release_gate"), Some(&4));
    assert_eq!(final_counts.get("incident_packet"), Some(&1));
}
