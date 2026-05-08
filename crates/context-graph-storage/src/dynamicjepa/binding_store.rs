use context_graph_core::dynamicjepa::{
    BindingId, BindingRecord, BindingRef, DynamicJepaError, DynamicJepaRecord, DynamicJepaResult,
    Validate,
};
use rocksdb::{IteratorMode, WriteBatch, DB};

use crate::dynamicjepa::audit::DjAuditRecord;
use crate::dynamicjepa::audit_witness::write_batch_with_audit_witnesses;
use crate::dynamicjepa::column_families::{CF_DJ_BINDINGS, CF_DJ_BINDINGS_BY_ENTITY};
use crate::dynamicjepa::common::{
    cf, count_cf, get_record, list_records, storage_error, write_batch,
};
use crate::dynamicjepa::encode::encode_record;
use crate::dynamicjepa::keys::binding_entity_key;

pub fn put_binding(db: &DB, record: &BindingRecord) -> DynamicJepaResult<()> {
    let encoded = encode_record(record)?;
    let mut batch = WriteBatch::default();
    batch.put_cf(
        cf(db, CF_DJ_BINDINGS)?,
        record.binding_id.into_bytes(),
        encoded,
    );
    batch.put_cf(
        cf(db, CF_DJ_BINDINGS_BY_ENTITY)?,
        binding_entity_key(&record.left_ref, record.binding_id),
        [],
    );
    write_batch(db, batch, "put_binding")
}

pub fn put_binding_with_audit_batch(
    db: &DB,
    record: &BindingRecord,
    audit: &DjAuditRecord,
) -> DynamicJepaResult<()> {
    record.validate_record()?;
    audit.validate()?;
    let encoded = encode_record(record)?;
    let mut batch = WriteBatch::default();
    batch.put_cf(
        cf(db, CF_DJ_BINDINGS)?,
        record.binding_id.into_bytes(),
        encoded,
    );
    batch.put_cf(
        cf(db, CF_DJ_BINDINGS_BY_ENTITY)?,
        binding_entity_key(&record.left_ref, record.binding_id),
        [],
    );
    write_batch_with_audit_witnesses(db, batch, &[audit], "put_binding_with_audit_batch")
}

pub fn get_binding(db: &DB, id: BindingId) -> DynamicJepaResult<Option<BindingRecord>> {
    get_record(db, CF_DJ_BINDINGS, id.into_bytes())
}

pub fn list_bindings(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<BindingRecord>> {
    list_records(db, CF_DJ_BINDINGS, limit, offset)
}

pub fn count_bindings(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_BINDINGS)
}

pub fn count_binding_entity_index(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_BINDINGS_BY_ENTITY)
}

pub fn binding_entity_index_key(
    entity_ref: &BindingRef,
    binding_id: BindingId,
) -> DynamicJepaResult<[u8; 32]> {
    entity_ref.validate()?;
    binding_id.validate()?;
    Ok(binding_entity_key(entity_ref, binding_id))
}

pub fn binding_entity_index_exists(
    db: &DB,
    entity_ref: &BindingRef,
    binding_id: BindingId,
) -> DynamicJepaResult<bool> {
    let key = binding_entity_index_key(entity_ref, binding_id)?;
    db.get_cf(cf(db, CF_DJ_BINDINGS_BY_ENTITY)?, key)
        .map(|value| value.is_some())
        .map_err(|err| DynamicJepaError::Storage {
            operation: "binding_entity_index_exists".to_string(),
            cf: CF_DJ_BINDINGS_BY_ENTITY.to_string(),
            message: err.to_string(),
            remediation: "inspect the binding entity index CF and key encoder".to_string(),
        })
}

pub fn list_binding_entity_index_keys(db: &DB) -> DynamicJepaResult<Vec<Vec<u8>>> {
    let iter = db.iterator_cf(cf(db, CF_DJ_BINDINGS_BY_ENTITY)?, IteratorMode::Start);
    let mut keys = Vec::new();
    for item in iter {
        let (key, _value) = item.map_err(|err| {
            storage_error(
                "list_binding_entity_index_keys",
                CF_DJ_BINDINGS_BY_ENTITY,
                err.to_string(),
                "inspect RocksDB LOG files and retry from a fresh DB",
            )
        })?;
        keys.push(key.to_vec());
    }
    Ok(keys)
}
