use super::errors::{CCRealityError, Result};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn read_active_runtime_root() -> Result<Option<PathBuf>> {
    let Some(home) = dirs::home_dir() else {
        return Ok(None);
    };
    let path = home.join(".cache/cgreality/active_runtime_root");
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|e| {
        CCRealityError::new(
            "CCREALITY_ACTIVE_ROOT_READ_FAILED",
            format!("failed to read active runtime root: {e}"),
            "active_runtime_root.path",
            "ensure ~/.cache/cgreality is readable",
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
    let Some(home) = dirs::home_dir() else {
        return Ok(None);
    };
    let path = home.join(".cache/cgreality/active_target_instance");
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|e| {
        CCRealityError::new(
            "CCREALITY_ACTIVE_INSTANCE_READ_FAILED",
            format!("failed to read active target instance: {e}"),
            "active_target_instance.path",
            "ensure ~/.cache/cgreality is readable",
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
            "run reality-loop smoke or attempt first",
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
            "run reality-loop smoke or attempt first",
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
        if name == "failures" || name == "workspaces" || name == "cgreality-shift-log" {
            continue;
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        candidates.push((modified, name));
    }
    candidates.sort();
    Ok(candidates.into_iter().last().map(|(_, name)| name))
}

pub fn latest_attempt_number(runtime_root: &Path, target_instance: Option<&str>) -> Result<Option<u64>> {
    let Some(run_id) = latest_run_id(runtime_root)? else {
        return Ok(None);
    };
    let run_dir = runtime_root.join(run_id);
    let mut attempts = Vec::new();
    collect_attempt_dirs(&run_dir, target_instance, &mut attempts)?;
    Ok(attempts.into_iter().map(|(n, _)| n).max())
}

pub fn latest_attempt_dir(runtime_root: &Path, target_instance: Option<&str>) -> Result<Option<(String, u64, PathBuf)>> {
    let Some(run_id) = latest_run_id(runtime_root)? else {
        return Ok(None);
    };
    let run_dir = runtime_root.join(&run_id);
    let mut attempts = Vec::new();
    collect_attempt_dirs(&run_dir, target_instance, &mut attempts)?;
    attempts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    Ok(attempts.into_iter().last().map(|(n, p)| (run_id, n, p)))
}

pub fn attempt_dir(runtime_root: &Path, run_id: &str, task_id: &str, attempt: u64) -> Result<PathBuf> {
    let run_dir = runtime_root.join(run_id);
    let direct = run_dir.join(task_id).join(format!("attempt-{attempt}"));
    if direct.is_dir() {
        return Ok(direct);
    }
    let smoke = run_dir.join(task_id).join(format!("smoke-attempt-{attempt}"));
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
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| fs_error("CCREALITY_DIR_CREATE_FAILED", parent, e))?;
    }
    let mut file = fs::File::create(path).map_err(|e| fs_error("CCREALITY_FILE_CREATE_FAILED", path, e))?;
    file.write_all(text.as_bytes()).map_err(|e| fs_error("CCREALITY_FILE_WRITE_FAILED", path, e))?;
    file.sync_all().map_err(|e| fs_error("CCREALITY_FILE_SYNC_FAILED", path, e))?;
    let readback = fs::read_to_string(path).map_err(|e| fs_error("CCREALITY_FILE_READBACK_FAILED", path, e))?;
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
        .map(str::trim)
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

pub fn optional_str(args: &Value, field: &str) -> Option<String> {
    args.get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn required_u64(args: &Value, field: &str) -> Result<u64> {
    args.get(field).and_then(Value::as_u64).ok_or_else(|| {
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

pub fn optional_u64(args: &Value, field: &str, default: u64) -> u64 {
    args.get(field).and_then(Value::as_u64).unwrap_or(default)
}

pub fn optional_bool(args: &Value, field: &str, default: bool) -> bool {
    args.get(field).and_then(Value::as_bool).unwrap_or(default)
}

pub fn session_id(args: &Value) -> String {
    optional_str(args, "session_id")
        .or_else(|| optional_str(args, "_session_id"))
        .or_else(|| std::env::var("CLAUDE_SESSION_ID").ok())
        .unwrap_or_else(|| "cgreality-stdio-session".to_string())
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
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(160)
        .collect::<String>();
    if id.is_empty() { "artifact".to_string() } else { id }
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
