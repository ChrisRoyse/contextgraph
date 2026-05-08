//! BlackBoxFix Phase 16 risk-register FSV tests.
//!
//! These tests execute the real `context-graph blackbox` binary against a real
//! SQLite ledger, a real local Git repository, and real JSON decision/package
//! files. Assertions inspect the physical source of truth directly:
//! `ledger_records` risk_assessment rows and dashboard API files on disk.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use tempfile::TempDir;

const TENANT: &str = "tenant_phase16_test";
const AGENT: &str = "agent_phase16_test";
const RISK_PRINCIPAL: &str = "user_risk";
const VIEWER_PRINCIPAL: &str = "user_viewer";
const SUBJECT: &str = "subject.phase16@example.test";

struct Phase16Case {
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
    must_git(repo, &["config", "user.email", "phase16@example.test"]);
    must_git(repo, &["config", "user.name", "Phase16 Test"]);
    fs::write(repo.join("app.txt"), "phase16\n").expect("write tracked file");
    must_git(repo, &["add", "app.txt"]);
    must_git(repo, &["commit", "-m", "initial phase16 fixture"]);
}

fn enterprise_config() -> Value {
    json!({
        "tenant_id": TENANT,
        "sso": {
            "protocol": "oidc",
            "issuer": "https://idp.phase16.example.test/tenant_phase16_test",
            "client_id": "blackboxfix-phase16",
            "jwks_sha256": "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            "required_claims": ["iss", "sub", "aud", "exp"]
        },
        "rbac": {
            "roles": [
                {
                    "role_id": "risk_admin",
                    "permissions": [
                        "risk.review",
                        "commercial.package",
                        "release.approve",
                        "privacy.review",
                        "dashboard.read"
                    ]
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
            "kms_key_id": "kms_phase16_tenant_key_001",
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
                "id": RISK_PRINCIPAL,
                "userName": "risk.phase16@example.test",
                "active": true,
                "groups": ["group_risk"]
            },
            {
                "id": VIEWER_PRINCIPAL,
                "userName": "viewer.phase16@example.test",
                "active": true,
                "groups": ["group_viewers"]
            }
        ],
        "groups": [
            {
                "id": "group_risk",
                "displayName": "Risk",
                "roles": ["risk_admin"]
            },
            {
                "id": "group_viewers",
                "displayName": "Viewers",
                "roles": ["viewer"]
            }
        ]
    })
}

fn risk_decisions() -> Value {
    json!({
        "tenant_id": TENANT,
        "decision_schema_version": 1,
        "decisions": [
            {
                "question_id": "next_hosted_ci_connector",
                "status": "deferred",
                "decision": "Choose Buildkite or CircleCI only when a paid pilot proves that CI footprint.",
                "owner": "gtm_owner",
                "review_trigger": "First paid pilot requests hosted CI outside GitHub Actions and GitLab CI.",
                "review_due": "2099-12-31",
                "severity": "medium",
                "rationale": "GitHub Actions and GitLab CI already cover the current wedge; adding another connector before a pilot would increase connector sprawl.",
                "evidence": ["docs/blackboxfix/16_risks_open_questions.md", "blackbox connector list"]
            },
            {
                "question_id": "paid_pilot_dashboard_minimum",
                "status": "resolved",
                "decision": "Use dashboard timeline, evidence, error drilldown, package, and risk assessment API files for the paid-pilot minimum.",
                "owner": "product_owner",
                "review_trigger": "Two pilot prospects ask for a dashboard surface that is absent from the static bundle.",
                "review_due": "2099-12-31",
                "severity": "high",
                "rationale": "The current dashboard exposes physical evidence, pilot packages, and risk assessments without adding a hosted control plane.",
                "evidence": ["api/summary.json", "api/pilot-packages.json", "api/risk-assessments.json"]
            },
            {
                "question_id": "raw_model_text_storage_default",
                "status": "accepted",
                "decision": "Store redacted prompt and response text only, bounded by existing redaction and source-evidence paths.",
                "owner": "security_owner",
                "review_trigger": "A regulated pilot requests untruncated raw model text retention.",
                "review_due": "2099-12-31",
                "severity": "critical",
                "rationale": "Secret-like material must not enter the ledger; raw text expansion needs a separate encrypted artifact store.",
                "evidence": ["redact_v1", "risk secret_leak_scan"]
            },
            {
                "question_id": "customer_managed_key_path",
                "status": "accepted",
                "decision": "Keep customer-managed key ids in enterprise tenant config and choose provider-specific KMS automation by first enterprise pilot environment.",
                "owner": "platform_owner",
                "review_trigger": "First enterprise pilot selects AWS KMS, GCP KMS, or HashiCorp Vault as the required key provider.",
                "review_due": "2099-12-31",
                "severity": "high",
                "rationale": "The ledger already persists key ids and data residency while avoiding provider-specific automation before demand.",
                "evidence": ["enterprise_tenant_config.deployment.kms_key_id"]
            },
            {
                "question_id": "eval_export_format_after_jsonl",
                "status": "resolved",
                "decision": "Use Parquet after JSONL because existing training-corpus export wiring already supports it.",
                "owner": "ml_platform_owner",
                "review_trigger": "A pilot requires HuggingFace Datasets as a delivery format.",
                "review_due": "2099-12-31",
                "severity": "low",
                "rationale": "Parquet minimizes new format risk and reuses the existing exporter direction.",
                "evidence": ["docs/blackboxfix/08_training_eval_corpus.md"]
            }
        ]
    })
}

fn new_case() -> Phase16Case {
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
    assert_eq!(init["schema_version"], 14);
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

    Phase16Case {
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
            "expected_effect": "phase16 risk register fixture"
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
        "phase16_real_tool",
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
        "git:/phase16/repo#working_tree",
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

fn record_trained_dynamicjepa_reality(ledger: &Path, root: &Path, run_id: &str, label: &str) {
    let artifact_path = root.join(format!("{label}-dynamicjepa-artifact.json"));
    write_json(
        &artifact_path,
        &json!({
            "record_kind": "dynamicjepa_artifact_readback",
            "artifact_id": format!("artifact-{label}"),
            "domain_pack_id": "contextgraph_swe_reality_loop_v1",
            "status": "verified"
        }),
    );
    let input = root.join(format!("{label}-dynamicjepa-reality.json"));
    write_json(
        &input,
        &json!({
            "record_kind": "dynamicjepa_reality",
            "schema_version": 1,
            "run_id": run_id,
            "attempt": 1,
            "domain_pack": "contextgraph_swe_reality_loop_v1",
            "source_mode": "trained_dynamicjepa_artifact",
            "panel_count": 1,
            "surprise_events": [],
            "surprise_event_count": 0,
            "surprise_candidate_rate": 0.0,
            "projection_surprise_rate": null,
            "trained_surprise_rate": 0.0,
            "prediction_error_l2": 0.0,
            "prediction_error_l2_status": {"status": "available", "source": "phase16 physical artifact readback"},
            "projection_accuracy": 1.0,
            "plan_trace_coverage": 1.0,
            "guard_decisions": [],
            "source_of_truth_refs": [
                {"kind": "dynamicjepa_artifact_readback", "source_of_truth": format!("file:{}", artifact_path.display())},
                {"kind": "blackbox_ledger", "source_of_truth": format!("sqlite:{}#ledger_records", ledger.display())}
            ],
            "reward_features": {
                "dynamicjepa_trained_panel_count": 1,
                "dynamicjepa_trained_surprise_count": 0,
                "dynamicjepa_trained_surprise_rate": 0.0,
                "dynamicjepa_trained_plan_trace_coverage": 1.0,
                "dynamicjepa_prediction_error_l2": 0.0,
                "dynamicjepa_prediction_accuracy": 1.0
            },
            "trained_artifact": {
                "artifact_id": format!("artifact-{label}"),
                "artifact_readback": format!("file:{}", artifact_path.display()),
                "lifecycle_policy": {
                    "status": "passed",
                    "checks": {
                        "artifact_status": "Active",
                        "file_hashes_verified": true,
                        "latent_collapsed": false,
                        "dataset_shards": 3,
                        "raw_events": 60,
                        "constellations": 1,
                        "threshold_calibrations": 1
                    },
                    "source_of_truth": {"model_artifact": format!("file:{}", artifact_path.display())}
                },
                "predictions": [{
                    "transition_id": format!("{label}-transition-0"),
                    "panel_id": format!("{label}-panel-0"),
                    "action_id": format!("{label}-action-0"),
                    "outcome_id": format!("{label}-outcome-0"),
                    "prediction_id": format!("{label}-prediction-0"),
                    "error_norm": 0.0,
                    "source_of_truth": format!("file:{}", artifact_path.display())
                }],
                "plan_trace_ids": [format!("{label}-plan-0")],
                "source_of_truth": format!("file:{}", artifact_path.display())
            },
            "action_controls": [{
                "status": "passed",
                "mode": "post_action_hard_gate",
                "risky_action": true,
                "transition_id": format!("{label}-transition-0"),
                "observed_candidate_action_id": format!("{label}-action-0"),
                "selected_action_id": format!("{label}-action-0"),
                "source_of_truth": "sqlite:test#dj_plan_traces,dj_guard_decisions"
            }],
            "recorded_at_unix": 1778210000
        }),
    );
    let output = stdout_json(&run_context_graph(&[
        "blackbox",
        "record",
        "dynamicjepa-reality",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--run-id",
        run_id,
        "--input",
        input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
    ]));
    assert_eq!(output["readback"]["status"], "passed");
}

fn compile_trained_dynamicjepa_reward(ledger: &Path, root: &Path, run_id: &str) {
    let spec = root.join(format!("{run_id}-trained-dynamicjepa-reward-spec.json"));
    write_json(
        &spec,
        &json!({
            "reward_spec_schema_version": 1,
            "reward_id": "phase16_trained_dynamicjepa_reward_v1",
            "objective": "require trained DynamicJEPA prediction features before risk acceptance",
            "bounds": {"min": -10.0, "max": 10.0},
            "components": [
                {"id": "correctness", "feature": "correctness", "weight": 4.0},
                {"id": "verification_confidence", "feature": "verification_confidence", "weight": 2.0},
                {"id": "risk_penalty", "feature": "risk", "weight": -2.0},
                {"id": "dj_prediction_accuracy_bonus", "feature": "dynamicjepa_prediction_accuracy", "weight": 1.0}
            ]
        }),
    );
    let output = stdout_json(&run_context_graph(&[
        "blackbox",
        "reward",
        "compile",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--run-id",
        run_id,
        "--input",
        spec.to_str().expect("reward spec path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
    ]));
    assert_eq!(output["readback"]["status"], "passed");
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
        RISK_PRINCIPAL,
        "--retention-class",
        "standard",
        "--data-subject",
        SUBJECT,
        "--purpose",
        "phase16-risk-register",
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
        RISK_PRINCIPAL,
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

fn verified_run(case: &Phase16Case, task: &str) -> String {
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
    record_trained_dynamicjepa_reality(&case.ledger, &case.root, &run_id, "phase16-trained-dj");
    score_run(&case.ledger, &run_id);
    compile_trained_dynamicjepa_reward(&case.ledger, &case.root, &run_id);
    evaluate_retention(&case.ledger, &run_id);
    gate_evaluate(&case.ledger, &run_id);
    run_id
}

fn failed_incident_run(case: &Phase16Case, task: &str) -> String {
    fs::write(case.repo.join("app.txt"), "phase16\nmodified\n").expect("dirty tracked file");
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

fn package_pilot_report(ledger: &Path, out: &Path) -> Value {
    stdout_json(&run_context_graph(&[
        "blackbox",
        "package",
        "pilot-report",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--tenant",
        TENANT,
        "--principal-id",
        RISK_PRINCIPAL,
        "--pilot-id",
        "pilot_phase16",
        "--min-actions",
        "2",
        "--out",
        out.to_str().expect("out path UTF-8"),
        "--agent",
        AGENT,
    ]))
}

fn risk_evaluate(ledger: &Path, decisions: &Path, principal: &str) -> Output {
    run_context_graph(&[
        "blackbox",
        "risk",
        "evaluate",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--tenant",
        TENANT,
        "--principal-id",
        principal,
        "--decisions",
        decisions.to_str().expect("decisions path UTF-8"),
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

fn latest_risk_payload(ledger: &Path) -> Value {
    let conn = Connection::open(ledger).expect("open physical ledger");
    let payload: String = conn
        .query_row(
            "SELECT payload_json
             FROM ledger_records
             WHERE record_kind = 'risk_assessment'
             ORDER BY sequence DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("read latest risk assessment");
    serde_json::from_str(&payload).expect("risk payload JSON")
}

#[test]
fn phase16_risk_assessment_persists_closure_and_edges() {
    let case = new_case();
    let source = format!("sqlite:{}#ledger_records", case.ledger.display());
    println!("FSV16 source_of_truth={source}");
    println!("FSV16 BEFORE counts: {:?}", kind_counts(&case.ledger));

    let happy_run = verified_run(&case, "phase16 verified risk happy path");
    let incident_run = failed_incident_run(&case, "phase16 failed eval candidate");
    println!("FSV16 fixture runs happy={happy_run} incident={incident_run}");

    let decisions = case.root.join("phase16-risk-decisions.json");
    write_json(&decisions, &risk_decisions());

    let blocked_before = kind_counts(&case.ledger);
    let blocked_error = stderr_json(&risk_evaluate(&case.ledger, &decisions, RISK_PRINCIPAL));
    let blocked_after = kind_counts(&case.ledger);
    let blocked_payload = latest_risk_payload(&case.ledger);
    println!(
        "FSV16 blocked BEFORE={:?} AFTER={:?} error={} stored={}",
        blocked_before, blocked_after, blocked_error, blocked_payload
    );
    assert_eq!(blocked_error["error_code"], "RISK_ASSESSMENT_BLOCKED");
    assert_eq!(blocked_payload["decision"]["status"], "blocked");
    assert!(blocked_payload["decision"]["failed_checks"]
        .as_array()
        .expect("failed checks")
        .iter()
        .any(|check| check["reason_code"] == "RISK_PILOT_PACKAGE_MISSING"));
    assert_eq!(
        blocked_after.get("risk_assessment").copied().unwrap_or(0),
        blocked_before.get("risk_assessment").copied().unwrap_or(0) + 1
    );

    let package_out = case.root.join("pilot-package");
    let package = package_pilot_report(&case.ledger, &package_out);
    assert_eq!(package["readback"]["status"], "passed");
    assert!(package_out.join("manifest.json").exists());

    let accepted_before = kind_counts(&case.ledger);
    let accepted = stdout_json(&risk_evaluate(&case.ledger, &decisions, RISK_PRINCIPAL));
    let accepted_after = kind_counts(&case.ledger);
    let accepted_payload = latest_risk_payload(&case.ledger);
    println!(
        "FSV16 accepted BEFORE={:?} AFTER={:?} output={} stored={}",
        accepted_before, accepted_after, accepted, accepted_payload
    );
    assert_eq!(accepted["readback"]["status"], "passed");
    assert_eq!(
        accepted_payload["risk_assessment_id"],
        accepted["risk_assessment_id"]
    );
    assert_eq!(accepted_payload["decision"]["status"], "accepted");
    assert_eq!(accepted_payload["decision"]["failed_checks"], json!([]));
    assert_eq!(
        accepted_payload["decision_register"]["summary"]["closed_decisions_total"],
        5
    );
    assert_eq!(
        accepted_payload["risk_summary"]["tool_evidence_coverage"]["coverage_ratio"],
        1.0
    );
    assert_eq!(
        accepted_after.get("risk_assessment").copied().unwrap_or(0),
        accepted_before.get("risk_assessment").copied().unwrap_or(0) + 1
    );

    let unauthorized_before_risk = record_kind_count(&case.ledger, "risk_assessment");
    let unauthorized_before_access = record_kind_count(&case.ledger, "enterprise_access_decision");
    let unauthorized_error =
        stderr_json(&risk_evaluate(&case.ledger, &decisions, VIEWER_PRINCIPAL));
    let unauthorized_after_risk = record_kind_count(&case.ledger, "risk_assessment");
    let unauthorized_after_access = record_kind_count(&case.ledger, "enterprise_access_decision");
    println!(
        "FSV16 unauthorized BEFORE risk={} access={} AFTER risk={} access={} error={}",
        unauthorized_before_risk,
        unauthorized_before_access,
        unauthorized_after_risk,
        unauthorized_after_access,
        unauthorized_error
    );
    assert_eq!(
        unauthorized_error["error_code"],
        "RISK_ASSESSMENT_RBAC_DENIED"
    );
    assert_eq!(unauthorized_after_risk, unauthorized_before_risk);
    assert_eq!(unauthorized_after_access, unauthorized_before_access + 1);

    let invalid_decisions = case.root.join("phase16-risk-decisions-empty.json");
    write_json(
        &invalid_decisions,
        &json!({
            "tenant_id": TENANT,
            "decision_schema_version": 1,
            "decisions": []
        }),
    );
    let invalid_before = kind_counts(&case.ledger);
    let invalid_error = stderr_json(&risk_evaluate(
        &case.ledger,
        &invalid_decisions,
        RISK_PRINCIPAL,
    ));
    let invalid_after = kind_counts(&case.ledger);
    println!(
        "FSV16 invalid BEFORE={:?} AFTER={:?} error={}",
        invalid_before, invalid_after, invalid_error
    );
    assert_eq!(invalid_error["error_code"], "RISK_DECISIONS_INVALID");
    assert_eq!(invalid_after, invalid_before);

    let dashboard_out = case.root.join("phase16-dashboard");
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
    let dashboard_risk = read_json(&dashboard_out.join("api/risk-assessments.json"));
    let dashboard_errors = read_json(&dashboard_out.join("api/errors.json"));
    let dashboard_html =
        fs::read_to_string(dashboard_out.join("index.html")).expect("dashboard HTML");
    println!(
        "FSV16 dashboard readback build={} summary={} risk={} errors={}",
        dashboard, dashboard_summary, dashboard_risk, dashboard_errors
    );
    assert_eq!(
        dashboard_summary["counts"]["risk_assessments_total"],
        record_kind_count(&case.ledger, "risk_assessment")
    );
    assert!(dashboard_risk["risk_assessments"]
        .as_array()
        .expect("risk assessments")
        .iter()
        .any(|assessment| assessment["status"] == "accepted"));
    assert!(dashboard_risk["risk_assessments"]
        .as_array()
        .expect("risk assessments")
        .iter()
        .any(|assessment| assessment["status"] == "blocked"));
    assert!(dashboard_errors["error_codes"]
        .as_array()
        .expect("dashboard errors")
        .iter()
        .any(|error| error["error_code"] == "RISK_PILOT_PACKAGE_MISSING"));
    assert!(dashboard_html.contains("Risk Assessments"));
    assert!(dashboard_html.contains("api/risk-assessments.json"));

    let dashboard_api = stdout_json(&run_context_graph(&[
        "blackbox",
        "dashboard",
        "api",
        "--ledger",
        case.ledger.to_str().expect("ledger path UTF-8"),
        "--view",
        "risk-assessments",
    ]));
    assert_eq!(
        dashboard_api["data"]["risk_assessments"]
            .as_array()
            .expect("risk assessments")
            .len() as i64,
        record_kind_count(&case.ledger, "risk_assessment")
    );

    let connector = stdout_json(&run_context_graph(&[
        "blackbox",
        "connector",
        "test",
        "--connector",
        "blackbox_risk_v1",
        "--source",
        case.ledger.to_str().expect("ledger path UTF-8"),
    ]));
    assert_eq!(connector["connector"], "blackbox_risk_v1");
    assert_eq!(
        connector["readback"]["risk_assessment_records_total"],
        record_kind_count(&case.ledger, "risk_assessment")
    );

    let final_counts = kind_counts(&case.ledger);
    println!("FSV16 FINAL physical counts: {:?}", final_counts);
    assert_eq!(final_counts.get("enterprise_tenant_config"), Some(&1));
    assert_eq!(final_counts.get("enterprise_directory_snapshot"), Some(&1));
    assert_eq!(final_counts.get("agent_run"), Some(&2));
    assert_eq!(final_counts.get("tool_call"), Some(&2));
    assert_eq!(final_counts.get("evidence_packet"), Some(&2));
    assert_eq!(final_counts.get("incident_packet"), Some(&1));
    assert_eq!(final_counts.get("release_gate"), Some(&1));
    assert_eq!(final_counts.get("pilot_package"), Some(&1));
    assert_eq!(final_counts.get("risk_assessment"), Some(&2));
}
