//! Inverted index operations for sparse vectors (E6 and E13).
//!
//! Contains methods for updating and removing fingerprints from:
//! - E13 SPLADE inverted index (learned expansion)
//! - E6 Sparse inverted index (exact keywords) - per e6upgrade.md
//!
//! Posting lists are stored sorted by UUID for O(log n) binary search.
//! Legacy unsorted lists are sorted on first access (one-time migration cost).

use rocksdb::WriteBatch;
use uuid::Uuid;

use context_graph_core::types::fingerprint::SparseVector;

use crate::teleological::column_families::{CF_E13_SPLADE_INVERTED, CF_E6_SPARSE_INVERTED};
use crate::teleological::schema::{e13_splade_inverted_key, e6_sparse_inverted_key};
use crate::teleological::serialization::{deserialize_memory_id_list, serialize_memory_id_list};

use super::store::RocksDbTeleologicalStore;
use super::types::{TeleologicalStoreError, TeleologicalStoreResult};

/// Deserialize a posting list, ensuring it is sorted for binary search.
/// Handles legacy unsorted lists by sorting them in-place. Returns whether
/// the list needed sorting (for write-back to fix the stored order).
fn deserialize_sorted_posting_list(
    data: &[u8],
) -> Result<(Vec<Uuid>, bool), context_graph_core::error::CoreError> {
    let mut ids = deserialize_memory_id_list(data)?;
    let was_unsorted = !ids.windows(2).all(|w| w[0] <= w[1]);
    if was_unsorted {
        ids.sort_unstable();
    }
    Ok((ids, was_unsorted))
}

// =============================================================================
// Generic inverted index helpers (Audit-14 STOR-M2 FIX)
//
// E6 and E13 inverted index update/remove were ~90 lines of copy-paste each.
// Extracted into two generic helpers parameterized by CF name and key function.
// =============================================================================

/// Generic helper: update an inverted index for a fingerprint.
fn update_inverted_index(
    db: &rocksdb::DB,
    cf: &rocksdb::ColumnFamily,
    batch: &mut WriteBatch,
    id: &Uuid,
    term_keys: &[[u8; 2]],
    cf_name: &'static str,
) -> TeleologicalStoreResult<()> {
    let results = db.multi_get_cf(term_keys.iter().map(|k| (cf, k.as_slice())));
    for (i, result) in results.into_iter().enumerate() {
        let term_key = &term_keys[i];
        let existing = result
            .map_err(|e| TeleologicalStoreError::rocksdb_op("multi_get", cf_name, None, e))?;
        let (mut ids, was_unsorted) = match existing {
            Some(data) => deserialize_sorted_posting_list(&data)?,
            None => (Vec::new(), false),
        };
        match ids.binary_search(id) {
            Ok(_) => {
                if was_unsorted {
                    let serialized = serialize_memory_id_list(&ids);
                    batch.put_cf(cf, term_key.as_slice(), &serialized);
                }
            }
            Err(pos) => {
                ids.insert(pos, *id);
                let serialized = serialize_memory_id_list(&ids);
                batch.put_cf(cf, term_key.as_slice(), &serialized);
            }
        }
    }
    Ok(())
}

/// Generic helper: remove a fingerprint from an inverted index.
fn remove_from_inverted_index(
    db: &rocksdb::DB,
    cf: &rocksdb::ColumnFamily,
    batch: &mut WriteBatch,
    id: &Uuid,
    term_keys: &[[u8; 2]],
    cf_name: &'static str,
) -> TeleologicalStoreResult<()> {
    let results = db.multi_get_cf(term_keys.iter().map(|k| (cf, k.as_slice())));
    for (i, result) in results.into_iter().enumerate() {
        let term_key = &term_keys[i];
        if let Some(data) =
            result.map_err(|e| TeleologicalStoreError::rocksdb_op("multi_get", cf_name, None, e))?
        {
            let (mut ids, _was_unsorted) = deserialize_sorted_posting_list(&data)?;
            if let Ok(pos) = ids.binary_search(id) {
                ids.remove(pos);
            } else {
                ids.retain(|&entry_id| entry_id != *id);
            }
            if ids.is_empty() {
                batch.delete_cf(cf, term_key.as_slice());
            } else {
                let serialized = serialize_memory_id_list(&ids);
                batch.put_cf(cf, term_key.as_slice(), &serialized);
            }
        }
    }
    Ok(())
}

impl RocksDbTeleologicalStore {
    /// Update the E13 SPLADE inverted index for a fingerprint.
    pub(crate) fn update_splade_inverted_index(
        &self,
        batch: &mut WriteBatch,
        id: &Uuid,
        sparse: &SparseVector,
    ) -> TeleologicalStoreResult<()> {
        let cf = self.get_cf(CF_E13_SPLADE_INVERTED)?;
        let term_keys: Vec<[u8; 2]> = sparse
            .indices
            .iter()
            .map(|&term_id| e13_splade_inverted_key(term_id))
            .collect();
        update_inverted_index(&self.db, cf, batch, id, &term_keys, CF_E13_SPLADE_INVERTED)
    }

    /// Remove a fingerprint's terms from the E13 SPLADE inverted index.
    pub(crate) fn remove_from_splade_inverted_index(
        &self,
        batch: &mut WriteBatch,
        id: &Uuid,
        sparse: &SparseVector,
    ) -> TeleologicalStoreResult<()> {
        let cf = self.get_cf(CF_E13_SPLADE_INVERTED)?;
        let term_keys: Vec<[u8; 2]> = sparse
            .indices
            .iter()
            .map(|&term_id| e13_splade_inverted_key(term_id))
            .collect();
        remove_from_inverted_index(&self.db, cf, batch, id, &term_keys, CF_E13_SPLADE_INVERTED)
    }

    // =========================================================================
    // E6 SPARSE INVERTED INDEX OPERATIONS (per e6upgrade.md)
    // =========================================================================

    /// Update the E6 Sparse inverted index for a fingerprint.
    pub(crate) fn update_e6_sparse_inverted_index(
        &self,
        batch: &mut WriteBatch,
        id: &Uuid,
        sparse: &SparseVector,
    ) -> TeleologicalStoreResult<()> {
        let cf = self.get_cf(CF_E6_SPARSE_INVERTED)?;
        let term_keys: Vec<[u8; 2]> = sparse
            .indices
            .iter()
            .map(|&term_id| e6_sparse_inverted_key(term_id))
            .collect();
        update_inverted_index(&self.db, cf, batch, id, &term_keys, CF_E6_SPARSE_INVERTED)
    }

    /// Remove a fingerprint's terms from the E6 sparse inverted index.
    pub(crate) fn remove_from_e6_sparse_inverted_index(
        &self,
        batch: &mut WriteBatch,
        id: &Uuid,
        sparse: &SparseVector,
    ) -> TeleologicalStoreResult<()> {
        let cf = self.get_cf(CF_E6_SPARSE_INVERTED)?;
        let term_keys: Vec<[u8; 2]> = sparse
            .indices
            .iter()
            .map(|&term_id| e6_sparse_inverted_key(term_id))
            .collect();
        remove_from_inverted_index(&self.db, cf, batch, id, &term_keys, CF_E6_SPARSE_INVERTED)
    }

    /// Recall candidates from E6 sparse inverted index.
    ///
    /// Returns memory IDs that share at least one term with the query sparse vector.
    /// Results are unsorted - use for Stage 1 candidate generation, not final ranking.
    ///
    /// # Arguments
    /// * `query_sparse` - The query's E6 sparse vector
    /// * `max_candidates` - Maximum number of candidates to return
    ///
    /// # Returns
    /// Vector of (memory_id, term_overlap_count) tuples for scoring
    pub fn e6_sparse_recall(
        &self,
        query_sparse: &SparseVector,
        max_candidates: usize,
    ) -> TeleologicalStoreResult<Vec<(Uuid, usize)>> {
        use std::collections::HashMap;

        let cf_inverted = self.get_cf(CF_E6_SPARSE_INVERTED)?;
        let mut candidate_counts: HashMap<Uuid, usize> = HashMap::new();

        // Audit-10 STOR-M3 FIX: Batch-read all posting lists via multi_get_cf
        // (was: per-term sequential db.get_cf). Matches E13 SPLADE pattern.
        let term_keys: Vec<[u8; 2]> = query_sparse
            .indices
            .iter()
            .map(|&term_id| e6_sparse_inverted_key(term_id))
            .collect();

        let results = self
            .db
            .multi_get_cf(term_keys.iter().map(|k| (cf_inverted, k.as_slice())));

        for result in results {
            let existing = result.map_err(|e| {
                TeleologicalStoreError::rocksdb_op("multi_get", CF_E6_SPARSE_INVERTED, None, e)
            })?;

            if let Some(data) = existing {
                let ids = deserialize_memory_id_list(&data)?;
                for id in ids {
                    // STOR-2 FIX: Skip soft-deleted fingerprints (ghost entries)
                    if self.is_soft_deleted(&id) {
                        continue;
                    }
                    *candidate_counts.entry(id).or_insert(0) += 1;
                }
            }
        }

        // Sort by term overlap count (descending) and take top candidates
        let mut candidates: Vec<(Uuid, usize)> = candidate_counts.into_iter().collect();
        candidates.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        candidates.truncate(max_candidates);

        Ok(candidates)
    }
}
