use context_graph_core::dynamicjepa::{
    DynamicJepaError, DynamicJepaRecord, DynamicJepaResult, VerificationRunId,
    VerificationRunRecord,
};
use rocksdb::{IteratorMode, WriteBatch, DB};
use uuid::Uuid;

use crate::dynamicjepa::audit::{DjAuditRecord, DJ_AUDIT_RECORD_VERSION};
use crate::dynamicjepa::audit_witness::write_batch_with_audit_witnesses;
use crate::dynamicjepa::column_families::{CF_DJ_AUDIT_LOG, CF_DJ_VERIFICATION_RUNS};
use crate::dynamicjepa::common::{cf, count_cf, get_record, list_records, storage_error};
use crate::dynamicjepa::encode::{decode_plain, encode_record};
use crate::dynamicjepa::keys::audit_key;

pub fn put_verification_run(db: &DB, record: &VerificationRunRecord) -> DynamicJepaResult<()> {
    crate::dynamicjepa::common::put_record(
        db,
        CF_DJ_VERIFICATION_RUNS,
        record.verification_run_id.into_bytes(),
        record,
    )
}

pub fn put_verification_run_with_audit_batch(
    db: &DB,
    record: &VerificationRunRecord,
    audit: &DjAuditRecord,
) -> DynamicJepaResult<()> {
    record.validate_record()?;
    audit.validate()?;
    let mut batch = WriteBatch::default();
    batch.put_cf(
        cf(db, CF_DJ_VERIFICATION_RUNS)?,
        record.verification_run_id.into_bytes(),
        encode_record(record)?,
    );
    write_batch_with_audit_witnesses(db, batch, &[audit], "put_verification_run_with_audit_batch")
}

pub fn get_verification_run(
    db: &DB,
    id: VerificationRunId,
) -> DynamicJepaResult<Option<VerificationRunRecord>> {
    get_record(db, CF_DJ_VERIFICATION_RUNS, id.into_bytes())
}

pub fn list_verification_runs(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<VerificationRunRecord>> {
    list_records(db, CF_DJ_VERIFICATION_RUNS, limit, offset)
}

pub fn count_verification_runs(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_VERIFICATION_RUNS)
}

pub fn put_audit_record(db: &DB, record: &DjAuditRecord) -> DynamicJepaResult<()> {
    record.validate()?;
    let batch = rocksdb::WriteBatch::default();
    write_batch_with_audit_witnesses(db, batch, &[record], "put_audit_record")
}

pub fn get_audit_record(
    db: &DB,
    timestamp_unix_nanos: u64,
    audit_id: Uuid,
) -> DynamicJepaResult<Option<DjAuditRecord>> {
    match db.get_cf(
        cf(db, CF_DJ_AUDIT_LOG)?,
        audit_key(timestamp_unix_nanos, audit_id),
    ) {
        Ok(Some(bytes)) => decode_plain(&bytes, DJ_AUDIT_RECORD_VERSION, "DjAuditRecord").map(Some),
        Ok(None) => Ok(None),
        Err(err) => Err(DynamicJepaError::Storage {
            operation: "get_audit_record".to_string(),
            cf: CF_DJ_AUDIT_LOG.to_string(),
            message: err.to_string(),
            remediation: "inspect the audit log CF and key encoder".to_string(),
        }),
    }
}

pub fn list_audit_records(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<DjAuditRecord>> {
    let iter = db.iterator_cf(cf(db, CF_DJ_AUDIT_LOG)?, IteratorMode::Start);
    let mut out = Vec::new();
    for (idx, item) in iter.enumerate() {
        let (_key, value) = item.map_err(|err| {
            storage_error(
                "list_audit_records",
                CF_DJ_AUDIT_LOG,
                err.to_string(),
                "inspect RocksDB LOG files and retry from a fresh DB",
            )
        })?;
        if idx < offset {
            continue;
        }
        if out.len() >= limit {
            break;
        }
        out.push(decode_plain(
            &value,
            DJ_AUDIT_RECORD_VERSION,
            "DjAuditRecord",
        )?);
    }
    Ok(out)
}

pub fn count_audit_records(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_AUDIT_LOG)
}
