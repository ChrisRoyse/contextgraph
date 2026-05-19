//! Typed-edge training-record persistence for the `export_typed_edges_corpus`
//! MCP tool (F1 of the typed-edges training-data factory).
//!
//! All records are stored in `CF_TYPED_EDGE_RECORDS` using the canonical
//! `[version: u8][bincode-encoded TypedEdgeTrainingRecord]` layout — encoded /
//! decoded via [`super::versioned_bincode`]. The 33-byte composite key is
//! shared with `CF_TYPED_EDGE_VALIDATIONS` and lives in
//! [`super::typed_edge_keys`].
//!
//! # Version Handling
//!
//! The version byte is [`TYPED_EDGE_RECORD_VERSION`]. Deserialization rejects
//! mismatched versions with `CoreError::SerializationError` — no automatic
//! migration. Bump the constant and add a migration path when the struct
//! layout changes in a non-backwards-compatible way.
//!
//! # FAIL FAST
//!
//! RocksDB errors propagate via [`TeleologicalStoreError::rocksdb_op`] with
//! operation name, CF name, and key context. No silent fallbacks.

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::typed_edge_export::{TypedEdgeTrainingRecord, TYPED_EDGE_RECORD_VERSION};
use rocksdb::{ColumnFamily, IteratorMode, WriteBatch};
use tracing::{debug, error};
use uuid::Uuid;

use crate::teleological::column_families::CF_TYPED_EDGE_RECORDS;

use super::store::RocksDbTeleologicalStore;
use super::typed_edge_keys::{
    parse_typed_edge_record_key, typed_edge_record_key, TYPED_EDGE_KEY_LEN,
};
use super::types::TeleologicalStoreError;
use super::versioned_bincode::{decode_versioned, encode_versioned};

impl RocksDbTeleologicalStore {
    /// Get the typed-edge records CF handle (FAIL FAST on missing).
    #[inline]
    pub(crate) fn cf_typed_edge_records(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_TYPED_EDGE_RECORDS)
            .expect("CF_TYPED_EDGE_RECORDS must exist — database initialization failed")
    }

    /// Store (or overwrite) a typed-edge training record keyed by
    /// `(source, target, edge_type)`.
    ///
    /// Idempotent — re-exports overwrite prior records for the same key.
    pub async fn store_typed_edge_record(
        &self,
        record: &TypedEdgeTrainingRecord,
    ) -> CoreResult<()> {
        let payload = encode_typed_edge_record(record)?;
        let key = typed_edge_record_key(
            record.source_memory_id,
            record.target_memory_id,
            record.edge_type,
        );
        let cf = self.cf_typed_edge_records();

        self.db.put_cf(cf, key, &payload).map_err(|e| {
            error!(
                src = %record.source_memory_id,
                tgt = %record.target_memory_id,
                et = record.edge_type,
                error = %e,
                "ROCKSDB ERROR: Failed to store typed-edge record"
            );
            TeleologicalStoreError::rocksdb_op(
                "put_typed_edge_record",
                CF_TYPED_EDGE_RECORDS,
                Some(record.source_memory_id),
                e,
            )
        })?;

        debug!(
            src = %record.source_memory_id,
            tgt = %record.target_memory_id,
            et = record.edge_type,
            bytes = payload.len(),
            "Stored typed-edge record"
        );
        Ok(())
    }

    /// Retrieve a typed-edge training record by composite key.
    ///
    /// Returns `None` if no record exists at the key. Returns an error for
    /// RocksDB I/O failure or decode failure (version mismatch, bad bincode).
    pub async fn get_typed_edge_record(
        &self,
        source: Uuid,
        target: Uuid,
        edge_type: u8,
    ) -> CoreResult<Option<TypedEdgeTrainingRecord>> {
        let key = typed_edge_record_key(source, target, edge_type);
        let cf = self.cf_typed_edge_records();

        match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => decode_typed_edge_record(&bytes).map(Some),
            Ok(None) => Ok(None),
            Err(e) => {
                error!(
                    src = %source,
                    tgt = %target,
                    et = edge_type,
                    error = %e,
                    "ROCKSDB ERROR: Failed to read typed-edge record"
                );
                Err(TeleologicalStoreError::rocksdb_op(
                    "get_typed_edge_record",
                    CF_TYPED_EDGE_RECORDS,
                    Some(source),
                    e,
                )
                .into())
            }
        }
    }

    /// Enumerate every stored `(source, target, edge_type)` composite key.
    ///
    /// Returns keys only (no payloads). Iteration mirrors RocksDB's natural
    /// key order. Malformed keys (length ≠ 33) trigger a structured
    /// `CoreError::SerializationError` to fail fast.
    pub async fn list_typed_edge_record_keys(&self) -> CoreResult<Vec<(Uuid, Uuid, u8)>> {
        let cf = self.cf_typed_edge_records();
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);
        let mut out = Vec::new();
        for item in iter {
            let (key, _value) = item.map_err(|e| {
                error!(
                    error = %e,
                    "ROCKSDB ERROR: iteration failed in list_typed_edge_record_keys"
                );
                TeleologicalStoreError::rocksdb_op(
                    "iterate_typed_edge_records",
                    CF_TYPED_EDGE_RECORDS,
                    None,
                    e,
                )
            })?;

            let parsed = parse_typed_edge_record_key(&key).ok_or_else(|| {
                CoreError::SerializationError(format!(
                    "CF_TYPED_EDGE_RECORDS key length {}, expected {}",
                    key.len(),
                    TYPED_EDGE_KEY_LEN
                ))
            })?;
            out.push(parsed);
        }
        Ok(out)
    }

    /// O(n) count of rows in `CF_TYPED_EDGE_RECORDS`.
    pub async fn count_typed_edge_records(&self) -> CoreResult<usize> {
        let cf = self.cf_typed_edge_records();
        Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
    }

    /// Delete every row from `CF_TYPED_EDGE_RECORDS` via a single WriteBatch.
    /// Returns the number of rows deleted.
    pub async fn clear_all_typed_edge_records(&self) -> CoreResult<usize> {
        let keys = self.list_typed_edge_record_keys().await?;
        let cf = self.cf_typed_edge_records();

        let mut batch = WriteBatch::default();
        for (src, tgt, et) in &keys {
            batch.delete_cf(cf, typed_edge_record_key(*src, *tgt, *et));
        }

        self.db.write(batch).map_err(|e| {
            error!(
                error = %e,
                "ROCKSDB ERROR: batch clear of typed-edge records failed"
            );
            TeleologicalStoreError::rocksdb_op(
                "clear_typed_edge_records",
                CF_TYPED_EDGE_RECORDS,
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

/// Encode a `TypedEdgeTrainingRecord` with the canonical
/// `[TYPED_EDGE_RECORD_VERSION][bincode]` layout.
pub fn encode_typed_edge_record(record: &TypedEdgeTrainingRecord) -> CoreResult<Vec<u8>> {
    encode_versioned(record, TYPED_EDGE_RECORD_VERSION, "TypedEdgeTrainingRecord")
}

/// Decode a `TypedEdgeTrainingRecord`, rejecting empty payloads and version
/// mismatches.
pub fn decode_typed_edge_record(bytes: &[u8]) -> CoreResult<TypedEdgeTrainingRecord> {
    decode_versioned(
        bytes,
        TYPED_EDGE_RECORD_VERSION,
        "TypedEdgeTrainingRecord",
        "re-run export_typed_edges_corpus",
    )
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::teleological::types::NUM_EMBEDDERS;
    use context_graph_core::typed_edge_export::{
        LLMValidationSummary, LLMVerdict, TypedEdgeTrainingRecord,
    };

    fn mock_record() -> TypedEdgeTrainingRecord {
        TypedEdgeTrainingRecord {
            source_memory_id: Uuid::new_v4(),
            target_memory_id: Uuid::new_v4(),
            edge_type: 3,
            edge_type_name: "causal_chain".into(),
            weight: 0.72,
            direction: 1,
            embedder_scores: {
                let mut s = [0f32; NUM_EMBEDDERS];
                s[0] = 0.8;
                s[4] = 0.72;
                s
            },
            agreement_count: 1,
            agreeing_embedders: 1 << 4,
            source_content: "A causes B.".into(),
            target_content: "B happens after A.".into(),
            source_session_id: Some("sess-1".into()),
            target_session_id: Some("sess-1".into()),
            source_type: Some("HookDescription".into()),
            target_type: Some("HookDescription".into()),
            mechanism_type: Some("direct".into()),
            llm_validation: Some(LLMValidationSummary {
                validated_at: chrono::Utc::now(),
                verdict: LLMVerdict::Valid,
                confidence: 0.9,
                rationale: "Direct cause-effect stated.".into(),
                validator_version: "deterministic-validator-v1@2026-05".into(),
            }),
            exported_at: chrono::Utc::now(),
            exporter_version: "typed_edge_export_v1".into(),
        }
    }

    #[test]
    fn encode_decode_roundtrip() {
        let record = mock_record();
        let bytes = encode_typed_edge_record(&record).unwrap();
        assert_eq!(bytes[0], TYPED_EDGE_RECORD_VERSION);
        let back = decode_typed_edge_record(&bytes).unwrap();
        assert_eq!(back, record);
    }

    #[test]
    fn decode_rejects_wrong_version() {
        let record = mock_record();
        let mut bytes = encode_typed_edge_record(&record).unwrap();
        bytes[0] = TYPED_EDGE_RECORD_VERSION.wrapping_add(1);
        let err = decode_typed_edge_record(&bytes).unwrap_err();
        assert!(
            format!("{}", err).contains("version mismatch"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn decode_rejects_empty_payload() {
        let err = decode_typed_edge_record(&[]).unwrap_err();
        assert!(
            format!("{}", err).contains("empty"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn key_layout_is_deterministic() {
        let s = Uuid::new_v4();
        let t = Uuid::new_v4();
        let k = typed_edge_record_key(s, t, 7);
        assert_eq!(k.len(), TYPED_EDGE_KEY_LEN);
        assert_eq!(&k[..16], s.as_bytes());
        assert_eq!(&k[16..32], t.as_bytes());
        assert_eq!(k[32], 7);
    }
}
