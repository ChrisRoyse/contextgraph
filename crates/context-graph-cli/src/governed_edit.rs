//! Governed-edit helpers for ccreality harness changes.
//!
//! This module is provider-neutral: it does not know about any outer model
//! vendor. It opens project file windows, applies SHA-guarded line edits,
//! runs a small allowlist of project commands, and writes readback artifacts
//! that callers can inspect as source of truth.

use serde::{Serialize, Serializer};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
struct GovernedEditErrorRepr {
    pub status: &'static str,
    pub error_code: String,
    pub message: String,
    pub field_path: String,
    pub remediation: String,
    pub details: Value,
    pub source_of_truth: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GovernedEditError(Box<GovernedEditErrorRepr>);

impl Serialize for GovernedEditError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl GovernedEditError {
    fn new(
        error_code: impl Into<String>,
        message: impl Into<String>,
        field_path: impl Into<String>,
        remediation: impl Into<String>,
        details: Value,
        source_of_truth: Option<String>,
    ) -> Self {
        Self(Box::new(GovernedEditErrorRepr {
            status: "error",
            error_code: error_code.into(),
            message: message.into(),
            field_path: field_path.into(),
            remediation: remediation.into(),
            details,
            source_of_truth,
        }))
    }

    #[cfg(test)]
    fn error_code(&self) -> &str {
        &self.0.error_code
    }
}

pub type Result<T> = std::result::Result<T, GovernedEditError>;

#[derive(Debug, Clone, Copy)]
pub enum VerifyScope {
    MejepaLoopOnly,
    Full,
}

#[derive(Debug, Clone, Serialize)]
struct ProcessResult {
    command: Vec<String>,
    cwd: String,
    exit_code: i32,
    stdout: String,
    stderr: String,
    elapsed_ms: u128,
}

pub fn project_root() -> Result<PathBuf> {
    let root = std::env::current_dir().map_err(|err| {
        GovernedEditError::new(
            "RUST_REALITY_GOVERNED_PROJECT_CWD_FAILED",
            "failed to read project current directory",
            "process.cwd",
            "run from the ContextGraph repository root",
            json!({"error": err.to_string()}),
            None,
        )
    })?;
    let cargo = root.join("Cargo.toml");
    require_file(
        &cargo,
        "project_root.Cargo.toml",
        "RUST_REALITY_GOVERNED_PROJECT_ROOT_INVALID",
    )?;
    Ok(root)
}

/// Validate a project-relative path for the harness write tools.
///
/// **GOVERNED-PATHS GATE FULLY REMOVED 2026-05-07** (operator directive: "remove all
/// those governed paths. i need them all gone. i need you to have no restrictions.
/// the ai needs to be able to do anything it needs to do"). The CLI engine no longer
/// consults the allow/deny policy — it accepts any project-relative path that passes
/// the basic shape check in `validate_relative_path` (non-empty, no NUL/newline,
/// no absolute prefix, no `..` traversal). Those remaining rules are FUNCTIONAL —
/// the harness tools integrate with `git apply --check` and assume project-relative
/// paths; absolute paths or `..` would error downstream regardless.
///
/// Path discipline (don't touch unrelated sensitive files) is now behavioral and
/// documented outside this helper. The helper only enforces project-relative
/// filesystem invariants required by the edit/readback implementation.
pub fn validate_governed_path(path: &str) -> Result<String> {
    validate_relative_path(path)
}

pub fn project_file_window(
    project_root: &Path,
    relative_path: &str,
    start_line: u64,
    end_line: u64,
) -> Result<Value> {
    if start_line < 1 {
        return Err(GovernedEditError::new(
            "RUST_REALITY_GOVERNED_PROJECT_WINDOW_START_INVALID",
            "start_line must be at least 1",
            "arguments.start_line",
            "open a window using one-indexed inclusive line numbers",
            json!({"start_line": start_line, "end_line": end_line}),
            None,
        ));
    }
    if end_line < start_line {
        return Err(GovernedEditError::new(
            "RUST_REALITY_GOVERNED_PROJECT_WINDOW_END_INVALID",
            "end_line must be greater than or equal to start_line",
            "arguments.end_line",
            "open a window using an inclusive range with end_line >= start_line",
            json!({"start_line": start_line, "end_line": end_line}),
            None,
        ));
    }

    let path = validate_governed_path(relative_path)?;
    let full_path = project_root.join(&path);
    ensure_inside_root(project_root, &full_path, "arguments.path")?;
    if !full_path.is_file() {
        return Err(GovernedEditError::new(
            "RUST_REALITY_GOVERNED_PROJECT_FILE_MISSING",
            "requested project file does not exist",
            "arguments.path",
            "open a real text file from the governed project tree",
            json!({"path": path, "source_of_truth": file_sot(&full_path)}),
            Some(file_sot(&full_path)),
        ));
    }

    let text = fs::read_to_string(&full_path).map_err(|err| {
        fs_error(
            "RUST_REALITY_GOVERNED_PROJECT_FILE_READ_FAILED",
            &full_path,
            err,
        )
    })?;
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

pub fn project_apply_line_window_edits(
    project_root: &Path,
    tool_dir: &Path,
    edits: &[Value],
    timeout_secs: u64,
) -> Result<Value> {
    if edits.is_empty() {
        return Err(GovernedEditError::new(
            "RUST_REALITY_GOVERNED_PROJECT_EDIT_EMPTY",
            "at least one line-window edit is required",
            "governed_edits",
            "open a governed file window, then submit an edit with observed_sha256",
            json!({"edits": edits}),
            None,
        ));
    }

    create_dir_checked(tool_dir)?;
    let edits_path = tool_dir.join("project-line-window-edits.json");
    write_json_checked(&edits_path, &edits)?;
    let before_status = run_required(
        &["git", "status", "--porcelain"],
        project_root,
        timeout_secs,
        "RUST_REALITY_GOVERNED_PROJECT_STATUS_FAILED",
        "project.git_status.before_edit",
    )?;

    let mut applied = Vec::new();
    for (index, edit) in edits.iter().enumerate() {
        let field_prefix = format!("governed_edits[{index}]");
        let path = required_str(edit, "path", &field_prefix, &edits_path)?;
        let path = validate_governed_path(path)?;
        let start_line = required_u64_at_least(edit, "start_line", 1, &field_prefix, &edits_path)?;
        let end_line =
            required_u64_at_least(edit, "end_line", start_line, &field_prefix, &edits_path)?;
        if end_line - start_line > 500 {
            return Err(GovernedEditError::new(
                "RUST_REALITY_GOVERNED_PROJECT_EDIT_RANGE_TOO_LARGE",
                "project edit range is too large",
                format!("{field_prefix}.end_line"),
                "split large project edits into smaller line-window edits",
                json!({"path": path, "start_line": start_line, "end_line": end_line}),
                Some(file_sot(&edits_path)),
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
                return Err(GovernedEditError::new(
                    "RUST_REALITY_GOVERNED_PROJECT_EDIT_SHA_FORMAT_INVALID",
                    "project edit observed_sha256 has an unrecognized format",
                    format!("{field_prefix}.observed_sha256"),
                    "observed_sha256 must be 'sha256:<64-hex>' or bare '<64-hex>' (will be normalized)",
                    json!({"path": path, "received": value}),
                    Some(file_sot(&edits_path)),
                ));
            }
            None => {
                return Err(GovernedEditError::new(
                    "RUST_REALITY_GOVERNED_PROJECT_EDIT_SHA_MISSING",
                    "project edit must include observed_sha256 from project_file_window",
                    format!("{field_prefix}.observed_sha256"),
                    "read the file first, then edit against that exact source reality",
                    json!({"path": path}),
                    Some(file_sot(&edits_path)),
                ));
            }
        };
        let replace = edit.get("replace").and_then(Value::as_str).ok_or_else(|| {
            GovernedEditError::new(
                "RUST_REALITY_GOVERNED_PROJECT_EDIT_REPLACE_MISSING",
                "project edit replacement text is missing",
                format!("{field_prefix}.replace"),
                "provide the exact replacement text for the requested line range",
                json!({"path": path}),
                Some(file_sot(&edits_path)),
            )
        })?;

        let full_path = project_root.join(&path);
        ensure_inside_root(project_root, &full_path, &format!("{field_prefix}.path"))?;
        let before = fs::read_to_string(&full_path).map_err(|err| {
            fs_error(
                "RUST_REALITY_GOVERNED_PROJECT_EDIT_FILE_READ_FAILED",
                &full_path,
                err,
            )
        })?;
        let actual_sha256 = sha256_file(&full_path)?;
        if actual_sha256 != observed_sha256 {
            return Err(GovernedEditError::new(
                "RUST_REALITY_GOVERNED_PROJECT_EDIT_SHA_MISMATCH",
                "project edit observed_sha256 does not match file readback",
                format!("{field_prefix}.observed_sha256"),
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
            return Err(GovernedEditError::new(
                "RUST_REALITY_GOVERNED_PROJECT_EDIT_RANGE_OUT_OF_BOUNDS",
                "project edit range is outside current file bounds",
                format!("{field_prefix}.end_line"),
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
        write_text_checked(&full_path, &after)?;
        let readback = fs::read_to_string(&full_path).map_err(|err| {
            fs_error(
                "RUST_REALITY_GOVERNED_PROJECT_EDIT_READBACK_FAILED",
                &full_path,
                err,
            )
        })?;
        if readback != after {
            return Err(GovernedEditError::new(
                "RUST_REALITY_GOVERNED_PROJECT_EDIT_READBACK_MISMATCH",
                "project edit file readback did not match written content",
                format!("{field_prefix}.readback"),
                "inspect filesystem durability before continuing",
                json!({
                    "path": path,
                    "expected_sha256": sha256_text(&after),
                    "actual_sha256": sha256_text(&readback)
                }),
                Some(file_sot(&full_path)),
            ));
        }
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
            "source_of_truth": file_sot(&full_path)
        }));
    }

    let diff = run_required(
        &["git", "diff", "--no-ext-diff", "--binary"],
        project_root,
        timeout_secs,
        "RUST_REALITY_GOVERNED_PROJECT_DIFF_FAILED",
        "project.git_diff",
    )?;
    let diff_path = tool_dir.join("project-diff-after-edit.patch");
    write_text_checked(&diff_path, &diff.stdout)?;
    let changed_files = applied
        .iter()
        .filter_map(|entry| {
            entry
                .get("path")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();
    let file_readbacks = changed_files
        .iter()
        .map(|path| {
            let full_path = project_root.join(path);
            Ok(json!({
                "path": path,
                "sha256": sha256_file(&full_path)?,
                "size_bytes": fs::metadata(&full_path)
                    .map_err(|err| fs_error("RUST_REALITY_GOVERNED_PROJECT_METADATA_FAILED", &full_path, err))?
                    .len(),
                "source_of_truth": file_sot(&full_path)
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    let readbacks_path = tool_dir.join("project-file-readbacks-after-edit.json");
    write_json_checked(&readbacks_path, &file_readbacks)?;
    let after_status = run_required(
        &["git", "status", "--porcelain"],
        project_root,
        timeout_secs,
        "RUST_REALITY_GOVERNED_PROJECT_STATUS_FAILED",
        "project.git_status.after_edit",
    )?;
    let status_path = tool_dir.join("project-status-after-edit.txt");
    write_text_checked(&status_path, &after_status.stdout)?;
    let result_path = tool_dir.join("project-line-window-edit-result.json");
    let payload = json!({
        "record_kind": "contextgraph_governed_project_edit_result",
        "status": "ok",
        "action": "edited real ContextGraph project files through governed line-window edits",
        "applied_edits": applied,
        "changed_files": changed_files,
        "file_readbacks": file_readbacks,
        "before_git_status": before_status.stdout,
        "after_git_status": after_status.stdout,
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
        return Err(GovernedEditError::new(
            "RUST_REALITY_GOVERNED_PROJECT_EDIT_RESULT_READBACK_MISMATCH",
            "project edit result artifact readback did not match written payload",
            "governed_edits.result_readback",
            "inspect filesystem durability before using edit result",
            json!({"expected": payload, "actual": readback}),
            Some(file_sot(&result_path)),
        ));
    }
    Ok(readback)
}

pub fn run_governed_command(
    project_root: &Path,
    tool_dir: &Path,
    command: &str,
    timeout_secs: u64,
) -> Result<Value> {
    let argv = parse_governed_command(command)?;
    create_dir_checked(tool_dir)?;
    let result = run_cmd(&argv, project_root, timeout_secs)?;
    let safe_name = safe_id(command);
    let output_path = tool_dir.join(format!("project-command-{safe_name}.json"));
    let payload = json!({
        "record_kind": "contextgraph_governed_project_command_result",
        "status": if result.exit_code == 0 { "ok" } else { "failed" },
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
        return Err(GovernedEditError::new(
            "RUST_REALITY_GOVERNED_PROJECT_COMMAND_READBACK_MISMATCH",
            "project command artifact readback did not match written payload",
            "governed_command.readback",
            "inspect filesystem durability before using command result",
            json!({"expected": payload, "actual": readback}),
            Some(file_sot(&output_path)),
        ));
    }
    Ok(readback)
}

pub fn verify_project_state(
    project_root: &Path,
    tool_dir: &Path,
    label: &str,
    scope: VerifyScope,
) -> Result<Value> {
    let verify_dir = tool_dir.join(format!("project-verification-{}", safe_id(label)));
    create_dir_checked(&verify_dir)?;
    let rustfmt_command = match scope {
        VerifyScope::MejepaLoopOnly => "cargo fmt --check -p context-graph-cli",
        VerifyScope::Full => "cargo fmt --check",
    };
    let check_command = match scope {
        VerifyScope::MejepaLoopOnly => {
            "cargo check -p context-graph-cli --no-default-features --lib"
        }
        VerifyScope::Full => "cargo check -p context-graph-cli",
    };
    let test_command = match scope {
        VerifyScope::MejepaLoopOnly => "cargo test -p context-graph-cli --no-default-features --lib -- --skip ignored --test-threads=1",
        VerifyScope::Full => "cargo test -p context-graph-cli -- --skip ignored --test-threads=1",
    };

    let rustfmt = run_governed_command(project_root, &verify_dir, rustfmt_command, 120)?;
    let cargo_check = run_governed_command(project_root, &verify_dir, check_command, 900)?;
    let cargo_test = run_governed_command(project_root, &verify_dir, test_command, 1200)?;
    let git_status = run_governed_command(project_root, &verify_dir, "git status --short", 120)?;
    let git_diff_stat = run_governed_command(project_root, &verify_dir, "git diff --stat", 120)?;
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
        "scope": match scope {
            VerifyScope::MejepaLoopOnly => "mejepa_loop_only",
            VerifyScope::Full => "full",
        },
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
        return Err(GovernedEditError::new(
            "RUST_REALITY_GOVERNED_PROJECT_VERIFY_READBACK_MISMATCH",
            "project state verification readback did not match written payload",
            "governed_project_verification.readback",
            "inspect filesystem durability before continuing",
            json!({"expected": payload, "actual": readback}),
            Some(file_sot(&path)),
        ));
    }
    if readback.get("status").and_then(Value::as_str) != Some("ok") {
        return Err(GovernedEditError::new(
            "RUST_REALITY_GOVERNED_PROJECT_VERIFY_FAILED",
            "governed project verification failed",
            "governed_project_verification",
            "inspect verification command artifacts and fix compile or test errors",
            json!({"verification": file_sot(&path)}),
            Some(file_sot(&path)),
        ));
    }
    Ok(readback)
}

fn validate_relative_path(path: &str) -> Result<String> {
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
        return Err(GovernedEditError::new(
            "RUST_REALITY_GOVERNED_PROJECT_PATH_INVALID",
            "project path must be repo-relative without traversal, absolute roots, or control characters",
            "arguments.path",
            "use a normalized relative path inside the ContextGraph project",
            json!({"path": path}),
            None,
        ));
    }
    Ok(path.to_string())
}

fn ensure_inside_root(project_root: &Path, path: &Path, field_path: &str) -> Result<()> {
    let root = project_root.canonicalize().map_err(|err| {
        fs_error(
            "RUST_REALITY_GOVERNED_PROJECT_ROOT_CANONICALIZE_FAILED",
            project_root,
            err,
        )
    })?;
    let target_parent = path
        .parent()
        .ok_or_else(|| {
            GovernedEditError::new(
                "RUST_REALITY_GOVERNED_PROJECT_PATH_PARENT_MISSING",
                "project path has no parent directory",
                field_path,
                "use a normal project file path",
                json!({"path": path_str(path)}),
                Some(file_sot(path)),
            )
        })?
        .canonicalize()
        .map_err(|err| {
            fs_error(
                "RUST_REALITY_GOVERNED_PROJECT_PATH_CANONICALIZE_FAILED",
                path,
                err,
            )
        })?;
    if !target_parent.starts_with(&root) {
        return Err(GovernedEditError::new(
            "RUST_REALITY_GOVERNED_PROJECT_PATH_ESCAPES_ROOT",
            "project path resolves outside the project root",
            field_path,
            "use a path inside the current ContextGraph checkout",
            json!({"root": path_str(&root), "path": path_str(path), "parent": path_str(&target_parent)}),
            Some(file_sot(path)),
        ));
    }
    Ok(())
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
        return Err(GovernedEditError::new(
            "RUST_REALITY_GOVERNED_COMMAND_DENIED",
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
        return Err(GovernedEditError::new(
            "RUST_REALITY_GOVERNED_COMMAND_DENIED",
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

fn command_exit_code(value: &Value) -> Option<i64> {
    value.pointer("/process/exit_code").and_then(Value::as_i64)
}

fn required_str<'a>(
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
            GovernedEditError::new(
                "RUST_REALITY_GOVERNED_PROJECT_EDIT_FIELD_MISSING",
                "required edit field is missing or empty",
                format!("{field_prefix}.{field}"),
                "provide all required line-window edit fields",
                json!({"field": field, "edit": edit}),
                Some(file_sot(source)),
            )
        })
}

fn required_u64_at_least(
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
            GovernedEditError::new(
                "RUST_REALITY_GOVERNED_PROJECT_EDIT_FIELD_INVALID",
                "required numeric edit field is missing or too small",
                format!("{field_prefix}.{field}"),
                "use current one-indexed line numbers from project_file_window",
                json!({"field": field, "minimum": minimum, "edit": edit}),
                Some(file_sot(source)),
            )
        })
}

fn run_required(
    args: &[&str],
    cwd: &Path,
    timeout: u64,
    code: &str,
    field_path: &str,
) -> Result<ProcessResult> {
    let command = args.iter().map(|item| item.to_string()).collect::<Vec<_>>();
    let result = run_cmd(&command, cwd, timeout)?;
    if result.exit_code != 0 {
        return Err(GovernedEditError::new(
            code,
            "required command failed",
            field_path,
            "inspect command stdout/stderr before continuing",
            json!({"process": result}),
            None,
        ));
    }
    Ok(result)
}

fn run_cmd(command: &[String], cwd: &Path, timeout_secs: u64) -> Result<ProcessResult> {
    if command.is_empty() {
        return Err(GovernedEditError::new(
            "RUST_REALITY_GOVERNED_PROCESS_COMMAND_EMPTY",
            "cannot spawn an empty command",
            "process.command",
            "provide an argv vector with a program name",
            json!({}),
            None,
        ));
    }
    let started = Instant::now();
    let output_dir = std::env::temp_dir().join(format!(
        "contextgraph-governed-run-cmd-{}-{}",
        std::process::id(),
        unix_ns()?
    ));
    create_dir_checked(&output_dir)?;
    let stdout_path = output_dir.join("stdout");
    let stderr_path = output_dir.join("stderr");
    let stdout_file = fs::File::create(&stdout_path).map_err(|err| {
        fs_error(
            "RUST_REALITY_GOVERNED_PROCESS_STDOUT_FILE_CREATE_FAILED",
            &stdout_path,
            err,
        )
    })?;
    let stderr_file = fs::File::create(&stderr_path).map_err(|err| {
        fs_error(
            "RUST_REALITY_GOVERNED_PROCESS_STDERR_FILE_CREATE_FAILED",
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
        // Child becomes its own process-group leader so timeout-kill can reap the
        // whole subtree (e.g., a `bash -c "cargo build && cargo test"` spawns rustc
        // workers; SIGKILL on the bash alone leaves rustc orphaned).
        .process_group(0);
    let mut child = process.spawn().map_err(|err| {
        GovernedEditError::new(
            "RUST_REALITY_GOVERNED_PROCESS_SPAWN_FAILED",
            "failed to spawn child process",
            "process.spawn",
            "inspect command path and permissions",
            json!({"command": command, "cwd": path_str(cwd), "error": err.to_string()}),
            None,
        )
    })?;
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        if let Some(_status) = child.try_wait().map_err(|err| {
            GovernedEditError::new(
                "RUST_REALITY_GOVERNED_PROCESS_WAIT_FAILED",
                "failed while waiting for child process",
                "process.wait",
                "inspect process state",
                json!({"command": command, "error": err.to_string()}),
                None,
            )
        })? {
            let status = child.wait().map_err(|err| {
                GovernedEditError::new(
                    "RUST_REALITY_GOVERNED_PROCESS_WAIT_FAILED",
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
            // SIGTERM the entire process group; wait 2s; SIGKILL the group.
            // process_group(0) at spawn ensured pgid == child.id().
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
    let bytes = fs::read(path).map_err(|err| {
        fs_error(
            "RUST_REALITY_GOVERNED_PROCESS_OUTPUT_READ_FAILED",
            path,
            err,
        )
    })?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn read_json<T: for<'de> serde::Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path)
        .map_err(|err| fs_error("RUST_REALITY_GOVERNED_JSON_READ_FAILED", path, err))?;
    serde_json::from_str(&text).map_err(|err| {
        GovernedEditError::new(
            "RUST_REALITY_GOVERNED_JSON_PARSE_FAILED",
            "JSON file could not be parsed",
            "json",
            "inspect JSON file syntax",
            json!({"path": path_str(path), "error": err.to_string()}),
            Some(file_sot(path)),
        )
    })
}

fn write_json_checked(path: &Path, value: &impl Serialize) -> Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(|err| {
        GovernedEditError::new(
            "RUST_REALITY_GOVERNED_JSON_SERIALIZE_FAILED",
            "failed to serialize JSON payload",
            "json.serialize",
            "inspect payload for non-serializable values",
            json!({"path": path_str(path), "error": err.to_string()}),
            Some(file_sot(path)),
        )
    })?;
    write_text_checked(path, &format!("{text}\n"))
}

fn write_text_checked(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_checked(parent)?;
    }
    let mut file = fs::File::create(path)
        .map_err(|err| fs_error("RUST_REALITY_GOVERNED_TEXT_WRITE_CREATE_FAILED", path, err))?;
    file.write_all(text.as_bytes())
        .map_err(|err| fs_error("RUST_REALITY_GOVERNED_TEXT_WRITE_FAILED", path, err))?;
    file.sync_all()
        .map_err(|err| fs_error("RUST_REALITY_GOVERNED_TEXT_SYNC_FAILED", path, err))?;
    let readback = fs::read_to_string(path)
        .map_err(|err| fs_error("RUST_REALITY_GOVERNED_TEXT_READBACK_FAILED", path, err))?;
    if readback != text {
        return Err(GovernedEditError::new(
            "RUST_REALITY_GOVERNED_TEXT_READBACK_MISMATCH",
            "text file readback did not match written content",
            "filesystem.readback",
            "inspect filesystem durability before continuing",
            json!({
                "path": path_str(path),
                "expected_sha256": sha256_text(text),
                "actual_sha256": sha256_text(&readback)
            }),
            Some(file_sot(path)),
        ));
    }
    Ok(())
}

fn create_dir_checked(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .map_err(|err| fs_error("RUST_REALITY_GOVERNED_DIR_CREATE_FAILED", path, err))
}

fn require_file(path: &Path, field: &str, code: &str) -> Result<()> {
    if !path.is_file() {
        return Err(GovernedEditError::new(
            code,
            "required file is missing",
            field,
            "provide a real readable file before continuing",
            json!({"path": path_str(path)}),
            Some(file_sot(path)),
        ));
    }
    Ok(())
}

fn sha256_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)
        .map_err(|err| fs_error("RUST_REALITY_GOVERNED_SHA256_READ_FAILED", path, err))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn unix_ns() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| {
            GovernedEditError::new(
                "RUST_REALITY_GOVERNED_TIME_FAILED",
                "system time was before UNIX_EPOCH",
                "time",
                "fix system clock",
                json!({"error": err.to_string()}),
                None,
            )
        })?
        .as_nanos())
}

fn fs_error(code: &str, path: &Path, err: std::io::Error) -> GovernedEditError {
    GovernedEditError::new(
        code,
        "filesystem operation failed",
        "filesystem",
        "inspect path, permissions, and disk space",
        json!({"path": path_str(path), "error": err.to_string()}),
        Some(file_sot(path)),
    )
}

fn path_str(path: &Path) -> &str {
    path.to_str().unwrap_or("")
}

fn file_sot(path: &Path) -> String {
    format!("file:{}", path_str(path))
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

    fn init_real_governed_git_repo() -> (TempDir, PathBuf) {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("repo");
        create_dir_checked(&root).expect("repo dir");
        run_required(
            &["git", "init"],
            &root,
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
            &root,
            30,
            "TEST_GIT_CONFIG_EMAIL_FAILED",
            "test.git.config.email",
        )
        .expect("git config email");
        run_required(
            &["git", "config", "user.name", "Reality Loop FSV"],
            &root,
            30,
            "TEST_GIT_CONFIG_NAME_FAILED",
            "test.git.config.name",
        )
        .expect("git config name");
        write_text_checked(&root.join("Cargo.toml"), "[workspace]\n").expect("cargo write");
        create_dir_checked(&root.join("docs/ccreality")).expect("docs dir");
        write_text_checked(&root.join("docs/ccreality/example.md"), "alpha\nbeta\n")
            .expect("doc write");
        run_required(
            &["git", "add", "Cargo.toml", "docs/ccreality/example.md"],
            &root,
            30,
            "TEST_GIT_ADD_FAILED",
            "test.git.add",
        )
        .expect("git add");
        run_required(
            &["git", "commit", "-m", "base"],
            &root,
            30,
            "TEST_GIT_COMMIT_FAILED",
            "test.git.commit",
        )
        .expect("git commit");
        (temp, root)
    }

    #[test]
    #[serial]
    fn project_edit_tool_updates_real_file_and_reads_back_diff() {
        let (_temp, root) = init_real_governed_git_repo();
        let source = root.join("docs/ccreality/example.md");
        let window = project_file_window(&root, "docs/ccreality/example.md", 1, 2)
            .expect("project file window");
        let before_sha = window["sha256"].as_str().expect("sha").to_string();
        let tool_dir = root.join("attempt/governed-tool");
        let edits = vec![json!({
            "path": "docs/ccreality/example.md",
            "start_line": 2,
            "end_line": 2,
            "replace": "gamma\n",
            "observed_sha256": before_sha
        })];
        eprintln!(
            "BEFORE governed_project_edit source_of_truth={} data={:?}",
            file_sot(&source),
            fs::read_to_string(&source).unwrap()
        );
        let result =
            project_apply_line_window_edits(&root, &tool_dir, &edits, 30).expect("project edit");
        let after = fs::read_to_string(&source).expect("after");
        let diff = fs::read_to_string(tool_dir.join("project-diff-after-edit.patch"))
            .expect("diff readback");
        let result_readback: Value =
            read_json(&tool_dir.join("project-line-window-edit-result.json"))
                .expect("result readback");
        eprintln!(
            "AFTER governed_project_edit result={} data={:?} diff={:?}",
            serde_json::to_string_pretty(&result_readback).unwrap(),
            after,
            diff
        );
        assert_eq!(after, "alpha\ngamma\n");
        assert_eq!(result, result_readback);
        assert_eq!(result["status"], "ok");
        assert!(diff.contains("-beta"));
        assert!(diff.contains("+gamma"));
        assert!(result["source_of_truth"]["diff"]
            .as_str()
            .unwrap()
            .starts_with("file:"));
    }

    #[test]
    #[serial]
    fn project_edit_rejects_stale_observed_sha256() {
        let (_temp, root) = init_real_governed_git_repo();
        let source = root.join("docs/ccreality/example.md");
        let before = fs::read_to_string(&source).expect("before");
        let tool_dir = root.join("attempt/governed-tool");
        let edits = vec![json!({
            "path": "docs/ccreality/example.md",
            "start_line": 2,
            "end_line": 2,
            "replace": "gamma\n",
            "observed_sha256": "sha256:stale"
        })];
        eprintln!(
            "BEFORE governed_project_stale source_of_truth={} actual_sha={} data={:?}",
            file_sot(&source),
            sha256_file(&source).unwrap(),
            before
        );
        let err = project_apply_line_window_edits(&root, &tool_dir, &edits, 30)
            .expect_err("stale sha must fail");
        let after = fs::read_to_string(&source).expect("after");
        eprintln!(
            "AFTER governed_project_stale error={} data={:?}",
            serde_json::to_string_pretty(&err).unwrap(),
            after
        );
        assert_eq!(before, after);
        assert_eq!(
            err.error_code(),
            "RUST_REALITY_GOVERNED_PROJECT_EDIT_SHA_MISMATCH"
        );
    }

    #[test]
    #[serial]
    fn governed_path_validation_allows_project_relative_paths_and_blocks_traversal() {
        let secret = validate_governed_path(".env").expect(".env is a valid relative path");
        let traversal = validate_governed_path("../Cargo.toml").expect_err("traversal blocked");
        let source = validate_governed_path("crates/context-graph-cli/src/bin/reality-loop.rs")
            .expect("source path allowed");
        eprintln!(
            "PATH_VALIDATION secret={} traversal={} allowed={}",
            secret,
            serde_json::to_string_pretty(&traversal).unwrap(),
            source
        );
        assert_eq!(secret, ".env");
        assert_eq!(
            traversal.error_code(),
            "RUST_REALITY_GOVERNED_PROJECT_PATH_INVALID"
        );
    }

    #[test]
    #[serial]
    fn governed_path_validation_has_no_policy_env_dependency() {
        let source = validate_governed_path("Cargo.toml").expect("policy env is not required");
        eprintln!("PATH_VALIDATION_NO_POLICY allowed={source}");
        assert_eq!(source, "Cargo.toml");
    }

    #[test]
    #[serial]
    fn governed_command_runs_allowlisted_command_and_writes_readback() {
        let (_temp, root) = init_real_governed_git_repo();
        let tool_dir = root.join("attempt/governed-command");
        let result =
            run_governed_command(&root, &tool_dir, "git status --short", 30).expect("git status");
        let output = tool_dir.join("project-command-git-status---short.json");
        let readback: Value = read_json(&output).expect("command readback");
        eprintln!(
            "COMMAND_READBACK source_of_truth={} payload={}",
            file_sot(&output),
            serde_json::to_string_pretty(&readback).unwrap()
        );
        assert_eq!(result, readback);
        assert_eq!(readback["status"], "ok");
        assert_eq!(readback["process"]["exit_code"], 0);
    }

    #[test]
    #[serial]
    fn governed_command_denies_shell_metachar_bypass() {
        let (_temp, root) = init_real_governed_git_repo();
        let marker = root.join("should-not-exist");
        let command = format!("cargo build && touch {}", path_str(&marker));
        let err = run_governed_command(&root, &root.join("attempt/denied"), &command, 30)
            .expect_err("chained command must be denied");
        eprintln!(
            "DENIED_COMMAND error={} marker_exists={}",
            serde_json::to_string_pretty(&err).unwrap(),
            marker.exists()
        );
        assert_eq!(err.error_code(), "RUST_REALITY_GOVERNED_COMMAND_DENIED");
        assert!(!marker.exists());
    }
}
