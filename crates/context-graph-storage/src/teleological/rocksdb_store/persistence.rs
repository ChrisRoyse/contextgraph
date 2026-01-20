//! Batch, statistics, and persistence operations.
//!
//! Contains batch store/retrieve, count/stats, flush/checkpoint/compact,
//! and topic portfolio persistence operations.

use std::path::PathBuf;

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

use super::store::RocksDbTeleologicalStore;
use super::types::TeleologicalStoreError;

// ============================================================================
// Batch Operations
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Store batch of fingerprints (internal async wrapper).
    pub(crate) async fn store_batch_async(
        &self,
        fingerprints: Vec<TeleologicalFingerprint>,
    ) -> CoreResult<Vec<Uuid>> {
        debug!("Storing batch of {} fingerprints", fingerprints.len());

        let mut ids = Vec::with_capacity(fingerprints.len());

        for fp in fingerprints {
            let id = fp.id;
            self.store_fingerprint_internal(&fp)?;
            ids.push(id);
        }

        info!("Stored batch of {} fingerprints", ids.len());
        Ok(ids)
    }

    /// Retrieve batch of fingerprints (internal async wrapper).
    pub(crate) async fn retrieve_batch_async(
        &self,
        ids: &[Uuid],
    ) -> CoreResult<Vec<Option<TeleologicalFingerprint>>> {
        debug!("Retrieving batch of {} fingerprints", ids.len());

        let mut results = Vec::with_capacity(ids.len());

        for &id in ids {
            let fp = self.retrieve_async(id).await?;
            results.push(fp);
        }

        Ok(results)
    }
}

// ============================================================================
// Statistics Operations
// ============================================================================

impl RocksDbTeleologicalStore {
    /// Count fingerprints (internal async wrapper).
    pub(crate) async fn count_async(&self) -> CoreResult<usize> {
        // Check cache first
        if let Ok(cached) = self.fingerprint_count.read() {
            if let Some(count) = *cached {
                return Ok(count);
            }
        }

        // Count by iterating
        let cf = self.get_cf(CF_FINGERPRINTS)?;
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        let mut count = 0;
        for item in iter {
            let (key, _) = item.map_err(|e| {
                TeleologicalStoreError::rocksdb_op("iterate", CF_FINGERPRINTS, None, e)
            })?;
            let id = parse_fingerprint_key(&key);

            if !self.is_soft_deleted(&id) {
                count += 1;
            }
        }

        // Cache the result
        if let Ok(mut cached) = self.fingerprint_count.write() {
            *cached = Some(count);
        }

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
    pub(crate) async fn flush_async(&self) -> CoreResult<()> {
        debug!("Flushing all column families");

        for cf_name in TELEOLOGICAL_CFS {
            let cf = self.get_cf(cf_name)?;
            self.db
                .flush_cf(cf)
                .map_err(|e| TeleologicalStoreError::RocksDbOperation {
                    operation: "flush",
                    cf: cf_name,
                    key: None,
                    source: e,
                })?;
        }

        for cf_name in QUANTIZED_EMBEDDER_CFS {
            let cf = self.get_cf(cf_name)?;
            self.db
                .flush_cf(cf)
                .map_err(|e| TeleologicalStoreError::RocksDbOperation {
                    operation: "flush",
                    cf: cf_name,
                    key: None,
                    source: e,
                })?;
        }

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
    pub(crate) async fn compact_async(&self) -> CoreResult<()> {
        debug!("Starting compaction of all column families");

        for cf_name in TELEOLOGICAL_CFS {
            let cf = self.get_cf(cf_name)?;
            self.db.compact_range_cf(cf, None::<&[u8]>, None::<&[u8]>);
        }

        for cf_name in QUANTIZED_EMBEDDER_CFS {
            let cf = self.get_cf(cf_name)?;
            self.db.compact_range_cf(cf, None::<&[u8]>, None::<&[u8]>);
        }

        // Purge soft-deleted entries
        if let Ok(mut deleted) = self.soft_deleted.write() {
            for (id, _) in deleted.drain() {
                debug!("Purging soft-deleted entry {} from tracking", id);
            }
        }

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
