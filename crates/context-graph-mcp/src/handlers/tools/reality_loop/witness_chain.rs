//! SHAKE-256 witness chain for ccreality optimizer writes.
//!
//! Source of truth: `<runtime_root>/<run_id>/claude-code-optimizer/witness-chain.bin`.

use super::errors::{CCRealityError, Result};
use super::helpers::*;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use serde_json::{json, Value};
use sha3::{
    digest::{ExtendableOutput, Update, XofReader},
    Shake256,
};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const WITNESS_ENTRY_SIZE: usize = 73;
const HASH_SIZE: usize = 32;
const ZERO_HASH: [u8; HASH_SIZE] = [0u8; HASH_SIZE];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WitnessOpType {
    Decision = 0,
    Recommendation = 1,
    HarnessTransition = 2,
    BanditSelect = 3,
    BanditReward = 4,
    Autoresearch = 5,
}

impl WitnessOpType {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Decision),
            1 => Some(Self::Recommendation),
            2 => Some(Self::HarnessTransition),
            3 => Some(Self::BanditSelect),
            4 => Some(Self::BanditReward),
            5 => Some(Self::Autoresearch),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Decision => "decision",
            Self::Recommendation => "recommendation",
            Self::HarnessTransition => "harness_transition",
            Self::BanditSelect => "bandit_select",
            Self::BanditReward => "bandit_reward",
            Self::Autoresearch => "autoresearch",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessEntry {
    pub timestamp_unix: u64,
    pub op_type: u8,
    pub content_sha256: [u8; HASH_SIZE],
    pub prev_chain_hash: [u8; HASH_SIZE],
}

impl WitnessEntry {
    fn to_bytes(&self) -> [u8; WITNESS_ENTRY_SIZE] {
        let mut out = [0u8; WITNESS_ENTRY_SIZE];
        out[0..8].copy_from_slice(&self.timestamp_unix.to_be_bytes());
        out[8] = self.op_type;
        out[9..41].copy_from_slice(&self.content_sha256);
        out[41..73].copy_from_slice(&self.prev_chain_hash);
        out
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != WITNESS_ENTRY_SIZE {
            return Err(CCRealityError::new(
                "CCREALITY_WITNESS_ENTRY_SIZE_INVALID",
                "witness entry slice has invalid byte length",
                "witness_chain.entry_size",
                "inspect witness-chain.bin for truncation",
                json!({"expected": WITNESS_ENTRY_SIZE, "actual": bytes.len()}),
                None,
            ));
        }
        let mut timestamp = [0u8; 8];
        timestamp.copy_from_slice(&bytes[0..8]);
        let mut content_sha256 = [0u8; HASH_SIZE];
        content_sha256.copy_from_slice(&bytes[9..41]);
        let mut prev_chain_hash = [0u8; HASH_SIZE];
        prev_chain_hash.copy_from_slice(&bytes[41..73]);
        Ok(Self {
            timestamp_unix: u64::from_be_bytes(timestamp),
            op_type: bytes[8],
            content_sha256,
            prev_chain_hash,
        })
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

pub async fn optimizer_witness_chain_verify(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let path = active_witness_chain_path(&args).await?;
    let verification = verify_chain(&path)?;
    Ok(json!({
        "status": "ok",
        "valid": true,
        "entry_size": WITNESS_ENTRY_SIZE,
        "entries": verification.entries,
        "last_chain_hash": hex_hash(&verification.last_chain_hash),
        "last_op": verification.last_op,
        "source_of_truth": file_sot(&path),
        "sha256": if path.is_file() { sha256_file(&path)? } else { "absent".to_string() },
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
        let entry = WitnessEntry::from_bytes(&bytes[start..start + WITNESS_ENTRY_SIZE])?;
        entries.push(entry_json(idx as u64, &entry, &hash_entry(&entry)));
    }
    Ok(json!({
        "status": "ok",
        "since_offset": since_offset,
        "entries": entries,
        "total_entries": verification.entries,
        "last_chain_hash": hex_hash(&verification.last_chain_hash),
        "source_of_truth": file_sot(&path),
    }))
}

fn append_entry(
    path: &Path,
    op_type: WitnessOpType,
    content_sha256: [u8; HASH_SIZE],
) -> Result<Value> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| fs_error("CCREALITY_WITNESS_DIR_CREATE_FAILED", parent, e))?;
    }
    let before = verify_chain(path)?;
    let offset = before.entries;
    let entry = WitnessEntry {
        timestamp_unix: unix_secs()?,
        op_type: op_type as u8,
        content_sha256,
        prev_chain_hash: before.last_chain_hash,
    };
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
    let after = verify_chain(path)?;
    if after.entries != offset + 1 || after.last_chain_hash != hash_entry(&entry) {
        return Err(CCRealityError::new(
            "CCREALITY_WITNESS_READBACK_MISMATCH",
            "witness-chain.bin readback did not preserve appended entry",
            "witness_chain.readback",
            "inspect filesystem durability before trusting optimizer audit state",
            json!({
                "path": path.display().to_string(),
                "expected_entries": offset + 1,
                "actual_entries": after.entries,
                "expected_last_chain_hash": hex_hash(&hash_entry(&entry)),
                "actual_last_chain_hash": hex_hash(&after.last_chain_hash),
            }),
            Some(file_sot(path)),
        ));
    }
    Ok(json!({
        "status": "ok",
        "offset": offset,
        "entry_size": WITNESS_ENTRY_SIZE,
        "op_type": op_type.as_str(),
        "content_sha256": hex_hash(&content_sha256),
        "prev_chain_hash": hex_hash(&entry.prev_chain_hash),
        "chain_hash": hex_hash(&after.last_chain_hash),
        "entries": after.entries,
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
    if !path.exists() {
        return Ok(Verification {
            entries: 0,
            last_chain_hash: ZERO_HASH,
            last_op: Value::Null,
        });
    }
    let bytes = read_chain_bytes(path)?;
    if bytes.len() % WITNESS_ENTRY_SIZE != 0 {
        return Err(invalid_chain_length(path, bytes.len()));
    }
    let mut expected_prev = ZERO_HASH;
    let mut last_op = Value::Null;
    let entries = bytes.len() / WITNESS_ENTRY_SIZE;
    for idx in 0..entries {
        let start = idx * WITNESS_ENTRY_SIZE;
        let entry = WitnessEntry::from_bytes(&bytes[start..start + WITNESS_ENTRY_SIZE])?;
        if entry.prev_chain_hash != expected_prev {
            return Err(CCRealityError::new(
                "CCREALITY_WITNESS_PREV_HASH_MISMATCH",
                "witness-chain.bin prev_chain_hash does not match replayed chain hash",
                "witness_chain.prev_chain_hash",
                "inspect optimizer writes around the failing offset; the chain may be tampered or truncated",
                json!({
                    "path": path.display().to_string(),
                    "offset": idx,
                    "expected_prev_chain_hash": hex_hash(&expected_prev),
                    "actual_prev_chain_hash": hex_hash(&entry.prev_chain_hash),
                }),
                Some(file_sot(path)),
            ));
        }
        if WitnessOpType::from_u8(entry.op_type).is_none() {
            return Err(CCRealityError::new(
                "CCREALITY_WITNESS_OP_TYPE_UNKNOWN",
                "witness-chain.bin contains an unknown op_type",
                "witness_chain.op_type",
                "upgrade the verifier or inspect the corrupt entry",
                json!({"path": path.display().to_string(), "offset": idx, "op_type": entry.op_type}),
                Some(file_sot(path)),
            ));
        }
        expected_prev = hash_entry(&entry);
        last_op = entry_json(idx as u64, &entry, &expected_prev);
    }
    Ok(Verification {
        entries: entries as u64,
        last_chain_hash: expected_prev,
        last_op,
    })
}

fn read_chain_bytes(path: &Path) -> Result<Vec<u8>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut file =
        fs::File::open(path).map_err(|e| fs_error("CCREALITY_WITNESS_READ_FAILED", path, e))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| fs_error("CCREALITY_WITNESS_READ_FAILED", path, e))?;
    Ok(bytes)
}

fn invalid_chain_length(path: &Path, len: usize) -> CCRealityError {
    CCRealityError::new(
        "CCREALITY_WITNESS_LENGTH_INVALID",
        "witness-chain.bin length is not divisible by WITNESS_ENTRY_SIZE",
        "witness_chain.length",
        "the chain is truncated or has an incompatible entry format",
        json!({"path": path.display().to_string(), "len": len, "entry_size": WITNESS_ENTRY_SIZE}),
        Some(file_sot(path)),
    )
}

fn hash_entry(entry: &WitnessEntry) -> [u8; HASH_SIZE] {
    let mut hasher = Shake256::default();
    hasher.update(&entry.to_bytes());
    let mut reader = hasher.finalize_xof();
    let mut out = [0u8; HASH_SIZE];
    XofReader::read(&mut reader, &mut out);
    out
}

fn parse_sha256_hex(value: &str) -> Result<[u8; HASH_SIZE]> {
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

fn hex_hash(hash: &[u8; HASH_SIZE]) -> String {
    format!("sha256:{}", hex::encode(hash))
}

fn entry_json(offset: u64, entry: &WitnessEntry, chain_hash: &[u8; HASH_SIZE]) -> Value {
    json!({
        "offset": offset,
        "timestamp_unix": entry.timestamp_unix,
        "op_type": WitnessOpType::from_u8(entry.op_type).map(WitnessOpType::as_str).unwrap_or("unknown"),
        "content_sha256": hex_hash(&entry.content_sha256),
        "prev_chain_hash": hex_hash(&entry.prev_chain_hash),
        "chain_hash": hex_hash(chain_hash),
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn content_hash(byte: u8) -> String {
        format!("sha256:{}", hex::encode([byte; HASH_SIZE]))
    }

    #[test]
    fn append_and_verify_chain_reads_physical_bytes() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("witness-chain.bin");
        append_entry(
            &path,
            WitnessOpType::Decision,
            parse_sha256_hex(&content_hash(1)).unwrap(),
        )
        .expect("append1");
        append_entry(
            &path,
            WitnessOpType::Recommendation,
            parse_sha256_hex(&content_hash(2)).unwrap(),
        )
        .expect("append2");
        let bytes = std::fs::read(&path).expect("physical chain");
        assert_eq!(bytes.len(), WITNESS_ENTRY_SIZE * 2);
        let verification = verify_chain(&path).expect("verify");
        assert_eq!(verification.entries, 2);
        assert_ne!(verification.last_chain_hash, ZERO_HASH);
    }

    #[test]
    fn truncated_chain_errors() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("witness-chain.bin");
        std::fs::write(&path, [1u8; 7]).expect("write corrupt");
        let err = verify_chain(&path).expect_err("must fail");
        assert_eq!(err.error_code, "CCREALITY_WITNESS_LENGTH_INVALID");
    }

    #[test]
    fn tampered_prev_hash_errors() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("witness-chain.bin");
        append_entry(
            &path,
            WitnessOpType::Decision,
            parse_sha256_hex(&content_hash(1)).unwrap(),
        )
        .expect("append1");
        append_entry(
            &path,
            WitnessOpType::Recommendation,
            parse_sha256_hex(&content_hash(2)).unwrap(),
        )
        .expect("append2");
        let mut bytes = std::fs::read(&path).expect("read");
        bytes[WITNESS_ENTRY_SIZE + 41] ^= 0x7f;
        std::fs::write(&path, bytes).expect("tamper");
        let err = verify_chain(&path).expect_err("must fail");
        assert_eq!(err.error_code, "CCREALITY_WITNESS_PREV_HASH_MISMATCH");
    }
}
