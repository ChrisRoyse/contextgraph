//! Witness-chain format manifest helpers.

use super::errors::{CCRealityError, Result};
use super::helpers::{file_sot, read_json, unix_secs, write_json_checked};
use context_graph_witness::WITNESS_ENTRY_SIZE;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const CANONICAL_LAYOUT_ID: &str =
    "context-graph-witness-v1:prev_hash|action_hash|timestamp_ns|witness_type";

pub fn format_manifest_path(chain_path: &Path) -> PathBuf {
    chain_path.with_file_name("witness-chain.format.json")
}

pub fn read_format_manifest_status(chain_path: &Path) -> Result<Value> {
    let path = format_manifest_path(chain_path);
    if !path.exists() {
        return Err(CCRealityError::new(
            "CCREALITY_WITNESS_FORMAT_MANIFEST_ABSENT",
            "witness-chain.format.json is absent; verification requires the canonical format manifest",
            "witness_chain.format_manifest",
            "append a witness entry to create the manifest, or repair/regenerate it before using the chain as verification evidence",
            json!({
                "expected_layout": CANONICAL_LAYOUT_ID,
                "path": path.display().to_string(),
            }),
            Some(file_sot(&path)),
        ));
    }
    let manifest = read_json(&path)?;
    validate_format_manifest(chain_path, &manifest)?;
    Ok(json!({
        "status": "valid",
        "source_of_truth": file_sot(&path),
        "manifest": manifest,
    }))
}

pub fn ensure_canonical_format_manifest(chain_path: &Path) -> Result<Value> {
    let path = format_manifest_path(chain_path);
    if path.exists() {
        let manifest = read_json(&path)?;
        validate_format_manifest(chain_path, &manifest)?;
        return Ok(json!({
            "status": "existing",
            "source_of_truth": file_sot(&path),
            "manifest": manifest,
        }));
    }
    let manifest = json!({
        "schema_version": 1,
        "record_kind": "ccreality_witness_chain_format_manifest",
        "created_at_unix": unix_secs()?,
        "chain_file": chain_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("witness-chain.bin"),
        "entry_size": WITNESS_ENTRY_SIZE,
        "layout_id": CANONICAL_LAYOUT_ID,
        "hash_algorithm": "SHAKE-256/256",
        "fields": [
            {"name": "prev_hash", "offset": 0, "len": 32},
            {"name": "action_hash", "offset": 32, "len": 32},
            {"name": "timestamp_ns_be", "offset": 64, "len": 8},
            {"name": "witness_type", "offset": 72, "len": 1}
        ]
    });
    write_json_checked(&path, &manifest)?;
    let readback = read_json(&path)?;
    validate_format_manifest(chain_path, &readback)?;
    Ok(json!({
        "status": "created",
        "source_of_truth": file_sot(&path),
        "manifest": readback,
    }))
}

fn validate_format_manifest(chain_path: &Path, manifest: &Value) -> Result<()> {
    let layout = manifest
        .get("layout_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let entry_size = manifest
        .get("entry_size")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    if layout != CANONICAL_LAYOUT_ID || entry_size != WITNESS_ENTRY_SIZE as u64 {
        return Err(CCRealityError::new(
            "CCREALITY_WITNESS_FORMAT_MANIFEST_MISMATCH",
            "witness-chain.format.json does not describe the canonical witness-chain layout",
            "witness_chain.format_manifest",
            "preserve the chain and repair or regenerate the format manifest before appending",
            json!({
                "expected_layout": CANONICAL_LAYOUT_ID,
                "actual_layout": layout,
                "expected_entry_size": WITNESS_ENTRY_SIZE,
                "actual_entry_size": entry_size,
            }),
            Some(file_sot(&format_manifest_path(chain_path))),
        ));
    }
    Ok(())
}
