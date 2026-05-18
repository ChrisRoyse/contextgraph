use rocksdb::{WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::cf;
use crate::data_models::TargetProvenance;
use crate::dynamic_embedder::RuntimeEmbedderId;
use crate::embedder_falsification::{
    evaluate_embedder_candidate_falsification, EmbedderCandidateHoldoutComparison,
    EmbedderFalsificationDecision, EmbedderFalsificationGate,
};
use crate::error::MejepaInferError;
use crate::frozen_target::FrozenTargetAdapter;
use crate::grad_hook::{InstrumentGradHandle, TensorId};

pub const DYNAMIC_EMBEDDER_FREEZE_AUDIT_SCHEMA_VERSION: u32 = 1;
pub const DYNAMIC_EMBEDDER_PROMOTION_WITNESS_OP: &str = "DynamicEmbedderPromote";

#[derive(Debug, Clone)]
pub struct DynamicEmbedderGradHandle {
    instrument_id: String,
    tensor_ids: Vec<TensorId>,
}

impl DynamicEmbedderGradHandle {
    pub fn new(id: &RuntimeEmbedderId) -> Result<Self, MejepaInferError> {
        id.validate().map_err(embed_error)?;
        if !id.is_dynamic() {
            return invalid(
                "dynamic_embedder_freeze.id",
                "freeze guard handles only apply to EDynamic ids",
            );
        }
        let slug = id.slug().into_owned();
        Ok(Self {
            instrument_id: format!("dynamic_embedder:{slug}"),
            tensor_ids: vec![TensorId(format!("{slug}:frozen_forward_parameters"))],
        })
    }
}

impl InstrumentGradHandle for DynamicEmbedderGradHandle {
    fn instrument_id(&self) -> &str {
        &self.instrument_id
    }

    fn tensor_ids(&self) -> &[TensorId] {
        &self.tensor_ids
    }
}

pub fn frozen_adapter_for_dynamic_embedders(
    provenance: TargetProvenance,
    ids: &[RuntimeEmbedderId],
) -> Result<FrozenTargetAdapter, MejepaInferError> {
    let mut handles = Vec::new();
    for id in ids {
        handles.push(Box::new(DynamicEmbedderGradHandle::new(id)?)
            as Box<dyn InstrumentGradHandle + Send + Sync>);
    }
    Ok(FrozenTargetAdapter::with_grad_handles(provenance, handles))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DynamicEmbedderPromotionReplayRecord {
    pub schema_version: u32,
    pub promotion_id: String,
    pub comparison: EmbedderCandidateHoldoutComparison,
    pub gate: EmbedderFalsificationGate,
    pub expected_decision_sha256: String,
}

impl DynamicEmbedderPromotionReplayRecord {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != DYNAMIC_EMBEDDER_FREEZE_AUDIT_SCHEMA_VERSION {
            return invalid(
                "dynamic_embedder_replay.schema_version",
                format!(
                    "expected {DYNAMIC_EMBEDDER_FREEZE_AUDIT_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        validate_single_line(
            "dynamic_embedder_replay.promotion_id",
            &self.promotion_id,
            256,
        )?;
        self.comparison.validate()?;
        self.gate.validate()?;
        validate_sha256_hex(
            "dynamic_embedder_replay.expected_decision_sha256",
            &self.expected_decision_sha256,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DynamicEmbedderPromotionReplayReport {
    pub promotion_id: String,
    pub reproduced: bool,
    pub expected_decision_sha256: String,
    pub replayed_decision_sha256: String,
    pub decision: EmbedderFalsificationDecision,
}

pub fn replay_dynamic_embedder_promotion(
    record: &DynamicEmbedderPromotionReplayRecord,
) -> Result<DynamicEmbedderPromotionReplayReport, MejepaInferError> {
    record.validate()?;
    let decision = evaluate_embedder_candidate_falsification(&record.comparison, record.gate)?;
    let replayed_decision_sha256 = dynamic_embedder_replay_decision_sha256(&decision)?;
    Ok(DynamicEmbedderPromotionReplayReport {
        promotion_id: record.promotion_id.clone(),
        reproduced: replayed_decision_sha256 == record.expected_decision_sha256,
        expected_decision_sha256: record.expected_decision_sha256.clone(),
        replayed_decision_sha256,
        decision,
    })
}

pub fn dynamic_embedder_replay_decision_sha256(
    decision: &EmbedderFalsificationDecision,
) -> Result<String, MejepaInferError> {
    sha256_json(decision)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DynamicEmbedderFreezeAuditRecord {
    pub schema_version: u32,
    pub id: RuntimeEmbedderId,
    pub registry_version: u64,
    pub promotion_id: String,
    pub provenance_record_sha256: String,
    pub witness_chain_path: String,
    pub witness_offset: u64,
    pub witness_chain_sha256: String,
    pub replay_record_sha256: String,
    pub replay_decision_sha256: String,
    pub gradient_guard_clean_passed: bool,
    pub gradient_guard_leak_code: String,
    pub created_at_unix_ms: i64,
    pub source_of_truth_cf: String,
}

impl DynamicEmbedderFreezeAuditRecord {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != DYNAMIC_EMBEDDER_FREEZE_AUDIT_SCHEMA_VERSION {
            return invalid(
                "dynamic_embedder_freeze.schema_version",
                format!(
                    "expected {DYNAMIC_EMBEDDER_FREEZE_AUDIT_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        self.id.validate().map_err(embed_error)?;
        if !self.id.is_dynamic() {
            return invalid(
                "dynamic_embedder_freeze.id",
                "freeze audit rows must reference EDynamic ids",
            );
        }
        if self.registry_version == 0 {
            return invalid(
                "dynamic_embedder_freeze.registry_version",
                "registry_version must be non-zero",
            );
        }
        validate_single_line(
            "dynamic_embedder_freeze.promotion_id",
            &self.promotion_id,
            256,
        )?;
        validate_sha256_hex(
            "dynamic_embedder_freeze.provenance_record_sha256",
            &self.provenance_record_sha256,
        )?;
        validate_single_line(
            "dynamic_embedder_freeze.witness_chain_path",
            &self.witness_chain_path,
            4096,
        )?;
        validate_sha256_hex(
            "dynamic_embedder_freeze.witness_chain_sha256",
            &self.witness_chain_sha256,
        )?;
        validate_sha256_hex(
            "dynamic_embedder_freeze.replay_record_sha256",
            &self.replay_record_sha256,
        )?;
        validate_sha256_hex(
            "dynamic_embedder_freeze.replay_decision_sha256",
            &self.replay_decision_sha256,
        )?;
        validate_single_line(
            "dynamic_embedder_freeze.gradient_guard_leak_code",
            &self.gradient_guard_leak_code,
            128,
        )?;
        if self.created_at_unix_ms <= 0 {
            return invalid(
                "dynamic_embedder_freeze.created_at_unix_ms",
                "created_at_unix_ms must be positive",
            );
        }
        if self.source_of_truth_cf != context_graph_mejepa_cf::CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE
        {
            return invalid(
                "dynamic_embedder_freeze.source_of_truth_cf",
                format!(
                    "expected {}",
                    context_graph_mejepa_cf::CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE
                ),
            );
        }
        Ok(())
    }
}

pub fn write_dynamic_embedder_freeze_audit_sync_readback(
    db: &DB,
    record: &DynamicEmbedderFreezeAuditRecord,
) -> Result<Vec<u8>, MejepaInferError> {
    record.validate()?;
    let key = dynamic_embedder_freeze_audit_key(
        &record.id,
        record.registry_version,
        &record.promotion_id,
    )?;
    let cf = cf(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE,
    )?;
    let value = bincode::serialize(record)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, &key, &value, &opts)?;
    db.flush_cf(cf)?;
    let readback = db
        .get_cf(cf, &key)?
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "dynamic_embedder_freeze.readback".to_string(),
            detail: "missing freeze audit row after write".to_string(),
        })?;
    if readback != value {
        return invalid(
            "dynamic_embedder_freeze.readback",
            "readback bytes differ from encoded record",
        );
    }
    let decoded: DynamicEmbedderFreezeAuditRecord = bincode::deserialize(&readback)?;
    decoded.validate()?;
    if decoded != *record {
        return invalid(
            "dynamic_embedder_freeze.readback",
            "decoded readback differs from input",
        );
    }
    Ok(key)
}

pub fn read_dynamic_embedder_freeze_audit(
    db: &DB,
    id: &RuntimeEmbedderId,
    registry_version: u64,
    promotion_id: &str,
) -> Result<Option<DynamicEmbedderFreezeAuditRecord>, MejepaInferError> {
    let key = dynamic_embedder_freeze_audit_key(id, registry_version, promotion_id)?;
    let cf = cf(
        db,
        context_graph_mejepa_cf::CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE,
    )?;
    let Some(bytes) = db.get_cf(cf, &key)? else {
        return Ok(None);
    };
    let record: DynamicEmbedderFreezeAuditRecord = bincode::deserialize(&bytes)?;
    record.validate()?;
    Ok(Some(record))
}

pub fn dynamic_embedder_freeze_audit_key(
    id: &RuntimeEmbedderId,
    registry_version: u64,
    promotion_id: &str,
) -> Result<Vec<u8>, MejepaInferError> {
    id.validate().map_err(embed_error)?;
    if registry_version == 0 {
        return invalid(
            "dynamic_embedder_freeze.registry_version",
            "registry_version must be non-zero",
        );
    }
    validate_single_line("dynamic_embedder_freeze.promotion_id", promotion_id, 256)?;
    Ok(format!(
        "freeze-audit:{}:{registry_version:020}:{promotion_id}",
        id.slug()
    )
    .into_bytes())
}

pub fn dynamic_embedder_record_sha256<T: Serialize>(
    record: &T,
) -> Result<String, MejepaInferError> {
    sha256_json(record)
}

fn validate_single_line(field: &str, value: &str, max_len: usize) -> Result<(), MejepaInferError> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.len() > max_len
        || value.bytes().any(|byte| byte == 0 || byte == b'\n')
    {
        return invalid(
            field,
            format!("must be non-empty trimmed single-line text up to {max_len} bytes"),
        );
    }
    Ok(())
}

fn validate_sha256_hex(field: &str, value: &str) -> Result<(), MejepaInferError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return invalid(field, "value must be 64 lowercase hex characters");
    }
    Ok(())
}

fn embed_error(err: context_graph_mejepa_embedders::EmbedError) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: "dynamic_embedder_freeze".to_string(),
        detail: err.to_string(),
    }
}

fn invalid<T>(field: &str, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail: detail.into(),
    })
}

fn sha256_json<T: Serialize>(record: &T) -> Result<String, MejepaInferError> {
    let bytes = serde_json::to_vec(record)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_models::TargetProvenance;
    use crate::grad_hook::{run_gradient_hook, GradStore};
    use std::collections::BTreeMap;

    #[test]
    fn dynamic_grad_handle_trips_existing_frozen_target_guard() {
        let id = RuntimeEmbedderId::dynamic(1, "guarded").unwrap();
        let adapter = frozen_adapter_for_dynamic_embedders(provenance(), &[id]).unwrap();
        let mut grads = GradStore::default();
        grads
            .insert_norm(
                TensorId("edynamic:1:guarded:frozen_forward_parameters".to_string()),
                1.0,
            )
            .unwrap();
        let err = run_gradient_hook(&adapter, &grads).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_PRED_FROZEN_TARGET_GRAD");
    }

    fn provenance() -> TargetProvenance {
        TargetProvenance::new(
            "dynamic-freeze-test",
            BTreeMap::from([("edynamic:1:guarded".to_string(), "frozen".to_string())]),
            0,
            None,
        )
    }
}
