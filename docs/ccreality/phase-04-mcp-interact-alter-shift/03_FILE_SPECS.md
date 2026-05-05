# Phase 4 — File Specifications

Skeletal but concrete. Adapt to existing rmcp/registration patterns.

## `lock.rs`

```rust
//! Single-writer edit lock for ccreality. The lock is a small file at
//! ~/.cache/cgreality/edit_lock containing "session_id:unix_ts". Acquired with
//! O_EXCL; released by Drop. See docs/ccreality/16_PARALLEL_AGENT_POLICY.md.

use super::errors::{CCRealityError, Result};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const LOCK_FILE: &str = ".cache/cgreality/edit_lock";
const STALE_LOCK_AGE_SECS: u64 = 3600;  // 1 hour

pub struct EditLock {
    path: PathBuf,
    session_id: String,
    acquired_at: u64,
}

impl EditLock {
    pub fn try_acquire(session_id: &str) -> Result<Self> {
        let path = lock_path()?;
        let parent = path.parent().ok_or_else(|| CCRealityError::new(
            "CCREALITY_HARNESS_EDIT_LOCK_PARENT_MISSING",
            "edit lock path has no parent directory",
            "edit_lock.path",
            "fix the configured edit-lock path",
            serde_json::json!({"path": path.display().to_string()}),
            Some(format!("file:{}", path.display())),
        ))?;
        std::fs::create_dir_all(parent).map_err(|e| CCRealityError::new(
            "CCREALITY_HARNESS_EDIT_LOCK_DIR_FAILED",
            format!("failed to create edit-lock directory: {e}"),
            "edit_lock.parent",
            "ensure the cgreality cache directory is writable",
            serde_json::json!({"path": parent.display().to_string()}),
            Some(format!("file:{}", parent.display())),
        ))?;

        // First check if there's a stale lock to clean up.
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Some((existing_session, ts_str)) = content.trim().split_once(':') {
                if let Ok(ts) = ts_str.parse::<u64>() {
                    let now = unix_secs()?;
                    if now.saturating_sub(ts) > STALE_LOCK_AGE_SECS {
                        tracing::warn!("removing stale edit lock from session {}", existing_session);
                        std::fs::remove_file(&path).map_err(|e| CCRealityError::new(
                            "CCREALITY_HARNESS_STALE_LOCK_REMOVE_FAILED",
                            format!("failed to remove stale edit lock: {e}"),
                            "edit_lock.remove_stale",
                            "inspect and remove the stale lock manually",
                            serde_json::json!({"path": path.display().to_string(), "holder": existing_session}),
                            Some(format!("file:{}", path.display())),
                        ))?;
                    } else {
                        return Err(CCRealityError::new(
                            "CCREALITY_HARNESS_EDIT_OWNER_BUSY",
                            format!("edit lock held by session {} since {}s ago", existing_session, now.saturating_sub(ts)),
                            "edit_lock",
                            "wait for the other writer to finish or remove the lock if stale",
                            serde_json::json!({"holder": existing_session, "since_unix": ts, "now_unix": now}),
                            Some(format!("file:{}", path.display())),
                        ));
                    }
                }
            }
        }

        let acquired_at = unix_secs()?;
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|e| CCRealityError::new(
                "CCREALITY_HARNESS_EDIT_LOCK_FAILED",
                format!("failed to acquire edit lock: {e}"),
                "edit_lock.create",
                "another writer may have acquired the lock simultaneously; retry once",
                serde_json::json!({"path": path.display().to_string()}),
                Some(format!("file:{}", path.display())),
            ))?;
        write!(f, "{}:{}", session_id, acquired_at).map_err(|e| CCRealityError::new(
            "CCREALITY_HARNESS_EDIT_LOCK_WRITE_FAILED",
            format!("failed to write edit lock: {e}"),
            "edit_lock.write",
            "ensure the cgreality cache directory is writable",
            serde_json::json!({"path": path.display().to_string()}),
            Some(format!("file:{}", path.display())),
        ))?;
        f.sync_all().map_err(|e| CCRealityError::new(
            "CCREALITY_HARNESS_EDIT_LOCK_SYNC_FAILED",
            format!("failed to sync edit lock: {e}"),
            "edit_lock.sync",
            "ensure the filesystem accepts fsync for the lock path",
            serde_json::json!({"path": path.display().to_string()}),
            Some(format!("file:{}", path.display())),
        ))?;
        Ok(Self { path, session_id: session_id.to_string(), acquired_at })
    }
}

impl Drop for EditLock {
    fn drop(&mut self) {
        // Only remove if we still own the lock.
        if let Ok(content) = std::fs::read_to_string(&self.path) {
            if content.starts_with(&format!("{}:", self.session_id)) {
                if let Err(err) = std::fs::remove_file(&self.path) {
                    tracing::warn!("failed to remove owned edit lock {}: {}", self.path.display(), err);
                }
            }
        }
    }
}

fn lock_path() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .ok_or_else(|| CCRealityError::new(
            "CCREALITY_HOME_DIR_MISSING",
            "no home directory; cannot place edit lock",
            "process.env.HOME",
            "set HOME",
            serde_json::json!({}), None,
        ))?
        .join(LOCK_FILE))
}

fn unix_secs() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| CCRealityError::new(
            "CCREALITY_SYSTEM_CLOCK_INVALID",
            format!("system clock is before UNIX_EPOCH: {e}"),
            "system_time",
            "fix the system clock before recording ccreality artifacts",
            serde_json::json!({}), None,
        ))
}
```

## `shift_log.rs`

```rust
use super::errors::{CCRealityError, Result};
use super::helpers::*;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct ShiftRecord {
    pub shift_id: String,
    pub timestamp_unix_ns: u128,
    pub tool_name: String,
    pub tool_use_id: Option<String>,
    pub session_id: String,
    pub subject: Value,
    pub before: Value,
    pub after: Value,
    pub delta_summary: Value,
    pub verification: Value,
    pub harness_transition_path: Option<String>,
}

impl ShiftRecord {
    pub fn new(tool_name: &str, session_id: &str, subject: Value) -> Result<Self> {
        Ok(Self {
            shift_id: format!("01J{}", uuid::Uuid::new_v4().as_simple().to_string()[..20].to_uppercase()),
            timestamp_unix_ns: unix_ns()?,
            tool_name: tool_name.to_string(),
            tool_use_id: None,
            session_id: session_id.to_string(),
            subject,
            before: json!({}),
            after: json!({}),
            delta_summary: json!({}),
            verification: json!({}),
            harness_transition_path: None,
        })
    }
}

pub fn append_shift(runtime_root: &Path, session_id: &str, record: &ShiftRecord) -> Result<()> {
    let dir = runtime_root.join("cgreality-shift-log");
    std::fs::create_dir_all(&dir).map_err(|e| CCRealityError::new(
        "CCREALITY_SHIFT_LOG_DIR_FAILED",
        format!("failed to create shift-log dir: {e}"),
        "shift_log.dir", "ensure runtime root is writable",
        json!({"path": dir.display().to_string()}),
        Some(format!("file:{}", dir.display())),
    ))?;
    let path = dir.join(format!("{}.jsonl", session_id));
    let mut f = OpenOptions::new().create(true).append(true).open(&path)
        .map_err(|e| CCRealityError::new(
            "CCREALITY_SHIFT_LOG_OPEN_FAILED",
            format!("failed to open shift log: {e}"),
            "shift_log.open", "ensure runtime root is writable",
            json!({"path": path.display().to_string()}),
            Some(format!("file:{}", path.display())),
        ))?;
    let line = serde_json::to_string(record).map_err(|e| CCRealityError::new(
        "CCREALITY_SHIFT_LOG_SERIALIZE_FAILED",
        format!("failed to serialize shift record: {e}"),
        "shift_log.serialize",
        "fix the shift record payload",
        json!({"shift_id": record.shift_id}),
        Some(format!("file:{}", path.display())),
    ))?;
    writeln!(f, "{}", line).map_err(|e| CCRealityError::new(
        "CCREALITY_SHIFT_LOG_WRITE_FAILED",
        format!("failed to append shift log: {e}"),
        "shift_log.write",
        "ensure runtime root is writable",
        json!({"path": path.display().to_string()}),
        Some(format!("file:{}", path.display())),
    ))?;
    f.sync_all().map_err(|e| CCRealityError::new(
        "CCREALITY_SHIFT_LOG_SYNC_FAILED",
        format!("failed to sync shift log: {e}"),
        "shift_log.sync",
        "ensure the filesystem accepts fsync for the shift log",
        json!({"path": path.display().to_string()}),
        Some(format!("file:{}", path.display())),
    ))?;
    Ok(())
}

pub async fn reality_shift_log(args: Value) -> Result<Value> {
    let session_id = required_str(&args, "session_id")?;
    let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(20).min(500) as usize;
    let since_id = args.get("since_shift_id").and_then(Value::as_str);

    let runtime_root = require_active_runtime_root().await?;
    let path = runtime_root.join("cgreality-shift-log").join(format!("{}.jsonl", session_id));
    if !path.is_file() {
        return Ok(json!({ "shifts": [], "source_of_truth": format!("file:{}", path.display()) }));
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| CCRealityError::new(
        "CCREALITY_SHIFT_LOG_READ_FAILED",
        format!("failed to read shift log: {e}"),
        "shift_log.read", "ensure file is readable",
        json!({"path": path.display().to_string()}),
        Some(format!("file:{}", path.display())),
    ))?;
    let mut records = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        let record = serde_json::from_str(line).map_err(|e| CCRealityError::new(
            "CCREALITY_SHIFT_LOG_JSON_INVALID",
            format!("invalid JSON at shift log line {}: {e}", idx + 1),
            "shift_log.parse",
            "repair or remove the corrupt shift log line",
            json!({"path": path.display().to_string(), "line": idx + 1}),
            Some(format!("file:{}", path.display())),
        ))?;
        records.push(record);
    }
    if let Some(since) = since_id {
        if let Some(idx) = records.iter().position(|r| r.get("shift_id").and_then(Value::as_str) == Some(since)) {
            records = records.into_iter().skip(idx + 1).collect();
        }
    }
    records.truncate(limit);
    Ok(json!({
        "shifts": records,
        "shift_count": records.len(),
        "source_of_truth": format!("file:{}", path.display())
    }))
}

pub async fn reality_shift_compare_to_my_view(args: Value) -> Result<Value> {
    let session_id = required_str(&args, "session_id")?;
    let files = args.get("files").and_then(Value::as_array)
        .ok_or_else(|| CCRealityError::new(
            "CCREALITY_ARG_FILES_NOT_ARRAY",
            "argument 'files' must be an array of strings",
            "arguments.files", "provide [\"path1\", \"path2\", ...]",
            json!({"args": args}), None,
        ))?;
    let runtime_root = require_active_runtime_root().await?;
    let log_path = runtime_root.join("cgreality-shift-log").join(format!("{}.jsonl", session_id));
    let session_records: Vec<Value> = if log_path.is_file() {
        let raw = std::fs::read_to_string(&log_path).map_err(|e| CCRealityError::new(
            "CCREALITY_SHIFT_LOG_READ_FAILED",
            format!("failed to read shift log: {e}"),
            "shift_log.read",
            "ensure file is readable",
            json!({"path": log_path.display().to_string()}),
            Some(format!("file:{}", log_path.display())),
        ))?;
        let mut parsed = Vec::new();
        for (idx, line) in raw.lines().enumerate() {
            parsed.push(serde_json::from_str(line).map_err(|e| CCRealityError::new(
                "CCREALITY_SHIFT_LOG_JSON_INVALID",
                format!("invalid JSON at shift log line {}: {e}", idx + 1),
                "shift_log.parse",
                "repair or remove the corrupt shift log line",
                json!({"path": log_path.display().to_string(), "line": idx + 1}),
                Some(format!("file:{}", log_path.display())),
            ))?);
        }
        parsed
    } else { vec![] };

    let mut results = Vec::new();
    for f in files {
        let path = match f.as_str() {
            Some(s) => s,
            None => continue,
        };
        let abs_path = std::path::PathBuf::from(path);
        if !abs_path.is_file() {
            results.push(json!({
                "path": path, "current_sha256": null, "session_last_known_sha256": null,
                "drift": "unseen_file", "external_writer_hint": null
            }));
            continue;
        }
        let current = sha256_file(&abs_path)?;
        // Find the most recent shift in this session that touched this file
        let last_session_known: Option<String> = session_records.iter().rev().find_map(|r| {
            let subject_path = r.pointer("/subject/path").and_then(Value::as_str)?;
            if subject_path == path { r.pointer("/after/sha256").and_then(Value::as_str).map(str::to_string) } else { None }
        });
        let drift = match (&last_session_known, &current) {
            (Some(known), curr) if known == curr => "no_change",
            (Some(_), _) => "external_drift_detected",
            (None, _) => "unseen_file",
        };
        results.push(json!({
            "path": path,
            "current_sha256": current,
            "session_last_known_sha256": last_session_known,
            "drift": drift,
            "external_writer_hint": null
        }));
    }
    Ok(json!({ "files": results, "source_of_truth": format!("file:{}", log_path.display()) }))
}

pub fn register(server: &mut crate::server::McpServer) {
    server.register_tool("reality_shift_log", input_schema_shift_log(), reality_shift_log);
    server.register_tool("reality_shift_compare_to_my_view", input_schema_shift_compare(), reality_shift_compare_to_my_view);
}

fn unix_ns() -> Result<u128> {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .map_err(|e| CCRealityError::new(
            "CCREALITY_SYSTEM_CLOCK_INVALID",
            format!("system clock is before UNIX_EPOCH: {e}"),
            "system_time",
            "fix the system clock before recording ccreality artifacts",
            json!({}), None,
        ))
}
```

## `schema_validator.rs`

```rust
use super::errors::{CCRealityError, Result};
use jsonschema::{JSONSchema, Draft};
use serde_json::Value;
use std::path::PathBuf;

pub struct RecommendationValidator { compiled: JSONSchema }

impl RecommendationValidator {
    pub fn load() -> Result<Self> {
        let path = std::env::var("CGREALITY_RECOMMENDATION_SCHEMA")
            .map(PathBuf::from)
            .map_err(|_| CCRealityError::new(
                "CCREALITY_RECOMMENDATION_SCHEMA_PATH_MISSING",
                "CGREALITY_RECOMMENDATION_SCHEMA is not set",
                "env.CGREALITY_RECOMMENDATION_SCHEMA", "set CGREALITY_RECOMMENDATION_SCHEMA to the exact schema path",
                serde_json::json!({}), None,
            ))?;
        let raw = std::fs::read_to_string(&path).map_err(|e| CCRealityError::new(
            "CCREALITY_RECOMMENDATION_SCHEMA_READ_FAILED",
            format!("failed to read schema: {e}"),
            "config.recommendation_schema", "ensure file exists",
            serde_json::json!({"path": path.display().to_string()}),
            Some(format!("file:{}", path.display())),
        ))?;
        let schema_value: Value = serde_json::from_str(&raw).map_err(|e| CCRealityError::new(
            "CCREALITY_RECOMMENDATION_SCHEMA_PARSE_FAILED",
            format!("schema is not valid JSON: {e}"),
            "config.recommendation_schema.json", "fix syntax",
            serde_json::json!({"path": path.display().to_string()}),
            Some(format!("file:{}", path.display())),
        ))?;
        let compiled = JSONSchema::options().with_draft(Draft::Draft202012).compile(&schema_value)
            .map_err(|e| CCRealityError::new(
                "CCREALITY_RECOMMENDATION_SCHEMA_COMPILE_FAILED",
                format!("schema compile error: {e}"),
                "config.recommendation_schema", "fix schema definition",
                serde_json::json!({"compile_error": e.to_string()}),
                Some(format!("file:{}", path.display())),
            ))?;
        Ok(Self { compiled })
    }

    pub fn validate(&self, value: &Value) -> Result<()> {
        let result = self.compiled.validate(value);
        if let Err(errors) = result {
            let details: Vec<Value> = errors.map(|e| serde_json::json!({
                "instance_path": e.instance_path.to_string(),
                "schema_path": e.schema_path.to_string(),
                "message": e.to_string(),
            })).collect();
            return Err(CCRealityError::new(
                "CCREALITY_RECOMMENDATION_SCHEMA_INVALID",
                format!("recommendation failed schema validation; {} errors", details.len()),
                "recommendation",
                "fix the recommendation per the listed errors",
                serde_json::json!({"errors": details, "value": value}),
                None,
            ));
        }
        Ok(())
    }
}
```

## `alter.rs`, `interact.rs`, `optimizer.rs` (skeletal)

Each file follows the same pattern as `analyze.rs` from Phase 3. Per-tool implementation is mechanical; the canonical patterns are:

```rust
// alter.rs — harness_apply_line_window_edit (the most complex)
pub async fn harness_apply_line_window_edit(args: Value) -> Result<Value> {
    let path = required_str(&args, "path")?;
    let start = required_u64(&args, "start_line")?;
    let end = required_u64(&args, "end_line")?;
    let observed_sha = required_str(&args, "observed_sha256")?;
    let replace = required_str(&args, "replace")?;
    let session_id = required_str(&args, "_session_id")?;

    // 1. Path policy
    let policy = super::path_policy::GovernedPathsPolicy::load()?;
    if let super::path_policy::PolicyVerdict::DenyExplicit(rule) = policy.is_allowed(&path) {
        return Err(CCRealityError::new(
            "CCREALITY_HARNESS_PATH_DENIED",
            format!("path '{}' matches deny rule '{}'", path, rule),
            "arguments.path", "edit a different file",
            json!({"path": path, "rule": rule}), None,
        ));
    }

    // 2. Acquire lock
    let _lock = super::lock::EditLock::try_acquire(&session_id)?;

    // 3. Use Phase 1's lifted module
    let project_root = context_graph_cli::governed_edit::project_root()?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let attempt = super::helpers::latest_attempt_number(&runtime_root, Some(&task))?
        .ok_or_else(|| CCRealityError::new(
            "CCREALITY_HARNESS_ATTEMPT_MISSING",
            "could not identify latest attempt for active target instance",
            "runtime.attempt",
            "run the engine before applying a harness edit",
            json!({"runtime_root": runtime_root.display().to_string(), "target_instance": task.clone()}),
            Some(format!("file:{}", runtime_root.display())),
        ))?;
    let run_id = super::helpers::latest_run_id(&runtime_root)?.ok_or_else(|| CCRealityError::new(
        "CCREALITY_HARNESS_RUN_MISSING",
        "could not identify latest run directory",
        "runtime.run_id",
        "run the engine before applying a harness edit",
        json!({"runtime_root": runtime_root.display().to_string()}),
        Some(format!("file:{}", runtime_root.display())),
    ))?;
    let attempt_dir = super::helpers::attempt_dir(&runtime_root, &run_id, &task, attempt);
    let tool_dir = attempt_dir.join("claude-code-optimizer/edits");
    std::fs::create_dir_all(&tool_dir).map_err(|e| CCRealityError::new(
        "CCREALITY_HARNESS_TOOL_DIR_CREATE_FAILED",
        format!("failed to create optimizer edit artifact directory: {e}"),
        "harness.tool_dir",
        "ensure runtime root is writable",
        json!({"path": tool_dir.display().to_string()}),
        Some(format!("file:{}", tool_dir.display())),
    ))?;

    let edits = vec![json!({
        "path": path,
        "start_line": start,
        "end_line": end,
        "observed_sha256": observed_sha,
        "replace": replace,
    })];
    let result = context_graph_cli::governed_edit::project_apply_line_window_edits(
        &project_root, &tool_dir, &edits, 60,
    ).map_err(|e| CCRealityError::new(
        format!("CCREALITY_HARNESS_EDIT_{}", e.error_code),
        e.message, e.field_path, e.remediation, e.details, e.source_of_truth,
    ))?;

    // 4. Compute deltas
    let after_sha = super::helpers::sha256_file(&project_root.join(&path))?;
    let lines_added = result.pointer("/applied/0/lines_added").and_then(Value::as_u64).ok_or_else(|| CCRealityError::new(
        "CCREALITY_HARNESS_EDIT_RESULT_MISSING_LINES_ADDED",
        "governed edit result did not include applied[0].lines_added",
        "governed_edit.result.applied[0].lines_added",
        "fix governed_edit to return the expected edit telemetry",
        json!({"result": result.clone()}),
        None,
    ))?;
    let lines_removed = result.pointer("/applied/0/lines_removed").and_then(Value::as_u64).ok_or_else(|| CCRealityError::new(
        "CCREALITY_HARNESS_EDIT_RESULT_MISSING_LINES_REMOVED",
        "governed edit result did not include applied[0].lines_removed",
        "governed_edit.result.applied[0].lines_removed",
        "fix governed_edit to return the expected edit telemetry",
        json!({"result": result.clone()}),
        None,
    ))?;

    // 5. Build shift record
    let mut shift = super::shift_log::ShiftRecord::new(
        "harness_apply_line_window_edit",
        &session_id,
        json!({"type": "file", "path": path}),
    )?;
    shift.before = json!({"sha256": observed_sha});
    shift.after = json!({"sha256": after_sha});
    shift.delta_summary = json!({"lines_added": lines_added, "lines_removed": lines_removed});

    // 6. Mint harness-transition record
    let transition_path = super::optimizer::write_harness_transition(&attempt_dir, &shift, &result)?;
    shift.harness_transition_path = Some(format!("file:{}", transition_path.display()));

    // 7. Append shift
    super::shift_log::append_shift(&runtime_root, &session_id, &shift)?;

    // 8. Return
    Ok(json!({
        "status": "ok",
        "path": path,
        "start_line": start,
        "end_line": end,
        "before_sha256": observed_sha,
        "after_sha256": after_sha,
        "lines_added": lines_added,
        "lines_removed": lines_removed,
        "harness_transition_path": shift.harness_transition_path,
        "shift_id": shift.shift_id,
        "source_of_truth": format!("file:{}", transition_path.display())
    }))
}
```

The other ALTER/INTERACT/optimizer tools follow the same structure. See spec doc 03 for full per-tool details. Each:
1. Parses args.
2. Validates policy.
3. Performs the action (subprocess for INTERACT; lifted helper for harness_*; SQL for ledger; etc.).
4. Builds and appends a shift record.
5. Returns a JSON envelope with `source_of_truth` and (for ALTER) `shift_id`.

## Cargo.toml additions

```toml
[dependencies]
jsonschema = "0.18"
fs2 = "0.4"
uuid = { version = "1", features = ["v4"] }
context-graph-cli = { path = "../context-graph-cli" }
rusqlite = { version = "0.31", features = ["bundled"] }  # if not already
```
