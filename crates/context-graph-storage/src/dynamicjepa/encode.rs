//! DynamicJEPA versioned bincode helpers for storage.

use context_graph_core::dynamicjepa::{
    decode_versioned_record, encode_versioned_record, DynamicJepaError, DynamicJepaRecord,
    DynamicJepaResult,
};
use serde::{de::DeserializeOwned, Serialize};

pub const DYNAMIC_JEPA_STORAGE_VALUE_VERSION: u8 = 1;

pub fn encode_record<R: DynamicJepaRecord>(record: &R) -> DynamicJepaResult<Vec<u8>> {
    encode_versioned_record(record)
}

pub fn decode_record<R>(bytes: &[u8]) -> DynamicJepaResult<R>
where
    R: DynamicJepaRecord + DeserializeOwned,
{
    decode_versioned_record(bytes)
}

pub fn encode_plain<T: Serialize>(
    value: &T,
    version: u8,
    payload_type: &'static str,
) -> DynamicJepaResult<Vec<u8>> {
    let mut body = bincode::serialize(value).map_err(|err| {
        DynamicJepaError::codec(
            version,
            version,
            payload_type,
            format!("bincode serialize failed: {err}; inspect the registry/audit payload"),
        )
    })?;
    let mut out = Vec::with_capacity(body.len() + 1);
    out.push(version);
    out.append(&mut body);
    Ok(out)
}

pub fn decode_plain<T: DeserializeOwned>(
    bytes: &[u8],
    expected_version: u8,
    payload_type: &'static str,
) -> DynamicJepaResult<T> {
    if bytes.is_empty() {
        return Err(DynamicJepaError::codec(
            expected_version,
            0,
            payload_type,
            "payload is empty; expected [record_version][bincode payload]",
        ));
    }
    let actual_version = bytes[0];
    if actual_version != expected_version {
        return Err(DynamicJepaError::codec(
            expected_version,
            actual_version,
            payload_type,
            "no migration or fallback is supported; wipe the demo DB and rebuild records",
        ));
    }
    bincode::deserialize(&bytes[1..]).map_err(|err| {
        DynamicJepaError::codec(
            expected_version,
            actual_version,
            payload_type,
            format!("bincode deserialize failed: {err}; inspect bytes at the source of truth"),
        )
    })
}
