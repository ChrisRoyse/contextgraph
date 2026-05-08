use std::collections::BTreeMap;

use context_graph_core::dynamicjepa::{
    AdapterRunRecord, AdapterSpec, BindingRecord, ConstellationCentroid, DatasetShardRecord,
    DomainPack, DynamicJepaError, DynamicJepaRecord, DynamicJepaResult, GuardDecisionRecord,
    InstrumentReading, InstrumentSpec, LatentPanel, ModelArtifactRecord, NormalizedAction,
    NormalizedOutcome, NormalizedState, PairwiseReading, PlanTraceRecord, PredictionRecord,
    RawDomainEvent, SkillPolicyRecord, StateTransition, SurpriseEventRecord, ThresholdCalibration,
    TrainingRunRecord, TrajectoryRecord, VerificationRunRecord,
};
use rocksdb::{IteratorMode, DB};
use serde::de::DeserializeOwned;
use serde_json::json;
use uuid::Uuid;

use crate::dynamicjepa::audit::{DjAuditRecord, DJ_AUDIT_RECORD_VERSION};
use crate::dynamicjepa::audit_witness::decode_audit_witness_value;
use crate::dynamicjepa::column_families::{
    CF_DJ_ACTIONS, CF_DJ_ADAPTER_REGISTRY, CF_DJ_ADAPTER_RUNS, CF_DJ_AUDIT_LOG,
    CF_DJ_AUDIT_WITNESS_CHAIN, CF_DJ_BINDINGS, CF_DJ_BINDINGS_BY_ENTITY, CF_DJ_CONSTELLATIONS,
    CF_DJ_DATASET_SHARDS, CF_DJ_DOMAIN_PACKS, CF_DJ_DOMAIN_PACK_BY_NAME_VERSION,
    CF_DJ_GUARD_DECISIONS, CF_DJ_INSTRUMENT_READINGS, CF_DJ_INSTRUMENT_REGISTRY,
    CF_DJ_LATENT_PANELS, CF_DJ_MODEL_ARTIFACTS, CF_DJ_NORMALIZED_STATES, CF_DJ_OUTCOMES,
    CF_DJ_PAIRWISE_READINGS, CF_DJ_PLAN_TRACES, CF_DJ_PREDICTIONS, CF_DJ_RAW_EVENTS,
    CF_DJ_SKILL_POLICIES, CF_DJ_SURPRISE_EVENTS, CF_DJ_THRESHOLD_CALIBRATIONS, CF_DJ_TRAINING_RUNS,
    CF_DJ_TRAJECTORIES, CF_DJ_TRANSITIONS, CF_DJ_VERIFICATION_RUNS, DJ_CF_NAMES,
};
use crate::dynamicjepa::common::{cf, count_cf, storage_error, to_json};
use crate::dynamicjepa::encode::{decode_plain, decode_record, DYNAMIC_JEPA_STORAGE_VALUE_VERSION};

pub fn snapshot_dj_counts(db: &DB) -> DynamicJepaResult<BTreeMap<String, u64>> {
    let mut counts = BTreeMap::new();
    for cf_name in DJ_CF_NAMES {
        counts.insert((*cf_name).to_string(), count_cf(db, cf_name)?);
    }
    Ok(counts)
}

pub fn inspect_cf(
    db: &DB,
    cf_name: &'static str,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<serde_json::Value>> {
    let iter = db.iterator_cf(cf(db, cf_name)?, IteratorMode::Start);
    let mut rows = Vec::new();
    for (idx, item) in iter.enumerate() {
        let (key, value) = item.map_err(|err| {
            storage_error(
                "inspect_cf",
                cf_name,
                err.to_string(),
                "inspect RocksDB LOG files and retry from a fresh DB",
            )
        })?;
        if idx < offset {
            continue;
        }
        if rows.len() >= limit {
            break;
        }
        rows.push(json!({
            "cf": cf_name,
            "key_hex": hex(&key),
            "value_len": value.len(),
            "decoded": decode_row(cf_name, &key, &value)?,
        }));
    }
    Ok(rows)
}

pub fn inspect_cf_key(
    db: &DB,
    cf_name: &'static str,
    key: &[u8],
) -> DynamicJepaResult<Option<serde_json::Value>> {
    match db.get_cf(cf(db, cf_name)?, key) {
        Ok(Some(value)) => Ok(Some(json!({
            "cf": cf_name,
            "key_hex": hex(key),
            "value_len": value.len(),
            "decoded": decode_row(cf_name, key, &value)?,
        }))),
        Ok(None) => Ok(None),
        Err(err) => Err(storage_error(
            "inspect_cf_key",
            cf_name,
            err.to_string(),
            "inspect RocksDB LOG files and verify the key encoder before retrying",
        )),
    }
}

fn decode_core_row<R>(bytes: &[u8], type_name: &'static str) -> DynamicJepaResult<serde_json::Value>
where
    R: DynamicJepaRecord + DeserializeOwned + serde::Serialize,
{
    let record: R = decode_record(bytes)?;
    to_json(&record, type_name)
}

fn decode_row(
    cf_name: &'static str,
    _key: &[u8],
    value: &[u8],
) -> DynamicJepaResult<serde_json::Value> {
    match cf_name {
        CF_DJ_DOMAIN_PACKS => decode_core_row::<DomainPack>(value, "DomainPack"),
        CF_DJ_DOMAIN_PACK_BY_NAME_VERSION => {
            if value.len() != 16 {
                return Err(DynamicJepaError::StorageInvariantViolation {
                    message: format!(
                        "{CF_DJ_DOMAIN_PACK_BY_NAME_VERSION} value must be 16-byte UUID, got {}",
                        value.len()
                    ),
                });
            }
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(value);
            Ok(json!({"domain_pack_uuid": Uuid::from_bytes(bytes).to_string()}))
        }
        CF_DJ_INSTRUMENT_REGISTRY => {
            let spec: InstrumentSpec =
                decode_plain(value, DYNAMIC_JEPA_STORAGE_VALUE_VERSION, "InstrumentSpec")?;
            to_json(&spec, "InstrumentSpec")
        }
        CF_DJ_ADAPTER_REGISTRY => {
            let spec: AdapterSpec =
                decode_plain(value, DYNAMIC_JEPA_STORAGE_VALUE_VERSION, "AdapterSpec")?;
            to_json(&spec, "AdapterSpec")
        }
        CF_DJ_RAW_EVENTS => decode_core_row::<RawDomainEvent>(value, "RawDomainEvent"),
        CF_DJ_NORMALIZED_STATES => decode_core_row::<NormalizedState>(value, "NormalizedState"),
        CF_DJ_ACTIONS => decode_core_row::<NormalizedAction>(value, "NormalizedAction"),
        CF_DJ_OUTCOMES => decode_core_row::<NormalizedOutcome>(value, "NormalizedOutcome"),
        CF_DJ_TRANSITIONS => decode_core_row::<StateTransition>(value, "StateTransition"),
        CF_DJ_ADAPTER_RUNS => decode_core_row::<AdapterRunRecord>(value, "AdapterRunRecord"),
        CF_DJ_INSTRUMENT_READINGS => {
            decode_core_row::<InstrumentReading>(value, "InstrumentReading")
        }
        CF_DJ_LATENT_PANELS => decode_core_row::<LatentPanel>(value, "LatentPanel"),
        CF_DJ_PAIRWISE_READINGS => decode_core_row::<PairwiseReading>(value, "PairwiseReading"),
        CF_DJ_CONSTELLATIONS => {
            decode_core_row::<ConstellationCentroid>(value, "ConstellationCentroid")
        }
        CF_DJ_THRESHOLD_CALIBRATIONS => {
            decode_core_row::<ThresholdCalibration>(value, "ThresholdCalibration")
        }
        CF_DJ_BINDINGS => decode_core_row::<BindingRecord>(value, "BindingRecord"),
        CF_DJ_BINDINGS_BY_ENTITY => Ok(json!({"index": "binding_by_entity"})),
        CF_DJ_TRAJECTORIES => decode_core_row::<TrajectoryRecord>(value, "TrajectoryRecord"),
        CF_DJ_DATASET_SHARDS => decode_core_row::<DatasetShardRecord>(value, "DatasetShardRecord"),
        CF_DJ_TRAINING_RUNS => decode_core_row::<TrainingRunRecord>(value, "TrainingRunRecord"),
        CF_DJ_MODEL_ARTIFACTS => {
            decode_core_row::<ModelArtifactRecord>(value, "ModelArtifactRecord")
        }
        CF_DJ_PREDICTIONS => decode_core_row::<PredictionRecord>(value, "PredictionRecord"),
        CF_DJ_SKILL_POLICIES => decode_core_row::<SkillPolicyRecord>(value, "SkillPolicyRecord"),
        CF_DJ_PLAN_TRACES => decode_core_row::<PlanTraceRecord>(value, "PlanTraceRecord"),
        CF_DJ_GUARD_DECISIONS => {
            decode_core_row::<GuardDecisionRecord>(value, "GuardDecisionRecord")
        }
        CF_DJ_SURPRISE_EVENTS => {
            decode_core_row::<SurpriseEventRecord>(value, "SurpriseEventRecord")
        }
        CF_DJ_VERIFICATION_RUNS => {
            decode_core_row::<VerificationRunRecord>(value, "VerificationRunRecord")
        }
        CF_DJ_AUDIT_LOG => {
            let audit: DjAuditRecord =
                decode_plain(value, DJ_AUDIT_RECORD_VERSION, "DjAuditRecord")?;
            to_json(&audit, "DjAuditRecord")
        }
        CF_DJ_AUDIT_WITNESS_CHAIN => decode_audit_witness_value(value),
        other => Err(DynamicJepaError::StorageInvariantViolation {
            message: format!("unknown DynamicJEPA column family {other:?}"),
        }),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
