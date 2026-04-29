use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::record_header::{validate_header, DjRecordHeader};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub trait Validate {
    fn validate(&self) -> DynamicJepaResult<()>;
}

pub trait DynamicJepaRecord: Validate + Serialize + Sized {
    const RECORD_VERSION: u8;
    const PAYLOAD_TYPE: &'static str;

    fn header(&self) -> &DjRecordHeader;
    fn header_mut(&mut self) -> &mut DjRecordHeader;

    fn compute_content_hash(&self) -> DynamicJepaResult<[u8; 32]> {
        canonical_payload_hash(self)
    }

    fn refresh_content_hash(&mut self) -> DynamicJepaResult<()> {
        let hash = self.compute_content_hash()?;
        self.header_mut().content_hash = hash;
        Ok(())
    }

    fn validate_record(&self) -> DynamicJepaResult<()> {
        validate_header(self.header(), Self::RECORD_VERSION, Self::PAYLOAD_TYPE)?;
        let actual = self.compute_content_hash()?;
        if self.header().content_hash != actual {
            return Err(DynamicJepaError::codec(
                Self::RECORD_VERSION,
                self.header().record_version,
                Self::PAYLOAD_TYPE,
                "record content_hash does not match payload; re-read the source of truth and regenerate the record instead of accepting corrupt bytes",
            ));
        }
        self.validate()
    }
}

pub fn canonical_payload_hash<T: Serialize>(record: &T) -> DynamicJepaResult<[u8; 32]> {
    let mut value = serde_json::to_value(record).map_err(|err| {
        DynamicJepaError::validation(
            "canonical_payload_hash",
            format!("serde_json conversion failed: {err}"),
            "ensure the record derives Serialize without unsupported fields",
        )
    })?;
    let object = value.as_object_mut().ok_or_else(|| {
        DynamicJepaError::validation(
            "canonical_payload_hash",
            "record must serialize to a JSON object",
            "records must be structs with a top-level header field",
        )
    })?;
    if object.remove("header").is_none() {
        return Err(DynamicJepaError::validation(
            "canonical_payload_hash.header",
            "record has no top-level header field",
            "add DjRecordHeader as the top-level header field",
        ));
    }
    let bytes = bincode::serialize(&Value::Object(object.clone())).map_err(|err| {
        DynamicJepaError::validation(
            "canonical_payload_hash.bincode",
            format!("bincode serialize canonical payload failed: {err}"),
            "remove non-deterministic or unsupported values from the record payload",
        )
    })?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hasher.finalize().into())
}

pub fn encode_versioned_record<R: DynamicJepaRecord>(record: &R) -> DynamicJepaResult<Vec<u8>> {
    record.validate_record()?;
    let mut body = bincode::serialize(record).map_err(|err| {
        DynamicJepaError::codec(
            R::RECORD_VERSION,
            record.header().record_version,
            R::PAYLOAD_TYPE,
            format!("bincode serialize failed: {err}; inspect the record payload for unsupported fields"),
        )
    })?;
    let mut bytes = Vec::with_capacity(body.len() + 1);
    bytes.push(R::RECORD_VERSION);
    bytes.append(&mut body);
    Ok(bytes)
}

pub fn decode_versioned_record<R>(bytes: &[u8]) -> DynamicJepaResult<R>
where
    R: DynamicJepaRecord + DeserializeOwned,
{
    if bytes.is_empty() {
        return Err(DynamicJepaError::codec(
            R::RECORD_VERSION,
            0,
            R::PAYLOAD_TYPE,
            "payload is empty; expected [record_version][bincode payload]",
        ));
    }
    let actual_version = bytes[0];
    if actual_version != R::RECORD_VERSION {
        return Err(DynamicJepaError::codec(
            R::RECORD_VERSION,
            actual_version,
            R::PAYLOAD_TYPE,
            "no migration or fallback is supported; wipe the demo DB and rebuild records",
        ));
    }
    let record: R = bincode::deserialize(&bytes[1..]).map_err(|err| {
        DynamicJepaError::codec(
            R::RECORD_VERSION,
            actual_version,
            R::PAYLOAD_TYPE,
            format!("bincode deserialize failed: {err}; inspect bytes at the source of truth"),
        )
    })?;
    record.validate_record()?;
    Ok(record)
}

pub fn ensure_no_duplicates<'a, I>(values: I, field: &str) -> DynamicJepaResult<()>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(DynamicJepaError::validation(
                field,
                format!("duplicate value {value:?}"),
                "deduplicate the domain pack contract before registration",
            ));
        }
    }
    Ok(())
}

pub fn ensure_finite(value: f32, field: &str) -> DynamicJepaResult<()> {
    if !value.is_finite() {
        return Err(DynamicJepaError::validation(
            field,
            format!("value must be finite, got {value}"),
            "reject NaN and infinity before writing records",
        ));
    }
    Ok(())
}

pub fn ensure_uuid(uuid: uuid::Uuid, field: &str) -> DynamicJepaResult<()> {
    if uuid.is_nil() {
        return Err(DynamicJepaError::validation(
            field,
            "uuid must not be nil",
            "generate a real UUID at the writer boundary",
        ));
    }
    Ok(())
}

#[macro_export]
macro_rules! impl_dynamic_jepa_record {
    ($ty:ty, $version:expr, $payload_type:expr) => {
        impl $crate::dynamicjepa::validation::DynamicJepaRecord for $ty {
            const RECORD_VERSION: u8 = $version;
            const PAYLOAD_TYPE: &'static str = $payload_type;

            fn header(&self) -> &$crate::dynamicjepa::record_header::DjRecordHeader {
                &self.header
            }

            fn header_mut(&mut self) -> &mut $crate::dynamicjepa::record_header::DjRecordHeader {
                &mut self.header
            }
        }
    };
}
