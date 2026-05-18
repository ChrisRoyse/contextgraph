use context_graph_mejepa_cf::{
    CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE, CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY,
};
pub use context_graph_mejepa_embedders::{
    dda_signal_count_for_chunks, upper_triangle_len, DynamicEmbedderKind,
    DynamicEmbedderProvenanceRecord, DynamicEmbedderRegistryRecord, RuntimeEmbedderId,
    RuntimeRoutingResult, RuntimeRoutingTable,
};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{de::DeserializeOwned, Serialize};

use crate::calibration::cf;
use crate::error::MejepaInferError;

pub fn write_dynamic_embedder_registry_sync_readback(
    db: &DB,
    record: &DynamicEmbedderRegistryRecord,
) -> Result<(), MejepaInferError> {
    record.validate().map_err(embed_error)?;
    write_value_sync_readback(
        db,
        CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY,
        dynamic_embedder_key(&record.id)?.as_bytes(),
        record,
    )
}

pub fn read_dynamic_embedder_registry(
    db: &DB,
    id: &RuntimeEmbedderId,
) -> Result<Option<DynamicEmbedderRegistryRecord>, MejepaInferError> {
    id.validate().map_err(embed_error)?;
    read_value(
        db,
        CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY,
        dynamic_embedder_key(id)?.as_bytes(),
    )
}

pub fn read_dynamic_embedder_registry_snapshot(
    db: &DB,
) -> Result<RuntimeRoutingTable, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY)?;
    let mut records = Vec::new();
    let mut registry_version = 1u64;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let record: DynamicEmbedderRegistryRecord = bincode::deserialize(&value)?;
        record.validate().map_err(embed_error)?;
        registry_version = registry_version.max(record.registry_version);
        records.push(record);
    }
    RuntimeRoutingTable::new(registry_version, records).map_err(embed_error)
}

pub fn write_dynamic_embedder_provenance_sync_readback(
    db: &DB,
    record: &DynamicEmbedderProvenanceRecord,
) -> Result<(), MejepaInferError> {
    record.validate().map_err(embed_error)?;
    write_value_sync_readback(
        db,
        CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE,
        provenance_key(record)?.as_bytes(),
        record,
    )
}

pub fn read_dynamic_embedder_provenance(
    db: &DB,
    id: &RuntimeEmbedderId,
    registry_version: u64,
) -> Result<Option<DynamicEmbedderProvenanceRecord>, MejepaInferError> {
    id.validate().map_err(embed_error)?;
    if registry_version == 0 {
        return Err(MejepaInferError::InvalidInput {
            field: "dynamic_embedder_provenance.registry_version".to_string(),
            detail: "registry_version must be non-zero".to_string(),
        });
    }
    read_value(
        db,
        CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE,
        format!("{}:{registry_version:020}", dynamic_embedder_key(id)?).as_bytes(),
    )
}

pub fn count_active_dynamic_embedders(db: &DB) -> Result<usize, MejepaInferError> {
    Ok(read_dynamic_embedder_registry_snapshot(db)?.active_dynamic_count())
}

fn dynamic_embedder_key(id: &RuntimeEmbedderId) -> Result<String, MejepaInferError> {
    id.validate().map_err(embed_error)?;
    Ok(id.slug().into_owned())
}

fn provenance_key(record: &DynamicEmbedderProvenanceRecord) -> Result<String, MejepaInferError> {
    if record.registry_version == 0 {
        return Err(MejepaInferError::InvalidInput {
            field: "dynamic_embedder_provenance.registry_version".to_string(),
            detail: "registry_version must be non-zero".to_string(),
        });
    }
    Ok(format!(
        "{}:{:020}",
        dynamic_embedder_key(&record.id)?,
        record.registry_version
    ))
}

fn write_value_sync_readback<T>(
    db: &DB,
    cf_name: &str,
    key: &[u8],
    value: &T,
) -> Result<(), MejepaInferError>
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let cf = cf(db, cf_name)?;
    let bytes = bincode::serialize(value)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, &bytes, &opts)?;
    db.flush_cf(cf)?;
    let readback = db
        .get_cf(cf, key)?
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: cf_name.to_string(),
            detail: "sync write readback returned no row".to_string(),
        })?;
    if readback != bytes {
        return Err(MejepaInferError::InvalidInput {
            field: cf_name.to_string(),
            detail: "sync write readback bytes differ from encoded input".to_string(),
        });
    }
    let decoded: T = bincode::deserialize(&readback)?;
    if decoded != *value {
        return Err(MejepaInferError::InvalidInput {
            field: cf_name.to_string(),
            detail: format!("sync write readback decoded value differs: {decoded:?}"),
        });
    }
    Ok(())
}

fn read_value<T>(db: &DB, cf_name: &str, key: &[u8]) -> Result<Option<T>, MejepaInferError>
where
    T: DeserializeOwned,
{
    let cf = cf(db, cf_name)?;
    db.get_cf(cf, key)?
        .map(|bytes| bincode::deserialize(&bytes).map_err(Into::into))
        .transpose()
}

fn embed_error(err: context_graph_mejepa_embedders::EmbedError) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: "dynamic_embedder".to_string(),
        detail: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_infer_rocksdb;

    #[test]
    fn persists_dynamic_registry_and_provenance() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path()).unwrap();
        let record = registry_record();
        let provenance = provenance_record(record.id.clone());

        write_dynamic_embedder_registry_sync_readback(db.as_ref(), &record).unwrap();
        write_dynamic_embedder_provenance_sync_readback(db.as_ref(), &provenance).unwrap();

        let loaded = read_dynamic_embedder_registry(db.as_ref(), &record.id)
            .unwrap()
            .unwrap();
        let loaded_provenance =
            read_dynamic_embedder_provenance(db.as_ref(), &record.id, record.registry_version)
                .unwrap()
                .unwrap();

        assert_eq!(loaded, record);
        assert_eq!(loaded_provenance, provenance);
    }

    fn registry_record() -> DynamicEmbedderRegistryRecord {
        DynamicEmbedderRegistryRecord {
            id: RuntimeEmbedderId::dynamic(1, "corpus_transe_v1").unwrap(),
            registry_version: 2,
            kind: DynamicEmbedderKind::Algorithmic,
            dimension: 128,
            route_languages: vec!["python".to_string()],
            route_entity_types: vec!["Function".to_string()],
            forward_artifact_path:
                "/mnt/d/contextgraph/models/dynamic/edynamic_1_corpus_transe_v1/forward.so"
                    .to_string(),
            forward_artifact_sha256:
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            required_vram_bytes: 64 * 1024 * 1024,
            active: true,
            promoted_at_unix_ms: 1_779_100_000_000,
        }
    }

    fn provenance_record(id: RuntimeEmbedderId) -> DynamicEmbedderProvenanceRecord {
        DynamicEmbedderProvenanceRecord {
            id,
            registry_version: 2,
            residual_signal_ref: "unknown_cluster:cluster-a".to_string(),
            architecture_generator: "deterministic-fixture".to_string(),
            training_cert_chain_hash:
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            heldout_global_delta: 0.006,
            heldout_min_cell_delta: 0.0,
            operator_approval_id: Some("approval-1".to_string()),
            forward_pass_artifact_sha256:
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            created_at_unix_ms: 1_779_100_000_001,
        }
    }
}
