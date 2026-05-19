//! Persisted LLM verdict storage for typed edges (F4 of the typed-edges
//! training-data factory).
//!
//! All rows are stored in `CF_TYPED_EDGE_VALIDATIONS` using the canonical
//! `[version: u8][bincode-encoded LLMEdgeValidation]` layout — encoded /
//! decoded via [`super::versioned_bincode`]. The 33-byte composite key shape
//! is shared with `CF_TYPED_EDGE_RECORDS` via [`super::typed_edge_keys`].
//!
//! # Version Handling
//!
//! The version byte is [`LLM_EDGE_VALIDATION_VERSION`]. Deserialization
//! rejects mismatched versions with `CoreError::SerializationError` — no
//! automatic migration.
//!
//! # FAIL FAST
//!
//! RocksDB errors propagate via [`TeleologicalStoreError::rocksdb_op`] with
//! operation name, CF name, and key context. No silent fallbacks.

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::llm_edge_validation::{LLMEdgeValidation, LLM_EDGE_VALIDATION_VERSION};
use rocksdb::{ColumnFamily, IteratorMode, WriteBatch};
use tracing::{debug, error};
use uuid::Uuid;

use crate::teleological::column_families::CF_TYPED_EDGE_VALIDATIONS;

use super::store::RocksDbTeleologicalStore;
use super::typed_edge_keys::{
    parse_typed_edge_record_key, typed_edge_record_key, TYPED_EDGE_KEY_LEN,
};
use super::types::TeleologicalStoreError;
use super::versioned_bincode::{decode_versioned, encode_versioned};

impl RocksDbTeleologicalStore {
    /// Get the typed-edge validations CF handle (FAIL FAST on missing).
    #[inline]
    pub(crate) fn cf_typed_edge_validations(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_TYPED_EDGE_VALIDATIONS)
            .expect("CF_TYPED_EDGE_VALIDATIONS must exist — database initialization failed")
    }

    /// Store (or overwrite) an LLM validation row keyed by
    /// `(source, target, edge_type)`.
    ///
    /// Idempotent — re-validation overwrites prior verdicts for the same key.
    pub async fn store_llm_edge_validation(
        &self,
        source: Uuid,
        target: Uuid,
        edge_type: u8,
        validation: &LLMEdgeValidation,
    ) -> CoreResult<()> {
        let payload = encode_llm_edge_validation(validation)?;
        let key = typed_edge_record_key(source, target, edge_type);
        let cf = self.cf_typed_edge_validations();

        self.db.put_cf(cf, key, &payload).map_err(|e| {
            error!(
                src = %source,
                tgt = %target,
                et = edge_type,
                error = %e,
                "ROCKSDB ERROR: Failed to store LLM edge validation"
            );
            TeleologicalStoreError::rocksdb_op(
                "put_llm_edge_validation",
                CF_TYPED_EDGE_VALIDATIONS,
                Some(source),
                e,
            )
        })?;

        debug!(
            src = %source,
            tgt = %target,
            et = edge_type,
            bytes = payload.len(),
            "Stored LLM edge validation"
        );
        Ok(())
    }

    /// Retrieve an LLM validation by composite key.
    ///
    /// Returns `None` if no row exists at the key. Returns an error for
    /// RocksDB I/O failure or decode failure (version mismatch, bad bincode).
    pub async fn get_llm_edge_validation(
        &self,
        source: Uuid,
        target: Uuid,
        edge_type: u8,
    ) -> CoreResult<Option<LLMEdgeValidation>> {
        let key = typed_edge_record_key(source, target, edge_type);
        let cf = self.cf_typed_edge_validations();

        match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => decode_llm_edge_validation(&bytes).map(Some),
            Ok(None) => Ok(None),
            Err(e) => {
                error!(
                    src = %source,
                    tgt = %target,
                    et = edge_type,
                    error = %e,
                    "ROCKSDB ERROR: Failed to read LLM edge validation"
                );
                Err(TeleologicalStoreError::rocksdb_op(
                    "get_llm_edge_validation",
                    CF_TYPED_EDGE_VALIDATIONS,
                    Some(source),
                    e,
                )
                .into())
            }
        }
    }

    /// Enumerate every stored `(source, target, edge_type)` composite key.
    ///
    /// Returns keys only (no payloads). Malformed keys (length ≠ 33) trigger
    /// a structured `CoreError::SerializationError`.
    pub async fn list_llm_edge_validation_keys(&self) -> CoreResult<Vec<(Uuid, Uuid, u8)>> {
        let cf = self.cf_typed_edge_validations();
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);
        let mut out = Vec::new();
        for item in iter {
            let (key, _value) = item.map_err(|e| {
                error!(
                    error = %e,
                    "ROCKSDB ERROR: iteration failed in list_llm_edge_validation_keys"
                );
                TeleologicalStoreError::rocksdb_op(
                    "iterate_llm_edge_validations",
                    CF_TYPED_EDGE_VALIDATIONS,
                    None,
                    e,
                )
            })?;

            let parsed = parse_typed_edge_record_key(&key).ok_or_else(|| {
                CoreError::SerializationError(format!(
                    "CF_TYPED_EDGE_VALIDATIONS key length {}, expected {}",
                    key.len(),
                    TYPED_EDGE_KEY_LEN
                ))
            })?;
            out.push(parsed);
        }
        Ok(out)
    }

    /// O(n) count of rows in `CF_TYPED_EDGE_VALIDATIONS`.
    pub async fn count_llm_edge_validations(&self) -> CoreResult<usize> {
        let cf = self.cf_typed_edge_validations();
        Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
    }

    /// Delete every row from `CF_TYPED_EDGE_VALIDATIONS` via a single
    /// WriteBatch. Returns the number of rows deleted.
    pub async fn clear_all_llm_edge_validations(&self) -> CoreResult<usize> {
        let keys = self.list_llm_edge_validation_keys().await?;
        let cf = self.cf_typed_edge_validations();

        let mut batch = WriteBatch::default();
        for (src, tgt, et) in &keys {
            batch.delete_cf(cf, typed_edge_record_key(*src, *tgt, *et));
        }

        self.db.write(batch).map_err(|e| {
            error!(
                error = %e,
                "ROCKSDB ERROR: batch clear of LLM edge validations failed"
            );
            TeleologicalStoreError::rocksdb_op(
                "clear_llm_edge_validations",
                CF_TYPED_EDGE_VALIDATIONS,
                None,
                e,
            )
        })?;

        Ok(keys.len())
    }
}

// ============================================================================
// Encoding / Decoding
// ============================================================================

/// Encode an `LLMEdgeValidation` with the canonical
/// `[LLM_EDGE_VALIDATION_VERSION][bincode]` layout.
pub fn encode_llm_edge_validation(validation: &LLMEdgeValidation) -> CoreResult<Vec<u8>> {
    encode_versioned(validation, LLM_EDGE_VALIDATION_VERSION, "LLMEdgeValidation")
}

/// Decode an `LLMEdgeValidation`, rejecting empty payloads and version
/// mismatches.
pub fn decode_llm_edge_validation(bytes: &[u8]) -> CoreResult<LLMEdgeValidation> {
    decode_versioned(
        bytes,
        LLM_EDGE_VALIDATION_VERSION,
        "LLMEdgeValidation",
        "rerun typed-edge validation import",
    )
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::typed_edge_export::LLMVerdict;

    fn mock_validation() -> LLMEdgeValidation {
        LLMEdgeValidation {
            validated_at: chrono::Utc::now(),
            verdict: LLMVerdict::Valid,
            confidence: 0.92,
            rationale: "Direct cause-effect chain described.".into(),
            auto_derived_weight: 0.58,
            validator_version: "deterministic-validator-v1@2026-05".into(),
            prompt_hash: [7u8; 32],
        }
    }

    #[test]
    fn encode_decode_roundtrip() {
        let v = mock_validation();
        let bytes = encode_llm_edge_validation(&v).unwrap();
        assert_eq!(bytes[0], LLM_EDGE_VALIDATION_VERSION);
        let back = decode_llm_edge_validation(&bytes).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn decode_rejects_wrong_version() {
        let v = mock_validation();
        let mut bytes = encode_llm_edge_validation(&v).unwrap();
        bytes[0] = LLM_EDGE_VALIDATION_VERSION.wrapping_add(1);
        let err = decode_llm_edge_validation(&bytes).unwrap_err();
        assert!(
            format!("{}", err).contains("version mismatch"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn decode_rejects_empty_payload() {
        let err = decode_llm_edge_validation(&[]).unwrap_err();
        assert!(
            format!("{}", err).contains("empty"),
            "unexpected error: {}",
            err
        );
    }
}
