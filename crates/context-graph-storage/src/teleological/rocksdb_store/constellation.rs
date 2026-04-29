//! Constellation persistence for the Phase 2 compiler.
//!
//! All records land in `CF_CONSTELLATIONS` using the canonical
//! `[CONSTELLATION_VERSION: u8][bincode-encoded Constellation]` layout. A
//! secondary index at `CF_CONSTELLATION_BY_SELECTOR` maps
//! `[kind_byte: u8][sha256(canonical_selector)[..16]: 16B]` → constellation
//! UUID so "does a constellation exist for this selector?" is an O(1) point
//! lookup.
//!
//! # Atomicity
//!
//! `store_constellation` writes the primary record and the secondary index
//! key in a single `WriteBatch`. `delete_constellation` mirrors the pattern.
//!
//! # Version handling
//!
//! The version byte is [`CONSTELLATION_VERSION`]. Deserialization rejects
//! mismatched versions with `CoreError::SerializationError` — no automatic
//! migration.
//!
//! # FAIL FAST
//!
//! CF handles use `.expect(...)`; RocksDB errors propagate via
//! [`TeleologicalStoreError::rocksdb_op`] with operation + CF + key context.

use context_graph_core::constellation::{
    Constellation, ConstellationSelector, CONSTELLATION_VERSION,
};
use context_graph_core::error::{CoreError, CoreResult};
use rocksdb::{ColumnFamily, IteratorMode, WriteBatch};
use sha2::{Digest, Sha256};
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::teleological::column_families::{CF_CONSTELLATIONS, CF_CONSTELLATION_BY_SELECTOR};

use super::store::RocksDbTeleologicalStore;
use super::types::TeleologicalStoreError;

/// Key length of the secondary index: `[kind: u8][hash16: 16B]`.
const SELECTOR_INDEX_KEY_LEN: usize = 1 + 16;

impl RocksDbTeleologicalStore {
    #[inline]
    fn cf_constellations(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONSTELLATIONS)
            .expect("CF_CONSTELLATIONS must exist — database initialization failed")
    }

    #[inline]
    fn cf_constellation_by_selector(&self) -> &ColumnFamily {
        self.db
            .cf_handle(CF_CONSTELLATION_BY_SELECTOR)
            .expect("CF_CONSTELLATION_BY_SELECTOR must exist — database initialization failed")
    }

    /// Persist a constellation and its selector-index entry atomically.
    ///
    /// The primary record is keyed by `constellation.id`. The secondary index
    /// entry is keyed by `[selector_kind_byte][sha256(canonical_selector)[..16]]`
    /// and maps to the same UUID.
    ///
    /// Re-stores are idempotent. When the selector's canonical form already
    /// maps to a DIFFERENT constellation UUID (the rebuild-with-new-id case
    /// triggered by `compile_constellation(..., rebuildIfExists=true)`), the
    /// old primary row is deleted in the same `WriteBatch` so the previous
    /// record does not become an unreachable orphan. All three operations —
    /// delete-old-primary, put-new-primary, put-new-index — are atomic.
    pub async fn store_constellation(&self, c: &Constellation) -> CoreResult<()> {
        let payload = encode_constellation(c)?;
        let primary_cf = self.cf_constellations();
        let index_cf = self.cf_constellation_by_selector();
        let index_key = selector_index_key(&c.selector);
        let new_id_bytes = *c.id.as_bytes();

        // Probe the current index pointer. If it already points at a different
        // UUID, that prior primary row is about to become unreachable — delete
        // it in the same WriteBatch to preserve the invariant "at most one
        // primary row per selector". A pointer equal to our new UUID (re-store
        // of the same record) or no prior entry at all needs no cleanup.
        let prior_id: Option<[u8; 16]> = match self.db.get_cf(index_cf, index_key) {
            Ok(Some(bytes)) if bytes.len() == 16 && bytes.as_slice() != new_id_bytes => {
                let mut buf = [0u8; 16];
                buf.copy_from_slice(&bytes);
                Some(buf)
            }
            Ok(_) => None,
            Err(e) => {
                error!(
                    id = %c.id,
                    error = %e,
                    "ROCKSDB ERROR: Failed to probe selector index before store"
                );
                return Err(TeleologicalStoreError::rocksdb_op(
                    "store_constellation.probe_index",
                    CF_CONSTELLATION_BY_SELECTOR,
                    Some(c.id),
                    e,
                )
                .into());
            }
        };

        let mut batch = WriteBatch::default();
        if let Some(old) = prior_id.as_ref() {
            batch.delete_cf(primary_cf, old);
        }
        batch.put_cf(primary_cf, c.id.as_bytes(), &payload);
        batch.put_cf(index_cf, index_key, c.id.as_bytes());

        self.db.write(batch).map_err(|e| {
            error!(
                id = %c.id,
                error = %e,
                "ROCKSDB ERROR: Failed to store constellation (atomic batch)"
            );
            TeleologicalStoreError::rocksdb_op(
                "store_constellation",
                CF_CONSTELLATIONS,
                Some(c.id),
                e,
            )
        })?;

        if let Some(old) = prior_id {
            debug!(
                new_id = %c.id,
                old_id = %Uuid::from_bytes(old),
                members = c.member_count,
                "Replaced prior constellation for selector (atomic swap)"
            );
        } else {
            debug!(
                id = %c.id,
                members = c.member_count,
                bytes = payload.len(),
                "Stored constellation + selector index"
            );
        }
        Ok(())
    }

    /// Retrieve a constellation by id.
    pub async fn get_constellation(&self, id: Uuid) -> CoreResult<Option<Constellation>> {
        let cf = self.cf_constellations();
        match self.db.get_cf(cf, id.as_bytes()) {
            Ok(Some(bytes)) => decode_constellation(&bytes).map(Some),
            Ok(None) => Ok(None),
            Err(e) => {
                error!(id = %id, error = %e, "ROCKSDB ERROR: Failed to read constellation");
                Err(TeleologicalStoreError::rocksdb_op(
                    "get_constellation",
                    CF_CONSTELLATIONS,
                    Some(id),
                    e,
                )
                .into())
            }
        }
    }

    /// Enumerate every stored constellation UUID.
    pub async fn list_constellation_ids(&self) -> CoreResult<Vec<Uuid>> {
        let cf = self.cf_constellations();
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);
        let mut out = Vec::new();
        for item in iter {
            match item {
                Ok((key, _)) => {
                    if key.len() != 16 {
                        warn!(
                            len = key.len(),
                            "Skipping non-UUID key in CF_CONSTELLATIONS"
                        );
                        continue;
                    }
                    let mut buf = [0u8; 16];
                    buf.copy_from_slice(&key);
                    out.push(Uuid::from_bytes(buf));
                }
                Err(e) => {
                    error!(error = %e, "ROCKSDB ERROR: iteration failed in list_constellation_ids");
                    return Err(TeleologicalStoreError::rocksdb_op(
                        "iterate_constellations",
                        CF_CONSTELLATIONS,
                        None,
                        e,
                    )
                    .into());
                }
            }
        }
        Ok(out)
    }

    /// Count the number of stored constellations (O(n) scan).
    pub async fn count_constellations(&self) -> CoreResult<usize> {
        let cf = self.cf_constellations();
        Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
    }

    /// Delete a constellation and its selector-index entry. Returns `true`
    /// when a record was actually deleted.
    ///
    /// Looks up the selector from the primary record to know which index key
    /// to remove; if the primary record is missing we return `false` without
    /// touching the index.
    pub async fn delete_constellation(&self, id: Uuid) -> CoreResult<bool> {
        // Resolve the selector first (the only `.await` in this function) so
        // CF handles never cross an await boundary — they are `!Sync` and
        // `tokio::spawn` would reject the resulting future otherwise.
        let Some(record) = self.get_constellation(id).await? else {
            return Ok(false);
        };
        let index_key = selector_index_key(&record.selector);
        let id_bytes = *id.as_bytes();

        // Now we can acquire CF handles and do purely synchronous RocksDB I/O.
        let primary_cf = self.cf_constellations();
        let index_cf = self.cf_constellation_by_selector();

        let mut batch = WriteBatch::default();
        batch.delete_cf(primary_cf, id_bytes);
        // Only delete the secondary index when it still points at this UUID.
        // Avoids clobbering a newer constellation that happens to share the
        // selector.
        match self.db.get_cf(index_cf, index_key) {
            Ok(Some(bytes)) if bytes.len() == 16 && bytes == id_bytes => {
                batch.delete_cf(index_cf, index_key);
            }
            Ok(_) => {
                debug!(
                    id = %id,
                    "Selector index entry points to a different constellation; leaving it alone"
                );
            }
            Err(e) => {
                error!(id = %id, error = %e, "ROCKSDB ERROR: Failed to probe selector index during delete");
                return Err(TeleologicalStoreError::rocksdb_op(
                    "probe_selector_index",
                    CF_CONSTELLATION_BY_SELECTOR,
                    Some(id),
                    e,
                )
                .into());
            }
        }

        self.db.write(batch).map_err(|e| {
            error!(id = %id, error = %e, "ROCKSDB ERROR: Failed to delete constellation");
            TeleologicalStoreError::rocksdb_op(
                "delete_constellation",
                CF_CONSTELLATIONS,
                Some(id),
                e,
            )
        })?;
        Ok(true)
    }

    /// Look up a constellation UUID by selector via the secondary index.
    pub async fn find_constellation_by_selector(
        &self,
        selector: &ConstellationSelector,
    ) -> CoreResult<Option<Uuid>> {
        let index_cf = self.cf_constellation_by_selector();
        let key = selector_index_key(selector);
        match self.db.get_cf(index_cf, key) {
            Ok(Some(bytes)) => {
                if bytes.len() != 16 {
                    return Err(CoreError::SerializationError(format!(
                        "CF_CONSTELLATION_BY_SELECTOR value has unexpected length {}",
                        bytes.len()
                    )));
                }
                let mut buf = [0u8; 16];
                buf.copy_from_slice(&bytes);
                Ok(Some(Uuid::from_bytes(buf)))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(TeleologicalStoreError::rocksdb_op(
                "find_constellation_by_selector",
                CF_CONSTELLATION_BY_SELECTOR,
                None,
                e,
            )
            .into()),
        }
    }

    /// Batch-fetch a page of constellations by id. Output indices align with
    /// input indices; missing slots return `None`.
    pub async fn multi_get_constellations(
        &self,
        ids: &[Uuid],
    ) -> CoreResult<Vec<Option<Constellation>>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let cf = self.cf_constellations();
        let key_bufs: Vec<[u8; 16]> = ids.iter().map(|id| *id.as_bytes()).collect();
        let keys = key_bufs.iter().map(|k| (cf, k.as_slice()));
        let results = self.db.multi_get_cf(keys);

        let mut out = Vec::with_capacity(ids.len());
        for (idx, res) in results.into_iter().enumerate() {
            match res {
                Ok(Some(bytes)) => match decode_constellation(&bytes) {
                    Ok(c) => out.push(Some(c)),
                    Err(e) => {
                        warn!(id = %ids[idx], error = %e, "multi_get_constellations: decode failed");
                        out.push(None);
                    }
                },
                Ok(None) => out.push(None),
                Err(e) => {
                    error!(
                        id = %ids[idx],
                        error = %e,
                        "ROCKSDB ERROR: multi_get_cf failure on constellation"
                    );
                    return Err(TeleologicalStoreError::rocksdb_op(
                        "multi_get_constellation",
                        CF_CONSTELLATIONS,
                        Some(ids[idx]),
                        e,
                    )
                    .into());
                }
            }
        }
        Ok(out)
    }
}

// ==========================================================================
// Encoding / decoding / index-key helpers
// ==========================================================================

/// Encode a `Constellation` with the canonical `[version byte][bincode]`
/// layout.
fn encode_constellation(c: &Constellation) -> CoreResult<Vec<u8>> {
    let mut bytes = bincode::serialize(c).map_err(|e| {
        CoreError::SerializationError(format!("bincode serialize Constellation: {}", e))
    })?;
    let mut out = Vec::with_capacity(1 + bytes.len());
    out.push(CONSTELLATION_VERSION);
    out.append(&mut bytes);
    Ok(out)
}

/// Decode a `Constellation`, rejecting version mismatches and empty
/// payloads.
fn decode_constellation(bytes: &[u8]) -> CoreResult<Constellation> {
    if bytes.is_empty() {
        return Err(CoreError::SerializationError(
            "constellation payload is empty (missing version byte)".into(),
        ));
    }
    let version = bytes[0];
    if version != CONSTELLATION_VERSION {
        return Err(CoreError::SerializationError(format!(
            "constellation version mismatch: got {}, expected {}. \
             No automatic migration is supported — re-run compile_constellation.",
            version, CONSTELLATION_VERSION
        )));
    }
    bincode::deserialize(&bytes[1..]).map_err(|e| {
        CoreError::SerializationError(format!("bincode deserialize Constellation: {}", e))
    })
}

/// Build the composite secondary-index key for a selector.
///
/// Layout: `[kind_byte: u8][sha256(canonical_form)[..16]: 16B]` →
/// `SELECTOR_INDEX_KEY_LEN == 17` bytes.
pub(crate) fn selector_index_key(selector: &ConstellationSelector) -> [u8; SELECTOR_INDEX_KEY_LEN] {
    let canonical = selector.canonical_form();
    let digest = Sha256::digest(canonical.as_bytes());
    let mut key = [0u8; SELECTOR_INDEX_KEY_LEN];
    key[0] = selector.kind_byte();
    key[1..].copy_from_slice(&digest[..16]);
    key
}

// ==========================================================================
// Unit tests
// ==========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use context_graph_core::constellation::types::{
        EmbedderStats, VectorKind, CROSS_CORRELATION_CENTROID_DIM,
    };
    use context_graph_core::teleological::types::NUM_EMBEDDERS;

    fn mock_constellation() -> Constellation {
        let per_embedder: Vec<EmbedderStats> = (0..NUM_EMBEDDERS)
            .map(|i| EmbedderStats {
                embedder_index: i as u8,
                dimension: 128,
                vector_kind: VectorKind::Dense,
                centroid: vec![0.1; 128],
                sparse_top_terms: Vec::new(),
                mean_token_count: None,
                pooled_token_centroid: Vec::new(),
                mean_l2: 1.0,
                stddev_l2: 0.1,
                cosine_spread_p50: 0.95,
                cosine_spread_p95: 0.99,
                min_cosine: 0.9,
                max_cosine: 1.0,
                coverage: 1.0,
            })
            .collect();
        Constellation {
            id: Uuid::new_v4(),
            label: "test".into(),
            created_at: Utc::now(),
            selector: ConstellationSelector::Tag { tag: "unit".into() },
            member_ids: vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()],
            member_count: 3,
            per_embedder,
            topic_profile_centroid: [0.0; NUM_EMBEDDERS],
            group_alignment_centroid: [0.0; 6],
            cross_correlation_centroid: vec![0.0; CROSS_CORRELATION_CENTROID_DIM],
            coherence: 0.95,
            purity: None,
        }
    }

    #[test]
    fn encode_decode_roundtrip() {
        let c = mock_constellation();
        let bytes = encode_constellation(&c).unwrap();
        assert_eq!(bytes[0], CONSTELLATION_VERSION);
        let back = decode_constellation(&bytes).unwrap();
        assert_eq!(back.id, c.id);
        assert_eq!(back.label, c.label);
        assert_eq!(back.per_embedder.len(), NUM_EMBEDDERS);
        assert_eq!(back.coherence, c.coherence);
    }

    #[test]
    fn decode_rejects_empty() {
        let err = decode_constellation(&[]).unwrap_err();
        assert!(format!("{}", err).contains("empty"), "unexpected: {}", err);
    }

    #[test]
    fn decode_rejects_wrong_version() {
        let c = mock_constellation();
        let mut bytes = encode_constellation(&c).unwrap();
        bytes[0] = CONSTELLATION_VERSION.wrapping_add(1);
        let err = decode_constellation(&bytes).unwrap_err();
        assert!(
            format!("{}", err).contains("version mismatch"),
            "unexpected: {}",
            err
        );
    }

    #[test]
    fn selector_index_key_is_stable() {
        let s = ConstellationSelector::Topic {
            topic_id: "alpha".into(),
        };
        let k1 = selector_index_key(&s);
        let k2 = selector_index_key(&s);
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), SELECTOR_INDEX_KEY_LEN);
        assert_eq!(k1[0], 0); // Topic kind byte
    }

    #[test]
    fn selector_index_keys_distinct_by_kind() {
        let a = selector_index_key(&ConstellationSelector::Topic {
            topic_id: "x".into(),
        });
        let b = selector_index_key(&ConstellationSelector::Session {
            session_id: "x".into(),
        });
        assert_ne!(a, b, "different selector kinds must produce different keys");
        assert_eq!(a[0], 0);
        assert_eq!(b[0], 1);
    }
}
