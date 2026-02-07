//! Audit log storage operations for RocksDbTeleologicalStore.
//!
//! Provides append-only audit record storage with secondary indexing by target entity.
//! Records are NEVER updated or deleted -- this is enforced at the application layer.
//!
//! # Key Design
//!
//! - Primary key: `{timestamp_nanos_be}_{uuid_bytes}` (24 bytes) for chronological ordering
//! - Secondary index: `{target_uuid_bytes}_{timestamp_nanos_be}` (24 bytes) for per-entity queries
//!
//! # Column Families
//!
//! - `CF_AUDIT_LOG`: Primary append-only audit record storage
//! - `CF_AUDIT_BY_TARGET`: Secondary index by target entity UUID
//!
//! # FAIL FAST Policy
//!
//! All RocksDB operations return detailed errors with operation name, CF, and key context.
//! No fallbacks, no mock data, no silent failures.

use chrono::{DateTime, Utc};
use tracing::{debug, error, info};
use uuid::Uuid;

use context_graph_core::types::audit::AuditRecord;

use crate::teleological::column_families::{CF_AUDIT_BY_TARGET, CF_AUDIT_LOG};

use super::store::RocksDbTeleologicalStore;
use super::types::{TeleologicalStoreError, TeleologicalStoreResult};

impl RocksDbTeleologicalStore {
    /// Append an audit record to the log. Append-only -- never updates existing records.
    ///
    /// Writes atomically to both the primary CF and the target secondary index
    /// using a RocksDB WriteBatch for consistency.
    ///
    /// # Arguments
    ///
    /// * `record` - The audit record to append.
    ///
    /// # Errors
    ///
    /// Returns `TeleologicalStoreError` if:
    /// - Column family handles are missing (database corruption)
    /// - Serialization fails (should never happen with valid types)
    /// - RocksDB write fails (disk error, etc.)
    pub fn append_audit_record(
        &self,
        record: &AuditRecord,
    ) -> TeleologicalStoreResult<()> {
        let primary_key = record.primary_key();
        let target_key = record.target_index_key();

        // Serialize the record using JSON (NOT bincode -- bincode cannot deserialize
        // serde_json::Value which AuditRecord.parameters uses).
        let bytes = serde_json::to_vec(record).map_err(|e| {
            error!(
                "FAIL FAST: Failed to serialize audit record {}: {}",
                record.id, e
            );
            TeleologicalStoreError::Serialization {
                id: Some(record.id),
                message: format!("Audit record serialization failed: {}", e),
            }
        })?;

        // Get column family handles
        let cf_log = self.get_cf(CF_AUDIT_LOG)?;
        let cf_target = self.get_cf(CF_AUDIT_BY_TARGET)?;

        // Atomic batch write: primary record + secondary index
        let mut batch = rocksdb::WriteBatch::default();
        batch.put_cf(cf_log, primary_key, &bytes);
        batch.put_cf(cf_target, target_key, &primary_key);

        self.db.write(batch).map_err(|e| {
            error!(
                "FAIL FAST: Failed to write audit record {} to RocksDB: {}",
                record.id, e
            );
            TeleologicalStoreError::rocksdb_op(
                "write_batch",
                CF_AUDIT_LOG,
                Some(record.id),
                e,
            )
        })?;

        debug!(
            "Appended audit record {} ({} bytes): op={}, target={}",
            record.id,
            bytes.len(),
            record.operation,
            record.target_id,
        );

        Ok(())
    }

    /// Query audit records by target entity ID.
    ///
    /// Uses the `CF_AUDIT_BY_TARGET` secondary index to find all records
    /// affecting a specific entity, then resolves full records from `CF_AUDIT_LOG`.
    ///
    /// Records are returned in chronological order (oldest first) due to the
    /// timestamp suffix in the secondary index key.
    ///
    /// # Arguments
    ///
    /// * `target_id` - The UUID of the target entity to query.
    /// * `limit` - Maximum number of records to return (0 = no limit).
    ///
    /// # Errors
    ///
    /// Returns `TeleologicalStoreError` if RocksDB operations fail.
    pub fn get_audit_by_target(
        &self,
        target_id: Uuid,
        limit: usize,
    ) -> TeleologicalStoreResult<Vec<AuditRecord>> {
        let cf_target = self.get_cf(CF_AUDIT_BY_TARGET)?;
        let cf_log = self.get_cf(CF_AUDIT_LOG)?;

        let prefix = AuditRecord::target_index_prefix(&target_id);
        let iter = self.db.prefix_iterator_cf(cf_target, prefix);

        let mut records = Vec::new();
        let effective_limit = if limit == 0 { usize::MAX } else { limit };

        for item in iter {
            if records.len() >= effective_limit {
                break;
            }

            let (key, primary_key_bytes) = item.map_err(|e| {
                error!(
                    "FAIL FAST: RocksDB iteration failed on CF '{}' for target {}: {}",
                    CF_AUDIT_BY_TARGET, target_id, e
                );
                TeleologicalStoreError::rocksdb_op(
                    "prefix_iterate",
                    CF_AUDIT_BY_TARGET,
                    None,
                    e,
                )
            })?;

            // Verify key prefix matches (prefix_iterator may overshoot)
            if key.len() < 16 || &key[..16] != target_id.as_bytes() {
                break;
            }

            // Resolve full record from primary CF
            match self.db.get_cf(cf_log, &*primary_key_bytes) {
                Ok(Some(record_bytes)) => {
                    let record: AuditRecord =
                        serde_json::from_slice(&record_bytes).map_err(|e| {
                            error!(
                                "FAIL FAST: Failed to deserialize audit record from CF '{}': {}",
                                CF_AUDIT_LOG, e
                            );
                            TeleologicalStoreError::Deserialization {
                                key: format!("audit_log:{}", hex::encode(&*primary_key_bytes)),
                                message: format!("Audit record deserialization failed: {}", e),
                            }
                        })?;
                    records.push(record);
                }
                Ok(None) => {
                    error!(
                        "FAIL FAST: Orphaned audit index entry in CF '{}' for target {} - \
                         primary record missing from CF '{}'",
                        CF_AUDIT_BY_TARGET, target_id, CF_AUDIT_LOG
                    );
                    // Continue rather than fail -- index inconsistency is non-fatal
                }
                Err(e) => {
                    error!(
                        "FAIL FAST: Failed to read audit record from CF '{}': {}",
                        CF_AUDIT_LOG, e
                    );
                    return Err(TeleologicalStoreError::rocksdb_op(
                        "get",
                        CF_AUDIT_LOG,
                        None,
                        e,
                    ));
                }
            }
        }

        debug!(
            "Retrieved {} audit records for target {} (limit={})",
            records.len(),
            target_id,
            limit
        );

        Ok(records)
    }

    /// Query audit records within a time range.
    ///
    /// Scans the primary `CF_AUDIT_LOG` using the chronologically ordered keys.
    /// Big-endian timestamp prefix ensures natural ordering.
    ///
    /// # Arguments
    ///
    /// * `from` - Start of time range (inclusive).
    /// * `to` - End of time range (inclusive).
    /// * `limit` - Maximum number of records to return (0 = no limit).
    ///
    /// # Errors
    ///
    /// Returns `TeleologicalStoreError` if RocksDB operations fail.
    pub fn get_audit_by_time_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        limit: usize,
    ) -> TeleologicalStoreResult<Vec<AuditRecord>> {
        let cf_log = self.get_cf(CF_AUDIT_LOG)?;

        // Build start key from `from` timestamp
        let from_nanos = from.timestamp_nanos_opt().unwrap_or(0);
        let to_nanos = to.timestamp_nanos_opt().unwrap_or(i64::MAX);
        let mut start_key = [0u8; 24];
        start_key[..8].copy_from_slice(&from_nanos.to_be_bytes());
        // UUID portion is all zeros = earliest possible key at this timestamp

        let iter = self.db.iterator_cf(
            cf_log,
            rocksdb::IteratorMode::From(&start_key, rocksdb::Direction::Forward),
        );

        let mut records = Vec::new();
        let effective_limit = if limit == 0 { usize::MAX } else { limit };

        for item in iter {
            if records.len() >= effective_limit {
                break;
            }

            let (key, value) = item.map_err(|e| {
                error!(
                    "FAIL FAST: RocksDB iteration failed on CF '{}': {}",
                    CF_AUDIT_LOG, e
                );
                TeleologicalStoreError::rocksdb_op("iterate", CF_AUDIT_LOG, None, e)
            })?;

            // Extract timestamp from key and check if we've exceeded the range
            if key.len() >= 8 {
                let mut ts_bytes = [0u8; 8];
                ts_bytes.copy_from_slice(&key[..8]);
                let key_nanos = i64::from_be_bytes(ts_bytes);
                if key_nanos > to_nanos {
                    break;
                }
            }

            let record: AuditRecord = serde_json::from_slice(&value).map_err(|e| {
                error!(
                    "FAIL FAST: Failed to deserialize audit record from CF '{}': {}",
                    CF_AUDIT_LOG, e
                );
                TeleologicalStoreError::Deserialization {
                    key: format!("audit_log:{}", hex::encode(&*key)),
                    message: format!("Audit record deserialization failed: {}", e),
                }
            })?;

            records.push(record);
        }

        debug!(
            "Retrieved {} audit records in time range [{}, {}] (limit={})",
            records.len(),
            from,
            to,
            limit
        );

        Ok(records)
    }

    /// Count total audit records in the log.
    ///
    /// Performs a full scan of `CF_AUDIT_LOG` to count records.
    /// For large logs this may be slow -- consider caching if needed.
    ///
    /// # Errors
    ///
    /// Returns `TeleologicalStoreError` if RocksDB operations fail.
    pub fn count_audit_records(&self) -> TeleologicalStoreResult<usize> {
        let cf_log = self.get_cf(CF_AUDIT_LOG)?;
        let iter = self.db.iterator_cf(cf_log, rocksdb::IteratorMode::Start);

        let mut count = 0usize;
        for item in iter {
            let _ = item.map_err(|e| {
                error!(
                    "FAIL FAST: RocksDB iteration failed on CF '{}' during count: {}",
                    CF_AUDIT_LOG, e
                );
                TeleologicalStoreError::rocksdb_op("iterate_count", CF_AUDIT_LOG, None, e)
            })?;
            count += 1;
        }

        info!("Audit log contains {} records", count);
        Ok(count)
    }
}

// Minimal hex encoding for error messages (avoids adding hex crate dependency)
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
