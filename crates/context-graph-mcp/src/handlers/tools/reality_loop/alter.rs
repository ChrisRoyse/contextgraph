use super::errors::{CCRealityError, Result};
use super::helpers::*;
use super::lock::EditLock;
use super::optimizer;
use super::path_policy;
use super::shift_log::{append_shift, ShiftRecord};
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

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
    let root = project_root()?;
    context_graph_cli::governed_edit::project_file_window(&root, &path, start, end)
        .map_err(map_governed_error)
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
    let result = context_graph_cli::governed_edit::project_apply_line_window_edits(
        &root,
        &tool_dir,
        &[edit],
        120,
    )
    .map_err(map_governed_error)?;
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
    let result =
        context_graph_cli::governed_edit::run_governed_command(&root, &tool_dir, &command, timeout)
            .map_err(map_governed_error)?;
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
    let scope = match optional_str(&args, "scope").as_deref() {
        Some("full") => context_graph_cli::governed_edit::VerifyScope::Full,
        _ => context_graph_cli::governed_edit::VerifyScope::RealityLoopOnly,
    };
    let root = project_root()?;
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
    context_graph_cli::governed_edit::verify_project_state(&root, &tool_dir, "mcp", scope)
        .map_err(map_governed_error)
}

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

fn map_governed_error(err: context_graph_cli::governed_edit::GovernedEditError) -> CCRealityError {
    CCRealityError::new(
        format!("CCREALITY_HARNESS_{}", err.error_code),
        err.message,
        err.field_path,
        err.remediation,
        err.details,
        err.source_of_truth,
    )
}

#[allow(dead_code)]
fn fsync_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
}
