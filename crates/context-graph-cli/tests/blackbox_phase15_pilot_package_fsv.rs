//! BlackBoxFix Phase 15 pilot-package FSV tests.
//!
//! These tests execute the real `context-graph blackbox` binary against a real
//! SQLite ledger, a real local Git repository, and real package output files.
//! Assertions inspect the physical sources of truth directly: `ledger_records`
//! `pilot_package` rows and package files on disk.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase15_test";
const AGENT: &str = "agent_phase15_test";
const PACKAGE_PRINCIPAL: &str = "user_package";
const VIEWER_PRINCIPAL: &str = "user_viewer";
const SUBJECT: &str = "subject.phase15@example.test";

struct Phase15Case {
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

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read JSON file")).expect("parse JSON file")
}

fn sha256_file(path: &Path) -> String {
    let bytes = fs::read(path).expect("read file for sha256");
    let digest = Sha256::digest(&bytes);
    format!("sha256:{digest:x}")
}

fn jsonl_line_count(path: &Path) -> usize {
    let text = fs::read_to_string(path).expect("read JSONL file");
    if text.is_empty() {
        0
    } else {
        text.lines().count()
    }
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
    must_git(repo, &["config", "user.email", "phase15@example.test"]);
    must_git(repo, &["config", "user.name", "Phase15 Test"]);
    fs::write(repo.join("app.txt"), "phase15\n").expect("write tracked file");
    must_git(repo, &["add", "app.txt"]);
    must_git(repo, &["commit", "-m", "initial phase15 fixture"]);
}

fn enterprise_config() -> Value {
    json!({
        "tenant_id": TENANT,
        "sso": {
            "protocol": "oidc",
            "issuer": "https://idp.phase15.example.test/tenant_phase15_test",
            "client_id": "blackboxfix-phase15",
            "jwks_sha256": "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
            "required_claims": ["iss", "sub", "aud", "exp"]
        },
        "rbac": {
            "roles": [
                {
                    "role_id": "commercial_admin",
                    "permissions": ["commercial.package", "release.approve", "privacy.review", "dashboard.read"]
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
                { "class_id": "standard", "retain_days": 365, "legal_hold": false }
            ]
        },
        "deployment": {
            "mode": "private_vpc",
            "region": "us-test-1",
            "data_residency": "us",
            "kms_key_id": "kms_phase15_tenant_key_001",
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
                "id": PACKAGE_PRINCIPAL,
                "userName": "package.phase15@example.test",
                "active": true,
                "groups": ["group_commercial"]
            },
            {
                "id": VIEWER_PRINCIPAL,
                "userName": "viewer.phase15@example.test",
                "active": true,
                "groups": ["group_viewers"]
            }
        ],
        "groups": [
            {
                "id": "group_commercial",
                "displayName": "Commercial",
                "roles": ["commercial_admin"]
            },
            {
                "id": "group_viewers",
                "displayName": "Viewers",
                "roles": ["viewer"]
            }
        ]
    })
}

fn new_case() -> Phase15Case {
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

    Phase15Case {
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
            "expected_effect": "phase15 pilot package fixture"
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
        "phase15_real_tool",
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
        "git:/phase15/repo#working_tree",
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

fn score_run(ledger: &Path, run_id: &str) {
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
    ]));
}

fn evaluate_retention(ledger: &Path, run_id: &str) {
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
        PACKAGE_PRINCIPAL,
        "--retention-class",
        "standard",
        "--data-subject",
        SUBJECT,
        "--purpose",
        "phase15-pilot-package",
        "--agent",
        AGENT,
    ]));
}

fn gate_evaluate(ledger: &Path, run_id: &str) {
    stdout_json(&run_context_graph(&[
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
        PACKAGE_PRINCIPAL,
        "--environment",
        "production",
        "--min-trust-level",
        "verified_with_policy",
        "--max-risk",
        "0.7",
        "--require-policy",
        "--require-privacy",
        "--agent",
        AGENT,
    ]));
}

fn verified_run(case: &Phase15Case, task: &str) -> String {
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
    score_run(&case.ledger, &run_id);
    evaluate_retention(&case.ledger, &run_id);
    gate_evaluate(&case.ledger, &run_id);
    run_id
}

fn failed_incident_run(case: &Phase15Case, task: &str) -> String {
    fs::write(case.repo.join("app.txt"), "phase15\nmodified\n").expect("dirty tracked file");
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
    run_id
}

fn package_pilot_report(ledger: &Path, out: &Path, principal: &str, min_actions: &str) -> Output {
    run_context_graph(&[
        "blackbox",
        "package",
        "pilot-report",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--tenant",
        TENANT,
        "--principal-id",
        principal,
        "--pilot-id",
        "pilot_phase15",
        "--min-actions",
        min_actions,
        "--out",
        out.to_str().expect("out path UTF-8"),
        "--agent",
        AGENT,
    ])
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

fn latest_package_payload(ledger: &Path) -> Value {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let payload: String = conn
        .query_row(
            "SELECT payload_json
             FROM ledger_records
             WHERE record_kind = 'pilot_package'
             ORDER BY sequence DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("read latest pilot package");
    serde_json::from_str(&payload).expect("pilot package payload JSON")
}

fn verify_manifest_hashes(out: &Path) -> Value {
    let manifest = read_json(&out.join("manifest.json"));
    for file in manifest["files"].as_array().expect("manifest files") {
        let rel = file["path"].as_str().expect("manifest file path");
        let path = out.join(rel);
        assert!(path.exists(), "manifest file missing: {}", path.display());
        assert_eq!(file["sha256"], sha256_file(&path), "manifest hash mismatch");
        assert_eq!(
            file["bytes"].as_u64().expect("manifest bytes"),
            fs::metadata(&path).expect("file metadata").len(),
            "manifest byte count mismatch"
        );
    }
    manifest
}

#[test]
fn phase15_pilot_package_persists_usage_report_and_edges() {
    let case = new_case();
    println!(
        "FSV15 source_of_truth=sqlite:{}#ledger_records",
        case.ledger.display()
    );
    println!(
        "FSV15 git_source_before={}",
        must_git(
            &case.repo,
            &["status", "--porcelain=v1", "--untracked-files=all"]
        )
    );
    println!("FSV15 BEFORE counts: {:?}", kind_counts(&case.ledger));

    let happy_run = verified_run(&case, "phase15 verified package happy path");
    let incident_run = failed_incident_run(&case, "phase15 failed eval candidate");
    println!("FSV15 fixture runs happy={happy_run} incident={incident_run}");

    let out = case.root.join("pilot-package");
    let package = stdout_json(&package_pilot_report(
        &case.ledger,
        &out,
        PACKAGE_PRINCIPAL,
        "2",
    ));
    let stored = latest_package_payload(&case.ledger);
    let manifest = verify_manifest_hashes(&out);
    let report = read_json(&out.join("pilot-report.json"));
    let usage_lines = jsonl_line_count(&out.join("usage-meter-events.jsonl"));
    let eval_lines = jsonl_line_count(&out.join("eval-candidates.jsonl"));
    println!(
        "FSV15 happy AFTER package={} stored={} manifest={} report={} usage_lines={} eval_lines={}",
        package, stored, manifest, report, usage_lines, eval_lines
    );
    assert_eq!(package["readback"]["status"], "passed");
    assert_eq!(stored["pilot_package_id"], package["pilot_package_id"]);
    assert_eq!(stored["pilot_success"]["status"], "passed");
    assert_eq!(report["pilot_success"]["status"], "passed");
    assert_eq!(report["usage_summary"]["agent_runs_total"], 2);
    assert_eq!(report["usage_summary"]["tool_calls_total"], 2);
    assert_eq!(report["usage_summary"]["tool_calls_with_evidence"], 2);
    assert!(usage_lines >= 2);
    assert!(eval_lines >= 1);
    assert_eq!(
        manifest["usage_meter_events"]["line_count"]
            .as_u64()
            .expect("usage line count") as usize,
        usage_lines
    );
    assert_eq!(
        manifest["eval_candidates"]["line_count"]
            .as_u64()
            .expect("eval line count") as usize,
        eval_lines
    );

    let unmet_out = case.root.join("pilot-package-unmet");
    let unmet_before = kind_counts(&case.ledger);
    let unmet_error = stderr_json(&package_pilot_report(
        &case.ledger,
        &unmet_out,
        PACKAGE_PRINCIPAL,
        "999",
    ));
    let unmet_after = kind_counts(&case.ledger);
    let unmet_stored = latest_package_payload(&case.ledger);
    println!(
        "FSV15 unmet BEFORE={:?} AFTER={:?} error={} stored={}",
        unmet_before, unmet_after, unmet_error, unmet_stored
    );
    assert_eq!(unmet_error["error_code"], "PILOT_PACKAGE_CRITERIA_NOT_MET");
    assert!(unmet_out.join("manifest.json").exists());
    assert_eq!(
        unmet_after.get("pilot_package").copied().unwrap_or(0),
        unmet_before.get("pilot_package").copied().unwrap_or(0) + 1
    );
    assert_eq!(
        unmet_stored["pilot_success"]["status"],
        "needs_more_evidence"
    );
    assert!(unmet_stored["pilot_success"]["failed_criteria"]
        .as_array()
        .expect("failed criteria")
        .iter()
        .any(|value| value == "PILOT_MIN_ACTIONS_NOT_MET"));

    let unauthorized_out = case.root.join("pilot-package-unauthorized");
    let unauthorized_before_package = record_kind_count(&case.ledger, "pilot_package");
    let unauthorized_before_access = record_kind_count(&case.ledger, "enterprise_access_decision");
    let unauthorized_error = stderr_json(&package_pilot_report(
        &case.ledger,
        &unauthorized_out,
        VIEWER_PRINCIPAL,
        "2",
    ));
    let unauthorized_after_package = record_kind_count(&case.ledger, "pilot_package");
    let unauthorized_after_access = record_kind_count(&case.ledger, "enterprise_access_decision");
    println!(
        "FSV15 unauthorized BEFORE package={} access={} AFTER package={} access={} error={}",
        unauthorized_before_package,
        unauthorized_before_access,
        unauthorized_after_package,
        unauthorized_after_access,
        unauthorized_error
    );
    assert_eq!(
        unauthorized_error["error_code"],
        "PILOT_PACKAGE_RBAC_DENIED"
    );
    assert_eq!(unauthorized_after_package, unauthorized_before_package);
    assert_eq!(unauthorized_after_access, unauthorized_before_access + 1);
    assert!(!unauthorized_out.exists());

    let invalid_out = case.root.join("pilot-package-invalid");
    let invalid_before = kind_counts(&case.ledger);
    let invalid_error = stderr_json(&package_pilot_report(
        &case.ledger,
        &invalid_out,
        PACKAGE_PRINCIPAL,
        "0",
    ));
    let invalid_after = kind_counts(&case.ledger);
    println!(
        "FSV15 invalid BEFORE={:?} AFTER={:?} error={}",
        invalid_before, invalid_after, invalid_error
    );
    assert_eq!(
        invalid_error["error_code"],
        "PILOT_PACKAGE_MIN_ACTIONS_INVALID"
    );
    assert_eq!(invalid_after, invalid_before);
    assert!(!invalid_out.exists());

    let dashboard_out = case.root.join("phase15-dashboard");
    let dashboard = stdout_json(&run_context_graph(&[
        "blackbox",
        "dashboard",
        "build",
        "--ledger",
        case.ledger.to_str().expect("ledger path UTF-8"),
        "--out",
        dashboard_out.to_str().expect("dashboard path UTF-8"),
    ]));
    let dashboard_summary = read_json(&dashboard_out.join("api/summary.json"));
    let dashboard_packages = read_json(&dashboard_out.join("api/pilot-packages.json"));
    let dashboard_errors = read_json(&dashboard_out.join("api/errors.json"));
    let dashboard_html =
        fs::read_to_string(dashboard_out.join("index.html")).expect("dashboard HTML");
    println!(
        "FSV15 dashboard readback build={} summary={} packages={} errors={}",
        dashboard, dashboard_summary, dashboard_packages, dashboard_errors
    );
    assert_eq!(
        dashboard_summary["counts"]["pilot_packages_total"],
        record_kind_count(&case.ledger, "pilot_package")
    );
    assert_eq!(
        dashboard_packages["pilot_packages"]
            .as_array()
            .expect("pilot packages")
            .len() as i64,
        record_kind_count(&case.ledger, "pilot_package")
    );
    assert!(dashboard_packages["pilot_packages"]
        .as_array()
        .expect("pilot packages")
        .iter()
        .any(|package| package["status"] == "passed"));
    assert!(dashboard_packages["pilot_packages"]
        .as_array()
        .expect("pilot packages")
        .iter()
        .any(|package| package["status"] == "needs_more_evidence"));
    assert!(dashboard_errors["error_codes"]
        .as_array()
        .expect("dashboard errors")
        .iter()
        .any(|error| error["error_code"] == "PILOT_MIN_ACTIONS_NOT_MET"));
    assert!(!dashboard_errors["error_codes"]
        .as_array()
        .expect("dashboard errors")
        .iter()
        .any(|error| error["error_code"] == "RELEASE_GATE_APPROVED"
            || error["error_code"] == "RETENTION_POLICY_APPLIED"));
    assert!(dashboard_html.contains("Pilot Packages"));
    assert!(dashboard_html.contains("api/pilot-packages.json"));

    let connector = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "blackbox_packaging_v1",
        "--source",
        case.ledger.to_str().expect("ledger path UTF-8"),
    ]));
    assert_eq!(connector["connector"], "blackbox_packaging_v1");
    assert_eq!(
        connector["readback"]["pilot_package_records_total"],
        record_kind_count(&case.ledger, "pilot_package")
    );

    let final_counts = kind_counts(&case.ledger);
    println!("FSV15 FINAL physical counts: {:?}", final_counts);
    assert_eq!(final_counts.get("enterprise_tenant_config"), Some(&1));
    assert_eq!(final_counts.get("enterprise_directory_snapshot"), Some(&1));
    assert_eq!(final_counts.get("agent_run"), Some(&2));
    assert_eq!(final_counts.get("tool_call"), Some(&2));
    assert_eq!(final_counts.get("evidence_packet"), Some(&2));
    assert_eq!(final_counts.get("incident_packet"), Some(&1));
    assert_eq!(final_counts.get("release_gate"), Some(&1));
    assert_eq!(final_counts.get("pilot_package"), Some(&2));
}
