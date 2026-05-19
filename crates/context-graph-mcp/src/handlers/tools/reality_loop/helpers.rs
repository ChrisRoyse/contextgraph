use super::errors::{CCRealityError, Result};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::ErrorKind;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn read_active_runtime_root() -> Result<Option<PathBuf>> {
    let path = context_graph_paths::cgreality_cache_file("active_runtime_root").map_err(|e| {
        CCRealityError::new(
            e.code,
            e.message,
            "active_runtime_root.path",
            e.remediation,
            json!({"data_root_env": context_graph_paths::ENV_DATA_ROOT}),
            None,
        )
    })?;
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|e| {
        CCRealityError::new(
            "CCREALITY_ACTIVE_ROOT_READ_FAILED",
            format!("failed to read active runtime root: {e}"),
            "active_runtime_root.path",
            "ensure /zfs/archive/contextgraph/state/cgreality is readable",
            json!({"path": path.display().to_string()}),
            Some(file_sot(&path)),
        )
    })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(PathBuf::from(trimmed)))
    }
}

pub async fn read_active_target_instance() -> Result<Option<String>> {
    let path =
        context_graph_paths::cgreality_cache_file("active_target_instance").map_err(|e| {
            CCRealityError::new(
                e.code,
                e.message,
                "active_target_instance.path",
                e.remediation,
                json!({"data_root_env": context_graph_paths::ENV_DATA_ROOT}),
                None,
            )
        })?;
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|e| {
        CCRealityError::new(
            "CCREALITY_ACTIVE_INSTANCE_READ_FAILED",
            format!("failed to read active target instance: {e}"),
            "active_target_instance.path",
            "ensure /zfs/archive/contextgraph/state/cgreality is readable",
            json!({"path": path.display().to_string()}),
            Some(file_sot(&path)),
        )
    })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

pub async fn require_active_runtime_root() -> Result<PathBuf> {
    read_active_runtime_root().await?.ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_NO_ACTIVE_RUNTIME_ROOT",
            "no active runtime root has been recorded",
            "active_runtime_root",
            "the legacy reality-loop runner is retired; use ME-JEPA evidence capture artifacts instead",
            json!({}),
            None,
        )
    })
}

pub async fn require_active_target_instance() -> Result<String> {
    read_active_target_instance().await?.ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_NO_ACTIVE_TARGET_INSTANCE",
            "no active target instance has been recorded",
            "active_target_instance",
            "the legacy reality-loop runner is retired; use the current SWE-bench task/corpus identifier instead",
            json!({}),
            None,
        )
    })
}

pub fn latest_run_id(runtime_root: &Path) -> Result<Option<String>> {
    let mut candidates = Vec::new();
    for entry in fs::read_dir(runtime_root).map_err(|e| {
        CCRealityError::new(
            "CCREALITY_RUNTIME_ROOT_READ_FAILED",
            format!("failed to read runtime root: {e}"),
            "runtime_root.read_dir",
            "ensure the active runtime root exists and is readable",
            json!({"path": runtime_root.display().to_string()}),
            Some(file_sot(runtime_root)),
        )
    })? {
        let entry = entry.map_err(|e| {
            CCRealityError::new(
                "CCREALITY_RUNTIME_ROOT_ENTRY_READ_FAILED",
                format!("failed to read runtime root entry: {e}"),
                "runtime_root.entry",
                "inspect runtime root permissions",
                json!({"path": runtime_root.display().to_string()}),
                Some(file_sot(runtime_root)),
            )
        })?;
        let meta = entry.metadata().map_err(|e| {
            CCRealityError::new(
                "CCREALITY_RUNTIME_ROOT_ENTRY_METADATA_FAILED",
                format!("failed to stat runtime root entry: {e}"),
                "runtime_root.entry.metadata",
                "inspect runtime root permissions",
                json!({"path": entry.path().display().to_string()}),
                Some(file_sot(&entry.path())),
            )
        })?;
        if !meta.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if is_runtime_metadata_dir(&name) {
            continue;
        }
        let modified_at = meta.modified().map_err(|e| {
            CCRealityError::new(
                "CCREALITY_RUNTIME_ROOT_ENTRY_MTIME_FAILED",
                format!("failed to read runtime root entry mtime: {e}"),
                "runtime_root.entry.modified",
                "inspect runtime root filesystem metadata",
                json!({"path": entry.path().display().to_string()}),
                Some(file_sot(&entry.path())),
            )
        })?;
        let modified = modified_at
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                CCRealityError::new(
                    "CCREALITY_RUNTIME_ROOT_ENTRY_MTIME_INVALID",
                    format!("runtime root entry mtime is before UNIX_EPOCH: {e}"),
                    "runtime_root.entry.modified",
                    "repair the runtime root entry timestamp before selecting latest run",
                    json!({"path": entry.path().display().to_string()}),
                    Some(file_sot(&entry.path())),
                )
            })?
            .as_secs();
        candidates.push((modified, name));
    }
    candidates.sort();
    Ok(candidates.into_iter().last().map(|(_, name)| name))
}

pub fn latest_attempt_number(
    runtime_root: &Path,
    target_instance: Option<&str>,
) -> Result<Option<u64>> {
    let Some(run_id) = latest_run_id(runtime_root)? else {
        return Ok(None);
    };
    let run_dir = runtime_root.join(run_id);
    let mut attempts = Vec::new();
    collect_attempt_dirs(&run_dir, target_instance, &mut attempts)?;
    Ok(attempts.into_iter().map(|(n, _)| n).max())
}

pub fn latest_attempt_dir(
    runtime_root: &Path,
    target_instance: Option<&str>,
) -> Result<Option<(String, u64, PathBuf)>> {
    let Some(run_id) = latest_run_id(runtime_root)? else {
        return Ok(None);
    };
    let run_dir = runtime_root.join(&run_id);
    let mut attempts = Vec::new();
    collect_attempt_dirs(&run_dir, target_instance, &mut attempts)?;
    attempts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    Ok(attempts.into_iter().last().map(|(n, p)| (run_id, n, p)))
}

pub fn attempt_dir(
    runtime_root: &Path,
    run_id: &str,
    task_id: &str,
    attempt: u64,
) -> Result<PathBuf> {
    let run_dir = runtime_root.join(run_id);
    let direct = run_dir.join(task_id).join(format!("attempt-{attempt}"));
    if direct.is_dir() {
        return Ok(direct);
    }
    let smoke = run_dir
        .join(task_id)
        .join(format!("smoke-attempt-{attempt}"));
    if smoke.is_dir() {
        return Ok(smoke);
    }
    let mut attempts = Vec::new();
    collect_attempt_dirs(&run_dir, Some(task_id), &mut attempts)?;
    attempts
        .into_iter()
        .find_map(|(n, p)| if n == attempt { Some(p) } else { None })
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_ATTEMPT_DIR_MISSING",
                "could not locate requested attempt directory",
                "arguments.attempt",
                "verify run_id, active target, and attempt number",
                json!({"runtime_root": runtime_root.display().to_string(), "run_id": run_id, "task_id": task_id, "attempt": attempt}),
                Some(file_sot(&run_dir)),
            )
        })
}

fn collect_attempt_dirs(
    root: &Path,
    target_instance: Option<&str>,
    out: &mut Vec<(u64, PathBuf)>,
) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root).map_err(|e| {
        CCRealityError::new(
            "CCREALITY_ATTEMPT_WALK_READ_FAILED",
            format!("failed to read attempt tree: {e}"),
            "attempt_tree.read_dir",
            "inspect runtime artifact permissions",
            json!({"path": root.display().to_string()}),
            Some(file_sot(root)),
        )
    })? {
        let entry = entry.map_err(|e| {
            CCRealityError::new(
                "CCREALITY_ATTEMPT_WALK_ENTRY_FAILED",
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
        if let Some(n) = parse_attempt_name(&name) {
            if target_instance.is_none_or(|target| path.display().to_string().contains(target)) {
                out.push((n, path));
            }
            continue;
        }
        collect_attempt_dirs(&path, target_instance, out)?;
    }
    Ok(())
}

fn parse_attempt_name(name: &str) -> Option<u64> {
    name.strip_prefix("attempt-")
        .or_else(|| name.strip_prefix("smoke-attempt-"))
        .and_then(|suffix| suffix.parse::<u64>().ok())
}

fn is_runtime_metadata_dir(name: &str) -> bool {
    name.starts_with('_')
        || matches!(
            name,
            "failures"
                | "workspaces"
                | "cgreality-shift-log"
                | "cgreality-hook-state"
                | "claude-code-optimizer"
        )
}

pub fn read_json(path: &Path) -> Result<Value> {
    let raw = fs::read_to_string(path).map_err(|e| {
        CCRealityError::new(
            "CCREALITY_ARTIFACT_READ_FAILED",
            format!("failed to read artifact: {e}"),
            "artifact.path",
            "verify the artifact path is correct and readable",
            json!({"path": path.display().to_string()}),
            Some(file_sot(path)),
        )
    })?;
    serde_json::from_str(&raw).map_err(|e| {
        CCRealityError::new(
            "CCREALITY_ARTIFACT_PARSE_FAILED",
            format!("artifact is not valid JSON: {e}"),
            "artifact.parse",
            "inspect artifact for corruption",
            json!({"path": path.display().to_string()}),
            Some(file_sot(path)),
        )
    })
}

pub fn write_json_checked(path: &Path, value: &impl Serialize) -> Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(|e| {
        CCRealityError::new(
            "CCREALITY_JSON_SERIALIZE_FAILED",
            format!("failed to serialize JSON artifact: {e}"),
            "json.serialize",
            "fix the artifact payload",
            json!({"path": path.display().to_string()}),
            Some(file_sot(path)),
        )
    })?;
    write_text_checked(path, &format!("{text}\n"))
}

pub fn write_text_checked(path: &Path, text: &str) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| fs_error("CCREALITY_DIR_CREATE_FAILED", parent, e))?;
    let (tmp_path, mut file) = create_temp_sibling_file(path)?;
    file.write_all(text.as_bytes())
        .map_err(|e| fs_error("CCREALITY_FILE_WRITE_FAILED", &tmp_path, e))?;
    file.sync_all()
        .map_err(|e| fs_error("CCREALITY_FILE_SYNC_FAILED", &tmp_path, e))?;
    drop(file);
    let temp_readback = fs::read_to_string(&tmp_path)
        .map_err(|e| fs_error("CCREALITY_FILE_READBACK_FAILED", &tmp_path, e))?;
    if temp_readback != text {
        let _ = fs::remove_file(&tmp_path);
        return Err(CCRealityError::new(
            "CCREALITY_FILE_READBACK_MISMATCH",
            "temporary file readback did not match written content",
            "filesystem.readback",
            "inspect filesystem durability before continuing",
            json!({"path": tmp_path.display().to_string(), "expected_sha256": sha256_text(text), "actual_sha256": sha256_text(&temp_readback)}),
            Some(file_sot(&tmp_path)),
        ));
    }
    fs::rename(&tmp_path, path).map_err(|e| fs_error("CCREALITY_FILE_RENAME_FAILED", path, e))?;
    sync_dir(parent, "CCREALITY_PARENT_DIR_SYNC_FAILED")?;
    let readback = fs::read_to_string(path)
        .map_err(|e| fs_error("CCREALITY_FILE_READBACK_FAILED", path, e))?;
    if readback != text {
        return Err(CCRealityError::new(
            "CCREALITY_FILE_READBACK_MISMATCH",
            "file readback did not match written content",
            "filesystem.readback",
            "inspect filesystem durability before continuing",
            json!({"path": path.display().to_string(), "expected_sha256": sha256_text(text), "actual_sha256": sha256_text(&readback)}),
            Some(file_sot(path)),
        ));
    }
    Ok(())
}

fn create_temp_sibling_file(path: &Path) -> Result<(PathBuf, fs::File)> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_FILE_NAME_INVALID",
                "artifact path does not have a valid UTF-8 file name",
                "artifact.path",
                "write artifacts to a normal file path with a UTF-8 file name",
                json!({"path": path.display().to_string()}),
                Some(file_sot(path)),
            )
        })?;
    for attempt in 0..1000u32 {
        let candidate = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            attempt
        ));
        match fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(fs_error(
                    "CCREALITY_TEMP_FILE_PROBE_FAILED",
                    &candidate,
                    err,
                ));
            }
        }
    }
    Err(CCRealityError::new(
        "CCREALITY_TEMP_FILE_EXHAUSTED",
        "could not allocate a unique temporary artifact path",
        "artifact.temp_path",
        "inspect stale temporary files in the artifact directory",
        json!({"path": path.display().to_string()}),
        Some(file_sot(parent)),
    ))
}

fn sync_dir(path: &Path, code: &'static str) -> Result<()> {
    let dir = fs::File::open(path).map_err(|e| fs_error(code, path, e))?;
    dir.sync_all().map_err(|e| fs_error(code, path, e))
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|e| fs_error("CCREALITY_SHA256_READ_FAILED", path, e))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

pub fn sha256_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

pub fn file_sot(path: &Path) -> String {
    format!("file:{}", path.display())
}

pub fn file_arg_to_path(path: &str) -> PathBuf {
    PathBuf::from(path.strip_prefix("file:").unwrap_or(path))
}

pub fn required_str(args: &Value, field: &str) -> Result<String> {
    args.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_ARG_MISSING_OR_NOT_STRING",
                format!("argument '{field}' must be a non-empty string"),
                format!("arguments.{field}"),
                format!("provide a non-empty string for arguments.{field}"),
                json!({"args": args}),
                None,
            )
        })
}

pub fn optional_str_strict(args: &Value, field: &str) -> Result<Option<String>> {
    match args.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.is_empty() => Ok(Some(value.clone())),
        Some(value) => Err(CCRealityError::new(
            "CCREALITY_ARG_INVALID_STRING",
            format!("argument '{field}' must be a non-empty string when provided"),
            format!("arguments.{field}"),
            format!("omit arguments.{field} or provide a non-empty string"),
            json!({"value": value}),
            None,
        )),
    }
}

pub fn required_u64(args: &Value, field: &str) -> Result<u64> {
    coerce_u64(args.get(field)).ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_ARG_MISSING_OR_NOT_U64",
            format!("argument '{field}' must be a non-negative integer"),
            format!("arguments.{field}"),
            format!("provide a non-negative integer for arguments.{field}"),
            json!({"args": args}),
            None,
        )
    })
}

pub fn optional_u64_strict(args: &Value, field: &str, default: u64) -> Result<u64> {
    match args.get(field) {
        None | Some(Value::Null) => Ok(default),
        Some(value) => coerce_u64(Some(value)).ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_ARG_INVALID_U64",
                format!("argument '{field}' must be a non-negative integer when provided"),
                format!("arguments.{field}"),
                format!("omit arguments.{field} or provide a non-negative integer"),
                json!({"value": value}),
                None,
            )
        }),
    }
}

pub fn optional_u64_value_strict(args: &Value, field: &str) -> Result<Option<u64>> {
    match args.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => coerce_u64(Some(value)).map(Some).ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_ARG_INVALID_U64",
                format!("argument '{field}' must be a non-negative integer when provided"),
                format!("arguments.{field}"),
                format!("omit arguments.{field} or provide a non-negative integer"),
                json!({"value": value}),
                None,
            )
        }),
    }
}

pub fn optional_bool_strict(args: &Value, field: &str, default: bool) -> Result<bool> {
    match args.get(field) {
        None | Some(Value::Null) => Ok(default),
        Some(value) => coerce_bool(Some(value)).ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_ARG_INVALID_BOOL",
                format!("argument '{field}' must be a boolean when provided"),
                format!("arguments.{field}"),
                format!("omit arguments.{field} or provide true/false"),
                json!({"value": value}),
                None,
            )
        }),
    }
}

/// Accept either a JSON number or a JSON string that parses to a non-negative integer.
/// This coexists with the JSON-RPC client wrapper that occasionally stringifies typed
/// arguments before they reach the MCP tool handler (see Phase 13 task T13.2).
fn coerce_u64(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn coerce_bool(value: Option<&Value>) -> Option<bool> {
    match value? {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// Recursively coerce JSON-stringified primitive/array/object values back to their
/// intrinsic types. Used at the entry of schema-validated handlers (e.g.
/// `optimizer_record_recommendation`) so the recommendation passes validation even
/// when the caller's transport stringified its typed arguments.
///
/// Coercion rules:
/// - leave strings that do not look like a JSON literal alone (preserves natural prose
///   such as `reason`).
/// - if a string parses as JSON AND the result is a non-string type (number, bool,
///   array, object, null), use the parsed value.
/// - traverse into objects and arrays recursively.
pub fn coerce_stringified_args(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, coerce_stringified_args(v)))
                .collect(),
        ),
        Value::Array(items) => {
            Value::Array(items.into_iter().map(coerce_stringified_args).collect())
        }
        Value::String(s) => match looks_like_json_literal(&s) {
            true => match serde_json::from_str::<Value>(&s) {
                Ok(parsed @ Value::Number(_))
                | Ok(parsed @ Value::Bool(_))
                | Ok(parsed @ Value::Array(_))
                | Ok(parsed @ Value::Object(_))
                | Ok(parsed @ Value::Null) => coerce_stringified_args(parsed),
                _ => Value::String(s),
            },
            false => Value::String(s),
        },
        other => other,
    }
}

fn looks_like_json_literal(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return false;
    }
    let first = trimmed.as_bytes()[0];
    matches!(first, b'{' | b'[' | b'-' | b'0'..=b'9')
        || matches!(trimmed, "true" | "false" | "null")
}

pub fn session_id(args: &Value) -> Result<String> {
    Ok(optional_str_strict(args, "session_id")?
        .or(optional_str_strict(args, "_session_id")?)
        .or_else(|| {
            std::env::var("CLAUDE_SESSION_ID")
                .ok()
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "cgreality-stdio-session".to_string()))
}

pub fn project_root() -> Result<PathBuf> {
    let root = std::env::var("CONTEXTGRAPH_ROOT")
        .map(PathBuf::from)
        .or_else(|_| std::env::current_dir())
        .map_err(|e| {
            CCRealityError::new(
                "CCREALITY_PROJECT_ROOT_RESOLVE_FAILED",
                format!("failed to resolve project root: {e}"),
                "project_root",
                "set CONTEXTGRAPH_ROOT or run from the ContextGraph checkout",
                json!({}),
                None,
            )
        })?;
    if !root.join("Cargo.toml").is_file() {
        return Err(CCRealityError::new(
            "CCREALITY_PROJECT_ROOT_INVALID",
            "project root does not contain Cargo.toml",
            "project_root.Cargo.toml",
            "set CONTEXTGRAPH_ROOT to /home/cabdru/contextgraph",
            json!({"project_root": root.display().to_string()}),
            Some(file_sot(&root)),
        ));
    }
    Ok(root)
}

pub fn safe_id(value: &str) -> String {
    let id = value
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
        .collect::<String>();
    if id.is_empty() {
        "artifact".to_string()
    } else {
        id
    }
}

pub fn unix_secs() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| {
            CCRealityError::new(
                "CCREALITY_SYSTEM_CLOCK_INVALID",
                format!("system clock is before UNIX_EPOCH: {e}"),
                "system_time",
                "fix system clock before recording artifacts",
                json!({}),
                None,
            )
        })
}

pub fn unix_ns() -> Result<u128> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .map_err(|e| {
            CCRealityError::new(
                "CCREALITY_SYSTEM_CLOCK_INVALID",
                format!("system clock is before UNIX_EPOCH: {e}"),
                "system_time",
                "fix system clock before recording artifacts",
                json!({}),
                None,
            )
        })
}

pub fn fs_error(code: &str, path: &Path, err: std::io::Error) -> CCRealityError {
    CCRealityError::new(
        code,
        format!("filesystem operation failed: {err}"),
        "filesystem",
        "inspect path, permissions, and disk space",
        json!({"path": path.display().to_string(), "error": err.to_string()}),
        Some(file_sot(path)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_run_id_ignores_runtime_metadata_directories() {
        let root = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(root.path().join("smoke-1778013223")).expect("run dir");
        for name in [
            "_engine_logs",
            "_manual_fsv_outputs",
            "failures",
            "workspaces",
            "cgreality-shift-log",
            "cgreality-hook-state",
            "claude-code-optimizer",
        ] {
            std::fs::create_dir_all(root.path().join(name)).expect("metadata dir");
        }

        let run_id = latest_run_id(root.path()).expect("latest run id");
        assert_eq!(run_id.as_deref(), Some("smoke-1778013223"));
    }

    #[test]
    fn runtime_metadata_dir_list_stays_explicit() {
        for name in [
            "_engine_logs",
            "_manual_fsv_outputs",
            "failures",
            "workspaces",
            "cgreality-shift-log",
            "cgreality-hook-state",
            "claude-code-optimizer",
        ] {
            assert!(is_runtime_metadata_dir(name), "{name} should be metadata");
        }
        assert!(!is_runtime_metadata_dir("smoke-1778013223"));
    }

    #[test]
    fn required_u64_accepts_stringified_integer() {
        let args = json!({"attempt": "1"});
        assert_eq!(required_u64(&args, "attempt").expect("coerced"), 1);
    }

    #[test]
    fn required_u64_accepts_native_integer() {
        let args = json!({"attempt": 7});
        assert_eq!(required_u64(&args, "attempt").expect("native"), 7);
    }

    #[test]
    fn required_u64_rejects_unparseable_string() {
        let args = json!({"attempt": "not-a-number"});
        let err = required_u64(&args, "attempt").expect_err("invalid");
        assert_eq!(err.error_code, "CCREALITY_ARG_MISSING_OR_NOT_U64");
    }

    #[test]
    fn optional_bool_accepts_stringified_true_false() {
        assert!(optional_bool_strict(&json!({"k": "true"}), "k", false).expect("true"));
        assert!(!optional_bool_strict(&json!({"k": "false"}), "k", true).expect("false"));
        assert!(optional_bool_strict(&json!({"k": true}), "k", false).expect("native"));
        let err =
            optional_bool_strict(&json!({"k": "garbage"}), "k", false).expect_err("invalid bool");
        assert_eq!(err.error_code, "CCREALITY_ARG_INVALID_BOOL");
    }

    #[test]
    fn coerce_stringified_args_round_trips_typed_recommendation() {
        let stringified = json!({
            "schema_version": "1",
            "attempt": "3",
            "turn_number": "2",
            "status": "changed",
            "reason": "natural prose stays unchanged",
            "source_files_changed": "[\"a.rs\",\"b.rs\"]",
            "memories_created": "[]",
            "harness_transitions_minted": "[\"file:/tmp/x\"]",
            "drift_detected": "{\"files\":[]}"
        });
        let coerced = coerce_stringified_args(stringified);
        assert_eq!(coerced["schema_version"], json!(1));
        assert_eq!(coerced["attempt"], json!(3));
        assert_eq!(coerced["turn_number"], json!(2));
        assert_eq!(coerced["status"], json!("changed"));
        assert_eq!(coerced["reason"], json!("natural prose stays unchanged"));
        assert_eq!(coerced["source_files_changed"], json!(["a.rs", "b.rs"]));
        assert_eq!(coerced["memories_created"], json!([]));
        assert_eq!(
            coerced["harness_transitions_minted"],
            json!(["file:/tmp/x"])
        );
        assert_eq!(coerced["drift_detected"], json!({"files": []}));
    }

    #[test]
    fn coerce_stringified_args_does_not_mangle_natural_strings() {
        let original = json!({
            "reason": "Phase 13 T13.2 fix",
            "next_inner_prompt_delta": "Use open_file_window before drafting any patch.",
            "source_of_truth": "file:placeholder"
        });
        let coerced = coerce_stringified_args(original.clone());
        assert_eq!(coerced, original);
    }

    #[test]
    fn looks_like_json_literal_is_strict() {
        assert!(looks_like_json_literal("1"));
        assert!(looks_like_json_literal("-5"));
        assert!(looks_like_json_literal("[1,2]"));
        assert!(looks_like_json_literal("{\"k\":1}"));
        assert!(looks_like_json_literal("true"));
        assert!(looks_like_json_literal("false"));
        assert!(looks_like_json_literal("null"));
        assert!(!looks_like_json_literal("changed"));
        assert!(!looks_like_json_literal("file:/tmp/x"));
        assert!(!looks_like_json_literal(""));
    }

    #[test]
    fn required_str_preserves_leading_and_trailing_whitespace() {
        // Regression: str::trim previously stripped indentation from the inner model's
        // `replace` payloads, corrupting py_compile-sensitive Python blocks.
        let payload = "    if x:\n        pass\n";
        let args = json!({"replace": payload});
        let got = required_str(&args, "replace").expect("non-empty");
        assert_eq!(got, payload, "required_str must preserve bytes verbatim");
    }

    #[test]
    fn optional_str_strict_preserves_leading_and_trailing_whitespace() {
        let payload = "\n  indented value\t\n";
        let args = json!({"replace": payload});
        let got = optional_str_strict(&args, "replace")
            .expect("valid")
            .expect("present");
        assert_eq!(
            got, payload,
            "optional_str_strict must preserve bytes verbatim"
        );
    }

    #[test]
    fn optional_str_strict_rejects_invalid_values() {
        let args = json!({"replace": 42});
        let err = optional_str_strict(&args, "replace").expect_err("invalid");
        assert_eq!(err.error_code, "CCREALITY_ARG_INVALID_STRING");
    }
}
