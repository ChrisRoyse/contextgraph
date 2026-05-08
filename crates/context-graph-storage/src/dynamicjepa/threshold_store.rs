use context_graph_core::dynamicjepa::{DynamicJepaRecord, DynamicJepaResult, ThresholdCalibration};
use rocksdb::{WriteBatch, DB};
use uuid::Uuid;

use crate::dynamicjepa::audit::DjAuditRecord;
use crate::dynamicjepa::audit_witness::write_batch_with_audit_witnesses;
use crate::dynamicjepa::column_families::CF_DJ_THRESHOLD_CALIBRATIONS;
use crate::dynamicjepa::common::{cf, count_cf, get_record, list_records, put_record};
use crate::dynamicjepa::encode::encode_record;
use crate::dynamicjepa::keys::threshold_calibration_key;

pub fn put_threshold_calibration(
    db: &DB,
    domain_uuid: Uuid,
    subject_id: &str,
    modality_id: u32,
    supersede_seq: u32,
    record: &ThresholdCalibration,
) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_THRESHOLD_CALIBRATIONS,
        threshold_calibration_key(&domain_uuid, subject_id, modality_id, supersede_seq),
        record,
    )
}

pub fn get_threshold_calibration(
    db: &DB,
    domain_uuid: Uuid,
    subject_id: &str,
    modality_id: u32,
    supersede_seq: u32,
) -> DynamicJepaResult<Option<ThresholdCalibration>> {
    get_record(
        db,
        CF_DJ_THRESHOLD_CALIBRATIONS,
        threshold_calibration_key(&domain_uuid, subject_id, modality_id, supersede_seq),
    )
}

pub fn put_threshold_calibrations_with_audit_batch(
    db: &DB,
    records: &[(Uuid, String, u32, u32, ThresholdCalibration)],
    audit: &DjAuditRecord,
) -> DynamicJepaResult<()> {
    for (_, _, _, _, record) in records {
        record.validate_record()?;
    }
    audit.validate()?;

    let mut batch = WriteBatch::default();
    for (domain_uuid, subject_id, modality_id, supersede_seq, record) in records {
        batch.put_cf(
            cf(db, CF_DJ_THRESHOLD_CALIBRATIONS)?,
            threshold_calibration_key(domain_uuid, subject_id, *modality_id, *supersede_seq),
            encode_record(record)?,
        );
    }
    write_batch_with_audit_witnesses(
        db,
        batch,
        &[audit],
        "put_threshold_calibrations_with_audit_batch",
    )
}

pub fn list_threshold_calibrations(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<ThresholdCalibration>> {
    list_records(db, CF_DJ_THRESHOLD_CALIBRATIONS, limit, offset)
}

pub fn count_threshold_calibrations(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_THRESHOLD_CALIBRATIONS)
}
