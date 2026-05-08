use context_graph_core::dynamicjepa::{
    DynamicJepaRecord, DynamicJepaResult, TrajectoryId, TrajectoryRecord,
};
use rocksdb::{WriteBatch, DB};

use crate::dynamicjepa::audit::DjAuditRecord;
use crate::dynamicjepa::audit_witness::write_batch_with_audit_witnesses;
use crate::dynamicjepa::column_families::CF_DJ_TRAJECTORIES;
use crate::dynamicjepa::common::{cf, count_cf, get_record, list_records, put_record};
use crate::dynamicjepa::encode::encode_record;

pub fn put_trajectory(db: &DB, record: &TrajectoryRecord) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_TRAJECTORIES,
        record.trajectory_id.into_bytes(),
        record,
    )
}

pub fn put_trajectory_with_audit_batch(
    db: &DB,
    record: &TrajectoryRecord,
    audit: &DjAuditRecord,
) -> DynamicJepaResult<()> {
    record.validate_record()?;
    audit.validate()?;
    let mut batch = WriteBatch::default();
    batch.put_cf(
        cf(db, CF_DJ_TRAJECTORIES)?,
        record.trajectory_id.into_bytes(),
        encode_record(record)?,
    );
    write_batch_with_audit_witnesses(db, batch, &[audit], "put_trajectory_with_audit_batch")
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
