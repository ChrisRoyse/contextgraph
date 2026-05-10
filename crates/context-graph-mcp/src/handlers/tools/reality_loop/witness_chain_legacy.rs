//! Legacy ccreality witness-chain decoder.
//!
//! This module exists only to diagnose and reconcile the MCP-local layout that
//! shipped before `context-graph-witness` became the canonical chain format.

use super::errors::{CCRealityError, Result};
use super::helpers::file_sot;
use context_graph_witness::{shake256_32, WitnessEntry, HASH_SIZE, WITNESS_ENTRY_SIZE, ZERO_HASH};
use serde_json::{json, Value};
use std::path::Path;

const LEGACY_MIN_UNIX_SECS: u64 = 1_577_836_800; // 2020-01-01T00:00:00Z
pub const LEGACY_LAYOUT_ID: &str =
    "ccreality-mcp-v0:timestamp_unix|op_type|content_sha256|prev_chain_hash";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyWitnessEntry {
    pub timestamp_unix: u64,
    pub op_type: u8,
    pub content_sha256: [u8; HASH_SIZE],
    pub prev_chain_hash: [u8; HASH_SIZE],
}

impl LegacyWitnessEntry {
    fn from_bytes(bytes: &[u8], path: Option<&Path>) -> Result<Self> {
        if bytes.len() != WITNESS_ENTRY_SIZE {
            return Err(CCRealityError::new(
                "CCREALITY_WITNESS_LEGACY_ENTRY_SIZE_INVALID",
                "legacy witness entry slice has invalid byte length",
                "witness_chain.legacy.entry_size",
                "inspect witness-chain.bin for truncation before repairing it",
                json!({"expected": WITNESS_ENTRY_SIZE, "actual": bytes.len()}),
                path.map(file_sot),
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

    fn to_bytes(&self) -> [u8; WITNESS_ENTRY_SIZE] {
        let mut out = [0u8; WITNESS_ENTRY_SIZE];
        out[0..8].copy_from_slice(&self.timestamp_unix.to_be_bytes());
        out[8] = self.op_type;
        out[9..41].copy_from_slice(&self.content_sha256);
        out[41..73].copy_from_slice(&self.prev_chain_hash);
        out
    }

    fn chain_hash(&self) -> [u8; HASH_SIZE] {
        shake256_32(&self.to_bytes())
    }
}

#[derive(Debug, Clone)]
pub struct LegacyVerification {
    pub entries: u64,
    pub last_legacy_chain_hash: [u8; HASH_SIZE],
    pub last_canonical_chain_hash: [u8; HASH_SIZE],
    pub last_op: Value,
    pub canonical_entries: Vec<WitnessEntry>,
}

pub fn verify_legacy_chain_bytes_with_type_validator<F>(
    bytes: &[u8],
    path: Option<&Path>,
    now_unix: u64,
    mut accepts_type: F,
) -> Result<LegacyVerification>
where
    F: FnMut(u8) -> bool,
{
    if !bytes.len().is_multiple_of(WITNESS_ENTRY_SIZE) {
        return Err(CCRealityError::new(
            "CCREALITY_WITNESS_LEGACY_LENGTH_INVALID",
            "legacy witness-chain.bin length is not divisible by WITNESS_ENTRY_SIZE",
            "witness_chain.legacy.length",
            "do not repair this file; inspect the truncated bytes first",
            json!({"len": bytes.len(), "entry_size": WITNESS_ENTRY_SIZE}),
            path.map(file_sot),
        ));
    }

    let max_unix_secs = now_unix.saturating_add(86_400);
    let mut expected_legacy_prev = ZERO_HASH;
    let mut canonical_prev = ZERO_HASH;
    let mut last_op = Value::Null;
    let entries = bytes.len() / WITNESS_ENTRY_SIZE;
    let mut canonical_entries = Vec::with_capacity(entries);

    for offset in 0..entries {
        let start = offset * WITNESS_ENTRY_SIZE;
        let entry =
            LegacyWitnessEntry::from_bytes(&bytes[start..start + WITNESS_ENTRY_SIZE], path)?;
        if !(LEGACY_MIN_UNIX_SECS..=max_unix_secs).contains(&entry.timestamp_unix) {
            return Err(CCRealityError::new(
                "CCREALITY_WITNESS_LEGACY_TIMESTAMP_OUT_OF_RANGE",
                "legacy witness entry timestamp is outside the accepted migration window",
                "witness_chain.legacy.timestamp_unix",
                "treat this as corrupt unless the timestamp range is explicitly audited",
                json!({
                    "offset": offset,
                    "timestamp_unix": entry.timestamp_unix,
                    "min_unix": LEGACY_MIN_UNIX_SECS,
                    "max_unix": max_unix_secs
                }),
                path.map(file_sot),
            ));
        }
        if entry.prev_chain_hash != expected_legacy_prev {
            return Err(CCRealityError::new(
                "CCREALITY_WITNESS_LEGACY_PREV_HASH_MISMATCH",
                "legacy witness-chain.bin prev_chain_hash does not match replayed legacy hash",
                "witness_chain.legacy.prev_chain_hash",
                "do not repair this file; preserve it and inspect the failing legacy offset",
                json!({
                    "offset": offset,
                    "expected_prev_chain_hash": prefixed_hash(&expected_legacy_prev),
                    "actual_prev_chain_hash": prefixed_hash(&entry.prev_chain_hash)
                }),
                path.map(file_sot),
            ));
        }
        if !accepts_type(entry.op_type) {
            return Err(CCRealityError::new(
                "CCREALITY_WITNESS_LEGACY_OP_TYPE_UNKNOWN",
                "legacy witness-chain.bin contains an unknown op_type",
                "witness_chain.legacy.op_type",
                "upgrade the repair code or inspect the corrupt legacy entry",
                json!({"offset": offset, "op_type": entry.op_type}),
                path.map(file_sot),
            ));
        }
        let timestamp_ns = entry
            .timestamp_unix
            .checked_mul(1_000_000_000)
            .ok_or_else(|| {
                CCRealityError::new(
                    "CCREALITY_WITNESS_LEGACY_TIMESTAMP_OVERFLOW",
                    "legacy timestamp overflowed nanoseconds conversion",
                    "witness_chain.legacy.timestamp_ns",
                    "do not repair until the source timestamp is understood",
                    json!({"offset": offset, "timestamp_unix": entry.timestamp_unix}),
                    path.map(file_sot),
                )
            })?;
        let canonical = WitnessEntry::new(
            canonical_prev,
            entry.content_sha256,
            timestamp_ns,
            entry.op_type,
        );
        canonical_prev = canonical.chain_hash();
        expected_legacy_prev = entry.chain_hash();
        last_op = legacy_entry_json(offset as u64, &entry, &expected_legacy_prev);
        canonical_entries.push(canonical);
    }

    Ok(LegacyVerification {
        entries: entries as u64,
        last_legacy_chain_hash: expected_legacy_prev,
        last_canonical_chain_hash: canonical_prev,
        last_op,
        canonical_entries,
    })
}

pub fn encode_canonical_entries(entries: &[WitnessEntry]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(entries.len() * WITNESS_ENTRY_SIZE);
    for entry in entries {
        bytes.extend_from_slice(&entry.to_bytes());
    }
    bytes
}

pub fn prefixed_hash(hash: &[u8; HASH_SIZE]) -> String {
    format!("shake256:{}", hex::encode(hash))
}

fn legacy_entry_json(
    offset: u64,
    entry: &LegacyWitnessEntry,
    chain_hash: &[u8; HASH_SIZE],
) -> Value {
    json!({
        "offset": offset,
        "layout": LEGACY_LAYOUT_ID,
        "timestamp_unix": entry.timestamp_unix,
        "op_type": entry.op_type,
        "content_sha256": format!("sha256:{}", hex::encode(entry.content_sha256)),
        "prev_chain_hash": prefixed_hash(&entry.prev_chain_hash),
        "legacy_chain_hash": prefixed_hash(chain_hash),
    })
}
