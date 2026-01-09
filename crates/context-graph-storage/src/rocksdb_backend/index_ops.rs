//! Secondary index query operations for RocksDB backend.
//!
//! Provides query methods that leverage indexes created during node storage.
//! All operations use RocksDB iterators for efficient scanning.
//!
//! # Index Key Formats
//! | Column Family | Key Format | Description |
//! |---------------|------------|-------------|
//! | `johari_{quadrant}` | 16-byte NodeId | Raw UUID bytes |
//! | `tags` | `{tag_string}:{16-byte NodeId}` | Prefix-scannable by tag |
//! | `sources` | `{source_string}:{16-byte NodeId}` | Prefix-scannable by source |
//! | `temporal` | `{8-byte timestamp BE}:{16-byte NodeId}` | Range-scannable by time |
//!
//! # Performance
//! All operations use iterator-based pagination for memory efficiency.
//! Results are `Vec<NodeId>`, NOT full MemoryNode objects.
//!
//! # Error Handling
//! Empty results return `Ok(Vec::new())`, NOT errors.
//! Only actual failures (IO, deserialization) return errors.

use chrono::{DateTime, Utc};
use rocksdb::IteratorMode;
use uuid::Uuid;

use crate::column_families::cf_names;
use context_graph_core::types::{JohariQuadrant, NodeId};

use super::core::RocksDbMemex;
use super::error::StorageError;

/// Helper function to convert a 16-byte slice to UUID.
///
/// # Arguments
/// * `bytes` - Slice of exactly 16 bytes
///
/// # Returns
/// * `Ok(Uuid)` - Successfully parsed UUID
/// * `Err(StorageError::ReadFailed)` - Slice is not exactly 16 bytes
#[inline]
fn uuid_from_slice(bytes: &[u8]) -> Result<Uuid, StorageError> {
    if bytes.len() != 16 {
        return Err(StorageError::ReadFailed(format!(
            "Invalid UUID bytes: expected 16, got {}",
            bytes.len()
        )));
    }
    let arr: [u8; 16] = bytes.try_into().expect("length already checked");
    Ok(Uuid::from_bytes(arr))
}

impl RocksDbMemex {
    /// Gets all node IDs in a specific Johari quadrant.
    ///
    /// Uses the `johari_{quadrant}` column family full scan.
    /// Keys in Johari CFs are raw 16-byte NodeId.
    ///
    /// # Arguments
    /// * `quadrant` - The Johari quadrant to query
    /// * `limit` - Maximum number of results (None = unlimited)
    /// * `offset` - Number of results to skip
    ///
    /// # Returns
    /// * `Ok(Vec<NodeId>)` - List of node IDs in the quadrant
    /// * Empty Vec if no nodes in quadrant (NOT an error)
    ///
    /// # Performance
    /// Target: O(n) where n = nodes in quadrant. Uses iterator, not full load.
    ///
    /// # Example
    /// ```
    /// use context_graph_core::types::JohariQuadrant;
    ///
    /// // JohariQuadrant can be used for filtering
    /// let quadrant = JohariQuadrant::Open;
    /// assert_eq!(format!("{:?}", quadrant), "Open");
    /// ```
    pub fn get_nodes_by_quadrant(
        &self,
        quadrant: JohariQuadrant,
        limit: Option<usize>,
        offset: usize,
    ) -> Result<Vec<NodeId>, StorageError> {
        // Handle limit=0 early (no results requested)
        if limit == Some(0) {
            return Ok(Vec::new());
        }

        let cf = self.get_cf(quadrant.column_family())?;
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);

        let mut results = Vec::new();
        let mut skipped = 0;

        for item in iter {
            let (key, _) = item.map_err(|e| StorageError::ReadFailed(e.to_string()))?;

            // Handle offset
            if skipped < offset {
                skipped += 1;
                continue;
            }

            // Parse 16-byte key as UUID
            if key.len() == 16 {
                let node_id = uuid_from_slice(&key)?;
                results.push(node_id);
            }

            // Handle limit
            if let Some(max) = limit {
                if results.len() >= max {
                    break;
                }
            }
        }

        Ok(results)
    }

    /// Gets all node IDs with a specific tag.
    ///
    /// Uses the `tags` column family with prefix scan.
    /// Key format: `{tag_string}:{16-byte NodeId}`
    ///
    /// # Arguments
    /// * `tag` - Tag to search for (exact match)
    /// * `limit` - Maximum results (None = unlimited)
    /// * `offset` - Results to skip
    ///
    /// # Returns
    /// * `Ok(Vec<NodeId>)` - List of node IDs with the tag
    /// * Empty Vec if no nodes have this tag (NOT an error)
    ///
    /// # Example
    /// ```
    /// // Tags are simple strings used for filtering
    /// let tag = "important";
    /// assert!(!tag.is_empty());
    /// ```
    pub fn get_nodes_by_tag(
        &self,
        tag: &str,
        limit: Option<usize>,
        offset: usize,
    ) -> Result<Vec<NodeId>, StorageError> {
        // Handle limit=0 early (no results requested)
        if limit == Some(0) {
            return Ok(Vec::new());
        }

        let cf = self.get_cf(cf_names::TAGS)?;

        // Create prefix: tag_bytes + ':'
        let mut prefix = Vec::with_capacity(tag.len() + 1);
        prefix.extend_from_slice(tag.as_bytes());
        prefix.push(b':');

        // Use regular iterator starting from prefix (not prefix_iterator which relies on
        // the 16-byte prefix extractor configured for UUID-based keys)
        let iter = self
            .db
            .iterator_cf(cf, IteratorMode::From(&prefix, rocksdb::Direction::Forward));

        let mut results = Vec::new();
        let mut skipped = 0;

        for item in iter {
            let (key, _) = item.map_err(|e| StorageError::ReadFailed(e.to_string()))?;

            // Stop if prefix no longer matches
            if !key.starts_with(&prefix) {
                break;
            }

            // Handle offset
            if skipped < offset {
                skipped += 1;
                continue;
            }

            // Extract NodeId from key (16 bytes after prefix)
            let uuid_start = prefix.len();
            if key.len() >= uuid_start + 16 {
                let node_id = uuid_from_slice(&key[uuid_start..uuid_start + 16])?;
                results.push(node_id);
            }

            // Handle limit
            if let Some(max) = limit {
                if results.len() >= max {
                    break;
                }
            }
        }

        Ok(results)
    }

    /// Gets all node IDs from a specific source.
    ///
    /// Uses the `sources` column family with prefix scan.
    /// Key format: `{source_string}:{16-byte NodeId}`
    ///
    /// # Arguments
    /// * `source` - Source to search for (exact match)
    /// * `limit` - Maximum results (None = unlimited)
    /// * `offset` - Results to skip
    ///
    /// # Returns
    /// * `Ok(Vec<NodeId>)` - List of node IDs from the source
    /// * Empty Vec if no nodes have this source (NOT an error)
    ///
    /// # Example
    /// ```
    /// // Sources identify where data came from
    /// let source = "api-gateway";
    /// assert!(!source.is_empty());
    /// ```
    pub fn get_nodes_by_source(
        &self,
        source: &str,
        limit: Option<usize>,
        offset: usize,
    ) -> Result<Vec<NodeId>, StorageError> {
        // Handle limit=0 early (no results requested)
        if limit == Some(0) {
            return Ok(Vec::new());
        }

        let cf = self.get_cf(cf_names::SOURCES)?;

        // Create prefix: source_bytes + ':'
        let mut prefix = Vec::with_capacity(source.len() + 1);
        prefix.extend_from_slice(source.as_bytes());
        prefix.push(b':');

        // Use regular iterator starting from prefix (not prefix_iterator which relies on
        // the 16-byte prefix extractor configured for UUID-based keys)
        let iter = self
            .db
            .iterator_cf(cf, IteratorMode::From(&prefix, rocksdb::Direction::Forward));

        let mut results = Vec::new();
        let mut skipped = 0;

        for item in iter {
            let (key, _) = item.map_err(|e| StorageError::ReadFailed(e.to_string()))?;

            if !key.starts_with(&prefix) {
                break;
            }

            if skipped < offset {
                skipped += 1;
                continue;
            }

            let uuid_start = prefix.len();
            if key.len() >= uuid_start + 16 {
                let node_id = uuid_from_slice(&key[uuid_start..uuid_start + 16])?;
                results.push(node_id);
            }

            if let Some(max) = limit {
                if results.len() >= max {
                    break;
                }
            }
        }

        Ok(results)
    }

    /// Gets all node IDs created within a time range.
    ///
    /// Uses the `temporal` column family with range scan.
    /// Key format: `{8-byte timestamp millis BE}:{16-byte NodeId}`
    ///
    /// # Arguments
    /// * `start` - Start of time range (inclusive)
    /// * `end` - End of time range (exclusive)
    /// * `limit` - Maximum results (None = unlimited)
    /// * `offset` - Results to skip
    ///
    /// # Returns
    /// * `Ok(Vec<NodeId>)` - List of node IDs created in the time range
    /// * Empty Vec if no nodes in range (NOT an error)
    /// * Nodes ordered by creation time (oldest first due to BE ordering)
    ///
    /// # Example
    /// ```
    /// use chrono::{Duration, Utc};
    ///
    /// // Create time range for queries
    /// let now = Utc::now();
    /// let yesterday = now - Duration::hours(24);
    /// assert!(yesterday < now);
    /// ```
    pub fn get_nodes_in_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: Option<usize>,
        offset: usize,
    ) -> Result<Vec<NodeId>, StorageError> {
        // Handle limit=0 early (no results requested)
        if limit == Some(0) {
            return Ok(Vec::new());
        }

        let cf = self.get_cf(cf_names::TEMPORAL)?;

        // Create start key (timestamp only, 8 bytes)
        let start_millis = start.timestamp_millis() as u64;
        let end_millis = end.timestamp_millis() as u64;
        let start_key = start_millis.to_be_bytes();

        let iter = self.db.iterator_cf(
            cf,
            IteratorMode::From(&start_key, rocksdb::Direction::Forward),
        );

        let mut results = Vec::new();
        let mut skipped = 0;

        for item in iter {
            let (key, _) = item.map_err(|e| StorageError::ReadFailed(e.to_string()))?;

            // Key must be 24 bytes: 8 timestamp + 16 UUID
            if key.len() != 24 {
                continue;
            }

            // Extract timestamp from first 8 bytes
            let ts_bytes: [u8; 8] = key[0..8].try_into().expect("slice is exactly 8 bytes");
            let key_millis = u64::from_be_bytes(ts_bytes);

            // Stop if past end time
            if key_millis >= end_millis {
                break;
            }

            // Handle offset
            if skipped < offset {
                skipped += 1;
                continue;
            }

            // Extract NodeId from bytes 8-24
            let node_id = uuid_from_slice(&key[8..24])?;
            results.push(node_id);

            // Handle limit
            if let Some(max) = limit {
                if results.len() >= max {
                    break;
                }
            }
        }

        Ok(results)
    }
}
