use super::errors::{CCRealityError, Result};
use super::helpers::*;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            shift_id: format!(
                "01J{}",
                &uuid::Uuid::new_v4().as_simple().to_string()[..20].to_uppercase()
            ),
            timestamp_unix_ns: unix_ns()?,
            tool_name: tool_name.to_string(),
            tool_use_id: None,
            session_id: normalize_session_id_hex32(session_id),
            subject,
            before: json!({}),
            after: json!({}),
            delta_summary: json!({}),
            verification: json!({}),
            harness_transition_path: None,
        })
    }
}

impl Handlers {
    pub(crate) async fn call_reality_shift_log(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_shift_log(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_shift_compare_to_my_view(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_shift_compare_to_my_view(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }
}

pub fn append_shift(runtime_root: &Path, session_id: &str, record: &ShiftRecord) -> Result<()> {
    let normalized_session_id = normalize_session_id_hex32(session_id);
    if record.session_id != normalized_session_id {
        return Err(CCRealityError::new(
            "CCREALITY_SHIFT_SESSION_ID_MISMATCH",
            "shift record session_id does not match the append target after 32-hex normalization",
            "shift_log.session_id",
            "construct ShiftRecord with the same raw session_id passed to append_shift",
            json!({
                "append_session_id_hex32": normalized_session_id,
                "record_session_id": record.session_id,
                "shift_id": record.shift_id
            }),
            None,
        ));
    }
    let dir = runtime_root.join("cgreality-shift-log");
    std::fs::create_dir_all(&dir)
        .map_err(|e| fs_error("CCREALITY_SHIFT_LOG_DIR_FAILED", &dir, e))?;
    let path = dir.join(format!("{}.jsonl", safe_id(&normalized_session_id)));
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| fs_error("CCREALITY_SHIFT_LOG_OPEN_FAILED", &path, e))?;
    let line = serde_json::to_string(record).map_err(|e| {
        CCRealityError::new(
            "CCREALITY_SHIFT_LOG_SERIALIZE_FAILED",
            format!("failed to serialize shift record: {e}"),
            "shift_log.serialize",
            "fix the shift record payload",
            json!({"shift_id": record.shift_id}),
            Some(file_sot(&path)),
        )
    })?;
    writeln!(file, "{line}").map_err(|e| fs_error("CCREALITY_SHIFT_LOG_WRITE_FAILED", &path, e))?;
    file.sync_all()
        .map_err(|e| fs_error("CCREALITY_SHIFT_LOG_SYNC_FAILED", &path, e))?;
    Ok(())
}

pub async fn reality_shift_log(args: Value) -> Result<Value> {
    let session_id = normalize_session_id_hex32(&required_str(&args, "session_id")?);
    let limit = optional_u64_strict(&args, "limit", 20)?.min(500) as usize;
    let since_id = optional_str_strict(&args, "since_shift_id")?;
    let runtime_root = require_active_runtime_root().await?;
    let path = runtime_root
        .join("cgreality-shift-log")
        .join(format!("{}.jsonl", safe_id(&session_id)));
    if !path.is_file() {
        return Ok(json!({"shifts": [], "shift_count": 0, "source_of_truth": file_sot(&path)}));
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| fs_error("CCREALITY_SHIFT_LOG_READ_FAILED", &path, e))?;
    let mut records = Vec::<Value>::new();
    for (idx, line) in raw.lines().enumerate() {
        let record: Value = serde_json::from_str(line).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_SHIFT_LOG_JSON_INVALID",
                format!("invalid JSON at shift log line {}: {e}", idx + 1),
                "shift_log.parse",
                "repair or remove the corrupt shift log line",
                json!({"path": path.display().to_string(), "line": idx + 1}),
                Some(file_sot(&path)),
            )
        })?;
        records.push(record);
    }
    if let Some(since) = since_id {
        if let Some(pos) = records
            .iter()
            .position(|r| r.get("shift_id").and_then(Value::as_str) == Some(&since))
        {
            records = records.into_iter().skip(pos + 1).collect();
        } else {
            return Err(CCRealityError::new(
                "CCREALITY_SHIFT_LOG_SINCE_ID_MISSING",
                "since_shift_id was not found in the session shift log",
                "shift_log.since_shift_id",
                "read the current shift log and retry with an existing shift_id",
                json!({"since_shift_id": since, "path": path.display().to_string()}),
                Some(file_sot(&path)),
            ));
        }
    }
    records.truncate(limit);
    Ok(json!({
        "shifts": records,
        "shift_count": records.len(),
        "source_of_truth": file_sot(&path)
    }))
}

pub async fn reality_shift_compare_to_my_view(args: Value) -> Result<Value> {
    let session_id = normalize_session_id_hex32(&required_str(&args, "session_id")?);
    let files = args.get("files").and_then(Value::as_array).ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_ARG_FILES_NOT_ARRAY",
            "argument 'files' must be an array of strings",
            "arguments.files",
            "provide [\"path1\", \"path2\"]",
            json!({"args": args}),
            None,
        )
    })?;
    let runtime_root = require_active_runtime_root().await?;
    let log = reality_shift_log(json!({"session_id": session_id, "limit": 500})).await?;
    let records = log
        .get("shifts")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_SHIFT_LOG_RESULT_INVALID_SHAPE",
                "reality_shift_log result did not contain an array at .shifts",
                "shift_log.shifts",
                "inspect the shift log reader output before comparing state",
                json!({"result": log}),
                Some(file_sot(&runtime_root.join("reality-shifts"))),
            )
        })?;
    let all_records = load_all_shift_records(&runtime_root)?;
    let root = project_root()?;
    let mut out = Vec::new();
    for (idx, file) in files.iter().enumerate() {
        let path_str_arg = file.as_str().ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_ARG_FILES_ITEM_NOT_STRING",
                "every item in argument 'files' must be a string path",
                format!("arguments.files[{idx}]"),
                "provide an array of string paths",
                json!({"value": file}),
                None,
            )
        })?;
        let path = if path_str_arg.starts_with('/') {
            std::path::PathBuf::from(path_str_arg)
        } else {
            root.join(path_str_arg)
        };
        if !path.is_file() {
            out.push(json!({
                "path": path_str_arg,
                "current_sha256": Value::Null,
                "session_last_known_sha256": Value::Null,
                "drift": "unseen_file",
                "external_writer_hint": Value::Null
            }));
            continue;
        }
        let current = sha256_file(&path)?;
        let last = latest_after_sha_for_file(&records, path_str_arg, &path, &root);
        let global_latest = latest_record_for_file(&all_records, path_str_arg, &path, &root);
        let stale_write =
            stale_write_conflict_for_session(&records, &all_records, path_str_arg, &path, &root);
        let drift = if stale_write.is_some() {
            "external_drift_detected"
        } else {
            match &last {
                Some(known) if known == &current => "no_change",
                Some(_) => "external_drift_detected",
                None => "unseen_file",
            }
        };
        let external_writer_hint = stale_write.or_else(|| {
            global_latest.and_then(|record| {
                let after = record.pointer("/after/sha256").and_then(Value::as_str)?;
                if Some(after) == last.as_deref() {
                    None
                } else {
                    Some(json!({
                        "session_id": record.get("session_id").and_then(Value::as_str),
                        "shift_id": record.get("shift_id").and_then(Value::as_str),
                        "after_sha256": after
                    }))
                }
            })
        });
        out.push(json!({
            "path": path_str_arg,
            "current_sha256": current,
            "session_last_known_sha256": last,
            "drift": drift,
            "external_writer_hint": external_writer_hint
        }));
    }
    Ok(json!({
        "files": out,
        "source_of_truth": file_sot(&runtime_root.join("cgreality-shift-log").join(format!("{}.jsonl", safe_id(&session_id))))
    }))
}

fn load_all_shift_records(runtime_root: &Path) -> Result<Vec<Value>> {
    let dir = runtime_root.join("cgreality-shift-log");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in
        std::fs::read_dir(&dir).map_err(|e| fs_error("CCREALITY_SHIFT_LOG_READ_FAILED", &dir, e))?
    {
        let entry = entry.map_err(|e| fs_error("CCREALITY_SHIFT_LOG_READ_FAILED", &dir, e))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| fs_error("CCREALITY_SHIFT_LOG_READ_FAILED", &path, e))?;
        for (idx, line) in raw.lines().enumerate() {
            let record: Value = serde_json::from_str(line).map_err(|e| {
                CCRealityError::new(
                    "CCREALITY_SHIFT_LOG_JSON_INVALID",
                    format!("invalid JSON at shift log line {}: {e}", idx + 1),
                    "shift_log.parse",
                    "repair or remove the corrupt shift log line",
                    json!({"path": path.display().to_string(), "line": idx + 1}),
                    Some(file_sot(&path)),
                )
            })?;
            out.push(record);
        }
    }
    out.sort_by_key(shift_timestamp);
    Ok(out)
}

fn latest_after_sha_for_file(
    records: &[Value],
    path_str_arg: &str,
    path: &Path,
    root: &Path,
) -> Option<String> {
    records
        .iter()
        .rev()
        .find(|record| record_matches_file(record, path_str_arg, path, root))
        .and_then(|record| {
            record
                .pointer("/after/sha256")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
}

fn latest_record_for_file<'a>(
    records: &'a [Value],
    path_str_arg: &str,
    path: &Path,
    root: &Path,
) -> Option<&'a Value> {
    records
        .iter()
        .rev()
        .find(|record| record_matches_file(record, path_str_arg, path, root))
}

fn stale_write_conflict_for_session(
    session_records: &[Value],
    all_records: &[Value],
    path_str_arg: &str,
    path: &Path,
    root: &Path,
) -> Option<Value> {
    for record in session_records
        .iter()
        .filter(|record| record_matches_file(record, path_str_arg, path, root))
    {
        let before = record.pointer("/before/sha256").and_then(Value::as_str)?;
        let timestamp = shift_timestamp(record);
        let previous = all_records.iter().rev().find(|candidate| {
            record_matches_file(candidate, path_str_arg, path, root)
                && shift_timestamp(candidate) < timestamp
        })?;
        let previous_after = previous.pointer("/after/sha256").and_then(Value::as_str)?;
        if previous_after != before {
            return Some(json!({
                "conflict": "stale_before_sha",
                "session_shift_id": record.get("shift_id").and_then(Value::as_str),
                "session_before_sha256": before,
                "previous_writer_session_id": previous.get("session_id").and_then(Value::as_str),
                "previous_writer_shift_id": previous.get("shift_id").and_then(Value::as_str),
                "previous_after_sha256": previous_after
            }));
        }
    }
    None
}

fn record_matches_file(record: &Value, path_str_arg: &str, path: &Path, root: &Path) -> bool {
    let Some(subject_path) = record.pointer("/subject/path").and_then(Value::as_str) else {
        return false;
    };
    subject_path == path_str_arg || root.join(subject_path) == path
}

fn shift_timestamp(record: &Value) -> u128 {
    record
        .get("timestamp_unix_ns")
        .and_then(Value::as_u64)
        .map(u128::from)
        .or_else(|| {
            record
                .get("timestamp_unix_ns")
                .and_then(Value::as_str)
                .and_then(|raw| raw.parse::<u128>().ok())
        })
        .unwrap_or(0)
}

pub(crate) fn normalize_session_id_hex32(session_id: &str) -> String {
    if session_id.len() == 32 && session_id.as_bytes().iter().all(|b| b.is_ascii_hexdigit()) {
        let lower = session_id.to_ascii_lowercase();
        if lower != "00000000000000000000000000000000" {
            return lower;
        }
    }
    let input = if session_id.is_empty() {
        "cgreality-hook-session-unknown"
    } else {
        session_id
    };
    let digest = Sha256::digest(input.as_bytes());
    hex::encode(&digest[..16])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_read_shift_file() {
        let root = tempfile::tempdir().expect("tempdir");
        let mut shift =
            ShiftRecord::new("unit", "phase4-shift-test", json!({"type": "unit"})).expect("shift");
        shift.after = json!({"sha256": "sha256:abc"});
        append_shift(root.path(), "phase4-shift-test", &shift).expect("append");
        let normalized = normalize_session_id_hex32("phase4-shift-test");
        let path = root
            .path()
            .join(format!("cgreality-shift-log/{normalized}.jsonl"));
        let raw = std::fs::read_to_string(path).expect("read");
        assert!(raw.contains(&shift.shift_id));
        assert!(raw.contains(&normalized));
    }

    #[test]
    fn session_normalization_is_stable_and_preserves_valid_hex32() {
        assert_eq!(
            normalize_session_id_hex32("0123456789ABCDEF0123456789ABCDEF"),
            "0123456789abcdef0123456789abcdef"
        );
        assert_eq!(
            normalize_session_id_hex32("phase4-shift-test"),
            "072aefeec4fb0227ab59c00233d37483"
        );
    }

    #[test]
    fn stale_before_sha_surfaces_concurrent_edit_conflict() {
        let root = std::path::PathBuf::from("/repo");
        let path = root.join("src/lib.rs");
        let session_b = vec![json!({
            "shift_id": "shift-b",
            "timestamp_unix_ns": 20u64,
            "session_id": "session-b",
            "subject": {"path": "src/lib.rs"},
            "before": {"sha256": "sha256:base"},
            "after": {"sha256": "sha256:b"}
        })];
        let all = vec![
            json!({
                "shift_id": "shift-a",
                "timestamp_unix_ns": 10u64,
                "session_id": "session-a",
                "subject": {"path": "src/lib.rs"},
                "before": {"sha256": "sha256:base"},
                "after": {"sha256": "sha256:a"}
            }),
            session_b[0].clone(),
        ];
        let conflict =
            stale_write_conflict_for_session(&session_b, &all, "src/lib.rs", &path, &root)
                .expect("stale write conflict");
        assert_eq!(conflict["conflict"], json!("stale_before_sha"));
        assert_eq!(conflict["previous_writer_session_id"], json!("session-a"));
    }
}
