//! BlackBoxFix reward compiler FSV tests.
//!
//! These tests run the real `context-graph blackbox` binary against a real
//! SQLite ledger and a real local Git repository. Assertions inspect the
//! physical ledger source of truth directly: `ledger_records.reward_compilation`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const TENANT: &str = "tenant_reward_test";
const AGENT: &str = "agent_reward_test";

struct RewardCase {
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

fn sha256_file(path: &Path) -> String {
    let bytes = fs::read(path).expect("read file for sha256");
    let digest = Sha256::digest(bytes);
    let mut out = String::from("sha256:");
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
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
    must_git(repo, &["config", "user.email", "reward@example.test"]);
    must_git(repo, &["config", "user.name", "Reward Test"]);
    fs::write(repo.join("service.yaml"), "health_check:\n  timeout: 30\n")
        .expect("write tracked file");
    must_git(repo, &["add", "service.yaml"]);
    must_git(repo, &["commit", "-m", "initial reward compiler fixture"]);
}

fn new_case() -> RewardCase {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().to_path_buf();
    let ledger = root.join("blackbox.sqlite");
    let repo = root.join("repo");
    stdout_json(&run_context_graph(&[
        "blackbox",
        "init",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
    ]));
    init_git_repo(&repo);
    RewardCase {
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
            "expected_effect": "reward compiler fixture"
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
        "reward_real_tool",
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
        "git:/reward/repo#working_tree",
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
) {
    let spec = root.join(format!("{run_id}-{tool_call_id}-verify.json"));
    write_json(
        &spec,
        &json!({
            "verifier_id": "git_working_tree_clean_v1",
            "run_id": run_id,
            "tool_call_id": tool_call_id,
            "source": { "repo": repo },
            "expected": { "clean": true }
        }),
    );
    stdout_json(&run_context_graph(&[
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
    ]));
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

fn reward_compile(ledger: &Path, run_id: &str, input: &Path) -> Output {
    run_context_graph(&[
        "blackbox",
        "reward",
        "compile",
        "--ledger",
        ledger.to_str().expect("ledger path UTF-8"),
        "--run-id",
        run_id,
        "--input",
        input.to_str().expect("reward spec path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
    ])
}

fn reward_spec(path: &Path) {
    write_json(
        path,
        &json!({
            "reward_spec_schema_version": 1,
            "reward_id": "verified_outcome_reward_v1",
            "objective": "maximize verified source-of-truth outcomes while penalizing risk, incidents, and missing governance",
            "bounds": {"min": -10.0, "max": 10.0},
            "components": [
                {"id": "correctness", "feature": "correctness", "weight": 4.0},
                {"id": "verification_confidence", "feature": "verification_confidence", "weight": 2.0},
                {"id": "risk_penalty", "feature": "risk", "weight": -2.0},
                {"id": "incident_penalty", "feature": "incident_present", "weight": -3.0},
                {"id": "policy_missing_penalty", "feature": "policy_missing", "weight": -1.0},
                {"id": "training_value", "feature": "training_value", "weight": 1.0}
            ]
        }),
    );
}

fn kind_counts(ledger: &Path) -> BTreeMap<String, i64> {
    let conn = Connection::open(ledger).expect("open ledger");
    let mut stmt = conn
        .prepare("SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind")
        .expect("prepare kind count query");
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("query kind counts");
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .expect("decode kind counts")
}

fn latest_payload(ledger: &Path, run_id: &str, kind: &str) -> Value {
    let conn = Connection::open(ledger).expect("open ledger");
    let payload_json: String = conn
        .query_row(
            "SELECT payload_json
             FROM ledger_records
             WHERE record_kind = ?1 AND run_id = ?2
             ORDER BY sequence DESC
             LIMIT 1",
            params![kind, run_id],
            |row| row.get(0),
        )
        .expect("read latest payload");
    serde_json::from_str(&payload_json).expect("payload JSON")
}

fn reward_count(ledger: &Path) -> i64 {
    let conn = Connection::open(ledger).expect("open ledger");
    conn.query_row(
        "SELECT COUNT(*) FROM ledger_records WHERE record_kind = 'reward_compilation'",
        [],
        |row| row.get(0),
    )
    .expect("count reward compilations")
}

#[test]
fn reward_compiler_persists_happy_path_and_fails_closed_edges() {
    let case = new_case();
    println!(
        "FSV_REWARD source_of_truth=sqlite:{}#ledger_records",
        case.ledger.display()
    );
    println!(
        "FSV_REWARD git_source_before={:?}",
        must_git(
            &case.repo,
            &["status", "--porcelain=v1", "--untracked-files=all"]
        )
    );
    println!("FSV_REWARD BEFORE counts: {:?}", kind_counts(&case.ledger));

    let happy_run = record_run(
        &case.ledger,
        "reward compiler verified deployment artifact",
        "medium",
    );
    policy_allow_git_apply_patch(&case.ledger, &happy_run);
    let happy_tool = record_tool_call(&case.ledger, &case.root, &happy_run, "happy");
    verify_working_tree_clean(
        &case.ledger,
        &case.root,
        &case.repo,
        &happy_run,
        &happy_tool,
    );
    let happy_score = score_run(&case.ledger, &happy_run);
    assert_eq!(happy_score["score"]["trust_level"], "verified_with_policy");

    let spec_path = case.root.join("reward-spec.json");
    reward_spec(&spec_path);
    let happy_before = kind_counts(&case.ledger);
    let reward_output = stdout_json(&reward_compile(&case.ledger, &happy_run, &spec_path));
    let stored_reward = latest_payload(&case.ledger, &happy_run, "reward_compilation");
    println!(
        "FSV_REWARD happy BEFORE counts={:?} AFTER counts={:?}",
        happy_before,
        kind_counts(&case.ledger)
    );
    println!("FSV_REWARD happy STORED reward: {}", stored_reward);
    assert_eq!(
        stored_reward["reward_compilation_id"],
        reward_output["reward_compilation_id"]
    );
    assert_eq!(stored_reward["reward"], reward_output["reward"]);
    assert_eq!(stored_reward["reward"]["raw"], 4.985);
    assert_eq!(stored_reward["reward"]["clipped"], 4.985);
    assert_eq!(stored_reward["reward"]["normalized_0_1"], 0.7493);
    assert_eq!(stored_reward["reward"]["loss"], -4.985);
    assert_eq!(stored_reward["reward"]["class"], "positive");
    assert_eq!(stored_reward["feature_vector"]["correctness"]["value"], 1.0);
    assert_eq!(
        stored_reward["feature_vector"]["verification_confidence"]["value"],
        0.9725
    );
    assert_eq!(stored_reward["feature_vector"]["risk"]["value"], 0.58);
    assert_eq!(
        stored_reward["reward_spec"]["sha256"],
        sha256_file(&spec_path)
    );
    assert_eq!(reward_output["readback"]["status"], "passed");
    assert_eq!(
        stored_reward["training_sample"]["sample_kind"],
        "verified_outcome_reward_sample"
    );
    assert!(
        stored_reward["episode_context"]["agent_run"]["task"]["requested_outcome"]
            .as_str()
            .expect("requested outcome")
            .contains("reward compiler")
    );

    let rescore = score_run(&case.ledger, &happy_run);
    let rescored_stored = latest_payload(&case.ledger, &happy_run, "scorecard");
    println!("FSV_REWARD rescore AFTER reward row: {}", rescored_stored);
    assert_eq!(rescore["score"], happy_score["score"]);
    assert!(!rescored_stored["input_record_refs"]
        .as_array()
        .expect("input refs")
        .iter()
        .any(|item| item["record_kind"] == "reward_compilation"));

    let empty_spec = case.root.join("empty-reward-spec.json");
    fs::write(&empty_spec, "").expect("write empty spec");
    let before_empty = reward_count(&case.ledger);
    let empty_error = stderr_json(&reward_compile(&case.ledger, &happy_run, &empty_spec));
    let after_empty = reward_count(&case.ledger);
    println!(
        "FSV_REWARD edge-empty BEFORE reward_count={} AFTER reward_count={} error={}",
        before_empty, after_empty, empty_error
    );
    assert_eq!(empty_error["error_code"], "REWARD_SPEC_INVALID");
    assert_eq!(before_empty, after_empty);

    let unknown_spec = case.root.join("unknown-feature-reward-spec.json");
    write_json(
        &unknown_spec,
        &json!({
            "reward_spec_schema_version": 1,
            "reward_id": "bad_unknown_feature",
            "objective": "prove unknown features fail closed",
            "bounds": {"min": -1.0, "max": 1.0},
            "components": [
                {"id": "unknown", "feature": "made_up_signal", "weight": 1.0}
            ]
        }),
    );
    let before_unknown = reward_count(&case.ledger);
    let unknown_error = stderr_json(&reward_compile(&case.ledger, &happy_run, &unknown_spec));
    let after_unknown = reward_count(&case.ledger);
    println!(
        "FSV_REWARD edge-unknown BEFORE reward_count={} AFTER reward_count={} error={}",
        before_unknown, after_unknown, unknown_error
    );
    assert_eq!(unknown_error["error_code"], "REWARD_FEATURE_UNKNOWN");
    assert_eq!(before_unknown, after_unknown);

    let no_tool_run = record_run(&case.ledger, "reward compiler null correctness edge", "low");
    let no_tool_score = score_run(&case.ledger, &no_tool_run);
    assert!(no_tool_score["score"]["correctness"].is_null());
    let null_feature_spec = case.root.join("null-feature-reward-spec.json");
    write_json(
        &null_feature_spec,
        &json!({
            "reward_spec_schema_version": 1,
            "reward_id": "null_feature_must_be_explicit",
            "objective": "prove nullable source features fail unless the spec declares null handling",
            "bounds": {"min": -1.0, "max": 1.0},
            "components": [
                {"id": "correctness", "feature": "correctness", "weight": 1.0}
            ]
        }),
    );
    let before_null = reward_count(&case.ledger);
    let null_error = stderr_json(&reward_compile(
        &case.ledger,
        &no_tool_run,
        &null_feature_spec,
    ));
    let after_null = reward_count(&case.ledger);
    println!(
        "FSV_REWARD edge-null BEFORE reward_count={} AFTER reward_count={} error={}",
        before_null, after_null, null_error
    );
    assert_eq!(null_error["error_code"], "REWARD_FEATURE_NULL");
    assert_eq!(before_null, after_null);

    let final_counts = kind_counts(&case.ledger);
    println!("FSV_REWARD FINAL physical counts: {:?}", final_counts);
    assert_eq!(final_counts.get("agent_run").copied().unwrap_or(0), 2);
    assert_eq!(final_counts.get("policy_decision").copied().unwrap_or(0), 1);
    assert_eq!(final_counts.get("tool_call").copied().unwrap_or(0), 1);
    assert_eq!(final_counts.get("expected_delta").copied().unwrap_or(0), 1);
    assert_eq!(
        final_counts.get("readback_snapshot").copied().unwrap_or(0),
        1
    );
    assert_eq!(final_counts.get("verifier_result").copied().unwrap_or(0), 1);
    assert_eq!(final_counts.get("evidence_packet").copied().unwrap_or(0), 1);
    assert_eq!(final_counts.get("scorecard").copied().unwrap_or(0), 3);
    assert_eq!(
        final_counts.get("reward_compilation").copied().unwrap_or(0),
        1
    );
}
