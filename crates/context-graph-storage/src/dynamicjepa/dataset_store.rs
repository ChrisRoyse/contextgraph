use context_graph_core::dynamicjepa::{
    DatasetId, DatasetShardId, DatasetShardRecord, DynamicJepaResult,
};
use rocksdb::DB;

use crate::dynamicjepa::column_families::CF_DJ_DATASET_SHARDS;
use crate::dynamicjepa::common::{count_cf, get_record, list_records, put_record};
use crate::dynamicjepa::keys::dataset_shard_key;

pub fn put_dataset_shard(db: &DB, record: &DatasetShardRecord) -> DynamicJepaResult<()> {
    put_record(
        db,
        CF_DJ_DATASET_SHARDS,
        dataset_shard_key(record.dataset_id, record.shard_id),
        record,
    )
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
