use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::heal::errors::HealError;

pub(crate) fn put_policy_record<T: Serialize + DeserializeOwned>(
    db: &DB,
    key: &[u8],
    value: &T,
) -> Result<(), HealError> {
    if key.is_empty() {
        return Err(HealError::invalid(
            "policy_store.key",
            "policy record key must be non-empty",
        ));
    }
    let cf = policy_cf(db)?;
    let bytes = bincode::serialize(value)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, &bytes, &opts)?;
    let readback = db.get_cf(cf, key)?.ok_or_else(|| {
        HealError::invalid(
            "policy_store.readback",
            format!(
                "missing policy row in {} after put",
                context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS
            ),
        )
    })?;
    if readback != bytes {
        return Err(HealError::invalid(
            "policy_store.readback",
            format!(
                "policy row readback bytes differ in {}",
                context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS
            ),
        ));
    }
    let decoded: T = bincode::deserialize(&readback)?;
    let decoded_bytes = bincode::serialize(&decoded)?;
    if decoded_bytes != bytes {
        return Err(HealError::invalid(
            "policy_store.readback",
            "policy row decoded readback does not reserialize to the original bytes",
        ));
    }
    Ok(())
}

pub(crate) fn get_policy_record<T: DeserializeOwned>(
    db: &DB,
    key: &[u8],
) -> Result<Option<T>, HealError> {
    if key.is_empty() {
        return Err(HealError::invalid(
            "policy_store.key",
            "policy record key must be non-empty",
        ));
    }
    let Some(bytes) = db.get_cf(policy_cf(db)?, key)? else {
        return Ok(None);
    };
    Ok(Some(bincode::deserialize(&bytes)?))
}

pub(crate) fn scan_policy_records<T: DeserializeOwned>(
    db: &DB,
    prefix: &[u8],
) -> Result<Vec<(Vec<u8>, T)>, HealError> {
    if prefix.is_empty() {
        return Err(HealError::invalid(
            "policy_store.prefix",
            "policy record prefix must be non-empty",
        ));
    }
    let cf = policy_cf(db)?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        if key.starts_with(prefix) {
            out.push((key.to_vec(), bincode::deserialize(&value)?));
        }
    }
    out.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(out)
}

pub(crate) fn delete_policy_record_readback(db: &DB, key: &[u8]) -> Result<(), HealError> {
    if key.is_empty() {
        return Err(HealError::invalid(
            "policy_store.key",
            "policy record key must be non-empty",
        ));
    }
    let cf = policy_cf(db)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.delete_cf_opt(cf, key, &opts)?;
    if db.get_cf(cf, key)?.is_some() {
        return Err(HealError::invalid(
            "policy_store.delete_readback",
            "policy row still exists after delete",
        ));
    }
    Ok(())
}

fn policy_cf<'a>(db: &'a DB) -> Result<&'a rocksdb::ColumnFamily, HealError> {
    db.cf_handle(context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS)
        .ok_or_else(|| {
            HealError::invalid(
                "policy_store.column_family",
                format!(
                    "missing {}",
                    context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS
                ),
            )
        })
}
