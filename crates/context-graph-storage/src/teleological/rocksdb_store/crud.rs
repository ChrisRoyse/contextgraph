//! CRUD operations for TeleologicalMemoryStore trait.
//!
//! Contains store, retrieve, update, and delete implementations.
//!
//! # Concurrency
//!
//! Individual CRUD operations use sync RocksDB calls directly since they're
//! typically fast single-key operations (< 1ms). The main performance benefit
//! of spawn_blocking comes from batch/iteration operations in search.rs and
//! persistence.rs.

use rocksdb::WriteBatch;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::types::fingerprint::TeleologicalFingerprint;

use crate::teleological::column_families::{
    CF_E12_LATE_INTERACTION, CF_E13_SPLADE_INVERTED, CF_E1_MATRYOSHKA_128, CF_FINGERPRINTS,
    CF_SOURCE_METADATA, CF_TOPIC_PROFILES, QUANTIZED_EMBEDDER_CFS,
};
use crate::teleological::schema::{
    content_key, e12_late_interaction_key, e1_matryoshka_128_key, fingerprint_key,
    source_metadata_key, topic_profile_key,
};
use crate::teleological::serialization::deserialize_teleological_fingerprint;

use super::store::RocksDbTeleologicalStore;
use super::types::TeleologicalStoreError;

/// Key prefix for soft-delete markers persisted in CF_SYSTEM.
/// Format: "soft_deleted::{uuid}" -> i64 timestamp (8 bytes, big-endian Unix epoch millis)
pub(crate) const SOFT_DELETE_PREFIX: &str = "soft_deleted::";

/// Build the CF_SYSTEM key for a soft-delete marker.
#[inline]
pub(crate) fn soft_delete_key(id: &Uuid) -> String {
    format!("{}{}", SOFT_DELETE_PREFIX, id)
}

impl RocksDbTeleologicalStore {
    /// Store a fingerprint (internal async wrapper).
    ///
    /// # Errors
    ///
    /// Returns `CoreError::ValidationError` if the ID is soft-deleted (MED-13).
    /// Storing data for a soft-deleted ID would write invisible data that
    /// occupies storage but is never returned by queries.
    pub(crate) async fn store_async(
        &self,
        fingerprint: TeleologicalFingerprint,
    ) -> CoreResult<Uuid> {
        let id = fingerprint.id;
        debug!("Storing fingerprint {}", id);

        // MED-13 FIX: Reject stores for soft-deleted IDs. Writing data for a
        // soft-deleted ID creates invisible records that waste storage. The caller
        // must either hard-delete first or use a new ID.
        if self.is_soft_deleted(&id) {
            error!(
                id = %id,
                "Attempted to store fingerprint with soft-deleted ID — FAIL FAST"
            );
            return Err(CoreError::ValidationError {
                field: "id".to_string(),
                message: format!(
                    "Cannot store fingerprint {}: ID is soft-deleted. \
                     Hard-delete first or use a new ID.",
                    id
                ),
            });
        }

        // Store in RocksDB (primary storage) — new insert, count for IDF
        self.store_fingerprint_internal(&fingerprint, true)?;

        // Add to per-embedder indexes for O(log n) search
        // DAT-4 fix: If indexing fails, rollback the RocksDB write to prevent
        // inconsistency where data exists in RocksDB but not in HNSW indexes.
        //
        // DATA-3 FIX: store_fingerprint_internal writes to multiple CFs via WriteBatch
        // (CF_FINGERPRINTS, CF_E1_MATRYOSHKA_128, CF_E13_SPLADE_INVERTED, CF_E6_SPARSE_INVERTED,
        // CF_E12_LATE_INTERACTION, CF_TOPIC_PROFILES, CF_SOURCE_METADATA, plus quantized CFs).
        // Previously, rollback only deleted from CF_FINGERPRINTS, leaving orphaned data
        // in all other CFs. Now we use delete_async(id, false) for a proper hard-delete
        // that cleans all CFs consistently.
        if let Err(e) = self.add_to_indexes(&fingerprint) {
            error!(
                id = %id,
                error = %e,
                "HNSW index add failed after RocksDB store — rolling back all CFs via hard-delete"
            );
            // Rollback: hard-delete from ALL column families (not just CF_FINGERPRINTS)
            match self.delete_async(id, false).await {
                Ok(_) => {
                    debug!(
                        id = %id,
                        "Rollback successful: hard-deleted fingerprint from all CFs"
                    );
                }
                Err(rollback_err) => {
                    error!(
                        id = %id,
                        error = %rollback_err,
                        "CRITICAL: Rollback hard-delete also failed — manual cleanup required"
                    );
                }
            }
            return Err(CoreError::IndexError(e.to_string()));
        }

        Ok(id)
    }

    /// Retrieve a fingerprint (internal async wrapper).
    pub(crate) async fn retrieve_async(
        &self,
        id: Uuid,
    ) -> CoreResult<Option<TeleologicalFingerprint>> {
        debug!("Retrieving fingerprint {}", id);

        // Check soft-deleted
        if self.is_soft_deleted(&id) {
            return Ok(None);
        }

        let raw = self.get_fingerprint_raw(id)?;

        match raw {
            Some(data) => {
                let fp = deserialize_teleological_fingerprint(&data)?;
                Ok(Some(fp))
            }
            None => Ok(None),
        }
    }

    /// Update a fingerprint (internal async wrapper).
    ///
    /// STOR-7 NOTE: There is a brief transient inconsistency window between
    /// removing old inverted index terms and adding new ones. A concurrent reader
    /// may see the fingerprint missing from inverted indexes but still present in
    /// HNSW. This is self-healing on completion and is an accepted design trade-off
    /// vs holding the secondary_index_lock across the entire operation (which would
    /// reduce write concurrency).
    pub(crate) async fn update_async(
        &self,
        fingerprint: TeleologicalFingerprint,
    ) -> CoreResult<bool> {
        let id = fingerprint.id;
        debug!("Updating fingerprint {}", id);

        // Check if exists and capture old raw bytes for rollback
        let old_raw_data = match self.get_fingerprint_raw(id)? {
            Some(data) => data,
            None => return Ok(false),
        };
        let old_fp = deserialize_teleological_fingerprint(&old_raw_data)?;

        // Remove old terms from inverted indexes first
        // STG-04 FIX: Hold secondary_index_lock for the remove batch to prevent
        // concurrent store_fingerprint_internal from reading stale posting lists.
        {
            let _index_guard = self.secondary_index_lock.lock();
            let mut batch = WriteBatch::default();

            // Remove from E13 SPLADE inverted index
            self.remove_from_splade_inverted_index(&mut batch, &id, &old_fp.semantic.e13_splade)?;

            // Remove from E6 sparse inverted index (if present)
            // Per e6upgrade.md: must remove old terms before adding new ones
            if let Some(old_e6_sparse) = &old_fp.e6_sparse {
                self.remove_from_e6_sparse_inverted_index(&mut batch, &id, old_e6_sparse)?;
            }

            self.db.write(batch).map_err(|e| {
                TeleologicalStoreError::rocksdb_op(
                    "write_batch",
                    CF_E13_SPLADE_INVERTED,
                    Some(id),
                    e,
                )
            })?;
            // Lock released here via drop(_index_guard)
        }

        // Remove from per-embedder indexes (will be re-added with updated vectors)
        self.remove_from_indexes(id)
            .map_err(|e| CoreError::IndexError(e.to_string()))?;

        // Store updated fingerprint in RocksDB — update, NOT a new doc
        self.store_fingerprint_internal(&fingerprint, false)?;

        // CRIT-3 FIX: If add_to_indexes fails after store, rollback uses the
        // captured old_raw_data (from BEFORE the write), not a re-read from
        // RocksDB which would return the NEW data.
        if let Err(e) = self.add_to_indexes(&fingerprint) {
            error!(
                id = %id,
                error = %e,
                "HNSW index add failed during update — rolling back to old fingerprint"
            );

            // M2 FIX: Remove the NEW fingerprint's inverted index terms BEFORE
            // restoring the old fingerprint. store_fingerprint_internal(&fingerprint, false)
            // above already wrote the new E6/E13 terms into the inverted indexes.
            // Without this cleanup, rollback via store_fingerprint_internal(&old_fp)
            // re-adds old terms but leaves new terms orphaned in the index, causing
            // false-positive search hits for queries matching the new (rolled-back) terms.
            {
                let _index_guard = self.secondary_index_lock.lock();
                let mut cleanup_batch = WriteBatch::default();

                // Remove new E13 SPLADE terms
                if let Err(ce) = self.remove_from_splade_inverted_index(
                    &mut cleanup_batch,
                    &id,
                    &fingerprint.semantic.e13_splade,
                ) {
                    warn!(
                        id = %id,
                        error = %ce,
                        "Rollback: failed to remove new E13 terms (orphaned entries will be filtered at read time)"
                    );
                }

                // Remove new E6 sparse terms (if present)
                if let Some(ref new_e6_sparse) = fingerprint.e6_sparse {
                    if let Err(ce) = self.remove_from_e6_sparse_inverted_index(
                        &mut cleanup_batch,
                        &id,
                        new_e6_sparse,
                    ) {
                        warn!(
                            id = %id,
                            error = %ce,
                            "Rollback: failed to remove new E6 terms (orphaned entries will be filtered at read time)"
                        );
                    }
                }

                if let Err(ce) = self.db.write(cleanup_batch) {
                    warn!(
                        id = %id,
                        error = %ce,
                        "Rollback: failed to commit inverted index cleanup batch"
                    );
                }
            }

            // Restore old fingerprint from captured bytes (NOT from RocksDB) — rollback, not new
            if let Err(re) = self.store_fingerprint_internal(&old_fp, false) {
                error!(
                    id = %id,
                    error = %re,
                    "CRITICAL: Rollback store_fingerprint_internal failed — fingerprint {} corrupted, manual cleanup required",
                    id
                );
            }
            if let Err(re) = self.add_to_indexes(&old_fp) {
                error!(
                    id = %id,
                    error = %re,
                    "CRITICAL: Rollback add_to_indexes failed — fingerprint {} in RocksDB but missing from HNSW",
                    id
                );
            }
            return Err(CoreError::IndexError(e.to_string()));
        }

        Ok(true)
    }

    /// Delete a fingerprint (internal async wrapper).
    pub(crate) async fn delete_async(&self, id: Uuid, soft: bool) -> CoreResult<bool> {
        debug!("Deleting fingerprint {} (soft={})", id, soft);

        let existing = self.get_fingerprint_raw(id)?;
        if existing.is_none() {
            return Ok(false);
        }

        if soft {
            // Soft delete: mark as deleted in memory AND persist to RocksDB
            // P5: DashMap - no write lock needed, lock-free insert
            let now_millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before UNIX epoch")
                .as_millis() as i64;
            self.soft_deleted.insert(id, now_millis);

            // SEC-06-FIX: Persist soft-delete marker to CF_SYSTEM so it survives restart
            // Value is i64 timestamp (8 bytes, big-endian) for GC retention checks
            let cf_system = self
                .get_cf(crate::column_families::cf_names::SYSTEM)
                .map_err(|e| CoreError::StorageError(format!("CF_SYSTEM not found: {e}")))?;
            let sd_key = soft_delete_key(&id);
            self.db
                .put_cf(cf_system, sd_key.as_bytes(), now_millis.to_be_bytes())
                .map_err(|e| {
                    CoreError::StorageError(format!(
                        "Failed to persist soft-delete marker for {}: {}",
                        id, e
                    ))
                })?;

            // STOR-1 FIX: Decrement total_doc_count for IDF accuracy.
            // Soft-deleted docs are excluded from search, so they must not inflate
            // the IDF denominator between restarts.
            // STOR-H1 FIX: Use fetch_update with checked subtraction to prevent
            // underflow wrapping to usize::MAX, which corrupts all BM25-IDF scoring.
            if self
                .total_doc_count
                .fetch_update(
                    std::sync::atomic::Ordering::Relaxed,
                    std::sync::atomic::Ordering::Relaxed,
                    |c| if c > 0 { Some(c - 1) } else { None },
                )
                .is_err()
            {
                error!(
                    id = %id,
                    "E_STOR_DOC_COUNT_001: total_doc_count underflow prevented \
                     (was already 0) during soft-delete for fingerprint {}",
                    id
                );
            }

            // Invalidate count cache (soft delete changes the effective count)
            *self.fingerprint_count.write() = None;
        } else {
            // Hard delete: remove from all column families
            // M1 FIX: Check if this ID was already soft-deleted BEFORE the hard-delete
            // proceeds. If it was, total_doc_count was already decremented during
            // soft-delete, so we must NOT decrement again at the end of this block.
            let was_soft_deleted = self.is_soft_deleted(&id);

            // STG-04 FIX: Hold lock during inverted index read-modify-write
            let _index_guard = self.secondary_index_lock.lock();
            let key = fingerprint_key(&id);
            let mut batch = WriteBatch::default();

            // Try to deserialize the old fingerprint for inverted index cleanup.
            // If deserialization fails, we still delete the main record but skip
            // inverted index cleanup (orphaned entries are harmless - they just
            // point to a deleted ID that will be filtered on read).
            let old_fp = match deserialize_teleological_fingerprint(&existing.unwrap()) {
                Ok(fp) => Some(fp),
                Err(e) => {
                    warn!(
                        "Failed to deserialize fingerprint {} during delete, \
                         skipping inverted index cleanup: {}",
                        id, e
                    );
                    None
                }
            };

            // Remove from fingerprints
            let cf_fp = self.get_cf(CF_FINGERPRINTS)?;
            batch.delete_cf(cf_fp, key);

            // Remove from topic profiles
            let cf_pv = self.get_cf(CF_TOPIC_PROFILES)?;
            batch.delete_cf(cf_pv, topic_profile_key(&id));

            // Remove from e1_matryoshka_128
            let cf_mat = self.get_cf(CF_E1_MATRYOSHKA_128)?;
            batch.delete_cf(cf_mat, e1_matryoshka_128_key(&id));

            // Remove from inverted indexes only if we could deserialize the old fingerprint
            if let Some(ref fp) = old_fp {
                // Remove from E13 SPLADE inverted index
                self.remove_from_splade_inverted_index(&mut batch, &id, &fp.semantic.e13_splade)?;

                // Remove from E6 sparse inverted index (if present)
                // Per e6upgrade.md: clean up E6 terms on delete
                if let Some(e6_sparse) = &fp.e6_sparse {
                    self.remove_from_e6_sparse_inverted_index(&mut batch, &id, e6_sparse)?;
                }
            }

            // Remove content (TASK-CONTENT-009: cascade content deletion)
            let cf_content = self.cf_content();
            batch.delete_cf(cf_content, content_key(&id));

            // Remove E12 late interaction tokens (TASK-STORAGE-P2-001)
            let cf_e12 = self.get_cf(CF_E12_LATE_INTERACTION)?;
            batch.delete_cf(cf_e12, e12_late_interaction_key(&id));

            // DAT-7: Remove source metadata (was missing from hard-delete)
            let cf_sm = self.get_cf(CF_SOURCE_METADATA)?;
            batch.delete_cf(cf_sm, source_metadata_key(&id));

            // DAT-7: Remove quantized embedder data from all 13 emb_X CFs
            // Note: emb_X CFs are not populated in production (quantized write path not wired).
            // These deletes are no-ops but kept for forward compatibility when quantized storage is enabled.
            let qkey = fingerprint_key(&id);
            for &cf_name in QUANTIZED_EMBEDDER_CFS {
                if let Ok(cf) = self.get_cf(cf_name) {
                    batch.delete_cf(cf, qkey);
                }
            }

            // TODO: CF_FILE_INDEX cleanup requires reverse lookup (fingerprint_id -> file_path).
            // Currently orphaned entries are harmless (get_fingerprints_for_file returns stale UUIDs
            // that fail retrieval, effectively filtered). Full cleanup needs source_metadata read.

            // Remove from soft-deleted tracking (memory + persisted marker)
            // P5: DashMap - no write lock needed
            self.soft_deleted.remove(&id);
            let cf_system = self
                .get_cf(crate::column_families::cf_names::SYSTEM)
                .map_err(|e| CoreError::StorageError(format!("CF_SYSTEM not found: {e}")))?;
            let sd_key = soft_delete_key(&id);
            batch.delete_cf(cf_system, sd_key.as_bytes());

            // STG-04: Release inverted-index lock before write.
            drop(_index_guard);

            // FIX-M5: Commit RocksDB FIRST, then remove from HNSW indexes.
            // If RocksDB fails, nothing is lost (HNSW still has the entry = safe).
            // If HNSW fails after RocksDB commit, entry is orphaned in HNSW
            // (harmless — search returns non-existent ID, filtered in post-processing).
            self.db.write(batch).map_err(|e| {
                TeleologicalStoreError::rocksdb_op("delete_batch", CF_FINGERPRINTS, Some(id), e)
            })?;

            // Best-effort HNSW cleanup — log but don't fail if indexes can't be updated
            if let Err(e) = self.remove_from_indexes(id) {
                warn!(id = %id, error = %e, "Hard-delete: HNSW index removal failed (orphan will be filtered at search time)");
            }

            // Invalidate count cache
            *self.fingerprint_count.write() = None;

            // M1 FIX: Only decrement total_doc_count if this fingerprint was NOT
            // already soft-deleted. Soft-delete already decremented the counter
            // (see soft-delete branch above). GC calls delete_async(id, false)
            // for expired soft-deletes, which would double-decrement without this guard.
            // STOR-H1 FIX: Use fetch_update with checked subtraction to prevent
            // underflow wrapping to usize::MAX, which corrupts all BM25-IDF scoring.
            if !was_soft_deleted {
                if self
                    .total_doc_count
                    .fetch_update(
                        std::sync::atomic::Ordering::Relaxed,
                        std::sync::atomic::Ordering::Relaxed,
                        |c| if c > 0 { Some(c - 1) } else { None },
                    )
                    .is_err()
                {
                    error!(
                        id = %id,
                        "E_STOR_DOC_COUNT_001: total_doc_count underflow prevented \
                         (was already 0) during hard-delete for fingerprint {}",
                        id
                    );
                }
            }
        }

        info!("Deleted fingerprint {} (soft={})", id, soft);
        Ok(true)
    }

    // ==================== Soft-Delete Garbage Collection ====================

    /// Garbage-collect soft-deleted entries whose retention period has expired.
    ///
    /// Scans the in-memory `soft_deleted` map for entries whose deletion timestamp
    /// is older than `retention_secs` seconds. For each expired entry, performs a
    /// hard delete (removes from all CFs + HNSW indexes).
    ///
    /// Returns the number of entries successfully hard-deleted.
    ///
    /// # Errors
    ///
    /// Individual hard-delete failures are logged and skipped — the GC continues
    /// processing remaining entries. Only returns Err on catastrophic failures
    /// (e.g., CF_SYSTEM not found).
    pub async fn gc_soft_deleted(&self, retention_secs: u64) -> CoreResult<usize> {
        let now_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_millis() as i64;
        let retention_millis = (retention_secs as i64) * 1000;
        let cutoff = now_millis - retention_millis;

        // P5: Snapshot expired IDs from DashMap (no global read lock)
        let expired_ids: Vec<Uuid> = self
            .soft_deleted
            .iter()
            .filter(|entry| *entry.value() < cutoff)
            .map(|entry| *entry.key())
            .collect();

        if expired_ids.is_empty() {
            debug!("GC: no soft-deleted entries past retention ({retention_secs}s)");
            return Ok(0);
        }

        info!(
            "GC: hard-deleting {} soft-deleted entries past {retention_secs}s retention",
            expired_ids.len()
        );

        let mut deleted = 0usize;
        for id in &expired_ids {
            match self.delete_async(*id, false).await {
                Ok(true) => {
                    debug!(id = %id, "GC: hard-deleted expired soft-deleted entry");
                    deleted += 1;
                }
                Ok(false) => {
                    // Entry was already gone from RocksDB, clean up tracking
                    warn!(
                        id = %id,
                        "GC: soft-deleted entry not found in RocksDB (already cleaned)"
                    );
                    // P5: DashMap - no write lock needed
                    self.soft_deleted.remove(id);
                    deleted += 1;
                }
                Err(e) => {
                    // Log and continue — don't fail entire GC for one entry
                    warn!(
                        id = %id,
                        error = %e,
                        "GC: failed to hard-delete soft-deleted entry, will retry next cycle"
                    );
                }
            }
        }

        info!("GC: completed, {deleted}/{} entries hard-deleted", expired_ids.len());
        Ok(deleted)
    }

    // ==================== Processing Cursor Storage ====================

    /// Store a processing cursor in CF_SYSTEM under a "cursor::" prefixed key.
    pub(crate) fn store_processing_cursor_sync(
        &self,
        key: &str,
        data: &[u8],
    ) -> CoreResult<()> {
        let cf = self
            .get_cf(crate::column_families::cf_names::SYSTEM)
            .map_err(|e| CoreError::StorageError(format!("CF_SYSTEM not found: {e}")))?;
        let prefixed_key = format!("cursor::{key}");
        self.db
            .put_cf(cf, prefixed_key.as_bytes(), data)
            .map_err(|e| {
                CoreError::StorageError(format!(
                    "Failed to store processing cursor '{key}': {e}"
                ))
            })?;
        debug!(key = key, bytes = data.len(), "Stored processing cursor");
        Ok(())
    }

    /// Retrieve a processing cursor from CF_SYSTEM.
    pub(crate) fn get_processing_cursor_sync(
        &self,
        key: &str,
    ) -> CoreResult<Option<Vec<u8>>> {
        let cf = self
            .get_cf(crate::column_families::cf_names::SYSTEM)
            .map_err(|e| CoreError::StorageError(format!("CF_SYSTEM not found: {e}")))?;
        let prefixed_key = format!("cursor::{key}");
        match self.db.get_cf(cf, prefixed_key.as_bytes()) {
            Ok(Some(data)) => {
                debug!(key = key, bytes = data.len(), "Retrieved processing cursor");
                Ok(Some(data))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(CoreError::StorageError(format!(
                "Failed to get processing cursor '{key}': {e}"
            ))),
        }
    }
}
