use context_graph_core::dynamicjepa::{
    DatasetId, DatasetShardId, DatasetShardRecord, DynamicJepaRecord, DynamicJepaResult,
};
use rocksdb::{WriteBatch, DB};

use crate::dynamicjepa::audit::{DjAuditRecord, DJ_AUDIT_RECORD_VERSION};
use crate::dynamicjepa::column_families::CF_DJ_AUDIT_LOG;
use crate::dynamicjepa::column_families::CF_DJ_DATASET_SHARDS;
use crate::dynamicjepa::common::{cf, count_cf, get_record, list_records, put_record, write_batch};
use crate::dynamicjepa::encode::{encode_plain, encode_record};
use crate::dynamicjepa::keys::{audit_key, dataset_shard_key};

pub fn put_dataset_shard(db: &DB, record: &DatasetShardRecord) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_DATASET_SHARDS,
        dataset_shard_key(record.dataset_id, record.shard_id),
        record,
    )
}

pub fn put_dataset_shard_with_audit_batch(
    db: &DB,
    record: &DatasetShardRecord,
    audit: &DjAuditRecord,
) -> DynamicJepaResult<()> {
    record.validate_record()?;
    audit.validate()?;
    let mut batch = WriteBatch::default();
    batch.put_cf(
        cf(db, CF_DJ_DATASET_SHARDS)?,
        dataset_shard_key(record.dataset_id, record.shard_id),
        encode_record(record)?,
    );
    batch.put_cf(
        cf(db, CF_DJ_AUDIT_LOG)?,
        audit_key(audit.timestamp_unix_nanos, audit.audit_id),
        encode_plain(audit, DJ_AUDIT_RECORD_VERSION, "DjAuditRecord")?,
    );
    write_batch(db, batch, "put_dataset_shard_with_audit_batch")
}

pub fn get_dataset_shard(
    db: &DB,
    dataset_id: DatasetId,
    shard_id: DatasetShardId,
) -> DynamicJepaResult<Option<DatasetShardRecord>> {
    get_record(
        db,
        CF_DJ_DATASET_SHARDS,
        dataset_shard_key(dataset_id, shard_id),
    )
}

pub fn list_dataset_shards(
    db: &DB,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<DatasetShardRecord>> {
    list_records(db, CF_DJ_DATASET_SHARDS, limit, offset)
}

pub fn count_dataset_shards(db: &DB) -> DynamicJepaResult<u64> {
    count_cf(db, CF_DJ_DATASET_SHARDS)
}
