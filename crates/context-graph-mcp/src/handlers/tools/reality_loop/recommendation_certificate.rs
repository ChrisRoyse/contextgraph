//! Merkle certificates for recommendation recall source evidence.

use super::errors::{CCRealityError, Result};
use super::helpers::{file_sot, fs_error};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn recommendation_certificate(path: &Path, body: &Value) -> Result<Value> {
    let mut leaves = Vec::new();
    leaves.push(source_leaf(
        "recommendation_file",
        &file_sot(path),
        &fs::read(path).map_err(|e| fs_error("CCREALITY_RECALL_CERT_READ_FAILED", path, e))?,
    ));
    if let Some(readbacks) = body
        .get("source_of_truth_readbacks")
        .and_then(Value::as_array)
    {
        for item in readbacks.iter().filter_map(Value::as_str) {
            if let Some(file) = item.strip_prefix("file:") {
                let source_path = PathBuf::from(file);
                let bytes = fs::read(&source_path).map_err(|e| {
                    fs_error("CCREALITY_RECALL_CERT_SOURCE_READ_FAILED", &source_path, e)
                })?;
                leaves.push(source_leaf("file", item, &bytes));
            } else if let Some(sqlite) = item.strip_prefix("sqlite:") {
                let db_path = sqlite.split('#').next().unwrap_or(sqlite);
                let source_path = PathBuf::from(db_path);
                let bytes = fs::read(&source_path).map_err(|e| {
                    fs_error("CCREALITY_RECALL_CERT_SQLITE_READ_FAILED", &source_path, e)
                })?;
                leaves.push(source_leaf("sqlite_file", item, &bytes));
            } else {
                leaves.push(source_leaf("opaque_reference", item, item.as_bytes()));
            }
        }
    }
    let leaf_hashes = leaves
        .iter()
        .map(|leaf| {
            let bytes = serde_json::to_vec(leaf).map_err(|err| {
                CCRealityError::new(
                    "CCREALITY_RECALL_CERT_LEAF_SERIALIZE_FAILED",
                    format!("failed to serialize certificate leaf: {err}"),
                    "certificate.leaf",
                    "inspect source references before issuing certificate",
                    json!({"leaf": leaf}),
                    Some(file_sot(path)),
                )
            })?;
            Ok(merkle_leaf_hash(&bytes))
        })
        .collect::<Result<Vec<_>>>()?;
    let merkle_root = merkle_root(&leaf_hashes);
    Ok(json!({
        "schema_version": 1,
        "record_kind": "ccreality_recommendation_recall_certificate",
        "hash_strategy": "rfc9162_domain_separated_sha256",
        "leaf_count": leaves.len(),
        "source_hashes": leaves,
        "merkle_root": format!("sha256:{}", hex::encode(merkle_root)),
    }))
}

fn source_leaf(kind: &str, source: &str, bytes: &[u8]) -> Value {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    json!({
        "kind": kind,
        "source": source,
        "content_sha256": format!("sha256:{:x}", hasher.finalize()),
    })
}

fn merkle_leaf_hash(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([0x00]);
    hasher.update(bytes);
    hasher.finalize().into()
}

fn merkle_node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update([0x01]);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return Sha256::digest([]).into();
    }
    if leaves.len() == 1 {
        return leaves[0];
    }
    let split = leaves.len().next_power_of_two() / 2;
    merkle_node_hash(
        &merkle_root(&leaves[..split]),
        &merkle_root(&leaves[split..]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merkle_root_changes_when_leaf_changes() {
        let a = merkle_root(&[merkle_leaf_hash(b"a"), merkle_leaf_hash(b"b")]);
        let b = merkle_root(&[merkle_leaf_hash(b"a"), merkle_leaf_hash(b"c")]);
        assert_ne!(a, b);
    }
}
