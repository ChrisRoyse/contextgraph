//! Explicit repair path for pre-canonical optimizer witness chains.

use super::errors::{CCRealityError, Result};
use super::helpers::*;
use super::witness_chain::{chain_hash_hex, invalid_chain_length, parse_sha256_hex, WitnessOpType};
use super::witness_chain_format::ensure_canonical_format_manifest;
use super::witness_chain_io::{
    read_chain_bytes, replace_bytes_checked, with_chain_lock, write_bytes_checked,
};
use super::witness_chain_legacy::{
    encode_canonical_entries, prefixed_hash, verify_legacy_chain_bytes_with_type_validator,
    LEGACY_LAYOUT_ID,
};
use context_graph_witness::{
    verify_chain_bytes_with_type_validator, WitnessEntry, HASH_SIZE, WITNESS_ENTRY_SIZE,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

pub async fn optimizer_witness_chain_repair_legacy(args: Value) -> Result<Value> {
    let args = coerce_stringified_args(args);
    let runtime_root = require_active_runtime_root().await?;
    let run_id = match optional_str_strict(&args, "run_id")? {
        Some(run_id) => run_id,
        None => latest_run_id(&runtime_root)?.ok_or_else(|| {
            CCRealityError::new(
                "CCREALITY_WITNESS_NO_ACTIVE_RUN",
                "no active run under the runtime root",
                "active_run",
                "run reality-loop smoke or attempt before repairing the witness chain",
                json!({"runtime_root": runtime_root.display().to_string()}),
                Some(file_sot(&runtime_root)),
            )
        })?,
    };
    let expected_legacy_sha256 = required_str(&args, "expected_legacy_sha256")?;
    repair_legacy_chain_for_run(&runtime_root, &run_id, &expected_legacy_sha256)
}

pub(super) fn repair_legacy_chain_for_run(
    runtime_root: &Path,
    run_id: &str,
    expected_legacy_sha256: &str,
) -> Result<Value> {
    let path = runtime_root
        .join(run_id)
        .join("claude-code-optimizer")
        .join("witness-chain.bin");
    with_chain_lock(&path, || {
        repair_legacy_chain_locked(runtime_root, run_id, &path, expected_legacy_sha256)
    })
}

fn repair_legacy_chain_locked(
    runtime_root: &Path,
    run_id: &str,
    path: &Path,
    expected_legacy_sha256: &str,
) -> Result<Value> {
    if !path.is_file() {
        return Err(CCRealityError::new(
            "CCREALITY_WITNESS_REPAIR_SOURCE_MISSING",
            "witness-chain.bin does not exist",
            "witness_chain.repair.source",
            "only run legacy repair against an existing legacy witness-chain.bin",
            json!({"path": path.display().to_string()}),
            Some(file_sot(path)),
        ));
    }
    let legacy_sha256 = sha256_file(path)?;
    if legacy_sha256 != expected_legacy_sha256 {
        return Err(CCRealityError::new(
            "CCREALITY_WITNESS_REPAIR_SOURCE_SHA_MISMATCH",
            "witness-chain.bin changed after the operator inspected it",
            "arguments.expected_legacy_sha256",
            "re-read the source-of-truth chain and pass its current SHA-256",
            json!({
                "expected_legacy_sha256": expected_legacy_sha256,
                "actual_legacy_sha256": legacy_sha256,
                "path": path.display().to_string()
            }),
            Some(file_sot(path)),
        ));
    }
    let legacy_bytes = read_chain_bytes(path)?;
    if legacy_bytes.is_empty() {
        return Err(CCRealityError::new(
            "CCREALITY_WITNESS_REPAIR_EMPTY_CHAIN",
            "witness-chain.bin is empty and does not need legacy repair",
            "witness_chain.repair.source",
            "leave the empty canonical chain as-is; the next append will create the manifest",
            json!({"path": path.display().to_string()}),
            Some(file_sot(path)),
        ));
    }
    if legacy_bytes.len() % WITNESS_ENTRY_SIZE != 0 {
        return Err(invalid_chain_length(path, legacy_bytes.len()));
    }
    if verify_chain_bytes_with_type_validator(&legacy_bytes, |ty| {
        WitnessOpType::from_u8(ty).is_some()
    })
    .is_ok()
    {
        return Err(CCRealityError::new(
            "CCREALITY_WITNESS_REPAIR_NOT_NEEDED",
            "witness-chain.bin already verifies as the canonical layout",
            "witness_chain.repair.source",
            "do not rewrite a canonical chain",
            json!({"path": path.display().to_string(), "sha256": legacy_sha256}),
            Some(file_sot(path)),
        ));
    }

    let now = unix_secs()?;
    let legacy =
        verify_legacy_chain_bytes_with_type_validator(&legacy_bytes, Some(path), now, |ty| {
            WitnessOpType::from_u8(ty).is_some()
        })
        .map_err(|legacy_err| {
            CCRealityError::new(
                "CCREALITY_WITNESS_REPAIR_LEGACY_REPLAY_FAILED",
                "witness-chain.bin is neither canonical nor a valid legacy chain",
                "witness_chain.repair.legacy_replay",
                "preserve the file and inspect the legacy replay error before attempting repair",
                json!({"legacy_error": legacy_err.into_value()}),
                Some(file_sot(path)),
            )
        })?;

    let repair_dir = runtime_root
        .join(run_id)
        .join("reality-optimizer")
        .join("witness-repair");
    let seq = next_repair_sequence(&repair_dir)?;
    let short_sha = legacy_sha256
        .strip_prefix("sha256:")
        .unwrap_or(&legacy_sha256)
        .chars()
        .take(16)
        .collect::<String>();
    let backup_path = repair_dir.join(format!("witness-chain-legacy-{seq:04}-{short_sha}.bin"));
    write_bytes_checked(&backup_path, &legacy_bytes)?;

    let canonical_pre_repair = encode_canonical_entries(&legacy.canonical_entries);
    let canonical_pre_repair_sha256 = sha256_bytes(&canonical_pre_repair);
    let claim_path = repair_dir.join(format!("repair-{seq:04}-claim.json"));
    let claim = json!({
        "schema_version": 1,
        "record_kind": "ccreality_witness_chain_legacy_repair_claim",
        "created_at_unix": now,
        "run_id": run_id,
        "reason": "unversioned witness-chain layout migration",
        "source_layout": LEGACY_LAYOUT_ID,
        "target_layout": "context-graph-witness-v1:prev_hash|action_hash|timestamp_ns|witness_type",
        "legacy_chain": {
            "source_of_truth": file_sot(path),
            "backup_source_of_truth": file_sot(&backup_path),
            "sha256": legacy_sha256,
            "bytes": legacy_bytes.len(),
            "entries": legacy.entries,
            "last_legacy_chain_hash": prefixed_hash(&legacy.last_legacy_chain_hash),
            "last_op": legacy.last_op,
        },
        "translated_canonical_chain_before_repair_entry": {
            "entries": legacy.entries,
            "bytes": canonical_pre_repair.len(),
            "sha256": canonical_pre_repair_sha256,
            "last_chain_hash": prefixed_hash(&legacy.last_canonical_chain_hash),
        },
        "policy": {
            "canonical_verifier_did_not_accept_legacy_bytes": true,
            "old_bytes_preserved_before_replace": true,
            "repair_is_explicit_not_silent_fallback": true,
        }
    });
    write_json_checked(&claim_path, &claim)?;
    let claim_sha256 = sha256_file(&claim_path)?;
    let claim_hash = parse_sha256_hex(&claim_sha256)?;
    let mut canonical_entries = legacy.canonical_entries;
    let repair_entry = repair_entry(&canonical_entries, claim_hash, now)?;
    canonical_entries.push(repair_entry);
    let canonical_bytes = encode_canonical_entries(&canonical_entries);
    let tmp_name = format!("witness-chain.repair-{seq:04}.{}.tmp", std::process::id());
    replace_bytes_checked(path, &canonical_bytes, &tmp_name)?;
    let verification = verify_chain_bytes_with_type_validator(&canonical_bytes, |ty| {
        WitnessOpType::from_u8(ty).is_some()
    })
    .map_err(|err| super::witness_chain::witness_error(path, err))?;
    let manifest = ensure_canonical_format_manifest(path)?;
    let final_sha256 = sha256_file(path)?;

    Ok(json!({
        "status": "ok",
        "repair_sequence": seq,
        "run_id": run_id,
        "source_of_truth": {
            "witness_chain": file_sot(path),
            "legacy_backup": file_sot(&backup_path),
            "repair_claim": file_sot(&claim_path),
            "format_manifest": manifest,
        },
        "legacy": {
            "sha256": legacy_sha256,
            "entries": legacy.entries,
            "last_chain_hash": prefixed_hash(&legacy.last_legacy_chain_hash),
        },
        "canonical": {
            "sha256": final_sha256,
            "entries": verification.entries,
            "last_chain_hash": chain_hash_hex(&verification.last_chain_hash),
            "repair_claim_sha256": claim_sha256,
            "repair_op_type": WitnessOpType::WitnessRepair.as_str(),
        }
    }))
}

fn repair_entry(
    canonical_entries: &[WitnessEntry],
    claim_hash: [u8; HASH_SIZE],
    now_unix: u64,
) -> Result<WitnessEntry> {
    let prev_hash = canonical_entries
        .last()
        .map(WitnessEntry::chain_hash)
        .unwrap_or([0u8; HASH_SIZE]);
    let timestamp_ns = now_unix.checked_mul(1_000_000_000).ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_WITNESS_REPAIR_TIMESTAMP_OVERFLOW",
            "repair timestamp overflowed nanoseconds conversion",
            "witness_chain.repair.timestamp_ns",
            "inspect system clock before repairing witness-chain state",
            json!({"timestamp_unix": now_unix}),
            None,
        )
    })?;
    Ok(WitnessEntry::new(
        prev_hash,
        claim_hash,
        timestamp_ns,
        WitnessOpType::WitnessRepair as u8,
    ))
}

fn next_repair_sequence(dir: &Path) -> Result<u64> {
    if !dir.exists() {
        std::fs::create_dir_all(dir)
            .map_err(|err| fs_error("CCREALITY_WITNESS_REPAIR_DIR_CREATE_FAILED", dir, err))?;
        return Ok(1);
    }
    let mut max_seq = 0u64;
    for entry in std::fs::read_dir(dir)
        .map_err(|err| fs_error("CCREALITY_WITNESS_REPAIR_DIR_READ_FAILED", dir, err))?
    {
        let entry =
            entry.map_err(|err| fs_error("CCREALITY_WITNESS_REPAIR_DIR_ENTRY_FAILED", dir, err))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(seq) = name
            .strip_prefix("repair-")
            .and_then(|suffix| suffix.strip_suffix("-claim.json"))
            .and_then(|digits| digits.parse::<u64>().ok())
        {
            max_seq = max_seq.max(seq);
        }
    }
    Ok(max_seq + 1)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}
