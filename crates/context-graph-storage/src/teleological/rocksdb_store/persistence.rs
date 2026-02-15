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

use tracing::{debug, info, warn};
use uuid::Uuid;

use context_graph_core::clustering::PersistedTopicPortfolio;
use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::traits::TeleologicalStorageBackend;
use context_graph_core::types::fingerprint::TeleologicalFingerprint;

use crate::teleological::column_families::{
    CF_FINGERPRINTS, CF_TOPIC_PORTFOLIO, QUANTIZED_EMBEDDER_CFS, TELEOLOGICAL_CFS,
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
    /// Note: Individual stores are handled by store_async which uses sync I/O.
    /// Batch operations call store_async for each fingerprint.
    pub(crate) async fn store_batch_async(
        &self,
        fingerprints: Vec<TeleologicalFingerprint>,
    ) -> CoreResult<Vec<Uuid>> {
        debug!("Storing batch of {} fingerprints", fingerprints.len());

        let mut ids = Vec::with_capacity(fingerprints.len());

        for fp in fingerprints {
            let id = fp.id;
            // Store in RocksDB (primary storage)
            self.store_fingerprint_internal(&fp)?;

            // CRIT-03 FIX: Add to per-embedder HNSW indexes for O(log n) search.
            // Without this, batch-stored fingerprints are invisible to all search
            // strategies until server restart triggers rebuild_indexes_from_store().
            self.add_to_indexes(&fp)
                .map_err(|e| CoreError::IndexError(format!(
                    "Failed to add fingerprint {} to HNSW indexes during batch store: {}",
                    id, e
                )))?;

            ids.push(id);
        }

        info!("Stored batch of {} fingerprints", ids.len());
        Ok(ids)
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

    /// Get storage size in bytes.
    pub(crate) fn storage_size_bytes_internal(&self) -> usize {
        let mut total = 0usize;

        for cf_name in TELEOLOGICAL_CFS {
            if let Ok(cf) = self.get_cf(cf_name) {
                if let Ok(Some(size)) = self
                    .db
                    .property_int_value_cf(cf, "rocksdb.estimate-live-data-size")
                {
                    total += size as usize;
                }
            }
        }

        for cf_name in QUANTIZED_EMBEDDER_CFS {
            if let Ok(cf) = self.get_cf(cf_name) {
                if let Ok(Some(size)) = self
                    .db
                    .property_int_value_cf(cf, "rocksdb.estimate-live-data-size")
                {
                    total += size as usize;
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
    /// Flush all column families (internal async wrapper).
    ///
    /// Uses `spawn_blocking` to move flush I/O to Tokio's blocking thread pool.
    pub(crate) async fn flush_async(&self) -> CoreResult<()> {
        debug!("Flushing all column families");

        let db = Arc::clone(&self.db);

        tokio::task::spawn_blocking(move || -> CoreResult<()> {
            for cf_name in TELEOLOGICAL_CFS {
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

            for cf_name in QUANTIZED_EMBEDDER_CFS {
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

            Ok(())
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))??;

        info!("Flushed all column families");
        Ok(())
    }

    /// Create checkpoint (internal async wrapper).
    pub(crate) async fn checkpoint_async(&self) -> CoreResult<PathBuf> {
        let checkpoint_path = self.path.join("checkpoints").join(format!(
            "checkpoint_{}",
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        ));

        debug!("Creating checkpoint at {:?}", checkpoint_path);

        std::fs::create_dir_all(&checkpoint_path).map_err(|e| {
            CoreError::StorageError(format!("Failed to create checkpoint directory: {}", e))
        })?;

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
        Ok(checkpoint_path)
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

            for id in &soft_deleted_ids {
                // Use the existing hard-delete path which removes from all CFs + indexes
                match self.delete_async(*id, false).await {
                    Ok(true) => {
                        debug!(id = %id, "Hard-deleted soft-deleted entry during compaction");
                    }
                    Ok(false) => {
                        // Entry was already gone from RocksDB, just clean up tracking
                        warn!(
                            id = %id,
                            "Soft-deleted entry not found in RocksDB during compaction (already cleaned)"
                        );
                    }
                    Err(e) => {
                        // Log but continue - don't fail entire compaction for one entry
                        warn!(
                            id = %id,
                            error = %e,
                            "Failed to hard-delete soft-deleted entry during compaction, \
                             entry will remain in soft_deleted tracking"
                        );
                    }
                }
            }
        }

        // Now compact RocksDB column families
        for cf_name in TELEOLOGICAL_CFS {
            let cf = self.get_cf(cf_name)?;
            self.db.compact_range_cf(cf, None::<&[u8]>, None::<&[u8]>);
        }

        for cf_name in QUANTIZED_EMBEDDER_CFS {
            let cf = self.get_cf(cf_name)?;
            self.db.compact_range_cf(cf, None::<&[u8]>, None::<&[u8]>);
        }

        // Drain remaining soft-deleted tracking (entries that were successfully hard-deleted
        // will already have been removed by delete_async)
        // P5: DashMap - no read/write lock needed
        let remaining = self.soft_deleted.len();
        if remaining > 0 {
            warn!(
                count = remaining,
                "Draining {} remaining soft-deleted entries from tracking after compaction",
                remaining
            );
        }
        self.soft_deleted.clear();

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
