use context_graph_core::dynamicjepa::{DynamicJepaError, DynamicJepaRecord, DynamicJepaResult};
use rocksdb::{ColumnFamily, IteratorMode, WriteBatch, DB};
use serde::{de::DeserializeOwned, Serialize};

use crate::dynamicjepa::encode::{decode_record, encode_record};

pub(crate) fn storage_error(
    operation: impl Into<String>,
    cf: impl Into<String>,
    message: impl Into<String>,
    remediation: impl Into<String>,
) -> DynamicJepaError {
    DynamicJepaError::Storage {
        operation: operation.into(),
        cf: cf.into(),
        message: message.into(),
        remediation: remediation.into(),
    }
}

pub(crate) fn cf<'a>(db: &'a DB, cf_name: &'static str) -> DynamicJepaResult<&'a ColumnFamily> {
    db.cf_handle(cf_name).ok_or_else(|| {
        storage_error(
            "cf_handle",
            cf_name,
            format!("column family {cf_name:?} is missing"),
            "open the DB through RocksDbTeleologicalStore so DynamicJEPA CF descriptors are registered",
        )
    })
}

pub(crate) fn write_batch(
    db: &DB,
    batch: WriteBatch,
    operation: &'static str,
) -> DynamicJepaResult<()> {
    db.write(batch).map_err(|err| {
        storage_error(
            operation,
            "write_batch",
            err.to_string(),
            "inspect RocksDB LOG files and rerun the operation against a fresh /tmp demo DB",
        )
    })
}

pub(crate) fn put_record<R: DynamicJepaRecord>(
    db: &DB,
    cf_name: &'static str,
    key: impl AsRef<[u8]>,
    record: &R,
) -> DynamicJepaResult<()> {
    let bytes = encode_record(record)?;
    db.put_cf(cf(db, cf_name)?, key, bytes).map_err(|err| {
        storage_error(
            "put_record",
            cf_name,
            err.to_string(),
            "verify the DB path is writable and the CF exists",
        )
    })
}

pub(crate) fn get_record<R>(
    db: &DB,
    cf_name: &'static str,
    key: impl AsRef<[u8]>,
) -> DynamicJepaResult<Option<R>>
where
    R: DynamicJepaRecord + DeserializeOwned,
{
    match db.get_cf(cf(db, cf_name)?, key) {
        Ok(Some(bytes)) => decode_record(&bytes).map(Some),
        Ok(None) => Ok(None),
        Err(err) => Err(storage_error(
            "get_record",
            cf_name,
            err.to_string(),
            "inspect RocksDB LOG files and verify the key encoder",
        )),
    }
}

pub(crate) fn list_records<R>(
    db: &DB,
    cf_name: &'static str,
    limit: usize,
    offset: usize,
) -> DynamicJepaResult<Vec<R>>
where
    R: DynamicJepaRecord + DeserializeOwned,
{
    let iter = db.iterator_cf(cf(db, cf_name)?, IteratorMode::Start);
    let mut out = Vec::new();
    for (idx, item) in iter.enumerate() {
        let (_key, value) = item.map_err(|err| {
            storage_error(
                "list_records",
                cf_name,
                err.to_string(),
                "inspect RocksDB LOG files and retry from a fresh DB",
            )
        })?;
        if idx < offset {
            continue;
        }
        if out.len() >= limit {
            break;
        }
        out.push(decode_record(&value)?);
    }
    Ok(out)
}

pub(crate) fn list_prefix_records<R>(
    db: &DB,
    cf_name: &'static str,
    prefix: &[u8],
) -> DynamicJepaResult<Vec<R>>
where
    R: DynamicJepaRecord + DeserializeOwned,
{
    let iter = db.prefix_iterator_cf(cf(db, cf_name)?, prefix);
    let mut out = Vec::new();
    for item in iter {
        let (key, value) = item.map_err(|err| {
            storage_error(
                "list_prefix_records",
                cf_name,
                err.to_string(),
                "inspect RocksDB LOG files and retry from a fresh DB",
            )
        })?;
        if !key.starts_with(prefix) {
            break;
        }
        out.push(decode_record(&value)?);
    }
    Ok(out)
}

pub(crate) fn count_cf(db: &DB, cf_name: &'static str) -> DynamicJepaResult<u64> {
    let iter = db.iterator_cf(cf(db, cf_name)?, IteratorMode::Start);
    let mut count = 0u64;
    for item in iter {
        item.map_err(|err| {
            storage_error(
                "count_cf",
                cf_name,
                err.to_string(),
                "inspect RocksDB LOG files and retry from a fresh DB",
            )
        })?;
        count += 1;
    }
    Ok(count)
}

pub(crate) fn to_json<T: Serialize>(
    value: &T,
    type_name: &'static str,
) -> DynamicJepaResult<serde_json::Value> {
    serde_json::to_value(value).map_err(|err| {
        DynamicJepaError::validation(
            type_name,
            format!("serde_json conversion failed: {err}"),
            "inspect the record shape; inspection output must be JSON serializable",
        )
    })
}
