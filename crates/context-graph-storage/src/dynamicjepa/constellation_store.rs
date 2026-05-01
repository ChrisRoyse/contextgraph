use context_graph_core::dynamicjepa::{ConstellationCentroid, DynamicJepaResult};
use rocksdb::DB;
use uuid::Uuid;

use crate::dynamicjepa::column_families::CF_DJ_CONSTELLATIONS;
use crate::dynamicjepa::common::{count_cf, get_record, list_records, put_record};
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
