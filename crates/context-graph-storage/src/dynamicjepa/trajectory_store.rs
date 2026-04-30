use context_graph_core::dynamicjepa::{DynamicJepaResult, TrajectoryId, TrajectoryRecord};
use rocksdb::DB;

use crate::dynamicjepa::column_families::CF_DJ_TRAJECTORIES;
use crate::dynamicjepa::common::{count_cf, get_record, list_records, put_record};

pub fn put_trajectory(db: &DB, record: &TrajectoryRecord) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_TRAJECTORIES,
        record.trajectory_id.into_bytes(),
        record,
    )
}

pub fn get_trajectory(db: &DB, id: TrajectoryId) -> DynamicJepaResult<Option<TrajectoryRecord>> {
    get_record(db, CF_DJ_TRAJECTORIES, id.into_bytes())
}

pub fn list_trajectories(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<TrajectoryRecord>> {
    list_records(db, CF_DJ_TRAJECTORIES, limit, offset)
}

pub fn count_trajectories(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_TRAJECTORIES)
}
