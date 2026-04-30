use context_graph_core::dynamicjepa::{
    DynamicJepaRecord, DynamicJepaResult, EventId, InstrumentId, InstrumentReading, LatentPanel,
    PanelId,
};
use rocksdb::{WriteBatch, DB};

use crate::dynamicjepa::audit::{DjAuditRecord, DJ_AUDIT_RECORD_VERSION};
use crate::dynamicjepa::column_families::{
    CF_DJ_AUDIT_LOG, CF_DJ_INSTRUMENT_READINGS, CF_DJ_LATENT_PANELS,
};
use crate::dynamicjepa::common::{
    cf, count_cf, get_record, list_prefix_records, list_records, put_record, write_batch,
};
use crate::dynamicjepa::encode::{encode_plain, encode_record};
use crate::dynamicjepa::keys::{audit_key, instrument_reading_key};

pub fn put_panel_with_readings_batch(
    db: &DB,
    readings: &[InstrumentReading],
    panel: &LatentPanel,
    audit: &DjAuditRecord,
) -> DynamicJepaResult<()> {
    panel.validate_record()?;
    audit.validate()?;
    let mut batch = WriteBatch::default();
    for reading in readings {
        reading.validate_record()?;
        batch.put_cf(
            cf(db, CF_DJ_INSTRUMENT_READINGS)?,
            instrument_reading_key(reading.event_id, &reading.instrument_id),
            encode_record(reading)?,
        );
    }
    batch.put_cf(
        cf(db, CF_DJ_LATENT_PANELS)?,
        panel.panel_id.into_bytes(),
        encode_record(panel)?,
    );
    batch.put_cf(
        cf(db, CF_DJ_AUDIT_LOG)?,
        audit_key(audit.timestamp_unix_nanos, audit.audit_id),
        encode_plain(audit, DJ_AUDIT_RECORD_VERSION, "DjAuditRecord")?,
    );
    write_batch(db, batch, "put_panel_with_readings_batch")
}

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
