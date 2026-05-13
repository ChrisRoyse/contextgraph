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

use crate::heal::errors::{HealError, IntegrityViolationKind};
use crate::heal::pipeline::{HealStatus, StatusChange};
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

    pub fn weights_sha_at_offset(&self, offset: u64) -> Result<Option<[u8; 32]>, HealError> {
        Ok(self
            .model_events
            .iter()
            .find(|event| event.offset == offset && event.op_type == "ModelPromote")
            .map(|event| event.weights_sha))
    }
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
    _storage: &HealRocksStore,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
