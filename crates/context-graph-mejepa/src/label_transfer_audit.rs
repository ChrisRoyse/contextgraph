//! Runtime audit trail for cross-language label transfer decisions.
//!
//! `label_can_transfer()` is the policy. This module is the consumer-side
//! source of truth: every attempted label application produces a durable row in
//! `CF_MEJEPA_LABEL_TRANSFER_DECISIONS`, including same-language accepts,
//! cross-language transfers, and rejects.

use crate::cross_language_label_gating::{
    label_can_transfer, LabelTransferDecision, LabelTransferError, LabelTransferQuery,
};
use crate::types::{ChunkId, Language, PredictionId, RootCauseClass};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MAX_LABEL_SOURCE_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LabelTransferApplicationInput {
    pub prediction_id: PredictionId,
    pub label_source: String,
    pub source_chunk_id: ChunkId,
    pub target_chunk_id: ChunkId,
    pub source_language: Language,
    pub target_language: Language,
    pub source_root_cause: RootCauseClass,
    pub target_root_cause: RootCauseClass,
    pub similarity: f32,
    pub source_cross_language_correlation: f32,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LabelTransferAuditRecord {
    pub prediction_id: PredictionId,
    pub label_source: String,
    pub source_chunk_id: ChunkId,
    pub target_chunk_id: ChunkId,
    pub source_language: Language,
    pub target_language: Language,
    pub source_root_cause: RootCauseClass,
    pub target_root_cause: RootCauseClass,
    pub similarity: f32,
    pub source_cross_language_correlation: f32,
    pub decision: LabelTransferDecision,
    pub applied_weight: f32,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LabelTransferAuditSummary {
    pub source_of_truth: String,
    pub total_rows: usize,
    pub same_language_count: usize,
    pub transfer_count: usize,
    pub reject_count: usize,
    pub pair_summaries: Vec<LabelTransferPairSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LabelTransferPairSummary {
    pub source_language: Language,
    pub target_language: Language,
    pub total_count: usize,
    pub accepted_count: usize,
    pub same_language_count: usize,
    pub transfer_count: usize,
    pub reject_count: usize,
    pub accept_ratio: f32,
    pub reject_ratio: f32,
    pub effective_weight_sum: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LabelTransferAuditError {
    #[error("MEJEPA_LABEL_TRANSFER_AUDIT_INVALID_INPUT: field={field} detail={detail}")]
    InvalidInput { field: String, detail: String },
    #[error("MEJEPA_LABEL_TRANSFER_AUDIT_CF_MISSING: {0}")]
    MissingColumnFamily(&'static str),
    #[error("MEJEPA_LABEL_TRANSFER_AUDIT_READBACK_MISMATCH: {0}")]
    ReadbackMismatch(String),
    #[error("MEJEPA_LABEL_TRANSFER_AUDIT_POLICY: {0}")]
    Policy(String),
    #[error("MEJEPA_LABEL_TRANSFER_AUDIT_ROCKSDB: {0}")]
    RocksDb(String),
    #[error("MEJEPA_LABEL_TRANSFER_AUDIT_BINCODE: {0}")]
    Bincode(String),
}

impl LabelTransferAuditError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "MEJEPA_LABEL_TRANSFER_AUDIT_INVALID_INPUT",
            Self::MissingColumnFamily(_) => "MEJEPA_LABEL_TRANSFER_AUDIT_CF_MISSING",
            Self::ReadbackMismatch(_) => "MEJEPA_LABEL_TRANSFER_AUDIT_READBACK_MISMATCH",
            Self::Policy(_) => "MEJEPA_LABEL_TRANSFER_AUDIT_POLICY",
            Self::RocksDb(_) => "MEJEPA_LABEL_TRANSFER_AUDIT_ROCKSDB",
            Self::Bincode(_) => "MEJEPA_LABEL_TRANSFER_AUDIT_BINCODE",
        }
    }
}

pub fn apply_label_transfer_decision(
    input: LabelTransferApplicationInput,
) -> Result<LabelTransferAuditRecord, LabelTransferAuditError> {
    validate_application_input(&input)?;
    let decision = label_can_transfer(&LabelTransferQuery {
        source_language: input.source_language,
        target_language: input.target_language,
        source_root_cause: input.source_root_cause,
        target_root_cause: input.target_root_cause,
        similarity: input.similarity,
        source_cross_language_correlation: input.source_cross_language_correlation,
    })
    .map_err(policy_error)?;
    let applied_weight = match decision {
        LabelTransferDecision::SameLanguage { weight }
        | LabelTransferDecision::Transfer { weight } => weight,
        LabelTransferDecision::Reject { .. } => 0.0,
    };
    Ok(LabelTransferAuditRecord {
        prediction_id: input.prediction_id,
        label_source: input.label_source,
        source_chunk_id: input.source_chunk_id,
        target_chunk_id: input.target_chunk_id,
        source_language: input.source_language,
        target_language: input.target_language,
        source_root_cause: input.source_root_cause,
        target_root_cause: input.target_root_cause,
        similarity: input.similarity,
        source_cross_language_correlation: input.source_cross_language_correlation,
        decision,
        applied_weight,
        created_at_unix_ms: input.created_at_unix_ms,
    })
}

pub fn apply_and_persist_label_transfer_decision(
    db: &DB,
    input: LabelTransferApplicationInput,
) -> Result<LabelTransferAuditRecord, LabelTransferAuditError> {
    let record = apply_label_transfer_decision(input)?;
    persist_label_transfer_decision(db, &record)?;
    Ok(record)
}

pub fn persist_label_transfer_decision(
    db: &DB,
    record: &LabelTransferAuditRecord,
) -> Result<(), LabelTransferAuditError> {
    validate_record(record)?;
    let cf_name = context_graph_mejepa_cf::CF_MEJEPA_LABEL_TRANSFER_DECISIONS;
    let cf = db
        .cf_handle(cf_name)
        .ok_or(LabelTransferAuditError::MissingColumnFamily(cf_name))?;
    let key = label_transfer_decision_key(record)?;
    let bytes = bincode::serialize(record).map_err(bincode_error)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, &key, &bytes, &opts)
        .map_err(rocksdb_error)?;
    let readback = db.get_cf(cf, &key).map_err(rocksdb_error)?.ok_or_else(|| {
        LabelTransferAuditError::ReadbackMismatch(
            "missing CF_MEJEPA_LABEL_TRANSFER_DECISIONS readback".to_string(),
        )
    })?;
    if readback != bytes {
        return Err(LabelTransferAuditError::ReadbackMismatch(
            "CF_MEJEPA_LABEL_TRANSFER_DECISIONS readback bytes differ".to_string(),
        ));
    }
    let decoded: LabelTransferAuditRecord =
        bincode::deserialize(&readback).map_err(bincode_error)?;
    if decoded != *record {
        return Err(LabelTransferAuditError::ReadbackMismatch(
            "CF_MEJEPA_LABEL_TRANSFER_DECISIONS decoded readback differs".to_string(),
        ));
    }
    Ok(())
}

pub fn label_transfer_decision_key(
    record: &LabelTransferAuditRecord,
) -> Result<Vec<u8>, LabelTransferAuditError> {
    validate_record(record)?;
    bincode::serialize(&(
        record.prediction_id,
        record.label_source.clone(),
        record.source_chunk_id.clone(),
        record.target_chunk_id.clone(),
    ))
    .map_err(bincode_error)
}

pub fn load_label_transfer_decision(
    db: &DB,
    key: &[u8],
) -> Result<Option<LabelTransferAuditRecord>, LabelTransferAuditError> {
    let cf_name = context_graph_mejepa_cf::CF_MEJEPA_LABEL_TRANSFER_DECISIONS;
    let cf = db
        .cf_handle(cf_name)
        .ok_or(LabelTransferAuditError::MissingColumnFamily(cf_name))?;
    let Some(bytes) = db.get_cf(cf, key).map_err(rocksdb_error)? else {
        return Ok(None);
    };
    let record = bincode::deserialize(&bytes).map_err(bincode_error)?;
    validate_record(&record)?;
    Ok(Some(record))
}

pub fn summarize_label_transfer_decisions(
    db: &DB,
) -> Result<LabelTransferAuditSummary, LabelTransferAuditError> {
    let cf_name = context_graph_mejepa_cf::CF_MEJEPA_LABEL_TRANSFER_DECISIONS;
    let cf = db
        .cf_handle(cf_name)
        .ok_or(LabelTransferAuditError::MissingColumnFamily(cf_name))?;
    let mut pair_counts = BTreeMap::<(Language, Language), PairAccumulator>::new();
    let mut summary = LabelTransferAuditSummary {
        source_of_truth: cf_name.to_string(),
        ..LabelTransferAuditSummary::default()
    };
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item.map_err(rocksdb_error)?;
        let record: LabelTransferAuditRecord =
            bincode::deserialize(&value).map_err(bincode_error)?;
        validate_record(&record)?;
        summary.total_rows += 1;
        let pair = pair_counts
            .entry((record.source_language, record.target_language))
            .or_default();
        pair.total_count += 1;
        pair.effective_weight_sum += record.applied_weight;
        match record.decision {
            LabelTransferDecision::SameLanguage { .. } => {
                summary.same_language_count += 1;
                pair.same_language_count += 1;
                pair.accepted_count += 1;
            }
            LabelTransferDecision::Transfer { .. } => {
                summary.transfer_count += 1;
                pair.transfer_count += 1;
                pair.accepted_count += 1;
            }
            LabelTransferDecision::Reject { .. } => {
                summary.reject_count += 1;
                pair.reject_count += 1;
            }
        }
    }
    summary.pair_summaries = pair_counts
        .into_iter()
        .map(|((source_language, target_language), acc)| {
            let total = acc.total_count.max(1) as f32;
            LabelTransferPairSummary {
                source_language,
                target_language,
                total_count: acc.total_count,
                accepted_count: acc.accepted_count,
                same_language_count: acc.same_language_count,
                transfer_count: acc.transfer_count,
                reject_count: acc.reject_count,
                accept_ratio: acc.accepted_count as f32 / total,
                reject_ratio: acc.reject_count as f32 / total,
                effective_weight_sum: acc.effective_weight_sum,
            }
        })
        .collect();
    Ok(summary)
}

fn validate_application_input(
    input: &LabelTransferApplicationInput,
) -> Result<(), LabelTransferAuditError> {
    validate_prediction_id(input.prediction_id)?;
    validate_label_source(&input.label_source)?;
    input
        .source_chunk_id
        .validate("label_transfer.source_chunk_id")
        .map_err(mejepa_input_error)?;
    input
        .target_chunk_id
        .validate("label_transfer.target_chunk_id")
        .map_err(mejepa_input_error)?;
    if input.created_at_unix_ms < 0 {
        return Err(LabelTransferAuditError::InvalidInput {
            field: "created_at_unix_ms".to_string(),
            detail: "must be non-negative".to_string(),
        });
    }
    Ok(())
}

fn validate_record(record: &LabelTransferAuditRecord) -> Result<(), LabelTransferAuditError> {
    validate_prediction_id(record.prediction_id)?;
    validate_label_source(&record.label_source)?;
    record
        .source_chunk_id
        .validate("label_transfer.source_chunk_id")
        .map_err(mejepa_input_error)?;
    record
        .target_chunk_id
        .validate("label_transfer.target_chunk_id")
        .map_err(mejepa_input_error)?;
    if !record.similarity.is_finite() || !(0.0..=1.0).contains(&record.similarity) {
        return Err(LabelTransferAuditError::InvalidInput {
            field: "similarity".to_string(),
            detail: format!("must be finite and in [0,1]; got {}", record.similarity),
        });
    }
    if !record.source_cross_language_correlation.is_finite()
        || !(-1.0..=1.0).contains(&record.source_cross_language_correlation)
    {
        return Err(LabelTransferAuditError::InvalidInput {
            field: "source_cross_language_correlation".to_string(),
            detail: format!(
                "must be finite and in [-1,1]; got {}",
                record.source_cross_language_correlation
            ),
        });
    }
    if !record.applied_weight.is_finite() || !(0.0..=1.0).contains(&record.applied_weight) {
        return Err(LabelTransferAuditError::InvalidInput {
            field: "applied_weight".to_string(),
            detail: format!("must be finite and in [0,1]; got {}", record.applied_weight),
        });
    }
    if record.created_at_unix_ms < 0 {
        return Err(LabelTransferAuditError::InvalidInput {
            field: "created_at_unix_ms".to_string(),
            detail: "must be non-negative".to_string(),
        });
    }
    Ok(())
}

fn validate_prediction_id(prediction_id: PredictionId) -> Result<(), LabelTransferAuditError> {
    if prediction_id.0 == [0_u8; 16] {
        return Err(LabelTransferAuditError::InvalidInput {
            field: "prediction_id".to_string(),
            detail: "must be non-zero".to_string(),
        });
    }
    Ok(())
}

fn validate_label_source(label_source: &str) -> Result<(), LabelTransferAuditError> {
    if label_source.trim().is_empty() {
        return Err(LabelTransferAuditError::InvalidInput {
            field: "label_source".to_string(),
            detail: "must be non-empty".to_string(),
        });
    }
    if label_source.len() > MAX_LABEL_SOURCE_BYTES {
        return Err(LabelTransferAuditError::InvalidInput {
            field: "label_source".to_string(),
            detail: format!("exceeds {MAX_LABEL_SOURCE_BYTES} bytes"),
        });
    }
    if label_source.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return Err(LabelTransferAuditError::InvalidInput {
            field: "label_source".to_string(),
            detail: "contains a control character".to_string(),
        });
    }
    Ok(())
}

fn mejepa_input_error(err: crate::MejepaInferError) -> LabelTransferAuditError {
    LabelTransferAuditError::InvalidInput {
        field: "label_transfer".to_string(),
        detail: err.to_string(),
    }
}

fn policy_error(err: LabelTransferError) -> LabelTransferAuditError {
    LabelTransferAuditError::Policy(err.to_string())
}

fn rocksdb_error(err: rocksdb::Error) -> LabelTransferAuditError {
    LabelTransferAuditError::RocksDb(err.to_string())
}

fn bincode_error(err: Box<bincode::ErrorKind>) -> LabelTransferAuditError {
    LabelTransferAuditError::Bincode(err.to_string())
}

#[derive(Debug, Default)]
struct PairAccumulator {
    total_count: usize,
    accepted_count: usize,
    same_language_count: usize,
    transfer_count: usize,
    reject_count: usize,
    effective_weight_sum: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cross_language_label_gating::TRANSFERRED_LABEL_WEIGHT;

    fn input(
        source_language: Language,
        target_language: Language,
        similarity: f32,
        correlation: f32,
    ) -> LabelTransferApplicationInput {
        LabelTransferApplicationInput {
            prediction_id: PredictionId([0x5a; 16]),
            label_source: "pytest:failure_mode:0".to_string(),
            source_chunk_id: ChunkId("python/src.py#fn".to_string()),
            target_chunk_id: ChunkId("rust/src.rs#fn".to_string()),
            source_language,
            target_language,
            source_root_cause: RootCauseClass::LogicError,
            target_root_cause: RootCauseClass::LogicError,
            similarity,
            source_cross_language_correlation: correlation,
            created_at_unix_ms: 1,
        }
    }

    #[test]
    fn cross_language_transfer_gets_half_weight() {
        let record =
            apply_label_transfer_decision(input(Language::Python, Language::Rust, 0.95, 0.90))
                .unwrap();
        assert_eq!(
            record.decision,
            LabelTransferDecision::Transfer {
                weight: TRANSFERRED_LABEL_WEIGHT
            }
        );
        assert_eq!(record.applied_weight, TRANSFERRED_LABEL_WEIGHT);
    }

    #[test]
    fn low_similarity_rejects_with_zero_weight() {
        let record =
            apply_label_transfer_decision(input(Language::Python, Language::Rust, 0.10, 0.90))
                .unwrap();
        assert!(matches!(
            record.decision,
            LabelTransferDecision::Reject { .. }
        ));
        assert_eq!(record.applied_weight, 0.0);
    }

    #[test]
    fn malformed_source_fails_closed() {
        let mut bad = input(Language::Python, Language::Rust, 0.95, 0.90);
        bad.label_source = "pytest\nbad".to_string();
        let err = apply_label_transfer_decision(bad).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_LABEL_TRANSFER_AUDIT_INVALID_INPUT");
    }
}
