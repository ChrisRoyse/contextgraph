use super::errors::{CCRealityError, Result};
use super::helpers::*;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use rusqlite::OptionalExtension;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

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

    pub(crate) async fn call_dynamicjepa_reality_for_attempt(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match dynamicjepa_reality_for_attempt(args).await {
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
    let compact = optional_bool_strict(&args, "compact", false)?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let path = attempt_dir(&runtime_root, &run_id, &task, attempt)?.join("attempt-summary.json");
    let mut body = read_json(&path)?;
    if compact {
        body = compact_attempt_summary(&body);
    }
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
    let compact = optional_bool_strict(&args, "compact", false)?;
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
    let compact = optional_bool_strict(&args, "compact", false)?;
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

pub async fn dynamicjepa_reality_for_attempt(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let dir = attempt_dir(&runtime_root, &run_id, &task, attempt)?;
    let packet_path = dir.join("reality-signal-packet.json");
    let packet = read_json(&packet_path)?;
    let block = packet
        .get("dynamicjepa_reality")
        .cloned()
        .ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_DYNAMICJEPA_REALITY_MISSING",
                "reality-signal packet is missing dynamicjepa_reality",
                "reality_signal_packet.dynamicjepa_reality",
                "rerun the attempt with a reality-loop build that records DynamicJEPA reality before scoring",
                json!({"packet": packet_path.display().to_string(), "run_id": run_id, "attempt": attempt}),
                Some(file_sot(&packet_path)),
            )
        })?;
    if block
        .pointer("/ledger_record/readback/status")
        .and_then(Value::as_str)
        != Some("passed")
    {
        return Err(CCRealityError::new(
            "CCREALITY_DYNAMICJEPA_REALITY_READBACK_NOT_PASSED",
            "dynamicjepa_reality ledger readback did not pass",
            "dynamicjepa_reality.ledger_record.readback.status",
            "inspect the ledger row before trusting DynamicJEPA reward features",
            json!({"dynamicjepa_reality": block, "packet": file_sot(&packet_path)}),
            Some(file_sot(&packet_path)),
        ));
    }
    let ledger = super::interact::find_ledger_for_run(&runtime_root, &run_id, Some(&task))?;
    let sqlite_verification =
        verify_dynamicjepa_reality_sqlite_row(&ledger, &run_id, &packet_path, &block)?;
    Ok(json!({
        "status": "ok",
        "dynamicjepa_reality": block,
        "sqlite_verification": sqlite_verification,
        "source_of_truth": file_sot(&packet_path),
        "sha256": sha256_file(&packet_path)?,
    }))
}

#[derive(Debug)]
struct DynamicJepaRealityLedgerRow {
    sequence: i64,
    record_kind: String,
    run_id: Option<String>,
    payload_sha256: String,
    record_sha256: String,
    payload: Value,
}

fn verify_dynamicjepa_reality_sqlite_row(
    ledger: &Path,
    run_id: &str,
    packet_path: &Path,
    packet_block: &Value,
) -> Result<Value> {
    let expected_payload = packet_block.get("block").ok_or_else(|| {
        dynamicjepa_sqlite_error(
            "CCREALITY_DYNAMICJEPA_PACKET_BLOCK_MISSING",
            packet_path,
            "dynamicjepa_reality.block",
            "reality-signal packet dynamicjepa_reality block is missing the persisted payload",
            "rerun the attempt with a current reality-loop build that records typed DynamicJEPA reality",
            json!({"packet": file_sot(packet_path), "dynamicjepa_reality": packet_block}),
            Some(file_sot(packet_path)),
        )
    })?;
    let expected_sequence = packet_block
        .pointer("/ledger_record/ledger_sequence")
        .and_then(Value::as_i64)
        .ok_or_else(|| {
            dynamicjepa_sqlite_error(
                "CCREALITY_DYNAMICJEPA_LEDGER_SEQUENCE_MISSING",
                packet_path,
                "dynamicjepa_reality.ledger_record.ledger_sequence",
                "dynamicjepa_reality packet is missing the ledger sequence to verify",
                "rerun the attempt so the packet includes the ledger readback record identity",
                json!({"packet": file_sot(packet_path), "dynamicjepa_reality": packet_block}),
                Some(file_sot(packet_path)),
            )
        })?;
    let expected_payload_sha256 = packet_block
        .pointer("/ledger_record/payload_sha256")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            dynamicjepa_sqlite_error(
                "CCREALITY_DYNAMICJEPA_PAYLOAD_HASH_MISSING",
                packet_path,
                "dynamicjepa_reality.ledger_record.payload_sha256",
                "dynamicjepa_reality packet is missing payload_sha256",
                "rerun the attempt so the packet includes the ledger append hash",
                json!({"packet": file_sot(packet_path), "dynamicjepa_reality": packet_block}),
                Some(file_sot(packet_path)),
            )
        })?;
    let expected_record_sha256 = packet_block
        .pointer("/ledger_record/record_sha256")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            dynamicjepa_sqlite_error(
                "CCREALITY_DYNAMICJEPA_RECORD_HASH_MISSING",
                packet_path,
                "dynamicjepa_reality.ledger_record.record_sha256",
                "dynamicjepa_reality packet is missing record_sha256",
                "rerun the attempt so the packet includes the ledger append hash",
                json!({"packet": file_sot(packet_path), "dynamicjepa_reality": packet_block}),
                Some(file_sot(packet_path)),
            )
        })?;

    let expected_payload_canonical_sha256 =
        canonical_json_sha256(expected_payload).map_err(|details| {
            dynamicjepa_sqlite_error(
                "CCREALITY_DYNAMICJEPA_PACKET_PAYLOAD_HASH_FAILED",
                packet_path,
                "dynamicjepa_reality.block",
                "failed to independently hash packet DynamicJEPA payload",
                "inspect the packet payload JSON before trusting reward features",
                details,
                Some(file_sot(packet_path)),
            )
        })?;
    if expected_payload_canonical_sha256 != expected_payload_sha256 {
        return Err(dynamicjepa_sqlite_error(
            "CCREALITY_DYNAMICJEPA_PACKET_PAYLOAD_HASH_MISMATCH",
            packet_path,
            "dynamicjepa_reality.ledger_record.payload_sha256",
            "packet payload hash does not match the packet's ledger_record payload_sha256",
            "rebuild the packet from the physical ledger row",
            json!({
                "packet": file_sot(packet_path),
                "expected_from_block": expected_payload_canonical_sha256,
                "packet_ledger_hash": expected_payload_sha256,
            }),
            Some(file_sot(packet_path)),
        ));
    }

    let conn =
        rusqlite::Connection::open_with_flags(ledger, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|err| {
            dynamicjepa_rusqlite_error("CCREALITY_DYNAMICJEPA_LEDGER_OPEN_FAILED", ledger, err)
        })?;
    let row = conn
        .query_row(
            "SELECT sequence, record_kind, run_id, payload_sha256, record_sha256, payload_json
             FROM ledger_records
             WHERE sequence = ?1",
            rusqlite::params![expected_sequence],
            |row| {
                let payload_json: String = row.get(5)?;
                let payload = serde_json::from_str::<Value>(&payload_json).map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        payload_json.len(),
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?;
                Ok(DynamicJepaRealityLedgerRow {
                    sequence: row.get(0)?,
                    record_kind: row.get(1)?,
                    run_id: row.get(2)?,
                    payload_sha256: row.get(3)?,
                    record_sha256: row.get(4)?,
                    payload,
                })
            },
        )
        .optional()
        .map_err(|err| {
            dynamicjepa_rusqlite_error("CCREALITY_DYNAMICJEPA_LEDGER_QUERY_FAILED", ledger, err)
        })?
        .ok_or_else(|| {
            dynamicjepa_sqlite_error(
                "CCREALITY_DYNAMICJEPA_LEDGER_ROW_MISSING",
                ledger,
                "ledger_records.sequence",
                format!(
                    "dynamicjepa_reality ledger row sequence {expected_sequence} was not found"
                ),
                "inspect the active runtime root and ledger path before trusting the packet",
                json!({"ledger": ledger.display().to_string(), "sequence": expected_sequence}),
                Some(format!("sqlite:{}#ledger_records", ledger.display())),
            )
        })?;

    let mut mismatches = Vec::new();
    if row.record_kind != "dynamicjepa_reality" {
        mismatches.push(json!({
            "field": "record_kind",
            "expected": "dynamicjepa_reality",
            "actual": row.record_kind,
        }));
    }
    if row.run_id.as_deref() != Some(run_id) {
        mismatches.push(json!({
            "field": "run_id",
            "expected": run_id,
            "actual": row.run_id,
        }));
    }
    if row.payload_sha256 != expected_payload_sha256 {
        mismatches.push(json!({
            "field": "payload_sha256",
            "expected": expected_payload_sha256,
            "actual": row.payload_sha256,
        }));
    }
    if row.record_sha256 != expected_record_sha256 {
        mismatches.push(json!({
            "field": "record_sha256",
            "expected": expected_record_sha256,
            "actual": row.record_sha256,
        }));
    }
    if &row.payload != expected_payload {
        mismatches.push(json!({
            "field": "payload_json",
            "expected_sha256": expected_payload_sha256,
            "actual_payload_sha256": canonical_json_sha256(&row.payload).unwrap_or_else(|_| "sha256:hash_failed".to_string()),
        }));
    }
    if !mismatches.is_empty() {
        return Err(dynamicjepa_sqlite_error(
            "CCREALITY_DYNAMICJEPA_LEDGER_ROW_MISMATCH",
            ledger,
            "ledger_records.dynamicjepa_reality",
            "physical SQLite dynamicjepa_reality row does not match the reality-signal packet",
            "discard the packet and rebuild it from the append-only ledger row",
            json!({
                "ledger": ledger.display().to_string(),
                "packet": file_sot(packet_path),
                "sequence": expected_sequence,
                "mismatches": mismatches,
            }),
            Some(format!("sqlite:{}#ledger_records", ledger.display())),
        ));
    }
    Ok(json!({
        "status": "passed",
        "source_of_truth": format!("sqlite:{}#ledger_records", ledger.display()),
        "sequence": row.sequence,
        "record_kind": "dynamicjepa_reality",
        "run_id": run_id,
        "payload_sha256": expected_payload_sha256,
        "record_sha256": expected_record_sha256,
    }))
}

fn dynamicjepa_rusqlite_error(
    code: &'static str,
    ledger: &Path,
    err: rusqlite::Error,
) -> CCRealityError {
    dynamicjepa_sqlite_error(
        code,
        ledger,
        "ledger_records.dynamicjepa_reality",
        format!("SQLite verification failed: {err}"),
        "inspect the ledger path, schema, and row identity before trusting DynamicJEPA reality",
        json!({"ledger": ledger.display().to_string(), "error": err.to_string()}),
        Some(format!("sqlite:{}#ledger_records", ledger.display())),
    )
}

fn dynamicjepa_sqlite_error(
    code: &'static str,
    path: &Path,
    field_path: impl Into<String>,
    message: impl Into<String>,
    remediation: impl Into<String>,
    details: Value,
    source_of_truth: Option<String>,
) -> CCRealityError {
    CCRealityError::new(
        code,
        message,
        field_path,
        remediation,
        {
            let mut out = details;
            if let Value::Object(map) = &mut out {
                map.entry("path".to_string())
                    .or_insert_with(|| json!(path.display().to_string()));
            }
            out
        },
        source_of_truth,
    )
}

fn canonical_json_sha256(value: &Value) -> std::result::Result<String, Value> {
    let canonical = canonicalize_json(value);
    let bytes = serde_json::to_vec(&canonical).map_err(|err| {
        json!({
            "error": err.to_string(),
            "value": value,
        })
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = BTreeMap::new();
            for (key, val) in map {
                sorted.insert(key.clone(), canonicalize_json(val));
            }
            let mut out = Map::new();
            for (key, val) in sorted {
                out.insert(key, val);
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_json).collect()),
        other => other.clone(),
    }
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
        return Err(CCRealityError::new(
            "CCREALITY_TRIGGER_DECISION_MISSING",
            "trigger-decision.json is missing for the requested attempt",
            "trigger_decision.path",
            "record an optimizer decision before reading trigger-decision evidence",
            json!({"path": path.display().to_string(), "run_id": run_id, "attempt": attempt}),
            Some(file_sot(&path)),
        ));
    }
    let mut body = read_json(&path)?;
    insert_sot(&mut body, &path);
    Ok(body)
}

pub async fn reality_harness_transitions(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let round = optional_u64_value_strict(&args, "round")?;
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
    let limit = optional_u64_strict(&args, "limit", 50)?.min(500) as usize;
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
    let limit = optional_u64_strict(&args, "limit", 50)?.min(500) as usize;
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
        return Err(CCRealityError::new(
            "CCREALITY_REPLAY_ARTIFACT_MISSING",
            "requested artifact path does not exist as a regular file",
            "replay_artifact.path",
            "verify the artifact path from the source-of-truth record before replaying it",
            json!({"path": path.display().to_string()}),
            Some(file_sot(&path)),
        ));
    }
    let meta = fs::metadata(&path)
        .map_err(|e| fs_error("CCREALITY_REPLAY_ARTIFACT_METADATA_FAILED", &path, e))?;
    let sha = sha256_file(&path)?;
    let mtime_unix = meta
        .modified()
        .map_err(|e| fs_error("CCREALITY_REPLAY_ARTIFACT_MTIME_FAILED", &path, e))?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| {
            CCRealityError::new(
                "CCREALITY_REPLAY_ARTIFACT_MTIME_INVALID",
                format!("artifact mtime is before UNIX_EPOCH: {e}"),
                "replay_artifact.mtime",
                "repair the artifact timestamp before replaying it",
                json!({"path": path.display().to_string()}),
                Some(file_sot(&path)),
            )
        })?
        .as_secs();
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
        "mtime_unix": mtime_unix,
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

fn compact_attempt_summary(summary: &Value) -> Value {
    let message = summary
        .pointer("/model_response/message/content")
        .or_else(|| summary.pointer("/model_response/message"))
        .and_then(Value::as_str);
    json!({
        "record_kind": "contextgraph_attempt_summary_compact",
        "attempt": summary.get("attempt").cloned().unwrap_or(Value::Null),
        "ledger_counts": summary.get("ledger_counts").cloned().unwrap_or(Value::Null),
        "reality_signal_verifier": compact_verifier(summary.get("reality_signal_verifier")),
        "ledger_verifier": compact_verifier(summary.get("ledger_verifier")),
        "score": summary.pointer("/score/score").cloned().unwrap_or(Value::Null),
        "reward": summary.pointer("/reward/reward").cloned().unwrap_or(Value::Null),
        "reward_components": summary.pointer("/reward/components").cloned().unwrap_or(Value::Null),
        "model_response": {
            "model": summary.pointer("/model_response/model").cloned().unwrap_or(Value::Null),
            "created_at_unix": summary.pointer("/model_response/created_at_unix").cloned().unwrap_or(Value::Null),
            "done": summary.pointer("/model_response/done").cloned().unwrap_or(Value::Null),
            "message_sha256": message.map(sha256_text),
            "message_size_chars": message.map(|value| value.chars().count()),
            "submitted_candidate_patch": summary.pointer("/model_response/contextgraph_runtime/submitted_candidate_patch").cloned().unwrap_or(Value::Null)
        }
    })
}

fn compact_verifier(verifier: Option<&Value>) -> Value {
    let Some(verifier) = verifier else {
        return Value::Null;
    };
    json!({
        "source_of_truth": verifier.get("source_of_truth").cloned().unwrap_or(Value::Null),
        "status": verifier.pointer("/verifier/status").or_else(|| verifier.pointer("/record/status")).cloned().unwrap_or(Value::Null),
        "verifier_status": verifier.pointer("/verifier/verifier_status").cloned().unwrap_or(Value::Null),
        "ledger_sequence": verifier.pointer("/record/ledger_sequence").cloned().unwrap_or(Value::Null)
    })
}

fn compact_signal(packet: &Value) -> Value {
    json!({
        "record_kind": packet.get("record_kind"),
        "attempt": packet.get("attempt"),
        "outcome_reality": packet.get("outcome_reality"),
        "patch_signals": packet.get("patch_signals"),
        "dynamicjepa_reality": packet.get("dynamicjepa_reality"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn sqlite_ledger_with_dynamicjepa_row(
        dir: &Path,
        run_id: &str,
        payload: &Value,
    ) -> (PathBuf, String, String) {
        let ledger = dir.join("ledger.sqlite");
        let conn = Connection::open(&ledger).expect("open real temp sqlite ledger");
        conn.execute_batch(
            "CREATE TABLE ledger_records (
                sequence INTEGER PRIMARY KEY,
                record_kind TEXT NOT NULL,
                run_id TEXT,
                payload_sha256 TEXT NOT NULL,
                record_sha256 TEXT NOT NULL,
                payload_json TEXT NOT NULL
            );",
        )
        .expect("create ledger_records");
        let payload_sha256 = canonical_json_sha256(payload).expect("hash payload");
        let record_sha256 =
            "sha256:1111111111111111111111111111111111111111111111111111111111111111";
        conn.execute(
            "INSERT INTO ledger_records
             (sequence, record_kind, run_id, payload_sha256, record_sha256, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                7_i64,
                "dynamicjepa_reality",
                run_id,
                payload_sha256,
                record_sha256,
                serde_json::to_string(payload).expect("serialize payload")
            ],
        )
        .expect("insert dynamicjepa_reality row");
        (ledger, payload_sha256, record_sha256.to_string())
    }

    fn packet_block(payload: Value, payload_sha256: &str, record_sha256: &str) -> Value {
        json!({
            "block": payload,
            "ledger_record": {
                "ledger_sequence": 7,
                "payload_sha256": payload_sha256,
                "record_sha256": record_sha256,
                "readback": {"status": "passed"}
            }
        })
    }

    #[test]
    fn dynamicjepa_sqlite_verification_reads_physical_row() {
        let temp = tempfile::tempdir().expect("tempdir");
        let run_id = "run_mcp_sqlite_fsv";
        let payload = json!({
            "run_id": run_id,
            "source_mode": "trained_dynamicjepa_artifact",
            "reward_features": {
                "dynamicjepa_prediction_accuracy": 0.75
            }
        });
        let (ledger, payload_sha256, record_sha256) =
            sqlite_ledger_with_dynamicjepa_row(temp.path(), run_id, &payload);
        let packet_path = temp.path().join("reality-signal-packet.json");
        let verified = verify_dynamicjepa_reality_sqlite_row(
            &ledger,
            run_id,
            &packet_path,
            &packet_block(payload, &payload_sha256, &record_sha256),
        )
        .expect("physical SQLite row verifies");

        assert_eq!(verified["status"], "passed");
        assert_eq!(
            verified["source_of_truth"],
            format!("sqlite:{}#ledger_records", ledger.display())
        );
        assert_eq!(verified["sequence"], 7);
        assert_eq!(verified["record_kind"], "dynamicjepa_reality");
    }

    #[test]
    fn dynamicjepa_sqlite_verification_rejects_packet_row_divergence() {
        let temp = tempfile::tempdir().expect("tempdir");
        let run_id = "run_mcp_sqlite_mismatch";
        let packet_payload = json!({
            "run_id": run_id,
            "source_mode": "trained_dynamicjepa_artifact",
            "reward_features": {
                "dynamicjepa_prediction_accuracy": 0.80
            }
        });
        let persisted_payload = json!({
            "run_id": run_id,
            "source_mode": "harness_transition_projection",
            "reward_features": {
                "dynamicjepa_prediction_accuracy": null
            }
        });
        let (ledger, _persisted_payload_sha256, record_sha256) =
            sqlite_ledger_with_dynamicjepa_row(temp.path(), run_id, &persisted_payload);
        let expected_packet_payload_sha256 =
            canonical_json_sha256(&packet_payload).expect("hash packet payload");
        let packet_path = temp.path().join("reality-signal-packet.json");
        let err = verify_dynamicjepa_reality_sqlite_row(
            &ledger,
            run_id,
            &packet_path,
            &packet_block(
                packet_payload,
                &expected_packet_payload_sha256,
                &record_sha256,
            ),
        )
        .expect_err("packet/row divergence must fail closed");

        assert_eq!(err.error_code, "CCREALITY_DYNAMICJEPA_LEDGER_ROW_MISMATCH");
        assert_eq!(
            err.source_of_truth,
            Some(format!("sqlite:{}#ledger_records", ledger.display()))
        );
    }
}
