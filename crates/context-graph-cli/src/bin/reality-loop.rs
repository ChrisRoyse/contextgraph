use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_RUNTIME_ROOT: &str = "/tmp/contextgraph-rust-reality-loop";
const DEFAULT_CONTEXT_GRAPH_BIN: &str = "target/debug/context-graph";
const DEFAULT_MODEL: &str = "gemma4:e4b";
const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434";
const DEFAULT_GPU_MIN_FREE_MB: u64 = 8192;
const DEFAULT_OLLAMA_KEEP_ALIVE: &str = "30m";
const DEFAULT_OFFICIAL_DOCKER_HOST: &str = "unix:///var/run/docker.sock";
const DEFAULT_OFFICIAL_DOCKER_CONFIG: &str = "/home/cabdru/.cache/contextgraph-docker-config";
const DATASET_NAME: &str = "princeton-nlp/SWE-bench_Lite";

#[derive(Parser, Debug, Clone)]
#[command(name = "reality-loop")]
#[command(about = "Rust-owned ContextGraph reality loop for local SWE-bench repair attempts")]
struct Args {
    #[arg(long, default_value = DEFAULT_MODEL)]
    model: String,
    #[arg(long)]
    task_manifest: PathBuf,
    #[arg(long, default_value = DEFAULT_RUNTIME_ROOT)]
    runtime_root: PathBuf,
    #[arg(long, default_value = DEFAULT_OLLAMA_URL)]
    ollama_url: String,
    #[arg(long, default_value = DEFAULT_CONTEXT_GRAPH_BIN)]
    context_graph_bin: PathBuf,
    #[arg(long, default_value_t = 1)]
    max_attempts: u64,
    #[arg(long)]
    run_until_resolved: bool,
    #[arg(long)]
    official_eval_each_attempt: bool,
    #[arg(
        long,
        default_value = "/home/cabdru/.cache/contextgraph-swebench/venv/bin/python"
    )]
    swebench_python: PathBuf,
    #[arg(long, default_value_t = 900)]
    official_timeout: u64,
    #[arg(long, default_value_t = 1)]
    official_max_workers: u64,
    #[arg(long, default_value = "env")]
    official_cache_level: String,
    #[arg(long, default_value = DEFAULT_OFFICIAL_DOCKER_HOST)]
    official_docker_host: String,
    #[arg(long, default_value = DEFAULT_OFFICIAL_DOCKER_CONFIG)]
    official_docker_config: PathBuf,
    #[arg(long, default_value_t = 0)]
    gpu_index: u64,
    #[arg(long, default_value_t = DEFAULT_GPU_MIN_FREE_MB)]
    gpu_min_free_mb: u64,
    #[arg(long)]
    gpu_max_used_mb: Option<u64>,
    #[arg(long, default_value = DEFAULT_OLLAMA_KEEP_ALIVE)]
    ollama_keep_alive: String,
    #[arg(long, default_value_t = 300)]
    model_timeout: u64,
    #[arg(long, default_value_t = 600)]
    git_timeout: u64,
    #[arg(long, default_value_t = 0)]
    sleep_between_attempts_ms: u64,
}

#[derive(Debug, Serialize)]
struct RealityError {
    status: &'static str,
    error_code: String,
    message: String,
    field_path: String,
    remediation: String,
    details: Value,
    source_of_truth: Option<String>,
}

impl RealityError {
    fn new(
        error_code: impl Into<String>,
        message: impl Into<String>,
        field_path: impl Into<String>,
        remediation: impl Into<String>,
        details: Value,
        source_of_truth: Option<String>,
    ) -> Self {
        Self {
            status: "error",
            error_code: error_code.into(),
            message: message.into(),
            field_path: field_path.into(),
            remediation: remediation.into(),
            details,
            source_of_truth,
        }
    }
}

type Result<T> = std::result::Result<T, RealityError>;

#[derive(Debug, Clone, Serialize)]
struct ProcessResult {
    command: Vec<String>,
    cwd: String,
    exit_code: i32,
    stdout: String,
    stderr: String,
    elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskManifest {
    dataset: Option<String>,
    instance_id: String,
    repo: String,
    base_commit: String,
    problem_statement: String,
    #[serde(default)]
    fail_to_pass: Vec<String>,
    #[serde(default)]
    pass_to_pass_count: Option<u64>,
    #[serde(default)]
    official_test_patch_sha256: Option<String>,
    #[serde(default)]
    oracle_patch_sha256: Option<String>,
    #[serde(default)]
    source_of_truth: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GpuState {
    index: u64,
    name: String,
    uuid: String,
    memory_total_mb: u64,
    memory_used_mb: u64,
    memory_free_mb: u64,
    driver_version: String,
    compute_processes_raw: String,
    source_of_truth: String,
    read_at_unix: u64,
}

#[derive(Debug, Clone, Serialize)]
struct AttemptRecord {
    attempt: u64,
    status: String,
    error_code: Option<String>,
    message: Option<String>,
    remediation: Option<String>,
    prompt_sha256: String,
    response_sha256: String,
    patch_sha256: Option<String>,
    repeated_patch: bool,
    validation_path: String,
    official_evidence_path: Option<String>,
    resolved: Option<bool>,
    failure_evidence: Value,
    official_reality: Option<Value>,
    patch_preview: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let exit_code = match run(args.clone()).await {
        Ok(summary) => {
            println!(
                "RUST_REALITY_LOOP_SUMMARY={}",
                summary["source_of_truth"]["summary"].as_str().unwrap_or("")
            );
            0
        }
        Err(err) => {
            match write_failure_artifact(&args, &err) {
                Ok(path) => eprintln!(
                    "RUST_REALITY_LOOP_ERROR_SOURCE_OF_TRUTH={}",
                    file_sot(&path)
                ),
                Err(write_err) => eprintln!("RUST_REALITY_LOOP_ERROR_LOGGING_FAILED={write_err}"),
            }
            eprintln!("{}", serde_json::to_string_pretty(&err).unwrap());
            1
        }
    };
    std::process::exit(exit_code);
}

async fn run(args: Args) -> Result<Value> {
    let runtime_root = resolve_path(&args.runtime_root)?;
    if args.max_attempts < 1 {
        return Err(RealityError::new(
            "RUST_REALITY_MAX_ATTEMPTS_INVALID",
            "--max-attempts must be at least 1",
            "max_attempts",
            "pass --max-attempts >= 1",
            json!({ "max_attempts": args.max_attempts }),
            None,
        ));
    }
    if args.run_until_resolved && !args.official_eval_each_attempt {
        return Err(RealityError::new(
            "RUST_REALITY_CONTINUOUS_REQUIRES_OFFICIAL_EVAL",
            "--run-until-resolved requires --official-eval-each-attempt",
            "official_eval_each_attempt",
            "enable official SWE-bench readback so success is resolved_ids, not model text",
            json!({ "run_until_resolved": true }),
            None,
        ));
    }
    let context_graph_bin = resolve_path(&args.context_graph_bin)?;
    require_file(
        &context_graph_bin,
        "context_graph_bin",
        "RUST_REALITY_CONTEXT_GRAPH_BIN_MISSING",
    )?;
    let task_manifest_path = resolve_path(&args.task_manifest)?;
    require_file(
        &task_manifest_path,
        "task_manifest",
        "RUST_REALITY_TASK_MANIFEST_MISSING",
    )?;
    let task: TaskManifest = read_json(&task_manifest_path)?;
    validate_task_manifest_contract(&task, &task_manifest_path)?;
    if !args.official_eval_each_attempt {
        return Err(RealityError::new(
            "RUST_REALITY_OFFICIAL_EVAL_REQUIRED",
            "SWE reality loops require official Docker SWE-bench evaluation on every candidate attempt",
            "official_eval_each_attempt",
            "rerun with --official-eval-each-attempt so measurement scope cannot collapse to local patch validation",
            json!({
                "claim_scope": "official_swe_instance_resolution_only",
                "reason": "local validation can prove patch application, but cannot prove SWE-bench resolution",
                "task_source_of_truth": file_sot(&task_manifest_path)
            }),
            Some(file_sot(&task_manifest_path)),
        ));
    }
    let loop_id = format!(
        "rust_reality_{}_{}",
        unix_ns()?,
        short_hash(&format!("{}:{}", args.model, task.instance_id))
    );
    let run_root = runtime_root.join(&loop_id);
    create_dir_checked(&run_root)?;
    let contract_path = run_root.join("measurement-contract.json");
    let measurement_contract = build_measurement_contract(&args, &task, &task_manifest_path)?;
    write_json_checked(&contract_path, &measurement_contract)?;
    let task_root = run_root
        .join("tasks")
        .join(&task.instance_id)
        .join(safe_id(&args.model));
    create_dir_checked(&task_root)?;
    let attempts_root = task_root.join("attempts");
    create_dir_checked(&attempts_root)?;
    let ledger = task_root.join("blackbox.sqlite");
    let reality_events = run_root.join("reality-events.jsonl");
    let workspace = runtime_root
        .join("workspaces")
        .join(&task.instance_id)
        .join("repo");

    let gpu_initial = read_gpu_vram_state(args.gpu_index)?;
    assert_gpu_budget(
        &gpu_initial,
        args.gpu_min_free_mb,
        args.gpu_max_used_mb,
        "loop_start",
        &args.model,
    )?;
    write_json_checked(&run_root.join("task-manifest-readback.json"), &task)?;

    blackbox(
        &context_graph_bin,
        &["blackbox", "init", "--ledger", path_str(&ledger)],
        &task_root,
    )?;
    let run_out = blackbox(
        &context_graph_bin,
        &[
            "blackbox",
            "record",
            "run",
            "--ledger",
            path_str(&ledger),
            "--task",
            &format!("Rust reality loop SWE repair for {}", task.instance_id),
            "--agent",
            "contextgraph_rust_reality_loop",
            "--model",
            &args.model,
            "--task-kind",
            "rust_reality_swe_loop",
            "--risk-tier",
            "high",
        ],
        &task_root,
    )?;
    let run_id = run_out
        .get("run_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_BLACKBOX_RUN_ID_MISSING",
                "blackbox record run did not return run_id",
                "blackbox.record_run.run_id",
                "inspect BlackBoxFix run output before continuing",
                run_out.clone(),
                Some(sqlite_sot(&ledger)),
            )
        })?
        .to_string();

    append_event(
        &reality_events,
        json!({
            "event_type": "rust_reality_loop_started",
            "status": "running",
            "loop_id": loop_id,
            "model": args.model,
            "task": task,
            "gpu_initial": gpu_initial,
            "source_of_truth": {
                "task_manifest": file_sot(&task_manifest_path),
                "measurement_contract": file_sot(&contract_path),
                "ledger": sqlite_sot(&ledger)
            }
        }),
    )?;

    ensure_workspace(&task, &workspace, args.git_timeout)?;
    let mut previous_patch_hashes: Vec<String> = Vec::new();
    let mut attempts: Vec<AttemptRecord> = Vec::new();
    let client = reqwest::Client::new();
    let mut solved = false;
    let mut attempt = 1_u64;
    while args.run_until_resolved || attempt <= args.max_attempts {
        let attempt_dir = attempts_root.join(format!("attempt-{attempt:06}"));
        create_dir_checked(&attempt_dir)?;
        reset_workspace(&workspace, &task.base_commit, args.git_timeout)?;
        let repo_context = collect_repository_context(&workspace, &task)?;
        write_json_checked(&attempt_dir.join("repository-context.json"), &repo_context)?;
        let prompt = compile_prompt(&task, &repo_context, &attempts, &previous_patch_hashes);
        let prompt_path = attempt_dir.join("prompt.txt");
        write_text_checked(&prompt_path, &prompt)?;
        let prompt_sha = sha256_file(&prompt_path)?;

        let decision_input = json!({
            "prompt_sha256": prompt_sha,
            "attempt": attempt,
            "previous_patch_hashes": previous_patch_hashes,
            "gpu_budget": {
                "gpu_index": args.gpu_index,
                "min_free_mb": args.gpu_min_free_mb,
                "max_used_mb": args.gpu_max_used_mb
            },
            "task_source_of_truth": file_sot(&task_manifest_path)
        });
        let decision_input_path = attempt_dir.join("model-decision-input.json");
        write_json_checked(&decision_input_path, &decision_input)?;
        blackbox(
            &context_graph_bin,
            &[
                "blackbox",
                "record",
                "model-decision",
                "--ledger",
                path_str(&ledger),
                "--run-id",
                &run_id,
                "--tool-name",
                "rust_generate_patch",
                "--capability",
                "contextgraph.rust.reality.swe.generate_patch",
                "--input",
                path_str(&decision_input_path),
                "--agent",
                "contextgraph_rust_reality_loop",
                "--model",
                &args.model,
            ],
            &task_root,
        )?;

        let model_response =
            ollama_chat_json(&client, &args, &prompt, &attempt_dir, attempt).await?;
        let response_text = model_response["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let response_path = attempt_dir.join("model-response.txt");
        write_text_checked(&response_path, &response_text)?;
        let (mut patch, mut parse_report) = parse_patch_report(&response_text);
        if patch.is_none() {
            if let Some(edits) = parse_report
                .get("structured_edits")
                .and_then(Value::as_array)
            {
                let (compiled_patch, compilation_report) =
                    compile_structured_edits(&workspace, &attempt_dir, edits, args.git_timeout)?;
                parse_report["structured_edit_compilation"] = compilation_report.clone();
                if let Some(compiled_patch) = compiled_patch {
                    patch = Some(compiled_patch);
                    parse_report["status"] = json!("passed");
                    parse_report["error_code"] = Value::Null;
                    parse_report["field_path"] = json!("model_response.edits");
                    parse_report["message"] = json!(
                        "model response supplied structured edits that Rust compiled into a git diff"
                    );
                } else {
                    parse_report["status"] = json!("failed");
                    parse_report["error_code"] = compilation_report["error_code"].clone();
                    parse_report["field_path"] = compilation_report["field_path"].clone();
                    parse_report["message"] = compilation_report["message"].clone();
                    parse_report["remediation"] = compilation_report["remediation"].clone();
                }
                reset_workspace(&workspace, &task.base_commit, args.git_timeout)?;
            }
        }
        let parse_report_path = attempt_dir.join("model-parse-report.json");
        write_json_checked(&parse_report_path, &parse_report)?;
        let patch_sha = patch.as_ref().map(|p| sha256_text(p));
        let repeated_patch = patch_sha
            .as_ref()
            .map(|sha| previous_patch_hashes.iter().any(|item| item == sha))
            .unwrap_or(false);
        if let Some(patch) = &patch {
            write_text_checked(&attempt_dir.join("candidate.patch"), patch)?;
        }
        if let Some(sha) = &patch_sha {
            if !previous_patch_hashes.iter().any(|item| item == sha) {
                previous_patch_hashes.push(sha.clone());
            }
        }

        let validation = validate_attempt(
            &task,
            &workspace,
            &attempt_dir,
            patch.as_deref(),
            &parse_report,
            &parse_report_path,
            repeated_patch,
            args.git_timeout,
        )?;
        let validation_path = attempt_dir.join("validation.json");
        write_json_checked(&validation_path, &validation)?;
        let tool_input_path = attempt_dir.join("tool-call-input.json");
        write_json_checked(
            &tool_input_path,
            &json!({
                "attempt": attempt,
                "expected_delta": {
                    "candidate_patch": "applies_to_base",
                    "official_swe_report": "resolved"
                },
                "candidate_patch_sha256": patch_sha,
                "model_parse_report": parse_report,
                "validation": validation,
                "source_of_truth": file_sot(&validation_path)
            }),
        )?;
        let tool_status = if validation["status"] == "passed" {
            "ok"
        } else if validation["status"] == "blocked" {
            "blocked"
        } else {
            "failed"
        };
        let tool_call = blackbox(
            &context_graph_bin,
            &[
                "blackbox",
                "record",
                "tool-call",
                "--ledger",
                path_str(&ledger),
                "--run-id",
                &run_id,
                "--tool",
                "rust_apply_candidate_patch",
                "--input",
                path_str(&tool_input_path),
                "--agent",
                "contextgraph_rust_reality_loop",
                "--model",
                &args.model,
                "--capability",
                "contextgraph.rust.reality.swe.apply_patch",
                "--status",
                tool_status,
            ],
            &task_root,
        )?;
        let tool_call_id = tool_call
            .get("tool_call_id")
            .and_then(Value::as_str)
            .unwrap_or("tool_call_id_missing")
            .to_string();
        let verifier = verify_attempt(
            &context_graph_bin,
            &ledger,
            &run_id,
            &tool_call_id,
            &task_root,
            &attempt_dir,
            &validation_path,
            &args.model,
        )?;

        let mut official_evidence_path = None;
        let mut official_reality = None;
        let mut resolved = None;
        if args.official_eval_each_attempt && patch.is_some() && validation["status"] == "passed" {
            let official = run_official_eval(
                &args,
                &run_root,
                &attempt_dir,
                &task,
                patch.as_ref().unwrap(),
            )?;
            let evidence =
                build_official_evidence(&official, &task, attempt, patch.as_ref().unwrap())?;
            let evidence_path = attempt_dir.join(if evidence["resolved"] == true {
                "official-success-evidence.json"
            } else {
                "official-failure-evidence.json"
            });
            write_json_checked(&evidence_path, &evidence)?;
            official_evidence_path = Some(path_str(&evidence_path).to_string());
            official_reality = Some(compact_official_evidence(&evidence));
            resolved = evidence["resolved"].as_bool();
            if resolved == Some(true) {
                solved = true;
            }
        }

        let score = blackbox(
            &context_graph_bin,
            &[
                "blackbox",
                "score",
                "run",
                "--ledger",
                path_str(&ledger),
                "--run-id",
                &run_id,
                "--model",
                &args.model,
            ],
            &task_root,
        )?;
        let reward_spec =
            resolve_path(Path::new("gemmafrontend/reward_spec.verified_outcome.json"))?;
        let reward = if reward_spec.exists() {
            Some(blackbox(
                &context_graph_bin,
                &[
                    "blackbox",
                    "reward",
                    "compile",
                    "--ledger",
                    path_str(&ledger),
                    "--run-id",
                    &run_id,
                    "--input",
                    path_str(&reward_spec),
                    "--model",
                    &args.model,
                ],
                &task_root,
            )?)
        } else {
            None
        };

        let attempt_record = AttemptRecord {
            attempt,
            status: validation["status"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            error_code: validation["error_code"].as_str().map(ToOwned::to_owned),
            message: validation["message"].as_str().map(ToOwned::to_owned),
            remediation: validation["remediation"].as_str().map(ToOwned::to_owned),
            prompt_sha256: prompt_sha,
            response_sha256: sha256_file(&response_path)?,
            patch_sha256: patch_sha,
            repeated_patch,
            validation_path: path_str(&validation_path).to_string(),
            official_evidence_path,
            resolved,
            failure_evidence: compact_failure_evidence(&validation),
            official_reality,
            patch_preview: patch.as_ref().map(|text| truncate_chars(text, 1600)),
        };
        write_json_checked(
            &attempt_dir.join("attempt-summary.json"),
            &json!({
                "attempt": attempt_record,
                "model_response": model_response,
                "blackbox_verifier": verifier,
                "score": score,
                "reward": reward,
                "ledger_counts": ledger_counts(&ledger)?,
            }),
        )?;
        append_event(
            &reality_events,
            json!({
                "event_type": "attempt_completed",
                "status": if solved { "solved" } else { "candidate_failed" },
                "attempt": attempt,
                "attempt_summary": file_sot(&attempt_dir.join("attempt-summary.json")),
                "ledger_counts": ledger_counts(&ledger)?,
                "gpu_state_after_attempt": read_gpu_vram_state(args.gpu_index)?,
            }),
        )?;
        attempts.push(attempt_record);
        write_summary(
            &run_root,
            &task,
            &args,
            &ledger,
            &reality_events,
            &contract_path,
            &attempts,
            solved,
            false,
        )?;
        if solved {
            append_event(
                &reality_events,
                json!({"event_type": "rust_reality_loop_completed", "status": "solved"}),
            )?;
            return write_summary(
                &run_root,
                &task,
                &args,
                &ledger,
                &reality_events,
                &contract_path,
                &attempts,
                solved,
                true,
            );
        }
        if !args.run_until_resolved && attempt >= args.max_attempts {
            break;
        }
        attempt += 1;
        if args.sleep_between_attempts_ms > 0 {
            std::thread::sleep(Duration::from_millis(args.sleep_between_attempts_ms));
        }
    }

    append_event(
        &reality_events,
        json!({"event_type": "rust_reality_loop_completed", "status": if solved { "solved" } else { "unsolved" }}),
    )?;
    let summary = write_summary(
        &run_root,
        &task,
        &args,
        &ledger,
        &reality_events,
        &contract_path,
        &attempts,
        solved,
        true,
    )?;
    Ok(summary)
}

fn validate_task_manifest_contract(task: &TaskManifest, manifest_path: &Path) -> Result<()> {
    let dataset = task.dataset.as_deref().unwrap_or("");
    let source_of_truth = task.source_of_truth.as_deref().unwrap_or("");
    if dataset != DATASET_NAME {
        return Err(RealityError::new(
            "RUST_REALITY_TASK_DATASET_MISMATCH",
            "task manifest dataset does not match the official SWE-bench Lite measurement contract",
            "task_manifest.dataset",
            "regenerate the task manifest from the official SWE-bench Lite split before running",
            json!({"expected": DATASET_NAME, "actual": dataset}),
            Some(file_sot(manifest_path)),
        ));
    }
    if task.instance_id.trim().is_empty() || !task.instance_id.contains("__") {
        return Err(RealityError::new(
            "RUST_REALITY_TASK_INSTANCE_ID_INVALID",
            "task manifest instance_id is not a valid SWE-bench instance id",
            "task_manifest.instance_id",
            "use the exact instance_id from the official dataset row",
            json!({"instance_id": task.instance_id}),
            Some(file_sot(manifest_path)),
        ));
    }
    if task.repo.split('/').count() != 2 || task.repo.contains("..") {
        return Err(RealityError::new(
            "RUST_REALITY_TASK_REPO_INVALID",
            "task manifest repo must be a GitHub owner/repo identifier",
            "task_manifest.repo",
            "use the repo value from the official dataset row",
            json!({"repo": task.repo}),
            Some(file_sot(manifest_path)),
        ));
    }
    if !is_40_hex(&task.base_commit) {
        return Err(RealityError::new(
            "RUST_REALITY_TASK_BASE_COMMIT_INVALID",
            "task manifest base_commit must be a full 40-character git SHA",
            "task_manifest.base_commit",
            "regenerate the manifest with the official base_commit",
            json!({"base_commit": task.base_commit}),
            Some(file_sot(manifest_path)),
        ));
    }
    if task.problem_statement.trim().is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_TASK_PROBLEM_STATEMENT_MISSING",
            "task manifest problem_statement is empty",
            "task_manifest.problem_statement",
            "regenerate the manifest from the official dataset row",
            json!({}),
            Some(file_sot(manifest_path)),
        ));
    }
    if task.fail_to_pass.is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_TASK_FAIL_TO_PASS_MISSING",
            "task manifest has no FAIL_TO_PASS tests",
            "task_manifest.fail_to_pass",
            "use a SWE-bench task with explicit FAIL_TO_PASS tests so the oracle boundary is concrete",
            json!({}),
            Some(file_sot(manifest_path)),
        ));
    }
    if task.pass_to_pass_count.unwrap_or(0) == 0 {
        return Err(RealityError::new(
            "RUST_REALITY_TASK_PASS_TO_PASS_MISSING",
            "task manifest has no PASS_TO_PASS count",
            "task_manifest.pass_to_pass_count",
            "regenerate the manifest with PASS_TO_PASS metadata so regression coverage is visible",
            json!({"pass_to_pass_count": task.pass_to_pass_count}),
            Some(file_sot(manifest_path)),
        ));
    }
    if source_of_truth.is_empty()
        || !source_of_truth.contains(DATASET_NAME)
        || !source_of_truth.contains(&task.instance_id)
    {
        return Err(RealityError::new(
            "RUST_REALITY_TASK_SOURCE_OF_TRUTH_INCOMPLETE",
            "task manifest source_of_truth does not bind dataset and instance id",
            "task_manifest.source_of_truth",
            "record the official dataset split and instance id in source_of_truth",
            json!({
                "source_of_truth": source_of_truth,
                "required_dataset": DATASET_NAME,
                "required_instance_id": task.instance_id
            }),
            Some(file_sot(manifest_path)),
        ));
    }
    Ok(())
}

fn build_measurement_contract(
    args: &Args,
    task: &TaskManifest,
    manifest_path: &Path,
) -> Result<Value> {
    Ok(json!({
        "schema": "contextgraph_reality_measurement_contract_v1",
        "contract_id": format!(
            "contract_{}_{}",
            safe_id(&task.instance_id),
            short_hash(&format!("{}:{}:{}", DATASET_NAME, task.instance_id, task.base_commit))
        ),
        "claim_scope": {
            "primary_claim": "official_swe_instance_resolution_only",
            "success_predicate": "official_report.resolved_ids contains task.instance_id",
            "explicit_non_claims": [
                "not a proof of all possible repository behavior",
                "not a proof of human product intent beyond the SWE-bench oracle",
                "not a success claim when official Docker evaluation is unavailable"
            ]
        },
        "oracle": {
            "dataset": DATASET_NAME,
            "split": "test",
            "instance_id": task.instance_id,
            "fail_to_pass": task.fail_to_pass,
            "pass_to_pass_count": task.pass_to_pass_count,
            "official_test_patch_sha256": task.official_test_patch_sha256,
            "oracle_patch_sha256": task.oracle_patch_sha256,
            "source_of_truth": task.source_of_truth
        },
        "immutable_boundaries": {
            "task_manifest": file_sot(manifest_path),
            "base_commit": task.base_commit,
            "repo": task.repo,
            "official_evaluator": "python -m swebench.harness.run_evaluation",
            "official_docker_host": args.official_docker_host,
            "official_docker_config": path_str(&args.official_docker_config),
            "reward_spec": file_sot(Path::new("gemmafrontend/reward_spec.verified_outcome.json"))
        },
        "mandatory_readbacks": [
            "task_manifest_readback",
            "measurement_contract_readback",
            "git_head_equals_base_commit",
            "git_status_clean_before_attempt",
            "candidate_patch_applies_to_base",
            "candidate_patch_does_not_modify_tests_or_harness",
            "official_docker_sdk_preflight",
            "official_swe_report_with_resolved_ids",
            "blackbox_sqlite_ledger_counts",
            "reality_event_log_append_readback"
        ],
        "model_editable_scope": [
            "candidate.patch",
            "structured edit plan compiled by Rust into candidate.patch",
            "notes"
        ],
        "model_forbidden_scope": [
            "task manifest",
            "official evaluator",
            "Docker harness",
            "BlackBoxFix ledger schema/hash chain",
            "reward spec",
            "verifier source",
            "benchmark tests/fixtures/harness files"
        ],
        "failure_policy": {
            "unmeasured_success": "blocked",
            "missing_source_of_truth": "error",
            "incomplete_oracle": "error",
            "official_evaluator_unavailable": "error",
            "test_or_harness_mutation": "failed"
        },
        "source_of_truth": file_sot(manifest_path)
    }))
}

fn is_40_hex(value: &str) -> bool {
    value.len() == 40 && value.as_bytes().iter().all(|byte| byte.is_ascii_hexdigit())
}

fn compact_failure_evidence(validation: &Value) -> Value {
    json!({
        "status": validation.get("status"),
        "error_code": validation.get("error_code"),
        "field_path": validation.get("field_path"),
        "message": validation.get("message"),
        "remediation": validation.get("remediation"),
        "git_apply_stderr": validation.pointer("/details/git_apply/stderr"),
        "git_apply_stdout": validation.pointer("/details/git_apply/stdout"),
        "source_of_truth": validation.get("source_of_truth"),
    })
}

fn compact_official_evidence(evidence: &Value) -> Value {
    let report = evidence.get("official_report").unwrap_or(&Value::Null);
    json!({
        "claim_scope": evidence.get("claim_scope"),
        "success_predicate": evidence.get("success_predicate"),
        "instance_id": evidence.get("instance_id"),
        "resolved": evidence.get("resolved"),
        "completed": evidence.get("completed"),
        "incomplete": evidence.get("incomplete"),
        "errored": evidence.get("errored"),
        "empty_patch": evidence.get("empty_patch"),
        "resolved_ids": report.get("resolved_ids"),
        "unresolved_ids": report.get("unresolved_ids"),
        "completed_ids": report.get("completed_ids"),
        "previous_patch_sha256": evidence.get("previous_patch_sha256"),
        "source_of_truth": evidence.get("source_of_truth"),
    })
}

fn write_failure_artifact(args: &Args, err: &RealityError) -> std::result::Result<PathBuf, String> {
    let runtime_root = if args.runtime_root.is_absolute() {
        args.runtime_root.clone()
    } else {
        std::env::current_dir()
            .map_err(|cwd_err| cwd_err.to_string())?
            .join(&args.runtime_root)
    };
    let failure_root = runtime_root.join("failures");
    fs::create_dir_all(&failure_root).map_err(|io_err| io_err.to_string())?;
    let path = failure_root.join(format!(
        "failure-{}-{}.json",
        unix_secs_for_logging(),
        safe_id(&err.error_code)
    ));
    let payload = json!({
        "record_kind": "rust_reality_loop_failure",
        "created_at_unix": unix_secs_for_logging(),
        "args": {
            "model": args.model,
            "task_manifest": path_str(&args.task_manifest),
            "runtime_root": path_str(&runtime_root),
            "max_attempts": args.max_attempts,
            "run_until_resolved": args.run_until_resolved,
        "official_eval_each_attempt": args.official_eval_each_attempt,
        "official_docker_host": args.official_docker_host,
        "official_docker_config": path_str(&args.official_docker_config),
        "gpu_index": args.gpu_index,
            "gpu_min_free_mb": args.gpu_min_free_mb,
            "gpu_max_used_mb": args.gpu_max_used_mb,
            "ollama_keep_alive": args.ollama_keep_alive
        },
        "error": err,
        "source_of_truth": file_sot(&path)
    });
    let text = serde_json::to_string_pretty(&payload).map_err(|json_err| json_err.to_string())?;
    fs::write(&path, text).map_err(|io_err| io_err.to_string())?;
    let readback = fs::read_to_string(&path).map_err(|io_err| io_err.to_string())?;
    let actual: Value = serde_json::from_str(&readback).map_err(|json_err| json_err.to_string())?;
    if actual != payload {
        return Err(format!(
            "failure artifact readback mismatch at {}",
            path_str(&path)
        ));
    }
    Ok(path)
}

fn run_cmd(command: &[String], cwd: &Path, timeout_secs: u64) -> Result<ProcessResult> {
    run_cmd_env(command, cwd, timeout_secs, &[])
}

fn run_cmd_env(
    command: &[String],
    cwd: &Path,
    timeout_secs: u64,
    envs: &[(String, String)],
) -> Result<ProcessResult> {
    let started = Instant::now();
    let mut process = Command::new(&command[0]);
    process
        .args(&command[1..])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in envs {
        process.env(key, value);
    }
    let mut child = process.spawn().map_err(|err| {
        RealityError::new(
            "RUST_REALITY_PROCESS_SPAWN_FAILED",
            "failed to spawn child process",
            "process.spawn",
            "inspect command path and permissions",
            json!({ "command": command, "error": err.to_string() }),
            None,
        )
    })?;
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        if let Some(_status) = child.try_wait().map_err(|err| {
            RealityError::new(
                "RUST_REALITY_PROCESS_WAIT_FAILED",
                "failed while waiting for child process",
                "process.wait",
                "inspect process state",
                json!({ "command": command, "error": err.to_string() }),
                None,
            )
        })? {
            let output = child.wait_with_output().map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_PROCESS_OUTPUT_FAILED",
                    "failed to collect child process output",
                    "process.output",
                    "inspect process state",
                    json!({ "command": command, "error": err.to_string() }),
                    None,
                )
            })?;
            return Ok(ProcessResult {
                command: command.to_vec(),
                cwd: path_str(cwd).to_string(),
                exit_code: output.status.code().unwrap_or(128),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                elapsed_ms: started.elapsed().as_millis(),
            });
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            return Ok(ProcessResult {
                command: command.to_vec(),
                cwd: path_str(cwd).to_string(),
                exit_code: 124,
                stdout: String::new(),
                stderr: format!("TIMEOUT after {timeout_secs}s"),
                elapsed_ms: started.elapsed().as_millis(),
            });
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn blackbox(bin: &Path, args: &[&str], cwd: &Path) -> Result<Value> {
    let mut command = vec![path_str(bin).to_string()];
    command.extend(args.iter().map(|item| item.to_string()));
    let result = run_cmd(&command, cwd, 240)?;
    if result.exit_code != 0 {
        return Err(RealityError::new(
            "RUST_REALITY_BLACKBOX_COMMAND_FAILED",
            "BlackBoxFix command failed",
            "blackbox.command",
            "inspect stdout/stderr and ledger path",
            json!({ "process": result }),
            None,
        ));
    }
    serde_json::from_str(&result.stdout).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_BLACKBOX_JSON_INVALID",
            "BlackBoxFix command did not return JSON",
            "blackbox.stdout",
            "inspect BlackBoxFix command output",
            json!({ "stdout": result.stdout, "stderr": result.stderr, "error": err.to_string() }),
            None,
        )
    })
}

async fn ollama_chat_json(
    client: &reqwest::Client,
    args: &Args,
    prompt: &str,
    attempt_dir: &Path,
    attempt: u64,
) -> Result<Value> {
    let gpu_before = read_gpu_vram_state(args.gpu_index)?;
    assert_gpu_budget(
        &gpu_before,
        args.gpu_min_free_mb,
        args.gpu_max_used_mb,
        "before_ollama_chat",
        &args.model,
    )?;
    let body = json!({
        "model": args.model,
        "messages": [
            {"role": "system", "content": "You are a software repair model. Return one raw JSON object only, with no markdown fences and no surrounding prose. Prefer structured edits over raw diffs: {\"edits\":[{\"path\":\"relative/source/file.py\",\"find\":\"exact old text\",\"replace\":\"exact new text\"}],\"notes\":\"brief\"}. ContextGraph will compile structured edits into a git diff and verify them against the real repository. You may return {\"patch\":\"<complete unified git diff>\",\"notes\":\"brief\"} only if you can produce a syntactically valid diff. Do not modify tests, fixtures, or benchmark harness files."},
            {"role": "user", "content": prompt}
        ],
        "stream": false,
        "think": false,
        "format": {
            "type": "object",
            "additionalProperties": false,
            "required": ["notes"],
            "properties": {
                "patch": {"type": "string"},
                "edits": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["path", "find", "replace"],
                        "properties": {
                            "path": {"type": "string"},
                            "find": {"type": "string"},
                            "replace": {"type": "string"}
                        }
                    }
                },
                "notes": {"type": "string"}
            }
        },
        "keep_alive": args.ollama_keep_alive,
        "options": {"temperature": 0, "num_predict": 4096}
    });
    let request_path = attempt_dir.join("ollama-request.json");
    write_json_checked(&request_path, &body)?;
    let url = format!("{}/api/chat", args.ollama_url.trim_end_matches('/'));
    let response = client
        .post(&url)
        .json(&body)
        .timeout(Duration::from_secs(args.model_timeout))
        .send()
        .await
        .map_err(|err| {
            RealityError::new(
                "RUST_REALITY_OLLAMA_REQUEST_FAILED",
                "Ollama request failed",
                "ollama.request",
                "inspect Ollama server/model state and GPU budget",
                json!({ "url": url, "error": err.to_string(), "request": file_sot(&request_path) }),
                Some(url.clone()),
            )
        })?;
    let status = response.status();
    let value: Value = response.json().await.map_err(|err| {
        RealityError::new(
            "RUST_REALITY_OLLAMA_JSON_INVALID",
            "Ollama response was not valid JSON",
            "ollama.response",
            "inspect Ollama server output",
            json!({ "error": err.to_string() }),
            Some(url.clone()),
        )
    })?;
    if !status.is_success() || value.get("error").is_some() {
        return Err(RealityError::new(
            "RUST_REALITY_OLLAMA_ERROR",
            "Ollama returned an error",
            "ollama.response",
            "inspect the model tag, prompt size, and Ollama server state",
            json!({ "status": status.as_u16(), "response": value }),
            Some(url),
        ));
    }
    let gpu_after = read_gpu_vram_state(args.gpu_index)?;
    assert_gpu_budget(
        &gpu_after,
        args.gpu_min_free_mb,
        args.gpu_max_used_mb,
        "after_ollama_chat",
        &args.model,
    )?;
    let mut enriched = value;
    enriched["contextgraph_runtime"] = json!({
        "attempt": attempt,
        "gpu_before": gpu_before,
        "gpu_after": gpu_after,
        "request_source_of_truth": file_sot(&request_path)
    });
    write_json_checked(&attempt_dir.join("ollama-response.json"), &enriched)?;
    Ok(enriched)
}

fn parse_patch_report(response_text: &str) -> (Option<String>, Value) {
    let candidates = json_candidates(response_text);
    let mut parse_errors = Vec::new();
    for candidate in candidates {
        match serde_json::from_str::<Value>(&candidate) {
            Ok(value) => {
                if let Some(edits) = value.get("edits").and_then(Value::as_array) {
                    if !edits.is_empty() {
                        return (
                            None,
                            json!({
                                "status": "pending_structured_edit_compilation",
                                "error_code": null,
                                "field_path": "model_response.edits",
                                "message": "model response supplied structured edits for deterministic Rust compilation",
                                "structured_edits": edits,
                                "notes": value.get("notes").cloned().unwrap_or(Value::Null)
                            }),
                        );
                    }
                }
                if let Some(patch) = value.get("patch").and_then(Value::as_str) {
                    if !patch.trim().is_empty() {
                        return (
                            Some(patch.to_string()),
                            json!({
                                "status": "passed",
                                "error_code": null,
                                "field_path": "model_response.patch",
                                "message": "model response satisfied the patch JSON contract",
                                "notes": value.get("notes").cloned().unwrap_or(Value::Null)
                            }),
                        );
                    }
                }
                return (
                    None,
                    json!({
                        "status": "failed",
                        "error_code": "RUST_REALITY_MODEL_ACTION_MISSING",
                        "field_path": "model_response.patch_or_edits",
                        "message": "model returned JSON, but patch and edits were missing or empty",
                        "details": {"parsed_keys": value.as_object().map(|obj| obj.keys().cloned().collect::<Vec<_>>()).unwrap_or_default()}
                    }),
                );
            }
            Err(err) => parse_errors.push(err.to_string()),
        }
    }
    (
        None,
        json!({
            "status": "failed",
            "error_code": "RUST_REALITY_MODEL_RESPONSE_JSON_INVALID",
            "field_path": "model_response",
            "message": "model response did not satisfy the required raw JSON object contract",
            "details": {"parse_errors": parse_errors}
        }),
    )
}

fn compile_structured_edits(
    repo_dir: &Path,
    attempt_dir: &Path,
    edits: &[Value],
    timeout: u64,
) -> Result<(Option<String>, Value)> {
    let edits_path = attempt_dir.join("structured-edits.json");
    write_json_checked(&edits_path, &edits)?;
    let mut applied = Vec::new();

    for (index, edit) in edits.iter().enumerate() {
        let field_prefix = format!("model_response.edits[{index}]");
        let path = match edit.get("path").and_then(Value::as_str) {
            Some(value) if !value.trim().is_empty() => value.trim(),
            _ => {
                return Ok(structured_edit_failure(
                    "RUST_REALITY_STRUCTURED_EDIT_PATH_MISSING",
                    &format!("{field_prefix}.path"),
                    "structured edit path is missing",
                    "return a relative production-source path that exists in the checked-out repository",
                    json!({"edit": edit}),
                    &edits_path,
                ));
            }
        };
        if path.starts_with('/')
            || path.contains('\\')
            || Path::new(path)
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Ok(structured_edit_failure(
                "RUST_REALITY_STRUCTURED_EDIT_PATH_INVALID",
                &format!("{field_prefix}.path"),
                "structured edit path must be a relative repository path",
                "return a normal repo-relative path without absolute roots, parent traversal, or backslashes",
                json!({"path": path}),
                &edits_path,
            ));
        }
        if is_forbidden_benchmark_mutation(path) {
            return Ok(structured_edit_failure(
                "RUST_REALITY_STRUCTURED_EDIT_FORBIDDEN_PATH",
                &format!("{field_prefix}.path"),
                "structured edit targets tests, fixtures, or benchmark harness files",
                "repair production source only; do not change tests, fixtures, or evaluation harnesses",
                json!({"path": path}),
                &edits_path,
            ));
        }
        let find = match edit.get("find").and_then(Value::as_str) {
            Some(value) if !value.is_empty() => value,
            _ => {
                return Ok(structured_edit_failure(
                    "RUST_REALITY_STRUCTURED_EDIT_FIND_MISSING",
                    &format!("{field_prefix}.find"),
                    "structured edit find text is missing",
                    "return exact old source text copied from the repository context",
                    json!({"path": path}),
                    &edits_path,
                ));
            }
        };
        let replace = match edit.get("replace").and_then(Value::as_str) {
            Some(value) => value,
            _ => {
                return Ok(structured_edit_failure(
                    "RUST_REALITY_STRUCTURED_EDIT_REPLACE_MISSING",
                    &format!("{field_prefix}.replace"),
                    "structured edit replace text is missing",
                    "return the exact replacement source text",
                    json!({"path": path}),
                    &edits_path,
                ));
            }
        };
        let target = repo_dir.join(path);
        if !target.exists() || !target.is_file() {
            return Ok(structured_edit_failure(
                "RUST_REALITY_STRUCTURED_EDIT_TARGET_MISSING",
                &format!("{field_prefix}.path"),
                "structured edit target file does not exist",
                "choose a real production-source file from repository context",
                json!({"path": path, "source_of_truth": file_sot(&target)}),
                &edits_path,
            ));
        }
        let before = match fs::read_to_string(&target) {
            Ok(value) => value,
            Err(err) => {
                return Ok(structured_edit_failure(
                    "RUST_REALITY_STRUCTURED_EDIT_TARGET_READ_FAILED",
                    &format!("{field_prefix}.path"),
                    "structured edit target could not be read as UTF-8 text",
                    "choose a text source file and avoid binary/generated artifacts",
                    json!({"path": path, "error": err.to_string()}),
                    &edits_path,
                ));
            }
        };
        let matches = before.matches(find).count();
        if matches != 1 {
            return Ok(structured_edit_failure(
                if matches == 0 {
                    "RUST_REALITY_STRUCTURED_EDIT_FIND_NOT_FOUND"
                } else {
                    "RUST_REALITY_STRUCTURED_EDIT_FIND_AMBIGUOUS"
                },
                &format!("{field_prefix}.find"),
                "structured edit find text must match exactly one location",
                "use the latest failure evidence and repository context to provide a unique exact source span",
                json!({
                    "path": path,
                    "matches": matches,
                    "find_sha256": sha256_text(find),
                    "source_of_truth": file_sot(&target)
                }),
                &edits_path,
            ));
        }
        let after = before.replacen(find, replace, 1);
        write_text_checked(&target, &after)?;
        applied.push(json!({
            "index": index,
            "path": path,
            "before_sha256": sha256_text(&before),
            "after_sha256": sha256_text(&after),
            "find_sha256": sha256_text(find),
            "replace_sha256": sha256_text(replace),
            "source_of_truth": file_sot(&target)
        }));
    }

    let diff = run_required(
        &["git", "diff", "--no-ext-diff", "--binary"],
        repo_dir,
        timeout,
        "RUST_REALITY_STRUCTURED_EDIT_GIT_DIFF_FAILED",
        "structured_edits.git_diff",
    )?;
    if diff.stdout.trim().is_empty() {
        return Ok(structured_edit_failure(
            "RUST_REALITY_STRUCTURED_EDIT_EMPTY_DIFF",
            "model_response.edits",
            "structured edits produced no repository diff",
            "change production source so the official oracle has a candidate patch to evaluate",
            json!({"applied_edits": applied}),
            &edits_path,
        ));
    }
    let patch_path = attempt_dir.join("candidate.patch");
    write_text_checked(&patch_path, &diff.stdout)?;
    Ok((
        Some(diff.stdout),
        json!({
            "status": "passed",
            "error_code": null,
            "field_path": "model_response.edits",
            "message": "Rust compiled structured edits into a git diff from the real checkout",
            "applied_edits": applied,
            "candidate_patch_sha256": sha256_file(&patch_path)?,
            "source_of_truth": {
                "structured_edits": file_sot(&edits_path),
                "candidate_patch": file_sot(&patch_path),
                "workspace_diff": format!("git:{}#diff", path_str(repo_dir))
            }
        }),
    ))
}

fn structured_edit_failure(
    error_code: &str,
    field_path: &str,
    message: &str,
    remediation: &str,
    details: Value,
    edits_path: &Path,
) -> (Option<String>, Value) {
    (
        None,
        json!({
            "status": "failed",
            "error_code": error_code,
            "field_path": field_path,
            "message": message,
            "remediation": remediation,
            "details": details,
            "source_of_truth": file_sot(edits_path)
        }),
    )
}

fn json_candidates(text: &str) -> Vec<String> {
    let mut out = vec![text.trim().to_string()];
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
        if start < end {
            out.push(text[start..=end].to_string());
        }
    }
    out
}

fn ensure_workspace(task: &TaskManifest, repo_dir: &Path, timeout: u64) -> Result<()> {
    if !repo_dir.exists() {
        create_dir_checked(repo_dir.parent().ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_WORKSPACE_PARENT_MISSING",
                "workspace parent path was missing",
                "workspace.parent",
                "choose a valid runtime root",
                json!({ "repo_dir": path_str(repo_dir) }),
                None,
            )
        })?)?;
        let repo_url = format!("https://github.com/{}.git", task.repo);
        run_required(
            &["git", "clone", &repo_url, path_str(repo_dir)],
            repo_dir.parent().unwrap(),
            timeout,
            "RUST_REALITY_GIT_CLONE_FAILED",
            "workspace.clone",
        )?;
    }
    run_required(
        &["git", "fetch", "--all", "--tags"],
        repo_dir,
        timeout,
        "RUST_REALITY_GIT_FETCH_FAILED",
        "workspace.fetch",
    )?;
    reset_workspace(repo_dir, &task.base_commit, timeout)
}

fn reset_workspace(repo_dir: &Path, base_commit: &str, timeout: u64) -> Result<()> {
    run_required(
        &["git", "reset", "--hard", base_commit],
        repo_dir,
        timeout,
        "RUST_REALITY_GIT_RESET_FAILED",
        "workspace.reset",
    )?;
    run_required(
        &["git", "clean", "-fdx"],
        repo_dir,
        timeout,
        "RUST_REALITY_GIT_CLEAN_FAILED",
        "workspace.clean",
    )?;
    let head = run_required(
        &["git", "rev-parse", "HEAD"],
        repo_dir,
        timeout,
        "RUST_REALITY_GIT_HEAD_READ_FAILED",
        "workspace.head",
    )?;
    let actual_head = head.stdout.trim();
    if actual_head != base_commit {
        return Err(RealityError::new(
            "RUST_REALITY_BASE_COMMIT_READBACK_MISMATCH",
            "workspace HEAD did not match the task manifest base commit after reset",
            "workspace.head",
            "inspect the task manifest base_commit and repository checkout before continuing",
            json!({
                "expected_base_commit": base_commit,
                "actual_head": actual_head,
                "repo_dir": path_str(repo_dir)
            }),
            Some(format!("git:{}#HEAD", path_str(repo_dir))),
        ));
    }
    let status = run_required(
        &["git", "status", "--porcelain"],
        repo_dir,
        timeout,
        "RUST_REALITY_GIT_STATUS_READ_FAILED",
        "workspace.status",
    )?;
    if !status.stdout.trim().is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_WORKSPACE_NOT_CLEAN_AFTER_RESET",
            "workspace was not clean after reset and clean",
            "workspace.status",
            "inspect untracked/modified files before continuing",
            json!({
                "git_status": status.stdout,
                "repo_dir": path_str(repo_dir)
            }),
            Some(format!("git:{}#status", path_str(repo_dir))),
        ));
    }
    Ok(())
}

fn validate_attempt(
    task: &TaskManifest,
    repo_dir: &Path,
    attempt_dir: &Path,
    patch: Option<&str>,
    parse_report: &Value,
    parse_report_path: &Path,
    repeated_patch: bool,
    timeout: u64,
) -> Result<Value> {
    if patch.is_none() {
        return Ok(json!({
            "status": "failed",
            "error_code": parse_report["error_code"].as_str().unwrap_or("RUST_REALITY_MODEL_ACTION_MISSING"),
            "field_path": parse_report["field_path"].as_str().unwrap_or("model_response.patch_or_edits"),
            "message": parse_report["message"].as_str().unwrap_or("model did not produce a usable patch or structured edit plan"),
            "remediation": parse_report["remediation"].as_str().unwrap_or("reinject parse failure and require the exact JSON response contract with either structured edits or a valid patch"),
            "details": {"parse_report": parse_report, "model_response": file_sot(&attempt_dir.join("model-response.txt"))},
            "source_of_truth": file_sot(parse_report_path)
        }));
    }
    let patch_path = attempt_dir.join("candidate.patch");
    if repeated_patch {
        return Ok(json!({
            "status": "failed",
            "error_code": "RUST_REALITY_PATCH_REPEATED",
            "field_path": "candidate.patch",
            "message": "model repeated a previous candidate patch exactly",
            "remediation": "produce a materially different patch; use the validation stderr and prior patch preview instead of repeating",
            "details": {"patch_sha256": sha256_file(&patch_path)?},
            "source_of_truth": file_sot(&patch_path)
        }));
    }
    let apply = run_cmd(
        &vec![
            "git".to_string(),
            "apply".to_string(),
            "--whitespace=nowarn".to_string(),
            path_str(&patch_path).to_string(),
        ],
        repo_dir,
        timeout,
    )?;
    if apply.exit_code != 0 {
        return Ok(json!({
            "status": "failed",
            "error_code": "RUST_REALITY_PATCH_APPLY_FAILED",
            "field_path": "candidate.patch",
            "message": "candidate patch did not apply to the base repository",
            "remediation": "repair the patch against the exact base commit",
            "details": {"git_apply": apply, "patch_sha256": sha256_file(&patch_path)?},
            "source_of_truth": file_sot(&patch_path)
        }));
    }
    let diff = run_required(
        &["git", "diff", "--no-ext-diff", "--binary"],
        repo_dir,
        timeout,
        "RUST_REALITY_GIT_DIFF_FAILED",
        "workspace.diff",
    )?;
    let actual_patch = attempt_dir.join("actual-applied.patch");
    write_text_checked(&actual_patch, &diff.stdout)?;
    let changed = run_required(
        &["git", "diff", "--name-only"],
        repo_dir,
        timeout,
        "RUST_REALITY_GIT_DIFF_NAME_ONLY_FAILED",
        "workspace.diff_name_only",
    )?;
    let changed_files: Vec<String> = changed.stdout.lines().map(ToOwned::to_owned).collect();
    let forbidden_paths: Vec<String> = changed_files
        .iter()
        .filter(|path| is_forbidden_benchmark_mutation(path))
        .cloned()
        .collect();
    if !forbidden_paths.is_empty() {
        return Ok(json!({
            "status": "failed",
            "error_code": "RUST_REALITY_PATCH_MODIFIES_TESTS",
            "field_path": "candidate.patch",
            "message": "candidate patch modifies benchmark tests or harness files",
            "remediation": "repair production source only; do not change tests, fixtures, or evaluation harnesses",
            "details": {
                "forbidden_paths": forbidden_paths,
                "changed_files": changed_files,
                "actual_patch_sha256": sha256_file(&actual_patch)?
            },
            "source_of_truth": file_sot(&actual_patch)
        }));
    }
    Ok(json!({
        "status": "passed",
        "error_code": null,
        "field_path": "candidate.patch",
        "message": "candidate patch applied to base repository",
        "remediation": "run official SWE-bench readback before claiming task success",
        "details": {
            "actual_patch_sha256": sha256_file(&actual_patch)?,
            "fail_to_pass": task.fail_to_pass
        },
        "source_of_truth": file_sot(&actual_patch)
    }))
}

fn is_forbidden_benchmark_mutation(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with("test_")
        || lower.contains("/test_")
        || lower.starts_with("tests/")
        || lower.contains("/tests/")
        || lower.ends_with("_test.py")
        || lower.contains("conftest.py")
        || lower.contains("tox.ini")
        || lower.contains("pytest")
}

fn verify_attempt(
    bin: &Path,
    ledger: &Path,
    run_id: &str,
    tool_call_id: &str,
    task_root: &Path,
    attempt_dir: &Path,
    validation_path: &Path,
    model: &str,
) -> Result<Value> {
    let verifier = task_root.join("verify_attempt_status.sh");
    write_text_checked(
        &verifier,
        r#"#!/usr/bin/env bash
set -euo pipefail
validation="$1"
if grep -q '"status": "passed"' "$validation"; then
  printf '{"status":"passed","source_of_truth":"file:%s"}\n' "$validation"
  exit 0
fi
printf '{"status":"failed","source_of_truth":"file:%s"}\n' "$validation"
exit 42
"#,
    )?;
    run_required(
        &["chmod", "+x", path_str(&verifier)],
        task_root,
        30,
        "RUST_REALITY_CHMOD_VERIFIER_FAILED",
        "verifier.chmod",
    )?;
    let rel_validation = validation_path
        .strip_prefix(task_root)
        .unwrap_or(validation_path);
    let rel_verifier = verifier.strip_prefix(task_root).unwrap_or(&verifier);
    let spec = json!({
        "verifier_id": "cmd_exit_zero_v1",
        "run_id": run_id,
        "tool_call_id": tool_call_id,
        "source": {"cwd": path_str(task_root)},
        "expected": {
            "command": ["bash", path_str(rel_verifier), path_str(rel_validation)],
            "artifacts": [
                path_str(rel_validation),
                path_str(attempt_dir.join("prompt.txt").strip_prefix(task_root).unwrap_or(&attempt_dir.join("prompt.txt"))),
                path_str(attempt_dir.join("model-response.txt").strip_prefix(task_root).unwrap_or(&attempt_dir.join("model-response.txt")))
            ]
        }
    });
    let spec_path = attempt_dir.join("blackbox-verify-spec.json");
    write_json_checked(&spec_path, &spec)?;
    let mut command = vec![path_str(bin).to_string()];
    command.extend([
        "blackbox".to_string(),
        "verify".to_string(),
        "--ledger".to_string(),
        path_str(ledger).to_string(),
        "--input".to_string(),
        path_str(&spec_path).to_string(),
        "--model".to_string(),
        model.to_string(),
    ]);
    let result = run_cmd(&command, task_root, 240)?;
    let raw = if result.stdout.trim().is_empty() {
        &result.stderr
    } else {
        &result.stdout
    };
    let parsed = serde_json::from_str::<Value>(raw).unwrap_or_else(|_| {
        json!({
            "status": "error",
            "error_code": "RUST_REALITY_BLACKBOX_VERIFY_NON_JSON",
            "process": result
        })
    });
    Ok(parsed)
}

fn run_official_eval(
    args: &Args,
    run_root: &Path,
    attempt_dir: &Path,
    task: &TaskManifest,
    patch: &str,
) -> Result<Value> {
    require_file(
        &args.swebench_python,
        "swebench_python",
        "RUST_REALITY_SWEBENCH_PYTHON_MISSING",
    )?;
    verify_official_docker_sdk(args, attempt_dir)?;
    let predictions = attempt_dir.join("official-prediction.jsonl");
    write_text_checked(
        &predictions,
        &format!(
            "{}\n",
            serde_json::to_string(&json!({
                "instance_id": task.instance_id,
                "model_name_or_path": args.model,
                "model_patch": patch
            }))
            .unwrap()
        ),
    )?;
    let official_root = run_root.join("official-eval").join(format!(
        "attempt-{attempt}",
        attempt = attempt_dir.file_name().unwrap().to_string_lossy()
    ));
    create_dir_checked(&official_root)?;
    let run_id = safe_id(&format!(
        "{}_{}_{}_{}",
        run_root.file_name().unwrap().to_string_lossy(),
        args.model,
        task.instance_id,
        attempt_dir.file_name().unwrap().to_string_lossy()
    ));
    let command = vec![
        path_str(&args.swebench_python).to_string(),
        "-m".to_string(),
        "swebench.harness.run_evaluation".to_string(),
        "--dataset_name".to_string(),
        DATASET_NAME.to_string(),
        "--split".to_string(),
        "test".to_string(),
        "--instance_ids".to_string(),
        task.instance_id.clone(),
        "--predictions_path".to_string(),
        path_str(&predictions).to_string(),
        "--max_workers".to_string(),
        args.official_max_workers.to_string(),
        "--timeout".to_string(),
        args.official_timeout.to_string(),
        "--cache_level".to_string(),
        args.official_cache_level.clone(),
        "--run_id".to_string(),
        run_id.clone(),
        "--report_dir".to_string(),
        path_str(&official_root).to_string(),
    ];
    let official_env = official_docker_env(args, attempt_dir)?;
    let result = run_cmd_env(
        &command,
        &official_root,
        args.official_timeout + 3600,
        &official_env,
    )?;
    let command_path = attempt_dir.join("official-eval-command.json");
    write_json_checked(
        &command_path,
        &json!({"process": result, "predictions": file_sot(&predictions), "report_dir": file_sot(&official_root)}),
    )?;
    if result.exit_code != 0 {
        return Err(RealityError::new(
            "RUST_REALITY_OFFICIAL_EVAL_FAILED",
            "official SWE-bench evaluation failed",
            "official_eval",
            "inspect official-eval-command.json and Docker/SWE-bench output",
            json!({ "command": file_sot(&command_path) }),
            Some(file_sot(&command_path)),
        ));
    }
    let final_report = find_json_report(&official_root, &run_id)?;
    Ok(json!({
        "status": "ok",
        "run_id": run_id,
        "report_dir": path_str(&official_root),
        "final_report": final_report,
        "source_of_truth": {
            "command": file_sot(&command_path),
            "predictions": file_sot(&predictions),
            "report": final_report["source_of_truth"].clone()
        }
    }))
}

fn verify_official_docker_sdk(args: &Args, attempt_dir: &Path) -> Result<()> {
    let official_env = official_docker_env(args, attempt_dir)?;
    let command = vec![
        path_str(&args.swebench_python).to_string(),
        "-c".to_string(),
        "import docker, json; c=docker.from_env(); v=c.version(); print(json.dumps({'status':'ok','version':v.get('Version'),'api_version':v.get('ApiVersion')}))".to_string(),
    ];
    let result = run_cmd_env(&command, Path::new("."), 60, &official_env)?;
    let preflight_path = attempt_dir.join("official-docker-preflight.json");
    write_json_checked(
        &preflight_path,
        &json!({
            "record_kind": "official_docker_preflight",
            "process": result,
            "source_of_truth": "python-docker-sdk:docker.from_env().version"
        }),
    )?;
    if result.exit_code != 0 {
        return Err(RealityError::new(
            "RUST_REALITY_OFFICIAL_DOCKER_UNAVAILABLE",
            "official SWE-bench evaluator cannot access Docker through the Python Docker SDK",
            "official_eval.docker",
            "expose Docker Desktop as /var/run/docker.sock for this WSL distro before running official evaluation",
            json!({ "preflight": file_sot(&preflight_path) }),
            Some(file_sot(&preflight_path)),
        ));
    }
    serde_json::from_str::<Value>(result.stdout.trim()).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_OFFICIAL_DOCKER_PREFLIGHT_JSON_INVALID",
            "Docker preflight did not return JSON",
            "official_eval.docker_preflight.stdout",
            "inspect official-docker-preflight.json",
            json!({ "error": err.to_string(), "preflight": file_sot(&preflight_path) }),
            Some(file_sot(&preflight_path)),
        )
    })?;
    Ok(())
}

fn official_docker_env(args: &Args, attempt_dir: &Path) -> Result<Vec<(String, String)>> {
    let docker_config = resolve_path(&args.official_docker_config)?;
    create_dir_checked(&docker_config)?;
    let config_path = docker_config.join("config.json");
    if !config_path.exists() {
        write_json_checked(&config_path, &json!({"auths": {}}))?;
    } else {
        let _: Value = read_json(&config_path)?;
    }
    let env_manifest = attempt_dir.join("official-docker-env.json");
    write_json_checked(
        &env_manifest,
        &json!({
            "record_kind": "official_docker_env",
            "docker_host": args.official_docker_host,
            "docker_config": path_str(&docker_config),
            "docker_config_sha256": sha256_file(&config_path)?,
            "docker_sock_exists": Path::new("/var/run/docker.sock").exists(),
            "source_of_truth": {
                "docker_config": file_sot(&config_path),
                "docker_socket": "unix:/var/run/docker.sock"
            }
        }),
    )?;
    Ok(vec![
        ("DOCKER_HOST".to_string(), args.official_docker_host.clone()),
        (
            "DOCKER_CONFIG".to_string(),
            path_str(&docker_config).to_string(),
        ),
    ])
}

fn build_official_evidence(
    official: &Value,
    task: &TaskManifest,
    attempt: u64,
    patch: &str,
) -> Result<Value> {
    let report = &official["final_report"]["payload"];
    let resolved_ids = json_string_array(report, "resolved_ids")?;
    let completed_ids = json_string_array(report, "completed_ids")?;
    let incomplete_ids = json_string_array(report, "incomplete_ids")?;
    let empty_patch_ids = json_string_array(report, "empty_patch_ids")?;
    let error_ids = json_string_array(report, "error_ids").unwrap_or_default();
    let instance_id = &task.instance_id;
    let resolved = resolved_ids.iter().any(|id| id == instance_id);
    let completed = completed_ids.iter().any(|id| id == instance_id);
    let incomplete = incomplete_ids.iter().any(|id| id == instance_id);
    let empty_patch = empty_patch_ids.iter().any(|id| id == instance_id);
    let errored = error_ids.iter().any(|id| id == instance_id);
    if !completed && !incomplete && !empty_patch && !errored {
        return Err(RealityError::new(
            "RUST_REALITY_OFFICIAL_REPORT_INSTANCE_MISSING",
            "official report did not classify the target instance",
            "official_report.instance_id",
            "inspect the official report, run_id, and --instance_ids before continuing",
            json!({
                "instance_id": instance_id,
                "completed_ids": completed_ids,
                "incomplete_ids": incomplete_ids,
                "empty_patch_ids": empty_patch_ids,
                "error_ids": error_ids,
                "report_source_of_truth": official["final_report"]["source_of_truth"]
            }),
            official["final_report"]["source_of_truth"]
                .as_str()
                .map(ToOwned::to_owned),
        ));
    }
    Ok(json!({
        "evidence_kind": "rust_official_swe_result",
        "attempt": attempt,
        "claim_scope": "official_swe_instance_resolution_only",
        "success_predicate": "official_report.resolved_ids contains task.instance_id",
        "instance_id": task.instance_id,
        "resolved": resolved,
        "completed": completed,
        "incomplete": incomplete,
        "empty_patch": empty_patch,
        "errored": errored,
        "previous_patch_sha256": sha256_text(patch),
        "source_of_truth": official["source_of_truth"].clone(),
        "official_report": report,
    }))
}

fn json_string_array(value: &Value, field: &str) -> Result<Vec<String>> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_OFFICIAL_REPORT_FIELD_MISSING",
                "official report was missing a required instance classification field",
                format!("official_report.{field}"),
                "inspect SWE-bench report schema before trusting the run",
                json!({"field": field, "report": value}),
                None,
            )
        })?
        .iter()
        .map(|item| {
            item.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                RealityError::new(
                    "RUST_REALITY_OFFICIAL_REPORT_FIELD_INVALID",
                    "official report classification field contained a non-string id",
                    format!("official_report.{field}"),
                    "inspect SWE-bench report schema before trusting the run",
                    json!({"field": field, "item": item}),
                    None,
                )
            })
        })
        .collect()
}

fn find_json_report(root: &Path, run_id: &str) -> Result<Value> {
    let mut candidates = Vec::new();
    collect_json_files(root, &mut candidates)?;
    candidates.sort();
    for path in candidates {
        let text = fs::read_to_string(&path)
            .map_err(|err| fs_error("RUST_REALITY_REPORT_READ_FAILED", &path, err))?;
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            let has_resolved = value.get("resolved_ids").is_some();
            let name_matches = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.contains(run_id))
                .unwrap_or(false);
            if has_resolved && name_matches {
                return Ok(
                    json!({"path": path_str(&path), "sha256": sha256_file(&path)?, "source_of_truth": file_sot(&path), "payload": value}),
                );
            }
        }
    }
    Err(RealityError::new(
        "RUST_REALITY_OFFICIAL_REPORT_MISSING",
        "official SWE-bench run did not produce a readable final report with resolved_ids",
        "official_report.resolved_ids",
        "inspect official report directory before continuing",
        json!({ "report_dir": path_str(root), "run_id": run_id }),
        Some(file_sot(root)),
    ))
}

fn collect_json_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in
        fs::read_dir(root).map_err(|err| fs_error("RUST_REALITY_READ_DIR_FAILED", root, err))?
    {
        let entry =
            entry.map_err(|err| fs_error("RUST_REALITY_READ_DIR_ENTRY_FAILED", root, err))?;
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            out.push(path);
        }
    }
    Ok(())
}

fn collect_repository_context(repo_dir: &Path, task: &TaskManifest) -> Result<Value> {
    let files = run_required(
        &["git", "ls-files"],
        repo_dir,
        120,
        "RUST_REALITY_GIT_LS_FILES_FAILED",
        "workspace.files",
    )?;
    let all_files: Vec<String> = files.stdout.lines().map(ToOwned::to_owned).collect();
    let all_file_set: BTreeSet<String> = all_files.iter().cloned().collect();
    let mut selected_paths = BTreeSet::new();
    let terms = task_terms(task);

    for candidate in production_path_candidates(task)
        .into_iter()
        .chain(path_candidates(task))
    {
        add_context_candidate(&mut selected_paths, &all_files, &all_file_set, &candidate);
    }

    for symbol in task_symbols(&terms) {
        if selected_paths.len() >= 10 {
            break;
        }
        let grep = run_cmd(
            &vec![
                "git".to_string(),
                "grep".to_string(),
                "-l".to_string(),
                format!("def {symbol}"),
            ],
            repo_dir,
            120,
        )?;
        if grep.exit_code == 0 {
            for line in grep.stdout.lines().take(4) {
                add_context_candidate(&mut selected_paths, &all_files, &all_file_set, line);
            }
        }
    }

    let grep_terms: Vec<String> = terms.iter().take(24).cloned().collect();
    for grep_term in grep_terms {
        if selected_paths.len() >= 10 {
            break;
        }
        let grep = run_cmd(
            &vec![
                "git".to_string(),
                "grep".to_string(),
                "-l".to_string(),
                grep_term,
            ],
            repo_dir,
            120,
        )?;
        if grep.exit_code == 0 {
            for line in grep.stdout.lines().take(8) {
                add_context_candidate(&mut selected_paths, &all_files, &all_file_set, line);
            }
        }
    }

    for line in &all_files {
        if selected_paths.len() >= 10 {
            break;
        }
        if !is_context_file(line) || is_low_value_context_path(line) {
            continue;
        }
        let lower = line.to_lowercase();
        if terms
            .iter()
            .any(|term| lower.contains(&term.to_lowercase()))
        {
            selected_paths.insert(line.to_string());
        }
    }

    let mut selected = Vec::new();
    for line in selected_paths {
        let path = repo_dir.join(&line);
        if path.exists() {
            let text = fs::read_to_string(&path).unwrap_or_default();
            selected.push(json!({
                "path": line,
                "sha256": sha256_file(&path)?,
                "source_of_truth": file_sot(&path),
                "text": focused_context_text(&text, &terms, 4800)
            }));
        }
    }
    Ok(json!({
        "source_of_truth": format!("git:{}#{}", path_str(repo_dir), task.base_commit),
        "terms": terms,
        "files": selected
    }))
}

fn add_context_candidate(
    selected_paths: &mut BTreeSet<String>,
    all_files: &[String],
    all_file_set: &BTreeSet<String>,
    candidate: &str,
) {
    if selected_paths.len() >= 10 {
        return;
    }
    if all_file_set.contains(candidate)
        && is_context_file(candidate)
        && !is_low_value_context_path(candidate)
    {
        selected_paths.insert(candidate.to_string());
    } else if !candidate.contains('/') {
        for path in all_files
            .iter()
            .filter(|path| path.ends_with(candidate))
            .filter(|path| is_context_file(path) && !is_low_value_context_path(path))
            .take(6)
        {
            selected_paths.insert(path.to_string());
        }
    }
}

fn task_terms(task: &TaskManifest) -> Vec<String> {
    let mut terms = BTreeSet::new();
    for part in task.repo.split(['/', '-', '_']) {
        if part.len() > 2 {
            terms.insert(part.to_string());
        }
    }
    for source in std::iter::once(task.problem_statement.as_str())
        .chain(task.fail_to_pass.iter().map(String::as_str))
    {
        for token in source.split(|c: char| !c.is_alphanumeric() && c != '_') {
            if token.len() > 3 && token.len() < 64 && !token.chars().all(|c| c.is_ascii_digit()) {
                terms.insert(token.to_string());
            }
        }
    }
    terms.into_iter().take(40).collect()
}

fn task_symbols(terms: &[String]) -> Vec<String> {
    terms
        .iter()
        .filter(|term| {
            term.contains('_')
                && term
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        })
        .cloned()
        .take(12)
        .collect()
}

fn production_path_candidates(task: &TaskManifest) -> Vec<String> {
    let mut out = BTreeSet::new();
    for path in path_candidates(task) {
        if let Some((prefix, test_name)) = path.split_once("/tests/test_") {
            let stem = test_name.strip_suffix(".py").unwrap_or(test_name);
            out.insert(format!("{prefix}/{stem}.py"));
            out.insert(format!("{prefix}/_{stem}.py"));
        }
    }
    out.into_iter().collect()
}

fn path_candidates(task: &TaskManifest) -> Vec<String> {
    let mut out = BTreeSet::new();
    for source in std::iter::once(task.problem_statement.as_str())
        .chain(task.fail_to_pass.iter().map(String::as_str))
    {
        for token in source.split(|ch: char| {
            ch.is_whitespace() || ch == '(' || ch == ')' || ch == ',' || ch == ':'
        }) {
            let candidate = token.trim_matches(|ch: char| {
                ch == '`'
                    || ch == '"'
                    || ch == '\''
                    || ch == '.'
                    || ch == ';'
                    || ch == '['
                    || ch == ']'
            });
            if candidate.contains('/') || candidate.ends_with(".py") || candidate.ends_with(".pyx")
            {
                let path = candidate.split("::").next().unwrap_or(candidate);
                out.insert(path.to_string());
            }
        }
    }
    out.into_iter().collect()
}

fn is_context_file(path: &str) -> bool {
    path.ends_with(".py")
        || path.ends_with(".pyx")
        || path.ends_with(".pxd")
        || path.ends_with(".cfg")
        || path.ends_with(".ini")
        || path.ends_with(".toml")
}

fn is_low_value_context_path(path: &str) -> bool {
    path.starts_with("doc/")
        || path.starts_with("docs/")
        || path.starts_with("examples/")
        || path.starts_with("benchmarks/")
        || path.starts_with(".github/")
        || path.starts_with("tests/")
        || path.contains("/tests/")
        || path.ends_with("conftest.py")
}

fn focused_context_text(text: &str, terms: &[String], limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let lower = text.to_ascii_lowercase();
    let mut windows = Vec::new();
    for term in terms.iter().filter(|term| term.len() >= 4) {
        if windows.len() >= 4 {
            break;
        }
        let needle = term.to_ascii_lowercase();
        if let Some(byte_pos) = lower.find(&needle) {
            windows.push(text_window(text, byte_pos, 1600));
        }
    }
    if windows.is_empty() {
        truncate_chars(text, limit)
    } else {
        truncate_chars(&windows.join("\n\n...[context window]...\n\n"), limit)
    }
}

fn text_window(text: &str, byte_pos: usize, radius: usize) -> String {
    let start = text[..byte_pos]
        .char_indices()
        .rev()
        .nth(radius)
        .map(|(idx, _)| idx)
        .unwrap_or(0);
    let end = text[byte_pos..]
        .char_indices()
        .nth(radius)
        .map(|(idx, _)| byte_pos + idx)
        .unwrap_or(text.len());
    text[start..end].to_string()
}

fn compile_prompt(
    task: &TaskManifest,
    repo_context: &Value,
    attempts: &[AttemptRecord],
    previous_patch_hashes: &[String],
) -> String {
    let previous = attempts
        .last()
        .map(|item| serde_json::to_string_pretty(item).unwrap())
        .unwrap_or_else(|| "null".to_string());
    format!(
        "You are repairing a real SWE-bench Lite task through ContextGraph's Rust reality loop.\n\
Success means the official SWE-bench report places instance_id in resolved_ids.\n\
Do not claim success. Prefer exact structured edits: {{\"edits\":[{{\"path\":\"relative/source/file.py\",\"find\":\"exact old text\",\"replace\":\"exact new text\"}}],\"notes\":\"<brief>\"}}. Use {{\"patch\":\"<unified git diff>\",\"notes\":\"<brief>\"}} only if you can produce a complete valid diff.\n\n\
Task:\ninstance_id: {}\nrepo: {}\nbase_commit: {}\nFAIL_TO_PASS: {}\n\n\
Problem statement:\n<<<PROBLEM\n{}\nPROBLEM\n\n\
Previous attempt summary:\n{}\n\n\
Do not repeat these patch hashes:\n{}\n\n\
Hard validation rules:\n\
- The response must be one raw JSON object with notes plus either edits or patch.\n\
- Prefer edits. Each edit path must be repo-relative production source; find must match exactly one source span; replace is the desired source span.\n\
- ContextGraph applies edits to the real checkout, generates candidate.patch with git diff, resets to base, then applies/evaluates the generated patch.\n\
- If you use patch instead, it must apply to the exact base commit with git apply and be a complete unified diff with correct ---/+++ headers and @@ hunks.\n\
- The patch or edits must modify production source, not tests, fixtures, or benchmark harness files.\n\
- Do not edit tests to make the benchmark pass.\n\n\
Repository context JSON:\n{}\n\n\
Final instruction: return exactly one JSON object with notes and either edits or patch. No markdown, no summary, no prose.\n",
        task.instance_id,
        task.repo,
        task.base_commit,
        serde_json::to_string(&task.fail_to_pass).unwrap(),
        task.problem_statement,
        previous,
        serde_json::to_string(previous_patch_hashes).unwrap(),
        serde_json::to_string_pretty(repo_context).unwrap()
    )
}

fn write_summary(
    run_root: &Path,
    task: &TaskManifest,
    args: &Args,
    ledger: &Path,
    events: &Path,
    contract: &Path,
    attempts: &[AttemptRecord],
    solved: bool,
    terminal: bool,
) -> Result<Value> {
    let path = run_root.join("reality-loop-summary.json");
    let summary = json!({
        "status": if solved { "solved" } else if terminal { "unsolved" } else { "running" },
        "schema": "contextgraph_rust_reality_loop_summary_v1",
        "model": args.model,
        "task": task,
        "run_until_resolved": args.run_until_resolved,
        "official_eval_each_attempt": args.official_eval_each_attempt,
        "attempts_total": attempts.len(),
        "attempts": attempts,
        "gpu_budget": {
            "gpu_index": args.gpu_index,
            "min_free_mb": args.gpu_min_free_mb,
            "max_used_mb": args.gpu_max_used_mb,
            "current": read_gpu_vram_state(args.gpu_index)?
        },
        "ledger": {
            "path": path_str(ledger),
            "counts": ledger_counts(ledger)?,
            "sha256": sha256_file(ledger)?,
            "source_of_truth": sqlite_sot(ledger)
        },
        "measurement_contract": {
            "path": path_str(contract),
            "sha256": sha256_file(contract)?,
            "source_of_truth": file_sot(contract)
        },
        "source_of_truth": {
            "summary": file_sot(&path),
            "events": file_sot(events),
            "measurement_contract": file_sot(contract),
            "ledger": sqlite_sot(ledger)
        }
    });
    write_json_checked(&path, &summary)?;
    Ok(summary)
}

fn append_event(path: &Path, event: Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_checked(parent)?;
    }
    let mut enriched = event;
    enriched["created_at_unix"] = json!(unix_secs()?);
    enriched["payload_sha256"] = json!(sha256_text(&serde_json::to_string(&enriched).unwrap()));
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| fs_error("RUST_REALITY_EVENT_OPEN_FAILED", path, err))?;
    writeln!(file, "{}", serde_json::to_string(&enriched).unwrap())
        .map_err(|err| fs_error("RUST_REALITY_EVENT_WRITE_FAILED", path, err))?;
    let readback = fs::read_to_string(path)
        .map_err(|err| fs_error("RUST_REALITY_EVENT_READBACK_FAILED", path, err))?;
    let last = readback.lines().last().unwrap_or("");
    let parsed: Value = serde_json::from_str(last).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_EVENT_READBACK_JSON_INVALID",
            "latest event readback was not JSON",
            "reality_events.latest",
            "inspect reality event log before continuing",
            json!({ "error": err.to_string(), "line": last }),
            Some(file_sot(path)),
        )
    })?;
    if parsed != enriched {
        return Err(RealityError::new(
            "RUST_REALITY_EVENT_READBACK_MISMATCH",
            "latest event readback did not match appended event",
            "reality_events.latest",
            "inspect filesystem durability before continuing",
            json!({ "expected": enriched, "actual": parsed }),
            Some(file_sot(path)),
        ));
    }
    Ok(())
}

fn read_gpu_vram_state(gpu_index: u64) -> Result<GpuState> {
    let command = vec![
        "nvidia-smi".to_string(),
        "--query-gpu=index,name,uuid,memory.total,memory.used,memory.free,driver_version"
            .to_string(),
        "--format=csv,noheader,nounits".to_string(),
    ];
    let result = run_cmd(&command, Path::new("."), 30)?;
    if result.exit_code != 0 {
        return Err(RealityError::new(
            "RUST_REALITY_GPU_VRAM_READ_FAILED",
            "nvidia-smi failed while reading GPU VRAM state",
            "gpu_vram.nvidia_smi",
            "fix NVIDIA driver/WSL GPU visibility before running the loop",
            json!({ "process": result }),
            Some("nvidia-smi:query-gpu".to_string()),
        ));
    }
    for line in result.stdout.lines() {
        let parts: Vec<_> = line.split(',').map(|item| item.trim()).collect();
        if parts.len() != 7 {
            continue;
        }
        let index = parts[0].parse::<u64>().map_err(|err| {
            RealityError::new(
                "RUST_REALITY_GPU_VRAM_PARSE_FAILED",
                "nvidia-smi GPU index was not numeric",
                "gpu_vram.index",
                "inspect nvidia-smi output",
                json!({ "line": line, "error": err.to_string() }),
                Some("nvidia-smi:query-gpu".to_string()),
            )
        })?;
        if index != gpu_index {
            continue;
        }
        let apps = run_cmd(
            &[
                "nvidia-smi".to_string(),
                "--query-compute-apps=pid,process_name,used_memory".to_string(),
                "--format=csv,noheader,nounits".to_string(),
            ],
            Path::new("."),
            30,
        )?;
        return Ok(GpuState {
            index,
            name: parts[1].to_string(),
            uuid: parts[2].to_string(),
            memory_total_mb: parse_u64(parts[3], "gpu_vram.memory_total_mb")?,
            memory_used_mb: parse_u64(parts[4], "gpu_vram.memory_used_mb")?,
            memory_free_mb: parse_u64(parts[5], "gpu_vram.memory_free_mb")?,
            driver_version: parts[6].to_string(),
            compute_processes_raw: apps.stdout.trim().to_string(),
            source_of_truth: "nvidia-smi:query-gpu+query-compute-apps".to_string(),
            read_at_unix: unix_secs()?,
        });
    }
    Err(RealityError::new(
        "RUST_REALITY_GPU_INDEX_MISSING",
        "configured GPU index was not present in nvidia-smi output",
        "gpu_index",
        "choose a valid --gpu-index",
        json!({ "gpu_index": gpu_index, "stdout": result.stdout }),
        Some("nvidia-smi:query-gpu".to_string()),
    ))
}

fn assert_gpu_budget(
    state: &GpuState,
    min_free_mb: u64,
    max_used_mb: Option<u64>,
    phase: &str,
    model: &str,
) -> Result<()> {
    if state.memory_free_mb < min_free_mb {
        return Err(RealityError::new(
            "RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED",
            "free GPU VRAM is below configured reserve",
            "gpu_vram.memory_free_mb",
            "unload other GPU processes or lower model/context pressure",
            json!({ "phase": phase, "model": model, "min_free_mb": min_free_mb, "state": state }),
            Some(state.source_of_truth.clone()),
        ));
    }
    if let Some(max_used) = max_used_mb {
        if state.memory_used_mb > max_used {
            return Err(RealityError::new(
                "RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED",
                "used GPU VRAM is above configured maximum",
                "gpu_vram.memory_used_mb",
                "unload other GPU processes or increase budget only after inspection",
                json!({ "phase": phase, "model": model, "max_used_mb": max_used, "state": state }),
                Some(state.source_of_truth.clone()),
            ));
        }
    }
    Ok(())
}

fn run_required(
    args: &[&str],
    cwd: &Path,
    timeout: u64,
    code: &str,
    field_path: &str,
) -> Result<ProcessResult> {
    let command: Vec<String> = args.iter().map(|item| item.to_string()).collect();
    let result = run_cmd(&command, cwd, timeout)?;
    if result.exit_code != 0 {
        return Err(RealityError::new(
            code,
            "required command failed",
            field_path,
            "inspect command stdout/stderr before continuing",
            json!({ "process": result }),
            None,
        ));
    }
    Ok(result)
}

fn ledger_counts(ledger: &Path) -> Result<BTreeMap<String, i64>> {
    let conn = rusqlite::Connection::open(ledger).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_LEDGER_OPEN_FAILED",
            "failed to open SQLite ledger",
            "ledger",
            "inspect ledger path and permissions",
            json!({ "path": path_str(ledger), "error": err.to_string() }),
            Some(sqlite_sot(ledger)),
        )
    })?;
    let mut stmt = conn
        .prepare("SELECT record_kind, COUNT(*) FROM ledger_records GROUP BY record_kind ORDER BY record_kind")
        .map_err(|err| {
            RealityError::new(
                "RUST_REALITY_LEDGER_QUERY_FAILED",
                "failed to query ledger counts",
                "ledger.ledger_records",
                "inspect ledger schema",
                json!({ "path": path_str(ledger), "error": err.to_string() }),
                Some(sqlite_sot(ledger)),
            )
        })?;
    let mut rows = stmt.query([]).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_LEDGER_QUERY_FAILED",
            "failed to query ledger rows",
            "ledger.ledger_records",
            "inspect ledger schema",
            json!({ "path": path_str(ledger), "error": err.to_string() }),
            Some(sqlite_sot(ledger)),
        )
    })?;
    let mut out = BTreeMap::new();
    while let Some(row) = rows.next().map_err(|err| {
        RealityError::new(
            "RUST_REALITY_LEDGER_ROW_READ_FAILED",
            "failed to read ledger count row",
            "ledger.ledger_records",
            "inspect ledger schema",
            json!({ "path": path_str(ledger), "error": err.to_string() }),
            Some(sqlite_sot(ledger)),
        )
    })? {
        let kind: String = row.get(0).unwrap_or_else(|_| "unknown".to_string());
        let count: i64 = row.get(1).unwrap_or(0);
        out.insert(kind, count);
    }
    Ok(out)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path)
        .map_err(|err| fs_error("RUST_REALITY_JSON_READ_FAILED", path, err))?;
    serde_json::from_str(&text).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_JSON_PARSE_FAILED",
            "JSON file could not be parsed",
            "json",
            "inspect JSON file syntax",
            json!({ "path": path_str(path), "error": err.to_string() }),
            Some(file_sot(path)),
        )
    })
}

fn write_json_checked(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_checked(parent)?;
    }
    let text = serde_json::to_string_pretty(value).unwrap();
    fs::write(path, text).map_err(|err| fs_error("RUST_REALITY_JSON_WRITE_FAILED", path, err))?;
    let readback = fs::read_to_string(path)
        .map_err(|err| fs_error("RUST_REALITY_JSON_READBACK_FAILED", path, err))?;
    let expected: Value = serde_json::from_str(&serde_json::to_string(value).unwrap()).unwrap();
    let actual: Value = serde_json::from_str(&readback).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_JSON_READBACK_PARSE_FAILED",
            "JSON readback could not be parsed",
            "json.readback",
            "inspect filesystem write/read durability",
            json!({ "path": path_str(path), "error": err.to_string() }),
            Some(file_sot(path)),
        )
    })?;
    if expected != actual {
        return Err(RealityError::new(
            "RUST_REALITY_JSON_READBACK_MISMATCH",
            "JSON readback did not match written value",
            "json.readback",
            "inspect filesystem write/read durability",
            json!({ "path": path_str(path), "expected": expected, "actual": actual }),
            Some(file_sot(path)),
        ));
    }
    Ok(())
}

fn write_text_checked(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_checked(parent)?;
    }
    fs::write(path, text).map_err(|err| fs_error("RUST_REALITY_TEXT_WRITE_FAILED", path, err))?;
    let readback = fs::read_to_string(path)
        .map_err(|err| fs_error("RUST_REALITY_TEXT_READBACK_FAILED", path, err))?;
    if readback != text {
        return Err(RealityError::new(
            "RUST_REALITY_TEXT_READBACK_MISMATCH",
            "text readback did not match written value",
            "text.readback",
            "inspect filesystem write/read durability",
            json!({ "path": path_str(path) }),
            Some(file_sot(path)),
        ));
    }
    Ok(())
}

fn create_dir_checked(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .map_err(|err| fs_error("RUST_REALITY_CREATE_DIR_FAILED", path, err))?;
    if !path.exists() || !path.is_dir() {
        return Err(RealityError::new(
            "RUST_REALITY_CREATE_DIR_READBACK_FAILED",
            "directory did not exist after create_dir_all",
            "filesystem.directory",
            "inspect filesystem permissions and path",
            json!({ "path": path_str(path) }),
            Some(file_sot(path)),
        ));
    }
    Ok(())
}

fn require_file(path: &Path, field: &str, code: &str) -> Result<()> {
    if !path.exists() || !path.is_file() {
        return Err(RealityError::new(
            code,
            "required file is missing",
            field,
            "provide a real readable file before running the loop",
            json!({ "path": path_str(path) }),
            Some(file_sot(path)),
        ));
    }
    Ok(())
}

fn resolve_path(path: &Path) -> Result<PathBuf> {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_CWD_FAILED",
                    "failed to read current working directory",
                    "cwd",
                    "inspect process working directory",
                    json!({ "error": err.to_string() }),
                    None,
                )
            })?
            .join(path)
    };
    Ok(resolved)
}

fn sha256_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes =
        fs::read(path).map_err(|err| fs_error("RUST_REALITY_SHA256_READ_FAILED", path, err))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn short_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())[..8].to_string()
}

fn unix_secs() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| {
            RealityError::new(
                "RUST_REALITY_TIME_FAILED",
                "system time was before UNIX_EPOCH",
                "time",
                "fix system clock",
                json!({ "error": err.to_string() }),
                None,
            )
        })?
        .as_secs())
}

fn unix_secs_for_logging() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn unix_ns() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| {
            RealityError::new(
                "RUST_REALITY_TIME_FAILED",
                "system time was before UNIX_EPOCH",
                "time",
                "fix system clock",
                json!({ "error": err.to_string() }),
                None,
            )
        })?
        .as_nanos())
}

fn parse_u64(value: &str, field: &str) -> Result<u64> {
    value.parse::<u64>().map_err(|err| {
        RealityError::new(
            "RUST_REALITY_U64_PARSE_FAILED",
            "numeric value could not be parsed",
            field,
            "inspect source-of-truth output",
            json!({ "value": value, "error": err.to_string() }),
            None,
        )
    })
}

fn fs_error(code: &str, path: &Path, err: std::io::Error) -> RealityError {
    RealityError::new(
        code,
        "filesystem operation failed",
        "filesystem",
        "inspect path, permissions, and disk space",
        json!({ "path": path_str(path), "error": err.to_string() }),
        Some(file_sot(path)),
    )
}

fn path_str(path: &Path) -> &str {
    path.to_str().unwrap_or("")
}

fn file_sot(path: &Path) -> String {
    format!("file:{}", path_str(path))
}

fn sqlite_sot(path: &Path) -> String {
    format!("sqlite:{}#ledger_records", path_str(path))
}

fn safe_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(160)
        .collect()
}

fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        value.to_string()
    } else {
        let mut out = value.chars().take(limit).collect::<String>();
        out.push_str("\n...[truncated]");
        out
    }
}
