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
        if is_runtime_metadata_dir(&name) {
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
    matches!(
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
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| fs_error("CCREALITY_DIR_CREATE_FAILED", parent, e))?;
    }
    let mut file =
        fs::File::create(path).map_err(|e| fs_error("CCREALITY_FILE_CREATE_FAILED", path, e))?;
    file.write_all(text.as_bytes())
        .map_err(|e| fs_error("CCREALITY_FILE_WRITE_FAILED", path, e))?;
    file.sync_all()
        .map_err(|e| fs_error("CCREALITY_FILE_SYNC_FAILED", path, e))?;
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

pub fn optional_u64(args: &Value, field: &str, default: u64) -> u64 {
    coerce_u64(args.get(field)).unwrap_or(default)
}

pub fn optional_bool(args: &Value, field: &str, default: bool) -> bool {
    coerce_bool(args.get(field)).unwrap_or(default)
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
        assert!(optional_bool(&json!({"k": "true"}), "k", false));
        assert!(!optional_bool(&json!({"k": "false"}), "k", true));
        assert!(optional_bool(&json!({"k": true}), "k", false));
        assert!(!optional_bool(&json!({"k": "garbage"}), "k", false));
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
            "drift_detected": "{\"files\":[]}",
            "cost_estimate_usd": "0.45"
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
        assert!(coerced["cost_estimate_usd"].as_f64().unwrap() - 0.45 < 1e-9);
    }

    #[test]
    fn coerce_stringified_args_does_not_mangle_natural_strings() {
        let original = json!({
            "reason": "Phase 13 T13.2 fix",
            "next_gemma_prompt_delta": "Use open_file_window before drafting any patch.",
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
}
