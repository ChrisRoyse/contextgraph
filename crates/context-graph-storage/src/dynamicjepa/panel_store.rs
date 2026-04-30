use context_graph_core::dynamicjepa::{
    DynamicJepaResult, EventId, InstrumentId, InstrumentReading, LatentPanel, PanelId,
};
use rocksdb::DB;

use crate::dynamicjepa::column_families::{CF_DJ_INSTRUMENT_READINGS, CF_DJ_LATENT_PANELS};
use crate::dynamicjepa::common::{
    count_cf, get_record, list_prefix_records, list_records, put_record,
};
use crate::dynamicjepa::keys::instrument_reading_key;

pub fn put_instrument_reading(db: &DB, record: &InstrumentReading) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_INSTRUMENT_READINGS,
        instrument_reading_key(record.event_id, &record.instrument_id),
        record,
    )
}

pub fn get_instrument_reading(
    db: &DB,
    event_id: EventId,
    instrument_id: &InstrumentId,
) -> DynamicJepaResult<Option<InstrumentReading>> {
    get_record(
        db,
        CF_DJ_INSTRUMENT_READINGS,
        instrument_reading_key(event_id, instrument_id),
    )
}

pub fn list_instrument_readings_for_event(
    db: &DB,
    event_id: EventId,
) -> DynamicJepaResult<Vec<InstrumentReading>> {
    list_prefix_records(db, CF_DJ_INSTRUMENT_READINGS, &event_id.into_bytes())
}

pub fn list_instrument_readings(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<InstrumentReading>> {
    list_records(db, CF_DJ_INSTRUMENT_READINGS, limit, offset)
}

pub fn count_instrument_readings(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_INSTRUMENT_READINGS)
}

pub fn put_latent_panel(db: &DB, record: &LatentPanel) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_LATENT_PANELS,
        record.panel_id.into_bytes(),
        record,
    )
}

pub fn get_latent_panel(db: &DB, id: PanelId) -> DynamicJepaResult<Option<LatentPanel>> {
    get_record(db, CF_DJ_LATENT_PANELS, id.into_bytes())
}

pub fn list_latent_panels(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<LatentPanel>> {
    list_records(db, CF_DJ_LATENT_PANELS, limit, offset)
}

pub fn count_latent_panels(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_LATENT_PANELS)
}
