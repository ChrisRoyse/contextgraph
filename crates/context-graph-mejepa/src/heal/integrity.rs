use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use context_graph_witness::{
    shake256_32, verify_chain_bytes, WitnessEntry, WitnessError, WITNESS_ENTRY_SIZE, ZERO_HASH,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::heal::cf::{
    decode_value, encode_active_pointer_key, encode_value, CF_MEJEPA_ACTIVE_POINTERS,
};
use crate::heal::errors::{HealError, IntegrityViolationKind};
use crate::heal::pipeline::{HealStatus, StatusChange};
use crate::heal::promote::TriggerReason;
use crate::heal::promote_approval::{
    mark_promotion_executed, queue_pending_retrain_request, PendingPromotionKind,
};
use crate::heal::store::HealRocksStore;

pub const WITNESS_TYPE_MODEL_PROMOTE: u8 = 50;
pub const WITNESS_TYPE_MODEL_ROLLBACK: u8 = 51;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChainIntegrityChecker {
    pub chain_path: PathBuf,
    pub last_verified_offset: u64,
    pub rollback_target: Option<[u8; 32]>,
}

impl ChainIntegrityChecker {
    pub fn try_new(chain_path: PathBuf) -> Result<Self, HealError> {
        if chain_path.as_os_str().is_empty() {
            return Err(HealError::invalid(
                "chain_integrity.chain_path",
                "chain path must be non-empty",
            ));
        }
        Ok(Self {
            chain_path,
            last_verified_offset: 0,
            rollback_target: None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityVerdict {
    Valid,
    LegacyRepaired { entries_repaired: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ChainIntegrityReport {
    pub variant: IntegrityVerdict,
    pub entry_count: u64,
    pub replay_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IntegrityEvidence {
    pub entry_count: u64,
    pub first_entry_sha: [u8; 32],
    pub last_entry_sha: [u8; 32],
    pub replay_duration_ms: u64,
    pub status_change_after: StatusChange,
    pub rollback_target_sha: Option<[u8; 32]>,
    pub broken_at_offset: Option<usize>,
    pub clean_chain_valid: bool,
    pub blocker_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct WitnessChainAppender {
    pub chain_path: PathBuf,
    model_events: Vec<ModelWitnessEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModelWitnessEvent {
    pub offset: u64,
    pub op_type: String,
    pub weights_sha: [u8; 32],
    pub evaluation_summary_sha: [u8; 32],
}

pub const ACTIVE_POINTER_WITNESS_QUARANTINE: &str = "witness_quarantine";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WitnessQuarantineState {
    Active,
    Cleared,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WitnessQuarantineRecord {
    pub state: WitnessQuarantineState,
    pub reason: String,
    pub error_code: String,
    pub chain_path: PathBuf,
    pub broken_at_offset: Option<usize>,
    pub repair_promotion_id: Option<String>,
    pub recorded_at_unix_ms: i64,
    pub cleared_at_unix_ms: Option<i64>,
    pub source_of_truth_cf: String,
}

impl WitnessQuarantineRecord {
    pub fn validate(&self) -> Result<(), HealError> {
        if self.reason.trim().is_empty() {
            return Err(HealError::invalid(
                "witness_quarantine.reason",
                "reason must be non-empty",
            ));
        }
        if self.error_code != "MEJEPA_OBSERVE_INTEGRITY_VIOLATION"
            && self.error_code != "MEJEPA_HEAL_WITNESS_QUARANTINED"
        {
            return Err(HealError::invalid(
                "witness_quarantine.error_code",
                format!("unsupported error code {}", self.error_code),
            ));
        }
        if self.chain_path.as_os_str().is_empty() {
            return Err(HealError::invalid(
                "witness_quarantine.chain_path",
                "chain path must be non-empty",
            ));
        }
        if self.source_of_truth_cf != CF_MEJEPA_ACTIVE_POINTERS {
            return Err(HealError::invalid(
                "witness_quarantine.source_of_truth_cf",
                format!("must be {CF_MEJEPA_ACTIVE_POINTERS}"),
            ));
        }
        if self.state == WitnessQuarantineState::Active
            && self.repair_promotion_id.as_deref().unwrap_or("").is_empty()
        {
            return Err(HealError::invalid(
                "witness_quarantine.repair_promotion_id",
                "active quarantine must name its repair approval record",
            ));
        }
        Ok(())
    }

    pub fn is_active(&self) -> bool {
        self.state == WitnessQuarantineState::Active
    }
}

impl WitnessChainAppender {
    pub fn new(chain_path: PathBuf) -> Result<Self, HealError> {
        if let Some(parent) = chain_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| HealError::io("create_dir_all", parent, err))?;
        }
        if !chain_path.exists() {
            fs::write(&chain_path, []).map_err(|err| HealError::io("write", &chain_path, err))?;
        }
        Ok(Self {
            chain_path,
            model_events: Vec::new(),
        })
    }

    pub fn append_model_event(
        &mut self,
        op_type: &str,
        weights_sha: [u8; 32],
        evaluation_summary_sha: [u8; 32],
    ) -> Result<u64, HealError> {
        let mut action = Sha256::new();
        action.update(op_type.as_bytes());
        action.update(weights_sha);
        action.update(evaluation_summary_sha);
        let action_hash: [u8; 32] = action.finalize().into();
        let bytes = fs::read(&self.chain_path)
            .map_err(|err| HealError::io("read", &self.chain_path, err))?;
        let prev = if bytes.is_empty() {
            ZERO_HASH
        } else {
            verify_chain_bytes(&bytes).map_err(map_witness_error)?;
            shake256_32(&bytes[bytes.len() - WITNESS_ENTRY_SIZE..])
        };
        let witness_type = if op_type == "ModelRollback" {
            WITNESS_TYPE_MODEL_ROLLBACK
        } else {
            WITNESS_TYPE_MODEL_PROMOTE
        };
        let entry = WitnessEntry::new(
            prev,
            action_hash,
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default() as u64,
            witness_type,
        );
        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.chain_path)
            .map_err(|err| HealError::io("open", &self.chain_path, err))?;
        file.write_all(&entry.to_bytes())
            .map_err(|err| HealError::io("write", &self.chain_path, err))?;
        file.sync_all()
            .map_err(|err| HealError::io("sync", &self.chain_path, err))?;
        let offset = bytes.len() as u64 / WITNESS_ENTRY_SIZE as u64;
        self.model_events.push(ModelWitnessEvent {
            offset,
            op_type: op_type.to_string(),
            weights_sha,
            evaluation_summary_sha,
        });
        Ok(offset)
    }

    pub fn append_model_event_checked(
        &mut self,
        storage: &HealRocksStore,
        op_type: &str,
        weights_sha: [u8; 32],
        evaluation_summary_sha: [u8; 32],
    ) -> Result<u64, HealError> {
        ensure_witness_writes_allowed(storage)?;
        self.append_model_event(op_type, weights_sha, evaluation_summary_sha)
    }

    pub fn weights_sha_at_offset(&self, offset: u64) -> Result<Option<[u8; 32]>, HealError> {
        Ok(self
            .model_events
            .iter()
            .find(|event| event.offset == offset && event.op_type == "ModelPromote")
            .map(|event| event.weights_sha))
    }
}

pub fn witness_quarantine_key() -> Result<Vec<u8>, HealError> {
    encode_active_pointer_key(ACTIVE_POINTER_WITNESS_QUARANTINE)
}

pub fn is_witness_quarantine_pointer_key(key: &[u8]) -> bool {
    key == ACTIVE_POINTER_WITNESS_QUARANTINE.as_bytes()
}

pub fn read_witness_quarantine_record(
    storage: &HealRocksStore,
) -> Result<Option<WitnessQuarantineRecord>, HealError> {
    let key = witness_quarantine_key()?;
    let Some(bytes) = storage.get_cf(CF_MEJEPA_ACTIVE_POINTERS, &key)? else {
        return Ok(None);
    };
    let record: WitnessQuarantineRecord = decode_value(&bytes)?;
    record.validate()?;
    Ok(Some(record))
}

pub fn active_witness_quarantine(
    storage: &HealRocksStore,
) -> Result<Option<WitnessQuarantineRecord>, HealError> {
    Ok(read_witness_quarantine_record(storage)?.filter(WitnessQuarantineRecord::is_active))
}

pub fn ensure_witness_writes_allowed(storage: &HealRocksStore) -> Result<(), HealError> {
    if let Some(record) = active_witness_quarantine(storage)? {
        return Err(HealError::WitnessQuarantined {
            reason: record.reason,
            repair_promotion_id: record.repair_promotion_id,
        });
    }
    Ok(())
}

pub fn persist_witness_quarantine(
    storage: &HealRocksStore,
    chain_path: PathBuf,
    kind: &IntegrityViolationKind,
    repair_promotion_id: String,
) -> Result<WitnessQuarantineRecord, HealError> {
    let record = WitnessQuarantineRecord {
        state: WitnessQuarantineState::Active,
        reason: format!("{kind:?}"),
        error_code: "MEJEPA_OBSERVE_INTEGRITY_VIOLATION".to_string(),
        chain_path,
        broken_at_offset: broken_offset_from_kind(kind),
        repair_promotion_id: Some(repair_promotion_id),
        recorded_at_unix_ms: chrono::Utc::now().timestamp_millis(),
        cleared_at_unix_ms: None,
        source_of_truth_cf: CF_MEJEPA_ACTIVE_POINTERS.to_string(),
    };
    record.validate()?;
    storage.put_cf_readback(
        CF_MEJEPA_ACTIVE_POINTERS,
        &witness_quarantine_key()?,
        &encode_value(&record)?,
    )?;
    Ok(record)
}

pub fn complete_witness_quarantine_repair(
    storage: &HealRocksStore,
    checker: &ChainIntegrityChecker,
    promotion_id: &str,
) -> Result<WitnessQuarantineRecord, HealError> {
    let active = active_witness_quarantine(storage)?.ok_or_else(|| {
        HealError::invalid(
            "witness_quarantine.state",
            "no active witness quarantine to repair",
        )
    })?;
    if active.repair_promotion_id.as_deref() != Some(promotion_id) {
        return Err(HealError::invalid(
            "witness_quarantine.repair_promotion_id",
            format!("approval {promotion_id} does not match active quarantine"),
        ));
    }
    let bytes = fs::read(&checker.chain_path)
        .map_err(|err| HealError::io("read", &checker.chain_path, err))?;
    verify_chain_bytes(&bytes).map_err(map_witness_error)?;
    let executed = mark_promotion_executed(storage, promotion_id)?;
    let cleared = WitnessQuarantineRecord {
        state: WitnessQuarantineState::Cleared,
        reason: active.reason,
        error_code: "MEJEPA_HEAL_WITNESS_QUARANTINED".to_string(),
        chain_path: active.chain_path,
        broken_at_offset: active.broken_at_offset,
        repair_promotion_id: Some(executed.promotion_id),
        recorded_at_unix_ms: active.recorded_at_unix_ms,
        cleared_at_unix_ms: Some(chrono::Utc::now().timestamp_millis()),
        source_of_truth_cf: CF_MEJEPA_ACTIVE_POINTERS.to_string(),
    };
    cleared.validate()?;
    storage.put_cf_readback(
        CF_MEJEPA_ACTIVE_POINTERS,
        &witness_quarantine_key()?,
        &encode_value(&cleared)?,
    )?;
    Ok(cleared)
}

pub fn verify(
    checker: &mut ChainIntegrityChecker,
    witness_chain: &WitnessChainAppender,
    storage: &HealRocksStore,
    status: &Arc<Mutex<HealStatus>>,
) -> Result<ChainIntegrityReport, HealError> {
    verify_with_memory_root(checker, witness_chain, storage, status, Path::new("memory"))
}

fn verify_with_memory_root(
    checker: &mut ChainIntegrityChecker,
    _witness_chain: &WitnessChainAppender,
    storage: &HealRocksStore,
    status: &Arc<Mutex<HealStatus>>,
    memory_root: &Path,
) -> Result<ChainIntegrityReport, HealError> {
    let started = Instant::now();
    let bytes = fs::read(&checker.chain_path)
        .map_err(|err| HealError::io("read", &checker.chain_path, err))?;
    match verify_chain_bytes(&bytes) {
        Ok(report) => {
            checker.last_verified_offset = report.entries;
            if let Ok(mut status) = status.lock() {
                status.last_integrity_check_at = chrono::Utc::now().timestamp();
            }
            let out = ChainIntegrityReport {
                variant: IntegrityVerdict::Valid,
                entry_count: report.entries,
                replay_duration_ms: started.elapsed().as_millis() as u64,
            };
            write_smap_journal_chain_verified_at(memory_root, &out)?;
            Ok(out)
        }
        Err(err) => {
            let kind = map_witness_kind(&err);
            if active_witness_quarantine(storage)?.is_none() {
                let recorded_at = chrono::Utc::now().timestamp_millis();
                let repair_promotion_id = queue_pending_retrain_request(
                    storage,
                    PendingPromotionKind::WitnessChainRepairRequired {
                        chain_path: checker.chain_path.display().to_string(),
                        broken_at_offset: broken_offset_from_kind(&kind),
                        quarantine_recorded_at_unix_ms: recorded_at,
                    },
                    TriggerReason::OperatorTriggered,
                    format!("witness chain integrity failed: {kind:?}"),
                )?;
                let _ = persist_witness_quarantine(
                    storage,
                    checker.chain_path.clone(),
                    &kind,
                    repair_promotion_id,
                )?;
            }
            if let Ok(mut status) = status.lock() {
                status.status_change = StatusChange::Paused;
            }
            write_smap_blocker_critical_at(
                memory_root,
                "witness chain integrity failed",
                &kind,
                checker.rollback_target.as_ref(),
            )?;
            Err(HealError::IntegrityViolation { kind })
        }
    }
}

pub fn scan_last_good_modelpromote_weights_sha(
    _storage: &HealRocksStore,
    witness_chain: &WitnessChainAppender,
    broken_offset: usize,
) -> Result<[u8; 32], HealError> {
    witness_chain
        .model_events
        .iter()
        .rev()
        .find(|event| event.offset < broken_offset as u64 && event.op_type == "ModelPromote")
        .map(|event| event.weights_sha)
        .ok_or(HealError::IntegrityViolation {
            kind: IntegrityViolationKind::NoGoodCheckpoint { broken_offset },
        })
}

pub fn write_smap_blocker_critical(
    reason: &str,
    kind: &IntegrityViolationKind,
    rollback_target: Option<&[u8; 32]>,
) -> Result<PathBuf, HealError> {
    write_smap_blocker_critical_at(Path::new("memory"), reason, kind, rollback_target)
}

fn write_smap_blocker_critical_at(
    memory_root: &Path,
    reason: &str,
    kind: &IntegrityViolationKind,
    rollback_target: Option<&[u8; 32]>,
) -> Result<PathBuf, HealError> {
    let root = memory_root.join("blockers");
    fs::create_dir_all(&root).map_err(|err| HealError::io("create_dir_all", &root, err))?;
    let ts = chrono::Utc::now();
    let path = root.join(format!(
        "{}--mejepa-chain-integrity-broken.md",
        ts.format("%Y-%m-%d")
    ));
    let body = format!(
        "---\nnamespace: blockers\ncreated: {}\nupdated: {}\nstatus: active\nseverity: critical\ntags: mejepa, phase5, witness, integrity\n---\n\n# ME-JEPA Witness Chain Integrity Failure\n\nreason: {}\nkind: {:?}\nrollback_target: {:?}\n",
        ts.to_rfc3339(),
        ts.to_rfc3339(),
        reason,
        kind,
        rollback_target.map(hex::encode)
    );
    fs::write(&path, body).map_err(|err| HealError::io("write", &path, err))?;
    Ok(path)
}

pub fn write_smap_journal_chain_verified(
    report: &ChainIntegrityReport,
) -> Result<PathBuf, HealError> {
    write_smap_journal_chain_verified_at(Path::new("memory"), report)
}

fn write_smap_journal_chain_verified_at(
    memory_root: &Path,
    report: &ChainIntegrityReport,
) -> Result<PathBuf, HealError> {
    let root = memory_root.join("journal");
    fs::create_dir_all(&root).map_err(|err| HealError::io("create_dir_all", &root, err))?;
    let ts = chrono::Utc::now();
    let path = root.join(format!(
        "{}--success-mejepa-chain-integrity.md",
        ts.format("%Y-%m-%d")
    ));
    let body = format!(
        "---\nnamespace: journal\ncreated: {}\nupdated: {}\nstatus: active\ntags: mejepa, phase5, witness, integrity\n---\n\n# ME-JEPA Witness Chain Verified\n\nentries: {}\nreplay_duration_ms: {}\n",
        ts.to_rfc3339(),
        ts.to_rfc3339(),
        report.entry_count,
        report.replay_duration_ms
    );
    fs::write(&path, body).map_err(|err| HealError::io("write", &path, err))?;
    Ok(path)
}

pub fn integrity_evidence_from_chain(
    checker: &ChainIntegrityChecker,
    status_change_after: StatusChange,
    broken_at_offset: Option<usize>,
    clean_chain_valid: bool,
    replay_duration_ms: u64,
) -> Result<IntegrityEvidence, HealError> {
    let bytes = fs::read(&checker.chain_path)
        .map_err(|err| HealError::io("read", &checker.chain_path, err))?;
    let entry_count = (bytes.len() / WITNESS_ENTRY_SIZE) as u64;
    let first_entry_sha = if bytes.len() >= WITNESS_ENTRY_SIZE {
        shake256_32(&bytes[..WITNESS_ENTRY_SIZE])
    } else {
        [0; 32]
    };
    let last_entry_sha = if bytes.len() >= WITNESS_ENTRY_SIZE {
        shake256_32(&bytes[bytes.len() - WITNESS_ENTRY_SIZE..])
    } else {
        [0; 32]
    };
    Ok(IntegrityEvidence {
        entry_count,
        first_entry_sha,
        last_entry_sha,
        replay_duration_ms,
        status_change_after,
        rollback_target_sha: checker.rollback_target,
        broken_at_offset,
        clean_chain_valid,
        blocker_path: None,
    })
}

fn map_witness_error(err: WitnessError) -> HealError {
    HealError::IntegrityViolation {
        kind: map_witness_kind(&err),
    }
}

fn map_witness_kind(err: &WitnessError) -> IntegrityViolationKind {
    match err {
        WitnessError::ChainLengthInvalid { len, entry_size } => IntegrityViolationKind::WrongSize {
            actual: *len as u64,
            expected_modulo: *entry_size as u64,
        },
        WitnessError::PrevHashMismatch {
            offset,
            expected_prev_hash,
            actual_prev_hash,
        } => IntegrityViolationKind::BrokenAt {
            offset: *offset,
            expected_prev_hash: *expected_prev_hash,
            actual_prev_hash: *actual_prev_hash,
        },
        WitnessError::EntryLengthInvalid { actual, expected } => {
            IntegrityViolationKind::WrongSize {
                actual: *actual as u64,
                expected_modulo: *expected as u64,
            }
        }
        WitnessError::WitnessTypeRejected { offset, .. } => IntegrityViolationKind::BrokenAt {
            offset: *offset,
            expected_prev_hash: [0; 32],
            actual_prev_hash: [0; 32],
        },
    }
}

fn broken_offset_from_kind(kind: &IntegrityViolationKind) -> Option<usize> {
    match kind {
        IntegrityViolationKind::BrokenAt { offset, .. }
        | IntegrityViolationKind::NoGoodCheckpoint {
            broken_offset: offset,
        } => Some(*offset),
        IntegrityViolationKind::WrongSize { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heal::cf::CF_MEJEPA_WEIGHT_BLOBS;
    use crate::heal::promote_approval::{
        apply_promotion_approval, PromotionApprovalAction, PromotionApprovalRequest,
        PromotionApprovalState,
    };
    use serde_json::json;

    #[test]
    fn chain_integrity_checker_rejects_empty_path() {
        assert!(ChainIntegrityChecker::try_new(PathBuf::new()).is_err());
    }

    #[test]
    fn appender_writes_real_73_byte_chain_and_verify_reads_it() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("chain.bin");
        let mut appender = WitnessChainAppender::new(path.clone()).unwrap();
        appender
            .append_model_event("ModelPromote", [1; 32], [2; 32])
            .unwrap();
        appender
            .append_model_event("ModelRollback", [3; 32], [4; 32])
            .unwrap();
        let storage = HealRocksStore::open(temp.path().join("db")).unwrap();
        let mut checker = ChainIntegrityChecker::try_new(path).unwrap();
        let status = Arc::new(Mutex::new(HealStatus::default()));
        let report = verify_with_memory_root(
            &mut checker,
            &appender,
            &storage,
            &status,
            &temp.path().join("memory"),
        )
        .unwrap();
        assert_eq!(report.entry_count, 2);
    }

    #[test]
    fn witness_quarantine_readback_blocks_writes_until_approved_repair() {
        let (_temp, readback_root) = witness_quarantine_readback_root();
        fs::create_dir_all(&readback_root).expect("readback root");
        let run_id = format!(
            "{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let db_path = readback_root.join(format!("rocks-{run_id}"));
        let chain_path = readback_root.join(format!("witness-chain-{run_id}.bin"));
        let memory_root = readback_root.join(format!("memory-{run_id}"));
        let readback_path = readback_root.join("witness_quarantine_readback.json");

        let storage = HealRocksStore::open(&db_path).expect("open heal store");
        let before_active = storage
            .count_cf(CF_MEJEPA_ACTIVE_POINTERS)
            .expect("before active pointers");
        let before_weights = storage
            .count_cf(CF_MEJEPA_WEIGHT_BLOBS)
            .expect("before weight blobs");
        let before_promotions = storage
            .count_cf(context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS)
            .expect("before promotions");

        let mut appender = WitnessChainAppender::new(chain_path.clone()).expect("appender");
        appender
            .append_model_event("ModelPromote", [1; 32], [2; 32])
            .expect("append promote");
        appender
            .append_model_event("ModelPromote", [3; 32], [4; 32])
            .expect("append second promote");
        corrupt_second_entry(&chain_path);

        let mut checker = ChainIntegrityChecker::try_new(chain_path.clone()).expect("checker");
        let status = Arc::new(Mutex::new(HealStatus::default()));
        let integrity_error = verify_with_memory_root(
            &mut checker,
            &appender,
            storage.as_ref(),
            &status,
            &memory_root,
        )
        .expect_err("corrupt witness chain must fail");
        assert_eq!(integrity_error.code(), "MEJEPA_OBSERVE_INTEGRITY_VIOLATION");
        let active = active_witness_quarantine(storage.as_ref())
            .expect("active quarantine read")
            .expect("active quarantine row");
        let repair_promotion_id = active
            .repair_promotion_id
            .clone()
            .expect("repair promotion id");

        let blocked_write = storage
            .put_cf_readback(CF_MEJEPA_WEIGHT_BLOBS, b"blocked-weight", b"blocked")
            .expect_err("active quarantine must reject normal writes");
        assert_eq!(blocked_write.code(), "MEJEPA_HEAL_WITNESS_QUARANTINED");
        assert!(storage
            .get_cf(CF_MEJEPA_WEIGHT_BLOBS, b"blocked-weight")
            .expect("blocked readback")
            .is_none());

        let corrupt_repair =
            complete_witness_quarantine_repair(storage.as_ref(), &checker, &repair_promotion_id)
                .expect_err("corrupt chain cannot clear quarantine");
        assert_eq!(corrupt_repair.code(), "MEJEPA_OBSERVE_INTEGRITY_VIOLATION");

        write_clean_chain(&chain_path);
        let unapproved_repair =
            complete_witness_quarantine_repair(storage.as_ref(), &checker, &repair_promotion_id)
                .expect_err("operator approval must be required");
        assert_eq!(unapproved_repair.code(), "MEJEPA_HEAL_INVALID_STATE");

        let approval = apply_promotion_approval(
            storage.as_ref(),
            PromotionApprovalRequest {
                promotion_id: repair_promotion_id.clone(),
                operator_id: "operator-readback".to_string(),
                action: PromotionApprovalAction::Approve,
                reason: "witness chain manually repaired and verified".to_string(),
                two_person_rule: false,
            },
        )
        .expect("approve witness repair");
        assert_eq!(approval.state_after, PromotionApprovalState::Approved);

        let cleared =
            complete_witness_quarantine_repair(storage.as_ref(), &checker, &repair_promotion_id)
                .expect("approved clean chain clears quarantine");
        assert_eq!(cleared.state, WitnessQuarantineState::Cleared);
        assert!(active_witness_quarantine(storage.as_ref())
            .expect("post-clear quarantine read")
            .is_none());

        storage
            .put_cf_readback(CF_MEJEPA_WEIGHT_BLOBS, b"allowed-weight", b"allowed")
            .expect("write resumes after repair approval");
        assert_eq!(
            storage
                .get_cf(CF_MEJEPA_WEIGHT_BLOBS, b"allowed-weight")
                .expect("allowed readback")
                .as_deref(),
            Some(&b"allowed"[..])
        );

        flush_readback_cfs(storage.as_ref());
        let after_active = storage
            .count_cf(CF_MEJEPA_ACTIVE_POINTERS)
            .expect("after active pointers");
        let after_weights = storage
            .count_cf(CF_MEJEPA_WEIGHT_BLOBS)
            .expect("after weight blobs");
        let after_promotions = storage
            .count_cf(context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS)
            .expect("after promotions");
        let db_handle = storage.db();
        drop(db_handle);
        drop(storage);

        let reopened = HealRocksStore::open(&db_path).expect("reopen heal store");
        let reopened_record = read_witness_quarantine_record(reopened.as_ref())
            .expect("reopened quarantine read")
            .expect("reopened quarantine row");
        let reopened_allowed = reopened
            .get_cf(CF_MEJEPA_WEIGHT_BLOBS, b"allowed-weight")
            .expect("reopened allowed weight")
            .expect("allowed weight exists after reopen");
        let sst_files = collect_sst_files(&db_path);
        let physical_artifacts = sst_files
            .iter()
            .map(|path| json!({"path": path, "bytes": fs::metadata(path).map(|m| m.len()).unwrap_or(0)}))
            .collect::<Vec<_>>();
        let report = json!({
            "readback_root": readback_root,
            "task_id": "TASK-FLYWHEEL-011",
            "issue": 90,
            "started_at_unix_ms": chrono::Utc::now().timestamp_millis(),
            "build_release_sha": git_head_short(),
            "source_of_truth": {
                "cf": CF_MEJEPA_ACTIVE_POINTERS,
                "key": ACTIVE_POINTER_WITNESS_QUARANTINE,
                "db_path": db_path,
                "witness_chain_path": chain_path,
            },
            "happy_path": [{
                "case": "corrupt_witness_sets_quarantine",
                "sot": CF_MEJEPA_ACTIVE_POINTERS,
                "before": before_active,
                "after": after_active,
                "expected": "active quarantine row persisted, later cleared after repair approval",
                "actual": {
                    "initial_state": active.state,
                    "cleared_state": cleared.state,
                    "repair_promotion_id": repair_promotion_id,
                },
                "pass": true,
                "evidence_path": readback_path,
            }],
            "boundary_cases": [
                {
                    "case": "active_quarantine_rejects_normal_weight_write",
                    "expected": "MEJEPA_HEAL_WITNESS_QUARANTINED",
                    "actual": blocked_write.code(),
                    "pass": blocked_write.code() == "MEJEPA_HEAL_WITNESS_QUARANTINED",
                },
                {
                    "case": "corrupt_chain_cannot_clear_quarantine",
                    "expected": "MEJEPA_OBSERVE_INTEGRITY_VIOLATION",
                    "actual": corrupt_repair.code(),
                    "pass": corrupt_repair.code() == "MEJEPA_OBSERVE_INTEGRITY_VIOLATION",
                },
                {
                    "case": "clean_chain_still_requires_operator_approval",
                    "expected": "MEJEPA_HEAL_INVALID_STATE",
                    "actual": unapproved_repair.code(),
                    "pass": unapproved_repair.code() == "MEJEPA_HEAL_INVALID_STATE",
                },
                {
                    "case": "unsupported_active_pointer_key_fails_closed",
                    "expected": "MEJEPA_HEAL_INVALID_STATE",
                    "actual": encode_active_pointer_key("unexpected_pointer").unwrap_err().code(),
                    "pass": encode_active_pointer_key("unexpected_pointer").is_err(),
                },
                {
                    "case": "post_approval_repair_resumes_writes_after_reopen",
                    "expected": "allowed",
                    "actual": String::from_utf8_lossy(&reopened_allowed),
                    "pass": reopened_allowed == b"allowed",
                }
            ],
            "all_passed": reopened_record == cleared && reopened_allowed == b"allowed" && !sst_files.is_empty(),
            "cf_counts_before": {
                CF_MEJEPA_ACTIVE_POINTERS: before_active,
                CF_MEJEPA_WEIGHT_BLOBS: before_weights,
                context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS: before_promotions,
            },
            "cf_counts_after": {
                CF_MEJEPA_ACTIVE_POINTERS: after_active,
                CF_MEJEPA_WEIGHT_BLOBS: after_weights,
                context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS: after_promotions,
            },
            "readback_equal": reopened_record == cleared && reopened_allowed == b"allowed",
            "physical_artifacts": physical_artifacts,
            "witness_chain_sha256": sha256_file_hex(&chain_path),
        });
        assert!(report["all_passed"].as_bool().unwrap_or(false));
        fs::write(&readback_path, serde_json::to_vec_pretty(&report).unwrap())
            .expect("write readback report");
        let readback: serde_json::Value =
            serde_json::from_slice(&fs::read(&readback_path).expect("read readback report"))
                .expect("parse readback report");
        assert_eq!(readback, report);
    }

    fn witness_quarantine_readback_root() -> (Option<tempfile::TempDir>, PathBuf) {
        if let Some(root) = std::env::var_os("CG_MEJEPA_WITNESS_QUARANTINE_READBACK_ROOT") {
            return (None, PathBuf::from(root));
        }
        let temp = tempfile::tempdir().expect("witness quarantine readback tempdir");
        let root = temp.path().join("phase-f-witness-quarantine-readback");
        (Some(temp), root)
    }

    fn corrupt_second_entry(path: &Path) {
        let mut bytes = fs::read(path).expect("read witness chain");
        bytes[WITNESS_ENTRY_SIZE] ^= 0x7f;
        fs::write(path, bytes).expect("write corrupt witness chain");
    }

    fn write_clean_chain(path: &Path) {
        let first = WitnessEntry::new(ZERO_HASH, [9; 32], 100, WITNESS_TYPE_MODEL_PROMOTE);
        let second = WitnessEntry::new(
            first.chain_hash(),
            [10; 32],
            101,
            WITNESS_TYPE_MODEL_PROMOTE,
        );
        let mut bytes = Vec::with_capacity(WITNESS_ENTRY_SIZE * 2);
        bytes.extend_from_slice(&first.to_bytes());
        bytes.extend_from_slice(&second.to_bytes());
        fs::write(path, bytes).expect("write clean witness chain");
    }

    fn flush_readback_cfs(storage: &HealRocksStore) {
        let db = storage.db();
        for cf_name in [
            CF_MEJEPA_ACTIVE_POINTERS,
            CF_MEJEPA_WEIGHT_BLOBS,
            context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS,
        ] {
            let cf = db.cf_handle(cf_name).expect("cf handle");
            db.flush_cf(cf).expect("flush cf");
        }
    }

    fn collect_sst_files(root: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        collect_sst_files_into(root, &mut out);
        out.sort();
        out
    }

    fn collect_sst_files_into(root: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_sst_files_into(&path, out);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("sst") {
                out.push(path);
            }
        }
    }

    fn sha256_file_hex(path: &Path) -> String {
        let bytes = fs::read(path).expect("read sha file");
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }

    fn git_head_short() -> String {
        std::process::Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
                } else {
                    None
                }
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_string())
    }
}
