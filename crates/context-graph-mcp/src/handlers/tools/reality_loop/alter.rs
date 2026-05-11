use super::errors::{CCRealityError, Result};
use super::helpers::*;
use super::lock::EditLock;
use super::optimizer;
use super::path_policy;
use super::shift_log::{append_shift, ShiftRecord};
use super::witness_chain::{append_witness_entry_for_run, WitnessOpType};
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use serde_json::{json, Value};
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
    open_project_file_window(&project_root()?, &path, start, end)
}

pub async fn harness_apply_line_window_edit(args: Value) -> Result<Value> {
    let path = required_str(&args, "path")?;
    path_policy::ensure_path_allowed(&path)?;
    let start = required_u64(&args, "start_line")?;
    let end = required_u64(&args, "end_line")?;
    let observed_sha = required_str(&args, "observed_sha256")?;
    let replace = required_str(&args, "replace")?;
    let session = session_id(&args)?;
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
    let before_bytes = fs::read(&before_path)
        .map_err(|e| fs_error("CCREALITY_HARNESS_BEFORE_READ_FAILED", &before_path, e))?;
    std::str::from_utf8(&before_bytes).map_err(|e| {
        CCRealityError::new(
            "CCREALITY_HARNESS_BEFORE_NON_UTF8",
            format!("pre-edit file content is not valid UTF-8: {e}"),
            "harness_apply_line_window_edit.before",
            "only use the line-window edit harness on UTF-8 source files",
            json!({"path": path}),
            Some(file_sot(&before_path)),
        )
    })?;
    let tool_dir = attempt_dir.join("claude-code-optimizer/edits");
    fs::create_dir_all(&tool_dir)
        .map_err(|e| fs_error("CCREALITY_HARNESS_EDIT_DIR_CREATE_FAILED", &tool_dir, e))?;
    let before_text_sot_path =
        tool_dir.join(format!("before-{}.txt", uuid::Uuid::new_v4().as_simple()));
    fs::write(&before_text_sot_path, &before_bytes).map_err(|e| {
        fs_error(
            "CCREALITY_HARNESS_BEFORE_TEXT_SOT_WRITE_FAILED",
            &before_text_sot_path,
            e,
        )
    })?;
    fs::File::open(&before_text_sot_path)
        .and_then(|file| file.sync_all())
        .map_err(|e| {
            fs_error(
                "CCREALITY_HARNESS_BEFORE_TEXT_SOT_SYNC_FAILED",
                &before_text_sot_path,
                e,
            )
        })?;
    let edit = json!({
        "path": path,
        "start_line": start,
        "end_line": end,
        "observed_sha256": observed_sha,
        "replace": replace
    });
    let result = apply_line_window_edit_direct(&root, &tool_dir, &edit, 120)?;
    let after_sha = sha256_file(&before_path)?;
    let lines_removed = end.saturating_sub(start) + 1;
    let lines_added = replace.lines().count() as u64;
    let diff_path = result
        .pointer("/source_of_truth/diff")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_HARNESS_EDIT_DIFF_SOT_MISSING",
                "governed edit result did not include source_of_truth.diff",
                "governed_edit.source_of_truth.diff",
                "inspect the governed-edit output schema before recording the transition",
                json!({"result": result}),
                None,
            )
        })?;
    let tool_use_id = optional_str_strict(&args, "tool_use_id")?;

    let transition_payload = json!({
        "run_id": run_id,
        "attempt": attempt,
        "tool_use_id": tool_use_id.clone(),
        "file_path": path,
        "before_sha256": before_sha,
        "after_sha256": after_sha,
        "observed_sha256": observed_sha,
        "lines_added": lines_added,
        "lines_removed": lines_removed,
        "git_diff_stat": diff_path,
        "before_text_source_of_truth": file_sot(&before_text_sot_path),
        "after_source_path": file_sot(&before_path),
        "outcome": "edited",
        "governed_edit_result": result
    });
    let transition_path = optimizer::write_harness_transition(&attempt_dir, &transition_payload)?;
    let transition_sha = sha256_file(&transition_path)?;
    let witness_append = append_witness_entry_for_run(
        &runtime_root,
        &run_id,
        WitnessOpType::HarnessTransition,
        &transition_sha,
    )?;

    let mut shift = ShiftRecord::new(
        "harness_apply_line_window_edit",
        &session,
        json!({
            "type": "file_edit",
            "task_id": format!("{run_id}:{attempt}:line-window-edit"),
            "path": path,
            "harness_transition": file_sot(&transition_path),
            "tests": ["phase7_harness_line_window_replay"],
            "problem_statement": format!("Replay direct line-window edit for {path}"),
            "os": std::env::consts::OS
        }),
    )?;
    shift.tool_use_id = tool_use_id.clone();
    shift.before = json!({
        "sha256": before_sha,
        "text_source_of_truth": file_sot(&before_text_sot_path)
    });
    shift.after = json!({"sha256": after_sha, "source_of_truth": file_sot(&before_path)});
    shift.delta_summary = json!({
        "artifact": file_sot(&transition_path),
        "lines_added": lines_added,
        "lines_removed": lines_removed,
        "git_diff_path": diff_path
    });
    shift.verification = optimizer::witness_segment_from_append(&witness_append)?;
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
        "witness_append": witness_append,
        "shift_id": shift.shift_id,
        "source_of_truth": file_sot(&transition_path)
    }))
}

pub async fn harness_run_command(args: Value) -> Result<Value> {
    let command = required_str(&args, "command")?;
    let timeout = optional_u64_strict(&args, "timeout_secs", 120)?.min(1800);
    let session = session_id(&args)?;
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
    let result = run_governed_command_direct(&root, Path::new(tool_dir_str), &command, timeout)?;
    let output = result
        .pointer("/source_of_truth/output")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_HARNESS_COMMAND_OUTPUT_SOT_MISSING",
                "run-command result did not include source_of_truth.output",
                "run_command.source_of_truth.output",
                "inspect the harness run-command output schema before recording the shift",
                json!({"result": result}),
                None,
            )
        })?
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
    let path = optional_str_strict(&args, "path")?;
    if let Some(path) = path.as_ref() {
        path_policy::ensure_path_allowed(path)?;
    }
    let diff = match path.as_ref() {
        Some(path) => run_git(&["diff", "--", path])?,
        None => run_git(&["diff"])?,
    };
    let stat = match path.as_ref() {
        Some(path) => run_git(&["diff", "--stat", "--", path])?,
        None => run_git(&["diff", "--stat"])?,
    };
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
    let scope_str = match optional_str_strict(&args, "scope")?.as_deref() {
        Some("full") => "full",
        Some("mejepa_loop_only") | Some("reality_loop_only") | None => "mejepa_loop_only",
        Some(other) => {
            return Err(CCRealityError::new(
                "CCREALITY_HARNESS_VERIFY_SCOPE_INVALID",
                format!("unsupported harness verification scope '{other}'"),
                "arguments.scope",
                "use scope=\"mejepa_loop_only\" or scope=\"full\"",
                json!({"scope": other}),
                None,
            ));
        }
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
    verify_project_state_direct(&project_root()?, Path::new(tool_dir_str), "mcp", scope_str)
}

fn open_project_file_window(
    project_root: &Path,
    relative_path: &str,
    start_line: u64,
    end_line: u64,
) -> Result<Value> {
    if start_line < 1 {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_WINDOW_START_INVALID",
            "start_line must be at least 1",
            "arguments.start_line",
            "open a window using one-indexed inclusive line numbers",
            json!({"start_line": start_line, "end_line": end_line}),
            None,
        ));
    }
    if end_line < start_line {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_WINDOW_END_INVALID",
            "end_line must be greater than or equal to start_line",
            "arguments.end_line",
            "open a window using an inclusive range with end_line >= start_line",
            json!({"start_line": start_line, "end_line": end_line}),
            None,
        ));
    }

    let path = validate_project_relative_path(relative_path, "arguments.path")?;
    let full_path = project_root.join(&path);
    ensure_existing_project_file(project_root, &full_path, "arguments.path")?;
    let text = fs::read_to_string(&full_path)
        .map_err(|err| fs_error("CCREALITY_HARNESS_WINDOW_READ_FAILED", &full_path, err))?;
    let spans = line_spans(&text);
    let total_lines = spans.len() as u64;
    let capped_end = end_line.min(start_line + 399);
    let window_text = if spans.is_empty() || start_line > total_lines {
        String::new()
    } else {
        let actual_end = capped_end.min(total_lines);
        let byte_start = spans[(start_line - 1) as usize].0;
        let byte_end = spans[(actual_end - 1) as usize].1;
        text[byte_start..byte_end].to_string()
    };

    Ok(json!({
        "status": "ok",
        "path": path,
        "start_line": start_line,
        "end_line": capped_end.min(total_lines.max(start_line)),
        "requested_end_line": end_line,
        "total_lines": total_lines,
        "text": window_text,
        "sha256": sha256_file(&full_path)?,
        "source_of_truth": file_sot(&full_path)
    }))
}

fn apply_line_window_edit_direct(
    project_root: &Path,
    tool_dir: &Path,
    edit: &Value,
    timeout_secs: u64,
) -> Result<Value> {
    fs::create_dir_all(tool_dir)
        .map_err(|err| fs_error("CCREALITY_HARNESS_TOOL_DIR_CREATE_FAILED", tool_dir, err))?;
    let edits_path = tool_dir.join("project-line-window-edits.json");
    write_json_checked(&edits_path, &vec![edit.clone()])?;
    let before_status = run_required_process(
        &["git", "status", "--porcelain"],
        project_root,
        timeout_secs,
        "CCREALITY_HARNESS_PROJECT_STATUS_FAILED",
        "project.git_status.before_edit",
    )?;

    let path = required_edit_str(edit, "path", "governed_edits[0]", &edits_path)?;
    let path = validate_project_relative_path(path, "governed_edits[0].path")?;
    let start_line =
        required_edit_u64_at_least(edit, "start_line", 1, "governed_edits[0]", &edits_path)?;
    let end_line = required_edit_u64_at_least(
        edit,
        "end_line",
        start_line,
        "governed_edits[0]",
        &edits_path,
    )?;
    if end_line - start_line > 500 {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_EDIT_RANGE_TOO_LARGE",
            "project edit range is too large",
            "governed_edits[0].end_line",
            "split large project edits into smaller line-window edits",
            json!({"path": path, "start_line": start_line, "end_line": end_line}),
            Some(file_sot(&edits_path)),
        ));
    }
    let observed_sha256 = normalize_observed_sha(edit, "governed_edits[0]", &edits_path)?;
    let replace = required_edit_str(edit, "replace", "governed_edits[0]", &edits_path)?;
    let full_path = project_root.join(&path);
    ensure_existing_project_file(project_root, &full_path, "governed_edits[0].path")?;

    let before = fs::read_to_string(&full_path)
        .map_err(|err| fs_error("CCREALITY_HARNESS_EDIT_FILE_READ_FAILED", &full_path, err))?;
    let actual_sha256 = sha256_file(&full_path)?;
    if actual_sha256 != observed_sha256 {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_EDIT_SHA_MISMATCH",
            "project edit observed_sha256 does not match file readback",
            "governed_edits[0].observed_sha256",
            "reopen the project file window before editing",
            json!({
                "path": path,
                "expected": observed_sha256,
                "actual": actual_sha256,
                "source_of_truth": file_sot(&full_path)
            }),
            Some(file_sot(&full_path)),
        ));
    }

    let spans = line_spans(&before);
    if spans.is_empty() || end_line as usize > spans.len() {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_EDIT_RANGE_OUT_OF_BOUNDS",
            "project edit range is outside current file bounds",
            "governed_edits[0].end_line",
            "reopen the project file window and use current line numbers",
            json!({
                "path": path,
                "start_line": start_line,
                "end_line": end_line,
                "total_lines": spans.len()
            }),
            Some(file_sot(&full_path)),
        ));
    }
    let byte_start = spans[(start_line - 1) as usize].0;
    let byte_end = spans[(end_line - 1) as usize].1;
    let old_span = &before[byte_start..byte_end];
    let mut after = String::with_capacity(before.len() + replace.len());
    after.push_str(&before[..byte_start]);
    after.push_str(replace);
    if !replace.ends_with('\n') && byte_end < before.len() {
        after.push('\n');
    }
    after.push_str(&before[byte_end..]);
    write_text_checked(&full_path, &after)?;
    let readback = fs::read_to_string(&full_path)
        .map_err(|err| fs_error("CCREALITY_HARNESS_EDIT_READBACK_FAILED", &full_path, err))?;
    if readback != after {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_EDIT_READBACK_MISMATCH",
            "project edit file readback did not match written content",
            "governed_edits[0].readback",
            "inspect filesystem durability before continuing",
            json!({
                "path": path,
                "expected_sha256": sha256_text(&after),
                "actual_sha256": sha256_text(&readback)
            }),
            Some(file_sot(&full_path)),
        ));
    }

    let applied = vec![json!({
        "index": 0,
        "path": path,
        "start_line": start_line,
        "end_line": end_line,
        "before_sha256": sha256_text(&before),
        "after_sha256": sha256_text(&after),
        "observed_sha256": observed_sha256,
        "old_span_sha256": sha256_text(old_span),
        "replace_sha256": sha256_text(replace),
        "source_of_truth": file_sot(&full_path)
    })];
    let diff = run_required_process(
        &["git", "diff", "--no-ext-diff", "--binary"],
        project_root,
        timeout_secs,
        "CCREALITY_HARNESS_PROJECT_DIFF_FAILED",
        "project.git_diff",
    )?;
    let diff_path = tool_dir.join("project-diff-after-edit.patch");
    write_text_checked(
        &diff_path,
        diff.get("stdout")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )?;
    let file_readbacks = vec![json!({
        "path": path,
        "sha256": sha256_file(&full_path)?,
        "size_bytes": fs::metadata(&full_path)
            .map_err(|err| fs_error("CCREALITY_HARNESS_EDIT_METADATA_FAILED", &full_path, err))?
            .len(),
        "source_of_truth": file_sot(&full_path)
    })];
    let readbacks_path = tool_dir.join("project-file-readbacks-after-edit.json");
    write_json_checked(&readbacks_path, &file_readbacks)?;
    let after_status = run_required_process(
        &["git", "status", "--porcelain"],
        project_root,
        timeout_secs,
        "CCREALITY_HARNESS_PROJECT_STATUS_FAILED",
        "project.git_status.after_edit",
    )?;
    let status_path = tool_dir.join("project-status-after-edit.txt");
    write_text_checked(
        &status_path,
        after_status
            .get("stdout")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )?;
    let result_path = tool_dir.join("project-line-window-edit-result.json");
    let payload = json!({
        "record_kind": "contextgraph_governed_project_edit_result",
        "status": "ok",
        "action": "edited real ContextGraph project files through in-process line-window edits",
        "applied_edits": applied,
        "changed_files": [path],
        "file_readbacks": file_readbacks,
        "before_git_status": before_status.get("stdout").cloned().unwrap_or(Value::Null),
        "after_git_status": after_status.get("stdout").cloned().unwrap_or(Value::Null),
        "diff_sha256": sha256_file(&diff_path)?,
        "source_of_truth": {
            "result": file_sot(&result_path),
            "edits": file_sot(&edits_path),
            "diff": file_sot(&diff_path),
            "file_readbacks": file_sot(&readbacks_path),
            "git_status": file_sot(&status_path),
            "project_root": file_sot(project_root)
        }
    });
    write_json_checked(&result_path, &payload)?;
    let readback: Value = read_json(&result_path)?;
    if readback != payload {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_EDIT_RESULT_READBACK_MISMATCH",
            "project edit result artifact readback did not match written payload",
            "governed_edits.result_readback",
            "inspect filesystem durability before using edit result",
            json!({"expected": payload, "actual": readback}),
            Some(file_sot(&result_path)),
        ));
    }
    Ok(readback)
}

fn run_governed_command_direct(
    project_root: &Path,
    tool_dir: &Path,
    command: &str,
    timeout_secs: u64,
) -> Result<Value> {
    let argv = parse_governed_command(command)?;
    fs::create_dir_all(tool_dir)
        .map_err(|err| fs_error("CCREALITY_HARNESS_TOOL_DIR_CREATE_FAILED", tool_dir, err))?;
    let result = run_cmd(&argv, project_root, timeout_secs)?;
    let safe_name = safe_id(command);
    let output_path = tool_dir.join(format!("project-command-{safe_name}.json"));
    let payload = json!({
        "record_kind": "contextgraph_governed_project_command_result",
        "status": if command_exit_code(&result) == Some(0) { "ok" } else { "failed" },
        "command": command,
        "argv": argv,
        "process": result,
        "source_of_truth": {
            "output": file_sot(&output_path),
            "project_root": file_sot(project_root)
        }
    });
    write_json_checked(&output_path, &payload)?;
    let readback: Value = read_json(&output_path)?;
    if readback != payload {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_COMMAND_READBACK_MISMATCH",
            "project command artifact readback did not match written payload",
            "governed_command.readback",
            "inspect filesystem durability before using command result",
            json!({"expected": payload, "actual": readback}),
            Some(file_sot(&output_path)),
        ));
    }
    Ok(readback)
}

fn verify_project_state_direct(
    project_root: &Path,
    tool_dir: &Path,
    label: &str,
    scope: &str,
) -> Result<Value> {
    let verify_dir = tool_dir.join(format!("project-verification-{}", safe_id(label)));
    let (rustfmt_command, check_command, test_command) = match scope {
        "mejepa_loop_only" => (
            "cargo fmt --check -p context-graph-cli",
            "cargo check -p context-graph-cli --no-default-features --lib",
            "cargo test -p context-graph-cli --no-default-features --lib -- --skip ignored --test-threads=1",
        ),
        "full" => (
            "cargo fmt --check",
            "cargo check -p context-graph-cli",
            "cargo test -p context-graph-cli -- --skip ignored --test-threads=1",
        ),
        _ => unreachable!("scope was validated by harness_verify_state"),
    };

    let rustfmt = run_governed_command_direct(project_root, &verify_dir, rustfmt_command, 120)?;
    let cargo_check = run_governed_command_direct(project_root, &verify_dir, check_command, 900)?;
    let cargo_test = run_governed_command_direct(project_root, &verify_dir, test_command, 1200)?;
    let git_status =
        run_governed_command_direct(project_root, &verify_dir, "git status --short", 120)?;
    let git_diff_stat =
        run_governed_command_direct(project_root, &verify_dir, "git diff --stat", 120)?;
    let status = if command_exit_code(&rustfmt) == Some(0)
        && command_exit_code(&cargo_check) == Some(0)
        && command_exit_code(&cargo_test) == Some(0)
    {
        "ok"
    } else {
        "failed"
    };
    let path = verify_dir.join("project-state-verification.json");
    let payload = json!({
        "record_kind": "contextgraph_governed_project_state_verification",
        "status": status,
        "label": label,
        "scope": scope,
        "rustfmt": rustfmt,
        "cargo_check": cargo_check,
        "cargo_test": cargo_test,
        "git_status": git_status,
        "git_diff_stat": git_diff_stat,
        "source_of_truth": file_sot(&path)
    });
    write_json_checked(&path, &payload)?;
    let readback: Value = read_json(&path)?;
    if readback != payload {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_VERIFY_READBACK_MISMATCH",
            "project state verification readback did not match written payload",
            "governed_project_verification.readback",
            "inspect filesystem durability before continuing",
            json!({"expected": payload, "actual": readback}),
            Some(file_sot(&path)),
        ));
    }
    if readback.get("status").and_then(Value::as_str) != Some("ok") {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_VERIFY_FAILED",
            "governed project verification failed",
            "governed_project_verification",
            "inspect verification command artifacts and fix compile or test errors",
            json!({"verification": file_sot(&path)}),
            Some(file_sot(&path)),
        ));
    }
    Ok(readback)
}

fn validate_project_relative_path(path: &str, field_path: &str) -> Result<String> {
    let path = path.trim();
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path.contains('\n')
        || Path::new(path)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_PROJECT_PATH_INVALID",
            "project path must be repo-relative without traversal, absolute roots, or control characters",
            field_path,
            "use a normalized relative path inside the ContextGraph project",
            json!({"path": path}),
            None,
        ));
    }
    Ok(path.to_string())
}

fn ensure_existing_project_file(
    project_root: &Path,
    full_path: &Path,
    field_path: &str,
) -> Result<()> {
    let root = project_root.canonicalize().map_err(|err| {
        fs_error(
            "CCREALITY_HARNESS_PROJECT_ROOT_CANONICALIZE_FAILED",
            project_root,
            err,
        )
    })?;
    let parent = full_path
        .parent()
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_HARNESS_PROJECT_PATH_PARENT_MISSING",
                "project path has no parent directory",
                field_path,
                "use a normal project file path",
                json!({"path": full_path.display().to_string()}),
                Some(file_sot(full_path)),
            )
        })?
        .canonicalize()
        .map_err(|err| {
            fs_error(
                "CCREALITY_HARNESS_PROJECT_PATH_CANONICALIZE_FAILED",
                full_path,
                err,
            )
        })?;
    if !parent.starts_with(&root) {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_PROJECT_PATH_ESCAPES_ROOT",
            "project path resolves outside the project root",
            field_path,
            "use a path inside the current ContextGraph checkout",
            json!({
                "root": root.display().to_string(),
                "path": full_path.display().to_string(),
                "parent": parent.display().to_string()
            }),
            Some(file_sot(full_path)),
        ));
    }
    if !full_path.is_file() {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_PROJECT_FILE_MISSING",
            "requested project file does not exist",
            field_path,
            "open or edit a real text file from the project tree",
            json!({"path": full_path.display().to_string()}),
            Some(file_sot(full_path)),
        ));
    }
    Ok(())
}

fn normalize_observed_sha(edit: &Value, field_prefix: &str, source: &Path) -> Result<String> {
    let value = edit
        .get("observed_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_HARNESS_EDIT_SHA_MISSING",
                "project edit must include observed_sha256 from harness_open_window",
                format!("{field_prefix}.observed_sha256"),
                "read the file first, then edit against that exact source reality",
                json!({"edit": edit}),
                Some(file_sot(source)),
            )
        })?;
    let suffix = value.strip_prefix("sha256:").unwrap_or(value);
    if suffix.len() == 64 && suffix.as_bytes().iter().all(|b| b.is_ascii_hexdigit()) {
        Ok(format!("sha256:{suffix}"))
    } else {
        Err(CCRealityError::new(
            "CCREALITY_HARNESS_EDIT_SHA_FORMAT_INVALID",
            "project edit observed_sha256 has an unrecognized format",
            format!("{field_prefix}.observed_sha256"),
            "observed_sha256 must be 'sha256:<64-hex>' or bare '<64-hex>'",
            json!({"received": value}),
            Some(file_sot(source)),
        ))
    }
}

fn required_edit_str<'a>(
    edit: &'a Value,
    field: &str,
    field_prefix: &str,
    source: &Path,
) -> Result<&'a str> {
    edit.get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_HARNESS_EDIT_FIELD_MISSING",
                "required edit field is missing or empty",
                format!("{field_prefix}.{field}"),
                "provide all required line-window edit fields",
                json!({"field": field, "edit": edit}),
                Some(file_sot(source)),
            )
        })
}

fn required_edit_u64_at_least(
    edit: &Value,
    field: &str,
    minimum: u64,
    field_prefix: &str,
    source: &Path,
) -> Result<u64> {
    edit.get(field)
        .and_then(Value::as_u64)
        .filter(|value| *value >= minimum)
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_HARNESS_EDIT_FIELD_INVALID",
                "required numeric edit field is missing or too small",
                format!("{field_prefix}.{field}"),
                "use current one-indexed line numbers from harness_open_window",
                json!({"field": field, "minimum": minimum, "edit": edit}),
                Some(file_sot(source)),
            )
        })
}

fn parse_governed_command(command: &str) -> Result<Vec<String>> {
    if command.trim().is_empty()
        || command.contains('\0')
        || command.contains('\n')
        || command.contains("&&")
        || command.contains("||")
        || command.contains(';')
        || command.contains('|')
        || command.contains('>')
        || command.contains('<')
        || command.contains('`')
        || command.contains("$(")
    {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_COMMAND_DENIED",
            "command is empty or contains shell/chaining metacharacters",
            "arguments.command",
            "submit one allowlisted command without shell syntax",
            json!({"command": command}),
            None,
        ));
    }
    let argv = command
        .split_whitespace()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if argv.is_empty() || !is_command_allowed(&argv) {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_COMMAND_DENIED",
            "requested command is not in the governed allowlist",
            "arguments.command",
            "use cargo fmt/check/test/build/clippy, rustfmt, git status/diff/log/show, or nvidia-smi",
            json!({"command": command, "argv": argv}),
            None,
        ));
    }
    Ok(argv)
}

fn is_command_allowed(argv: &[String]) -> bool {
    match argv.first().map(String::as_str) {
        Some("cargo") => matches!(
            argv.get(1).map(String::as_str),
            Some("fmt" | "check" | "test" | "build" | "clippy")
        ),
        Some("rustfmt") => true,
        Some("git") => matches!(
            argv.get(1).map(String::as_str),
            Some("status" | "diff" | "log" | "show")
        ),
        Some("nvidia-smi") => true,
        _ => false,
    }
}

fn run_required_process(
    args: &[&str],
    cwd: &Path,
    timeout: u64,
    code: &'static str,
    field_path: &'static str,
) -> Result<Value> {
    let command = args.iter().map(|item| item.to_string()).collect::<Vec<_>>();
    let result = run_cmd(&command, cwd, timeout)?;
    if command_exit_code(&result) != Some(0) {
        return Err(CCRealityError::new(
            code,
            "required command failed",
            field_path,
            "inspect command stdout/stderr before continuing",
            json!({"process": result}),
            Some(file_sot(cwd)),
        ));
    }
    Ok(result)
}

fn run_cmd(command: &[String], cwd: &Path, timeout_secs: u64) -> Result<Value> {
    if command.is_empty() {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_PROCESS_COMMAND_EMPTY",
            "cannot spawn an empty command",
            "process.command",
            "provide an argv vector with a program name",
            json!({}),
            None,
        ));
    }
    let started = Instant::now();
    let output_dir = std::env::temp_dir().join(format!(
        "contextgraph-mcp-harness-run-cmd-{}-{}",
        std::process::id(),
        unix_nanos()?
    ));
    fs::create_dir_all(&output_dir).map_err(|err| {
        fs_error(
            "CCREALITY_HARNESS_PROCESS_OUTPUT_DIR_CREATE_FAILED",
            &output_dir,
            err,
        )
    })?;
    let stdout_path = output_dir.join("stdout");
    let stderr_path = output_dir.join("stderr");
    let stdout_file = fs::File::create(&stdout_path).map_err(|err| {
        fs_error(
            "CCREALITY_HARNESS_PROCESS_STDOUT_CREATE_FAILED",
            &stdout_path,
            err,
        )
    })?;
    let stderr_file = fs::File::create(&stderr_path).map_err(|err| {
        fs_error(
            "CCREALITY_HARNESS_PROCESS_STDERR_CREATE_FAILED",
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
        .process_group(0);
    let mut child = process.spawn().map_err(|err| {
        CCRealityError::new(
            "CCREALITY_HARNESS_PROCESS_SPAWN_FAILED",
            "failed to spawn child process",
            "process.spawn",
            "inspect command path and permissions",
            json!({"command": command, "cwd": cwd.display().to_string(), "error": err.to_string()}),
            Some(file_sot(cwd)),
        )
    })?;
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        if child
            .try_wait()
            .map_err(|err| {
                CCRealityError::new(
                    "CCREALITY_HARNESS_PROCESS_WAIT_FAILED",
                    "failed while waiting for child process",
                    "process.wait",
                    "inspect process state",
                    json!({"command": command, "error": err.to_string()}),
                    None,
                )
            })?
            .is_some()
        {
            let status = child.wait().map_err(|err| {
                CCRealityError::new(
                    "CCREALITY_HARNESS_PROCESS_WAIT_FAILED",
                    "failed while finalizing child process wait",
                    "process.wait",
                    "inspect process state",
                    json!({"command": command, "error": err.to_string()}),
                    None,
                )
            })?;
            let stdout = read_process_output(&stdout_path)?;
            let stderr = read_process_output(&stderr_path)?;
            let _ = fs::remove_dir_all(&output_dir);
            return Ok(json!({
                "command": command,
                "cwd": cwd.display().to_string(),
                "exit_code": status.code().unwrap_or(128),
                "stdout": stdout,
                "stderr": stderr,
                "elapsed_ms": started.elapsed().as_millis()
            }));
        }
        if Instant::now() > deadline {
            let pgid_pid = child.id() as i32;
            if pgid_pid > 0 {
                unsafe {
                    libc::kill(-pgid_pid, libc::SIGTERM);
                }
                std::thread::sleep(Duration::from_secs(2));
                unsafe {
                    libc::kill(-pgid_pid, libc::SIGKILL);
                }
            }
            let _ = child.wait();
            let stdout = read_process_output(&stdout_path).unwrap_or_default();
            let stderr = read_process_output(&stderr_path).unwrap_or_default();
            let _ = fs::remove_dir_all(&output_dir);
            return Ok(json!({
                "command": command,
                "cwd": cwd.display().to_string(),
                "exit_code": 124,
                "stdout": stdout,
                "stderr": if stderr.is_empty() {
                    format!("TIMEOUT after {timeout_secs}s")
                } else {
                    format!("{stderr}\nTIMEOUT after {timeout_secs}s")
                },
                "elapsed_ms": started.elapsed().as_millis()
            }));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn command_exit_code(value: &Value) -> Option<i64> {
    value
        .pointer("/process/exit_code")
        .or_else(|| value.get("exit_code"))
        .and_then(Value::as_i64)
}

fn read_process_output(path: &Path) -> Result<String> {
    let bytes = fs::read(path)
        .map_err(|err| fs_error("CCREALITY_HARNESS_PROCESS_OUTPUT_READ_FAILED", path, err))?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn unix_nanos() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| {
            CCRealityError::new(
                "CCREALITY_HARNESS_TIME_FAILED",
                "system time was before UNIX_EPOCH",
                "time",
                "fix system clock",
                json!({"error": err.to_string()}),
                None,
            )
        })?
        .as_nanos())
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

fn tail(text: &str, limit: usize) -> String {
    let len = text.chars().count();
    if len <= limit {
        text.to_string()
    } else {
        text.chars().skip(len - limit).collect()
    }
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
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_READ_COMMAND_FAILED",
            format!(
                "git read command exited nonzero: {:?}",
                output.status.code()
            ),
            "harness.git",
            "inspect git stderr and fix the requested path or repository state",
            json!({
                "command": ["git"],
                "args": args,
                "project_root": root.display().to_string(),
                "exit_code": output.status.code(),
                "stdout_tail": tail(&stdout, 4096),
                "stderr_tail": tail(&stderr, 4096)
            }),
            Some(file_sot(&root)),
        ));
    }
    Ok((stdout, stderr))
}

fn changed_files_from_diff(diff: &str) -> Vec<String> {
    diff.lines()
        .filter_map(|line| line.strip_prefix("diff --git a/"))
        .filter_map(|rest| rest.split(" b/").next())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn harness_verify_state_rejects_unknown_scope_before_runtime_lookup() {
        let err = harness_verify_state(json!({"scope": "quiet"}))
            .await
            .expect_err("invalid scope must fail");
        assert_eq!(err.error_code, "CCREALITY_HARNESS_VERIFY_SCOPE_INVALID");
    }
}
