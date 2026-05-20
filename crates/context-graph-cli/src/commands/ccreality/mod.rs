//! ccreality hook support commands.
//!
//! These commands are intentionally thin: they call the ccreality MCP surface
//! and return the MCP readback. They do not direct-write reality-loop artifacts.

use clap::{Args, Subcommand, ValueEnum};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::UNIX_EPOCH;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

type CCResult<T> = std::result::Result<T, Value>;

#[derive(Subcommand, Debug)]
pub enum CCRealityCommands {
    /// Record a post-edit harness transition through the cgreality MCP tool.
    #[command(name = "record-harness-transition")]
    RecordHarnessTransition(RecordHarnessTransitionArgs),
}

#[derive(Args, Debug, Clone)]
pub struct RecordHarnessTransitionArgs {
    /// Runtime run id. When omitted, the active runtime root is inspected.
    #[arg(long)]
    pub run_id: Option<String>,

    /// Attempt number. When omitted, the active runtime root is inspected.
    #[arg(long)]
    pub attempt: Option<u64>,

    /// Claude Code session id.
    #[arg(long)]
    pub session_id: Option<String>,

    /// Claude Code tool use id.
    #[arg(long)]
    pub tool_use_id: Option<String>,

    /// Claude Code tool name.
    #[arg(long)]
    pub tool_name: Option<String>,

    /// Repo-relative file path that changed.
    #[arg(long)]
    pub file_path: String,

    /// SHA recorded by the PreToolUse hook.
    #[arg(long)]
    pub before_sha256: String,

    /// SHA observed by the PostToolUse hook.
    #[arg(long)]
    pub after_sha256: String,

    /// Source-of-truth JSON file written by the PreToolUse edit hook.
    #[arg(long)]
    pub preedit_state_path: Option<String>,

    /// Absolute source path read by the PostToolUse edit hook after the edit.
    #[arg(long)]
    pub after_source_path: Option<String>,

    /// Lines added by the edit.
    #[arg(long, default_value_t = 0)]
    pub lines_added: u64,

    /// Lines removed by the edit.
    #[arg(long, default_value_t = 0)]
    pub lines_removed: u64,

    /// Git diff stat after the edit.
    #[arg(long, default_value = "")]
    pub git_diff_stat: String,

    /// Cargo check status string.
    #[arg(long, default_value = "")]
    pub cargo_status: String,

    /// Tail of cargo stderr when cargo check fails.
    #[arg(long, default_value = "")]
    pub cargo_stderr_tail: String,

    /// Timeout for the MCP subprocess call.
    #[arg(long, default_value_t = 20)]
    pub timeout_seconds: u64,

    /// Output format.
    #[arg(long, value_enum, default_value_t = CCRealityOutputFormat::JsonCompact)]
    pub format: CCRealityOutputFormat,
}

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum CCRealityOutputFormat {
    Json,
    #[default]
    JsonCompact,
}

pub async fn handle_ccreality_command(cmd: CCRealityCommands) -> i32 {
    let result = match cmd {
        CCRealityCommands::RecordHarnessTransition(args) => record_harness_transition(args).await,
    };

    match result {
        Ok(value) => match serde_json::to_string(&value) {
            Ok(line) => {
                println!("{line}");
                0
            }
            Err(e) => {
                eprintln!(
                    "{}",
                    serde_json::to_string(&ccreality_error(
                        "CCREALITY_CLI_JSON_SERIALIZE_FAILED",
                        format!("failed to serialize ccreality CLI output: {e}"),
                        "cli.stdout",
                        "inspect the returned JSON value",
                        json!({}),
                        None,
                    ))
                    .unwrap_or_else(|_| "{\"status\":\"error\"}".to_string())
                );
                1
            }
        },
        Err(error) => {
            eprintln!(
                "{}",
                serde_json::to_string(&error)
                    .unwrap_or_else(|_| "{\"status\":\"error\"}".to_string())
            );
            1
        }
    }
}

async fn record_harness_transition(args: RecordHarnessTransitionArgs) -> CCResult<Value> {
    let root = project_root()?;
    let (run_id, attempt) = match (args.run_id.clone(), args.attempt) {
        (Some(run_id), Some(attempt)) => (run_id, attempt),
        (None, None) => latest_active_attempt()?,
        _ => {
            return Err(ccreality_error(
                "CCREALITY_CLI_RUN_ATTEMPT_PAIR_INCOMPLETE",
                "run_id and attempt must be supplied together, or both omitted for active-runtime discovery",
                "arguments.run_id",
                "supply both --run-id and --attempt or omit both",
                json!({"run_id_supplied": args.run_id.is_some(), "attempt_supplied": args.attempt.is_some()}),
                None,
            ));
        }
    };

    let cargo_check = if args.cargo_status.is_empty() {
        Value::Null
    } else {
        json!({
            "status": args.cargo_status,
            "stderr_tail": args.cargo_stderr_tail,
            "command": "cargo check -p context-graph-cli --no-default-features --lib"
        })
    };

    let payload = json!({
        "run_id": run_id,
        "attempt": attempt,
        "session_id": args.session_id.unwrap_or_else(|| "cgreality-hook-session-unknown".to_string()),
        "tool_use_id": args.tool_use_id.unwrap_or_else(|| "cgreality-tool-use-unknown".to_string()),
        "tool_name": args.tool_name.unwrap_or_else(|| "EditOrWrite".to_string()),
        "file_path": args.file_path,
        "before_sha256": args.before_sha256,
        "after_sha256": args.after_sha256,
        "preedit_state_path": args.preedit_state_path,
        "after_source_path": args.after_source_path,
        "lines_added": args.lines_added,
        "lines_removed": args.lines_removed,
        "git_diff_stat": args.git_diff_stat,
        "cargo_check": cargo_check,
        "recorded_by": "context-graph-cli ccreality record-harness-transition"
    });

    call_mcp_tool(
        &root,
        "optimizer_record_harness_transition",
        payload,
        args.timeout_seconds,
    )
    .await
}

async fn call_mcp_tool(
    root: &Path,
    tool_name: &str,
    arguments: Value,
    timeout_seconds: u64,
) -> CCResult<Value> {
    let mcp_bin = mcp_binary(root)?;
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": arguments
        }
    });

    let mut child = Command::new(&mcp_bin)
        .arg("--mode")
        .arg("reality-loop")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            ccreality_error(
                "CCREALITY_CLI_MCP_SPAWN_FAILED",
                format!("failed to spawn context-graph-mcp: {e}"),
                "mcp.spawn",
                "run cargo build -p context-graph-mcp --bin context-graph-mcp",
                json!({"mcp_bin": mcp_bin.display().to_string()}),
                Some(file_sot(&mcp_bin)),
            )
        })?;

    let Some(mut stdin) = child.stdin.take() else {
        return Err(ccreality_error(
            "CCREALITY_CLI_MCP_STDIN_UNAVAILABLE",
            "context-graph-mcp child stdin was unavailable",
            "mcp.stdin",
            "inspect tokio process setup",
            json!({"mcp_bin": mcp_bin.display().to_string()}),
            Some(file_sot(&mcp_bin)),
        ));
    };

    stdin
        .write_all(request.to_string().as_bytes())
        .await
        .map_err(|e| {
            ccreality_error(
                "CCREALITY_CLI_MCP_STDIN_WRITE_FAILED",
                format!("failed to write MCP JSON-RPC request: {e}"),
                "mcp.stdin",
                "inspect the MCP subprocess lifecycle",
                json!({"tool_name": tool_name}),
                Some(file_sot(&mcp_bin)),
            )
        })?;
    drop(stdin);

    let output = timeout(
        Duration::from_secs(timeout_seconds),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| {
        ccreality_error(
            "CCREALITY_CLI_MCP_TIMEOUT",
            "context-graph-mcp did not return before timeout",
            "mcp.timeout",
            "inspect MCP startup and runtime-root state",
            json!({"timeout_seconds": timeout_seconds, "tool_name": tool_name}),
            Some(file_sot(&mcp_bin)),
        )
    })?
    .map_err(|e| {
        ccreality_error(
            "CCREALITY_CLI_MCP_WAIT_FAILED",
            format!("failed waiting for context-graph-mcp: {e}"),
            "mcp.wait",
            "inspect MCP subprocess state",
            json!({"tool_name": tool_name}),
            Some(file_sot(&mcp_bin)),
        )
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(ccreality_error(
            "CCREALITY_CLI_MCP_EXIT_FAILED",
            "context-graph-mcp exited unsuccessfully",
            "mcp.exit_status",
            "inspect MCP stderr and the JSON-RPC request payload",
            json!({"exit_code": output.status.code(), "stdout": stdout, "stderr": stderr}),
            Some(file_sot(&mcp_bin)),
        ));
    }

    let rpc: Value = serde_json::from_str(&stdout).map_err(|e| {
        ccreality_error(
            "CCREALITY_CLI_MCP_STDOUT_JSON_INVALID",
            format!("context-graph-mcp stdout was not JSON: {e}"),
            "mcp.stdout",
            "run the MCP request manually and inspect stdout/stderr",
            json!({"stdout": stdout, "stderr": stderr}),
            Some(file_sot(&mcp_bin)),
        )
    })?;

    if let Some(error) = rpc.get("error") {
        return Err(ccreality_error(
            "CCREALITY_CLI_MCP_JSONRPC_ERROR",
            "context-graph-mcp returned a JSON-RPC error",
            "mcp.response.error",
            "inspect the tool arguments and MCP stderr",
            json!({"error": error, "stderr": stderr}),
            Some(file_sot(&mcp_bin)),
        ));
    }

    let text = rpc
        .pointer("/result/content/0/text")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ccreality_error(
                "CCREALITY_CLI_MCP_TEXT_MISSING",
                "MCP response did not contain result.content[0].text",
                "mcp.response.result.content",
                "inspect the MCP handler response shape",
                json!({"response": rpc, "stderr": stderr}),
                Some(file_sot(&mcp_bin)),
            )
        })?;

    let value: Value = serde_json::from_str(text).map_err(|e| {
        ccreality_error(
            "CCREALITY_CLI_MCP_TEXT_JSON_INVALID",
            format!("MCP text payload was not JSON: {e}"),
            "mcp.response.result.content[0].text",
            "inspect the MCP handler text payload",
            json!({"text": text, "stderr": stderr}),
            Some(file_sot(&mcp_bin)),
        )
    })?;

    if rpc.pointer("/result/isError").and_then(Value::as_bool) == Some(true)
        || value.get("status").and_then(Value::as_str) == Some("error")
    {
        return Err(ccreality_error(
            "CCREALITY_CLI_MCP_TOOL_ERROR",
            "MCP tool returned an error payload",
            "mcp.tool_result",
            "inspect the tool error_code, field_path, and remediation",
            json!({"tool_error": value, "stderr": stderr}),
            Some(file_sot(&mcp_bin)),
        ));
    }

    Ok(value)
}

fn latest_active_attempt() -> CCResult<(String, u64)> {
    let runtime_root_file = cache_file("active_runtime_root")?;
    let target_file = cache_file("active_target_instance")?;
    let runtime_root =
        read_trimmed_path(&runtime_root_file, "CCREALITY_CLI_ACTIVE_ROOT_READ_FAILED")?;
    let target = read_trimmed_string(&target_file, "CCREALITY_CLI_ACTIVE_TARGET_READ_FAILED")?;

    if !runtime_root.is_dir() {
        return Err(ccreality_error(
            "CCREALITY_CLI_ACTIVE_ROOT_MISSING",
            "active runtime root does not exist on disk",
            "active_runtime_root",
            "legacy reality-loop attempts are retired; provide explicit run_id and attempt or use ME-JEPA evidence capture",
            json!({"runtime_root": runtime_root.display().to_string()}),
            Some(file_sot(&runtime_root_file)),
        ));
    }

    let run_id = latest_run_id(&runtime_root)?;
    let run_dir = runtime_root.join(&run_id);
    let mut attempts = Vec::new();
    collect_attempt_dirs(&run_dir, &target, &mut attempts)?;
    attempts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let Some((attempt, _path)) = attempts.into_iter().last() else {
        return Err(ccreality_error(
            "CCREALITY_CLI_ATTEMPT_DIR_MISSING",
            "no attempt directory was found for the active target",
            "active_runtime_root.attempts",
            "legacy reality-loop attempts are retired; provide explicit run_id and attempt or use ME-JEPA evidence capture",
            json!({"runtime_root": runtime_root.display().to_string(), "run_id": run_id, "target": target}),
            Some(file_sot(&runtime_root)),
        ));
    };
    Ok((run_id, attempt))
}

fn latest_run_id(runtime_root: &Path) -> CCResult<String> {
    let mut candidates = Vec::<(u64, String)>::new();
    for entry in fs::read_dir(runtime_root).map_err(|e| {
        ccreality_error(
            "CCREALITY_CLI_RUNTIME_ROOT_READ_FAILED",
            format!("failed to read runtime root: {e}"),
            "runtime_root.read_dir",
            "inspect active runtime root permissions",
            json!({"runtime_root": runtime_root.display().to_string()}),
            Some(file_sot(runtime_root)),
        )
    })? {
        let entry = entry.map_err(|e| {
            ccreality_error(
                "CCREALITY_CLI_RUNTIME_ROOT_ENTRY_FAILED",
                format!("failed to read runtime root entry: {e}"),
                "runtime_root.entry",
                "inspect active runtime root permissions",
                json!({"runtime_root": runtime_root.display().to_string()}),
                Some(file_sot(runtime_root)),
            )
        })?;
        let metadata = entry.metadata().map_err(|e| {
            ccreality_error(
                "CCREALITY_CLI_RUNTIME_ROOT_METADATA_FAILED",
                format!("failed to read runtime root entry metadata: {e}"),
                "runtime_root.entry.metadata",
                "inspect active runtime root permissions",
                json!({"path": entry.path().display().to_string()}),
                Some(file_sot(&entry.path())),
            )
        })?;
        if !metadata.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if is_runtime_metadata_dir(&name) {
            continue;
        }
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        candidates.push((modified, name));
    }
    candidates.sort();
    candidates
        .into_iter()
        .last()
        .map(|(_, name)| name)
        .ok_or_else(|| {
            ccreality_error(
                "CCREALITY_CLI_RUN_DIR_MISSING",
                "no run directory was found under the active runtime root",
                "runtime_root.runs",
                "legacy reality-loop attempts are retired; provide explicit run_id and attempt or use ME-JEPA evidence capture",
                json!({"runtime_root": runtime_root.display().to_string()}),
                Some(file_sot(runtime_root)),
            )
        })
}

fn is_runtime_metadata_dir(name: &str) -> bool {
    matches!(
        name,
        "failures"
            | "workspaces"
            | "cgreality-shift-log"
            | "cgreality-hook-state"
            | "claude-code-optimizer"
    )
}

fn collect_attempt_dirs(root: &Path, target: &str, out: &mut Vec<(u64, PathBuf)>) -> CCResult<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root).map_err(|e| {
        ccreality_error(
            "CCREALITY_CLI_ATTEMPT_WALK_READ_FAILED",
            format!("failed to walk attempt tree: {e}"),
            "attempt_tree.read_dir",
            "inspect runtime artifact permissions",
            json!({"path": root.display().to_string()}),
            Some(file_sot(root)),
        )
    })? {
        let entry = entry.map_err(|e| {
            ccreality_error(
                "CCREALITY_CLI_ATTEMPT_WALK_ENTRY_FAILED",
                format!("failed to read attempt tree entry: {e}"),
                "attempt_tree.entry",
                "inspect runtime artifact permissions",
                json!({"path": root.display().to_string()}),
                Some(file_sot(root)),
            )
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(num) = parse_attempt_name(&name) {
            if path.display().to_string().contains(target) {
                out.push((num, path));
            }
        } else {
            collect_attempt_dirs(&path, target, out)?;
        }
    }
    Ok(())
}

fn parse_attempt_name(name: &str) -> Option<u64> {
    name.strip_prefix("attempt-")
        .or_else(|| name.strip_prefix("smoke-attempt-"))
        .and_then(|suffix| suffix.parse::<u64>().ok())
}

fn project_root() -> CCResult<PathBuf> {
    if let Ok(root) = env::var("CONTEXTGRAPH_ROOT") {
        let root = PathBuf::from(root);
        if is_contextgraph_project_root(&root) {
            return Ok(root);
        }
        return Err(ccreality_error(
            "CCREALITY_CLI_PROJECT_ROOT_INVALID",
            "CONTEXTGRAPH_ROOT does not point at a ContextGraph checkout",
            "env.CONTEXTGRAPH_ROOT",
            "set CONTEXTGRAPH_ROOT to /home/cabdru/contextgraph",
            json!({"CONTEXTGRAPH_ROOT": root.display().to_string()}),
            Some(file_sot(&root)),
        ));
    }

    let cwd = env::current_dir().map_err(|e| {
        ccreality_error(
            "CCREALITY_CLI_CURRENT_DIR_FAILED",
            format!("failed to read current working directory: {e}"),
            "process.cwd",
            "run the command from the ContextGraph checkout",
            json!({}),
            None,
        )
    })?;
    for ancestor in cwd.ancestors() {
        if is_contextgraph_project_root(ancestor) {
            return Ok(ancestor.to_path_buf());
        }
    }
    Err(ccreality_error(
        "CCREALITY_CLI_PROJECT_ROOT_NOT_FOUND",
        "could not locate the ContextGraph project root",
        "process.cwd",
        "run from /home/cabdru/contextgraph or set CONTEXTGRAPH_ROOT",
        json!({"cwd": cwd.display().to_string()}),
        None,
    ))
}

fn is_contextgraph_project_root(path: &Path) -> bool {
    path.join("Cargo.toml").is_file()
        && path.join("AGENTS.md").is_file()
        && path.join("CLAUDE.md").is_file()
        && path.join(".mcp.json").is_file()
        && path.join("crates/context-graph-mcp").is_dir()
}

/// Resolve the MCP binary the CLI shells out to.
///
/// Priority order: operator override first, otherwise the command in
/// `.mcp.json` (which Claude Code itself spawns). HTTP-style `.mcp.json`
/// configs do not expose a local command for this subprocess path, so they use
/// the newest local context-graph-mcp binary and report both profile paths if
/// neither exists.
///
/// The previous default (`target/debug/context-graph-mcp` regardless
/// of staleness) caused recurring `CCREALITY_WITNESS_OP_TYPE_UNKNOWN`
/// hook errors when the debug binary was older than the release MCP
/// that wrote new op_types into the witness chain. Explicit command configs
/// still fail closed when that exact binary is missing.
fn mcp_binary(root: &Path) -> CCResult<PathBuf> {
    if let Some(override_path) = env::var_os("CGREALITY_MCP_BIN") {
        let path = normalize_program_path(root, PathBuf::from(override_path));
        return validate_mcp_binary(&path, "mcp.binary.override", Some(file_sot(&path)));
    }

    let config_path = root.join(".mcp.json");
    let config_text = fs::read_to_string(&config_path).map_err(|e| {
        ccreality_error(
            "CCREALITY_CLI_MCP_CONFIG_READ_FAILED",
            format!("failed to read .mcp.json: {e}"),
            "mcp.config",
            "restore .mcp.json or set CGREALITY_MCP_BIN explicitly",
            json!({"path": config_path.display().to_string()}),
            Some(file_sot(&config_path)),
        )
    })?;
    let config: Value = serde_json::from_str(&config_text).map_err(|e| {
        ccreality_error(
            "CCREALITY_CLI_MCP_CONFIG_JSON_INVALID",
            format!(".mcp.json is not valid JSON: {e}"),
            "mcp.config",
            "fix .mcp.json or set CGREALITY_MCP_BIN explicitly",
            json!({"path": config_path.display().to_string()}),
            Some(file_sot(&config_path)),
        )
    })?;
    if let Some(command) = config
        .pointer("/mcpServers/cgreality/command")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        let path = normalize_program_path(root, PathBuf::from(command));
        return validate_mcp_binary(
            &path,
            "mcpServers.cgreality.command",
            Some(file_sot(&config_path)),
        );
    }

    if config
        .pointer("/mcpServers/cgreality/url")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .is_some()
    {
        return newest_local_mcp_binary(
            root,
            "mcpServers.cgreality.url",
            Some(file_sot(&config_path)),
        );
    }

    Err(ccreality_error(
        "CCREALITY_CLI_MCP_CONFIG_COMMAND_MISSING",
        ".mcp.json does not define mcpServers.cgreality.command or mcpServers.cgreality.url",
        "mcpServers.cgreality",
        "set CGREALITY_MCP_BIN explicitly, add a command to .mcp.json, or build a local MCP binary for the HTTP-style config",
        json!({"path": config_path.display().to_string()}),
        Some(file_sot(&config_path)),
    ))
}

fn normalize_program_path(root: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn newest_local_mcp_binary(
    root: &Path,
    field_path: &'static str,
    source_of_truth: Option<String>,
) -> CCResult<PathBuf> {
    let release = root.join("target/release/context-graph-mcp");
    let debug = root.join("target/debug/context-graph-mcp");
    match (is_executable_file(&release), is_executable_file(&debug)) {
        (true, true) => {
            let release_modified = fs::metadata(&release).and_then(|meta| meta.modified()).ok();
            let debug_modified = fs::metadata(&debug).and_then(|meta| meta.modified()).ok();
            if debug_modified > release_modified {
                Ok(debug)
            } else {
                Ok(release)
            }
        }
        (true, false) => Ok(release),
        (false, true) => Ok(debug),
        (false, false) => Err(ccreality_error(
            "CCREALITY_CLI_MCP_BINARY_MISSING",
            "no local context-graph-mcp binary is executable for the HTTP-style .mcp.json config",
            field_path,
            "build target/release/context-graph-mcp or target/debug/context-graph-mcp, or set CGREALITY_MCP_BIN explicitly",
            json!({
                "release": {
                    "path": release.display().to_string(),
                    "exists": release.exists(),
                    "is_file": release.is_file(),
                    "is_executable": is_executable_file(&release)
                },
                "debug": {
                    "path": debug.display().to_string(),
                    "exists": debug.exists(),
                    "is_file": debug.is_file(),
                    "is_executable": is_executable_file(&debug)
                }
            }),
            source_of_truth,
        )),
    }
}

fn validate_mcp_binary(
    path: &Path,
    field_path: &'static str,
    source_of_truth: Option<String>,
) -> CCResult<PathBuf> {
    if is_executable_file(path) {
        return Ok(path.to_path_buf());
    }
    Err(ccreality_error(
        "CCREALITY_CLI_MCP_BINARY_MISSING",
        "configured context-graph-mcp binary is missing or not executable",
        field_path,
        "build the configured MCP binary; do not rely on a different profile fallback",
        json!({
            "path": path.display().to_string(),
            "exists": path.exists(),
            "is_file": path.is_file(),
            "is_executable": is_executable_file(path),
        }),
        source_of_truth,
    ))
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match fs::metadata(path) {
            Ok(metadata) => metadata.permissions().mode() & 0o111 != 0,
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn cache_file(name: &str) -> CCResult<PathBuf> {
    if let Ok(cache_dir) = env::var("CGREALITY_CACHE_DIR") {
        let path = PathBuf::from(&cache_dir).join(name);
        let path = context_graph_paths::require_under_data_root(&path, "CGREALITY_CACHE_DIR")
            .map_err(|e| {
                ccreality_error(
                    e.code,
                    e.message,
                    "env.CGREALITY_CACHE_DIR",
                    e.remediation,
                    json!({"cache_dir": cache_dir, "name": name}),
                    None,
                )
            })?;
        return Ok(path);
    }
    context_graph_paths::cgreality_cache_file(name).map_err(|e| {
        ccreality_error(
            e.code,
            e.message,
            "cgreality.cache_file",
            e.remediation,
            json!({"name": name, "data_root_env": context_graph_paths::ENV_DATA_ROOT}),
            None,
        )
    })
}

fn read_trimmed_path(path: &Path, error_code: &'static str) -> CCResult<PathBuf> {
    Ok(PathBuf::from(read_trimmed_string(path, error_code)?))
}

fn read_trimmed_string(path: &Path, error_code: &'static str) -> CCResult<String> {
    let raw = fs::read_to_string(path).map_err(|e| {
        ccreality_error(
            error_code,
            format!("failed to read cgreality cache file: {e}"),
            "cgreality.cache_file",
            "legacy reality-loop attempts are retired; provide explicit run_id and attempt or use ME-JEPA evidence capture",
            json!({"path": path.display().to_string()}),
            Some(file_sot(path)),
        )
    })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        Err(ccreality_error(
            "CCREALITY_CLI_CACHE_FILE_EMPTY",
            "cgreality cache file is empty",
            "cgreality.cache_file",
            "legacy reality-loop attempts are retired; provide explicit run_id and attempt or use ME-JEPA evidence capture",
            json!({"path": path.display().to_string()}),
            Some(file_sot(path)),
        ))
    } else {
        Ok(trimmed.to_string())
    }
}

fn ccreality_error(
    error_code: impl Into<String>,
    message: impl Into<String>,
    field_path: impl Into<String>,
    remediation: impl Into<String>,
    details: Value,
    source_of_truth: Option<String>,
) -> Value {
    json!({
        "status": "error",
        "error_code": error_code.into(),
        "message": message.into(),
        "field_path": field_path.into(),
        "remediation": remediation.into(),
        "details": details,
        "source_of_truth": source_of_truth
    })
}

fn file_sot(path: &Path) -> String {
    format!("file:{}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "cgreality-cli-{label}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn write_executable(path: &Path) {
        fs::create_dir_all(path.parent().expect("binary parent")).expect("create binary dir");
        fs::write(path, b"#!/bin/sh\nexit 0\n").expect("write binary placeholder");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("chmod executable");
        }
    }

    fn write_mcp_config(root: &Path, command: &Path) {
        fs::create_dir_all(root).expect("create root");
        fs::write(
            root.join(".mcp.json"),
            serde_json::json!({
                "mcpServers": {
                    "cgreality": {
                        "command": command.display().to_string()
                    }
                }
            })
            .to_string(),
        )
        .expect("write .mcp.json");
    }

    fn write_mcp_http_config(root: &Path) {
        fs::create_dir_all(root).expect("create root");
        fs::write(
            root.join(".mcp.json"),
            serde_json::json!({
                "mcpServers": {
                    "cgreality": {
                        "type": "http",
                        "url": "http://127.0.0.1:3101/mcp"
                    }
                }
            })
            .to_string(),
        )
        .expect("write .mcp.json");
    }

    struct EnvGuard {
        key: &'static str,
        old: Option<String>,
    }

    impl EnvGuard {
        fn unset(key: &'static str) -> Self {
            let old = env::var(key).ok();
            env::remove_var(key);
            Self { key, old }
        }

        fn set(key: &'static str, value: &Path) -> Self {
            let old = env::var(key).ok();
            env::set_var(key, value);
            Self { key, old }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(old) = &self.old {
                env::set_var(self.key, old);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    #[test]
    fn latest_run_id_ignores_hook_state_metadata_dir() {
        let root = temp_root("latest-run");
        fs::create_dir_all(root.join("smoke-older")).expect("create smoke run");

        // Ensure metadata has a newer filesystem timestamp than the run dir.
        thread::sleep(Duration::from_millis(1100));
        fs::create_dir_all(root.join("cgreality-hook-state")).expect("create hook state");

        let run_id = latest_run_id(&root).expect("latest run id");
        assert_eq!(run_id, "smoke-older");

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    fn runtime_metadata_dir_names_are_excluded() {
        for name in [
            "failures",
            "workspaces",
            "cgreality-shift-log",
            "cgreality-hook-state",
            "claude-code-optimizer",
        ] {
            assert!(is_runtime_metadata_dir(name), "{name} should be metadata");
        }
        assert!(!is_runtime_metadata_dir("smoke-1778012709"));
    }

    #[test]
    #[serial_test::serial]
    fn mcp_binary_uses_mcp_json_command_as_source_of_truth() {
        let _env = EnvGuard::unset("CGREALITY_MCP_BIN");
        let root = temp_root("mcp-json");
        let configured = root.join("target/release/context-graph-mcp");
        let debug = root.join("target/debug/context-graph-mcp");
        write_executable(&configured);
        write_executable(&debug);
        write_mcp_config(&root, &configured);

        let selected = mcp_binary(&root).expect("configured MCP binary selected");
        assert_eq!(selected, configured);

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    #[serial_test::serial]
    fn mcp_binary_fails_closed_instead_of_falling_back_to_debug() {
        let _env = EnvGuard::unset("CGREALITY_MCP_BIN");
        let root = temp_root("mcp-no-debug-fallback");
        let configured = root.join("target/release/context-graph-mcp");
        let debug = root.join("target/debug/context-graph-mcp");
        write_executable(&debug);
        write_mcp_config(&root, &configured);

        let err = mcp_binary(&root).expect_err("debug fallback must be explicit");
        assert_eq!(err["error_code"], "CCREALITY_CLI_MCP_BINARY_MISSING");
        assert_eq!(
            err["field_path"],
            serde_json::json!("mcpServers.cgreality.command")
        );
        assert_eq!(err["details"]["exists"], false);
        assert_eq!(err["details"]["is_executable"], false);
        assert_eq!(err["source_of_truth"], file_sot(&root.join(".mcp.json")));

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    #[serial_test::serial]
    fn mcp_binary_http_config_uses_newest_local_binary() {
        let _env = EnvGuard::unset("CGREALITY_MCP_BIN");
        let root = temp_root("mcp-http-local-newest");
        let release = root.join("target/release/context-graph-mcp");
        let debug = root.join("target/debug/context-graph-mcp");
        write_executable(&release);
        thread::sleep(Duration::from_millis(2100));
        write_executable(&debug);
        write_mcp_http_config(&root);

        let selected = mcp_binary(&root).expect("newest local MCP binary selected");
        assert_eq!(selected, debug);

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    #[serial_test::serial]
    fn mcp_binary_http_config_fails_when_no_local_binary_exists() {
        let _env = EnvGuard::unset("CGREALITY_MCP_BIN");
        let root = temp_root("mcp-http-no-local-binary");
        write_mcp_http_config(&root);

        let err = mcp_binary(&root).expect_err("missing local MCP binary must reject");
        assert_eq!(err["error_code"], "CCREALITY_CLI_MCP_BINARY_MISSING");
        assert_eq!(
            err["field_path"],
            serde_json::json!("mcpServers.cgreality.url")
        );
        assert_eq!(err["source_of_truth"], file_sot(&root.join(".mcp.json")));

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    #[serial_test::serial]
    fn mcp_binary_accepts_explicit_override() {
        let root = temp_root("mcp-override");
        let override_bin = root.join("custom/context-graph-mcp");
        write_executable(&override_bin);
        let _env = EnvGuard::set("CGREALITY_MCP_BIN", &override_bin);

        let selected = mcp_binary(&root).expect("explicit override selected");
        assert_eq!(selected, override_bin);

        fs::remove_dir_all(root).expect("cleanup temp root");
    }

    #[test]
    #[serial_test::serial]
    fn mcp_binary_rejects_non_executable_override_with_physical_details() {
        let root = temp_root("mcp-bad-override");
        let override_bin = root.join("custom/context-graph-mcp");
        fs::create_dir_all(override_bin.parent().expect("binary parent"))
            .expect("create override dir");
        fs::write(&override_bin, b"not executable\n").expect("write non-executable file");
        let _env = EnvGuard::set("CGREALITY_MCP_BIN", &override_bin);

        let err = mcp_binary(&root).expect_err("non-executable override must reject");
        assert_eq!(err["error_code"], "CCREALITY_CLI_MCP_BINARY_MISSING");
        assert_eq!(err["details"]["exists"], true);
        assert_eq!(err["details"]["is_file"], true);
        #[cfg(unix)]
        assert_eq!(err["details"]["is_executable"], false);
        assert_eq!(err["source_of_truth"], file_sot(&override_bin));

        fs::remove_dir_all(root).expect("cleanup temp root");
    }
}
