use context_graph_core::dynamicjepa::{DynamicJepaResult, SurpriseEventId, SurpriseEventRecord};
use rocksdb::DB;

use crate::dynamicjepa::column_families::CF_DJ_SURPRISE_EVENTS;
use crate::dynamicjepa::common::{count_cf, get_record, list_records, put_record};

pub fn put_surprise_event(db: &DB, record: &SurpriseEventRecord) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_SURPRISE_EVENTS,
        record.surprise_event_id.into_bytes(),
        record,
    )
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
