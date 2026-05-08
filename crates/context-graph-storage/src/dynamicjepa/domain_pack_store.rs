use context_graph_core::dynamicjepa::{
    AdapterSpec, DomainPack, DomainPackId, DynamicJepaError, DynamicJepaResult, InstrumentSpec,
};
use rocksdb::{WriteBatch, DB};
use uuid::Uuid;

use crate::dynamicjepa::audit::{AuditStatus, DjAuditRecord};
use crate::dynamicjepa::audit_witness::write_batch_with_audit_witnesses;
use crate::dynamicjepa::column_families::{
    CF_DJ_ADAPTER_REGISTRY, CF_DJ_AUDIT_LOG, CF_DJ_AUDIT_WITNESS_CHAIN, CF_DJ_DOMAIN_PACKS,
    CF_DJ_DOMAIN_PACK_BY_NAME_VERSION, CF_DJ_INSTRUMENT_REGISTRY,
};
use crate::dynamicjepa::common::{cf, count_cf, get_record, list_records};
use crate::dynamicjepa::encode::{
    decode_plain, encode_plain, encode_record, DYNAMIC_JEPA_STORAGE_VALUE_VERSION,
};
use crate::dynamicjepa::keys::{
    adapter_registry_key, domain_pack_storage_uuid, instrument_registry_key, name_version_key,
    uuid_key,
};

pub fn put_domain_pack(db: &DB, record: &DomainPack) -> DynamicJepaResult<Uuid> {
    let domain_uuid = domain_pack_storage_uuid(&record.id, &record.version);
    let encoded = encode_record(record)?;
    let mut batch = WriteBatch::default();

    batch.put_cf(cf(db, CF_DJ_DOMAIN_PACKS)?, uuid_key(domain_uuid), encoded);
    batch.put_cf(
        cf(db, CF_DJ_DOMAIN_PACK_BY_NAME_VERSION)?,
        name_version_key(&record.id, &record.version),
        uuid_key(domain_uuid),
    );

    for spec in &record.instrument_specs {
        batch.put_cf(
            cf(db, CF_DJ_INSTRUMENT_REGISTRY)?,
            instrument_registry_key(domain_uuid, &spec.instrument_id),
            encode_plain(spec, DYNAMIC_JEPA_STORAGE_VALUE_VERSION, "InstrumentSpec")?,
        );
    }
    for spec in &record.adapter_specs {
        batch.put_cf(
            cf(db, CF_DJ_ADAPTER_REGISTRY)?,
            adapter_registry_key(domain_uuid, &spec.adapter_id),
            encode_plain(spec, DYNAMIC_JEPA_STORAGE_VALUE_VERSION, "AdapterSpec")?,
        );
    }

    let audit_id = Uuid::new_v5(
        &crate::dynamicjepa::keys::DYNAMIC_JEPA_DOMAIN_PACK_NAMESPACE,
        format!("register_domain_pack:{}:{}", record.id, record.version).as_bytes(),
    );
    let audit = DjAuditRecord {
        audit_id,
        timestamp_unix_nanos: (record.header.created_at_unix_ms as u64) * 1_000_000,
        operation: "register_domain_pack".to_string(),
        actor: "storage".to_string(),
        input_ids: vec![format!("{}:{}", record.id, record.version)],
        output_ids: vec![domain_uuid.to_string()],
        cfs_touched: vec![
            CF_DJ_DOMAIN_PACKS.to_string(),
            CF_DJ_DOMAIN_PACK_BY_NAME_VERSION.to_string(),
            CF_DJ_INSTRUMENT_REGISTRY.to_string(),
            CF_DJ_ADAPTER_REGISTRY.to_string(),
            CF_DJ_AUDIT_LOG.to_string(),
            CF_DJ_AUDIT_WITNESS_CHAIN.to_string(),
        ],
        content_hashes: vec![record.header.content_hash],
        status: AuditStatus::Ok,
        verification_run_id: None,
        signal_yield: 0,
    };
    audit.validate()?;
    write_batch_with_audit_witnesses(db, batch, &[&audit], "put_domain_pack")?;
    Ok(domain_uuid)
}

pub fn get_domain_pack_by_storage_id(
    db: &DB,
    domain_uuid: Uuid,
) -> DynamicJepaResult<Option<DomainPack>> {
    get_record(db, CF_DJ_DOMAIN_PACKS, uuid_key(domain_uuid))
}

pub fn get_domain_pack(
    db: &DB,
    id: &DomainPackId,
    version: &str,
) -> DynamicJepaResult<Option<DomainPack>> {
    let index_value = db
        .get_cf(
            cf(db, CF_DJ_DOMAIN_PACK_BY_NAME_VERSION)?,
            name_version_key(id, version),
        )
        .map_err(|err| DynamicJepaError::Storage {
            operation: "get_domain_pack.index".to_string(),
            cf: CF_DJ_DOMAIN_PACK_BY_NAME_VERSION.to_string(),
            message: err.to_string(),
            remediation: "inspect the name/version index CF and rerun registration".to_string(),
        })?;
    let Some(bytes) = index_value else {
        return Ok(None);
    };
    if bytes.len() != 16 {
        return Err(DynamicJepaError::StorageInvariantViolation {
            message: format!(
                "domain_pack_by_nv value must be 16-byte UUID, got {} bytes",
                bytes.len()
            ),
        });
    }
    let mut id_bytes = [0u8; 16];
    id_bytes.copy_from_slice(&bytes);
    get_domain_pack_by_storage_id(db, Uuid::from_bytes(id_bytes))
}

pub fn list_domain_packs(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<DomainPack>> {
    list_records(db, CF_DJ_DOMAIN_PACKS, limit, offset)
}

pub fn count_domain_packs(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_DOMAIN_PACKS)
}

pub fn get_instrument_spec(
    db: &DB,
    domain_pack_uuid: Uuid,
    instrument_id: &context_graph_core::dynamicjepa::InstrumentId,
) -> DynamicJepaResult<Option<InstrumentSpec>> {
    match db.get_cf(
        cf(db, CF_DJ_INSTRUMENT_REGISTRY)?,
        instrument_registry_key(domain_pack_uuid, instrument_id),
    ) {
        Ok(Some(bytes)) => {
            decode_plain(&bytes, DYNAMIC_JEPA_STORAGE_VALUE_VERSION, "InstrumentSpec").map(Some)
        }
        Ok(None) => Ok(None),
        Err(err) => Err(DynamicJepaError::Storage {
            operation: "get_instrument_spec".to_string(),
            cf: CF_DJ_INSTRUMENT_REGISTRY.to_string(),
            message: err.to_string(),
            remediation: "inspect the instrument registry CF".to_string(),
        }),
    }
}

pub fn get_adapter_spec(
    db: &DB,
    domain_pack_uuid: Uuid,
    adapter_id: &context_graph_core::dynamicjepa::AdapterId,
) -> DynamicJepaResult<Option<AdapterSpec>> {
    match db.get_cf(
        cf(db, CF_DJ_ADAPTER_REGISTRY)?,
        adapter_registry_key(domain_pack_uuid, adapter_id),
    ) {
        Ok(Some(bytes)) => {
            decode_plain(&bytes, DYNAMIC_JEPA_STORAGE_VALUE_VERSION, "AdapterSpec").map(Some)
        }
        Ok(None) => Ok(None),
        Err(err) => Err(DynamicJepaError::Storage {
            operation: "get_adapter_spec".to_string(),
            cf: CF_DJ_ADAPTER_REGISTRY.to_string(),
            message: err.to_string(),
            remediation: "inspect the adapter registry CF".to_string(),
        }),
    }
}

pub fn count_domain_pack_by_name_version(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_DOMAIN_PACK_BY_NAME_VERSION)
}

pub fn count_instrument_registry(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_INSTRUMENT_REGISTRY)
}

pub fn count_adapter_registry(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_ADAPTER_REGISTRY)
}
