use context_graph_core::dynamicjepa::{DynamicJepaResult, ThresholdCalibration};
use rocksdb::DB;
use uuid::Uuid;

use crate::dynamicjepa::column_families::CF_DJ_THRESHOLD_CALIBRATIONS;
use crate::dynamicjepa::common::{count_cf, get_record, list_records, put_record};
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
