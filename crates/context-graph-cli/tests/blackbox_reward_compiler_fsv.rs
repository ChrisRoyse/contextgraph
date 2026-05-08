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

fn record_dynamicjepa_reality(
    ledger: &Path,
    root: &Path,
    run_id: &str,
    label: &str,
    panel_count: u64,
    surprise_count: u64,
) -> Value {
    let input = write_dynamicjepa_reality_payload(root, run_id, label, panel_count, surprise_count);
    stdout_json(&run_context_graph(&[
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
    ]))
}

fn write_dynamicjepa_reality_payload(
    root: &Path,
    run_id: &str,
    label: &str,
    panel_count: u64,
    surprise_count: u64,
) -> PathBuf {
    assert!(surprise_count <= panel_count);
    let fixture_dir = root.join(format!("{label}-dynamicjepa-fixture"));
    fs::create_dir_all(&fixture_dir).expect("create DynamicJEPA fixture dir");
    let transcript = fixture_dir.join("agent-tool-transcript.json");
    let mut source_refs = Vec::new();
    let mut surprise_events = Vec::new();
    source_refs.push(json!({
        "kind": "agent_tool_transcript",
        "source_of_truth": format!("file:{}", transcript.display()),
        "sha256": "sha256:pending"
    }));
    for idx in 0..panel_count {
        let transition = fixture_dir.join(format!("transition-{idx}.json"));
        let is_surprise = idx < surprise_count;
        write_json(
            &transition,
            &json!({
                "record_kind": "contextgraph_harness_transition",
                "transition_id": format!("{label}-transition-{idx}"),
                "tool_name": if is_surprise { "apply_line_window_edit" } else { "open_file_window" },
                "dynamicjepa_mapping": {
                    "domain_pack_id": "contextgraph_swe_reality_loop_v1",
                    "state_id": format!("{label}-state-{idx}"),
                    "action_id": format!("{label}-action-{idx}"),
                    "outcome_id": format!("{label}-outcome-{idx}"),
                    "transition_id": format!("{label}-transition-{idx}"),
                    "surprise_kind": if is_surprise { json!("UnexpectedOutcome") } else { Value::Null }
                },
                "outcome": {
                    "label": if is_surprise { "Harmful" } else { "Useful" },
                    "utility_delta": if is_surprise { -0.35 } else { 0.25 },
                    "correction_required": is_surprise
                },
                "source_of_truth": format!("file:{}", transition.display())
            }),
        );
        let transition_sha = sha256_file(&transition);
        source_refs.push(json!({
            "kind": "harness_transition",
            "transition_id": format!("{label}-transition-{idx}"),
            "source_of_truth": format!("file:{}", transition.display()),
            "sha256": transition_sha
        }));
        if is_surprise {
            surprise_events.push(json!({
                "event_id": format!("srp-{label}-{idx}"),
                "kind": "UnexpectedOutcome",
                "magnitude": 0.35,
                "transition_id": format!("{label}-transition-{idx}"),
                "tool_name": "apply_line_window_edit",
                "tool_family": "edit",
                "outcome_label": "Harmful",
                "source_of_truth": format!("file:{}", transition.display())
            }));
        }
    }
    write_json(
        &transcript,
        &json!({
            "record_kind": "contextgraph_agent_tool_transcript",
            "executed_tool_calls": panel_count,
            "entries": []
        }),
    );
    source_refs[0]["sha256"] = json!(sha256_file(&transcript));
    source_refs.push(json!({
        "kind": "blackbox_ledger",
        "source_of_truth": "sqlite:pending#ledger_records",
        "record_kind": "dynamicjepa_reality"
    }));
    let surprise_rate = if panel_count == 0 {
        0.0
    } else {
        ((surprise_count as f64 / panel_count as f64) * 10_000.0).round() / 10_000.0
    };
    let projection_accuracy = ((1.0 - surprise_rate) * 10_000.0).round() / 10_000.0;
    let payload = json!({
        "record_kind": "dynamicjepa_reality",
        "schema_version": 1,
        "run_id": run_id,
        "attempt": 1,
        "domain_pack": "contextgraph_swe_reality_loop_v1",
        "source_mode": "harness_transition_projection",
        "panel_count": panel_count,
        "surprise_events": surprise_events,
        "surprise_event_count": surprise_count,
        "surprise_candidate_rate": surprise_rate,
        "prediction_error_l2": Value::Null,
        "prediction_error_l2_status": {
            "status": "unavailable",
            "reason": "test fixture uses physical harness-transition projection records, not a trained DynamicJEPA artifact"
        },
        "projection_accuracy": projection_accuracy,
        "plan_trace_coverage": 1.0,
        "guard_decisions": [],
        "source_of_truth_refs": source_refs,
        "reward_features": {
            "dynamicjepa_projection_panel_count": panel_count,
            "dynamicjepa_projection_surprise_count": surprise_count,
            "dynamicjepa_projection_surprise_rate": surprise_rate,
            "dynamicjepa_projection_accuracy": projection_accuracy,
            "dynamicjepa_plan_trace_coverage": 1.0
        },
        "recorded_at_unix": 1778210000
    });
    let input = root.join(format!("{label}-dynamicjepa-reality.json"));
    write_json(&input, &payload);
    input
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

fn dynamicjepa_reward_spec(path: &Path) {
    write_json(
        path,
        &json!({
            "reward_spec_schema_version": 1,
            "reward_id": "dynamicjepa_reward_features_v1",
            "objective": "prove DynamicJEPA reward features are compiled only from persisted scorecard state",
            "bounds": {"min": -10.0, "max": 10.0},
            "components": [
                {"id": "dj_surprise_rate_penalty", "feature": "dynamicjepa_projection_surprise_rate", "weight": -2.0},
                {"id": "dj_projection_accuracy_bonus", "feature": "dynamicjepa_projection_accuracy", "weight": 3.0},
                {"id": "dj_plan_trace_bonus", "feature": "dynamicjepa_plan_trace_coverage", "weight": 1.0}
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

fn kind_count(ledger: &Path, kind: &str) -> i64 {
    let conn = Connection::open(ledger).expect("open ledger");
    conn.query_row(
        "SELECT COUNT(*) FROM ledger_records WHERE record_kind = ?1",
        params![kind],
        |row| row.get(0),
    )
    .expect("count ledger records by kind")
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

#[test]
fn dynamicjepa_reality_features_are_persisted_scored_and_fail_closed() {
    let case = new_case();
    println!(
        "FSV_DJ_REWARD source_of_truth=sqlite:{}#ledger_records",
        case.ledger.display()
    );
    println!(
        "FSV_DJ_REWARD BEFORE counts: {:?}",
        kind_counts(&case.ledger)
    );

    let happy_run = record_run(
        &case.ledger,
        "dynamicjepa reward projection happy path",
        "medium",
    );
    let dj_before = kind_counts(&case.ledger);
    let dj_record =
        record_dynamicjepa_reality(&case.ledger, &case.root, &happy_run, "happy-dj", 4, 1);
    println!(
        "FSV_DJ_REWARD happy record BEFORE counts={:?} AFTER counts={:?}",
        dj_before,
        kind_counts(&case.ledger)
    );
    assert_eq!(dj_record["readback"]["status"], "passed");
    assert_eq!(kind_count(&case.ledger, "dynamicjepa_reality"), 1);

    let score = score_run(&case.ledger, &happy_run);
    let stored_score = latest_payload(&case.ledger, &happy_run, "scorecard");
    println!("FSV_DJ_REWARD happy STORED scorecard: {}", stored_score);
    assert_eq!(
        score["score_inputs"]["dynamicjepa_reality"]["status"],
        "present"
    );
    assert_eq!(
        stored_score["score_inputs"]["dynamicjepa_reality"]["reward_features"]
            ["dynamicjepa_projection_surprise_rate"],
        0.25
    );
    assert_eq!(
        stored_score["score_inputs"]["dynamicjepa_reality"]["reward_features"]
            ["dynamicjepa_projection_accuracy"],
        0.75
    );

    let spec = case.root.join("dynamicjepa-reward-spec.json");
    dynamicjepa_reward_spec(&spec);
    let reward_before = reward_count(&case.ledger);
    let reward_output = stdout_json(&reward_compile(&case.ledger, &happy_run, &spec));
    let stored_reward = latest_payload(&case.ledger, &happy_run, "reward_compilation");
    println!(
        "FSV_DJ_REWARD happy reward BEFORE reward_count={} AFTER reward_count={} STORED={}",
        reward_before,
        reward_count(&case.ledger),
        stored_reward
    );
    assert_eq!(reward_count(&case.ledger), reward_before + 1);
    assert_eq!(
        stored_reward["reward_compilation_id"],
        reward_output["reward_compilation_id"]
    );
    assert_eq!(
        stored_reward["feature_vector"]["dynamicjepa_projection_surprise_rate"]["value"],
        0.25
    );
    assert_eq!(
        stored_reward["feature_vector"]["dynamicjepa_projection_accuracy"]["value"],
        0.75
    );
    assert_eq!(
        stored_reward["feature_vector"]["dynamicjepa_plan_trace_coverage"]["value"],
        1.0
    );
    assert_eq!(stored_reward["reward"]["raw"], 2.75);

    let invalid_run = record_run(
        &case.ledger,
        "dynamicjepa invalid count mismatch edge",
        "medium",
    );
    let bad_input = write_dynamicjepa_reality_payload(&case.root, &invalid_run, "bad-dj", 2, 1);
    let mut bad_payload: Value =
        serde_json::from_slice(&fs::read(&bad_input).expect("read bad payload")).unwrap();
    bad_payload["surprise_event_count"] = json!(2);
    write_json(&bad_input, &bad_payload);
    let before_bad = kind_count(&case.ledger, "dynamicjepa_reality");
    let bad_error = stderr_json(&run_context_graph(&[
        "blackbox",
        "record",
        "dynamicjepa-reality",
        "--ledger",
        case.ledger.to_str().expect("ledger path UTF-8"),
        "--run-id",
        &invalid_run,
        "--input",
        bad_input.to_str().expect("input path UTF-8"),
        "--agent",
        AGENT,
        "--tenant",
        TENANT,
    ]));
    let after_bad = kind_count(&case.ledger, "dynamicjepa_reality");
    println!(
        "FSV_DJ_REWARD edge-invalid BEFORE dynamicjepa_reality_count={} AFTER={} error={}",
        before_bad, after_bad, bad_error
    );
    assert_eq!(bad_error["error_code"], "DYNAMICJEPA_REALITY_INVALID");
    assert_eq!(before_bad, after_bad);

    let missing_run = record_run(&case.ledger, "dynamicjepa missing record edge", "medium");
    score_run(&case.ledger, &missing_run);
    let before_missing_reward = reward_count(&case.ledger);
    let missing_error = stderr_json(&reward_compile(&case.ledger, &missing_run, &spec));
    let after_missing_reward = reward_count(&case.ledger);
    println!(
        "FSV_DJ_REWARD edge-missing BEFORE reward_count={} AFTER={} error={}",
        before_missing_reward, after_missing_reward, missing_error
    );
    assert_eq!(missing_error["error_code"], "REWARD_FEATURE_NULL");
    assert_eq!(before_missing_reward, after_missing_reward);

    let max_run = record_run(&case.ledger, "dynamicjepa max surprise edge", "medium");
    let max_before = kind_counts(&case.ledger);
    record_dynamicjepa_reality(&case.ledger, &case.root, &max_run, "max-dj", 3, 3);
    score_run(&case.ledger, &max_run);
    let max_reward = stdout_json(&reward_compile(&case.ledger, &max_run, &spec));
    let stored_max_reward = latest_payload(&case.ledger, &max_run, "reward_compilation");
    println!(
        "FSV_DJ_REWARD edge-max BEFORE counts={:?} AFTER counts={:?} STORED={}",
        max_before,
        kind_counts(&case.ledger),
        stored_max_reward
    );
    assert_eq!(
        max_reward["feature_vector"]["dynamicjepa_projection_surprise_rate"]["value"],
        1.0
    );
    assert_eq!(
        stored_max_reward["feature_vector"]["dynamicjepa_projection_accuracy"]["value"],
        0.0
    );
    assert_eq!(stored_max_reward["reward"]["raw"], -1.0);

    let final_counts = kind_counts(&case.ledger);
    println!("FSV_DJ_REWARD FINAL physical counts: {:?}", final_counts);
    assert_eq!(
        final_counts
            .get("dynamicjepa_reality")
            .copied()
            .unwrap_or(0),
        2
    );
    assert_eq!(
        final_counts.get("reward_compilation").copied().unwrap_or(0),
        2
    );
}
