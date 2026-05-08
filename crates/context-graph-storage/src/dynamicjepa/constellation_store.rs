use context_graph_core::dynamicjepa::{
    ConstellationCentroid, DynamicJepaRecord, DynamicJepaResult,
};
use rocksdb::{WriteBatch, DB};
use uuid::Uuid;

use crate::dynamicjepa::audit::DjAuditRecord;
use crate::dynamicjepa::audit_witness::write_batch_with_audit_witnesses;
use crate::dynamicjepa::column_families::CF_DJ_CONSTELLATIONS;
use crate::dynamicjepa::common::{cf, count_cf, get_record, list_records, put_record};
use crate::dynamicjepa::encode::encode_record;
use crate::dynamicjepa::keys::constellation_key;

pub fn put_constellation(
    db: &DB,
    domain_uuid: Uuid,
    subject_id: &str,
    modality_id: u32,
    record: &ConstellationCentroid,
) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_CONSTELLATIONS,
        constellation_key(&domain_uuid, subject_id, modality_id),
        record,
    )
}

pub fn get_constellation(
    db: &DB,
    domain_uuid: Uuid,
    subject_id: &str,
    modality_id: u32,
) -> DynamicJepaResult<Option<ConstellationCentroid>> {
    get_record(
        db,
        CF_DJ_CONSTELLATIONS,
        constellation_key(&domain_uuid, subject_id, modality_id),
    )
}

pub fn put_constellations_with_audit_batch(
    db: &DB,
    records: &[(Uuid, String, u32, ConstellationCentroid)],
    audit: &DjAuditRecord,
) -> DynamicJepaResult<()> {
    for (_, _, _, record) in records {
        record.validate_record()?;
    }
    audit.validate()?;

    let mut batch = WriteBatch::default();
    for (domain_uuid, subject_id, modality_id, record) in records {
        batch.put_cf(
            cf(db, CF_DJ_CONSTELLATIONS)?,
            constellation_key(domain_uuid, subject_id, *modality_id),
            encode_record(record)?,
        );
    }
    write_batch_with_audit_witnesses(db, batch, &[audit], "put_constellations_with_audit_batch")
}

pub fn list_constellations(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<ConstellationCentroid>> {
    list_records(db, CF_DJ_CONSTELLATIONS, limit, offset)
}

pub fn count_constellations(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_CONSTELLATIONS)
}
