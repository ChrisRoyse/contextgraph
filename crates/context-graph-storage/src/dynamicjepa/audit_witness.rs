// Inspired by ruvnet/RuVector crates/rvf/rvf-crypto/src/witness.rs at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.

use std::collections::BTreeSet;
use std::sync::Mutex;

use context_graph_core::dynamicjepa::{DynamicJepaError, DynamicJepaResult};
use context_graph_witness::{
    hex_hash, shake256_32, WitnessEntry, WitnessError, HASH_SIZE, WITNESS_ENTRY_SIZE, ZERO_HASH,
};
use rocksdb::{IteratorMode, WriteBatch, DB};
use serde::Serialize;
use uuid::Uuid;

use crate::dynamicjepa::audit::{DjAuditRecord, DJ_AUDIT_RECORD_VERSION};
use crate::dynamicjepa::column_families::{CF_DJ_AUDIT_LOG, CF_DJ_AUDIT_WITNESS_CHAIN};
use crate::dynamicjepa::common::{cf, storage_error, write_batch};
use crate::dynamicjepa::encode::{decode_plain, encode_plain};
use crate::dynamicjepa::keys::audit_key;

const AUDIT_KEY_SIZE: usize = 24;
const WITNESS_SEQ_KEY_SIZE: usize = 8;
const AUDIT_WITNESS_VALUE_SIZE: usize = AUDIT_KEY_SIZE + WITNESS_ENTRY_SIZE;

static AUDIT_WITNESS_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DjAuditWitnessVerification {
    pub entries: u64,
    pub audit_rows: u64,
    pub witness_rows: u64,
    pub last_chain_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DjAuditWitnessEntryView {
    pub sequence: u64,
    pub key_hex: String,
    pub audit_id: Uuid,
    pub operation: String,
    pub timestamp_unix_nanos: u64,
    pub witness_type: u8,
    pub witness_type_name: &'static str,
    pub action_hash: String,
    pub prev_chain_hash: String,
    pub chain_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuditWitnessRow {
    audit_key: [u8; AUDIT_KEY_SIZE],
    entry: WitnessEntry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuditWitnessChainState {
    entries: u64,
    audit_rows: u64,
    witness_rows: u64,
    last_chain_hash: [u8; HASH_SIZE],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppendedAuditWitness {
    sequence: u64,
    audit_key: [u8; AUDIT_KEY_SIZE],
}

pub(crate) fn write_batch_with_audit_witnesses(
    db: &DB,
    mut batch: WriteBatch,
    audits: &[&DjAuditRecord],
    operation: &'static str,
) -> DynamicJepaResult<()> {
    let _guard =
        AUDIT_WITNESS_LOCK
            .lock()
            .map_err(|_| DynamicJepaError::StorageInvariantViolation {
                message: "DynamicJEPA audit witness mutex was poisoned".to_string(),
            })?;
    let state = verify_audit_witness_chain_state(db)?;
    let mut next_sequence = state.entries;
    let mut prev_chain_hash = state.last_chain_hash;
    let mut pending_audit_keys = BTreeSet::new();
    let mut appended = Vec::with_capacity(audits.len());
    for audit in audits {
        appended.push(put_audit_and_witness_in_batch(
            db,
            &mut batch,
            audit,
            &mut pending_audit_keys,
            &mut next_sequence,
            &mut prev_chain_hash,
        )?);
    }
    write_batch(db, batch, operation)?;
    verify_audit_witness_rows(db, audits, &appended)?;
    Ok(())
}

pub fn verify_audit_witness_chain(db: &DB) -> DynamicJepaResult<DjAuditWitnessVerification> {
    let state = verify_audit_witness_chain_state(db)?;
    Ok(DjAuditWitnessVerification {
        entries: state.entries,
        audit_rows: state.audit_rows,
        witness_rows: state.witness_rows,
        last_chain_hash: format!("shake256:{}", hex_hash(&state.last_chain_hash)),
    })
}

pub fn list_audit_witness_entries(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<DjAuditWitnessEntryView>> {
    let mut rows = Vec::new();
    let mut expected_prev = ZERO_HASH;
    let witness_iter = db.iterator_cf(cf(db, CF_DJ_AUDIT_WITNESS_CHAIN)?, IteratorMode::Start);
    for (idx, item) in witness_iter.enumerate() {
        let (sequence_key, witness_bytes) = item.map_err(|err| {
            storage_error(
                "list_audit_witness_entries.witness_iter",
                CF_DJ_AUDIT_WITNESS_CHAIN,
                err.to_string(),
                "inspect RocksDB LOG files and retry verification from a fresh DB open",
            )
        })?;
        let sequence = decode_sequence_key(&sequence_key)?;
        if sequence != idx as u64 {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "DynamicJEPA audit witness sequence gap: expected={} actual={}",
                    idx, sequence
                ),
            });
        }
        let row = decode_audit_witness_row(&witness_bytes)?;
        let audit_bytes = db
            .get_cf(cf(db, CF_DJ_AUDIT_LOG)?, row.audit_key)
            .map_err(|err| {
                storage_error(
                    "list_audit_witness_entries.audit_get",
                    CF_DJ_AUDIT_LOG,
                    err.to_string(),
                    "inspect the audit log CF for missing or corrupt rows",
                )
            })?
            .ok_or_else(|| DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "missing DynamicJEPA audit row for witness sequence={} audit_key={}",
                    sequence,
                    hex_bytes(&row.audit_key)
                ),
            })?;
        let audit: DjAuditRecord =
            decode_plain(&audit_bytes, DJ_AUDIT_RECORD_VERSION, "DjAuditRecord")?;
        verify_entry_matches_audit(
            &row.audit_key,
            &audit_bytes,
            &audit,
            &row.entry,
            expected_prev,
        )?;
        let chain_hash = row.entry.chain_hash();
        expected_prev = chain_hash;
        if idx < offset {
            continue;
        }
        if rows.len() >= limit {
            break;
        }
        rows.push(DjAuditWitnessEntryView {
            sequence,
            key_hex: hex_bytes(&row.audit_key),
            audit_id: audit.audit_id,
            operation: audit.operation,
            timestamp_unix_nanos: audit.timestamp_unix_nanos,
            witness_type: row.entry.witness_type,
            witness_type_name: audit_witness_type_name(row.entry.witness_type).ok_or_else(
                || DynamicJepaError::StorageInvariantViolation {
                    message: format!("unknown audit witness type {}", row.entry.witness_type),
                },
            )?,
            action_hash: format!("shake256:{}", hex_hash(&row.entry.action_hash)),
            prev_chain_hash: format!("shake256:{}", hex_hash(&row.entry.prev_hash)),
            chain_hash: format!("shake256:{}", hex_hash(&chain_hash)),
        });
    }
    Ok(rows)
}

pub fn decode_audit_witness_value(value: &[u8]) -> DynamicJepaResult<serde_json::Value> {
    let row = decode_audit_witness_row(value)?;
    Ok(serde_json::json!({
        "audit_key_hex": hex_bytes(&row.audit_key),
        "prev_chain_hash": format!("shake256:{}", hex_hash(&row.entry.prev_hash)),
        "action_hash": format!("shake256:{}", hex_hash(&row.entry.action_hash)),
        "timestamp_unix_nanos": row.entry.timestamp_ns,
        "witness_type": row.entry.witness_type,
        "witness_type_name": audit_witness_type_name(row.entry.witness_type).unwrap_or("unknown"),
        "chain_hash": format!("shake256:{}", hex_hash(&row.entry.chain_hash())),
    }))
}

fn verify_audit_witness_chain_state(db: &DB) -> DynamicJepaResult<AuditWitnessChainState> {
    let mut expected_prev = ZERO_HASH;
    let mut seen_audit_keys = BTreeSet::new();
    let witness_iter = db.iterator_cf(cf(db, CF_DJ_AUDIT_WITNESS_CHAIN)?, IteratorMode::Start);
    let mut witness_rows = 0u64;
    for item in witness_iter {
        let (sequence_key, witness_bytes) = item.map_err(|err| {
            storage_error(
                "verify_audit_witness_chain.witness_iter",
                CF_DJ_AUDIT_WITNESS_CHAIN,
                err.to_string(),
                "inspect RocksDB LOG files and retry verification from a fresh DB open",
            )
        })?;
        let sequence = decode_sequence_key(&sequence_key)?;
        if sequence != witness_rows {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "DynamicJEPA audit witness sequence gap: expected={} actual={}",
                    witness_rows, sequence
                ),
            });
        }
        let row = decode_audit_witness_row(&witness_bytes)?;
        if !seen_audit_keys.insert(row.audit_key) {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "duplicate DynamicJEPA audit witness row for audit key {}",
                    hex_bytes(&row.audit_key)
                ),
            });
        }
        let audit_bytes = db
            .get_cf(cf(db, CF_DJ_AUDIT_LOG)?, row.audit_key)
            .map_err(|err| {
                storage_error(
                    "verify_audit_witness_chain.audit_get",
                    CF_DJ_AUDIT_LOG,
                    err.to_string(),
                    "inspect the audit log CF for missing or corrupt rows",
                )
            })?
            .ok_or_else(|| DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "missing DynamicJEPA audit row for witness sequence={} audit_key={}",
                    sequence,
                    hex_bytes(&row.audit_key)
                ),
            })?;
        let audit: DjAuditRecord =
            decode_plain(&audit_bytes, DJ_AUDIT_RECORD_VERSION, "DjAuditRecord")?;
        verify_entry_matches_audit(
            &row.audit_key,
            &audit_bytes,
            &audit,
            &row.entry,
            expected_prev,
        )?;
        expected_prev = row.entry.chain_hash();
        witness_rows += 1;
    }

    let mut audit_rows = 0u64;
    let audit_iter = db.iterator_cf(cf(db, CF_DJ_AUDIT_LOG)?, IteratorMode::Start);
    for item in audit_iter {
        let (audit_key, _) = item.map_err(|err| {
            storage_error(
                "verify_audit_witness_chain.audit_iter",
                CF_DJ_AUDIT_LOG,
                err.to_string(),
                "inspect RocksDB LOG files and retry verification from a fresh DB open",
            )
        })?;
        let audit_key = decode_audit_key(&audit_key)?;
        if !seen_audit_keys.contains(&audit_key) {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "missing DynamicJEPA audit witness row for audit key {}",
                    hex_bytes(&audit_key)
                ),
            });
        }
        audit_rows += 1;
    }

    if witness_rows != audit_rows {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "DynamicJEPA audit witness row count mismatch: audit_rows={audit_rows} witness_rows={witness_rows}"
            ),
        });
    }

    Ok(AuditWitnessChainState {
        entries: witness_rows,
        audit_rows,
        witness_rows,
        last_chain_hash: expected_prev,
    })
}

fn put_audit_and_witness_in_batch(
    db: &DB,
    batch: &mut WriteBatch,
    audit: &DjAuditRecord,
    pending_audit_keys: &mut BTreeSet<[u8; AUDIT_KEY_SIZE]>,
    next_sequence: &mut u64,
    prev_chain_hash: &mut [u8; HASH_SIZE],
) -> DynamicJepaResult<AppendedAuditWitness> {
    audit.validate()?;
    let audit_key = audit_key(audit.timestamp_unix_nanos, audit.audit_id);
    if !pending_audit_keys.insert(audit_key) {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "duplicate audit key inside one DynamicJEPA write batch: key={} audit_id={}",
                hex_bytes(&audit_key),
                audit.audit_id
            ),
        });
    }
    if db
        .get_cf(cf(db, CF_DJ_AUDIT_LOG)?, audit_key)
        .map_err(|err| {
            storage_error(
                "put_audit_and_witness_in_batch.audit_collision_check",
                CF_DJ_AUDIT_LOG,
                err.to_string(),
                "inspect the audit key generator before retrying",
            )
        })?
        .is_some()
    {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "audit key collision before witness append: key={} audit_id={}",
                hex_bytes(&audit_key),
                audit.audit_id
            ),
        });
    }
    let witness_sequence = *next_sequence;
    let sequence_key = sequence_key(witness_sequence);
    if db
        .get_cf(cf(db, CF_DJ_AUDIT_WITNESS_CHAIN)?, sequence_key)
        .map_err(|err| {
            storage_error(
                "put_audit_and_witness_in_batch.sequence_collision_check",
                CF_DJ_AUDIT_WITNESS_CHAIN,
                err.to_string(),
                "run dynamicjepa verify-audit-witness and inspect the witness-chain CF",
            )
        })?
        .is_some()
    {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "audit witness sequence collision before append: sequence={witness_sequence}"
            ),
        });
    }
    let audit_bytes = encode_plain(audit, DJ_AUDIT_RECORD_VERSION, "DjAuditRecord")?;
    let witness_type = audit_witness_type(&audit.operation)?;
    let entry = WitnessEntry::new(
        *prev_chain_hash,
        shake256_32(&audit_bytes),
        audit.timestamp_unix_nanos,
        witness_type,
    );
    let chain_hash = entry.chain_hash();
    let row = AuditWitnessRow { audit_key, entry };
    batch.put_cf(cf(db, CF_DJ_AUDIT_LOG)?, audit_key, audit_bytes);
    batch.put_cf(
        cf(db, CF_DJ_AUDIT_WITNESS_CHAIN)?,
        sequence_key,
        encode_audit_witness_row(&row),
    );
    *next_sequence = next_sequence.checked_add(1).ok_or_else(|| {
        DynamicJepaError::StorageInvariantViolation {
            message: "DynamicJEPA audit witness sequence exceeded u64::MAX".to_string(),
        }
    })?;
    *prev_chain_hash = chain_hash;
    Ok(AppendedAuditWitness {
        sequence: witness_sequence,
        audit_key,
    })
}

fn verify_audit_witness_rows(
    db: &DB,
    audits: &[&DjAuditRecord],
    appended: &[AppendedAuditWitness],
) -> DynamicJepaResult<()> {
    if audits.len() != appended.len() {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "audit witness readback request mismatch: audits={} appended={}",
                audits.len(),
                appended.len()
            ),
        });
    }
    for (audit, appended) in audits.iter().zip(appended) {
        let audit_bytes = db
            .get_cf(cf(db, CF_DJ_AUDIT_LOG)?, appended.audit_key)
            .map_err(|err| {
                storage_error(
                    "verify_audit_witness_rows.audit_readback",
                    CF_DJ_AUDIT_LOG,
                    err.to_string(),
                    "inspect the audit log CF after the failed write",
                )
            })?
            .ok_or_else(|| DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "audit readback missing for sequence={} key={}",
                    appended.sequence,
                    hex_bytes(&appended.audit_key)
                ),
            })?;
        let witness_bytes = db
            .get_cf(
                cf(db, CF_DJ_AUDIT_WITNESS_CHAIN)?,
                sequence_key(appended.sequence),
            )
            .map_err(|err| {
                storage_error(
                    "verify_audit_witness_rows.witness_readback",
                    CF_DJ_AUDIT_WITNESS_CHAIN,
                    err.to_string(),
                    "inspect the audit witness CF after the failed write",
                )
            })?
            .ok_or_else(|| DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "audit witness readback missing for sequence={}",
                    appended.sequence
                ),
            })?;
        let stored: DjAuditRecord =
            decode_plain(&audit_bytes, DJ_AUDIT_RECORD_VERSION, "DjAuditRecord")?;
        if stored != **audit {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!("audit readback mismatch for {}", audit.audit_id),
            });
        }
        let row = decode_audit_witness_row(&witness_bytes)?;
        if row.audit_key != appended.audit_key {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "audit witness readback key mismatch for sequence={} expected={} actual={}",
                    appended.sequence,
                    hex_bytes(&appended.audit_key),
                    hex_bytes(&row.audit_key)
                ),
            });
        }
        if row.entry.action_hash != shake256_32(&audit_bytes)
            || row.entry.timestamp_ns != audit.timestamp_unix_nanos
            || row.entry.witness_type != audit_witness_type(&audit.operation)?
        {
            return Err(DynamicJepaError::StorageInvariantViolation {
                message: format!(
                    "audit witness payload mismatch for audit_id={} sequence={}",
                    audit.audit_id, appended.sequence
                ),
            });
        }
    }
    verify_audit_witness_chain(db)?;
    Ok(())
}

fn verify_entry_matches_audit(
    key: &[u8],
    audit_bytes: &[u8],
    audit: &DjAuditRecord,
    entry: &WitnessEntry,
    expected_prev: [u8; HASH_SIZE],
) -> DynamicJepaResult<()> {
    if entry.prev_hash != expected_prev {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "audit witness prev hash mismatch key={} expected=shake256:{} actual=shake256:{}",
                hex_bytes(key),
                hex_hash(&expected_prev),
                hex_hash(&entry.prev_hash)
            ),
        });
    }
    let expected_action = shake256_32(audit_bytes);
    if entry.action_hash != expected_action {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "audit witness action hash mismatch key={} expected=shake256:{} actual=shake256:{}",
                hex_bytes(key),
                hex_hash(&expected_action),
                hex_hash(&entry.action_hash)
            ),
        });
    }
    if entry.timestamp_ns != audit.timestamp_unix_nanos {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "audit witness timestamp mismatch key={} expected={} actual={}",
                hex_bytes(key),
                audit.timestamp_unix_nanos,
                entry.timestamp_ns
            ),
        });
    }
    let expected_type = audit_witness_type(&audit.operation)?;
    if entry.witness_type != expected_type {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "audit witness type mismatch key={} operation={} expected={} actual={}",
                hex_bytes(key),
                audit.operation,
                expected_type,
                entry.witness_type
            ),
        });
    }
    Ok(())
}

fn encode_audit_witness_row(row: &AuditWitnessRow) -> [u8; AUDIT_WITNESS_VALUE_SIZE] {
    let mut bytes = [0u8; AUDIT_WITNESS_VALUE_SIZE];
    bytes[..AUDIT_KEY_SIZE].copy_from_slice(&row.audit_key);
    bytes[AUDIT_KEY_SIZE..].copy_from_slice(&row.entry.to_bytes());
    bytes
}

fn decode_audit_witness_row(value: &[u8]) -> DynamicJepaResult<AuditWitnessRow> {
    if value.len() != AUDIT_WITNESS_VALUE_SIZE {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "{CF_DJ_AUDIT_WITNESS_CHAIN} value must be {AUDIT_WITNESS_VALUE_SIZE} bytes, got {}",
                value.len()
            ),
        });
    }
    let audit_key = decode_audit_key(&value[..AUDIT_KEY_SIZE])?;
    let entry = WitnessEntry::from_bytes(&value[AUDIT_KEY_SIZE..]).map_err(witness_error)?;
    Ok(AuditWitnessRow { audit_key, entry })
}

fn sequence_key(sequence: u64) -> [u8; WITNESS_SEQ_KEY_SIZE] {
    sequence.to_be_bytes()
}

fn decode_sequence_key(key: &[u8]) -> DynamicJepaResult<u64> {
    if key.len() != WITNESS_SEQ_KEY_SIZE {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "{CF_DJ_AUDIT_WITNESS_CHAIN} key must be {WITNESS_SEQ_KEY_SIZE} bytes, got {}",
                key.len()
            ),
        });
    }
    let mut bytes = [0u8; WITNESS_SEQ_KEY_SIZE];
    bytes.copy_from_slice(key);
    Ok(u64::from_be_bytes(bytes))
}

fn decode_audit_key(key: &[u8]) -> DynamicJepaResult<[u8; AUDIT_KEY_SIZE]> {
    if key.len() != AUDIT_KEY_SIZE {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "{CF_DJ_AUDIT_LOG} key must be {AUDIT_KEY_SIZE} bytes, got {}",
                key.len()
            ),
        });
    }
    let mut bytes = [0u8; AUDIT_KEY_SIZE];
    bytes.copy_from_slice(key);
    Ok(bytes)
}

fn witness_error(err: WitnessError) -> DynamicJepaError {
    DynamicJepaError::StorageInvariantViolation {
        message: format!("DynamicJEPA audit witness chain invalid: {err}"),
    }
}

pub fn audit_witness_type(operation: &str) -> DynamicJepaResult<u8> {
    match operation {
        "ingest_event" => Ok(0x01),
        "run_adapter_success" => Ok(0x02),
        "run_adapter_failure" => Ok(0x03),
        "materialize_panel" => Ok(0x04),
        "materialize_pairwise" => Ok(0x05),
        "compile_transition" => Ok(0x06),
        "compile_trajectories" => Ok(0x07),
        "compile_dataset" => Ok(0x08),
        "train_predictor" => Ok(0x09),
        "register_artifact" => Ok(0x0A),
        "predict" => Ok(0x0B),
        "plan" => Ok(0x0C),
        "record_surprise" => Ok(0x0D),
        "verify_artifact_files" => Ok(0x0E),
        "verify_counter_world" => Ok(0x0F),
        "verify_gridworld" => Ok(0x10),
        "verify_career_taxonomy" => Ok(0x11),
        "research_smoke" => Ok(0x12),
        "build_constellation" => Ok(0x13),
        "calibrate_threshold" => Ok(0x14),
        "recalibrate_threshold" => Ok(0x15),
        "audit_pairwise_mi" => Ok(0x16),
        "compute_mc_ratio" => Ok(0x17),
        "inspect_counts" => Ok(0x18),
        "inspect_cf" => Ok(0x19),
        "register_domain_pack" => Ok(0x1A),
        "bind" => Ok(0x1B),
        "verification_run" => Ok(0x1C),
        other => Err(DynamicJepaError::SignalYieldUnknownOperation {
            operation: other.to_string(),
        }),
    }
}

pub fn audit_witness_type_name(witness_type: u8) -> Option<&'static str> {
    Some(match witness_type {
        0x01 => "ingest_event",
        0x02 => "run_adapter_success",
        0x03 => "run_adapter_failure",
        0x04 => "materialize_panel",
        0x05 => "materialize_pairwise",
        0x06 => "compile_transition",
        0x07 => "compile_trajectories",
        0x08 => "compile_dataset",
        0x09 => "train_predictor",
        0x0A => "register_artifact",
        0x0B => "predict",
        0x0C => "plan",
        0x0D => "record_surprise",
        0x0E => "verify_artifact_files",
        0x0F => "verify_counter_world",
        0x10 => "verify_gridworld",
        0x11 => "verify_career_taxonomy",
        0x12 => "research_smoke",
        0x13 => "build_constellation",
        0x14 => "calibrate_threshold",
        0x15 => "recalibrate_threshold",
        0x16 => "audit_pairwise_mi",
        0x17 => "compute_mc_ratio",
        0x18 => "inspect_counts",
        0x19 => "inspect_cf",
        0x1A => "register_domain_pack",
        0x1B => "bind",
        0x1C => "verification_run",
        _ => return None,
    })
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
