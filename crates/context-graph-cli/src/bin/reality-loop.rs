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
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::{Read, Write};
use std::num::NonZeroU32;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_RUNTIME_ROOT: &str = "/tmp/contextgraph-rust-reality-loop";
const DEFAULT_CONTEXT_GRAPH_BIN: &str = "target/debug/context-graph";
const DEFAULT_MODEL: &str = "qwen3-coder:30b-a3b";
const DEFAULT_INNER_MODEL_PATH: &str =
    "models/qwen3-coder-30b-a3b/Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf";
const DEFAULT_INNER_MODEL_PIN_PATH: &str = "config/ccreality_model_pins.json";
const DEFAULT_INNER_MODEL_PIN_KEY: &str = "qwen3_coder_30b_a3b_q4_k_xl";
/// Minimum free GPU VRAM that must remain *during* a per-chat call, after the
/// inner-actor model is already resident. Sized for steady-state operation: model
/// weights + KV cache are already allocated, so we only need enough free VRAM for
/// CUDA driver overhead and bursty allocations during attention. Tightened from
/// 8192 → 1536 in `eb8a0d10` because the qwen3.6-27b GGUF left only ~1.5GB free
/// once loaded with the full 256K context. The Qwen3-Coder-30B-A3B-Q4 swap
/// (2026-05-08) reduced model footprint to ~17.67 GB so this stays adequate.
const DEFAULT_GPU_MIN_FREE_MB: u64 = 1536;
/// Minimum free GPU VRAM required *before* the inner-actor model is loaded. The
/// loop_start preflight uses this. Sized for the model file (~17.67 GB
/// Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL) plus KV cache headroom. Without this
/// separate threshold, a shared `--gpu-min-free-mb` of 1536 would let preflight
/// pass on a machine with only 1.5 GB free, then the model load would CUDA-OOM
/// mid-load. Setting to 18432 (18 GB) means at most ~14 GB can be held by other
/// GPU consumers when the loop starts. (Lowered from 25600 on 2026-05-08 swap.)
const DEFAULT_GPU_PRELOAD_MIN_FREE_MB: u64 = 18432;
const DEFAULT_INNER_MIN_CONTEXT_TOKENS: u32 = 128_000;
const DEFAULT_INNER_MAX_NEW_TOKENS: usize = 8192;
const DEFAULT_INNER_TEMPERATURE: f32 = 0.0;
const DEFAULT_INNER_TOP_P: f32 = 0.9;
const DEFAULT_INNER_BATCH_SIZE: usize = 2048;
const DEFAULT_INNER_SEED: u32 = 1;
const DEFAULT_AGENT_TOOL_ROUNDS: u64 = 24;
/// Hard cap on rounds-per-attempt enforced inside `inner_coding_harness_chat_json`,
/// independent of `--agent-tool-rounds`. Chosen to cover the longest known successful
/// trajectory (~16 rounds on psf__requests-2317) with headroom while preventing the
/// exponential-escape and context-overflow cascades observed past ~15 rounds. Bump
/// this constant — do not loosen the deadlock detector — if a genuinely complex task
/// proves to need more.
const MAX_ROUNDS_PER_ATTEMPT_HARD_CAP: u64 = 20;
/// Number of consecutive rounds that must end with the same non-success
/// `error_code` before the harness force-fails the attempt with
/// `RUST_REALITY_REPEATED_ERROR_DEADLOCK`. Five matches the operator's stuck-loop
/// guidance and gives the model enough rope to self-correct once or twice first.
const REPEATED_ERROR_DEADLOCK_LEN: usize = 5;
const DEFAULT_OFFICIAL_DOCKER_HOST: &str = "unix:///var/run/docker.sock";
const DEFAULT_OFFICIAL_DOCKER_CONFIG: &str = "/home/cabdru/.cache/contextgraph-docker-config";
const DEFAULT_SMOKE_RUNTIME_ROOT: &str = "/tmp/cgreality-smoke";
const DEFAULT_REPLAY_OUTPUT_LABEL: &str = "replay";
const DATASET_NAME: &str = "princeton-nlp/SWE-bench_Lite";
const LOCAL_PYTHON_BIN: &str = "python3";
const DYNAMICJEPA_REALITY_DOMAIN_PACK: &str = "contextgraph_swe_reality_loop_v1";
const DEFAULT_DYNAMICJEPA_SKILL_ID: &str = "d91e5e0c-3557-57a9-8b48-0fa2acb429a0";
#[derive(Parser, Debug, Clone)]
#[command(name = "reality-loop")]
#[command(version)]
#[command(
    about = "ContextGraph reality engine: runs the inner model SWE-bench attempts and synthetic smokes; writes durable artifacts."
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
    /// Run one or more the inner model SWE-bench repair attempts. Writes attempt artifacts.
    Attempt(AttemptArgs),
    /// Run a synthetic smoke attempt. No the inner model, no Docker. For wiring verification.
    Smoke(SmokeArgs),
    /// Re-run official SWE-bench Docker evaluation against an existing attempt patch.
    OfficialEval(OfficialEvalArgs),
    /// Apply an existing attempt patch and run one exact fail_to_pass pytest target.
    TargetedTest(TargetedTestArgs),
    /// Replay an existing attempt's reality-signal-packet builder against current code.
    Replay(ReplayArgs),
    /// Full-state-verify every JSON artifact under a runtime root.
    Verify(VerifyArgs),
    /// Governed harness operations (open file windows, apply edits, run commands, verify state).
    /// Designed to be invoked as a fresh subprocess per call so the on-disk binary is always
    /// the source of truth — the MCP server shells out to these to avoid stale-link staleness.
    Harness(HarnessArgs),
}

#[derive(Parser, Debug, Clone)]
struct HarnessArgs {
    #[command(subcommand)]
    cmd: HarnessCmd,
}

#[derive(Subcommand, Debug, Clone)]
enum HarnessCmd {
    /// Read a window of lines from a governed file. Emits JSON envelope on stdout.
    /// All inputs via argv; no stdin payload.
    OpenWindow(HarnessOpenWindowArgs),
    /// Apply line-window edits to a governed file. Edit payload (single edit object) on
    /// stdin as JSON: {"path","start_line","end_line","observed_sha256","replace"}.
    /// Stdin avoids argv length and quoting hazards on multi-line replace text.
    ApplyLineWindowEdit(HarnessApplyLineWindowEditArgs),
    /// Run an allowlisted shell command via the governed runner. Command string on stdin
    /// as JSON: {"command": "..."}. Uses stdin to avoid argv quoting hazards.
    RunCommand(HarnessRunCommandArgs),
    /// Verify project state (cargo fmt + check + targeted test). All inputs via argv.
    VerifyState(HarnessVerifyStateArgs),
}

#[derive(Parser, Debug, Clone)]
struct HarnessOpenWindowArgs {
    #[arg(long)]
    path: String,
    #[arg(long)]
    start_line: u64,
    #[arg(long)]
    end_line: u64,
}

#[derive(Parser, Debug, Clone)]
struct HarnessApplyLineWindowEditArgs {
    /// Where to write the diff/transition artifacts (attempt-relative).
    #[arg(long)]
    tool_dir: PathBuf,
    /// Per-edit cargo wallclock cap.
    #[arg(long, default_value_t = 120)]
    timeout_secs: u64,
}

#[derive(Parser, Debug, Clone)]
struct HarnessRunCommandArgs {
    /// Where to write the command output artifact (attempt-relative).
    #[arg(long)]
    tool_dir: PathBuf,
    /// Wallclock cap for the command.
    #[arg(long, default_value_t = 120)]
    timeout_secs: u64,
}

#[derive(Parser, Debug, Clone)]
struct HarnessVerifyStateArgs {
    /// Where to write the verification artifact (attempt-relative).
    #[arg(long)]
    tool_dir: PathBuf,
    /// Either "reality_loop_only" (default) or "full".
    #[arg(long, default_value = "reality_loop_only")]
    scope: String,
    /// Caller-supplied label embedded in the verification record.
    #[arg(long, default_value = "cli")]
    label: String,
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
#[command(about = "Run one or more the inner model SWE-bench repair attempts")]
struct AttemptArgs {
    /// Stable model label used in artifacts and BlackBox ledger rows.
    #[arg(long, default_value = DEFAULT_MODEL)]
    model: String,
    /// Real GGUF model file loaded by the Rust the inner model coding harness.
    #[arg(long, default_value = DEFAULT_INNER_MODEL_PATH)]
    inner_model_path: PathBuf,
    /// Optional explicit llama.cpp chat template name or template text.
    #[arg(long)]
    inner_chat_template: Option<String>,
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
    /// Minimum free GPU VRAM required at loop_start, before any model bytes hit the GPU.
    /// Distinct from `--gpu-min-free-mb` (which gates per-chat checks after load).
    /// Defaults to model_size + KV-cache + safety. Lower this only if you have a smaller
    /// model and know what you're doing.
    #[arg(long, default_value_t = DEFAULT_GPU_PRELOAD_MIN_FREE_MB)]
    gpu_preload_min_free_mb: u64,
    #[arg(long)]
    gpu_max_used_mb: Option<u64>,
    /// Context window tokens. Omit to use the loaded GGUF model's full training context.
    #[arg(long)]
    inner_context_tokens: Option<u32>,
    #[arg(long, default_value_t = DEFAULT_INNER_MAX_NEW_TOKENS)]
    inner_max_new_tokens: usize,
    #[arg(long, default_value_t = DEFAULT_INNER_TEMPERATURE)]
    inner_temperature: f32,
    #[arg(long, default_value_t = DEFAULT_INNER_TOP_P)]
    inner_top_p: f32,
    #[arg(long, default_value_t = u32::MAX)]
    inner_gpu_layers: u32,
    #[arg(long, default_value_t = DEFAULT_INNER_BATCH_SIZE)]
    inner_batch_size: usize,
    #[arg(long, default_value_t = DEFAULT_INNER_SEED)]
    inner_seed: u32,
    /// Path to the model-pin manifest (sha256/size/identity-substring expectations).
    #[arg(long, default_value = DEFAULT_INNER_MODEL_PIN_PATH)]
    inner_model_pin_path: PathBuf,
    /// Active model-pin entry key (looked up under .models in the pin manifest).
    #[arg(long, default_value = DEFAULT_INNER_MODEL_PIN_KEY)]
    inner_model_pin_key: String,
    #[arg(long, default_value_t = 600)]
    git_timeout: u64,
    #[arg(long, default_value_t = DEFAULT_AGENT_TOOL_ROUNDS)]
    agent_tool_rounds: u64,
    #[arg(long, default_value_t = 300)]
    targeted_test_timeout: u64,
    #[arg(long, default_value_t = 0)]
    sleep_between_attempts_ms: u64,
    /// DynamicJEPA RocksDB path used to ingest attempt panels and run artifact-backed inference.
    /// Must be supplied with --dynamicjepa-artifact-id when trained DJ reward features are required.
    #[arg(long)]
    dynamicjepa_db: Option<PathBuf>,
    /// DynamicJEPA model artifact id trained for contextgraph_swe_reality_loop_v1.
    /// Must be supplied with --dynamicjepa-db; the reality loop verifies artifact files before inference.
    #[arg(long)]
    dynamicjepa_artifact_id: Option<String>,
    /// Skill id used when persisting DynamicJEPA plan traces for SWE-loop attempts.
    #[arg(long, default_value = DEFAULT_DYNAMICJEPA_SKILL_ID)]
    dynamicjepa_skill_id: String,
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
struct GpuComputeProcess {
    gpu_uuid: String,
    pid: u64,
    process_name: String,
    used_memory_mb: u64,
    procfs: GpuProcfsSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GpuProcfsSnapshot {
    status: String,
    uid: Option<u64>,
    comm: Option<String>,
    cmdline: Option<String>,
    exe: Option<String>,
    errors: Vec<String>,
    source_of_truth: Vec<String>,
}

#[derive(Debug, Clone)]
struct GpuComputeProcessRow {
    gpu_uuid: String,
    pid: u64,
    process_name: String,
    used_memory_mb: u64,
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
    compute_process_count: usize,
    compute_processes: Vec<GpuComputeProcess>,
    compute_processes_raw: String,
    source_of_truth: String,
    read_at_unix: u64,
}

#[derive(Debug, Clone, Serialize)]
struct AttemptRecord {
    attempt: u64,
    task_id: String,
    task_summary: String,
    input_excerpt: Option<String>,
    output_excerpt: Option<String>,
    critique: Option<String>,
    reward: f64,
    success: bool,
    outcome_kind: String,
    latency_ms: Option<u64>,
    tokens_used: Option<u64>,
    tags: Vec<String>,
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
    dynamicjepa_initial_panel_id: Option<String>,
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
        Cmd::Harness(args) => dispatch_harness(args).await,
    };
    std::process::exit(exit_code);
}

async fn dispatch_harness(args: HarnessArgs) -> i32 {
    match args.cmd {
        HarnessCmd::OpenWindow(a) => harness_dispatch(harness_open_window(a)).await,
        HarnessCmd::ApplyLineWindowEdit(a) => {
            harness_dispatch(harness_apply_line_window_edit(a)).await
        }
        HarnessCmd::RunCommand(a) => harness_dispatch(harness_run_command(a)).await,
        HarnessCmd::VerifyState(a) => harness_dispatch(harness_verify_state(a)).await,
    }
}

async fn harness_dispatch<F>(future: F) -> i32
where
    F: std::future::Future<Output = std::result::Result<Value, Value>>,
{
    match future.await {
        Ok(payload) => {
            println!("{}", serde_json::to_string_pretty(&payload).unwrap());
            0
        }
        Err(err_payload) => {
            // Tool-level error — JSON envelope on STDOUT (machine-parseable),
            // exit code 1 (per LSP/JSON-RPC convention: tool ran, returned an error).
            // Reserve exit 2 for invocation errors (clap parse failure already exits 2)
            // and exit 64+ for infrastructure crashes (panics, OOM).
            println!("{}", serde_json::to_string_pretty(&err_payload).unwrap());
            1
        }
    }
}

async fn harness_open_window(args: HarnessOpenWindowArgs) -> std::result::Result<Value, Value> {
    let root = context_graph_cli::governed_edit::project_root().map_err(governed_error_to_json)?;
    context_graph_cli::governed_edit::project_file_window(
        &root,
        &args.path,
        args.start_line,
        args.end_line,
    )
    .map_err(governed_error_to_json)
}

async fn harness_apply_line_window_edit(
    args: HarnessApplyLineWindowEditArgs,
) -> std::result::Result<Value, Value> {
    let edit = read_stdin_json()?;
    let root = context_graph_cli::governed_edit::project_root().map_err(governed_error_to_json)?;
    context_graph_cli::governed_edit::project_apply_line_window_edits(
        &root,
        &args.tool_dir,
        &[edit],
        args.timeout_secs,
    )
    .map_err(governed_error_to_json)
}

async fn harness_run_command(args: HarnessRunCommandArgs) -> std::result::Result<Value, Value> {
    let payload = read_stdin_json()?;
    let command = payload
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            json!({
                "status": "error",
                "error_code": "RUST_REALITY_HARNESS_RUN_COMMAND_FIELD_MISSING",
                "message": "stdin JSON must contain a 'command' string field",
                "field_path": "stdin.command",
                "remediation": "pipe {\"command\": \"<allowlisted shell command>\"} on stdin",
                "details": payload.clone(),
                "source_of_truth": null
            })
        })?;
    let root = context_graph_cli::governed_edit::project_root().map_err(governed_error_to_json)?;
    context_graph_cli::governed_edit::run_governed_command(
        &root,
        &args.tool_dir,
        command,
        args.timeout_secs,
    )
    .map_err(governed_error_to_json)
}

async fn harness_verify_state(args: HarnessVerifyStateArgs) -> std::result::Result<Value, Value> {
    let scope = match args.scope.as_str() {
        "full" => context_graph_cli::governed_edit::VerifyScope::Full,
        "reality_loop_only" => context_graph_cli::governed_edit::VerifyScope::RealityLoopOnly,
        other => {
            return Err(json!({
                "status": "error",
                "error_code": "RUST_REALITY_HARNESS_VERIFY_SCOPE_INVALID",
                "message": format!("scope must be 'reality_loop_only' or 'full', got '{other}'"),
                "field_path": "args.scope",
                "remediation": "pass --scope reality_loop_only or --scope full",
                "details": json!({"scope": other}),
                "source_of_truth": null
            }));
        }
    };
    let root = context_graph_cli::governed_edit::project_root().map_err(governed_error_to_json)?;
    context_graph_cli::governed_edit::verify_project_state(
        &root,
        &args.tool_dir,
        &args.label,
        scope,
    )
    .map_err(governed_error_to_json)
}

fn read_stdin_json() -> std::result::Result<Value, Value> {
    use std::io::Read;
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf).map_err(|e| {
        json!({
            "status": "error",
            "error_code": "RUST_REALITY_HARNESS_STDIN_READ_FAILED",
            "message": format!("failed to read stdin: {e}"),
            "field_path": "stdin",
            "remediation": "ensure caller writes the JSON payload then closes stdin",
            "details": json!({"io_error": e.to_string()}),
            "source_of_truth": null
        })
    })?;
    if buf.trim().is_empty() {
        return Err(json!({
            "status": "error",
            "error_code": "RUST_REALITY_HARNESS_STDIN_EMPTY",
            "message": "stdin was empty; expected a JSON payload",
            "field_path": "stdin",
            "remediation": "write the JSON payload to the child's stdin before closing it",
            "details": json!({}),
            "source_of_truth": null
        }));
    }
    serde_json::from_str(&buf).map_err(|e| {
        json!({
            "status": "error",
            "error_code": "RUST_REALITY_HARNESS_STDIN_JSON_INVALID",
            "message": format!("stdin was not valid JSON: {e}"),
            "field_path": "stdin",
            "remediation": "send a single JSON document on stdin",
            "details": json!({"stdin_preview": tail_str(&buf, 1024), "json_error": e.to_string()}),
            "source_of_truth": null
        })
    })
}

fn governed_error_to_json(err: context_graph_cli::governed_edit::GovernedEditError) -> Value {
    json!({
        "status": "error",
        "error_code": err.error_code,
        "message": err.message,
        "field_path": err.field_path,
        "remediation": err.remediation,
        "details": err.details,
        "source_of_truth": err.source_of_truth
    })
}

fn tail_str(s: &str, n: usize) -> String {
    let len = s.chars().count();
    if len <= n {
        s.to_string()
    } else {
        s.chars().skip(len - n).collect()
    }
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
        "task_summary": format!("synthetic smoke {}", args.label),
        "input_excerpt": "deterministic smoke trigger",
        "output_excerpt": "deterministic smoke artifacts written",
        "critique": "smoke happy path passed; artifacts were written and can be replayed",
        "reward": 0.5,
        "success": true,
        "outcome_kind": "smoke_passed",
        "latency_ms": null,
        "tokens_used": null,
        "tags": ["smoke", "validation-passed"],
        "synthetic": true,
        "validation": { "status": "passed", "synthetic": true },
        "official_evidence": null,
        "reality_signal_packet_path": path_str(&signal_path),
        "problem_reality_packet_path": path_str(&problem_path),
        "reflexion_episode": {
            "task_id": task_id,
            "task_summary": format!("synthetic smoke {}", args.label),
            "input_excerpt": "deterministic smoke trigger",
            "output_excerpt": "deterministic smoke artifacts written",
            "critique": "smoke happy path passed; artifacts were written and can be replayed",
            "reward": 0.5,
            "success": true,
            "outcome_kind": "smoke_passed",
            "latency_ms": null,
            "tokens_used": null,
            "tags": ["smoke", "validation-passed"]
        },
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
        inner_model_path: PathBuf::from(DEFAULT_INNER_MODEL_PATH),
        inner_chat_template: None,
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
        gpu_preload_min_free_mb: DEFAULT_GPU_PRELOAD_MIN_FREE_MB,
        gpu_max_used_mb: None,
        inner_context_tokens: None,
        inner_max_new_tokens: DEFAULT_INNER_MAX_NEW_TOKENS,
        inner_temperature: DEFAULT_INNER_TEMPERATURE,
        inner_top_p: DEFAULT_INNER_TOP_P,
        inner_gpu_layers: u32::MAX,
        inner_batch_size: DEFAULT_INNER_BATCH_SIZE,
        inner_seed: DEFAULT_INNER_SEED,
        inner_model_pin_path: PathBuf::from(DEFAULT_INNER_MODEL_PIN_PATH),
        inner_model_pin_key: DEFAULT_INNER_MODEL_PIN_KEY.to_string(),
        git_timeout: 600,
        agent_tool_rounds: DEFAULT_AGENT_TOOL_ROUNDS,
        targeted_test_timeout: 300,
        sleep_between_attempts_ms: 0,
        dynamicjepa_db: None,
        dynamicjepa_artifact_id: None,
        dynamicjepa_skill_id: DEFAULT_DYNAMICJEPA_SKILL_ID.to_string(),
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

fn validate_inner_harness_args(args: &AttemptArgs) -> Result<()> {
    if args.inner_context_tokens == Some(0) {
        return Err(RealityError::new(
            "RUST_REALITY_INNER_CONTEXT_TOKENS_INVALID",
            "--inner-context-tokens must be greater than zero",
            "inner_context_tokens",
            "omit --inner-context-tokens to use the GGUF training context, or set a positive explicit context window",
            json!({"inner_context_tokens": args.inner_context_tokens}),
            None,
        ));
    }
    if args.inner_max_new_tokens == 0 {
        return Err(RealityError::new(
            "RUST_REALITY_INNER_MAX_NEW_TOKENS_INVALID",
            "--inner-max-new-tokens must be greater than zero",
            "inner_max_new_tokens",
            "set a positive generation budget so the harness can emit a tool call",
            json!({"inner_max_new_tokens": args.inner_max_new_tokens}),
            None,
        ));
    }
    if args.inner_batch_size == 0 {
        return Err(RealityError::new(
            "RUST_REALITY_INNER_BATCH_SIZE_INVALID",
            "--inner-batch-size must be greater than zero",
            "inner_batch_size",
            "set a positive llama.cpp prompt batch size",
            json!({"inner_batch_size": args.inner_batch_size}),
            None,
        ));
    }
    if args.inner_batch_size > u32::MAX as usize {
        return Err(RealityError::new(
            "RUST_REALITY_INNER_BATCH_SIZE_TOO_LARGE",
            "--inner-batch-size exceeds llama.cpp u32 limits",
            "inner_batch_size",
            "set a positive batch size no larger than u32::MAX",
            json!({"inner_batch_size": args.inner_batch_size, "max": u32::MAX}),
            None,
        ));
    }
    if !args.inner_temperature.is_finite() || !(0.0..=2.0).contains(&args.inner_temperature) {
        return Err(RealityError::new(
            "RUST_REALITY_INNER_TEMPERATURE_INVALID",
            "--inner-temperature must be a finite value between 0.0 and 2.0",
            "inner_temperature",
            "use deterministic 0.0 for SWE repair unless intentionally experimenting",
            json!({"inner_temperature": args.inner_temperature}),
            None,
        ));
    }
    if !args.inner_top_p.is_finite() || args.inner_top_p <= 0.0 || args.inner_top_p > 1.0 {
        return Err(RealityError::new(
            "RUST_REALITY_INNER_TOP_P_INVALID",
            "--inner-top-p must be a finite value in (0.0, 1.0]",
            "inner_top_p",
            "use 0.9 for bounded sampling or 1.0 to disable nucleus pruning",
            json!({"inner_top_p": args.inner_top_p}),
            None,
        ));
    }
    match (&args.dynamicjepa_db, &args.dynamicjepa_artifact_id) {
        (Some(db), Some(artifact_id)) => {
            if artifact_id.trim().is_empty() {
                return Err(RealityError::new(
                    "RUST_REALITY_DYNAMICJEPA_ARTIFACT_ID_INVALID",
                    "--dynamicjepa-artifact-id must not be empty",
                    "dynamicjepa_artifact_id",
                    "pass the model_artifact_id produced by `context-graph dynamicjepa train`",
                    json!({"dynamicjepa_db": path_str(db), "dynamicjepa_artifact_id": artifact_id}),
                    Some(file_sot(db)),
                ));
            }
        }
        (None, None) => {}
        _ => {
            return Err(RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_CONFIG_INCOMPLETE",
                "--dynamicjepa-db and --dynamicjepa-artifact-id must be supplied together",
                "dynamicjepa",
                "register contextgraph_swe_reality_loop_v1, train a DynamicJEPA artifact, then pass both values or omit both to emit projection-only evidence",
                json!({
                    "dynamicjepa_db": args.dynamicjepa_db.as_ref().map(|p| path_str(p)),
                    "dynamicjepa_artifact_id": args.dynamicjepa_artifact_id.clone(),
                }),
                args.dynamicjepa_db.as_ref().map(|p| file_sot(p)),
            ));
        }
    }
    Ok(())
}

async fn run(args: AttemptArgs) -> Result<Value> {
    let runtime_root = resolve_path(&args.runtime_root)?;
    create_dir_checked(&runtime_root)?;
    // GC: garbage-collect aged runs, workspace clones, shift-logs, and oversized
    // hook-events at every attempt start. Best-effort; never blocks the attempt.
    // Report is overwritten at <runtime_root>/.gc-last-report.json so operators
    // can du-track effectiveness without per-run accumulation.
    {
        let gc_report = gc_runtime_root(&runtime_root);
        let report_path = runtime_root.join(".gc-last-report.json");
        let _ = fs::write(
            &report_path,
            serde_json::to_string_pretty(&gc_report)
                .unwrap_or_else(|_| "{}".to_string())
                .as_bytes(),
        );
        if gc_report.runs_deleted > 0
            || gc_report.workspaces_deleted > 0
            || gc_report.shift_logs_deleted > 0
            || gc_report.hook_events_truncated
        {
            eprintln!(
                "[reality-loop] gc: runs={} ({} MB), workspaces={} ({} MB), shift_logs={} ({} MB), hook_events_truncated={}",
                gc_report.runs_deleted,
                gc_report.runs_bytes_freed / (1024 * 1024),
                gc_report.workspaces_deleted,
                gc_report.workspaces_bytes_freed / (1024 * 1024),
                gc_report.shift_logs_deleted,
                gc_report.shift_logs_bytes_freed / (1024 * 1024),
                gc_report.hook_events_truncated,
            );
        }
    }
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
    validate_inner_harness_args(&args)?;
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
        args.gpu_preload_min_free_mb,
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
    let inner_harness = InnerCodingHarness::load(&args)?;
    let inner_model_readback_path = run_root.join("inner-model-readback.json");
    let inner_model_readback = inner_harness.model_capability_report(&inner_model_readback_path)?;
    write_json_checked(&inner_model_readback_path, &inner_model_readback)?;
    append_event(
        &reality_events,
        json!({
            "event_type": "inner_harness_model_capability_readback",
            "status": "ok",
            "model": args.model,
            "model_path": file_sot(&inner_harness.model_path),
            "effective_context_tokens": inner_harness.context_tokens,
            "max_new_tokens": inner_harness.max_new_tokens,
            "source_of_truth": file_sot(&inner_model_readback_path),
            "sha256": sha256_file(&inner_model_readback_path)?
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

        let mut tool_context = AgentToolContext {
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
            dynamicjepa_initial_panel_id: None,
        };
        tool_context.dynamicjepa_initial_panel_id =
            bootstrap_dynamicjepa_cold_start_panel(&tool_context, attempt)?;
        let model_response =
            inner_coding_harness_chat_json(&inner_harness, &tool_context, &prompt, attempt)?;
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

        let agent_tool_transcript_path = model_response
            .pointer("/contextgraph_runtime/agent_tool_transcript")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let dynamicjepa_reality = build_dynamicjepa_reality_for_attempt(
            &run_id,
            attempt,
            agent_tool_transcript_path.as_deref(),
            &ledger,
            &attempt_dir,
            &workspace,
            &context_graph_bin,
            &task_root,
            args.dynamicjepa_db.as_deref(),
            args.dynamicjepa_artifact_id.as_deref(),
            &args.dynamicjepa_skill_id,
        )?;
        let dynamicjepa_reality_path = attempt_dir.join("dynamicjepa-reality.json");
        write_json_checked(&dynamicjepa_reality_path, &dynamicjepa_reality)?;
        let dynamicjepa_reality_record = blackbox(
            &context_graph_bin,
            &[
                "blackbox",
                "record",
                "dynamicjepa-reality",
                "--ledger",
                path_str(&ledger),
                "--run-id",
                &run_id,
                "--input",
                path_str(&dynamicjepa_reality_path),
                "--agent",
                "contextgraph_rust_reality_loop",
                "--model",
                &args.model,
            ],
            &task_root,
        )?;

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
        let reward_spec = resolve_path(Path::new("config/reward_spec.verified_outcome.json"))?;
        if !reward_spec.is_file() {
            return Err(RealityError::new(
                "RUST_REALITY_REWARD_SPEC_MISSING",
                "verified outcome reward spec is required before an attempt can complete",
                "config/reward_spec.verified_outcome.json",
                "restore the reward spec or pass a build containing it; attempts must not complete without reward compilation",
                json!({"reward_spec": path_str(&reward_spec)}),
                Some(file_sot(&reward_spec)),
            ));
        }
        let reward = Some(blackbox(
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
        )?);
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
            &dynamicjepa_reality,
            &dynamicjepa_reality_record,
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

        let reflexion_reward = normalized_attempt_reward(reward.as_ref(), resolved);
        let reflexion_success = attempt_success(resolved, &validation);
        let reflexion_outcome_kind =
            attempt_outcome_kind(&validation, resolved, repeated_patch, patch_sha.as_deref());
        let reflexion_tags =
            attempt_tags(&validation, resolved, repeated_patch, patch_sha.as_deref());
        let reflexion_critique =
            attempt_critique(&validation, resolved, repeated_patch, patch_sha.as_deref());
        let attempt_record = AttemptRecord {
            attempt,
            task_id: task.instance_id.clone(),
            task_summary: task_summary_for_reflexion(&task),
            input_excerpt: Some(truncate_chars(&task.problem_statement, 1200)),
            output_excerpt: model_output_excerpt(&model_response),
            critique: Some(reflexion_critique),
            reward: reflexion_reward,
            success: reflexion_success,
            outcome_kind: reflexion_outcome_kind,
            latency_ms: None,
            tokens_used: None,
            tags: reflexion_tags,
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
                "dynamicjepa_reality": dynamicjepa_reality,
                "dynamicjepa_reality_record": dynamicjepa_reality_record,
                "score": score,
                "reward": reward,
                "reflexion_episode": attempt_record_reflexion_episode(&attempt_record),
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
                "provider": "contextgraph_rust_inner_coding_harness",
                "model_label": args.model,
                "inner_model_path": file_sot(&resolve_path(&args.inner_model_path)?),
                "chat_template": args.inner_chat_template.as_deref(),
                "context_policy": if args.inner_context_tokens.is_some() { "explicit_cli" } else { "gguf_n_ctx_train_max" },
                "requested_context_tokens": args.inner_context_tokens,
                "minimum_context_tokens": DEFAULT_INNER_MIN_CONTEXT_TOKENS,
                "max_new_tokens": args.inner_max_new_tokens,
                "temperature": args.inner_temperature,
                "top_p": args.inner_top_p,
                "gpu_layers": args.inner_gpu_layers,
                "batch_size": args.inner_batch_size,
                "seed": args.inner_seed
            },
            "reward_spec": file_sot(Path::new("config/reward_spec.verified_outcome.json"))
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

/// CONTEXT-BUDGET: maximum size in bytes for a single tool result that gets
/// embedded in the conversation history sent to the inner model. Tool results
/// accumulate across rounds (each round's prompt = system + problem packet +
/// all prior assistant messages + all prior tool results), so an unbounded
/// 60 KB read_problem_reality output stacked across 24 rounds = ~1.4 MB of
/// pure tool-result text in the round-24 prompt — enough to push past Qwen's
/// 256K context AND make every round's prefill slow because we mint a fresh
/// LlamaContext per chat call (full re-prefill of growing prompt).
///
/// 4 KB ≈ 1000 tokens is small enough to stack 24 tool results inside a
/// 32K-token budget with room for the system prompt + tool definitions + any
/// active assistant reasoning. Big enough to convey an open_file_window of
/// ~60 source lines, a one-paragraph issue summary, or a directory listing
/// of ~50 files. Bigger results get truncated with an explicit marker telling
/// the model how to recover (call again with a narrower scope). The full
/// result is always persisted to <tool_dir>/tool-result.json regardless of
/// truncation. Lowered from 16 KB on 2026-05-08 (turn 79) after empirical
/// observation that round-3 prompts already hit 40K tokens with the prior cap;
/// per-round prefill was ~10 min for a 72-token response.
const TOOL_RESULT_HISTORY_CAP_BYTES: usize = 4 * 1024;

fn truncate_tool_result_for_history(text: &str) -> String {
    if text.len() <= TOOL_RESULT_HISTORY_CAP_BYTES {
        return text.to_string();
    }
    // Find a UTF-8 char boundary at or before the cap so we never produce
    // invalid UTF-8 (would break JSON serialization downstream).
    let mut cut = TOOL_RESULT_HISTORY_CAP_BYTES;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    let kept = &text[..cut];
    let elided = text.len() - cut;
    format!(
        "{kept}\n[CONTEXT-BUDGET-TRUNCATED: {elided} bytes elided from this tool result. \
The full untruncated result is on disk at the source-of-truth path. To see what was elided, \
re-call this tool with a tighter scope: open_file_window with a smaller line range, \
search_repo with a more specific query, read_problem_reality with detail=compact, etc.]"
    )
}

/// GC retention policy constants. These bound on-disk growth of the runtime root
/// without losing recent diagnostic value. Tuned for a /loop running 24/7 on
/// /tmp (typical /tmp = 100-1000 GB, but other tools also consume it).
///
/// - 30 days for runs: long enough to span the operator's typical inspection
///   window after a failure (post-incident review, weekly retros, etc.) without
///   accumulating indefinitely.
/// - 14 days for workspaces: a workspace clone is reproducible from the
///   task manifest's base_commit; aging out and re-cloning is cheap (~1 min).
/// - 7 days for shift-log session files: each /loop session writes one; daily
///   rotation is more than enough for diagnosis.
/// - 100 MB for hook-events: tight cap because this is a single append-only
///   file, no per-session split. Truncated tail-keep to 50 MB on overflow.
/// - 1 hour minimum age: safety bumper. Never delete anything created in the
///   last hour, regardless of nominal retention. Protects against clock-skew
///   misconfiguration deleting the active run.
const GC_RUNS_RETENTION_DAYS: u64 = 30;
const GC_WORKSPACE_RETENTION_DAYS: u64 = 14;
const GC_SHIFT_LOG_RETENTION_DAYS: u64 = 7;
const GC_HOOK_EVENTS_MAX_BYTES: u64 = 100 * 1024 * 1024;
const GC_HOOK_EVENTS_TARGET_AFTER_TRUNCATE_BYTES: u64 = 50 * 1024 * 1024;
const GC_MIN_ENTRY_AGE_SECONDS: u64 = 3600;

/// Operator-visible report from one GC pass. Serialized to
/// `<runtime_root>/.gc-last-report.json` so operators can verify GC ran and
/// see what was freed without re-running the engine.
#[derive(Debug, Default, Serialize)]
struct GcReport {
    runtime_root: String,
    started_at_unix: u64,
    finished_at_unix: u64,
    runs_deleted: usize,
    runs_bytes_freed: u64,
    workspaces_deleted: usize,
    workspaces_bytes_freed: u64,
    shift_logs_deleted: usize,
    shift_logs_bytes_freed: u64,
    hook_events_bytes_before: u64,
    hook_events_bytes_after: u64,
    hook_events_truncated: bool,
    errors: Vec<String>,
    policy: GcPolicySnapshot,
}

#[derive(Debug, Default, Serialize)]
struct GcPolicySnapshot {
    runs_retention_days: u64,
    workspace_retention_days: u64,
    shift_log_retention_days: u64,
    hook_events_max_bytes: u64,
    hook_events_target_after_truncate_bytes: u64,
    min_entry_age_seconds: u64,
}

/// Walk a directory recursively and sum file sizes. Best-effort: any errors
/// (permission denied, broken symlink) are silently skipped. Used to attribute
/// freed bytes per category in the GC report.
fn dir_size_bytes(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() {
            total = total.saturating_add(dir_size_bytes(&entry.path()));
        } else if meta.is_file() {
            total = total.saturating_add(meta.len());
        }
    }
    total
}

/// True iff `path`'s mtime is older than `retention_days` AND older than the
/// safety floor `GC_MIN_ENTRY_AGE_SECONDS`. The double-check protects against
/// (a) clock skew on system boot deleting fresh data and (b) misconfiguration
/// where retention_days might be set to 0.
fn path_older_than(path: &Path, retention_days: u64, now_unix: u64) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    let Ok(mtime) = meta.modified() else {
        return false;
    };
    let Ok(dur) = mtime.duration_since(UNIX_EPOCH) else {
        return false;
    };
    let age = now_unix.saturating_sub(dur.as_secs());
    age >= GC_MIN_ENTRY_AGE_SECONDS && age >= retention_days.saturating_mul(86400)
}

/// Tail-truncate a JSONL file to keep only the last `target_bytes` bytes,
/// aligning the cut to the next newline so partial lines aren't kept. Returns
/// the new file size.
fn truncate_jsonl_to_tail(path: &Path, target_bytes: u64) -> std::result::Result<u64, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    let total = bytes.len() as u64;
    if total <= target_bytes {
        return Ok(total);
    }
    let cut = (total - target_bytes) as usize;
    let aligned_cut = bytes[cut..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|i| cut + i + 1)
        .unwrap_or(cut);
    let kept = &bytes[aligned_cut..];
    fs::write(path, kept).map_err(|e| format!("write {}: {}", path.display(), e))?;
    Ok(kept.len() as u64)
}

/// Run one GC pass over `runtime_root`. Best-effort: every error is recorded
/// in `errors` but never propagated. The function returns the report by value
/// so callers can decide whether to log it, persist it, or both.
fn gc_runtime_root(runtime_root: &Path) -> GcReport {
    let started = unix_secs_for_logging();
    let mut report = GcReport {
        runtime_root: path_str(runtime_root).to_string(),
        started_at_unix: started,
        policy: GcPolicySnapshot {
            runs_retention_days: GC_RUNS_RETENTION_DAYS,
            workspace_retention_days: GC_WORKSPACE_RETENTION_DAYS,
            shift_log_retention_days: GC_SHIFT_LOG_RETENTION_DAYS,
            hook_events_max_bytes: GC_HOOK_EVENTS_MAX_BYTES,
            hook_events_target_after_truncate_bytes: GC_HOOK_EVENTS_TARGET_AFTER_TRUNCATE_BYTES,
            min_entry_age_seconds: GC_MIN_ENTRY_AGE_SECONDS,
        },
        ..Default::default()
    };
    let now_unix = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => {
            report
                .errors
                .push("system time before unix epoch; skipping GC".to_string());
            report.finished_at_unix = unix_secs_for_logging();
            return report;
        }
    };
    // 1. Old run dirs: rust_reality_<id>/
    if let Ok(entries) = fs::read_dir(runtime_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.starts_with("rust_reality_") {
                continue;
            }
            if !path_older_than(&path, GC_RUNS_RETENTION_DAYS, now_unix) {
                continue;
            }
            let size = dir_size_bytes(&path);
            match fs::remove_dir_all(&path) {
                Ok(_) => {
                    report.runs_deleted += 1;
                    report.runs_bytes_freed = report.runs_bytes_freed.saturating_add(size);
                }
                Err(e) => report
                    .errors
                    .push(format!("rm run {}: {}", path.display(), e)),
            }
        }
    }
    // 2. Old workspace clones: workspaces/<instance>/repo/
    let workspaces = runtime_root.join("workspaces");
    if let Ok(entries) = fs::read_dir(&workspaces) {
        for entry in entries.flatten() {
            let path = entry.path();
            let repo = path.join("repo");
            if !repo.exists() {
                continue;
            }
            if !path_older_than(&repo, GC_WORKSPACE_RETENTION_DAYS, now_unix) {
                continue;
            }
            let size = dir_size_bytes(&repo);
            match fs::remove_dir_all(&repo) {
                Ok(_) => {
                    report.workspaces_deleted += 1;
                    report.workspaces_bytes_freed =
                        report.workspaces_bytes_freed.saturating_add(size);
                }
                Err(e) => report
                    .errors
                    .push(format!("rm workspace {}: {}", repo.display(), e)),
            }
        }
    }
    // 3. Old shift-log session files
    let shift_log_root = runtime_root.join("cgreality-shift-log");
    if let Ok(entries) = fs::read_dir(&shift_log_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            if !path_older_than(&path, GC_SHIFT_LOG_RETENTION_DAYS, now_unix) {
                continue;
            }
            let size = meta.len();
            match fs::remove_file(&path) {
                Ok(_) => {
                    report.shift_logs_deleted += 1;
                    report.shift_logs_bytes_freed =
                        report.shift_logs_bytes_freed.saturating_add(size);
                }
                Err(e) => report
                    .errors
                    .push(format!("rm shift-log {}: {}", path.display(), e)),
            }
        }
    }
    // 4. Oversized hook-events.jsonl: tail-truncate to TARGET_AFTER_TRUNCATE
    let hook_events = runtime_root.join("cgreality-hook-events.jsonl");
    if let Ok(meta) = fs::metadata(&hook_events) {
        let size = meta.len();
        report.hook_events_bytes_before = size;
        if size > GC_HOOK_EVENTS_MAX_BYTES {
            match truncate_jsonl_to_tail(&hook_events, GC_HOOK_EVENTS_TARGET_AFTER_TRUNCATE_BYTES) {
                Ok(after) => {
                    report.hook_events_truncated = true;
                    report.hook_events_bytes_after = after;
                }
                Err(e) => report.errors.push(format!("truncate hook-events: {}", e)),
            }
        } else {
            report.hook_events_bytes_after = size;
        }
    }
    report.finished_at_unix = unix_secs_for_logging();
    report
}

/// U1-LIMIT: cap on failure-* files retained under `<runtime_root>/failures/`.
/// Each record can be ~100 KB (full tool-call args embedded), so a tight failure
/// loop fills disk fast. After each write we prune oldest beyond this count.
const MAX_FAILURE_FILES_RETAINED: usize = 100;

/// Best-effort prune: keep the newest `max_files` failure-*.json files in
/// `failure_root`, delete older ones. Silent on errors — we never fail a
/// failure write because cleanup hit a permission issue.
fn prune_failure_dir(failure_root: &Path, max_files: usize) -> std::result::Result<usize, String> {
    let entries = match fs::read_dir(failure_root) {
        Ok(e) => e,
        Err(e) => return Err(e.to_string()),
    };
    let mut files: Vec<(SystemTime, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|s| s.starts_with("failure-") && s.ends_with(".json"))
                .unwrap_or(false)
        })
        .filter_map(|e| {
            let mtime = e.metadata().ok()?.modified().ok()?;
            Some((mtime, e.path()))
        })
        .collect();
    if files.len() <= max_files {
        return Ok(0);
    }
    files.sort_by_key(|(mtime, _)| *mtime); // oldest first
    let to_delete = files.len() - max_files;
    let mut deleted = 0usize;
    for (_, path) in files.iter().take(to_delete) {
        if fs::remove_file(path).is_ok() {
            deleted += 1;
        }
    }
    Ok(deleted)
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
    // U1: prune after each write. Best-effort — never propagate cleanup errors
    // back to the caller; the failure record itself was written successfully.
    let _ = prune_failure_dir(&failure_root, MAX_FAILURE_FILES_RETAINED);
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
        .stderr(Stdio::from(stderr_file))
        // Put the child in its OWN process group (pgid == child pid). On timeout we
        // kill the entire group (-pgid) so all descendants — docker containers
        // launched by python swebench, cargo workers, git subprocs — die with the
        // immediate child. Without this, SIGKILL on the immediate child orphans
        // grandchildren (notably leaks Docker containers labeled to this run).
        .process_group(0);
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
            // Kill the WHOLE process group, not just the immediate child. This
            // ensures docker containers / python subprocs / cargo workers spawned
            // by the child get cleaned up. Negative PID = pgid kill (process_group(0)
            // earlier set pgid == child.id()).
            kill_process_group_graceful(child.id() as i32);
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

/// Send SIGTERM to the process group rooted at `pgid_pid` (the child's pid, since
/// we spawned with `process_group(0)`), give it 2 seconds to exit gracefully,
/// then SIGKILL the group. Failures are silent — the caller has already decided
/// to give up on the child and will reap it via `wait()` afterwards.
fn kill_process_group_graceful(pgid_pid: i32) {
    if pgid_pid <= 0 {
        return;
    }
    // SAFETY: libc::kill with a negative pid sends to the process group abs(pid).
    // No memory access; just a syscall.
    unsafe {
        libc::kill(-pgid_pid, libc::SIGTERM);
    }
    std::thread::sleep(Duration::from_secs(2));
    unsafe {
        libc::kill(-pgid_pid, libc::SIGKILL);
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

struct InnerCodingHarness {
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

fn inner_model_identity_readback(
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
        "qwen3.context_length",
        "qwen2.context_length",
        "tokenizer.chat_template",
    ] {
        if let Ok(value) = model.meta_val_str(key) {
            selected_metadata.insert(key.to_string(), value);
        }
    }
    selected_metadata
}

fn validate_inner_model_identity(
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
    if !compact_identity.contains("qwen3") {
        return Err(RealityError::new(
            "RUST_REALITY_INNER_MODEL_IDENTITY_MISMATCH",
            "loaded GGUF does not identify as Qwen 3",
            "inner_harness.model.metadata",
            "install the intended Qwen 3.6 27B GGUF; the SWE harness will not run a substitute model under a pinned label",
            json!({
                "required_identity": "qwen3",
                "identity_readback": identity,
                "model_path": file_sot(model_path)
            }),
            Some(file_sot(model_path)),
        ));
    }
    Ok(())
}

/// Cryptographic pin entry for the the inner model coding harness.
///
/// Loaded from `config/ccreality_model_pins.json` (or
/// `--inner-model-pin-path`). Every field is mandatory: the harness fails
/// closed on missing fields, missing keys, missing files, size drift, and
/// SHA drift. There is no fallback to "unpinned" mode by design.
#[derive(Debug, Deserialize)]
struct InnerModelPin {
    #[allow(dead_code)] // surfaced in error details for operator readability
    repo_id: String,
    #[allow(dead_code)] // surfaced in error details for operator readability
    filename: String,
    /// `sha256:<hex>` matching the streaming SHA-256 of the model file.
    expected_sha256: String,
    /// File size in bytes. Cheap pre-check before SHA over a 15 GiB file.
    expected_size_bytes: u64,
    /// Lower bound on `n_ctx_train` from GGUF metadata.
    expected_min_context_tokens: u32,
    /// Required substrings inside the lowercase-alnum-stripped identity blob.
    /// Defense-in-depth alongside SHA: catches "right bytes, but the file we
    /// pinned was the wrong model in the first place" only after fixed.
    expected_identity_substrings: Vec<String>,
}

fn load_model_pin(pin_path: &Path, key: &str) -> Result<InnerModelPin> {
    let content = fs::read_to_string(pin_path).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_INNER_MODEL_PIN_CONFIG_MISSING",
            "model pin config file is missing or unreadable",
            "ccreality_model_pins.path",
            "ensure config/ccreality_model_pins.json exists and is readable, or pass --inner-model-pin-path",
            json!({"error": err.to_string(), "pin_path": file_sot(pin_path), "pin_key": key}),
            Some(file_sot(pin_path)),
        )
    })?;
    let config: Value = serde_json::from_str(&content).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_INNER_MODEL_PIN_CONFIG_INVALID_JSON",
            "model pin config file is not valid JSON",
            "ccreality_model_pins.json",
            "fix the JSON syntax in the model pin config",
            json!({"error": err.to_string(), "pin_path": file_sot(pin_path), "pin_key": key}),
            Some(file_sot(pin_path)),
        )
    })?;
    let pointer = format!("/models/{key}");
    let pin_value = config.pointer(&pointer).ok_or_else(|| {
        RealityError::new(
            "RUST_REALITY_INNER_MODEL_PIN_NOT_FOUND",
            "no pin entry exists for the requested model key",
            "ccreality_model_pins.models",
            "add a pin entry under .models for this key, or pass --inner-model-pin-key with a key that exists",
            json!({"requested_key": key, "pin_path": file_sot(pin_path)}),
            Some(file_sot(pin_path)),
        )
    })?;
    serde_json::from_value::<InnerModelPin>(pin_value.clone()).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_INNER_MODEL_PIN_SCHEMA_INVALID",
            "model pin entry schema is invalid",
            &format!("ccreality_model_pins.models.{key}"),
            "the pin entry must include all of: repo_id, filename, expected_sha256, expected_size_bytes, expected_min_context_tokens, expected_identity_substrings",
            json!({
                "error": err.to_string(),
                "key": key,
                "value": pin_value,
                "pin_path": file_sot(pin_path)
            }),
            Some(file_sot(pin_path)),
        )
    })
}

fn validate_inner_model_pin(
    model_path: &Path,
    pin: &InnerModelPin,
    pin_path: &Path,
    pin_key: &str,
) -> Result<()> {
    // Size first: cheap fail-fast before paying for a streaming SHA over a 15 GiB file.
    let metadata = fs::metadata(model_path)
        .map_err(|err| fs_error("RUST_REALITY_INNER_MODEL_PIN_STAT_FAILED", model_path, err))?;
    let actual_size = metadata.len();
    if actual_size != pin.expected_size_bytes {
        return Err(RealityError::new(
            "RUST_REALITY_INNER_MODEL_SIZE_MISMATCH",
            "loaded GGUF size does not match the pinned size",
            "inner_harness.model.size_bytes",
            "the file on disk has been replaced, truncated, or corrupted; re-download to match the pin, or update the pin if intentionally upgrading",
            json!({
                "expected_size_bytes": pin.expected_size_bytes,
                "actual_size_bytes": actual_size,
                "pin_key": pin_key,
                "pin_path": file_sot(pin_path),
                "model_path": file_sot(model_path),
            }),
            Some(file_sot(model_path)),
        ));
    }
    // SHA-256 next: streaming so we don't allocate 15 GiB of resident memory.
    let actual_sha = sha256_file_streaming(model_path)?;
    if actual_sha != pin.expected_sha256 {
        return Err(RealityError::new(
            "RUST_REALITY_INNER_MODEL_SHA_MISMATCH",
            "loaded GGUF SHA-256 does not match the pinned hash",
            "inner_harness.model.sha256",
            "the file content drifted from the pin; re-download from the pinned repo_id/filename, or update the pin if intentionally upgrading",
            json!({
                "expected_sha256": pin.expected_sha256,
                "actual_sha256": actual_sha,
                "pin_key": pin_key,
                "pin_path": file_sot(pin_path),
                "model_path": file_sot(model_path),
            }),
            Some(file_sot(model_path)),
        ));
    }
    Ok(())
}

fn validate_inner_model_pin_identity_substrings(
    model_path: &Path,
    pin: &InnerModelPin,
    pin_path: &Path,
    pin_key: &str,
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
    let missing: Vec<&String> = pin
        .expected_identity_substrings
        .iter()
        .filter(|needle| {
            let n: String = needle
                .chars()
                .filter(|ch| ch.is_ascii_alphanumeric())
                .map(|ch| ch.to_ascii_lowercase())
                .collect();
            !compact_identity.contains(&n)
        })
        .collect();
    if !missing.is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_INNER_MODEL_PIN_IDENTITY_SUBSTRING_MISSING",
            "loaded GGUF identity does not include all required substrings from the pin",
            "inner_harness.model.metadata",
            "the pinned bytes match but the metadata does not declare the expected identity; this typically means the pin was authored against a wrongly-tagged file",
            json!({
                "expected_identity_substrings": pin.expected_identity_substrings,
                "missing_substrings": missing,
                "identity_readback": identity,
                "pin_key": pin_key,
                "pin_path": file_sot(pin_path),
                "model_path": file_sot(model_path),
            }),
            Some(file_sot(model_path)),
        ));
    }
    Ok(())
}

impl InnerCodingHarness {
    fn load(args: &AttemptArgs) -> Result<Self> {
        let model_path = resolve_path(&args.inner_model_path)?;
        require_file(
            &model_path,
            "inner_model_path",
            "RUST_REALITY_INNER_MODEL_PATH_MISSING",
        )?;
        // Cryptographic pin gate: load the active model pin and verify the
        // bytes on disk match BEFORE we touch the GPU backend. Size first
        // (cheap), then streaming SHA-256. Mismatch fails loud — no fallback.
        let pin_path = resolve_path(&args.inner_model_pin_path)?;
        require_file(
            &pin_path,
            "inner_model_pin_path",
            "RUST_REALITY_INNER_MODEL_PIN_CONFIG_MISSING",
        )?;
        let pin = load_model_pin(&pin_path, &args.inner_model_pin_key)?;
        validate_inner_model_pin(&model_path, &pin, &pin_path, &args.inner_model_pin_key)?;
        let backend = LlamaBackend::init().map_err(|err| {
            RealityError::new(
                "RUST_REALITY_INNER_BACKEND_INIT_FAILED",
                "failed to initialize the Rust llama.cpp backend",
                "inner_harness.backend",
                "inspect CUDA/llama.cpp runtime initialization before retrying",
                json!({"error": err.to_string(), "model_path": file_sot(&model_path)}),
                Some(file_sot(&model_path)),
            )
        })?;
        let model_params = LlamaModelParams::default().with_n_gpu_layers(args.inner_gpu_layers);
        let model =
            LlamaModel::load_from_file(&backend, &model_path, &model_params).map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_INNER_MODEL_LOAD_FAILED",
                    "failed to load the inner model GGUF model through the Rust coding harness",
                    "inner_harness.model_path",
                    "provide a real the inner model GGUF file readable by the Rust harness",
                    json!({
                        "error": err.to_string(),
                        "model_path": file_sot(&model_path),
                        "gpu_layers": args.inner_gpu_layers
                    }),
                    Some(file_sot(&model_path)),
                )
            })?;
        let chat_template = match args.inner_chat_template.as_deref() {
            Some(template) => LlamaChatTemplate::new(template).map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_INNER_CHAT_TEMPLATE_INVALID",
                    "explicit the inner model chat template contained an invalid NUL byte",
                    "inner_chat_template",
                    "provide a valid llama.cpp chat template name or template text",
                    json!({"error": err.to_string()}),
                    Some(file_sot(&model_path)),
                )
            })?,
            None => model.chat_template(None).map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_INNER_CHAT_TEMPLATE_MISSING",
                    "the inner model GGUF model does not expose a usable chat template",
                    "inner_harness.chat_template",
                    "install a the inner model GGUF with chat-template metadata or pass --inner-chat-template explicitly",
                    json!({"error": err.to_string(), "model_path": file_sot(&model_path)}),
                    Some(file_sot(&model_path)),
                )
            })?,
        };
        let model_context_train_tokens = model.n_ctx_train();
        let model_identity = inner_model_identity_readback(&model, &model_path);
        validate_inner_model_identity(&model_path, &model_identity)?;
        validate_inner_model_pin_identity_substrings(
            &model_path,
            &pin,
            &pin_path,
            &args.inner_model_pin_key,
            &model_identity,
        )?;
        if model_context_train_tokens == 0 {
            return Err(RealityError::new(
                "RUST_REALITY_INNER_MODEL_CONTEXT_INVALID",
                "loaded GGUF reported a zero training context window",
                "inner_harness.model.n_ctx_train",
                "install a valid the inner model GGUF with context metadata",
                json!({"model_path": file_sot(&model_path), "n_ctx_train": model_context_train_tokens}),
                Some(file_sot(&model_path)),
            ));
        }
        // The pin can require a higher per-model floor than the global default.
        // Both gates fail loud independently — defense in depth.
        let effective_min_context = pin
            .expected_min_context_tokens
            .max(DEFAULT_INNER_MIN_CONTEXT_TOKENS);
        if model_context_train_tokens < effective_min_context {
            return Err(RealityError::new(
                "RUST_REALITY_INNER_MODEL_CONTEXT_BELOW_REQUIRED_FLOOR",
                "loaded the inner model GGUF context is below the SWE coding harness capability floor",
                "inner_harness.model.n_ctx_train",
                "install an inner-model GGUF with at least the pinned training context so repository context and tool transcripts fit without silent truncation",
                json!({
                    "minimum_context_tokens": effective_min_context,
                    "global_minimum_context_tokens": DEFAULT_INNER_MIN_CONTEXT_TOKENS,
                    "pin_minimum_context_tokens": pin.expected_min_context_tokens,
                    "model_context_train_tokens": model_context_train_tokens,
                    "model_identity": model_identity,
                    "model_path": file_sot(&model_path)
                }),
                Some(file_sot(&model_path)),
            ));
        }
        let context_tokens = args
            .inner_context_tokens
            .unwrap_or(model_context_train_tokens);
        if context_tokens > model_context_train_tokens {
            return Err(RealityError::new(
                "RUST_REALITY_INNER_CONTEXT_EXCEEDS_MODEL_LIMIT",
                "requested the inner model context exceeds the loaded GGUF training context",
                "inner_context_tokens",
                "omit --inner-context-tokens to use the model maximum, or set a value within the GGUF n_ctx_train limit",
                json!({
                    "requested_context_tokens": context_tokens,
                    "model_context_train_tokens": model_context_train_tokens,
                    "model_path": file_sot(&model_path)
                }),
                Some(file_sot(&model_path)),
            ));
        }
        if context_tokens < DEFAULT_INNER_MIN_CONTEXT_TOKENS {
            return Err(RealityError::new(
                "RUST_REALITY_INNER_EFFECTIVE_CONTEXT_BELOW_REQUIRED_FLOOR",
                "effective the inner model context is below the SWE coding harness capability floor",
                "inner_context_tokens",
                "omit --inner-context-tokens to use the GGUF maximum, or set an explicit context of at least 128K tokens",
                json!({
                    "minimum_context_tokens": DEFAULT_INNER_MIN_CONTEXT_TOKENS,
                    "effective_context_tokens": context_tokens,
                    "requested_context_tokens": args.inner_context_tokens,
                    "model_context_train_tokens": model_context_train_tokens,
                    "model_path": file_sot(&model_path)
                }),
                Some(file_sot(&model_path)),
            ));
        }
        if args.inner_max_new_tokens >= context_tokens as usize {
            return Err(RealityError::new(
                "RUST_REALITY_INNER_MAX_NEW_TOKENS_EXCEEDS_CONTEXT",
                "the inner model max_new_tokens leaves no room for a prompt inside the configured context window",
                "inner_max_new_tokens",
                "set max_new_tokens lower than the effective context window",
                json!({
                    "max_new_tokens": args.inner_max_new_tokens,
                    "effective_context_tokens": context_tokens,
                    "requested_context_tokens": args.inner_context_tokens,
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
            requested_context_tokens: args.inner_context_tokens,
            model_context_train_tokens,
            context_tokens,
            max_new_tokens: args.inner_max_new_tokens,
            temperature: args.inner_temperature,
            top_p: args.inner_top_p,
            batch_size: args.inner_batch_size,
            seed: args.inner_seed,
        })
    }

    fn model_capability_report(&self, report_path: &Path) -> Result<Value> {
        let chat_template = self.chat_template.to_string().map_err(|err| {
            RealityError::new(
                "RUST_REALITY_INNER_CHAT_TEMPLATE_UTF8_INVALID",
                "the inner model chat template was not valid UTF-8",
                "inner_harness.chat_template",
                "install a GGUF with a valid UTF-8 chat template or pass a valid explicit template",
                json!({"error": err.to_string(), "model_path": file_sot(&self.model_path)}),
                Some(file_sot(&self.model_path)),
            )
        })?;
        let mut selected_metadata = BTreeMap::new();
        selected_metadata.extend(inner_model_identity_readback(&self.model, &self.model_path));
        Ok(json!({
            "record_kind": "contextgraph_inner_harness_model_capability_readback",
            "status": "ok",
            "provider": "contextgraph_rust_inner_coding_harness",
            "model_path": file_sot(&self.model_path),
            "requested_context_tokens": self.requested_context_tokens,
            "effective_context_tokens": self.context_tokens,
            "context_policy": if self.requested_context_tokens.is_some() { "explicit_cli" } else { "gguf_n_ctx_train_max" },
            "model_context_train_tokens": self.model_context_train_tokens,
            "minimum_context_tokens": DEFAULT_INNER_MIN_CONTEXT_TOKENS,
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
                    "RUST_REALITY_INNER_MESSAGES_MISSING",
                    "the inner model harness request did not contain messages array",
                    "inner_harness.request.messages",
                    "fix the Rust request builder before running another attempt",
                    json!({"request": file_sot(request_path)}),
                    Some(file_sot(request_path)),
                )
            })?;
        let tools = body.get("tools").and_then(Value::as_array).ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_INNER_TOOLS_MISSING",
                "the inner model harness request did not contain tools array",
                "inner_harness.request.tools",
                "fix the Rust request builder before running another attempt",
                json!({"request": file_sot(request_path)}),
                Some(file_sot(request_path)),
            )
        })?;
        if tools.is_empty() {
            return Err(RealityError::new(
                "RUST_REALITY_INNER_TOOLS_EMPTY",
                "the inner model coding harness refuses to run without governed tools",
                "inner_harness.request.tools",
                "provide the ContextGraph coding-agent tool schema",
                json!({"request": file_sot(request_path)}),
                Some(file_sot(request_path)),
            ));
        }
        let messages_json =
            serde_json::to_string(&Value::Array(messages.clone())).map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_INNER_MESSAGES_SERIALIZE_FAILED",
                    "failed to serialize the inner model chat messages",
                    "inner_harness.request.messages",
                    "inspect the message JSON shape",
                    json!({"error": err.to_string(), "request": file_sot(request_path)}),
                    Some(file_sot(request_path)),
                )
            })?;
        let tools_json = serde_json::to_string(&Value::Array(tools.clone())).map_err(|err| {
            RealityError::new(
                "RUST_REALITY_INNER_TOOLS_SERIALIZE_FAILED",
                "failed to serialize the inner model tool schemas",
                "inner_harness.request.tools",
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
                    "RUST_REALITY_INNER_CHAT_TEMPLATE_APPLY_FAILED",
                    "failed to render the inner model chat template with ContextGraph tools",
                    "inner_harness.chat_template",
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
                "RUST_REALITY_INNER_TOOL_GRAMMAR_MISSING",
                "the inner model chat template did not produce a tool-call grammar",
                "inner_harness.tool_grammar",
                "use a the inner model GGUF/template combination that supports strict OpenAI-compatible tool grammar",
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
                    "RUST_REALITY_INNER_TOOL_RESPONSE_PARSE_FAILED",
                    "the inner model generation could not be parsed as an OpenAI-compatible assistant message",
                    "inner_harness.response",
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
                "RUST_REALITY_INNER_TOOL_RESPONSE_JSON_INVALID",
                "parsed the inner model assistant message was not JSON",
                "inner_harness.response_json",
                "inspect the llama.cpp OpenAI-compatible parser output",
                json!({
                    "error": err.to_string(),
                    "parsed_text": parsed_text,
                    "raw_generation": file_sot(&raw_generation_path)
                }),
                Some(file_sot(&raw_generation_path)),
            )
        })?;
        let message = normalize_inner_harness_message(&parsed, request_path, &raw_generation_path)?;
        Ok(json!({
            "model": args.model,
            "created_at_unix": unix_secs()?,
            "done": true,
            "message": message,
            "contextgraph_inner_harness": {
                "provider": "contextgraph_rust_inner_coding_harness",
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
                "RUST_REALITY_INNER_CONTEXT_TOKENS_INVALID",
                "the inner model context window cannot be zero",
                "inner_context_tokens",
                "set --inner-context-tokens to a positive value",
                json!({"inner_context_tokens": self.context_tokens}),
                Some(file_sot(request_path)),
            )
        })?;
        let batch_size = u32::try_from(self.batch_size).map_err(|err| {
            RealityError::new(
                "RUST_REALITY_INNER_BATCH_SIZE_TOO_LARGE",
                "the inner model batch size could not be represented for llama.cpp context params",
                "inner_batch_size",
                "set --inner-batch-size no larger than u32::MAX",
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
                    "RUST_REALITY_INNER_CONTEXT_CREATE_FAILED",
                    "failed to create llama.cpp generation context",
                    "inner_harness.context",
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
                    "RUST_REALITY_INNER_PROMPT_TOKENIZE_FAILED",
                    "failed to tokenize rendered the inner model prompt",
                    "inner_harness.prompt",
                    "inspect chat template output and model tokenizer compatibility",
                    json!({"error": err.to_string(), "request": file_sot(request_path)}),
                    Some(file_sot(request_path)),
                )
            })?;
        if tokens.len() >= self.context_tokens as usize {
            return Err(RealityError::new(
                "RUST_REALITY_INNER_PROMPT_CONTEXT_EXCEEDED",
                "rendered the inner model prompt exceeds the configured context window",
                "inner_harness.prompt_tokens",
                "reduce repository context/tool transcript size or increase --inner-context-tokens after verifying model capacity",
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
                "RUST_REALITY_INNER_PROMPT_AND_OUTPUT_CONTEXT_EXCEEDED",
                "rendered the inner model prompt plus requested generation budget exceeds the configured context window",
                "inner_harness.prompt_tokens_plus_max_new_tokens",
                "reduce prompt/tool transcript size or use a the inner model GGUF with a larger training context; the harness will not silently lower the requested budget",
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
                        "RUST_REALITY_INNER_GENERATION_BATCH_ADD_FAILED",
                        "failed to append generated token to llama.cpp batch",
                        "inner_harness.generation_batch",
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
                    "RUST_REALITY_INNER_GENERATION_DECODE_FAILED",
                    "llama.cpp failed while decoding a generated token",
                    "inner_harness.generation_decode",
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
                "RUST_REALITY_INNER_TOKEN_DECODE_FAILED",
                "failed to decode generated llama.cpp tokens",
                "inner_harness.generated_tokens",
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
                "RUST_REALITY_INNER_TOOL_GRAMMAR_MISSING",
                "the inner model chat template did not produce a tool-call grammar",
                "inner_harness.tool_grammar",
                "use a the inner model GGUF/template combination that supports strict tool grammar",
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
                        "RUST_REALITY_INNER_GRAMMAR_SAMPLER_FAILED",
                        "failed to create lazy-pattern tool grammar sampler",
                        "inner_harness.tool_grammar",
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
                        "RUST_REALITY_INNER_GRAMMAR_SAMPLER_FAILED",
                        "failed to create lazy tool grammar sampler",
                        "inner_harness.tool_grammar",
                        "inspect generated tool grammar and chat-template triggers",
                        json!({"error": err.to_string(), "request": file_sot(request_path)}),
                        Some(file_sot(request_path)),
                    )
                })
            }
        } else {
            LlamaSampler::grammar(&self.model, grammar, "root").map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_INNER_GRAMMAR_SAMPLER_FAILED",
                    "failed to create strict tool grammar sampler",
                    "inner_harness.tool_grammar",
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
        .unwrap_or("inner-harness");
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
                        "RUST_REALITY_INNER_PROMPT_BATCH_ADD_FAILED",
                        "failed to append prompt token to llama.cpp batch",
                        "inner_harness.prompt_batch",
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
                "RUST_REALITY_INNER_PROMPT_DECODE_FAILED",
                "llama.cpp failed while decoding rendered prompt",
                "inner_harness.prompt_decode",
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

fn normalize_inner_harness_message(
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
            "RUST_REALITY_INNER_MESSAGE_SHAPE_INVALID",
            "the inner model parser output was not an assistant message object",
            "inner_harness.response.message",
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
                "RUST_REALITY_INNER_MESSAGE_ROLE_INVALID",
                "the inner model parser output did not return an assistant role",
                "inner_harness.response.message.role",
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
                "RUST_REALITY_INNER_MESSAGE_ROLE_MISSING",
                "the inner model parser output omitted assistant message role",
                "inner_harness.response.message.role",
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
                "RUST_REALITY_INNER_TOOL_CALLS_SHAPE_INVALID",
                "the inner model parser output tool_calls was not an array",
                "inner_harness.response.message.tool_calls",
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

fn inner_coding_harness_chat_json(
    harness: &InnerCodingHarness,
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
        "before_inner_coding_harness",
        //cgreality
        &args.model,
    )?;
    // Per-attempt wallclock budget: at ~10-25 min/round, a 30-round attempt
    // can exceed the 1h MCP-stdio idle watchdog before producing any summary.
    // This budget is checked at the top of each round (post-check, not a
    // tokio::time::timeout, because harness.chat() is synchronous and likely
    // holds !Send GGUF FFI handles). A chat call already in progress runs to
    // completion before the abort fires; the alternative is a watchdog kill
    // with no attempt-summary.json. Default 7200s (120 min); override via
    // CGREALITY_PER_ATTEMPT_DEADLINE_SECS. Routes through the existing
    // forced_stop machinery so attempt-summary.json is still written.
    let attempt_start = Instant::now();
    let attempt_deadline_secs: u64 = std::env::var("CGREALITY_PER_ATTEMPT_DEADLINE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7200); // default 120 min — at 10-25 min/round, allows 5-12 rounds before clean abort (turn-72 raised from 2700 after empirical observation that 5-round trajectory minimum needs ~90+ min)
    let attempt_deadline = Duration::from_secs(attempt_deadline_secs);
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
Edit-scope guardrail (critical): when calling apply_line_window_edit, apply_structured_edit, or apply_unified_diff, replace the SMALLEST contiguous range needed for the fix. \
Never delete module-level definitions (functions, classes, constants, dicts, imports) that lie outside your fix scope; if any such definitions exist inside your replace range, they MUST appear unchanged in your replace text. \
Most bugs are fixable by a 1-10 line change inside ONE function; if your replace range spans more than one def/class/top-level statement, reconsider because you are probably over-selecting. \
Before submitting, mentally verify that your replace text preserves every import, function signature, and top-level definition that existed in your replace range. \
Post-edit self-check (critical): after every apply_line_window_edit call, the response payload contains a post_edit_window field showing the post-edit file content (edit_range +/- 5 lines). You MUST read post_edit_window before doing anything else. \
Verify the edit landed in the intended file region; verify no module-level definitions (functions, classes, imports, constants) outside the fix scope were deleted or corrupted; verify indentation is consistent and no if/for/def/class body is empty. \
If post_edit_window shows broken indentation, an empty block body, missing imports, or content in the wrong region, the edit is wrong: call open_file_window on adjacent lines and re-issue apply_line_window_edit with a corrected line range, or switch to apply_structured_edit with a precise unique find span. \
Do NOT submit the candidate patch until post_edit_window confirms the edit is syntactically valid and semantically targeted. \
Pre-submit verification (critical): after post_edit_window confirms the edit, BEFORE producing your final candidate patch you MUST call run_targeted_test once for each FAIL_TO_PASS test id listed in the problem-reality packet. \
A response with status 'ok' (exit_code 0) means that test now passes against your edit; status 'failed' (exit_code != 0) means the patch does not yet fix that test, so revise the edit and re-test. \
After all FAIL_TO_PASS tests return 'ok', also call run_targeted_test on at least 2-3 PASS_TO_PASS tests from the same packet to catch regressions; if any returns 'failed', your edit broke a previously passing test and must be revised. \
Tool limitation: if run_targeted_test stdout/stderr shows ImportError or ModuleNotFoundError for a missing dependency, that is an environment limitation of this host, NOT a defect in your patch. Skip that test, note it in your reasoning, rely on the official Docker evaluator downstream, and do not loop retrying. \
Only after FAIL_TO_PASS tests pass (or are environment-skipped) and sampled PASS_TO_PASS tests still pass should you submit. \
Do not modify tests, fixtures, or benchmark harness files.",        harness.context_tokens, harness.max_new_tokens, args.agent_tool_rounds    );
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
    let mut dynamicjepa_live_last_panel_id: Option<String> =
        ctx.dynamicjepa_initial_panel_id.clone();
    // Force-stop bookkeeping: when set, the round loop breaks and the existing
    // post-loop blocked path uses these fields instead of the generic
    // RUST_REALITY_FINAL_EDIT_TOOL_REQUIRED error. Keeps the engine's exit
    // contract (Ok(blocked_value)) while giving the optimizer a precise
    // diagnosis. See MAX_ROUNDS_PER_ATTEMPT_HARD_CAP and
    // REPEATED_ERROR_DEADLOCK_LEN at module scope.
    let mut forced_stop: Option<(String, String, String)> = None; // (error_code, message, remediation)
    let mut forced_stop_round: Option<u64> = None;
    let mut last_error_codes: VecDeque<String> =
        VecDeque::with_capacity(REPEATED_ERROR_DEADLOCK_LEN);
    for round in 1..=args.agent_tool_rounds {
        if round > MAX_ROUNDS_PER_ATTEMPT_HARD_CAP {
            forced_stop = Some((
                "RUST_REALITY_MAX_ROUNDS_EXCEEDED".to_string(),
                format!(
                    "model exceeded max-rounds-per-attempt ({MAX_ROUNDS_PER_ATTEMPT_HARD_CAP}) without producing a valid candidate patch"
                ),
                "raise MAX_ROUNDS_PER_ATTEMPT_HARD_CAP only after confirming the failing trajectory is genuinely exploring; otherwise diagnose why the inner model cannot converge inside the cap"
                    .to_string(),
            ));
            forced_stop_round = Some(round);
            transcript_entries.push(json!({
                "round": round,
                "status": "forced_stop",
                "error_code": "RUST_REALITY_MAX_ROUNDS_EXCEEDED",
                "max_rounds_per_attempt_hard_cap": MAX_ROUNDS_PER_ATTEMPT_HARD_CAP,
                "tool_round_budget": args.agent_tool_rounds,
            }));
            break;
        }
        // Per-attempt wallclock budget post-check. Mirrors the
        // MAX_ROUNDS_EXCEEDED forced-stop pattern above. A chat call already
        // in progress completes before this fires (see attempt_deadline init
        // for rationale: synchronous chat() + likely !Send GGUF FFI).
        let elapsed = attempt_start.elapsed();
        if elapsed >= attempt_deadline {
            forced_stop = Some((
                "RUST_REALITY_ATTEMPT_BUDGET_EXCEEDED".to_string(),
                format!(
                    "attempt wallclock budget exceeded ({}s >= {}s) before producing a valid candidate patch",
                    elapsed.as_secs(),
                    attempt_deadline_secs
                ),
                "raise CGREALITY_PER_ATTEMPT_DEADLINE_SECS (default 7200 = 120 min) only after confirming the inner model is making real progress per round; otherwise diagnose why generation is too slow (large prompt, kv-cache pressure, host load)".to_string(),
            ));
            forced_stop_round = Some(round);
            transcript_entries.push(json!({
                "round": round,
                "status": "forced_stop",
                "error_code": "RUST_REALITY_ATTEMPT_BUDGET_EXCEEDED",
                "elapsed_secs": elapsed.as_secs(),
                "attempt_deadline_secs": attempt_deadline_secs,
            }));
            break;
        }
        let request_path = tools_root.join(format!("tool-round-{round:02}-request.json"));
        let edit_deadline = edit_deadline_active(
            round,
            args.agent_tool_rounds,
            &latest_editable_windows,
            no_tool_after_window_count,
            repeated_navigation_count,
            submitted_candidate_patch.is_some(),
        );
        // EDIT_LEAD_MARKER
        let round_tools = if edit_deadline {
            agent_edit_tool_specs()
        } else {
            tools.clone()
        };
        evict_messages_for_token_budget(
            &mut messages,
            harness.context_tokens,
            harness.max_new_tokens,
        );
        let body = json!({
                    "model": args.model,
                    "messages": messages,
        // FIX_LEAD
                    "stream": false,
                    "think": false,
        // noop_marker_3
                    "tools": round_tools,            "contextgraph_tool_policy": {
                        "edit_deadline_active": edit_deadline,
                        "latest_editable_windows": latest_editable_windows.clone(),
                        "no_tool_after_window_count": no_tool_after_window_count,
                        "repeated_navigation_count": repeated_navigation_count
                    },
                    "runtime": {
                        "provider": "contextgraph_rust_inner_coding_harness",
                        "model_path": file_sot(&harness.model_path)
                    },
                    "options": {
                        "temperature": args.inner_temperature,
                        "top_p": args.inner_top_p,
                        "requested_context_tokens": args.inner_context_tokens,
                        "effective_context_tokens": harness.context_tokens,
                        "model_context_train_tokens": harness.model_context_train_tokens,
                        "max_new_tokens": harness.max_new_tokens,
                        "gpu_layers": args.inner_gpu_layers,
                        "batch_size": args.inner_batch_size,
                        "seed": args.inner_seed
                    }
                });
        write_json_checked(&request_path, &body)?;
        let response = match harness.chat(args, &body, &request_path) {
            Ok(r) => r,
            Err(err) => {
                // V3-FORENSIC: capture gpu_after on the chat error path before
                // propagating. Without this, the failure record at
                // <run_root>/failures/failure-<ts>-<code>.json contains only
                // gpu_before — we lose evidence of whether chat leaked or grew
                // VRAM (CUDA OOM, tokenizer OOM, KV cache spike) before erroring.
                // Best-effort: ignore failures of read_gpu_vram_state itself.
                if let Ok(gpu_after) = read_gpu_vram_state(args.gpu_index) {
                    let path =
                        tools_root.join(format!("tool-round-{round:02}-gpu-after-chat-error.json"));
                    let _ = write_json_checked(
                        &path,
                        &json!({
                            "round": round,
                            "gpu_before": gpu_before,
                            "gpu_after_chat_error": gpu_after,
                            "delta_used_mb": (gpu_after.memory_used_mb as i64)
                                - (gpu_before.memory_used_mb as i64),
                            "delta_free_mb": (gpu_after.memory_free_mb as i64)
                                - (gpu_before.memory_free_mb as i64),
                            "error_code": err.error_code,
                            "source_of_truth": "nvidia-smi:query-gpu+query-compute-apps",
                        }),
                    );
                }
                return Err(err);
            }
        };
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
            // No tool call this round resets any error-code streak: the model
            // is no longer producing the "same" failure, it produced none at all.
            last_error_codes.clear();
            continue;
        }
        messages.push(response["message"].clone());
        let mut tool_results = Vec::new();
        // Round-level error code: None if any tool in this round succeeded;
        // otherwise the error_code of the last failing tool call. Used by the
        // consecutive-error deadlock detector below.
        let mut round_error_code: Option<String> = None;
        let mut round_had_ok_tool = false;
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
            let pre_action_control = pre_action_dynamicjepa_gate(
                ctx,
                &name,
                &arguments,
                dynamicjepa_live_last_panel_id.as_deref(),
                &tool_dir,
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
            if result_status == "ok" {
                round_had_ok_tool = true;
            } else if let Some(code) = result_readback.get("error_code").and_then(Value::as_str) {
                round_error_code = Some(code.to_string());
            }
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
            let live_dynamicjepa = live_ingest_dynamicjepa_transition(
                ctx,
                attempt,
                &harness_transition_path,
                &tool_dir,
            )?;
            if let Some(record) = &live_dynamicjepa {
                if let Some(panel_id) = record.get("panel_id").and_then(Value::as_str) {
                    dynamicjepa_live_last_panel_id = Some(panel_id.to_string());
                }
            }
            // CONTEXT-BUDGET: tool results are the dominant context-growth driver
            // across rounds. read_problem_reality(compact) can produce 60 KB, and
            // open_file_window/search_repo on large files can be similar. Each
            // result accumulates in the conversation history and gets re-prefilled
            // every round (we mint a fresh LlamaContext per chat call). Without a
            // cap, round-N prompt size is the sum of all prior results — quadratic
            // attention cost on a 27B model crawls past ~50K tokens.
            //
            // Truncate the COPY that goes into history; the full JSON is still
            // persisted at <tool_dir>/tool-result.json. The marker tells the inner
            // model exactly how to recover (re-call with tighter scope) so this is
            // not an information loss — just a context-budget discipline.
            let full_result_text = serde_json::to_string(&result_readback).unwrap();
            let result_text = truncate_tool_result_for_history(&full_result_text);
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
                "dynamicjepa_pre_action_control": pre_action_control,
                "dynamicjepa_live_ingest": live_dynamicjepa,
                "blackbox_ledger_evidence": ledger_evidence
            }));
        }
        // Update the rolling tail of round-level error codes for the
        // consecutive-error deadlock detector. A round with ANY ok tool
        // resets the streak. A round whose tools all failed contributes
        // its last failing error_code (or "UNKNOWN" if none was emitted).
        if round_had_ok_tool {
            last_error_codes.clear();
        } else {
            let code = round_error_code.unwrap_or_else(|| "UNKNOWN".to_string());
            if last_error_codes.len() == REPEATED_ERROR_DEADLOCK_LEN {
                last_error_codes.pop_front();
            }
            last_error_codes.push_back(code);
        }
        transcript_entries.push(json!({
            "round": round,
            "status": "tool_results_appended",
            "edit_deadline_active": edit_deadline,
            "latest_editable_windows": latest_editable_windows.clone(),
            "tool_request_protocol": tool_request_protocol,
            "request": file_sot(&request_path),
            "response": file_sot(&response_path),
            "tool_results": tool_results,
            "round_error_code": last_error_codes.back()
        }));
        if let Some(deadlock_code) = repeated_error_deadlock_code(&last_error_codes) {
            forced_stop = Some((
                "RUST_REALITY_REPEATED_ERROR_DEADLOCK".to_string(),
                format!(
                    "model produced the same tool error_code ({deadlock_code}) for {REPEATED_ERROR_DEADLOCK_LEN} consecutive rounds without recovering"
                ),
                "diagnose why the failing tool keeps rejecting the model's input (e.g., observed_sha256 drift, malformed find/replace, repeated py_compile failure) and either fix the tool's error contract or guide the model with a clearer remediation prompt".to_string(),
            ));
            forced_stop_round = Some(round);
            transcript_entries.push(json!({
                "round": round,
                "status": "forced_stop",
                "error_code": "RUST_REALITY_REPEATED_ERROR_DEADLOCK",
                "deadlock_error_code": deadlock_code,
                "deadlock_length": REPEATED_ERROR_DEADLOCK_LEN,
            }));
            break;
        }
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
            "dynamicjepa_initial_panel_id": ctx.dynamicjepa_initial_panel_id.clone(),
            "executed_tool_calls": executed_tool_calls,            "status": "blocked",
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
            "field_path": "inner_harness.message.tool_calls",
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
        write_json_checked(&attempt_dir.join("inner-harness-response.json"), &blocked)?;
        return Ok(blocked);
    }

    let gpu_after = read_gpu_vram_state(args.gpu_index)?;
    assert_gpu_budget(
        &gpu_after,
        args.gpu_min_free_mb,
        args.gpu_max_used_mb,
        "after_inner_coding_harness",
        &args.model,
    )?;
    let Some(submitted_candidate_patch) = submitted_candidate_patch else {
        let attempted_edit = failed_candidate_submission.is_some();
        // forced_stop wins over the generic "no final edit" diagnosis: it
        // pinpoints why the loop was cut short (round cap or repeated-error
        // deadlock) so the optimizer reads a precise error_code instead of
        // the vague RUST_REALITY_FINAL_EDIT_TOOL_REQUIRED.
        let (error_code, message, remediation) = if let Some((c, m, r)) = forced_stop.as_ref() {
            (c.as_str(), m.as_str(), r.as_str())
        } else if attempted_edit {
            (
                "RUST_REALITY_FINAL_EDIT_TOOL_FAILED",
                "model called a final edit tool but it did not produce a candidate patch",
                "reinject the failed final edit result, open the exact source window, and submit a line-window edit, unique find/replace edit, or observed unified diff",
            )
        } else {
            (
                "RUST_REALITY_FINAL_EDIT_TOOL_REQUIRED",
                "model used governed tools but never submitted a candidate patch through apply_line_window_edit, apply_structured_edit, or apply_unified_diff",
                "call apply_line_window_edit with observed_sha256 from open_file_window, apply_structured_edit with exact find/replace edits copied from opened source windows, or apply_unified_diff with observed file hashes",
            )
        };
        let forced_stop_field_path = if forced_stop.is_some() {
            "agent_tools.round_loop_forced_stop"
        } else {
            "agent_tools.final_edit_submission"
        };
        let transcript = json!({
            "record_kind": "contextgraph_agent_tool_transcript",
            "attempt": attempt,
            "model": args.model,
            "tool_round_budget": args.agent_tool_rounds,
            "max_rounds_per_attempt_hard_cap": MAX_ROUNDS_PER_ATTEMPT_HARD_CAP,
            "repeated_error_deadlock_len": REPEATED_ERROR_DEADLOCK_LEN,
            "tools": agent_tool_names(),
            "entries": transcript_entries,
            "dynamicjepa_initial_panel_id": ctx.dynamicjepa_initial_panel_id.clone(),
            "executed_tool_calls": executed_tool_calls,
            "status": "blocked",
            "error_code": error_code,
            "message": message,
            "remediation": remediation,
            "forced_stop_round": forced_stop_round,
            "failed_candidate_submission": failed_candidate_submission,
            "source_of_truth": file_sot(&transcript_path)
        });
        write_json_checked(&transcript_path, &transcript)?;
        let blocked = json!({
            "record_kind": "contextgraph_model_step_blocked",
            "status": "blocked",
            "error_code": error_code,
            "field_path": forced_stop_field_path,
            "message_text": message,
            "remediation": remediation,
            "message": {"role": "assistant", "content": ""},
            "details": {
                "attempt": attempt,
                "model": args.model,
                "tool_round_budget": args.agent_tool_rounds,
                "max_rounds_per_attempt_hard_cap": MAX_ROUNDS_PER_ATTEMPT_HARD_CAP,
                "repeated_error_deadlock_len": REPEATED_ERROR_DEADLOCK_LEN,
                "executed_tool_calls": executed_tool_calls,
                "forced_stop_round": forced_stop_round,
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
        write_json_checked(&attempt_dir.join("inner-harness-response.json"), &blocked)?;
        return Ok(blocked);
    };
    let transcript = json!({
        "record_kind": "contextgraph_agent_tool_transcript",
        "attempt": attempt,
        "model": args.model,
        "tool_round_budget": args.agent_tool_rounds,
        "tools": agent_tool_names(),
        "entries": transcript_entries,
        "dynamicjepa_initial_panel_id": ctx.dynamicjepa_initial_panel_id.clone(),
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
    write_json_checked(&attempt_dir.join("inner-harness-response.json"), &enriched)?;
    Ok(enriched)
}

/// Returns `Some(code)` when the rolling tail of round-level error codes shows
/// `REPEATED_ERROR_DEADLOCK_LEN` consecutive identical non-empty codes — the
/// signal that the model is stuck repeating the exact same failure (e.g.
/// `RUST_REALITY_LINE_EDIT_PY_COMPILE_FAILED` round after round). Returns
/// `None` otherwise. Pure function so the round-loop logic is unit-testable
/// without spinning up a real the inner model harness.
fn repeated_error_deadlock_code(tail: &VecDeque<String>) -> Option<String> {
    if tail.len() < REPEATED_ERROR_DEADLOCK_LEN {
        return None;
    }
    let first = tail.front()?;
    if first.is_empty() {
        return None;
    }
    if tail.iter().all(|code| code == first) {
        Some(first.clone())
    } else {
        None
    }
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

/// Approximate token count for a JSON message using a conservative
/// chars-per-token ratio for the inner model's tokenizer (~3.5 chars/token). This is
/// intentionally a heuristic — exact counting requires the model tokenizer
/// which lives inside `harness.chat`. The estimate is only used to decide
/// when to evict middle-of-conversation tool turns before the prompt grows
/// past the context cap. Eviction preserves the system prompt (index 0),
/// the initial user task description (index 1), and the most recent turns
/// so the model never loses its grounding instructions or recent state.
fn estimate_message_tokens(message: &Value) -> usize {
    let serialized = serde_json::to_string(message).unwrap_or_default();
    // 3.5 chars/token rounded up; +8 for chat-template framing per message.
    (serialized.len() * 2 / 7) + 8
}

/// Proactive context-window guard. When the accumulated `messages` plus
/// reserved generation budget approach the model's context cap, evict the
/// oldest middle-section assistant/tool turn pairs until we are back below
/// the safety threshold. The first two messages (system, initial user) and
/// the most recent turns are always preserved. Tool-call/tool-response
/// pairing is preserved by evicting an assistant message together with all
/// `tool` messages that immediately follow it.
fn evict_messages_for_token_budget(
    messages: &mut Vec<Value>,
    context_tokens: u32,
    max_new_tokens: usize,
) {
    // Reserve room for the model's reply and chat-template overhead. Use
    // 80% of (context - max_new_tokens) as the soft target so the next
    // round's response has headroom even if its prompt grows slightly.
    let context = context_tokens as usize;
    let reserved = max_new_tokens;
    if context <= reserved {
        return;
    }
    let prompt_budget = context.saturating_sub(reserved);
    let soft_target = prompt_budget * 4 / 5;

    let mut total: usize = messages.iter().map(estimate_message_tokens).sum();
    if total <= soft_target {
        return;
    }

    // Always preserve: index 0 (system), index 1 (initial user prompt).
    // Always preserve: the last `keep_recent` messages.
    let keep_head = 2usize;
    let keep_recent = 6usize;
    loop {
        if total <= soft_target {
            break;
        }
        if messages.len() <= keep_head + keep_recent {
            break;
        }
        // Eviction window starts at index `keep_head`. Evict one assistant
        // message plus any contiguous following `tool` messages so that
        // tool_call/tool_response pairing is preserved.
        let evict_start = keep_head;
        if evict_start >= messages.len().saturating_sub(keep_recent) {
            break;
        }
        let mut evict_end = evict_start + 1;
        while evict_end < messages.len().saturating_sub(keep_recent) {
            let role = messages[evict_end]
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("");
            if role == "tool" {
                evict_end += 1;
            } else {
                break;
            }
        }
        // Subtract estimated tokens for the slice we are about to drain.
        let removed_tokens: usize = messages[evict_start..evict_end]
            .iter()
            .map(estimate_message_tokens)
            .sum();
        messages.drain(evict_start..evict_end);
        total = total.saturating_sub(removed_tokens);
        if removed_tokens == 0 {
            // Defensive: avoid infinite loop if estimator returned zero.
            break;
        }
    }
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
    // noop_marker_compact_1
    {
        return None;
    }
    let path = payload.get("path").and_then(Value::as_str)?;
    let sha256 = payload.get("sha256").and_then(Value::as_str)?;
    Some(json!({
        "path": path,
        "start_line": payload.get("start_line"),
        "end_line": payload.get("end_line"),        "sha256": sha256,
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
                "description": "Preferred final patch submission tool for the inner model coding-agent mode. Replace line ranges from a file returned by open_file_window. You must pass the file sha256 observed from open_file_window so ContextGraph can prove the edit targets the same durable source reality you inspected.",
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
                "the inner model tool call did not include a function name",
                "inner_harness.message.tool_calls[].function.name",
                "inspect the inner model tool-call response format",
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
                "the inner model tool call arguments string was not JSON",
                "inner_harness.message.tool_calls[].function.arguments",
                "inspect model tool-call argument formatting",
                json!({"arguments": text, "error": err.to_string(), "tool_call": call}),
                None,
            )
        }),
        other => Err(RealityError::new(
            "RUST_REALITY_TOOL_CALL_ARGUMENTS_INVALID",
            "the inner model tool call arguments must be a JSON object or JSON object string",
            "inner_harness.message.tool_calls[].function.arguments",
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
                "inner_harness.message.content.arguments",
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
            "inner_harness.message.content",
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

fn pre_action_dynamicjepa_gate(
    ctx: &AgentToolContext<'_>,
    name: &str,
    arguments: &Value,
    last_panel_id: Option<&str>,
    tool_dir: &Path,
) -> Result<Option<Value>> {
    if !is_final_edit_tool(name) {
        return Ok(None);
    }
    let (Some(dynamicjepa_db), Some(artifact_id)) = (
        ctx.args.dynamicjepa_db.as_deref(),
        ctx.args.dynamicjepa_artifact_id.as_deref(),
    ) else {
        return Ok(None);
    };
    let Some(panel_id) = last_panel_id else {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_PREACTION_PANEL_MISSING",
            "DynamicJEPA pre-action control refused to execute a workspace mutation before any real prior panel was persisted",
            "dynamicjepa.pre_action.current_panel_id",
            "run at least one governed read/navigation transition so the planner has a physical current panel before mutation",
            json!({"tool_name": name, "arguments": arguments}),
            Some(file_sot(dynamicjepa_db)),
        ));
    };
    let observed_action = inferred_dynamicjepa_action_from_tool(ctx.repo_dir, name, arguments)?;
    let candidate_action_path = tool_dir.join("dynamicjepa-pending-action.json");
    write_json_checked(&candidate_action_path, &observed_action)?;
    let plan = dynamicjepa_json_command(
        ctx.context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "plan".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--artifact-id".to_string(),
            artifact_id.to_string(),
            "--panel-id".to_string(),
            panel_id.to_string(),
            "--skill-id".to_string(),
            ctx.args.dynamicjepa_skill_id.clone(),
            "--candidate-action-json".to_string(),
            path_str(&candidate_action_path).to_string(),
            "--json".to_string(),
        ],
        ctx.task_root,
        "dynamicjepa_pre_action_plan",
        Some(file_sot(dynamicjepa_db)),
    )?;
    let plan_path = tool_dir.join("dynamicjepa-pre-action-plan.json");
    write_json_checked(&plan_path, &plan)?;
    let control =
        validate_dynamicjepa_action_control(&plan, &observed_action, "pre_action", dynamicjepa_db)?;
    let control = json!({
        "status": "passed",
        "mode": "pre_action_hard_gate",
        "current_panel_id": panel_id,
        "observed_action": observed_action,
        "plan": file_sot(&plan_path),
        "post_action_validator": control,
        "source_of_truth": format!("rocksdb:{}#dj_plan_traces,dj_guard_decisions", dynamicjepa_db.display()),
    });
    let control_path = tool_dir.join("dynamicjepa-pre-action-control.json");
    write_json_checked(&control_path, &control)?;
    Ok(Some(read_json(&control_path)?))
}

fn bootstrap_dynamicjepa_cold_start_panel(
    ctx: &AgentToolContext<'_>,
    attempt: u64,
) -> Result<Option<String>> {
    match (
        ctx.args.dynamicjepa_db.as_deref(),
        ctx.args.dynamicjepa_artifact_id.as_deref(),
    ) {
        (None, None) => return Ok(None),
        (Some(_), Some(_)) => {}
        (db, artifact_id) => {
            return Err(RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_CONFIG_INCOMPLETE",
                "DynamicJEPA cold-start pre-action control requires both a RocksDB path and a model artifact id",
                "dynamicjepa",
                "pass --dynamicjepa-db and --dynamicjepa-artifact-id together or omit both",
                json!({
                    "dynamicjepa_db": db.map(path_str),
                    "dynamicjepa_artifact_id": artifact_id,
                }),
                db.map(file_sot),
            ));
        }
    }
    let dynamicjepa_db = ctx.args.dynamicjepa_db.as_deref().unwrap();
    let artifact_id = ctx.args.dynamicjepa_artifact_id.as_deref().unwrap();
    let live_dir = ctx.attempt_dir.join("dynamicjepa-live");
    create_dir_checked(&live_dir)?;
    let cold_dir = live_dir.join("cold-start");
    create_dir_checked(&cold_dir)?;

    let artifact = dynamicjepa_json_command(
        ctx.context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "get-artifact".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--id".to_string(),
            artifact_id.to_string(),
            "--verify-files".to_string(),
            "--json".to_string(),
        ],
        ctx.task_root,
        "dynamicjepa_cold_start_get_artifact",
        Some(file_sot(dynamicjepa_db)),
    )?;
    write_json_checked(&cold_dir.join("artifact-readback.json"), &artifact)?;
    let artifact_domain = artifact
        .pointer("/decoded_records/artifact/domain_pack_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_COLD_START_ARTIFACT_DOMAIN_MISSING",
                "DynamicJEPA cold-start artifact readback did not contain domain_pack_id",
                "dynamicjepa.get_artifact.decoded_records.artifact.domain_pack_id",
                "inspect the model artifact row in dj_model_artifacts before actor execution",
                json!({"artifact": artifact}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?;
    if artifact_domain != DYNAMICJEPA_REALITY_DOMAIN_PACK {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_COLD_START_DOMAIN_MISMATCH",
            "DynamicJEPA cold-start refused an artifact from a non-SWE-loop domain",
            "dynamicjepa.get_artifact.decoded_records.artifact.domain_pack_id",
            "train or pass an artifact whose domain_pack_id is contextgraph_swe_reality_loop_v1",
            json!({"expected": DYNAMICJEPA_REALITY_DOMAIN_PACK, "actual": artifact_domain, "artifact_id": artifact_id}),
            Some(file_sot(dynamicjepa_db)),
        ));
    }

    let counts_before = dynamicjepa_json_command(
        ctx.context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "inspect-counts".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--json".to_string(),
        ],
        ctx.task_root,
        "dynamicjepa_cold_start_counts_before",
        Some(file_sot(dynamicjepa_db)),
    )?;
    write_json_checked(&cold_dir.join("counts-before.json"), &counts_before)?;

    let seed_paths = dynamicjepa_problem_seed_paths(ctx)?;
    let repo_status = run_required(
        &["git", "status", "--porcelain"],
        ctx.repo_dir,
        ctx.args.git_timeout,
        "RUST_REALITY_DYNAMICJEPA_COLD_START_STATUS_FAILED",
        "dynamicjepa.cold_start.repo_status",
    )?
    .stdout;
    let repo_head = run_required(
        &["git", "rev-parse", "HEAD"],
        ctx.repo_dir,
        ctx.args.git_timeout,
        "RUST_REALITY_DYNAMICJEPA_COLD_START_HEAD_FAILED",
        "dynamicjepa.cold_start.repo_head",
    )?
    .stdout
    .trim()
    .to_string();
    let path_risk = swe_path_risk(&[], &seed_paths);
    let target_path_bucket = swe_path_bucket(&[], &seed_paths);
    let symbol_bucket = swe_symbol_bucket("read_problem_reality", &[], &seed_paths);
    let dependency_layer = swe_dependency_layer(&[], &seed_paths);
    let test_target_kind = swe_test_target_kind("read_problem_reality", None, &[], &seed_paths);
    let ownership_bucket = swe_ownership_bucket(&[], &seed_paths);
    let attempt_i64 = i64::try_from(attempt).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_COLD_START_ATTEMPT_OVERFLOW",
            "attempt number did not fit the SWE DynamicJEPA i64 schema",
            "dynamicjepa.cold_start.state.attempt",
            "inspect the run attempt counter before bootstrapping DynamicJEPA",
            json!({"attempt": attempt, "error": err.to_string()}),
            None,
        )
    })?;
    let ts_ms = attempt_i64.saturating_mul(1_000_000);
    let problem_reality_json = serde_json::to_string(ctx.problem_reality).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_COLD_START_PROBLEM_SERIALIZE_FAILED",
            "problem reality packet could not be serialized for cold-start identity hashing",
            "dynamicjepa.cold_start.problem_reality",
            "inspect problem-reality-packet.json before bootstrapping DynamicJEPA",
            json!({"error": err.to_string()}),
            ctx.problem_reality
                .pointer("/source_of_truth/task_manifest")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        )
    })?;
    let transition_id = format!(
        "cold-start-{}",
        short_hash(&sha256_text(
            &serde_json::to_string(&json!({
                "run_id": ctx.run_id,
                "attempt": attempt,
                "repo_head": repo_head,
                "seed_paths": &seed_paths,
                "problem_reality_sha256": sha256_text(&problem_reality_json),
            }))
            .unwrap()
        ))
    );
    let event = json!({
        "state": {
            "counter": 0,
            "attempt": attempt_i64,
            "round": 0,
            "tool_family": "reality_readback",
            "repo_dirty_state": if repo_status.trim().is_empty() { "clean" } else { "dirty" },
            "prompt_version": "v1",
            "model_id": swe_model_id_variant(&ctx.args.model),
            "primary_path_bucket": target_path_bucket,
            "symbol_bucket": symbol_bucket,
            "dependency_layer": dependency_layer,
            "test_target_kind": test_target_kind,
            "error_signature": "none",
            "ownership_bucket": ownership_bucket
        },
        "action": {
            "delta": 0,
            "tool_family": "reality_readback",
            "command_kind": "read_problem_reality",
            "edit_size_bucket": "none",
            "path_risk": path_risk,
            "target_path_bucket": target_path_bucket,
            "symbol_bucket": symbol_bucket,
            "dependency_layer": dependency_layer,
            "test_target_kind": test_target_kind,
            "ownership_bucket": ownership_bucket,
            "planned_patch_bytes": 0,
            "planned_changed_file_count": 0,
            "patch_intent_bucket": "none",
            "patch_shape_bucket": "none",
            "patch_scope_bucket": "none",
            "ast_language": "none",
            "ast_parse_status": "none",
            "ast_node_kind": "none",
            "ast_symbol_kind": "none",
            "ast_edit_kind": "none",
            "ast_touched_node_count": 0,
            "ast_symbol_count": 0,
            "semantic_provider": "none",
            "semantic_status": "none",
            "symbol_visibility": "none",
            "patch_delta_kind": "none",
            "control_flow_delta": "none",
            "api_contract_delta": "none",
            "dependency_blast_radius": "none",
            "reverse_reference_count": 0,
            "compiler_semantic_status": "none",
            "type_resolution_status": "none",
            "import_resolution_status": "none",
            "compiler_diagnostic_count": 0,
            "test_attribution_status": "none",
            "attributed_test_count": 0,
            "predicted_verifier_target": "none"
        },
        "outcome": {
            "next_counter": 0,
            "outcome_label": "Useful",
            "result_status": "ok",
            "verifier_status": "not_run",
            "test_failure_class": "none",
            "incident_code": "none",
            "changed_file_count": 0,
            "patch_bytes": 0,
            "correction_required": "false",
            "file_risk": path_risk,
            "error_signature": "none",
            "symbol_bucket": symbol_bucket,
            "dependency_layer": dependency_layer,
            "test_target_kind": test_target_kind,
            "ownership_bucket": ownership_bucket
        },
        "ts_ms": ts_ms,
        "trajectory_id": format!("{}:attempt-{}", ctx.run_id, attempt),
        "run_id": ctx.run_id,
        "transition_id": transition_id,
        "cold_start": {
            "source": "problem_reality_packet_and_repository_context",
            "seed_paths": seed_paths,
            "problem_reality": ctx.problem_reality.get("source_of_truth"),
            "repository_context": ctx.repo_context.get("source_of_truth"),
            "repo_head": repo_head
        }
    });
    let event_path = cold_dir.join("event.jsonl");
    write_text_checked(
        &event_path,
        &format!("{}\n", serde_json::to_string(&event).unwrap()),
    )?;
    let ingest = dynamicjepa_json_command(
        ctx.context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "ingest-event".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--domain".to_string(),
            DYNAMICJEPA_REALITY_DOMAIN_PACK.to_string(),
            "--adapter".to_string(),
            "json_event".to_string(),
            "--file".to_string(),
            path_str(&event_path).to_string(),
            "--json".to_string(),
        ],
        ctx.task_root,
        "dynamicjepa_cold_start_ingest_event",
        Some(file_sot(dynamicjepa_db)),
    )?;
    write_json_checked(&cold_dir.join("ingest-readback.json"), &ingest)?;
    if ingest
        .pointer("/created_ids/failed_adapter_run_ids")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
    {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_COLD_START_INGEST_FAILED",
            "DynamicJEPA cold-start event failed adapter validation",
            "dynamicjepa.cold_start.ingest.failed_adapter_run_ids",
            "inspect cold-start event fields against the registered SWE-loop domain pack",
            json!({"ingest": ingest, "event": file_sot(&event_path)}),
            Some(file_sot(dynamicjepa_db)),
        ));
    }
    let transition_ids = required_json_string_array(
        &ingest,
        "/created_ids/transition_ids",
        "dynamicjepa.cold_start.ingest.created_ids.transition_ids",
        dynamicjepa_db,
    )?;
    let action_ids = required_json_string_array(
        &ingest,
        "/created_ids/action_ids",
        "dynamicjepa.cold_start.ingest.created_ids.action_ids",
        dynamicjepa_db,
    )?;
    let outcome_ids = required_json_string_array(
        &ingest,
        "/created_ids/outcome_ids",
        "dynamicjepa.cold_start.ingest.created_ids.outcome_ids",
        dynamicjepa_db,
    )?;
    if transition_ids.len() != 1 || action_ids.len() != 1 || outcome_ids.len() != 1 {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_COLD_START_INGEST_COUNT_MISMATCH",
            "DynamicJEPA cold-start ingest did not create exactly one transition/action/outcome",
            "dynamicjepa.cold_start.ingest.created_ids",
            "inspect cold-start ingestion output before actor execution",
            json!({"ingest": ingest}),
            Some(file_sot(dynamicjepa_db)),
        ));
    }
    let materialized = dynamicjepa_json_command(
        ctx.context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "materialize-panel".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--transition-id".to_string(),
            transition_ids[0].clone(),
            "--json".to_string(),
        ],
        ctx.task_root,
        "dynamicjepa_cold_start_materialize_panel",
        Some(file_sot(dynamicjepa_db)),
    )?;
    write_json_checked(&cold_dir.join("materialize-readback.json"), &materialized)?;
    let panel_ids = required_json_string_array(
        &materialized,
        "/created_ids/panel_ids",
        "dynamicjepa.cold_start.materialize.created_ids.panel_ids",
        dynamicjepa_db,
    )?;
    if panel_ids.len() != 1 {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_COLD_START_PANEL_COUNT_MISMATCH",
            "DynamicJEPA cold-start materialization did not create exactly one panel",
            "dynamicjepa.cold_start.materialize.created_ids.panel_ids",
            "inspect cold-start transition and latent panel rows",
            json!({"materialized": materialized}),
            Some(file_sot(dynamicjepa_db)),
        ));
    }
    let panel = dynamicjepa_json_command(
        ctx.context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "get-panel".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--panel-id".to_string(),
            panel_ids[0].clone(),
            "--include-readings".to_string(),
            "--json".to_string(),
        ],
        ctx.task_root,
        "dynamicjepa_cold_start_get_panel",
        Some(format!(
            "rocksdb:{}#dj_latent_panels",
            dynamicjepa_db.display()
        )),
    )?;
    write_json_checked(&cold_dir.join("panel-readback.json"), &panel)?;
    if panel
        .pointer("/decoded_records/instrument_readings")
        .and_then(Value::as_array)
        .map(|items| items.is_empty())
        .unwrap_or(true)
    {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_COLD_START_PANEL_EMPTY",
            "DynamicJEPA cold-start panel readback contained no instrument readings",
            "dynamicjepa.get_panel.decoded_records.instrument_readings",
            "inspect domain pack instruments and cold-start materialization before actor execution",
            json!({"panel": panel, "panel_id": panel_ids[0]}),
            Some(format!(
                "rocksdb:{}#dj_latent_panels,dj_instrument_readings",
                dynamicjepa_db.display()
            )),
        ));
    }
    let counts_after = dynamicjepa_json_command(
        ctx.context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "inspect-counts".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--json".to_string(),
        ],
        ctx.task_root,
        "dynamicjepa_cold_start_counts_after",
        Some(file_sot(dynamicjepa_db)),
    )?;
    write_json_checked(&cold_dir.join("counts-after.json"), &counts_after)?;
    for cf in [
        "dj_raw_events",
        "dj_adapter_runs",
        "dj_normalized_states",
        "dj_actions",
        "dj_outcomes",
        "dj_transitions",
        "dj_latent_panels",
    ] {
        let before = counts_before
            .pointer(&format!("/counts/{cf}"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let after = counts_after
            .pointer(&format!("/counts/{cf}"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if after != before + 1 {
            return Err(RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_COLD_START_COUNT_DELTA_MISMATCH",
                "DynamicJEPA cold-start source-of-truth count delta did not match one persisted panel event",
                format!("dynamicjepa.cold_start.counts.{cf}"),
                "inspect cold-start ingest/materialize readbacks and RocksDB column-family counts",
                json!({"cf": cf, "before": before, "after": after, "counts_before": file_sot(&cold_dir.join("counts-before.json")), "counts_after": file_sot(&cold_dir.join("counts-after.json"))}),
                Some(file_sot(dynamicjepa_db)),
            ));
        }
    }
    let record = json!({
        "record_kind": "dynamicjepa_cold_start_panel_bootstrap",
        "status": "passed",
        "artifact_id": artifact_id,
        "transition_id": transition_id,
        "dynamicjepa_transition_id": transition_ids[0],
        "action_id": action_ids[0],
        "outcome_id": outcome_ids[0],
        "panel_id": panel_ids[0],
        "event": file_sot(&event_path),
        "ingest": file_sot(&cold_dir.join("ingest-readback.json")),
        "materialize": file_sot(&cold_dir.join("materialize-readback.json")),
        "panel_readback": file_sot(&cold_dir.join("panel-readback.json")),
        "counts_before": file_sot(&cold_dir.join("counts-before.json")),
        "counts_after": file_sot(&cold_dir.join("counts-after.json")),
        "source_of_truth": format!("rocksdb:{}#dj_raw_events,dj_transitions,dj_latent_panels", dynamicjepa_db.display()),
    });
    let record_path = cold_dir.join("record.json");
    write_json_checked(&record_path, &record)?;
    update_dynamicjepa_live_index(&live_dir, &record)?;
    Ok(Some(panel_ids[0].clone()))
}

fn dynamicjepa_problem_seed_paths(ctx: &AgentToolContext<'_>) -> Result<Vec<String>> {
    let mut paths = BTreeSet::new();
    if let Some(files) = ctx.repo_context.get("files").and_then(Value::as_array) {
        for file in files {
            if let Some(path) = file.get("path").and_then(Value::as_str) {
                paths.insert(path.to_string());
            }
        }
    }
    if let Some(candidates) = ctx
        .problem_reality
        .pointer("/repository_reality/production_candidate_files")
        .and_then(Value::as_array)
    {
        for candidate in candidates {
            if let Some(path) = candidate.as_str().or_else(|| {
                candidate
                    .get("path")
                    .or_else(|| candidate.get("file"))
                    .and_then(Value::as_str)
            }) {
                paths.insert(path.to_string());
            }
        }
    }
    if paths.is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_COLD_START_SEED_PATHS_MISSING",
            "DynamicJEPA cold-start could not derive any real repository path from problem reality or repository context",
            "dynamicjepa.cold_start.seed_paths",
            "repair repository context collection before enabling pre-action DJ control",
            json!({
                "repo_context_source": ctx.repo_context.get("source_of_truth"),
                "problem_reality_source": ctx.problem_reality.get("source_of_truth"),
            }),
            ctx.problem_reality
                .pointer("/source_of_truth/task_manifest")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        ));
    }
    Ok(paths.into_iter().take(16).collect())
}

fn live_ingest_dynamicjepa_transition(
    ctx: &AgentToolContext<'_>,
    attempt: u64,
    harness_transition_path: &Path,
    tool_dir: &Path,
) -> Result<Option<Value>> {
    let (Some(dynamicjepa_db), Some(_artifact_id)) = (
        ctx.args.dynamicjepa_db.as_deref(),
        ctx.args.dynamicjepa_artifact_id.as_deref(),
    ) else {
        return Ok(None);
    };
    let live_dir = ctx.attempt_dir.join("dynamicjepa-live");
    create_dir_checked(&live_dir)?;
    let live_name = tool_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("tool");
    let mini_transcript_path = live_dir.join(format!("{live_name}-transcript.json"));
    let mini_transcript = json!({
        "record_kind": "contextgraph_agent_tool_transcript",
        "attempt": attempt,
        "model": ctx.args.model,
        "entries": [{
            "round": 0,
            "status": "dynamicjepa_live_single_transition",
            "tool_results": [{
                "harness_transition": file_sot(harness_transition_path),
            }]
        }],
        "executed_tool_calls": 1,
        "source_of_truth": file_sot(&mini_transcript_path),
    });
    write_json_checked(&mini_transcript_path, &mini_transcript)?;
    let events = dynamicjepa_swe_events_from_transcript(
        ctx.run_id,
        attempt,
        &mini_transcript_path,
        &mini_transcript,
        ctx.repo_dir,
    )?;
    if events.len() != 1 {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_LIVE_EVENT_COUNT_MISMATCH",
            "live DynamicJEPA pre-action ingestion expected exactly one event for one harness transition",
            "dynamicjepa.live.events",
            "inspect the live transcript builder before allowing pre-action planning",
            json!({"event_count": events.len(), "transcript": file_sot(&mini_transcript_path)}),
            Some(file_sot(&mini_transcript_path)),
        ));
    }
    let event_path = live_dir.join(format!("{live_name}-event.jsonl"));
    let event_json = serde_json::to_string(&events[0].event).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_LIVE_EVENT_SERIALIZE_FAILED",
            "live DynamicJEPA event could not be serialized",
            "dynamicjepa.live.event",
            "inspect the transition-to-domain mapping code",
            json!({"error": err.to_string(), "event": events[0].event}),
            Some(file_sot(&mini_transcript_path)),
        )
    })?;
    write_text_checked(&event_path, &format!("{event_json}\n"))?;
    let ingest = dynamicjepa_json_command(
        ctx.context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "ingest-event".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--domain".to_string(),
            DYNAMICJEPA_REALITY_DOMAIN_PACK.to_string(),
            "--adapter".to_string(),
            "json_event".to_string(),
            "--file".to_string(),
            path_str(&event_path).to_string(),
            "--json".to_string(),
        ],
        ctx.task_root,
        "dynamicjepa_live_ingest_event",
        Some(file_sot(dynamicjepa_db)),
    )?;
    let ingest_path = live_dir.join(format!("{live_name}-ingest.json"));
    write_json_checked(&ingest_path, &ingest)?;
    if ingest
        .pointer("/created_ids/failed_adapter_run_ids")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
    {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_LIVE_INGEST_ADAPTER_FAILED",
            "live DynamicJEPA ingestion produced failed adapter runs before pre-action planning",
            "dynamicjepa.live.ingest.failed_adapter_run_ids",
            "inspect the live event and registered SWE domain pack mapping",
            json!({"ingest": ingest, "event": file_sot(&event_path)}),
            Some(file_sot(dynamicjepa_db)),
        ));
    }
    let transition_ids = required_json_string_array(
        &ingest,
        "/created_ids/transition_ids",
        "dynamicjepa.live.ingest.created_ids.transition_ids",
        dynamicjepa_db,
    )?;
    let action_ids = required_json_string_array(
        &ingest,
        "/created_ids/action_ids",
        "dynamicjepa.live.ingest.created_ids.action_ids",
        dynamicjepa_db,
    )?;
    let outcome_ids = required_json_string_array(
        &ingest,
        "/created_ids/outcome_ids",
        "dynamicjepa.live.ingest.created_ids.outcome_ids",
        dynamicjepa_db,
    )?;
    if transition_ids.len() != 1 || action_ids.len() != 1 || outcome_ids.len() != 1 {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_LIVE_INGEST_COUNT_MISMATCH",
            "live DynamicJEPA ingest did not create exactly one transition/action/outcome",
            "dynamicjepa.live.ingest.created_ids",
            "inspect live ingestion output before pre-action planning",
            json!({"ingest": ingest}),
            Some(file_sot(dynamicjepa_db)),
        ));
    }
    let materialized = dynamicjepa_json_command(
        ctx.context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "materialize-panel".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--transition-id".to_string(),
            transition_ids[0].clone(),
            "--json".to_string(),
        ],
        ctx.task_root,
        "dynamicjepa_live_materialize_panel",
        Some(file_sot(dynamicjepa_db)),
    )?;
    let materialize_path = live_dir.join(format!("{live_name}-materialize.json"));
    write_json_checked(&materialize_path, &materialized)?;
    let panel_ids = required_json_string_array(
        &materialized,
        "/created_ids/panel_ids",
        "dynamicjepa.live.materialize.created_ids.panel_ids",
        dynamicjepa_db,
    )?;
    if panel_ids.len() != 1 {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_LIVE_PANEL_COUNT_MISMATCH",
            "live DynamicJEPA materialization did not create exactly one panel",
            "dynamicjepa.live.materialize.created_ids.panel_ids",
            "inspect live transition id and latent panel source-of-truth rows",
            json!({"materialized": materialized}),
            Some(file_sot(dynamicjepa_db)),
        ));
    }
    let record = json!({
        "transition_id": events[0].transition_id,
        "dynamicjepa_transition_id": transition_ids[0],
        "action_id": action_ids[0],
        "outcome_id": outcome_ids[0],
        "panel_id": panel_ids[0],
        "event": file_sot(&event_path),
        "ingest": file_sot(&ingest_path),
        "materialize": file_sot(&materialize_path),
        "source_of_truth": format!("rocksdb:{}#dj_raw_events,dj_transitions,dj_latent_panels", dynamicjepa_db.display()),
    });
    let record_path = live_dir.join(format!("{live_name}-record.json"));
    write_json_checked(&record_path, &record)?;
    update_dynamicjepa_live_index(&live_dir, &record)?;
    Ok(Some(read_json(&record_path)?))
}

fn update_dynamicjepa_live_index(live_dir: &Path, record: &Value) -> Result<()> {
    let index_path = live_dir.join("index.json");
    let mut records = if index_path.exists() {
        let existing: Value = read_json(&index_path)?;
        existing
            .get("records")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    records.push(record.clone());
    write_json_checked(
        &index_path,
        &json!({
            "record_kind": "dynamicjepa_live_preaction_index",
            "records": records,
            "source_of_truth": file_sot(&index_path),
        }),
    )
}

fn inferred_dynamicjepa_action_from_tool(
    repo_dir: &Path,
    name: &str,
    arguments: &Value,
) -> Result<Value> {
    let paths = dynamicjepa_tool_argument_paths(name, arguments);
    let patch_bytes = dynamicjepa_tool_argument_patch_bytes(name, arguments)?;
    let edit_size_bucket = swe_edit_size_bucket(patch_bytes);
    let path_risk = swe_path_risk(&paths, &[]);
    let target_path_bucket = swe_path_bucket(&paths, &[]);
    let ast_semantics = dynamicjepa_ast_edit_semantics(repo_dir, name, arguments, &paths)?;
    let planned_changed_file_count = i64::try_from(paths.len()).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_PREACTION_PATH_COUNT_OVERFLOW",
            "pre-action DynamicJEPA planned path count did not fit the SWE domain schema",
            "dynamicjepa.pre_action.action.planned_changed_file_count",
            "inspect the tool arguments before planner gating",
            json!({"tool_name": name, "path_count": paths.len(), "error": err.to_string()}),
            None,
        )
    })?;
    let delta = match name {
        "apply_line_window_edit" => 7,
        "apply_structured_edit" => 6,
        "apply_unified_diff" if edit_size_bucket == "large" || path_risk == "multiple" => -4,
        "apply_unified_diff" => 5,
        _ => 0,
    };
    Ok(json!({
        "delta": delta,
        "tool_family": swe_tool_family_variant(harness_tool_family(name)),
        "command_kind": swe_command_kind_variant(name),
        "edit_size_bucket": edit_size_bucket,
        "path_risk": path_risk,
        "target_path_bucket": target_path_bucket,
        "symbol_bucket": swe_symbol_bucket_from_ast_or_path(name, &paths, &[], &ast_semantics),
        "dependency_layer": swe_dependency_layer(&paths, &[]),
        "test_target_kind": swe_test_target_kind(name, None, &paths, &[]),
        "ownership_bucket": swe_ownership_bucket(&paths, &[]),
        "planned_patch_bytes": patch_bytes,
        "planned_changed_file_count": planned_changed_file_count,
        "patch_intent_bucket": swe_patch_intent_bucket(name, patch_bytes, path_risk),
        "patch_shape_bucket": swe_patch_shape_bucket(name, arguments, &paths),
        "patch_scope_bucket": swe_patch_scope_bucket_from_ast_or_path(name, &paths, &ast_semantics),
        "ast_language": ast_semantics.language,
        "ast_parse_status": ast_semantics.parse_status,
        "ast_node_kind": ast_semantics.node_kind,
        "ast_symbol_kind": ast_semantics.symbol_kind,
        "ast_edit_kind": ast_semantics.edit_kind,
        "ast_touched_node_count": ast_semantics.touched_node_count,
        "ast_symbol_count": ast_semantics.symbol_count,
        "semantic_provider": ast_semantics.semantic_provider,
        "semantic_status": ast_semantics.semantic_status,
        "symbol_visibility": ast_semantics.symbol_visibility,
        "patch_delta_kind": ast_semantics.patch_delta_kind,
        "control_flow_delta": ast_semantics.control_flow_delta,
        "api_contract_delta": ast_semantics.api_contract_delta,
        "dependency_blast_radius": ast_semantics.dependency_blast_radius,
        "reverse_reference_count": ast_semantics.reverse_reference_count,
        "compiler_semantic_status": ast_semantics.compiler_semantic_status,
        "type_resolution_status": ast_semantics.type_resolution_status,
        "import_resolution_status": ast_semantics.import_resolution_status,
        "compiler_diagnostic_count": ast_semantics.compiler_diagnostic_count,
        "test_attribution_status": ast_semantics.test_attribution_status,
        "attributed_test_count": ast_semantics.attributed_test_count,
        "predicted_verifier_target": ast_semantics.predicted_verifier_target,
    }))
}

fn dynamicjepa_tool_argument_patch_bytes(name: &str, arguments: &Value) -> Result<i64> {
    let bytes = match name {
        "apply_unified_diff" => arguments
            .get("patch")
            .and_then(Value::as_str)
            .map(str::len)
            .unwrap_or(0),
        "apply_line_window_edit" | "apply_structured_edit" => arguments
            .get("edits")
            .and_then(Value::as_array)
            .map(|edits| {
                edits
                    .iter()
                    .filter_map(|edit| {
                        edit.get("replace")
                            .or_else(|| edit.get("replacement"))
                            .and_then(Value::as_str)
                    })
                    .map(str::len)
                    .sum::<usize>()
            })
            .unwrap_or(0),
        _ => 0,
    };
    i64::try_from(bytes).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_PREACTION_PATCH_SIZE_OVERFLOW",
            "pre-action DynamicJEPA patch size did not fit the SWE domain schema",
            "dynamicjepa.pre_action.action.patch_bytes",
            "inspect the tool arguments before planner gating",
            json!({"tool_name": name, "bytes": bytes, "error": err.to_string()}),
            None,
        )
    })
}

fn dynamicjepa_tool_argument_paths(name: &str, arguments: &Value) -> Vec<String> {
    let mut paths = BTreeSet::new();
    if let Some(edits) = arguments.get("edits").and_then(Value::as_array) {
        for edit in edits {
            if let Some(path) = edit.get("path").and_then(Value::as_str) {
                paths.insert(path.to_string());
            }
        }
    }
    if let Some(observed) = arguments.get("observed_files").and_then(Value::as_array) {
        for item in observed {
            if let Some(path) = item.get("path").and_then(Value::as_str) {
                paths.insert(path.to_string());
            }
        }
    }
    if name == "apply_unified_diff" {
        if let Some(patch) = arguments.get("patch").and_then(Value::as_str) {
            for line in patch.lines() {
                for prefix in ["+++ b/", "--- a/"] {
                    if let Some(path) = line.strip_prefix(prefix) {
                        if path != "/dev/null" {
                            paths.insert(path.to_string());
                        }
                    }
                }
            }
        }
    }
    paths.into_iter().collect()
}

#[derive(Debug, Clone)]
struct SweAstEditSemantics {
    language: &'static str,
    parse_status: &'static str,
    node_kind: &'static str,
    symbol_kind: &'static str,
    edit_kind: &'static str,
    touched_node_count: i64,
    symbol_count: i64,
    semantic_provider: &'static str,
    semantic_status: &'static str,
    symbol_visibility: &'static str,
    patch_delta_kind: &'static str,
    control_flow_delta: &'static str,
    api_contract_delta: &'static str,
    dependency_blast_radius: &'static str,
    reverse_reference_count: i64,
    compiler_semantic_status: &'static str,
    type_resolution_status: &'static str,
    import_resolution_status: &'static str,
    compiler_diagnostic_count: i64,
    test_attribution_status: &'static str,
    attributed_test_count: i64,
    predicted_verifier_target: &'static str,
}

impl SweAstEditSemantics {
    fn none() -> Self {
        Self {
            language: "none",
            parse_status: "none",
            node_kind: "none",
            symbol_kind: "none",
            edit_kind: "none",
            touched_node_count: 0,
            symbol_count: 0,
            semantic_provider: "none",
            semantic_status: "none",
            symbol_visibility: "none",
            patch_delta_kind: "none",
            control_flow_delta: "none",
            api_contract_delta: "none",
            dependency_blast_radius: "none",
            reverse_reference_count: 0,
            compiler_semantic_status: "none",
            type_resolution_status: "none",
            import_resolution_status: "none",
            compiler_diagnostic_count: 0,
            test_attribution_status: "none",
            attributed_test_count: 0,
            predicted_verifier_target: "none",
        }
    }
}

#[derive(Debug, Clone)]
struct SweCompilerSemanticEvidence {
    semantic_provider: &'static str,
    compiler_semantic_status: &'static str,
    type_resolution_status: &'static str,
    import_resolution_status: &'static str,
    diagnostic_count: i64,
    predicted_verifier_target: &'static str,
}

impl SweCompilerSemanticEvidence {
    fn none() -> Self {
        Self {
            semantic_provider: "parser_symbol_graph",
            compiler_semantic_status: "none",
            type_resolution_status: "none",
            import_resolution_status: "none",
            diagnostic_count: 0,
            predicted_verifier_target: "none",
        }
    }
}

#[derive(Debug, Clone)]
struct SweTestAttributionEvidence {
    status: &'static str,
    test_count: i64,
}

fn dynamicjepa_ast_edit_semantics(
    repo_dir: &Path,
    tool_name: &str,
    arguments: &Value,
    paths: &[String],
) -> Result<SweAstEditSemantics> {
    if !is_final_edit_tool(tool_name) {
        return Ok(SweAstEditSemantics::none());
    }
    if paths.is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_AST_TARGET_PATHS_MISSING",
            "DynamicJEPA AST action semantics could not identify any target path for a mutating tool call",
            "dynamicjepa.action.ast.paths",
            "include governed edit path evidence in the tool arguments before pre-action planning or ingestion",
            json!({"tool_name": tool_name, "arguments": arguments}),
            Some(file_sot(repo_dir)),
        ));
    }

    let mut per_path = Vec::new();
    for path in paths {
        per_path.push(dynamicjepa_ast_semantics_for_path(
            repo_dir, tool_name, arguments, path,
        )?);
    }
    let languages = per_path
        .iter()
        .map(|sem| sem.language)
        .collect::<BTreeSet<_>>();
    let node_kinds = per_path
        .iter()
        .map(|sem| sem.node_kind)
        .collect::<BTreeSet<_>>();
    let symbol_kinds = per_path
        .iter()
        .map(|sem| sem.symbol_kind)
        .collect::<BTreeSet<_>>();
    let edit_kinds = per_path
        .iter()
        .map(|sem| sem.edit_kind)
        .collect::<BTreeSet<_>>();
    let semantic_providers = per_path
        .iter()
        .map(|sem| sem.semantic_provider)
        .collect::<BTreeSet<_>>();
    let semantic_statuses = per_path
        .iter()
        .map(|sem| sem.semantic_status)
        .collect::<BTreeSet<_>>();
    let symbol_visibilities = per_path
        .iter()
        .map(|sem| sem.symbol_visibility)
        .collect::<BTreeSet<_>>();
    let patch_delta_kinds = per_path
        .iter()
        .map(|sem| sem.patch_delta_kind)
        .collect::<BTreeSet<_>>();
    let control_flow_deltas = per_path
        .iter()
        .map(|sem| sem.control_flow_delta)
        .collect::<BTreeSet<_>>();
    let api_contract_deltas = per_path
        .iter()
        .map(|sem| sem.api_contract_delta)
        .collect::<BTreeSet<_>>();
    let dependency_blast_radii = per_path
        .iter()
        .map(|sem| sem.dependency_blast_radius)
        .collect::<BTreeSet<_>>();
    let compiler_semantic_statuses = per_path
        .iter()
        .map(|sem| sem.compiler_semantic_status)
        .collect::<BTreeSet<_>>();
    let type_resolution_statuses = per_path
        .iter()
        .map(|sem| sem.type_resolution_status)
        .collect::<BTreeSet<_>>();
    let import_resolution_statuses = per_path
        .iter()
        .map(|sem| sem.import_resolution_status)
        .collect::<BTreeSet<_>>();
    let test_attribution_statuses = per_path
        .iter()
        .map(|sem| sem.test_attribution_status)
        .collect::<BTreeSet<_>>();
    let predicted_verifier_targets = per_path
        .iter()
        .map(|sem| sem.predicted_verifier_target)
        .collect::<BTreeSet<_>>();
    Ok(SweAstEditSemantics {
        language: collapse_ast_category(&languages, "mixed"),
        parse_status: if per_path.iter().any(|sem| sem.parse_status == "parsed") {
            "parsed"
        } else {
            collapse_ast_category(
                &per_path
                    .iter()
                    .map(|sem| sem.parse_status)
                    .collect::<BTreeSet<_>>(),
                "none",
            )
        },
        node_kind: collapse_ast_category(&node_kinds, "mixed"),
        symbol_kind: collapse_ast_category(&symbol_kinds, "mixed"),
        edit_kind: collapse_ast_category(&edit_kinds, "mixed"),
        touched_node_count: per_path
            .iter()
            .map(|sem| sem.touched_node_count)
            .sum::<i64>(),
        symbol_count: per_path.iter().map(|sem| sem.symbol_count).sum::<i64>(),
        semantic_provider: collapse_ast_category(&semantic_providers, "mixed"),
        semantic_status: if per_path.iter().any(|sem| sem.semantic_status == "analyzed") {
            "analyzed"
        } else {
            collapse_ast_category(&semantic_statuses, "none")
        },
        symbol_visibility: collapse_ast_category(&symbol_visibilities, "mixed"),
        patch_delta_kind: collapse_ast_category(&patch_delta_kinds, "mixed"),
        control_flow_delta: collapse_ast_category(&control_flow_deltas, "mixed"),
        api_contract_delta: collapse_ast_category(&api_contract_deltas, "mixed"),
        dependency_blast_radius: collapse_ast_category(&dependency_blast_radii, "mixed"),
        reverse_reference_count: per_path
            .iter()
            .map(|sem| sem.reverse_reference_count)
            .sum::<i64>(),
        compiler_semantic_status: collapse_ast_category(&compiler_semantic_statuses, "mixed"),
        type_resolution_status: collapse_ast_category(&type_resolution_statuses, "mixed"),
        import_resolution_status: collapse_ast_category(&import_resolution_statuses, "mixed"),
        compiler_diagnostic_count: per_path
            .iter()
            .map(|sem| sem.compiler_diagnostic_count)
            .sum::<i64>(),
        test_attribution_status: collapse_ast_category(&test_attribution_statuses, "mixed"),
        attributed_test_count: per_path
            .iter()
            .map(|sem| sem.attributed_test_count)
            .sum::<i64>(),
        predicted_verifier_target: collapse_ast_category(&predicted_verifier_targets, "mixed"),
    })
}

fn collapse_ast_category(values: &BTreeSet<&'static str>, mixed: &'static str) -> &'static str {
    let meaningful = values
        .iter()
        .copied()
        .filter(|value| *value != "none")
        .collect::<BTreeSet<_>>();
    if meaningful.is_empty() {
        "none"
    } else if meaningful.len() == 1 {
        meaningful.iter().next().copied().unwrap_or("none")
    } else {
        mixed
    }
}

fn dynamicjepa_ast_semantics_for_path(
    repo_dir: &Path,
    tool_name: &str,
    arguments: &Value,
    repo_relative_path: &str,
) -> Result<SweAstEditSemantics> {
    ensure_dynamicjepa_ast_safe_repo_path(repo_relative_path, repo_dir)?;
    if swe_path_bucket(&[repo_relative_path.to_string()], &[]) == "config_or_build" {
        return Ok(SweAstEditSemantics {
            language: "none",
            parse_status: "none",
            node_kind: "config",
            symbol_kind: "config_key",
            edit_kind: "config",
            touched_node_count: 1,
            symbol_count: 1,
            semantic_provider: "parser_symbol_graph",
            semantic_status: "analyzed",
            symbol_visibility: "none",
            patch_delta_kind: "config",
            control_flow_delta: "none",
            api_contract_delta: "none",
            dependency_blast_radius: "single_file",
            reverse_reference_count: 0,
            compiler_semantic_status: "none",
            type_resolution_status: "none",
            import_resolution_status: "none",
            compiler_diagnostic_count: 0,
            test_attribution_status: "inferred_from_path",
            attributed_test_count: 0,
            predicted_verifier_target: "static_check",
        });
    }
    let Some(path_language) = swe_ast_language_for_path(repo_relative_path) else {
        if swe_path_risk(&[repo_relative_path.to_string()], &[]) == "source" {
            return Err(RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_AST_UNSUPPORTED_SOURCE_LANGUAGE",
                "DynamicJEPA AST action semantics refused a source edit for a language without a parser-backed extractor",
                "dynamicjepa.action.ast_language",
                "add a tree-sitter grammar and language-specific extractor before using this source language for ME-JEPA action control",
                json!({"path": repo_relative_path, "tool_name": tool_name}),
                Some(format!("git:{}#{}", repo_dir.display(), repo_relative_path)),
            ));
        }
        return Ok(SweAstEditSemantics::none());
    };

    let source_path = repo_dir.join(repo_relative_path);
    let source = fs::read_to_string(&source_path).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_AST_SOURCE_READ_FAILED",
            "DynamicJEPA AST action semantics could not read the target source file",
            "dynamicjepa.action.ast.source_file",
            "inspect the governed edit path and repository checkout before pre-action planning",
            json!({"path": repo_relative_path, "error": err.to_string()}),
            Some(file_sot(&source_path)),
        )
    })?;
    let language =
        swe_ast_language_for_path_and_source(repo_relative_path, &source).unwrap_or(path_language);
    let ranges =
        dynamicjepa_ast_byte_ranges_for_path(tool_name, arguments, repo_relative_path, &source)?;
    if ranges.is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_AST_EDIT_RANGE_MISSING",
            "DynamicJEPA AST action semantics could not resolve any concrete edit range in the target source file",
            "dynamicjepa.action.ast.edit_range",
            "provide line ranges, exact find spans, or unified-diff hunks that map to the real checked-out file",
            json!({"path": repo_relative_path, "tool_name": tool_name}),
            Some(file_sot(&source_path)),
        ));
    }

    let mut parser = tree_sitter::Parser::new();
    let language_fn: tree_sitter::Language = match language {
        "python" => tree_sitter_python::LANGUAGE.into(),
        "rust" => tree_sitter_rust::LANGUAGE.into(),
        "javascript" => tree_sitter_javascript::LANGUAGE.into(),
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "tsx" => tree_sitter_typescript::LANGUAGE_TSX.into(),
        "go" => tree_sitter_go::LANGUAGE.into(),
        "java" => tree_sitter_java::LANGUAGE.into(),
        "c" => tree_sitter_c::LANGUAGE.into(),
        "cpp" => tree_sitter_cpp::LANGUAGE.into(),
        "csharp" => tree_sitter_c_sharp::LANGUAGE.into(),
        "ruby" => tree_sitter_ruby::LANGUAGE.into(),
        "php" => tree_sitter_php::LANGUAGE_PHP_ONLY.into(),
        _ => unreachable!("language is constrained by swe_ast_language_for_path"),
    };
    parser.set_language(&language_fn).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_AST_LANGUAGE_SET_FAILED",
            "DynamicJEPA AST parser could not set the requested tree-sitter language",
            "dynamicjepa.action.ast.parser.language",
            "verify tree-sitter grammar crate versions before training or pre-action use",
            json!({"language": language, "path": repo_relative_path, "error": err.to_string()}),
            Some(file_sot(&source_path)),
        )
    })?;
    let tree = parser.parse(&source, None).ok_or_else(|| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_AST_PARSE_RETURNED_NONE",
            "tree-sitter returned no syntax tree for a source file required by DynamicJEPA action control",
            "dynamicjepa.action.ast.parser.tree",
            "inspect parser grammar compatibility and the source file bytes",
            json!({"language": language, "path": repo_relative_path}),
            Some(file_sot(&source_path)),
        )
    })?;
    let root = tree.root_node();
    if root.has_error() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_AST_PARSE_SYNTAX_ERROR",
            "DynamicJEPA AST action semantics refused a source file with tree-sitter syntax errors",
            "dynamicjepa.action.ast_parse_status",
            "fix or exclude syntactically invalid source before training or pre-action control; parser-backed semantics must not be guessed",
            json!({"language": language, "path": repo_relative_path}),
            Some(file_sot(&source_path)),
        ));
    }

    let mut node_kinds = BTreeSet::new();
    let mut symbol_kinds = BTreeSet::new();
    let mut edit_kinds = BTreeSet::new();
    let mut symbol_visibilities = BTreeSet::new();
    let mut touched_symbols = BTreeSet::new();
    let mut changed_text = String::new();
    let mut touched = 0i64;
    for range in &ranges {
        let start = range.0.min(source.len());
        let end_exclusive = range
            .1
            .min(source.len())
            .max(start.saturating_add(1).min(source.len()));
        let (node_start, node_end_exclusive) =
            ast_significant_byte_range(&source, start, end_exclusive)
                .unwrap_or((start, end_exclusive));
        let node_end = node_end_exclusive.saturating_sub(1).max(node_start);
        let node = root
            .descendant_for_byte_range(node_start, node_end)
            .unwrap_or(root);
        let semantic = dynamicjepa_ast_semantic_node(language, node, &source, repo_relative_path);
        node_kinds.insert(semantic.node_kind);
        symbol_kinds.insert(semantic.symbol_kind);
        symbol_visibilities.insert(semantic.visibility);
        if let Some(name) = semantic.symbol_name {
            touched_symbols.insert(name);
        }
        edit_kinds.insert(dynamicjepa_ast_edit_kind_for_range(
            semantic.node_kind,
            semantic.node_start_line,
            range.2,
        ));
        changed_text.push_str(&source[start..end_exclusive.min(source.len())]);
        changed_text.push('\n');
        touched += 1;
    }
    let after_source =
        dynamicjepa_ast_after_source_for_path(tool_name, arguments, repo_relative_path, &source)?;
    if let Some(after) = after_source.as_deref() {
        changed_text.push_str(after);
    } else if tool_name == "apply_unified_diff" {
        changed_text.push_str(arguments.get("patch").and_then(Value::as_str).unwrap_or(""));
    }
    let reverse_reference_count =
        dynamicjepa_reverse_reference_count(repo_dir, repo_relative_path, &touched_symbols)?;
    let compiler_evidence = dynamicjepa_compiler_lsp_semantics_for_path(
        repo_dir,
        tool_name,
        arguments,
        repo_relative_path,
        language,
        after_source.as_deref(),
    )?;
    let test_attribution =
        dynamicjepa_test_attribution_for_path(repo_dir, repo_relative_path, language, &ranges)?;
    let node_kind = collapse_ast_category(&node_kinds, "mixed");
    let symbol_visibility = collapse_ast_category(&symbol_visibilities, "mixed");
    let edit_kind = collapse_ast_category(&edit_kinds, "mixed");

    Ok(SweAstEditSemantics {
        language,
        parse_status: "parsed",
        node_kind,
        symbol_kind: collapse_ast_category(&symbol_kinds, "mixed"),
        edit_kind,
        touched_node_count: touched,
        symbol_count: symbol_kinds.iter().filter(|kind| **kind != "none").count() as i64,
        semantic_provider: compiler_evidence.semantic_provider,
        semantic_status: "analyzed",
        symbol_visibility,
        patch_delta_kind: dynamicjepa_patch_delta_kind(
            repo_relative_path,
            node_kind,
            edit_kind,
            symbol_visibility,
            &changed_text,
        ),
        control_flow_delta: dynamicjepa_control_flow_delta(&changed_text),
        api_contract_delta: dynamicjepa_api_contract_delta(
            edit_kind,
            symbol_visibility,
            &changed_text,
        ),
        dependency_blast_radius: dynamicjepa_dependency_blast_radius(
            repo_relative_path,
            reverse_reference_count,
        ),
        reverse_reference_count,
        compiler_semantic_status: compiler_evidence.compiler_semantic_status,
        type_resolution_status: compiler_evidence.type_resolution_status,
        import_resolution_status: compiler_evidence.import_resolution_status,
        compiler_diagnostic_count: compiler_evidence.diagnostic_count,
        test_attribution_status: test_attribution.status,
        attributed_test_count: test_attribution.test_count,
        predicted_verifier_target: compiler_evidence.predicted_verifier_target,
    })
}

fn ensure_dynamicjepa_ast_safe_repo_path(path: &str, repo_dir: &Path) -> Result<()> {
    let path_obj = Path::new(path);
    if path_obj.is_absolute()
        || path_obj
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_AST_PATH_OUTSIDE_REPO",
            "DynamicJEPA AST action semantics refused a path outside the governed repository",
            "dynamicjepa.action.ast.path",
            "use repo-relative governed file paths only",
            json!({"path": path}),
            Some(file_sot(repo_dir)),
        ));
    }
    Ok(())
}

fn swe_ast_language_for_path(path: &str) -> Option<&'static str> {
    if path.ends_with(".py") {
        Some("python")
    } else if path.ends_with(".rs") {
        Some("rust")
    } else if path.ends_with(".jsx")
        || path.ends_with(".js")
        || path.ends_with(".mjs")
        || path.ends_with(".cjs")
    {
        Some("javascript")
    } else if path.ends_with(".tsx") {
        Some("tsx")
    } else if path.ends_with(".ts") || path.ends_with(".mts") || path.ends_with(".cts") {
        Some("typescript")
    } else if path.ends_with(".go") {
        Some("go")
    } else if path.ends_with(".java") {
        Some("java")
    } else if path.ends_with(".c") {
        Some("c")
    } else if path.ends_with(".h") {
        Some("c")
    } else if path.ends_with(".cc")
        || path.ends_with(".cpp")
        || path.ends_with(".cxx")
        || path.ends_with(".hpp")
        || path.ends_with(".hh")
        || path.ends_with(".hxx")
        || path.ends_with(".h++")
        || path.ends_with(".ipp")
        || path.ends_with(".inl")
        || path.ends_with(".tpp")
        || path.ends_with(".txx")
        || path.ends_with(".ixx")
    {
        Some("cpp")
    } else if path.ends_with(".cs") {
        Some("csharp")
    } else if path.ends_with(".rb") {
        Some("ruby")
    } else if path.ends_with(".php") {
        Some("php")
    } else {
        None
    }
}

fn swe_ast_language_for_path_and_source(path: &str, source: &str) -> Option<&'static str> {
    if path.ends_with(".h") && source_contains_cpp_only_syntax(source) {
        return Some("cpp");
    }
    swe_ast_language_for_path(path)
}

fn source_contains_cpp_only_syntax(source: &str) -> bool {
    let compact = source.replace('\n', " ");
    contains_any(
        &compact,
        &[
            "namespace ",
            "template <",
            "template<",
            "class ",
            "std::",
            "using namespace ",
            "public:",
            "private:",
            "protected:",
            "constexpr ",
            "nullptr",
            "noexcept",
            "override",
            "typename ",
            "operator ",
        ],
    ) || source.contains("::")
}

fn dynamicjepa_ast_byte_ranges_for_path(
    tool_name: &str,
    arguments: &Value,
    path: &str,
    source: &str,
) -> Result<Vec<(usize, usize, u32)>> {
    match tool_name {
        "apply_line_window_edit" => dynamicjepa_ast_line_edit_ranges(arguments, path, source),
        "apply_structured_edit" => dynamicjepa_ast_structured_edit_ranges(arguments, path, source),
        "apply_unified_diff" => dynamicjepa_ast_unified_diff_ranges(arguments, path, source),
        _ => Ok(Vec::new()),
    }
}

fn dynamicjepa_ast_line_edit_ranges(
    arguments: &Value,
    path: &str,
    source: &str,
) -> Result<Vec<(usize, usize, u32)>> {
    let edits = arguments
        .get("edits")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_AST_LINE_EDITS_MISSING",
                "line-window AST extraction requires an edits array",
                "dynamicjepa.action.ast.line_window.edits",
                "provide apply_line_window_edit arguments with path/start_line/end_line",
                json!({"arguments": arguments}),
                None,
            )
        })?;
    let mut ranges = Vec::new();
    for edit in edits {
        if edit.get("path").and_then(Value::as_str) != Some(path) {
            continue;
        }
        let start_line = edit
            .get("start_line")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                RealityError::new(
                    "RUST_REALITY_DYNAMICJEPA_AST_START_LINE_MISSING",
                    "line-window AST extraction requires start_line",
                    "dynamicjepa.action.ast.line_window.start_line",
                    "provide exact governed line-window edit coordinates",
                    json!({"path": path, "edit": edit}),
                    None,
                )
            })?;
        let end_line = edit
            .get("end_line")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                RealityError::new(
                    "RUST_REALITY_DYNAMICJEPA_AST_END_LINE_MISSING",
                    "line-window AST extraction requires end_line",
                    "dynamicjepa.action.ast.line_window.end_line",
                    "provide exact governed line-window edit coordinates",
                    json!({"path": path, "edit": edit}),
                    None,
                )
            })?;
        ranges.push(line_range_to_byte_range(
            source, start_line, end_line, path,
        )?);
    }
    Ok(ranges)
}

fn dynamicjepa_ast_structured_edit_ranges(
    arguments: &Value,
    path: &str,
    source: &str,
) -> Result<Vec<(usize, usize, u32)>> {
    let edits = arguments
        .get("edits")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_AST_STRUCTURED_EDITS_MISSING",
                "structured AST extraction requires an edits array",
                "dynamicjepa.action.ast.structured.edits",
                "provide apply_structured_edit arguments with exact find spans",
                json!({"arguments": arguments}),
                None,
            )
        })?;
    let mut ranges = Vec::new();
    for edit in edits {
        if edit.get("path").and_then(Value::as_str) != Some(path) {
            continue;
        }
        let find = edit.get("find").and_then(Value::as_str).ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_AST_FIND_SPAN_MISSING",
                "structured AST extraction requires an exact find span",
                "dynamicjepa.action.ast.structured.find",
                "provide the exact source span copied from a governed source window",
                json!({"path": path, "edit": edit}),
                None,
            )
        })?;
        if find.is_empty() {
            return Err(RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_AST_FIND_SPAN_EMPTY",
                "structured AST extraction refused an empty find span",
                "dynamicjepa.action.ast.structured.find",
                "provide a non-empty exact source span",
                json!({"path": path}),
                None,
            ));
        }
        let matches = source.match_indices(find).collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_AST_FIND_SPAN_NOT_UNIQUE",
                "structured AST extraction requires the find span to match the source file exactly once",
                "dynamicjepa.action.ast.structured.find",
                "open the source window and provide a unique find span before DynamicJEPA action control",
                json!({"path": path, "match_count": matches.len()}),
                None,
            ));
        }
        let start = matches[0].0;
        let end = start + find.len();
        let start_line = byte_to_one_based_line(source, start)?;
        ranges.push((start, end, start_line));
    }
    Ok(ranges)
}

fn dynamicjepa_ast_unified_diff_ranges(
    arguments: &Value,
    path: &str,
    source: &str,
) -> Result<Vec<(usize, usize, u32)>> {
    let patch = arguments
        .get("patch")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_AST_PATCH_MISSING",
                "unified-diff AST extraction requires a patch string",
                "dynamicjepa.action.ast.unified_diff.patch",
                "provide the unified diff before DynamicJEPA action control",
                json!({"arguments": arguments}),
                None,
            )
        })?;
    let mut current_path: Option<String> = None;
    let mut ranges = Vec::new();
    for line in patch.lines() {
        if let Some(next_path) = line.strip_prefix("+++ b/") {
            current_path = Some(next_path.to_string());
            continue;
        }
        if !line.starts_with("@@") || current_path.as_deref() != Some(path) {
            continue;
        }
        let (start_line, line_count) = parse_unified_diff_old_range(line, path)?;
        let end_line = if line_count == 0 {
            start_line
        } else {
            start_line.saturating_add(line_count).saturating_sub(1)
        };
        ranges.push(line_range_to_byte_range(
            source,
            start_line as u64,
            end_line as u64,
            path,
        )?);
    }
    Ok(ranges)
}

fn dynamicjepa_ast_after_source_for_path(
    tool_name: &str,
    arguments: &Value,
    path: &str,
    source: &str,
) -> Result<Option<String>> {
    match tool_name {
        "apply_line_window_edit" => Ok(Some(dynamicjepa_apply_line_window_to_source(
            arguments, path, source,
        )?)),
        "apply_structured_edit" => Ok(Some(dynamicjepa_apply_structured_to_source(
            arguments, path, source,
        )?)),
        // Unified-diff semantic deltas are classified from the hunk text. The
        // real apply path still uses `git apply --check` and physical readback;
        // this extractor refuses to synthesize a partial after-file.
        "apply_unified_diff" => Ok(None),
        _ => Ok(None),
    }
}

fn dynamicjepa_apply_line_window_to_source(
    arguments: &Value,
    path: &str,
    source: &str,
) -> Result<String> {
    let edits = arguments
        .get("edits")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_SEMANTIC_LINE_EDITS_MISSING",
                "semantic diff extraction requires an edits array for line-window edits",
                "dynamicjepa.action.semantic.line_window.edits",
                "provide the same governed line-window edits used by the mutating tool",
                json!({"arguments": arguments}),
                None,
            )
        })?;
    let mut replacements = Vec::new();
    for edit in edits {
        if edit.get("path").and_then(Value::as_str) != Some(path) {
            continue;
        }
        let replace = edit.get("replace").and_then(Value::as_str).ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_SEMANTIC_LINE_REPLACE_MISSING",
                "semantic diff extraction requires replacement text for line-window edits",
                "dynamicjepa.action.semantic.line_window.replace",
                "provide exact replacement text before pre-action planning or ingestion",
                json!({"path": path, "edit": edit}),
                None,
            )
        })?;
        let start_line = edit
            .get("start_line")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                RealityError::new(
                    "RUST_REALITY_DYNAMICJEPA_SEMANTIC_LINE_START_MISSING",
                    "semantic diff extraction requires start_line",
                    "dynamicjepa.action.semantic.line_window.start_line",
                    "provide exact governed line-window coordinates",
                    json!({"path": path, "edit": edit}),
                    None,
                )
            })?;
        let end_line = edit
            .get("end_line")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                RealityError::new(
                    "RUST_REALITY_DYNAMICJEPA_SEMANTIC_LINE_END_MISSING",
                    "semantic diff extraction requires end_line",
                    "dynamicjepa.action.semantic.line_window.end_line",
                    "provide exact governed line-window coordinates",
                    json!({"path": path, "edit": edit}),
                    None,
                )
            })?;
        let (start, end, _) = line_range_to_byte_range(source, start_line, end_line, path)?;
        replacements.push((start, end, replace.to_string()));
    }
    dynamicjepa_apply_byte_replacements(source, replacements, path)
}

fn dynamicjepa_apply_structured_to_source(
    arguments: &Value,
    path: &str,
    source: &str,
) -> Result<String> {
    let edits = arguments
        .get("edits")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_SEMANTIC_STRUCTURED_EDITS_MISSING",
                "semantic diff extraction requires an edits array for structured edits",
                "dynamicjepa.action.semantic.structured.edits",
                "provide exact find/replace edits before pre-action planning or ingestion",
                json!({"arguments": arguments}),
                None,
            )
        })?;
    let mut replacements = Vec::new();
    for edit in edits {
        if edit.get("path").and_then(Value::as_str) != Some(path) {
            continue;
        }
        let find = edit.get("find").and_then(Value::as_str).ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_SEMANTIC_FIND_MISSING",
                "semantic diff extraction requires a structured edit find span",
                "dynamicjepa.action.semantic.structured.find",
                "provide the exact find span used by the mutating tool",
                json!({"path": path, "edit": edit}),
                None,
            )
        })?;
        let replace = edit.get("replace").and_then(Value::as_str).ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_SEMANTIC_REPLACE_MISSING",
                "semantic diff extraction requires structured edit replacement text",
                "dynamicjepa.action.semantic.structured.replace",
                "provide the exact replacement span used by the mutating tool",
                json!({"path": path, "edit": edit}),
                None,
            )
        })?;
        let matches = source.match_indices(find).collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_SEMANTIC_FIND_NOT_UNIQUE",
                "semantic diff extraction requires the structured find span to match exactly once",
                "dynamicjepa.action.semantic.structured.find",
                "use the latest source window and provide one unique find span",
                json!({"path": path, "match_count": matches.len()}),
                None,
            ));
        }
        replacements.push((matches[0].0, matches[0].0 + find.len(), replace.to_string()));
    }
    dynamicjepa_apply_byte_replacements(source, replacements, path)
}

fn dynamicjepa_apply_byte_replacements(
    source: &str,
    mut replacements: Vec<(usize, usize, String)>,
    path: &str,
) -> Result<String> {
    replacements.sort_by_key(|(start, _, _)| *start);
    let mut cursor = 0usize;
    let mut out = String::with_capacity(source.len());
    for (start, end, replace) in replacements {
        if start < cursor || end < start || end > source.len() {
            return Err(RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_SEMANTIC_REPLACEMENT_OVERLAP",
                "semantic diff extraction refused overlapping or invalid edit spans",
                "dynamicjepa.action.semantic.replacements",
                "submit non-overlapping governed edits before DynamicJEPA action control",
                json!({"path": path, "start": start, "end": end, "cursor": cursor, "source_len": source.len()}),
                None,
            ));
        }
        out.push_str(&source[cursor..start]);
        out.push_str(&replace);
        if !replace.ends_with('\n') && end < source.len() {
            out.push('\n');
        }
        cursor = end;
    }
    out.push_str(&source[cursor..]);
    Ok(out)
}

fn dynamicjepa_compiler_lsp_semantics_for_path(
    repo_dir: &Path,
    tool_name: &str,
    arguments: &Value,
    path: &str,
    language: &'static str,
    after_source: Option<&str>,
) -> Result<SweCompilerSemanticEvidence> {
    if after_source.is_none() && tool_name != "apply_unified_diff" {
        return Ok(SweCompilerSemanticEvidence::none());
    }
    let snapshot_root = std::env::temp_dir().join(format!(
        "contextgraph-dj-compiler-semantic-{}-{}",
        std::process::id(),
        unix_ns()?
    ));
    create_dir_checked(&snapshot_root)?;
    copy_dynamicjepa_semantic_repo_snapshot(repo_dir, &snapshot_root)?;
    let snapshot_target = snapshot_root.join(path);
    if let Some(after) = after_source {
        write_text_checked(&snapshot_target, after)?;
    } else {
        let patch = arguments
            .get("patch")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RealityError::new(
                    "RUST_REALITY_DYNAMICJEPA_COMPILER_PATCH_MISSING",
                    "compiler/LSP semantic extraction for a unified diff requires the exact patch text",
                    "dynamicjepa.action.compiler_lsp.patch",
                    "provide a standard unified diff before pre-action planning or ingestion",
                    json!({"path": path, "tool_name": tool_name}),
                    Some(file_sot(repo_dir)),
                )
            })?;
        let patch_path = snapshot_root.join("dynamicjepa-pending.patch");
        write_text_checked(&patch_path, patch)?;
        for (step, args) in [
            (
                "check",
                vec![
                    "git".to_string(),
                    "apply".to_string(),
                    "--check".to_string(),
                    path_str(&patch_path).to_string(),
                ],
            ),
            (
                "apply",
                vec![
                    "git".to_string(),
                    "apply".to_string(),
                    path_str(&patch_path).to_string(),
                ],
            ),
        ] {
            let result = run_cmd(&args, &snapshot_root, 60)?;
            if result.exit_code != 0 {
                let _ = fs::remove_dir_all(&snapshot_root);
                return Err(RealityError::new(
                    "RUST_REALITY_DYNAMICJEPA_COMPILER_PATCH_APPLY_FAILED",
                    "compiler/LSP semantic extraction could not apply the pending unified diff to a semantic snapshot",
                    "dynamicjepa.action.compiler_lsp.patch_apply",
                    "inspect the exact patch against the current checked-out source before planning or ingestion",
                    json!({"path": path, "step": step, "process": result}),
                    Some(file_sot(repo_dir)),
                ));
            }
        }
    }

    let evidence =
        dynamicjepa_run_language_semantic_tool(&snapshot_root, &snapshot_target, language)?;
    let _ = fs::remove_dir_all(&snapshot_root);
    Ok(evidence)
}

fn copy_dynamicjepa_semantic_repo_snapshot(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src).map_err(|err| {
        fs_error(
            "RUST_REALITY_DYNAMICJEPA_COMPILER_SNAPSHOT_READDIR_FAILED",
            src,
            err,
        )
    })? {
        let entry = entry.map_err(|err| {
            fs_error(
                "RUST_REALITY_DYNAMICJEPA_COMPILER_SNAPSHOT_DIRENTRY_FAILED",
                src,
                err,
            )
        })?;
        let name = entry.file_name().to_string_lossy().to_string();
        if matches!(
            name.as_str(),
            ".git"
                | "target"
                | "node_modules"
                | "__pycache__"
                | ".venv"
                | "venv"
                | "dist"
                | "build"
                | ".next"
        ) {
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(&name);
        let file_type = entry.file_type().map_err(|err| {
            fs_error(
                "RUST_REALITY_DYNAMICJEPA_COMPILER_SNAPSHOT_FILETYPE_FAILED",
                &src_path,
                err,
            )
        })?;
        if file_type.is_dir() {
            create_dir_checked(&dst_path)?;
            copy_dynamicjepa_semantic_repo_snapshot(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            if let Some(parent) = dst_path.parent() {
                create_dir_checked(parent)?;
            }
            fs::copy(&src_path, &dst_path).map_err(|err| {
                fs_error(
                    "RUST_REALITY_DYNAMICJEPA_COMPILER_SNAPSHOT_COPY_FAILED",
                    &src_path,
                    err,
                )
            })?;
        } else {
            return Err(RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_COMPILER_SNAPSHOT_UNSUPPORTED_PATH",
                "compiler/LSP semantic extraction refused a non-file, non-directory repository entry",
                "dynamicjepa.action.compiler_lsp.snapshot",
                "replace symlink/special source entries with real files before compiler-backed semantic gating",
                json!({"path": file_sot(&src_path)}),
                Some(file_sot(&src_path)),
            ));
        }
    }
    Ok(())
}

fn dynamicjepa_run_language_semantic_tool(
    repo_root: &Path,
    target: &Path,
    language: &'static str,
) -> Result<SweCompilerSemanticEvidence> {
    let rel_target = target.strip_prefix(repo_root).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_COMPILER_TARGET_RELATIVE_FAILED",
            "compiler/LSP semantic extraction could not derive a snapshot-relative target path",
            "dynamicjepa.action.compiler_lsp.target_path",
            "inspect semantic snapshot path construction",
            json!({"target": path_str(target), "repo_root": path_str(repo_root), "error": err.to_string()}),
            Some(file_sot(repo_root)),
        )
    })?;
    let rel_target = rel_target.to_string_lossy().replace('\\', "/");
    if language == "cpp" {
        return dynamicjepa_run_cpp_compile_commands_semantic_tool(repo_root, &rel_target);
    }
    let (command, cwd, kind): (Option<Vec<String>>, PathBuf, &'static str) = match language {
        "rust" => {
            if command_exists("cargo") && repo_root.join("Cargo.toml").exists() {
                (
                    Some(vec![
                        "cargo".to_string(),
                        "check".to_string(),
                        "--message-format=json".to_string(),
                        "--quiet".to_string(),
                    ]),
                    repo_root.to_path_buf(),
                    "rust_cargo_check",
                )
            } else if command_exists("rustc") {
                (
                    Some(vec![
                        "rustc".to_string(),
                        "--crate-type".to_string(),
                        "lib".to_string(),
                        "--edition=2021".to_string(),
                        rel_target.clone(),
                    ]),
                    repo_root.to_path_buf(),
                    "rustc_single_file",
                )
            } else {
                (None, repo_root.to_path_buf(), "rust_toolchain_missing")
            }
        }
        "python" => {
            if command_exists("python3") {
                (
                    Some(vec![
                        "python3".to_string(),
                        "-m".to_string(),
                        "py_compile".to_string(),
                        rel_target.clone(),
                    ]),
                    repo_root.to_path_buf(),
                    "python_py_compile",
                )
            } else {
                (None, repo_root.to_path_buf(), "python_toolchain_missing")
            }
        }
        "javascript" | "typescript" | "tsx" => {
            let local_tsc = repo_root.join("node_modules/.bin/tsc");
            if local_tsc.exists() {
                (
                    Some(vec![
                        path_str(&local_tsc).to_string(),
                        "--noEmit".to_string(),
                        "--pretty".to_string(),
                        "false".to_string(),
                    ]),
                    repo_root.to_path_buf(),
                    "typescript_tsc",
                )
            } else {
                (
                    None,
                    repo_root.to_path_buf(),
                    "typescript_toolchain_missing",
                )
            }
        }
        "go" => {
            if command_exists("go") {
                (
                    Some(vec![
                        "go".to_string(),
                        "test".to_string(),
                        "./...".to_string(),
                    ]),
                    repo_root.to_path_buf(),
                    "go_test_compile",
                )
            } else {
                (None, repo_root.to_path_buf(), "go_toolchain_missing")
            }
        }
        "java" => {
            if command_exists("javac") {
                (
                    Some(vec!["javac".to_string(), rel_target.clone()]),
                    repo_root.to_path_buf(),
                    "javac",
                )
            } else {
                (None, repo_root.to_path_buf(), "java_toolchain_missing")
            }
        }
        "c" => {
            let (argv, cwd) = dynamicjepa_c_compile_command_from_database(repo_root, &rel_target)?;
            (Some(argv), cwd, "c_compile_commands_syntax")
        }
        "ruby" => {
            if command_exists("ruby") {
                (
                    Some(vec![
                        "ruby".to_string(),
                        "-c".to_string(),
                        rel_target.clone(),
                    ]),
                    repo_root.to_path_buf(),
                    "ruby_syntax",
                )
            } else {
                (None, repo_root.to_path_buf(), "ruby_toolchain_missing")
            }
        }
        "csharp" => {
            let has_project = dynamicjepa_repo_contains_suffix(repo_root, ".sln")?
                || dynamicjepa_repo_contains_suffix(repo_root, ".csproj")?;
            if command_exists("dotnet") && has_project {
                (
                    Some(vec![
                        "dotnet".to_string(),
                        "build".to_string(),
                        "--no-restore".to_string(),
                    ]),
                    repo_root.to_path_buf(),
                    "dotnet_build",
                )
            } else {
                (None, repo_root.to_path_buf(), "csharp_toolchain_missing")
            }
        }
        "php" => {
            if command_exists("php") {
                (
                    Some(vec![
                        "php".to_string(),
                        "-l".to_string(),
                        rel_target.clone(),
                    ]),
                    repo_root.to_path_buf(),
                    "php_lint",
                )
            } else {
                (None, repo_root.to_path_buf(), "php_toolchain_missing")
            }
        }
        _ => (None, repo_root.to_path_buf(), "unsupported_language"),
    };

    let Some(command) = command else {
        if language == "java" && kind == "java_toolchain_missing" {
            return Err(RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_JAVA_TOOLCHAIN_MISSING",
                "DynamicJEPA Java compiler semantics require javac; parser-only fallback is refused",
                "dynamicjepa.action.compiler_lsp.java.toolchain",
                "install a JDK with javac on PATH before using Java source for ME-JEPA action control",
                json!({
                    "language": language,
                    "path": rel_target,
                    "required_tool": "javac",
                    "toolchain_kind": kind,
                    "PATH": std::env::var_os("PATH")
                        .map(|value| value.to_string_lossy().to_string())
                        .unwrap_or_default(),
                }),
                Some(file_sot(target)),
            ));
        }
        return Ok(SweCompilerSemanticEvidence {
            semantic_provider: "parser_symbol_graph",
            compiler_semantic_status: if kind == "unsupported_language" {
                "unsupported_language"
            } else {
                "toolchain_missing"
            },
            type_resolution_status: "unsupported",
            import_resolution_status: "unsupported",
            diagnostic_count: 0,
            predicted_verifier_target: "static_check",
        });
    };

    let result = run_cmd(&command, &cwd, 60)?;
    let output = format!("{}\n{}", result.stdout, result.stderr);
    let diagnostics = dynamicjepa_compiler_diagnostic_count(language, &result, &output);
    let status = if result.exit_code == 0 {
        "checked"
    } else {
        "diagnostics"
    };
    Ok(SweCompilerSemanticEvidence {
        semantic_provider: "compiler_lsp",
        compiler_semantic_status: status,
        type_resolution_status: dynamicjepa_type_resolution_status(language, status, &output),
        import_resolution_status: dynamicjepa_import_resolution_status(language, status, &output),
        diagnostic_count: diagnostics,
        predicted_verifier_target: dynamicjepa_predicted_verifier_target(
            language,
            status,
            diagnostics,
        ),
    })
}

fn dynamicjepa_c_compile_command_from_database(
    repo_root: &Path,
    rel_target: &str,
) -> Result<(Vec<String>, PathBuf)> {
    let compile_commands_path = repo_root.join("compile_commands.json");
    if !compile_commands_path.is_file() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_C_COMPILE_COMMANDS_MISSING",
            "DynamicJEPA C compiler semantics require compile_commands.json; parser-only or guessed gcc flags are refused",
            "dynamicjepa.action.compiler_lsp.c.compile_commands",
            "generate compile_commands.json for the C repair repository before ME-JEPA action control",
            json!({"path": rel_target, "compile_commands": file_sot(&compile_commands_path)}),
            Some(file_sot(repo_root)),
        ));
    }
    let compile_commands: Value = read_json(&compile_commands_path)?;
    let entries = compile_commands.as_array().ok_or_else(|| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_C_COMPILE_COMMANDS_SCHEMA_INVALID",
            "C compile_commands.json must be a JSON array",
            "dynamicjepa.action.compiler_lsp.c.compile_commands",
            "regenerate the compilation database in Clang JSON Compilation Database format",
            json!({"compile_commands": file_sot(&compile_commands_path)}),
            Some(file_sot(&compile_commands_path)),
        )
    })?;
    let mut matches = Vec::new();
    for (idx, entry) in entries.iter().enumerate() {
        let Some(file_value) = entry.get("file").and_then(Value::as_str) else {
            return Err(RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_C_COMPILE_COMMAND_FILE_MISSING",
                "C compile_commands entry is missing the required file field",
                "dynamicjepa.action.compiler_lsp.c.compile_commands[].file",
                "regenerate compile_commands.json with a file path for every C translation unit",
                json!({"entry_index": idx, "compile_commands": file_sot(&compile_commands_path)}),
                Some(file_sot(&compile_commands_path)),
            ));
        };
        if dynamicjepa_c_compile_command_file_matches(file_value, rel_target, repo_root) {
            matches.push((idx, entry, file_value));
        }
    }
    if matches.len() != 1 {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_C_COMPILE_COMMAND_NOT_UNIQUE",
            "C compiler semantics require exactly one compile_commands entry for the edited translation unit",
            "dynamicjepa.action.compiler_lsp.c.compile_commands[].file",
            "ensure the C compilation database has one structured command for the edited file",
            json!({
                "path": rel_target,
                "match_count": matches.len(),
                "matched_entry_indexes": matches.iter().map(|(idx, _, _)| *idx).collect::<Vec<_>>(),
                "compile_commands": file_sot(&compile_commands_path),
            }),
            Some(file_sot(&compile_commands_path)),
        ));
    }
    let (_, entry, file_value) = matches[0];
    let directory = entry
        .get("directory")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_C_COMPILE_COMMAND_DIRECTORY_MISSING",
                "C compile_commands entry is missing the required directory field",
                "dynamicjepa.action.compiler_lsp.c.compile_commands[].directory",
                "regenerate compile_commands.json with the compiler working directory",
                json!({"path": rel_target, "compile_commands": file_sot(&compile_commands_path)}),
                Some(file_sot(&compile_commands_path)),
            )
        })?;
    let original_dir = PathBuf::from(directory);
    let cwd = dynamicjepa_c_compile_command_snapshot_cwd(
        repo_root,
        rel_target,
        file_value,
        &original_dir,
    )?;
    let arguments = entry.get("arguments").and_then(Value::as_array).ok_or_else(|| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_C_COMPILE_COMMAND_ARGUMENTS_MISSING",
            "C compiler semantics require structured compile_commands arguments; shell command strings are refused",
            "dynamicjepa.action.compiler_lsp.c.compile_commands[].arguments",
            "regenerate compile_commands.json with argv-style arguments so ME-JEPA can exec without shell parsing",
            json!({"path": rel_target, "compile_commands": file_sot(&compile_commands_path)}),
            Some(file_sot(&compile_commands_path)),
        )
    })?;
    let original_root = dynamicjepa_c_compile_command_original_root(file_value, rel_target);
    let mut argv = Vec::new();
    for (arg_idx, argument) in arguments.iter().enumerate() {
        let arg = argument.as_str().ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_C_COMPILE_COMMAND_ARGUMENT_NON_STRING",
                "C compile_commands arguments must all be strings",
                "dynamicjepa.action.compiler_lsp.c.compile_commands[].arguments[]",
                "regenerate compile_commands.json with string argv entries",
                json!({"path": rel_target, "argument_index": arg_idx}),
                Some(file_sot(&compile_commands_path)),
            )
        })?;
        argv.push(dynamicjepa_rewrite_c_compile_arg_for_snapshot(
            arg,
            rel_target,
            file_value,
            original_root.as_deref(),
            repo_root,
        ));
    }
    if argv.is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_C_COMPILE_COMMAND_EMPTY",
            "C compile_commands entry had an empty arguments array",
            "dynamicjepa.action.compiler_lsp.c.compile_commands[].arguments",
            "regenerate compile_commands.json with the compiler executable and arguments",
            json!({"path": rel_target}),
            Some(file_sot(&compile_commands_path)),
        ));
    }
    if !argv.iter().any(|arg| arg == "-fsyntax-only") {
        argv.push("-fsyntax-only".to_string());
    }
    Ok((argv, cwd))
}

fn dynamicjepa_c_compile_command_file_matches(
    file_value: &str,
    rel_target: &str,
    repo_root: &Path,
) -> bool {
    let normalized = file_value.replace('\\', "/");
    if Path::new(file_value)
        .strip_prefix(repo_root)
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/") == rel_target)
        .unwrap_or(false)
    {
        return true;
    }
    normalized == rel_target || normalized.ends_with(&format!("/{rel_target}"))
}

fn dynamicjepa_c_compile_command_original_root(
    file_value: &str,
    rel_target: &str,
) -> Option<PathBuf> {
    let normalized = file_value.replace('\\', "/");
    let suffix = format!("/{rel_target}");
    let prefix = normalized.strip_suffix(&suffix)?;
    if prefix.is_empty() {
        None
    } else {
        Some(PathBuf::from(prefix))
    }
}

fn dynamicjepa_c_compile_command_snapshot_cwd(
    repo_root: &Path,
    rel_target: &str,
    file_value: &str,
    original_dir: &Path,
) -> Result<PathBuf> {
    if !original_dir.is_absolute() {
        return Ok(repo_root.join(original_dir));
    }
    let Some(original_root) = dynamicjepa_c_compile_command_original_root(file_value, rel_target)
    else {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_C_COMPILE_COMMAND_ABSOLUTE_CWD_UNMAPPED",
            "C compile_commands entry used an absolute directory that could not be mapped into the semantic snapshot",
            "dynamicjepa.action.compiler_lsp.c.compile_commands[].directory",
            "emit compile_commands paths under the source root or use relative directories for C action control",
            json!({"path": rel_target, "directory": path_str(original_dir), "file": file_value}),
            Some(file_sot(repo_root)),
        ));
    };
    let suffix = original_dir.strip_prefix(&original_root).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_C_COMPILE_COMMAND_CWD_OUTSIDE_ROOT",
            "C compile_commands directory is outside the source root inferred from the file entry",
            "dynamicjepa.action.compiler_lsp.c.compile_commands[].directory",
            "emit C compile_commands entries whose directory and file share the same source root",
            json!({
                "path": rel_target,
                "directory": path_str(original_dir),
                "file": file_value,
                "inferred_original_root": path_str(&original_root),
                "error": err.to_string(),
            }),
            Some(file_sot(repo_root)),
        )
    })?;
    Ok(repo_root.join(suffix))
}

fn dynamicjepa_rewrite_c_compile_arg_for_snapshot(
    arg: &str,
    rel_target: &str,
    file_value: &str,
    original_root: Option<&Path>,
    repo_root: &Path,
) -> String {
    if dynamicjepa_c_compile_command_file_matches(arg, rel_target, repo_root) || arg == file_value {
        return rel_target.to_string();
    }
    if let Some(include_path) = arg.strip_prefix("-I") {
        if Path::new(include_path).is_absolute() {
            return format!(
                "-I{}",
                path_str(&dynamicjepa_rewrite_c_absolute_path_for_snapshot(
                    include_path,
                    original_root,
                    repo_root,
                ))
            );
        }
    }
    if Path::new(arg).is_absolute() {
        return path_str(&dynamicjepa_rewrite_c_absolute_path_for_snapshot(
            arg,
            original_root,
            repo_root,
        ))
        .to_string();
    }
    arg.to_string()
}

fn dynamicjepa_rewrite_c_absolute_path_for_snapshot(
    path: &str,
    original_root: Option<&Path>,
    repo_root: &Path,
) -> PathBuf {
    let path = Path::new(path);
    if let Some(original_root) = original_root {
        if let Ok(suffix) = path.strip_prefix(original_root) {
            return repo_root.join(suffix);
        }
    }
    path.to_path_buf()
}

fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|dir| dir.join(name))
                .find(|candidate| candidate.exists() && candidate.is_file())
        })
        .is_some()
}

fn dynamicjepa_compiler_diagnostic_count(
    language: &str,
    result: &ProcessResult,
    output: &str,
) -> i64 {
    if result.exit_code == 0 {
        return 0;
    }
    if language == "rust" {
        let count = result
            .stdout
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|value| value.get("reason").and_then(Value::as_str) == Some("compiler-message"))
            .filter(|value| {
                matches!(
                    value.pointer("/message/level").and_then(Value::as_str),
                    Some("error" | "warning")
                )
            })
            .count();
        if count > 0 {
            return count.min(10_000) as i64;
        }
    }
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
        .max(1)
        .min(10_000) as i64
}

fn dynamicjepa_type_resolution_status(
    language: &str,
    compiler_status: &str,
    output: &str,
) -> &'static str {
    if compiler_status != "checked" {
        let lower = output.to_ascii_lowercase();
        if contains_any(
            &lower,
            &[
                "type mismatch",
                "mismatched types",
                "cannot infer",
                "type annotations needed",
                "expected ",
                "found ",
                "no method named",
                "undefined method",
                "unknown type",
            ],
        ) {
            return "unresolved";
        }
    }
    match language {
        "rust" | "typescript" | "tsx" | "go" | "java" | "c" | "cpp" | "csharp" => {
            if compiler_status == "checked" {
                "resolved"
            } else {
                "unknown"
            }
        }
        _ => "unsupported",
    }
}

fn dynamicjepa_import_resolution_status(
    language: &str,
    compiler_status: &str,
    output: &str,
) -> &'static str {
    let lower = output.to_ascii_lowercase();
    if contains_any(
        &lower,
        &[
            "unresolved import",
            "cannot find module",
            "cannot find crate",
            "no such file or directory",
            "package .* is not in",
            "module not found",
            "cannot resolve",
        ],
    ) {
        return "unresolved";
    }
    match language {
        "rust" | "typescript" | "tsx" | "go" | "java" | "c" | "cpp" | "csharp" => {
            if compiler_status == "checked" {
                "resolved"
            } else {
                "unknown"
            }
        }
        _ => "unsupported",
    }
}

fn dynamicjepa_predicted_verifier_target(
    language: &str,
    compiler_status: &str,
    diagnostic_count: i64,
) -> &'static str {
    if diagnostic_count > 0 || compiler_status == "diagnostics" {
        return "typecheck";
    }
    match language {
        "rust" | "typescript" | "tsx" | "go" | "java" | "c" | "cpp" | "csharp" => "typecheck",
        "python" | "ruby" | "php" | "javascript" => "static_check",
        _ => "unknown",
    }
}

fn dynamicjepa_run_cpp_compile_commands_semantic_tool(
    repo_root: &Path,
    rel_target: &str,
) -> Result<SweCompilerSemanticEvidence> {
    let compile_commands_path = repo_root.join("compile_commands.json");
    if !compile_commands_path.is_file() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_CPP_COMPILE_COMMANDS_MISSING",
            "C++ compiler/LSP semantic extraction requires compile_commands.json; parser-only C++ evidence is refused",
            "dynamicjepa.action.compiler_lsp.compile_commands",
            "generate a compilation database for the real C++ repository before ME-JEPA action control",
            json!({"target": rel_target, "repo_root": path_str(repo_root)}),
            Some(file_sot(repo_root)),
        ));
    }
    let compile_commands: Value = read_json(&compile_commands_path)?;
    let entries = compile_commands.as_array().ok_or_else(|| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_CPP_COMPILE_COMMANDS_SCHEMA_INVALID",
            "compile_commands.json must be a JSON array for C++ compiler semantic extraction",
            "dynamicjepa.action.compiler_lsp.compile_commands",
            "regenerate compile_commands.json using CMake/Ninja/Bear with structured arguments",
            json!({"compile_commands": file_sot(&compile_commands_path)}),
            Some(file_sot(&compile_commands_path)),
        )
    })?;

    let target_is_header = is_cpp_header_path(rel_target);
    let mut selected = Vec::new();
    for (idx, entry) in entries.iter().enumerate() {
        let repo_relative = compile_command_repo_relative_file(repo_root, entry, idx)?;
        if swe_ast_language_for_path(&repo_relative) != Some("cpp") {
            continue;
        }
        if repo_relative == rel_target || target_is_header {
            selected.push((idx, repo_relative, entry));
        }
    }
    if selected.is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_CPP_COMPILE_COMMANDS_TARGET_MISSING",
            "compile_commands.json did not contain a C++ translation unit capable of checking the edited target",
            "dynamicjepa.action.compiler_lsp.compile_commands.target",
            "include the edited C++ file or at least one C++ translation unit for header edits in compile_commands.json",
            json!({"target": rel_target, "compile_commands": file_sot(&compile_commands_path)}),
            Some(file_sot(&compile_commands_path)),
        ));
    }

    let mut combined_output = String::new();
    let mut diagnostic_count = 0i64;
    let mut status = "checked";
    for (idx, repo_relative, entry) in selected {
        let (argv, cwd) = compile_command_argv_for_snapshot(repo_root, entry, idx)?;
        let result = run_cmd(&argv, &cwd, 60)?;
        let output = format!("{}\n{}", result.stdout, result.stderr);
        combined_output.push_str(&format!(
            "\n--- compile_commands[{idx}] {repo_relative} exit={} ---\n{}",
            result.exit_code, output
        ));
        let diagnostics = dynamicjepa_compiler_diagnostic_count("cpp", &result, &output);
        diagnostic_count = diagnostic_count.saturating_add(diagnostics);
        if result.exit_code != 0 {
            status = "diagnostics";
        }
    }

    Ok(SweCompilerSemanticEvidence {
        semantic_provider: "compiler_lsp_compile_commands",
        compiler_semantic_status: status,
        type_resolution_status: dynamicjepa_type_resolution_status("cpp", status, &combined_output),
        import_resolution_status: dynamicjepa_import_resolution_status(
            "cpp",
            status,
            &combined_output,
        ),
        diagnostic_count: diagnostic_count.min(10_000),
        predicted_verifier_target: dynamicjepa_predicted_verifier_target(
            "cpp",
            status,
            diagnostic_count,
        ),
    })
}

fn compile_command_repo_relative_file(
    repo_root: &Path,
    entry: &Value,
    idx: usize,
) -> Result<String> {
    let file_value = entry.get("file").and_then(Value::as_str).ok_or_else(|| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_CPP_COMPILE_COMMANDS_FILE_MISSING",
            "compile_commands.json entry is missing file",
            "dynamicjepa.action.compiler_lsp.compile_commands[].file",
            "regenerate compile_commands.json with file paths for every C++ translation unit",
            json!({"entry_index": idx, "entry": entry}),
            Some(file_sot(repo_root)),
        )
    })?;
    let file_path = Path::new(file_value);
    if file_path.is_absolute() {
        if let Ok(rel) = file_path.strip_prefix(repo_root) {
            return Ok(rel.to_string_lossy().replace('\\', "/"));
        }
        if let Some(rel) = rebase_absolute_compile_command_path(repo_root, file_path) {
            return Ok(rel);
        }
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_CPP_COMPILE_COMMANDS_FILE_OUTSIDE_SNAPSHOT",
            "compile_commands.json entry points outside the semantic snapshot and could not be rebased",
            "dynamicjepa.action.compiler_lsp.compile_commands[].file",
            "regenerate compile_commands.json with repo-relative paths or paths rooted in the checked snapshot",
            json!({"entry_index": idx, "file": file_value, "repo_root": path_str(repo_root)}),
            Some(file_sot(repo_root)),
        ));
    }
    let normalized = file_value.replace('\\', "/");
    if repo_root.join(&normalized).exists() {
        Ok(normalized)
    } else {
        Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_CPP_COMPILE_COMMANDS_FILE_MISSING_ON_DISK",
            "compile_commands.json entry references a file that does not exist in the semantic snapshot",
            "dynamicjepa.action.compiler_lsp.compile_commands[].file",
            "regenerate compile_commands.json after refreshing the checkout",
            json!({"entry_index": idx, "file": file_value, "repo_root": path_str(repo_root)}),
            Some(file_sot(repo_root)),
        ))
    }
}

fn compile_command_argv_for_snapshot(
    repo_root: &Path,
    entry: &Value,
    idx: usize,
) -> Result<(Vec<String>, PathBuf)> {
    if entry.get("command").and_then(Value::as_str).is_some() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_CPP_COMPILE_COMMANDS_SHELL_COMMAND_REFUSED",
            "C++ compiler semantic extraction refuses shell command strings in compile_commands.json",
            "dynamicjepa.action.compiler_lsp.compile_commands[].command",
            "regenerate compile_commands.json with structured arguments arrays",
            json!({"entry_index": idx}),
            Some(file_sot(repo_root)),
        ));
    }
    let file_value = entry.get("file").and_then(Value::as_str).ok_or_else(|| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_CPP_COMPILE_COMMANDS_FILE_MISSING",
            "compile_commands.json entry is missing file",
            "dynamicjepa.action.compiler_lsp.compile_commands[].file",
            "regenerate compile_commands.json with file paths for every C++ translation unit",
            json!({"entry_index": idx, "entry": entry}),
            Some(file_sot(repo_root)),
        )
    })?;
    let repo_relative = compile_command_repo_relative_file(repo_root, entry, idx)?;
    let snapshot_file = repo_root.join(&repo_relative);
    let arguments = entry
        .get("arguments")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_CPP_COMPILE_COMMANDS_ARGUMENTS_MISSING",
                "compile_commands.json C++ entries must include structured arguments",
                "dynamicjepa.action.compiler_lsp.compile_commands[].arguments",
                "regenerate compile_commands.json with argv-style arguments arrays",
                json!({"entry_index": idx, "file": file_value}),
                Some(file_sot(repo_root)),
            )
        })?;
    let mut argv = Vec::new();
    for argument in arguments {
        let value = argument.as_str().ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_CPP_COMPILE_COMMANDS_ARGUMENT_NON_STRING",
                "compile_commands.json C++ argument was not a string",
                "dynamicjepa.action.compiler_lsp.compile_commands[].arguments[]",
                "regenerate compile_commands.json with string argv entries",
                json!({"entry_index": idx, "argument": argument}),
                Some(file_sot(repo_root)),
            )
        })?;
        let rebased = if value == file_value || value.replace('\\', "/") == repo_relative {
            path_str(&snapshot_file).to_string()
        } else if Path::new(value).is_absolute() {
            rebase_absolute_compile_command_path(repo_root, Path::new(value))
                .map(|rel| path_str(&repo_root.join(rel)).to_string())
                .unwrap_or_else(|| value.to_string())
        } else {
            value.to_string()
        };
        argv.push(rebased);
    }
    if argv.is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_CPP_COMPILE_COMMANDS_ARGUMENTS_EMPTY",
            "compile_commands.json C++ entry had an empty arguments array",
            "dynamicjepa.action.compiler_lsp.compile_commands[].arguments",
            "regenerate compile_commands.json with compiler argv entries",
            json!({"entry_index": idx}),
            Some(file_sot(repo_root)),
        ));
    }
    if !argv.iter().any(|arg| arg == "-fsyntax-only") {
        argv.push("-fsyntax-only".to_string());
    }
    let cwd = entry
        .get("directory")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .filter(|path| !path.is_absolute() || path.starts_with(repo_root))
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                repo_root.join(path)
            }
        })
        .unwrap_or_else(|| repo_root.to_path_buf());
    Ok((argv, cwd))
}

fn rebase_absolute_compile_command_path(repo_root: &Path, absolute: &Path) -> Option<String> {
    let components = absolute
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => value.to_str().map(ToOwned::to_owned),
            _ => None,
        })
        .collect::<Vec<_>>();
    for start in 0..components.len() {
        let rel = components[start..].join("/");
        if repo_root.join(&rel).exists() {
            return Some(rel);
        }
    }
    None
}

fn is_cpp_header_path(path: &str) -> bool {
    [
        ".h", ".hh", ".hpp", ".hxx", ".h++", ".ipp", ".inl", ".tpp", ".txx", ".ixx",
    ]
    .iter()
    .any(|suffix| path.ends_with(suffix))
}

fn dynamicjepa_test_attribution_for_path(
    repo_dir: &Path,
    path: &str,
    language: &'static str,
    ranges: &[(usize, usize, u32)],
) -> Result<SweTestAttributionEvidence> {
    if path_is_test(path) {
        return Ok(SweTestAttributionEvidence {
            status: "inferred_from_path",
            test_count: 1,
        });
    }
    if let Some(count) = dynamicjepa_coverage_attribution_count(repo_dir, path, ranges)? {
        return Ok(SweTestAttributionEvidence {
            status: "inferred_from_coverage",
            test_count: count,
        });
    }
    if dynamicjepa_repo_has_test_manifest(repo_dir, language) {
        return Ok(SweTestAttributionEvidence {
            status: "inferred_from_manifest",
            test_count: 1,
        });
    }
    Ok(SweTestAttributionEvidence {
        status: "missing",
        test_count: 0,
    })
}

fn dynamicjepa_coverage_attribution_count(
    repo_dir: &Path,
    path: &str,
    ranges: &[(usize, usize, u32)],
) -> Result<Option<i64>> {
    let coverage_path = repo_dir.join("coverage.json");
    if !coverage_path.exists() {
        return Ok(None);
    }
    let coverage: Value = read_json(&coverage_path)?;
    let files = coverage
        .get("files")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_COVERAGE_SCHEMA_INVALID",
            "coverage attribution found coverage.json but it did not contain a files object",
            "coverage_json.files",
            "regenerate coverage.json from the real coverage tool before using test attribution",
            json!({"coverage": file_sot(&coverage_path)}),
            Some(file_sot(&coverage_path)),
        )
        })?;
    let normalized = path.replace('\\', "/");
    let Some(file) = files.get(&normalized).or_else(|| {
        files
            .iter()
            .find(|(candidate, _)| candidate.replace('\\', "/").ends_with(&normalized))
            .map(|(_, value)| value)
    }) else {
        return Ok(None);
    };
    let executed = file
        .get("executed_lines")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_COVERAGE_LINES_MISSING",
                "coverage attribution found the edited source file but no executed_lines array",
                "coverage_json.files[].executed_lines",
                "regenerate coverage.json with line execution data before using test attribution",
                json!({"coverage": file_sot(&coverage_path), "path": path}),
                Some(file_sot(&coverage_path)),
            )
        })?
        .iter()
        .filter_map(Value::as_u64)
        .collect::<BTreeSet<_>>();
    if executed.is_empty() {
        return Ok(None);
    }
    let touched_lines = ranges
        .iter()
        .map(|(_, _, line)| u64::from(*line))
        .collect::<BTreeSet<_>>();
    let hit_count = touched_lines
        .iter()
        .filter(|line| executed.contains(line))
        .count();
    if hit_count == 0 {
        Ok(None)
    } else {
        Ok(Some(hit_count.min(10_000) as i64))
    }
}

fn dynamicjepa_repo_has_test_manifest(repo_dir: &Path, language: &'static str) -> bool {
    match language {
        "rust" => repo_dir.join("Cargo.toml").exists() || repo_dir.join("tests").is_dir(),
        "python" => {
            repo_dir.join("pytest.ini").exists()
                || repo_dir.join("pyproject.toml").exists()
                || repo_dir.join("setup.cfg").exists()
                || repo_dir.join("tests").is_dir()
        }
        "javascript" | "typescript" | "tsx" => {
            repo_dir.join("package.json").exists() || repo_dir.join("__tests__").is_dir()
        }
        "go" => dynamicjepa_repo_contains_suffix(repo_dir, "_test.go").unwrap_or(false),
        "java" => repo_dir.join("pom.xml").exists() || repo_dir.join("build.gradle").exists(),
        "ruby" => repo_dir.join("spec").is_dir() || repo_dir.join("test").is_dir(),
        _ => repo_dir.join("tests").is_dir() || repo_dir.join("test").is_dir(),
    }
}

fn dynamicjepa_repo_contains_suffix(repo_dir: &Path, suffix: &str) -> Result<bool> {
    for entry in fs::read_dir(repo_dir).map_err(|err| {
        fs_error(
            "RUST_REALITY_DYNAMICJEPA_TEST_MANIFEST_READDIR_FAILED",
            repo_dir,
            err,
        )
    })? {
        let entry = entry.map_err(|err| {
            fs_error(
                "RUST_REALITY_DYNAMICJEPA_TEST_MANIFEST_DIRENTRY_FAILED",
                repo_dir,
                err,
            )
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| {
            fs_error(
                "RUST_REALITY_DYNAMICJEPA_TEST_MANIFEST_FILETYPE_FAILED",
                &path,
                err,
            )
        })?;
        if file_type.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if matches!(
                name.as_str(),
                ".git" | "target" | "node_modules" | "__pycache__"
            ) {
                continue;
            }
            if dynamicjepa_repo_contains_suffix(&path, suffix)? {
                return Ok(true);
            }
        } else if file_type.is_file()
            && path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.ends_with(suffix))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn parse_unified_diff_old_range(line: &str, path: &str) -> Result<(u32, u32)> {
    let rest = line.strip_prefix("@@ -").ok_or_else(|| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_AST_HUNK_HEADER_INVALID",
            "unified-diff AST extraction could not parse a hunk header",
            "dynamicjepa.action.ast.unified_diff.hunk",
            "provide standard unified diff hunk headers",
            json!({"path": path, "hunk": line}),
            None,
        )
    })?;
    let old_range = rest.split_whitespace().next().ok_or_else(|| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_AST_HUNK_RANGE_MISSING",
            "unified-diff AST extraction could not find the old-file hunk range",
            "dynamicjepa.action.ast.unified_diff.hunk.old_range",
            "provide standard unified diff hunk headers",
            json!({"path": path, "hunk": line}),
            None,
        )
    })?;
    let mut parts = old_range.split(',');
    let start = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_AST_HUNK_START_INVALID",
                "unified-diff AST extraction could not parse the old-file hunk start line",
                "dynamicjepa.action.ast.unified_diff.hunk.old_start",
                "provide standard unified diff hunk headers",
                json!({"path": path, "hunk": line}),
                None,
            )
        })?;
    let count = parts
        .next()
        .map(|value| value.parse::<u32>())
        .transpose()
        .map_err(|err| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_AST_HUNK_COUNT_INVALID",
                "unified-diff AST extraction could not parse the old-file hunk line count",
                "dynamicjepa.action.ast.unified_diff.hunk.old_count",
                "provide standard unified diff hunk headers",
                json!({"path": path, "hunk": line, "error": err.to_string()}),
                None,
            )
        })?
        .unwrap_or(1);
    Ok((start, count))
}

fn line_range_to_byte_range(
    source: &str,
    start_line: u64,
    end_line: u64,
    path: &str,
) -> Result<(usize, usize, u32)> {
    if start_line == 0 || end_line == 0 || end_line < start_line {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_AST_LINE_RANGE_INVALID",
            "AST extraction refused an invalid one-based line range",
            "dynamicjepa.action.ast.line_range",
            "provide a valid governed line range",
            json!({"path": path, "start_line": start_line, "end_line": end_line}),
            None,
        ));
    }
    let mut line_starts = vec![0usize];
    for (idx, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            line_starts.push(idx + 1);
        }
    }
    let start_idx = usize::try_from(start_line - 1).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_AST_START_LINE_OVERFLOW",
            "AST extraction start_line did not fit usize",
            "dynamicjepa.action.ast.line_range.start_line",
            "provide a bounded line range",
            json!({"path": path, "start_line": start_line, "error": err.to_string()}),
            None,
        )
    })?;
    let end_idx = usize::try_from(end_line - 1).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_AST_END_LINE_OVERFLOW",
            "AST extraction end_line did not fit usize",
            "dynamicjepa.action.ast.line_range.end_line",
            "provide a bounded line range",
            json!({"path": path, "end_line": end_line, "error": err.to_string()}),
            None,
        )
    })?;
    if start_idx >= line_starts.len() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_AST_START_LINE_OUT_OF_BOUNDS",
            "AST extraction start_line exceeds the source file line count",
            "dynamicjepa.action.ast.line_range.start_line",
            "open the current source window before planning or ingesting this edit",
            json!({"path": path, "start_line": start_line, "line_count": line_starts.len()}),
            None,
        ));
    }
    if end_idx >= line_starts.len() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_AST_END_LINE_OUT_OF_BOUNDS",
            "AST extraction end_line exceeds the source file line count",
            "dynamicjepa.action.ast.line_range.end_line",
            "open the current source window before planning or ingesting this edit",
            json!({"path": path, "end_line": end_line, "line_count": line_starts.len()}),
            None,
        ));
    }
    let start_byte = line_starts[start_idx];
    let end_byte = if end_idx + 1 < line_starts.len() {
        line_starts[end_idx + 1]
    } else {
        source.len()
    };
    Ok((start_byte, end_byte.max(start_byte), start_line as u32))
}

fn ast_significant_byte_range(
    source: &str,
    start: usize,
    end_exclusive: usize,
) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut significant_start = start.min(bytes.len());
    let mut significant_end = end_exclusive.min(bytes.len()).max(significant_start);
    while significant_start < significant_end && bytes[significant_start].is_ascii_whitespace() {
        significant_start += 1;
    }
    while significant_end > significant_start && bytes[significant_end - 1].is_ascii_whitespace() {
        significant_end -= 1;
    }
    (significant_start < significant_end).then_some((significant_start, significant_end))
}

fn byte_to_one_based_line(source: &str, byte_offset: usize) -> Result<u32> {
    let mut line = 1u32;
    for (idx, byte) in source.bytes().enumerate() {
        if idx >= byte_offset {
            break;
        }
        if byte == b'\n' {
            line = line.saturating_add(1);
        }
    }
    Ok(line)
}

#[derive(Debug, Clone)]
struct AstSemanticNode {
    node_kind: &'static str,
    symbol_kind: &'static str,
    node_start_line: u32,
    symbol_name: Option<String>,
    visibility: &'static str,
}

fn dynamicjepa_ast_semantic_node(
    language: &'static str,
    node: tree_sitter::Node<'_>,
    source: &str,
    path: &str,
) -> AstSemanticNode {
    let mut current = Some(node);
    while let Some(candidate) = current {
        let node_kind = ast_node_kind_for_language(language, candidate, source, path);
        if node_kind != "unknown" {
            return AstSemanticNode {
                node_kind,
                symbol_kind: ast_symbol_kind_for_node_kind(node_kind),
                node_start_line: candidate.start_position().row as u32 + 1,
                symbol_name: ast_node_name_for_language(language, candidate, source),
                visibility: ast_symbol_visibility(language, candidate, source),
            };
        }
        current = candidate.parent();
    }
    AstSemanticNode {
        node_kind: "module",
        symbol_kind: "module",
        node_start_line: 1,
        symbol_name: Path::new(path)
            .file_stem()
            .and_then(|value| value.to_str())
            .map(ToOwned::to_owned),
        visibility: "none",
    }
}

fn ast_node_kind_for_language(
    language: &'static str,
    node: tree_sitter::Node<'_>,
    source: &str,
    path: &str,
) -> &'static str {
    let kind = node.kind();
    match language {
        "python" => match kind {
            "function_definition" => {
                if path_is_test(path)
                    || ast_node_name(node, source).is_some_and(|name| name.starts_with("test"))
                {
                    "test"
                } else if ast_has_ancestor_kind(node, "class_definition") {
                    "method"
                } else {
                    "function"
                }
            }
            "class_definition" => "class",
            "import_statement" | "import_from_statement" => "import",
            "call" => "call",
            "assignment" | "augmented_assignment" => "assignment",
            "module" => "module",
            _ => "unknown",
        },
        "rust" => match kind {
            "function_item" => {
                if ast_has_ancestor_kind(node, "impl_item") {
                    "method"
                } else {
                    "function"
                }
            }
            "struct_item" | "enum_item" | "trait_item" | "impl_item" => "class",
            "use_declaration" | "extern_crate_declaration" => "import",
            "call_expression" => "call",
            "let_declaration" | "assignment_expression" | "const_item" | "static_item" => {
                "assignment"
            }
            "source_file" => "module",
            _ => "unknown",
        },
        "javascript" | "typescript" | "tsx" => match kind {
            "function_declaration" | "generator_function_declaration" | "arrow_function" => {
                if path_is_test(path)
                    || ast_node_name(node, source).is_some_and(|name| name.starts_with("test"))
                {
                    "test"
                } else {
                    "function"
                }
            }
            "method_definition" => "method",
            "class_declaration" => "class",
            "import_statement" => "import",
            "call_expression" => "call",
            "assignment_expression" | "lexical_declaration" | "variable_declarator" => "assignment",
            "program" => "module",
            _ => "unknown",
        },
        "go" => match kind {
            "function_declaration" => {
                if path_is_test(path)
                    || ast_node_name(node, source).is_some_and(|name| name.starts_with("Test"))
                {
                    "test"
                } else {
                    "function"
                }
            }
            "method_declaration" => "method",
            "type_declaration" => "class",
            "import_declaration" => "import",
            "call_expression" => "call",
            "short_var_declaration"
            | "var_declaration"
            | "assignment_statement"
            | "const_declaration" => "assignment",
            "source_file" => "module",
            _ => "unknown",
        },
        "java" => match kind {
            "method_declaration" | "constructor_declaration" => {
                if path_is_test(path)
                    || java_method_has_test_annotation(node, source)
                    || ast_node_name(node, source).is_some_and(|name| name.starts_with("test"))
                {
                    "test"
                } else {
                    "method"
                }
            }
            "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration" => "class",
            "import_declaration" => "import",
            "method_invocation" | "object_creation_expression" => "call",
            "local_variable_declaration" | "assignment_expression" | "field_declaration" => {
                "assignment"
            }
            "program" => "module",
            _ => "unknown",
        },
        "c" => ast_c_node_kind(node),
        "cpp" => ast_cpp_node_kind(node, source),
        "csharp" => match kind {
            "method_declaration" | "constructor_declaration" => "method",
            "class_declaration"
            | "interface_declaration"
            | "struct_declaration"
            | "enum_declaration"
            | "record_declaration" => "class",
            "using_directive" => "import",
            "invocation_expression" | "object_creation_expression" => "call",
            "variable_declaration" | "assignment_expression" | "field_declaration" => "assignment",
            "compilation_unit" => "module",
            _ => "unknown",
        },
        "ruby" => match kind {
            "method" | "singleton_method" => "method",
            "class" | "module" => "class",
            "require" | "require_relative" => "import",
            "call" | "command" => "call",
            "assignment" | "operator_assignment" => "assignment",
            "program" => "module",
            _ => "unknown",
        },
        "php" => match kind {
            "function_definition" => "function",
            "method_declaration" => "method",
            "class_declaration"
            | "interface_declaration"
            | "trait_declaration"
            | "enum_declaration" => "class",
            "namespace_use_declaration" | "require_expression" | "include_expression" => "import",
            "function_call_expression"
            | "member_call_expression"
            | "object_creation_expression" => "call",
            "assignment_expression" | "variable_name" => "assignment",
            "program" => "module",
            _ => "unknown",
        },
        _ => "unknown",
    }
}

fn ast_c_node_kind(node: tree_sitter::Node<'_>) -> &'static str {
    match node.kind() {
        "function_definition" => "function",
        "function_declarator" if !ast_has_ancestor_kind(node, "function_definition") => "function",
        "declaration" if ast_c_contains_function_declarator(node) => "function",
        "preproc_function_def" | "preproc_def" => "function",
        "struct_specifier" | "enum_specifier" | "union_specifier"
            if ast_c_compound_specifier_declares_body(node) =>
        {
            "class"
        }
        "preproc_include" => "import",
        "call_expression" => "call",
        "declaration" | "init_declarator" | "assignment_expression" => "assignment",
        "translation_unit" => "module",
        _ => "unknown",
    }
}

fn ast_c_compound_specifier_declares_body(node: tree_sitter::Node<'_>) -> bool {
    node.child_by_field_name("body").is_some()
}

fn ast_c_contains_function_declarator(node: tree_sitter::Node<'_>) -> bool {
    if node.kind() == "function_declarator" {
        return true;
    }
    let mut cursor = node.walk();
    let found = node
        .children(&mut cursor)
        .any(ast_c_contains_function_declarator);
    found
}

fn ast_cpp_node_kind(node: tree_sitter::Node<'_>, source: &str) -> &'static str {
    match node.kind() {
        "function_definition" | "function_declarator" => {
            if ast_cpp_function_is_method(node, source) {
                "method"
            } else {
                "function"
            }
        }
        "field_declaration" => {
            if ast_descendant_has_kind(node, "function_declarator") {
                "method"
            } else {
                "assignment"
            }
        }
        "class_specifier" | "struct_specifier" | "enum_specifier" | "union_specifier" => "class",
        "namespace_definition" | "translation_unit" => "module",
        "preproc_include" | "using_declaration" | "using_directive" | "alias_declaration" => {
            "import"
        }
        "call_expression" => "call",
        "declaration" | "init_declarator" | "assignment_expression" => "assignment",
        "template_declaration" => ast_cpp_template_node_kind(node, source),
        _ => "unknown",
    }
}

fn ast_cpp_template_node_kind(node: tree_sitter::Node<'_>, source: &str) -> &'static str {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        let kind = ast_cpp_node_kind(child, source);
        if kind != "unknown" && kind != "import" {
            return kind;
        }
    }
    "unknown"
}

fn ast_cpp_function_is_method(node: tree_sitter::Node<'_>, source: &str) -> bool {
    if ast_has_ancestor_any(
        node,
        &["class_specifier", "struct_specifier", "field_declaration"],
    ) {
        return true;
    }
    let text = node.utf8_text(source.as_bytes()).unwrap_or("");
    let Some((qualifier, _)) = text.rsplit_once("::") else {
        return false;
    };
    let qualifier = qualifier
        .split(|ch: char| !is_identifier_char(ch))
        .filter(|part| !part.is_empty())
        .last()
        .unwrap_or("");
    !qualifier.is_empty()
        && (source_contains_type_declaration(source, "class", qualifier)
            || source_contains_type_declaration(source, "struct", qualifier)
            || source_contains_type_declaration(source, "union", qualifier))
}

fn source_contains_type_declaration(source: &str, keyword: &str, name: &str) -> bool {
    source.match_indices(keyword).any(|(idx, _)| {
        if idx > 0
            && source[..idx]
                .chars()
                .next_back()
                .is_some_and(is_identifier_char)
        {
            return false;
        }
        let after_keyword = idx + keyword.len();
        let rest = &source[after_keyword..];
        if !rest
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_whitespace())
        {
            return false;
        }
        rest.trim_start()
            .strip_prefix(name)
            .is_some_and(|tail| tail.chars().next().is_none_or(|ch| !is_identifier_char(ch)))
    })
}

fn ast_has_ancestor_any(node: tree_sitter::Node<'_>, kinds: &[&str]) -> bool {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if kinds.iter().any(|kind| candidate.kind() == *kind) {
            return true;
        }
        current = candidate.parent();
    }
    false
}

fn ast_descendant_has_kind(node: tree_sitter::Node<'_>, kind: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == kind || ast_descendant_has_kind(child, kind) {
            return true;
        }
    }
    false
}

fn ast_node_name_for_language(
    language: &'static str,
    node: tree_sitter::Node<'_>,
    source: &str,
) -> Option<String> {
    if language == "cpp" {
        ast_cpp_node_name(node, source).or_else(|| ast_node_name(node, source))
    } else {
        ast_node_name(node, source)
    }
}

fn ast_cpp_node_name(node: tree_sitter::Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "template_declaration"
        | "function_definition"
        | "declaration"
        | "field_declaration"
        | "init_declarator" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if matches!(
                    child.kind(),
                    "function_declarator"
                        | "init_declarator"
                        | "qualified_identifier"
                        | "class_specifier"
                        | "struct_specifier"
                ) {
                    if let Some(name) = ast_cpp_node_name(child, source) {
                        return Some(name);
                    }
                }
            }
            None
        }
        "function_declarator"
        | "reference_declarator"
        | "pointer_declarator"
        | "parenthesized_declarator" => node
            .child_by_field_name("declarator")
            .and_then(|child| ast_cpp_node_name(child, source))
            .or_else(|| {
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    if let Some(name) = ast_cpp_node_name(child, source) {
                        return Some(name);
                    }
                }
                None
            }),
        "qualified_identifier" => {
            let mut cursor = node.walk();
            let mut last = None;
            for child in node.named_children(&mut cursor) {
                if let Some(name) = ast_cpp_node_name(child, source) {
                    last = Some(name);
                }
            }
            last
        }
        "identifier" | "field_identifier" | "type_identifier" | "destructor_name"
        | "operator_name" => node
            .utf8_text(source.as_bytes())
            .ok()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty()),
        _ => None,
    }
}

fn ast_node_name(node: tree_sitter::Node<'_>, source: &str) -> Option<String> {
    if matches!(node.kind(), "function_definition" | "function_declarator") {
        if let Some(declarator) = node.child_by_field_name("declarator") {
            if let Some(name) = ast_first_identifier_text(declarator, source) {
                return Some(name.to_owned());
            }
        }
    }
    node.child_by_field_name("name")
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .or_else(|| ast_first_identifier_text(node, source))
        .map(ToOwned::to_owned)
}

fn ast_first_identifier_text<'a>(node: tree_sitter::Node<'_>, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "identifier"
                | "field_identifier"
                | "property_identifier"
                | "type_identifier"
                | "constant"
                | "simple_identifier"
                | "name"
                | "variable_name"
        ) {
            if let Ok(text) = child.utf8_text(source.as_bytes()) {
                return Some(text);
            }
        }
        if let Some(text) = ast_first_identifier_text(child, source) {
            return Some(text);
        }
    }
    None
}

fn ast_has_ancestor_kind(node: tree_sitter::Node<'_>, kind: &str) -> bool {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == kind {
            return true;
        }
        current = candidate.parent();
    }
    false
}

fn ast_symbol_visibility(
    language: &'static str,
    node: tree_sitter::Node<'_>,
    source: &str,
) -> &'static str {
    let node_text = node.utf8_text(source.as_bytes()).unwrap_or("");
    let name = ast_node_name(node, source).unwrap_or_default();
    match language {
        "python" | "ruby" => {
            if name.starts_with('_') && !name.starts_with("__") {
                "private"
            } else {
                "public"
            }
        }
        "go" => name
            .chars()
            .next()
            .map(|ch| {
                if ch.is_uppercase() {
                    "exported"
                } else {
                    "private"
                }
            })
            .unwrap_or("unknown"),
        "rust" => {
            if node_text.trim_start().starts_with("pub ")
                || node_text.contains(" pub ")
                || node_text.contains("pub(")
            {
                "public"
            } else {
                "private"
            }
        }
        "javascript" | "typescript" | "tsx" => {
            if node_text.trim_start().starts_with("export ") || node_text.contains("\nexport ") {
                "exported"
            } else {
                "public"
            }
        }
        "java" | "csharp" | "php" => {
            if node_text.contains(" private ") || node_text.trim_start().starts_with("private ") {
                "private"
            } else if node_text.contains(" public ")
                || node_text.trim_start().starts_with("public ")
            {
                "public"
            } else {
                "unknown"
            }
        }
        "c" => {
            let visibility_text = ast_c_visibility_text(node, source);
            if visibility_text.trim_start().starts_with("static ")
                || visibility_text.contains("\nstatic ")
                || visibility_text.contains(" static ")
            {
                "private"
            } else {
                "public"
            }
        }
        "cpp" => ast_cpp_symbol_visibility(node, source),
        _ => "unknown",
    }
}

fn ast_cpp_symbol_visibility(node: tree_sitter::Node<'_>, source: &str) -> &'static str {
    let node_text = node.utf8_text(source.as_bytes()).unwrap_or("");
    if node_text.trim_start().starts_with("static ") || node_text.contains("\nstatic ") {
        return "private";
    }
    let Some(class_node) = ast_nearest_ancestor_any(node, &["class_specifier", "struct_specifier"])
    else {
        return "public";
    };
    let mut visibility = if class_node.kind() == "struct_specifier" {
        "public"
    } else {
        "private"
    };
    let target_start = node.start_byte();
    let body = class_node
        .child_by_field_name("body")
        .or_else(|| ast_first_child_kind(class_node, "field_declaration_list"));
    if let Some(body) = body {
        let mut cursor = body.walk();
        for child in body.named_children(&mut cursor) {
            if child.start_byte() >= target_start {
                break;
            }
            if child.kind() == "access_specifier" {
                let text = child.utf8_text(source.as_bytes()).unwrap_or("").trim();
                visibility = if text.starts_with("public") {
                    "public"
                } else if text.starts_with("protected") {
                    "protected"
                } else {
                    "private"
                };
            }
        }
    }
    visibility
}

fn ast_nearest_ancestor_any<'a>(
    node: tree_sitter::Node<'a>,
    kinds: &[&str],
) -> Option<tree_sitter::Node<'a>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if kinds.iter().any(|kind| candidate.kind() == *kind) {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

fn ast_first_child_kind<'a>(
    node: tree_sitter::Node<'a>,
    kind: &str,
) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = node.walk();
    let found = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == kind);
    found
}

fn ast_c_visibility_text(node: tree_sitter::Node<'_>, source: &str) -> String {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if matches!(
            candidate.kind(),
            "function_definition" | "declaration" | "preproc_function_def" | "preproc_def"
        ) {
            return candidate
                .utf8_text(source.as_bytes())
                .unwrap_or("")
                .to_string();
        }
        current = candidate.parent();
    }
    node.utf8_text(source.as_bytes()).unwrap_or("").to_string()
}

fn ast_symbol_kind_for_node_kind(node_kind: &str) -> &'static str {
    match node_kind {
        "function" => "function",
        "method" => "method",
        "class" => "class",
        "test" => "test_case",
        "module" => "module",
        "import" | "assignment" | "call" => "module",
        "config" => "config_key",
        "none" => "none",
        "mixed" => "mixed",
        _ => "unknown",
    }
}

fn dynamicjepa_patch_delta_kind(
    path: &str,
    node_kind: &'static str,
    edit_kind: &'static str,
    symbol_visibility: &'static str,
    changed_text: &str,
) -> &'static str {
    let lower = changed_text.to_ascii_lowercase();
    if swe_path_bucket(&[path.to_string()], &[]) == "config_or_build" {
        return "config";
    }
    if path_is_test(path) && contains_any(&lower, &["assert", "expect", "should", "pytest"]) {
        return "test_assertion";
    }
    if edit_kind == "signature"
        || (matches!(symbol_visibility, "public" | "exported")
            && contains_any(&lower, &["pub ", "public ", "export "]))
    {
        return "api_contract";
    }
    if contains_any(
        &lower,
        &["return ", "return\n", "->", "=>", " yield ", "yield "],
    ) {
        return "return_contract";
    }
    if contains_any(
        &lower,
        &[
            "if ", "elif ", "else:", "for ", "while ", "match ", "switch", "case ", "?",
        ],
    ) {
        return "control_flow";
    }
    if contains_any(
        &lower,
        &[
            "except", "catch", "raise ", "throw ", "err(", "error", "result<", "panic!",
        ],
    ) {
        return "error_handling";
    }
    if matches!(node_kind, "import")
        || contains_any(&lower, &["import ", "from ", "use ", "#include", "require"])
    {
        return "import_dependency";
    }
    if matches!(node_kind, "assignment") {
        return "data_shape";
    }
    if changed_text.trim().is_empty() {
        "none"
    } else {
        "behavior_logic"
    }
}

fn dynamicjepa_control_flow_delta(changed_text: &str) -> &'static str {
    let lower = changed_text.to_ascii_lowercase();
    if contains_any(
        &lower,
        &[
            "if ", "elif ", "else:", "for ", "while ", "match ", "switch", "case ", "break",
            "continue", "try:", "catch", "except",
        ],
    ) {
        "changed"
    } else if changed_text.trim().is_empty() {
        "none"
    } else {
        "unchanged"
    }
}

fn dynamicjepa_api_contract_delta(
    edit_kind: &'static str,
    symbol_visibility: &'static str,
    changed_text: &str,
) -> &'static str {
    let lower = changed_text.to_ascii_lowercase();
    if edit_kind == "signature" {
        "signature_changed"
    } else if matches!(symbol_visibility, "public" | "exported")
        && contains_any(&lower, &["pub ", "public ", "export ", "private "])
    {
        "visibility_changed"
    } else if contains_any(&lower, &["return ", "->", "=>", ": "]) {
        "return_changed"
    } else if changed_text.trim().is_empty() {
        "none"
    } else {
        "unchanged"
    }
}

fn dynamicjepa_dependency_blast_radius(path: &str, reverse_reference_count: i64) -> &'static str {
    if reverse_reference_count <= 0 {
        "single_file"
    } else if reverse_reference_count <= 2 {
        "local_symbol"
    } else if reverse_reference_count <= 12 {
        "cross_file"
    } else if swe_dependency_layer(&[path.to_string()], &[]) != "unknown" {
        "cross_layer"
    } else {
        "cross_file"
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn dynamicjepa_reverse_reference_count(
    repo_dir: &Path,
    edited_path: &str,
    symbols: &BTreeSet<String>,
) -> Result<i64> {
    if symbols.is_empty() {
        return Ok(0);
    }
    let mut files = Vec::new();
    collect_dynamicjepa_source_files(repo_dir, repo_dir, &mut files)?;
    let mut count = 0i64;
    for path in files {
        let rel = path.strip_prefix(repo_dir).map_err(|err| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_SEMANTIC_RELATIVE_PATH_FAILED",
                "semantic dependency graph could not derive a repo-relative path",
                "dynamicjepa.action.semantic.source_path",
                "inspect repository checkout path canonicalization",
                json!({"path": path_str(&path), "repo_dir": path_str(repo_dir), "error": err.to_string()}),
                Some(file_sot(repo_dir)),
            )
        })?;
        let rel = rel.to_string_lossy().replace('\\', "/");
        if rel == edited_path {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|err| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_SEMANTIC_SOURCE_READ_FAILED",
                "semantic dependency graph could not read a source file",
                "dynamicjepa.action.semantic.repository_source",
                "remove unreadable generated/binary files from source paths or fix checkout permissions",
                json!({"path": file_sot(&path), "error": err.to_string()}),
                Some(file_sot(&path)),
            )
        })?;
        for symbol in symbols {
            count = count.saturating_add(count_identifier_occurrences(&text, symbol) as i64);
        }
    }
    Ok(count.min(10_000))
}

fn collect_dynamicjepa_source_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_SEMANTIC_READDIR_FAILED",
            "semantic dependency graph could not enumerate repository files",
            "dynamicjepa.action.semantic.repository_walk",
            "fix repository checkout permissions before training or pre-action control",
            json!({"dir": file_sot(dir), "error": err.to_string()}),
            Some(file_sot(dir)),
        )
    })? {
        let entry = entry.map_err(|err| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_SEMANTIC_DIRENTRY_FAILED",
                "semantic dependency graph could not read a repository directory entry",
                "dynamicjepa.action.semantic.repository_walk",
                "fix repository checkout permissions before training or pre-action control",
                json!({"dir": file_sot(dir), "error": err.to_string()}),
                Some(file_sot(dir)),
            )
        })?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" || name == "target" || name == "node_modules" || name == "__pycache__" {
            continue;
        }
        let file_type = entry.file_type().map_err(|err| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_SEMANTIC_FILETYPE_FAILED",
                "semantic dependency graph could not inspect a repository path type",
                "dynamicjepa.action.semantic.repository_walk.file_type",
                "fix repository checkout permissions before training or pre-action control",
                json!({"path": file_sot(&path), "error": err.to_string()}),
                Some(file_sot(&path)),
            )
        })?;
        if file_type.is_dir() {
            collect_dynamicjepa_source_files(root, &path, out)?;
        } else if file_type.is_file() {
            let rel = path.strip_prefix(root).map_err(|err| {
                RealityError::new(
                    "RUST_REALITY_DYNAMICJEPA_SEMANTIC_RELATIVE_PATH_FAILED",
                    "semantic dependency graph could not derive a repo-relative path",
                    "dynamicjepa.action.semantic.source_path",
                    "inspect repository checkout path canonicalization",
                    json!({"path": path_str(&path), "repo_dir": path_str(root), "error": err.to_string()}),
                    Some(file_sot(root)),
                )
            })?;
            let rel = rel.to_string_lossy().replace('\\', "/");
            if swe_ast_language_for_path(&rel).is_some() {
                out.push(path);
            }
        }
    }
    Ok(())
}

fn count_identifier_occurrences(text: &str, symbol: &str) -> usize {
    if symbol.is_empty() {
        return 0;
    }
    text.match_indices(symbol)
        .filter(|(idx, _)| {
            let before = if *idx == 0 {
                None
            } else {
                text[..*idx].chars().next_back()
            };
            let after = text[*idx + symbol.len()..].chars().next();
            !before.is_some_and(is_identifier_char) && !after.is_some_and(is_identifier_char)
        })
        .count()
}

fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn dynamicjepa_ast_edit_kind_for_range(
    node_kind: &'static str,
    node_start_line: u32,
    edit_start_line: u32,
) -> &'static str {
    match node_kind {
        "function" | "method" if edit_start_line <= node_start_line => "signature",
        "function" | "method" => "function_body",
        "class" if edit_start_line <= node_start_line => "signature",
        "class" => "class_body",
        "import" => "import_section",
        "call" => "call_site",
        "assignment" => "assignment",
        "test" => "test_case",
        "module" => "module_scope",
        "config" => "config",
        "none" => "none",
        "mixed" => "mixed",
        _ => "unknown",
    }
}

fn path_is_test(path: &str) -> bool {
    path.starts_with("tests/")
        || path.contains("/tests/")
        || path.ends_with("_test.py")
        || path.ends_with(".test.js")
        || path.ends_with(".spec.js")
        || path.ends_with(".test.ts")
        || path.ends_with(".spec.ts")
        || path.ends_with(".test.tsx")
        || path.ends_with(".spec.tsx")
        || java_path_is_test(path)
}

fn java_path_is_test(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    if normalized.starts_with("src/test/java/")
        || normalized.contains("/src/test/java/")
        || normalized.starts_with("src/it/java/")
        || normalized.contains("/src/it/java/")
    {
        return true;
    }

    let file_name = normalized.rsplit('/').next().unwrap_or(&normalized);
    file_name.ends_with(".java")
        && (file_name.starts_with("Test")
            || file_name.ends_with("Test.java")
            || file_name.ends_with("Tests.java")
            || file_name.ends_with("TestCase.java")
            || file_name.starts_with("IT")
            || file_name.ends_with("IT.java")
            || file_name.ends_with("ITCase.java"))
}

fn java_method_has_test_annotation(node: tree_sitter::Node<'_>, source: &str) -> bool {
    if node.kind() != "method_declaration" {
        return false;
    }
    let mut cursor = node.walk();
    let found = node.children(&mut cursor).any(|child| {
        matches!(
            child.kind(),
            "modifiers" | "annotation" | "marker_annotation"
        ) && java_tree_has_test_annotation(child, source)
    });
    found
}

fn java_tree_has_test_annotation(node: tree_sitter::Node<'_>, source: &str) -> bool {
    if matches!(node.kind(), "annotation" | "marker_annotation")
        && node
            .utf8_text(source.as_bytes())
            .ok()
            .and_then(java_annotation_simple_name)
            .is_some_and(java_annotation_is_test)
    {
        return true;
    }
    let mut cursor = node.walk();
    let found = node
        .children(&mut cursor)
        .any(|child| java_tree_has_test_annotation(child, source));
    found
}

fn java_annotation_simple_name(text: &str) -> Option<&str> {
    let trimmed = text.trim_start();
    let after_at = trimmed.strip_prefix('@')?;
    let end = after_at
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'))
        .unwrap_or(after_at.len());
    let qualified = &after_at[..end];
    if qualified.is_empty() {
        None
    } else {
        Some(qualified.rsplit('.').next().unwrap_or(qualified))
    }
}

fn java_annotation_is_test(name: &str) -> bool {
    matches!(
        name,
        "Test" | "ParameterizedTest" | "RepeatedTest" | "TestFactory" | "TestTemplate"
    )
}

fn swe_patch_intent_bucket(tool_name: &str, patch_bytes: i64, path_risk: &str) -> &'static str {
    if !is_final_edit_tool(tool_name) {
        return "none";
    }
    match path_risk {
        "multiple" => "multi_file",
        "tests" => "test_adjustment",
        "config_or_build" => "config_update",
        _ if patch_bytes > 16_384 => "refactor",
        "source" => "localized_fix",
        "none" => "none",
        _ => "unknown",
    }
}

fn swe_patch_shape_bucket(tool_name: &str, arguments: &Value, paths: &[String]) -> &'static str {
    match tool_name {
        "apply_line_window_edit" => "line_window",
        "apply_structured_edit" => "structured_replace",
        "apply_unified_diff" => {
            let hunk_count = arguments
                .get("patch")
                .and_then(Value::as_str)
                .map(|patch| patch.lines().filter(|line| line.starts_with("@@")).count())
                .unwrap_or(0);
            if paths.len() > 1 {
                "unified_multi_file"
            } else if hunk_count > 1 {
                "unified_multi_hunk"
            } else if hunk_count == 1 {
                "unified_single_hunk"
            } else {
                "unknown"
            }
        }
        _ => "none",
    }
}

fn swe_patch_scope_bucket(tool_name: &str, paths: &[String]) -> &'static str {
    if !is_final_edit_tool(tool_name) || paths.is_empty() {
        return "none";
    }
    if paths.len() == 1 {
        return match tool_name {
            "apply_line_window_edit" | "apply_structured_edit" => "single_symbol",
            _ => "single_file",
        };
    }
    let layers = paths
        .iter()
        .map(|path| swe_dependency_layer(std::slice::from_ref(path), &[]))
        .collect::<BTreeSet<_>>();
    if layers.len() > 1 {
        "cross_layer"
    } else {
        "multi_file"
    }
}

fn swe_patch_scope_bucket_from_ast_or_path(
    tool_name: &str,
    paths: &[String],
    ast: &SweAstEditSemantics,
) -> &'static str {
    if !is_final_edit_tool(tool_name) || paths.is_empty() {
        return "none";
    }
    if paths.len() == 1 && ast.parse_status == "parsed" {
        return match ast.symbol_kind {
            "function" | "method" | "class" | "test_case" => "single_symbol",
            "module" | "config_key" => "single_file",
            _ => swe_patch_scope_bucket(tool_name, paths),
        };
    }
    swe_patch_scope_bucket(tool_name, paths)
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
                "inner_harness_response": file_sot(response_path)
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
            "domain_pack_id": DYNAMICJEPA_REALITY_DOMAIN_PACK,
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
    // /tmp/ccreality-prod-3/rust_reality_1778039147061721975_e525987d where the inner model
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
            // the field names so the inner model actually sees the SyntaxError text.
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
        "action": "candidate patch UPDATED with this structured edit. Workspace reset to base per harness contract — your next open_file_window will return BASE FILE CONTENT (not your edit). The post_edit_window field in this response is the AUTHORITATIVE view of your current candidate. Calling any apply_* tool again REPLACES the current candidate, it does not stack. The current candidate runs in the official Docker eval at end-of-attempt.",
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
    // tail from the compilation detail to the top level of the tool result so the inner model
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
            // the field names so the inner model actually sees the SyntaxError text.
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
        "action": "candidate patch UPDATED with this line-window edit. Workspace reset to base per harness contract — your next open_file_window will return BASE FILE CONTENT (not your edit). The post_edit_window field in this response is the AUTHORITATIVE view of your current candidate. Calling any apply_* tool again REPLACES the current candidate, it does not stack. The current candidate runs in the official Docker eval at end-of-attempt.",
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
        "action": "candidate patch UPDATED with this unified diff. Workspace reset to base per harness contract — your next open_file_window will return BASE FILE CONTENT (not your edit). The post_edit_window field (if present) is the AUTHORITATIVE view of your current candidate. Calling any apply_* tool again REPLACES the current candidate, it does not stack. The current candidate runs in the official Docker eval at end-of-attempt.",
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
    // Distinguish pytest exit codes so the inner actor does not misinterpret
    // infrastructure problems (collection errors, missing test deps) as bad patches.
    // Reference: https://docs.pytest.org/en/stable/reference/exit-codes.html
    //   0 = all tests passed
    //   1 = test assertion failure
    //   2 = test execution interrupted
    //   3 = pytest internal error
    //   4 = pytest UsageError (e.g., conftest ImportError, missing test dep like 'hypothesis')
    //   5 = no tests collected (test_id did not match any test)
    let mut response = json!({
        "status": match process.exit_code {
            0 => "ok",
            1 | 2 | 3 => "failed",
            4 | 5 => "infra_unavailable",
            _ => "failed",
        },
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
    });
    if let Some(obj) = response.as_object_mut() {
        match process.exit_code {
            1 => {
                obj.insert("real_test_failure".to_string(), json!(true));
            }
            2 => {
                obj.insert("interrupted".to_string(), json!(true));
            }
            3 => {
                obj.insert("internal_error".to_string(), json!(true));
            }
            4 => {
                obj.insert("collection_error".to_string(), json!(true));
                obj.insert(
                    "note".to_string(),
                    json!("Test collection failed (often due to missing test infra dep such as hypothesis or a pytest plugin). This does NOT indicate a bad patch. If your apply_line_window_edit returned status:ok, your patch is already submitted; the official eval runs at end-of-attempt automatically."),
                );
            }
            5 => {
                obj.insert("no_tests_collected".to_string(), json!(true));
                obj.insert(
                    "note".to_string(),
                    json!("pytest collected zero tests for this test_id. Verify the test_id matches a FAIL_TO_PASS entry from the task manifest. This does NOT indicate a bad patch."),
                );
            }
            _ => {}
        }
    }
    Ok(response)
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
        "this tool only serves PRIOR completed-attempt evidence; official eval for the CURRENT attempt runs automatically when the attempt terminates. If your apply_line_window_edit returned status:ok, your patch is already submitted.",
        "stop calling run_official_swe_eval and stop re-applying edits; allow the attempt to terminate so the outer loop can run the official eval automatically",
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
        let find_byte_start = before.find(find).unwrap_or(0);
        let after = before.replacen(find, replace, 1);
        let edit_start_line = before[..find_byte_start].matches('\n').count() as u64 + 1;
        let replace_newlines = replace.matches('\n').count() as u64;
        let edit_end_line = if replace.is_empty() {
            edit_start_line
        } else if replace.ends_with('\n') {
            edit_start_line + replace_newlines.saturating_sub(1)
        } else {
            edit_start_line + replace_newlines
        };
        original_contents
            .entry(path.to_string())
            .or_insert_with(|| before.clone());
        write_text_checked(&target, &after)?;
        let post_edit_window =
            build_post_edit_window(path, &after, edit_start_line, edit_end_line, &target);
        applied.push(json!({
            "index": index,
            "path": path,
            "before_sha256": sha256_text(&before),
            "after_sha256": sha256_text(&after),
            "find_sha256": sha256_text(find),
            "replace_sha256": sha256_text(replace),
            "post_edit_window": post_edit_window,
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
            Some(value) if value.starts_with("sha256:") => value.to_string(),
            Some(value)
                if value.len() == 64 && value.as_bytes().iter().all(|b| b.is_ascii_hexdigit()) =>
            {
                // Lenient normalization: bare 64-hex is accepted and prefixed
                // internally. Downstream byte-for-byte comparison still validates                // the actual hash, so this introduces zero integrity risk.
                format!("sha256:{value}")
            }
            Some(value) => {
                return Ok(structured_edit_failure(
                    "RUST_REALITY_LINE_EDIT_OBSERVED_SHA_FORMAT_INVALID",
                    &format!("{field_prefix}.observed_sha256"),
                    "line-window edit observed_sha256 has an unrecognized format",
                    "observed_sha256 must be 'sha256:<64-hex>' or bare '<64-hex>' (will be normalized)",
                    json!({"path": path, "received": value}),
                    &edits_path,
                ));
            }
            None => {
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
        // line_spans uses split_inclusive('\n'), so byte_end points to the first byte
        // of the line AFTER end_line (i.e. before[byte_end..] begins with that line's
        // text content, not a leading '\n'). If the model's replace text does not end
        // with '\n' and we are not at end-of-file, we must insert the missing newline
        // separator; otherwise replace's last line silently merges with the next line,
        // producing invalid source (e.g. two Python statements concatenated on one line).
        if !replace.ends_with('\n') && byte_end < before.len() {
            after.push('\n');
        }
        after.push_str(&before[byte_end..]);
        original_contents
            .entry(path.to_string())
            .or_insert_with(|| before.clone());
        write_text_checked(&target, &after)?;
        let post_edit_window = build_post_edit_window(path, &after, start_line, end_line, &target);
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
            "post_edit_window": post_edit_window,
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

    let hunk_ranges = parse_unified_diff_post_apply_ranges(patch);
    let applied = changed_files
        .iter()
        .map(|path| {
            let target = repo_dir.join(path);
            let after_sha = sha256_file(&target)?;
            let mut entry = json!({
                "path": path,
                "observed_sha256": observed.get(path),
                "after_sha256": after_sha,
                "source_of_truth": file_sot(&target)
            });
            if let Some(&(start, end)) = hunk_ranges.get(path) {
                if let Ok(after_text) = fs::read_to_string(&target) {
                    let window = build_post_edit_window(path, &after_text, start, end, &target);
                    entry
                        .as_object_mut()
                        .expect("entry is object")
                        .insert("post_edit_window".to_string(), window);
                }
            }
            Ok(entry)
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
            json!({
                "changed_files": changed_files,
                "static_checks": static_checks,
                "applied_edits": applied
            }),
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

/// Parse a unified diff and return per-file post-apply line ranges as the
/// union of all hunks for that file. Hunk header form: `@@ -X,Y +A,B @@` (Y/B
/// optional; default 1). Returned `(start, end)` is inclusive 1-based against
/// the post-apply file.
fn parse_unified_diff_post_apply_ranges(patch: &str) -> BTreeMap<String, (u64, u64)> {
    let mut ranges: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    let mut current_path: Option<String> = None;
    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("+++ ") {
            // Strip optional "b/" git prefix and any trailing tab metadata.
            let path_part = rest.split('\t').next().unwrap_or(rest).trim();
            let cleaned = path_part.strip_prefix("b/").unwrap_or(path_part);
            if cleaned == "/dev/null" || cleaned.is_empty() {
                current_path = None;
            } else {
                current_path = Some(cleaned.to_string());
            }
            continue;
        }
        if !line.starts_with("@@") {
            continue;
        }
        let Some(path) = current_path.as_ref() else {
            continue;
        };
        // Find the "+" segment of the hunk header.
        let Some(plus_idx) = line.find('+') else {
            continue;
        };
        let after_plus = &line[plus_idx + 1..];
        let nums = after_plus.split_whitespace().next().unwrap_or("");
        let mut parts = nums.split(',');
        let start: u64 = match parts.next().and_then(|s| s.parse().ok()) {
            Some(v) => v,
            None => continue,
        };
        let count: u64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(1);
        // For empty target (count=0) the hunk's anchor is `start` itself; treat
        // it as a single-line window so the readback still has signal.
        let end = if count == 0 { start } else { start + count - 1 };
        ranges
            .entry(path.clone())
            .and_modify(|cur| {
                cur.0 = cur.0.min(start);
                cur.1 = cur.1.max(end);
            })
            .or_insert((start, end));
    }
    ranges
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

/// Build a post-edit window snippet so the model can verify what its edit produced.
///
/// Returns a JSON object with `path`, `start_line` (1-based), `end_line` (1-based),
/// `total_lines`, and `lines: [{n, text}]` covering `edit_start - 5` through
/// `edit_end + 5` (clamped to file bounds). Each line's `text` is the raw line
/// content with any trailing `\n` stripped. Output is bounded to MAX_LINES so
/// pathological large edits do not bloat tool transcripts.
fn build_post_edit_window(
    path: &str,
    after_text: &str,
    edit_start_line: u64,
    edit_end_line: u64,
    full_path: &Path,
) -> Value {
    const CONTEXT: u64 = 5;
    const MAX_LINES: usize = 500;
    const MAX_BYTES: usize = 20_000;
    let spans = line_spans(after_text);
    let total_lines = spans.len();
    if total_lines == 0 {
        return json!({
            "path": path,
            "start_line": 0,
            "end_line": 0,
            "total_lines": 0,
            "lines": [],
            "source_of_truth": file_sot(full_path)
        });
    }
    let total_lines_u64 = total_lines as u64;
    let window_start = edit_start_line.saturating_sub(CONTEXT).max(1);
    let window_end = edit_end_line.saturating_add(CONTEXT).min(total_lines_u64);
    let mut lines = Vec::new();
    let mut bytes_used = 0usize;
    let mut truncated = false;
    for n in window_start..=window_end {
        if lines.len() >= MAX_LINES {
            truncated = true;
            break;
        }
        let idx = (n - 1) as usize;
        let (lo, hi) = spans[idx];
        let raw = &after_text[lo..hi];
        let text = raw.strip_suffix('\n').unwrap_or(raw);
        let text = text.strip_suffix('\r').unwrap_or(text);
        if bytes_used + text.len() > MAX_BYTES {
            truncated = true;
            break;
        }
        bytes_used += text.len();
        lines.push(json!({"n": n, "text": text}));
    }
    json!({
        "path": path,
        "start_line": window_start,
        "end_line": window_end,
        "total_lines": total_lines_u64,
        "lines": lines,
        "truncated": truncated,
        "source_of_truth": file_sot(full_path)
    })
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
    // C2: best-effort Docker container cleanup keyed on this eval's --run_id.
    // SWE-bench's harness normally cleans its own containers on graceful exit,
    // but a SIGKILL of the python proc (run_cmd_env timeout, kernel OOM, panic)
    // can leak containers labeled with the run_id. C1's process_group + 2s
    // SIGTERM grace mitigates the common case; this is the defense-in-depth.
    // Failures are silent — a stray container is not a reason to fail the eval.
    cleanup_swebench_containers_for_run(&run_id);
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

/// Best-effort cleanup of any Docker containers whose names include this run_id.
/// SWE-bench's evaluator names containers like `sweb.eval.<instance>.<run_id>.<...>`,
/// so a substring filter on `name=<run_id>` reliably scopes cleanup to this eval.
/// Silent on errors — never propagates back to the caller. Skips entirely if the
/// docker binary is missing.
fn cleanup_swebench_containers_for_run(run_id: &str) {
    if run_id.is_empty() {
        return;
    }
    let filter = format!("name={}", run_id);
    let list = match std::process::Command::new("docker")
        .args(["ps", "-aq", "--filter", &filter])
        .output()
    {
        Ok(o) => o,
        Err(_) => return,
    };
    if !list.status.success() {
        return;
    }
    let ids: Vec<String> = String::from_utf8_lossy(&list.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if ids.is_empty() {
        return;
    }
    let mut args: Vec<String> = vec!["rm".to_string(), "-f".to_string()];
    args.extend(ids);
    let _ = std::process::Command::new("docker").args(&args).output();
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
    dynamicjepa_reality: &Value,
    dynamicjepa_reality_record: &Value,
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
    let dynamicjepa_intervention_hint = derive_dynamicjepa_intervention_hint(
        dynamicjepa_reality,
        validation,
        official_evidence.as_ref(),
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
        "dynamicjepa_reality": {
            "block": dynamicjepa_reality,
            "ledger_record": dynamicjepa_reality_record,
            "intervention_hint": dynamicjepa_intervention_hint,
            "source_of_truth": {
                "dynamicjepa_reality_file": optional_file_sot(&attempt_dir.join("dynamicjepa-reality.json")),
                "blackbox_ledger": dynamicjepa_reality_record.get("source_of_truth")
            }
        },
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
                "dynamicjepa_reality.block.reward_features",
                "dynamicjepa_reality.intervention_hint",
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
            "agent_tool_transcript": agent_tool_transcript_path,
            "dynamicjepa_reality": optional_file_sot(&attempt_dir.join("dynamicjepa-reality.json")),
            "dynamicjepa_reality_ledger": dynamicjepa_reality_record.get("source_of_truth")
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
        "dynamicjepa_reality": {
            "block": {
                "source_mode": packet.pointer("/dynamicjepa_reality/block/source_mode"),
                "panel_count": packet.pointer("/dynamicjepa_reality/block/panel_count"),
                "surprise_event_count": packet.pointer("/dynamicjepa_reality/block/surprise_event_count"),
                "surprise_candidate_rate": packet.pointer("/dynamicjepa_reality/block/surprise_candidate_rate"),
                "projection_surprise_rate": packet.pointer("/dynamicjepa_reality/block/projection_surprise_rate"),
                "trained_surprise_rate": packet.pointer("/dynamicjepa_reality/block/trained_surprise_rate"),
                "projection_accuracy": packet.pointer("/dynamicjepa_reality/block/projection_accuracy"),
                "prediction_error_l2": packet.pointer("/dynamicjepa_reality/block/prediction_error_l2"),
                "plan_trace_coverage": packet.pointer("/dynamicjepa_reality/block/plan_trace_coverage"),
                "reward_features": packet.pointer("/dynamicjepa_reality/block/reward_features")
            },
            "intervention_hint": packet.pointer("/dynamicjepa_reality/intervention_hint"),
            "ledger_record": {
                "status": packet.pointer("/dynamicjepa_reality/ledger_record/status"),
                "ledger_sequence": packet.pointer("/dynamicjepa_reality/ledger_record/ledger_sequence"),
                "readback": packet.pointer("/dynamicjepa_reality/ledger_record/readback/status")
            }
        },
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

fn derive_dynamicjepa_intervention_hint(
    dynamicjepa_reality: &Value,
    validation: &Value,
    official_evidence: Option<&Value>,
) -> Value {
    let source_mode = dynamicjepa_reality
        .get("source_mode")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let surprise_rate = dynamicjepa_reality
        .get("trained_surprise_rate")
        .or_else(|| dynamicjepa_reality.get("projection_surprise_rate"))
        .or_else(|| dynamicjepa_reality.get("surprise_candidate_rate"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let prediction_error_l2 = dynamicjepa_reality
        .get("prediction_error_l2")
        .and_then(Value::as_f64);
    let plan_trace_coverage = dynamicjepa_reality
        .get("plan_trace_coverage")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let resolved = official_evidence
        .and_then(|value| value.get("resolved"))
        .and_then(Value::as_bool);
    let validation_status = validation.get("status").and_then(Value::as_str);
    let validation_error_code = validation.get("error_code").and_then(Value::as_str);
    let unresolved =
        resolved == Some(false) || matches!(validation_status, Some("failed" | "error"));

    if source_mode != "trained_dynamicjepa_artifact" {
        return json!({
            "status": "capability_gap",
            "intervention_surface": "dynamicjepa_artifact_training",
            "recommendation": "train_and_pass_contextgraph_swe_reality_loop_v1_artifact",
            "confidence": 1.0,
            "reason": "DynamicJEPA reality is projection evidence, not learned world-model inference",
            "observed": {
                "source_mode": source_mode,
                "projection_surprise_rate": dynamicjepa_reality.get("projection_surprise_rate"),
                "trained_surprise_rate": dynamicjepa_reality.get("trained_surprise_rate"),
                "prediction_error_l2": prediction_error_l2,
                "plan_trace_coverage": plan_trace_coverage,
            },
        });
    }

    let high_surprise =
        surprise_rate >= 0.30 || prediction_error_l2.is_some_and(|value| value >= 0.35);
    let low_surprise =
        surprise_rate <= 0.10 && prediction_error_l2.is_some_and(|value| value <= 0.10);
    let surprise_tool_families = dynamicjepa_reality
        .get("surprise_events")
        .and_then(Value::as_array)
        .map(|events| {
            let mut counts = BTreeMap::<String, u64>::new();
            for event in events {
                if let Some(tool_family) = event.get("tool_family").and_then(Value::as_str) {
                    *counts.entry(tool_family.to_string()).or_insert(0) += 1;
                }
            }
            counts
        })
        .unwrap_or_default();
    let guard_decision_count = dynamicjepa_reality
        .get("guard_decisions")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);

    let (status, intervention_surface, recommendation, reason, confidence) = if low_surprise
        && unresolved
    {
        (
            "model_bottleneck",
            "inner_model",
            "escalate_model_or_task_context",
            "trained DynamicJEPA surprise is low while the attempt remains unresolved",
            0.85,
        )
    } else if high_surprise && guard_decision_count > 0 {
        (
            "actionable",
            "tool_guard_surface",
            "inspect_tool_guard_or_tool_spec",
            "trained DynamicJEPA surprise coincides with guard decisions",
            0.90,
        )
    } else if high_surprise && !surprise_tool_families.is_empty() {
        (
            "actionable",
            "tool_surface",
            "inspect_high_surprise_tool_family",
            "trained DynamicJEPA surprise localizes to one or more tool families",
            0.82,
        )
    } else if high_surprise {
        (
            "actionable",
            "prompt_template",
            "inspect_prompt_state_action_mapping",
            "trained DynamicJEPA surprise is high without a narrower tool or guard localization",
            0.70,
        )
    } else {
        (
            "stable",
            "current_loop",
            "continue_with_verified_failure_evidence",
            "trained DynamicJEPA surprise and prediction error are within the configured low-risk band",
            0.60,
        )
    };

    json!({
        "status": status,
        "intervention_surface": intervention_surface,
        "recommendation": recommendation,
        "confidence": confidence,
        "reason": reason,
        "observed": {
            "source_mode": source_mode,
            "trained_surprise_rate": round_signal(surprise_rate),
            "prediction_error_l2": prediction_error_l2.map(round_signal),
            "plan_trace_coverage": round_signal(plan_trace_coverage),
            "guard_decision_count": guard_decision_count,
            "surprise_tool_family_counts": surprise_tool_families,
            "resolved": resolved,
            "validation_status": validation_status,
            "validation_error_code": validation_error_code,
        },
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

fn build_dynamicjepa_reality_for_attempt(
    run_id: &str,
    attempt: u64,
    transcript_path: Option<&str>,
    ledger: &Path,
    attempt_dir: &Path,
    repo_dir: &Path,
    context_graph_bin: &Path,
    task_root: &Path,
    dynamicjepa_db: Option<&Path>,
    dynamicjepa_artifact_id: Option<&str>,
    dynamicjepa_skill_id: &str,
) -> Result<Value> {
    match (dynamicjepa_db, dynamicjepa_artifact_id) {
        (Some(db), Some(artifact_id)) => {
            return build_trained_dynamicjepa_reality_for_attempt(
                run_id,
                attempt,
                transcript_path,
                ledger,
                attempt_dir,
                repo_dir,
                context_graph_bin,
                task_root,
                db,
                artifact_id,
                dynamicjepa_skill_id,
            );
        }
        (None, None) => {}
        _ => {
            return Err(RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_CONFIG_INCOMPLETE",
                "trained DynamicJEPA mode requires both a RocksDB path and a model artifact id",
                "dynamicjepa",
                "pass --dynamicjepa-db and --dynamicjepa-artifact-id together",
                json!({
                    "dynamicjepa_db": dynamicjepa_db.map(|path| path_str(path)),
                    "dynamicjepa_artifact_id": dynamicjepa_artifact_id,
                }),
                dynamicjepa_db.map(|path| file_sot(path)),
            ));
        }
    }
    let Some(path) = transcript_path else {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_TRANSCRIPT_MISSING",
            "DynamicJEPA reward features require the physical agent tool transcript",
            "model_response.contextgraph_runtime.agent_tool_transcript",
            "rerun with the current governed inner harness so every attempt writes an agent_tool_transcript artifact",
            json!({"run_id": run_id, "attempt": attempt}),
            Some(sqlite_sot(ledger)),
        ));
    };
    let transcript_path = PathBuf::from(path.strip_prefix("file:").unwrap_or(path));
    let transcript: Value = read_json(&transcript_path)?;
    let mut panel_count = 0_u64;
    let mut surprise_events = Vec::new();
    let mut guard_decisions = Vec::new();
    let mut source_refs = vec![json!({
        "kind": "agent_tool_transcript",
        "source_of_truth": file_sot(&transcript_path),
        "sha256": sha256_file(&transcript_path)?,
    })];

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
                                    "RUST_REALITY_DYNAMICJEPA_TRANSITION_REF_MISSING",
                                    "tool transcript entry was missing the harness transition needed for DynamicJEPA reality",
                                    "agent_tool_transcript.entries[].tool_results[].harness_transition",
                                    "rerun with the current Rust reality-loop so every tool call emits a transition artifact",
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
                            "RUST_REALITY_DYNAMICJEPA_TRANSITION_KIND_INVALID",
                            "DynamicJEPA reality transition artifact had the wrong record_kind",
                            "harness_transition.record_kind",
                            "inspect transition artifact provenance and rerun the attempt",
                            json!({
                                "transition": file_sot(&transition_path),
                                "record_kind": transition.get("record_kind")
                            }),
                            Some(file_sot(&transition_path)),
                        ));
                    }
                    let mapping = transition
                        .get("dynamicjepa_mapping")
                        .and_then(Value::as_object)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_MAPPING_MISSING",
                                "harness transition was missing dynamicjepa_mapping",
                                "harness_transition.dynamicjepa_mapping",
                                "regenerate the transition with the current harness before scoring DynamicJEPA reward features",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    let domain_pack = mapping
                        .get("domain_pack_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_DOMAIN_PACK_MISSING",
                                "dynamicjepa_mapping.domain_pack_id was missing",
                                "harness_transition.dynamicjepa_mapping.domain_pack_id",
                                "regenerate the transition with a typed DynamicJEPA domain pack id",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    if domain_pack != DYNAMICJEPA_REALITY_DOMAIN_PACK {
                        return Err(RealityError::new(
                            "RUST_REALITY_DYNAMICJEPA_DOMAIN_PACK_MISMATCH",
                            "dynamicjepa_mapping.domain_pack_id did not match the reality-loop domain pack",
                            "harness_transition.dynamicjepa_mapping.domain_pack_id",
                            "do not mix DynamicJEPA domains inside one reality-loop reward record",
                            json!({
                                "expected": DYNAMICJEPA_REALITY_DOMAIN_PACK,
                                "actual": domain_pack,
                                "transition": file_sot(&transition_path)
                            }),
                            Some(file_sot(&transition_path)),
                        ));
                    }
                    let transition_id = mapping
                        .get("transition_id")
                        .and_then(Value::as_str)
                        .or_else(|| transition.get("transition_id").and_then(Value::as_str))
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_TRANSITION_ID_MISSING",
                                "dynamicjepa_mapping.transition_id was missing",
                                "harness_transition.dynamicjepa_mapping.transition_id",
                                "regenerate the transition with a stable transition_id",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    for (field, value) in [
                        ("state_id", mapping.get("state_id")),
                        ("action_id", mapping.get("action_id")),
                        ("outcome_id", mapping.get("outcome_id")),
                    ] {
                        if value
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|item| !item.is_empty())
                            .is_none()
                        {
                            return Err(RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_MAPPING_FIELD_MISSING",
                                "dynamicjepa_mapping was missing a required state/action/outcome id",
                                format!("harness_transition.dynamicjepa_mapping.{field}"),
                                "regenerate the transition with complete state/action/outcome ids",
                                json!({"field": field, "transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            ));
                        }
                    }

                    panel_count += 1;
                    let transition_sha = sha256_file(&transition_path)?;
                    source_refs.push(json!({
                        "kind": "harness_transition",
                        "transition_id": transition_id,
                        "source_of_truth": file_sot(&transition_path),
                        "sha256": transition_sha,
                    }));
                    let outcome_label = transition
                        .pointer("/outcome/label")
                        .and_then(Value::as_str)
                        .unwrap_or("Unknown");
                    let surprise_kind = mapping
                        .get("surprise_kind")
                        .and_then(Value::as_str)
                        .filter(|kind| !kind.trim().is_empty());
                    if surprise_kind.is_some() || outcome_label == "Harmful" {
                        let utility_delta = transition
                            .pointer("/outcome/utility_delta")
                            .and_then(Value::as_f64)
                            .unwrap_or(0.0);
                        let tool_family = transition
                            .pointer("/constellation_signal/tool_family")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown_tool_family");
                        let event_id = format!("srp-{}", short_hash(transition_id));
                        surprise_events.push(json!({
                            "event_id": event_id,
                            "kind": surprise_kind.unwrap_or("HarmfulOutcome"),
                            "magnitude": round_signal(utility_delta.abs()),
                            "transition_id": transition_id,
                            "tool_name": transition.get("tool_name"),
                            "tool_family": tool_family,
                            "outcome_label": outcome_label,
                            "source_of_truth": file_sot(&transition_path),
                        }));
                    }
                    if transition
                        .pointer("/outcome/correction_required")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        guard_decisions.push(json!({
                            "guard": "correction_required",
                            "decision": "requires_correction",
                            "transition_id": transition_id,
                            "tool_name": transition.get("tool_name"),
                            "outcome_label": outcome_label,
                            "source_of_truth": file_sot(&transition_path),
                        }));
                    }
                }
            }
        }
    }

    let executed = transcript
        .get("executed_tool_calls")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_EXECUTED_COUNT_MISSING",
                "agent tool transcript was missing executed_tool_calls",
                "agent_tool_transcript.executed_tool_calls",
                "rerun with the current inner harness so DynamicJEPA coverage can be measured",
                json!({"transcript": file_sot(&transcript_path)}),
                Some(file_sot(&transcript_path)),
            )
        })?;
    if panel_count != executed {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_PANEL_COUNT_MISMATCH",
            "DynamicJEPA panel count did not equal executed tool-call count",
            "agent_tool_transcript.executed_tool_calls",
            "inspect missing transition artifacts before scoring this attempt",
            json!({
                "executed_tool_calls": executed,
                "panel_count": panel_count,
                "transcript": file_sot(&transcript_path)
            }),
            Some(file_sot(&transcript_path)),
        ));
    }
    source_refs.push(json!({
        "kind": "blackbox_ledger",
        "source_of_truth": sqlite_sot(ledger),
        "record_kind": "dynamicjepa_reality",
    }));
    let surprise_count = surprise_events.len() as u64;
    let surprise_rate = if panel_count == 0 {
        0.0
    } else {
        round_signal(surprise_count as f64 / panel_count as f64)
    };
    let plan_trace_coverage = if executed == 0 {
        1.0
    } else {
        round_signal(panel_count as f64 / executed as f64)
    };
    let projection_accuracy = round_signal(1.0 - surprise_rate);
    Ok(json!({
        "record_kind": "dynamicjepa_reality",
        "schema_version": 1,
        "run_id": run_id,
        "attempt": attempt,
        "domain_pack": DYNAMICJEPA_REALITY_DOMAIN_PACK,
        "source_mode": "harness_transition_projection",
        "panel_count": panel_count,
        "surprise_events": surprise_events,
        "surprise_event_count": surprise_count,
        "surprise_candidate_rate": surprise_rate,
        "projection_surprise_rate": surprise_rate,
        "trained_surprise_rate": Value::Null,
        "prediction_error_l2": Value::Null,
        "prediction_error_l2_status": {
            "status": "unavailable",
            "reason": "this attempt has DynamicJEPA harness-transition projections but no trained DynamicJEPA artifact prediction",
        },
        "projection_accuracy": projection_accuracy,
        "plan_trace_coverage": plan_trace_coverage,
        "guard_decisions": guard_decisions,
        "source_of_truth_refs": source_refs,
        "reward_features": {
            "dynamicjepa_projection_panel_count": panel_count,
            "dynamicjepa_projection_surprise_count": surprise_count,
            "dynamicjepa_projection_surprise_rate": surprise_rate,
            "dynamicjepa_projection_accuracy": projection_accuracy,
            "dynamicjepa_projection_plan_trace_coverage": plan_trace_coverage,
        },
        "recorded_at_unix": unix_secs()?,
    }))
}

#[allow(clippy::too_many_arguments)]
fn build_trained_dynamicjepa_reality_for_attempt(
    run_id: &str,
    attempt: u64,
    transcript_path: Option<&str>,
    ledger: &Path,
    attempt_dir: &Path,
    repo_dir: &Path,
    context_graph_bin: &Path,
    task_root: &Path,
    dynamicjepa_db: &Path,
    artifact_id: &str,
    skill_id: &str,
) -> Result<Value> {
    let Some(path) = transcript_path else {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_TRANSCRIPT_MISSING",
            "trained DynamicJEPA reward features require the physical agent tool transcript",
            "model_response.contextgraph_runtime.agent_tool_transcript",
            "rerun with the current governed inner harness so every attempt writes an agent_tool_transcript artifact",
            json!({"run_id": run_id, "attempt": attempt}),
            Some(sqlite_sot(ledger)),
        ));
    };
    let transcript_path = PathBuf::from(path.strip_prefix("file:").unwrap_or(path));
    let transcript: Value = read_json(&transcript_path)?;
    let events = dynamicjepa_swe_events_from_transcript(
        run_id,
        attempt,
        &transcript_path,
        &transcript,
        repo_dir,
    )?;
    if events.is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_NO_PANELS_FOR_TRAINED_ARTIFACT",
            "trained DynamicJEPA mode cannot score an attempt with zero tool transitions",
            "agent_tool_transcript.executed_tool_calls",
            "rerun an attempt that emits at least one governed tool transition before compiling trained DJ reward features",
            json!({"run_id": run_id, "attempt": attempt, "transcript": file_sot(&transcript_path)}),
            Some(file_sot(&transcript_path)),
        ));
    }

    let dj_dir = attempt_dir.join("dynamicjepa-trained");
    create_dir_checked(&dj_dir)?;
    let events_path = dj_dir.join("swe-reality-events.jsonl");
    let mut jsonl = String::new();
    for event in &events {
        jsonl.push_str(&serde_json::to_string(&event.event).map_err(|err| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_EVENT_SERIALIZE_FAILED",
                "DynamicJEPA SWE event could not be serialized",
                "dynamicjepa.event",
                "inspect transition-to-domain mapping code",
                json!({"error": err.to_string(), "event": event.event}),
                Some(file_sot(&transcript_path)),
            )
        })?);
        jsonl.push('\n');
    }
    write_text_checked(&events_path, &jsonl)?;

    let artifact = dynamicjepa_json_command(
        context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "get-artifact".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--id".to_string(),
            artifact_id.to_string(),
            "--verify-files".to_string(),
            "--json".to_string(),
        ],
        task_root,
        "dynamicjepa_get_artifact",
        Some(file_sot(dynamicjepa_db)),
    )?;
    write_json_checked(&dj_dir.join("artifact-readback.json"), &artifact)?;
    let artifact_domain = artifact
        .pointer("/decoded_records/artifact/domain_pack_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_ARTIFACT_DOMAIN_MISSING",
                "DynamicJEPA artifact readback did not contain domain_pack_id",
                "dynamicjepa.get_artifact.decoded_records.artifact.domain_pack_id",
                "inspect the model artifact row in dj_model_artifacts",
                json!({"artifact": artifact}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?;
    if artifact_domain != DYNAMICJEPA_REALITY_DOMAIN_PACK {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_ARTIFACT_DOMAIN_MISMATCH",
            "DynamicJEPA artifact domain does not match the SWE reality-loop domain pack",
            "dynamicjepa.get_artifact.decoded_records.artifact.domain_pack_id",
            "train or pass an artifact whose domain_pack_id is contextgraph_swe_reality_loop_v1",
            json!({"expected": DYNAMICJEPA_REALITY_DOMAIN_PACK, "actual": artifact_domain, "artifact_id": artifact_id}),
            Some(file_sot(dynamicjepa_db)),
        ));
    }
    let artifact_lifecycle_policy = validate_dynamicjepa_artifact_lifecycle(
        context_graph_bin,
        task_root,
        dynamicjepa_db,
        &artifact,
        artifact_id,
        &dj_dir,
    )?;

    let ingest = dynamicjepa_json_command(
        context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "ingest-event".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--domain".to_string(),
            DYNAMICJEPA_REALITY_DOMAIN_PACK.to_string(),
            "--adapter".to_string(),
            "json_event".to_string(),
            "--file".to_string(),
            path_str(&events_path).to_string(),
            "--json".to_string(),
        ],
        task_root,
        "dynamicjepa_ingest_event",
        Some(file_sot(dynamicjepa_db)),
    )?;
    write_json_checked(&dj_dir.join("ingest-readback.json"), &ingest)?;
    let transition_ids = required_json_string_array(
        &ingest,
        "/created_ids/transition_ids",
        "dynamicjepa.ingest.created_ids.transition_ids",
        dynamicjepa_db,
    )?;
    let action_ids = required_json_string_array(
        &ingest,
        "/created_ids/action_ids",
        "dynamicjepa.ingest.created_ids.action_ids",
        dynamicjepa_db,
    )?;
    let outcome_ids = required_json_string_array(
        &ingest,
        "/created_ids/outcome_ids",
        "dynamicjepa.ingest.created_ids.outcome_ids",
        dynamicjepa_db,
    )?;
    let failed_adapter_runs = ingest
        .pointer("/created_ids/failed_adapter_run_ids")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if failed_adapter_runs != 0 {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_INGEST_ADAPTER_FAILED",
            "DynamicJEPA SWE event ingestion produced failed adapter runs",
            "dynamicjepa.ingest.created_ids.failed_adapter_run_ids",
            "inspect the event JSONL and registered domain pack adapter mapping",
            json!({"ingest": ingest, "events": file_sot(&events_path)}),
            Some(file_sot(dynamicjepa_db)),
        ));
    }
    if transition_ids.len() != events.len()
        || action_ids.len() != events.len()
        || outcome_ids.len() != events.len()
    {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_INGEST_COUNT_MISMATCH",
            "DynamicJEPA ingest created-id counts did not match emitted SWE events",
            "dynamicjepa.ingest.created_ids",
            "inspect the ingest output before running predictions",
            json!({
                "events": events.len(),
                "transition_ids": transition_ids.len(),
                "action_ids": action_ids.len(),
                "outcome_ids": outcome_ids.len()
            }),
            Some(file_sot(dynamicjepa_db)),
        ));
    }

    let mut panel_ids = Vec::with_capacity(transition_ids.len());
    for (idx, transition_id) in transition_ids.iter().enumerate() {
        let materialized = dynamicjepa_json_command(
            context_graph_bin,
            vec![
                "dynamicjepa".to_string(),
                "materialize-panel".to_string(),
                "--db".to_string(),
                path_str(dynamicjepa_db).to_string(),
                "--transition-id".to_string(),
                transition_id.clone(),
                "--json".to_string(),
            ],
            task_root,
            "dynamicjepa_materialize_panel",
            Some(file_sot(dynamicjepa_db)),
        )?;
        write_json_checked(
            &dj_dir.join(format!("materialize-{idx:03}.json")),
            &materialized,
        )?;
        let created_panels = required_json_string_array(
            &materialized,
            "/created_ids/panel_ids",
            "dynamicjepa.materialize.created_ids.panel_ids",
            dynamicjepa_db,
        )?;
        if created_panels.len() != 1 {
            return Err(RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_PANEL_COUNT_MISMATCH",
                "materialize-panel by transition did not create exactly one panel",
                "dynamicjepa.materialize.created_ids.panel_ids",
                "inspect the transition id and existing dj_latent_panels rows",
                json!({"transition_id": transition_id, "panel_ids": created_panels}),
                Some(file_sot(dynamicjepa_db)),
            ));
        }
        panel_ids.push(created_panels[0].clone());
    }

    let mut predictions = Vec::with_capacity(panel_ids.len());
    let mut surprise_events = Vec::new();
    let mut error_sum = 0.0_f64;
    for idx in 0..panel_ids.len() {
        let predict = dynamicjepa_json_command(
            context_graph_bin,
            vec![
                "dynamicjepa".to_string(),
                "predict".to_string(),
                "--db".to_string(),
                path_str(dynamicjepa_db).to_string(),
                "--artifact-id".to_string(),
                artifact_id.to_string(),
                "--panel-id".to_string(),
                panel_ids[idx].clone(),
                "--action-id".to_string(),
                action_ids[idx].clone(),
                "--json".to_string(),
            ],
            task_root,
            "dynamicjepa_predict",
            Some(file_sot(dynamicjepa_db)),
        )?;
        write_json_checked(&dj_dir.join(format!("predict-{idx:03}.json")), &predict)?;
        let prediction_id = predict
            .pointer("/created_ids/prediction_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RealityError::new(
                    "RUST_REALITY_DYNAMICJEPA_PREDICTION_ID_MISSING",
                    "DynamicJEPA predict output was missing prediction_id",
                    "dynamicjepa.predict.created_ids.prediction_id",
                    "inspect dj_predictions readback before compiling reward",
                    json!({"predict": predict}),
                    Some(file_sot(dynamicjepa_db)),
                )
            })?
            .to_string();
        let surprise = dynamicjepa_json_command(
            context_graph_bin,
            vec![
                "dynamicjepa".to_string(),
                "record-surprise".to_string(),
                "--db".to_string(),
                path_str(dynamicjepa_db).to_string(),
                "--prediction-id".to_string(),
                prediction_id.clone(),
                "--observed-outcome-id".to_string(),
                outcome_ids[idx].clone(),
                "--observed-panel-id".to_string(),
                panel_ids[idx].clone(),
                "--json".to_string(),
            ],
            task_root,
            "dynamicjepa_record_surprise",
            Some(file_sot(dynamicjepa_db)),
        )?;
        write_json_checked(&dj_dir.join(format!("surprise-{idx:03}.json")), &surprise)?;
        let error_norm = surprise
            .pointer("/summary/error_norm")
            .and_then(Value::as_f64)
            .ok_or_else(|| {
                RealityError::new(
                    "RUST_REALITY_DYNAMICJEPA_SURPRISE_ERROR_MISSING",
                    "DynamicJEPA surprise output was missing numeric error_norm",
                    "dynamicjepa.record_surprise.summary.error_norm",
                    "inspect dj_predictions and dj_surprise_events readbacks",
                    json!({"surprise": surprise}),
                    Some(file_sot(dynamicjepa_db)),
                )
            })?;
        error_sum += error_norm;
        let wrote_surprise = surprise
            .pointer("/summary/wrote_surprise")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if wrote_surprise {
            let event_id = surprise
                .pointer("/created_ids/surprise_event_id")
                .and_then(Value::as_str)
                .unwrap_or("surprise_event_id_missing");
            surprise_events.push(json!({
                "event_id": event_id,
                "kind": "UnexpectedOutcome",
                "magnitude": round_signal(error_norm),
                "transition_id": events[idx].transition_id,
                "prediction_id": prediction_id,
                "panel_id": panel_ids[idx],
                "source_of_truth": format!("rocksdb:{}#dj_surprise_events", dynamicjepa_db.display()),
            }));
        }
        predictions.push(json!({
            "transition_id": events[idx].transition_id,
            "panel_id": panel_ids[idx],
            "action_id": action_ids[idx],
            "outcome_id": outcome_ids[idx],
            "prediction_id": prediction_id,
            "error_norm": round_signal(error_norm),
            "source_of_truth": format!("rocksdb:{}#dj_predictions", dynamicjepa_db.display()),
        }));
    }

    let mut guard_decisions = Vec::new();
    let mut plan_trace_ids = Vec::with_capacity(panel_ids.len());
    let mut action_controls = Vec::with_capacity(panel_ids.len());
    for (idx, panel_id) in panel_ids.iter().enumerate() {
        let observed_action = events[idx].event.get("action").ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_EVENT_ACTION_MISSING",
                "structured SWE event was missing action for DJ-guided action control",
                "dynamicjepa.event.action",
                "inspect transition-to-domain mapping before reward consumption",
                json!({"event": events[idx].event}),
                Some(file_sot(&events_path)),
            )
        })?;
        let candidate_action_path = dj_dir.join(format!("plan-{idx:03}-candidate-action.json"));
        write_json_checked(&candidate_action_path, observed_action)?;
        let plan = dynamicjepa_json_command(
            context_graph_bin,
            vec![
                "dynamicjepa".to_string(),
                "plan".to_string(),
                "--db".to_string(),
                path_str(dynamicjepa_db).to_string(),
                "--artifact-id".to_string(),
                artifact_id.to_string(),
                "--panel-id".to_string(),
                panel_id.clone(),
                "--skill-id".to_string(),
                skill_id.to_string(),
                "--candidate-action-json".to_string(),
                path_str(&candidate_action_path).to_string(),
                "--json".to_string(),
            ],
            task_root,
            "dynamicjepa_plan",
            Some(file_sot(dynamicjepa_db)),
        )?;
        write_json_checked(&dj_dir.join(format!("plan-{idx:03}.json")), &plan)?;
        if let Some(plan_id) = plan
            .pointer("/created_ids/plan_trace_id")
            .and_then(Value::as_str)
        {
            plan_trace_ids.push(plan_id.to_string());
        }
        if let Some(items) = plan
            .pointer("/decoded_records/guard_decisions")
            .and_then(Value::as_array)
        {
            guard_decisions.extend(items.iter().cloned());
        }
        action_controls.push(validate_dynamicjepa_action_control(
            &plan,
            observed_action,
            &events[idx].transition_id,
            dynamicjepa_db,
        )?);
    }

    let panel_count = panel_ids.len() as u64;
    let surprise_count = surprise_events.len() as u64;
    let surprise_rate = round_signal(surprise_count as f64 / panel_count as f64);
    let prediction_error_l2 = round_signal(error_sum / panel_count as f64);
    let prediction_accuracy = round_signal((1.0 - prediction_error_l2).clamp(0.0, 1.0));
    let plan_trace_coverage = round_signal(plan_trace_ids.len() as f64 / panel_count as f64);
    let attempt_segment_drift = validate_dynamicjepa_attempt_segment_drift(
        &events,
        &artifact_lifecycle_policy,
        dynamicjepa_db,
    )?;
    Ok(json!({
        "record_kind": "dynamicjepa_reality",
        "schema_version": 1,
        "run_id": run_id,
        "attempt": attempt,
        "domain_pack": DYNAMICJEPA_REALITY_DOMAIN_PACK,
        "source_mode": "trained_dynamicjepa_artifact",
        "panel_count": panel_count,
        "surprise_events": surprise_events,
        "surprise_event_count": surprise_count,
        "surprise_candidate_rate": surprise_rate,
        "projection_surprise_rate": Value::Null,
        "trained_surprise_rate": surprise_rate,
        "prediction_error_l2": prediction_error_l2,
        "prediction_error_l2_status": {
            "status": "available",
            "source": "dynamicjepa record-surprise summary.error_norm averaged over attempt panels",
        },
        "projection_accuracy": round_signal(1.0 - surprise_rate),
        "plan_trace_coverage": plan_trace_coverage,
        "guard_decisions": guard_decisions,
        "action_controls": action_controls,
        "attempt_segment_drift": attempt_segment_drift,
        "source_of_truth_refs": [
            {
                "kind": "agent_tool_transcript",
                "source_of_truth": file_sot(&transcript_path),
                "sha256": sha256_file(&transcript_path)?,
            },
            {
                "kind": "dynamicjepa_swe_events_jsonl",
                "source_of_truth": file_sot(&events_path),
                "sha256": sha256_file(&events_path)?,
            },
            {
                "kind": "dynamicjepa_rocksdb",
                "source_of_truth": format!("rocksdb:{}#dj_predictions,dj_plan_traces,dj_guard_decisions,dj_surprise_events", dynamicjepa_db.display()),
                "domain_pack": DYNAMICJEPA_REALITY_DOMAIN_PACK,
                "artifact_id": artifact_id,
            },
            {
                "kind": "blackbox_ledger",
                "source_of_truth": sqlite_sot(ledger),
                "record_kind": "dynamicjepa_reality",
            }
        ],
        "reward_features": {
            "dynamicjepa_trained_panel_count": panel_count,
            "dynamicjepa_trained_surprise_count": surprise_count,
            "dynamicjepa_trained_surprise_rate": surprise_rate,
            "dynamicjepa_trained_plan_trace_coverage": plan_trace_coverage,
            "dynamicjepa_prediction_error_l2": prediction_error_l2,
            "dynamicjepa_prediction_accuracy": prediction_accuracy,
        },
        "trained_artifact": {
            "artifact_id": artifact_id,
            "artifact_readback": file_sot(&dj_dir.join("artifact-readback.json")),
            "lifecycle_policy": artifact_lifecycle_policy,
            "predictions": predictions,
            "plan_trace_ids": plan_trace_ids,
            "source_of_truth": format!("rocksdb:{}#dj_model_artifacts", dynamicjepa_db.display()),
        },
        "recorded_at_unix": unix_secs()?,
    }))
}

fn validate_dynamicjepa_artifact_lifecycle(
    context_graph_bin: &Path,
    task_root: &Path,
    dynamicjepa_db: &Path,
    artifact: &Value,
    artifact_id: &str,
    dj_dir: &Path,
) -> Result<Value> {
    let artifact_record = artifact
        .pointer("/decoded_records/artifact")
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_ARTIFACT_RECORD_MISSING",
                "DynamicJEPA artifact readback did not include decoded_records.artifact",
                "dynamicjepa.get_artifact.decoded_records.artifact",
                "inspect dj_model_artifacts before using this artifact for reward",
                json!({"artifact_readback": artifact}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?;
    let status = artifact_record
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_ARTIFACT_STATUS_MISSING",
                "DynamicJEPA artifact readback did not include artifact status",
                "dynamicjepa.get_artifact.decoded_records.artifact.status",
                "inspect dj_model_artifacts before using this artifact for reward",
                json!({"artifact": artifact_record}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?;
    if status != "Active" {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_ARTIFACT_NOT_ACTIVE",
            "DynamicJEPA artifact lifecycle gate rejected a non-active artifact",
            "dynamicjepa.get_artifact.decoded_records.artifact.status",
            "promote only hash-verified Active artifacts",
            json!({"artifact_id": artifact_id, "status": status}),
            Some(file_sot(dynamicjepa_db)),
        ));
    }
    let artifact_hashes = artifact
        .get("artifact_hashes")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_ARTIFACT_HASHES_MISSING",
                "DynamicJEPA get-artifact --verify-files did not return artifact_hashes",
                "dynamicjepa.get_artifact.artifact_hashes",
                "rerun get-artifact with --verify-files and do not use unverified model bytes",
                json!({"artifact": artifact}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?;
    if artifact_hashes.is_empty()
        || artifact_hashes
            .iter()
            .any(|item| item.get("equal").and_then(Value::as_bool) != Some(true))
    {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_ARTIFACT_HASH_MISMATCH",
            "DynamicJEPA artifact lifecycle gate rejected missing or mismatched file hashes",
            "dynamicjepa.get_artifact.artifact_hashes",
            "rerun training from a clean artifact root; inference must not continue on hash mismatch",
            json!({"artifact_id": artifact_id, "artifact_hashes": artifact_hashes}),
            Some(file_sot(dynamicjepa_db)),
        ));
    }
    let artifact_root = artifact_record
        .get("artifact_root")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_ARTIFACT_ROOT_MISSING",
                "DynamicJEPA artifact readback did not include artifact_root",
                "dynamicjepa.get_artifact.decoded_records.artifact.artifact_root",
                "inspect dj_model_artifacts before using this artifact for reward",
                json!({"artifact": artifact_record}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?;
    let artifact_created_at = artifact_record
        .get("created_at_unix_ms")
        .and_then(Value::as_i64)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_ARTIFACT_CREATED_AT_MISSING",
                "DynamicJEPA artifact readback did not include created_at_unix_ms",
                "dynamicjepa.get_artifact.decoded_records.artifact.created_at_unix_ms",
                "inspect dj_model_artifacts before using this artifact for reward",
                json!({"artifact": artifact_record}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?;
    let now_ms = i64::try_from(unix_secs()?).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_UNIX_TIME_OVERFLOW",
            "current unix seconds did not fit i64 during artifact lifecycle gating",
            "system_time",
            "inspect system clock before using DynamicJEPA artifacts",
            json!({"error": err.to_string()}),
            None,
        )
    })? * 1000;
    if artifact_created_at > now_ms {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_ARTIFACT_FROM_FUTURE",
            "DynamicJEPA artifact lifecycle gate rejected an artifact timestamp newer than the system clock",
            "dynamicjepa.get_artifact.decoded_records.artifact.created_at_unix_ms",
            "fix system clock skew or retrain the artifact before reward consumption",
            json!({"artifact_id": artifact_id, "created_at_unix_ms": artifact_created_at, "now_unix_ms": now_ms}),
            Some(file_sot(dynamicjepa_db)),
        ));
    }
    let max_age_ms = 30_i64 * 24 * 60 * 60 * 1000;
    if now_ms.saturating_sub(artifact_created_at) > max_age_ms {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_ARTIFACT_STALE",
            "DynamicJEPA artifact lifecycle gate rejected a stale artifact",
            "dynamicjepa.get_artifact.decoded_records.artifact.created_at_unix_ms",
            "retrain or explicitly promote a fresh SWE-loop artifact before reward consumption",
            json!({"artifact_id": artifact_id, "created_at_unix_ms": artifact_created_at, "now_unix_ms": now_ms, "max_age_ms": max_age_ms}),
            Some(file_sot(dynamicjepa_db)),
        ));
    }
    let evaluation_report_path = artifact_root.join("evaluation_report.json");
    let evaluation_report: Value = read_json(&evaluation_report_path)?;
    let latent_collapsed = evaluation_report
        .pointer("/collapse_diagnostics/latent_collapsed")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_COLLAPSE_DIAGNOSTICS_MISSING",
                "DynamicJEPA artifact evaluation_report is missing collapse diagnostics",
                "artifact.evaluation_report.collapse_diagnostics.latent_collapsed",
                "rerun training; production artifacts require non-collapse diagnostics",
                json!({"artifact_id": artifact_id, "evaluation_report": file_sot(&evaluation_report_path)}),
                Some(file_sot(&evaluation_report_path)),
            )
        })?;
    if latent_collapsed {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_ARTIFACT_COLLAPSED",
            "DynamicJEPA artifact lifecycle gate rejected a collapsed latent representation",
            "artifact.evaluation_report.collapse_diagnostics.latent_collapsed",
            "retrain with non-collapse diagnostics passing before reward consumption",
            json!({"artifact_id": artifact_id, "evaluation_report": file_sot(&evaluation_report_path)}),
            Some(file_sot(&evaluation_report_path)),
        ));
    }
    let val_latent_mse = evaluation_report
        .pointer("/split_metrics/val/latent_mse")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_VAL_METRIC_MISSING",
                "DynamicJEPA artifact evaluation_report is missing finite val latent_mse",
                "artifact.evaluation_report.split_metrics.val.latent_mse",
                "rerun training; production artifacts require finite held-out metrics",
                json!({"artifact_id": artifact_id, "evaluation_report": file_sot(&evaluation_report_path)}),
                Some(file_sot(&evaluation_report_path)),
            )
        })?;
    let target_architecture = evaluation_report
        .get("target_architecture")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_TARGET_ARCHITECTURE_MISSING",
                "DynamicJEPA artifact evaluation_report is missing target_architecture",
                "artifact.evaluation_report.target_architecture",
                "retrain with an explicit ME-JEPA target architecture",
                json!({"artifact_id": artifact_id, "evaluation_report": file_sot(&evaluation_report_path)}),
                Some(file_sot(&evaluation_report_path)),
            )
        })?;
    if target_architecture != "frozen_instrument_projection" {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_TARGET_ARCHITECTURE_NOT_ME",
            "DynamicJEPA artifact lifecycle gate rejected an EMA-target artifact for ME-JEPA production use",
            "artifact.evaluation_report.target_architecture",
            "retrain with model.target_architecture=frozen_instrument_projection and ema_momentum=0.0",
            json!({"artifact_id": artifact_id, "target_architecture": target_architecture}),
            Some(file_sot(&evaluation_report_path)),
        ));
    }
    let surprise_calibration = evaluation_report
        .get("surprise_calibration")
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_SURPRISE_CALIBRATION_MISSING",
                "DynamicJEPA artifact evaluation_report is missing held-out surprise calibration",
                "artifact.evaluation_report.surprise_calibration",
                "retrain with a real val split so model-error surprise has a calibrated threshold",
                json!({"artifact_id": artifact_id, "evaluation_report": file_sot(&evaluation_report_path)}),
                Some(file_sot(&evaluation_report_path)),
            )
        })?;
    if surprise_calibration.get("status").and_then(Value::as_str) != Some("calibrated") {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_SURPRISE_UNCALIBRATED",
            "DynamicJEPA artifact lifecycle gate rejected an artifact without calibrated model-error surprise",
            "artifact.evaluation_report.surprise_calibration.status",
            "compile held-out validation rows and retrain before reward or planning use",
            json!({"artifact_id": artifact_id, "surprise_calibration": surprise_calibration}),
            Some(file_sot(&evaluation_report_path)),
        ));
    }
    let surprise_threshold = surprise_calibration
        .get("threshold_cosine")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_SURPRISE_THRESHOLD_INVALID",
                "DynamicJEPA artifact surprise calibration is missing finite threshold_cosine",
                "artifact.evaluation_report.surprise_calibration.threshold_cosine",
                "inspect evaluation_report.json and retrain from held-out outcomes",
                json!({"artifact_id": artifact_id, "surprise_calibration": surprise_calibration}),
                Some(file_sot(&evaluation_report_path)),
            )
        })?;
    let calibration_set_count = surprise_calibration
        .get("calibration_set_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_SURPRISE_CALIBRATION_COUNT_MISSING",
                "DynamicJEPA artifact surprise calibration is missing calibration_set_count",
                "artifact.evaluation_report.surprise_calibration.calibration_set_count",
                "retrain with persisted held-out validation rows",
                json!({"artifact_id": artifact_id, "surprise_calibration": surprise_calibration}),
                Some(file_sot(&evaluation_report_path)),
            )
        })?;
    if calibration_set_count < 5 {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_SURPRISE_CALIBRATION_TOO_SMALL",
            "DynamicJEPA artifact lifecycle gate rejected a too-small held-out surprise calibration set",
            "artifact.evaluation_report.surprise_calibration.calibration_set_count",
            "compile a larger validation split before production use",
            json!({"artifact_id": artifact_id, "calibration_set_count": calibration_set_count}),
            Some(file_sot(&evaluation_report_path)),
        ));
    }
    let segmented_surprise_calibration = validate_dynamicjepa_segmented_surprise_calibration(
        &evaluation_report,
        &evaluation_report_path,
        artifact_id,
    )?;
    let training_run_id = artifact_record
        .get("training_run_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_TRAINING_RUN_ID_MISSING",
                "DynamicJEPA artifact readback did not include training_run_id",
                "dynamicjepa.get_artifact.decoded_records.artifact.training_run_id",
                "inspect dj_model_artifacts before using this artifact for reward",
                json!({"artifact": artifact_record}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?;
    let training = dynamicjepa_json_command(
        context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "get-training-run".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--id".to_string(),
            training_run_id.to_string(),
            "--json".to_string(),
        ],
        task_root,
        "dynamicjepa_get_training_run",
        Some(file_sot(dynamicjepa_db)),
    )?;
    write_json_checked(&dj_dir.join("training-run-readback.json"), &training)?;
    if training
        .pointer("/decoded_records/training_run/status")
        .and_then(Value::as_str)
        != Some("Completed")
    {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_TRAINING_NOT_COMPLETED",
            "DynamicJEPA artifact lifecycle gate rejected an artifact whose training run is not completed",
            "dynamicjepa.get_training_run.decoded_records.training_run.status",
            "use only artifacts backed by completed training runs",
            json!({"artifact_id": artifact_id, "training": training}),
            Some(file_sot(dynamicjepa_db)),
        ));
    }
    let counts = dynamicjepa_json_command(
        context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "inspect-counts".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--json".to_string(),
        ],
        task_root,
        "dynamicjepa_inspect_counts",
        Some(file_sot(dynamicjepa_db)),
    )?;
    write_json_checked(&dj_dir.join("artifact-lifecycle-counts.json"), &counts)?;
    let threshold_calibrations =
        required_dynamicjepa_count(&counts, "dj_threshold_calibrations", dynamicjepa_db)?;
    if threshold_calibrations == 0 {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_UNCALIBRATED_ARTIFACT",
            "DynamicJEPA artifact lifecycle gate rejected an artifact without persisted threshold calibration rows",
            "rocksdb.dj_threshold_calibrations",
            "build constellations and calibrate thresholds on held-out events before using trained DJ reward features",
            json!({"artifact_id": artifact_id, "counts": counts.get("counts")}),
            Some(format!("rocksdb:{}#dj_threshold_calibrations", dynamicjepa_db.display())),
        ));
    }
    let constellations = required_dynamicjepa_count(&counts, "dj_constellations", dynamicjepa_db)?;
    if constellations == 0 {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_CONSTELLATION_MISSING",
            "DynamicJEPA artifact lifecycle gate rejected an artifact without persisted guard constellations",
            "rocksdb.dj_constellations",
            "build guard constellations before calibrating thresholds and using trained DJ reward features",
            json!({"artifact_id": artifact_id, "counts": counts.get("counts")}),
            Some(format!("rocksdb:{}#dj_constellations", dynamicjepa_db.display())),
        ));
    }
    let dataset_shards = required_dynamicjepa_count(&counts, "dj_dataset_shards", dynamicjepa_db)?;
    let raw_events = required_dynamicjepa_count(&counts, "dj_raw_events", dynamicjepa_db)?;
    if dataset_shards < 3 || raw_events < 60 {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_DATASET_DIVERSITY_TOO_LOW",
            "DynamicJEPA artifact lifecycle gate rejected an artifact with insufficient source dataset diversity",
            "rocksdb.dj_dataset_shards,dj_raw_events",
            "compile train/val/test shards from at least the SWE-loop verification corpus before production reward use",
            json!({"artifact_id": artifact_id, "dataset_shards": dataset_shards, "raw_events": raw_events}),
            Some(file_sot(dynamicjepa_db)),
        ));
    }
    let corpus_diversity = validate_dynamicjepa_corpus_diversity(
        context_graph_bin,
        task_root,
        dynamicjepa_db,
        dj_dir,
        raw_events,
    )?;
    let comparative_promotion = validate_dynamicjepa_comparative_promotion(
        context_graph_bin,
        task_root,
        dynamicjepa_db,
        artifact_id,
        &artifact_root,
        val_latent_mse,
        dj_dir,
    )?;
    Ok(json!({
        "status": "passed",
        "artifact_id": artifact_id,
        "checks": {
            "artifact_status": status,
            "file_hashes_verified": true,
            "artifact_age_ms": now_ms.saturating_sub(artifact_created_at),
            "latent_collapsed": latent_collapsed,
            "val_latent_mse": round_signal(val_latent_mse),
            "target_architecture": target_architecture,
            "surprise_threshold_cosine": round_signal(surprise_threshold),
            "surprise_calibration_set_count": calibration_set_count,
            "segmented_surprise_calibration": segmented_surprise_calibration,
            "training_run_status": "Completed",
            "dataset_shards": dataset_shards,
            "raw_events": raw_events,
            "corpus_diversity": corpus_diversity,
            "comparative_promotion": comparative_promotion,
            "constellations": constellations,
            "threshold_calibrations": threshold_calibrations
        },
        "source_of_truth": {
            "model_artifact": format!("rocksdb:{}#dj_model_artifacts", dynamicjepa_db.display()),
            "training_run": format!("rocksdb:{}#dj_training_runs", dynamicjepa_db.display()),
            "threshold_calibrations": format!("rocksdb:{}#dj_threshold_calibrations", dynamicjepa_db.display()),
            "evaluation_report": file_sot(&evaluation_report_path),
            "counts_readback": file_sot(&dj_dir.join("artifact-lifecycle-counts.json"))
        }
    }))
}

fn validate_dynamicjepa_segmented_surprise_calibration(
    evaluation_report: &Value,
    evaluation_report_path: &Path,
    artifact_id: &str,
) -> Result<Value> {
    let segments = evaluation_report
        .get("surprise_segment_calibrations")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_SEGMENT_CALIBRATION_MISSING",
                "DynamicJEPA artifact evaluation_report is missing segmented surprise calibration",
                "artifact.evaluation_report.surprise_segment_calibrations",
                "retrain with action-segment metadata so surprise thresholds are calibrated per tool/risk segment",
                json!({"artifact_id": artifact_id, "evaluation_report": file_sot(evaluation_report_path)}),
                Some(file_sot(evaluation_report_path)),
            )
        })?;
    let calibrated = segments
        .iter()
        .filter(|(_, report)| report.get("status").and_then(Value::as_str) == Some("calibrated"))
        .map(|(key, _)| key.clone())
        .collect::<BTreeSet<_>>();
    let required_segments = [
        "action.tool_family=repository_navigation",
        "action.tool_family=workspace_mutation",
        "action.path_risk=source",
        "action.path_risk=none",
        "action.ast_language=python",
        "action.ast_node_kind=function",
        "action.ast_edit_kind=function_body",
        "action.semantic_status=analyzed",
        "action.patch_delta_kind=return_contract",
        "action.dependency_blast_radius=local_symbol",
        "action.compiler_semantic_status=checked",
        "action.type_resolution_status=resolved",
        "action.import_resolution_status=resolved",
        "action.test_attribution_status=inferred_from_manifest",
        "action.predicted_verifier_target=typecheck",
    ];
    let missing = required_segments
        .iter()
        .filter(|segment| !calibrated.contains(&segment.to_string()))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_SEGMENT_CALIBRATION_INCOMPLETE",
            "DynamicJEPA artifact lifecycle gate rejected missing calibrated tool/risk surprise segments",
            "artifact.evaluation_report.surprise_segment_calibrations",
            "compile a broader real SWE validation split and retrain before production reward or pre-action use",
            json!({
                "artifact_id": artifact_id,
                "missing_segments": missing,
                "calibrated_segments": calibrated,
                "evaluation_report": file_sot(evaluation_report_path),
            }),
            Some(file_sot(evaluation_report_path)),
        ));
    }
    let calibrated_segment_count = calibrated.len();
    Ok(json!({
        "status": "passed",
        "required_segments": required_segments,
        "calibrated_segments": calibrated,
        "calibrated_segment_count": calibrated_segment_count,
        "source_of_truth": file_sot(evaluation_report_path),
    }))
}

fn validate_dynamicjepa_attempt_segment_drift(
    events: &[SweDynamicJepaEvent],
    artifact_lifecycle_policy: &Value,
    dynamicjepa_db: &Path,
) -> Result<Value> {
    let calibrated_segments = artifact_lifecycle_policy
        .pointer("/checks/segmented_surprise_calibration/calibrated_segments")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_DRIFT_CALIBRATION_SCHEMA_INVALID",
                "DynamicJEPA lifecycle policy did not expose calibrated surprise segments for drift detection",
                "dynamicjepa.trained_artifact.lifecycle_policy.checks.segmented_surprise_calibration.calibrated_segments",
                "inspect lifecycle policy generation before consuming trained DJ reward features",
                json!({"artifact_lifecycle_policy": artifact_lifecycle_policy}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?
        .iter()
        .map(|value| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                RealityError::new(
                    "RUST_REALITY_DYNAMICJEPA_DRIFT_CALIBRATION_SEGMENT_INVALID",
                    "calibrated_segments contained a non-string segment key",
                    "dynamicjepa.segment_drift.calibrated_segments[]",
                    "inspect evaluation_report segmented calibration serialization",
                    json!({"segment": value}),
                    Some(file_sot(dynamicjepa_db)),
                )
            })
        })
        .collect::<Result<BTreeSet<_>>>()?;
    let mut observed_counts: BTreeMap<String, u64> = BTreeMap::new();
    for event in events {
        let action = event.event.get("action").ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_DRIFT_EVENT_ACTION_MISSING",
                "SWE event was missing action during segment drift detection",
                "dynamicjepa.event.action",
                "inspect transition-to-domain mapping before reward consumption",
                json!({"transition_id": event.transition_id, "event": event.event}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?;
        for field in [
            "tool_family",
            "path_risk",
            "patch_delta_kind",
            "dependency_blast_radius",
            "semantic_status",
            "compiler_semantic_status",
            "type_resolution_status",
            "import_resolution_status",
            "test_attribution_status",
            "predicted_verifier_target",
        ] {
            let value = action.get(field).and_then(Value::as_str).ok_or_else(|| {
                RealityError::new(
                    "RUST_REALITY_DYNAMICJEPA_DRIFT_EVENT_SEGMENT_MISSING",
                    "SWE event action was missing a segment field needed for drift detection",
                    format!("dynamicjepa.event.action.{field}"),
                    "regenerate argument-aware SWE events before reward consumption",
                    json!({"transition_id": event.transition_id, "event": event.event}),
                    Some(file_sot(dynamicjepa_db)),
                )
            })?;
            *observed_counts
                .entry(format!("action.{field}={value}"))
                .or_insert(0) += 1;
        }
    }
    let missing_segments = observed_counts
        .keys()
        .filter(|key| !calibrated_segments.contains(*key))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_segments.is_empty() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_SEGMENT_DRIFT_UNCALIBRATED",
            "attempt contains action segments that were not calibrated in the artifact validation split",
            "dynamicjepa.attempt_segment_drift.observed_segments",
            "retrain with validation coverage for the observed action segments before scoring this attempt",
            json!({
                "missing_segments": missing_segments,
                "observed_counts": observed_counts,
                "calibrated_segments": calibrated_segments,
            }),
            Some(file_sot(dynamicjepa_db)),
        ));
    }
    Ok(json!({
        "status": "passed",
        "mode": "calibrated_segment_coverage",
        "observed_counts": observed_counts,
        "source_of_truth": format!("rocksdb:{}#dj_actions,dj_model_artifacts", dynamicjepa_db.display()),
    }))
}

fn required_dynamicjepa_count(counts: &Value, cf_name: &str, dynamicjepa_db: &Path) -> Result<u64> {
    counts
        .pointer(&format!("/counts/{cf_name}"))
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_COUNTS_SCHEMA_INVALID",
                "DynamicJEPA inspect-counts output was missing a required column-family count",
                format!("dynamicjepa.inspect_counts.counts.{cf_name}"),
                "inspect the DynamicJEPA CLI output schema before lifecycle gating",
                json!({"cf_name": cf_name, "counts": counts}),
                Some(file_sot(dynamicjepa_db)),
            )
        })
}

fn validate_dynamicjepa_corpus_diversity(
    context_graph_bin: &Path,
    task_root: &Path,
    dynamicjepa_db: &Path,
    dj_dir: &Path,
    raw_events: u64,
) -> Result<Value> {
    let limit = raw_events.clamp(1, 10_000).to_string();
    let actions = dynamicjepa_json_command(
        context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "inspect-cf".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--cf".to_string(),
            "dj_actions".to_string(),
            "--limit".to_string(),
            limit.clone(),
            "--json".to_string(),
        ],
        task_root,
        "dynamicjepa_inspect_actions_for_diversity",
        Some(file_sot(dynamicjepa_db)),
    )?;
    let outcomes = dynamicjepa_json_command(
        context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "inspect-cf".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--cf".to_string(),
            "dj_outcomes".to_string(),
            "--limit".to_string(),
            limit,
            "--json".to_string(),
        ],
        task_root,
        "dynamicjepa_inspect_outcomes_for_diversity",
        Some(file_sot(dynamicjepa_db)),
    )?;
    let evidence_path = dj_dir.join("artifact-corpus-diversity.json");
    let action_tool_families = unique_dynamicjepa_field_values(&actions, "tool_family");
    let command_kinds = unique_dynamicjepa_field_values(&actions, "command_kind");
    let path_buckets = unique_dynamicjepa_field_values(&actions, "target_path_bucket");
    let dependency_layers = unique_dynamicjepa_field_values(&actions, "dependency_layer");
    let patch_intents = unique_dynamicjepa_field_values(&actions, "patch_intent_bucket");
    let patch_shapes = unique_dynamicjepa_field_values(&actions, "patch_shape_bucket");
    let patch_scopes = unique_dynamicjepa_field_values(&actions, "patch_scope_bucket");
    let ast_languages = unique_dynamicjepa_field_values(&actions, "ast_language");
    let ast_parse_statuses = unique_dynamicjepa_field_values(&actions, "ast_parse_status");
    let ast_node_kinds = unique_dynamicjepa_field_values(&actions, "ast_node_kind");
    let ast_symbol_kinds = unique_dynamicjepa_field_values(&actions, "ast_symbol_kind");
    let ast_edit_kinds = unique_dynamicjepa_field_values(&actions, "ast_edit_kind");
    let semantic_providers = unique_dynamicjepa_field_values(&actions, "semantic_provider");
    let semantic_statuses = unique_dynamicjepa_field_values(&actions, "semantic_status");
    let symbol_visibilities = unique_dynamicjepa_field_values(&actions, "symbol_visibility");
    let patch_delta_kinds = unique_dynamicjepa_field_values(&actions, "patch_delta_kind");
    let control_flow_deltas = unique_dynamicjepa_field_values(&actions, "control_flow_delta");
    let api_contract_deltas = unique_dynamicjepa_field_values(&actions, "api_contract_delta");
    let dependency_blast_radii =
        unique_dynamicjepa_field_values(&actions, "dependency_blast_radius");
    let compiler_semantic_statuses =
        unique_dynamicjepa_field_values(&actions, "compiler_semantic_status");
    let type_resolution_statuses =
        unique_dynamicjepa_field_values(&actions, "type_resolution_status");
    let import_resolution_statuses =
        unique_dynamicjepa_field_values(&actions, "import_resolution_status");
    let test_attribution_statuses =
        unique_dynamicjepa_field_values(&actions, "test_attribution_status");
    let predicted_verifier_targets =
        unique_dynamicjepa_field_values(&actions, "predicted_verifier_target");
    let result_statuses = unique_dynamicjepa_field_values(&outcomes, "result_status");
    let verifier_statuses = unique_dynamicjepa_field_values(&outcomes, "verifier_status");
    let error_signatures = unique_dynamicjepa_field_values(&outcomes, "error_signature");
    let summary = json!({
        "status": "passed",
        "source_of_truth": {
            "actions": format!("rocksdb:{}#dj_actions", dynamicjepa_db.display()),
            "outcomes": format!("rocksdb:{}#dj_outcomes", dynamicjepa_db.display()),
            "evidence": file_sot(&evidence_path),
        },
        "unique_counts": {
            "action_tool_family": action_tool_families.len(),
            "command_kind": command_kinds.len(),
            "target_path_bucket": path_buckets.len(),
            "dependency_layer": dependency_layers.len(),
            "patch_intent_bucket": patch_intents.len(),
            "patch_shape_bucket": patch_shapes.len(),
            "patch_scope_bucket": patch_scopes.len(),
            "ast_language": ast_languages.len(),
            "ast_parse_status": ast_parse_statuses.len(),
            "ast_node_kind": ast_node_kinds.len(),
            "ast_symbol_kind": ast_symbol_kinds.len(),
            "ast_edit_kind": ast_edit_kinds.len(),
            "semantic_provider": semantic_providers.len(),
            "semantic_status": semantic_statuses.len(),
            "symbol_visibility": symbol_visibilities.len(),
            "patch_delta_kind": patch_delta_kinds.len(),
            "control_flow_delta": control_flow_deltas.len(),
            "api_contract_delta": api_contract_deltas.len(),
            "dependency_blast_radius": dependency_blast_radii.len(),
            "compiler_semantic_status": compiler_semantic_statuses.len(),
            "type_resolution_status": type_resolution_statuses.len(),
            "import_resolution_status": import_resolution_statuses.len(),
            "test_attribution_status": test_attribution_statuses.len(),
            "predicted_verifier_target": predicted_verifier_targets.len(),
            "result_status": result_statuses.len(),
            "verifier_status": verifier_statuses.len(),
            "error_signature": error_signatures.len(),
        },
        "unique_values": {
            "action_tool_family": action_tool_families,
            "command_kind": command_kinds,
            "target_path_bucket": path_buckets,
            "dependency_layer": dependency_layers,
            "patch_intent_bucket": patch_intents,
            "patch_shape_bucket": patch_shapes,
            "patch_scope_bucket": patch_scopes,
            "ast_language": ast_languages,
            "ast_parse_status": ast_parse_statuses,
            "ast_node_kind": ast_node_kinds,
            "ast_symbol_kind": ast_symbol_kinds,
            "ast_edit_kind": ast_edit_kinds,
            "semantic_provider": semantic_providers,
            "semantic_status": semantic_statuses,
            "symbol_visibility": symbol_visibilities,
            "patch_delta_kind": patch_delta_kinds,
            "control_flow_delta": control_flow_deltas,
            "api_contract_delta": api_contract_deltas,
            "dependency_blast_radius": dependency_blast_radii,
            "compiler_semantic_status": compiler_semantic_statuses,
            "type_resolution_status": type_resolution_statuses,
            "import_resolution_status": import_resolution_statuses,
            "test_attribution_status": test_attribution_statuses,
            "predicted_verifier_target": predicted_verifier_targets,
            "result_status": result_statuses,
            "verifier_status": verifier_statuses,
            "error_signature": error_signatures,
        },
    });
    write_json_checked(&evidence_path, &summary)?;
    for (field, min, actual) in [
        (
            "action_tool_family",
            3usize,
            summary
                .pointer("/unique_counts/action_tool_family")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "command_kind",
            5usize,
            summary
                .pointer("/unique_counts/command_kind")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "target_path_bucket",
            2usize,
            summary
                .pointer("/unique_counts/target_path_bucket")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "dependency_layer",
            2usize,
            summary
                .pointer("/unique_counts/dependency_layer")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "result_status",
            2usize,
            summary
                .pointer("/unique_counts/result_status")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "patch_intent_bucket",
            3usize,
            summary
                .pointer("/unique_counts/patch_intent_bucket")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "patch_shape_bucket",
            3usize,
            summary
                .pointer("/unique_counts/patch_shape_bucket")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "patch_scope_bucket",
            2usize,
            summary
                .pointer("/unique_counts/patch_scope_bucket")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "ast_language",
            2usize,
            summary
                .pointer("/unique_counts/ast_language")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "ast_node_kind",
            3usize,
            summary
                .pointer("/unique_counts/ast_node_kind")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "ast_edit_kind",
            3usize,
            summary
                .pointer("/unique_counts/ast_edit_kind")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "semantic_status",
            2usize,
            summary
                .pointer("/unique_counts/semantic_status")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "patch_delta_kind",
            3usize,
            summary
                .pointer("/unique_counts/patch_delta_kind")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "dependency_blast_radius",
            2usize,
            summary
                .pointer("/unique_counts/dependency_blast_radius")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "compiler_semantic_status",
            2usize,
            summary
                .pointer("/unique_counts/compiler_semantic_status")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "type_resolution_status",
            2usize,
            summary
                .pointer("/unique_counts/type_resolution_status")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "test_attribution_status",
            2usize,
            summary
                .pointer("/unique_counts/test_attribution_status")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
        (
            "verifier_status",
            2usize,
            summary
                .pointer("/unique_counts/verifier_status")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
        ),
    ] {
        if actual < min {
            return Err(RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_CORPUS_DIVERSITY_TOO_LOW",
                "DynamicJEPA artifact lifecycle gate rejected insufficient real SWE-loop corpus diversity",
                format!("dynamicjepa.corpus_diversity.{field}"),
                "ingest a broader real attempt corpus before promoting this artifact",
                json!({"field": field, "minimum_unique": min, "actual_unique": actual, "evidence": file_sot(&evidence_path)}),
                Some(file_sot(&evidence_path)),
            ));
        }
    }
    Ok(read_json(&evidence_path)?)
}

fn unique_dynamicjepa_field_values(command_output: &Value, field: &str) -> Vec<String> {
    let mut values = BTreeSet::new();
    if let Some(rows) = command_output
        .pointer("/decoded_records/rows")
        .and_then(Value::as_array)
    {
        for row in rows {
            let Some(fields) = row
                .pointer("/decoded/fields")
                .or_else(|| row.pointer("/decoded/record/fields"))
            else {
                continue;
            };
            if let Some(value) = fields.get(field).and_then(field_value_to_plain_json) {
                if let Some(text) = value.as_str() {
                    values.insert(text.to_string());
                }
            }
        }
    }
    values.into_iter().collect()
}

fn dynamicjepa_downstream_utility_from_rows(
    context_graph_bin: &Path,
    task_root: &Path,
    dynamicjepa_db: &Path,
    artifact_id: &str,
    dj_dir: &Path,
    label: &str,
) -> Result<Value> {
    let predictions = dynamicjepa_inspect_cf_json(
        context_graph_bin,
        task_root,
        dynamicjepa_db,
        "dj_predictions",
        "dynamicjepa_inspect_predictions_for_downstream_utility",
    )?;
    let plans = dynamicjepa_inspect_cf_json(
        context_graph_bin,
        task_root,
        dynamicjepa_db,
        "dj_plan_traces",
        "dynamicjepa_inspect_plan_traces_for_downstream_utility",
    )?;
    let guards = dynamicjepa_inspect_cf_json(
        context_graph_bin,
        task_root,
        dynamicjepa_db,
        "dj_guard_decisions",
        "dynamicjepa_inspect_guard_decisions_for_downstream_utility",
    )?;
    let surprises = dynamicjepa_inspect_cf_json(
        context_graph_bin,
        task_root,
        dynamicjepa_db,
        "dj_surprise_events",
        "dynamicjepa_inspect_surprises_for_downstream_utility",
    )?;

    let mut prediction_ids = BTreeSet::new();
    let mut prediction_count = 0usize;
    for record in decoded_records_from_inspect(&predictions) {
        if record.get("model_artifact_id").and_then(Value::as_str) == Some(artifact_id) {
            prediction_count += 1;
            if let Some(id) = record.get("prediction_id").and_then(Value::as_str) {
                prediction_ids.insert(id.to_string());
            }
        }
    }

    let mut plan_ids = BTreeSet::new();
    let mut plan_count = 0usize;
    let mut selected_count = 0usize;
    for record in decoded_records_from_inspect(&plans) {
        if record.get("model_artifact_id").and_then(Value::as_str) == Some(artifact_id) {
            plan_count += 1;
            if record.get("status").and_then(Value::as_str) == Some("Selected") {
                selected_count += 1;
            }
            if let Some(id) = record.get("plan_trace_id").and_then(Value::as_str) {
                plan_ids.insert(id.to_string());
            }
        }
    }

    let mut guard_count = 0usize;
    let mut guard_reject_count = 0usize;
    for record in decoded_records_from_inspect(&guards) {
        let Some(plan_trace_id) = record.get("plan_trace_id").and_then(Value::as_str) else {
            continue;
        };
        if !plan_ids.contains(plan_trace_id) {
            continue;
        }
        guard_count += 1;
        let decision = record.get("decision");
        if decision
            .is_some_and(|value| value.as_str() == Some("Reject") || value.get("Reject").is_some())
        {
            guard_reject_count += 1;
        }
    }

    let mut surprise_count = 0usize;
    for record in decoded_records_from_inspect(&surprises) {
        if record
            .get("prediction_id")
            .and_then(Value::as_str)
            .is_some_and(|id| prediction_ids.contains(id))
        {
            surprise_count += 1;
        }
    }

    if prediction_count == 0 || plan_count == 0 {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_PROMOTION_DOWNSTREAM_EVIDENCE_MISSING",
            "DynamicJEPA artifact promotion requires persisted downstream prediction and plan evidence for the candidate artifact",
            "rocksdb.dj_predictions,dj_plan_traces",
            "run held-out replay or live-shadow planning with this artifact before promotion; do not promote from model metrics alone",
            json!({
                "artifact_id": artifact_id,
                "prediction_count": prediction_count,
                "plan_count": plan_count,
                "source_of_truth": format!("rocksdb:{}#dj_predictions,dj_plan_traces", dynamicjepa_db.display())
            }),
            Some(file_sot(dynamicjepa_db)),
        ));
    }

    let selected_rate = selected_count as f64 / plan_count as f64;
    let guard_reject_rate = if guard_count == 0 {
        0.0
    } else {
        guard_reject_count as f64 / guard_count as f64
    };
    let surprise_rate = surprise_count as f64 / prediction_count as f64;
    let score = round_signal(selected_rate - guard_reject_rate - surprise_rate);
    let evidence_path = dj_dir.join(format!(
        "artifact-downstream-utility-{}.json",
        safe_id(label)
    ));
    let evidence = json!({
        "status": "passed",
        "artifact_id": artifact_id,
        "downstream_utility_score": score,
        "components": {
            "selected_plan_rate": round_signal(selected_rate),
            "guard_reject_rate": round_signal(guard_reject_rate),
            "surprise_rate": round_signal(surprise_rate),
            "prediction_count": prediction_count,
            "plan_count": plan_count,
            "guard_decision_count": guard_count,
            "guard_reject_count": guard_reject_count,
            "surprise_count": surprise_count,
        },
        "source_of_truth": {
            "predictions": format!("rocksdb:{}#dj_predictions", dynamicjepa_db.display()),
            "plan_traces": format!("rocksdb:{}#dj_plan_traces", dynamicjepa_db.display()),
            "guard_decisions": format!("rocksdb:{}#dj_guard_decisions", dynamicjepa_db.display()),
            "surprise_events": format!("rocksdb:{}#dj_surprise_events", dynamicjepa_db.display()),
            "evidence": file_sot(&evidence_path),
        }
    });
    write_json_checked(&evidence_path, &evidence)?;
    read_json(&evidence_path)
}

fn dynamicjepa_inspect_cf_json(
    context_graph_bin: &Path,
    task_root: &Path,
    dynamicjepa_db: &Path,
    cf: &str,
    operation: &'static str,
) -> Result<Value> {
    dynamicjepa_json_command(
        context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "inspect-cf".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--cf".to_string(),
            cf.to_string(),
            "--limit".to_string(),
            "10000".to_string(),
            "--json".to_string(),
        ],
        task_root,
        operation,
        Some(file_sot(dynamicjepa_db)),
    )
}

fn decoded_records_from_inspect(output: &Value) -> Vec<&Value> {
    output
        .pointer("/decoded_records/rows")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    row.pointer("/decoded/record")
                        .or_else(|| row.get("decoded"))
                        .or_else(|| row.pointer("/decoded_records/record"))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn validate_dynamicjepa_comparative_promotion(
    context_graph_bin: &Path,
    task_root: &Path,
    dynamicjepa_db: &Path,
    artifact_id: &str,
    artifact_root: &Path,
    val_latent_mse: f64,
    dj_dir: &Path,
) -> Result<Value> {
    let candidate_report_path = artifact_root.join("evaluation_report.json");
    let candidate_report: Value = read_json(&candidate_report_path)?;
    let candidate_heldout_proxy = dynamicjepa_heldout_utility_score(&candidate_report).ok_or_else(|| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_PROMOTION_UTILITY_MISSING",
            "DynamicJEPA candidate artifact evaluation_report is missing held-out utility metrics",
            "artifact.evaluation_report.split_metrics.val",
            "retrain with finite val latent_mse, latent_cosine, and action_contrast_acc before promotion",
            json!({"artifact_id": artifact_id, "evaluation_report": file_sot(&candidate_report_path)}),
            Some(file_sot(&candidate_report_path)),
        )
    })?;
    let candidate_downstream = dynamicjepa_downstream_utility_from_rows(
        context_graph_bin,
        task_root,
        dynamicjepa_db,
        artifact_id,
        dj_dir,
        "candidate",
    )?;
    let candidate_utility = candidate_downstream
        .get("downstream_utility_score")
        .and_then(Value::as_f64)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_PROMOTION_DOWNSTREAM_SCORE_MISSING",
                "DynamicJEPA candidate downstream utility evidence did not contain a finite score",
                "dynamicjepa.artifact_promotion.downstream_utility_score",
                "run held-out replay or live-shadow planning for this artifact before promotion",
                json!({"artifact_id": artifact_id, "downstream_utility": candidate_downstream}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?;
    let artifacts = dynamicjepa_json_command(
        context_graph_bin,
        vec![
            "dynamicjepa".to_string(),
            "inspect-cf".to_string(),
            "--db".to_string(),
            path_str(dynamicjepa_db).to_string(),
            "--cf".to_string(),
            "dj_model_artifacts".to_string(),
            "--limit".to_string(),
            "1000".to_string(),
            "--json".to_string(),
        ],
        task_root,
        "dynamicjepa_inspect_artifacts_for_promotion",
        Some(file_sot(dynamicjepa_db)),
    )?;
    let evidence_path = dj_dir.join("artifact-comparative-promotion.json");
    let mut baselines = Vec::new();
    if let Some(rows) = artifacts
        .pointer("/decoded_records/rows")
        .and_then(Value::as_array)
    {
        for row in rows {
            let Some(decoded) = row.get("decoded") else {
                continue;
            };
            let record = decoded.get("record").unwrap_or(decoded);
            let row_artifact_id = record.get("artifact_id").and_then(Value::as_str);
            if row_artifact_id == Some(artifact_id) {
                continue;
            }
            if record.get("domain_pack_id").and_then(Value::as_str)
                != Some(DYNAMICJEPA_REALITY_DOMAIN_PACK)
            {
                continue;
            }
            if record.get("status").and_then(Value::as_str) != Some("Active") {
                continue;
            }
            let Some(root) = record.get("artifact_root").and_then(Value::as_str) else {
                continue;
            };
            let report_path = PathBuf::from(root).join("evaluation_report.json");
            if !report_path.exists() || report_path == artifact_root.join("evaluation_report.json")
            {
                continue;
            }
            let report: Value = read_json(&report_path)?;
            if report.get("target_architecture").and_then(Value::as_str)
                != Some("frozen_instrument_projection")
            {
                continue;
            }
            let Some(metric) = report
                .pointer("/split_metrics/val/latent_mse")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite())
            else {
                continue;
            };
            let Some(heldout_proxy) = dynamicjepa_heldout_utility_score(&report) else {
                continue;
            };
            let Some(row_artifact_id) = row_artifact_id else {
                continue;
            };
            let downstream = dynamicjepa_downstream_utility_from_rows(
                context_graph_bin,
                task_root,
                dynamicjepa_db,
                row_artifact_id,
                dj_dir,
                &format!("baseline-{}", safe_id(row_artifact_id)),
            )?;
            let Some(utility_score) = downstream
                .get("downstream_utility_score")
                .and_then(Value::as_f64)
            else {
                continue;
            };
            baselines.push(json!({
                "artifact_id": row_artifact_id,
                "val_latent_mse": metric,
                "heldout_proxy_score": heldout_proxy,
                "downstream_utility_score": utility_score,
                "downstream_utility": downstream,
                "evaluation_report": file_sot(&report_path),
            }));
        }
    }
    baselines.sort_by(|left, right| {
        let l = left
            .get("downstream_utility_score")
            .and_then(Value::as_f64)
            .unwrap_or(f64::NEG_INFINITY);
        let r = right
            .get("downstream_utility_score")
            .and_then(Value::as_f64)
            .unwrap_or(f64::NEG_INFINITY);
        r.total_cmp(&l)
    });
    let best_baseline = baselines.first().cloned();
    let passed = best_baseline
        .as_ref()
        .and_then(|baseline| {
            baseline
                .get("downstream_utility_score")
                .and_then(Value::as_f64)
        })
        .map(|baseline_score| candidate_utility > baseline_score)
        .unwrap_or(true);
    let summary = json!({
        "status": if passed { "passed" } else { "failed" },
        "mode": if best_baseline.is_some() { "candidate_beats_active_downstream_utility_baseline" } else { "initial_artifact_with_downstream_utility_evidence" },
        "candidate": {
            "artifact_id": artifact_id,
            "val_latent_mse": val_latent_mse,
            "heldout_proxy_score": candidate_heldout_proxy,
            "downstream_utility_score": candidate_utility,
            "downstream_utility": candidate_downstream,
            "evaluation_report": file_sot(&artifact_root.join("evaluation_report.json")),
        },
        "best_active_baseline": best_baseline,
        "baseline_count": baselines.len(),
        "source_of_truth": {
            "artifacts": format!("rocksdb:{}#dj_model_artifacts", dynamicjepa_db.display()),
            "evidence": file_sot(&evidence_path),
        },
    });
    write_json_checked(&evidence_path, &summary)?;
    if !passed {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_PROMOTION_REGRESSION",
            "DynamicJEPA artifact lifecycle gate rejected a candidate that did not beat the active downstream-utility baseline",
            "dynamicjepa.artifact_promotion.downstream_utility_score",
            "keep the active artifact and replay/evaluate the candidate on held-out SWE attempts with fewer guard rejects and surprise events",
            read_json(&evidence_path)?,
            Some(file_sot(&evidence_path)),
        ));
    }
    Ok(read_json(&evidence_path)?)
}

fn dynamicjepa_heldout_utility_score(report: &Value) -> Option<f64> {
    let mse = report
        .pointer("/split_metrics/val/latent_mse")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())?;
    let cosine = report
        .pointer("/split_metrics/val/latent_cosine")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())?;
    let contrast = report
        .pointer("/split_metrics/val/action_contrast_acc")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())?;
    let calibrated_segment_count = report
        .get("surprise_segment_calibrations")
        .and_then(Value::as_object)
        .map(|segments| {
            segments
                .values()
                .filter(|segment| {
                    segment.get("status").and_then(Value::as_str) == Some("calibrated")
                })
                .count() as f64
        })
        .unwrap_or(0.0);
    Some(round_signal(
        contrast + cosine - mse + calibrated_segment_count.min(20.0) * 0.01,
    ))
}

fn validate_dynamicjepa_action_control(
    plan: &Value,
    observed_action: &Value,
    transition_id: &str,
    dynamicjepa_db: &Path,
) -> Result<Value> {
    let observed_family = observed_action
        .get("tool_family")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_ACTION_CONTROL_FIELD_MISSING",
                "observed DynamicJEPA action is missing tool_family",
                "dynamicjepa.event.action.tool_family",
                "regenerate structured SWE events before action control",
                json!({"transition_id": transition_id, "observed_action": observed_action}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?;
    let risky = observed_family == "workspace_mutation";
    if !risky {
        return Ok(json!({
            "status": "observed",
            "mode": "post_action_audit",
            "risky_action": false,
            "transition_id": transition_id,
            "observed_action": observed_action,
            "source_of_truth": format!("rocksdb:{}#dj_plan_traces,dj_guard_decisions", dynamicjepa_db.display()),
        }));
    }
    let candidates = plan
        .pointer("/decoded_records/candidate_actions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_ACTION_CONTROL_PLAN_SCHEMA_INVALID",
                "DynamicJEPA plan output was missing candidate_actions for action control",
                "dynamicjepa.plan.decoded_records.candidate_actions",
                "inspect plan output before reward consumption",
                json!({"transition_id": transition_id, "plan": plan}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?;
    let matching_candidate = candidates.iter().find(|candidate| {
        candidate
            .get("fields")
            .is_some_and(|fields| action_fields_match(fields, observed_action))
    });
    let Some(candidate) = matching_candidate else {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_ACTION_NOT_DECLARED",
            "risky observed action was not present in the DynamicJEPA planner candidate set",
            "dynamicjepa.plan.decoded_records.candidate_actions",
            "update the SWE-loop domain pack planner records or block this action before execution",
            json!({"transition_id": transition_id, "observed_action": observed_action, "plan": plan}),
            Some(file_sot(dynamicjepa_db)),
        ));
    };
    let candidate_action_id = candidate
        .get("action_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_ACTION_CONTROL_CANDIDATE_ID_MISSING",
                "DynamicJEPA plan candidate was missing action_id",
                "dynamicjepa.plan.decoded_records.candidate_actions[].action_id",
                "inspect plan output before reward consumption",
                json!({"transition_id": transition_id, "candidate": candidate}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?;
    let selected_action_id = plan
        .pointer("/decoded_records/plan_trace/selected_action_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_ACTION_CONTROL_SELECTED_ID_MISSING",
                "DynamicJEPA plan trace did not select an action for risky action control",
                "dynamicjepa.plan.decoded_records.plan_trace.selected_action_id",
                "inspect guard decisions and model artifact before reward consumption",
                json!({"transition_id": transition_id, "plan": plan}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?;
    let guard_decisions = plan
        .pointer("/decoded_records/guard_decisions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_ACTION_CONTROL_GUARDS_MISSING",
                "DynamicJEPA plan output was missing guard_decisions for action control",
                "dynamicjepa.plan.decoded_records.guard_decisions",
                "inspect plan output before reward consumption",
                json!({"transition_id": transition_id, "plan": plan}),
                Some(file_sot(dynamicjepa_db)),
            )
        })?;
    let rejecting_guard = guard_decisions.iter().find(|guard| {
        guard.get("candidate_action_id").and_then(Value::as_str) == Some(candidate_action_id)
            && guard.get("decision").and_then(Value::as_str) == Some("Reject")
    });
    if let Some(guard) = rejecting_guard {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_ACTION_GUARD_REJECTED",
            "risky observed action was rejected by DynamicJEPA guard decisions",
            "dynamicjepa.plan.decoded_records.guard_decisions",
            "block this action before execution or fix the domain guard calibration",
            json!({"transition_id": transition_id, "observed_action": observed_action, "guard": guard}),
            Some(format!(
                "rocksdb:{}#dj_guard_decisions",
                dynamicjepa_db.display()
            )),
        ));
    }
    if selected_action_id != candidate_action_id {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_ACTION_DIVERGED_FROM_PLAN",
            "risky observed action did not match DynamicJEPA selected action",
            "dynamicjepa.plan.decoded_records.plan_trace.selected_action_id",
            "route risky tool actions through DynamicJEPA planning before execution",
            json!({
                "transition_id": transition_id,
                "observed_action": observed_action,
                "observed_candidate_action_id": candidate_action_id,
                "selected_action_id": selected_action_id,
            }),
            Some(format!(
                "rocksdb:{}#dj_plan_traces",
                dynamicjepa_db.display()
            )),
        ));
    }
    Ok(json!({
        "status": "passed",
        "mode": "post_action_hard_gate",
        "risky_action": true,
        "transition_id": transition_id,
        "observed_candidate_action_id": candidate_action_id,
        "selected_action_id": selected_action_id,
        "observed_action": observed_action,
        "source_of_truth": format!("rocksdb:{}#dj_plan_traces,dj_guard_decisions", dynamicjepa_db.display()),
    }))
}

fn action_fields_match(candidate_fields: &Value, observed_action: &Value) -> bool {
    let Some(candidate) = candidate_fields.as_object() else {
        return false;
    };
    let Some(observed) = observed_action.as_object() else {
        return false;
    };
    const HARD_ACTION_CONTROL_FIELDS: &[&str] = &[
        "delta",
        "tool_family",
        "command_kind",
        "edit_size_bucket",
        "path_risk",
        "target_path_bucket",
        "planned_changed_file_count",
        "patch_intent_bucket",
        "patch_shape_bucket",
        "patch_scope_bucket",
        "ast_language",
        "ast_parse_status",
        "ast_node_kind",
        "ast_symbol_kind",
        "ast_edit_kind",
        "ast_touched_node_count",
        "ast_symbol_count",
        "semantic_provider",
        "semantic_status",
        "symbol_visibility",
        "patch_delta_kind",
        "control_flow_delta",
        "api_contract_delta",
        "dependency_blast_radius",
        "reverse_reference_count",
        "compiler_semantic_status",
        "type_resolution_status",
        "import_resolution_status",
        "compiler_diagnostic_count",
        "test_attribution_status",
        "attributed_test_count",
        "predicted_verifier_target",
    ];
    for field in HARD_ACTION_CONTROL_FIELDS {
        let Some(observed_value) = observed.get(*field) else {
            return false;
        };
        let Some(candidate_value) = candidate.get(*field) else {
            return false;
        };
        if field_value_to_plain_json(candidate_value) != Some(observed_value.clone()) {
            return false;
        }
    }
    true
}

fn field_value_to_plain_json(value: &Value) -> Option<Value> {
    if let Some(v) = value.get("I64").and_then(Value::as_i64) {
        return Some(json!(v));
    }
    if let Some(v) = value.get("F64").and_then(Value::as_f64) {
        return Some(json!(v));
    }
    if let Some(v) = value.get("Bool").and_then(Value::as_bool) {
        return Some(json!(v));
    }
    if let Some(v) = value.get("String").and_then(Value::as_str) {
        return Some(json!(v));
    }
    if let Some(v) = value
        .pointer("/Categorical/variant")
        .and_then(Value::as_str)
    {
        return Some(json!(v));
    }
    None
}

#[derive(Debug, Clone)]
struct SweDynamicJepaEvent {
    event: Value,
    transition_id: String,
}

fn dynamicjepa_swe_events_from_transcript(
    run_id: &str,
    attempt: u64,
    transcript_path: &Path,
    transcript: &Value,
    repo_dir: &Path,
) -> Result<Vec<SweDynamicJepaEvent>> {
    let mut events = Vec::new();
    if let Some(entries) = transcript.get("entries").and_then(Value::as_array) {
        for entry in entries {
            if let Some(results) = entry.get("tool_results").and_then(Value::as_array) {
                for result in results {
                    let transition_sot = result
                        .get("harness_transition")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_TRANSITION_REF_MISSING",
                                "tool transcript entry was missing the harness transition needed for trained DynamicJEPA reality",
                                "agent_tool_transcript.entries[].tool_results[].harness_transition",
                                "rerun with the current Rust reality-loop so every tool call emits a transition artifact",
                                json!({"tool_result": result, "transcript": file_sot(transcript_path)}),
                                Some(file_sot(transcript_path)),
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
                            "RUST_REALITY_DYNAMICJEPA_TRANSITION_KIND_INVALID",
                            "trained DynamicJEPA reality requires current contextgraph_harness_transition artifacts",
                            "harness_transition.record_kind",
                            "regenerate the transition with the current harness before trained inference",
                            json!({"transition": file_sot(&transition_path), "record_kind": transition.get("record_kind")}),
                            Some(file_sot(&transition_path)),
                        ));
                    }
                    let mapping = transition
                        .get("dynamicjepa_mapping")
                        .and_then(Value::as_object)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_MAPPING_MISSING",
                                "harness transition was missing dynamicjepa_mapping",
                                "harness_transition.dynamicjepa_mapping",
                                "regenerate the transition with the current harness before trained inference",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    let domain_pack = mapping
                        .get("domain_pack_id")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_DOMAIN_PACK_MISSING",
                                "dynamicjepa_mapping.domain_pack_id was missing",
                                "harness_transition.dynamicjepa_mapping.domain_pack_id",
                                "regenerate the transition with a typed DynamicJEPA domain pack id",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    if domain_pack != DYNAMICJEPA_REALITY_DOMAIN_PACK {
                        return Err(RealityError::new(
                            "RUST_REALITY_DYNAMICJEPA_DOMAIN_PACK_MISMATCH",
                            "dynamicjepa_mapping.domain_pack_id did not match the reality-loop domain pack",
                            "harness_transition.dynamicjepa_mapping.domain_pack_id",
                            "do not mix DynamicJEPA domains inside one reality-loop trained inference record",
                            json!({"expected": DYNAMICJEPA_REALITY_DOMAIN_PACK, "actual": domain_pack, "transition": file_sot(&transition_path)}),
                            Some(file_sot(&transition_path)),
                        ));
                    }
                    let transition_id = mapping
                        .get("transition_id")
                        .and_then(Value::as_str)
                        .or_else(|| transition.get("transition_id").and_then(Value::as_str))
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_TRANSITION_ID_MISSING",
                                "dynamicjepa_mapping.transition_id was missing",
                                "harness_transition.dynamicjepa_mapping.transition_id",
                                "regenerate the transition with a stable transition_id",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?
                        .to_string();
                    let counter = transition
                        .pointer("/before_state/executed_tool_calls_before")
                        .and_then(Value::as_i64)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_STATE_COUNTER_MISSING",
                                "harness transition was missing executed_tool_calls_before",
                                "harness_transition.before_state.executed_tool_calls_before",
                                "regenerate the transition with the current harness state snapshot",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    let utility_delta = transition
                        .pointer("/outcome/utility_delta")
                        .and_then(Value::as_f64)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_UTILITY_DELTA_MISSING",
                                "harness transition was missing outcome.utility_delta",
                                "harness_transition.outcome.utility_delta",
                                "regenerate the transition with current outcome shaping",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    if !utility_delta.is_finite() {
                        return Err(RealityError::new(
                            "RUST_REALITY_DYNAMICJEPA_UTILITY_DELTA_INVALID",
                            "harness transition utility_delta was non-finite",
                            "harness_transition.outcome.utility_delta",
                            "inspect transition generation before DynamicJEPA ingestion",
                            json!({"transition": file_sot(&transition_path), "utility_delta": utility_delta}),
                            Some(file_sot(&transition_path)),
                        ));
                    }
                    let delta = (utility_delta * 10.0).round() as i64;
                    if !(-10..=10).contains(&delta) {
                        return Err(RealityError::new(
                            "RUST_REALITY_DYNAMICJEPA_DELTA_OUT_OF_RANGE",
                            "quantized tool utility delta exceeded the SWE DynamicJEPA action bounds",
                            "dynamicjepa.event.action.delta",
                            "adjust the domain pack bounds only after inspecting the real transition utility distribution",
                            json!({"transition": file_sot(&transition_path), "utility_delta": utility_delta, "delta": delta}),
                            Some(file_sot(&transition_path)),
                        ));
                    }
                    let round = transition
                        .get("round")
                        .and_then(Value::as_u64)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_ROUND_MISSING",
                                "harness transition was missing round for structured SWE event materialization",
                                "harness_transition.round",
                                "regenerate the transition with current reality-loop metadata",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    let index = transition
                        .get("index")
                        .and_then(Value::as_u64)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_INDEX_MISSING",
                                "harness transition was missing index for structured SWE event materialization",
                                "harness_transition.index",
                                "regenerate the transition with current reality-loop metadata",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    let ts_ms = (attempt as i64)
                        .saturating_mul(1_000_000)
                        .saturating_add((round as i64).saturating_mul(1_000))
                        .saturating_add(index as i64);
                    let tool_name = transition
                        .get("tool_name")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_TOOL_NAME_MISSING",
                                "harness transition was missing tool_name for structured SWE event materialization",
                                "harness_transition.tool_name",
                                "regenerate the transition with current tool metadata",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    let tool_family = transition
                        .pointer("/constellation_signal/tool_family")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_TOOL_FAMILY_MISSING",
                                "harness transition was missing constellation_signal.tool_family for structured SWE event materialization",
                                "harness_transition.constellation_signal.tool_family",
                                "regenerate the transition with current constellation metadata",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    let tool_family = swe_tool_family_variant(tool_family);
                    let command_kind = swe_command_kind_variant(tool_name);
                    let requested_sot = transition
                        .pointer("/action/requested")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_REQUESTED_TOOL_CALL_MISSING",
                                "harness transition was missing action.requested for argument-aware SWE event materialization",
                                "harness_transition.action.requested",
                                "regenerate the transition with the requested-tool-call source-of-truth path",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    let requested_path = file_sot_to_path(
                        requested_sot,
                        "RUST_REALITY_DYNAMICJEPA_REQUESTED_TOOL_CALL_SOT_INVALID",
                        "harness_transition.action.requested",
                    )?;
                    let requested_call: Value = read_json(&requested_path)?;
                    let requested_arguments = requested_call
                        .get("arguments")
                        .filter(|value| value.is_object())
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_REQUESTED_TOOL_ARGUMENTS_INVALID",
                                "requested tool-call readback did not contain a JSON object arguments field",
                                "requested_tool_call.arguments",
                                "inspect the requested-tool-call artifact before DynamicJEPA ingestion",
                                json!({"requested_tool_call": file_sot(&requested_path), "arguments": requested_call.get("arguments")}),
                                Some(file_sot(&requested_path)),
                            )
                        })?;
                    let planned_paths =
                        dynamicjepa_tool_argument_paths(tool_name, requested_arguments);
                    let planned_patch_bytes =
                        dynamicjepa_tool_argument_patch_bytes(tool_name, requested_arguments)?;
                    let planned_changed_file_count =
                        i64::try_from(planned_paths.len()).map_err(|err| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_PLANNED_PATH_COUNT_OVERFLOW",
                                "planned tool argument path count did not fit the SWE DynamicJEPA i64 schema",
                                "dynamicjepa.event.action.planned_changed_file_count",
                                "inspect requested tool arguments before DynamicJEPA ingestion",
                                json!({"transition": file_sot(&transition_path), "path_count": planned_paths.len(), "error": err.to_string()}),
                                Some(file_sot(&requested_path)),
                            )
                        })?;
                    let changed_files = transition_string_array(
                        &transition,
                        "/observation/changed_files",
                        "harness_transition.observation.changed_files",
                        &transition_path,
                    )?;
                    let observed_paths = transition_string_array(
                        &transition,
                        "/observation/observed_paths",
                        "harness_transition.observation.observed_paths",
                        &transition_path,
                    )?;
                    let patch_bytes = transition_patch_size_bytes(&transition)?;
                    let path_risk = swe_path_risk(&changed_files, &observed_paths);
                    let target_path_bucket = swe_path_bucket(&changed_files, &observed_paths);
                    let ast_semantics = dynamicjepa_ast_edit_semantics(
                        repo_dir,
                        tool_name,
                        requested_arguments,
                        &planned_paths,
                    )?;
                    let symbol_bucket = swe_symbol_bucket_from_ast_or_path(
                        tool_name,
                        &changed_files,
                        &observed_paths,
                        &ast_semantics,
                    );
                    let dependency_layer = swe_dependency_layer(&changed_files, &observed_paths);
                    let result_status = transition
                        .pointer("/observation/status")
                        .and_then(Value::as_str)
                        .map(swe_result_status_variant)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_RESULT_STATUS_MISSING",
                                "harness transition was missing observation.status for structured SWE event materialization",
                                "harness_transition.observation.status",
                                "regenerate the transition with current tool result status metadata",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    let error_code = transition
                        .pointer("/outcome/error_code")
                        .or_else(|| transition.pointer("/observation/error_code"))
                        .and_then(Value::as_str);
                    let error_signature = swe_error_signature(error_code);
                    let test_target_kind = swe_test_target_kind(
                        tool_name,
                        error_code,
                        &changed_files,
                        &observed_paths,
                    );
                    let ownership_bucket = swe_ownership_bucket(&changed_files, &observed_paths);
                    let outcome_label = transition
                        .pointer("/outcome/label")
                        .and_then(Value::as_str)
                        .map(swe_outcome_label_variant)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_OUTCOME_LABEL_MISSING",
                                "harness transition was missing outcome.label for structured SWE event materialization",
                                "harness_transition.outcome.label",
                                "regenerate the transition with current outcome metadata",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    let correction_required = transition
                        .pointer("/outcome/correction_required")
                        .and_then(Value::as_bool)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_CORRECTION_REQUIRED_MISSING",
                                "harness transition was missing outcome.correction_required for structured SWE event materialization",
                                "harness_transition.outcome.correction_required",
                                "regenerate the transition with current outcome metadata",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    let repo_dirty_state = transition
                        .pointer("/before_state/repo_status_porcelain")
                        .and_then(Value::as_str)
                        .map(|status| if status.trim().is_empty() { "clean" } else { "dirty" })
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_REPO_STATUS_MISSING",
                                "harness transition was missing repo_status_porcelain for structured SWE event materialization",
                                "harness_transition.before_state.repo_status_porcelain",
                                "regenerate the transition with current git status metadata",
                                json!({"transition": file_sot(&transition_path)}),
                                Some(file_sot(&transition_path)),
                            )
                        })?;
                    let model_id = transcript
                        .get("model")
                        .and_then(Value::as_str)
                        .map(swe_model_id_variant)
                        .ok_or_else(|| {
                            RealityError::new(
                                "RUST_REALITY_DYNAMICJEPA_TRANSCRIPT_MODEL_MISSING",
                                "agent tool transcript was missing model for structured SWE event materialization",
                                "agent_tool_transcript.model",
                                "regenerate the transcript with current inner-model metadata",
                                json!({"transcript": file_sot(transcript_path)}),
                                Some(file_sot(transcript_path)),
                            )
                        })?;
                    events.push(SweDynamicJepaEvent {
                        transition_id: transition_id.clone(),
                        event: json!({
                            "state": {
                                "counter": counter,
                                "attempt": attempt as i64,
                                "round": round as i64,
                                "tool_family": tool_family,
                                "repo_dirty_state": repo_dirty_state,
                                "prompt_version": "v1",
                                "model_id": model_id,
                                "primary_path_bucket": target_path_bucket,
                                "symbol_bucket": symbol_bucket,
                                "dependency_layer": dependency_layer,
                                "test_target_kind": test_target_kind,
                                "error_signature": error_signature,
                                "ownership_bucket": ownership_bucket
                            },
                            "action": {
                                "delta": delta,
                                "tool_family": tool_family,
                                "command_kind": command_kind,
                                "edit_size_bucket": swe_edit_size_bucket(patch_bytes),
                                "path_risk": path_risk,
                                "target_path_bucket": target_path_bucket,
                                "symbol_bucket": symbol_bucket,
                                "dependency_layer": dependency_layer,
                                "test_target_kind": test_target_kind,
                                "ownership_bucket": ownership_bucket,
                                "planned_patch_bytes": planned_patch_bytes,
                                "planned_changed_file_count": planned_changed_file_count,
                                "patch_intent_bucket": swe_patch_intent_bucket(tool_name, planned_patch_bytes, path_risk),
                                "patch_shape_bucket": swe_patch_shape_bucket(tool_name, requested_arguments, &planned_paths),
                                "patch_scope_bucket": swe_patch_scope_bucket_from_ast_or_path(tool_name, &planned_paths, &ast_semantics),
                                "ast_language": ast_semantics.language,
                                "ast_parse_status": ast_semantics.parse_status,
                                "ast_node_kind": ast_semantics.node_kind,
                                "ast_symbol_kind": ast_semantics.symbol_kind,
                                "ast_edit_kind": ast_semantics.edit_kind,
                                "ast_touched_node_count": ast_semantics.touched_node_count,
                                "ast_symbol_count": ast_semantics.symbol_count,
                                "semantic_provider": ast_semantics.semantic_provider,
                                "semantic_status": ast_semantics.semantic_status,
                                "symbol_visibility": ast_semantics.symbol_visibility,
                                "patch_delta_kind": ast_semantics.patch_delta_kind,
                                "control_flow_delta": ast_semantics.control_flow_delta,
                                "api_contract_delta": ast_semantics.api_contract_delta,
                                "dependency_blast_radius": ast_semantics.dependency_blast_radius,
                                "reverse_reference_count": ast_semantics.reverse_reference_count,
                                "compiler_semantic_status": ast_semantics.compiler_semantic_status,
                                "type_resolution_status": ast_semantics.type_resolution_status,
                                "import_resolution_status": ast_semantics.import_resolution_status,
                                "compiler_diagnostic_count": ast_semantics.compiler_diagnostic_count,
                                "test_attribution_status": ast_semantics.test_attribution_status,
                                "attributed_test_count": ast_semantics.attributed_test_count,
                                "predicted_verifier_target": ast_semantics.predicted_verifier_target
                            },
                            "outcome": {
                                "next_counter": counter + delta,
                                "outcome_label": outcome_label,
                                "result_status": result_status,
                                "verifier_status": swe_verifier_status(tool_name, result_status),
                                "test_failure_class": swe_test_failure_class(error_code),
                                "incident_code": swe_incident_code(error_code),
                                "changed_file_count": i64::try_from(changed_files.len()).map_err(|err| RealityError::new(
                                    "RUST_REALITY_DYNAMICJEPA_CHANGED_FILE_COUNT_OVERFLOW",
                                    "changed_files length did not fit the SWE DynamicJEPA i64 schema",
                                    "dynamicjepa.event.outcome.changed_file_count",
                                    "inspect the transition artifact before DynamicJEPA ingestion",
                                    json!({"transition": file_sot(&transition_path), "error": err.to_string()}),
                                    Some(file_sot(&transition_path)),
                                ))?,
                                "patch_bytes": patch_bytes,
                                "correction_required": if correction_required { "true" } else { "false" },
                                "file_risk": path_risk,
                                "error_signature": error_signature,
                                "symbol_bucket": symbol_bucket,
                                "dependency_layer": dependency_layer,
                                "test_target_kind": test_target_kind,
                                "ownership_bucket": ownership_bucket
                            },
                            "ts_ms": ts_ms,
                            "trajectory_id": format!("{run_id}:attempt-{attempt}"),
                            "run_id": run_id,
                            "transition_id": transition_id,
                        }),
                    });
                }
            }
        }
    }
    let executed = transcript
        .get("executed_tool_calls")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_EXECUTED_COUNT_MISSING",
                "agent tool transcript was missing executed_tool_calls",
                "agent_tool_transcript.executed_tool_calls",
                "rerun with the current inner harness so DynamicJEPA coverage can be measured",
                json!({"transcript": file_sot(transcript_path)}),
                Some(file_sot(transcript_path)),
            )
        })?;
    if events.len() as u64 != executed {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_PANEL_COUNT_MISMATCH",
            "DynamicJEPA event count did not equal executed tool-call count",
            "agent_tool_transcript.executed_tool_calls",
            "inspect missing transition artifacts before trained DynamicJEPA inference",
            json!({"executed_tool_calls": executed, "event_count": events.len(), "transcript": file_sot(transcript_path)}),
            Some(file_sot(transcript_path)),
        ));
    }
    Ok(events)
}

fn transition_string_array(
    transition: &Value,
    pointer: &str,
    field_path: &'static str,
    transition_path: &Path,
) -> Result<Vec<String>> {
    let array = transition.pointer(pointer).and_then(Value::as_array).ok_or_else(|| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_TRANSITION_ARRAY_MISSING",
            "harness transition was missing a required array for structured SWE event materialization",
            field_path,
            "regenerate the transition with current observation metadata",
            json!({"transition": file_sot(transition_path), "pointer": pointer}),
            Some(file_sot(transition_path)),
        )
    })?;
    array
        .iter()
        .enumerate()
        .map(|(idx, value)| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                RealityError::new(
                    "RUST_REALITY_DYNAMICJEPA_TRANSITION_ARRAY_ITEM_INVALID",
                    "harness transition array contained a non-string item",
                    format!("{field_path}[{idx}]"),
                    "regenerate the transition with string file path arrays",
                    json!({"transition": file_sot(transition_path), "item": value}),
                    Some(file_sot(transition_path)),
                )
            })
        })
        .collect()
}

fn transition_patch_size_bytes(transition: &Value) -> Result<i64> {
    let Some(raw) = transition
        .pointer("/observation/candidate_patch")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(0);
    };
    let path = PathBuf::from(raw.strip_prefix("file:").unwrap_or(raw));
    if !path.exists() {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_PATCH_SOT_MISSING",
            "harness transition candidate_patch source-of-truth file does not exist",
            "harness_transition.observation.candidate_patch",
            "regenerate the final edit transition or repair the candidate patch artifact path",
            json!({"candidate_patch": raw}),
            Some(file_sot(&path)),
        ));
    }
    let size = fs::metadata(&path)
        .map_err(|err| {
            RealityError::new(
                "RUST_REALITY_DYNAMICJEPA_PATCH_METADATA_FAILED",
                "failed to read candidate_patch metadata for structured SWE event materialization",
                "harness_transition.observation.candidate_patch",
                "verify the candidate patch artifact is readable",
                json!({"candidate_patch": file_sot(&path), "error": err.to_string()}),
                Some(file_sot(&path)),
            )
        })?
        .len();
    i64::try_from(size).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_PATCH_BYTES_OVERFLOW",
            "candidate_patch size did not fit the SWE DynamicJEPA i64 schema",
            "dynamicjepa.event.outcome.patch_bytes",
            "inspect the candidate patch artifact size before DynamicJEPA ingestion",
            json!({"candidate_patch": file_sot(&path), "size_bytes": size, "error": err.to_string()}),
            Some(file_sot(&path)),
        )
    })
}

fn swe_tool_family_variant(value: &str) -> &'static str {
    match value {
        "reality_readback" => "reality_readback",
        "repository_navigation" => "repository_navigation",
        "workspace_mutation" => "workspace_mutation",
        "verification" => "verification",
        "memory_retrieval" => "memory_retrieval",
        "deterministic_analysis" => "deterministic_analysis",
        _ => "unknown_tool_family",
    }
}

fn swe_command_kind_variant(tool_name: &str) -> &'static str {
    match tool_name {
        "read_problem_reality" => "read_problem_reality",
        "read_official_failure_reality" => "read_official_failure_reality",
        "list_files" => "list_files",
        "inspect_symbol" => "inspect_symbol",
        "search_repo" => "search_repo",
        "open_file_window" => "open_file_window",
        "apply_line_window_edit" => "apply_line_window_edit",
        "apply_structured_edit" => "apply_structured_edit",
        "apply_unified_diff" => "apply_unified_diff",
        "run_targeted_test" => "run_targeted_test",
        "run_official_swe_eval" => "run_official_swe_eval",
        "compare_attempts" => "compare_attempts",
        "retrieve_similar_verified_episodes" => "retrieve_similar_verified_episodes",
        "synthesize_helper_tool" => "synthesize_helper_tool",
        _ => "unknown_tool",
    }
}

fn swe_result_status_variant(status: &str) -> &'static str {
    match status {
        "ok" => "ok",
        "failed" => "failed",
        "blocked" => "blocked",
        "error" => "error",
        _ => "unknown",
    }
}

fn swe_outcome_label_variant(label: &str) -> &'static str {
    match label {
        "Useful" => "Useful",
        "Neutral" => "Neutral",
        "Harmful" => "Harmful",
        _ => "Unknown",
    }
}

fn swe_edit_size_bucket(patch_bytes: i64) -> &'static str {
    match patch_bytes {
        0 => "none",
        1..=2048 => "small",
        2049..=16384 => "medium",
        _ => "large",
    }
}

fn swe_path_risk(changed_files: &[String], observed_paths: &[String]) -> &'static str {
    let paths = if changed_files.is_empty() {
        observed_paths
    } else {
        changed_files
    };
    if paths.is_empty() {
        return "none";
    }
    if paths.len() > 1 {
        return "multiple";
    }
    let path = paths[0].as_str();
    if path.starts_with("tests/") || path.contains("/tests/") || path.ends_with("_test.py") {
        "tests"
    } else if path.ends_with("pyproject.toml")
        || path.ends_with("setup.py")
        || path.ends_with("setup.cfg")
        || path.ends_with("Cargo.toml")
        || path.ends_with("Cargo.lock")
    {
        "config_or_build"
    } else if looks_like_source_path(path) {
        "source"
    } else {
        "unknown"
    }
}

fn swe_path_bucket(changed_files: &[String], observed_paths: &[String]) -> &'static str {
    let paths = if changed_files.is_empty() {
        observed_paths
    } else {
        changed_files
    };
    if paths.is_empty() {
        return "none";
    }
    if paths.len() > 1 {
        return "multiple";
    }
    let path = paths[0].as_str();
    if path.starts_with("tests/") || path.contains("/tests/") || path.ends_with("_test.py") {
        "tests"
    } else if path.ends_with("pyproject.toml")
        || path.ends_with("setup.py")
        || path.ends_with("setup.cfg")
        || path.ends_with("Cargo.toml")
        || path.ends_with("Cargo.lock")
    {
        "config_or_build"
    } else if path.ends_with(".md") || path.starts_with("docs/") || path.contains("/docs/") {
        "docs"
    } else if path.contains("migration") || path.contains("migrations/") {
        "migrations"
    } else if looks_like_source_path(path) {
        "source"
    } else {
        "unknown"
    }
}

fn swe_symbol_bucket(
    tool_name: &str,
    changed_files: &[String],
    observed_paths: &[String],
) -> &'static str {
    let paths = if changed_files.is_empty() {
        observed_paths
    } else {
        changed_files
    };
    if paths.is_empty() {
        return "none";
    }
    if tool_name == "inspect_symbol" {
        return "function";
    }
    let first = paths[0].as_str();
    if first.starts_with("tests/") || first.contains("/tests/") || first.ends_with("_test.py") {
        "test_case"
    } else if first.ends_with("pyproject.toml")
        || first.ends_with("setup.cfg")
        || first.ends_with("Cargo.toml")
    {
        "config_key"
    } else if looks_like_source_path(first) {
        "module"
    } else {
        "unknown"
    }
}

fn looks_like_source_path(path: &str) -> bool {
    [
        ".py", ".rs", ".js", ".jsx", ".ts", ".tsx", ".go", ".java", ".c", ".cc", ".cpp", ".cxx",
        ".h", ".hh", ".hpp", ".hxx", ".h++", ".ipp", ".inl", ".tpp", ".txx", ".ixx", ".cs", ".rb",
        ".php", ".swift", ".kt", ".kts", ".scala",
    ]
    .iter()
    .any(|suffix| path.ends_with(suffix))
}

fn swe_symbol_bucket_from_ast_or_path(
    tool_name: &str,
    changed_files: &[String],
    observed_paths: &[String],
    ast: &SweAstEditSemantics,
) -> &'static str {
    if ast.parse_status == "parsed" {
        return match ast.symbol_kind {
            "function" | "method" => "function",
            "class" => "class",
            "module" | "mixed" => "module",
            "test_case" => "test_case",
            "config_key" => "config_key",
            "none" => "none",
            _ => "unknown",
        };
    }
    swe_symbol_bucket(tool_name, changed_files, observed_paths)
}

fn swe_dependency_layer(changed_files: &[String], observed_paths: &[String]) -> &'static str {
    let paths = if changed_files.is_empty() {
        observed_paths
    } else {
        changed_files
    };
    let Some(path) = paths.first().map(String::as_str) else {
        return "unknown";
    };
    let lower = path.to_ascii_lowercase();
    if lower.contains("/cli") || lower.contains("bin/") || lower.contains("command") {
        "cli"
    } else if lower.contains("handler") || lower.contains("service") || lower.contains("/api") {
        "api"
    } else if lower.contains("schema") || lower.contains("model") || lower.contains("record") {
        "data_model"
    } else if lower.contains("storage") || lower.contains("rocksdb") || lower.contains("sqlite") {
        "storage"
    } else if lower.starts_with("tests/") || lower.contains("/tests/") || lower.contains("harness")
    {
        "test_harness"
    } else if lower.ends_with("toml")
        || lower.ends_with("json")
        || lower.ends_with("yaml")
        || lower.ends_with("yml")
        || lower.contains("config")
    {
        "config"
    } else {
        "unknown"
    }
}

fn swe_error_signature(error_code: Option<&str>) -> &'static str {
    let Some(code) = error_code else {
        return "none";
    };
    if code.contains("ASSERT") {
        "assertion"
    } else if code.contains("IMPORT") {
        "import_error"
    } else if code.contains("PY_COMPILE") || code.contains("SYNTAX") {
        "syntax_error"
    } else if code.contains("TIMEOUT") {
        "timeout"
    } else if code.contains("COLLECT") {
        "collection"
    } else if code.trim().is_empty() {
        "none"
    } else {
        "tool_error"
    }
}

fn swe_test_target_kind(
    tool_name: &str,
    error_code: Option<&str>,
    changed_files: &[String],
    observed_paths: &[String],
) -> &'static str {
    if tool_name == "run_official_swe_eval" {
        return "official_swe";
    }
    if tool_name == "run_targeted_test" {
        return "unit";
    }
    if matches!(swe_error_signature(error_code), "collection") {
        return "collection";
    }
    let bucket = swe_path_bucket(changed_files, observed_paths);
    if bucket == "tests" {
        "unit"
    } else {
        "none"
    }
}

fn swe_ownership_bucket(changed_files: &[String], observed_paths: &[String]) -> &'static str {
    match swe_path_bucket(changed_files, observed_paths) {
        "none" => "none",
        "tests" => "tests",
        "config_or_build" => "build_or_config",
        "source" | "docs" | "migrations" => "application",
        "multiple" => "application",
        _ => "unknown",
    }
}

fn swe_verifier_status(tool_name: &str, result_status: &str) -> &'static str {
    match (tool_name, result_status) {
        ("run_targeted_test", "ok") => "targeted_ok",
        ("run_targeted_test", "failed" | "blocked" | "error") => "targeted_failed",
        ("run_official_swe_eval", "ok") => "official_ok",
        ("run_official_swe_eval", "failed" | "blocked" | "error") => "official_failed",
        (_, "blocked") => "blocked",
        _ => "not_run",
    }
}

fn swe_test_failure_class(error_code: Option<&str>) -> &'static str {
    let Some(code) = error_code else {
        return "none";
    };
    if code.contains("ASSERT") {
        "assertion"
    } else if code.contains("IMPORT") {
        "import_error"
    } else if code.contains("PY_COMPILE") || code.contains("SYNTAX") {
        "syntax_error"
    } else if code.contains("TIMEOUT") {
        "timeout"
    } else if code.contains("COLLECT") {
        "collection"
    } else {
        "unknown"
    }
}

fn swe_incident_code(error_code: Option<&str>) -> &'static str {
    let Some(code) = error_code else {
        return "none";
    };
    if code.contains("POLICY") || code.contains("SOURCE_TEST_BOUNDARY") {
        "policy_violation"
    } else if code.contains("REPEATED_ERROR_DEADLOCK") {
        "repeated_error_deadlock"
    } else if code.contains("EDIT") || code.contains("COMPILE") || code.contains("DIFF") {
        "edit_compile_error"
    } else {
        "tool_error"
    }
}

fn swe_model_id_variant(model: &str) -> &'static str {
    let normalized = model.to_ascii_lowercase().replace(['-', '_', '.'], "");
    if normalized.contains("qwen") && normalized.contains("27b") {
        "qwen3_6_27b"
    } else {
        "other"
    }
}

fn dynamicjepa_json_command(
    bin: &Path,
    args: Vec<String>,
    cwd: &Path,
    operation: &'static str,
    source_of_truth: Option<String>,
) -> Result<Value> {
    let mut command = vec![path_str(bin).to_string()];
    command.extend(args);
    let result = run_cmd(&command, cwd, 600)?;
    if result.exit_code != 0 {
        return Err(RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_COMMAND_FAILED",
            "DynamicJEPA command failed",
            operation,
            "inspect stdout/stderr, DynamicJEPA RocksDB path, domain registration, and artifact id",
            json!({ "process": result }),
            source_of_truth.clone(),
        ));
    }
    serde_json::from_str(&result.stdout).map_err(|err| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_JSON_INVALID",
            "DynamicJEPA command did not return JSON",
            operation,
            "inspect DynamicJEPA command output",
            json!({"stdout": result.stdout, "stderr": result.stderr, "error": err.to_string()}),
            source_of_truth,
        )
    })
}

fn required_json_string_array(
    value: &Value,
    pointer: &str,
    field_path: &'static str,
    source_path: &Path,
) -> Result<Vec<String>> {
    let array = value.pointer(pointer).and_then(Value::as_array).ok_or_else(|| {
        RealityError::new(
            "RUST_REALITY_DYNAMICJEPA_OUTPUT_SCHEMA_INVALID",
            "DynamicJEPA command output was missing a required string array",
            field_path,
            "inspect the command JSON output and update the caller only after confirming schema changes",
            json!({"pointer": pointer, "value": value}),
            Some(file_sot(source_path)),
        )
    })?;
    array
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            item.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                RealityError::new(
                    "RUST_REALITY_DYNAMICJEPA_OUTPUT_SCHEMA_INVALID",
                    "DynamicJEPA command output array contained a non-string item",
                    format!("{field_path}[{idx}]"),
                    "inspect the command JSON output and source record ids",
                    json!({"item": item, "pointer": pointer}),
                    Some(file_sot(source_path)),
                )
            })
        })
        .collect()
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

fn normalized_attempt_reward(reward: Option<&Value>, resolved: Option<bool>) -> f64 {
    if resolved == Some(true) {
        return 1.0;
    }
    extract_reward_scalar(reward).unwrap_or(0.0).clamp(0.0, 1.0)
}

fn attempt_success(resolved: Option<bool>, _validation: &Value) -> bool {
    resolved == Some(true)
}

fn attempt_outcome_kind(
    validation: &Value,
    resolved: Option<bool>,
    repeated_patch: bool,
    patch_sha: Option<&str>,
) -> String {
    if resolved == Some(true) {
        "official_resolved".to_string()
    } else if resolved == Some(false) {
        "official_unresolved".to_string()
    } else if repeated_patch {
        "repeated_patch".to_string()
    } else if patch_sha.is_none() {
        "no_candidate_patch".to_string()
    } else if validation["status"] != "passed" {
        "validation_failed".to_string()
    } else {
        "validation_passed_official_not_run".to_string()
    }
}

fn attempt_tags(
    validation: &Value,
    resolved: Option<bool>,
    repeated_patch: bool,
    patch_sha: Option<&str>,
) -> Vec<String> {
    let mut tags = BTreeSet::new();
    match validation["status"].as_str().unwrap_or("unknown") {
        "passed" => {
            tags.insert("validation-passed".to_string());
        }
        "failed" => {
            tags.insert("validation-failed".to_string());
        }
        other => {
            tags.insert(format!("validation-{}", safe_id(other)));
        }
    }
    match resolved {
        Some(true) => {
            tags.insert("official-resolved".to_string());
        }
        Some(false) => {
            tags.insert("official-unresolved".to_string());
        }
        None => {
            tags.insert("official-not-run".to_string());
        }
    }
    if repeated_patch {
        tags.insert("repeated-patch".to_string());
    }
    if patch_sha.is_none() {
        tags.insert("no-patch".to_string());
    }
    if let Some(code) = validation["error_code"].as_str() {
        tags.insert(format!("error-{}", safe_id(&code.to_ascii_lowercase())));
        let lower = code.to_ascii_lowercase();
        for (needle, tag) in [
            ("import", "import-error"),
            ("compile", "compile-error"),
            ("syntax", "syntax-error"),
            ("timeout", "timeout"),
            ("docker", "docker-error"),
            ("patch", "patch-error"),
            ("schema", "schema-error"),
            ("ground", "tool-grounding-error"),
        ] {
            if lower.contains(needle) {
                tags.insert(tag.to_string());
            }
        }
    }
    tags.into_iter().collect()
}

fn attempt_critique(
    validation: &Value,
    resolved: Option<bool>,
    repeated_patch: bool,
    patch_sha: Option<&str>,
) -> String {
    let status = validation["status"].as_str().unwrap_or("unknown");
    let error = validation["error_code"].as_str().unwrap_or("none");
    let message = validation["message"].as_str().unwrap_or("");
    let official = match resolved {
        Some(true) => "official resolved",
        Some(false) => "official unresolved",
        None => "official evaluation not run",
    };
    let patch_state = if repeated_patch {
        "repeated patch"
    } else if patch_sha.is_some() {
        "new candidate patch"
    } else {
        "no candidate patch"
    };
    truncate_chars(
        &format!(
            "validation_status={status}; error_code={error}; {official}; {patch_state}; message={message}"
        ),
        1200,
    )
}

fn task_summary_for_reflexion(task: &TaskManifest) -> String {
    truncate_chars(
        &format!(
            "{} repo={} base_commit={} problem={}",
            task.instance_id, task.repo, task.base_commit, task.problem_statement
        ),
        1200,
    )
}

fn model_output_excerpt(model_response: &Value) -> Option<String> {
    for path in [
        "/final_answer",
        "/message",
        "/response",
        "/raw_response",
        "/assistant_response",
    ] {
        if let Some(text) = model_response.pointer(path).and_then(Value::as_str) {
            return Some(truncate_chars(text, 1200));
        }
    }
    serde_json::to_string(model_response)
        .ok()
        .map(|text| truncate_chars(&text, 1200))
}

fn attempt_record_reflexion_episode(record: &AttemptRecord) -> Value {
    json!({
        "task_id": record.task_id.clone(),
        "task_summary": record.task_summary.clone(),
        "input_excerpt": record.input_excerpt.clone(),
        "output_excerpt": record.output_excerpt.clone(),
        "critique": record.critique.clone(),
        "reward": record.reward,
        "success": record.success,
        "outcome_kind": record.outcome_kind.clone(),
        "latency_ms": record.latency_ms,
        "tokens_used": record.tokens_used,
        "tags": record.tags.clone(),
    })
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
    let dynamicjepa_intervention_hint = record
        .reality_signal_compact
        .pointer("/dynamicjepa_reality/intervention_hint")
        .cloned()
        .unwrap_or(Value::Null);

    // attempts.jsonl — compact per-attempt feed
    let attempts_path = dir.join("attempts.jsonl");
    let attempt_row = json!({
        "schema_version": 1,
        "record_kind": "ccreality_autoresearch_attempt",
        "model": model,
        "instance_id": task.instance_id,
        "repo": task.repo,
        "attempt": record.attempt,
        "task_id": record.task_id.clone(),
        "task_summary": record.task_summary.clone(),
        "input_excerpt": record.input_excerpt.clone(),
        "output_excerpt": record.output_excerpt.clone(),
        "critique": record.critique.clone(),
        "reward": record.reward,
        "success": record.success,
        "outcome_kind": record.outcome_kind.clone(),
        "latency_ms": record.latency_ms,
        "tokens_used": record.tokens_used,
        "tags": record.tags.clone(),
        "status": record.status,
        "error_code": record.error_code,
        "patch_sha256": record.patch_sha256,
        "repeated_patch": record.repeated_patch,
        "official_resolved": resolved,
        "reward_scalar": reward_scalar,
        "dynamicjepa_intervention_hint": dynamicjepa_intervention_hint,
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
    if let Some(surface) = record
        .reality_signal_compact
        .pointer("/dynamicjepa_reality/intervention_hint/intervention_surface")
        .and_then(Value::as_str)
    {
        let reason = record
            .reality_signal_compact
            .pointer("/dynamicjepa_reality/intervention_hint/reason")
            .and_then(Value::as_str)
            .unwrap_or("DynamicJEPA intervention hint present");
        match surface {
            "inner_model" | "dynamicjepa_artifact_training" => {
                return AutoresearchDecision {
                    decision: "escalate",
                    reasoning: format!("DynamicJEPA intervention surface={surface}: {reason}"),
                };
            }
            "tool_surface" | "tool_guard_surface" | "prompt_template" => {
                return AutoresearchDecision {
                    decision: "repair",
                    reasoning: format!("DynamicJEPA intervention surface={surface}: {reason}"),
                };
            }
            _ => {}
        }
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
        let (compute_processes_raw, compute_processes) = read_gpu_compute_processes(parts[2])?;
        return Ok(GpuState {
            index,
            name: parts[1].to_string(),
            uuid: parts[2].to_string(),
            memory_total_mb: parse_u64(parts[3], "gpu_vram.memory_total_mb")?,
            memory_used_mb: parse_u64(parts[4], "gpu_vram.memory_used_mb")?,
            memory_free_mb: parse_u64(parts[5], "gpu_vram.memory_free_mb")?,
            driver_version: parts[6].to_string(),
            compute_process_count: compute_processes.len(),
            compute_processes,
            compute_processes_raw,
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

fn read_gpu_compute_processes(gpu_uuid: &str) -> Result<(String, Vec<GpuComputeProcess>)> {
    let apps = run_cmd(
        &[
            "nvidia-smi".to_string(),
            "--query-compute-apps=gpu_uuid,pid,process_name,used_memory".to_string(),
            "--format=csv,noheader,nounits".to_string(),
        ],
        Path::new("."),
        30,
    )?;
    if apps.exit_code != 0 {
        return Err(RealityError::new(
            "RUST_REALITY_GPU_COMPUTE_APPS_READ_FAILED",
            "nvidia-smi failed while reading GPU compute process state",
            "gpu_vram.compute_processes",
            "inspect nvidia-smi --help-query-compute-apps and driver process visibility before rerunning the loop",
            json!({ "process": apps, "gpu_uuid": gpu_uuid }),
            Some("nvidia-smi:query-compute-apps".to_string()),
        ));
    }
    let raw = apps.stdout.trim().to_string();
    let rows = parse_gpu_compute_process_rows(&raw, gpu_uuid)?;
    let processes = rows
        .into_iter()
        .map(|row| GpuComputeProcess {
            gpu_uuid: row.gpu_uuid,
            pid: row.pid,
            process_name: row.process_name,
            used_memory_mb: row.used_memory_mb,
            procfs: procfs_snapshot_for_pid(row.pid),
        })
        .collect();
    Ok((raw, processes))
}

fn parse_gpu_compute_process_rows(
    raw: &str,
    target_gpu_uuid: &str,
) -> Result<Vec<GpuComputeProcessRow>> {
    let mut rows = Vec::new();
    for (line_index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<_> = trimmed.split(',').map(|item| item.trim()).collect();
        if parts.len() != 4 {
            return Err(RealityError::new(
                "RUST_REALITY_GPU_COMPUTE_APPS_PARSE_FAILED",
                "nvidia-smi compute-apps row did not match expected CSV shape",
                "gpu_vram.compute_processes",
                "query compute apps with gpu_uuid,pid,process_name,used_memory and inspect the raw row",
                json!({
                    "line_index": line_index,
                    "line": trimmed,
                    "expected_fields": ["gpu_uuid", "pid", "process_name", "used_memory"],
                    "actual_field_count": parts.len()
                }),
                Some("nvidia-smi:query-compute-apps".to_string()),
            ));
        }
        if parts[0] != target_gpu_uuid {
            continue;
        }
        let pid = parts[1].parse::<u64>().map_err(|err| {
            RealityError::new(
                "RUST_REALITY_GPU_COMPUTE_APPS_PID_PARSE_FAILED",
                "nvidia-smi compute-apps PID was not numeric",
                "gpu_vram.compute_processes.pid",
                "inspect nvidia-smi compute-apps output before using the GPU state as evidence",
                json!({ "line_index": line_index, "line": trimmed, "value": parts[1], "error": err.to_string() }),
                Some("nvidia-smi:query-compute-apps".to_string()),
            )
        })?;
        // nvidia-smi sentinel: when the driver cannot introspect a process's
        // VRAM usage (common in WSL2 and for processes without proper introspection
        // access), the used_memory field is "[N/A]" instead of an integer. This is
        // not a parse failure — it means the row's memory contribution is unknown.
        // Skip the row so the GpuState still reports a usable snapshot of the rows
        // the driver could quantify, rather than failing the entire attempt.
        if parts[3] == "[N/A]" {
            continue;
        }
        let used_memory_mb = parts[3].parse::<u64>().map_err(|err| {
            RealityError::new(
                "RUST_REALITY_GPU_COMPUTE_APPS_MEMORY_PARSE_FAILED",
                "nvidia-smi compute-apps used_memory was not numeric",
                "gpu_vram.compute_processes.used_memory_mb",
                "inspect nvidia-smi compute-apps output before using the GPU state as evidence",
                json!({ "line_index": line_index, "line": trimmed, "value": parts[3], "error": err.to_string() }),
                Some("nvidia-smi:query-compute-apps".to_string()),
            )
        })?;
        rows.push(GpuComputeProcessRow {
            gpu_uuid: parts[0].to_string(),
            pid,
            process_name: parts[2].to_string(),
            used_memory_mb,
        });
    }
    Ok(rows)
}

fn procfs_snapshot_for_pid(pid: u64) -> GpuProcfsSnapshot {
    let proc_root = PathBuf::from(format!("/proc/{pid}"));
    let mut errors = Vec::new();
    let mut source_of_truth = Vec::new();

    if !proc_root.exists() {
        return GpuProcfsSnapshot {
            status: "exited_before_procfs_read".to_string(),
            uid: None,
            comm: None,
            cmdline: None,
            exe: None,
            errors: vec![format!(
                "{} did not exist at read time",
                path_str(&proc_root)
            )],
            source_of_truth: vec![file_sot(&proc_root)],
        };
    }

    let status_path = proc_root.join("status");
    source_of_truth.push(file_sot(&status_path));
    let uid = match fs::read_to_string(&status_path) {
        Ok(text) => parse_proc_status_uid(&text).or_else(|| {
            errors.push(format!("Uid field missing in {}", path_str(&status_path)));
            None
        }),
        Err(err) => {
            errors.push(format!("{}: {}", path_str(&status_path), err));
            None
        }
    };

    let comm_path = proc_root.join("comm");
    source_of_truth.push(file_sot(&comm_path));
    let comm = match fs::read_to_string(&comm_path) {
        Ok(text) => Some(text.trim().to_string()).filter(|value| !value.is_empty()),
        Err(err) => {
            errors.push(format!("{}: {}", path_str(&comm_path), err));
            None
        }
    };

    let cmdline_path = proc_root.join("cmdline");
    source_of_truth.push(file_sot(&cmdline_path));
    let cmdline = match fs::read(&cmdline_path) {
        Ok(bytes) => {
            let text = bytes
                .split(|byte| *byte == 0)
                .filter_map(|part| std::str::from_utf8(part).ok())
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            Some(text).filter(|value| !value.is_empty())
        }
        Err(err) => {
            errors.push(format!("{}: {}", path_str(&cmdline_path), err));
            None
        }
    };

    let exe_path = proc_root.join("exe");
    source_of_truth.push(file_sot(&exe_path));
    let exe = match fs::read_link(&exe_path) {
        Ok(path) => Some(path_str(&path).to_string()).filter(|value| !value.is_empty()),
        Err(err) => {
            errors.push(format!("{}: {}", path_str(&exe_path), err));
            None
        }
    };

    GpuProcfsSnapshot {
        status: if errors.is_empty() {
            "ok".to_string()
        } else {
            "partial".to_string()
        },
        uid,
        comm,
        cmdline,
        exe,
        errors,
        source_of_truth,
    }
}

fn parse_proc_status_uid(status_text: &str) -> Option<u64> {
    status_text.lines().find_map(|line| {
        line.strip_prefix("Uid:")
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|value| value.parse::<u64>().ok())
    })
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

/// Streaming SHA-256 for large files (the inner model BF16 GGUF is ~15 GiB; the
/// `fs::read`-based `sha256_file` would allocate the entire payload into
/// memory). 64 KiB chunks balance syscall overhead and resident set size.
fn sha256_file_streaming(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)
        .map_err(|err| fs_error("RUST_REALITY_SHA256_OPEN_FAILED", path, err))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|err| fs_error("RUST_REALITY_SHA256_READ_FAILED", path, err))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
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

    fn add_real_rust_crate(repo: &Path) {
        write_text_checked(
            &repo.join("Cargo.toml"),
            "[package]\nname = \"dynamicjepa_semantic_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n",
        )
        .expect("cargo manifest write");
        write_text_checked(
            &repo.join("src/lib.rs"),
            "pub fn bump(x: i32) -> i32 {\n    x + 1\n}\n",
        )
        .expect("rust source write");
    }

    fn add_real_c_project(repo: &Path) {
        write_text_checked(
            &repo.join("include/math_util.h"),
            "#ifndef MATH_UTIL_H\n#define MATH_UTIL_H\n\n#define CG_SCALE(x) ((x) * 2)\n\nstruct Point {\n    int x;\n    int y;\n};\n\nint add(int left, int right);\n\n#endif\n",
        )
        .expect("C header write");
        write_text_checked(
            &repo.join("src/math_util.c"),
            "#include \"math_util.h\"\n\nstatic int helper(int value) {\n    return CG_SCALE(value);\n}\n\nint add(int left, int right) {\n    struct Point p = {left, right};\n    return helper(p.x + p.y);\n}\n",
        )
        .expect("C source write");
        let compile_commands = json!([{
            "directory": path_str(repo),
            "file": path_str(&repo.join("src/math_util.c")),
            "arguments": [
                "gcc",
                "-Iinclude",
                "-c",
                "src/math_util.c",
                "-o",
                "math_util.o"
            ]
        }]);
        write_text_checked(
            &repo.join("compile_commands.json"),
            &format!(
                "{}\n",
                serde_json::to_string_pretty(&compile_commands)
                    .expect("serialize C compile commands")
            ),
        )
        .expect("C compile_commands write");
    }

    fn add_real_cpp_project(repo: &Path) {
        write_text_checked(
            &repo.join("include/accumulator.h"),
            "#pragma once\n#include <algorithm>\n\nnamespace demo {\nclass Accumulator {\n    int total_;\npublic:\n    explicit Accumulator(int seed) : total_(seed) {}\n    int add(int value) {\n        total_ += value;\n        return std::max(total_, value);\n    }\nprivate:\n    void reset() { total_ = 0; }\n};\n}\n",
        )
        .expect("C++ header write");
        write_text_checked(
            &repo.join("src/main.cpp"),
            "#include \"accumulator.h\"\n\nint main() {\n    demo::Accumulator acc(2);\n    return acc.add(2) == 4 ? 0 : 1;\n}\n",
        )
        .expect("C++ source write");
        let compile_commands = json!([{
            "directory": path_str(repo),
            "file": path_str(&repo.join("src/main.cpp")),
            "arguments": [
                "g++",
                "-std=c++17",
                "-Iinclude",
                "-c",
                path_str(&repo.join("src/main.cpp")),
                "-o",
                "main.o"
            ]
        }]);
        write_text_checked(
            &repo.join("compile_commands.json"),
            &format!(
                "{}\n",
                serde_json::to_string_pretty(&compile_commands)
                    .expect("serialize C++ compile commands")
            ),
        )
        .expect("C++ compile_commands write");
    }

    fn java_semantic_for_source(path: &str, source: &str, needle: &str) -> AstSemanticNode {
        let mut parser = tree_sitter::Parser::new();
        let language: tree_sitter::Language = tree_sitter_java::LANGUAGE.into();
        parser.set_language(&language).expect("java parser linked");
        let tree = parser.parse(source, None).expect("java syntax tree");
        let root = tree.root_node();
        assert!(
            !root.has_error(),
            "synthetic Java fixture must parse cleanly: {source}"
        );
        let start = source.find(needle).expect("needle in Java source");
        let end = start + needle.len();
        let node = root
            .descendant_for_byte_range(start, end)
            .expect("AST node for Java needle");
        dynamicjepa_ast_semantic_node("java", node, source, path)
    }

    #[test]
    fn dynamicjepa_ast_action_semantics_parse_real_python_edit() {
        let (_temp, repo) = init_real_git_repo();
        let args = json!({
            "edits": [{
                "path": "src/example.py",
                "start_line": 2,
                "end_line": 2,
                "replace": "    return x + 2\n",
                "observed_sha256": sha256_file(&repo.join("src/example.py")).expect("sha")
            }]
        });
        let action = inferred_dynamicjepa_action_from_tool(&repo, "apply_line_window_edit", &args)
            .expect("parser-backed action semantics");
        assert_eq!(action["ast_language"], "python");
        assert_eq!(action["ast_parse_status"], "parsed");
        assert_eq!(action["ast_node_kind"], "function");
        assert_eq!(action["ast_symbol_kind"], "function");
        assert_eq!(action["ast_edit_kind"], "function_body");
        assert_eq!(action["ast_touched_node_count"], 1);
        assert_eq!(action["semantic_provider"], "compiler_lsp");
        assert_eq!(action["semantic_status"], "analyzed");
        assert_eq!(action["compiler_semantic_status"], "checked");
        assert_eq!(action["type_resolution_status"], "unsupported");
        assert_eq!(action["import_resolution_status"], "unsupported");
        assert_eq!(action["compiler_diagnostic_count"], 0);
        assert_eq!(action["test_attribution_status"], "missing");
        assert_eq!(action["attributed_test_count"], 0);
        assert_eq!(action["predicted_verifier_target"], "static_check");
        assert_eq!(action["patch_delta_kind"], "return_contract");
        assert_eq!(action["api_contract_delta"], "return_changed");
        assert_eq!(action["symbol_bucket"], "function");
        assert_eq!(action["patch_scope_bucket"], "single_symbol");
    }

    #[test]
    fn dynamicjepa_ast_action_semantics_runs_real_rust_cargo_check() {
        let (_temp, repo) = init_real_git_repo();
        add_real_rust_crate(&repo);
        let args = json!({
            "edits": [{
                "path": "src/lib.rs",
                "start_line": 2,
                "end_line": 2,
                "replace": "    x + 2\n",
                "observed_sha256": sha256_file(&repo.join("src/lib.rs")).expect("sha")
            }]
        });
        let action = inferred_dynamicjepa_action_from_tool(&repo, "apply_line_window_edit", &args)
            .expect("rust compiler-backed action semantics");
        assert_eq!(action["ast_language"], "rust");
        assert_eq!(action["semantic_provider"], "compiler_lsp");
        assert_eq!(action["compiler_semantic_status"], "checked");
        assert_eq!(action["type_resolution_status"], "resolved");
        assert_eq!(action["import_resolution_status"], "resolved");
        assert_eq!(action["compiler_diagnostic_count"], 0);
        assert_eq!(action["test_attribution_status"], "inferred_from_manifest");
        assert_eq!(action["attributed_test_count"], 1);
        assert_eq!(action["predicted_verifier_target"], "typecheck");
        assert_eq!(action["patch_delta_kind"], "api_contract");
    }

    #[test]
    fn dynamicjepa_ast_action_semantics_records_real_rust_diagnostics() {
        let (_temp, repo) = init_real_git_repo();
        add_real_rust_crate(&repo);
        let args = json!({
            "edits": [{
                "path": "src/lib.rs",
                "start_line": 2,
                "end_line": 2,
                "replace": "    \"bad\"\n",
                "observed_sha256": sha256_file(&repo.join("src/lib.rs")).expect("sha")
            }]
        });
        let action = inferred_dynamicjepa_action_from_tool(&repo, "apply_line_window_edit", &args)
            .expect("rust compiler diagnostics must be recorded");
        assert_eq!(action["ast_language"], "rust");
        assert_eq!(action["semantic_provider"], "compiler_lsp");
        assert_eq!(action["compiler_semantic_status"], "diagnostics");
        assert_eq!(action["type_resolution_status"], "unresolved");
        assert_eq!(action["predicted_verifier_target"], "typecheck");
        assert!(action["compiler_diagnostic_count"].as_i64().unwrap_or(0) > 0);
    }

    #[test]
    fn dynamicjepa_ast_action_semantics_parse_real_go_edit_with_reverse_refs() {
        let (_temp, repo) = init_real_git_repo();
        write_text_checked(
            &repo.join("src/calc.go"),
            "package main\n\nfunc add(x int) int {\n    return x + 1\n}\n",
        )
        .expect("go source write");
        write_text_checked(
            &repo.join("src/use.go"),
            "package main\n\nfunc call() int {\n    return add(2)\n}\n",
        )
        .expect("go ref source write");
        let args = json!({
            "edits": [{
                "path": "src/calc.go",
                "start_line": 4,
                "end_line": 4,
                "replace": "    return x + 2\n",
                "observed_sha256": sha256_file(&repo.join("src/calc.go")).expect("sha")
            }]
        });
        let action = inferred_dynamicjepa_action_from_tool(&repo, "apply_line_window_edit", &args)
            .expect("go parser-backed action semantics");
        assert_eq!(action["ast_language"], "go");
        assert_eq!(action["ast_parse_status"], "parsed");
        assert_eq!(action["ast_node_kind"], "function");
        assert_eq!(action["patch_delta_kind"], "return_contract");
        assert_eq!(action["dependency_blast_radius"], "local_symbol");
        assert_eq!(action["reverse_reference_count"], 1);
        if command_exists("go") {
            assert_eq!(action["semantic_provider"], "compiler_lsp");
            assert_eq!(action["compiler_semantic_status"], "checked");
        } else {
            assert_eq!(action["semantic_provider"], "parser_symbol_graph");
            assert_eq!(action["compiler_semantic_status"], "toolchain_missing");
        }
    }

    #[test]
    fn dynamicjepa_ast_action_semantics_parse_real_c_edit_with_compile_commands() {
        let (_temp, repo) = init_real_git_repo();
        add_real_c_project(&repo);
        let args = json!({
            "edits": [{
                "path": "src/math_util.c",
                "start_line": 9,
                "end_line": 9,
                "replace": "    return helper(p.x + p.y + 1);\n",
                "observed_sha256": sha256_file(&repo.join("src/math_util.c")).expect("sha")
            }]
        });
        println!(
            "BEFORE C action state: compile_commands={}, source={}",
            fs::read_to_string(repo.join("compile_commands.json"))
                .expect("read C compile commands"),
            fs::read_to_string(repo.join("src/math_util.c")).expect("read C source")
        );

        let action = inferred_dynamicjepa_action_from_tool(&repo, "apply_line_window_edit", &args)
            .expect("C compiler-backed action semantics");
        println!("AFTER C action semantic state: {action}");

        assert_eq!(action["ast_language"], "c");
        assert_eq!(action["ast_parse_status"], "parsed");
        assert_eq!(action["ast_node_kind"], "function");
        assert_eq!(action["ast_symbol_kind"], "function");
        assert_eq!(action["ast_edit_kind"], "function_body");
        assert_eq!(action["symbol_visibility"], "public");
        assert_eq!(action["semantic_provider"], "compiler_lsp");
        assert_eq!(action["semantic_status"], "analyzed");
        assert_eq!(action["compiler_semantic_status"], "checked");
        assert_eq!(action["type_resolution_status"], "resolved");
        assert_eq!(action["import_resolution_status"], "resolved");
        assert_eq!(action["compiler_diagnostic_count"], 0);
        assert_eq!(action["predicted_verifier_target"], "typecheck");
        assert_eq!(action["patch_delta_kind"], "return_contract");
        assert_eq!(
            fs::read_to_string(repo.join("src/math_util.c")).expect("read C source after"),
            "#include \"math_util.h\"\n\nstatic int helper(int value) {\n    return CG_SCALE(value);\n}\n\nint add(int left, int right) {\n    struct Point p = {left, right};\n    return helper(p.x + p.y);\n}\n"
        );
    }

    #[test]
    fn dynamicjepa_ast_action_semantics_c_fails_closed_without_compile_commands() {
        let (_temp, repo) = init_real_git_repo();
        write_text_checked(
            &repo.join("src/math_util.c"),
            "int add(int left, int right) {\n    return left + right;\n}\n",
        )
        .expect("C source write");
        let args = json!({
            "edits": [{
                "path": "src/math_util.c",
                "start_line": 2,
                "end_line": 2,
                "replace": "    return left + right + 1;\n",
                "observed_sha256": sha256_file(&repo.join("src/math_util.c")).expect("sha")
            }]
        });
        println!(
            "BEFORE C missing compile_commands state: compile_commands_exists={}, source={}",
            repo.join("compile_commands.json").exists(),
            fs::read_to_string(repo.join("src/math_util.c")).expect("read C source")
        );

        let err = inferred_dynamicjepa_action_from_tool(&repo, "apply_line_window_edit", &args)
            .expect_err("C action semantics must fail closed without compile_commands.json");
        println!("AFTER C missing compile_commands error state: {err:?}");

        assert_eq!(
            err.error_code,
            "RUST_REALITY_DYNAMICJEPA_C_COMPILE_COMMANDS_MISSING"
        );
        assert_eq!(
            err.field_path,
            "dynamicjepa.action.compiler_lsp.c.compile_commands"
        );
        assert_eq!(
            fs::read_to_string(repo.join("src/math_util.c")).expect("read C source after"),
            "int add(int left, int right) {\n    return left + right;\n}\n"
        );
    }

    #[test]
    fn dynamicjepa_ast_action_semantics_parse_real_cpp_header_with_compile_commands_fsv() {
        let (_temp, repo) = init_real_git_repo();
        add_real_cpp_project(&repo);
        let args = json!({
            "edits": [{
                "path": "include/accumulator.h",
                "start_line": 9,
                "end_line": 9,
                "replace": "    int add(int value) {\n",
                "observed_sha256": sha256_file(&repo.join("include/accumulator.h")).expect("sha")
            }]
        });
        println!(
            "BEFORE C++ action state: compile_commands={}, header={}",
            fs::read_to_string(repo.join("compile_commands.json"))
                .expect("read C++ compile commands"),
            fs::read_to_string(repo.join("include/accumulator.h")).expect("read C++ header")
        );

        let action = inferred_dynamicjepa_action_from_tool(&repo, "apply_line_window_edit", &args)
            .expect("C++ compiler-backed action semantics");
        let evidence_path = repo.join("cpp-action-fsv.json");
        let evidence = json!({
            "source_of_truth": file_sot(&evidence_path),
            "action": action,
            "header_readback": fs::read_to_string(repo.join("include/accumulator.h")).expect("read C++ header after"),
            "compile_commands_readback": fs::read_to_string(repo.join("compile_commands.json")).expect("read C++ compile_commands after"),
        });
        write_json_checked(&evidence_path, &evidence).expect("write C++ action FSV evidence");
        let readback: Value = read_json(&evidence_path).expect("read C++ action FSV evidence");
        println!(
            "AFTER C++ action semantic readback: {}",
            serde_json::to_string_pretty(&readback).expect("serialize C++ readback")
        );

        let action = readback.get("action").expect("action evidence");
        assert_eq!(action["ast_language"], "cpp");
        assert_eq!(action["ast_parse_status"], "parsed");
        assert_eq!(action["ast_node_kind"], "method");
        assert_eq!(action["ast_symbol_kind"], "method");
        assert_eq!(action["ast_edit_kind"], "signature");
        assert_eq!(action["symbol_visibility"], "public");
        assert_eq!(action["semantic_provider"], "compiler_lsp_compile_commands");
        assert_eq!(action["semantic_status"], "analyzed");
        assert_eq!(action["compiler_semantic_status"], "checked");
        assert_eq!(action["type_resolution_status"], "resolved");
        assert_eq!(action["import_resolution_status"], "resolved");
        assert_eq!(action["compiler_diagnostic_count"], 0);
        assert_eq!(action["predicted_verifier_target"], "typecheck");
        assert_eq!(action["symbol_bucket"], "function");
        assert_eq!(
            readback["header_readback"].as_str().expect("header text"),
            "#pragma once\n#include <algorithm>\n\nnamespace demo {\nclass Accumulator {\n    int total_;\npublic:\n    explicit Accumulator(int seed) : total_(seed) {}\n    int add(int value) {\n        total_ += value;\n        return std::max(total_, value);\n    }\nprivate:\n    void reset() { total_ = 0; }\n};\n}\n"
        );
    }

    #[test]
    fn dynamicjepa_ast_action_semantics_cpp_fails_closed_without_compile_commands() {
        let (_temp, repo) = init_real_git_repo();
        write_text_checked(
            &repo.join("include/accumulator.h"),
            "class Accumulator {\npublic:\n    int add(int value) { return value + 1; }\n};\n",
        )
        .expect("C++ header write");
        let args = json!({
            "edits": [{
                "path": "include/accumulator.h",
                "start_line": 3,
                "end_line": 3,
                "replace": "    int add(int value) { return value + 2; }\n",
                "observed_sha256": sha256_file(&repo.join("include/accumulator.h")).expect("sha")
            }]
        });
        println!(
            "BEFORE C++ missing compile_commands state: compile_commands_exists={}, header={}",
            repo.join("compile_commands.json").exists(),
            fs::read_to_string(repo.join("include/accumulator.h")).expect("read C++ header")
        );

        let err = inferred_dynamicjepa_action_from_tool(&repo, "apply_line_window_edit", &args)
            .expect_err("C++ action semantics must fail closed without compile_commands.json");
        println!("AFTER C++ missing compile_commands error state: {err:?}");

        assert_eq!(
            err.error_code,
            "RUST_REALITY_DYNAMICJEPA_CPP_COMPILE_COMMANDS_MISSING"
        );
        assert_eq!(
            err.field_path,
            "dynamicjepa.action.compiler_lsp.compile_commands"
        );
        assert_eq!(
            fs::read_to_string(repo.join("include/accumulator.h")).expect("read C++ header after"),
            "class Accumulator {\npublic:\n    int add(int value) { return value + 1; }\n};\n"
        );
    }

    #[test]
    fn dynamicjepa_ast_action_semantics_cpp_refuses_shell_command_compile_commands() {
        let (_temp, repo) = init_real_git_repo();
        add_real_cpp_project(&repo);
        let command_compile_commands = json!([{
            "directory": path_str(&repo),
            "file": path_str(&repo.join("src/main.cpp")),
            "command": format!(
                "g++ -std=c++17 -Iinclude -c {} -o main.o",
                path_str(&repo.join("src/main.cpp"))
            )
        }]);
        write_text_checked(
            &repo.join("compile_commands.json"),
            &format!(
                "{}\n",
                serde_json::to_string_pretty(&command_compile_commands)
                    .expect("serialize command compile_commands")
            ),
        )
        .expect("C++ shell command compile_commands write");
        let args = json!({
            "edits": [{
                "path": "include/accumulator.h",
                "start_line": 9,
                "end_line": 9,
                "replace": "    int add(int value) {\n",
                "observed_sha256": sha256_file(&repo.join("include/accumulator.h")).expect("sha")
            }]
        });
        println!(
            "BEFORE C++ shell compile_commands state: compile_commands={}, header={}",
            fs::read_to_string(repo.join("compile_commands.json"))
                .expect("read C++ shell compile commands"),
            fs::read_to_string(repo.join("include/accumulator.h")).expect("read C++ header")
        );

        let err = inferred_dynamicjepa_action_from_tool(&repo, "apply_line_window_edit", &args)
            .expect_err("C++ action semantics must refuse shell command compile_commands");
        println!(
            "AFTER C++ shell compile_commands error state: {err:?}; compile_commands={}",
            fs::read_to_string(repo.join("compile_commands.json"))
                .expect("read C++ shell compile_commands after")
        );

        assert_eq!(
            err.error_code,
            "RUST_REALITY_DYNAMICJEPA_CPP_COMPILE_COMMANDS_SHELL_COMMAND_REFUSED"
        );
        assert_eq!(
            err.field_path,
            "dynamicjepa.action.compiler_lsp.compile_commands[].command"
        );
        assert_eq!(
            fs::read_to_string(repo.join("include/accumulator.h")).expect("read C++ header after"),
            "#pragma once\n#include <algorithm>\n\nnamespace demo {\nclass Accumulator {\n    int total_;\npublic:\n    explicit Accumulator(int seed) : total_(seed) {}\n    int add(int value) {\n        total_ += value;\n        return std::max(total_, value);\n    }\nprivate:\n    void reset() { total_ = 0; }\n};\n}\n"
        );
    }

    #[test]
    fn dynamicjepa_ast_action_semantics_cpp_fails_closed_on_parser_errors() {
        let (_temp, repo) = init_real_git_repo();
        write_text_checked(
            &repo.join("include/broken.h"),
            "class Broken {\npublic:\n    int add(\n};\n",
        )
        .expect("broken C++ header write");
        let args = json!({
            "edits": [{
                "path": "include/broken.h",
                "start_line": 3,
                "end_line": 3,
                "replace": "    int add(\n",
                "observed_sha256": sha256_file(&repo.join("include/broken.h")).expect("sha")
            }]
        });
        println!(
            "BEFORE C++ parse-error state: header={}",
            fs::read_to_string(repo.join("include/broken.h")).expect("read broken C++ header")
        );

        let err = inferred_dynamicjepa_action_from_tool(&repo, "apply_line_window_edit", &args)
            .expect_err("C++ parser errors must fail before compiler semantic extraction");
        println!("AFTER C++ parse-error state: {err:?}");

        assert_eq!(
            err.error_code,
            "RUST_REALITY_DYNAMICJEPA_AST_PARSE_SYNTAX_ERROR"
        );
        assert_eq!(err.field_path, "dynamicjepa.action.ast_parse_status");
    }

    #[test]
    fn dynamicjepa_java_ast_recognizes_standard_test_files_and_annotations() {
        let surefire_source = "package com.acme;\n\nimport org.junit.jupiter.api.Test;\n\nclass CalculatorTests {\n    @Test\n    void adds() {\n        assertEquals(4, 2 + 2);\n    }\n}\n";
        println!(
            "BEFORE java surefire state: path_is_test={}, source_len={}",
            path_is_test("src/test/java/com/acme/CalculatorTests.java"),
            surefire_source.len()
        );
        let surefire = java_semantic_for_source(
            "src/test/java/com/acme/CalculatorTests.java",
            surefire_source,
            "void adds",
        );
        println!("AFTER java surefire semantic state: {:?}", surefire);
        assert_eq!(surefire.node_kind, "test");
        assert_eq!(surefire.symbol_kind, "test_case");
        assert_eq!(surefire.symbol_name.as_deref(), Some("adds"));

        let nonstandard_source = "package com.acme;\n\nclass SmokeCheck {\n    @org.junit.jupiter.params.ParameterizedTest(name = \"case {0}\")\n    void computes() {\n        verifyReality();\n    }\n}\n";
        println!(
            "BEFORE java annotation state: path_is_test={}, source_len={}",
            path_is_test("src/main/java/com/acme/SmokeCheck.java"),
            nonstandard_source.len()
        );
        let annotated = java_semantic_for_source(
            "src/main/java/com/acme/SmokeCheck.java",
            nonstandard_source,
            "void computes",
        );
        println!("AFTER java annotation semantic state: {:?}", annotated);
        assert_eq!(annotated.node_kind, "test");
        assert_eq!(annotated.symbol_kind, "test_case");
        assert_eq!(annotated.symbol_name.as_deref(), Some("computes"));

        for path in [
            "src/test/java/com/acme/Utility.java",
            "src/test/java/com/acme/TestCalculator.java",
            "src/test/java/com/acme/CalculatorTest.java",
            "src/test/java/com/acme/CalculatorTests.java",
            "src/test/java/com/acme/CalculatorTestCase.java",
            "src/it/java/com/acme/ITCalculator.java",
            "src/it/java/com/acme/CalculatorIT.java",
            "src/it/java/com/acme/CalculatorITCase.java",
        ] {
            assert!(path_is_test(path), "standard Java test path missed: {path}");
        }
        assert!(
            !path_is_test("src/main/java/com/acme/Calculator.java"),
            "production Java source must not be classified as a test"
        );
    }

    #[test]
    fn dynamicjepa_java_action_semantics_require_real_javac_or_record_checked_state() {
        let (_temp, repo) = init_real_git_repo();
        let java_source = "package com.acme;\n\npublic class Calc {\n    public int inc(int x) {\n        return x + 1;\n    }\n}\n";
        let expected_after_source = "package com.acme;\n\npublic class Calc {\n    public int inc(int x) {\n        return x + 2;\n    }\n}\n";
        write_text_checked(&repo.join("src/main/java/com/acme/Calc.java"), java_source)
            .expect("java source write");
        let args = json!({
            "edits": [{
                "path": "src/main/java/com/acme/Calc.java",
                "start_line": 5,
                "end_line": 5,
                "replace": "        return x + 2;\n",
                "observed_sha256": sha256_file(&repo.join("src/main/java/com/acme/Calc.java")).expect("sha")
            }]
        });
        println!(
            "BEFORE java action state: javac_present={}, source={}",
            command_exists("javac"),
            fs::read_to_string(repo.join("src/main/java/com/acme/Calc.java"))
                .expect("read Java source")
        );
        if command_exists("javac") {
            let action =
                inferred_dynamicjepa_action_from_tool(&repo, "apply_line_window_edit", &args)
                    .expect("java compiler-backed action semantics");
            println!("AFTER java action checked state: {action}");
            let evidence_path = repo.join("java-action-fsv.json");
            write_json_checked(
                &evidence_path,
                &json!({
                    "source_of_truth": file_sot(&evidence_path),
                    "action": action,
                    "repo_source_readback": fs::read_to_string(repo.join("src/main/java/com/acme/Calc.java"))
                        .expect("read Java repo source after"),
                }),
            )
            .expect("write Java action FSV evidence");
            let readback: Value = read_json(&evidence_path).expect("read Java action FSV evidence");
            println!(
                "READBACK java action checked state: {}",
                serde_json::to_string_pretty(&readback).expect("serialize Java action readback")
            );
            assert_eq!(action["ast_language"], "java");
            assert_eq!(action["ast_parse_status"], "parsed");
            assert_eq!(action["ast_node_kind"], "method");
            assert_eq!(action["semantic_provider"], "compiler_lsp");
            assert_eq!(action["compiler_semantic_status"], "checked");
            assert_eq!(action["predicted_verifier_target"], "typecheck");
        } else {
            let err = inferred_dynamicjepa_action_from_tool(&repo, "apply_line_window_edit", &args)
                .expect_err("missing javac must fail closed for Java action semantics");
            println!("AFTER java action missing-toolchain state: {err:?}");
            assert_eq!(
                err.error_code,
                "RUST_REALITY_DYNAMICJEPA_JAVA_TOOLCHAIN_MISSING"
            );
            assert_eq!(
                err.field_path,
                "dynamicjepa.action.compiler_lsp.java.toolchain"
            );
            let source_of_truth = err
                .source_of_truth
                .as_deref()
                .expect("Java missing-toolchain error must name source-of-truth file");
            let snapshot_path = source_of_truth
                .strip_prefix("file:")
                .map(PathBuf::from)
                .expect("Java missing-toolchain source-of-truth must be a local file");
            let snapshot_readback =
                fs::read_to_string(&snapshot_path).expect("read Java snapshot source-of-truth");
            println!(
                "READBACK java missing-toolchain source-of-truth: path={} source={}",
                snapshot_path.display(),
                snapshot_readback
            );
            assert_eq!(snapshot_readback, expected_after_source);
            assert_eq!(
                fs::read_to_string(repo.join("src/main/java/com/acme/Calc.java"))
                    .expect("read Java repo source after failed action"),
                java_source,
                "candidate compiler snapshot must not mutate the source repo"
            );
        }
    }

    #[test]
    fn dynamicjepa_ast_action_semantics_fail_closed_for_unsupported_source() {
        let (_temp, repo) = init_real_git_repo();
        write_text_checked(&repo.join("src/main.swift"), "func main() {}\n")
            .expect("swift source write");
        let args = json!({
            "edits": [{
                "path": "src/main.swift",
                "start_line": 1,
                "end_line": 1,
                "replace": "func main() { print(\"x\") }\n",
                "observed_sha256": sha256_file(&repo.join("src/main.swift")).expect("sha")
            }]
        });
        let err = inferred_dynamicjepa_action_from_tool(&repo, "apply_line_window_edit", &args)
            .expect_err("unsupported source language must fail closed");
        assert_eq!(
            err.error_code,
            "RUST_REALITY_DYNAMICJEPA_AST_UNSUPPORTED_SOURCE_LANGUAGE"
        );
        assert_eq!(err.field_path, "dynamicjepa.action.ast_language");
    }

    #[test]
    fn dynamicjepa_ast_action_semantics_fail_closed_for_nonunique_structured_span() {
        let (_temp, repo) = init_real_git_repo();
        write_text_checked(
            &repo.join("src/dupe.py"),
            "def a():\n    return 1\n\ndef b():\n    return 1\n",
        )
        .expect("dupe source write");
        let args = json!({
            "edits": [{
                "path": "src/dupe.py",
                "find": "    return 1\n",
                "replace": "    return 2\n"
            }]
        });
        let err = inferred_dynamicjepa_action_from_tool(&repo, "apply_structured_edit", &args)
            .expect_err("non-unique structured find span must fail closed");
        assert_eq!(
            err.error_code,
            "RUST_REALITY_DYNAMICJEPA_AST_FIND_SPAN_NOT_UNIQUE"
        );
        assert_eq!(err.field_path, "dynamicjepa.action.ast.structured.find");
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
    fn structured_edit_and_unified_diff_emit_post_edit_window_fsv() {
        let (_temp, repo) = init_real_git_repo();
        let source = repo.join("src/example.py");

        // Structured edit success path: post_edit_window must surface in applied_edits[0].
        let structured_dir = repo
            .parent()
            .expect("repo parent")
            .join("attempt-000001")
            .join("agent-tools")
            .join("round-01-00-apply_structured_edit_post_edit_window");
        create_dir_checked(&structured_dir).expect("structured tool dir");
        let edits = vec![json!({
            "path": "src/example.py",
            "find": "def bump(x):\n    return x + 1\n",
            "replace": "def bump(x):\n    return x + 5\n"
        })];
        let (patch, compilation) = compile_structured_edits(&repo, &structured_dir, &edits, 30)
            .expect("compile structured edits");
        assert!(
            patch.is_some(),
            "structured edit compilation must produce a patch: {}",
            serde_json::to_string_pretty(&compilation).unwrap()
        );
        let window = compilation
            .pointer("/applied_edits/0/post_edit_window")
            .expect("post_edit_window present in structured-edit applied_edits[0]");
        let lines = window
            .pointer("/lines")
            .and_then(|v| v.as_array())
            .expect("post_edit_window.lines is an array");
        let saw_replacement = lines.iter().any(|line| {
            line.pointer("/text")
                .and_then(|v| v.as_str())
                .map(|t| t.contains("return x + 5"))
                .unwrap_or(false)
        });
        assert!(
            saw_replacement,
            "structured-edit post_edit_window must contain the replacement text; window={}",
            serde_json::to_string_pretty(window).unwrap()
        );

        // Reset workspace before running the unified-diff path.
        run_required(
            &["git", "reset", "--hard", "HEAD"],
            &repo,
            30,
            "TEST_GIT_RESET_FAILED",
            "test.git.reset",
        )
        .expect("git reset");

        // Unified diff success path: post_edit_window must surface per changed file.
        let unified_dir = repo
            .parent()
            .expect("repo parent")
            .join("attempt-000001")
            .join("agent-tools")
            .join("round-01-00-apply_unified_diff_post_edit_window");
        create_dir_checked(&unified_dir).expect("unified tool dir");
        let observed_sha256 = sha256_file(&source).expect("source sha");
        let patch_text = "\
diff --git a/src/example.py b/src/example.py
--- a/src/example.py
+++ b/src/example.py
@@ -1,2 +1,2 @@
 def bump(x):
-    return x + 1
+    return x + 7
";
        let observed_files =
            vec![json!({"path": "src/example.py", "observed_sha256": observed_sha256})];
        let (patch, compilation) =
            compile_unified_diff_submission(&repo, &unified_dir, patch_text, &observed_files, 30)
                .expect("compile unified diff");
        assert_eq!(compilation["status"], "passed");
        assert!(patch.is_some());
        let window = compilation
            .pointer("/applied_edits/0/post_edit_window")
            .expect("post_edit_window present in unified-diff applied_edits[0]");
        let lines = window
            .pointer("/lines")
            .and_then(|v| v.as_array())
            .expect("unified-diff post_edit_window.lines is an array");
        let saw_replacement = lines.iter().any(|line| {
            line.pointer("/text")
                .and_then(|v| v.as_str())
                .map(|t| t.contains("return x + 7"))
                .unwrap_or(false)
        });
        assert!(
            saw_replacement,
            "unified-diff post_edit_window must contain the replacement text; window={}",
            serde_json::to_string_pretty(window).unwrap()
        );
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
    fn line_window_edit_emits_post_edit_window_on_success_and_py_compile_failure_fsv() {
        // Build a repo with a multi-line python file so we can exercise context expansion.
        let (_temp, repo) = init_real_git_repo();
        let multi = "def a():\n    return 1\n\ndef b():\n    return 2\n\ndef c():\n    return 3\n\ndef d():\n    return 4\n\ndef e():\n    return 5\n";
        write_text_checked(&repo.join("src/example.py"), multi).expect("write multi");
        run_required(
            &["git", "add", "src/example.py"],
            &repo,
            30,
            "TEST_GIT_ADD_FAILED",
            "test.git.add",
        )
        .expect("git add multi");
        run_required(
            &["git", "commit", "-m", "multi"],
            &repo,
            30,
            "TEST_GIT_COMMIT_FAILED",
            "test.git.commit.multi",
        )
        .expect("git commit multi");
        let source = repo.join("src/example.py");
        let observed_sha256 = sha256_file(&source).expect("sha");

        // Success path: edit line 8 (`    return 3`) -> `    return 33`.
        let tool_dir_ok = repo
            .parent()
            .expect("repo parent")
            .join("attempt-000001")
            .join("agent-tools")
            .join("round-01-00-apply_line_window_edit-ok");
        create_dir_checked(&tool_dir_ok).expect("tool dir ok");
        let edits_ok = vec![json!({
            "path": "src/example.py",
            "start_line": 8,
            "end_line": 8,
            "replace": "    return 33\n",
            "observed_sha256": observed_sha256.clone(),
        })];
        let (patch_ok, compilation_ok) =
            compile_line_window_edits(&repo, &tool_dir_ok, &edits_ok, 30).expect("compile ok");
        assert!(patch_ok.is_some(), "success path must produce a patch");
        let window_ok = compilation_ok
            .pointer("/applied_edits/0/post_edit_window")
            .expect("post_edit_window present on success");
        assert_eq!(window_ok["path"], "src/example.py");
        assert_eq!(window_ok["start_line"], 3);
        assert_eq!(window_ok["end_line"], 13);
        let lines_ok = window_ok["lines"].as_array().expect("lines array");
        assert!(!lines_ok.is_empty());
        let edited_row = lines_ok
            .iter()
            .find(|row| row["n"].as_u64() == Some(8))
            .expect("line 8 in window");
        assert_eq!(edited_row["text"].as_str(), Some("    return 33"));
        // Restore for the failure path by overwriting from in-memory truth.
        write_text_checked(&source, multi).expect("restore source");

        // Failure path: edit line 8 with broken Python so the syntax probe rejects the patch.
        let observed_after_restore = sha256_file(&source).expect("sha after restore");
        let tool_dir_fail = repo
            .parent()
            .expect("repo parent")
            .join("attempt-000001")
            .join("agent-tools")
            .join("round-01-00-apply_line_window_edit-fail");
        create_dir_checked(&tool_dir_fail).expect("tool dir fail");
        let edits_fail = vec![json!({
            "path": "src/example.py",
            "start_line": 8,
            "end_line": 8,
            "replace": "    return \"\"\"oops\n",
            "observed_sha256": observed_after_restore,
        })];
        let (patch_fail, compilation_fail) =
            compile_line_window_edits(&repo, &tool_dir_fail, &edits_fail, 30)
                .expect("compile fail");
        assert!(patch_fail.is_none(), "failure path must not return a patch");
        assert_eq!(
            compilation_fail["error_code"],
            "RUST_REALITY_LINE_EDIT_PY_COMPILE_FAILED"
        );
        let window_fail = compilation_fail
            .pointer("/details/applied_edits/0/post_edit_window")
            .expect("post_edit_window present on PY_COMPILE_FAILED");
        assert_eq!(window_fail["path"], "src/example.py");
        let lines_fail = window_fail["lines"].as_array().expect("lines array");
        let broken_row = lines_fail
            .iter()
            .find(|row| row["n"].as_u64() == Some(8))
            .expect("broken line 8 in window");
        assert_eq!(
            broken_row["text"].as_str(),
            Some("    return \"\"\"oops"),
            "post_edit_window must echo the broken text so the model can self-diagnose"
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
            problem_statement: "Bug in src/nonexistent_module.py with the foo function."
                .to_string(),
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
                "The bug is in pkg/zeta.py and also affects pkg/alpha.py downstream.".to_string(),
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
        // FSV: the compact packet (what reaches the inner model's prompt) MUST surface the
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
            "compact packet must surface mentioned_paths to the inner model's prompt"
        );
    }

    /// Resolves to the contextgraph project root from the test process so the
    /// pin/model paths can use real on-disk artifacts regardless of cargo's
    /// current_dir at test time.
    fn fsv_project_root() -> PathBuf {
        // CARGO_MANIFEST_DIR for this crate = .../crates/context-graph-cli
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .expect("contextgraph project root above crates/context-graph-cli")
    }

    fn real_pin_path() -> PathBuf {
        fsv_project_root().join(DEFAULT_INNER_MODEL_PIN_PATH)
    }

    fn real_model_path() -> PathBuf {
        fsv_project_root().join(DEFAULT_INNER_MODEL_PATH)
    }

    #[test]
    fn load_model_pin_resolves_real_pin_entry_fsv() {
        let pin_path = real_pin_path();
        assert!(
            pin_path.exists(),
            "config/ccreality_model_pins.json missing; pin gate cannot be FSV'd: {:?}",
            pin_path
        );
        eprintln!(
            "BEFORE_FSV pin_path={:?} key={}",
            pin_path, DEFAULT_INNER_MODEL_PIN_KEY
        );
        let pin = load_model_pin(&pin_path, DEFAULT_INNER_MODEL_PIN_KEY)
            .expect("pin entry must resolve for the default key");
        eprintln!(
            "AFTER_FSV pin.expected_sha256={} expected_size_bytes={} expected_min_context_tokens={}",
            pin.expected_sha256, pin.expected_size_bytes, pin.expected_min_context_tokens
        );
        assert_eq!(
            pin.repo_id, "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF",
            "pin must reference the official unsloth repository"
        );
        assert_eq!(pin.filename, "Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf");
        assert!(
            pin.expected_sha256.starts_with("sha256:"),
            "expected_sha256 must be in `sha256:<hex>` form, got {}",
            pin.expected_sha256
        );
        assert!(
            pin.expected_size_bytes > 17_000_000_000,
            "Q4_K_XL GGUF should be ~17.67 GB; got {}",
            pin.expected_size_bytes
        );
        assert_eq!(pin.expected_min_context_tokens, 128_000);
        assert!(
            pin.expected_identity_substrings
                .iter()
                .any(|s| s.eq_ignore_ascii_case("qwen3")),
            "pin must require qwen3 substring; got {:?}",
            pin.expected_identity_substrings
        );
    }

    #[test]
    fn load_model_pin_fails_loud_on_missing_pin_file_fsv() {
        let temp = tempfile::tempdir().expect("tempdir");
        let bogus = temp.path().join("nonexistent.json");
        eprintln!("BEFORE_FSV pin_path={:?} (does NOT exist by design)", bogus);
        let err = load_model_pin(&bogus, DEFAULT_INNER_MODEL_PIN_KEY)
            .expect_err("missing pin file must fail loud");
        eprintln!(
            "AFTER_FSV error_code={} field_path={}",
            err.error_code, err.field_path
        );
        assert_eq!(
            err.error_code,
            "RUST_REALITY_INNER_MODEL_PIN_CONFIG_MISSING"
        );
    }

    #[test]
    fn load_model_pin_fails_loud_on_unknown_key_fsv() {
        let pin_path = real_pin_path();
        eprintln!(
            "BEFORE_FSV pin_path={:?} key=this_key_does_not_exist",
            pin_path
        );
        let err = load_model_pin(&pin_path, "this_key_does_not_exist")
            .expect_err("unknown key must fail loud");
        eprintln!("AFTER_FSV error_code={}", err.error_code);
        assert_eq!(err.error_code, "RUST_REALITY_INNER_MODEL_PIN_NOT_FOUND");
    }

    #[test]
    fn load_model_pin_fails_loud_on_invalid_json_fsv() {
        let temp = tempfile::tempdir().expect("tempdir");
        let bad = temp.path().join("bad.json");
        fs::write(&bad, "{ this is not json").expect("write bad json");
        eprintln!("BEFORE_FSV bad_pin={:?} content=invalid", bad);
        let err = load_model_pin(&bad, DEFAULT_INNER_MODEL_PIN_KEY)
            .expect_err("invalid JSON must fail loud");
        eprintln!("AFTER_FSV error_code={}", err.error_code);
        assert_eq!(
            err.error_code,
            "RUST_REALITY_INNER_MODEL_PIN_CONFIG_INVALID_JSON"
        );
    }

    #[test]
    fn load_model_pin_fails_loud_on_schema_invalid_fsv() {
        let temp = tempfile::tempdir().expect("tempdir");
        let pin_file = temp.path().join("schema_bad.json");
        // Missing required fields like expected_sha256 and expected_size_bytes.
        let body = json!({
            "$schema_version": "1",
            "models": {
                "qwen3_coder_30b_a3b_q4_k_xl": {
                    "repo_id": "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF",
                    "filename": "Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf"
                }
            }
        });
        fs::write(&pin_file, serde_json::to_string(&body).expect("ser"))
            .expect("write schema-bad pin");
        eprintln!(
            "BEFORE_FSV schema_bad_pin={:?} (missing required fields)",
            pin_file
        );
        let err = load_model_pin(&pin_file, DEFAULT_INNER_MODEL_PIN_KEY)
            .expect_err("incomplete schema must fail loud");
        eprintln!("AFTER_FSV error_code={}", err.error_code);
        assert_eq!(
            err.error_code,
            "RUST_REALITY_INNER_MODEL_PIN_SCHEMA_INVALID"
        );
    }

    #[test]
    fn validate_inner_model_pin_rejects_size_drift_fsv() {
        let pin_path = real_pin_path();
        let pin = load_model_pin(&pin_path, DEFAULT_INNER_MODEL_PIN_KEY)
            .expect("pin must load for happy-path baseline");
        // Synthesize a pin with the wrong expected size — the file is real,
        // the pin is wrong. validate_inner_model_pin must reject at the size gate
        // before paying for SHA.
        let tampered = InnerModelPin {
            repo_id: pin.repo_id.clone(),
            filename: pin.filename.clone(),
            expected_sha256: pin.expected_sha256.clone(),
            expected_size_bytes: pin.expected_size_bytes + 1, // +1 byte == size mismatch
            expected_min_context_tokens: pin.expected_min_context_tokens,
            expected_identity_substrings: pin.expected_identity_substrings.clone(),
        };
        let model_path = real_model_path();
        if !model_path.exists() {
            eprintln!("SKIP: real BF16 GGUF not on disk at {:?}", model_path);
            return;
        }
        eprintln!(
            "BEFORE_FSV model_path={:?} actual_size={} pinned_size={}",
            model_path,
            fs::metadata(&model_path).map(|m| m.len()).unwrap_or(0),
            tampered.expected_size_bytes
        );
        let err = validate_inner_model_pin(
            &model_path,
            &tampered,
            &pin_path,
            DEFAULT_INNER_MODEL_PIN_KEY,
        )
        .expect_err("size drift must fail loud");
        eprintln!("AFTER_FSV error_code={}", err.error_code);
        assert_eq!(err.error_code, "RUST_REALITY_INNER_MODEL_SIZE_MISMATCH");
    }

    #[test]
    fn validate_inner_model_pin_rejects_sha_drift_fsv() {
        let pin_path = real_pin_path();
        let pin = load_model_pin(&pin_path, DEFAULT_INNER_MODEL_PIN_KEY)
            .expect("pin must load for happy-path baseline");
        // Same size, wrong SHA → must reject at SHA gate.
        let tampered = InnerModelPin {
            repo_id: pin.repo_id.clone(),
            filename: pin.filename.clone(),
            expected_sha256:
                "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
            expected_size_bytes: pin.expected_size_bytes,
            expected_min_context_tokens: pin.expected_min_context_tokens,
            expected_identity_substrings: pin.expected_identity_substrings.clone(),
        };
        let model_path = real_model_path();
        if !model_path.exists() {
            eprintln!("SKIP: real BF16 GGUF not on disk at {:?}", model_path);
            return;
        }
        eprintln!(
            "BEFORE_FSV model_path={:?} pinned_sha=zeros actual_sha=(real)",
            model_path
        );
        let err = validate_inner_model_pin(
            &model_path,
            &tampered,
            &pin_path,
            DEFAULT_INNER_MODEL_PIN_KEY,
        )
        .expect_err("sha drift must fail loud");
        eprintln!("AFTER_FSV error_code={}", err.error_code);
        assert_eq!(err.error_code, "RUST_REALITY_INNER_MODEL_SHA_MISMATCH");
    }

    #[test]
    fn validate_inner_model_pin_passes_with_real_bf16_gguf_fsv() {
        let pin_path = real_pin_path();
        let pin = load_model_pin(&pin_path, DEFAULT_INNER_MODEL_PIN_KEY).expect("pin must load");
        let model_path = real_model_path();
        if !model_path.exists() {
            eprintln!("SKIP: real BF16 GGUF not on disk at {:?}", model_path);
            return;
        }
        eprintln!(
            "BEFORE_FSV model_path={:?} pin.expected_sha256={}",
            model_path, pin.expected_sha256
        );
        validate_inner_model_pin(&model_path, &pin, &pin_path, DEFAULT_INNER_MODEL_PIN_KEY)
            .expect("real model file MUST satisfy the pin");
        let actual_sha = sha256_file_streaming(&model_path).expect("stream sha");
        eprintln!(
            "AFTER_FSV streaming_sha256={} matches_pin={}",
            actual_sha,
            actual_sha == pin.expected_sha256
        );
        assert_eq!(actual_sha, pin.expected_sha256);
    }

    #[test]
    fn validate_inner_model_pin_identity_substrings_rejects_missing_substring_fsv() {
        let pin_path = real_pin_path();
        let mut pin =
            load_model_pin(&pin_path, DEFAULT_INNER_MODEL_PIN_KEY).expect("pin must load");
        // Inject a substring that real inner-model GGUF metadata cannot contain.
        pin.expected_identity_substrings
            .push("definitely-not-a-real-model-name-xyzzy".to_string());
        let identity: BTreeMap<String, String> = [
            ("general.architecture".to_string(), "qwen3".to_string()),
            (
                "general.name".to_string(),
                "Qwen3.6-27B-Instruct".to_string(),
            ),
        ]
        .into_iter()
        .collect();
        let model_path = PathBuf::from("/dev/null");
        eprintln!(
            "BEFORE_FSV identity={:?} required_includes={:?}",
            identity, pin.expected_identity_substrings
        );
        let err = validate_inner_model_pin_identity_substrings(
            &model_path,
            &pin,
            &pin_path,
            DEFAULT_INNER_MODEL_PIN_KEY,
            &identity,
        )
        .expect_err("missing substring must fail loud");
        eprintln!("AFTER_FSV error_code={}", err.error_code);
        assert_eq!(
            err.error_code,
            "RUST_REALITY_INNER_MODEL_PIN_IDENTITY_SUBSTRING_MISSING"
        );
    }

    #[test]
    fn gpu_compute_process_rows_parse_and_filter_by_uuid_fsv() {
        let raw = "\
GPU-target, 4242, /opt/contextgraph/reality-loop, 8192
GPU-other, 9999, /usr/bin/other, 1024
";
        eprintln!("BEFORE_GPU_PARSE raw={raw:?} target_uuid=GPU-target");
        let rows = parse_gpu_compute_process_rows(raw, "GPU-target").expect("parse rows");
        eprintln!("AFTER_GPU_PARSE rows={rows:#?}");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].gpu_uuid, "GPU-target");
        assert_eq!(rows[0].pid, 4242);
        assert_eq!(rows[0].process_name, "/opt/contextgraph/reality-loop");
        assert_eq!(rows[0].used_memory_mb, 8192);
    }

    #[test]
    fn gpu_compute_process_rows_empty_output_edge_fsv() {
        eprintln!("BEFORE_GPU_EMPTY raw='' target_uuid=GPU-target");
        let rows = parse_gpu_compute_process_rows("", "GPU-target").expect("empty parse");
        eprintln!("AFTER_GPU_EMPTY rows={rows:#?}");
        assert!(
            rows.is_empty(),
            "empty nvidia-smi compute-apps output should mean zero exposed compute processes"
        );
    }

    #[test]
    fn gpu_compute_process_rows_reject_malformed_csv_edge_fsv() {
        let raw = "GPU-target, 4242, missing-memory-field";
        eprintln!("BEFORE_GPU_MALFORMED raw={raw:?}");
        let err = parse_gpu_compute_process_rows(raw, "GPU-target")
            .expect_err("malformed row must fail loud");
        eprintln!(
            "AFTER_GPU_MALFORMED error={}",
            serde_json::to_string_pretty(&err).unwrap()
        );
        assert_eq!(err.error_code, "RUST_REALITY_GPU_COMPUTE_APPS_PARSE_FAILED");
        assert_eq!(
            err.details
                .get("actual_field_count")
                .and_then(Value::as_u64),
            Some(3)
        );
    }

    #[test]
    fn gpu_compute_process_rows_reject_non_numeric_memory_edge_fsv() {
        let raw = "GPU-target, 4242, /opt/contextgraph/reality-loop, N/A";
        eprintln!("BEFORE_GPU_BAD_MEMORY raw={raw:?}");
        let err = parse_gpu_compute_process_rows(raw, "GPU-target")
            .expect_err("non-numeric memory must fail loud");
        eprintln!(
            "AFTER_GPU_BAD_MEMORY error={}",
            serde_json::to_string_pretty(&err).unwrap()
        );
        assert_eq!(
            err.error_code,
            "RUST_REALITY_GPU_COMPUTE_APPS_MEMORY_PARSE_FAILED"
        );
        assert_eq!(err.field_path, "gpu_vram.compute_processes.used_memory_mb");
    }

    #[test]
    fn gpu_compute_process_rows_skip_bracketed_na_memory_edge_fsv() {
        // nvidia-smi sentinel: "[N/A]" memory means the driver could not
        // introspect this process's VRAM usage. Skip the row, do not fail.
        let raw = "GPU-target, 31923, [Not Found], [N/A]\n\
                   GPU-target, 4242, /opt/contextgraph/reality-loop, 8192";
        eprintln!("BEFORE_GPU_NA_SENTINEL raw={raw:?}");
        let rows = parse_gpu_compute_process_rows(raw, "GPU-target")
            .expect("[N/A] sentinel must be skipped, not failed");
        eprintln!("AFTER_GPU_NA_SENTINEL rows={rows:#?}");
        assert_eq!(rows.len(), 1, "the [N/A] row must be skipped");
        assert_eq!(rows[0].pid, 4242);
        assert_eq!(rows[0].used_memory_mb, 8192);
    }

    #[test]
    fn procfs_snapshot_reads_current_process_source_of_truth_fsv() {
        let pid = std::process::id() as u64;
        let proc_root = PathBuf::from(format!("/proc/{pid}"));
        eprintln!(
            "BEFORE_PROCFS pid={} source_of_truth={}",
            pid,
            file_sot(&proc_root)
        );
        let snapshot = procfs_snapshot_for_pid(pid);
        eprintln!("AFTER_PROCFS snapshot={snapshot:#?}");
        assert_eq!(snapshot.status, "ok");
        assert!(snapshot.uid.is_some(), "current process UID must be read");
        assert!(snapshot.comm.is_some(), "current process comm must be read");
        assert!(
            snapshot
                .source_of_truth
                .iter()
                .any(|path| path.ends_with("/status")),
            "procfs status path must be listed as source of truth"
        );
    }

    #[test]
    fn gpu_budget_failure_surfaces_structured_compute_processes_fsv() {
        let pid = std::process::id() as u64;
        let state = GpuState {
            index: 0,
            name: "NVIDIA GeForce RTX 5090".to_string(),
            uuid: "GPU-target".to_string(),
            memory_total_mb: 32607,
            memory_used_mb: 31000,
            memory_free_mb: 1607,
            driver_version: "test-driver".to_string(),
            compute_process_count: 1,
            compute_processes: vec![GpuComputeProcess {
                gpu_uuid: "GPU-target".to_string(),
                pid,
                process_name: "reality-loop-test".to_string(),
                used_memory_mb: 29400,
                procfs: procfs_snapshot_for_pid(pid),
            }],
            compute_processes_raw: format!("GPU-target, {pid}, reality-loop-test, 29400"),
            source_of_truth: "nvidia-smi:query-gpu+query-compute-apps".to_string(),
            read_at_unix: 1,
        };
        eprintln!("BEFORE_GPU_BUDGET state={state:#?}");
        let err = assert_gpu_budget(&state, 8192, None, "test_phase", DEFAULT_MODEL)
            .expect_err("low free VRAM must fail");
        eprintln!(
            "AFTER_GPU_BUDGET error={}",
            serde_json::to_string_pretty(&err).unwrap()
        );
        assert_eq!(err.error_code, "RUST_REALITY_GPU_VRAM_BUDGET_EXCEEDED");
        assert_eq!(
            err.details
                .pointer("/state/compute_process_count")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            err.details
                .pointer("/state/compute_processes/0/pid")
                .and_then(Value::as_u64),
            Some(pid)
        );
        assert_eq!(
            err.details
                .pointer("/state/compute_processes/0/used_memory_mb")
                .and_then(Value::as_u64),
            Some(29400)
        );
    }

    /// Builds a minimal AttemptArgs pointing at the real on-disk pin manifest
    /// and BF16 GGUF, suitable only for harness-load FSV. Other fields are
    /// safe defaults — none of the SWE-bench paths execute in this test.
    fn fsv_load_test_args() -> AttemptArgs {
        AttemptArgs {
            model: DEFAULT_MODEL.to_string(),
            inner_model_path: real_model_path(),
            inner_chat_template: None,
            task_manifest: PathBuf::from("/dev/null"),
            runtime_root: PathBuf::from("/tmp/cgreality-fsv-load-test"),
            context_graph_bin: PathBuf::from(DEFAULT_CONTEXT_GRAPH_BIN),
            max_attempts: 1,
            run_until_resolved: false,
            smoke_test: true,
            official_eval_each_attempt: false,
            swebench_python: PathBuf::from("/usr/bin/python3"),
            official_timeout: 60,
            official_max_workers: 1,
            official_cache_level: "env".to_string(),
            official_docker_host: DEFAULT_OFFICIAL_DOCKER_HOST.to_string(),
            official_docker_config: PathBuf::from(DEFAULT_OFFICIAL_DOCKER_CONFIG),
            gpu_index: 0,
            gpu_min_free_mb: 0,
            gpu_preload_min_free_mb: 0,
            gpu_max_used_mb: None,
            // Omit explicit context-tokens so the harness uses the model's
            // full n_ctx_train. The pin floor (128K) is enforced — the test
            // proves a real inner-model GGUF satisfies that floor.
            inner_context_tokens: None,
            inner_max_new_tokens: 32,
            inner_temperature: DEFAULT_INNER_TEMPERATURE,
            inner_top_p: DEFAULT_INNER_TOP_P,
            inner_gpu_layers: u32::MAX,
            inner_batch_size: 512,
            inner_seed: DEFAULT_INNER_SEED,
            inner_model_pin_path: real_pin_path(),
            inner_model_pin_key: DEFAULT_INNER_MODEL_PIN_KEY.to_string(),
            git_timeout: 30,
            agent_tool_rounds: 1,
            targeted_test_timeout: 60,
            sleep_between_attempts_ms: 0,
            dynamicjepa_db: None,
            dynamicjepa_artifact_id: None,
            dynamicjepa_skill_id: DEFAULT_DYNAMICJEPA_SKILL_ID.to_string(),
        }
    }

    #[test]
    #[ignore = "loads ~15GB BF16 GGUF onto the RTX 5090 and computes streaming SHA over it (~25s wall). Run via `cargo test --ignored inner_harness_load_end_to_end`."]
    fn inner_harness_load_end_to_end_with_real_bf16_gguf_fsv() {
        let args = fsv_load_test_args();
        eprintln!(
            "BEFORE_FSV model_path={:?} pin_path={:?} pin_key={}",
            args.inner_model_path, args.inner_model_pin_path, args.inner_model_pin_key
        );
        if !args.inner_model_path.exists() {
            panic!(
                "real BF16 GGUF not on disk; fsv impossible: {:?}",
                args.inner_model_path
            );
        }
        if !args.inner_model_pin_path.exists() {
            panic!(
                "real pin manifest not on disk; fsv impossible: {:?}",
                args.inner_model_pin_path
            );
        }
        let harness =
            InnerCodingHarness::load(&args).expect("real BF16 GGUF + matching pin must load");
        eprintln!(
            "AFTER_FSV n_ctx_train={} effective_ctx={} max_new_tokens={} model_path={:?}",
            harness.model_context_train_tokens,
            harness.context_tokens,
            harness.max_new_tokens,
            harness.model_path
        );
        assert_eq!(harness.model_path, args.inner_model_path);
        assert!(
            harness.model_context_train_tokens >= DEFAULT_INNER_MIN_CONTEXT_TOKENS,
            "inner model must report a training context >= {} tokens; got {}",
            DEFAULT_INNER_MIN_CONTEXT_TOKENS,
            harness.model_context_train_tokens
        );
        assert_eq!(
            harness.context_tokens, harness.model_context_train_tokens,
            "with --inner-context-tokens omitted, effective ctx must equal n_ctx_train; got effective={} train={}",
            harness.context_tokens, harness.model_context_train_tokens
        );
        assert!(
            harness.context_tokens >= 128_000,
            "inner model effective ctx must be >= 128K; got {}",
            harness.context_tokens
        );
        let identity = inner_model_identity_readback(&harness.model, &harness.model_path);
        eprintln!("AFTER_FSV identity_readback={:#?}", identity);
        let identity_blob = identity
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("|")
            .to_ascii_lowercase();
        assert!(
            identity_blob.contains("qwen3")
                || identity_blob.contains("qwen-3")
                || identity_blob.contains("qwen 3"),
            "loaded GGUF identity must declare the pinned inner model; got blob={identity_blob}"
        );
        // Drop is automatic at end-of-scope: harness's LlamaModel + LlamaBackend
        // release GPU memory when the struct goes out of scope.
    }

    /// FSV: the round-loop's repeated-error deadlock detector must fire only
    /// when the rolling tail is full of identical non-empty error codes. This
    /// guards the consecutive-error escape hatch added to
    /// `inner_coding_harness_chat_json` so a stuck the inner model trajectory cannot
    /// burn ~5 minutes of GPU producing the same parser error round after round.
    #[test]
    fn repeated_error_deadlock_code_fsv() {
        // Empty tail: no deadlock.
        let empty: VecDeque<String> = VecDeque::new();
        assert_eq!(repeated_error_deadlock_code(&empty), None);

        // Below threshold even when all codes match: still no deadlock.
        let mut short: VecDeque<String> = VecDeque::new();
        for _ in 0..(REPEATED_ERROR_DEADLOCK_LEN - 1) {
            short.push_back("RUST_REALITY_LINE_EDIT_PY_COMPILE_FAILED".to_string());
        }
        assert_eq!(repeated_error_deadlock_code(&short), None);

        // At threshold with mixed codes: no deadlock.
        let mut mixed: VecDeque<String> = VecDeque::new();
        for i in 0..REPEATED_ERROR_DEADLOCK_LEN {
            mixed.push_back(if i == 2 {
                "RUST_REALITY_LINE_EDIT_START_MISSING".to_string()
            } else {
                "RUST_REALITY_LINE_EDIT_PY_COMPILE_FAILED".to_string()
            });
        }
        assert_eq!(repeated_error_deadlock_code(&mixed), None);

        // At threshold with all identical non-empty codes: deadlock fires.
        let mut deadlock: VecDeque<String> = VecDeque::new();
        for _ in 0..REPEATED_ERROR_DEADLOCK_LEN {
            deadlock.push_back("RUST_REALITY_LINE_EDIT_PY_COMPILE_FAILED".to_string());
        }
        assert_eq!(
            repeated_error_deadlock_code(&deadlock).as_deref(),
            Some("RUST_REALITY_LINE_EDIT_PY_COMPILE_FAILED")
        );

        // All-empty codes must not fire — empty isn't a meaningful streak.
        let mut empties: VecDeque<String> = VecDeque::new();
        for _ in 0..REPEATED_ERROR_DEADLOCK_LEN {
            empties.push_back(String::new());
        }
        assert_eq!(repeated_error_deadlock_code(&empties), None);
    }

    /// FSV: hard cap and deadlock-len constants encode the operator's chosen
    /// policy. If anyone bumps these, this assertion forces them to update
    /// the doc comments and the diagnostician runbook deliberately.
    #[test]
    fn round_cap_constants_fsv() {
        assert_eq!(MAX_ROUNDS_PER_ATTEMPT_HARD_CAP, 20);
        assert_eq!(REPEATED_ERROR_DEADLOCK_LEN, 5);
        // Hard cap must comfortably exceed the deadlock detector window so
        // the deadlock detector can actually fire before the cap kicks in.
        assert!((MAX_ROUNDS_PER_ATTEMPT_HARD_CAP as usize) > REPEATED_ERROR_DEADLOCK_LEN);
    }

    /// FSV: per-attempt wallclock budget env-var parse contract. Default
    /// 7200s (120 min) is sized so that at the observed 10-25 min/round generation rate the model has time for 5-12 rounds — enough for read_problem_reality + open_file_window(s) + apply_edit + run_targeted_test before clean abort. Turn-72 raised from 2700 after empirical observation.
    /// generation rate, an attempt forced-stops cleanly via
    /// `RUST_REALITY_ATTEMPT_BUDGET_EXCEEDED` before the 1h MCP-stdio idle
    /// watchdog kills the engine without writing `attempt-summary.json`.
    /// Garbage env values must fall through to the default rather than
    /// panicking — the loop must keep running even with a typo'd override.
    #[test]
    fn attempt_budget_env_parses_with_sensible_default_edge_fsv() {
        // unset → default 7200
        std::env::remove_var("CGREALITY_PER_ATTEMPT_DEADLINE_SECS");
        let secs: u64 = std::env::var("CGREALITY_PER_ATTEMPT_DEADLINE_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(7200);
        assert_eq!(secs, 7200, "default must be 7200 (120 min)");

        // set valid → parsed value
        std::env::set_var("CGREALITY_PER_ATTEMPT_DEADLINE_SECS", "600");
        let secs: u64 = std::env::var("CGREALITY_PER_ATTEMPT_DEADLINE_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(7200);
        assert_eq!(secs, 600, "explicit 600 must be honored");

        // garbage → fallback to default (NOT a parse error)
        std::env::set_var("CGREALITY_PER_ATTEMPT_DEADLINE_SECS", "not-a-number");
        let secs: u64 = std::env::var("CGREALITY_PER_ATTEMPT_DEADLINE_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(7200);
        assert_eq!(secs, 7200, "garbage env value must fall through to default");

        std::env::remove_var("CGREALITY_PER_ATTEMPT_DEADLINE_SECS");
    }
}
