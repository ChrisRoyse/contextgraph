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
fn deserialize_sorted_posting_list(data: &[u8]) -> Result<(Vec<Uuid>, bool), context_graph_core::error::CoreError> {
    let mut ids = deserialize_memory_id_list(data)?;
    let was_unsorted = !ids.windows(2).all(|w| w[0] <= w[1]);
    if was_unsorted {
        ids.sort_unstable();
    }
    Ok((ids, was_unsorted))
}

impl RocksDbTeleologicalStore {
    /// Update the E13 SPLADE inverted index for a fingerprint.
    ///
    /// P4: Batch-reads all posting lists via multi_get_cf (was: per-term sequential read).
    /// This reduces lock hold time from O(terms x posting_list_size) to O(1 batch).
    pub(crate) fn update_splade_inverted_index(
        &self,
        batch: &mut WriteBatch,
        id: &Uuid,
        sparse: &SparseVector,
    ) -> TeleologicalStoreResult<()> {
        let cf_inverted = self.get_cf(CF_E13_SPLADE_INVERTED)?;

        // P4: Collect all term keys first
        let term_keys: Vec<[u8; 2]> = sparse.indices.iter()
            .map(|&term_id| e13_splade_inverted_key(term_id))
            .collect();

        // P4: Batch-read all posting lists in one RocksDB call
        let results = self.db.multi_get_cf(
            term_keys.iter().map(|k| (cf_inverted, k.as_slice()))
        );

        // Apply all mutations
        for (i, result) in results.into_iter().enumerate() {
            let term_key = &term_keys[i];
            let existing = result.map_err(|e| {
                TeleologicalStoreError::rocksdb_op("multi_get", CF_E13_SPLADE_INVERTED, None, e)
            })?;

            let (mut ids, was_unsorted) = match existing {
                Some(data) => deserialize_sorted_posting_list(&data)?,
                None => (Vec::new(), false),
            };

            match ids.binary_search(id) {
                Ok(_) => {
                    if was_unsorted {
                        let serialized = serialize_memory_id_list(&ids);
                        batch.put_cf(cf_inverted, term_key.as_slice(), &serialized);
                    }
                }
                Err(pos) => {
                    ids.insert(pos, *id);
                    let serialized = serialize_memory_id_list(&ids);
                    batch.put_cf(cf_inverted, term_key.as_slice(), &serialized);
                }
            }
        }

        Ok(())
    }

    /// Remove a fingerprint's terms from the E13 SPLADE inverted index.
    ///
    /// P4: Batch-reads all posting lists via multi_get_cf.
    pub(crate) fn remove_from_splade_inverted_index(
        &self,
        batch: &mut WriteBatch,
        id: &Uuid,
        sparse: &SparseVector,
    ) -> TeleologicalStoreResult<()> {
        let cf_inverted = self.get_cf(CF_E13_SPLADE_INVERTED)?;

        let term_keys: Vec<[u8; 2]> = sparse.indices.iter()
            .map(|&term_id| e13_splade_inverted_key(term_id))
            .collect();

        let results = self.db.multi_get_cf(
            term_keys.iter().map(|k| (cf_inverted, k.as_slice()))
        );

        for (i, result) in results.into_iter().enumerate() {
            let term_key = &term_keys[i];
            if let Some(data) = result.map_err(|e| {
                TeleologicalStoreError::rocksdb_op("multi_get", CF_E13_SPLADE_INVERTED, None, e)
            })? {
                let mut ids: Vec<Uuid> = deserialize_memory_id_list(&data)?;
                ids.retain(|&entry_id| entry_id != *id);

                if ids.is_empty() {
                    batch.delete_cf(cf_inverted, term_key.as_slice());
                } else {
                    let serialized = serialize_memory_id_list(&ids);
                    batch.put_cf(cf_inverted, term_key.as_slice(), &serialized);
                }
            }
        }

        Ok(())
    }

    // =========================================================================
    // E6 SPARSE INVERTED INDEX OPERATIONS (per e6upgrade.md)
    // =========================================================================

    /// Update the E6 Sparse inverted index for a fingerprint.
    ///
    /// P4: Batch-reads all posting lists via multi_get_cf.
    pub(crate) fn update_e6_sparse_inverted_index(
        &self,
        batch: &mut WriteBatch,
        id: &Uuid,
        sparse: &SparseVector,
    ) -> TeleologicalStoreResult<()> {
        let cf_inverted = self.get_cf(CF_E6_SPARSE_INVERTED)?;

        let term_keys: Vec<[u8; 2]> = sparse.indices.iter()
            .map(|&term_id| e6_sparse_inverted_key(term_id))
            .collect();

        let results = self.db.multi_get_cf(
            term_keys.iter().map(|k| (cf_inverted, k.as_slice()))
        );

        for (i, result) in results.into_iter().enumerate() {
            let term_key = &term_keys[i];
            let existing = result.map_err(|e| {
                TeleologicalStoreError::rocksdb_op("multi_get", CF_E6_SPARSE_INVERTED, None, e)
            })?;

            let (mut ids, was_unsorted) = match existing {
                Some(data) => deserialize_sorted_posting_list(&data)?,
                None => (Vec::new(), false),
            };

            match ids.binary_search(id) {
                Ok(_) => {
                    if was_unsorted {
                        let serialized = serialize_memory_id_list(&ids);
                        batch.put_cf(cf_inverted, term_key.as_slice(), &serialized);
                    }
                }
                Err(pos) => {
                    ids.insert(pos, *id);
                    let serialized = serialize_memory_id_list(&ids);
                    batch.put_cf(cf_inverted, term_key.as_slice(), &serialized);
                }
            }
        }

        Ok(())
    }

    /// Remove a fingerprint's terms from the E6 sparse inverted index.
    ///
    /// P4: Batch-reads all posting lists via multi_get_cf.
    pub(crate) fn remove_from_e6_sparse_inverted_index(
        &self,
        batch: &mut WriteBatch,
        id: &Uuid,
        sparse: &SparseVector,
    ) -> TeleologicalStoreResult<()> {
        let cf_inverted = self.get_cf(CF_E6_SPARSE_INVERTED)?;

        let term_keys: Vec<[u8; 2]> = sparse.indices.iter()
            .map(|&term_id| e6_sparse_inverted_key(term_id))
            .collect();

        let results = self.db.multi_get_cf(
            term_keys.iter().map(|k| (cf_inverted, k.as_slice()))
        );

        for (i, result) in results.into_iter().enumerate() {
            let term_key = &term_keys[i];
            if let Some(data) = result.map_err(|e| {
                TeleologicalStoreError::rocksdb_op("multi_get", CF_E6_SPARSE_INVERTED, None, e)
            })? {
                let mut ids: Vec<Uuid> = deserialize_memory_id_list(&data)?;
                ids.retain(|&entry_id| entry_id != *id);

                if ids.is_empty() {
                    batch.delete_cf(cf_inverted, term_key.as_slice());
                } else {
                    let serialized = serialize_memory_id_list(&ids);
                    batch.put_cf(cf_inverted, term_key.as_slice(), &serialized);
                }
            }
        }

        Ok(())
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

        // For each query term, fetch the posting list and count matches
        // STOR-2 FIX: Filter out soft-deleted IDs from posting lists
        for &term_id in &query_sparse.indices {
            let term_key = e6_sparse_inverted_key(term_id);

            let existing = self.db.get_cf(cf_inverted, term_key).map_err(|e| {
                TeleologicalStoreError::rocksdb_op("get", CF_E6_SPARSE_INVERTED, None, e)
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
        candidates.sort_by(|a, b| b.1.cmp(&a.1));
        candidates.truncate(max_candidates);

        Ok(candidates)
    }

    /// Get E6 term overlap score between a query and stored fingerprint.
    ///
    /// Returns the fraction of query terms that appear in the document.
    /// Used for E6 tie-breaking when E1 scores are close (Stage 3.5).
    ///
    /// # Arguments
    /// * `query_sparse` - The query's E6 sparse vector
    /// * `doc_sparse` - The document's E6 sparse vector
    ///
    /// # Returns
    /// Score in [0.0, 1.0] representing query term coverage
    pub fn e6_term_overlap_score(query_sparse: &SparseVector, doc_sparse: &SparseVector) -> f32 {
        if query_sparse.indices.is_empty() {
            return 0.0;
        }

        // Count shared terms using merge-join (both vectors are sorted by term_id)
        let mut shared = 0usize;
        let mut i = 0;
        let mut j = 0;

        while i < query_sparse.indices.len() && j < doc_sparse.indices.len() {
            match query_sparse.indices[i].cmp(&doc_sparse.indices[j]) {
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
                std::cmp::Ordering::Equal => {
                    shared += 1;
                    i += 1;
                    j += 1;
                }
            }
        }

        shared as f32 / query_sparse.indices.len() as f32
    }
}
