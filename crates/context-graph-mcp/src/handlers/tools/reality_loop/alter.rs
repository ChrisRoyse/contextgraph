use super::errors::{CCRealityError, Result};
use super::helpers::*;
use super::interact::locate_engine_binary;
use super::lock::EditLock;
use super::optimizer;
use super::path_policy;
use super::shift_log::{append_shift, ShiftRecord};
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

impl Handlers {
    pub(crate) async fn call_harness_open_window(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match harness_open_window(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_harness_apply_line_window_edit(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match harness_apply_line_window_edit(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_harness_run_command(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match harness_run_command(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_harness_git_diff(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match harness_git_diff(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_harness_git_status(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match harness_git_status(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_harness_verify_state(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match harness_verify_state(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }
}

pub async fn harness_open_window(args: Value) -> Result<Value> {
    let path = required_str(&args, "path")?;
    path_policy::ensure_path_allowed(&path)?;
    let start = required_u64(&args, "start_line")?;
    let end = required_u64(&args, "end_line")?;
    spawn_harness_subcommand(
        "open-window",
        &[
            "--path",
            &path,
            "--start-line",
            &start.to_string(),
            "--end-line",
            &end.to_string(),
        ],
        None,
        60,
    )
    .await
}

pub async fn harness_apply_line_window_edit(args: Value) -> Result<Value> {
    let path = required_str(&args, "path")?;
    path_policy::ensure_path_allowed(&path)?;
    let start = required_u64(&args, "start_line")?;
    let end = required_u64(&args, "end_line")?;
    let observed_sha = required_str(&args, "observed_sha256")?;
    let replace = required_str(&args, "replace")?;
    let session = session_id(&args);
    let _lock = EditLock::try_acquire(&session)?;

    let root = project_root()?;
    let runtime_root = require_active_runtime_root().await?;
    let target = read_active_target_instance().await?;
    let (run_id, attempt, attempt_dir) = latest_attempt_dir(&runtime_root, target.as_deref())?
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_HARNESS_ATTEMPT_MISSING",
                "could not identify latest attempt for active target",
                "runtime.attempt",
                "run the engine before applying a harness edit",
                json!({"runtime_root": runtime_root.display().to_string(), "target": target}),
                Some(file_sot(&runtime_root)),
            )
        })?;

    let before_path = root.join(&path);
    let before_sha = sha256_file(&before_path)?;
    let tool_dir = attempt_dir.join("claude-code-optimizer/edits");
    let edit = json!({
        "path": path,
        "start_line": start,
        "end_line": end,
        "observed_sha256": observed_sha,
        "replace": replace
    });
    let _ = root; // root resolved earlier for before_sha; subprocess re-resolves it.
    let result = spawn_harness_subcommand(
        "apply-line-window-edit",
        &[
            "--tool-dir",
            tool_dir.to_str().ok_or_else(|| {
                CCRealityError::new(
                    "CCREALITY_HARNESS_TOOL_DIR_NON_UTF8",
                    "tool_dir contains non-UTF-8 bytes",
                    "alter.tool_dir",
                    "ensure runtime root paths are valid UTF-8",
                    json!({"tool_dir": tool_dir.display().to_string()}),
                    None,
                )
            })?,
            "--timeout-secs",
            "120",
        ],
        Some(&edit),
        180,
    )
    .await?;
    let after_sha = sha256_file(&before_path)?;
    let lines_removed = end.saturating_sub(start) + 1;
    let lines_added = replace.lines().count() as u64;
    let diff_path = result
        .pointer("/source_of_truth/diff")
        .and_then(Value::as_str)
        .unwrap_or("");

    let transition_payload = json!({
        "run_id": run_id,
        "attempt": attempt,
        "tool_use_id": optional_str(&args, "tool_use_id"),
        "file_path": path,
        "before_sha256": before_sha,
        "after_sha256": after_sha,
        "observed_sha256": observed_sha,
        "lines_added": lines_added,
        "lines_removed": lines_removed,
        "git_diff_stat": diff_path,
        "outcome": "edited",
        "governed_edit_result": result
    });
    let transition_path = optimizer::write_harness_transition(&attempt_dir, &transition_payload)?;

    let mut shift = ShiftRecord::new(
        "harness_apply_line_window_edit",
        &session,
        json!({"type": "file", "path": path}),
    )?;
    shift.tool_use_id = optional_str(&args, "tool_use_id");
    shift.before = json!({"sha256": before_sha});
    shift.after = json!({"sha256": after_sha, "source_of_truth": file_sot(&before_path)});
    shift.delta_summary = json!({"lines_added": lines_added, "lines_removed": lines_removed, "git_diff_path": diff_path});
    shift.harness_transition_path = Some(file_sot(&transition_path));
    append_shift(&runtime_root, &session, &shift)?;

    Ok(json!({
        "status": "ok",
        "path": path,
        "start_line": start,
        "end_line": end,
        "before_sha256": before_sha,
        "after_sha256": after_sha,
        "lines_added": lines_added,
        "lines_removed": lines_removed,
        "git_diff_path": diff_path,
        "harness_transition_path": file_sot(&transition_path),
        "shift_id": shift.shift_id,
        "source_of_truth": file_sot(&transition_path)
    }))
}

pub async fn harness_run_command(args: Value) -> Result<Value> {
    let command = required_str(&args, "command")?;
    let timeout = optional_u64(&args, "timeout_secs", 120).min(1800);
    let session = session_id(&args);
    let root = project_root()?;
    let runtime_root = require_active_runtime_root().await?;
    let target = read_active_target_instance().await?;
    let (_run_id, _attempt, attempt_dir) = latest_attempt_dir(&runtime_root, target.as_deref())?
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_HARNESS_ATTEMPT_MISSING",
                "could not identify latest attempt for active target",
                "runtime.attempt",
                "run the engine before executing harness commands",
                json!({"runtime_root": runtime_root.display().to_string()}),
                Some(file_sot(&runtime_root)),
            )
        })?;
    let tool_dir = attempt_dir.join("claude-code-optimizer/commands");
    let _ = root;
    let timeout_str = timeout.to_string();
    let tool_dir_str = tool_dir.to_str().ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_HARNESS_TOOL_DIR_NON_UTF8",
            "tool_dir contains non-UTF-8 bytes",
            "run_command.tool_dir",
            "ensure runtime root paths are valid UTF-8",
            json!({"tool_dir": tool_dir.display().to_string()}),
            None,
        )
    })?;
    let result = spawn_harness_subcommand(
        "run-command",
        &["--tool-dir", tool_dir_str, "--timeout-secs", &timeout_str],
        Some(&json!({"command": command})),
        timeout + 30,
    )
    .await?;
    let output = result
        .pointer("/source_of_truth/output")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let mut shift = ShiftRecord::new(
        "harness_run_command",
        &session,
        json!({"type": "command", "command_sha256": sha256_text(&command), "command": command}),
    )?;
    shift.after = json!({"source_of_truth": output});
    shift.delta_summary = json!({"exit_code": result.pointer("/process/exit_code")});
    append_shift(&runtime_root, &session, &shift)?;

    let mut response = result;
    if let Value::Object(map) = &mut response {
        map.insert("shift_id".to_string(), json!(shift.shift_id));
    }
    Ok(response)
}

pub async fn harness_git_diff(args: Value) -> Result<Value> {
    let path = optional_str(&args, "path");
    if let Some(path) = path.as_ref() {
        path_policy::ensure_path_allowed(path)?;
    }
    let diff = match path {
        Some(path) => run_git(&["diff", "--", &path])?,
        None => run_git(&["diff"])?,
    };
    let stat = run_git(&["diff", "--stat"])?;
    Ok(json!({
        "diff": diff.0,
        "diff_stat": stat.0,
        "changed_files": changed_files_from_diff(&diff.0),
        "source_of_truth": "git:working-tree-diff"
    }))
}

pub async fn harness_git_status(_args: Value) -> Result<Value> {
    let status = run_git(&["status", "--porcelain"])?.0;
    let mut modified = Vec::new();
    let mut staged = Vec::new();
    let mut untracked = Vec::new();
    for line in status.lines() {
        let path = line.get(3..).unwrap_or("").to_string();
        if line.starts_with("??") {
            untracked.push(path);
        } else {
            if line.as_bytes().first().is_some_and(|b| *b != b' ') {
                staged.push(path.clone());
            }
            if line.as_bytes().get(1).is_some_and(|b| *b != b' ') {
                modified.push(path);
            }
        }
    }
    Ok(json!({
        "raw": status,
        "modified": modified,
        "staged": staged,
        "untracked": untracked,
        "source_of_truth": "git:status --porcelain"
    }))
}

pub async fn harness_verify_state(args: Value) -> Result<Value> {
    let scope_str = match optional_str(&args, "scope").as_deref() {
        Some("full") => "full",
        _ => "reality_loop_only",
    };
    let runtime_root = require_active_runtime_root().await?;
    let target = read_active_target_instance().await?;
    let (_run_id, _attempt, attempt_dir) = latest_attempt_dir(&runtime_root, target.as_deref())?
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_HARNESS_ATTEMPT_MISSING",
                "could not identify latest attempt for active target",
                "runtime.attempt",
                "run the engine before verifying state through MCP",
                json!({"runtime_root": runtime_root.display().to_string()}),
                Some(file_sot(&runtime_root)),
            )
        })?;
    let tool_dir = attempt_dir.join("claude-code-optimizer/verification");
    let tool_dir_str = tool_dir.to_str().ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_HARNESS_TOOL_DIR_NON_UTF8",
            "tool_dir contains non-UTF-8 bytes",
            "verify_state.tool_dir",
            "ensure runtime root paths are valid UTF-8",
            json!({"tool_dir": tool_dir.display().to_string()}),
            None,
        )
    })?;
    spawn_harness_subcommand(
        "verify-state",
        &[
            "--tool-dir",
            tool_dir_str,
            "--scope",
            scope_str,
            "--label",
            "mcp",
        ],
        None,
        // verify-state runs cargo fmt+check+test; allow up to 30 minutes wall.
        1800,
    )
    .await
}

/// Spawn `target/release/reality-loop harness <subcommand> <args...>` as a fresh
/// subprocess. The CLI binary is re-resolved via `locate_engine_binary()` on every
/// call, so harness rebuilds (`cargo build --release`) take effect on the *next*
/// MCP tool invocation without restarting the MCP server. This is the entire
/// point of the MCP-as-CLI-shim refactor.
///
/// Argv carries scalar flags. `stdin_json`, when present, is serialized and
/// piped to the child's stdin (used for multi-line edit `replace` payloads to
/// avoid Linux ARG_MAX/quoting hazards — see Linux ARG_MAX 128 KB per single arg).
///
/// Subprocess hygiene per tokio docs + Kobzol's process-spawning analysis:
/// `kill_on_drop(true)` ensures the child dies if our future is cancelled (MCP
/// shutdown, panic, caller cancel) — no orphan reparented to PID 1. CWD is set
/// explicitly to the project root so the child's `governed_edit::project_root()`
/// resolves consistently with our parent's view.
///
/// Output contract (matches harness CLI's `harness_dispatch`):
/// - exit 0 → stdout is a single JSON envelope with `status: "ok"` (returned as `Ok`)
/// - exit 1 → stdout is a single JSON envelope with `status: "error"` plus the
///   GovernedEditError-shaped fields; mapped back to `CCRealityError` and returned as `Err`
/// - exit 2 → clap argv parse failed (covered by spawn-time validation; if we hit
///   this in practice it's a bug in our caller; surfaced verbatim)
/// - exit 64+ → infrastructure failure (panic, OOM); stderr tail attached
async fn spawn_harness_subcommand(
    subcommand: &str,
    argv_flags: &[&str],
    stdin_json: Option<&Value>,
    timeout_secs: u64,
) -> Result<Value> {
    let bin = locate_engine_binary()?;
    let mut cmd = Command::new(&bin);
    cmd.arg("harness").arg(subcommand);
    for flag in argv_flags {
        cmd.arg(flag);
    }
    cmd.kill_on_drop(true);
    cmd.current_dir(project_root()?);
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if stdin_json.is_some() {
        cmd.stdin(std::process::Stdio::piped());
    } else {
        cmd.stdin(std::process::Stdio::null());
    }
    let mut child = cmd.spawn().map_err(|e| {
        CCRealityError::new(
            "CCREALITY_HARNESS_SUBCOMMAND_SPAWN_FAILED",
            format!("failed to spawn harness CLI subprocess: {e}"),
            format!("harness.{subcommand}.spawn"),
            "ensure target/release/reality-loop is built (cargo build --release -p context-graph-cli --bin reality-loop) and the path resolves",
            json!({
                "subcommand": subcommand,
                "bin": bin.display().to_string(),
                "argv_flags": argv_flags,
                "io_error": e.to_string()
            }),
            Some(file_sot(&bin)),
        )
    })?;
    if let Some(payload) = stdin_json {
        let mut stdin_handle = child.stdin.take().ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_HARNESS_SUBCOMMAND_STDIN_UNAVAILABLE",
                "child stdin was not piped despite request",
                format!("harness.{subcommand}.stdin"),
                "this is an MCP-server bug; report it",
                json!({"subcommand": subcommand}),
                None,
            )
        })?;
        let bytes = serde_json::to_vec(payload).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_HARNESS_SUBCOMMAND_STDIN_SERIALIZE_FAILED",
                format!("failed to serialize stdin payload as JSON: {e}"),
                format!("harness.{subcommand}.stdin"),
                "this is an MCP-server bug; report it",
                json!({"subcommand": subcommand}),
                None,
            )
        })?;
        stdin_handle.write_all(&bytes).await.map_err(|e| {
            CCRealityError::new(
                "CCREALITY_HARNESS_SUBCOMMAND_STDIN_WRITE_FAILED",
                format!("failed to write stdin payload: {e}"),
                format!("harness.{subcommand}.stdin"),
                "child may have exited before reading stdin; inspect stderr",
                json!({"subcommand": subcommand, "io_error": e.to_string()}),
                None,
            )
        })?;
        // Drop the stdin handle so the child sees EOF and proceeds to read_to_string completion.
        drop(stdin_handle);
    }
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        child.wait_with_output(),
    )
    .await
    .map_err(|_| {
        CCRealityError::new(
            "CCREALITY_HARNESS_SUBCOMMAND_TIMEOUT",
            format!("harness CLI subprocess exceeded {timeout_secs}s wallclock"),
            format!("harness.{subcommand}.timeout"),
            "raise the per-tool timeout or investigate why the child wedged",
            json!({"subcommand": subcommand, "timeout_secs": timeout_secs}),
            None,
        )
    })?
    .map_err(|e| {
        CCRealityError::new(
            "CCREALITY_HARNESS_SUBCOMMAND_WAIT_FAILED",
            format!("failed to await harness CLI subprocess: {e}"),
            format!("harness.{subcommand}.wait"),
            "investigate child crash; check OOM, signal delivery, kernel logs",
            json!({"subcommand": subcommand, "io_error": e.to_string()}),
            None,
        )
    })?;
    let stdout_text = String::from_utf8(output.stdout).map_err(|e| {
        CCRealityError::new(
            "CCREALITY_HARNESS_SUBCOMMAND_STDOUT_UTF8_INVALID",
            format!("harness CLI stdout was not valid UTF-8: {e}"),
            format!("harness.{subcommand}.stdout"),
            "harness CLI bug; inspect the child's stderr for details",
            json!({"subcommand": subcommand}),
            None,
        )
    })?;
    let stderr_text = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout_json: Value = if stdout_text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&stdout_text).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_HARNESS_SUBCOMMAND_STDOUT_JSON_INVALID",
                format!("harness CLI stdout was not JSON: {e}"),
                format!("harness.{subcommand}.stdout"),
                "harness CLI bug; the child must always emit a single JSON document on stdout",
                json!({
                    "subcommand": subcommand,
                    "stdout_preview": tail(&stdout_text, 4096),
                    "stderr_tail": tail(&stderr_text, 4096),
                    "exit_code": output.status.code()
                }),
                None,
            )
        })?
    };
    let exit_code = output.status.code().unwrap_or(-1);
    if exit_code == 0 {
        return Ok(stdout_json);
    }
    if exit_code == 1 {
        // Tool-level error: stdout is the GovernedEditError-shaped envelope.
        // Map fields back to CCRealityError so the JSON-RPC client sees a
        // first-class MCP error with the same code/message/remediation as
        // when the call ran in-process.
        return Err(envelope_to_ccreality(stdout_json, &stderr_text, subcommand));
    }
    // Exit 2 = clap parse failure (caller bug). Exit 64+ = infrastructure (panic, OOM).
    Err(CCRealityError::new(
        "CCREALITY_HARNESS_SUBCOMMAND_NONZERO_EXIT",
        format!("harness CLI subprocess exited with non-tool-error code {exit_code}"),
        format!("harness.{subcommand}.exit"),
        "exit 2 indicates a bad invocation (MCP-server bug); exit 64+ indicates a child crash (inspect stderr)",
        json!({
            "subcommand": subcommand,
            "exit_code": exit_code,
            "stdout": stdout_json,
            "stderr_tail": tail(&stderr_text, 4096)
        }),
        None,
    ))
}

fn envelope_to_ccreality(envelope: Value, stderr_text: &str, subcommand: &str) -> CCRealityError {
    let error_code = envelope
        .get("error_code")
        .and_then(Value::as_str)
        .unwrap_or("CCREALITY_HARNESS_SUBCOMMAND_ERROR_ENVELOPE_MALFORMED")
        .to_string();
    let message = envelope
        .get("message")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("harness CLI returned an error envelope without a message field for subcommand {subcommand}"));
    let field_path = envelope
        .get("field_path")
        .and_then(Value::as_str)
        .unwrap_or("harness.subcommand")
        .to_string();
    let remediation = envelope
        .get("remediation")
        .and_then(Value::as_str)
        .unwrap_or("inspect the harness CLI's structured error envelope on stdout")
        .to_string();
    let mut details = envelope.get("details").cloned().unwrap_or(json!({}));
    if let Value::Object(map) = &mut details {
        map.insert("subcommand".to_string(), json!(subcommand));
        let stderr_tail = tail(stderr_text, 4096);
        if !stderr_tail.is_empty() {
            map.insert("subprocess_stderr_tail".to_string(), json!(stderr_tail));
        }
    }
    let source_of_truth = envelope
        .get("source_of_truth")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    CCRealityError::new(error_code, message, field_path, remediation, details, source_of_truth)
}

fn tail(text: &str, limit: usize) -> String {
    let len = text.chars().count();
    if len <= limit {
        text.to_string()
    } else {
        text.chars().skip(len - limit).collect()
    }
}

#[allow(dead_code)]
fn _silence_pathbuf_lint(_p: &PathBuf) {}

fn run_git(args: &[&str]) -> Result<(String, String)> {
    let root = project_root()?;
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(&root)
        .output()
        .map_err(|e| {
            CCRealityError::new(
                "CCREALITY_HARNESS_READ_COMMAND_SPAWN_FAILED",
                format!("failed to spawn read command: {e}"),
                "harness.read_command",
                "inspect command and project root",
                json!({"command": ["git"], "args": args, "project_root": root.display().to_string()}),
                Some(file_sot(&root)),
            )
        })?;
    Ok((
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    ))
}

fn changed_files_from_diff(diff: &str) -> Vec<String> {
    diff.lines()
        .filter_map(|line| line.strip_prefix("diff --git a/"))
        .filter_map(|rest| rest.split(" b/").next())
        .map(ToOwned::to_owned)
        .collect()
}

// `map_governed_error` was removed in the MCP-as-CLI-shim refactor. Errors now
// arrive as JSON envelopes from the subprocess and are mapped via
// `envelope_to_ccreality`.

#[allow(dead_code)]
fn fsync_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
}
