use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::heal::errors::HealError;
use crate::heal::policy::{
    load_policy_record, persist_policy_record, policy_key, scan_policy_records,
};
use crate::heal::store::HealRocksStore;
use crate::types::HeadId;

const HEAD_FISHER_PREFIX: &[u8] = b"phase_e/head-fisher/";
const CHUNK_FOUNDATIONALITY_FISHER_PREFIX: &[u8] = b"phase_e/chunk-foundationality-fisher/";
pub const CHUNK_FOUNDATIONALITY_FISHER_SCHEMA_VERSION: u32 = 1;
pub const FOUNDATIONALITY_FISHER_MULTIPLIER_SOURCE: &str =
    "CF_MEJEPA_CHUNK_FOUNDATIONALITY.fisher_multiplier";

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FoundationalityFisherEntry {
    pub chunk_id: String,
    pub base_fisher: f32,
    pub fisher_multiplier: f32,
    pub weighted_fisher: f32,
    pub foundationality_score: f32,
    pub fisher_lambda: f32,
    pub foundationality_rank: u32,
    pub dependency_graph_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FoundationalityWeightedFisherReport {
    pub schema_version: u32,
    pub source_step_count: u64,
    pub chunk_count: usize,
    pub diagonal_len: usize,
    pub multiplier_source: String,
    pub source_foundationality_cf: String,
    pub consumed_at_unix_ms: i64,
    pub entries: Vec<FoundationalityFisherEntry>,
    pub weighted_diagonal: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FoundationalityWeightedFisherReadback {
    pub policy_key_hex: String,
    pub readback_matches: bool,
    pub report: FoundationalityWeightedFisherReport,
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

impl FoundationalityFisherEntry {
    fn try_new(
        chunk_id: String,
        base_fisher: f32,
        score: crate::ChunkFoundationalityScore,
    ) -> Result<Self, HealError> {
        validate_chunk_id("foundationality_fisher.chunk_id", &chunk_id)?;
        validate_nonnegative_finite("foundationality_fisher.base_fisher", base_fisher)?;
        score.validate().map_err(map_foundationality_error)?;
        if score.chunk_id.as_str() != chunk_id {
            return Err(HealError::invalid(
                "foundationality_fisher.chunk_id",
                format!(
                    "requested chunk id {chunk_id:?} but CF row returned {:?}",
                    score.chunk_id
                ),
            ));
        }
        validate_nonnegative_finite(
            "foundationality_fisher.persisted_multiplier",
            score.fisher_multiplier,
        )?;
        let weighted_fisher = base_fisher * score.fisher_multiplier;
        validate_nonnegative_finite("foundationality_fisher.weighted", weighted_fisher)?;
        Ok(Self {
            chunk_id,
            base_fisher,
            fisher_multiplier: score.fisher_multiplier,
            weighted_fisher,
            foundationality_score: score.foundationality_score,
            fisher_lambda: score.fisher_lambda,
            foundationality_rank: score.rank,
            dependency_graph_sha256: score.dependency_graph_sha256,
        })
    }
}

impl FoundationalityWeightedFisherReport {
    fn try_new(
        source_step_count: u64,
        entries: Vec<FoundationalityFisherEntry>,
    ) -> Result<Self, HealError> {
        if entries.is_empty() {
            return Err(HealError::invalid(
                "foundationality_fisher.entries",
                "at least one chunk Fisher entry is required",
            ));
        }
        let weighted_diagonal = entries
            .iter()
            .map(|entry| entry.weighted_fisher)
            .collect::<Vec<_>>();
        let report = Self {
            schema_version: CHUNK_FOUNDATIONALITY_FISHER_SCHEMA_VERSION,
            source_step_count,
            chunk_count: entries.len(),
            diagonal_len: weighted_diagonal.len(),
            multiplier_source: FOUNDATIONALITY_FISHER_MULTIPLIER_SOURCE.to_string(),
            source_foundationality_cf: context_graph_mejepa_cf::CF_MEJEPA_CHUNK_FOUNDATIONALITY
                .to_string(),
            consumed_at_unix_ms: chrono::Utc::now().timestamp_millis(),
            entries,
            weighted_diagonal,
        };
        report.validate()?;
        Ok(report)
    }

    pub fn validate(&self) -> Result<(), HealError> {
        if self.schema_version != CHUNK_FOUNDATIONALITY_FISHER_SCHEMA_VERSION {
            return Err(HealError::invalid(
                "foundationality_fisher.schema_version",
                format!(
                    "expected {CHUNK_FOUNDATIONALITY_FISHER_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            ));
        }
        if self.multiplier_source != FOUNDATIONALITY_FISHER_MULTIPLIER_SOURCE {
            return Err(HealError::invalid(
                "foundationality_fisher.multiplier_source",
                "report must bind to the persisted chunk foundationality multiplier field",
            ));
        }
        if self.source_foundationality_cf
            != context_graph_mejepa_cf::CF_MEJEPA_CHUNK_FOUNDATIONALITY
        {
            return Err(HealError::invalid(
                "foundationality_fisher.source_foundationality_cf",
                format!(
                    "expected {}, got {}",
                    context_graph_mejepa_cf::CF_MEJEPA_CHUNK_FOUNDATIONALITY,
                    self.source_foundationality_cf
                ),
            ));
        }
        if self.consumed_at_unix_ms <= 0 {
            return Err(HealError::invalid(
                "foundationality_fisher.consumed_at_unix_ms",
                "timestamp must be positive",
            ));
        }
        if self.entries.is_empty() {
            return Err(HealError::invalid(
                "foundationality_fisher.entries",
                "entries must be non-empty",
            ));
        }
        if self.chunk_count != self.entries.len()
            || self.diagonal_len != self.weighted_diagonal.len()
        {
            return Err(HealError::invalid(
                "foundationality_fisher.len",
                "chunk_count/diagonal_len must match payload lengths",
            ));
        }
        if self.weighted_diagonal.len() != self.entries.len() {
            return Err(HealError::invalid(
                "foundationality_fisher.weighted_diagonal",
                "weighted diagonal length must match entries length",
            ));
        }
        for (idx, entry) in self.entries.iter().enumerate() {
            validate_chunk_id("foundationality_fisher.entry.chunk_id", &entry.chunk_id)?;
            validate_nonnegative_finite(
                "foundationality_fisher.entry.base_fisher",
                entry.base_fisher,
            )?;
            validate_nonnegative_finite(
                "foundationality_fisher.entry.fisher_multiplier",
                entry.fisher_multiplier,
            )?;
            validate_nonnegative_finite(
                "foundationality_fisher.entry.weighted_fisher",
                entry.weighted_fisher,
            )?;
            if (self.weighted_diagonal[idx] - entry.weighted_fisher).abs() > 1e-6 {
                return Err(HealError::invalid(
                    "foundationality_fisher.weighted_diagonal",
                    format!("weighted diagonal entry {idx} does not match row payload"),
                ));
            }
        }
        Ok(())
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
    Ok(
        scan_policy_records::<PerHeadFisherSnapshot>(storage, HEAD_FISHER_PREFIX)?
            .into_iter()
            .map(|(_, value)| value)
            .collect(),
    )
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

pub fn consume_persisted_chunk_foundationality_fisher(
    storage: &HealRocksStore,
    chunk_ids: &[String],
    base_fisher_diagonal: &[f32],
    source_step_count: u64,
) -> Result<FoundationalityWeightedFisherReadback, HealError> {
    if chunk_ids.is_empty() {
        return Err(HealError::invalid(
            "foundationality_fisher.chunk_ids",
            "at least one chunk id is required",
        ));
    }
    if base_fisher_diagonal.is_empty() {
        return Err(HealError::invalid(
            "foundationality_fisher.base_diagonal",
            "base Fisher diagonal must be non-empty",
        ));
    }
    if chunk_ids.len() != base_fisher_diagonal.len() {
        return Err(HealError::invalid(
            "foundationality_fisher.len",
            format!(
                "chunk id count {} != base Fisher diagonal len {}",
                chunk_ids.len(),
                base_fisher_diagonal.len()
            ),
        ));
    }
    let db = storage.db();
    let mut entries = Vec::with_capacity(chunk_ids.len());
    for (chunk_id, base_fisher) in chunk_ids.iter().zip(base_fisher_diagonal) {
        validate_chunk_id("foundationality_fisher.chunk_id", chunk_id)?;
        validate_nonnegative_finite("foundationality_fisher.base_fisher", *base_fisher)?;
        let score = crate::read_chunk_foundationality_score(db.as_ref(), chunk_id)
            .map_err(map_foundationality_error)?
            .ok_or_else(|| {
                HealError::invalid(
                    "foundationality_fisher.missing_foundationality",
                    format!(
                        "missing persisted foundationality row for chunk {chunk_id:?} in {}",
                        context_graph_mejepa_cf::CF_MEJEPA_CHUNK_FOUNDATIONALITY
                    ),
                )
            })?;
        entries.push(FoundationalityFisherEntry::try_new(
            chunk_id.clone(),
            *base_fisher,
            score,
        )?);
    }
    let report = FoundationalityWeightedFisherReport::try_new(source_step_count, entries)?;
    let key = foundationality_weighted_fisher_key(&report)?;
    persist_policy_record(storage, &key, &report)?;
    let readback = load_policy_record::<FoundationalityWeightedFisherReport>(storage, &key)?
        .ok_or_else(|| {
            HealError::invalid(
                "foundationality_fisher.readback",
                format!("missing policy row after write {}", hex::encode(&key)),
            )
        })?;
    readback.validate()?;
    Ok(FoundationalityWeightedFisherReadback {
        policy_key_hex: hex::encode(&key),
        readback_matches: readback == report,
        report: readback,
    })
}

pub fn list_foundationality_weighted_fisher_reports(
    storage: &HealRocksStore,
) -> Result<Vec<FoundationalityWeightedFisherReport>, HealError> {
    Ok(scan_policy_records::<FoundationalityWeightedFisherReport>(
        storage,
        CHUNK_FOUNDATIONALITY_FISHER_PREFIX,
    )?
    .into_iter()
    .map(|(_, value)| value)
    .collect())
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

fn foundationality_weighted_fisher_key(
    report: &FoundationalityWeightedFisherReport,
) -> Result<Vec<u8>, HealError> {
    report.validate()?;
    let mut hasher = Sha256::new();
    hasher.update(report.source_step_count.to_be_bytes());
    for entry in &report.entries {
        hasher.update(entry.chunk_id.as_bytes());
        hasher.update(entry.base_fisher.to_bits().to_be_bytes());
        hasher.update(entry.fisher_multiplier.to_bits().to_be_bytes());
        hasher.update(entry.weighted_fisher.to_bits().to_be_bytes());
        hasher.update(entry.dependency_graph_sha256.as_bytes());
    }
    policy_key(&[
        "phase_e",
        "chunk-foundationality-fisher",
        &format!(
            "{:020}-{}",
            report.source_step_count,
            hex::encode(hasher.finalize())
        ),
    ])
}

fn map_foundationality_error(err: crate::MejepaInferError) -> HealError {
    HealError::invalid(
        "foundationality_fisher.cf_read",
        format!("{}: {err}", err.code()),
    )
}

fn validate_chunk_id(field: &str, value: &str) -> Result<(), HealError> {
    if value.trim().is_empty()
        || value.len() > 512
        || value.bytes().any(|byte| byte == b'\n' || byte == 0)
    {
        return Err(HealError::invalid(
            field,
            "chunk id must be non-empty single-line text <= 512 bytes",
        ));
    }
    Ok(())
}

fn validate_nonnegative_finite(field: &str, value: f32) -> Result<(), HealError> {
    if !value.is_finite() || value < 0.0 {
        return Err(HealError::invalid(
            field,
            "value must be finite and non-negative",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        write_chunk_foundationality_score_sync_readback, ChunkFoundationalityScore,
        CHUNK_FOUNDATIONALITY_SCHEMA_VERSION,
    };

    fn score(
        chunk_id: &str,
        foundationality_score: f32,
        fisher_lambda: f32,
        rank: u32,
    ) -> ChunkFoundationalityScore {
        ChunkFoundationalityScore {
            schema_version: CHUNK_FOUNDATIONALITY_SCHEMA_VERSION,
            chunk_id: chunk_id.to_string(),
            foundationality_score,
            raw_pagerank: foundationality_score as f64,
            rank,
            upstream_count: 1,
            downstream_count: 1,
            dependency_graph_sha256: "a".repeat(64),
            computed_at_unix_ms: 1,
            fisher_lambda,
            fisher_multiplier: 1.0 + fisher_lambda * foundationality_score,
        }
    }

    #[test]
    fn persisted_foundationality_multiplier_weights_fisher_and_round_trips_policy() {
        let temp = tempfile::tempdir().unwrap();
        let storage = HealRocksStore::open(temp.path()).unwrap();
        let db = storage.db();
        write_chunk_foundationality_score_sync_readback(
            db.as_ref(),
            &score("core:contract", 1.0, 2.0, 1),
        )
        .unwrap();
        write_chunk_foundationality_score_sync_readback(
            db.as_ref(),
            &score("leaf:test", 0.25, 2.0, 2),
        )
        .unwrap();

        let chunk_ids = vec!["core:contract".to_string(), "leaf:test".to_string()];
        let readback =
            consume_persisted_chunk_foundationality_fisher(&storage, &chunk_ids, &[2.0, 2.0], 42)
                .unwrap();

        assert!(readback.readback_matches);
        assert_eq!(
            readback.report.multiplier_source,
            FOUNDATIONALITY_FISHER_MULTIPLIER_SOURCE
        );
        assert_eq!(readback.report.weighted_diagonal, vec![6.0, 3.0]);
        assert_eq!(readback.report.entries[0].fisher_multiplier, 3.0);
        assert_eq!(
            storage
                .count_cf(context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS)
                .unwrap(),
            1
        );
        let reports = list_foundationality_weighted_fisher_reports(&storage).unwrap();
        assert_eq!(reports, vec![readback.report]);
    }

    #[test]
    fn missing_foundationality_row_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let storage = HealRocksStore::open(temp.path()).unwrap();
        let err = consume_persisted_chunk_foundationality_fisher(
            &storage,
            &["missing:chunk".to_string()],
            &[1.0],
            42,
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_HEAL_INVALID_STATE");
        assert!(err
            .to_string()
            .contains("missing persisted foundationality row"));
    }
}
