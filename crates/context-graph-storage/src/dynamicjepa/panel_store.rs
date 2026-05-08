use context_graph_core::dynamicjepa::{
    DynamicJepaError, DynamicJepaRecord, DynamicJepaResult, EventId, InstrumentId,
    InstrumentReading, LatentPanel, PairwiseReading, PanelId, ReadingStatus,
};
use rocksdb::{WriteBatch, DB};
use std::collections::{BTreeMap, BTreeSet};

use crate::dynamicjepa::audit::DjAuditRecord;
use crate::dynamicjepa::audit_witness::write_batch_with_audit_witnesses;
use crate::dynamicjepa::column_families::{
    CF_DJ_INSTRUMENT_READINGS, CF_DJ_LATENT_PANELS, CF_DJ_PAIRWISE_READINGS,
};
use crate::dynamicjepa::common::{
    cf, count_cf, get_record, list_prefix_records, list_records, put_record,
};
use crate::dynamicjepa::encode::encode_record;
use crate::dynamicjepa::keys::{instrument_reading_key, pairwise_reading_key};

pub fn put_panel_with_readings_and_pairwise_batch(
    db: &DB,
    readings: &[InstrumentReading],
    pairwise_readings: &[PairwiseReading],
    panel: &LatentPanel,
    audits: &[DjAuditRecord],
) -> DynamicJepaResult<()> {
    panel.validate_record()?;
    validate_panel_pairwise_batch(readings, pairwise_readings, panel, audits)?;
    let mut batch = WriteBatch::default();
    for reading in readings {
        reading.validate_record()?;
        batch.put_cf(
            cf(db, CF_DJ_INSTRUMENT_READINGS)?,
            instrument_reading_key(reading.event_id, &reading.instrument_id),
            encode_record(reading)?,
        );
    }
    let instrument_ordinals = instrument_ordinals(readings)?;
    for pairwise in pairwise_readings {
        pairwise.validate_record()?;
        let instrument_j = instrument_ordinal(&instrument_ordinals, &pairwise.instrument_j)?;
        let instrument_k = instrument_ordinal(&instrument_ordinals, &pairwise.instrument_k)?;
        if instrument_j >= instrument_k {
            return Err(DynamicJepaError::PairwiseAsymmetricOrdering {
                instrument_j: pairwise.instrument_j.to_string(),
                instrument_k: pairwise.instrument_k.to_string(),
            });
        }
        batch.put_cf(
            cf(db, CF_DJ_PAIRWISE_READINGS)?,
            pairwise_reading_key(&pairwise.event_id.as_uuid(), instrument_j, instrument_k),
            encode_record(pairwise)?,
        );
    }
    batch.put_cf(
        cf(db, CF_DJ_LATENT_PANELS)?,
        panel.panel_id.into_bytes(),
        encode_record(panel)?,
    );
    let audit_refs = audits.iter().collect::<Vec<_>>();
    write_batch_with_audit_witnesses(
        db,
        batch,
        &audit_refs,
        "put_panel_with_readings_and_pairwise_batch",
    )
}

fn validate_panel_pairwise_batch(
    readings: &[InstrumentReading],
    pairwise_readings: &[PairwiseReading],
    panel: &LatentPanel,
    audits: &[DjAuditRecord],
) -> DynamicJepaResult<()> {
    if audits.is_empty() {
        return Err(DynamicJepaError::validation(
            "DjAuditRecord",
            "panel materialization batch requires at least one audit row",
            "write materialize_panel and materialize_pairwise audit provenance with the same RocksDB batch",
        ));
    }
    for audit in audits {
        audit.validate()?;
    }
    let successful_readings = readings
        .iter()
        .filter(|reading| matches!(reading.status, ReadingStatus::Ok))
        .count();
    let expected_pairwise = successful_readings
        .checked_mul(successful_readings.saturating_sub(1))
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "InstrumentReading.count",
                "successful reading count overflowed pairwise cardinality",
                "use realistic bounded panel sizes",
            )
        })?
        / 2;
    if pairwise_readings.len() != expected_pairwise {
        return Err(DynamicJepaError::PairwiseRowCountMismatch {
            event_id: panel.event_id.as_uuid(),
            expected: expected_pairwise as u64,
            actual: pairwise_readings.len() as u64,
        });
    }
    let mut panel_pairwise_ids = panel
        .pairwise_reading_ids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let mut row_pairwise_ids = pairwise_readings
        .iter()
        .map(|row| row.pairwise_id.to_string())
        .collect::<Vec<_>>();
    panel_pairwise_ids.sort();
    row_pairwise_ids.sort();
    if panel_pairwise_ids != row_pairwise_ids {
        return Err(DynamicJepaError::validation(
            "LatentPanel.pairwise_reading_ids",
            "panel pairwise_reading_ids do not match pairwise rows in the batch",
            "compose the panel only after computing the exact pairwise rows for the event",
        ));
    }
    let mut seen_readings = BTreeSet::new();
    for reading in readings {
        if reading.event_id != panel.event_id {
            return Err(DynamicJepaError::validation(
                "InstrumentReading.event_id",
                format!(
                    "reading event {} does not match panel event {}",
                    reading.event_id, panel.event_id
                ),
                "write one panel batch per event",
            ));
        }
        if !seen_readings.insert(reading.instrument_id.to_string()) {
            return Err(DynamicJepaError::validation(
                "InstrumentReading.instrument_id",
                format!("duplicate reading instrument {}", reading.instrument_id),
                "write at most one reading per instrument for a panel event",
            ));
        }
    }
    let successful = readings
        .iter()
        .filter(|reading| matches!(reading.status, ReadingStatus::Ok))
        .map(|reading| reading.instrument_id.to_string())
        .collect::<BTreeSet<_>>();
    let mut seen_pairs = BTreeSet::new();
    for pairwise in pairwise_readings {
        if pairwise.event_id != panel.event_id {
            return Err(DynamicJepaError::validation(
                "PairwiseReading.event_id",
                format!(
                    "pairwise event {} does not match panel event {}",
                    pairwise.event_id, panel.event_id
                ),
                "write one pairwise batch per event",
            ));
        }
        if !successful.contains(pairwise.instrument_j.as_str())
            || !successful.contains(pairwise.instrument_k.as_str())
        {
            return Err(DynamicJepaError::validation(
                "PairwiseReading.instrument_ids",
                format!(
                    "pairwise row {}/{} does not reference two successful instrument readings",
                    pairwise.instrument_j, pairwise.instrument_k
                ),
                "compute pairwise rows only from successful same-event readings",
            ));
        }
        let key = (
            pairwise.instrument_j.to_string(),
            pairwise.instrument_k.to_string(),
        );
        if !seen_pairs.insert(key) {
            return Err(DynamicJepaError::validation(
                "PairwiseReading.instrument_ids",
                "duplicate pairwise row in panel batch",
                "emit exactly one row for each unordered instrument pair",
            ));
        }
    }
    Ok(())
}

fn instrument_ordinals(readings: &[InstrumentReading]) -> DynamicJepaResult<BTreeMap<String, u32>> {
    let mut out = BTreeMap::new();
    for (idx, reading) in readings.iter().enumerate() {
        let ordinal = u32::try_from(idx + 1).map_err(|_| {
            DynamicJepaError::validation(
                "InstrumentReading.index",
                "instrument index exceeds u32 key capacity",
                "reduce the domain pack instrument count before materializing pairwise rows",
            )
        })?;
        out.insert(reading.instrument_id.to_string(), ordinal);
    }
    Ok(out)
}

fn instrument_ordinal(
    ordinals: &BTreeMap<String, u32>,
    instrument_id: &InstrumentId,
) -> DynamicJepaResult<u32> {
    ordinals
        .get(instrument_id.as_str())
        .copied()
        .ok_or_else(|| {
            DynamicJepaError::validation(
                "PairwiseReading.instrument_id",
                format!("instrument {instrument_id} is not present in the panel reading set"),
                "compute pairwise rows from the same readings passed to the panel batch writer",
            )
        })
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
