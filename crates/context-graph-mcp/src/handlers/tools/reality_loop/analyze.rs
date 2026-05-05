use super::errors::{CCRealityError, Result};
use super::helpers::*;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

impl Handlers {
    pub(crate) async fn call_reality_latest_root(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_latest_root(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_attempt_summary(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_attempt_summary(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_official_report(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_official_report(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_problem_packet(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_problem_packet(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_signal(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_signal(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_failure(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_failure(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_trigger_decision(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_trigger_decision(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_harness_transitions(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_harness_transitions(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_compare_attempts(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_compare_attempts(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_audit_trail(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_audit_trail(self, args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_replay_artifact(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_replay_artifact(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }
}

pub async fn reality_latest_root(_args: Value) -> Result<Value> {
    let runtime_root = read_active_runtime_root().await?;
    let target = read_active_target_instance().await?;
    let active_run_id = match runtime_root.as_ref() {
        Some(r) => latest_run_id(r)?,
        None => None,
    };
    let last_attempt_number = match runtime_root.as_ref() {
        Some(r) => latest_attempt_number(r, target.as_deref())?,
        None => None,
    };
    Ok(json!({
        "runtime_root": runtime_root.as_ref().map(|p| p.display().to_string()),
        "active_run_id": active_run_id,
        "active_task_id": target.clone(),
        "active_target_instance": target,
        "success_predicate": "official SWE-bench report places target instance in resolved_ids",
        "last_attempt_number": last_attempt_number,
        "source_of_truth": runtime_root.as_ref().map(|p| file_sot(p))
    }))
}

pub async fn reality_attempt_summary(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let path = attempt_dir(&runtime_root, &run_id, &task, attempt)?.join("attempt-summary.json");
    let mut body = read_json(&path)?;
    insert_sot(&mut body, &path);
    Ok(body)
}

pub async fn reality_official_report(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let dir = attempt_dir(&runtime_root, &run_id, &task, attempt)?;
    let mut candidates = Vec::new();
    collect_named_json(&dir, "official-success-evidence.json", &mut candidates)?;
    collect_named_json(&dir, "official-failure-evidence.json", &mut candidates)?;
    if candidates.is_empty() {
        return Err(CCRealityError::new(
            "CCREALITY_OFFICIAL_REPORT_MISSING",
            "official evidence report is missing for attempt",
            "official_report.path",
            "run the attempt with official evaluation enabled",
            json!({"attempt_dir": dir.display().to_string()}),
            Some(file_sot(&dir)),
        ));
    }
    let path = candidates.remove(0);
    let mut body = read_json(&path)?;
    insert_sot(&mut body, &path);
    Ok(body)
}

pub async fn reality_problem_packet(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let compact = optional_bool(&args, "compact", false);
    let runtime_root = require_active_runtime_root().await?;
    let run_dir = runtime_root.join(&run_id);
    let mut candidates = Vec::new();
    collect_named_json(&run_dir, "problem-reality-packet.json", &mut candidates)?;
    let path = candidates.into_iter().next().ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_PROBLEM_PACKET_MISSING",
            "problem-reality-packet.json is missing for run",
            "problem_packet.path",
            "verify run_id and rerun the engine if artifacts are missing",
            json!({"run_dir": run_dir.display().to_string()}),
            Some(file_sot(&run_dir)),
        )
    })?;
    let mut body = read_json(&path)?;
    if compact {
        body = compact_problem(&body);
    }
    insert_sot(&mut body, &path);
    Ok(body)
}

pub async fn reality_signal(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let compact = optional_bool(&args, "compact", false);
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let path =
        attempt_dir(&runtime_root, &run_id, &task, attempt)?.join("reality-signal-packet.json");
    let mut body = read_json(&path)?;
    if compact {
        body = compact_signal(&body);
    }
    insert_sot(&mut body, &path);
    Ok(body)
}

pub async fn reality_failure(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let dir = attempt_dir(&runtime_root, &run_id, &task, attempt)?;
    let validation_path = dir.join("validation.json");
    if validation_path.is_file() {
        let validation = read_json(&validation_path)?;
        return Ok(json!({
            "failure_evidence": compact_failure(&validation),
            "source_of_truth": file_sot(&validation_path)
        }));
    }
    let summary_path = dir.join("attempt-summary.json");
    let summary = read_json(&summary_path)?;
    Ok(json!({
        "failure_evidence": summary.pointer("/attempt/failure_evidence").or_else(|| summary.get("validation")).cloned().unwrap_or(Value::Null),
        "source_of_truth": file_sot(&summary_path)
    }))
}

pub async fn reality_trigger_decision(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let path = attempt_dir(&runtime_root, &run_id, &task, attempt)?.join("trigger-decision.json");
    if !path.is_file() {
        return Ok(json!({"status": "no_decision_recorded", "source_of_truth": Value::Null}));
    }
    let mut body = read_json(&path)?;
    insert_sot(&mut body, &path);
    Ok(body)
}

pub async fn reality_harness_transitions(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let round = args.get("round").and_then(Value::as_u64);
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let dir = attempt_dir(&runtime_root, &run_id, &task, attempt)?.join("harness-transitions");
    if let Some(round) = round {
        let path = dir.join(format!("round-{round:02}.json"));
        let mut body = read_json(&path)?;
        insert_sot(&mut body, &path);
        return Ok(body);
    }
    if !dir.is_dir() {
        return Ok(json!({"transitions": [], "source_of_truth": file_sot(&dir)}));
    }
    let mut transitions = Vec::new();
    for entry in fs::read_dir(&dir)
        .map_err(|e| fs_error("CCREALITY_HARNESS_TRANSITIONS_READ_FAILED", &dir, e))?
    {
        let entry =
            entry.map_err(|e| fs_error("CCREALITY_HARNESS_TRANSITION_ENTRY_FAILED", &dir, e))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let body = read_json(&path)?;
        transitions.push(json!({
            "round": body.get("round"),
            "tool_use_id": body.get("tool_use_id"),
            "file_path": body.get("file_path"),
            "outcome": body.get("outcome").or_else(|| body.get("status")),
            "source_of_truth": file_sot(&path)
        }));
    }
    Ok(json!({"transitions": transitions, "source_of_truth": file_sot(&dir)}))
}

pub async fn reality_compare_attempts(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let a = required_u64(&args, "attempt_a")?;
    let b = required_u64(&args, "attempt_b")?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let dir_a = attempt_dir(&runtime_root, &run_id, &task, a)?;
    let dir_b = attempt_dir(&runtime_root, &run_id, &task, b)?;
    let sum_a = read_json(&dir_a.join("attempt-summary.json"))?;
    let sum_b = read_json(&dir_b.join("attempt-summary.json"))?;
    let reward_a = sum_a
        .pointer("/attempt/reality_signal_compact/reward_signal/clipped")
        .or_else(|| sum_a.pointer("/reward/reward/clipped"))
        .and_then(Value::as_f64);
    let reward_b = sum_b
        .pointer("/attempt/reality_signal_compact/reward_signal/clipped")
        .or_else(|| sum_b.pointer("/reward/reward/clipped"))
        .and_then(Value::as_f64);
    Ok(json!({
        "patch_diff_summary": {
            "lines_added": 0,
            "lines_removed": 0,
            "files_changed": []
        },
        "validation_status_change": {
            "from": sum_a.pointer("/attempt/status").or_else(|| sum_a.pointer("/validation/status")),
            "to": sum_b.pointer("/attempt/status").or_else(|| sum_b.pointer("/validation/status"))
        },
        "official_resolved_change": {
            "from": sum_a.pointer("/attempt/resolved"),
            "to": sum_b.pointer("/attempt/resolved")
        },
        "reward_delta": match (reward_a, reward_b) { (Some(x), Some(y)) => json!(y - x), _ => Value::Null },
        "patch_sha_a": sum_a.pointer("/attempt/patch_sha256"),
        "patch_sha_b": sum_b.pointer("/attempt/patch_sha256"),
        "patch_repeated": sum_a.pointer("/attempt/patch_sha256") == sum_b.pointer("/attempt/patch_sha256"),
        "source_of_truth": [file_sot(&dir_a.join("attempt-summary.json")), file_sot(&dir_b.join("attempt-summary.json"))]
    }))
}

pub async fn reality_audit_trail(handlers: &Handlers, args: Value) -> Result<Value> {
    let entity_id = required_str(&args, "entity_id")?;
    let limit = optional_u64(&args, "limit", 50).min(500) as usize;
    let uuid = uuid::Uuid::parse_str(&entity_id).map_err(|e| {
        CCRealityError::new(
            "CCREALITY_AUDIT_ENTITY_ID_INVALID",
            format!("entity_id is not a UUID: {e}"),
            "arguments.entity_id",
            "provide a valid memory UUID",
            json!({"entity_id": entity_id}),
            None,
        )
    })?;
    let records = handlers
        .teleological_store
        .get_audit_by_target(uuid, limit)
        .await
        .map_err(|e| {
            CCRealityError::new(
                "CCREALITY_AUDIT_TRAIL_READ_FAILED",
                format!("failed to read audit trail: {e}"),
                "audit_trail.store",
                "inspect RocksDB audit column family",
                json!({"entity_id": entity_id}),
                Some("rocksdb:CF_AUDIT_LOG".to_string()),
            )
        })?;
    Ok(json!({
        "audit_trail": records,
        "count": records.len(),
        "entity_id": entity_id,
        "source_of_truth": "rocksdb:CF_AUDIT_LOG"
    }))
}

#[allow(dead_code)]
pub async fn reality_runtime_audit_trail(args: Value) -> Result<Value> {
    let entity_id = required_str(&args, "entity_id")?;
    let limit = optional_u64(&args, "limit", 50).min(500) as usize;
    let uuid = uuid::Uuid::parse_str(&entity_id).map_err(|e| {
        CCRealityError::new(
            "CCREALITY_AUDIT_ENTITY_ID_INVALID",
            format!("entity_id is not a UUID: {e}"),
            "arguments.entity_id",
            "provide a valid audit entity UUID",
            json!({"entity_id": entity_id}),
            None,
        )
    })?;
    let runtime_root = require_active_runtime_root().await?;
    let path = runtime_root
        .join("cgreality-audit-log")
        .join(format!("{uuid}.jsonl"));
    if !path.is_file() {
        return Err(CCRealityError::new(
            "CCREALITY_RUNTIME_AUDIT_TRAIL_MISSING",
            "runtime audit trail is missing for entity",
            "runtime_audit.path",
            "write a cgreality runtime audit record before querying the lightweight server",
            json!({"entity_id": entity_id, "path": path.display().to_string()}),
            Some(file_sot(&path)),
        ));
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| fs_error("CCREALITY_RUNTIME_AUDIT_TRAIL_READ_FAILED", &path, e))?;
    let mut records = Vec::new();
    for (idx, line) in raw.lines().enumerate() {
        let record: Value = serde_json::from_str(line).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_RUNTIME_AUDIT_TRAIL_JSON_INVALID",
                format!("invalid runtime audit JSON at line {}: {e}", idx + 1),
                "runtime_audit.parse",
                "repair or remove the corrupt audit log line",
                json!({"path": path.display().to_string(), "line": idx + 1}),
                Some(file_sot(&path)),
            )
        })?;
        records.push(record);
    }
    records.truncate(limit);
    Ok(json!({
        "audit_trail": records,
        "count": records.len(),
        "entity_id": entity_id,
        "source_of_truth": file_sot(&path)
    }))
}

pub async fn reality_replay_artifact(args: Value) -> Result<Value> {
    let path_arg = required_str(&args, "path")?;
    let path = file_arg_to_path(&path_arg);
    if !path.is_file() {
        return Ok(json!({
            "exists": false,
            "path": path.display().to_string(),
            "source_of_truth": file_sot(&path)
        }));
    }
    let meta = fs::metadata(&path)
        .map_err(|e| fs_error("CCREALITY_REPLAY_ARTIFACT_METADATA_FAILED", &path, e))?;
    let sha = sha256_file(&path)?;
    let content_or_pointer = if meta.len() < 32 * 1024 {
        Value::String(
            fs::read_to_string(&path)
                .map_err(|e| fs_error("CCREALITY_REPLAY_ARTIFACT_READ_FAILED", &path, e))?,
        )
    } else {
        json!({"pointer": file_sot(&path), "reason": "file larger than inline limit"})
    };
    Ok(json!({
        "exists": true,
        "sha256": sha,
        "size_bytes": meta.len(),
        "mtime_unix": meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()),
        "content_or_pointer": content_or_pointer,
        "source_of_truth": file_sot(&path)
    }))
}

fn insert_sot(value: &mut Value, path: &std::path::Path) {
    if let Value::Object(map) = value {
        map.insert("source_of_truth".to_string(), Value::String(file_sot(path)));
    }
}

fn compact_problem(packet: &Value) -> Value {
    json!({
        "record_kind": packet.get("record_kind"),
        "task_identity": packet.get("task_identity").or_else(|| packet.get("task_id")),
        "oracle_reality": packet.get("oracle_reality"),
        "problem_terms": packet.pointer("/problem_statement_reality/terms"),
        "production_candidate_files": packet.pointer("/repository_reality/production_candidate_files"),
        "source_of_truth": packet.get("source_of_truth")
    })
}

fn compact_signal(packet: &Value) -> Value {
    json!({
        "record_kind": packet.get("record_kind"),
        "attempt": packet.get("attempt"),
        "outcome_reality": packet.get("outcome_reality"),
        "patch_signals": packet.get("patch_signals"),
        "score_reality": packet.get("score_reality"),
        "reinjection_directive": packet.get("reinjection_directive"),
        "source_of_truth": packet.get("source_of_truth")
    })
}

fn compact_failure(validation: &Value) -> Value {
    json!({
        "status": validation.get("status"),
        "error_code": validation.get("error_code"),
        "message": validation.get("message"),
        "remediation": validation.get("remediation"),
        "source_of_truth": validation.get("source_of_truth")
    })
}

fn collect_named_json(
    root: &std::path::Path,
    file_name: &str,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in
        fs::read_dir(root).map_err(|e| fs_error("CCREALITY_NAMED_JSON_READ_DIR_FAILED", root, e))?
    {
        let entry = entry.map_err(|e| fs_error("CCREALITY_NAMED_JSON_ENTRY_FAILED", root, e))?;
        let path = entry.path();
        if path.is_dir() {
            collect_named_json(&path, file_name, out)?;
        } else if path.file_name().and_then(|n| n.to_str()) == Some(file_name) {
            out.push(path);
        }
    }
    Ok(())
}
