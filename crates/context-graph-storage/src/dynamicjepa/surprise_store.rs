use context_graph_core::dynamicjepa::{
    DynamicJepaRecord, DynamicJepaResult, SurpriseEventId, SurpriseEventRecord,
};
use rocksdb::{WriteBatch, DB};

use crate::dynamicjepa::audit::{DjAuditRecord, DJ_AUDIT_RECORD_VERSION};
use crate::dynamicjepa::column_families::{CF_DJ_AUDIT_LOG, CF_DJ_SURPRISE_EVENTS};
use crate::dynamicjepa::common::{cf, count_cf, get_record, list_records, put_record, write_batch};
use crate::dynamicjepa::encode::{encode_plain, encode_record};
use crate::dynamicjepa::keys::audit_key;

pub fn put_surprise_event(db: &DB, record: &SurpriseEventRecord) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_SURPRISE_EVENTS,
        record.surprise_event_id.into_bytes(),
        record,
    )
}

pub fn put_surprise_event_with_audit_batch(
    db: &DB,
    record: &SurpriseEventRecord,
    audit: &DjAuditRecord,
) -> DynamicJepaResult<()> {
    record.validate_record()?;
    audit.validate()?;
    let mut batch = WriteBatch::default();
    batch.put_cf(
        cf(db, CF_DJ_SURPRISE_EVENTS)?,
        record.surprise_event_id.into_bytes(),
        encode_record(record)?,
    );
    batch.put_cf(
        cf(db, CF_DJ_AUDIT_LOG)?,
        audit_key(audit.timestamp_unix_nanos, audit.audit_id),
        encode_plain(audit, DJ_AUDIT_RECORD_VERSION, "DjAuditRecord")?,
    );
    write_batch(db, batch, "put_surprise_event_with_audit_batch")
}

pub fn get_surprise_event(
    db: &DB,
    id: SurpriseEventId,
) -> DynamicJepaResult<Option<SurpriseEventRecord>> {
    get_record(db, CF_DJ_SURPRISE_EVENTS, id.into_bytes())
}

pub fn list_surprise_events(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<SurpriseEventRecord>> {
    list_records(db, CF_DJ_SURPRISE_EVENTS, limit, offset)
}

pub fn count_surprise_events(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_SURPRISE_EVENTS)
}
