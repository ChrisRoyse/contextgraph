use context_graph_core::dynamicjepa::{
    DynamicJepaError, DynamicJepaResult, EventId, PairwiseReading,
};
use rocksdb::DB;
use uuid::Uuid;

use crate::dynamicjepa::column_families::CF_DJ_PAIRWISE_READINGS;
use crate::dynamicjepa::common::{
    count_cf, get_record, list_prefix_records, list_records, put_record,
};
use crate::dynamicjepa::keys::pairwise_reading_key;

pub fn put_pairwise_reading(
    db: &DB,
    event_uuid: Uuid,
    instrument_j: u32,
    instrument_k: u32,
    record: &PairwiseReading,
) -> DynamicJepaResult<()> {
    validate_pairwise_key_parts(event_uuid, instrument_j, instrument_k, record)?;
    put_record(
        db,
        CF_DJ_PAIRWISE_READINGS,
        pairwise_reading_key(&event_uuid, instrument_j, instrument_k),
        record,
    )
}

pub fn get_pairwise_reading(
    db: &DB,
    event_uuid: Uuid,
    instrument_j: u32,
    instrument_k: u32,
) -> DynamicJepaResult<Option<PairwiseReading>> {
    validate_numeric_pair_order(instrument_j, instrument_k)?;
    get_record(
        db,
        CF_DJ_PAIRWISE_READINGS,
        pairwise_reading_key(&event_uuid, instrument_j, instrument_k),
    )
}

pub fn list_pairwise_readings_for_event(
    db: &DB,
    event_id: EventId,
) -> DynamicJepaResult<Vec<PairwiseReading>> {
    list_prefix_records(db, CF_DJ_PAIRWISE_READINGS, &event_id.into_bytes())
}

pub fn list_pairwise_readings(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<PairwiseReading>> {
    list_records(db, CF_DJ_PAIRWISE_READINGS, limit, offset)
}

pub fn count_pairwise_readings(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_PAIRWISE_READINGS)
}

fn validate_pairwise_key_parts(
    event_uuid: Uuid,
    instrument_j: u32,
    instrument_k: u32,
    record: &PairwiseReading,
) -> DynamicJepaResult<()> {
    if record.event_id.as_uuid() != event_uuid {
        return Err(DynamicJepaError::validation(
            "PairwiseReading.event_id",
            format!(
                "record event_id {} does not match key event_uuid {}",
                record.event_id, event_uuid
            ),
            "use one event UUID for both the key and persisted pairwise row",
        ));
    }
    validate_numeric_pair_order(instrument_j, instrument_k)
}

fn validate_numeric_pair_order(instrument_j: u32, instrument_k: u32) -> DynamicJepaResult<()> {
    if instrument_j >= instrument_k {
        return Err(DynamicJepaError::PairwiseAsymmetricOrdering {
            instrument_j: instrument_j.to_string(),
            instrument_k: instrument_k.to_string(),
        });
    }
    Ok(())
}
