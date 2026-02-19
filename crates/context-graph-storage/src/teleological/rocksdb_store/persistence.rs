//! Batch, statistics, and persistence operations.
//!
//! Contains batch store/retrieve, count/stats, flush/checkpoint/compact,
//! and topic portfolio persistence operations.
//!
//! # Concurrency
//!
//! Methods that perform O(n) RocksDB iteration use `spawn_blocking` to move
//! I/O to Tokio's blocking thread pool, enabling parallel agent access.

use std::path::PathBuf;
use std::sync::Arc;

use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::clustering::PersistedTopicPortfolio;
use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::traits::TeleologicalStorageBackend;
use context_graph_core::types::fingerprint::TeleologicalFingerprint;

use crate::column_families::cf_names;
use crate::teleological::column_families::{
    CF_FINGERPRINTS, CF_TOPIC_PORTFOLIO, QUANTIZED_EMBEDDER_CFS, TELEOLOGICAL_CFS,
    CODE_CFS, CAUSAL_CFS,
};
use crate::teleological::schema::parse_fingerprint_key;
use crate::teleological::serialization::deserialize_teleological_fingerprint;

use super::store::RocksDbTeleologicalStore;
use super::types::TeleologicalStoreError;

// ============================================================================
// Batch Operations
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Store batch of fingerprints (internal async wrapper).
    ///
    /// HIGH-7 FIX: If `add_to_indexes` fails for fingerprint N, attempt rollback
    /// (hard-delete) for that specific fingerprint. Continue processing remaining
    /// fingerprints. Return only successfully stored IDs. Return error only if
    /// ALL fingerprints fail.
    ///
    /// Note: Individual stores are handled by store_async which uses sync I/O.
    /// Batch operations call store_fingerprint_internal for each fingerprint.
    pub(crate) async fn store_batch_async(
        &self,
        fingerprints: Vec<TeleologicalFingerprint>,
    ) -> CoreResult<Vec<Uuid>> {
        debug!("Storing batch of {} fingerprints", fingerprints.len());

        let total = fingerprints.len();
        let mut succeeded: Vec<Uuid> = Vec::with_capacity(total);
        let mut failed: Vec<(Uuid, String)> = Vec::new();

        for fp in fingerprints {
            let id = fp.id;

            // STOR-5 FIX: Reject stores for soft-deleted IDs (same guard as store_async).
            // Without this, batch stores can write phantom records hidden by soft-delete filter.
            if self.is_soft_deleted(&id) {
                error!(
                    id = %id,
                    "Batch store: skipping soft-deleted ID — FAIL FAST"
                );
                failed.push((id, format!("Cannot store fingerprint {}: ID is soft-deleted", id)));
                continue;
            }

            // Store in RocksDB (primary storage) — new insert, count for IDF
            if let Err(e) = self.store_fingerprint_internal(&fp, true) {
                error!(
                    id = %id,
                    error = %e,
                    "Failed to store fingerprint in RocksDB during batch store"
                );
                failed.push((id, format!("RocksDB store failed: {}", e)));
                continue;
            }

            // CRIT-03 FIX: Add to per-embedder HNSW indexes for O(log n) search.
            // HIGH-7 FIX: If indexing fails, rollback the RocksDB write for this
            // specific fingerprint and continue with the rest.
            if let Err(e) = self.add_to_indexes(&fp) {
                error!(
                    id = %id,
                    error = %e,
                    "Failed to add fingerprint to HNSW indexes during batch store — \
                     rolling back RocksDB write"
                );

                // Attempt rollback: hard-delete this fingerprint from RocksDB
                match self.delete_async(id, false).await {
                    Ok(_) => {
                        debug!(id = %id, "Rollback successful: hard-deleted fingerprint after index failure");
                    }
                    Err(rollback_err) => {
                        // Rollback failed — fingerprint is in RocksDB but NOT in HNSW indexes.
                        // This is a data inconsistency that will be repaired on next server restart
                        // via rebuild_indexes_from_store(). Log at error level for visibility.
                        error!(
                            id = %id,
                            index_error = %e,
                            rollback_error = %rollback_err,
                            "CRITICAL: Rollback failed after index failure — fingerprint {} is in \
                             RocksDB but absent from HNSW indexes. Will be repaired on next restart.",
                            id
                        );
                    }
                }

                failed.push((id, format!("Index add failed: {}", e)));
                continue;
            }

            succeeded.push(id);
        }

        // Log batch result summary
        if failed.is_empty() {
            info!("Stored batch of {} fingerprints (all succeeded)", succeeded.len());
        } else {
            warn!(
                succeeded = succeeded.len(),
                failed = failed.len(),
                total = total,
                "Batch store completed with failures"
            );
            for (id, reason) in &failed {
                error!(id = %id, reason = %reason, "Batch store failure detail");
            }
        }

        // FAIL FAST: If ALL fingerprints failed, return error
        if succeeded.is_empty() && !failed.is_empty() {
            return Err(CoreError::StorageError(format!(
                "Batch store failed for all {} fingerprints. First failure: {}",
                failed.len(),
                failed[0].1
            )));
        }

        Ok(succeeded)
    }

    /// Retrieve batch of fingerprints (internal async wrapper).
    ///
    /// Uses `spawn_blocking` to move batch I/O to Tokio's blocking thread pool.
    pub(crate) async fn retrieve_batch_async(
        &self,
        ids: &[Uuid],
    ) -> CoreResult<Vec<Option<TeleologicalFingerprint>>> {
        debug!("Retrieving batch of {} fingerprints", ids.len());

        // Clone Arc-wrapped fields for spawn_blocking closure
        // CRITICAL: Use Arc::clone for soft_deleted instead of cloning the HashMap
        let db = Arc::clone(&self.db);
        let soft_deleted = Arc::clone(&self.soft_deleted);
        let ids_clone: Vec<Uuid> = ids.to_vec();

        let results = tokio::task::spawn_blocking(move || -> CoreResult<Vec<Option<TeleologicalFingerprint>>> {
            use crate::teleological::schema::fingerprint_key;

            let cf = db.cf_handle(CF_FINGERPRINTS).ok_or_else(|| {
                TeleologicalStoreError::ColumnFamilyNotFound {
                    name: CF_FINGERPRINTS.to_string(),
                }
            })?;

            let mut results = Vec::with_capacity(ids_clone.len());

            for id in ids_clone {
                // Skip soft-deleted entries (read lock inside spawn_blocking)
                // FAIL FAST: Panic if lock is poisoned (thread panic elsewhere)
                let is_deleted = soft_deleted.contains_key(&id);
                if is_deleted {
                    results.push(None);
                    continue;
                }

                let key = fingerprint_key(&id);
                match db.get_cf(cf, key) {
                    Ok(Some(data)) => {
                        let fp = deserialize_teleological_fingerprint(&data)?;
                        results.push(Some(fp));
                    }
                    Ok(None) => results.push(None),
                    Err(e) => {
                        return Err(TeleologicalStoreError::rocksdb_op("get", CF_FINGERPRINTS, Some(id), e).into());
                    }
                }
            }

            Ok(results)
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))??;

        Ok(results)
    }
}

// ============================================================================
// Statistics Operations
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Count fingerprints (internal async wrapper).
    ///
    /// Uses `spawn_blocking` to move O(n) iteration to Tokio's blocking thread pool.
    pub(crate) async fn count_async(&self) -> CoreResult<usize> {
        // Check cache first (outside spawn_blocking since it's fast)
        // MED-11 FIX: parking_lot::RwLock returns guard directly (non-poisonable)
        if let Some(count) = *self.fingerprint_count.read() {
            return Ok(count);
        }

        // Clone Arc-wrapped fields for spawn_blocking closure
        // CRITICAL: Use Arc::clone for soft_deleted instead of cloning the HashMap
        let db = Arc::clone(&self.db);
        let soft_deleted = Arc::clone(&self.soft_deleted);

        // Count by iterating in blocking thread pool
        let count = tokio::task::spawn_blocking(move || -> CoreResult<usize> {
            let cf = db.cf_handle(CF_FINGERPRINTS).ok_or_else(|| {
                TeleologicalStoreError::ColumnFamilyNotFound {
                    name: CF_FINGERPRINTS.to_string(),
                }
            })?;
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);

            let mut count = 0;
            for item in iter {
                let (key, _) = item.map_err(|e| {
                    TeleologicalStoreError::rocksdb_op("iterate", CF_FINGERPRINTS, None, e)
                })?;
                let id = parse_fingerprint_key(&key);

                // P5: DashMap - lock-free contains_key check
                let is_deleted = soft_deleted.contains_key(&id);
                if !is_deleted {
                    count += 1;
                }
            }

            Ok(count)
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))??;

        // Cache the result
        *self.fingerprint_count.write() = Some(count);

        Ok(count)
    }

    /// Get storage size in bytes across ALL 51 column families.
    pub(crate) fn storage_size_bytes_internal(&self) -> usize {
        let mut total = 0usize;

        // Iterate ALL CF groups: base(11) + teleological(20) + quantized(13) + code(5) + causal(2) = 51
        let all_cf_arrays: &[&[&str]] = &[
            cf_names::ALL,
            TELEOLOGICAL_CFS,
            QUANTIZED_EMBEDDER_CFS,
            CODE_CFS,
            CAUSAL_CFS,
        ];

        for cf_names_arr in all_cf_arrays {
            for cf_name in *cf_names_arr {
                if let Ok(cf) = self.get_cf(cf_name) {
                    if let Ok(Some(size)) = self
                        .db
                        .property_int_value_cf(cf, "rocksdb.estimate-live-data-size")
                    {
                        total += size as usize;
                    }
                }
            }
        }

        total
    }

    /// Get backend type.
    pub(crate) fn backend_type_internal(&self) -> TeleologicalStorageBackend {
        TeleologicalStorageBackend::RocksDb
    }
}

// ============================================================================
// Persistence Operations
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Flush ALL 51 column families (internal async wrapper).
    ///
    /// Uses `spawn_blocking` to move flush I/O to Tokio's blocking thread pool.
    /// Covers base(11) + teleological(20) + quantized(13) + code(5) + causal(2) = 51 CFs.
    pub(crate) async fn flush_async(&self) -> CoreResult<()> {
        debug!("Flushing all 51 column families");

        let db = Arc::clone(&self.db);

        tokio::task::spawn_blocking(move || -> CoreResult<()> {
            let all_cf_arrays: &[&[&str]] = &[
                cf_names::ALL,
                TELEOLOGICAL_CFS,
                QUANTIZED_EMBEDDER_CFS,
                CODE_CFS,
                CAUSAL_CFS,
            ];

            for cf_names_arr in all_cf_arrays {
                for cf_name in *cf_names_arr {
                    let cf = db.cf_handle(cf_name).ok_or_else(|| {
                        TeleologicalStoreError::ColumnFamilyNotFound {
                            name: cf_name.to_string(),
                        }
                    })?;
                    db.flush_cf(cf)
                        .map_err(|e| TeleologicalStoreError::RocksDbOperation {
                            operation: "flush",
                            cf: cf_name,
                            key: None,
                            source: e,
                        })?;
                }
            }

            Ok(())
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))??;

        info!("Flushed all 51 column families");
        Ok(())
    }

    /// Create checkpoint (synchronous version for background tasks).
    pub fn checkpoint(&self) -> CoreResult<PathBuf> {
        self.checkpoint_sync()
    }

    /// Create checkpoint (synchronous implementation).
    pub(crate) fn checkpoint_sync(&self) -> CoreResult<PathBuf> {
        let checkpoint_path = self.path.join("checkpoints").join(format!(
            "checkpoint_{}",
            chrono::Utc::now().format("%Y%m%d_%H%M%S_%3f")
        ));

        debug!("Creating checkpoint at {:?}", checkpoint_path);

        // Only create the parent "checkpoints/" directory, NOT the checkpoint dir itself.
        // RocksDB's Checkpoint::create_checkpoint requires the target directory to NOT exist.
        if let Some(parent) = checkpoint_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CoreError::StorageError(format!("Failed to create checkpoints directory: {}", e))
            })?;
        }

        let checkpoint = rocksdb::checkpoint::Checkpoint::new(&self.db).map_err(|e| {
            TeleologicalStoreError::CheckpointFailed {
                message: e.to_string(),
            }
        })?;

        checkpoint
            .create_checkpoint(&checkpoint_path)
            .map_err(|e| TeleologicalStoreError::CheckpointFailed {
                message: e.to_string(),
            })?;

        info!("Created checkpoint at {:?}", checkpoint_path);

        // Auto-cleanup: keep only the N most recent checkpoints
        const MAX_CHECKPOINTS: usize = 5;
        if let Err(e) = Self::cleanup_old_checkpoints(&self.path, MAX_CHECKPOINTS) {
            warn!(error = %e, "Checkpoint cleanup failed (non-fatal)");
        }

        Ok(checkpoint_path)
    }

    /// Remove old checkpoints, keeping only the `keep` most recent ones.
    fn cleanup_old_checkpoints(db_path: &std::path::Path, keep: usize) -> CoreResult<()> {
        let checkpoint_dir = db_path.join("checkpoints");
        if !checkpoint_dir.exists() {
            return Ok(());
        }

        let mut entries: Vec<_> = std::fs::read_dir(&checkpoint_dir)
            .map_err(|e| CoreError::StorageError(format!("Failed to read checkpoint directory: {}", e)))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        if entries.len() <= keep {
            return Ok(());
        }

        // Sort by name (timestamp-based names sort chronologically)
        entries.sort_by_key(|e| e.file_name());

        // Remove oldest entries (keep the last `keep`)
        let to_remove = entries.len() - keep;
        for entry in entries.into_iter().take(to_remove) {
            info!("Removing old checkpoint: {:?}", entry.path());
            if let Err(e) = std::fs::remove_dir_all(entry.path()) {
                warn!(path = ?entry.path(), error = %e, "Failed to remove old checkpoint");
            }
        }

        Ok(())
    }

    /// Restore from checkpoint (internal async wrapper).
    pub(crate) async fn restore_async(&self, checkpoint_path: &std::path::Path) -> CoreResult<()> {
        warn!(
            "Restore operation requested from {:?}. This is destructive!",
            checkpoint_path
        );

        if !checkpoint_path.exists() {
            return Err(TeleologicalStoreError::RestoreFailed {
                path: checkpoint_path.to_string_lossy().to_string(),
                message: "Checkpoint path does not exist".to_string(),
            }
            .into());
        }

        Err(TeleologicalStoreError::RestoreFailed {
            path: checkpoint_path.to_string_lossy().to_string(),
            message: "In-place restore not supported. Please restart the application with the checkpoint path.".to_string(),
        }.into())
    }

    /// Compact all column families (internal async wrapper).
    ///
    /// CRIT-04 FIX: Hard-deletes soft-deleted entries from RocksDB BEFORE
    /// draining the in-memory HashMap. Previously, draining without hard-delete
    /// caused soft-deleted data to reappear after compaction.
    pub(crate) async fn compact_async(&self) -> CoreResult<()> {
        debug!("Starting compaction of all column families");

        // CRIT-04 FIX: Hard-delete soft-deleted entries from RocksDB first.
        // Collect IDs while holding the read lock, then release before mutation.
        // P5: DashMap - iterate without global read lock
        let soft_deleted_ids: Vec<Uuid> = self.soft_deleted.iter().map(|entry| *entry.key()).collect();

        if !soft_deleted_ids.is_empty() {
            info!(
                count = soft_deleted_ids.len(),
                "Hard-deleting {} soft-deleted entries from RocksDB during compaction",
                soft_deleted_ids.len()
            );

            // MED-14 + MED-15 FIX: Track which entries were successfully hard-deleted.
            // Only remove those specific entries from the DashMap. Failed entries remain
            // in soft_deleted for retry on next compaction. This also eliminates the
            // MED-15 race condition: new soft-deletes inserted between snapshot and
            // removal are never swept away because we only remove by specific ID.
            let mut successfully_deleted: Vec<Uuid> = Vec::new();

            for id in &soft_deleted_ids {
                // Use the existing hard-delete path which removes from all CFs + indexes
                match self.delete_async(*id, false).await {
                    Ok(true) => {
                        debug!(id = %id, "Hard-deleted soft-deleted entry during compaction");
                        successfully_deleted.push(*id);
                    }
                    Ok(false) => {
                        // Entry was already gone from RocksDB, just clean up tracking
                        warn!(
                            id = %id,
                            "Soft-deleted entry not found in RocksDB during compaction (already cleaned)"
                        );
                        successfully_deleted.push(*id);
                    }
                    Err(e) => {
                        // Log but continue - don't fail entire compaction for one entry.
                        // Entry stays in soft_deleted for retry on next compaction cycle.
                        warn!(
                            id = %id,
                            error = %e,
                            "Failed to hard-delete soft-deleted entry during compaction — \
                             keeping in soft_deleted for retry on next cycle"
                        );
                    }
                }
            }

            // MED-14 FIX: Only remove entries that were successfully hard-deleted.
            // delete_async(id, false) already removes from soft_deleted internally,
            // but entries that failed remain tracked for next compaction.
            let failed_count = soft_deleted_ids.len() - successfully_deleted.len();
            if failed_count > 0 {
                warn!(
                    failed = failed_count,
                    succeeded = successfully_deleted.len(),
                    "Compaction: {} soft-deleted entries failed hard-delete, retained for next cycle",
                    failed_count
                );
            }
        }

        // Now compact ALL 51 RocksDB column families
        let all_cf_arrays: &[&[&str]] = &[
            cf_names::ALL,
            TELEOLOGICAL_CFS,
            QUANTIZED_EMBEDDER_CFS,
            CODE_CFS,
            CAUSAL_CFS,
        ];

        for cf_names_arr in all_cf_arrays {
            for cf_name in *cf_names_arr {
                let cf = self.get_cf(cf_name)?;
                self.db.compact_range_cf(cf, None::<&[u8]>, None::<&[u8]>);
            }
        }

        // MED-14 FIX: Do NOT call self.soft_deleted.clear(). Entries that failed
        // hard-delete must remain in the DashMap so they are retried on next compaction.
        // Successfully deleted entries were already removed by delete_async(id, false).

        info!("Compaction complete");
        Ok(())
    }
}

// ============================================================================
// Topic Portfolio Persistence Operations
// ============================================================================

/// Sentinel key for the most recent topic portfolio across all sessions.
const LATEST_PORTFOLIO_KEY: &[u8] = b"__latest__";

impl RocksDbTeleologicalStore {
    /// Persist topic portfolio for a session (internal async wrapper).
    ///
    /// Stores the portfolio under both the session_id key and the "__latest__"
    /// sentinel for cross-session restoration.
    pub(crate) async fn persist_topic_portfolio_async(
        &self,
        session_id: &str,
        portfolio: &PersistedTopicPortfolio,
    ) -> CoreResult<()> {
        debug!(
            session_id = %session_id,
            topic_count = portfolio.topics.len(),
            churn_rate = portfolio.churn_rate,
            entropy = portfolio.entropy,
            "Persisting topic portfolio"
        );

        let cf = self.get_cf(CF_TOPIC_PORTFOLIO)?;

        // Serialize portfolio to JSON bytes
        let value = portfolio.to_bytes().map_err(|e| {
            CoreError::SerializationError(format!("Failed to serialize topic portfolio: {}", e))
        })?;

        // Store under session_id key
        self.db
            .put_cf(cf, session_id.as_bytes(), &value)
            .map_err(|e| TeleologicalStoreError::RocksDbOperation {
                operation: "put",
                cf: CF_TOPIC_PORTFOLIO,
                key: Some(session_id.to_string()),
                source: e,
            })?;

        // Also store as "__latest__" for cross-session restoration
        self.db
            .put_cf(cf, LATEST_PORTFOLIO_KEY, &value)
            .map_err(|e| TeleologicalStoreError::RocksDbOperation {
                operation: "put",
                cf: CF_TOPIC_PORTFOLIO,
                key: Some("__latest__".to_string()),
                source: e,
            })?;

        info!(
            session_id = %session_id,
            topic_count = portfolio.topics.len(),
            "Topic portfolio persisted"
        );

        Ok(())
    }

    /// Load topic portfolio for a specific session (internal async wrapper).
    pub(crate) async fn load_topic_portfolio_async(
        &self,
        session_id: &str,
    ) -> CoreResult<Option<PersistedTopicPortfolio>> {
        debug!(session_id = %session_id, "Loading topic portfolio");

        let cf = self.get_cf(CF_TOPIC_PORTFOLIO)?;

        match self.db.get_cf(cf, session_id.as_bytes()) {
            Ok(Some(bytes)) => {
                let portfolio = PersistedTopicPortfolio::from_bytes(&bytes).map_err(|e| {
                    CoreError::SerializationError(format!(
                        "Failed to deserialize topic portfolio: {}",
                        e
                    ))
                })?;

                info!(
                    session_id = %session_id,
                    topic_count = portfolio.topics.len(),
                    "Topic portfolio loaded"
                );

                Ok(Some(portfolio))
            }
            Ok(None) => {
                debug!(session_id = %session_id, "No topic portfolio found for session");
                Ok(None)
            }
            Err(e) => Err(TeleologicalStoreError::RocksDbOperation {
                operation: "get",
                cf: CF_TOPIC_PORTFOLIO,
                key: Some(session_id.to_string()),
                source: e,
            }
            .into()),
        }
    }

    /// Load the most recent topic portfolio (internal async wrapper).
    ///
    /// Uses the "__latest__" sentinel key.
    pub(crate) async fn load_latest_topic_portfolio_async(
        &self,
    ) -> CoreResult<Option<PersistedTopicPortfolio>> {
        debug!("Loading latest topic portfolio");

        let cf = self.get_cf(CF_TOPIC_PORTFOLIO)?;

        match self.db.get_cf(cf, LATEST_PORTFOLIO_KEY) {
            Ok(Some(bytes)) => {
                let portfolio = PersistedTopicPortfolio::from_bytes(&bytes).map_err(|e| {
                    CoreError::SerializationError(format!(
                        "Failed to deserialize latest topic portfolio: {}",
                        e
                    ))
                })?;

                info!(
                    session_id = %portfolio.session_id,
                    topic_count = portfolio.topics.len(),
                    "Latest topic portfolio loaded"
                );

                Ok(Some(portfolio))
            }
            Ok(None) => {
                debug!("No latest topic portfolio found");
                Ok(None)
            }
            Err(e) => Err(TeleologicalStoreError::RocksDbOperation {
                operation: "get",
                cf: CF_TOPIC_PORTFOLIO,
                key: Some("__latest__".to_string()),
                source: e,
            }
            .into()),
        }
    }
}

// ============================================================================
// Clustering Support Operations
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Scan all fingerprints and return their embeddings for clustering.
    ///
    /// This method iterates over all fingerprints in storage and extracts
    /// their 13-element embedding arrays for use in HDBSCAN clustering.
    ///
    /// Uses `spawn_blocking` to move O(n) iteration to Tokio's blocking thread pool.
    ///
    /// # Arguments
    /// * `limit` - Optional maximum number of fingerprints to scan
    ///
    /// # Returns
    /// Vector of (fingerprint_id, embeddings_array) tuples.
    ///
    /// # Errors
    /// - `CoreError::StorageError` - RocksDB iteration failed
    /// - `CoreError::SerializationError` - Fingerprint deserialization failed
    pub(crate) async fn scan_fingerprints_for_clustering_async(
        &self,
        limit: Option<usize>,
    ) -> CoreResult<Vec<(Uuid, [Vec<f32>; 13])>> {
        info!(limit = ?limit, "Scanning fingerprints for clustering");

        // Clone Arc-wrapped fields for spawn_blocking closure
        // CRITICAL: Use Arc::clone for soft_deleted instead of cloning the HashMap
        let db = Arc::clone(&self.db);
        let soft_deleted = Arc::clone(&self.soft_deleted);

        let results = tokio::task::spawn_blocking(move || -> CoreResult<Vec<(Uuid, [Vec<f32>; 13])>> {
            let cf = db.cf_handle(CF_FINGERPRINTS).ok_or_else(|| {
                TeleologicalStoreError::ColumnFamilyNotFound {
                    name: CF_FINGERPRINTS.to_string(),
                }
            })?;
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);

            let mut results = Vec::new();

            for item in iter {
                let (key, value) = item.map_err(|e| {
                    TeleologicalStoreError::rocksdb_op("iterate", CF_FINGERPRINTS, None, e)
                })?;

                // Parse fingerprint ID from key
                let id = parse_fingerprint_key(&key);

                // Skip soft-deleted fingerprints (read lock inside spawn_blocking)
                // FAIL FAST: Panic if lock is poisoned
                let is_deleted = soft_deleted.contains_key(&id);
                if is_deleted {
                    continue;
                }

                // Deserialize fingerprint using the custom serialization format
                // (has version prefix, not plain bincode)
                let fp = match deserialize_teleological_fingerprint(&value) {
                    Ok(fp) => fp,
                    Err(e) => {
                        warn!(
                            "Skipping corrupted fingerprint {} during clustering scan: {}",
                            id, e
                        );
                        continue;
                    }
                };

                // Extract the 13 embeddings as cluster array
                let cluster_array = fp.semantic.to_cluster_array();
                results.push((id, cluster_array));

                // Apply limit if specified
                if let Some(max) = limit {
                    if results.len() >= max {
                        break;
                    }
                }
            }

            Ok(results)
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))??;

        info!(
            count = results.len(),
            "Scanned fingerprints for clustering complete"
        );
        Ok(results)
    }

    /// List fingerprints without semantic bias (MED-13/14/15 root cause fix).
    ///
    /// Scans CF_FINGERPRINTS directly, returning full TeleologicalFingerprint
    /// objects. Skips soft-deleted entries. Uses spawn_blocking for O(n) scan.
    pub(crate) async fn list_fingerprints_unbiased_async(
        &self,
        limit: usize,
    ) -> CoreResult<Vec<TeleologicalFingerprint>> {
        info!(limit = limit, "Scanning fingerprints (unbiased, no semantic query)");

        let db = Arc::clone(&self.db);
        let soft_deleted = Arc::clone(&self.soft_deleted);

        let results = tokio::task::spawn_blocking(move || -> CoreResult<Vec<TeleologicalFingerprint>> {
            let cf = db.cf_handle(CF_FINGERPRINTS).ok_or_else(|| {
                TeleologicalStoreError::ColumnFamilyNotFound {
                    name: CF_FINGERPRINTS.to_string(),
                }
            })?;
            let iter = db.iterator_cf(cf, rocksdb::IteratorMode::Start);

            let mut results = Vec::with_capacity(limit.min(1024));

            for item in iter {
                let (key, value) = item.map_err(|e| {
                    TeleologicalStoreError::rocksdb_op("iterate", CF_FINGERPRINTS, None, e)
                })?;

                let id = parse_fingerprint_key(&key);

                // Skip soft-deleted
                let is_deleted = soft_deleted.contains_key(&id);
                if is_deleted {
                    continue;
                }

                let fp = match deserialize_teleological_fingerprint(&value) {
                    Ok(fp) => fp,
                    Err(e) => {
                        warn!(
                            "Skipping corrupted fingerprint {} during unbiased scan: {}",
                            id, e
                        );
                        continue;
                    }
                };
                results.push(fp);

                if results.len() >= limit {
                    break;
                }
            }

            Ok(results)
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))??;

        info!(count = results.len(), "Unbiased fingerprint scan complete");
        Ok(results)
    }
}
