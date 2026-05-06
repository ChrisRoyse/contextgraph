use clap::{Parser, Subcommand};
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{
    AddBos, ChatTemplateResult, GrammarTriggerType, LlamaChatTemplate, LlamaModel,
};
use llama_cpp_2::openai::OpenAIChatTemplateParams;
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;
use llama_cpp_2::TokenToStringError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_RUNTIME_ROOT: &str = "/tmp/contextgraph-rust-reality-loop";
const DEFAULT_CONTEXT_GRAPH_BIN: &str = "target/debug/context-graph";
const DEFAULT_MODEL: &str = "gemma4:e4b";
const DEFAULT_GEMMA_MODEL_PATH: &str = "models/gemma4-e4b/gemma4-e4b.Q4_K_M.gguf";
const DEFAULT_GPU_MIN_FREE_MB: u64 = 8192;
const DEFAULT_GEMMA_MIN_CONTEXT_TOKENS: u32 = 128_000;
const DEFAULT_GEMMA_MAX_NEW_TOKENS: usize = 16384;
const DEFAULT_GEMMA_TEMPERATURE: f32 = 0.0;
const DEFAULT_GEMMA_TOP_P: f32 = 0.9;
const DEFAULT_GEMMA_BATCH_SIZE: usize = 2048;
const DEFAULT_GEMMA_SEED: u32 = 1;
const DEFAULT_AGENT_TOOL_ROUNDS: u64 = 24;
const DEFAULT_OFFICIAL_DOCKER_HOST: &str = "unix:///var/run/docker.sock";
const DEFAULT_OFFICIAL_DOCKER_CONFIG: &str = "/home/cabdru/.cache/contextgraph-docker-config";
const DEFAULT_SMOKE_RUNTIME_ROOT: &str = "/tmp/cgreality-smoke";
const DEFAULT_REPLAY_OUTPUT_LABEL: &str = "replay";
const DATASET_NAME: &str = "princeton-nlp/SWE-bench_Lite";
const LOCAL_PYTHON_BIN: &str = "python3";

#[derive(Parser, Debug, Clone)]
#[command(name = "reality-loop")]
#[command(version)]
#[command(
    about = "ContextGraph reality engine: runs Gemma SWE-bench attempts and synthetic smokes; writes durable artifacts."
)]
#[command(
    long_about = "After Phase 1+ of the ccreality cutover, this binary is a pure reality engine. The outer optimizer lives in Claude Code, which talks to ContextGraph through the cgreality MCP server. See docs/ccreality/05_REALITY_ENGINE_RESPONSIBILITY.md."
)]
struct TopArgs {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug, Clone)]
enum Cmd {
    /// Run one or more Gemma SWE-bench repair attempts. Writes attempt artifacts.
    Attempt(AttemptArgs),
    /// Run a synthetic smoke attempt. No Gemma, no Docker. For wiring verification.
    Smoke(SmokeArgs),
    /// Re-run official SWE-bench Docker evaluation against an existing attempt patch.
    OfficialEval(OfficialEvalArgs),
    /// Apply an existing attempt patch and run one exact fail_to_pass pytest target.
    TargetedTest(TargetedTestArgs),
    /// Replay an existing attempt's reality-signal-packet builder against current code.
    Replay(ReplayArgs),
    /// Full-state-verify every JSON artifact under a runtime root.
    Verify(VerifyArgs),
}

#[derive(Parser, Debug, Clone)]
struct SmokeArgs {
    #[arg(long, default_value = DEFAULT_SMOKE_RUNTIME_ROOT)]
    runtime_root: PathBuf,
    /// Label appended to the synthetic instance ID.
    #[arg(long, default_value = "smoke")]
    label: String,
    /// Force-fail the smoke to exercise failure artifact writing.
    #[arg(long, default_value_t = false)]
    force_fail: bool,
}

#[derive(Parser, Debug, Clone)]
struct ReplayArgs {
    /// Path to an existing attempt-summary.json.
    #[arg(long)]
    attempt_summary_path: PathBuf,
    /// Where to write the replayed reality-signal-packet envelope.
    #[arg(long)]
    output_dir: Option<PathBuf>,
    /// Suffix on the output filename.
    #[arg(long, default_value = DEFAULT_REPLAY_OUTPUT_LABEL)]
    label: String,
}

#[derive(Parser, Debug, Clone)]
struct OfficialEvalArgs {
    #[arg(long, default_value = DEFAULT_RUNTIME_ROOT)]
    runtime_root: PathBuf,
    #[arg(long)]
    run_id: String,
    #[arg(long)]
    target_instance: String,
    #[arg(long)]
    attempt: u64,
    #[arg(long, default_value = DEFAULT_MODEL)]
    model: String,
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
}

#[derive(Parser, Debug, Clone)]
struct TargetedTestArgs {
    #[arg(long, default_value = DEFAULT_RUNTIME_ROOT)]
    runtime_root: PathBuf,
    #[arg(long)]
    run_id: String,
    #[arg(long)]
    target_instance: String,
    #[arg(long)]
    attempt: u64,
    #[arg(long)]
    test_id: String,
    #[arg(long, default_value_t = 300)]
    timeout_secs: u64,
    #[arg(long, default_value_t = 600)]
    git_timeout: u64,
}

#[derive(Parser, Debug, Clone)]
struct VerifyArgs {
    /// Runtime root containing attempt directories.
    #[arg(long)]
    runtime_root: PathBuf,
    /// Specific attempt to verify. When omitted, all JSON artifacts are enumerated.
    #[arg(long)]
    attempt: Option<u64>,
}

#[derive(Parser, Debug, Clone)]
#[command(about = "Run one or more Gemma SWE-bench repair attempts")]
struct AttemptArgs {
    /// Stable model label used in artifacts and BlackBox ledger rows.
    #[arg(long, default_value = DEFAULT_MODEL)]
    model: String,
    /// Real GGUF model file loaded by the Rust Gemma coding harness.
    #[arg(long, default_value = DEFAULT_GEMMA_MODEL_PATH)]
    gemma_model_path: PathBuf,
    /// Optional explicit llama.cpp chat template name or template text.
    #[arg(long)]
    gemma_chat_template: Option<String>,
    #[arg(long)]
    task_manifest: PathBuf,
    #[arg(long, default_value = DEFAULT_RUNTIME_ROOT)]
    runtime_root: PathBuf,
    #[arg(long, default_value = DEFAULT_CONTEXT_GRAPH_BIN)]
    context_graph_bin: PathBuf,
    #[arg(long, default_value_t = 1)]
    max_attempts: u64,
    #[arg(long)]
    run_until_resolved: bool,
    #[arg(long)]
    smoke_test: bool,
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
    /// Context window tokens. Omit to use the loaded GGUF model's full training context.
    #[arg(long)]
    gemma_context_tokens: Option<u32>,
    #[arg(long, default_value_t = DEFAULT_GEMMA_MAX_NEW_TOKENS)]
    gemma_max_new_tokens: usize,
    #[arg(long, default_value_t = DEFAULT_GEMMA_TEMPERATURE)]
    gemma_temperature: f32,
    #[arg(long, default_value_t = DEFAULT_GEMMA_TOP_P)]
    gemma_top_p: f32,
    #[arg(long, default_value_t = u32::MAX)]
    gemma_gpu_layers: u32,
    #[arg(long, default_value_t = DEFAULT_GEMMA_BATCH_SIZE)]
    gemma_batch_size: usize,
    #[arg(long, default_value_t = DEFAULT_GEMMA_SEED)]
    gemma_seed: u32,
    #[arg(long, default_value_t = 600)]
    git_timeout: u64,
    #[arg(long, default_value_t = DEFAULT_AGENT_TOOL_ROUNDS)]
    agent_tool_rounds: u64,
    #[arg(long, default_value_t = 300)]
    targeted_test_timeout: u64,
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
    reality_signal_packet_path: String,
    reality_signal_compact: Value,
    agent_tool_transcript_path: Option<String>,
    patch_preview: Option<String>,
}

struct AgentToolContext<'a> {
    args: &'a AttemptArgs,
    task: &'a TaskManifest,
    context_graph_bin: &'a Path,
    ledger: &'a Path,
    run_id: &'a str,
    task_root: &'a Path,
    repo_dir: &'a Path,
    attempt_dir: &'a Path,
    repo_context: &'a Value,
    problem_reality: &'a Value,
    attempts: &'a [AttemptRecord],
}

#[tokio::main]
async fn main() {
    let top = match TopArgs::try_parse() {
        Ok(top) => top,
        Err(err) => {
            let exit_code = err.exit_code();
            let kind = err.kind();
            let rendered = err.to_string();
            if matches!(
                kind,
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                print!("{rendered}");
                std::process::exit(exit_code);
            }
            let reality_err = RealityError::new(
                "RUST_REALITY_CLI_PARSE_FAILED",
                "reality-loop CLI argument parsing failed",
                "cli.args",
                "inspect the rejected argument and run --help for the supported reality-loop contract",
                json!({"clap_error_kind": format!("{kind:?}"), "clap_error": rendered}),
                None,
            );
            eprintln!("{}", serde_json::to_string_pretty(&reality_err).unwrap());
            std::process::exit(exit_code);
        }
    };
    let exit_code = match top.cmd {
        Cmd::Attempt(args) => dispatch_attempt(args).await,
        Cmd::Smoke(args) => dispatch_smoke(args).await,
        Cmd::OfficialEval(args) => dispatch_official_eval(args).await,
        Cmd::TargetedTest(args) => dispatch_targeted_test(args).await,
        Cmd::Replay(args) => dispatch_replay(args).await,
        Cmd::Verify(args) => dispatch_verify(args).await,
    };
    std::process::exit(exit_code);
}

async fn dispatch<F>(future: F, failure_runtime_root: &Path) -> i32
where
    F: std::future::Future<Output = Result<Value>>,
{
    match future.await {
        Ok(summary) => {
            println!("{}", serde_json::to_string_pretty(&summary).unwrap());
            0
        }
        Err(err) => {
            if let Err(write_err) = write_failure_artifact_for_command(failure_runtime_root, &err) {
                let failure_write = json!({
                    "status": "error",
                    "error_code": "RUST_REALITY_FAILURE_ARTIFACT_WRITE_FAILED",
                    "message": write_err,
                    "original_error_code": err.error_code,
                    "source_of_truth": file_sot(failure_runtime_root)
                });
                eprintln!("{}", serde_json::to_string_pretty(&failure_write).unwrap());
            }
            eprintln!("{}", serde_json::to_string_pretty(&err).unwrap());
            1
        }
    }
}

async fn dispatch_attempt(args: AttemptArgs) -> i32 {
    let failure_runtime_root = args.runtime_root.clone();
    dispatch(run(args), &failure_runtime_root).await
}

async fn dispatch_smoke(args: SmokeArgs) -> i32 {
    let failure_runtime_root = args.runtime_root.clone();
    dispatch(run_smoke(args), &failure_runtime_root).await
}

async fn dispatch_official_eval(args: OfficialEvalArgs) -> i32 {
    let failure_runtime_root = args.runtime_root.clone();
    dispatch(run_official_eval_subcommand(args), &failure_runtime_root).await
}

async fn dispatch_targeted_test(args: TargetedTestArgs) -> i32 {
    let failure_runtime_root = args.runtime_root.clone();
    dispatch(run_targeted_test_subcommand(args), &failure_runtime_root).await
}

async fn dispatch_replay(args: ReplayArgs) -> i32 {
    let failure_runtime_root = match args
        .output_dir
        .clone()
        .or_else(|| args.attempt_summary_path.parent().map(Path::to_path_buf))
    {
        Some(path) => path,
        None => {
            let err = RealityError::new(
                "RUST_REALITY_REPLAY_FAILURE_ROOT_MISSING",
                "replay could not identify a runtime root for failure artifacts",
                "replay.output_dir",
                "pass --output-dir explicitly",
                json!({ "attempt_summary_path": path_str(&args.attempt_summary_path) }),
                None,
            );
            eprintln!("{}", serde_json::to_string_pretty(&err).unwrap());
            return 1;
        }
    };
    dispatch(run_replay(args), &failure_runtime_root).await
}

async fn dispatch_verify(args: VerifyArgs) -> i32 {
    let failure_runtime_root = args.runtime_root.clone();
    dispatch(run_verify(args), &failure_runtime_root).await
}

async fn run_smoke(args: SmokeArgs) -> Result<Value> {
    let runtime_root = resolve_path(&args.runtime_root)?;
    create_dir_checked(&runtime_root)?;
    let run_id = format!("smoke-{}", unix_secs_for_logging());
    let task_id = format!("cgreality-smoke-{}", safe_id(&args.label));

    if args.force_fail {
        return Err(RealityError::new(
            "RUST_REALITY_SMOKE_FORCED_FAIL",
            "smoke forced to fail via --force-fail",
            "smoke.force_fail",
            "remove --force-fail to verify the happy path",
            json!({ "label": args.label, "runtime_root": path_str(&runtime_root) }),
            Some(file_sot(&runtime_root)),
        ));
    }

    let attempt_dir = runtime_root
        .join(&run_id)
        .join(&task_id)
        .join("smoke-attempt-1");
    create_dir_checked(&attempt_dir)?;

    let problem_path = attempt_dir.join("problem-reality-packet.json");
    let signal_path = attempt_dir.join("reality-signal-packet.json");
    let summary_path = attempt_dir.join("attempt-summary.json");

    let problem = json!({
        "record_kind": "ccreality_smoke_problem_reality",
        "schema_version": 1,
        "task_id": task_id,
        "label": args.label,
        "synthetic": true,
        "expected_outcome": "deterministic smoke artifacts exist and can be replayed",
        "source_of_truth": file_sot(&problem_path)
    });
    write_json_checked(&problem_path, &problem)?;

    let signal = json!({
        "record_kind": "ccreality_smoke_reality_signal",
        "schema_version": 1,
        "task_id": task_id,
        "attempt": 1,
        "score_reality": { "reward": { "clipped": 0.5, "raw": 0.5 } },
        "patch_signals": { "patch_sha256": null, "repeated_patch": false },
        "outcome_reality": { "validation_status": "passed", "source_of_truth": { "official_evidence": null } },
        "reinjection_directive": "smoke synthetic; no real reinjection needed",
        "source_of_truth": file_sot(&signal_path)
    });
    write_json_checked(&signal_path, &signal)?;

    let summary = json!({
        "record_kind": "ccreality_smoke_attempt_summary",
        "schema_version": 1,
        "attempt": 1,
        "task_id": task_id,
        "synthetic": true,
        "validation": { "status": "passed", "synthetic": true },
        "official_evidence": null,
        "reality_signal_packet_path": path_str(&signal_path),
        "problem_reality_packet_path": path_str(&problem_path),
        "source_of_truth": file_sot(&summary_path)
    });
    write_json_checked(&summary_path, &summary)?;
    update_active_runtime_root(&runtime_root, Some(&task_id))?;

    Ok(json!({
        "status": "ok",
        "kind": "smoke",
        "runtime_root": path_str(&runtime_root),
        "run_id": run_id,
        "task_id": task_id,
        "attempt": 1,
        "attempt_summary_path": file_sot(&summary_path),
        "source_of_truth": {
            "problem_reality": file_sot(&problem_path),
            "reality_signal": file_sot(&signal_path),
            "attempt_summary": file_sot(&summary_path)
        }
    }))
}

async fn run_official_eval_subcommand(args: OfficialEvalArgs) -> Result<Value> {
    let runtime_root = resolve_path(&args.runtime_root)?;
    let run_root = runtime_root.join(&args.run_id);
    let attempt_dir = locate_existing_attempt(
        &runtime_root,
        &args.run_id,
        &args.target_instance,
        args.attempt,
    )?;
    let task_manifest_path = run_root.join("task-manifest-readback.json");
    require_file(
        &task_manifest_path,
        "official_eval.task_manifest_readback",
        "RUST_REALITY_OFFICIAL_EVAL_TASK_MANIFEST_MISSING",
    )?;
    let task: TaskManifest = read_json(&task_manifest_path)?;
    if task.instance_id != args.target_instance {
        return Err(RealityError::new(
            "RUST_REALITY_OFFICIAL_EVAL_TARGET_MISMATCH",
            "requested target_instance does not match task-manifest readback",
            "official_eval.target_instance",
            "use the active target instance recorded by the engine for this run",
            json!({
                "requested": args.target_instance,
                "manifest_instance_id": task.instance_id,
                "task_manifest": file_sot(&task_manifest_path)
            }),
            Some(file_sot(&task_manifest_path)),
        ));
    }
    let patch_path = attempt_dir.join("candidate.patch");
    require_file(
        &patch_path,
        "official_eval.candidate_patch",
        "RUST_REALITY_OFFICIAL_EVAL_CANDIDATE_PATCH_MISSING",
    )?;
    let patch = fs::read_to_string(&patch_path).map_err(|err| {
        fs_error(
            "RUST_REALITY_OFFICIAL_EVAL_PATCH_READ_FAILED",
            &patch_path,
            err,
        )
    })?;
    if patch.trim().is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_OFFICIAL_EVAL_CANDIDATE_PATCH_EMPTY",
            "candidate.patch exists but is empty",
            "official_eval.candidate_patch",
            "run official evaluation only after a non-empty candidate patch exists",
            json!({"candidate_patch": file_sot(&patch_path)}),
            Some(file_sot(&patch_path)),
        ));
    }

    let attempt_args = official_eval_attempt_args(&args, &task_manifest_path);
    let official = run_official_eval(&attempt_args, &run_root, &attempt_dir, &task, &patch)?;
    let evidence = build_official_evidence(&official, &task, args.attempt, &patch)?;
    let evidence_path = attempt_dir.join(format!(
        "official-rerun-evidence-{}.json",
        unix_secs_for_logging()
    ));
    write_json_checked(&evidence_path, &evidence)?;
    Ok(json!({
        "kind": "official-eval",
        "status": "ok",
        "run_id": args.run_id,
        "target_instance": args.target_instance,
        "attempt": args.attempt,
        "resolved": evidence.get("resolved").cloned().unwrap_or(Value::Null),
        "candidate_patch_sha256": sha256_file(&patch_path)?,
        "official": official,
        "official_evidence": evidence,
        "official_evidence_sha256": sha256_file(&evidence_path)?,
        "source_of_truth": {
            "candidate_patch": file_sot(&patch_path),
            "official_evidence": file_sot(&evidence_path),
            "attempt_dir": file_sot(&attempt_dir)
        }
    }))
}

async fn run_targeted_test_subcommand(args: TargetedTestArgs) -> Result<Value> {
    let runtime_root = resolve_path(&args.runtime_root)?;
    let run_root = runtime_root.join(&args.run_id);
    let attempt_dir = locate_existing_attempt(
        &runtime_root,
        &args.run_id,
        &args.target_instance,
        args.attempt,
    )?;
    if args.test_id.trim().is_empty() || args.test_id.contains('\n') || args.test_id.contains('\0')
    {
        return Err(RealityError::new(
            "RUST_REALITY_TARGETED_TEST_ID_INVALID",
            "test_id is empty or contains invalid control characters",
            "targeted_test.test_id",
            "use one exact fail_to_pass test id from the task manifest",
            json!({"test_id": args.test_id}),
            None,
        ));
    }
    let task_manifest_path = run_root.join("task-manifest-readback.json");
    require_file(
        &task_manifest_path,
        "targeted_test.task_manifest_readback",
        "RUST_REALITY_TARGETED_TEST_TASK_MANIFEST_MISSING",
    )?;
    let task: TaskManifest = read_json(&task_manifest_path)?;
    if task.instance_id != args.target_instance {
        return Err(RealityError::new(
            "RUST_REALITY_TARGETED_TEST_TARGET_MISMATCH",
            "requested target_instance does not match task-manifest readback",
            "targeted_test.target_instance",
            "use the active target instance recorded by the engine for this run",
            json!({
                "requested": args.target_instance,
                "manifest_instance_id": task.instance_id,
                "task_manifest": file_sot(&task_manifest_path)
            }),
            Some(file_sot(&task_manifest_path)),
        ));
    }
    if !task
        .fail_to_pass
        .iter()
        .any(|test_id| test_id == &args.test_id)
    {
        return Err(RealityError::new(
            "RUST_REALITY_TARGETED_TEST_NOT_IN_ORACLE",
            "test_id is not listed in the task manifest fail_to_pass oracle",
            "targeted_test.test_id",
            "choose one exact fail_to_pass test id from task-manifest-readback.json",
            json!({
                "test_id": args.test_id,
                "fail_to_pass": task.fail_to_pass,
                "task_manifest": file_sot(&task_manifest_path)
            }),
            Some(file_sot(&task_manifest_path)),
        ));
    }
    let repo_dir = runtime_root
        .join("workspaces")
        .join(&args.target_instance)
        .join("repo");
    if !repo_dir.is_dir() {
        return Err(RealityError::new(
            "RUST_REALITY_TARGETED_TEST_REPO_MISSING",
            "target checkout is missing",
            "targeted_test.repo",
            "run a full attempt first so the real repository checkout exists",
            json!({"repo_dir": path_str(&repo_dir)}),
            Some(file_sot(&repo_dir)),
        ));
    }
    let patch_path = attempt_dir.join("candidate.patch");
    require_file(
        &patch_path,
        "targeted_test.candidate_patch",
        "RUST_REALITY_TARGETED_TEST_CANDIDATE_PATCH_MISSING",
    )?;
    let patch = fs::read_to_string(&patch_path).map_err(|err| {
        fs_error(
            "RUST_REALITY_TARGETED_TEST_PATCH_READ_FAILED",
            &patch_path,
            err,
        )
    })?;
    if patch.trim().is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_TARGETED_TEST_CANDIDATE_PATCH_EMPTY",
            "candidate.patch exists but is empty",
            "targeted_test.candidate_patch",
            "run targeted tests only after a non-empty candidate patch exists",
            json!({"candidate_patch": file_sot(&patch_path)}),
            Some(file_sot(&patch_path)),
        ));
    }

    reset_workspace(&repo_dir, &task.base_commit, args.git_timeout)?;
    let apply_check = run_cmd(
        &[
            "git".to_string(),
            "apply".to_string(),
            "--check".to_string(),
            path_str(&patch_path).to_string(),
        ],
        &repo_dir,
        args.git_timeout,
    )?;
    if apply_check.exit_code != 0 {
        return Err(RealityError::new(
            "RUST_REALITY_TARGETED_TEST_PATCH_APPLY_CHECK_FAILED",
            "candidate patch did not apply cleanly to the target checkout",
            "targeted_test.candidate_patch",
            "inspect candidate.patch and regenerate it against the task base commit",
            json!({"process": apply_check, "candidate_patch": file_sot(&patch_path)}),
            Some(file_sot(&patch_path)),
        ));
    }
    let apply = run_cmd(
        &[
            "git".to_string(),
            "apply".to_string(),
            path_str(&patch_path).to_string(),
        ],
        &repo_dir,
        args.git_timeout,
    )?;
    if apply.exit_code != 0 {
        return Err(RealityError::new(
            "RUST_REALITY_TARGETED_TEST_PATCH_APPLY_FAILED",
            "candidate patch apply failed after apply --check passed",
            "targeted_test.candidate_patch",
            "inspect git apply output and repository state",
            json!({"process": apply, "candidate_patch": file_sot(&patch_path)}),
            Some(file_sot(&patch_path)),
        ));
    }
    let process_result = run_cmd(
        &[
            LOCAL_PYTHON_BIN.to_string(),
            "-m".to_string(),
            "pytest".to_string(),
            "-q".to_string(),
            args.test_id.clone(),
        ],
        &repo_dir,
        args.timeout_secs,
    );
    let reset_result = reset_workspace(&repo_dir, &task.base_commit, args.git_timeout);
    let process = match (process_result, reset_result) {
        (Ok(process), Ok(())) => process,
        (Err(process_err), Ok(())) => return Err(process_err),
        (Ok(process), Err(reset_err)) => {
            return Err(RealityError::new(
                "RUST_REALITY_TARGETED_TEST_RESET_FAILED",
                "targeted test completed but workspace reset failed",
                "targeted_test.workspace_reset",
                "inspect the checkout immediately before any further test runs",
                json!({"process": process, "reset_error": reset_err}),
                Some(file_sot(&repo_dir)),
            ))
        }
        (Err(process_err), Err(reset_err)) => {
            return Err(RealityError::new(
                "RUST_REALITY_TARGETED_TEST_AND_RESET_FAILED",
                "targeted test command failed to execute and workspace reset also failed",
                "targeted_test",
                "inspect the checkout and command environment before continuing",
                json!({"process_error": process_err, "reset_error": reset_err}),
                Some(file_sot(&repo_dir)),
            ))
        }
    };

    let artifact = json!({
        "record_kind": "ccreality_targeted_test",
        "schema_version": 1,
        "run_id": args.run_id,
        "target_instance": args.target_instance,
        "attempt": args.attempt,
        "test_id": args.test_id,
        "status": if process.exit_code == 0 { "passed" } else { "failed" },
        "candidate_patch_sha256": sha256_file(&patch_path)?,
        "process": {
            "command": process.command,
            "cwd": process.cwd,
            "exit_code": process.exit_code,
            "elapsed_ms": process.elapsed_ms,
            "stdout_tail": tail_chars(&process.stdout, 6000),
            "stderr_tail": tail_chars(&process.stderr, 6000)
        },
        "source_of_truth": {
            "candidate_patch": file_sot(&patch_path),
            "repo_head_after_reset": format!("git:{}#HEAD", path_str(&repo_dir)),
            "repo_status_after_reset": format!("git:{}#status", path_str(&repo_dir))
        }
    });
    let artifact_path = attempt_dir.join("targeted-tests").join(format!(
        "{}-{}.json",
        safe_id(&args.test_id),
        unix_secs_for_logging()
    ));
    write_json_checked(&artifact_path, &artifact)?;
    Ok(json!({
        "kind": "targeted-test",
        "status": artifact.get("status").cloned().unwrap_or(Value::Null),
        "run_id": artifact.get("run_id").cloned().unwrap_or(Value::Null),
        "target_instance": artifact.get("target_instance").cloned().unwrap_or(Value::Null),
        "attempt": artifact.get("attempt").cloned().unwrap_or(Value::Null),
        "test_id": artifact.get("test_id").cloned().unwrap_or(Value::Null),
        "candidate_patch_sha256": artifact.get("candidate_patch_sha256").cloned().unwrap_or(Value::Null),
        "process": artifact.get("process").cloned().unwrap_or(Value::Null),
        "artifact_sha256": sha256_file(&artifact_path)?,
        "source_of_truth": file_sot(&artifact_path)
    }))
}

async fn run_replay(args: ReplayArgs) -> Result<Value> {
    let summary_path = resolve_path(&args.attempt_summary_path)?;
    require_file(
        &summary_path,
        "attempt_summary_path",
        "RUST_REALITY_REPLAY_SUMMARY_MISSING",
    )?;
    let summary: Value = read_json(&summary_path)?;
    let out_dir = if let Some(out_dir) = args.output_dir.as_deref() {
        resolve_path(out_dir)?
    } else {
        summary_path
            .parent()
            .ok_or_else(|| {
                RealityError::new(
                    "RUST_REALITY_REPLAY_OUTPUT_DIR_MISSING",
                    "attempt-summary path has no parent directory for replay output",
                    "replay.attempt_summary_path",
                    "pass --output-dir explicitly",
                    json!({ "attempt_summary_path": path_str(&summary_path) }),
                    Some(file_sot(&summary_path)),
                )
            })?
            .to_path_buf()
    };
    create_dir_checked(&out_dir)?;
    let out_path = out_dir.join(format!(
        "reality-signal-packet.{}.json",
        safe_id(&args.label)
    ));

    let signal_path_str = summary
        .pointer("/reality_signal_packet_path")
        .or_else(|| summary.pointer("/attempt/reality_signal_packet_path"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_REPLAY_NO_SIGNAL_REF",
                "attempt-summary missing reality_signal_packet_path",
                "attempt.reality_signal_packet_path",
                "use a properly formed attempt-summary",
                json!({ "summary_keys": summary.as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()) }),
                Some(file_sot(&summary_path)),
            )
        })?;
    let signal_path = file_sot_to_path(signal_path_str, "", "attempt.reality_signal_packet_path")
        .unwrap_or_else(|_| PathBuf::from(signal_path_str));
    let signal: Value = read_json(&signal_path)?;
    let replayed = json!({
        "record_kind": "ccreality_replay",
        "schema_version": 1,
        "source_attempt_summary": file_sot(&summary_path),
        "label": args.label,
        "replayed_at_unix": unix_secs_for_logging(),
        "signal": signal,
        "source_of_truth": file_sot(&out_path)
    });
    write_json_checked(&out_path, &replayed)?;
    Ok(json!({
        "status": "ok",
        "kind": "replay",
        "output_path": file_sot(&out_path),
        "source_of_truth": file_sot(&out_path)
    }))
}

async fn run_verify(args: VerifyArgs) -> Result<Value> {
    let runtime_root = resolve_path(&args.runtime_root)?;
    if !runtime_root.is_dir() {
        return Err(RealityError::new(
            "RUST_REALITY_VERIFY_RUNTIME_ROOT_MISSING",
            "runtime root is not a directory",
            "verify.runtime_root",
            "pass an existing runtime root",
            json!({ "runtime_root": path_str(&runtime_root) }),
            Some(file_sot(&runtime_root)),
        ));
    }
    let mut json_files = Vec::new();
    collect_json_files(&runtime_root, &mut json_files)?;
    json_files.sort();
    let mut entries = Vec::new();
    let mut drift = Vec::new();
    for path in &json_files {
        if let Some(attempt) = args.attempt {
            let marker = format!("attempt-{attempt}");
            let padded = format!("attempt-{attempt:06}");
            let smoke = format!("smoke-attempt-{attempt}");
            let path_text = path_str(path);
            if !path_text.contains(&marker)
                && !path_text.contains(&padded)
                && !path_text.contains(&smoke)
            {
                continue;
            }
        }
        let sha = sha256_file(path)?;
        let size = fs::metadata(path)
            .map_err(|err| fs_error("RUST_REALITY_VERIFY_METADATA_FAILED", path, err))?
            .len();
        let sha_sidecar = path.with_extension("json.sha256");
        let expected_sidecar = if sha_sidecar.is_file() {
            Some(fs::read_to_string(&sha_sidecar).map_err(|err| {
                fs_error(
                    "RUST_REALITY_VERIFY_SHA_SIDECAR_READ_FAILED",
                    &sha_sidecar,
                    err,
                )
            })?)
        } else {
            None
        };
        if let Some(expected) = expected_sidecar.as_deref() {
            let expected = expected.trim();
            if expected != sha {
                drift.push(json!({
                    "path": path_str(path),
                    "expected_sha256": expected,
                    "actual_sha256": sha
                }));
            }
        }
        entries.push(json!({
            "path": path_str(path),
            "sha256": sha,
            "size_bytes": size,
            "sha256_sidecar": expected_sidecar.map(|_| file_sot(&sha_sidecar))
        }));
    }
    Ok(json!({
        "status": if drift.is_empty() { "ok" } else { "drift_detected" },
        "kind": "verify",
        "runtime_root": path_str(&runtime_root),
        "attempt_filter": args.attempt,
        "json_artifact_count": entries.len(),
        "drift_count": drift.len(),
        "drift": drift,
        "artifacts": entries,
        "source_of_truth": file_sot(&runtime_root)
    }))
}

fn official_eval_attempt_args(args: &OfficialEvalArgs, task_manifest: &Path) -> AttemptArgs {
    AttemptArgs {
        model: args.model.clone(),
        gemma_model_path: PathBuf::from(DEFAULT_GEMMA_MODEL_PATH),
        gemma_chat_template: None,
        task_manifest: task_manifest.to_path_buf(),
        runtime_root: args.runtime_root.clone(),
        context_graph_bin: PathBuf::from(DEFAULT_CONTEXT_GRAPH_BIN),
        max_attempts: 1,
        run_until_resolved: false,
        smoke_test: false,
        official_eval_each_attempt: true,
        swebench_python: args.swebench_python.clone(),
        official_timeout: args.official_timeout,
        official_max_workers: args.official_max_workers,
        official_cache_level: args.official_cache_level.clone(),
        official_docker_host: args.official_docker_host.clone(),
        official_docker_config: args.official_docker_config.clone(),
        gpu_index: 0,
        gpu_min_free_mb: DEFAULT_GPU_MIN_FREE_MB,
        gpu_max_used_mb: None,
        gemma_context_tokens: None,
        gemma_max_new_tokens: DEFAULT_GEMMA_MAX_NEW_TOKENS,
        gemma_temperature: DEFAULT_GEMMA_TEMPERATURE,
        gemma_top_p: DEFAULT_GEMMA_TOP_P,
        gemma_gpu_layers: u32::MAX,
        gemma_batch_size: DEFAULT_GEMMA_BATCH_SIZE,
        gemma_seed: DEFAULT_GEMMA_SEED,
        git_timeout: 600,
        agent_tool_rounds: DEFAULT_AGENT_TOOL_ROUNDS,
        targeted_test_timeout: 300,
        sleep_between_attempts_ms: 0,
    }
}

fn locate_existing_attempt(
    runtime_root: &Path,
    run_id: &str,
    target_instance: &str,
    attempt: u64,
) -> Result<PathBuf> {
    let run_root = runtime_root.join(run_id);
    if !run_root.is_dir() {
        return Err(RealityError::new(
            "RUST_REALITY_RUN_ROOT_MISSING",
            "run root does not exist",
            "run_id",
            "verify run_id and runtime_root against reality_latest_root",
            json!({"runtime_root": path_str(runtime_root), "run_id": run_id}),
            Some(file_sot(&run_root)),
        ));
    }

    let direct = run_root
        .join(target_instance)
        .join(format!("attempt-{attempt}"));
    if direct.is_dir() {
        return Ok(direct);
    }
    let smoke = run_root
        .join(target_instance)
        .join(format!("smoke-attempt-{attempt}"));
    if smoke.is_dir() {
        return Ok(smoke);
    }

    let mut matches = Vec::new();
    collect_attempt_dirs_for_run(&run_root, target_instance, attempt, &mut matches)?;
    matches.sort();
    matches.into_iter().next().ok_or_else(|| {
        RealityError::new(
            "RUST_REALITY_ATTEMPT_DIR_MISSING",
            "could not locate requested attempt directory",
            "attempt",
            "verify run_id, target_instance, and attempt number against runtime artifacts",
            json!({
                "runtime_root": path_str(runtime_root),
                "run_id": run_id,
                "target_instance": target_instance,
                "attempt": attempt
            }),
            Some(file_sot(&run_root)),
        )
    })
}

fn collect_attempt_dirs_for_run(
    root: &Path,
    target_instance: &str,
    attempt: u64,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)
        .map_err(|err| fs_error("RUST_REALITY_ATTEMPT_WALK_READ_FAILED", root, err))?
    {
        let entry =
            entry.map_err(|err| fs_error("RUST_REALITY_ATTEMPT_WALK_ENTRY_FAILED", root, err))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let parsed = name
            .strip_prefix("attempt-")
            .or_else(|| name.strip_prefix("smoke-attempt-"))
            .and_then(|suffix| suffix.parse::<u64>().ok());
        if parsed == Some(attempt) && path.display().to_string().contains(target_instance) {
            out.push(path);
            continue;
        }
        collect_attempt_dirs_for_run(&path, target_instance, attempt, out)?;
    }
    Ok(())
}

fn validate_gemma_harness_args(args: &AttemptArgs) -> Result<()> {
    if args.gemma_context_tokens == Some(0) {
        return Err(RealityError::new(
            "RUST_REALITY_GEMMA_CONTEXT_TOKENS_INVALID",
            "--gemma-context-tokens must be greater than zero",
            "gemma_context_tokens",
            "omit --gemma-context-tokens to use the GGUF training context, or set a positive explicit context window",
            json!({"gemma_context_tokens": args.gemma_context_tokens}),
            None,
        ));
    }
    if args.gemma_max_new_tokens == 0 {
        return Err(RealityError::new(
            "RUST_REALITY_GEMMA_MAX_NEW_TOKENS_INVALID",
            "--gemma-max-new-tokens must be greater than zero",
            "gemma_max_new_tokens",
            "set a positive generation budget so the harness can emit a tool call",
            json!({"gemma_max_new_tokens": args.gemma_max_new_tokens}),
            None,
        ));
    }
    if args.gemma_batch_size == 0 {
        return Err(RealityError::new(
            "RUST_REALITY_GEMMA_BATCH_SIZE_INVALID",
            "--gemma-batch-size must be greater than zero",
            "gemma_batch_size",
            "set a positive llama.cpp prompt batch size",
            json!({"gemma_batch_size": args.gemma_batch_size}),
            None,
        ));
    }
    if args.gemma_batch_size > u32::MAX as usize {
        return Err(RealityError::new(
            "RUST_REALITY_GEMMA_BATCH_SIZE_TOO_LARGE",
            "--gemma-batch-size exceeds llama.cpp u32 limits",
            "gemma_batch_size",
            "set a positive batch size no larger than u32::MAX",
            json!({"gemma_batch_size": args.gemma_batch_size, "max": u32::MAX}),
            None,
        ));
    }
    if !args.gemma_temperature.is_finite() || !(0.0..=2.0).contains(&args.gemma_temperature) {
        return Err(RealityError::new(
            "RUST_REALITY_GEMMA_TEMPERATURE_INVALID",
            "--gemma-temperature must be a finite value between 0.0 and 2.0",
            "gemma_temperature",
            "use deterministic 0.0 for SWE repair unless intentionally experimenting",
            json!({"gemma_temperature": args.gemma_temperature}),
            None,
        ));
    }
    if !args.gemma_top_p.is_finite() || args.gemma_top_p <= 0.0 || args.gemma_top_p > 1.0 {
        return Err(RealityError::new(
            "RUST_REALITY_GEMMA_TOP_P_INVALID",
            "--gemma-top-p must be a finite value in (0.0, 1.0]",
            "gemma_top_p",
            "use 0.9 for bounded sampling or 1.0 to disable nucleus pruning",
            json!({"gemma_top_p": args.gemma_top_p}),
            None,
        ));
    }
    Ok(())
}

async fn run(args: AttemptArgs) -> Result<Value> {
    let runtime_root = resolve_path(&args.runtime_root)?;
    create_dir_checked(&runtime_root)?;
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
    if args.smoke_test && args.run_until_resolved {
        return Err(RealityError::new(
            "RUST_REALITY_SMOKE_RUN_UNTIL_RESOLVED_INVALID",
            "--smoke-test cannot be combined with --run-until-resolved",
            "smoke_test",
            "run a one-attempt smoke first, then launch the continuous loop separately",
            json!({"smoke_test": true, "run_until_resolved": true}),
            None,
        ));
    }
    if args.smoke_test && args.max_attempts != 1 {
        return Err(RealityError::new(
            "RUST_REALITY_SMOKE_MAX_ATTEMPTS_INVALID",
            "--smoke-test requires --max-attempts 1",
            "max_attempts",
            "keep smoke testing bounded to one attempt so failures are fast and inspectable",
            json!({"smoke_test": true, "max_attempts": args.max_attempts}),
            None,
        ));
    }
    validate_gemma_harness_args(&args)?;
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
    update_active_runtime_root(&runtime_root, Some(&task.instance_id))?;
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
            "smoke_test": args.smoke_test,
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
    let initial_repo_context = collect_repository_context(&workspace, &task)?;
    let problem_reality_packet = build_problem_reality_packet(
        &task,
        &workspace,
        &initial_repo_context,
        &task_manifest_path,
    )?;
    let problem_reality_path = run_root.join("problem-reality-packet.json");
    write_json_checked(&problem_reality_path, &problem_reality_packet)?;
    append_event(
        &reality_events,
        json!({
            "event_type": "problem_reality_packet_materialized",
            "status": "ok",
            "problem_reality": file_sot(&problem_reality_path),
            "problem_reality_sha256": sha256_file(&problem_reality_path)?,
            "source_of_truth": file_sot(&problem_reality_path)
        }),
    )?;
    let gemma_harness = GemmaCodingHarness::load(&args)?;
    let gemma_model_readback_path = run_root.join("gemma-harness-model-readback.json");
    let gemma_model_readback = gemma_harness.model_capability_report(&gemma_model_readback_path)?;
    write_json_checked(&gemma_model_readback_path, &gemma_model_readback)?;
    append_event(
        &reality_events,
        json!({
            "event_type": "gemma_harness_model_capability_readback",
            "status": "ok",
            "model": args.model,
            "model_path": file_sot(&gemma_harness.model_path),
            "effective_context_tokens": gemma_harness.context_tokens,
            "max_new_tokens": gemma_harness.max_new_tokens,
            "source_of_truth": file_sot(&gemma_model_readback_path),
            "sha256": sha256_file(&gemma_model_readback_path)?
        }),
    )?;
    let mut previous_patch_hashes: Vec<String> = Vec::new();
    let mut attempts: Vec<AttemptRecord> = Vec::new();
    let mut solved = false;
    let mut attempt = 1_u64;
    while args.run_until_resolved || attempt <= args.max_attempts {
        let attempt_dir = attempts_root.join(format!("attempt-{attempt:06}"));
        create_dir_checked(&attempt_dir)?;
        reset_workspace(&workspace, &task.base_commit, args.git_timeout)?;
        let repo_context = collect_repository_context(&workspace, &task)?;
        write_json_checked(&attempt_dir.join("repository-context.json"), &repo_context)?;
        let prompt = compile_prompt(
            &task,
            &repo_context,
            &problem_reality_packet,
            &attempts,
            &previous_patch_hashes,
        );
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

        let tool_context = AgentToolContext {
            args: &args,
            task: &task,
            context_graph_bin: &context_graph_bin,
            ledger: &ledger,
            run_id: &run_id,
            task_root: &task_root,
            repo_dir: &workspace,
            attempt_dir: &attempt_dir,
            repo_context: &repo_context,
            problem_reality: &problem_reality_packet,
            attempts: &attempts,
        };
        let model_response =
            gemma_coding_harness_chat_json(&gemma_harness, &tool_context, &prompt, attempt)?;
        let response_text = model_response["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let response_path = attempt_dir.join("model-response.txt");
        write_text_checked(&response_path, &response_text)?;
        let (mut patch, mut parse_report) = if model_response.get("status").and_then(Value::as_str)
            == Some("blocked")
        {
            (
                None,
                json!({
                    "status": "failed",
                    "error_code": model_response.get("error_code").and_then(Value::as_str).unwrap_or("RUST_REALITY_MODEL_STEP_BLOCKED"),
                    "field_path": model_response.get("field_path").and_then(Value::as_str).unwrap_or("model_response"),
                    "message": model_response.get("message_text").and_then(Value::as_str).unwrap_or("model step was blocked before final patch generation"),
                    "remediation": model_response.get("remediation").and_then(Value::as_str).unwrap_or("reinject the blocked-step evidence and require the model to satisfy the governed tool contract"),
                    "details": {
                        "blocked_model_response": model_response
                    },
                    "source_of_truth": model_response.get("source_of_truth")
                }),
            )
        } else if let Some((submitted_patch, submitted_report)) =
            patch_from_tool_submission(&model_response)?
        {
            (Some(submitted_patch), submitted_report)
        } else {
            parse_patch_report(&response_text)
        };
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

        // Phase 14 (forward) intervention #4: capture BlackBox score + reward
        // compilation errors as evidence rather than letting `?` propagate them up
        // and terminate the attempt before write_summary / write_optimizer_artifacts /
        // write_training_export run. Diagnosed from
        // /tmp/ccreality-prod-3/rust_reality_1778039692404664879_e525987d where
        // Docker eval completed cleanly (completed=true, resolved=false, patch
        // applied, all 8 FAIL_TO_PASS still failing) but reward compilation
        // returned RUST_REALITY_BLACKBOX_COMMAND_FAILED with stderr
        // "feature correctness is null because source /score/correctness had no
        // measurable value". The attempt aborted, attempt-summary.json was never
        // written, Phase 15 + Phase 18 artifacts were lost. With this lift, score
        // and reward errors become first-class fields in the attempt-summary so
        // the optimizer can react to them and the autoresearch / training writers
        // still fire.
        let score = match blackbox(
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
        ) {
            Ok(v) => v,
            Err(err) => json!({
                "status": "error",
                "error_code": err.error_code.clone(),
                "field_path": err.field_path.clone(),
                "message": err.message.clone(),
                "remediation": err.remediation.clone(),
                "details": err.details.clone(),
                "source_of_truth": err.source_of_truth.clone(),
            }),
        };
        let reward_spec =
            resolve_path(Path::new("gemmafrontend/reward_spec.verified_outcome.json"))?;
        let reward = if reward_spec.exists() {
            Some(
                match blackbox(
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
                ) {
                    Ok(v) => v,
                    Err(err) => json!({
                        "status": "error",
                        "error_code": err.error_code.clone(),
                        "field_path": err.field_path.clone(),
                        "message": err.message.clone(),
                        "remediation": err.remediation.clone(),
                        "details": err.details.clone(),
                        "source_of_truth": err.source_of_truth.clone(),
                    }),
                },
            )
        } else {
            None
        };
        let agent_tool_transcript_path = model_response
            .pointer("/contextgraph_runtime/agent_tool_transcript")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let ledger_counts_now = ledger_counts(&ledger)?;
        let reality_signal_packet = build_reality_signal_packet(
            attempt,
            &task,
            &problem_reality_packet,
            &validation,
            official_evidence_path.as_deref(),
            official_reality.as_ref(),
            patch.as_deref(),
            patch_sha.as_deref(),
            repeated_patch,
            &model_response,
            agent_tool_transcript_path.as_deref(),
            &score,
            reward.as_ref(),
            &ledger_counts_now,
            &attempt_dir,
        )?;
        let reality_signal_packet_path = attempt_dir.join("reality-signal-packet.json");
        write_json_checked(&reality_signal_packet_path, &reality_signal_packet)?;
        let reality_signal_compact = compact_reality_signal_packet(&reality_signal_packet);
        let reality_signal_verifier = verify_reality_signal_packet(
            &context_graph_bin,
            &ledger,
            &run_id,
            &task_root,
            &attempt_dir,
            &reality_signal_packet_path,
            &args.model,
        )?;

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
            reality_signal_packet_path: path_str(&reality_signal_packet_path).to_string(),
            reality_signal_compact,
            agent_tool_transcript_path,
            patch_preview: patch.as_ref().map(|text| truncate_chars(text, 1600)),
        };
        write_json_checked(
            &attempt_dir.join("attempt-summary.json"),
            &json!({
                "attempt": attempt_record,
                "model_response": model_response,
                "blackbox_verifier": verifier,
                "reality_signal_verifier": reality_signal_verifier,
                "score": score,
                "reward": reward,
                "ledger_counts": ledger_counts(&ledger)?,
            }),
        )?;
        // Phase 15: emit autoresearch engine artifacts under <run_root>/reality-optimizer/
        write_optimizer_artifacts(
            &run_root,
            &args.model,
            &task,
            &attempt_record,
            reward.as_ref(),
            &attempts,
        )?;
        // Phase 18: emit verified-episode training samples under <run_root>/training/
        write_training_export(
            &run_root,
            &args.model,
            &task,
            &attempt_record,
            reward.as_ref(),
            &attempts,
            &attempt_dir,
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
            &problem_reality_path,
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
                &problem_reality_path,
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
        &problem_reality_path,
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
    args: &AttemptArgs,
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
            "model_runtime": {
                "provider": "contextgraph_rust_gemma_coding_harness",
                "model_label": args.model,
                "gemma_model_path": file_sot(&resolve_path(&args.gemma_model_path)?),
                "chat_template": args.gemma_chat_template.as_deref(),
                "context_policy": if args.gemma_context_tokens.is_some() { "explicit_cli" } else { "gguf_n_ctx_train_max" },
                "requested_context_tokens": args.gemma_context_tokens,
                "minimum_context_tokens": DEFAULT_GEMMA_MIN_CONTEXT_TOKENS,
                "max_new_tokens": args.gemma_max_new_tokens,
                "temperature": args.gemma_temperature,
                "top_p": args.gemma_top_p,
                "gpu_layers": args.gemma_gpu_layers,
                "batch_size": args.gemma_batch_size,
                "seed": args.gemma_seed
            },
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

fn build_problem_reality_packet(
    task: &TaskManifest,
    repo_dir: &Path,
    repo_context: &Value,
    manifest_path: &Path,
) -> Result<Value> {
    let head = run_required(
        &["git", "rev-parse", "HEAD"],
        repo_dir,
        120,
        "RUST_REALITY_PROBLEM_HEAD_READ_FAILED",
        "problem_reality.git_head",
    )?;
    let actual_head = head.stdout.trim();
    if actual_head != task.base_commit {
        return Err(RealityError::new(
            "RUST_REALITY_PROBLEM_BASE_COMMIT_MISMATCH",
            "problem reality capture saw a git HEAD that does not match the task base commit",
            "problem_reality.git_head",
            "reset the workspace to the task base commit before capturing problem reality",
            json!({"expected": task.base_commit, "actual": actual_head}),
            Some(format!("git:{}#HEAD", path_str(repo_dir))),
        ));
    }
    let status = run_required(
        &["git", "status", "--porcelain"],
        repo_dir,
        120,
        "RUST_REALITY_PROBLEM_STATUS_READ_FAILED",
        "problem_reality.git_status",
    )?;
    if !status.stdout.trim().is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_PROBLEM_WORKSPACE_DIRTY",
            "problem reality must be captured from a clean base checkout",
            "problem_reality.git_status",
            "reset and clean the workspace before continuing",
            json!({"git_status": status.stdout}),
            Some(format!("git:{}#status", path_str(repo_dir))),
        ));
    }
    let files_out = run_required(
        &["git", "ls-files"],
        repo_dir,
        120,
        "RUST_REALITY_PROBLEM_GIT_LS_FILES_FAILED",
        "problem_reality.git_ls_files",
    )?;
    let all_files: Vec<String> = files_out.stdout.lines().map(ToOwned::to_owned).collect();
    let terms = task_terms(task);
    let symbols = task_symbols(&terms);
    let fail_to_pass: Vec<Value> = task
        .fail_to_pass
        .iter()
        .map(|test_id| test_target_reality(test_id))
        .collect();
    let production_candidates = problem_production_candidates(task, repo_context, &all_files);
    let issue_file_mentions = path_candidates(task);
    Ok(json!({
        "record_kind": "contextgraph_swe_problem_reality_packet",
        "schema_version": 1,
        "claim_scope": "problem_reality_for_official_swe_instance",
        "task_identity": {
            "dataset": task.dataset,
            "instance_id": task.instance_id,
            "repo": task.repo,
            "base_commit": task.base_commit,
            "source_of_truth": task.source_of_truth
        },
        "oracle_reality": {
            "success_predicate": "official_report.resolved_ids contains task.instance_id",
            "fail_to_pass_total": task.fail_to_pass.len(),
            "fail_to_pass": fail_to_pass,
            "pass_to_pass_count": task.pass_to_pass_count,
            "official_test_patch_sha256": task.official_test_patch_sha256,
            "oracle_patch_sha256": task.oracle_patch_sha256
        },
        "problem_statement_reality": {
            "sha256": sha256_text(&task.problem_statement),
            "terms": terms,
            "symbols": symbols,
            "mentioned_paths": issue_file_mentions,
            "preview": truncate_chars(&task.problem_statement, 4000)
        },
        "repository_reality": {
            "head": actual_head,
            "status": "clean",
            "file_inventory": repository_file_inventory(&all_files),
            "selected_context_files": repo_context.get("files").cloned().unwrap_or_else(|| json!([])),
            "production_candidate_files": production_candidates,
            "source_of_truth": {
                "head": format!("git:{}#HEAD", path_str(repo_dir)),
                "status": format!("git:{}#status", path_str(repo_dir)),
                "files": format!("git:{}#ls-files", path_str(repo_dir)),
                "selected_context": repo_context.get("source_of_truth")
            }
        },
        "root_cause_discovery_protocol": {
            "required_model_questions": [
                "Which FAIL_TO_PASS behavior is actually failing?",
                "Which production file implements that behavior?",
                "Which exact source span controls the failing behavior?",
                "What minimal production edit changes that behavior without editing tests?",
                "Which validation readback will prove the edit affected the intended reality?",
                "Which evidence would falsify the proposed root cause?",
                "Which existing behavior must remain unchanged?"
            ],
            "required_tools_before_edit": [
                "read_problem_reality",
                "search_repo or inspect_symbol",
                "open_file_window"
            ],
            "edit_grounding_rule": "changed production files should have been read through ContextGraph tools or selected repository context before final edit"
        },
        "measurement_boundary": {
            "what_this_packet_proves": [
                "the task identity, base commit, official oracle targets, problem terms, and repository context were read from durable sources",
                "the model can be told the root-cause discovery surface before it edits"
            ],
            "non_claims": [
                "this packet alone does not prove the root cause",
                "official SWE-bench resolved_ids remains the only success proof",
                "human intent beyond the SWE-bench oracle remains outside the official success claim"
            ],
            "known_blindspots_now_measured_explicitly": [
                "dynamic coverage is unknown until targeted or official tests run",
                "problem-statement ambiguity is represented as non-claim instead of hidden success",
                "test oracle narrowness is visible through FAIL_TO_PASS/PASS_TO_PASS metadata"
            ],
            "additional_reality_signals_to_reduce_before_confidence": [
                "pre-patch failing-test stdout/stderr and stack location",
                "dynamic coverage or execution trace for touched source spans",
                "dependency and runtime environment versions used by local reproduction and official Docker evaluation",
                "minimal counterexample that distinguishes the bug from neighboring behavior",
                "negative invariants that prove the patch did not only satisfy the metric while breaking intent",
                "flakiness and retry reality for nondeterministic failures",
                "performance, memory, security, and public-API side effects when the task surface implies them",
                "similar verified episodes and failed patch patterns from ContextGraph memory"
            ]
        },
        "source_of_truth": {
            "task_manifest": file_sot(manifest_path),
            "repo": format!("git:{}#{}", path_str(repo_dir), task.base_commit),
            "repo_context": repo_context.get("source_of_truth")
        }
    }))
}

fn compact_problem_reality_packet(packet: &Value) -> Value {
    json!({
        "record_kind": packet.get("record_kind"),
        "task_identity": packet.get("task_identity"),
        "oracle_reality": packet.get("oracle_reality"),
        "problem_terms": packet.pointer("/problem_statement_reality/terms"),
        "problem_symbols": packet.pointer("/problem_statement_reality/symbols"),
        "issue_mentioned_paths": packet.pointer("/problem_statement_reality/mentioned_paths"),
        "production_candidate_files": packet.pointer("/repository_reality/production_candidate_files"),
        "selected_context_files": packet.pointer("/repository_reality/selected_context_files"),
        "root_cause_discovery_protocol": packet.get("root_cause_discovery_protocol"),
        "measurement_boundary": packet.get("measurement_boundary"),
        "source_of_truth": packet.get("source_of_truth")
    })
}

fn test_target_reality(test_id: &str) -> Value {
    let mut parts = test_id.split("::");
    let path = parts.next().unwrap_or(test_id).to_string();
    let remainder: Vec<String> = parts.map(ToOwned::to_owned).collect();
    json!({
        "test_id": test_id,
        "path": path,
        "node_parts": remainder,
        "source_of_truth": "task_manifest.fail_to_pass"
    })
}

fn repository_file_inventory(files: &[String]) -> Value {
    let mut extensions: BTreeMap<String, u64> = BTreeMap::new();
    let mut top_dirs: BTreeMap<String, u64> = BTreeMap::new();
    for file in files {
        let ext = Path::new(file)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("[no_extension]")
            .to_string();
        *extensions.entry(ext).or_insert(0) += 1;
        let top = file.split('/').next().unwrap_or("[root]").to_string();
        *top_dirs.entry(top).or_insert(0) += 1;
    }
    json!({
        "tracked_files_total": files.len(),
        "extensions": extensions,
        "top_dirs": top_dirs
    })
}

fn problem_production_candidates(
    task: &TaskManifest,
    repo_context: &Value,
    all_files: &[String],
) -> Value {
    // Issue-text-mention priority: ordering preserves the precision signal that the issue
    // text directly named a file (per arxiv 2512.07022 file-localization findings).
    // Priority 0 = paths produced by production_path_candidates (issue-text mention order).
    // Priority 1 = paths returned by repo_context.files (BM25-ranked by reality engine).
    let all_file_set: BTreeSet<String> = all_files.iter().cloned().collect();
    let mut ordered: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for candidate in production_path_candidates(task) {
        if all_file_set.contains(&candidate)
            && is_context_file(&candidate)
            && !is_low_value_context_path(&candidate)
            && seen.insert(candidate.clone())
        {
            ordered.push(candidate);
        }
    }
    if let Some(files) = repo_context.get("files").and_then(Value::as_array) {
        for file in files {
            if let Some(path) = file.get("path").and_then(Value::as_str) {
                if is_context_file(path)
                    && !is_low_value_context_path(path)
                    && seen.insert(path.to_string())
                {
                    ordered.push(path.to_string());
                }
            }
        }
    }
    json!(ordered)
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
    let instance_payload = evidence
        .pointer("/instance_report/payload")
        .unwrap_or(&Value::Null);
    let instance_report = evidence
        .get("instance_id")
        .and_then(Value::as_str)
        .and_then(|instance_id| instance_payload.get(instance_id))
        .unwrap_or(instance_payload);
    let fail_to_pass = instance_report
        .pointer("/tests_status/FAIL_TO_PASS")
        .unwrap_or(&Value::Null);
    let pass_to_pass = instance_report
        .pointer("/tests_status/PASS_TO_PASS")
        .unwrap_or(&Value::Null);
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
        "patch_successfully_applied": instance_report.get("patch_successfully_applied"),
        "fail_to_pass": {
            "success_count": fail_to_pass.get("success").and_then(Value::as_array).map(|items| items.len()),
            "failure_count": fail_to_pass.get("failure").and_then(Value::as_array).map(|items| items.len()),
            "success": fail_to_pass.get("success"),
            "failure": fail_to_pass.get("failure")
        },
        "pass_to_pass": {
            "success_count": pass_to_pass.get("success").and_then(Value::as_array).map(|items| items.len()),
            "failure_count": pass_to_pass.get("failure").and_then(Value::as_array).map(|items| items.len()),
            "failure": pass_to_pass.get("failure")
        },
        "test_output_tail": evidence.pointer("/test_output_tail/tail"),
        "previous_patch_sha256": evidence.get("previous_patch_sha256"),
        "source_of_truth": evidence.get("source_of_truth"),
    })
}

fn write_failure_artifact_for_command(
    runtime_root: &Path,
    err: &RealityError,
) -> std::result::Result<PathBuf, String> {
    if runtime_root.as_os_str().is_empty() {
        return Err("runtime_root required for failure artifact".to_string());
    }
    let runtime_root = if runtime_root.is_absolute() {
        runtime_root.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|cwd_err| cwd_err.to_string())?
            .join(runtime_root)
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
        "error": err,
        "source_of_truth": file_sot(&path)
    });
    let text = serde_json::to_string_pretty(&payload).map_err(|json_err| json_err.to_string())?;
    let mut file = fs::File::create(&path).map_err(|io_err| io_err.to_string())?;
    file.write_all(text.as_bytes())
        .map_err(|io_err| io_err.to_string())?;
    file.sync_all().map_err(|io_err| io_err.to_string())?;
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
    let output_dir = std::env::temp_dir().join(format!(
        "contextgraph-run-cmd-{}-{}",
        std::process::id(),
        unix_ns()?
    ));
    create_dir_checked(&output_dir)?;
    let stdout_path = output_dir.join("stdout");
    let stderr_path = output_dir.join("stderr");
    let stdout_file = fs::File::create(&stdout_path).map_err(|err| {
        fs_error(
            "RUST_REALITY_PROCESS_STDOUT_FILE_CREATE_FAILED",
            &stdout_path,
            err,
        )
    })?;
    let stderr_file = fs::File::create(&stderr_path).map_err(|err| {
        fs_error(
            "RUST_REALITY_PROCESS_STDERR_FILE_CREATE_FAILED",
            &stderr_path,
            err,
        )
    })?;
    let mut process = Command::new(&command[0]);
    process
        .args(&command[1..])
        .current_dir(cwd)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));
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
            let status = child.wait().map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_PROCESS_WAIT_FAILED",
                    "failed while finalizing child process wait",
                    "process.wait",
                    "inspect process state",
                    json!({ "command": command, "error": err.to_string() }),
                    None,
                )
            })?;
            let stdout = read_process_output(&stdout_path)?;
            let stderr = read_process_output(&stderr_path)?;
            let _ = fs::remove_dir_all(&output_dir);
            return Ok(ProcessResult {
                command: command.to_vec(),
                cwd: path_str(cwd).to_string(),
                exit_code: status.code().unwrap_or(128),
                stdout,
                stderr,
                elapsed_ms: started.elapsed().as_millis(),
            });
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            let stdout = read_process_output(&stdout_path).unwrap_or_default();
            let stderr = read_process_output(&stderr_path).unwrap_or_default();
            let _ = fs::remove_dir_all(&output_dir);
            return Ok(ProcessResult {
                command: command.to_vec(),
                cwd: path_str(cwd).to_string(),
                exit_code: 124,
                stdout,
                stderr: if stderr.is_empty() {
                    format!("TIMEOUT after {timeout_secs}s")
                } else {
                    format!("{stderr}\nTIMEOUT after {timeout_secs}s")
                },
                elapsed_ms: started.elapsed().as_millis(),
            });
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn read_process_output(path: &Path) -> Result<String> {
    let bytes = fs::read(path)
        .map_err(|err| fs_error("RUST_REALITY_PROCESS_OUTPUT_READ_FAILED", path, err))?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
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

struct GemmaCodingHarness {
    backend: LlamaBackend,
    model: LlamaModel,
    model_path: PathBuf,
    chat_template: LlamaChatTemplate,
    requested_context_tokens: Option<u32>,
    model_context_train_tokens: u32,
    context_tokens: u32,
    max_new_tokens: usize,
    temperature: f32,
    top_p: f32,
    batch_size: usize,
    seed: u32,
}

fn gemma_model_identity_readback(
    model: &LlamaModel,
    model_path: &Path,
) -> BTreeMap<String, String> {
    let mut selected_metadata = BTreeMap::new();
    if let Some(file_name) = model_path.file_name().and_then(|name| name.to_str()) {
        selected_metadata.insert(
            "contextgraph.model_path_file_name".to_string(),
            file_name.to_string(),
        );
    }
    for key in [
        "general.architecture",
        "general.name",
        "general.basename",
        "general.finetune",
        "general.size_label",
        "general.type",
        "llama.context_length",
        "gemma.context_length",
        "gemma3.context_length",
        "gemma4.context_length",
        "tokenizer.chat_template",
    ] {
        if let Ok(value) = model.meta_val_str(key) {
            selected_metadata.insert(key.to_string(), value);
        }
    }
    selected_metadata
}

fn validate_gemma4_model_identity(
    model_path: &Path,
    identity: &BTreeMap<String, String>,
) -> Result<()> {
    let identity_text = identity
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("\n")
        .to_ascii_lowercase();
    let compact_identity = identity_text
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>();
    if !compact_identity.contains("gemma4") {
        return Err(RealityError::new(
            "RUST_REALITY_GEMMA_MODEL_IDENTITY_MISMATCH",
            "loaded GGUF does not identify as Gemma 4",
            "gemma_harness.model.metadata",
            "install the intended Gemma 4 GGUF; the SWE harness will not run a substitute model under a Gemma label",
            json!({
                "required_identity": "gemma4",
                "identity_readback": identity,
                "model_path": file_sot(model_path)
            }),
            Some(file_sot(model_path)),
        ));
    }
    Ok(())
}

impl GemmaCodingHarness {
    fn load(args: &AttemptArgs) -> Result<Self> {
        let model_path = resolve_path(&args.gemma_model_path)?;
        require_file(
            &model_path,
            "gemma_model_path",
            "RUST_REALITY_GEMMA_MODEL_PATH_MISSING",
        )?;
        let backend = LlamaBackend::init().map_err(|err| {
            RealityError::new(
                "RUST_REALITY_GEMMA_BACKEND_INIT_FAILED",
                "failed to initialize the Rust llama.cpp backend",
                "gemma_harness.backend",
                "inspect CUDA/llama.cpp runtime initialization before retrying",
                json!({"error": err.to_string(), "model_path": file_sot(&model_path)}),
                Some(file_sot(&model_path)),
            )
        })?;
        let model_params = LlamaModelParams::default().with_n_gpu_layers(args.gemma_gpu_layers);
        let model =
            LlamaModel::load_from_file(&backend, &model_path, &model_params).map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_GEMMA_MODEL_LOAD_FAILED",
                    "failed to load Gemma GGUF model through the Rust coding harness",
                    "gemma_harness.model_path",
                    "provide a real Gemma GGUF file readable by the Rust harness",
                    json!({
                        "error": err.to_string(),
                        "model_path": file_sot(&model_path),
                        "gpu_layers": args.gemma_gpu_layers
                    }),
                    Some(file_sot(&model_path)),
                )
            })?;
        let chat_template = match args.gemma_chat_template.as_deref() {
            Some(template) => LlamaChatTemplate::new(template).map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_GEMMA_CHAT_TEMPLATE_INVALID",
                    "explicit Gemma chat template contained an invalid NUL byte",
                    "gemma_chat_template",
                    "provide a valid llama.cpp chat template name or template text",
                    json!({"error": err.to_string()}),
                    Some(file_sot(&model_path)),
                )
            })?,
            None => model.chat_template(None).map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_GEMMA_CHAT_TEMPLATE_MISSING",
                    "Gemma GGUF model does not expose a usable chat template",
                    "gemma_harness.chat_template",
                    "install a Gemma GGUF with chat-template metadata or pass --gemma-chat-template explicitly",
                    json!({"error": err.to_string(), "model_path": file_sot(&model_path)}),
                    Some(file_sot(&model_path)),
                )
            })?,
        };
        let model_context_train_tokens = model.n_ctx_train();
        let model_identity = gemma_model_identity_readback(&model, &model_path);
        validate_gemma4_model_identity(&model_path, &model_identity)?;
        if model_context_train_tokens == 0 {
            return Err(RealityError::new(
                "RUST_REALITY_GEMMA_MODEL_CONTEXT_INVALID",
                "loaded GGUF reported a zero training context window",
                "gemma_harness.model.n_ctx_train",
                "install a valid Gemma GGUF with context metadata",
                json!({"model_path": file_sot(&model_path), "n_ctx_train": model_context_train_tokens}),
                Some(file_sot(&model_path)),
            ));
        }
        if model_context_train_tokens < DEFAULT_GEMMA_MIN_CONTEXT_TOKENS {
            return Err(RealityError::new(
                "RUST_REALITY_GEMMA_MODEL_CONTEXT_BELOW_REQUIRED_FLOOR",
                "loaded Gemma GGUF context is below the SWE coding harness capability floor",
                "gemma_harness.model.n_ctx_train",
                "install a Gemma 4 GGUF with at least a 128K training context so repository context and tool transcripts fit without silent truncation",
                json!({
                    "minimum_context_tokens": DEFAULT_GEMMA_MIN_CONTEXT_TOKENS,
                    "model_context_train_tokens": model_context_train_tokens,
                    "model_identity": model_identity,
                    "model_path": file_sot(&model_path)
                }),
                Some(file_sot(&model_path)),
            ));
        }
        let context_tokens = args
            .gemma_context_tokens
            .unwrap_or(model_context_train_tokens);
        if context_tokens > model_context_train_tokens {
            return Err(RealityError::new(
                "RUST_REALITY_GEMMA_CONTEXT_EXCEEDS_MODEL_LIMIT",
                "requested Gemma context exceeds the loaded GGUF training context",
                "gemma_context_tokens",
                "omit --gemma-context-tokens to use the model maximum, or set a value within the GGUF n_ctx_train limit",
                json!({
                    "requested_context_tokens": context_tokens,
                    "model_context_train_tokens": model_context_train_tokens,
                    "model_path": file_sot(&model_path)
                }),
                Some(file_sot(&model_path)),
            ));
        }
        if context_tokens < DEFAULT_GEMMA_MIN_CONTEXT_TOKENS {
            return Err(RealityError::new(
                "RUST_REALITY_GEMMA_EFFECTIVE_CONTEXT_BELOW_REQUIRED_FLOOR",
                "effective Gemma context is below the SWE coding harness capability floor",
                "gemma_context_tokens",
                "omit --gemma-context-tokens to use the GGUF maximum, or set an explicit context of at least 128K tokens",
                json!({
                    "minimum_context_tokens": DEFAULT_GEMMA_MIN_CONTEXT_TOKENS,
                    "effective_context_tokens": context_tokens,
                    "requested_context_tokens": args.gemma_context_tokens,
                    "model_context_train_tokens": model_context_train_tokens,
                    "model_path": file_sot(&model_path)
                }),
                Some(file_sot(&model_path)),
            ));
        }
        if args.gemma_max_new_tokens >= context_tokens as usize {
            return Err(RealityError::new(
                "RUST_REALITY_GEMMA_MAX_NEW_TOKENS_EXCEEDS_CONTEXT",
                "Gemma max_new_tokens leaves no room for a prompt inside the configured context window",
                "gemma_max_new_tokens",
                "set max_new_tokens lower than the effective context window",
                json!({
                    "max_new_tokens": args.gemma_max_new_tokens,
                    "effective_context_tokens": context_tokens,
                    "requested_context_tokens": args.gemma_context_tokens,
                    "model_context_train_tokens": model_context_train_tokens
                }),
                Some(file_sot(&model_path)),
            ));
        }
        Ok(Self {
            backend,
            model,
            model_path,
            chat_template,
            requested_context_tokens: args.gemma_context_tokens,
            model_context_train_tokens,
            context_tokens,
            max_new_tokens: args.gemma_max_new_tokens,
            temperature: args.gemma_temperature,
            top_p: args.gemma_top_p,
            batch_size: args.gemma_batch_size,
            seed: args.gemma_seed,
        })
    }

    fn model_capability_report(&self, report_path: &Path) -> Result<Value> {
        let chat_template = self.chat_template.to_string().map_err(|err| {
            RealityError::new(
                "RUST_REALITY_GEMMA_CHAT_TEMPLATE_UTF8_INVALID",
                "Gemma chat template was not valid UTF-8",
                "gemma_harness.chat_template",
                "install a GGUF with a valid UTF-8 chat template or pass a valid explicit template",
                json!({"error": err.to_string(), "model_path": file_sot(&self.model_path)}),
                Some(file_sot(&self.model_path)),
            )
        })?;
        let mut selected_metadata = BTreeMap::new();
        selected_metadata.extend(gemma_model_identity_readback(&self.model, &self.model_path));
        Ok(json!({
            "record_kind": "contextgraph_gemma_harness_model_capability_readback",
            "status": "ok",
            "provider": "contextgraph_rust_gemma_coding_harness",
            "model_path": file_sot(&self.model_path),
            "requested_context_tokens": self.requested_context_tokens,
            "effective_context_tokens": self.context_tokens,
            "context_policy": if self.requested_context_tokens.is_some() { "explicit_cli" } else { "gguf_n_ctx_train_max" },
            "model_context_train_tokens": self.model_context_train_tokens,
            "minimum_context_tokens": DEFAULT_GEMMA_MIN_CONTEXT_TOKENS,
            "max_new_tokens": self.max_new_tokens,
            "temperature": self.temperature,
            "top_p": self.top_p,
            "batch_size": self.batch_size,
            "seed": self.seed,
            "model_readback": {
                "n_vocab": self.model.n_vocab(),
                "n_embd": self.model.n_embd(),
                "n_layer": self.model.n_layer(),
                "n_head": self.model.n_head(),
                "n_head_kv": self.model.n_head_kv(),
                "n_params": self.model.n_params(),
                "size_bytes": self.model.size(),
                "metadata_count": self.model.meta_count(),
                "selected_metadata": selected_metadata
            },
            "chat_template": {
                "sha256": sha256_text(&chat_template),
                "preview": truncate_chars(&chat_template, 4000)
            },
            "source_of_truth": {
                "report": file_sot(report_path),
                "model_path": file_sot(&self.model_path)
            }
        }))
    }

    fn chat(&self, args: &AttemptArgs, body: &Value, request_path: &Path) -> Result<Value> {
        let messages = body
            .get("messages")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                RealityError::new(
                    "RUST_REALITY_GEMMA_MESSAGES_MISSING",
                    "Gemma harness request did not contain messages array",
                    "gemma_harness.request.messages",
                    "fix the Rust request builder before running another attempt",
                    json!({"request": file_sot(request_path)}),
                    Some(file_sot(request_path)),
                )
            })?;
        let tools = body.get("tools").and_then(Value::as_array).ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_GEMMA_TOOLS_MISSING",
                "Gemma harness request did not contain tools array",
                "gemma_harness.request.tools",
                "fix the Rust request builder before running another attempt",
                json!({"request": file_sot(request_path)}),
                Some(file_sot(request_path)),
            )
        })?;
        if tools.is_empty() {
            return Err(RealityError::new(
                "RUST_REALITY_GEMMA_TOOLS_EMPTY",
                "Gemma coding harness refuses to run without governed tools",
                "gemma_harness.request.tools",
                "provide the ContextGraph coding-agent tool schema",
                json!({"request": file_sot(request_path)}),
                Some(file_sot(request_path)),
            ));
        }
        let messages_json =
            serde_json::to_string(&Value::Array(messages.clone())).map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_GEMMA_MESSAGES_SERIALIZE_FAILED",
                    "failed to serialize Gemma chat messages",
                    "gemma_harness.request.messages",
                    "inspect the message JSON shape",
                    json!({"error": err.to_string(), "request": file_sot(request_path)}),
                    Some(file_sot(request_path)),
                )
            })?;
        let tools_json = serde_json::to_string(&Value::Array(tools.clone())).map_err(|err| {
            RealityError::new(
                "RUST_REALITY_GEMMA_TOOLS_SERIALIZE_FAILED",
                "failed to serialize Gemma tool schemas",
                "gemma_harness.request.tools",
                "inspect the tool JSON schema",
                json!({"error": err.to_string(), "request": file_sot(request_path)}),
                Some(file_sot(request_path)),
            )
        })?;
        let params = OpenAIChatTemplateParams {
            messages_json: &messages_json,
            tools_json: Some(&tools_json),
            tool_choice: Some("auto"),
            json_schema: None,
            grammar: None,
            reasoning_format: None,
            chat_template_kwargs: None,
            add_generation_prompt: true,
            use_jinja: true,
            parallel_tool_calls: false,
            enable_thinking: false,
            add_bos: false,
            add_eos: false,
            parse_tool_calls: true,
        };
        let rendered = self
            .model
            .apply_chat_template_oaicompat(&self.chat_template, &params)
            .map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_GEMMA_CHAT_TEMPLATE_APPLY_FAILED",
                    "failed to render Gemma chat template with ContextGraph tools",
                    "gemma_harness.chat_template",
                    "inspect the GGUF chat template and tool schema compatibility",
                    json!({
                        "error": err.to_string(),
                        "request": file_sot(request_path),
                        "model_path": file_sot(&self.model_path)
                    }),
                    Some(file_sot(request_path)),
                )
            })?;
        let rendered_prompt_path = request_sibling_path(request_path, "rendered-prompt.txt");
        write_text_checked(&rendered_prompt_path, &rendered.prompt)?;
        let grammar = rendered.grammar.as_deref().ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_GEMMA_TOOL_GRAMMAR_MISSING",
                "Gemma chat template did not produce a tool-call grammar",
                "gemma_harness.tool_grammar",
                "use a Gemma GGUF/template combination that supports strict OpenAI-compatible tool grammar",
                json!({
                    "request": file_sot(request_path),
                    "rendered_prompt": file_sot(&rendered_prompt_path),
                    "model_path": file_sot(&self.model_path),
                    "parse_tool_calls": rendered.parse_tool_calls
                }),
                Some(file_sot(&rendered_prompt_path)),
            )
        })?;
        let grammar_path = request_sibling_path(request_path, "tool-grammar.gbnf");
        write_text_checked(&grammar_path, grammar)?;
        let (raw_generation, prompt_tokens, generated_tokens) =
            self.generate(&rendered, request_path)?;
        let raw_generation_path = request_sibling_path(request_path, "raw-generation.txt");
        write_text_checked(&raw_generation_path, &raw_generation)?;
        let parsed_text = rendered
            .parse_response_oaicompat(&raw_generation, false)
            .map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_GEMMA_TOOL_RESPONSE_PARSE_FAILED",
                    "Gemma generation could not be parsed as an OpenAI-compatible assistant message",
                    "gemma_harness.response",
                    "inspect raw generation, prompt, and tool grammar before retrying",
                    json!({
                        "error": err.to_string(),
                        "request": file_sot(request_path),
                        "raw_generation": file_sot(&raw_generation_path),
                        "rendered_prompt": file_sot(&rendered_prompt_path),
                        "tool_grammar": file_sot(&grammar_path)
                    }),
                    Some(file_sot(&raw_generation_path)),
                )
            })?;
        let parsed: Value = serde_json::from_str(&parsed_text).map_err(|err| {
            RealityError::new(
                "RUST_REALITY_GEMMA_TOOL_RESPONSE_JSON_INVALID",
                "parsed Gemma assistant message was not JSON",
                "gemma_harness.response_json",
                "inspect the llama.cpp OpenAI-compatible parser output",
                json!({
                    "error": err.to_string(),
                    "parsed_text": parsed_text,
                    "raw_generation": file_sot(&raw_generation_path)
                }),
                Some(file_sot(&raw_generation_path)),
            )
        })?;
        let message = normalize_gemma_harness_message(&parsed, request_path, &raw_generation_path)?;
        Ok(json!({
            "model": args.model,
            "created_at_unix": unix_secs()?,
            "done": true,
            "message": message,
            "contextgraph_gemma_harness": {
                "provider": "contextgraph_rust_gemma_coding_harness",
                "model_path": file_sot(&self.model_path),
                "requested_context_tokens": self.requested_context_tokens,
                "effective_context_tokens": self.context_tokens,
                "model_context_train_tokens": self.model_context_train_tokens,
                "max_new_tokens": self.max_new_tokens,
                "prompt_tokens": prompt_tokens,
                "generated_tokens": generated_tokens,
                "chat_format": rendered.chat_format,
                "parse_tool_calls": rendered.parse_tool_calls,
                "grammar_lazy": rendered.grammar_lazy,
                "additional_stops": rendered.additional_stops,
                "source_of_truth": {
                    "request": file_sot(request_path),
                    "rendered_prompt": file_sot(&rendered_prompt_path),
                    "tool_grammar": file_sot(&grammar_path),
                    "raw_generation": file_sot(&raw_generation_path),
                    "model_path": file_sot(&self.model_path)
                }
            }
        }))
    }

    fn generate(
        &self,
        rendered: &ChatTemplateResult,
        request_path: &Path,
    ) -> Result<(String, usize, usize)> {
        let context_tokens = NonZeroU32::new(self.context_tokens).ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_GEMMA_CONTEXT_TOKENS_INVALID",
                "Gemma context window cannot be zero",
                "gemma_context_tokens",
                "set --gemma-context-tokens to a positive value",
                json!({"gemma_context_tokens": self.context_tokens}),
                Some(file_sot(request_path)),
            )
        })?;
        let batch_size = u32::try_from(self.batch_size).map_err(|err| {
            RealityError::new(
                "RUST_REALITY_GEMMA_BATCH_SIZE_TOO_LARGE",
                "Gemma batch size could not be represented for llama.cpp context params",
                "gemma_batch_size",
                "set --gemma-batch-size no larger than u32::MAX",
                json!({"error": err.to_string(), "batch_size": self.batch_size}),
                Some(file_sot(request_path)),
            )
        })?;
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(context_tokens))
            .with_n_batch(batch_size)
            .with_n_ubatch(batch_size);
        let mut ctx = self
            .model
            .new_context(&self.backend, ctx_params)
            .map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_GEMMA_CONTEXT_CREATE_FAILED",
                    "failed to create llama.cpp generation context",
                    "gemma_harness.context",
                    "inspect context window size, GPU memory, and model compatibility",
                    json!({
                        "error": err.to_string(),
                        "effective_context_tokens": self.context_tokens,
                        "requested_context_tokens": self.requested_context_tokens,
                        "model_context_train_tokens": self.model_context_train_tokens,
                        "batch_size": self.batch_size,
                        "model_path": file_sot(&self.model_path),
                        "request": file_sot(request_path)
                    }),
                    Some(file_sot(&self.model_path)),
                )
            })?;
        let tokens = self
            .model
            .str_to_token(&rendered.prompt, AddBos::Never)
            .map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_GEMMA_PROMPT_TOKENIZE_FAILED",
                    "failed to tokenize rendered Gemma prompt",
                    "gemma_harness.prompt",
                    "inspect chat template output and model tokenizer compatibility",
                    json!({"error": err.to_string(), "request": file_sot(request_path)}),
                    Some(file_sot(request_path)),
                )
            })?;
        if tokens.len() >= self.context_tokens as usize {
            return Err(RealityError::new(
                "RUST_REALITY_GEMMA_PROMPT_CONTEXT_EXCEEDED",
                "rendered Gemma prompt exceeds the configured context window",
                "gemma_harness.prompt_tokens",
                "reduce repository context/tool transcript size or increase --gemma-context-tokens after verifying model capacity",
                json!({
                    "prompt_tokens": tokens.len(),
                    "context_tokens": self.context_tokens,
                    "request": file_sot(request_path)
                }),
                Some(file_sot(request_path)),
            ));
        }
        let available_generation_tokens = self.context_tokens as usize - tokens.len();
        if self.max_new_tokens > available_generation_tokens {
            return Err(RealityError::new(
                "RUST_REALITY_GEMMA_PROMPT_AND_OUTPUT_CONTEXT_EXCEEDED",
                "rendered Gemma prompt plus requested generation budget exceeds the configured context window",
                "gemma_harness.prompt_tokens_plus_max_new_tokens",
                "reduce prompt/tool transcript size or use a Gemma GGUF with a larger training context; the harness will not silently lower the requested budget",
                json!({
                    "prompt_tokens": tokens.len(),
                    "max_new_tokens": self.max_new_tokens,
                    "available_generation_tokens": available_generation_tokens,
                    "effective_context_tokens": self.context_tokens,
                    "model_context_train_tokens": self.model_context_train_tokens,
                    "request": file_sot(request_path)
                }),
                Some(file_sot(request_path)),
            ));
        }
        decode_prompt_tokens(&mut ctx, &tokens, self.batch_size, request_path)?;
        let grammar_sampler = self.grammar_sampler(rendered, request_path)?;
        let mut sampler_chain = vec![
            grammar_sampler,
            LlamaSampler::top_p(self.top_p, 1),
            LlamaSampler::temp(self.temperature),
        ];
        if self.temperature <= 0.0 {
            sampler_chain.push(LlamaSampler::greedy());
        } else {
            sampler_chain.push(LlamaSampler::dist(self.seed));
        }
        let mut sampler = LlamaSampler::chain_simple(sampler_chain);
        let mut generated_tokens: Vec<LlamaToken> = Vec::new();
        for (current_pos, _) in (tokens.len()..).zip(0..self.max_new_tokens) {
            let token = sampler.sample(&ctx, -1);
            if self.model.is_eog_token(token) {
                break;
            }
            generated_tokens.push(token);
            let mut batch = LlamaBatch::new(1, 1);
            batch
                .add(token, current_pos as i32, &[0], true)
                .map_err(|err| {
                    RealityError::new(
                        "RUST_REALITY_GEMMA_GENERATION_BATCH_ADD_FAILED",
                        "failed to append generated token to llama.cpp batch",
                        "gemma_harness.generation_batch",
                        "inspect llama.cpp batch state and generated token position",
                        json!({
                            "error": err.to_string(),
                            "current_pos": current_pos,
                            "request": file_sot(request_path)
                        }),
                        Some(file_sot(request_path)),
                    )
                })?;
            ctx.decode(&mut batch).map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_GEMMA_GENERATION_DECODE_FAILED",
                    "llama.cpp failed while decoding a generated token",
                    "gemma_harness.generation_decode",
                    "inspect model/runtime state and GPU memory",
                    json!({
                        "error": err.to_string(),
                        "current_pos": current_pos,
                        "request": file_sot(request_path)
                    }),
                    Some(file_sot(request_path)),
                )
            })?;
        }
        let mut output = decode_llama_tokens(&self.model, &generated_tokens).map_err(|err| {
            RealityError::new(
                "RUST_REALITY_GEMMA_TOKEN_DECODE_FAILED",
                "failed to decode generated llama.cpp tokens",
                "gemma_harness.generated_tokens",
                "inspect generated token stream and model tokenizer",
                json!({"error": err.to_string(), "request": file_sot(request_path)}),
                Some(file_sot(request_path)),
            )
        })?;
        for stop in &rendered.additional_stops {
            if let Some(index) = output.find(stop) {
                output.truncate(index);
                break;
            }
        }
        Ok((output, tokens.len(), generated_tokens.len()))
    }

    fn grammar_sampler(
        &self,
        rendered: &ChatTemplateResult,
        request_path: &Path,
    ) -> Result<LlamaSampler> {
        let grammar = rendered.grammar.as_deref().ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_GEMMA_TOOL_GRAMMAR_MISSING",
                "Gemma chat template did not produce a tool-call grammar",
                "gemma_harness.tool_grammar",
                "use a Gemma GGUF/template combination that supports strict tool grammar",
                json!({"request": file_sot(request_path)}),
                Some(file_sot(request_path)),
            )
        })?;
        if rendered.grammar_lazy {
            let mut token_triggers = Vec::new();
            let mut word_triggers = Vec::new();
            let mut pattern_triggers = Vec::new();
            for trigger in &rendered.grammar_triggers {
                match trigger.trigger_type {
                    GrammarTriggerType::Token => {
                        if let Some(token) = trigger.token {
                            token_triggers.push(token);
                        }
                    }
                    GrammarTriggerType::Word => word_triggers.push(trigger.value.clone()),
                    GrammarTriggerType::Pattern | GrammarTriggerType::PatternFull => {
                        pattern_triggers.push(trigger.value.clone());
                    }
                }
            }
            if !pattern_triggers.is_empty() {
                LlamaSampler::grammar_lazy_patterns(
                    &self.model,
                    grammar,
                    "root",
                    &pattern_triggers,
                    &token_triggers,
                )
                .map_err(|err| {
                    RealityError::new(
                        "RUST_REALITY_GEMMA_GRAMMAR_SAMPLER_FAILED",
                        "failed to create lazy-pattern tool grammar sampler",
                        "gemma_harness.tool_grammar",
                        "inspect generated tool grammar and chat-template triggers",
                        json!({"error": err.to_string(), "request": file_sot(request_path)}),
                        Some(file_sot(request_path)),
                    )
                })
            } else {
                LlamaSampler::grammar_lazy(
                    &self.model,
                    grammar,
                    "root",
                    word_triggers.iter().map(String::as_str),
                    &token_triggers,
                )
                .map_err(|err| {
                    RealityError::new(
                        "RUST_REALITY_GEMMA_GRAMMAR_SAMPLER_FAILED",
                        "failed to create lazy tool grammar sampler",
                        "gemma_harness.tool_grammar",
                        "inspect generated tool grammar and chat-template triggers",
                        json!({"error": err.to_string(), "request": file_sot(request_path)}),
                        Some(file_sot(request_path)),
                    )
                })
            }
        } else {
            LlamaSampler::grammar(&self.model, grammar, "root").map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_GEMMA_GRAMMAR_SAMPLER_FAILED",
                    "failed to create strict tool grammar sampler",
                    "gemma_harness.tool_grammar",
                    "inspect generated tool grammar",
                    json!({"error": err.to_string(), "request": file_sot(request_path)}),
                    Some(file_sot(request_path)),
                )
            })
        }
    }
}

fn request_sibling_path(request_path: &Path, suffix: &str) -> PathBuf {
    let stem = request_path
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("gemma-harness");
    request_path.with_file_name(format!("{stem}-{suffix}"))
}

fn decode_prompt_tokens(
    ctx: &mut LlamaContext<'_>,
    tokens: &[LlamaToken],
    batch_size: usize,
    request_path: &Path,
) -> Result<()> {
    let mut offset = 0_usize;
    while offset < tokens.len() {
        let end = (offset + batch_size).min(tokens.len());
        let mut batch = LlamaBatch::new(batch_size, 1);
        for (relative_index, token) in tokens[offset..end].iter().copied().enumerate() {
            let absolute_index = offset + relative_index;
            let is_last = absolute_index + 1 == tokens.len();
            batch
                .add(token, absolute_index as i32, &[0], is_last)
                .map_err(|err| {
                    RealityError::new(
                        "RUST_REALITY_GEMMA_PROMPT_BATCH_ADD_FAILED",
                        "failed to append prompt token to llama.cpp batch",
                        "gemma_harness.prompt_batch",
                        "inspect prompt length, batch size, and llama.cpp context state",
                        json!({
                            "error": err.to_string(),
                            "absolute_index": absolute_index,
                            "batch_size": batch_size,
                            "request": file_sot(request_path)
                        }),
                        Some(file_sot(request_path)),
                    )
                })?;
        }
        ctx.decode(&mut batch).map_err(|err| {
            RealityError::new(
                "RUST_REALITY_GEMMA_PROMPT_DECODE_FAILED",
                "llama.cpp failed while decoding rendered prompt",
                "gemma_harness.prompt_decode",
                "inspect prompt, context window, and GPU memory",
                json!({
                    "error": err.to_string(),
                    "offset": offset,
                    "end": end,
                    "request": file_sot(request_path)
                }),
                Some(file_sot(request_path)),
            )
        })?;
        offset = end;
    }
    Ok(())
}

fn decode_llama_tokens(
    model: &LlamaModel,
    tokens: &[LlamaToken],
) -> std::result::Result<String, TokenToStringError> {
    let mut bytes = Vec::with_capacity(tokens.len() * 4);
    for token in tokens.iter().copied() {
        let mut buffer_size = 8_usize;
        loop {
            match model.token_to_piece_bytes(token, buffer_size, true, None) {
                Ok(piece) => {
                    bytes.extend_from_slice(&piece);
                    break;
                }
                Err(TokenToStringError::InsufficientBufferSpace(size)) if size < 0 => {
                    buffer_size = usize::try_from(-size).expect("buffer size is positive");
                }
                Err(err) => return Err(err),
            }
        }
    }
    Ok(String::from_utf8(bytes)?)
}

fn normalize_gemma_harness_message(
    parsed: &Value,
    request_path: &Path,
    raw_generation_path: &Path,
) -> Result<Value> {
    let mut message = if let Some(message) = parsed.get("message") {
        message.clone()
    } else if let Some(message) = parsed
        .pointer("/choices/0/message")
        .filter(|message| message.is_object())
    {
        message.clone()
    } else {
        parsed.clone()
    };
    let Some(obj) = message.as_object_mut() else {
        return Err(RealityError::new(
            "RUST_REALITY_GEMMA_MESSAGE_SHAPE_INVALID",
            "Gemma parser output was not an assistant message object",
            "gemma_harness.response.message",
            "inspect raw generation and parser output",
            json!({
                "parsed": parsed,
                "request": file_sot(request_path),
                "raw_generation": file_sot(raw_generation_path)
            }),
            Some(file_sot(raw_generation_path)),
        ));
    };
    match obj.get("role").and_then(Value::as_str) {
        Some("assistant") => {}
        Some(other) => {
            return Err(RealityError::new(
                "RUST_REALITY_GEMMA_MESSAGE_ROLE_INVALID",
                "Gemma parser output did not return an assistant role",
                "gemma_harness.response.message.role",
                "inspect chat template parser output and role mapping",
                json!({
                    "role": other,
                    "parsed": parsed,
                    "request": file_sot(request_path),
                    "raw_generation": file_sot(raw_generation_path)
                }),
                Some(file_sot(raw_generation_path)),
            ));
        }
        None => {
            return Err(RealityError::new(
                "RUST_REALITY_GEMMA_MESSAGE_ROLE_MISSING",
                "Gemma parser output omitted assistant message role",
                "gemma_harness.response.message.role",
                "inspect chat template parser output",
                json!({
                    "parsed": parsed,
                    "request": file_sot(request_path),
                    "raw_generation": file_sot(raw_generation_path)
                }),
                Some(file_sot(raw_generation_path)),
            ));
        }
    }
    if !obj.contains_key("content") {
        obj.insert("content".to_string(), json!(""));
    }
    if let Some(tool_calls) = obj.get("tool_calls") {
        if !tool_calls.is_array() {
            return Err(RealityError::new(
                "RUST_REALITY_GEMMA_TOOL_CALLS_SHAPE_INVALID",
                "Gemma parser output tool_calls was not an array",
                "gemma_harness.response.message.tool_calls",
                "inspect chat template parser output",
                json!({
                    "tool_calls": tool_calls,
                    "request": file_sot(request_path),
                    "raw_generation": file_sot(raw_generation_path)
                }),
                Some(file_sot(raw_generation_path)),
            ));
        }
    }
    Ok(message)
}

fn gemma_coding_harness_chat_json(
    harness: &GemmaCodingHarness,
    ctx: &AgentToolContext<'_>,
    prompt: &str,
    attempt: u64,
) -> Result<Value> {
    let args = ctx.args;
    let attempt_dir = ctx.attempt_dir;
    let gpu_before = read_gpu_vram_state(args.gpu_index)?;
    assert_gpu_budget(
        &gpu_before,
        args.gpu_min_free_mb,
        args.gpu_max_used_mb,
        "before_gemma_coding_harness",
        &args.model,
    )?;
    let system_prompt = format!(
        "You are a model that can do function calling with the following functions. \
You are a software repair model inside ContextGraph's Rust reality loop. \
You must call governed tools to inspect reality and must submit the candidate patch with apply_line_window_edit, apply_structured_edit, or apply_unified_diff. \
On attempt 1, start by calling read_problem_reality, search_repo, inspect_symbol, open_file_window, or synthesize_helper_tool so your edit is grounded in real task and repository state. \
On later attempts, call read_official_failure_reality or compare_attempts before revising a failed patch. \
The application executes tools; you do not execute code yourself. \
Prefer apply_line_window_edit after open_file_window because it replaces line ranges from a file hash you observed. \
Use apply_structured_edit only when you can provide an exact unique find span copied from real source windows. \
Use apply_unified_diff only when you can provide a real unified diff and observed file hashes from opened source windows. \
ContextGraph compiles that tool call into a git diff and verifies it against the real repository. \
Runtime limits: effective context window is {} tokens from the loaded GGUF; per-turn max_new_tokens is {}; governed tool-round budget is {}. \
Do not try to finish with prose, raw JSON, or a hand-written patch in assistant content. \
Do not modify tests, fixtures, or benchmark harness files.",
        harness.context_tokens, harness.max_new_tokens, args.agent_tool_rounds
    );
    let mut messages = vec![
        json!({"role": "system", "content": system_prompt}),
        json!({"role": "user", "content": prompt}),
    ];
    let tools = agent_tool_specs();
    let tools_root = attempt_dir.join("agent-tools");
    create_dir_checked(&tools_root)?;
    let mut transcript_entries = Vec::new();
    let mut executed_tool_calls = 0_u64;
    let mut submitted_candidate_patch: Option<Value> = None;
    let mut failed_candidate_submission: Option<Value> = None;
    let mut latest_editable_windows: Vec<Value> = Vec::new();
    let mut no_tool_after_window_count = 0_u64;
    let mut repeated_navigation_count = 0_u64;
    let mut seen_tool_calls: BTreeMap<String, u64> = BTreeMap::new();

    for round in 1..=args.agent_tool_rounds {
        let request_path = tools_root.join(format!("tool-round-{round:02}-request.json"));
        let edit_deadline = edit_deadline_active(
            round,
            args.agent_tool_rounds,
            &latest_editable_windows,
            no_tool_after_window_count,
            repeated_navigation_count,
            submitted_candidate_patch.is_some(),
        );
        let round_tools = if edit_deadline {
            agent_edit_tool_specs()
        } else {
            tools.clone()
        };
        let body = json!({
            "model": args.model,
            "messages": messages,
            "stream": false,
            "think": false,
            "tools": round_tools,
            "contextgraph_tool_policy": {
                "edit_deadline_active": edit_deadline,
                "latest_editable_windows": latest_editable_windows.clone(),
                "no_tool_after_window_count": no_tool_after_window_count,
                "repeated_navigation_count": repeated_navigation_count
            },
            "runtime": {
                "provider": "contextgraph_rust_gemma_coding_harness",
                "model_path": file_sot(&harness.model_path)
            },
            "options": {
                "temperature": args.gemma_temperature,
                "top_p": args.gemma_top_p,
                "requested_context_tokens": args.gemma_context_tokens,
                "effective_context_tokens": harness.context_tokens,
                "model_context_train_tokens": harness.model_context_train_tokens,
                "max_new_tokens": harness.max_new_tokens,
                "gpu_layers": args.gemma_gpu_layers,
                "batch_size": args.gemma_batch_size,
                "seed": args.gemma_seed
            }
        });
        write_json_checked(&request_path, &body)?;
        let response = harness.chat(args, &body, &request_path)?;
        let response_path = tools_root.join(format!("tool-round-{round:02}-response.json"));
        write_json_checked(&response_path, &response)?;
        let mut tool_request_protocol = "contextgraph_native_tool_calls";
        let mut tool_calls = response
            .pointer("/message/tool_calls")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if tool_calls.is_empty() {
            let content_requests = content_json_tool_calls(&response, round)?;
            if !content_requests.is_empty() {
                tool_request_protocol = "contextgraph_content_json_tool_request";
                tool_calls = content_requests;
            }
        }
        if tool_calls.is_empty() {
            if !latest_editable_windows.is_empty() {
                no_tool_after_window_count += 1;
            }
            transcript_entries.push(json!({
                "round": round,
                "status": "no_tool_call",
                "edit_deadline_active": edit_deadline,
                "latest_editable_windows": latest_editable_windows.clone(),
                "accepted_protocols": [
                    "contextgraph_native_tool_calls",
                    "contextgraph_content_json_tool_request"
                ],
                "request": file_sot(&request_path),
                "response": file_sot(&response_path)
            }));
            messages.push(response["message"].clone());
            messages.push(json!({
                "role": "user",
                "content": no_tool_repair_prompt(
                    edit_deadline,
                    &latest_editable_windows,
                    no_tool_after_window_count,
                    repeated_navigation_count
                )
            }));
            continue;
        }
        messages.push(response["message"].clone());
        let mut tool_results = Vec::new();
        for (index, call) in tool_calls.iter().enumerate() {
            let name = tool_call_name(call)?;
            let arguments = tool_call_arguments(call)?;
            let arguments_sha256 = sha256_text(&serde_json::to_string(&arguments).unwrap());
            let tool_call_fingerprint = format!("{name}:{arguments_sha256}");
            let duplicate_count_before = *seen_tool_calls.get(&tool_call_fingerprint).unwrap_or(&0);
            seen_tool_calls.insert(tool_call_fingerprint, duplicate_count_before + 1);
            let duplicate_navigation_call =
                duplicate_count_before > 0 && is_navigation_or_reality_tool(&name);
            if duplicate_navigation_call {
                repeated_navigation_count += 1;
            }
            let call_id = tool_call_id(call, round, index);
            let tool_dir =
                tools_root.join(format!("round-{round:02}-{index:02}-{}", safe_id(&name)));
            create_dir_checked(&tool_dir)?;
            let requested_path = tool_dir.join("requested-tool-call.json");
            write_json_checked(
                &requested_path,
                &json!({
                "round": round,
                "index": index,
                "tool_call_id": call_id,
                "tool_name": name,
                "tool_request_protocol": tool_request_protocol,
                "arguments": arguments,
                "raw_call": call,
                "source_of_truth": file_sot(&requested_path)
                }),
            )?;
            let result = execute_agent_tool_call(ctx, &name, &arguments, &tool_dir)?;
            let result_path = tool_dir.join("tool-result.json");
            write_json_checked(&result_path, &result)?;
            let result_readback: Value = read_json(&result_path)?;
            if name == "open_file_window" {
                if let Some(window) = compact_editable_window(&result_readback) {
                    latest_editable_windows.push(window);
                    if latest_editable_windows.len() > 4 {
                        latest_editable_windows.remove(0);
                    }
                }
            }
            let result_status = result_readback
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("failed");
            if is_final_edit_tool(&name) && result_status == "ok" {
                submitted_candidate_patch = Some(materialize_tool_candidate_submission(
                    &result_readback,
                    &tool_dir,
                )?);
            } else if is_final_edit_tool(&name) {
                failed_candidate_submission = Some(compact_failed_candidate_submission(
                    &result_readback,
                    &result_path,
                ));
            }
            let harness_transition_path = tool_dir.join("harness-transition.json");
            let harness_transition = build_harness_transition(
                ctx,
                attempt,
                round,
                index,
                executed_tool_calls,
                submitted_candidate_patch.is_some(),
                &call_id,
                &name,
                tool_request_protocol,
                &arguments,
                &requested_path,
                &response_path,
                &result_path,
                &result_readback,
                &harness_transition_path,
            )?;
            write_json_checked(&harness_transition_path, &harness_transition)?;
            let harness_transition_readback: Value = read_json(&harness_transition_path)?;
            let ledger_evidence = record_agent_tool_boundary(
                ctx,
                &name,
                &requested_path,
                &result_path,
                &harness_transition_path,
                result_status,
                &tool_dir,
            )?;
            let result_text = serde_json::to_string(&result_readback).unwrap();
            messages.push(json!({
                "role": "tool",
                "name": name,
                "tool_name": name,
                "tool_call_id": call_id,
                "content": result_text
            }));
            executed_tool_calls += 1;
            tool_results.push(json!({
                "tool_call_id": call_id,
                "tool_name": name,
                "tool_request_protocol": tool_request_protocol,
                "arguments_sha256": arguments_sha256,
                "duplicate_tool_call_count_before": duplicate_count_before,
                "duplicate_navigation_call": duplicate_navigation_call,
                "edit_deadline_active": edit_deadline,
                "requested": file_sot(&requested_path),
                "result": file_sot(&result_path),
                "harness_transition": file_sot(&harness_transition_path),
                "harness_transition_sha256": sha256_file(&harness_transition_path)?,
                "harness_transition_status": harness_transition_readback.get("outcome").and_then(|value| value.get("label")),
                "status": result_readback.get("status"),
                "error_code": result_readback.get("error_code"),
                "blackbox_ledger_evidence": ledger_evidence
            }));
        }
        transcript_entries.push(json!({
            "round": round,
            "status": "tool_results_appended",
            "edit_deadline_active": edit_deadline,
            "latest_editable_windows": latest_editable_windows.clone(),
            "tool_request_protocol": tool_request_protocol,
            "request": file_sot(&request_path),
            "response": file_sot(&response_path),
            "tool_results": tool_results
        }));
    }

    let transcript_path = tools_root.join("agent-tool-transcript.json");
    if args.agent_tool_rounds > 0 && executed_tool_calls == 0 {
        let transcript = json!({
            "record_kind": "contextgraph_agent_tool_transcript",
            "attempt": attempt,
            "model": args.model,
            "tool_round_budget": args.agent_tool_rounds,
            "tools": agent_tool_names(),
            "entries": transcript_entries,
            "executed_tool_calls": executed_tool_calls,
            "status": "blocked",
            "error_code": "RUST_REALITY_AGENT_TOOL_REQUIRED_BUT_MISSING",
            "source_of_truth": file_sot(&transcript_path)
        });
        write_json_checked(&transcript_path, &transcript)?;
        let gpu_after = read_gpu_vram_state(args.gpu_index)?;
        assert_gpu_budget(
            &gpu_after,
            args.gpu_min_free_mb,
            args.gpu_max_used_mb,
            "after_blocked_tool_phase",
            &args.model,
        )?;
        let blocked = json!({
            "record_kind": "contextgraph_model_step_blocked",
            "status": "blocked",
            "error_code": "RUST_REALITY_AGENT_TOOL_REQUIRED_BUT_MISSING",
            "field_path": "gemma_harness.message.tool_calls",
            "message_text": "model did not call any governed ContextGraph tool before trying to answer",
            "remediation": "reinject this blocked-step reality and require read_problem_reality, search_repo, inspect_symbol, open_file_window, or read_official_failure_reality before submitting edits",
            "message": {"role": "assistant", "content": ""},
            "details": {
                "attempt": attempt,
                "model": args.model,
                "tool_round_budget": args.agent_tool_rounds,
                "tools": agent_tool_names()
            },
            "contextgraph_runtime": {
                "attempt": attempt,
                "gpu_before": gpu_before,
                "gpu_after": gpu_after,
                "agent_tool_transcript": file_sot(&transcript_path)
            },
            "source_of_truth": file_sot(&transcript_path)
        });
        write_json_checked(&attempt_dir.join("gemma-harness-response.json"), &blocked)?;
        return Ok(blocked);
    }

    let gpu_after = read_gpu_vram_state(args.gpu_index)?;
    assert_gpu_budget(
        &gpu_after,
        args.gpu_min_free_mb,
        args.gpu_max_used_mb,
        "after_gemma_coding_harness",
        &args.model,
    )?;
    let Some(submitted_candidate_patch) = submitted_candidate_patch else {
        let attempted_edit = failed_candidate_submission.is_some();
        let error_code = if attempted_edit {
            "RUST_REALITY_FINAL_EDIT_TOOL_FAILED"
        } else {
            "RUST_REALITY_FINAL_EDIT_TOOL_REQUIRED"
        };
        let message = if attempted_edit {
            "model called a final edit tool but it did not produce a candidate patch"
        } else {
            "model used governed tools but never submitted a candidate patch through apply_line_window_edit, apply_structured_edit, or apply_unified_diff"
        };
        let remediation = if attempted_edit {
            "reinject the failed final edit result, open the exact source window, and submit a line-window edit, unique find/replace edit, or observed unified diff"
        } else {
            "call apply_line_window_edit with observed_sha256 from open_file_window, apply_structured_edit with exact find/replace edits copied from opened source windows, or apply_unified_diff with observed file hashes"
        };
        let transcript = json!({
            "record_kind": "contextgraph_agent_tool_transcript",
            "attempt": attempt,
            "model": args.model,
            "tool_round_budget": args.agent_tool_rounds,
            "tools": agent_tool_names(),
            "entries": transcript_entries,
            "executed_tool_calls": executed_tool_calls,
            "status": "blocked",
            "error_code": error_code,
            "message": message,
            "remediation": remediation,
            "failed_candidate_submission": failed_candidate_submission,
            "source_of_truth": file_sot(&transcript_path)
        });
        write_json_checked(&transcript_path, &transcript)?;
        let blocked = json!({
            "record_kind": "contextgraph_model_step_blocked",
            "status": "blocked",
            "error_code": error_code,
            "field_path": "agent_tools.final_edit_submission",
            "message_text": message,
            "remediation": remediation,
            "message": {"role": "assistant", "content": ""},
            "details": {
                "attempt": attempt,
                "model": args.model,
                "tool_round_budget": args.agent_tool_rounds,
                "executed_tool_calls": executed_tool_calls,
                "tools": agent_tool_names(),
                "failed_candidate_submission": failed_candidate_submission
            },
            "contextgraph_runtime": {
                "attempt": attempt,
                "gpu_before": gpu_before,
                "gpu_after": gpu_after,
                "agent_tool_transcript": file_sot(&transcript_path)
            },
            "source_of_truth": file_sot(&transcript_path)
        });
        write_json_checked(&attempt_dir.join("gemma-harness-response.json"), &blocked)?;
        return Ok(blocked);
    };
    let transcript = json!({
        "record_kind": "contextgraph_agent_tool_transcript",
        "attempt": attempt,
        "model": args.model,
        "tool_round_budget": args.agent_tool_rounds,
        "tools": agent_tool_names(),
        "entries": transcript_entries,
        "executed_tool_calls": executed_tool_calls,
        "final_action_mode": "tool_mediated_final_edit_submission",
        "submitted_candidate_patch": submitted_candidate_patch,
        "status": "ok",
        "source_of_truth": file_sot(&tools_root.join("agent-tool-transcript.json"))
    });
    write_json_checked(&transcript_path, &transcript)?;
    let enriched = json!({
        "model": args.model,
        "created_at_unix": unix_secs()?,
        "done": true,
        "message": {
            "role": "assistant",
            "content": ""
        },
        "contextgraph_runtime": {
            "attempt": attempt,
            "gpu_before": gpu_before,
            "gpu_after": gpu_after,
            "final_action_mode": "tool_mediated_final_edit_submission",
            "submitted_candidate_patch": submitted_candidate_patch,
            "agent_tool_transcript": file_sot(&transcript_path)
        }
    });
    write_json_checked(&attempt_dir.join("gemma-harness-response.json"), &enriched)?;
    Ok(enriched)
}

fn agent_tool_names() -> Vec<&'static str> {
    vec![
        "read_problem_reality",
        "read_official_failure_reality",
        "list_files",
        "inspect_symbol",
        "search_repo",
        "open_file_window",
        "apply_line_window_edit",
        "apply_structured_edit",
        "apply_unified_diff",
        "run_targeted_test",
        "run_official_swe_eval",
        "compare_attempts",
        "retrieve_similar_verified_episodes",
        "synthesize_helper_tool",
    ]
}

fn is_final_edit_tool(name: &str) -> bool {
    matches!(
        name,
        "apply_line_window_edit" | "apply_structured_edit" | "apply_unified_diff"
    )
}

fn agent_edit_tool_specs() -> Value {
    let specs = agent_tool_specs();
    let edit_names = [
        "apply_line_window_edit",
        "apply_structured_edit",
        "apply_unified_diff",
    ];
    let filtered = specs
        .as_array()
        .expect("agent tool specs are an array")
        .iter()
        .filter(|spec| {
            spec.pointer("/function/name")
                .and_then(Value::as_str)
                .is_some_and(|name| edit_names.contains(&name))
        })
        .cloned()
        .collect::<Vec<_>>();
    Value::Array(filtered)
}

fn is_navigation_or_reality_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "read_problem_reality"
            | "read_official_failure_reality"
            | "list_files"
            | "inspect_symbol"
            | "search_repo"
            | "open_file_window"
            | "compare_attempts"
            | "retrieve_similar_verified_episodes"
            | "synthesize_helper_tool"
    )
}

fn edit_deadline_active(
    round: u64,
    max_rounds: u64,
    latest_editable_windows: &[Value],
    no_tool_after_window_count: u64,
    repeated_navigation_count: u64,
    submitted_patch: bool,
) -> bool {
    if submitted_patch || latest_editable_windows.is_empty() {
        return false;
    }
    let low_remaining_budget = round >= max_rounds.saturating_sub(6).max(1);
    no_tool_after_window_count > 0 || repeated_navigation_count > 0 || low_remaining_budget
}

fn compact_editable_window(result_readback: &Value) -> Option<Value> {
    let result = result_readback.get("result").unwrap_or(result_readback);
    if result.get("status").and_then(Value::as_str) != Some("ok") {
        return None;
    }
    let payload = result.get("payload").unwrap_or(result);
    if payload
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| status != "ok")
    {
        return None;
    }
    let path = payload.get("path").and_then(Value::as_str)?;
    let sha256 = payload.get("sha256").and_then(Value::as_str)?;
    Some(json!({
        "path": path,
        "start_line": payload.get("start_line"),
        "end_line": payload.get("end_line"),
        "sha256": sha256,
        "source_of_truth": payload.get("source_of_truth"),
        "edit_tool": "apply_line_window_edit",
        "edit_argument_skeleton": {
            "edits": [
                {
                    "path": path,
                    "start_line": "<line number from this opened window>",
                    "end_line": "<line number from this opened window>",
                    "replace": "<complete replacement text for that exact line range>",
                    "observed_sha256": sha256
                }
            ]
        }
    }))
}

fn no_tool_repair_prompt(
    edit_deadline: bool,
    latest_editable_windows: &[Value],
    no_tool_after_window_count: u64,
    repeated_navigation_count: u64,
) -> String {
    if edit_deadline {
        return format!(
            "ContextGraph did not receive a governed tool call. EDIT DEADLINE IS ACTIVE. \
You have already opened source windows, so the next assistant message must call exactly one edit tool: \
apply_line_window_edit, apply_structured_edit, or apply_unified_diff. Do not call read/list/search/open tools again. \
Use apply_unified_diff only if you include observed file hashes from latest_editable_windows. \
Do not answer in prose, raw diff text, or final JSON. Use an observed_sha256 from latest_editable_windows. \
If the edit tool fails, reopen the exact source window in the following round and retry. \
no_tool_after_window_count={no_tool_after_window_count}; repeated_navigation_count={repeated_navigation_count}; \
latest_editable_windows={}",
            serde_json::to_string(latest_editable_windows).unwrap_or_else(|_| "[]".to_string())
        );
    }
    "ContextGraph did not receive a governed tool call. You must call exactly one governed tool now. For attempt 1, use read_problem_reality, search_repo, inspect_symbol, open_file_window, or synthesize_helper_tool. When ready to submit the fix, prefer apply_line_window_edit with line numbers and observed_sha256 from open_file_window, use apply_structured_edit with a unique exact find span, or use apply_unified_diff with observed file hashes. Do not return final JSON or prose.".to_string()
}

fn agent_tool_specs() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "read_problem_reality",
                "description": "Read the durable SWE problem reality packet: oracle tests, base commit, issue terms, implicated files, source/test boundary, and root-cause discovery checklist. Use this before choosing files to edit.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "detail": {"type": "string", "enum": ["compact", "full"], "description": "compact returns only model-facing root-cause evidence; full returns the entire packet."}
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "read_official_failure_reality",
                "description": "Read the latest official SWE-bench failure evidence from durable attempt artifacts. Use this before revising a failed patch.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "attempt": {"type": "integer", "description": "Optional prior attempt number to inspect. Defaults to latest attempt with official evidence."}
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_files",
                "description": "List real repository files by optional substring filter. Use this to navigate the checkout before opening source windows.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "query": {"type": "string", "description": "Optional substring that repo-relative paths must contain."},
                        "max_results": {"type": "integer", "minimum": 1, "maximum": 200}
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "inspect_symbol",
                "description": "Find real source locations for a symbol and return compact file windows with hashes.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["symbol"],
                    "properties": {
                        "symbol": {"type": "string"},
                        "max_results": {"type": "integer", "minimum": 1, "maximum": 20}
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "search_repo",
                "description": "Run a bounded literal git grep over the real checked-out repository.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["query"],
                    "properties": {
                        "query": {"type": "string"},
                        "max_results": {"type": "integer", "minimum": 1, "maximum": 80}
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "open_file_window",
                "description": "Read a real repo-relative file slice by line range with SHA-256 readback.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["path"],
                    "properties": {
                        "path": {"type": "string"},
                        "start_line": {"type": "integer", "minimum": 1},
                        "end_line": {"type": "integer", "minimum": 1}
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "apply_structured_edit",
                "description": "Submit exact find/replace edits as the candidate patch. Use this when the find span is copied from a real source window and matches exactly once. ContextGraph compiles it against the real checkout, verifies the patch artifact, then resets the workspace. A successful call is the final patch submission for this attempt.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["edits"],
                    "properties": {
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
                        }
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "apply_line_window_edit",
                "description": "Preferred final patch submission tool for Gemma coding-agent mode. Replace line ranges from a file returned by open_file_window. You must pass the file sha256 observed from open_file_window so ContextGraph can prove the edit targets the same durable source reality you inspected.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["edits"],
                    "properties": {
                        "edits": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "additionalProperties": false,
                                "required": ["path", "start_line", "end_line", "replace", "observed_sha256"],
                                "properties": {
                                    "path": {"type": "string"},
                                    "start_line": {"type": "integer", "minimum": 1},
                                    "end_line": {"type": "integer", "minimum": 1},
                                    "replace": {"type": "string"},
                                    "observed_sha256": {"type": "string", "description": "The sha256 returned by open_file_window for this file."}
                                }
                            }
                        }
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "apply_unified_diff",
                "description": "Submit a unified diff as the final candidate patch. ContextGraph applies it to the real checkout, verifies changed files against observed source hashes from open_file_window, rejects tests/fixtures/harness changes, runs syntax checks, writes candidate.patch, then resets the workspace.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["patch", "observed_files"],
                    "properties": {
                        "patch": {"type": "string", "description": "A non-empty unified diff generated against the task base commit."},
                        "observed_files": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "additionalProperties": false,
                                "required": ["path", "observed_sha256"],
                                "properties": {
                                    "path": {"type": "string"},
                                    "observed_sha256": {"type": "string", "description": "The sha256 returned by open_file_window for this file before editing."}
                                }
                            }
                        }
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "run_targeted_test",
                "description": "Run one bounded targeted pytest command in the real checkout and return stdout/stderr. Official SWE-bench remains the only success oracle.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["test_id"],
                    "properties": {
                        "test_id": {"type": "string"}
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "run_official_swe_eval",
                "description": "Read or explain official SWE-bench evaluation state. It cannot run before a candidate patch exists.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "attempt": {"type": "integer"}
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "compare_attempts",
                "description": "Compare prior attempts, patches, failure evidence, official results, and repeated-patch signals.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "limit": {"type": "integer", "minimum": 1, "maximum": 20}
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "retrieve_similar_verified_episodes",
                "description": "Retrieve real prior official evidence artifacts for the same repository or instance from the configured runtime root.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "query": {"type": "string"},
                        "max_results": {"type": "integer", "minimum": 1, "maximum": 20}
                    }
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "synthesize_helper_tool",
                "description": "Run a vetted deterministic analyzer. Arbitrary generated code is not executed.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["analyzer"],
                    "properties": {
                        "analyzer": {"type": "string", "enum": ["test_failure_digest", "diff_risk_scan", "repository_file_inventory"]},
                        "question": {"type": "string"}
                    }
                }
            }
        }
    ])
}

fn tool_call_name(call: &Value) -> Result<String> {
    call.pointer("/function/name")
        .or_else(|| call.get("name"))
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(|name| name.trim().to_string())
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_TOOL_CALL_NAME_MISSING",
                "Gemma tool call did not include a function name",
                "gemma_harness.message.tool_calls[].function.name",
                "inspect Gemma tool-call response format",
                json!({"tool_call": call}),
                None,
            )
        })
}

fn tool_call_arguments(call: &Value) -> Result<Value> {
    let raw = call
        .pointer("/function/arguments")
        .or_else(|| call.pointer("/function/parameters"))
        .or_else(|| call.get("arguments"))
        .or_else(|| call.get("parameters"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    match raw {
        Value::Object(_) => Ok(raw),
        Value::String(text) if text.trim().is_empty() => Ok(json!({})),
        Value::String(text) => serde_json::from_str::<Value>(&text).map_err(|err| {
            RealityError::new(
                "RUST_REALITY_TOOL_CALL_ARGUMENTS_JSON_INVALID",
                "Gemma tool call arguments string was not JSON",
                "gemma_harness.message.tool_calls[].function.arguments",
                "inspect model tool-call argument formatting",
                json!({"arguments": text, "error": err.to_string(), "tool_call": call}),
                None,
            )
        }),
        other => Err(RealityError::new(
            "RUST_REALITY_TOOL_CALL_ARGUMENTS_INVALID",
            "Gemma tool call arguments must be a JSON object or JSON object string",
            "gemma_harness.message.tool_calls[].function.arguments",
            "inspect model tool-call argument formatting",
            json!({"arguments": other, "tool_call": call}),
            None,
        )),
    }
}

fn content_json_tool_calls(response: &Value, round: u64) -> Result<Vec<Value>> {
    let Some(content) = response.pointer("/message/content").and_then(Value::as_str) else {
        return Ok(Vec::new());
    };
    let trimmed = content.trim();
    if !trimmed.starts_with('{') {
        return Ok(Vec::new());
    }
    for candidate in json_candidates(trimmed) {
        let Ok(value) = serde_json::from_str::<Value>(&candidate) else {
            continue;
        };
        let Some(obj) = value.as_object() else {
            continue;
        };
        let Some(name) = obj
            .get("tool_name")
            .or_else(|| obj.get("tool"))
            .or_else(|| obj.get("name"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
        else {
            return Ok(Vec::new());
        };
        let arguments = obj
            .get("arguments")
            .or_else(|| obj.get("params"))
            .or_else(|| obj.get("input"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        if !arguments.is_object() {
            return Err(RealityError::new(
                "RUST_REALITY_CONTENT_TOOL_ARGUMENTS_INVALID",
                "content JSON tool request arguments must be a JSON object",
                "gemma_harness.message.content.arguments",
                "emit {\"tool_name\":\"search_repo\",\"arguments\":{\"query\":\"...\"}} or use native tool_calls",
                json!({"content": truncate_chars(trimmed, 2000), "arguments": arguments}),
                None,
            ));
        }
        return Ok(vec![json!({
            "id": format!("content_tool_round_{round:02}_00"),
            "function": {
                "index": 0,
                "name": name,
                "arguments": arguments
            },
            "contextgraph_tool_protocol": "content_json_tool_request"
        })]);
    }
    if trimmed.contains("\"tool_name\"") || trimmed.contains("\"tool\"") {
        return Err(RealityError::new(
            "RUST_REALITY_CONTENT_TOOL_JSON_INVALID",
            "assistant content looked like a tool request but was not valid JSON",
            "gemma_harness.message.content",
            "emit a valid JSON object tool request or use native tool_calls",
            json!({"content": truncate_chars(trimmed, 2000)}),
            None,
        ));
    }
    Ok(Vec::new())
}

fn tool_call_id(call: &Value, round: u64, index: usize) -> String {
    call.get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("tool_round_{round:02}_{index:02}"))
}

fn execute_agent_tool_call(
    ctx: &AgentToolContext<'_>,
    name: &str,
    arguments: &Value,
    tool_dir: &Path,
) -> Result<Value> {
    let started = unix_secs()?;
    let result = match name {
        "read_problem_reality" => tool_read_problem_reality(ctx, arguments),
        "read_official_failure_reality" => tool_read_official_failure_reality(ctx, arguments),
        "list_files" => tool_list_files(ctx, arguments),
        "inspect_symbol" => tool_inspect_symbol(ctx, arguments),
        "search_repo" => tool_search_repo(ctx, arguments),
        "open_file_window" => tool_open_file_window(ctx, arguments),
        "apply_line_window_edit" => tool_apply_line_window_edit(ctx, arguments, tool_dir),
        "apply_structured_edit" => tool_apply_structured_edit(ctx, arguments, tool_dir),
        "apply_unified_diff" => tool_apply_unified_diff(ctx, arguments, tool_dir),
        "run_targeted_test" => tool_run_targeted_test(ctx, arguments),
        "run_official_swe_eval" => tool_run_official_swe_eval(ctx, arguments),
        "compare_attempts" => tool_compare_attempts(ctx, arguments),
        "retrieve_similar_verified_episodes" => {
            tool_retrieve_similar_verified_episodes(ctx, arguments)
        }
        "synthesize_helper_tool" => tool_synthesize_helper_tool(ctx, arguments),
        _ => Ok(tool_blocked(
            name,
            "RUST_REALITY_AGENT_TOOL_UNKNOWN",
            "tool_name",
            "model requested a tool that is not registered in the ContextGraph Rust gateway",
            "choose one of the listed ContextGraph tools",
            json!({"requested_tool": name, "allowed_tools": agent_tool_names()}),
            None,
        )),
    }?;
    let status = result
        .get("status")
        .cloned()
        .unwrap_or_else(|| json!("failed"));
    let error_code = result.get("error_code").cloned().unwrap_or(Value::Null);
    Ok(json!({
        "record_kind": "contextgraph_agent_tool_result",
        "tool_name": name,
        "status": status,
        "error_code": error_code,
        "started_at_unix": started,
        "finished_at_unix": unix_secs()?,
        "arguments": arguments,
        "result": result,
        "source_of_truth": file_sot(&tool_dir.join("tool-result.json"))
    }))
}

fn build_harness_transition(
    ctx: &AgentToolContext<'_>,
    attempt: u64,
    round: u64,
    index: usize,
    executed_tool_calls_before: u64,
    submitted_patch_after_tool: bool,
    tool_call_id: &str,
    tool_name: &str,
    tool_request_protocol: &str,
    arguments: &Value,
    requested_path: &Path,
    response_path: &Path,
    result_path: &Path,
    result_readback: &Value,
    transition_path: &Path,
) -> Result<Value> {
    let repo_head = run_required(
        &["git", "rev-parse", "HEAD"],
        ctx.repo_dir,
        ctx.args.git_timeout,
        "RUST_REALITY_HARNESS_TRANSITION_HEAD_READ_FAILED",
        "harness_transition.before_state.repo_head",
    )?
    .stdout
    .trim()
    .to_string();
    let repo_status = run_required(
        &["git", "status", "--porcelain"],
        ctx.repo_dir,
        ctx.args.git_timeout,
        "RUST_REALITY_HARNESS_TRANSITION_STATUS_READ_FAILED",
        "harness_transition.before_state.repo_status",
    )?
    .stdout;
    let result_status = result_readback
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("failed");
    let error_code = result_readback
        .get("error_code")
        .or_else(|| result_readback.pointer("/result/error_code"))
        .and_then(Value::as_str);
    let mut observed_paths = BTreeSet::new();
    collect_tool_result_paths(result_readback, &mut observed_paths);
    let patch_sha256 = result_readback
        .pointer("/result/patch_sha256")
        .or_else(|| result_readback.get("patch_sha256"))
        .cloned()
        .unwrap_or(Value::Null);
    let candidate_patch_sot = result_readback
        .pointer("/result/compilation/source_of_truth/candidate_patch")
        .or_else(|| result_readback.pointer("/compilation/source_of_truth/candidate_patch"))
        .cloned()
        .unwrap_or(Value::Null);
    let applied_edits = result_readback
        .pointer("/result/compilation/applied_edits")
        .or_else(|| result_readback.pointer("/compilation/applied_edits"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let changed_files = applied_edits
        .iter()
        .filter_map(|edit| edit.get("path").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>();
    let outcome = harness_transition_outcome(
        tool_name,
        result_status,
        error_code,
        submitted_patch_after_tool,
        !observed_paths.is_empty(),
    );
    let transition_basis = json!({
        "attempt": attempt,
        "round": round,
        "index": index,
        "tool_call_id": tool_call_id,
        "tool_name": tool_name,
        "arguments_sha256": sha256_text(&serde_json::to_string(arguments).unwrap()),
        "result_sha256": sha256_file(result_path)?,
        "repo_head": repo_head,
        "result_status": result_status,
        "error_code": error_code
    });
    let transition_id = sha256_text(&serde_json::to_string(&transition_basis).unwrap());
    let action_id = sha256_text(&format!(
        "{}:{}",
        tool_name,
        serde_json::to_string(arguments).unwrap()
    ));
    let before_state_id = sha256_text(&format!(
        "{}:{}:{}:{}",
        ctx.task.instance_id, attempt, executed_tool_calls_before, repo_head
    ));
    let outcome_id = sha256_text(&format!(
        "{}:{}:{}",
        result_status,
        error_code.unwrap_or("none"),
        sha256_file(result_path)?
    ));
    Ok(json!({
        "record_kind": "contextgraph_harness_transition",
        "schema_version": 1,
        "transition_id": transition_id,
        "attempt": attempt,
        "round": round,
        "index": index,
        "tool_call_id": tool_call_id,
        "tool_name": tool_name,
        "tool_request_protocol": tool_request_protocol,
        "before_state": {
            "state_id": before_state_id,
            "repo_head": repo_head,
            "repo_status_porcelain": repo_status,
            "executed_tool_calls_before": executed_tool_calls_before,
            "submitted_patch_after_tool": submitted_patch_after_tool,
            "source_of_truth": {
                "repo_head": format!("git:{}#HEAD", path_str(ctx.repo_dir)),
                "repo_status": format!("git:{}#status", path_str(ctx.repo_dir)),
                "gemma_harness_response": file_sot(response_path)
            }
        },
        "action": {
            "action_id": action_id,
            "arguments_sha256": sha256_text(&serde_json::to_string(arguments).unwrap()),
            "requested": file_sot(requested_path),
            "requested_sha256": sha256_file(requested_path)?,
            "protocol": tool_request_protocol
        },
        "observation": {
            "outcome_id": outcome_id,
            "result": file_sot(result_path),
            "result_sha256": sha256_file(result_path)?,
            "status": result_status,
            "error_code": error_code,
            "observed_paths": observed_paths.into_iter().collect::<Vec<_>>(),
            "changed_files": changed_files.into_iter().collect::<Vec<_>>(),
            "candidate_patch": candidate_patch_sot,
            "patch_sha256": patch_sha256,
            "source_of_truth": file_sot(result_path)
        },
        "outcome": outcome,
        "dynamicjepa_mapping": {
            "domain_pack_id": "contextgraph_swe_reality_loop_v1",
            "state_id": before_state_id,
            "action_id": action_id,
            "outcome_id": outcome_id,
            "transition_id": transition_id,
            "latent_panel_hint": {
                "tool_family": harness_tool_family(tool_name),
                "repo_head": repo_head,
                "attempt": attempt,
                "round": round
            },
            "surprise_kind": if result_status == "ok" { Value::Null } else { json!("UnexpectedOutcome") }
        },
        "learning_signal": harness_learning_signal(tool_name, result_status, error_code, submitted_patch_after_tool, executed_tool_calls_before),
        "constellation_signal": {
            "selector_tags": harness_constellation_tags(tool_name, result_status, error_code),
            "tool_family": harness_tool_family(tool_name),
            "can_cluster_as_success_pattern": result_status == "ok",
            "can_cluster_as_failure_pattern": result_status != "ok"
        },
        "blackboxfix_boundary": {
            "requested": file_sot(requested_path),
            "result": file_sot(result_path),
            "transition": file_sot(transition_path)
        },
        "source_of_truth": file_sot(transition_path)
    }))
}

fn harness_transition_outcome(
    tool_name: &str,
    result_status: &str,
    error_code: Option<&str>,
    submitted_patch_after_tool: bool,
    observed_any_path: bool,
) -> Value {
    let (label, utility_delta, correction_required, reuse_observed) = if result_status != "ok" {
        ("Harmful", -0.35, true, false)
    } else if is_final_edit_tool(tool_name) {
        if submitted_patch_after_tool {
            ("Useful", 0.7, false, true)
        } else {
            ("Harmful", -0.4, true, false)
        }
    } else if observed_any_path
        || matches!(
            tool_name,
            "read_problem_reality"
                | "read_official_failure_reality"
                | "compare_attempts"
                | "retrieve_similar_verified_episodes"
                | "synthesize_helper_tool"
        )
    {
        ("Useful", 0.25, false, true)
    } else {
        ("Neutral", 0.05, false, false)
    };
    json!({
        "label": label,
        "utility_delta": utility_delta,
        "correction_required": correction_required,
        "reuse_observed": reuse_observed,
        "error_code": error_code
    })
}

fn harness_learning_signal(
    tool_name: &str,
    result_status: &str,
    error_code: Option<&str>,
    submitted_patch_after_tool: bool,
    executed_tool_calls_before: u64,
) -> Value {
    let success = result_status == "ok";
    let edit_tool = is_final_edit_tool(tool_name);
    let exploration = matches!(
        tool_name,
        "read_problem_reality"
            | "read_official_failure_reality"
            | "list_files"
            | "inspect_symbol"
            | "search_repo"
            | "open_file_window"
            | "compare_attempts"
            | "retrieve_similar_verified_episodes"
            | "synthesize_helper_tool"
    );
    let delta_e_scalar = if edit_tool && submitted_patch_after_tool {
        0.7
    } else if success && exploration {
        0.25
    } else if success {
        0.1
    } else {
        -0.35
    };
    let surprise_score = if success { 0.0 } else { 1.0 };
    let coherence_delta = if success { 0.15 } else { -0.25 };
    let consolidation_readiness = ((executed_tool_calls_before + 1) as f64 / 8.0).min(1.0);
    let transfer_score = if tool_name == "retrieve_similar_verified_episodes" && success {
        0.5
    } else {
        0.0
    };
    json!({
        "delta_e_scalar": delta_e_scalar,
        "surprise_score": surprise_score,
        "coherence_delta": coherence_delta,
        "consolidation_readiness": consolidation_readiness,
        "transfer_score": transfer_score,
        "productive_surprise": !success && error_code.is_some(),
        "signal_families": {
            "delta_e": delta_e_scalar,
            "surprise": surprise_score,
            "coherence": coherence_delta,
            "consolidation": consolidation_readiness,
            "transfer": transfer_score
        }
    })
}

fn harness_tool_family(tool_name: &str) -> &'static str {
    match tool_name {
        "read_problem_reality" | "read_official_failure_reality" | "compare_attempts" => {
            "reality_readback"
        }
        "list_files" | "inspect_symbol" | "search_repo" | "open_file_window" => {
            "repository_navigation"
        }
        "apply_line_window_edit" | "apply_structured_edit" | "apply_unified_diff" => {
            "workspace_mutation"
        }
        "run_targeted_test" | "run_official_swe_eval" => "verification",
        "retrieve_similar_verified_episodes" => "memory_retrieval",
        "synthesize_helper_tool" => "deterministic_analysis",
        _ => "unknown_tool_family",
    }
}

fn harness_constellation_tags(
    tool_name: &str,
    result_status: &str,
    error_code: Option<&str>,
) -> Vec<String> {
    let mut tags = vec![
        format!("tool_family:{}", harness_tool_family(tool_name)),
        format!("tool:{tool_name}"),
        format!("status:{result_status}"),
    ];
    if let Some(error_code) = error_code {
        tags.push(format!("error_code:{error_code}"));
    }
    tags
}

fn record_agent_tool_boundary(
    ctx: &AgentToolContext<'_>,
    tool_name: &str,
    requested_path: &Path,
    result_path: &Path,
    transition_path: &Path,
    status: &str,
    tool_dir: &Path,
) -> Result<Value> {
    let normalized_status = match status {
        "ok" => "ok",
        "blocked" => "blocked",
        "failed" => "failed",
        _ => "failed",
    };
    let record = blackbox(
        ctx.context_graph_bin,
        &[
            "blackbox",
            "record",
            "tool-call",
            "--ledger",
            path_str(ctx.ledger),
            "--run-id",
            ctx.run_id,
            "--tool",
            &format!("contextgraph_agent_tool.{tool_name}"),
            "--input",
            path_str(requested_path),
            "--agent",
            "contextgraph_rust_reality_loop",
            "--model",
            &ctx.args.model,
            "--capability",
            &format!(
                "contextgraph.rust.reality.agent_tool.{}",
                safe_id(tool_name)
            ),
            "--status",
            normalized_status,
        ],
        ctx.task_root,
    )?;
    let tool_call_id = record
        .get("tool_call_id")
        .and_then(Value::as_str)
        .unwrap_or("tool_call_id_missing")
        .to_string();
    let rel_requested = requested_path
        .strip_prefix(ctx.task_root)
        .unwrap_or(requested_path);
    let rel_result = result_path
        .strip_prefix(ctx.task_root)
        .unwrap_or(result_path);
    let rel_transition = transition_path
        .strip_prefix(ctx.task_root)
        .unwrap_or(transition_path);
    let verifier_spec = json!({
        "verifier_id": "cmd_exit_zero_v1",
        "run_id": ctx.run_id,
        "tool_call_id": tool_call_id,
        "source": {"cwd": path_str(ctx.task_root)},
        "expected": {
            "command": [
                "bash",
                "-c",
                "test -s \"$1\" && test -s \"$2\" && test -s \"$3\" && grep -q '\"record_kind\": \"contextgraph_agent_tool_result\"' \"$2\" && grep -q '\"record_kind\": \"contextgraph_harness_transition\"' \"$3\"",
                "verify-agent-tool-artifacts",
                path_str(rel_requested),
                path_str(rel_result),
                path_str(rel_transition)
            ],
            "artifacts": [
                path_str(rel_requested),
                path_str(rel_result),
                path_str(rel_transition)
            ]
        }
    });
    let spec_path = tool_dir.join("blackbox-agent-tool-verify-spec.json");
    write_json_checked(&spec_path, &verifier_spec)?;
    let verifier = blackbox(
        ctx.context_graph_bin,
        &[
            "blackbox",
            "verify",
            "--ledger",
            path_str(ctx.ledger),
            "--input",
            path_str(&spec_path),
            "--model",
            &ctx.args.model,
        ],
        ctx.task_root,
    )?;
    Ok(json!({
        "record": record,
        "verifier": verifier,
        "verifier_spec": file_sot(&spec_path),
        "source_of_truth": sqlite_sot(ctx.ledger)
    }))
}

fn tool_read_problem_reality(ctx: &AgentToolContext<'_>, arguments: &Value) -> Result<Value> {
    let detail = arguments
        .get("detail")
        .and_then(Value::as_str)
        .unwrap_or("compact");
    let payload = match detail {
        "full" => ctx.problem_reality.clone(),
        "compact" => compact_problem_reality_packet(ctx.problem_reality),
        other => {
            return Ok(tool_blocked(
                "read_problem_reality",
                "RUST_REALITY_TOOL_PROBLEM_REALITY_DETAIL_INVALID",
                "arguments.detail",
                "problem reality detail must be compact or full",
                "use detail=compact unless the full source packet is necessary",
                json!({"detail": other}),
                ctx.problem_reality
                    .get("source_of_truth")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            ));
        }
    };
    Ok(tool_ok(
        "read_problem_reality",
        "read durable SWE problem reality packet",
        payload,
    ))
}

fn tool_read_official_failure_reality(
    ctx: &AgentToolContext<'_>,
    arguments: &Value,
) -> Result<Value> {
    let requested_attempt = arguments.get("attempt").and_then(Value::as_u64);
    let mut candidates: Vec<&AttemptRecord> = ctx
        .attempts
        .iter()
        .filter(|attempt| {
            requested_attempt
                .map(|number| attempt.attempt == number)
                .unwrap_or(true)
        })
        .filter(|attempt| {
            attempt.official_evidence_path.is_some() || attempt.official_reality.is_some()
        })
        .collect();
    candidates.sort_by_key(|attempt| attempt.attempt);
    let Some(attempt) = candidates.last() else {
        return Ok(tool_blocked(
            "read_official_failure_reality",
            "RUST_REALITY_TOOL_OFFICIAL_FAILURE_MISSING",
            "attempts.official_evidence_path",
            "no prior official SWE-bench evidence exists for the requested attempt",
            "use repository tools on attempt 1; after an official failure, call this tool before revising",
            json!({"requested_attempt": requested_attempt, "attempts_total": ctx.attempts.len()}),
            Some(format!("file:{}#attempts", path_str(ctx.attempt_dir))),
        ));
    };
    let evidence = if let Some(path) = &attempt.official_evidence_path {
        let path = PathBuf::from(path);
        let value: Value = read_json(&path)?;
        json!({
            "attempt": attempt.attempt,
            "official_reality": compact_official_evidence(&value),
            "official_evidence": value,
            "source_of_truth": file_sot(&path),
            "sha256": sha256_file(&path)?
        })
    } else {
        json!({
            "attempt": attempt.attempt,
            "official_reality": attempt.official_reality,
            "source_of_truth": attempt.validation_path
        })
    };
    Ok(tool_ok(
        "read_official_failure_reality",
        "read durable official SWE-bench failure evidence",
        evidence,
    ))
}

fn tool_list_files(ctx: &AgentToolContext<'_>, arguments: &Value) -> Result<Value> {
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let max_results = bounded_tool_u64(arguments, "max_results", 120, 1, 200) as usize;
    let result = run_required(
        &["git", "ls-files"],
        ctx.repo_dir,
        120,
        "RUST_REALITY_TOOL_LIST_FILES_FAILED",
        "list_files.git_ls_files",
    )?;
    let mut files = Vec::new();
    for path in result.stdout.lines() {
        if !query.is_empty() && !path.contains(&query) {
            continue;
        }
        if is_forbidden_benchmark_mutation(path) {
            continue;
        }
        files.push(path.to_string());
        if files.len() >= max_results {
            break;
        }
    }
    Ok(tool_ok(
        "list_files",
        "listed real tracked production files in the checked-out repository",
        json!({
            "query": query,
            "files": files,
            "returned": files.len(),
            "max_results": max_results,
            "source_of_truth": format!("git:{}#ls-files", path_str(ctx.repo_dir))
        }),
    ))
}

fn tool_inspect_symbol(ctx: &AgentToolContext<'_>, arguments: &Value) -> Result<Value> {
    // Phase 14 (forward) intervention #2: accept `symbol`, `symbol_name`, or `name`
    // as the symbol argument so a model that confuses the arg name produces a
    // recoverable tool result instead of a fatal RUST_REALITY_TOOL_ARGUMENT_MISSING
    // that aborts the entire attempt. Diagnosed from
    // /tmp/ccreality-prod-3/rust_reality_1778039147061721975_e525987d where Gemma
    // called inspect_symbol with {symbol_name, symbol_type} and the run aborted
    // before writing attempt-summary.json. The previous tool_inspect_symbol used
    // required_tool_str() which returns Err(RealityError) — fatal — for missing
    // args. We now return a tool_blocked envelope (status=failed, recoverable)
    // listing the accepted aliases so the model can reissue.
    let symbol_opt = arguments
        .get("symbol")
        .and_then(Value::as_str)
        .or_else(|| arguments.get("symbol_name").and_then(Value::as_str))
        .or_else(|| arguments.get("name").and_then(Value::as_str))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let symbol = match symbol_opt {
        Some(s) => s,
        None => {
            return Ok(tool_blocked(
                "inspect_symbol",
                "RUST_REALITY_TOOL_INSPECT_SYMBOL_NAME_MISSING",
                "arguments.symbol",
                "inspect_symbol requires a non-empty symbol name",
                "pass `symbol` (preferred) or alias `symbol_name`/`name` with the identifier you want to find",
                json!({
                    "accepted_aliases": ["symbol", "symbol_name", "name"],
                    "received_arguments": arguments,
                }),
                None,
            ));
        }
    };
    if !symbol
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
    {
        return Ok(tool_blocked(
            "inspect_symbol",
            "RUST_REALITY_TOOL_SYMBOL_INVALID",
            "arguments.symbol",
            "symbol contains unsupported characters",
            "use a literal identifier or dotted identifier",
            json!({"symbol": symbol}),
            None,
        ));
    }
    let max_results = bounded_tool_u64(arguments, "max_results", 8, 1, 20) as usize;
    let grep = run_cmd(
        &vec![
            "git".to_string(),
            "grep".to_string(),
            "-n".to_string(),
            "-I".to_string(),
            "--".to_string(),
            symbol.clone(),
        ],
        ctx.repo_dir,
        120,
    )?;
    let mut windows = Vec::new();
    if grep.exit_code == 0 {
        for line in grep.stdout.lines().take(max_results) {
            if let Some((path, line_no, text)) = parse_grep_line(line) {
                if is_forbidden_benchmark_mutation(&path) {
                    continue;
                }
                let start = line_no.saturating_sub(20).max(1);
                let end = line_no + 40;
                windows.push(file_window(ctx.repo_dir, &path, start, end, Some(text))?);
            }
        }
    }
    Ok(tool_ok(
        "inspect_symbol",
        "inspected real repository symbol matches",
        json!({
            "symbol": symbol,
            "matches": windows,
            "grep_exit_code": grep.exit_code,
            "source_of_truth": format!("git:{}#grep:{}", path_str(ctx.repo_dir), symbol)
        }),
    ))
}

fn tool_search_repo(ctx: &AgentToolContext<'_>, arguments: &Value) -> Result<Value> {
    let query = required_tool_str(arguments, "query")?;
    if query.trim().is_empty() {
        return Ok(tool_blocked(
            "search_repo",
            "RUST_REALITY_TOOL_QUERY_EMPTY",
            "arguments.query",
            "search query is empty",
            "provide a non-empty literal term from the problem statement or failure output",
            json!({}),
            None,
        ));
    }
    let max_results = bounded_tool_u64(arguments, "max_results", 40, 1, 80) as usize;
    let grep = run_cmd(
        &vec![
            "git".to_string(),
            "grep".to_string(),
            "-n".to_string(),
            "-I".to_string(),
            "--".to_string(),
            query.clone(),
        ],
        ctx.repo_dir,
        120,
    )?;
    let mut matches = Vec::new();
    if grep.exit_code == 0 {
        for line in grep.stdout.lines().take(max_results) {
            if let Some((path, line_no, text)) = parse_grep_line(line) {
                matches.push(json!({"path": path, "line": line_no, "text": text}));
            }
        }
    }
    Ok(tool_ok(
        "search_repo",
        "searched real repository through git grep",
        json!({
            "query": query,
            "matches": matches,
            "match_count_returned": matches.len(),
            "grep_exit_code": grep.exit_code,
            "source_of_truth": format!("git:{}#grep", path_str(ctx.repo_dir))
        }),
    ))
}

fn tool_open_file_window(ctx: &AgentToolContext<'_>, arguments: &Value) -> Result<Value> {
    let path = required_tool_str(arguments, "path")?;
    let start = bounded_tool_u64(arguments, "start_line", 1, 1, 1_000_000);
    let end = bounded_tool_u64(arguments, "end_line", start + 160, start, start + 240);
    let window = file_window(ctx.repo_dir, &path, start, end, None)?;
    if window.get("status").and_then(Value::as_str) == Some("blocked") {
        return Ok(window);
    }
    Ok(tool_ok(
        "open_file_window",
        "read real file window from the checked-out repository",
        window,
    ))
}

fn tool_apply_structured_edit(
    ctx: &AgentToolContext<'_>,
    arguments: &Value,
    tool_dir: &Path,
) -> Result<Value> {
    let Some(edits) = arguments.get("edits").and_then(Value::as_array) else {
        return Ok(tool_blocked(
            "apply_structured_edit",
            "RUST_REALITY_TOOL_EDITS_MISSING",
            "arguments.edits",
            "structured edit tool requires an edits array",
            "provide exact find/replace edits copied from source windows",
            json!({"arguments": arguments}),
            None,
        ));
    };
    let (patch, compilation) =
        compile_structured_edits(ctx.repo_dir, tool_dir, edits, ctx.args.git_timeout)?;
    reset_workspace(ctx.repo_dir, &ctx.task.base_commit, ctx.args.git_timeout)?;
    let status = if patch.is_some() { "ok" } else { "failed" };
    // Phase 14 (forward): same top-level error lift as apply_line_window_edit.
    let top_error_code = compilation
        .get("error_code")
        .cloned()
        .unwrap_or(Value::Null);
    let top_message = compilation.get("message").cloned().unwrap_or(Value::Null);
    let top_remediation = compilation
        .get("remediation")
        .cloned()
        .unwrap_or(Value::Null);
    let top_field_path = compilation
        .get("field_path")
        .cloned()
        .unwrap_or(Value::Null);
    let syntax_probe_summary = compilation
        .pointer("/details/syntax_probe")
        .or_else(|| compilation.pointer("/syntax_probe"))
        .map(|probe| {
            // The probe writes process.stderr_tail and process.stdout_tail
            // (already truncated by tail_chars). The earlier turn-01 lift
            // pointed at /process/stderr and /process/stdout — fields that
            // do not exist — so syntax_probe_summary surfaced empty strings
            // even when py_compile clearly produced output. This corrects
            // the field names so Gemma actually sees the SyntaxError text.
            // We additionally truncate to 1500 chars to keep the tool result
            // bounded inside the model context window.
            let stderr_tail = probe
                .pointer("/process/stderr_tail")
                .or_else(|| probe.pointer("/process/stderr"))
                .and_then(Value::as_str)
                .map(|s| truncate_chars(s, 1500))
                .unwrap_or_default();
            let stdout_tail = probe
                .pointer("/process/stdout_tail")
                .or_else(|| probe.pointer("/process/stdout"))
                .and_then(Value::as_str)
                .map(|s| truncate_chars(s, 1500))
                .unwrap_or_default();
            let exit_code = probe.pointer("/process/exit_code").and_then(Value::as_i64);
            json!({
                "status": probe.get("status"),
                "error_code": probe.get("error_code"),
                "exit_code": exit_code,
                "stderr_tail": stderr_tail,
                "stdout_tail": stdout_tail,
            })
        })
        .unwrap_or(Value::Null);
    Ok(json!({
        "status": status,
        "tool_name": "apply_structured_edit",
        "action": "compiled structured edits against real checkout, generated patch preview, then reset workspace to base commit",
        "patch_preview": patch.as_ref().map(|text| truncate_chars(text, 4000)),
        "patch_sha256": patch.as_ref().map(|text| sha256_text(text)),
        "error_code": top_error_code,
        "message": top_message,
        "remediation": top_remediation,
        "field_path": top_field_path,
        "syntax_probe_summary": syntax_probe_summary,
        "compilation": compilation,
        "workspace_after": {
            "head": ctx.task.base_commit,
            "source_of_truth": format!("git:{}#HEAD", path_str(ctx.repo_dir))
        }
    }))
}

fn tool_apply_line_window_edit(
    ctx: &AgentToolContext<'_>,
    arguments: &Value,
    tool_dir: &Path,
) -> Result<Value> {
    let Some(edits) = arguments.get("edits").and_then(Value::as_array) else {
        return Ok(tool_blocked(
            "apply_line_window_edit",
            "RUST_REALITY_TOOL_LINE_EDITS_MISSING",
            "arguments.edits",
            "line-window edit tool requires an edits array",
            "provide line edits using path, start_line, end_line, replace, and observed_sha256 from open_file_window",
            json!({"arguments": arguments}),
            None,
        ));
    };
    let (patch, compilation) =
        compile_line_window_edits(ctx.repo_dir, tool_dir, edits, ctx.args.git_timeout)?;
    reset_workspace(ctx.repo_dir, &ctx.task.base_commit, ctx.args.git_timeout)?;
    let status = if patch.is_some() { "ok" } else { "failed" };
    // Phase 14 (forward) intervention: lift error_code/message/remediation/syntax_probe
    // tail from the compilation detail to the top level of the tool result so Gemma
    // sees the actionable failure reason without having to navigate
    // result.compilation.details.syntax_probe. This is a tool-grounding fix grounded
    // in the diagnosis from /tmp/ccreality-prod-3 attempt-000001 (rounds 4-7) where
    // three apply_line_window_edit calls failed with RUST_REALITY_LINE_EDIT_PY_COMPILE_FAILED
    // but the tool result reported {status:failed, error_code:null, message:null,
    // remediation:null} — leaving the model unable to self-correct.
    let top_error_code = compilation
        .get("error_code")
        .cloned()
        .unwrap_or(Value::Null);
    let top_message = compilation.get("message").cloned().unwrap_or(Value::Null);
    let top_remediation = compilation
        .get("remediation")
        .cloned()
        .unwrap_or(Value::Null);
    let top_field_path = compilation
        .get("field_path")
        .cloned()
        .unwrap_or(Value::Null);
    let syntax_probe_summary = compilation
        .pointer("/details/syntax_probe")
        .or_else(|| compilation.pointer("/syntax_probe"))
        .map(|probe| {
            // The probe writes process.stderr_tail and process.stdout_tail
            // (already truncated by tail_chars). The earlier turn-01 lift
            // pointed at /process/stderr and /process/stdout — fields that
            // do not exist — so syntax_probe_summary surfaced empty strings
            // even when py_compile clearly produced output. This corrects
            // the field names so Gemma actually sees the SyntaxError text.
            // We additionally truncate to 1500 chars to keep the tool result
            // bounded inside the model context window.
            let stderr_tail = probe
                .pointer("/process/stderr_tail")
                .or_else(|| probe.pointer("/process/stderr"))
                .and_then(Value::as_str)
                .map(|s| truncate_chars(s, 1500))
                .unwrap_or_default();
            let stdout_tail = probe
                .pointer("/process/stdout_tail")
                .or_else(|| probe.pointer("/process/stdout"))
                .and_then(Value::as_str)
                .map(|s| truncate_chars(s, 1500))
                .unwrap_or_default();
            let exit_code = probe.pointer("/process/exit_code").and_then(Value::as_i64);
            json!({
                "status": probe.get("status"),
                "error_code": probe.get("error_code"),
                "exit_code": exit_code,
                "stderr_tail": stderr_tail,
                "stdout_tail": stdout_tail,
            })
        })
        .unwrap_or(Value::Null);
    Ok(json!({
        "status": status,
        "tool_name": "apply_line_window_edit",
        "action": "compiled line-window edits against real checkout, generated patch preview, then reset workspace to base commit",
        "patch_preview": patch.as_ref().map(|text| truncate_chars(text, 4000)),
        "patch_sha256": patch.as_ref().map(|text| sha256_text(text)),
        "error_code": top_error_code,
        "message": top_message,
        "remediation": top_remediation,
        "field_path": top_field_path,
        "syntax_probe_summary": syntax_probe_summary,
        "compilation": compilation,
        "workspace_after": {
            "head": ctx.task.base_commit,
            "source_of_truth": format!("git:{}#HEAD", path_str(ctx.repo_dir))
        }
    }))
}

fn tool_apply_unified_diff(
    ctx: &AgentToolContext<'_>,
    arguments: &Value,
    tool_dir: &Path,
) -> Result<Value> {
    let Some(patch) = arguments.get("patch").and_then(Value::as_str) else {
        return Ok(tool_blocked(
            "apply_unified_diff",
            "RUST_REALITY_TOOL_UNIFIED_DIFF_MISSING",
            "arguments.patch",
            "unified diff tool requires a non-empty patch string",
            "provide a unified diff generated against the task base commit",
            json!({"arguments": arguments}),
            None,
        ));
    };
    let Some(observed_files) = arguments.get("observed_files").and_then(Value::as_array) else {
        return Ok(tool_blocked(
            "apply_unified_diff",
            "RUST_REALITY_TOOL_UNIFIED_DIFF_OBSERVED_FILES_MISSING",
            "arguments.observed_files",
            "unified diff tool requires observed file hashes from open_file_window",
            "open each file changed by the diff and include path plus observed_sha256",
            json!({"arguments": arguments}),
            None,
        ));
    };
    let (compiled_patch, compilation) = compile_unified_diff_submission(
        ctx.repo_dir,
        tool_dir,
        patch,
        observed_files,
        ctx.args.git_timeout,
    )?;
    reset_workspace(ctx.repo_dir, &ctx.task.base_commit, ctx.args.git_timeout)?;
    let status = if compiled_patch.is_some() {
        "ok"
    } else {
        "failed"
    };
    let top_error_code = compilation
        .get("error_code")
        .cloned()
        .unwrap_or(Value::Null);
    let top_message = compilation.get("message").cloned().unwrap_or(Value::Null);
    let top_remediation = compilation
        .get("remediation")
        .cloned()
        .unwrap_or(Value::Null);
    let top_field_path = compilation
        .get("field_path")
        .cloned()
        .unwrap_or(Value::Null);
    Ok(json!({
        "status": status,
        "tool_name": "apply_unified_diff",
        "action": "applied unified diff against real checkout, verified observed file hashes, generated normalized candidate patch, then reset workspace to base commit",
        "patch_preview": compiled_patch.as_ref().map(|text| truncate_chars(text, 4000)),
        "patch_sha256": compiled_patch.as_ref().map(|text| sha256_text(text)),
        "error_code": top_error_code,
        "message": top_message,
        "remediation": top_remediation,
        "field_path": top_field_path,
        "compilation": compilation,
        "workspace_after": {
            "head": ctx.task.base_commit,
            "source_of_truth": format!("git:{}#HEAD", path_str(ctx.repo_dir))
        }
    }))
}

fn materialize_tool_candidate_submission(result: &Value, tool_dir: &Path) -> Result<Value> {
    let inner = result.get("result").unwrap_or(result);
    let candidate_sot = result
        .pointer("/result/compilation/source_of_truth/candidate_patch")
        .or_else(|| inner.pointer("/compilation/source_of_truth/candidate_patch"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_TOOL_SUBMITTED_PATCH_SOT_MISSING",
                "edit tool returned ok without a candidate patch source of truth",
                "agent_tool_result.result.compilation.source_of_truth.candidate_patch",
                "inspect the persisted tool result wrapper and edit compiler output",
                json!({"result": result}),
                None,
            )
        })?;
    let candidate_path = file_sot_to_path(
        candidate_sot,
        "RUST_REALITY_TOOL_SUBMITTED_PATCH_SOT_INVALID",
        "agent_tool_result.result.compilation.source_of_truth.candidate_patch",
    )?;
    let candidate_path = resolve_path(&candidate_path)?;
    let tool_dir = resolve_path(tool_dir)?;
    if !candidate_path.starts_with(&tool_dir) {
        return Err(RealityError::new(
            "RUST_REALITY_TOOL_SUBMITTED_PATCH_PATH_OUTSIDE_TOOL_DIR",
            "edit tool candidate patch was outside the governed tool artifact directory",
            "agent_tool_result.result.compilation.source_of_truth.candidate_patch",
            "reject the tool result and inspect path provenance",
            json!({
                "candidate_patch_path": path_str(&candidate_path),
                "tool_dir": path_str(&tool_dir)
            }),
            Some(candidate_sot.to_string()),
        ));
    }
    require_file(
        &candidate_path,
        "agent_tool_result.result.candidate_patch",
        "RUST_REALITY_TOOL_SUBMITTED_PATCH_MISSING",
    )?;
    let patch_text = fs::read_to_string(&candidate_path).map_err(|err| {
        fs_error(
            "RUST_REALITY_TOOL_SUBMITTED_PATCH_READ_FAILED",
            &candidate_path,
            err,
        )
    })?;
    if patch_text.trim().is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_TOOL_SUBMITTED_PATCH_EMPTY",
            "edit tool candidate patch was empty after source-of-truth readback",
            "agent_tool_result.result.candidate_patch",
            "submit edits that produce a non-empty production-source diff",
            json!({"candidate_patch_path": path_str(&candidate_path)}),
            Some(file_sot(&candidate_path)),
        ));
    }
    let actual_sha = sha256_file(&candidate_path)?;
    let expected_sha = result
        .pointer("/result/patch_sha256")
        .or_else(|| inner.get("patch_sha256"))
        .or_else(|| result.pointer("/result/compilation/candidate_patch_sha256"))
        .or_else(|| inner.pointer("/compilation/candidate_patch_sha256"))
        .and_then(Value::as_str);
    if let Some(expected_sha) = expected_sha {
        if expected_sha != actual_sha {
            return Err(RealityError::new(
                "RUST_REALITY_TOOL_SUBMITTED_PATCH_SHA_MISMATCH",
                "edit tool patch hash did not match file readback",
                "agent_tool_result.result.patch_sha256",
                "reject the submitted patch and inspect artifact integrity",
                json!({
                    "expected": expected_sha,
                    "actual": actual_sha,
                    "candidate_patch": file_sot(&candidate_path)
                }),
                Some(file_sot(&candidate_path)),
            ));
        }
    }
    Ok(json!({
        "record_kind": "contextgraph_tool_submitted_candidate_patch",
        "status": "ok",
        "tool_name": result.get("tool_name").or_else(|| inner.get("tool_name")).and_then(Value::as_str).unwrap_or("final_edit_submission"),
        "candidate_patch_path": path_str(&candidate_path),
        "candidate_patch": file_sot(&candidate_path),
        "patch_sha256": actual_sha,
        "patch_stats": patch_line_stats(&patch_text),
        "structured_edits": result.pointer("/result/compilation/source_of_truth/structured_edits").or_else(|| inner.pointer("/compilation/source_of_truth/structured_edits")),
        "compilation": {
            "status": result.pointer("/result/compilation/status").or_else(|| inner.pointer("/compilation/status")),
            "message": result.pointer("/result/compilation/message").or_else(|| inner.pointer("/compilation/message")),
            "applied_edits": result.pointer("/result/compilation/applied_edits").or_else(|| inner.pointer("/compilation/applied_edits"))
        },
        "source_of_truth": {
            "candidate_patch": file_sot(&candidate_path),
            "tool_artifact_dir": file_sot(&tool_dir)
        }
    }))
}

fn compact_failed_candidate_submission(result: &Value, result_path: &Path) -> Value {
    let compilation = result
        .get("result")
        .and_then(|value| value.get("compilation"));
    json!({
        "record_kind": "contextgraph_failed_candidate_patch_submission",
        "tool_name": result
            .get("tool_name")
            .or_else(|| result.get("result").and_then(|value| value.get("tool_name")))
            .and_then(Value::as_str)
            .unwrap_or("final_edit_submission"),
        "status": result.get("status"),
        "error_code": result.get("error_code").or_else(|| compilation.and_then(|value| value.get("error_code"))),
        "field_path": compilation.and_then(|value| value.get("field_path")),
        "message": compilation.and_then(|value| value.get("message")),
        "remediation": compilation.and_then(|value| value.get("remediation")),
        "details": compilation.and_then(|value| value.get("details")),
        "source_of_truth": file_sot(result_path)
    })
}

fn tool_run_targeted_test(ctx: &AgentToolContext<'_>, arguments: &Value) -> Result<Value> {
    let test_id = required_tool_str(arguments, "test_id")?;
    if test_id.trim().is_empty() || test_id.contains('\n') || test_id.contains('\0') {
        return Ok(tool_blocked(
            "run_targeted_test",
            "RUST_REALITY_TOOL_TEST_ID_INVALID",
            "arguments.test_id",
            "test id is empty or contains invalid control characters",
            "use one exact FAIL_TO_PASS test id from the task manifest or official evidence",
            json!({"test_id": test_id}),
            None,
        ));
    }
    let command = vec![
        LOCAL_PYTHON_BIN.to_string(),
        "-m".to_string(),
        "pytest".to_string(),
        "-q".to_string(),
        test_id.clone(),
    ];
    let process = run_cmd(&command, ctx.repo_dir, ctx.args.targeted_test_timeout)?;
    Ok(json!({
        "status": if process.exit_code == 0 { "ok" } else { "failed" },
        "action": "ran bounded targeted pytest in the real checkout",
        "test_id": test_id,
        "process": {
            "command": process.command,
            "cwd": process.cwd,
            "exit_code": process.exit_code,
            "elapsed_ms": process.elapsed_ms,
            "stdout_tail": tail_chars(&process.stdout, 6000),
            "stderr_tail": tail_chars(&process.stderr, 6000)
        },
        "source_of_truth": format!("process:{LOCAL_PYTHON_BIN} -m pytest:{}", path_str(ctx.repo_dir))
    }))
}

fn tool_run_official_swe_eval(ctx: &AgentToolContext<'_>, arguments: &Value) -> Result<Value> {
    let requested_attempt = arguments.get("attempt").and_then(Value::as_u64);
    let mut candidates: Vec<&AttemptRecord> = ctx
        .attempts
        .iter()
        .filter(|attempt| {
            requested_attempt
                .map(|number| attempt.attempt == number)
                .unwrap_or(true)
        })
        .filter(|attempt| attempt.official_evidence_path.is_some())
        .collect();
    candidates.sort_by_key(|attempt| attempt.attempt);
    if let Some(attempt) = candidates.last() {
        let path = PathBuf::from(attempt.official_evidence_path.as_ref().unwrap());
        let evidence: Value = read_json(&path)?;
        return Ok(tool_ok(
            "run_official_swe_eval",
            "read existing official SWE-bench evaluation evidence",
            json!({
                "attempt": attempt.attempt,
                "resolved": attempt.resolved,
                "official_evidence": compact_official_evidence(&evidence),
                "source_of_truth": file_sot(&path),
                "sha256": sha256_file(&path)?
            }),
        ));
    }
    Ok(tool_blocked(
        "run_official_swe_eval",
        "RUST_REALITY_TOOL_OFFICIAL_EVAL_REQUIRES_CANDIDATE_PATCH",
        "candidate.patch",
        "official SWE-bench evaluation cannot run before the model has produced a candidate patch",
        "call apply_line_window_edit, apply_structured_edit, or apply_unified_diff; the Rust loop will run official SWE-bench evaluation after validation passes",
        json!({"requested_attempt": requested_attempt, "attempts_total": ctx.attempts.len()}),
        Some(format!("file:{}#attempts", path_str(ctx.attempt_dir))),
    ))
}

fn tool_compare_attempts(ctx: &AgentToolContext<'_>, arguments: &Value) -> Result<Value> {
    let limit = bounded_tool_u64(arguments, "limit", 8, 1, 20) as usize;
    let mut attempts: Vec<Value> = ctx
        .attempts
        .iter()
        .rev()
        .take(limit)
        .map(|attempt| {
            json!({
                "attempt": attempt.attempt,
                "status": attempt.status,
                "error_code": attempt.error_code,
                "message": attempt.message,
                "patch_sha256": attempt.patch_sha256,
                "repeated_patch": attempt.repeated_patch,
                "resolved": attempt.resolved,
                "failure_evidence": attempt.failure_evidence,
                "official_reality": attempt.official_reality,
                "patch_preview": attempt.patch_preview,
                "validation_path": attempt.validation_path,
                "official_evidence_path": attempt.official_evidence_path
            })
        })
        .collect();
    attempts.reverse();
    Ok(tool_ok(
        "compare_attempts",
        "compared persisted prior attempt records",
        json!({
            "attempts_total": ctx.attempts.len(),
            "returned": attempts,
            "source_of_truth": format!("memory:attempts:{}#{}", ctx.task.instance_id, path_str(ctx.attempt_dir))
        }),
    ))
}

fn tool_retrieve_similar_verified_episodes(
    ctx: &AgentToolContext<'_>,
    arguments: &Value,
) -> Result<Value> {
    let max_results = bounded_tool_u64(arguments, "max_results", 8, 1, 20) as usize;
    let query = arguments
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or(&ctx.task.instance_id)
        .to_string();
    let runtime_root = resolve_path(&ctx.args.runtime_root)?;
    let mut evidence_files = Vec::new();
    collect_evidence_files(&runtime_root, &mut evidence_files)?;
    evidence_files.sort();
    let mut episodes = Vec::new();
    for path in evidence_files {
        if episodes.len() >= max_results {
            break;
        }
        let text = fs::read_to_string(&path)
            .map_err(|err| fs_error("RUST_REALITY_EPISODE_READ_FAILED", &path, err))?;
        if !text.contains(&ctx.task.instance_id)
            && !text.contains(&ctx.task.repo)
            && !query.is_empty()
            && !text.contains(&query)
        {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            episodes.push(json!({
                "path": path_str(&path),
                "sha256": sha256_file(&path)?,
                "resolved": value.get("resolved"),
                "instance_id": value.get("instance_id"),
                "official_reality": compact_official_evidence(&value),
                "source_of_truth": file_sot(&path)
            }));
        }
    }
    Ok(tool_ok(
        "retrieve_similar_verified_episodes",
        "retrieved real prior official evidence artifacts from runtime root",
        json!({
            "query": query,
            "runtime_root": path_str(&runtime_root),
            "episodes": episodes,
            "source_of_truth": file_sot(&runtime_root)
        }),
    ))
}

fn tool_synthesize_helper_tool(ctx: &AgentToolContext<'_>, arguments: &Value) -> Result<Value> {
    let analyzer = required_tool_str(arguments, "analyzer")?;
    let payload = match analyzer.as_str() {
        "test_failure_digest" => json!({
            "analyzer": analyzer,
            "fail_to_pass": ctx.task.fail_to_pass,
            "latest_official_reality": ctx.attempts.iter().rev().find_map(|attempt| attempt.official_reality.clone()),
            "latest_failure_evidence": ctx.attempts.last().map(|attempt| attempt.failure_evidence.clone()),
            "source_of_truth": ctx.task.source_of_truth
        }),
        "diff_risk_scan" => json!({
            "analyzer": analyzer,
            "forbidden_patterns": [
                "tests/",
                "/tests/",
                "test_",
                "_test.py",
                "conftest.py",
                "tox.ini",
                "pytest"
            ],
            "previous_patch_hashes": ctx.attempts.iter().filter_map(|attempt| attempt.patch_sha256.clone()).collect::<Vec<_>>(),
            "source_of_truth": format!("memory:attempts:{}#patches", ctx.task.instance_id)
        }),
        "repository_file_inventory" => json!({
            "analyzer": analyzer,
            "repository_context": ctx.repo_context,
            "source_of_truth": ctx.repo_context.get("source_of_truth")
        }),
        _ => {
            return Ok(tool_blocked(
                "synthesize_helper_tool",
                "RUST_REALITY_TOOL_ANALYZER_UNKNOWN",
                "arguments.analyzer",
                "requested analyzer is not one of the vetted deterministic analyzers",
                "choose test_failure_digest, diff_risk_scan, or repository_file_inventory",
                json!({"analyzer": analyzer}),
                None,
            ));
        }
    };
    Ok(tool_ok(
        "synthesize_helper_tool",
        "ran vetted deterministic analyzer without executing model-generated code",
        payload,
    ))
}

fn tool_ok(tool_name: &str, action: &str, payload: Value) -> Value {
    json!({
        "status": "ok",
        "tool_name": tool_name,
        "action": action,
        "payload": payload
    })
}

fn tool_blocked(
    tool_name: &str,
    error_code: &str,
    field_path: &str,
    message: &str,
    remediation: &str,
    details: Value,
    source_of_truth: Option<String>,
) -> Value {
    json!({
        "status": "blocked",
        "tool_name": tool_name,
        "error_code": error_code,
        "field_path": field_path,
        "message": message,
        "remediation": remediation,
        "details": details,
        "source_of_truth": source_of_truth
    })
}

fn required_tool_str(arguments: &Value, field: &str) -> Result<String> {
    arguments
        .get(field)
        .and_then(Value::as_str)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_TOOL_ARGUMENT_MISSING",
                "required tool argument is missing",
                format!("arguments.{field}"),
                "inspect model tool-call arguments",
                json!({"field": field, "arguments": arguments}),
                None,
            )
        })
}

fn bounded_tool_u64(arguments: &Value, field: &str, default: u64, min: u64, max: u64) -> u64 {
    arguments
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or(default)
        .clamp(min, max)
}

fn parse_grep_line(line: &str) -> Option<(String, u64, String)> {
    let mut parts = line.splitn(3, ':');
    let path = parts.next()?.to_string();
    let line_no = parts.next()?.parse::<u64>().ok()?;
    let text = parts.next().unwrap_or("").to_string();
    Some((path, line_no, text))
}

fn file_window(
    repo_dir: &Path,
    relative_path: &str,
    start_line: u64,
    end_line: u64,
    matched_line: Option<String>,
) -> Result<Value> {
    if relative_path.starts_with('/')
        || relative_path.contains('\\')
        || Path::new(relative_path)
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Ok(tool_blocked(
            "open_file_window",
            "RUST_REALITY_TOOL_PATH_INVALID",
            "arguments.path",
            "path must be repo-relative without parent traversal",
            "choose a path returned by search_repo or repository context",
            json!({"path": relative_path}),
            None,
        ));
    }
    let path = repo_dir.join(relative_path);
    if !path.exists() || !path.is_file() {
        return Ok(tool_blocked(
            "open_file_window",
            "RUST_REALITY_TOOL_FILE_MISSING",
            "arguments.path",
            "requested file does not exist in the real checkout",
            "choose a path returned by search_repo or repository context",
            json!({"path": relative_path, "source_of_truth": file_sot(&path)}),
            Some(file_sot(&path)),
        ));
    }
    let text = fs::read_to_string(&path)
        .map_err(|err| fs_error("RUST_REALITY_TOOL_FILE_READ_FAILED", &path, err))?;
    let lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.len() as u64;
    let start = start_line.max(1).min(total_lines.max(1));
    let end = end_line
        .max(start)
        .min((start + 240).min(total_lines.max(start)));
    let slice = if total_lines == 0 {
        String::new()
    } else {
        lines[(start - 1) as usize..end as usize].join("\n")
    };
    Ok(json!({
        "status": "ok",
        "path": relative_path,
        "start_line": start,
        "end_line": end,
        "total_lines": total_lines,
        "text": slice,
        "matched_line": matched_line,
        "sha256": sha256_file(&path)?,
        "source_of_truth": file_sot(&path)
    }))
}

fn collect_evidence_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in
        fs::read_dir(root).map_err(|err| fs_error("RUST_REALITY_READ_DIR_FAILED", root, err))?
    {
        let entry =
            entry.map_err(|err| fs_error("RUST_REALITY_READ_DIR_ENTRY_FAILED", root, err))?;
        let path = entry.path();
        if path.is_dir() {
            collect_evidence_files(&path, out)?;
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name == "official-failure-evidence.json" || name == "official-success-evidence.json"
            })
        {
            out.push(path);
        }
    }
    Ok(())
}

fn tail_chars(value: &str, limit: usize) -> String {
    let len = value.chars().count();
    if len <= limit {
        value.to_string()
    } else {
        value.chars().skip(len - limit).collect()
    }
}

fn patch_from_tool_submission(model_response: &Value) -> Result<Option<(String, Value)>> {
    let Some(submission) = model_response
        .pointer("/contextgraph_runtime/submitted_candidate_patch")
        .filter(|value| !value.is_null())
    else {
        return Ok(None);
    };
    let path = submission
        .get("candidate_patch_path")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| {
            submission
                .get("candidate_patch")
                .and_then(Value::as_str)
                .and_then(|sot| file_sot_to_path(sot, "", "").ok())
        })
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_TOOL_SUBMITTED_PATCH_PATH_MISSING",
                "tool-mediated model response did not include a candidate patch path",
                "contextgraph_runtime.submitted_candidate_patch.candidate_patch_path",
                "inspect apply_structured_edit result materialization",
                json!({"submitted_candidate_patch": submission}),
                None,
            )
        })?;
    require_file(
        &path,
        "contextgraph_runtime.submitted_candidate_patch.candidate_patch_path",
        "RUST_REALITY_TOOL_SUBMITTED_PATCH_READBACK_MISSING",
    )?;
    let patch = fs::read_to_string(&path).map_err(|err| {
        fs_error(
            "RUST_REALITY_TOOL_SUBMITTED_PATCH_READBACK_FAILED",
            &path,
            err,
        )
    })?;
    if patch.trim().is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_TOOL_SUBMITTED_PATCH_READBACK_EMPTY",
            "tool-mediated candidate patch file was empty on final readback",
            "contextgraph_runtime.submitted_candidate_patch.candidate_patch_path",
            "submit exact edits that produce a non-empty diff",
            json!({"candidate_patch_path": path_str(&path)}),
            Some(file_sot(&path)),
        ));
    }
    let actual_sha = sha256_file(&path)?;
    if let Some(expected_sha) = submission.get("patch_sha256").and_then(Value::as_str) {
        if expected_sha != actual_sha {
            return Err(RealityError::new(
                "RUST_REALITY_TOOL_SUBMITTED_PATCH_FINAL_SHA_MISMATCH",
                "tool-mediated candidate patch hash changed before final readback",
                "contextgraph_runtime.submitted_candidate_patch.patch_sha256",
                "reject the candidate patch and inspect artifact integrity",
                json!({
                    "expected": expected_sha,
                    "actual": actual_sha,
                    "candidate_patch": file_sot(&path)
                }),
                Some(file_sot(&path)),
            ));
        }
    }
    Ok(Some((
        patch,
        json!({
            "status": "passed",
            "error_code": null,
            "field_path": "agent_tools.final_edit_submission",
            "message": "candidate patch was submitted through a governed final edit tool and read back from its durable artifact",
            "final_action_mode": "tool_mediated_final_edit_submission",
            "tool_submitted_candidate_patch": submission,
            "source_of_truth": file_sot(&path)
        }),
    )))
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
    let mut original_contents = BTreeMap::new();

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
        original_contents
            .entry(path.to_string())
            .or_insert_with(|| before.clone());
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

    let syntax_probe = python_syntax_probe_after_candidate_edits(
        repo_dir,
        attempt_dir,
        "structured-edit",
        &applied,
        timeout,
    )?;
    if syntax_probe.get("status").and_then(Value::as_str) == Some("failed") {
        let restore = restore_original_candidate_sources(
            repo_dir,
            attempt_dir,
            "structured-edit",
            &original_contents,
        )?;
        return Ok(structured_edit_failure(
            "RUST_REALITY_STRUCTURED_EDIT_PY_COMPILE_FAILED",
            "model_response.edits",
            "structured edits produced syntactically invalid Python source before candidate patch generation",
            "reopen the edited source window, preserve Python syntax, and resubmit a candidate patch only after the syntax probe passes",
            json!({
                "applied_edits": applied,
                "syntax_probe": syntax_probe,
                "workspace_restore": restore
            }),
            &edits_path,
        ));
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
            "syntax_probe": syntax_probe,
            "candidate_patch_sha256": sha256_file(&patch_path)?,
            "source_of_truth": {
                "structured_edits": file_sot(&edits_path),
                "candidate_patch": file_sot(&patch_path),
                "workspace_diff": format!("git:{}#diff", path_str(repo_dir))
            }
        }),
    ))
}

fn compile_line_window_edits(
    repo_dir: &Path,
    attempt_dir: &Path,
    edits: &[Value],
    timeout: u64,
) -> Result<(Option<String>, Value)> {
    let edits_path = attempt_dir.join("line-window-edits.json");
    write_json_checked(&edits_path, &edits)?;
    let mut applied = Vec::new();
    let mut original_contents = BTreeMap::new();

    for (index, edit) in edits.iter().enumerate() {
        let field_prefix = format!("model_response.line_edits[{index}]");
        let path = match edit.get("path").and_then(Value::as_str) {
            Some(value) if !value.trim().is_empty() => value.trim(),
            _ => {
                return Ok(structured_edit_failure(
                    "RUST_REALITY_LINE_EDIT_PATH_MISSING",
                    &format!("{field_prefix}.path"),
                    "line-window edit path is missing",
                    "return a relative production-source path from open_file_window",
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
                "RUST_REALITY_LINE_EDIT_PATH_INVALID",
                &format!("{field_prefix}.path"),
                "line-window edit path must be a relative repository path",
                "return a normal repo-relative path without absolute roots, parent traversal, or backslashes",
                json!({"path": path}),
                &edits_path,
            ));
        }
        if is_forbidden_benchmark_mutation(path) {
            return Ok(structured_edit_failure(
                "RUST_REALITY_LINE_EDIT_FORBIDDEN_PATH",
                &format!("{field_prefix}.path"),
                "line-window edit targets tests, fixtures, or benchmark harness files",
                "repair production source only; do not change tests, fixtures, or evaluation harnesses",
                json!({"path": path}),
                &edits_path,
            ));
        }
        let start_line = match edit.get("start_line").and_then(Value::as_u64) {
            Some(value) if value >= 1 => value,
            _ => {
                return Ok(structured_edit_failure(
                    "RUST_REALITY_LINE_EDIT_START_MISSING",
                    &format!("{field_prefix}.start_line"),
                    "line-window edit start_line is missing or invalid",
                    "use the line numbers returned by open_file_window",
                    json!({"edit": edit}),
                    &edits_path,
                ));
            }
        };
        let end_line = match edit.get("end_line").and_then(Value::as_u64) {
            Some(value) if value >= start_line => value,
            _ => {
                return Ok(structured_edit_failure(
                    "RUST_REALITY_LINE_EDIT_END_INVALID",
                    &format!("{field_prefix}.end_line"),
                    "line-window edit end_line is missing or before start_line",
                    "use the inclusive line range returned by open_file_window",
                    json!({"edit": edit}),
                    &edits_path,
                ));
            }
        };
        if end_line - start_line > 240 {
            return Ok(structured_edit_failure(
                "RUST_REALITY_LINE_EDIT_RANGE_TOO_LARGE",
                &format!("{field_prefix}.end_line"),
                "line-window edit range is too large",
                "open a smaller source window and replace at most 241 lines at a time",
                json!({"path": path, "start_line": start_line, "end_line": end_line}),
                &edits_path,
            ));
        }
        let observed_sha256 = match edit.get("observed_sha256").and_then(Value::as_str) {
            Some(value) if value.starts_with("sha256:") => value,
            _ => {
                return Ok(structured_edit_failure(
                    "RUST_REALITY_LINE_EDIT_OBSERVED_SHA_MISSING",
                    &format!("{field_prefix}.observed_sha256"),
                    "line-window edit must include observed_sha256 from open_file_window",
                    "call open_file_window for this file and copy its sha256 into observed_sha256",
                    json!({"path": path}),
                    &edits_path,
                ));
            }
        };
        let replace = match edit.get("replace").and_then(Value::as_str) {
            Some(value) => value,
            _ => {
                return Ok(structured_edit_failure(
                    "RUST_REALITY_LINE_EDIT_REPLACE_MISSING",
                    &format!("{field_prefix}.replace"),
                    "line-window edit replace text is missing",
                    "return the exact replacement source text for the requested line range",
                    json!({"path": path, "start_line": start_line, "end_line": end_line}),
                    &edits_path,
                ));
            }
        };
        let target = repo_dir.join(path);
        if !target.exists() || !target.is_file() {
            return Ok(structured_edit_failure(
                "RUST_REALITY_LINE_EDIT_TARGET_MISSING",
                &format!("{field_prefix}.path"),
                "line-window edit target file does not exist",
                "choose a real production-source file from open_file_window or list_files",
                json!({"path": path, "source_of_truth": file_sot(&target)}),
                &edits_path,
            ));
        }
        let before = match fs::read_to_string(&target) {
            Ok(value) => value,
            Err(err) => {
                return Ok(structured_edit_failure(
                    "RUST_REALITY_LINE_EDIT_TARGET_READ_FAILED",
                    &format!("{field_prefix}.path"),
                    "line-window edit target could not be read as UTF-8 text",
                    "choose a text source file and avoid binary/generated artifacts",
                    json!({"path": path, "error": err.to_string()}),
                    &edits_path,
                ));
            }
        };
        let actual_sha256 = sha256_file(&target)?;
        if actual_sha256 != observed_sha256 {
            return Ok(structured_edit_failure(
                "RUST_REALITY_LINE_EDIT_OBSERVED_SHA_MISMATCH",
                &format!("{field_prefix}.observed_sha256"),
                "line-window edit observed_sha256 does not match current file readback",
                "reopen the file window and submit an edit against the current source reality",
                json!({
                    "path": path,
                    "expected": observed_sha256,
                    "actual": actual_sha256,
                    "source_of_truth": file_sot(&target)
                }),
                &edits_path,
            ));
        }
        let spans = line_spans(&before);
        if spans.is_empty() || end_line as usize > spans.len() {
            return Ok(structured_edit_failure(
                "RUST_REALITY_LINE_EDIT_RANGE_OUT_OF_BOUNDS",
                &format!("{field_prefix}.end_line"),
                "line-window edit range is outside current file bounds",
                "reopen the file window and use a valid inclusive range",
                json!({
                    "path": path,
                    "start_line": start_line,
                    "end_line": end_line,
                    "total_lines": spans.len(),
                    "source_of_truth": file_sot(&target)
                }),
                &edits_path,
            ));
        }
        let byte_start = spans[(start_line - 1) as usize].0;
        let byte_end = spans[(end_line - 1) as usize].1;
        let old_span = &before[byte_start..byte_end];
        let mut after = String::with_capacity(before.len() + replace.len());
        after.push_str(&before[..byte_start]);
        after.push_str(replace);
        after.push_str(&before[byte_end..]);
        original_contents
            .entry(path.to_string())
            .or_insert_with(|| before.clone());
        write_text_checked(&target, &after)?;
        applied.push(json!({
            "index": index,
            "path": path,
            "start_line": start_line,
            "end_line": end_line,
            "before_sha256": sha256_text(&before),
            "after_sha256": sha256_text(&after),
            "observed_sha256": observed_sha256,
            "old_span_sha256": sha256_text(old_span),
            "replace_sha256": sha256_text(replace),
            "source_of_truth": file_sot(&target)
        }));
    }

    let syntax_probe = python_syntax_probe_after_candidate_edits(
        repo_dir,
        attempt_dir,
        "line-window-edit",
        &applied,
        timeout,
    )?;
    if syntax_probe.get("status").and_then(Value::as_str) == Some("failed") {
        let restore = restore_original_candidate_sources(
            repo_dir,
            attempt_dir,
            "line-window-edit",
            &original_contents,
        )?;
        return Ok(structured_edit_failure(
            "RUST_REALITY_LINE_EDIT_PY_COMPILE_FAILED",
            "model_response.line_edits",
            "line-window edits produced syntactically invalid Python source before candidate patch generation",
            "reopen the edited source window, preserve Python syntax, and resubmit a candidate patch only after the syntax probe passes",
            json!({
                "applied_edits": applied,
                "syntax_probe": syntax_probe,
                "workspace_restore": restore
            }),
            &edits_path,
        ));
    }

    let diff = run_required(
        &["git", "diff", "--no-ext-diff", "--binary"],
        repo_dir,
        timeout,
        "RUST_REALITY_LINE_EDIT_GIT_DIFF_FAILED",
        "line_window_edits.git_diff",
    )?;
    if diff.stdout.trim().is_empty() {
        return Ok(structured_edit_failure(
            "RUST_REALITY_LINE_EDIT_EMPTY_DIFF",
            "model_response.line_edits",
            "line-window edits produced no repository diff",
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
            "field_path": "model_response.line_edits",
            "message": "Rust compiled line-window edits into a git diff from the real checkout",
            "applied_edits": applied,
            "syntax_probe": syntax_probe,
            "candidate_patch_sha256": sha256_file(&patch_path)?,
            "source_of_truth": {
                "line_window_edits": file_sot(&edits_path),
                "candidate_patch": file_sot(&patch_path),
                "workspace_diff": format!("git:{}#diff", path_str(repo_dir))
            }
        }),
    ))
}

fn compile_unified_diff_submission(
    repo_dir: &Path,
    attempt_dir: &Path,
    patch: &str,
    observed_files: &[Value],
    timeout: u64,
) -> Result<(Option<String>, Value)> {
    let submitted_patch_path = attempt_dir.join("unified-diff-submission.patch");
    write_text_checked(&submitted_patch_path, patch)?;
    if patch.trim().is_empty() {
        return Ok(structured_edit_failure(
            "RUST_REALITY_UNIFIED_DIFF_EMPTY",
            "model_response.unified_diff.patch",
            "unified diff submission was empty",
            "submit a non-empty unified diff against production source",
            json!({"patch_sha256": sha256_text(patch)}),
            &submitted_patch_path,
        ));
    }
    if observed_files.is_empty() {
        return Ok(structured_edit_failure(
            "RUST_REALITY_UNIFIED_DIFF_OBSERVED_FILES_EMPTY",
            "model_response.unified_diff.observed_files",
            "unified diff submission did not include observed source hashes",
            "open each changed source file and include observed_sha256 before submitting a diff",
            json!({}),
            &submitted_patch_path,
        ));
    }

    let mut observed = BTreeMap::new();
    for (index, item) in observed_files.iter().enumerate() {
        let field_prefix = format!("model_response.unified_diff.observed_files[{index}]");
        let path = match item.get("path").and_then(Value::as_str) {
            Some(value) if !value.trim().is_empty() => value.trim(),
            _ => {
                return Ok(structured_edit_failure(
                    "RUST_REALITY_UNIFIED_DIFF_OBSERVED_PATH_MISSING",
                    &format!("{field_prefix}.path"),
                    "observed file path is missing",
                    "include repo-relative path values from open_file_window",
                    json!({"observed_file": item}),
                    &submitted_patch_path,
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
                "RUST_REALITY_UNIFIED_DIFF_OBSERVED_PATH_INVALID",
                &format!("{field_prefix}.path"),
                "observed file path must be a relative repository path",
                "use normal repo-relative paths without absolute roots, parent traversal, or backslashes",
                json!({"path": path}),
                &submitted_patch_path,
            ));
        }
        let observed_sha256 = match item.get("observed_sha256").and_then(Value::as_str) {
            Some(value) if value.starts_with("sha256:") => value,
            _ => {
                return Ok(structured_edit_failure(
                    "RUST_REALITY_UNIFIED_DIFF_OBSERVED_SHA_MISSING",
                    &format!("{field_prefix}.observed_sha256"),
                    "observed file hash is missing",
                    "copy observed_sha256 from open_file_window for every changed file",
                    json!({"path": path}),
                    &submitted_patch_path,
                ));
            }
        };
        let target = repo_dir.join(path);
        if !target.exists() || !target.is_file() {
            return Ok(structured_edit_failure(
                "RUST_REALITY_UNIFIED_DIFF_OBSERVED_TARGET_MISSING",
                &format!("{field_prefix}.path"),
                "observed file target does not exist",
                "open a real production-source file before submitting the diff",
                json!({"path": path, "source_of_truth": file_sot(&target)}),
                &submitted_patch_path,
            ));
        }
        let actual_sha256 = sha256_file(&target)?;
        if actual_sha256 != observed_sha256 {
            return Ok(structured_edit_failure(
                "RUST_REALITY_UNIFIED_DIFF_OBSERVED_SHA_MISMATCH",
                &format!("{field_prefix}.observed_sha256"),
                "observed file hash does not match current source readback",
                "reopen the file window and submit a diff against the current source reality",
                json!({
                    "path": path,
                    "expected": observed_sha256,
                    "actual": actual_sha256,
                    "source_of_truth": file_sot(&target)
                }),
                &submitted_patch_path,
            ));
        }
        observed.insert(path.to_string(), observed_sha256.to_string());
    }

    let apply_check = run_cmd(
        &vec![
            "git".to_string(),
            "apply".to_string(),
            "--check".to_string(),
            "--whitespace=nowarn".to_string(),
            path_str(&submitted_patch_path).to_string(),
        ],
        repo_dir,
        timeout,
    )?;
    if apply_check.exit_code != 0 {
        return Ok(structured_edit_failure(
            "RUST_REALITY_UNIFIED_DIFF_APPLY_CHECK_FAILED",
            "model_response.unified_diff.patch",
            "unified diff did not apply cleanly to the base checkout",
            "regenerate the diff against the exact task base commit and observed source windows",
            json!({"process": apply_check, "patch": file_sot(&submitted_patch_path)}),
            &submitted_patch_path,
        ));
    }
    let apply = run_cmd(
        &vec![
            "git".to_string(),
            "apply".to_string(),
            "--whitespace=nowarn".to_string(),
            path_str(&submitted_patch_path).to_string(),
        ],
        repo_dir,
        timeout,
    )?;
    if apply.exit_code != 0 {
        return Ok(structured_edit_failure(
            "RUST_REALITY_UNIFIED_DIFF_APPLY_FAILED",
            "model_response.unified_diff.patch",
            "unified diff apply failed after apply --check passed",
            "inspect git apply output and regenerate the candidate diff",
            json!({"process": apply, "patch": file_sot(&submitted_patch_path)}),
            &submitted_patch_path,
        ));
    }
    let changed = run_required(
        &["git", "diff", "--name-only"],
        repo_dir,
        timeout,
        "RUST_REALITY_UNIFIED_DIFF_NAME_ONLY_FAILED",
        "unified_diff.git_diff_name_only",
    )?;
    let changed_files: Vec<String> = changed.stdout.lines().map(ToOwned::to_owned).collect();
    if changed_files.is_empty() {
        return Ok(structured_edit_failure(
            "RUST_REALITY_UNIFIED_DIFF_EMPTY_DIFF",
            "model_response.unified_diff.patch",
            "unified diff applied but produced no repository diff",
            "change production source so the official oracle has a candidate patch to evaluate",
            json!({"patch": file_sot(&submitted_patch_path)}),
            &submitted_patch_path,
        ));
    }
    let forbidden_paths: Vec<String> = changed_files
        .iter()
        .filter(|path| is_forbidden_benchmark_mutation(path))
        .cloned()
        .collect();
    if !forbidden_paths.is_empty() {
        return Ok(structured_edit_failure(
            "RUST_REALITY_UNIFIED_DIFF_FORBIDDEN_PATH",
            "model_response.unified_diff.patch",
            "unified diff targets tests, fixtures, or benchmark harness files",
            "repair production source only; do not change tests, fixtures, or evaluation harnesses",
            json!({"forbidden_paths": forbidden_paths, "changed_files": changed_files}),
            &submitted_patch_path,
        ));
    }
    let unobserved_changed_files: Vec<String> = changed_files
        .iter()
        .filter(|path| !observed.contains_key(*path))
        .cloned()
        .collect();
    if !unobserved_changed_files.is_empty() {
        return Ok(structured_edit_failure(
            "RUST_REALITY_UNIFIED_DIFF_CHANGED_FILE_NOT_OBSERVED",
            "model_response.unified_diff.observed_files",
            "unified diff changed files that were not backed by observed source hashes",
            "open every changed source file and include its observed_sha256 before submitting the diff",
            json!({
                "unobserved_changed_files": unobserved_changed_files,
                "changed_files": changed_files,
                "observed_files": observed.keys().cloned().collect::<Vec<_>>()
            }),
            &submitted_patch_path,
        ));
    }

    let applied = changed_files
        .iter()
        .map(|path| {
            let target = repo_dir.join(path);
            Ok(json!({
                "path": path,
                "observed_sha256": observed.get(path),
                "after_sha256": sha256_file(&target)?,
                "source_of_truth": file_sot(&target)
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    let static_checks = run_static_patch_checks(repo_dir, attempt_dir, &changed_files, timeout)?;
    if static_checks.get("status").and_then(Value::as_str) != Some("passed") {
        return Ok(structured_edit_failure(
            static_checks
                .get("error_code")
                .and_then(Value::as_str)
                .unwrap_or("RUST_REALITY_UNIFIED_DIFF_STATIC_CHECK_FAILED"),
            static_checks
                .get("field_path")
                .and_then(Value::as_str)
                .unwrap_or("candidate.patch.static_checks"),
            static_checks
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unified diff failed static source verification"),
            static_checks
                .get("remediation")
                .and_then(Value::as_str)
                .unwrap_or("fix syntax/import errors and resubmit the diff"),
            json!({"changed_files": changed_files, "static_checks": static_checks}),
            &submitted_patch_path,
        ));
    }
    let diff = run_required(
        &["git", "diff", "--no-ext-diff", "--binary"],
        repo_dir,
        timeout,
        "RUST_REALITY_UNIFIED_DIFF_GIT_DIFF_FAILED",
        "unified_diff.git_diff",
    )?;
    let patch_path = attempt_dir.join("candidate.patch");
    write_text_checked(&patch_path, &diff.stdout)?;
    Ok((
        Some(diff.stdout),
        json!({
            "status": "passed",
            "error_code": null,
            "field_path": "model_response.unified_diff.patch",
            "message": "Rust applied unified diff, verified observed file hashes, and normalized it into a git diff from the real checkout",
            "applied_edits": applied,
            "changed_files": changed_files,
            "static_checks": static_checks,
            "candidate_patch_sha256": sha256_file(&patch_path)?,
            "source_of_truth": {
                "unified_diff_submission": file_sot(&submitted_patch_path),
                "candidate_patch": file_sot(&patch_path),
                "workspace_diff": format!("git:{}#diff", path_str(repo_dir))
            }
        }),
    ))
}

fn line_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = 0;
    for segment in text.split_inclusive('\n') {
        let end = start + segment.len();
        spans.push((start, end));
        start = end;
    }
    if start < text.len() {
        spans.push((start, text.len()));
    }
    spans
}

fn python_syntax_probe_after_candidate_edits(
    repo_dir: &Path,
    attempt_dir: &Path,
    label: &str,
    applied: &[Value],
    timeout: u64,
) -> Result<Value> {
    let mut python_files = BTreeSet::new();
    for edit in applied {
        if let Some(path) = edit.get("path").and_then(Value::as_str) {
            if path.ends_with(".py") {
                python_files.insert(path.to_string());
            }
        }
    }
    let python_files = python_files.into_iter().collect::<Vec<_>>();
    let probe_path = attempt_dir.join(format!("{label}-python-py-compile.json"));
    if python_files.is_empty() {
        let payload = json!({
            "record_kind": "contextgraph_candidate_edit_syntax_probe",
            "status": "passed",
            "check": "python_py_compile",
            "skipped": true,
            "reason": "candidate edit changed no Python source files",
            "python_files": python_files,
            "source_of_truth": file_sot(&probe_path)
        });
        write_json_checked(&probe_path, &payload)?;
        let readback: Value = read_json(&probe_path)?;
        if readback != payload {
            return Err(RealityError::new(
                "RUST_REALITY_EDIT_SYNTAX_PROBE_READBACK_MISMATCH",
                "candidate edit syntax probe readback did not match written payload",
                "candidate_edit.syntax_probe.readback",
                "inspect filesystem durability before accepting edit output",
                json!({"expected": payload, "actual": readback}),
                Some(file_sot(&probe_path)),
            ));
        }
        return Ok(readback);
    }

    let mut command = vec![
        LOCAL_PYTHON_BIN.to_string(),
        "-B".to_string(),
        "-m".to_string(),
        "py_compile".to_string(),
    ];
    command.extend(python_files.iter().cloned());
    let process = run_cmd(&command, repo_dir, timeout)?;
    let status = if process.exit_code == 0 {
        "passed"
    } else {
        "failed"
    };
    let payload = json!({
        "record_kind": "contextgraph_candidate_edit_syntax_probe",
        "status": status,
        "check": "python_py_compile",
        "skipped": false,
        "error_code": if process.exit_code == 0 { Value::Null } else { json!("RUST_REALITY_EDIT_PY_COMPILE_FAILED") },
        "field_path": if process.exit_code == 0 { Value::Null } else { json!("candidate_edit.changed_python_files") },
        "message": if process.exit_code == 0 {
            "changed Python source files passed py_compile before candidate patch generation"
        } else {
            "changed Python source files failed py_compile before candidate patch generation"
        },
        "remediation": if process.exit_code == 0 {
            "continue candidate patch generation"
        } else {
            "repair syntax before writing candidate.patch or running SWE-bench evaluation"
        },
        "python_files": python_files,
        "process": {
            "command": process.command,
            "cwd": process.cwd,
            "exit_code": process.exit_code,
            "elapsed_ms": process.elapsed_ms,
            "stdout_tail": tail_chars(&process.stdout, 4000),
            "stderr_tail": tail_chars(&process.stderr, 8000)
        },
        "source_of_truth": file_sot(&probe_path)
    });
    write_json_checked(&probe_path, &payload)?;
    let readback: Value = read_json(&probe_path)?;
    if readback != payload {
        return Err(RealityError::new(
            "RUST_REALITY_EDIT_SYNTAX_PROBE_READBACK_MISMATCH",
            "candidate edit syntax probe readback did not match written payload",
            "candidate_edit.syntax_probe.readback",
            "inspect filesystem durability before accepting edit output",
            json!({"expected": payload, "actual": readback}),
            Some(file_sot(&probe_path)),
        ));
    }
    Ok(readback)
}

fn restore_original_candidate_sources(
    repo_dir: &Path,
    attempt_dir: &Path,
    label: &str,
    original_contents: &BTreeMap<String, String>,
) -> Result<Value> {
    let restore_path = attempt_dir.join(format!("{label}-workspace-restore.json"));
    let mut readbacks = Vec::new();
    for (path, content) in original_contents {
        let full_path = repo_dir.join(path);
        write_text_checked(&full_path, content)?;
        let readback = fs::read_to_string(&full_path).map_err(|err| {
            fs_error("RUST_REALITY_EDIT_RESTORE_READBACK_FAILED", &full_path, err)
        })?;
        if readback != *content {
            return Err(RealityError::new(
                "RUST_REALITY_EDIT_RESTORE_READBACK_MISMATCH",
                "candidate edit workspace restore readback did not match original source",
                "candidate_edit.restore.readback",
                "inspect filesystem durability before continuing",
                json!({
                    "path": path,
                    "expected_sha256": sha256_text(content),
                    "actual_sha256": sha256_text(&readback)
                }),
                Some(file_sot(&full_path)),
            ));
        }
        readbacks.push(json!({
            "path": path,
            "sha256": sha256_text(&readback),
            "size_bytes": readback.len(),
            "source_of_truth": file_sot(&full_path)
        }));
    }
    let payload = json!({
        "record_kind": "contextgraph_candidate_edit_workspace_restore",
        "status": "ok",
        "restored_files": readbacks,
        "source_of_truth": file_sot(&restore_path)
    });
    write_json_checked(&restore_path, &payload)?;
    let readback: Value = read_json(&restore_path)?;
    if readback != payload {
        return Err(RealityError::new(
            "RUST_REALITY_EDIT_RESTORE_ARTIFACT_READBACK_MISMATCH",
            "candidate edit restore artifact readback did not match written payload",
            "candidate_edit.restore_artifact.readback",
            "inspect filesystem durability before continuing",
            json!({"expected": payload, "actual": readback}),
            Some(file_sot(&restore_path)),
        ));
    }
    Ok(readback)
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
    let static_checks = run_static_patch_checks(repo_dir, attempt_dir, &changed_files, timeout)?;
    if static_checks.get("status").and_then(Value::as_str) != Some("passed") {
        return Ok(json!({
            "status": "failed",
            "error_code": static_checks.get("error_code").and_then(Value::as_str).unwrap_or("RUST_REALITY_PATCH_STATIC_CHECK_FAILED"),
            "field_path": static_checks.get("field_path").and_then(Value::as_str).unwrap_or("candidate.patch.static_checks"),
            "message": static_checks.get("message").and_then(Value::as_str).unwrap_or("candidate patch failed static source verification"),
            "remediation": static_checks.get("remediation").and_then(Value::as_str).unwrap_or("read the static check output, fix syntax/import errors, and produce a new production-source patch"),
            "details": {
                "changed_files": changed_files,
                "static_checks": static_checks,
                "actual_patch_sha256": sha256_file(&actual_patch)?
            },
            "source_of_truth": file_sot(&attempt_dir.join("static-checks.json"))
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
            "fail_to_pass": task.fail_to_pass,
            "changed_files": changed_files,
            "static_checks": static_checks
        },
        "source_of_truth": file_sot(&actual_patch)
    }))
}

fn run_static_patch_checks(
    repo_dir: &Path,
    attempt_dir: &Path,
    changed_files: &[String],
    timeout: u64,
) -> Result<Value> {
    let python_files: Vec<String> = changed_files
        .iter()
        .filter(|path| path.ends_with(".py"))
        .cloned()
        .collect();
    let static_path = attempt_dir.join("static-checks.json");
    if python_files.is_empty() {
        let payload = json!({
            "record_kind": "contextgraph_patch_static_checks",
            "status": "passed",
            "check": "python_py_compile",
            "skipped": true,
            "reason": "candidate patch changed no Python source files",
            "python_files": python_files,
            "source_of_truth": file_sot(&static_path)
        });
        write_json_checked(&static_path, &payload)?;
        return Ok(payload);
    }
    let mut command = vec![
        LOCAL_PYTHON_BIN.to_string(),
        "-m".to_string(),
        "py_compile".to_string(),
    ];
    command.extend(python_files.iter().cloned());
    let process = run_cmd(&command, repo_dir, timeout)?;
    let status = if process.exit_code == 0 {
        "passed"
    } else {
        "failed"
    };
    let payload = json!({
        "record_kind": "contextgraph_patch_static_checks",
        "status": status,
        "check": "python_py_compile",
        "skipped": false,
        "error_code": if process.exit_code == 0 { Value::Null } else { json!("RUST_REALITY_PATCH_PY_COMPILE_FAILED") },
        "field_path": if process.exit_code == 0 { Value::Null } else { json!("candidate.patch.changed_python_files") },
        "message": if process.exit_code == 0 {
            "changed Python source files passed py_compile"
        } else {
            "changed Python source files failed py_compile"
        },
        "remediation": if process.exit_code == 0 {
            "continue to official SWE-bench evaluation"
        } else {
            "fix the syntax/compile error before running expensive official SWE-bench evaluation"
        },
        "python_files": python_files,
        "process": {
            "command": process.command,
            "cwd": process.cwd,
            "exit_code": process.exit_code,
            "elapsed_ms": process.elapsed_ms,
            "stdout_tail": tail_chars(&process.stdout, 4000),
            "stderr_tail": tail_chars(&process.stderr, 8000)
        },
        "source_of_truth": file_sot(&static_path)
    });
    write_json_checked(&static_path, &payload)?;
    Ok(payload)
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
    args: &AttemptArgs,
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
    let instance_report = find_instance_report(&official_root, &task.instance_id)?;
    let test_output =
        find_instance_text_artifact(&official_root, &task.instance_id, "test_output.txt")?;
    Ok(json!({
        "status": "ok",
        "run_id": run_id,
        "report_dir": path_str(&official_root),
        "final_report": final_report,
        "instance_report": instance_report,
        "test_output": test_output,
        "source_of_truth": {
            "command": file_sot(&command_path),
            "predictions": file_sot(&predictions),
            "final_report": final_report["source_of_truth"].clone(),
            "instance_report": instance_report["source_of_truth"].clone(),
            "test_output": test_output["source_of_truth"].clone()
        }
    }))
}

fn verify_official_docker_sdk(args: &AttemptArgs, attempt_dir: &Path) -> Result<()> {
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

fn official_docker_env(args: &AttemptArgs, attempt_dir: &Path) -> Result<Vec<(String, String)>> {
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
        "instance_report": official["instance_report"].clone(),
        "test_output_tail": official["test_output"].clone(),
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

fn find_instance_report(root: &Path, instance_id: &str) -> Result<Value> {
    let mut candidates = Vec::new();
    collect_json_files(root, &mut candidates)?;
    candidates.sort();
    for path in candidates {
        let path_text = path_str(&path);
        let name_matches = path.file_name().and_then(|name| name.to_str()) == Some("report.json");
        if !name_matches || !path_text.contains(instance_id) {
            continue;
        }
        let text = fs::read_to_string(&path)
            .map_err(|err| fs_error("RUST_REALITY_INSTANCE_REPORT_READ_FAILED", &path, err))?;
        let payload: Value = serde_json::from_str(&text).map_err(|err| {
            RealityError::new(
                "RUST_REALITY_INSTANCE_REPORT_JSON_INVALID",
                "official per-instance report was not valid JSON",
                "official_eval.instance_report",
                "inspect the official SWE-bench per-instance report before using it as prompt evidence",
                json!({"path": path_str(&path), "error": err.to_string()}),
                Some(file_sot(&path)),
            )
        })?;
        return Ok(json!({
            "path": path_str(&path),
            "sha256": sha256_file(&path)?,
            "source_of_truth": file_sot(&path),
            "payload": payload
        }));
    }
    Err(RealityError::new(
        "RUST_REALITY_INSTANCE_REPORT_MISSING",
        "official SWE-bench run did not produce a per-instance report.json for the target instance",
        "official_eval.instance_report",
        "inspect official report directory and SWE-bench logs before continuing",
        json!({"report_dir": path_str(root), "instance_id": instance_id}),
        Some(file_sot(root)),
    ))
}

fn find_instance_text_artifact(root: &Path, instance_id: &str, file_name: &str) -> Result<Value> {
    let mut candidates = Vec::new();
    collect_named_files(root, file_name, &mut candidates)?;
    candidates.sort();
    for path in candidates {
        if !path_str(&path).contains(instance_id) {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|err| {
            fs_error(
                "RUST_REALITY_INSTANCE_TEXT_ARTIFACT_READ_FAILED",
                &path,
                err,
            )
        })?;
        return Ok(json!({
            "path": path_str(&path),
            "sha256": sha256_file(&path)?,
            "size_bytes": text.len(),
            "tail": tail_chars(&text, 12000),
            "source_of_truth": file_sot(&path)
        }));
    }
    Err(RealityError::new(
        "RUST_REALITY_INSTANCE_TEXT_ARTIFACT_MISSING",
        "official SWE-bench run did not produce the required per-instance text artifact",
        format!("official_eval.{file_name}"),
        "inspect official report directory and SWE-bench logs before continuing",
        json!({"report_dir": path_str(root), "instance_id": instance_id, "file_name": file_name}),
        Some(file_sot(root)),
    ))
}

fn collect_named_files(root: &Path, file_name: &str, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in
        fs::read_dir(root).map_err(|err| fs_error("RUST_REALITY_READ_DIR_FAILED", root, err))?
    {
        let entry =
            entry.map_err(|err| fs_error("RUST_REALITY_READ_DIR_ENTRY_FAILED", root, err))?;
        let path = entry.path();
        if path.is_dir() {
            collect_named_files(&path, file_name, out)?;
        } else if path.file_name().and_then(|name| name.to_str()) == Some(file_name) {
            out.push(path);
        }
    }
    Ok(())
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
    // Preserves issue-text mention order. Emits two classes of production candidates:
    //   1. Direct: a non-test .py / .pyx mention is itself a production candidate.
    //   2. Derived: from `prefix/tests/test_X.py`, derive `prefix/X.py` and `prefix/_X.py`.
    // Order of emission follows mention order from path_candidates so the highest-precision
    // signal (the issue text named this file directly) lands at index 0.
    let mut out: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for path in path_candidates(task) {
        let is_python = path.ends_with(".py") || path.ends_with(".pyx");
        let looks_like_test = path.contains("/tests/")
            || path.contains("/test_")
            || path.starts_with("test_")
            || path.starts_with("tests/");
        if is_python && !looks_like_test && seen.insert(path.clone()) {
            out.push(path.clone());
        }
        if let Some((prefix, test_name)) = path.split_once("/tests/test_") {
            let stem = test_name.strip_suffix(".py").unwrap_or(test_name);
            let prod = format!("{prefix}/{stem}.py");
            if seen.insert(prod.clone()) {
                out.push(prod);
            }
            let prod_under = format!("{prefix}/_{stem}.py");
            if seen.insert(prod_under.clone()) {
                out.push(prod_under);
            }
        }
    }
    out
}

fn path_candidates(task: &TaskManifest) -> Vec<String> {
    // Order-preserving deduplication: emit paths in the order they appear in the issue text
    // (first), then in fail_to_pass test ids (second). Earlier mentions win on collision.
    let mut out: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
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
                if !path.is_empty() && seen.insert(path.to_string()) {
                    out.push(path.to_string());
                }
            }
        }
    }
    out
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

fn build_reality_signal_packet(
    attempt: u64,
    task: &TaskManifest,
    problem_reality: &Value,
    validation: &Value,
    official_evidence_path: Option<&str>,
    official_reality: Option<&Value>,
    patch: Option<&str>,
    patch_sha: Option<&str>,
    repeated_patch: bool,
    model_response: &Value,
    agent_tool_transcript_path: Option<&str>,
    score: &Value,
    reward: Option<&Value>,
    ledger_counts: &BTreeMap<String, i64>,
    attempt_dir: &Path,
) -> Result<Value> {
    let official_evidence = match official_evidence_path {
        Some(path) => Some(read_json::<Value>(&PathBuf::from(path))?),
        None => None,
    };
    let changed_files = changed_files_from_validation(validation);
    let tool_grounding = extract_tool_grounding(agent_tool_transcript_path, &changed_files)?;
    let harness_reality = extract_harness_transition_reality(agent_tool_transcript_path)?;
    let tool_reward_signals = derive_tool_reward_signals(
        &tool_grounding,
        patch.is_some(),
        repeated_patch,
        validation,
        official_evidence_path.is_some(),
    );
    let failure_digest = build_failure_digest(validation, official_evidence.as_ref());
    let patch_stats = patch.map(patch_line_stats).unwrap_or_else(|| {
        json!({
            "line_count": 0,
            "added_lines": 0,
            "removed_lines": 0,
            "file_header_count": 0
        })
    });
    let reward_signal = reward.map(compact_reward_signal).unwrap_or_else(|| {
        json!({
            "status": "missing",
            "reason": "reward compiler output was not available for this attempt"
        })
    });
    Ok(json!({
        "record_kind": "contextgraph_reality_signal_packet",
        "schema_version": 1,
        "attempt": attempt,
        "task": {
            "instance_id": task.instance_id,
            "repo": task.repo,
            "base_commit": task.base_commit,
            "fail_to_pass_total": task.fail_to_pass.len(),
            "pass_to_pass_count": task.pass_to_pass_count,
            "source_of_truth": task.source_of_truth
        },
        "problem_reality": compact_problem_reality_packet(problem_reality),
        "outcome_reality": {
            "validation_status": validation.get("status"),
            "validation_error_code": validation.get("error_code"),
            "validation_message": validation.get("message"),
            "official_evidence_path": official_evidence_path,
            "official_reality": official_reality,
            "resolved": official_evidence.as_ref().and_then(|value| value.get("resolved")).cloned(),
            "success_predicate": "official_report.resolved_ids contains task.instance_id",
            "source_of_truth": {
                "validation": validation.get("source_of_truth"),
                "official_evidence": official_evidence_path
            }
        },
        "failure_digest": failure_digest,
        "official_test_progress": official_test_progress(task, official_evidence.as_ref()),
        "patch_signals": {
            "patch_sha256": patch_sha,
            "repeated_patch": repeated_patch,
            "changed_files": changed_files,
            "patch_stats": patch_stats,
            "patch_preview": patch.map(|text| truncate_chars(text, 3000)),
            "source_of_truth": {
                "candidate_patch": optional_file_sot(&attempt_dir.join("candidate.patch")),
                "actual_applied_patch": optional_file_sot(&attempt_dir.join("actual-applied.patch")),
                "static_checks": optional_file_sot(&attempt_dir.join("static-checks.json"))
            }
        },
        "tool_grounding": tool_grounding,
        "harness_reality": harness_reality,
        "tool_reward_signals": tool_reward_signals,
        "score_reality": {
            "score": score,
            "reward": reward_signal,
            "ledger_counts": ledger_counts,
            "source_of_truth": "BlackBoxFix SQLite ledger and reward compiler output"
        },
        "model_io_reality": {
            "response_sha256": sha256_text(&serde_json::to_string(model_response).unwrap()),
            "content_sha256": model_response.pointer("/message/content").and_then(Value::as_str).map(sha256_text),
            "agent_tool_transcript": agent_tool_transcript_path
        },
        "reinjection_directive": {
            "next_prompt_must_include": [
                "problem_reality.task_identity",
                "problem_reality.oracle_reality",
                "failure_digest.normalized_failure_class",
                "failure_digest.first_error_location",
                "official_test_progress",
                "patch_signals.changed_files",
                "tool_grounding.changed_files_seen_by_tools",
                "harness_reality.transition_reward_signals",
                "tool_reward_signals",
                "score_reality.reward"
            ],
            "do_not_repeat_patch_sha256": patch_sha,
            "required_next_tool": required_next_tool_for_validation(validation, official_evidence_path.is_some()),
            "optimization_target": "increase official FAIL_TO_PASS success without PASS_TO_PASS regression, while keeping source/test boundary intact"
        },
        "source_of_truth": {
            "attempt_dir": file_sot(attempt_dir),
            "validation": validation.get("source_of_truth"),
            "official_evidence": official_evidence_path,
            "agent_tool_transcript": agent_tool_transcript_path
        }
    }))
}

fn compact_reality_signal_packet(packet: &Value) -> Value {
    json!({
        "record_kind": packet.get("record_kind"),
        "attempt": packet.get("attempt"),
        "task": packet.get("task"),
        "outcome_reality": packet.get("outcome_reality"),
        "failure_digest": packet.get("failure_digest"),
        "official_test_progress": packet.get("official_test_progress"),
        "patch_signals": {
            "patch_sha256": packet.pointer("/patch_signals/patch_sha256"),
            "repeated_patch": packet.pointer("/patch_signals/repeated_patch"),
            "changed_files": packet.pointer("/patch_signals/changed_files"),
            "patch_stats": packet.pointer("/patch_signals/patch_stats")
        },
        "tool_grounding": packet.get("tool_grounding"),
        "harness_reality": packet.get("harness_reality"),
        "tool_reward_signals": packet.get("tool_reward_signals"),
        "score_reality": {
            "reward": packet.pointer("/score_reality/reward/reward"),
            "score": packet.pointer("/score_reality/score/score"),
            "ledger_counts": packet.pointer("/score_reality/ledger_counts")
        },
        "reinjection_directive": packet.get("reinjection_directive"),
        "source_of_truth": packet.get("source_of_truth")
    })
}

fn required_next_tool_for_validation(
    validation: &Value,
    has_official_evidence: bool,
) -> &'static str {
    match validation.get("error_code").and_then(Value::as_str) {
        Some("RUST_REALITY_FINAL_EDIT_TOOL_FAILED") => {
            "open_file_window for the exact source span, then apply_line_window_edit with observed_sha256, apply_structured_edit with a unique find block, or apply_unified_diff with observed file hashes"
        }
        Some("RUST_REALITY_FINAL_EDIT_TOOL_REQUIRED") => {
            "apply_line_window_edit with observed_sha256 from open_file_window, apply_structured_edit with exact find/replace edits copied from opened source windows, or apply_unified_diff with observed file hashes"
        }
        _ if has_official_evidence => "read_official_failure_reality or compare_attempts",
        _ => "read_problem_reality plus search/open_file_window",
    }
}

fn derive_tool_reward_signals(
    tool_grounding: &Value,
    submitted_patch: bool,
    repeated_patch: bool,
    validation: &Value,
    official_eval_ran: bool,
) -> Value {
    let tools = tool_grounding
        .get("tools_used")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let used = |name: &str| tools.iter().any(|item| item.as_str() == Some(name));
    let changed_files_read = tool_grounding
        .get("changed_files_read_before_edit")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let no_tool_rounds = tool_grounding
        .get("no_tool_rounds")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let edit_deadline_no_tool_rounds = tool_grounding
        .get("edit_deadline_no_tool_rounds")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let duplicate_navigation_calls = tool_grounding
        .get("duplicate_navigation_calls")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let validation_passed = validation.get("status").and_then(Value::as_str) == Some("passed");
    let mut score = 0.0_f64;
    if used("read_problem_reality") {
        score += 1.0;
    }
    if used("search_repo") || used("inspect_symbol") || used("list_files") {
        score += 1.0;
    }
    if used("open_file_window") {
        score += 1.0;
    }
    if used("apply_line_window_edit") {
        score += 1.5;
    } else if used("apply_structured_edit") {
        score += 1.0;
    } else if used("apply_unified_diff") {
        score += 1.0;
    }
    if submitted_patch {
        score += 2.0;
    }
    if changed_files_read {
        score += 1.0;
    }
    if official_eval_ran {
        score += 1.0;
    }
    if validation_passed {
        score += 1.0;
    }
    if repeated_patch {
        score -= 2.0;
    }
    score -= no_tool_rounds as f64 * 0.25;
    score -= edit_deadline_no_tool_rounds as f64 * 0.75;
    score -= duplicate_navigation_calls as f64 * 0.5;
    json!({
        "record_kind": "contextgraph_tool_use_reward_signals",
        "objective": "reward coding-agent behaviors that increase verified SWE repair odds before final official success exists",
        "features": {
            "used_problem_reality": used("read_problem_reality"),
            "used_navigation_or_search": used("search_repo") || used("inspect_symbol") || used("list_files"),
            "opened_source_window": used("open_file_window"),
            "used_preferred_line_window_edit": used("apply_line_window_edit"),
            "used_structured_find_replace_edit": used("apply_structured_edit"),
            "used_unified_diff_edit": used("apply_unified_diff"),
            "submitted_patch_artifact": submitted_patch,
            "changed_files_read_before_edit": changed_files_read,
            "used_targeted_test": used("run_targeted_test"),
            "official_eval_ran": official_eval_ran,
            "validation_passed": validation_passed,
            "repeated_patch": repeated_patch,
            "no_tool_rounds": no_tool_rounds,
            "edit_deadline_no_tool_rounds": edit_deadline_no_tool_rounds,
            "duplicate_navigation_calls": duplicate_navigation_calls
        },
        "score_unbounded": score,
        "recommended_reward_components": {
            "problem_reality_read": if used("read_problem_reality") { 1.0 } else { 0.0 },
            "repo_navigation": if used("search_repo") || used("inspect_symbol") || used("list_files") { 1.0 } else { 0.0 },
            "source_window_grounding": if used("open_file_window") { 1.0 } else { 0.0 },
            "preferred_edit_interface": if used("apply_line_window_edit") { 1.5 } else { 0.0 },
            "unified_diff_interface": if used("apply_unified_diff") { 1.0 } else { 0.0 },
            "patch_artifact_submitted": if submitted_patch { 2.0 } else { 0.0 },
            "changed_files_read_before_edit": if changed_files_read { 1.0 } else { 0.0 },
            "official_eval_readback": if official_eval_ran { 1.0 } else { 0.0 },
            "repeated_patch_penalty": if repeated_patch { -2.0 } else { 0.0 },
            "no_tool_round_penalty": -(no_tool_rounds as f64 * 0.25),
            "edit_deadline_no_tool_penalty": -(edit_deadline_no_tool_rounds as f64 * 0.75),
            "duplicate_navigation_penalty": -(duplicate_navigation_calls as f64 * 0.5)
        },
        "source_of_truth": tool_grounding.get("source_of_truth")
    })
}

fn build_failure_digest(validation: &Value, official_evidence: Option<&Value>) -> Value {
    let official_tail = official_evidence
        .and_then(|value| value.pointer("/test_output_tail/tail"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let validation_error = validation
        .get("error_code")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let (failure_class, exception_type) = classify_failure(validation_error, official_tail);
    json!({
        "normalized_failure_class": failure_class,
        "exception_type": exception_type,
        "first_error_location": first_error_location(official_tail),
        "validation": {
            "status": validation.get("status"),
            "error_code": validation.get("error_code"),
            "field_path": validation.get("field_path"),
            "message": validation.get("message"),
            "remediation": validation.get("remediation"),
            "source_of_truth": validation.get("source_of_truth")
        },
        "official_test_output_tail": truncate_chars(official_tail, 6000)
    })
}

fn classify_failure(validation_error: &str, official_tail: &str) -> (String, Option<String>) {
    let exception_type = [
        "IndentationError",
        "SyntaxError",
        "ImportError",
        "ModuleNotFoundError",
        "AssertionError",
        "AttributeError",
        "TypeError",
        "ValueError",
        "NameError",
        "RuntimeError",
    ]
    .iter()
    .find(|needle| official_tail.contains(**needle))
    .map(|value| (*value).to_string());
    let class = match validation_error {
        "RUST_REALITY_MODEL_RESPONSE_JSON_INVALID" => "model_contract_json_invalid",
        "RUST_REALITY_AGENT_TOOL_REQUIRED_BUT_MISSING" => "model_tool_contract_missing",
        "RUST_REALITY_FINAL_EDIT_TOOL_REQUIRED" => "model_final_edit_tool_missing",
        "RUST_REALITY_FINAL_EDIT_TOOL_FAILED" => "structured_edit_tool_failed",
        "RUST_REALITY_MODEL_ACTION_MISSING" => "model_action_missing",
        "RUST_REALITY_PATCH_REPEATED" => "repeated_patch",
        "RUST_REALITY_PATCH_APPLY_FAILED" => "patch_apply_failed",
        "RUST_REALITY_PATCH_MODIFIES_TESTS" => "forbidden_test_or_harness_mutation",
        "RUST_REALITY_PATCH_PY_COMPILE_FAILED" => "static_compile_failed",
        _ if exception_type.as_deref() == Some("IndentationError")
            || exception_type.as_deref() == Some("SyntaxError") =>
        {
            "official_syntax_or_parse_failure"
        }
        _ if exception_type.as_deref() == Some("AssertionError") => "official_assertion_failure",
        _ if exception_type.is_some() => "official_runtime_failure",
        _ if !official_tail.trim().is_empty() => "official_unresolved_without_classified_exception",
        _ => "candidate_validation_failure",
    };
    (class.to_string(), exception_type)
}

fn first_error_location(text: &str) -> Value {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("File \"") || trimmed.starts_with("E     File \"") {
            return json!({"line": trimmed});
        }
    }
    Value::Null
}

fn official_test_progress(task: &TaskManifest, official_evidence: Option<&Value>) -> Value {
    let Some(evidence) = official_evidence else {
        return json!({
            "status": "not_run",
            "fail_to_pass_total": task.fail_to_pass.len(),
            "pass_to_pass_expected_count": task.pass_to_pass_count
        });
    };
    let compact = compact_official_evidence(evidence);
    let fail_to_pass_success = compact
        .pointer("/fail_to_pass/success_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let fail_to_pass_failure = compact
        .pointer("/fail_to_pass/failure_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let fail_to_pass_total = task.fail_to_pass.len() as u64;
    let passed_fail_to_pass_ratio = if fail_to_pass_total == 0 {
        Value::Null
    } else {
        json!(round_signal(
            fail_to_pass_success as f64 / fail_to_pass_total as f64
        ))
    };
    json!({
        "status": "read",
        "resolved": compact.get("resolved"),
        "completed": compact.get("completed"),
        "errored": compact.get("errored"),
        "empty_patch": compact.get("empty_patch"),
        "patch_successfully_applied": compact.get("patch_successfully_applied"),
        "fail_to_pass_total": fail_to_pass_total,
        "passed_fail_to_pass": fail_to_pass_success,
        "failed_fail_to_pass": fail_to_pass_failure,
        "passed_fail_to_pass_ratio": passed_fail_to_pass_ratio,
        "graduated_correctness_signal": {
            "component": "passed_fail_to_pass / total_fail_to_pass",
            "value": passed_fail_to_pass_ratio,
            "source": "official SWE-bench instance_report.tests_status.FAIL_TO_PASS readback"
        },
        "fail_to_pass": compact.get("fail_to_pass"),
        "pass_to_pass_expected_count": task.pass_to_pass_count,
        "pass_to_pass": compact.get("pass_to_pass"),
        "resolved_ids": compact.get("resolved_ids"),
        "unresolved_ids": compact.get("unresolved_ids"),
        "completed_ids": compact.get("completed_ids")
    })
}

fn round_signal(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn patch_line_stats(patch: &str) -> Value {
    let mut added = 0_u64;
    let mut removed = 0_u64;
    let mut headers = 0_u64;
    for line in patch.lines() {
        if line.starts_with("+++ ") || line.starts_with("--- ") {
            headers += 1;
        } else if line.starts_with('+') {
            added += 1;
        } else if line.starts_with('-') {
            removed += 1;
        }
    }
    json!({
        "line_count": patch.lines().count(),
        "added_lines": added,
        "removed_lines": removed,
        "file_header_count": headers
    })
}

fn changed_files_from_validation(validation: &Value) -> Vec<String> {
    validation
        .pointer("/details/changed_files")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn extract_tool_grounding(
    transcript_path: Option<&str>,
    changed_files: &[String],
) -> Result<Value> {
    let Some(path) = transcript_path else {
        return Ok(json!({
            "status": "missing",
            "executed_tool_calls": 0,
            "tools_used": [],
            "opened_or_inspected_files": [],
            "changed_files_seen_by_tools": [],
            "changed_files_without_tool_read": changed_files,
            "changed_files_read_before_edit": false,
            "source_of_truth": null
        }));
    };
    let transcript_path = PathBuf::from(path.strip_prefix("file:").unwrap_or(path));
    let transcript: Value = read_json(&transcript_path)?;
    let mut tools_used = Vec::new();
    let mut seen_files = BTreeSet::new();
    let mut no_tool_rounds = 0_u64;
    let mut edit_deadline_no_tool_rounds = 0_u64;
    let mut duplicate_navigation_calls = 0_u64;
    if let Some(entries) = transcript.get("entries").and_then(Value::as_array) {
        for entry in entries {
            if entry.get("status").and_then(Value::as_str) == Some("no_tool_call") {
                no_tool_rounds += 1;
                if entry
                    .get("edit_deadline_active")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    edit_deadline_no_tool_rounds += 1;
                }
            }
            if let Some(results) = entry.get("tool_results").and_then(Value::as_array) {
                for result in results {
                    if let Some(tool) = result.get("tool_name").and_then(Value::as_str) {
                        tools_used.push(tool.to_string());
                    }
                    if result
                        .get("duplicate_navigation_call")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        duplicate_navigation_calls += 1;
                    }
                    if let Some(result_sot) = result.get("result").and_then(Value::as_str) {
                        let path =
                            PathBuf::from(result_sot.strip_prefix("file:").unwrap_or(result_sot));
                        if let Ok(tool_result) = read_json::<Value>(&path) {
                            collect_tool_result_paths(&tool_result, &mut seen_files);
                        }
                    }
                }
            }
        }
    }
    tools_used.sort();
    tools_used.dedup();
    let changed_seen: Vec<String> = changed_files
        .iter()
        .filter(|path| seen_files.contains(*path))
        .cloned()
        .collect();
    let changed_missing: Vec<String> = changed_files
        .iter()
        .filter(|path| !seen_files.contains(*path))
        .cloned()
        .collect();
    Ok(json!({
        "status": transcript.get("status"),
        "executed_tool_calls": transcript.get("executed_tool_calls"),
        "no_tool_rounds": no_tool_rounds,
        "edit_deadline_no_tool_rounds": edit_deadline_no_tool_rounds,
        "duplicate_navigation_calls": duplicate_navigation_calls,
        "tools_used": tools_used,
        "opened_or_inspected_files": seen_files.into_iter().collect::<Vec<_>>(),
        "changed_files_seen_by_tools": changed_seen,
        "changed_files_without_tool_read": changed_missing,
        "changed_files_read_before_edit": changed_missing.is_empty(),
        "source_of_truth": file_sot(&transcript_path)
    }))
}

fn extract_harness_transition_reality(transcript_path: Option<&str>) -> Result<Value> {
    let Some(path) = transcript_path else {
        return Ok(json!({
            "status": "missing",
            "transitions_total": 0,
            "source_of_truth": null
        }));
    };
    let transcript_path = PathBuf::from(path.strip_prefix("file:").unwrap_or(path));
    let transcript: Value = read_json(&transcript_path)?;
    let mut transitions_total = 0_u64;
    let mut correction_required_total = 0_u64;
    let mut utility_delta_sum = 0.0_f64;
    let mut by_tool: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_outcome: BTreeMap<String, u64> = BTreeMap::new();
    let mut by_tool_family: BTreeMap<String, u64> = BTreeMap::new();
    let mut transition_refs = Vec::new();
    if let Some(entries) = transcript.get("entries").and_then(Value::as_array) {
        for entry in entries {
            if let Some(results) = entry.get("tool_results").and_then(Value::as_array) {
                for result in results {
                    let transition_sot =
                        result
                            .get("harness_transition")
                            .and_then(Value::as_str)
                            .ok_or_else(|| {
                                RealityError::new(
                                    "RUST_REALITY_HARNESS_TRANSITION_REF_MISSING",
                                    "tool transcript entry was missing a harness transition source of truth",
                                    "agent_tool_transcript.entries[].tool_results[].harness_transition",
                                    "rerun with current Rust reality-loop so every tool call emits a transition artifact",
                                    json!({"tool_result": result, "transcript": file_sot(&transcript_path)}),
                                    Some(file_sot(&transcript_path)),
                                )
                            })?;
                    let transition_path = PathBuf::from(
                        transition_sot
                            .strip_prefix("file:")
                            .unwrap_or(transition_sot),
                    );
                    let transition: Value = read_json(&transition_path)?;
                    if transition.get("record_kind").and_then(Value::as_str)
                        != Some("contextgraph_harness_transition")
                    {
                        return Err(RealityError::new(
                            "RUST_REALITY_HARNESS_TRANSITION_KIND_INVALID",
                            "harness transition artifact had the wrong record_kind",
                            "harness_transition.record_kind",
                            "inspect transition artifact provenance and rerun the attempt",
                            json!({
                                "transition": file_sot(&transition_path),
                                "record_kind": transition.get("record_kind")
                            }),
                            Some(file_sot(&transition_path)),
                        ));
                    }
                    transitions_total += 1;
                    let tool = transition
                        .get("tool_name")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string();
                    *by_tool.entry(tool).or_insert(0) += 1;
                    let outcome = transition
                        .pointer("/outcome/label")
                        .and_then(Value::as_str)
                        .unwrap_or("Unknown")
                        .to_string();
                    *by_outcome.entry(outcome).or_insert(0) += 1;
                    let tool_family = transition
                        .pointer("/constellation_signal/tool_family")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown_tool_family")
                        .to_string();
                    *by_tool_family.entry(tool_family).or_insert(0) += 1;
                    utility_delta_sum += transition
                        .pointer("/outcome/utility_delta")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0);
                    if transition
                        .pointer("/outcome/correction_required")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        correction_required_total += 1;
                    }
                    transition_refs.push(json!({
                        "transition_id": transition.get("transition_id"),
                        "tool_name": transition.get("tool_name"),
                        "outcome": transition.get("outcome"),
                        "source_of_truth": file_sot(&transition_path),
                        "sha256": sha256_file(&transition_path)?
                    }));
                }
            }
        }
    }
    let executed = transcript
        .get("executed_tool_calls")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if transitions_total != executed {
        return Err(RealityError::new(
            "RUST_REALITY_HARNESS_TRANSITION_COUNT_MISMATCH",
            "harness transition count did not equal executed tool-call count",
            "agent_tool_transcript.executed_tool_calls",
            "inspect missing transition artifacts before using this attempt as training reality",
            json!({
                "executed_tool_calls": executed,
                "transitions_total": transitions_total,
                "transcript": file_sot(&transcript_path)
            }),
            Some(file_sot(&transcript_path)),
        ));
    }
    let useful_transition_count = by_outcome.get("Useful").copied().unwrap_or(0);
    let harmful_transition_count = by_outcome.get("Harmful").copied().unwrap_or(0);
    let neutral_transition_count = by_outcome.get("Neutral").copied().unwrap_or(0);
    let transition_coverage = if executed == 0 {
        1.0
    } else {
        transitions_total as f64 / executed as f64
    };
    let tool_family_selectors = by_tool_family.clone();
    Ok(json!({
        "record_kind": "contextgraph_harness_reality_summary",
        "status": transcript.get("status"),
        "transitions_total": transitions_total,
        "coverage": transition_coverage,
        "by_tool": by_tool,
        "by_outcome": by_outcome,
        "by_tool_family": by_tool_family,
        "transition_reward_signals": {
            "utility_delta_sum": utility_delta_sum,
            "correction_required_total": correction_required_total,
            "useful_transition_count": useful_transition_count,
            "harmful_transition_count": harmful_transition_count,
            "neutral_transition_count": neutral_transition_count,
            "transition_coverage": transition_coverage
        },
        "dynamicjepa_projection": {
            "state_action_outcome_rows": transitions_total,
            "surprise_candidates": harmful_transition_count,
            "planning_feedback_rows": transitions_total,
            "source": "contextgraph_harness_transition.dynamicjepa_mapping"
        },
        "learning_projection": {
            "learning_event_candidates": transitions_total,
            "correction_required_total": correction_required_total,
            "source": "contextgraph_harness_transition.learning_signal"
        },
        "constellation_projection": {
            "candidate_cluster_rows": transitions_total,
            "selectors": tool_family_selectors,
            "source": "contextgraph_harness_transition.constellation_signal"
        },
        "transitions": transition_refs,
        "source_of_truth": file_sot(&transcript_path)
    }))
}

fn collect_tool_result_paths(value: &Value, out: &mut BTreeSet<String>) {
    if let Some(path) = value
        .pointer("/result/payload/path")
        .and_then(Value::as_str)
    {
        out.insert(path.to_string());
    }
    if let Some(matches) = value
        .pointer("/result/payload/matches")
        .and_then(Value::as_array)
    {
        for item in matches {
            if let Some(path) = item.get("path").and_then(Value::as_str) {
                out.insert(path.to_string());
            }
        }
    }
    if let Some(files) = value
        .pointer("/result/payload/repository_context/files")
        .or_else(|| value.pointer("/result/payload/selected_context_files"))
        .or_else(|| value.pointer("/result/payload/files"))
        .and_then(Value::as_array)
    {
        for item in files {
            if let Some(path) = item
                .get("path")
                .and_then(Value::as_str)
                .or_else(|| item.as_str())
            {
                out.insert(path.to_string());
            }
        }
    }
    if let Some(edits) = value
        .pointer("/result/compilation/applied_edits")
        .or_else(|| value.pointer("/result/payload/compilation/applied_edits"))
        .and_then(Value::as_array)
    {
        for item in edits {
            if let Some(path) = item.get("path").and_then(Value::as_str) {
                out.insert(path.to_string());
            }
        }
    }
}

fn compact_reward_signal(reward: &Value) -> Value {
    json!({
        "reward": reward.get("reward"),
        "feature_vector": reward.get("feature_vector"),
        "reward_id": reward.get("reward_id"),
        "source_of_truth": reward.get("source_of_truth"),
        "source_record_refs_sha256": reward.get("source_record_refs_sha256")
    })
}

fn verify_reality_signal_packet(
    bin: &Path,
    ledger: &Path,
    run_id: &str,
    task_root: &Path,
    attempt_dir: &Path,
    packet_path: &Path,
    model: &str,
) -> Result<Value> {
    let input_path = attempt_dir.join("reality-signal-tool-input.json");
    write_json_checked(
        &input_path,
        &json!({
            "expected_delta": {
                "reality_signal_packet": "persisted_and_readable",
                "record_kind": "contextgraph_reality_signal_packet"
            },
            "packet": file_sot(packet_path)
        }),
    )?;
    let record = blackbox(
        bin,
        &[
            "blackbox",
            "record",
            "tool-call",
            "--ledger",
            path_str(ledger),
            "--run-id",
            run_id,
            "--tool",
            "rust_materialize_reality_signal_packet",
            "--input",
            path_str(&input_path),
            "--agent",
            "contextgraph_rust_reality_loop",
            "--model",
            model,
            "--capability",
            "contextgraph.rust.reality.signal_packet",
            "--status",
            "ok",
        ],
        task_root,
    )?;
    let tool_call_id = record
        .get("tool_call_id")
        .and_then(Value::as_str)
        .unwrap_or("tool_call_id_missing")
        .to_string();
    let rel_packet = packet_path.strip_prefix(task_root).unwrap_or(packet_path);
    let spec = json!({
        "verifier_id": "cmd_exit_zero_v1",
        "run_id": run_id,
        "tool_call_id": tool_call_id,
        "source": {"cwd": path_str(task_root)},
        "expected": {
            "command": [
                "bash",
                "-c",
                "test -s \"$1\" && grep -q '\"record_kind\": \"contextgraph_reality_signal_packet\"' \"$1\" && grep -q '\"reinjection_directive\"' \"$1\"",
                "verify-reality-signal-packet",
                path_str(rel_packet)
            ],
            "artifacts": [path_str(rel_packet)]
        }
    });
    let spec_path = attempt_dir.join("blackbox-reality-signal-verify-spec.json");
    write_json_checked(&spec_path, &spec)?;
    let verifier = blackbox(
        bin,
        &[
            "blackbox",
            "verify",
            "--ledger",
            path_str(ledger),
            "--input",
            path_str(&spec_path),
            "--model",
            model,
        ],
        task_root,
    )?;
    Ok(json!({
        "record": record,
        "verifier": verifier,
        "verifier_spec": file_sot(&spec_path),
        "source_of_truth": sqlite_sot(ledger)
    }))
}

fn compile_prompt(
    task: &TaskManifest,
    repo_context: &Value,
    problem_reality: &Value,
    attempts: &[AttemptRecord],
    previous_patch_hashes: &[String],
) -> String {
    let previous_attempt = attempts
        .last()
        .map(|item| serde_json::to_string_pretty(item).unwrap())
        .unwrap_or_else(|| "null".to_string());
    let previous_reality_signal = attempts
        .last()
        .map(|item| serde_json::to_string_pretty(&item.reality_signal_compact).unwrap())
        .unwrap_or_else(|| "null".to_string());
    let problem_reality =
        serde_json::to_string_pretty(&compact_problem_reality_packet(problem_reality)).unwrap();
    format!(
        "You are repairing a real SWE-bench Lite task through ContextGraph's Rust reality loop.\n\
Success means the official SWE-bench report places instance_id in resolved_ids.\n\
Do not claim success. Inspect reality with governed tools, then submit the candidate patch by calling apply_line_window_edit, apply_structured_edit, or apply_unified_diff with evidence copied from real source windows.\n\n\
Task:\ninstance_id: {}\nrepo: {}\nbase_commit: {}\nFAIL_TO_PASS: {}\n\n\
Problem statement:\n<<<PROBLEM\n{}\nPROBLEM\n\n\
Problem reality packet:\n{}\n\n\
Previous attempt summary:\n{}\n\n\
Previous reality signal packet for reinjection:\n{}\n\n\
Do not repeat these patch hashes:\n{}\n\n\
Hard validation rules:\n\
- Final candidate submission is tool-mediated. Call apply_line_window_edit, apply_structured_edit, or apply_unified_diff; do not return a hand-written JSON final answer or prose.\n\
- Each edit path must be repo-relative production source. Line-window and unified-diff submissions must include observed_sha256 values from opened file windows. Structured find text must match exactly one source span.\n\
- ContextGraph applies edits or diffs to the real checkout, generates candidate.patch with git diff, resets to base, then applies/evaluates the generated patch.\n\
- The edit must modify production source, not tests, fixtures, or benchmark harness files.\n\
- Do not edit tests to make the benchmark pass.\n\n\
Repository context JSON:\n{}\n\n\
Tool phase instruction: call governed ContextGraph tools to inspect real source, problem reality, failure reality, or prior verified attempts. If the previous reality signal says a changed file was not read by tools, read that file before editing again. When ready, call apply_line_window_edit, apply_structured_edit, or apply_unified_diff as the final candidate submission for this attempt.\n",
        task.instance_id,
        task.repo,
        task.base_commit,
        serde_json::to_string(&task.fail_to_pass).unwrap(),
        task.problem_statement,
        problem_reality,
        previous_attempt,
        previous_reality_signal,
        serde_json::to_string(previous_patch_hashes).unwrap(),
        serde_json::to_string_pretty(repo_context).unwrap()
    )
}

fn write_summary(
    run_root: &Path,
    task: &TaskManifest,
    args: &AttemptArgs,
    ledger: &Path,
    events: &Path,
    contract: &Path,
    problem_reality: &Path,
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
        "smoke_test": args.smoke_test,
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
        "problem_reality": {
            "path": path_str(problem_reality),
            "sha256": sha256_file(problem_reality)?,
            "source_of_truth": file_sot(problem_reality)
        },
        "source_of_truth": {
            "summary": file_sot(&path),
            "events": file_sot(events),
            "measurement_contract": file_sot(contract),
            "problem_reality": file_sot(problem_reality),
            "ledger": sqlite_sot(ledger)
        }
    });
    write_json_checked(&path, &summary)?;
    Ok(summary)
}

// ----- Phase 15: autoresearch engine artifacts ---------------------------------------
//
// After every attempt the engine writes durable autoresearch records under
// <run_root>/reality-optimizer/ so the outer optimizer (Claude Code) can reason
// about its own progress: attempts.jsonl is the compact per-attempt feed,
// reward-series.jsonl is the scalar trace, decisions.jsonl is the keep/discard
// verdict, and champion-state.json names the best-known result per (model, task).
// experiment-registry.json is initialized empty so the optimizer can append
// candidate harness-change experiments via MCP tools.

fn extract_reward_scalar(reward: Option<&Value>) -> Option<f64> {
    let r = reward?;
    // try several common shapes; the BlackBox reward compiler nests the scalar
    // under different paths depending on the spec version
    for path in [
        "/reward/reward/clipped",
        "/reward/reward/raw",
        "/reward/clipped",
        "/reward/raw",
        "/reward/scalar",
        "/reward",
        "/score",
    ] {
        if let Some(v) = r.pointer(path).and_then(Value::as_f64) {
            return Some(v);
        }
    }
    None
}

fn extract_official_resolved(record: &AttemptRecord) -> Option<bool> {
    record.resolved
}

fn write_optimizer_artifacts(
    run_root: &Path,
    model: &str,
    task: &TaskManifest,
    record: &AttemptRecord,
    reward: Option<&Value>,
    previous_attempts: &[AttemptRecord],
) -> Result<()> {
    let dir = run_root.join("reality-optimizer");
    create_dir_checked(&dir)?;

    // initialize experiment-registry.json if missing (optimizer populates it via MCP)
    let registry_path = dir.join("experiment-registry.json");
    if !registry_path.exists() {
        let init = json!({
            "schema_version": 1,
            "record_kind": "ccreality_experiment_registry",
            "created_at_unix": unix_secs()?,
            "experiments": []
        });
        write_json_checked(&registry_path, &init)?;
    }

    let reward_scalar = extract_reward_scalar(reward);
    let resolved = extract_official_resolved(record);
    let unix = unix_secs()?;

    // attempts.jsonl — compact per-attempt feed
    let attempts_path = dir.join("attempts.jsonl");
    let attempt_row = json!({
        "schema_version": 1,
        "record_kind": "ccreality_autoresearch_attempt",
        "model": model,
        "instance_id": task.instance_id,
        "repo": task.repo,
        "attempt": record.attempt,
        "status": record.status,
        "error_code": record.error_code,
        "patch_sha256": record.patch_sha256,
        "repeated_patch": record.repeated_patch,
        "official_resolved": resolved,
        "reward_scalar": reward_scalar,
        "reality_signal_packet": record.reality_signal_packet_path,
        "validation_path": record.validation_path,
        "created_at_unix": unix,
    });
    append_jsonl_checked(&attempts_path, &attempt_row)?;

    // reward-series.jsonl — scalar trace per (model, task, attempt)
    if let Some(scalar) = reward_scalar {
        let reward_path = dir.join("reward-series.jsonl");
        let row = json!({
            "schema_version": 1,
            "record_kind": "ccreality_reward_series_point",
            "model": model,
            "instance_id": task.instance_id,
            "attempt": record.attempt,
            "reward_scalar": scalar,
            "official_resolved": resolved,
            "created_at_unix": unix,
        });
        append_jsonl_checked(&reward_path, &row)?;
    }

    // decisions.jsonl — auto keep/discard/repair/escalate per docs/reality/05
    let decision = autoresearch_decision(record, reward_scalar, previous_attempts, model, task);
    let decisions_path = dir.join("decisions.jsonl");
    let decision_row = json!({
        "schema_version": 1,
        "record_kind": "ccreality_autoresearch_decision",
        "model": model,
        "instance_id": task.instance_id,
        "attempt": record.attempt,
        "decision": decision.decision,
        "reasoning": decision.reasoning,
        "reward_scalar": reward_scalar,
        "official_resolved": resolved,
        "created_at_unix": unix,
    });
    append_jsonl_checked(&decisions_path, &decision_row)?;

    // champion-state.json — best (model, task) result so far
    update_champion_state(&dir, model, task, record, reward_scalar, resolved)?;

    Ok(())
}

struct AutoresearchDecision {
    decision: &'static str,
    reasoning: String,
}

fn autoresearch_decision(
    record: &AttemptRecord,
    reward_scalar: Option<f64>,
    previous: &[AttemptRecord],
    _model: &str,
    _task: &TaskManifest,
) -> AutoresearchDecision {
    if record.resolved == Some(true) {
        return AutoresearchDecision {
            decision: "keep",
            reasoning: "official_resolved=true; promotion candidate per Phase 19".to_string(),
        };
    }
    if record.repeated_patch {
        return AutoresearchDecision {
            decision: "discard",
            reasoning: "repeated_patch detected; failure does not produce new evidence".to_string(),
        };
    }
    if record.error_code.is_some() && record.patch_sha256.is_none() {
        return AutoresearchDecision {
            decision: "escalate",
            reasoning: format!(
                "infrastructure or contract failure: error_code={}",
                record.error_code.as_deref().unwrap_or("unknown")
            ),
        };
    }
    let prev_reward = previous
        .iter()
        .filter_map(|p| {
            p.reality_signal_compact
                .pointer("/score_reality/reward/clipped")
                .and_then(Value::as_f64)
                .or_else(|| {
                    p.reality_signal_compact
                        .pointer("/score_reality/reward/raw")
                        .and_then(Value::as_f64)
                })
        })
        .last();
    match (reward_scalar, prev_reward) {
        (Some(cur), Some(prev)) if cur > prev + 0.05 => AutoresearchDecision {
            decision: "repair",
            reasoning: format!("reward improved (prev={prev:.3} → cur={cur:.3}); next attempt should continue this trajectory"),
        },
        (Some(cur), Some(prev)) if cur < prev - 0.05 => AutoresearchDecision {
            decision: "discard",
            reasoning: format!("reward regressed (prev={prev:.3} → cur={cur:.3})"),
        },
        _ => AutoresearchDecision {
            decision: "repair",
            reasoning: "reward flat or first attempt; continue iterating with updated prompt evidence".to_string(),
        },
    }
}

fn update_champion_state(
    dir: &Path,
    model: &str,
    task: &TaskManifest,
    record: &AttemptRecord,
    reward_scalar: Option<f64>,
    resolved: Option<bool>,
) -> Result<()> {
    let path = dir.join("champion-state.json");
    let mut state = if path.is_file() {
        read_json::<Value>(&path)?
    } else {
        json!({
            "schema_version": 1,
            "record_kind": "ccreality_champion_state",
            "champions": []
        })
    };
    let champions = state["champions"].as_array_mut().ok_or_else(|| {
        RealityError::new(
            "RUST_REALITY_CHAMPION_STATE_INVALID_SHAPE",
            "champion-state.json does not contain an array at .champions",
            "champion_state.shape",
            "wipe champion-state.json so the engine can re-initialize it",
            json!({"path": path_str(&path)}),
            Some(file_sot(&path)),
        )
    })?;

    // Find existing champion for (model, task)
    let position = champions.iter().position(|c| {
        c.get("model").and_then(Value::as_str) == Some(model)
            && c.get("task").and_then(Value::as_str) == Some(task.instance_id.as_str())
    });
    let candidate = json!({
        "model": model,
        "task": task.instance_id,
        "repo": task.repo,
        "attempt": record.attempt,
        "official_resolved": resolved,
        "best_reward": reward_scalar,
        "patch_sha256": record.patch_sha256,
        "reality_signal_packet": record.reality_signal_packet_path,
        "promoted_at_unix": Value::Null,  // promotion happens via Phase 19 only
        "candidate_at_unix": unix_secs()?,
    });

    let should_replace = match position {
        None => true,
        Some(idx) => {
            let existing = &champions[idx];
            let existing_resolved = existing.get("official_resolved").and_then(Value::as_bool);
            let existing_reward = existing.get("best_reward").and_then(Value::as_f64);
            // resolved beats unresolved; among same-resolved, higher reward wins
            match (resolved, existing_resolved) {
                (Some(true), Some(true)) | (Some(false), Some(false)) | (None, None) => {
                    matches!((reward_scalar, existing_reward),
                        (Some(cur), Some(prev)) if cur > prev)
                        || matches!((reward_scalar, existing_reward), (Some(_), None))
                }
                (Some(true), _) => true,
                (_, Some(true)) => false,
                _ => false,
            }
        }
    };
    if should_replace {
        match position {
            Some(idx) => champions[idx] = candidate,
            None => champions.push(candidate),
        }
        write_json_checked(&path, &state)?;
    }
    Ok(())
}

// ----- Phase 18: training export -----------------------------------------------------
//
// After every attempt with official Docker readback, the engine appends a verified
// training sample under <run_root>/training/. Four classes are produced:
//
//   positive.jsonl       — official_resolved + clean validation
//   negative.jsonl       — official_unresolved with verified failure
//   preference_pairs.jsonl — same (model, task) where this attempt's reward
//                            differs from a prior attempt's by >= 0.5
//   rejected_patches.jsonl — harness-rejected patches (malformed, repeated, etc.)
//
// The intent matches docs/fullloop/09_PHASE_18_TRAINING_EXPORT.md and
// docs/reality/07_implementation_roadmap.md Phase 5. Rows are append-only with
// read-after-write byte-equal verification.

fn write_training_export(
    run_root: &Path,
    model: &str,
    task: &TaskManifest,
    record: &AttemptRecord,
    reward: Option<&Value>,
    previous: &[AttemptRecord],
    attempt_dir: &Path,
) -> Result<()> {
    let dir = run_root.join("training");
    create_dir_checked(&dir)?;
    let unix = unix_secs()?;
    let reward_scalar = extract_reward_scalar(reward);
    let resolved = extract_official_resolved(record);
    let candidate_patch_path = attempt_dir.join("candidate.patch");
    let candidate_patch_pointer = if candidate_patch_path.is_file() {
        Some(file_sot(&candidate_patch_path))
    } else {
        None
    };

    // -- rejected_patches.jsonl: harness rejected this attempt's edit/patch ----
    if record.repeated_patch
        || matches!(
            record.error_code.as_deref(),
            Some("RUST_REALITY_PATCH_REPEATED")
                | Some("RUST_REALITY_PATCH_APPLY_FAILED")
                | Some("RUST_REALITY_FINAL_EDIT_TOOL_FAILED")
                | Some("RUST_REALITY_FINAL_EDIT_TOOL_REQUIRED")
                | Some("RUST_REALITY_STRUCTURED_EDIT_FIND_AMBIGUOUS")
                | Some("RUST_REALITY_MODEL_RESPONSE_JSON_INVALID")
        )
    {
        let row = json!({
            "schema_version": 1,
            "kind": "rejected_patch",
            "instance_id": task.instance_id,
            "repo": task.repo,
            "model": model,
            "attempt": record.attempt,
            "patch_sha256": record.patch_sha256,
            "reject_reason": record.error_code,
            "patch_text_pointer": candidate_patch_pointer,
            "validation_path": record.validation_path,
            "created_at_unix": unix,
        });
        append_jsonl_checked(&dir.join("rejected_patches.jsonl"), &row)?;
    }

    // -- positive.jsonl: official_resolved with no critical incident ---------------
    if resolved == Some(true) {
        let row = json!({
            "schema_version": 1,
            "kind": "positive",
            "instance_id": task.instance_id,
            "repo": task.repo,
            "model": model,
            "attempt": record.attempt,
            "prompt_sha256": record.prompt_sha256,
            "response_sha256": record.response_sha256,
            "patch_sha256": record.patch_sha256,
            "candidate_patch": candidate_patch_pointer,
            "official_evidence_path": record.official_evidence_path,
            "reality_signal_packet_path": record.reality_signal_packet_path,
            "reward_scalar": reward_scalar,
            "source_of_truth_refs": [
                record.official_evidence_path.clone().map(Value::String).unwrap_or(Value::Null),
                Value::String(record.reality_signal_packet_path.clone()),
            ],
            "created_at_unix": unix,
        });
        append_jsonl_checked(&dir.join("positive.jsonl"), &row)?;
    }
    // -- negative.jsonl: official_unresolved with verified failure -----------------
    else if resolved == Some(false) || record.status != "passed" {
        let failure_class = record
            .reality_signal_compact
            .pointer("/failure_digest/class")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .or_else(|| record.error_code.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let first_error_location = record
            .reality_signal_compact
            .pointer("/failure_digest/first_error_location")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let failed_test_names = record
            .reality_signal_compact
            .pointer("/official_progress/fail_to_pass/failed_tests")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let row = json!({
            "schema_version": 1,
            "kind": "negative",
            "instance_id": task.instance_id,
            "repo": task.repo,
            "model": model,
            "attempt": record.attempt,
            "prompt_sha256": record.prompt_sha256,
            "response_sha256": record.response_sha256,
            "patch_sha256": record.patch_sha256,
            "candidate_patch": candidate_patch_pointer,
            "failure_class": failure_class,
            "first_error_location": first_error_location,
            "failed_test_names": failed_test_names,
            "reality_signal_packet_path": record.reality_signal_packet_path,
            "reward_scalar": reward_scalar,
            "created_at_unix": unix,
        });
        append_jsonl_checked(&dir.join("negative.jsonl"), &row)?;
    }

    // -- preference_pairs.jsonl: this attempt vs a prior attempt with reward delta ≥ 0.5 ---
    if let Some(cur_reward) = reward_scalar {
        for prev in previous.iter() {
            let prev_reward = prev
                .reality_signal_compact
                .pointer("/score_reality/reward/clipped")
                .and_then(Value::as_f64)
                .or_else(|| {
                    prev.reality_signal_compact
                        .pointer("/score_reality/reward/raw")
                        .and_then(Value::as_f64)
                });
            let Some(prev_r) = prev_reward else { continue };
            let delta = (cur_reward - prev_r).abs();
            if delta < 0.5 {
                continue;
            }
            let (preferred, rejected, preferred_reward, rejected_reward) = if cur_reward > prev_r {
                (record, prev, cur_reward, prev_r)
            } else {
                (prev, record, prev_r, cur_reward)
            };
            let row = json!({
                "schema_version": 1,
                "kind": "preference_pair",
                "instance_id": task.instance_id,
                "model": model,
                "preferred_attempt": preferred.attempt,
                "preferred_reward": preferred_reward,
                "preferred_patch_sha256": preferred.patch_sha256,
                "preferred_reality_signal_packet": preferred.reality_signal_packet_path,
                "rejected_attempt": rejected.attempt,
                "rejected_reward": rejected_reward,
                "rejected_patch_sha256": rejected.patch_sha256,
                "rejected_reality_signal_packet": rejected.reality_signal_packet_path,
                "preference_strength": delta,
                "rationale": format!(
                    "preferred attempt {} reward={:.3}; rejected attempt {} reward={:.3}",
                    preferred.attempt, preferred_reward, rejected.attempt, rejected_reward
                ),
                "created_at_unix": unix,
            });
            append_jsonl_checked(&dir.join("preference_pairs.jsonl"), &row)?;
        }
    }

    // -- manifest.json: summary the operator + LoRA training driver consult -----
    let manifest_path = dir.join("manifest.json");
    let counts = count_training_jsonl(&dir)?;
    let manifest = json!({
        "schema_version": 1,
        "record_kind": "ccreality_training_export_manifest",
        "exporter_version": "v1",
        "active_runtime_root": run_root.display().to_string(),
        "model": model,
        "instance_id": task.instance_id,
        "counts": counts,
        "last_updated_at_unix": unix,
    });
    write_json_checked(&manifest_path, &manifest)?;

    Ok(())
}

fn count_training_jsonl(dir: &Path) -> Result<Value> {
    let mut counts = serde_json::Map::new();
    for class in [
        "positive",
        "negative",
        "preference_pairs",
        "rejected_patches",
    ] {
        let path = dir.join(format!("{class}.jsonl"));
        let n: u64 = if path.is_file() {
            fs::read_to_string(&path)
                .map_err(|err| fs_error("RUST_REALITY_TRAINING_COUNT_READ_FAILED", &path, err))?
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count() as u64
        } else {
            0
        };
        counts.insert(format!("{class}_count"), json!(n));
    }
    Ok(Value::Object(counts))
}

fn append_jsonl_checked(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_checked(parent)?;
    }
    let line = serde_json::to_string(value).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_OPTIMIZER_ARTIFACT_SERIALIZE_FAILED",
            "failed to serialize autoresearch artifact row",
            "optimizer_artifact.serialize",
            "inspect the row payload",
            json!({"error": err.to_string()}),
            Some(file_sot(path)),
        )
    })?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| fs_error("RUST_REALITY_OPTIMIZER_ARTIFACT_OPEN_FAILED", path, err))?;
    writeln!(file, "{line}")
        .map_err(|err| fs_error("RUST_REALITY_OPTIMIZER_ARTIFACT_WRITE_FAILED", path, err))?;
    file.sync_all()
        .map_err(|err| fs_error("RUST_REALITY_OPTIMIZER_ARTIFACT_SYNC_FAILED", path, err))?;
    let readback = fs::read_to_string(path)
        .map_err(|err| fs_error("RUST_REALITY_OPTIMIZER_ARTIFACT_READBACK_FAILED", path, err))?;
    let last_line = readback.lines().last().unwrap_or("");
    if last_line != line {
        return Err(RealityError::new(
            "RUST_REALITY_OPTIMIZER_ARTIFACT_READBACK_MISMATCH",
            "autoresearch artifact readback did not match written row",
            "optimizer_artifact.readback",
            "inspect filesystem durability",
            json!({"path": path_str(path), "expected": line, "actual": last_line}),
            Some(file_sot(path)),
        ));
    }
    Ok(())
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
    let mut file = fs::File::create(path)
        .map_err(|err| fs_error("RUST_REALITY_JSON_WRITE_CREATE_FAILED", path, err))?;
    file.write_all(text.as_bytes())
        .map_err(|err| fs_error("RUST_REALITY_JSON_WRITE_FAILED", path, err))?;
    file.sync_all()
        .map_err(|err| fs_error("RUST_REALITY_JSON_SYNC_FAILED", path, err))?;
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
    let mut file = fs::File::create(path)
        .map_err(|err| fs_error("RUST_REALITY_TEXT_WRITE_CREATE_FAILED", path, err))?;
    file.write_all(text.as_bytes())
        .map_err(|err| fs_error("RUST_REALITY_TEXT_WRITE_FAILED", path, err))?;
    file.sync_all()
        .map_err(|err| fs_error("RUST_REALITY_TEXT_SYNC_FAILED", path, err))?;
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

fn update_active_runtime_root(runtime_root: &Path, target_instance: Option<&str>) -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| {
        RealityError::new(
            "RUST_REALITY_HOME_DIR_MISSING",
            "could not resolve $HOME for cgreality cache",
            "process.env.HOME",
            "set HOME to a writable directory",
            json!({}),
            None,
        )
    })?;
    let cache = home.join(".cache/cgreality");
    create_dir_checked(&cache)?;
    let canonical = runtime_root.canonicalize().map_err(|err| {
        fs_error(
            "RUST_REALITY_RUNTIME_ROOT_CANONICALIZE_FAILED",
            runtime_root,
            err,
        )
    })?;
    write_text_checked(
        &cache.join("active_runtime_root"),
        &canonical.to_string_lossy(),
    )?;
    if let Some(target_instance) = target_instance {
        write_text_checked(&cache.join("active_target_instance"), target_instance)?;
    }
    Ok(())
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

fn optional_file_sot(path: &Path) -> Value {
    if path.exists() {
        json!({"exists": true, "source_of_truth": file_sot(path)})
    } else {
        json!({"exists": false, "path": path_str(path)})
    }
}

fn file_sot_to_path(sot: &str, code: &str, field_path: &str) -> Result<PathBuf> {
    let Some(path) = sot.strip_prefix("file:") else {
        return Err(RealityError::new(
            if code.is_empty() {
                "RUST_REALITY_FILE_SOT_INVALID"
            } else {
                code
            },
            "source-of-truth pointer was not a file: URI",
            if field_path.is_empty() {
                "source_of_truth"
            } else {
                field_path
            },
            "use file:/absolute/path for filesystem source-of-truth pointers",
            json!({"source_of_truth": sot}),
            Some(sot.to_string()),
        ));
    };
    if path.trim().is_empty() {
        return Err(RealityError::new(
            if code.is_empty() {
                "RUST_REALITY_FILE_SOT_EMPTY"
            } else {
                code
            },
            "source-of-truth file path was empty",
            if field_path.is_empty() {
                "source_of_truth"
            } else {
                field_path
            },
            "provide a non-empty file:/absolute/path pointer",
            json!({"source_of_truth": sot}),
            Some(sot.to_string()),
        ));
    }
    Ok(PathBuf::from(path))
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn init_real_git_repo() -> (TempDir, PathBuf) {
        let temp = tempfile::tempdir().expect("tempdir");
        let repo = temp.path().join("repo");
        create_dir_checked(&repo).expect("repo dir");
        run_required(
            &["git", "init"],
            &repo,
            30,
            "TEST_GIT_INIT_FAILED",
            "test.git.init",
        )
        .expect("git init");
        run_required(
            &[
                "git",
                "config",
                "user.email",
                "reality-loop@example.invalid",
            ],
            &repo,
            30,
            "TEST_GIT_CONFIG_EMAIL_FAILED",
            "test.git.config.email",
        )
        .expect("git config email");
        run_required(
            &["git", "config", "user.name", "Reality Loop FSV"],
            &repo,
            30,
            "TEST_GIT_CONFIG_NAME_FAILED",
            "test.git.config.name",
        )
        .expect("git config name");
        write_text_checked(
            &repo.join("src/example.py"),
            "def bump(x):\n    return x + 1\n",
        )
        .expect("source write");
        run_required(
            &["git", "add", "src/example.py"],
            &repo,
            30,
            "TEST_GIT_ADD_FAILED",
            "test.git.add",
        )
        .expect("git add");
        run_required(
            &["git", "commit", "-m", "base"],
            &repo,
            30,
            "TEST_GIT_COMMIT_FAILED",
            "test.git.commit",
        )
        .expect("git commit");
        (temp, repo)
    }

    #[test]
    fn tool_mediated_final_edit_submission_fsv() {
        let (_temp, repo) = init_real_git_repo();
        let tool_dir = repo
            .parent()
            .expect("repo parent")
            .join("attempt-000001")
            .join("agent-tools")
            .join("round-01-00-apply_structured_edit");
        create_dir_checked(&tool_dir).expect("tool dir");
        let before = fs::read_to_string(repo.join("src/example.py")).expect("before read");
        eprintln!(
            "BEFORE source_of_truth={} data={:?}",
            file_sot(&repo.join("src/example.py")),
            before
        );

        let edits = vec![json!({
            "path": "src/example.py",
            "find": "def bump(x):\n    return x + 1\n",
            "replace": "def bump(x):\n    return x + 2\n"
        })];
        let (patch, compilation) =
            compile_structured_edits(&repo, &tool_dir, &edits, 30).expect("compile edits");
        let patch = patch.unwrap_or_else(|| {
            panic!(
                "structured edit compilation did not produce a patch: {}",
                serde_json::to_string_pretty(&compilation).unwrap()
            )
        });
        let candidate_patch = tool_dir.join("candidate.patch");
        let result = json!({
            "status": "ok",
            "tool_name": "apply_structured_edit",
            "patch_sha256": sha256_file(&candidate_patch).expect("patch sha"),
            "compilation": compilation
        });
        let submission =
            materialize_tool_candidate_submission(&result, &tool_dir).expect("submission");
        let model_response = json!({
            "contextgraph_runtime": {
                "submitted_candidate_patch": submission
            }
        });
        let (readback_patch, parse_report) = patch_from_tool_submission(&model_response)
            .expect("patch extraction")
            .expect("submitted patch");
        assert_eq!(patch, readback_patch);
        assert_eq!(parse_report["status"], "passed");
        assert_eq!(
            parse_report["final_action_mode"],
            "tool_mediated_final_edit_submission"
        );

        run_required(
            &["git", "reset", "--hard", "HEAD"],
            &repo,
            30,
            "TEST_GIT_RESET_FAILED",
            "test.git.reset",
        )
        .expect("reset");
        run_required(
            &["git", "apply", "--check", path_str(&candidate_patch)],
            &repo,
            30,
            "TEST_GIT_APPLY_CHECK_FAILED",
            "test.git.apply_check",
        )
        .expect("git apply check");
        run_required(
            &["git", "apply", path_str(&candidate_patch)],
            &repo,
            30,
            "TEST_GIT_APPLY_FAILED",
            "test.git.apply",
        )
        .expect("git apply");
        let after = fs::read_to_string(repo.join("src/example.py")).expect("after read");
        eprintln!(
            "AFTER source_of_truth={} data={:?}",
            file_sot(&repo.join("src/example.py")),
            after
        );
        assert_eq!(after, "def bump(x):\n    return x + 2\n");
    }

    #[test]
    fn unified_diff_submission_is_tool_mediated_and_observed_file_backed_fsv() {
        let (_temp, repo) = init_real_git_repo();
        let tool_dir = repo
            .parent()
            .expect("repo parent")
            .join("attempt-000001")
            .join("agent-tools")
            .join("round-01-00-apply_unified_diff");
        create_dir_checked(&tool_dir).expect("tool dir");
        let source = repo.join("src/example.py");
        let observed_sha256 = sha256_file(&source).expect("source sha");
        let before = fs::read_to_string(&source).expect("before read");
        eprintln!(
            "BEFORE unified_diff source_of_truth={} sha256={} data={:?}",
            file_sot(&source),
            observed_sha256,
            before
        );
        let patch_text = "\
diff --git a/src/example.py b/src/example.py
--- a/src/example.py
+++ b/src/example.py
@@ -1,2 +1,2 @@
 def bump(x):
-    return x + 1
+    return x + 2
";
        let observed_files =
            vec![json!({"path": "src/example.py", "observed_sha256": observed_sha256})];
        let (patch, compilation) =
            compile_unified_diff_submission(&repo, &tool_dir, patch_text, &observed_files, 30)
                .expect("compile unified diff");
        assert_eq!(compilation["status"], "passed");
        let patch = patch.unwrap_or_else(|| {
            panic!(
                "unified diff compilation did not produce a patch: {}",
                serde_json::to_string_pretty(&compilation).unwrap()
            )
        });
        let after = fs::read_to_string(&source).expect("after read");
        eprintln!(
            "AFTER unified_diff source_of_truth={} data={:?} compilation={}",
            file_sot(&source),
            after,
            serde_json::to_string_pretty(&compilation).unwrap()
        );
        assert!(after.contains("return x + 2"));
        let candidate_patch = tool_dir.join("candidate.patch");
        let result = json!({
            "status": "ok",
            "tool_name": "apply_unified_diff",
            "patch_sha256": sha256_file(&candidate_patch).expect("patch sha"),
            "compilation": compilation
        });
        let submission =
            materialize_tool_candidate_submission(&result, &tool_dir).expect("submission");
        assert_eq!(submission["tool_name"], "apply_unified_diff");
        assert_eq!(
            patch,
            fs::read_to_string(&candidate_patch).expect("patch readback")
        );
        run_required(
            &["git", "reset", "--hard", "HEAD"],
            &repo,
            30,
            "TEST_GIT_RESET_FAILED",
            "test.git.reset",
        )
        .expect("git reset");
        run_required(
            &["git", "apply", "--check", path_str(&candidate_patch)],
            &repo,
            30,
            "TEST_GIT_APPLY_CHECK_FAILED",
            "test.git.apply_check",
        )
        .expect("git apply check");
    }

    #[test]
    fn line_window_edit_submission_fsv() {
        let (_temp, repo) = init_real_git_repo();
        let tool_dir = repo
            .parent()
            .expect("repo parent")
            .join("attempt-000001")
            .join("agent-tools")
            .join("round-01-00-apply_line_window_edit");
        create_dir_checked(&tool_dir).expect("tool dir");
        let source = repo.join("src/example.py");
        let before = fs::read_to_string(&source).expect("before read");
        let observed_sha256 = sha256_file(&source).expect("source sha");
        eprintln!(
            "BEFORE line_window source_of_truth={} sha256={} data={:?}",
            file_sot(&source),
            observed_sha256,
            before
        );
        let edits = vec![json!({
            "path": "src/example.py",
            "start_line": 2,
            "end_line": 2,
            "replace": "    return x + 3\n",
            "observed_sha256": observed_sha256
        })];
        let (patch, compilation) =
            compile_line_window_edits(&repo, &tool_dir, &edits, 30).expect("compile line edits");
        assert!(
            patch.is_some(),
            "line-window edit compilation did not produce a patch: {}",
            serde_json::to_string_pretty(&compilation).unwrap()
        );
        let candidate_patch = tool_dir.join("candidate.patch");
        let result = json!({
            "status": "ok",
            "tool_name": "apply_line_window_edit",
            "patch_sha256": sha256_file(&candidate_patch).expect("patch sha"),
            "compilation": compilation
        });
        let submission =
            materialize_tool_candidate_submission(&result, &tool_dir).expect("submission");
        assert_eq!(submission["tool_name"], "apply_line_window_edit");
        let wrapped_result = json!({
            "record_kind": "contextgraph_agent_tool_result",
            "tool_name": "apply_line_window_edit",
            "status": "ok",
            "result": result,
            "source_of_truth": file_sot(&tool_dir.join("tool-result.json"))
        });
        let wrapped_submission = materialize_tool_candidate_submission(&wrapped_result, &tool_dir)
            .expect("wrapped submission");
        eprintln!(
            "AFTER wrapped_line_window_submission source_of_truth={} submission={}",
            file_sot(&candidate_patch),
            serde_json::to_string_pretty(&wrapped_submission).unwrap()
        );
        assert_eq!(wrapped_submission["tool_name"], "apply_line_window_edit");
        assert_eq!(
            wrapped_submission["patch_sha256"],
            submission["patch_sha256"]
        );
        run_required(
            &["git", "reset", "--hard", "HEAD"],
            &repo,
            30,
            "TEST_GIT_RESET_FAILED",
            "test.git.reset",
        )
        .expect("reset");
        run_required(
            &["git", "apply", path_str(&candidate_patch)],
            &repo,
            30,
            "TEST_GIT_APPLY_FAILED",
            "test.git.apply",
        )
        .expect("git apply");
        let after = fs::read_to_string(&source).expect("after read");
        eprintln!(
            "AFTER line_window source_of_truth={} data={:?}",
            file_sot(&source),
            after
        );
        assert_eq!(after, "def bump(x):\n    return x + 3\n");
    }

    #[test]
    fn line_window_edit_rejects_stale_observed_sha256() {
        let (_temp, repo) = init_real_git_repo();
        let tool_dir = repo
            .parent()
            .expect("repo parent")
            .join("attempt-000001")
            .join("agent-tools")
            .join("round-01-00-apply_line_window_edit");
        create_dir_checked(&tool_dir).expect("tool dir");
        let source = repo.join("src/example.py");
        let actual_sha256 = sha256_file(&source).expect("source sha");
        let edits = vec![json!({
            "path": "src/example.py",
            "start_line": 2,
            "end_line": 2,
            "replace": "    return x + 3\n",
            "observed_sha256": "sha256:stale"
        })];
        eprintln!(
            "BEFORE stale_line_window source_of_truth={} actual_sha256={}",
            file_sot(&source),
            actual_sha256
        );
        let (patch, compilation) =
            compile_line_window_edits(&repo, &tool_dir, &edits, 30).expect("compile line edits");
        eprintln!(
            "AFTER stale_line_window compilation={}",
            serde_json::to_string_pretty(&compilation).unwrap()
        );
        assert!(patch.is_none());
        assert_eq!(
            compilation["error_code"],
            "RUST_REALITY_LINE_EDIT_OBSERVED_SHA_MISMATCH"
        );
    }

    #[test]
    fn line_window_edit_rejects_python_syntax_failure_before_candidate_patch_fsv() {
        let (_temp, repo) = init_real_git_repo();
        let tool_dir = repo
            .parent()
            .expect("repo parent")
            .join("attempt-000001")
            .join("agent-tools")
            .join("round-01-00-apply_line_window_edit");
        create_dir_checked(&tool_dir).expect("tool dir");
        let source = repo.join("src/example.py");
        let before = fs::read_to_string(&source).expect("before read");
        let observed_sha256 = sha256_file(&source).expect("source sha");
        eprintln!(
            "BEFORE syntax_line_window source_of_truth={} sha256={} data={:?}",
            file_sot(&source),
            observed_sha256,
            before
        );
        let edits = vec![json!({
            "path": "src/example.py",
            "start_line": 2,
            "end_line": 2,
            "replace": "    return \"\"\"unterminated\n",
            "observed_sha256": observed_sha256
        })];
        let (patch, compilation) =
            compile_line_window_edits(&repo, &tool_dir, &edits, 30).expect("compile line edits");
        let after = fs::read_to_string(&source).expect("after read");
        eprintln!(
            "AFTER syntax_line_window compilation={} data={:?}",
            serde_json::to_string_pretty(&compilation).unwrap(),
            after
        );
        assert!(patch.is_none());
        assert_eq!(
            compilation["error_code"],
            "RUST_REALITY_LINE_EDIT_PY_COMPILE_FAILED"
        );
        assert_eq!(after, before);
        assert!(
            !tool_dir.join("candidate.patch").exists(),
            "candidate.patch must not be written for syntactically invalid edits"
        );
        assert_eq!(
            compilation
                .pointer("/details/syntax_probe/process/exit_code")
                .and_then(Value::as_i64),
            Some(1)
        );
        assert_eq!(
            compilation
                .pointer("/details/workspace_restore/status")
                .and_then(Value::as_str),
            Some("ok")
        );
    }

    #[test]
    fn structured_edit_rejects_python_syntax_failure_before_candidate_patch_fsv() {
        let (_temp, repo) = init_real_git_repo();
        let tool_dir = repo
            .parent()
            .expect("repo parent")
            .join("attempt-000001")
            .join("agent-tools")
            .join("round-01-00-apply_structured_edit");
        create_dir_checked(&tool_dir).expect("tool dir");
        let source = repo.join("src/example.py");
        let before = fs::read_to_string(&source).expect("before read");
        eprintln!(
            "BEFORE syntax_structured source_of_truth={} data={:?}",
            file_sot(&source),
            before
        );
        let edits = vec![json!({
            "path": "src/example.py",
            "find": "    return x + 1\n",
            "replace": "    return (\n"
        })];
        let (patch, compilation) =
            compile_structured_edits(&repo, &tool_dir, &edits, 30).expect("compile edits");
        let after = fs::read_to_string(&source).expect("after read");
        eprintln!(
            "AFTER syntax_structured compilation={} data={:?}",
            serde_json::to_string_pretty(&compilation).unwrap(),
            after
        );
        assert!(patch.is_none());
        assert_eq!(
            compilation["error_code"],
            "RUST_REALITY_STRUCTURED_EDIT_PY_COMPILE_FAILED"
        );
        assert_eq!(after, before);
        assert!(
            !tool_dir.join("candidate.patch").exists(),
            "candidate.patch must not be written for syntactically invalid edits"
        );
        assert_eq!(
            compilation
                .pointer("/details/syntax_probe/process/exit_code")
                .and_then(Value::as_i64),
            Some(1)
        );
    }

    #[test]
    fn editable_window_compacts_real_tool_result_payload_shape() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("requests").join("models.py");
        fs::create_dir_all(source.parent().expect("source parent")).expect("source parent");
        fs::write(
            &source,
            "class Request(object):\n    def prepare_method(self, method):\n        pass\n",
        )
        .expect("source write");
        let observed_sha256 = sha256_file(&source).expect("source sha");
        let result_readback = json!({
            "record_kind": "contextgraph_agent_tool_result",
            "tool_name": "open_file_window",
            "status": "ok",
            "result": {
                "action": "read file line window",
                "status": "ok",
                "tool_name": "open_file_window",
                "payload": {
                    "status": "ok",
                    "path": "requests/models.py",
                    "start_line": 1,
                    "end_line": 3,
                    "sha256": observed_sha256,
                    "source_of_truth": file_sot(&source),
                    "text": "class Request(object):\n    def prepare_method(self, method):\n        pass\n",
                    "total_lines": 3
                }
            },
            "source_of_truth": file_sot(&temp.path().join("tool-result.json"))
        });
        eprintln!(
            "BEFORE compact_editable_window source_of_truth={} payload={}",
            file_sot(&source),
            serde_json::to_string_pretty(&result_readback).unwrap()
        );
        let compact = compact_editable_window(&result_readback).expect("editable window");
        eprintln!(
            "AFTER compact_editable_window compact={}",
            serde_json::to_string_pretty(&compact).unwrap()
        );
        assert_eq!(compact["path"], "requests/models.py");
        assert_eq!(compact["sha256"], observed_sha256);
        assert_eq!(compact["start_line"], 1);
        assert_eq!(compact["end_line"], 3);
        assert_eq!(compact["edit_tool"], "apply_line_window_edit");
    }

    #[test]
    fn tool_submission_rejects_missing_patch_source_of_truth() {
        let temp = tempfile::tempdir().expect("tempdir");
        let tool_dir = temp.path().join("tool");
        create_dir_checked(&tool_dir).expect("tool dir");
        let before = json!({"tool_result": {"status": "ok"}});
        eprintln!("BEFORE missing_sot={}", before);
        let err = materialize_tool_candidate_submission(&before["tool_result"], &tool_dir)
            .expect_err("missing SOT should fail");
        eprintln!(
            "AFTER error={}",
            serde_json::to_string_pretty(&err).unwrap()
        );
        assert_eq!(
            err.error_code,
            "RUST_REALITY_TOOL_SUBMITTED_PATCH_SOT_MISSING"
        );
    }

    #[test]
    fn tool_submission_rejects_patch_hash_mismatch() {
        let temp = tempfile::tempdir().expect("tempdir");
        let tool_dir = temp.path().join("tool");
        create_dir_checked(&tool_dir).expect("tool dir");
        let patch_path = tool_dir.join("candidate.patch");
        write_text_checked(
            &patch_path,
            "diff --git a/src/example.py b/src/example.py\n--- a/src/example.py\n+++ b/src/example.py\n@@ -1,2 +1,2 @@\n def bump(x):\n-    return x + 1\n+    return x + 2\n",
        )
        .expect("patch write");
        let result = json!({
            "status": "ok",
            "patch_sha256": "sha256:not-the-real-hash",
            "compilation": {
                "source_of_truth": {
                    "candidate_patch": file_sot(&patch_path)
                }
            }
        });
        eprintln!(
            "BEFORE hash_check expected={} actual={}",
            result["patch_sha256"],
            sha256_file(&patch_path).unwrap()
        );
        let err =
            materialize_tool_candidate_submission(&result, &tool_dir).expect_err("hash mismatch");
        eprintln!(
            "AFTER error={}",
            serde_json::to_string_pretty(&err).unwrap()
        );
        assert_eq!(
            err.error_code,
            "RUST_REALITY_TOOL_SUBMITTED_PATCH_SHA_MISMATCH"
        );
    }

    #[test]
    fn final_submission_rejects_empty_patch_readback() {
        let temp = tempfile::tempdir().expect("tempdir");
        let patch_path = temp.path().join("candidate.patch");
        write_text_checked(&patch_path, "").expect("empty patch write");
        let model_response = json!({
            "contextgraph_runtime": {
                "submitted_candidate_patch": {
                    "candidate_patch_path": path_str(&patch_path),
                    "patch_sha256": sha256_file(&patch_path).unwrap()
                }
            }
        });
        eprintln!(
            "BEFORE empty_patch source_of_truth={} sha256={}",
            file_sot(&patch_path),
            sha256_file(&patch_path).unwrap()
        );
        let err = patch_from_tool_submission(&model_response).expect_err("empty patch should fail");
        eprintln!(
            "AFTER error={}",
            serde_json::to_string_pretty(&err).unwrap()
        );
        assert_eq!(
            err.error_code,
            "RUST_REALITY_TOOL_SUBMITTED_PATCH_READBACK_EMPTY"
        );
    }

    #[test]
    fn edit_deadline_restricts_tool_surface_after_open_window_stall() {
        let window = json!({
            "path": "requests/models.py",
            "start_line": 340,
            "end_line": 370,
            "sha256": "sha256:window",
            "source_of_truth": "file:/tmp/repo/requests/models.py"
        });
        eprintln!("BEFORE edit_deadline window={window}");
        assert!(!edit_deadline_active(4, 18, &[window.clone()], 0, 0, false));
        assert!(edit_deadline_active(6, 18, &[window.clone()], 1, 0, false));
        let tools = agent_edit_tool_specs();
        let tool_names = tools
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|tool| tool.pointer("/function/name").and_then(Value::as_str))
            .collect::<Vec<_>>();
        eprintln!("AFTER edit_deadline tools={tool_names:?}");
        assert_eq!(
            tool_names,
            vec![
                "apply_structured_edit",
                "apply_line_window_edit",
                "apply_unified_diff"
            ]
        );
        let prompt = no_tool_repair_prompt(true, &[window], 1, 0);
        assert!(prompt.contains("EDIT DEADLINE IS ACTIVE"));
        assert!(prompt.contains("sha256:window"));
        assert!(prompt.contains("apply_line_window_edit"));
        assert!(prompt.contains("apply_unified_diff"));
    }

    #[test]
    fn local_python_interpreter_is_real_source_of_truth() {
        let temp = tempfile::tempdir().expect("tempdir");
        let process = run_cmd(
            &[LOCAL_PYTHON_BIN.to_string(), "--version".to_string()],
            temp.path(),
            30,
        )
        .expect("python version command");
        eprintln!(
            "PYTHON_SOURCE_OF_TRUTH command={:?} exit={} stdout={:?} stderr={:?}",
            process.command, process.exit_code, process.stdout, process.stderr
        );
        assert_eq!(process.exit_code, 0);
        assert!(process.stdout.contains("Python") || process.stderr.contains("Python"));
    }

    #[test]
    fn problem_production_candidates_priority_zero_is_issue_text_mention_fsv() {
        // Regression for empirical bug observed in
        //   /tmp/ccreality-prod-3/rust_reality_1778040002059614711_e525987d/problem-reality-packet.json
        // BEFORE FIX: BTreeSet alphabetized — requests/models.py landed at index 0 and
        // requests/sessions.py at index 6, even though the issue text only named sessions.py.
        // EXPECTED AFTER FIX: sessions.py lands at priority-0 (issue-text mention preserved).
        let task = TaskManifest {
            dataset: None,
            instance_id: "psf__requests-2317".to_string(),
            repo: "psf/requests".to_string(),
            base_commit: "deadbeef".to_string(),
            problem_statement:
                "Method = builtins.str(method) in requests/sessions.py is causing a UnicodeDecodeError when the URL contains non-ASCII characters. See test_requests.py for the failing test."
                    .to_string(),
            fail_to_pass: vec![
                "test_requests.py::RequestsTestCase::test_unicode_method_name".to_string(),
            ],
            pass_to_pass_count: None,
            official_test_patch_sha256: None,
            oracle_patch_sha256: None,
            source_of_truth: None,
        };
        let all_files = vec![
            "requests/__init__.py".to_string(),
            "requests/adapters.py".to_string(),
            "requests/auth.py".to_string(),
            "requests/models.py".to_string(),
            "requests/sessions.py".to_string(),
            "requests/structures.py".to_string(),
            "test_requests.py".to_string(),
        ];
        let repo_context = json!({
            "files": [
                {"path": "requests/adapters.py"},
                {"path": "requests/auth.py"},
                {"path": "requests/models.py"},
                {"path": "requests/sessions.py"},
                {"path": "requests/structures.py"}
            ]
        });
        eprintln!(
            "BEFORE_FIX_EVIDENCE empirical_packet=/tmp/ccreality-prod-3/rust_reality_1778040002059614711_e525987d/problem-reality-packet.json sessions_index_was=6 models_index_was=0"
        );
        let result = problem_production_candidates(&task, &repo_context, &all_files);
        let arr = result.as_array().expect("result is array");
        let paths: Vec<&str> = arr.iter().map(|v| v.as_str().expect("string")).collect();
        eprintln!(
            "AFTER_FIX paths_index_0_5={:?} total={}",
            &paths[..paths.len().min(6)],
            paths.len()
        );
        assert_eq!(
            paths[0], "requests/sessions.py",
            "issue-text-mentioned production file must land at priority 0; got {:?}",
            paths
        );
        let sessions_idx = paths
            .iter()
            .position(|&p| p == "requests/sessions.py")
            .expect("sessions.py present");
        let models_idx = paths
            .iter()
            .position(|&p| p == "requests/models.py")
            .expect("models.py present");
        assert!(
            sessions_idx < models_idx,
            "sessions.py must precede models.py — issue-text priority over BM25; got {:?}",
            paths
        );
    }

    #[test]
    fn problem_production_candidates_handles_no_py_mentions_edge() {
        // Edge case 1: issue text has no .py mentions at all. production_path_candidates
        // returns empty; repo_context drives entirely; no panic, no error.
        let task = TaskManifest {
            dataset: None,
            instance_id: "test_no_py_mentions".to_string(),
            repo: "x/y".to_string(),
            base_commit: "deadbeef".to_string(),
            problem_statement: "Something is broken when temperatures exceed 100 degrees."
                .to_string(),
            fail_to_pass: vec![],
            pass_to_pass_count: None,
            official_test_patch_sha256: None,
            oracle_patch_sha256: None,
            source_of_truth: None,
        };
        let all_files = vec!["src/foo.py".to_string(), "src/bar.py".to_string()];
        let repo_context = json!({
            "files": [{"path": "src/foo.py"}, {"path": "src/bar.py"}]
        });
        eprintln!(
            "BEFORE_NO_PY problem_statement={:?} expected_mentions=0",
            task.problem_statement
        );
        let result = problem_production_candidates(&task, &repo_context, &all_files);
        let arr = result.as_array().expect("result is array");
        let paths: Vec<&str> = arr.iter().map(|v| v.as_str().expect("string")).collect();
        eprintln!("AFTER_NO_PY paths={:?}", paths);
        assert_eq!(
            paths,
            vec!["src/foo.py", "src/bar.py"],
            "with no issue mentions, repo_context order is preserved"
        );
    }

    #[test]
    fn problem_production_candidates_skips_phantom_mention_edge() {
        // Edge case 2: issue text mentions a path that doesn't exist in the repo
        // (typo, refactored away). all_file_set filtering MUST reject it. No error,
        // no panic — the phantom mention is silently absent from candidate_files.
        let task = TaskManifest {
            dataset: None,
            instance_id: "test_phantom_mention".to_string(),
            repo: "x/y".to_string(),
            base_commit: "deadbeef".to_string(),
            problem_statement:
                "Bug in src/nonexistent_module.py with the foo function.".to_string(),
            fail_to_pass: vec![],
            pass_to_pass_count: None,
            official_test_patch_sha256: None,
            oracle_patch_sha256: None,
            source_of_truth: None,
        };
        let all_files = vec!["src/real_module.py".to_string()];
        let repo_context = json!({
            "files": [{"path": "src/real_module.py"}]
        });
        eprintln!(
            "BEFORE_PHANTOM mention=src/nonexistent_module.py all_files={:?}",
            all_files
        );
        let result = problem_production_candidates(&task, &repo_context, &all_files);
        let arr = result.as_array().expect("result is array");
        let paths: Vec<&str> = arr.iter().map(|v| v.as_str().expect("string")).collect();
        eprintln!("AFTER_PHANTOM paths={:?}", paths);
        assert!(
            !paths.contains(&"src/nonexistent_module.py"),
            "phantom mention must not enter candidate_files; got {:?}",
            paths
        );
        assert_eq!(paths, vec!["src/real_module.py"]);
    }

    #[test]
    fn problem_production_candidates_preserves_multi_mention_order_edge() {
        // Edge case 3: issue text mentions multiple production files; first-mentioned
        // wins index 0 even when alphabetically later than other mentions.
        let task = TaskManifest {
            dataset: None,
            instance_id: "test_multi_mention".to_string(),
            repo: "x/y".to_string(),
            base_commit: "deadbeef".to_string(),
            problem_statement:
                "The bug is in pkg/zeta.py and also affects pkg/alpha.py downstream."
                    .to_string(),
            fail_to_pass: vec![],
            pass_to_pass_count: None,
            official_test_patch_sha256: None,
            oracle_patch_sha256: None,
            source_of_truth: None,
        };
        let all_files = vec![
            "pkg/alpha.py".to_string(),
            "pkg/beta.py".to_string(),
            "pkg/zeta.py".to_string(),
        ];
        let repo_context = json!({
            "files": [
                {"path": "pkg/alpha.py"},
                {"path": "pkg/beta.py"},
                {"path": "pkg/zeta.py"}
            ]
        });
        eprintln!(
            "BEFORE_MULTI mention_order=[zeta.py, alpha.py] repo_context_order=[alpha.py, beta.py, zeta.py]"
        );
        let result = problem_production_candidates(&task, &repo_context, &all_files);
        let arr = result.as_array().expect("result is array");
        let paths: Vec<&str> = arr.iter().map(|v| v.as_str().expect("string")).collect();
        eprintln!("AFTER_MULTI paths={:?}", paths);
        assert_eq!(paths[0], "pkg/zeta.py", "first-mentioned file at index 0");
        assert_eq!(paths[1], "pkg/alpha.py", "second-mentioned file at index 1");
        assert_eq!(
            paths[2], "pkg/beta.py",
            "non-mentioned file from repo_context appears last"
        );
    }

    #[test]
    fn compact_problem_reality_packet_surfaces_issue_mentioned_paths_fsv() {
        // FSV: the compact packet (what reaches Gemma's prompt) MUST surface the
        // issue-text-mentioned paths so the model sees the explicit precision signal.
        let packet = json!({
            "record_kind": "contextgraph_swe_problem_reality_packet",
            "task_identity": {"instance_id": "psf__requests-2317"},
            "oracle_reality": {"fail_to_pass_total": 1},
            "problem_statement_reality": {
                "terms": ["sessions", "unicode"],
                "symbols": ["test_unicode_method_name"],
                "mentioned_paths": ["requests/sessions.py", "test_requests.py"]
            },
            "repository_reality": {
                "production_candidate_files": ["requests/sessions.py"],
                "selected_context_files": []
            },
            "root_cause_discovery_protocol": {},
            "measurement_boundary": {},
            "source_of_truth": {}
        });
        eprintln!(
            "BEFORE_COMPACT mentioned_paths_in_packet={:?}",
            packet.pointer("/problem_statement_reality/mentioned_paths")
        );
        let compact = compact_problem_reality_packet(&packet);
        eprintln!(
            "AFTER_COMPACT issue_mentioned_paths={:?}",
            compact.get("issue_mentioned_paths")
        );
        let mentioned: Vec<&str> = compact
            .get("issue_mentioned_paths")
            .and_then(|v| v.as_array())
            .expect("issue_mentioned_paths must be present in compact packet")
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(
            mentioned,
            vec!["requests/sessions.py", "test_requests.py"],
            "compact packet must surface mentioned_paths to Gemma's prompt"
        );
    }
}
