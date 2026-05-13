use rocksdb::IteratorMode;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::heal::cf::{decode_value, encode_value, CF_MEJEPA_MODEL_PROMOTIONS};
use crate::heal::errors::HealError;
use crate::heal::store::HealRocksStore;

pub const PHASE_E_PREFIX: &str = "phase_e";

pub fn policy_key(parts: &[&str]) -> Result<Vec<u8>, HealError> {
    if parts.is_empty() {
        return Err(HealError::invalid(
            "phase_e.policy_key",
            "policy key must contain at least one part",
        ));
    }
    let mut out = Vec::new();
    for (idx, part) in parts.iter().enumerate() {
        if part.trim().is_empty() || part.bytes().any(|byte| byte == b'\n' || byte == 0) {
            return Err(HealError::invalid(
                "phase_e.policy_key",
                format!("key part {idx} must be non-empty single-line text"),
            ));
        }
        if idx > 0 {
            out.push(b'/');
        }
        out.extend_from_slice(part.as_bytes());
    }
    Ok(out)
}

pub fn timestamped_policy_key(kind: &str) -> Result<Vec<u8>, HealError> {
    let ts = chrono::Utc::now().timestamp_millis().to_string();
    policy_key(&[PHASE_E_PREFIX, kind, &ts])
}

pub fn persist_policy_record<T>(
    storage: &HealRocksStore,
    key: &[u8],
    value: &T,
) -> Result<(), HealError>
where
    T: Serialize + DeserializeOwned,
{
    let bytes = encode_value(value)?;
    storage.put_cf_readback(CF_MEJEPA_MODEL_PROMOTIONS, key, &bytes)?;
    let readback = storage
        .get_cf(CF_MEJEPA_MODEL_PROMOTIONS, key)?
        .ok_or_else(|| {
            HealError::invalid(
                "phase_e.policy_readback",
                format!(
                    "missing CF_MEJEPA_MODEL_PROMOTIONS row {}",
                    hex::encode(key)
                ),
            )
        })?;
    if readback != bytes {
        return Err(HealError::invalid(
            "phase_e.policy_readback",
            format!(
                "CF_MEJEPA_MODEL_PROMOTIONS row {} read back with different bytes",
                hex::encode(key)
            ),
        ));
    }
    let decoded: T = decode_value(&readback)?;
    let decoded_bytes = encode_value(&decoded)?;
    if decoded_bytes != bytes {
        return Err(HealError::invalid(
            "phase_e.policy_readback",
            format!(
                "CF_MEJEPA_MODEL_PROMOTIONS row {} decoded non-deterministically",
                hex::encode(key)
            ),
        ));
    }
    Ok(())
}

pub fn load_policy_record<T>(storage: &HealRocksStore, key: &[u8]) -> Result<Option<T>, HealError>
where
    T: DeserializeOwned,
{
    let Some(bytes) = storage.get_cf(CF_MEJEPA_MODEL_PROMOTIONS, key)? else {
        return Ok(None);
    };
    Ok(Some(decode_value(&bytes)?))
}

pub fn scan_policy_records<T>(
    storage: &HealRocksStore,
    prefix: &[u8],
) -> Result<Vec<(Vec<u8>, T)>, HealError>
where
    T: DeserializeOwned,
{
    if prefix.is_empty() {
        return Err(HealError::invalid(
            "phase_e.policy_scan_prefix",
            "scan prefix must be non-empty",
        ));
    }
    let db = storage.db();
    let cf = db.cf_handle(CF_MEJEPA_MODEL_PROMOTIONS).ok_or_else(|| {
        HealError::invalid(
            "phase_e.policy_cf",
            format!("missing {CF_MEJEPA_MODEL_PROMOTIONS}"),
        )
    })?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, value) = item?;
        if key.starts_with(prefix) {
            out.push((key.to_vec(), decode_value(&value)?));
        }
    }
    out.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(out)
}

pub fn delete_policy_record_readback(
    storage: &HealRocksStore,
    key: &[u8],
) -> Result<(), HealError> {
    if key.is_empty() {
        return Err(HealError::invalid(
            "phase_e.policy_key",
            "policy key must be non-empty",
        ));
    }
    let db = storage.db();
    let cf = db.cf_handle(CF_MEJEPA_MODEL_PROMOTIONS).ok_or_else(|| {
        HealError::invalid(
            "phase_e.policy_cf",
            format!("missing {CF_MEJEPA_MODEL_PROMOTIONS}"),
        )
    })?;
    let mut opts = rocksdb::WriteOptions::default();
    opts.set_sync(true);
    db.delete_cf_opt(cf, key, &opts)?;
    if db.get_cf(cf, key)?.is_some() {
        return Err(HealError::invalid(
            "phase_e.policy_delete_readback",
            format!("row {} still exists after delete", hex::encode(key)),
        ));
    }
    Ok(())
}
