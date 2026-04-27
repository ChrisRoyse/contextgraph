//! Training record persistence for the `export_training_corpus` MCP tool.
//!
//! All records are stored in `CF_TRAINING_RECORDS` using the canonical
//! `[version: u8][bincode-encoded TrainingRecord]` layout.
//!
//! # Version Handling
//!
//! The version byte is [`TRAINING_RECORD_VERSION`]. Deserialization rejects
//! mismatched versions with `CoreError::SerializationError` — no automatic
//! migration. Bump the constant and add a migration path when the struct
//! layout changes in a non-backwards-compatible way.
//!
//! # FAIL FAST
//!
//! RocksDB errors propagate via [`TeleologicalStoreError::rocksdb_op`] with
//! operation name, CF name, and key context. No silent fallbacks.

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::training::{TrainingRecord, TRAINING_RECORD_VERSION};
use rocksdb::{ColumnFamily, IteratorMode};
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::teleological::column_families::{CF_TOPIC_PROFILES, CF_TRAINING_RECORDS};

use super::store::RocksDbTeleologicalStore;
use super::types::TeleologicalStoreError;

impl RocksDbTeleologicalStore {
    /// Get the training_records CF handle (FAIL FAST on missing).
    #[inline]
    fn cf_training_records(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_TRAINING_RECORDS)
            .expect("CF_TRAINING_RECORDS must exist — database initialization failed")
    }

    /// Store (or replace) a training record keyed by memory UUID.
    ///
    /// Encoding: `[TRAINING_RECORD_VERSION: u8][bincode]`.
    /// This is idempotent — re-exports overwrite prior records for the same UUID.
    pub async fn store_training_record(&self, id: Uuid, record: &TrainingRecord) -> CoreResult<()> {
        let payload = encode_training_record(record)?;
        let cf = self.cf_training_records();
        let key = id.as_bytes();

        self.db.put_cf(cf, key, &payload).map_err(|e| {
            error!(
                id = %id,
                error = %e,
                "ROCKSDB ERROR: Failed to store training record"
            );
            TeleologicalStoreError::rocksdb_op(
                "put_training_record",
                CF_TRAINING_RECORDS,
                Some(id),
                e,
            )
        })?;

        debug!(id = %id, bytes = payload.len(), "Stored training record");
        Ok(())
    }

    /// Retrieve a training record by memory UUID.
    ///
    /// Returns `None` if no record is stored for this ID. Returns an error if
    /// the stored bytes have an unexpected version or fail bincode deserialization.
    pub async fn get_training_record(&self, id: Uuid) -> CoreResult<Option<TrainingRecord>> {
        let cf = self.cf_training_records();
        let key = id.as_bytes();

        match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => decode_training_record(&bytes).map(Some),
            Ok(None) => Ok(None),
            Err(e) => {
                error!(id = %id, error = %e, "ROCKSDB ERROR: Failed to read training record");
                Err(TeleologicalStoreError::rocksdb_op(
                    "get_training_record",
                    CF_TRAINING_RECORDS,
                    Some(id),
                    e,
                )
                .into())
            }
        }
    }

    /// Return all training-record UUIDs currently stored.
    ///
    /// Uses a CF iterator — O(n) over records. Safe to call from an async
    /// context: all I/O happens in one synchronous sweep with no await points.
    /// For very large corpora, consider `count_training_records` as a cheaper
    /// existence check.
    pub async fn list_training_record_ids(&self) -> CoreResult<Vec<Uuid>> {
        let cf = self.cf_training_records();
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);
        let mut out = Vec::new();
        for item in iter {
            match item {
                Ok((key, _value)) => {
                    if key.len() != 16 {
                        warn!(
                            len = key.len(),
                            "Skipping training record with non-UUID key length"
                        );
                        continue;
                    }
                    let mut buf = [0u8; 16];
                    buf.copy_from_slice(&key);
                    out.push(Uuid::from_bytes(buf));
                }
                Err(e) => {
                    error!(error = %e, "ROCKSDB ERROR: iteration failed in list_training_record_ids");
                    return Err(TeleologicalStoreError::rocksdb_op(
                        "iterate_training_records",
                        CF_TRAINING_RECORDS,
                        None,
                        e,
                    )
                    .into());
                }
            }
        }
        Ok(out)
    }

    /// Count how many training records are currently stored.
    pub async fn count_training_records(&self) -> CoreResult<usize> {
        let cf = self.cf_training_records();
        Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
    }

    /// Delete a training record. Returns `true` if something was deleted.
    pub async fn delete_training_record(&self, id: Uuid) -> CoreResult<bool> {
        let cf = self.cf_training_records();
        let key = id.as_bytes();

        let existed = matches!(self.db.get_cf(cf, key), Ok(Some(_)));
        if !existed {
            return Ok(false);
        }

        self.db.delete_cf(cf, key).map_err(|e| {
            error!(id = %id, error = %e, "ROCKSDB ERROR: Failed to delete training record");
            TeleologicalStoreError::rocksdb_op(
                "delete_training_record",
                CF_TRAINING_RECORDS,
                Some(id),
                e,
            )
        })?;

        Ok(true)
    }

    /// Clear all training records. Used only by the MCP tool on explicit
    /// `clearExisting=true` request or by tests. Returns the number of records
    /// deleted.
    pub async fn clear_all_training_records(&self) -> CoreResult<usize> {
        let ids = self.list_training_record_ids().await?;
        let cf = self.cf_training_records();
        for id in &ids {
            self.db.delete_cf(cf, id.as_bytes()).map_err(|e| {
                error!(id = %id, error = %e, "ROCKSDB ERROR: Failed to delete training record during clear");
                TeleologicalStoreError::rocksdb_op(
                    "delete_training_record",
                    CF_TRAINING_RECORDS,
                    Some(*id),
                    e,
                )
            })?;
        }
        Ok(ids.len())
    }

    /// Batch-fetch a page of training records.
    ///
    /// Returns `Vec<Option<TrainingRecord>>` with the same length as `ids`
    /// (indices align). Missing or undecodable entries return `None` in their
    /// slot; the only errors bubbled up are RocksDB-level read failures.
    ///
    /// This is ~30× faster than sequential `get_training_record` calls over
    /// the same page (per lesson 6 in `tasks/lessons.md`).
    pub async fn multi_get_training_records(
        &self,
        ids: &[Uuid],
    ) -> CoreResult<Vec<Option<TrainingRecord>>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let cf = self.cf_training_records();
        let key_bufs: Vec<[u8; 16]> = ids.iter().map(|id| *id.as_bytes()).collect();
        let keys = key_bufs.iter().map(|k| (cf, k.as_slice()));
        let results = self.db.multi_get_cf(keys);

        let mut out = Vec::with_capacity(ids.len());
        for (idx, res) in results.into_iter().enumerate() {
            match res {
                Ok(Some(bytes)) => match decode_training_record(&bytes) {
                    Ok(r) => out.push(Some(r)),
                    Err(e) => {
                        warn!(id = %ids[idx], error = %e, "multi_get: decode failed, slot=None");
                        out.push(None);
                    }
                },
                Ok(None) => out.push(None),
                Err(e) => {
                    error!(
                        id = %ids[idx],
                        error = %e,
                        "ROCKSDB ERROR: multi_get_cf failure on training record"
                    );
                    return Err(TeleologicalStoreError::rocksdb_op(
                        "multi_get_training_record",
                        CF_TRAINING_RECORDS,
                        Some(ids[idx]),
                        e,
                    )
                    .into());
                }
            }
        }
        Ok(out)
    }

    /// Read the per-memory 14D topic profile from `CF_TOPIC_PROFILES`.
    ///
    /// Returns `Ok(None)` when no row exists for this memory (use the fallback
    /// path in `topic_profile_or_fallback`). Rejects malformed rows (neither
    /// 52 nor a multiple of 4 bytes) with a `CoreError::SerializationError`.
    pub async fn get_topic_profile(&self, id: Uuid) -> CoreResult<Option<[f32; 14]>> {
        let cf = self
            .db
            .cf_handle(CF_TOPIC_PROFILES)
            .expect("CF_TOPIC_PROFILES must exist — database initialization failed");
        match self.db.get_cf(cf, id.as_bytes()) {
            Ok(Some(bytes)) => {
                if bytes.len() != 56 {
                    return Err(CoreError::SerializationError(format!(
                        "CF_TOPIC_PROFILES row for {} has {} bytes; expected 56 (14 × f32 post-E14)",
                        id,
                        bytes.len()
                    )));
                }
                let mut out = [0f32; 14];
                for (i, chunk) in bytes.chunks_exact(4).enumerate() {
                    let arr: [u8; 4] = chunk.try_into().expect("chunk_exact(4) guarantees 4 bytes");
                    out[i] = f32::from_le_bytes(arr);
                }
                Ok(Some(out))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(TeleologicalStoreError::rocksdb_op(
                "get_topic_profile",
                CF_TOPIC_PROFILES,
                Some(id),
                e,
            )
            .into()),
        }
    }
}

// ============================================================================
// Encoding / Decoding
// ============================================================================

/// Encode a TrainingRecord with version byte prefix.
///
/// Exposed publicly so downstream exporters (Phase 6 CLI) can emit bytes that
/// are byte-for-byte identical to what gets stored in `CF_TRAINING_RECORDS`.
pub fn encode_training_record(record: &TrainingRecord) -> CoreResult<Vec<u8>> {
    let mut bytes = bincode::serialize(record).map_err(|e| {
        CoreError::SerializationError(format!("bincode serialize TrainingRecord: {}", e))
    })?;
    let mut out = Vec::with_capacity(1 + bytes.len());
    out.push(TRAINING_RECORD_VERSION);
    out.append(&mut bytes);
    Ok(out)
}

/// Decode a TrainingRecord, rejecting version mismatches.
///
/// Exposed publicly so downstream reloaders (Phase 6 CLI tests + consumers)
/// can round-trip the exact wire format.
pub fn decode_training_record(bytes: &[u8]) -> CoreResult<TrainingRecord> {
    if bytes.is_empty() {
        return Err(CoreError::SerializationError(
            "training record payload is empty (missing version byte)".into(),
        ));
    }
    let version = bytes[0];
    if version != TRAINING_RECORD_VERSION {
        return Err(CoreError::SerializationError(format!(
            "training record version mismatch: got {}, expected {}. \
             No automatic migration is supported — re-run export_training_corpus.",
            version, TRAINING_RECORD_VERSION
        )));
    }
    bincode::deserialize(&bytes[1..]).map_err(|e| {
        CoreError::SerializationError(format!("bincode deserialize TrainingRecord: {}", e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::teleological::synergy_matrix::SynergyMatrix;
    use context_graph_core::teleological::types::NUM_EMBEDDERS;
    use context_graph_core::training::{
        compute_cross_correlations, compute_group_alignments, NUM_CROSS_CORRELATIONS,
    };

    fn mock_record() -> TrainingRecord {
        let mut profile = [0.0f32; NUM_EMBEDDERS];
        profile[0] = 0.9;
        let synergy = SynergyMatrix::with_base_synergies();
        let cc = compute_cross_correlations(&profile, &synergy);
        let groups = compute_group_alignments(&profile);
        TrainingRecord {
            memory_id: Uuid::new_v4(),
            content: "round-trip test".into(),
            importance: 0.42,
            created_at: chrono::Utc::now(),
            session_id: None,
            source_type: None,
            source_path: None,
            content_hash: None,
            e1_semantic: vec![0.1, 0.2],
            e2_temporal_recent: Vec::new(),
            e3_temporal_periodic: Vec::new(),
            e4_temporal_positional: Vec::new(),
            e5_causal_cause: Vec::new(),
            e5_causal_effect: Vec::new(),
            e7_code: Vec::new(),
            e8_graph_source: Vec::new(),
            e8_graph_target: Vec::new(),
            e9_hdc: Vec::new(),
            e10_paraphrase: Vec::new(),
            e10_context: Vec::new(),
            e11_entity: Vec::new(),
            e14_bge_m3_dense: Vec::new(),
            e6_sparse_indices: Vec::new(),
            e6_sparse_values: Vec::new(),
            e13_splade_indices: Vec::new(),
            e13_splade_values: Vec::new(),
            e12_token_embeddings: Vec::new(),
            topic_profile: profile,
            cross_correlations: cc,
            group_alignments: groups,
            outgoing_edges: Vec::new(),
            incoming_edges: Vec::new(),
            knn_neighbors: (0..NUM_EMBEDDERS).map(|_| Vec::new()).collect(),
            causal_effects: Vec::new(),
            causal_causes: Vec::new(),
            topic_memberships: Vec::new(),
            temporal_labels: None,
            tucker_core: None,
            edge_type_distribution: [0u32; 8],
        }
    }

    #[test]
    fn encode_decode_roundtrip() {
        let record = mock_record();
        let bytes = encode_training_record(&record).unwrap();
        assert_eq!(bytes[0], TRAINING_RECORD_VERSION);
        let back = decode_training_record(&bytes).unwrap();
        assert_eq!(back.memory_id, record.memory_id);
        assert_eq!(back.content, record.content);
        assert_eq!(back.cross_correlations.len(), NUM_CROSS_CORRELATIONS);
    }

    #[test]
    fn decode_rejects_wrong_version() {
        let record = mock_record();
        let mut bytes = encode_training_record(&record).unwrap();
        bytes[0] = TRAINING_RECORD_VERSION.wrapping_add(1);
        let err = decode_training_record(&bytes).unwrap_err();
        assert!(
            format!("{}", err).contains("version mismatch"),
            "unexpected error: {}",
            err,
        );
    }

    #[test]
    fn decode_rejects_empty_payload() {
        let err = decode_training_record(&[]).unwrap_err();
        assert!(
            format!("{}", err).contains("empty"),
            "unexpected error: {}",
            err,
        );
    }
}
