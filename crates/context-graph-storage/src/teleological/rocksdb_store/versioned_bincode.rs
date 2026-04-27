//! Shared `[version: u8][bincode]` encode/decode helpers for typed-edge CFs.
//!
//! `CF_TYPED_EDGE_RECORDS` and `CF_TYPED_EDGE_VALIDATIONS` share the same wire
//! shape: one leading version byte, then a bincode-serialized payload. Both
//! CFs reject mismatched version bytes with a structured
//! [`CoreError::SerializationError`] carrying the expected version, the actual
//! version, and the payload type name — no automatic migration.
//!
//! The two CRUD modules (`typed_edge_export`, `llm_validation`) wrap the two
//! functions here with thin type-specific encode/decode pairs so callers keep
//! a stable public API but the version-handling logic lives in one place.

use context_graph_core::error::{CoreError, CoreResult};
use serde::{de::DeserializeOwned, Serialize};

/// Encode `value` with a leading `version` byte followed by a bincode payload.
///
/// Error messages include `type_name` so a failure is self-describing — the
/// caller does not need to wrap the return value with extra context.
pub(super) fn encode_versioned<T: Serialize>(
    value: &T,
    version: u8,
    type_name: &'static str,
) -> CoreResult<Vec<u8>> {
    let mut body = bincode::serialize(value).map_err(|e| {
        CoreError::SerializationError(format!("bincode serialize {}: {}", type_name, e))
    })?;
    let mut out = Vec::with_capacity(1 + body.len());
    out.push(version);
    out.append(&mut body);
    Ok(out)
}

/// Decode a `[version: u8][bincode]` payload for `T`, rejecting empty slices
/// and version mismatches with a structured `SerializationError`.
///
/// `retry_hint` is appended to the version-mismatch error so operators know
/// which tool to re-run (e.g. "re-run export_typed_edges_corpus"). The `T` is
/// named in every error via `type_name` for debuggability.
pub(super) fn decode_versioned<T: DeserializeOwned>(
    bytes: &[u8],
    expected_version: u8,
    type_name: &'static str,
    retry_hint: &'static str,
) -> CoreResult<T> {
    if bytes.is_empty() {
        return Err(CoreError::SerializationError(format!(
            "{} payload is empty (missing version byte)",
            type_name
        )));
    }
    let version = bytes[0];
    if version != expected_version {
        return Err(CoreError::SerializationError(format!(
            "{} version mismatch: got {}, expected {}. \
             No automatic migration is supported — {}.",
            type_name, version, expected_version, retry_hint
        )));
    }
    bincode::deserialize(&bytes[1..]).map_err(|e| {
        CoreError::SerializationError(format!("bincode deserialize {}: {}", type_name, e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Sample {
        a: u32,
        b: String,
    }

    #[test]
    fn round_trips_payload() {
        let s = Sample {
            a: 7,
            b: "hi".into(),
        };
        let bytes = encode_versioned(&s, 1, "Sample").unwrap();
        assert_eq!(bytes[0], 1);
        let back: Sample = decode_versioned(&bytes, 1, "Sample", "re-run").unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn rejects_wrong_version() {
        let s = Sample {
            a: 1,
            b: "x".into(),
        };
        let mut bytes = encode_versioned(&s, 1, "Sample").unwrap();
        bytes[0] = 2;
        let err = decode_versioned::<Sample>(&bytes, 1, "Sample", "re-run").unwrap_err();
        assert!(err.to_string().contains("version mismatch"));
        assert!(err.to_string().contains("Sample"));
    }

    #[test]
    fn rejects_empty_payload() {
        let err = decode_versioned::<Sample>(&[], 1, "Sample", "re-run").unwrap_err();
        assert!(err.to_string().contains("empty"));
        assert!(err.to_string().contains("Sample"));
    }
}
