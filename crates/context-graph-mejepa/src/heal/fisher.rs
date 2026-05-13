use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::heal::errors::HealError;
use crate::heal::policy::{persist_policy_record, policy_key, scan_policy_records};
use crate::heal::store::HealRocksStore;
use crate::types::HeadId;

const HEAD_FISHER_PREFIX: &[u8] = b"phase_e/head-fisher/";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PerHeadFisherSnapshot {
    pub head: HeadId,
    pub diagonal: Vec<f32>,
    pub rank: usize,
    pub step: u64,
    pub model_id: String,
    pub witness_chain_offset: u64,
    pub persisted_at_unix_ms: i64,
}

impl PerHeadFisherSnapshot {
    pub fn try_new(
        head: HeadId,
        diagonal: Vec<f32>,
        step: u64,
        model_id: impl Into<String>,
        witness_chain_offset: u64,
    ) -> Result<Self, HealError> {
        if diagonal.is_empty()
            || diagonal
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(HealError::invalid(
                "per_head_fisher.diagonal",
                "diagonal must be non-empty finite non-negative values",
            ));
        }
        let model_id = model_id.into();
        if model_id.trim().is_empty() {
            return Err(HealError::invalid(
                "per_head_fisher.model_id",
                "model id must be non-empty",
            ));
        }
        let rank = diagonal.iter().filter(|value| **value > 1e-12).count();
        Ok(Self {
            head,
            diagonal,
            rank,
            step,
            model_id,
            witness_chain_offset,
            persisted_at_unix_ms: chrono::Utc::now().timestamp_millis(),
        })
    }
}

pub fn persist_per_head_fisher_snapshot(
    storage: &HealRocksStore,
    snapshot: &PerHeadFisherSnapshot,
) -> Result<Vec<u8>, HealError> {
    let key = fisher_key(snapshot)?;
    persist_policy_record(storage, &key, snapshot)?;
    Ok(key)
}

pub fn list_per_head_fisher_snapshots(
    storage: &HealRocksStore,
) -> Result<Vec<PerHeadFisherSnapshot>, HealError> {
    Ok(scan_policy_records::<PerHeadFisherSnapshot>(storage, HEAD_FISHER_PREFIX)?
        .into_iter()
        .map(|(_, value)| value)
        .collect())
}

pub fn snapshot_per_head_fisher(
    storage: &HealRocksStore,
    global_diagonal: &[f32],
    _global_rank: usize,
    source_step_count: u64,
) -> Result<Vec<PerHeadFisherSnapshot>, HealError> {
    if global_diagonal.is_empty() {
        return Err(HealError::invalid(
            "per_head_fisher.global_diagonal",
            "global Fisher diagonal must be non-empty",
        ));
    }
    if global_diagonal
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(HealError::invalid(
            "per_head_fisher.global_diagonal",
            "global Fisher diagonal must contain finite non-negative values",
        ));
    }
    let mut snapshots = Vec::new();
    for (head_idx, head) in HeadId::ALL.into_iter().enumerate() {
        let diagonal = global_diagonal
            .iter()
            .enumerate()
            .filter_map(|(idx, value)| (idx % HeadId::ALL.len() == head_idx).then_some(*value))
            .collect::<Vec<_>>();
        if diagonal.is_empty() {
            continue;
        }
        let snapshot = PerHeadFisherSnapshot::try_new(
            head,
            diagonal,
            source_step_count,
            "active_mejepa_predictor",
            source_step_count,
        )?;
        persist_per_head_fisher_snapshot(storage, &snapshot)?;
        snapshots.push(snapshot);
    }
    if snapshots.is_empty() {
        return Err(HealError::invalid(
            "per_head_fisher.snapshots",
            "no per-head snapshots were produced",
        ));
    }
    Ok(snapshots)
}

fn fisher_key(snapshot: &PerHeadFisherSnapshot) -> Result<Vec<u8>, HealError> {
    let mut hasher = Sha256::new();
    hasher.update(snapshot.head.as_str());
    hasher.update(snapshot.step.to_be_bytes());
    hasher.update(snapshot.model_id.as_bytes());
    hasher.update(snapshot.witness_chain_offset.to_be_bytes());
    policy_key(&[
        "phase_e",
        "head-fisher",
        snapshot.head.as_str(),
        &format!("{:020}-{}", snapshot.step, hex::encode(hasher.finalize())),
    ])
}
