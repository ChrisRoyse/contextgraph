//! SHAKE-256 witness chain for ccreality optimizer writes.
//!
//! Source of truth: `<runtime_root>/<run_id>/claude-code-optimizer/witness-chain.bin`.

use super::errors::{CCRealityError, Result};
use super::helpers::*;
use super::witness_chain_format::{ensure_canonical_format_manifest, read_format_manifest_status};
use super::witness_chain_io::{read_chain_bytes, sync_parent_dir, with_chain_lock};
use super::witness_chain_legacy::verify_legacy_chain_bytes_with_type_validator;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use context_graph_witness::{
    HASH_SIZE, WITNESS_ENTRY_SIZE, WitnessEntry, ZERO_HASH, hex_hash as witness_hex_hash,
    verify_chain_bytes_with_type_validator,
};
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WitnessOpType {
    Decision = 0,
    Recommendation = 1,
    HarnessTransition = 2,
    BanditSelect = 3,
    BanditReward = 4,
    Autoresearch = 5,
    InfluenceComputation = 6,
    RecommendationRecall = 7,
    WitnessRepair = 8,
}

impl WitnessOpType {
    pub(super) fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Decision),
            1 => Some(Self::Recommendation),
            2 => Some(Self::HarnessTransition),
            3 => Some(Self::BanditSelect),
            4 => Some(Self::BanditReward),
            5 => Some(Self::Autoresearch),
            6 => Some(Self::InfluenceComputation),
            7 => Some(Self::RecommendationRecall),
            8 => Some(Self::WitnessRepair),
            _ => None,
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Decision => "decision",
            Self::Recommendation => "recommendation",
            Self::HarnessTransition => "harness_transition",
            Self::BanditSelect => "bandit_select",
            Self::BanditReward => "bandit_reward",
            Self::Autoresearch => "autoresearch",
            Self::InfluenceComputation => "influence_computation",
            Self::RecommendationRecall => "recommendation_recall",
            Self::WitnessRepair => "witness_repair",
        }
    }
}

impl Handlers {
    pub(crate) async fn call_optimizer_witness_chain_verify(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match optimizer_witness_chain_verify(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_optimizer_witness_chain_diff(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match optimizer_witness_chain_diff(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_optimizer_witness_chain_repair_legacy(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match super::witness_chain_repair::optimizer_witness_chain_repair_legacy(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }
}

pub fn append_witness_entry_for_run(
    runtime_root: &Path,
    run_id: &str,
    op_type: WitnessOpType,
    content_sha256: &str,
) -> Result<Value> {
    let path = witness_chain_path(runtime_root, run_id);
    let content_hash = parse_sha256_hex(content_sha256)?;
    append_entry(&path, op_type, content_hash)
}

pub(super) fn verify_witness_chain_for_run(runtime_root: &Path, run_id: &str) -> Result<Value> {
    let path = witness_chain_path(runtime_root, run_id);
    let verification = verify_chain(&path)?;
    Ok(json!({
        "valid": true,
        "entries": verification.entries,
        "last_chain_hash": chain_hash_hex(&verification.last_chain_hash),
        "last_op": verification.last_op,
        "format_manifest": read_format_manifest_status(&path)?,
        "source_of_truth": file_sot(&path),
        "sha256": sha256_file(&path)?,
    }))
}

pub async fn optimizer_witness_chain_verify(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let path = active_witness_chain_path(&args).await?;
    let verification = verify_chain(&path)?;
    Ok(json!({
        "status": "ok",
        "valid": true,
        "entry_size": WITNESS_ENTRY_SIZE,
        "entries": verification.entries,
        "last_chain_hash": chain_hash_hex(&verification.last_chain_hash),
        "last_op": verification.last_op,
        "format_manifest": read_format_manifest_status(&path)?,
        "source_of_truth": file_sot(&path),
        "sha256": sha256_file(&path)?,
    }))
}

pub async fn optimizer_witness_chain_diff(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let since_offset = required_u64(&args, "since_offset")? as usize;
    let path = active_witness_chain_path(&args).await?;
    let bytes = read_chain_bytes(&path)?;
    if bytes.len() % WITNESS_ENTRY_SIZE != 0 {
        return Err(invalid_chain_length(&path, bytes.len()));
    }
    let entry_count = bytes.len() / WITNESS_ENTRY_SIZE;
    if since_offset > entry_count {
        return Err(CCRealityError::new(
            "CCREALITY_WITNESS_DIFF_OFFSET_OUT_OF_RANGE",
            "since_offset is beyond the end of witness-chain.bin",
            "arguments.since_offset",
            "request an offset no larger than the current entry count",
            json!({"since_offset": since_offset, "entries": entry_count}),
            Some(file_sot(&path)),
        ));
    }
    let verification = verify_chain(&path)?;
    let mut entries = Vec::new();
    for idx in since_offset..entry_count {
        let start = idx * WITNESS_ENTRY_SIZE;
        let entry = WitnessEntry::from_bytes(&bytes[start..start + WITNESS_ENTRY_SIZE])
            .map_err(|err| witness_error(&path, err))?;
        entries.push(entry_json(idx as u64, &entry, &entry.chain_hash()));
    }
    Ok(json!({
        "status": "ok",
        "since_offset": since_offset,
        "entries": entries,
        "total_entries": verification.entries,
        "last_chain_hash": chain_hash_hex(&verification.last_chain_hash),
        "source_of_truth": file_sot(&path),
    }))
}

fn append_entry(
    path: &Path,
    op_type: WitnessOpType,
    content_sha256: [u8; HASH_SIZE],
) -> Result<Value> {
    with_chain_lock(path, || append_entry_locked(path, op_type, content_sha256))
}

fn append_entry_locked(
    path: &Path,
    op_type: WitnessOpType,
    content_sha256: [u8; HASH_SIZE],
) -> Result<Value> {
    let before = verify_chain_for_append(path)?;
    let format_manifest = ensure_canonical_format_manifest(path)?;
    let offset = before.entries;
    let entry = WitnessEntry::new(
        before.last_chain_hash,
        content_sha256,
        unix_secs()?.checked_mul(1_000_000_000).ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_WITNESS_TIMESTAMP_OVERFLOW",
                "witness timestamp overflowed nanoseconds conversion",
                "witness_chain.timestamp_ns",
                "inspect system clock before recording optimizer audit state",
                json!({}),
                None,
            )
        })?,
        op_type as u8,
    );
    let entry_bytes = entry.to_bytes();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| fs_error("CCREALITY_WITNESS_OPEN_FAILED", path, e))?;
    file.write_all(&entry_bytes)
        .map_err(|e| fs_error("CCREALITY_WITNESS_APPEND_FAILED", path, e))?;
    file.sync_all()
        .map_err(|e| fs_error("CCREALITY_WITNESS_SYNC_FAILED", path, e))?;
    sync_parent_dir(path, "CCREALITY_WITNESS_PARENT_SYNC_FAILED")?;
    let after = verify_chain(path)?;
    if after.entries != offset + 1 || after.last_chain_hash != entry.chain_hash() {
        return Err(CCRealityError::new(
            "CCREALITY_WITNESS_READBACK_MISMATCH",
            "witness-chain.bin readback did not preserve appended entry",
            "witness_chain.readback",
            "inspect filesystem durability before trusting optimizer audit state",
            json!({
                "path": path.display().to_string(),
                "expected_entries": offset + 1,
                "actual_entries": after.entries,
                "expected_last_chain_hash": chain_hash_hex(&entry.chain_hash()),
                "actual_last_chain_hash": chain_hash_hex(&after.last_chain_hash),
            }),
            Some(file_sot(path)),
        ));
    }
    Ok(json!({
        "status": "ok",
        "offset": offset,
        "entry_size": WITNESS_ENTRY_SIZE,
        "op_type": op_type.as_str(),
        "content_sha256": sha256_hex(&content_sha256),
        "action_hash": chain_hash_hex(&entry.action_hash),
        "prev_chain_hash": chain_hash_hex(&entry.prev_hash),
        "chain_hash": chain_hash_hex(&after.last_chain_hash),
        "entries": after.entries,
        "format_manifest": format_manifest,
        "source_of_truth": file_sot(path),
    }))
}

#[derive(Debug)]
struct Verification {
    entries: u64,
    last_chain_hash: [u8; HASH_SIZE],
    last_op: Value,
}

fn verify_chain(path: &Path) -> Result<Verification> {
    match verify_chain_canonical(path) {
        Ok(verification) => Ok(verification),
        Err(canonical_err) => match detect_legacy_layout(path) {
            Ok(Some(legacy)) => Err(legacy_layout_detected_error(path, canonical_err, legacy)),
            Ok(None) => Err(canonical_err),
            Err(_) => Err(canonical_err),
        },
    }
}

fn verify_chain_for_append(path: &Path) -> Result<Verification> {
    if !path.exists() {
        return Ok(Verification {
            entries: 0,
            last_chain_hash: ZERO_HASH,
            last_op: Value::Null,
        });
    }
    verify_chain(path)
}

fn verify_chain_canonical(path: &Path) -> Result<Verification> {
    if !path.exists() {
        return Err(missing_chain_error(path));
    }
    let bytes = read_chain_bytes(path)?;
    if bytes.len() % WITNESS_ENTRY_SIZE != 0 {
        return Err(invalid_chain_length(path, bytes.len()));
    }
    let verification = verify_chain_bytes_with_type_validator(&bytes, |witness_type| {
        WitnessOpType::from_u8(witness_type).is_some()
    })
    .map_err(|err| witness_error(path, err))?;
    let mut last_op = Value::Null;
    let entries = bytes.len() / WITNESS_ENTRY_SIZE;
    for idx in 0..entries {
        let start = idx * WITNESS_ENTRY_SIZE;
        let entry = WitnessEntry::from_bytes(&bytes[start..start + WITNESS_ENTRY_SIZE])
            .map_err(|err| witness_error(path, err))?;
        last_op = entry_json(idx as u64, &entry, &entry.chain_hash());
    }
    Ok(Verification {
        entries: verification.entries,
        last_chain_hash: verification.last_chain_hash,
        last_op,
    })
}

fn missing_chain_error(path: &Path) -> CCRealityError {
    CCRealityError::new(
        "CCREALITY_WITNESS_CHAIN_ABSENT",
        "witness-chain.bin is absent; verification requires durable witness evidence",
        "witness_chain.source_of_truth",
        "append a witness entry first or use an explicit initialization path that is not counted as verification evidence",
        json!({"path": path.display().to_string()}),
        Some(file_sot(path)),
    )
}

fn detect_legacy_layout(
    path: &Path,
) -> Result<Option<super::witness_chain_legacy::LegacyVerification>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = read_chain_bytes(path)?;
    if bytes.is_empty() || bytes.len() % WITNESS_ENTRY_SIZE != 0 {
        return Ok(None);
    }
    match verify_legacy_chain_bytes_with_type_validator(&bytes, Some(path), unix_secs()?, |ty| {
        WitnessOpType::from_u8(ty).is_some()
    }) {
        Ok(legacy) => Ok(Some(legacy)),
        Err(_) => Ok(None),
    }
}

fn legacy_layout_detected_error(
    path: &Path,
    canonical_err: CCRealityError,
    legacy: super::witness_chain_legacy::LegacyVerification,
) -> CCRealityError {
    CCRealityError::new(
        "CCREALITY_WITNESS_LEGACY_LAYOUT_DETECTED",
        "witness-chain.bin is valid under the pre-canonical legacy layout, but not under the canonical context-graph-witness layout",
        "witness_chain.layout",
        "run optimizer_witness_chain_repair_legacy with the current witness-chain SHA-256; do not append new optimizer evidence until repair succeeds",
        json!({
            "legacy_entries": legacy.entries,
            "legacy_last_chain_hash": chain_hash_hex(&legacy.last_legacy_chain_hash),
            "translated_canonical_last_chain_hash": chain_hash_hex(&legacy.last_canonical_chain_hash),
            "legacy_last_op": legacy.last_op,
            "canonical_error": canonical_err.into_value(),
        }),
        Some(file_sot(path)),
    )
}

pub(super) fn invalid_chain_length(path: &Path, len: usize) -> CCRealityError {
    CCRealityError::new(
        "CCREALITY_WITNESS_LENGTH_INVALID",
        "witness-chain.bin length is not divisible by WITNESS_ENTRY_SIZE",
        "witness_chain.length",
        "the chain is truncated or has an incompatible entry format",
        json!({"path": path.display().to_string(), "len": len, "entry_size": WITNESS_ENTRY_SIZE}),
        Some(file_sot(path)),
    )
}

pub(super) fn parse_sha256_hex(value: &str) -> Result<[u8; HASH_SIZE]> {
    let raw = value.strip_prefix("sha256:").unwrap_or(value);
    let bytes = hex::decode(raw).map_err(|err| {
        CCRealityError::new(
            "CCREALITY_WITNESS_CONTENT_HASH_INVALID_HEX",
            format!("content_sha256 is not valid hex: {err}"),
            "content_sha256",
            "pass a sha256:<64 lowercase hex chars> content hash",
            json!({"content_sha256": value}),
            None,
        )
    })?;
    if bytes.len() != HASH_SIZE {
        return Err(CCRealityError::new(
            "CCREALITY_WITNESS_CONTENT_HASH_INVALID_LENGTH",
            "content_sha256 must decode to 32 bytes",
            "content_sha256",
            "pass a sha256:<64 lowercase hex chars> content hash",
            json!({"content_sha256": value, "decoded_len": bytes.len()}),
            None,
        ));
    }
    let mut out = [0u8; HASH_SIZE];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn sha256_hex(hash: &[u8; HASH_SIZE]) -> String {
    format!("sha256:{}", hex::encode(hash))
}

pub(super) fn chain_hash_hex(hash: &[u8; HASH_SIZE]) -> String {
    format!("shake256:{}", witness_hex_hash(hash))
}

fn entry_json(offset: u64, entry: &WitnessEntry, chain_hash: &[u8; HASH_SIZE]) -> Value {
    json!({
        "offset": offset,
        "timestamp_unix": entry.timestamp_ns / 1_000_000_000,
        "timestamp_ns": entry.timestamp_ns,
        "op_type": WitnessOpType::from_u8(entry.witness_type).map(WitnessOpType::as_str).unwrap_or("unknown"),
        "content_sha256": sha256_hex(&entry.action_hash),
        "action_hash": chain_hash_hex(&entry.action_hash),
        "prev_chain_hash": chain_hash_hex(&entry.prev_hash),
        "chain_hash": chain_hash_hex(chain_hash),
    })
}

pub(super) fn witness_error(
    path: &Path,
    err: context_graph_witness::WitnessError,
) -> CCRealityError {
    match err {
        context_graph_witness::WitnessError::EntryLengthInvalid { expected, actual } => {
            CCRealityError::new(
                "CCREALITY_WITNESS_ENTRY_SIZE_INVALID",
                "witness entry slice has invalid byte length",
                "witness_chain.entry_size",
                "inspect witness-chain.bin for truncation",
                json!({"expected": expected, "actual": actual}),
                Some(file_sot(path)),
            )
        }
        context_graph_witness::WitnessError::ChainLengthInvalid { len, entry_size: _ } => {
            invalid_chain_length(path, len)
        }
        context_graph_witness::WitnessError::PrevHashMismatch {
            offset,
            expected_prev_hash,
            actual_prev_hash,
        } => CCRealityError::new(
            "CCREALITY_WITNESS_PREV_HASH_MISMATCH",
            "witness-chain.bin prev_chain_hash does not match replayed chain hash",
            "witness_chain.prev_chain_hash",
            "inspect optimizer writes around the failing offset; the chain may be tampered or truncated",
            json!({
                "path": path.display().to_string(),
                "offset": offset,
                "expected_prev_chain_hash": chain_hash_hex(&expected_prev_hash),
                "actual_prev_chain_hash": chain_hash_hex(&actual_prev_hash),
            }),
            Some(file_sot(path)),
        ),
        context_graph_witness::WitnessError::WitnessTypeRejected {
            offset,
            witness_type,
        } => CCRealityError::new(
            "CCREALITY_WITNESS_OP_TYPE_UNKNOWN",
            "witness-chain.bin contains an unknown op_type",
            "witness_chain.op_type",
            "upgrade the verifier or inspect the corrupt entry",
            json!({"path": path.display().to_string(), "offset": offset, "op_type": witness_type}),
            Some(file_sot(path)),
        ),
    }
}

fn witness_chain_path(runtime_root: &Path, run_id: &str) -> PathBuf {
    runtime_root
        .join(run_id)
        .join("claude-code-optimizer")
        .join("witness-chain.bin")
}

async fn active_witness_chain_path(args: &Value) -> Result<PathBuf> {
    let runtime_root = require_active_runtime_root().await?;
    let run_id = match optional_str_strict(args, "run_id")? {
        Some(run_id) => run_id,
        None => latest_run_id(&runtime_root)?.ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_WITNESS_NO_ACTIVE_RUN",
                "no active run under the runtime root",
                "active_run",
                "run reality-loop smoke or attempt before verifying the witness chain",
                json!({"runtime_root": runtime_root.display().to_string()}),
                Some(file_sot(&runtime_root)),
            )
        })?,
    };
    Ok(witness_chain_path(&runtime_root, &run_id))
}
