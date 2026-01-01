//! RocksDB storage backend implementation.
//!
//! Provides persistent storage using RocksDB with column families
//! for Johari quadrant separation and efficient indexing.
//!
//! # Performance Targets (constitution.yaml)
//! - inject_context: p95 < 25ms, p99 < 50ms
//! - hopfield: < 1ms
//! - store_node: < 1ms p95
//! - get_node: < 500μs p95
//!
//! # Column Families
//! Uses 12 CFs defined in `column_families.rs`:
//! - nodes, edges, embeddings, metadata
//! - johari_open, johari_hidden, johari_blind, johari_unknown
//! - temporal, tags, sources, system
//!
//! # CRUD Operations (TASK-M02-017)
//! - `store_node()`: Atomic write to nodes, embeddings, johari, temporal, tags, sources CFs
//! - `get_node()`: Retrieve and deserialize MemoryNode by ID
//! - `update_node()`: Update with index maintenance when quadrant/tags change
//! - `delete_node()`: Soft delete (SEC-06 compliance) or hard delete

use chrono::{DateTime, Utc};
use rocksdb::{Cache, ColumnFamily, Options, WriteBatch, DB};
use std::collections::HashSet;
use std::path::Path;
use thiserror::Error;

use crate::column_families::{cf_names, get_column_family_descriptors};
use crate::serialization::{
    deserialize_node, serialize_embedding, serialize_node, serialize_uuid, SerializationError,
};
use context_graph_core::types::{MemoryNode, NodeId, ValidationError};

/// Default block cache size: 256MB (per constitution.yaml).
pub const DEFAULT_CACHE_SIZE: usize = 256 * 1024 * 1024;

/// Default maximum open files.
pub const DEFAULT_MAX_OPEN_FILES: i32 = 1000;

/// Storage operation errors.
///
/// These errors cover database lifecycle operations and CRUD operations.
/// Designed for fail-fast debugging with descriptive error messages.
///
/// # TASK-M02-017 Additions
/// - `NotFound`: Node/entity not found by ID
/// - `Serialization`: Serialization/deserialization errors
/// - `ValidationFailed`: Node validation failed before storage
#[derive(Debug, Error)]
pub enum StorageError {
    /// Database failed to open.
    #[error("Failed to open database at '{path}': {message}")]
    OpenFailed { path: String, message: String },

    /// Column family not found (should never happen if DB opened correctly).
    #[error("Column family '{name}' not found")]
    ColumnFamilyNotFound { name: String },

    /// Write operation failed.
    #[error("Write failed: {0}")]
    WriteFailed(String),

    /// Read operation failed.
    #[error("Read failed: {0}")]
    ReadFailed(String),

    /// Flush operation failed.
    #[error("Flush failed: {0}")]
    FlushFailed(String),

    /// Node not found by ID.
    ///
    /// Returned by `get_node()`, `update_node()`, and `delete_node()` when
    /// the requested node does not exist in the database.
    #[error("Node not found: {id}")]
    NotFound {
        /// The node ID that was not found (as string for display)
        id: String,
    },

    /// Serialization or deserialization error.
    ///
    /// Wraps errors from the serialization module during storage operations.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Node validation failed.
    ///
    /// Returned when `MemoryNode::validate()` fails before storage.
    /// Fail fast: invalid nodes are never stored.
    #[error("Validation error: {0}")]
    ValidationFailed(String),
}

impl From<SerializationError> for StorageError {
    fn from(e: SerializationError) -> Self {
        StorageError::Serialization(e.to_string())
    }
}

impl From<ValidationError> for StorageError {
    fn from(e: ValidationError) -> Self {
        StorageError::ValidationFailed(e.to_string())
    }
}

/// Configuration options for RocksDbMemex.
///
/// # Defaults
/// - `max_open_files`: 1000
/// - `block_cache_size`: 256MB (268,435,456 bytes)
/// - `enable_wal`: true (durability)
/// - `create_if_missing`: true (convenience)
#[derive(Debug, Clone)]
pub struct RocksDbConfig {
    /// Maximum open files (default: 1000).
    pub max_open_files: i32,
    /// Block cache size in bytes (default: 256MB).
    pub block_cache_size: usize,
    /// Enable Write-Ahead Logging (default: true).
    pub enable_wal: bool,
    /// Create database if missing (default: true).
    pub create_if_missing: bool,
}

impl Default for RocksDbConfig {
    fn default() -> Self {
        Self {
            max_open_files: DEFAULT_MAX_OPEN_FILES,
            block_cache_size: DEFAULT_CACHE_SIZE,
            enable_wal: true,
            create_if_missing: true,
        }
    }
}

/// RocksDB-backed storage implementation.
///
/// Provides persistent storage for MemoryNodes and GraphEdges with
/// optimized column families for different access patterns.
///
/// # Thread Safety
/// RocksDB's `DB` type is internally thread-safe for concurrent reads and writes.
/// This struct can be shared across threads via `Arc<RocksDbMemex>`.
///
/// # Column Families
/// Opens all 12 column families defined in `column_families.rs`.
///
/// # Example
/// ```rust,ignore
/// use context_graph_storage::rocksdb_backend::{RocksDbMemex, RocksDbConfig};
/// use tempfile::TempDir;
///
/// let tmp = TempDir::new().unwrap();
/// let db = RocksDbMemex::open(tmp.path()).expect("open failed");
/// assert!(db.health_check().is_ok());
/// ```
pub struct RocksDbMemex {
    /// The RocksDB database instance.
    db: DB,
    /// Shared block cache (kept alive for DB lifetime).
    #[allow(dead_code)]
    cache: Cache,
    /// Database path for reference.
    path: String,
}

impl RocksDbMemex {
    /// Open a RocksDB database at the specified path with default configuration.
    ///
    /// Creates the database and all 12 column families if they don't exist.
    ///
    /// # Arguments
    /// * `path` - Path to the database directory
    ///
    /// # Returns
    /// * `Ok(RocksDbMemex)` - Successfully opened database
    /// * `Err(StorageError::OpenFailed)` - Database could not be opened
    ///
    /// # Example
    /// ```rust,ignore
    /// use context_graph_storage::rocksdb_backend::RocksDbMemex;
    /// use tempfile::TempDir;
    ///
    /// let tmp = TempDir::new().unwrap();
    /// let db = RocksDbMemex::open(tmp.path()).expect("open failed");
    /// ```
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        Self::open_with_config(path, RocksDbConfig::default())
    }

    /// Open a RocksDB database with custom configuration.
    ///
    /// # Arguments
    /// * `path` - Path to the database directory
    /// * `config` - Custom configuration options
    ///
    /// # Returns
    /// * `Ok(RocksDbMemex)` - Successfully opened database
    /// * `Err(StorageError::OpenFailed)` - Database could not be opened
    pub fn open_with_config<P: AsRef<Path>>(
        path: P,
        config: RocksDbConfig,
    ) -> Result<Self, StorageError> {
        let path_str = path.as_ref().to_string_lossy().to_string();

        // Create shared block cache
        let cache = Cache::new_lru_cache(config.block_cache_size);

        // Create DB options
        let mut db_opts = Options::default();
        db_opts.create_if_missing(config.create_if_missing);
        db_opts.create_missing_column_families(true);
        db_opts.set_max_open_files(config.max_open_files);

        // WAL configuration
        if !config.enable_wal {
            db_opts.set_manual_wal_flush(true);
        }

        // Get column family descriptors with optimized options
        let cf_descriptors = get_column_family_descriptors(&cache);

        // Open database with all column families
        let db = DB::open_cf_descriptors(&db_opts, &path_str, cf_descriptors).map_err(|e| {
            StorageError::OpenFailed {
                path: path_str.clone(),
                message: e.to_string(),
            }
        })?;

        Ok(Self {
            db,
            cache,
            path: path_str,
        })
    }

    /// Get a reference to a column family by name.
    ///
    /// # Arguments
    /// * `name` - Column family name (use `cf_names::*` constants)
    ///
    /// # Returns
    /// * `Ok(&ColumnFamily)` - Reference to the column family
    /// * `Err(StorageError::ColumnFamilyNotFound)` - CF doesn't exist
    ///
    /// # Example
    /// ```rust,ignore
    /// use context_graph_storage::rocksdb_backend::RocksDbMemex;
    /// use context_graph_storage::column_families::cf_names;
    ///
    /// let cf = db.get_cf(cf_names::NODES)?;
    /// ```
    pub fn get_cf(&self, name: &str) -> Result<&ColumnFamily, StorageError> {
        self.db
            .cf_handle(name)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound {
                name: name.to_string(),
            })
    }

    /// Get the database path.
    ///
    /// # Returns
    /// The path where the database is stored.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Check if the database is healthy.
    ///
    /// Verifies all 12 column families are accessible.
    ///
    /// # Returns
    /// * `Ok(())` - All CFs accessible
    /// * `Err(StorageError::ColumnFamilyNotFound)` - A CF is missing
    pub fn health_check(&self) -> Result<(), StorageError> {
        for cf_name in cf_names::ALL {
            self.get_cf(cf_name)?;
        }
        Ok(())
    }

    /// Flush all column families to disk.
    ///
    /// Forces all buffered writes to be persisted.
    ///
    /// # Returns
    /// * `Ok(())` - All CFs flushed successfully
    /// * `Err(StorageError::FlushFailed)` - Flush operation failed
    pub fn flush_all(&self) -> Result<(), StorageError> {
        for cf_name in cf_names::ALL {
            let cf = self.get_cf(cf_name)?;
            self.db
                .flush_cf(cf)
                .map_err(|e| StorageError::FlushFailed(e.to_string()))?;
        }
        Ok(())
    }

    /// Get a reference to the underlying RocksDB instance.
    ///
    /// Use this for advanced operations not covered by the high-level API.
    /// Be careful not to violate data invariants.
    pub fn db(&self) -> &DB {
        &self.db
    }

    // =========================================================================
    // CRUD Operations (TASK-M02-017)
    // =========================================================================

    /// Stores a MemoryNode atomically across all relevant column families.
    ///
    /// Writes to: nodes, embeddings, johari_{quadrant}, temporal, tags (per tag), sources
    /// Uses WriteBatch for atomicity.
    ///
    /// # Validation
    /// Calls node.validate() before storage - returns error if validation fails.
    /// FAIL FAST: Invalid nodes are never stored.
    ///
    /// # Performance
    /// Target: <1ms p95 latency
    ///
    /// # Errors
    /// - `StorageError::ValidationFailed` if node.validate() fails
    /// - `StorageError::Serialization` if serialization fails
    /// - `StorageError::WriteFailed` if RocksDB write fails
    /// - `StorageError::ColumnFamilyNotFound` if CF missing (should never happen)
    ///
    /// # Example
    /// ```rust,ignore
    /// use context_graph_storage::rocksdb_backend::RocksDbMemex;
    /// use context_graph_core::types::MemoryNode;
    ///
    /// let db = RocksDbMemex::open("./data")?;
    /// let node = MemoryNode::new("Test content".into(), vec![0.1; 1536]);
    /// db.store_node(&node)?;
    /// ```
    pub fn store_node(&self, node: &MemoryNode) -> Result<(), StorageError> {
        // 1. Validate node FIRST - fail fast
        node.validate()?;

        // 2. Create WriteBatch for atomic operation
        let mut batch = WriteBatch::default();

        // 3. Get all required column families
        let cf_nodes = self.get_cf(cf_names::NODES)?;
        let cf_embeddings = self.get_cf(cf_names::EMBEDDINGS)?;
        let cf_temporal = self.get_cf(cf_names::TEMPORAL)?;
        let cf_tags = self.get_cf(cf_names::TAGS)?;
        let cf_sources = self.get_cf(cf_names::SOURCES)?;
        let cf_johari = self.get_cf(node.quadrant.column_family())?;

        // 4. Serialize and write to nodes CF
        let node_key = serialize_uuid(&node.id);
        let node_value = serialize_node(node)?;
        batch.put_cf(cf_nodes, node_key.as_slice(), &node_value);

        // 5. Write embedding to embeddings CF
        let embedding_value = serialize_embedding(&node.embedding);
        batch.put_cf(cf_embeddings, node_key.as_slice(), &embedding_value);

        // 6. Add to Johari quadrant index (empty value, key is index)
        batch.put_cf(cf_johari, node_key.as_slice(), []);

        // 7. Add to temporal index: timestamp_millis:uuid format
        let temporal_key = format_temporal_key(node.created_at, &node.id);
        batch.put_cf(cf_temporal, temporal_key.as_slice(), []);

        // 8. Add to tag indexes (one entry per tag)
        for tag in &node.metadata.tags {
            let tag_key = format_tag_key(tag, &node.id);
            batch.put_cf(cf_tags, tag_key.as_slice(), []);
        }

        // 9. Add to sources index if source present
        if let Some(source) = &node.metadata.source {
            let source_key = format_source_key(source, &node.id);
            batch.put_cf(cf_sources, source_key.as_slice(), []);
        }

        // 10. Execute atomic batch write
        self.db
            .write(batch)
            .map_err(|e| StorageError::WriteFailed(e.to_string()))?;

        Ok(())
    }

    /// Retrieves a MemoryNode by its ID.
    ///
    /// # Performance
    /// Target: <500μs p95 latency
    ///
    /// # Errors
    /// - `StorageError::NotFound` if node doesn't exist
    /// - `StorageError::Serialization` if deserialization fails
    /// - `StorageError::ReadFailed` if RocksDB read fails
    ///
    /// # Example
    /// ```rust,ignore
    /// use context_graph_storage::rocksdb_backend::RocksDbMemex;
    /// use uuid::Uuid;
    ///
    /// let db = RocksDbMemex::open("./data")?;
    /// let node = db.get_node(&some_id)?;
    /// println!("Content: {}", node.content);
    /// ```
    pub fn get_node(&self, id: &NodeId) -> Result<MemoryNode, StorageError> {
        let cf_nodes = self.get_cf(cf_names::NODES)?;
        let node_key = serialize_uuid(id);

        let node_bytes = self
            .db
            .get_cf(cf_nodes, node_key.as_slice())
            .map_err(|e| StorageError::ReadFailed(e.to_string()))?
            .ok_or_else(|| StorageError::NotFound { id: id.to_string() })?;

        deserialize_node(&node_bytes).map_err(StorageError::from)
    }

    /// Updates an existing MemoryNode, maintaining index consistency.
    ///
    /// Handles index updates when quadrant or tags change.
    /// DOES NOT create if node doesn't exist - returns NotFound error.
    /// FAIL FAST: Non-existent nodes cause immediate error.
    ///
    /// # Errors
    /// - `StorageError::NotFound` if node doesn't exist
    /// - `StorageError::ValidationFailed` if node.validate() fails
    /// - `StorageError::Serialization` if serialization fails
    /// - `StorageError::WriteFailed` if RocksDB write fails
    ///
    /// # Example
    /// ```rust,ignore
    /// use context_graph_storage::rocksdb_backend::RocksDbMemex;
    ///
    /// let db = RocksDbMemex::open("./data")?;
    /// let mut node = db.get_node(&some_id)?;
    /// node.content = "Updated content".to_string();
    /// db.update_node(&node)?;
    /// ```
    pub fn update_node(&self, node: &MemoryNode) -> Result<(), StorageError> {
        // 1. Validate node FIRST - fail fast
        node.validate()?;

        // 2. Get existing node (MUST exist, fail if not)
        let old_node = self.get_node(&node.id)?;

        // 3. Create WriteBatch for atomic operation
        let mut batch = WriteBatch::default();

        // 4. Get CFs
        let cf_nodes = self.get_cf(cf_names::NODES)?;
        let cf_embeddings = self.get_cf(cf_names::EMBEDDINGS)?;
        let cf_tags = self.get_cf(cf_names::TAGS)?;
        let cf_sources = self.get_cf(cf_names::SOURCES)?;

        let node_key = serialize_uuid(&node.id);

        // 5. Update node data
        batch.put_cf(cf_nodes, node_key.as_slice(), serialize_node(node)?);
        batch.put_cf(
            cf_embeddings,
            node_key.as_slice(),
            serialize_embedding(&node.embedding),
        );

        // 6. Handle Johari quadrant change
        if old_node.quadrant != node.quadrant {
            let old_cf = self.get_cf(old_node.quadrant.column_family())?;
            let new_cf = self.get_cf(node.quadrant.column_family())?;
            batch.delete_cf(old_cf, node_key.as_slice());
            batch.put_cf(new_cf, node_key.as_slice(), []);
        }

        // 7. Handle tag changes
        let old_tags: HashSet<_> = old_node.metadata.tags.into_iter().collect();
        let new_tags: HashSet<_> = node.metadata.tags.iter().cloned().collect();

        for removed_tag in old_tags.difference(&new_tags) {
            batch.delete_cf(cf_tags, format_tag_key(removed_tag, &node.id));
        }
        for added_tag in new_tags.difference(&old_tags) {
            batch.put_cf(cf_tags, format_tag_key(added_tag, &node.id), []);
        }

        // 8. Handle source changes
        let old_source = old_node.metadata.source.as_ref();
        let new_source = node.metadata.source.as_ref();

        if old_source != new_source {
            // Remove old source index entry
            if let Some(old_src) = old_source {
                batch.delete_cf(cf_sources, format_source_key(old_src, &node.id));
            }
            // Add new source index entry
            if let Some(new_src) = new_source {
                batch.put_cf(cf_sources, format_source_key(new_src, &node.id), []);
            }
        }

        // 9. Write atomically
        self.db
            .write(batch)
            .map_err(|e| StorageError::WriteFailed(e.to_string()))?;

        Ok(())
    }

    /// Deletes a MemoryNode and removes it from all indexes.
    ///
    /// # Arguments
    /// * `id` - Node ID to delete
    /// * `soft_delete` - If true, marks as deleted (SEC-06); if false, permanently removes
    ///
    /// # SEC-06 Compliance
    /// Soft delete preserves data for 30-day recovery per constitution.yaml.
    /// Soft-deleted nodes remain in the nodes CF but with `metadata.deleted = true`.
    ///
    /// # Errors
    /// - `StorageError::NotFound` if node doesn't exist
    /// - `StorageError::WriteFailed` if RocksDB write fails
    ///
    /// # Example
    /// ```rust,ignore
    /// use context_graph_storage::rocksdb_backend::RocksDbMemex;
    ///
    /// let db = RocksDbMemex::open("./data")?;
    ///
    /// // Soft delete (SEC-06 compliant, recoverable for 30 days)
    /// db.delete_node(&some_id, true)?;
    ///
    /// // Hard delete (permanent, use with caution)
    /// db.delete_node(&other_id, false)?;
    /// ```
    pub fn delete_node(&self, id: &NodeId, soft_delete: bool) -> Result<(), StorageError> {
        // 1. Get existing node (MUST exist) - fail fast for non-existent
        let node = self.get_node(id)?;

        let mut batch = WriteBatch::default();
        let node_key = serialize_uuid(id);

        if soft_delete {
            // SEC-06: Mark as deleted, preserve data for 30-day recovery
            let mut updated_node = node.clone();
            updated_node.metadata.mark_deleted();

            let cf_nodes = self.get_cf(cf_names::NODES)?;
            batch.put_cf(cf_nodes, node_key.as_slice(), serialize_node(&updated_node)?);
        } else {
            // Hard delete: Remove from ALL column families
            let cf_nodes = self.get_cf(cf_names::NODES)?;
            let cf_embeddings = self.get_cf(cf_names::EMBEDDINGS)?;
            let cf_temporal = self.get_cf(cf_names::TEMPORAL)?;
            let cf_tags = self.get_cf(cf_names::TAGS)?;
            let cf_sources = self.get_cf(cf_names::SOURCES)?;
            let cf_johari = self.get_cf(node.quadrant.column_family())?;

            batch.delete_cf(cf_nodes, node_key.as_slice());
            batch.delete_cf(cf_embeddings, node_key.as_slice());
            batch.delete_cf(cf_johari, node_key.as_slice());
            batch.delete_cf(cf_temporal, format_temporal_key(node.created_at, id));

            for tag in &node.metadata.tags {
                batch.delete_cf(cf_tags, format_tag_key(tag, id));
            }

            if let Some(source) = &node.metadata.source {
                batch.delete_cf(cf_sources, format_source_key(source, id));
            }
        }

        self.db
            .write(batch)
            .map_err(|e| StorageError::WriteFailed(e.to_string()))?;

        Ok(())
    }
}

// =========================================================================
// Private Helper Functions for Key Formatting (TASK-M02-017)
// =========================================================================

/// Format temporal index key: 8-byte timestamp (millis, big-endian) + 16-byte UUID.
///
/// Big-endian ensures lexicographic ordering matches temporal ordering,
/// enabling efficient range scans by time.
#[inline]
fn format_temporal_key(timestamp: DateTime<Utc>, id: &NodeId) -> Vec<u8> {
    let millis = timestamp.timestamp_millis() as u64;
    let mut key = Vec::with_capacity(24);
    key.extend_from_slice(&millis.to_be_bytes());
    key.extend_from_slice(&serialize_uuid(id));
    key
}

/// Format tag index key: tag_bytes + ':' + 16-byte UUID.
///
/// Enables prefix scans by tag name.
#[inline]
fn format_tag_key(tag: &str, id: &NodeId) -> Vec<u8> {
    let mut key = Vec::with_capacity(tag.len() + 1 + 16);
    key.extend_from_slice(tag.as_bytes());
    key.push(b':');
    key.extend_from_slice(&serialize_uuid(id));
    key
}

/// Format source index key: source_bytes + ':' + 16-byte UUID.
///
/// Enables prefix scans by source.
#[inline]
fn format_source_key(source: &str, id: &NodeId) -> Vec<u8> {
    let mut key = Vec::with_capacity(source.len() + 1 + 16);
    key.extend_from_slice(source.as_bytes());
    key.push(b':');
    key.extend_from_slice(&serialize_uuid(id));
    key
}

// DB is automatically closed when RocksDbMemex is dropped (RocksDB's Drop impl)

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // =========================================================================
    // Helper Functions
    // =========================================================================

    fn create_temp_db() -> (TempDir, RocksDbMemex) {
        let tmp = TempDir::new().expect("Failed to create temp dir");
        let db = RocksDbMemex::open(tmp.path()).expect("Failed to open database");
        (tmp, db)
    }

    // =========================================================================
    // RocksDbConfig Tests
    // =========================================================================

    #[test]
    fn test_config_default_values() {
        let config = RocksDbConfig::default();
        assert_eq!(config.max_open_files, 1000);
        assert_eq!(config.block_cache_size, 256 * 1024 * 1024);
        assert!(config.enable_wal);
        assert!(config.create_if_missing);
    }

    #[test]
    fn test_config_custom_values() {
        let config = RocksDbConfig {
            max_open_files: 500,
            block_cache_size: 128 * 1024 * 1024,
            enable_wal: false,
            create_if_missing: false,
        };
        assert_eq!(config.max_open_files, 500);
        assert_eq!(config.block_cache_size, 128 * 1024 * 1024);
        assert!(!config.enable_wal);
        assert!(!config.create_if_missing);
    }

    #[test]
    fn test_config_clone() {
        let config = RocksDbConfig::default();
        let cloned = config.clone();
        assert_eq!(config.max_open_files, cloned.max_open_files);
    }

    #[test]
    fn test_config_debug() {
        let config = RocksDbConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("RocksDbConfig"));
        assert!(debug.contains("max_open_files"));
    }

    // =========================================================================
    // StorageError Tests
    // =========================================================================

    #[test]
    fn test_error_open_failed() {
        let error = StorageError::OpenFailed {
            path: "/tmp/test".to_string(),
            message: "permission denied".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("/tmp/test"));
        assert!(msg.contains("permission denied"));
    }

    #[test]
    fn test_error_column_family_not_found() {
        let error = StorageError::ColumnFamilyNotFound {
            name: "unknown_cf".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("unknown_cf"));
    }

    #[test]
    fn test_error_write_failed() {
        let error = StorageError::WriteFailed("disk full".to_string());
        assert!(error.to_string().contains("disk full"));
    }

    #[test]
    fn test_error_read_failed() {
        let error = StorageError::ReadFailed("io error".to_string());
        assert!(error.to_string().contains("io error"));
    }

    #[test]
    fn test_error_flush_failed() {
        let error = StorageError::FlushFailed("sync failed".to_string());
        assert!(error.to_string().contains("sync failed"));
    }

    #[test]
    fn test_error_debug() {
        let error = StorageError::WriteFailed("test".to_string());
        let debug = format!("{:?}", error);
        assert!(debug.contains("WriteFailed"));
    }

    // =========================================================================
    // Database Open/Close Tests
    // =========================================================================

    #[test]
    fn test_open_creates_database() {
        println!("=== TEST: open() creates database ===");
        let tmp = TempDir::new().expect("create temp dir");
        let path = tmp.path();

        println!("BEFORE: Database path = {:?}", path);
        println!("BEFORE: Path exists = {}", path.exists());

        let db = RocksDbMemex::open(path).expect("open failed");

        println!("AFTER: Database opened successfully");
        println!("AFTER: db.path() = {}", db.path());

        assert!(path.exists(), "Database directory should exist");
        assert_eq!(db.path(), path.to_string_lossy());
    }

    #[test]
    fn test_open_with_custom_config() {
        println!("=== TEST: open_with_config() custom settings ===");
        let tmp = TempDir::new().expect("create temp dir");

        let config = RocksDbConfig {
            max_open_files: 100,
            block_cache_size: 64 * 1024 * 1024, // 64MB
            enable_wal: true,
            create_if_missing: true,
        };

        println!("BEFORE: Custom config = {:?}", config);

        let db = RocksDbMemex::open_with_config(tmp.path(), config).expect("open failed");

        println!("AFTER: Database opened with custom config");
        assert!(db.health_check().is_ok());
    }

    #[test]
    fn test_open_invalid_path_fails() {
        // Try to open in a non-existent path without create_if_missing
        let config = RocksDbConfig {
            create_if_missing: false,
            ..Default::default()
        };

        let result = RocksDbMemex::open_with_config("/nonexistent/path/db", config);
        assert!(result.is_err());

        if let Err(StorageError::OpenFailed { path, message }) = result {
            assert!(path.contains("nonexistent"));
            assert!(!message.is_empty());
        }
    }

    // =========================================================================
    // Column Family Tests
    // =========================================================================

    #[test]
    fn test_get_cf_returns_valid_handle() {
        let (_tmp, db) = create_temp_db();

        for cf_name in cf_names::ALL {
            let cf = db.get_cf(cf_name);
            assert!(cf.is_ok(), "CF '{}' should exist", cf_name);
        }
    }

    #[test]
    fn test_get_cf_unknown_returns_error() {
        let (_tmp, db) = create_temp_db();

        let result = db.get_cf("nonexistent_cf");
        assert!(result.is_err());

        if let Err(StorageError::ColumnFamilyNotFound { name }) = result {
            assert_eq!(name, "nonexistent_cf");
        } else {
            panic!("Expected ColumnFamilyNotFound error");
        }
    }

    #[test]
    fn test_all_12_cfs_accessible() {
        println!("=== TEST: All 12 column families accessible ===");
        let (_tmp, db) = create_temp_db();

        println!("BEFORE: Checking {} column families", cf_names::ALL.len());

        for (i, cf_name) in cf_names::ALL.iter().enumerate() {
            let cf = db
                .get_cf(cf_name)
                .unwrap_or_else(|_| panic!("CF {} should exist", cf_name));
            println!("  CF {}: '{}' -> handle obtained", i, cf_name);
            // CF handle is valid (non-null pointer internally)
            let _ = cf;
        }

        println!("AFTER: All 12 CFs verified accessible");
    }

    // =========================================================================
    // Health Check Tests
    // =========================================================================

    #[test]
    fn test_health_check_passes() {
        let (_tmp, db) = create_temp_db();
        let result = db.health_check();
        assert!(result.is_ok(), "Health check should pass: {:?}", result);
    }

    #[test]
    fn test_health_check_verifies_all_cfs() {
        println!("=== TEST: health_check verifies all CFs ===");
        let (_tmp, db) = create_temp_db();

        println!("BEFORE: Running health check");
        let result = db.health_check();
        println!("AFTER: Health check result = {:?}", result);

        assert!(result.is_ok());
    }

    // =========================================================================
    // Flush Tests
    // =========================================================================

    #[test]
    fn test_flush_all_succeeds() {
        let (_tmp, db) = create_temp_db();
        let result = db.flush_all();
        assert!(result.is_ok(), "Flush should succeed: {:?}", result);
    }

    #[test]
    fn test_flush_all_on_empty_db() {
        println!("=== TEST: flush_all on empty database ===");
        let (_tmp, db) = create_temp_db();

        println!("BEFORE: Flushing empty database");
        let result = db.flush_all();
        println!("AFTER: Flush result = {:?}", result);

        assert!(result.is_ok());
    }

    // =========================================================================
    // Reopen Tests
    // =========================================================================

    #[test]
    fn test_reopen_preserves_cfs() {
        println!("=== TEST: Reopen preserves column families ===");
        let tmp = TempDir::new().expect("create temp dir");
        let path = tmp.path().to_path_buf();

        // Open first time
        {
            println!("BEFORE: Opening database first time");
            let db = RocksDbMemex::open(&path).expect("first open failed");
            assert!(db.health_check().is_ok());
            println!("AFTER: First open successful, dropping database");
        } // db dropped here

        // Reopen
        {
            println!("BEFORE: Reopening database");
            let db = RocksDbMemex::open(&path).expect("reopen failed");
            println!("AFTER: Reopen successful");

            // Verify all CFs still exist
            for cf_name in cf_names::ALL {
                let cf = db.get_cf(cf_name);
                assert!(cf.is_ok(), "CF '{}' should exist after reopen", cf_name);
            }
            println!("RESULT: All 12 CFs preserved after reopen");
        }
    }

    // =========================================================================
    // Edge Case Tests (REQUIRED - print before/after state)
    // =========================================================================

    #[test]
    fn edge_case_multiple_opens_same_path_fails() {
        println!("=== EDGE CASE 1: Multiple opens on same path ===");
        let tmp = TempDir::new().expect("create temp dir");

        let db1 = RocksDbMemex::open(tmp.path()).expect("first open");
        println!("BEFORE: First database opened at {:?}", tmp.path());

        // Second open should fail (RocksDB lock)
        let result = RocksDbMemex::open(tmp.path());
        println!("AFTER: Second open attempt result = {:?}", result.is_err());

        assert!(result.is_err(), "Second open should fail due to lock");
        drop(db1);
        println!("RESULT: PASS - RocksDB prevents concurrent opens");
    }

    #[test]
    fn edge_case_minimum_cache_size() {
        println!("=== EDGE CASE 2: Minimum cache size (1MB) ===");
        let tmp = TempDir::new().expect("create temp dir");

        let config = RocksDbConfig {
            block_cache_size: 1024 * 1024, // 1MB
            ..Default::default()
        };

        println!("BEFORE: Opening with 1MB cache");
        let db = RocksDbMemex::open_with_config(tmp.path(), config).expect("open failed");
        println!("AFTER: Database opened with minimal cache");

        assert!(db.health_check().is_ok());
        println!("RESULT: PASS - Works with minimum cache");
    }

    #[test]
    fn edge_case_path_with_spaces() {
        println!("=== EDGE CASE 3: Path with spaces ===");
        let tmp = TempDir::new().expect("create temp dir");
        let path_with_spaces = tmp.path().join("path with spaces");
        std::fs::create_dir_all(&path_with_spaces).expect("create dir");

        println!(
            "BEFORE: Opening at path with spaces: {:?}",
            path_with_spaces
        );
        let db = RocksDbMemex::open(&path_with_spaces).expect("open failed");
        println!("AFTER: Database opened successfully");

        assert!(db.health_check().is_ok());
        assert!(db.path().contains("path with spaces"));
        println!("RESULT: PASS - Path with spaces handled correctly");
    }

    // =========================================================================
    // db() accessor test
    // =========================================================================

    #[test]
    fn test_db_accessor() {
        let (_tmp, db) = create_temp_db();
        let raw_db = db.db();
        // Just verify we can access properties
        let path = raw_db.path();
        assert!(!path.to_string_lossy().is_empty());
    }

    // =========================================================================
    // Path accessor test
    // =========================================================================

    #[test]
    fn test_path_accessor() {
        let tmp = TempDir::new().expect("create temp dir");
        let expected_path = tmp.path().to_string_lossy().to_string();
        let db = RocksDbMemex::open(tmp.path()).expect("open failed");
        assert_eq!(db.path(), expected_path);
    }

    // =========================================================================
    // Constants Tests
    // =========================================================================

    #[test]
    fn test_default_cache_size_constant() {
        assert_eq!(DEFAULT_CACHE_SIZE, 256 * 1024 * 1024);
        assert_eq!(DEFAULT_CACHE_SIZE, 268_435_456); // 256MB in bytes
    }

    #[test]
    fn test_default_max_open_files_constant() {
        assert_eq!(DEFAULT_MAX_OPEN_FILES, 1000);
    }

    // =========================================================================
    // Integration-style Tests
    // =========================================================================

    #[test]
    fn test_full_lifecycle() {
        println!("=== TEST: Full database lifecycle ===");
        let tmp = TempDir::new().expect("create temp dir");
        let path = tmp.path().to_path_buf();

        // 1. Open
        println!("STEP 1: Opening database");
        let db = RocksDbMemex::open(&path).expect("open failed");

        // 2. Health check
        println!("STEP 2: Health check");
        assert!(db.health_check().is_ok());

        // 3. Access all CFs
        println!("STEP 3: Access all 12 CFs");
        for cf_name in cf_names::ALL {
            let _ = db.get_cf(cf_name).expect("CF should exist");
        }

        // 4. Flush
        println!("STEP 4: Flush all CFs");
        assert!(db.flush_all().is_ok());

        // 5. Verify path
        println!("STEP 5: Verify path");
        assert_eq!(db.path(), path.to_string_lossy());

        // 6. Get raw DB reference
        println!("STEP 6: Get raw DB reference");
        let _ = db.db();

        // 7. Drop (implicit close)
        println!("STEP 7: Drop database");
        drop(db);

        // 8. Reopen to verify data persistence
        println!("STEP 8: Reopen and verify");
        let db2 = RocksDbMemex::open(&path).expect("reopen failed");
        assert!(db2.health_check().is_ok());

        println!("RESULT: PASS - Full lifecycle completed successfully");
    }

    #[test]
    fn test_wal_disabled() {
        println!("=== TEST: WAL disabled configuration ===");
        let tmp = TempDir::new().expect("create temp dir");

        let config = RocksDbConfig {
            enable_wal: false,
            ..Default::default()
        };

        println!("BEFORE: Opening with WAL disabled");
        let db = RocksDbMemex::open_with_config(tmp.path(), config).expect("open failed");
        println!("AFTER: Database opened with WAL disabled");

        assert!(db.health_check().is_ok());
        println!("RESULT: PASS - Database works with WAL disabled");
    }

    #[test]
    fn test_database_files_created() {
        println!("=== TEST: Database files created on disk ===");
        let tmp = TempDir::new().expect("create temp dir");
        let path = tmp.path();

        println!("BEFORE: Directory contents: {:?}", std::fs::read_dir(path).map(|r| r.count()));

        let db = RocksDbMemex::open(path).expect("open failed");
        db.flush_all().expect("flush failed");

        println!("AFTER: Directory contents:");
        for entry in std::fs::read_dir(path).expect("read dir") {
            if let Ok(e) = entry {
                println!("  - {:?}", e.file_name());
            }
        }

        // Verify RocksDB files exist
        let has_sst_or_manifest = std::fs::read_dir(path)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .any(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.contains("MANIFEST") || name.contains("CURRENT") || name.contains("LOG")
            });

        assert!(has_sst_or_manifest, "RocksDB control files should exist");
        println!("RESULT: PASS - Database files verified on disk");

        drop(db);
    }

    // =========================================================================
    // New Error Variant Tests (TASK-M02-017)
    // =========================================================================

    #[test]
    fn test_error_not_found() {
        let error = StorageError::NotFound {
            id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        };
        let msg = error.to_string();
        assert!(msg.contains("Node not found"));
        assert!(msg.contains("550e8400"));
    }

    #[test]
    fn test_error_serialization() {
        let error = StorageError::Serialization("invalid msgpack".to_string());
        let msg = error.to_string();
        assert!(msg.contains("Serialization error"));
        assert!(msg.contains("invalid msgpack"));
    }

    #[test]
    fn test_error_validation_failed() {
        let error = StorageError::ValidationFailed("importance out of range".to_string());
        let msg = error.to_string();
        assert!(msg.contains("Validation error"));
        assert!(msg.contains("importance out of range"));
    }

    #[test]
    fn test_from_serialization_error() {
        use crate::serialization::SerializationError;
        let ser_error = SerializationError::SerializeFailed("test".to_string());
        let storage_error: StorageError = ser_error.into();
        assert!(matches!(storage_error, StorageError::Serialization(_)));
    }

    #[test]
    fn test_from_validation_error() {
        use context_graph_core::types::ValidationError;
        let val_error = ValidationError::InvalidEmbeddingDimension {
            expected: 1536,
            actual: 100,
        };
        let storage_error: StorageError = val_error.into();
        assert!(matches!(storage_error, StorageError::ValidationFailed(_)));
    }

    // =========================================================================
    // CRUD Helper Functions (TASK-M02-017)
    // Using REAL data, no mocks - per requirements
    // =========================================================================

    use context_graph_core::types::{
        EmbeddingVector, JohariQuadrant, NodeMetadata, DEFAULT_EMBEDDING_DIM,
    };

    /// Create a valid normalized embedding vector.
    /// Normalization ensures magnitude ~= 1.0 (validates per MemoryNode::validate).
    fn create_normalized_embedding(dim: usize) -> EmbeddingVector {
        let val = 1.0 / (dim as f32).sqrt();
        vec![val; dim]
    }

    /// Create a valid MemoryNode with real data that passes validate().
    fn create_valid_test_node() -> MemoryNode {
        let embedding = create_normalized_embedding(DEFAULT_EMBEDDING_DIM);
        let mut node = MemoryNode::new("Test content for CRUD operations".to_string(), embedding);
        node.importance = 0.75;
        node.emotional_valence = 0.5;
        node.quadrant = JohariQuadrant::Open;
        node.metadata = NodeMetadata::new()
            .with_source("test-source")
            .with_language("en");
        // Ensure the node is valid
        assert!(node.validate().is_ok(), "Test node must be valid");
        node
    }

    /// Create a valid MemoryNode with tags for testing tag index operations.
    fn create_node_with_tags(tags: Vec<&str>) -> MemoryNode {
        let mut node = create_valid_test_node();
        for tag in tags {
            node.metadata.add_tag(tag);
        }
        node
    }

    // =========================================================================
    // store_node Tests (TASK-M02-017)
    // =========================================================================

    #[test]
    fn test_store_node_basic() {
        println!("=== TEST: store_node basic operation ===");
        let (_tmp, db) = create_temp_db();
        let node = create_valid_test_node();

        println!("BEFORE: Storing node {}", node.id);
        let result = db.store_node(&node);
        println!("AFTER: Store result = {:?}", result.is_ok());

        assert!(result.is_ok(), "store_node should succeed: {:?}", result);
        println!("RESULT: PASS - Node stored successfully");
    }

    #[test]
    fn test_store_node_and_get_roundtrip() {
        println!("=== TEST: store_node + get_node roundtrip ===");
        let (_tmp, db) = create_temp_db();
        let node = create_valid_test_node();
        let id = node.id;

        println!("BEFORE: Storing node {}", id);
        db.store_node(&node).expect("store failed");

        println!("AFTER: Retrieving node {}", id);
        let retrieved = db.get_node(&id).expect("get failed");

        // Verify all fields
        assert_eq!(node.id, retrieved.id, "ID mismatch");
        assert_eq!(node.content, retrieved.content, "Content mismatch");
        assert_eq!(node.embedding, retrieved.embedding, "Embedding mismatch");
        assert_eq!(node.quadrant, retrieved.quadrant, "Quadrant mismatch");
        assert_eq!(node.importance, retrieved.importance, "Importance mismatch");
        assert_eq!(
            node.emotional_valence, retrieved.emotional_valence,
            "Valence mismatch"
        );
        assert_eq!(
            node.metadata.source, retrieved.metadata.source,
            "Source mismatch"
        );

        println!("RESULT: PASS - All fields preserved in roundtrip");
    }

    #[test]
    fn evidence_store_node_creates_all_indexes() {
        println!("=== EVIDENCE: store_node creates all indexes ===");
        let (_tmp, db) = create_temp_db();

        let mut node = create_valid_test_node();
        node.metadata.add_tag("important");
        node.metadata.add_tag("verified");
        node.metadata.source = Some("test-source".to_string());

        println!("BEFORE: Storing node {} with 2 tags and source", node.id);
        db.store_node(&node).expect("store failed");
        println!("AFTER: Verifying all indexes...");

        // 1. Verify node exists
        let retrieved = db.get_node(&node.id).expect("get failed");
        println!("  nodes CF: Node exists ✓");
        assert_eq!(retrieved.id, node.id);

        // 2. Verify embedding
        let cf_emb = db.get_cf(cf_names::EMBEDDINGS).unwrap();
        let node_key = serialize_uuid(&node.id);
        let emb_bytes = db.db().get_cf(cf_emb, &node_key).unwrap();
        assert!(emb_bytes.is_some(), "Embedding should exist");
        println!(
            "  embeddings CF: Embedding exists ({} bytes) ✓",
            emb_bytes.unwrap().len()
        );

        // 3. Verify johari index
        let cf_johari = db.get_cf(node.quadrant.column_family()).unwrap();
        let johari_entry = db.db().get_cf(cf_johari, &node_key).unwrap();
        assert!(johari_entry.is_some(), "Johari index should exist");
        println!(
            "  {} CF: Index entry exists ✓",
            node.quadrant.column_family()
        );

        // 4. Verify temporal index
        let cf_temporal = db.get_cf(cf_names::TEMPORAL).unwrap();
        let temporal_key = format_temporal_key(node.created_at, &node.id);
        let temporal_entry = db.db().get_cf(cf_temporal, &temporal_key).unwrap();
        assert!(temporal_entry.is_some(), "Temporal index should exist");
        println!("  temporal CF: Index entry exists ✓");

        // 5. Verify tag indexes
        let cf_tags = db.get_cf(cf_names::TAGS).unwrap();
        for tag in &node.metadata.tags {
            let tag_key = format_tag_key(tag, &node.id);
            let tag_entry = db.db().get_cf(cf_tags, &tag_key).unwrap();
            assert!(tag_entry.is_some(), "Tag index for '{}' should exist", tag);
            println!("  tags CF: '{}' index exists ✓", tag);
        }

        // 6. Verify source index
        let cf_sources = db.get_cf(cf_names::SOURCES).unwrap();
        let source_key = format_source_key(node.metadata.source.as_ref().unwrap(), &node.id);
        let source_entry = db.db().get_cf(cf_sources, &source_key).unwrap();
        assert!(source_entry.is_some(), "Source index should exist");
        println!("  sources CF: Source index exists ✓");

        println!("RESULT: All indexes verified in RocksDB ✓");
    }

    #[test]
    fn test_store_node_validation_failure() {
        println!("=== TEST: store_node validation failure (fail fast) ===");
        let (_tmp, db) = create_temp_db();

        // Create invalid node with wrong embedding dimension
        let bad_embedding = vec![0.1; 100]; // Wrong dimension (not 1536)
        let mut node = MemoryNode::new("Test".to_string(), bad_embedding);
        node.importance = 0.5; // Valid importance

        println!(
            "BEFORE: Attempting to store invalid node (dim={})",
            node.embedding.len()
        );
        let result = db.store_node(&node);
        println!("AFTER: Result = {:?}", result);

        assert!(result.is_err(), "Should fail for invalid embedding dimension");
        assert!(
            matches!(result, Err(StorageError::ValidationFailed(_))),
            "Should be ValidationFailed error"
        );
        println!("RESULT: PASS - Validation failure handled correctly (fail fast)");
    }

    // =========================================================================
    // get_node Tests (TASK-M02-017)
    // =========================================================================

    #[test]
    fn test_get_node_not_found() {
        println!("=== TEST: get_node returns NotFound for missing node ===");
        let (_tmp, db) = create_temp_db();
        let fake_id = uuid::Uuid::new_v4();

        println!("BEFORE: Querying for non-existent node {}", fake_id);
        let result = db.get_node(&fake_id);
        println!("AFTER: Result = {:?}", result);

        assert!(result.is_err(), "Should fail for missing node");
        assert!(
            matches!(result, Err(StorageError::NotFound { .. })),
            "Should be NotFound error"
        );
        println!("RESULT: PASS - NotFound returned correctly (fail fast)");
    }

    // =========================================================================
    // update_node Tests (TASK-M02-017)
    // =========================================================================

    #[test]
    fn test_update_node_basic() {
        println!("=== TEST: update_node basic operation ===");
        let (_tmp, db) = create_temp_db();
        let mut node = create_valid_test_node();

        db.store_node(&node).expect("store failed");
        println!("BEFORE: Stored node with content = {:?}", node.content);

        node.content = "Updated content".to_string();
        db.update_node(&node).expect("update failed");
        println!("AFTER: Updated content");

        let retrieved = db.get_node(&node.id).expect("get failed");
        assert_eq!(retrieved.content, "Updated content");
        println!("RESULT: PASS - Content updated successfully");
    }

    #[test]
    fn test_update_node_not_found_fails() {
        println!("=== TEST: update_node returns NotFound for missing node ===");
        let (_tmp, db) = create_temp_db();
        let node = create_valid_test_node();

        // Don't store the node - try to update directly
        println!("BEFORE: Attempting to update non-existent node {}", node.id);
        let result = db.update_node(&node);
        println!("AFTER: Result = {:?}", result);

        assert!(result.is_err(), "Should fail for missing node");
        assert!(
            matches!(result, Err(StorageError::NotFound { .. })),
            "Should be NotFound error"
        );
        println!("RESULT: PASS - NotFound returned for non-existent update (fail fast)");
    }

    #[test]
    fn edge_case_quadrant_transition() {
        println!("=== EDGE CASE EC-002: Quadrant Transition ===");
        let (_tmp, db) = create_temp_db();
        let mut node = create_valid_test_node();
        node.quadrant = JohariQuadrant::Open;
        let node_key = serialize_uuid(&node.id);

        db.store_node(&node).expect("store failed");

        // Verify in johari_open
        let cf_open = db.get_cf(cf_names::JOHARI_OPEN).unwrap();
        let before_open = db.db().get_cf(cf_open, &node_key).unwrap();
        println!(
            "BEFORE: johari_open entry exists = {}",
            before_open.is_some()
        );
        assert!(before_open.is_some(), "Should exist in johari_open");

        // Change quadrant
        node.quadrant = JohariQuadrant::Hidden;
        db.update_node(&node).expect("update failed");

        // Verify REMOVED from johari_open, ADDED to johari_hidden
        let after_open = db.db().get_cf(cf_open, &node_key).unwrap();
        let cf_hidden = db.get_cf(cf_names::JOHARI_HIDDEN).unwrap();
        let after_hidden = db.db().get_cf(cf_hidden, &node_key).unwrap();

        println!(
            "AFTER: johari_open entry exists = {}, johari_hidden entry exists = {}",
            after_open.is_some(),
            after_hidden.is_some()
        );

        assert!(after_open.is_none(), "Should be REMOVED from johari_open");
        assert!(after_hidden.is_some(), "Should be ADDED to johari_hidden");
        println!("RESULT: PASS - Quadrant transition handled correctly");
    }

    #[test]
    fn edge_case_empty_tags_update() {
        println!("=== EDGE CASE EC-001: Empty Tags Update ===");
        let (_tmp, db) = create_temp_db();

        // Create node with tags
        let mut node = create_node_with_tags(vec!["a", "b", "c"]);

        db.store_node(&node).expect("store failed");
        println!("BEFORE: Node has tags {:?}", node.metadata.tags);

        // Verify tags exist
        let cf_tags = db.get_cf(cf_names::TAGS).unwrap();
        for tag in &["a", "b", "c"] {
            let tag_key = format_tag_key(tag, &node.id);
            let exists = db.db().get_cf(cf_tags, &tag_key).unwrap().is_some();
            println!("  Tag '{}' exists: {}", tag, exists);
        }

        // Update with empty tags
        node.metadata.tags = vec![];
        db.update_node(&node).expect("update failed");
        println!("AFTER: Updated with empty tags");

        // Verify all tag indexes removed
        for tag in &["a", "b", "c"] {
            let tag_key = format_tag_key(tag, &node.id);
            let exists = db.db().get_cf(cf_tags, &tag_key).unwrap();
            assert!(exists.is_none(), "Tag '{}' should be removed", tag);
            println!("  Tag '{}' removed: ✓", tag);
        }

        // Verify node still has empty tags
        let retrieved = db.get_node(&node.id).expect("get failed");
        assert!(retrieved.metadata.tags.is_empty());

        println!("RESULT: PASS - Empty tags update handled correctly");
    }

    // =========================================================================
    // delete_node Tests (TASK-M02-017)
    // =========================================================================

    #[test]
    fn test_delete_node_not_found_fails() {
        println!("=== TEST: delete_node returns NotFound for missing node ===");
        let (_tmp, db) = create_temp_db();
        let fake_id = uuid::Uuid::new_v4();

        println!(
            "BEFORE: Attempting to delete non-existent node {}",
            fake_id
        );
        let result = db.delete_node(&fake_id, false);
        println!("AFTER: Result = {:?}", result);

        assert!(result.is_err(), "Should fail for missing node");
        assert!(
            matches!(result, Err(StorageError::NotFound { .. })),
            "Should be NotFound error"
        );
        println!("RESULT: PASS - NotFound returned for non-existent delete (fail fast)");
    }

    #[test]
    fn test_delete_node_soft_delete() {
        println!("=== TEST: delete_node soft delete (SEC-06) ===");
        let (_tmp, db) = create_temp_db();
        let node = create_valid_test_node();
        let id = node.id;

        db.store_node(&node).expect("store failed");
        println!("BEFORE: Node {} stored, deleted={}", id, node.metadata.deleted);

        db.delete_node(&id, true).expect("soft delete failed");
        println!("AFTER: Soft delete executed");

        // Node should still exist with deleted=true
        let retrieved = db.get_node(&id).expect("get should succeed after soft delete");
        assert!(
            retrieved.metadata.deleted,
            "metadata.deleted should be true"
        );
        assert!(
            retrieved.metadata.deleted_at.is_some(),
            "deleted_at should be set"
        );
        println!(
            "  metadata.deleted = {}, deleted_at = {:?}",
            retrieved.metadata.deleted, retrieved.metadata.deleted_at
        );

        println!("RESULT: PASS - Soft delete preserves data (SEC-06 compliant)");
    }

    #[test]
    fn test_delete_node_hard_delete() {
        println!("=== TEST: delete_node hard delete ===");
        let (_tmp, db) = create_temp_db();

        let mut node = create_valid_test_node();
        node.metadata.add_tag("test-tag");
        node.metadata.source = Some("test-source".to_string());
        let id = node.id;
        let node_key = serialize_uuid(&id);

        db.store_node(&node).expect("store failed");
        println!("BEFORE: Node {} stored with tag and source", id);

        // Verify exists in all CFs
        let cf_nodes = db.get_cf(cf_names::NODES).unwrap();
        let cf_emb = db.get_cf(cf_names::EMBEDDINGS).unwrap();
        let cf_johari = db.get_cf(node.quadrant.column_family()).unwrap();
        let cf_tags = db.get_cf(cf_names::TAGS).unwrap();
        let cf_sources = db.get_cf(cf_names::SOURCES).unwrap();

        assert!(db.db().get_cf(cf_nodes, &node_key).unwrap().is_some());
        assert!(db.db().get_cf(cf_emb, &node_key).unwrap().is_some());
        assert!(db.db().get_cf(cf_johari, &node_key).unwrap().is_some());
        println!("  Verified all index entries exist BEFORE delete");

        // Hard delete
        db.delete_node(&id, false).expect("hard delete failed");
        println!("AFTER: Hard delete executed");

        // Verify REMOVED from ALL CFs
        assert!(
            db.db().get_cf(cf_nodes, &node_key).unwrap().is_none(),
            "Node should be GONE from nodes CF"
        );
        println!("  nodes CF: Node removed ✓");

        assert!(
            db.db().get_cf(cf_emb, &node_key).unwrap().is_none(),
            "Embedding should be GONE"
        );
        println!("  embeddings CF: Embedding removed ✓");

        assert!(
            db.db().get_cf(cf_johari, &node_key).unwrap().is_none(),
            "Johari index should be GONE"
        );
        println!("  {} CF: Index removed ✓", node.quadrant.column_family());

        // Verify tag index removed
        let tag_key = format_tag_key("test-tag", &id);
        assert!(
            db.db().get_cf(cf_tags, &tag_key).unwrap().is_none(),
            "Tag index should be GONE"
        );
        println!("  tags CF: Tag index removed ✓");

        // Verify source index removed
        let source_key = format_source_key("test-source", &id);
        assert!(
            db.db().get_cf(cf_sources, &source_key).unwrap().is_none(),
            "Source index should be GONE"
        );
        println!("  sources CF: Source index removed ✓");

        // Verify get_node returns NotFound
        let result = db.get_node(&id);
        assert!(
            matches!(result, Err(StorageError::NotFound { .. })),
            "get_node should return NotFound"
        );
        println!("  get_node returns NotFound ✓");

        println!("RESULT: PASS - Hard delete removed from all CFs");
    }

    // =========================================================================
    // Edge Case Tests (TASK-M02-017 - EC-003)
    // =========================================================================

    #[test]
    fn edge_case_unicode_content() {
        println!("=== EDGE CASE EC-003: Unicode Content ===");
        let (_tmp, db) = create_temp_db();

        let mut node = create_valid_test_node();
        node.content = "日本語 🎉 émojis λ α β γ δ ε ζ".to_string();

        println!("BEFORE: content = {:?}", node.content);
        println!("BEFORE: content.len() = {} bytes", node.content.len());

        db.store_node(&node).expect("store failed");
        let retrieved = db.get_node(&node.id).expect("get failed");

        println!("AFTER: content = {:?}", retrieved.content);
        println!("AFTER: content.len() = {} bytes", retrieved.content.len());

        assert_eq!(node.content, retrieved.content);
        println!("RESULT: PASS - Unicode content preserved");
    }

    // =========================================================================
    // Helper Function Tests (TASK-M02-017)
    // =========================================================================

    #[test]
    fn test_format_temporal_key() {
        let id = uuid::Uuid::new_v4();
        let timestamp = Utc::now();

        let key = format_temporal_key(timestamp, &id);

        assert_eq!(key.len(), 24, "Temporal key should be 8+16 bytes");

        // First 8 bytes are timestamp (big-endian)
        let millis_bytes: [u8; 8] = key[0..8].try_into().unwrap();
        let millis = u64::from_be_bytes(millis_bytes);
        assert_eq!(millis, timestamp.timestamp_millis() as u64);

        // Last 16 bytes are UUID
        let uuid_bytes: [u8; 16] = key[8..24].try_into().unwrap();
        assert_eq!(uuid_bytes, serialize_uuid(&id));
    }

    #[test]
    fn test_format_tag_key() {
        let id = uuid::Uuid::new_v4();
        let tag = "important";

        let key = format_tag_key(tag, &id);

        assert_eq!(key.len(), 9 + 1 + 16, "Tag key should be tag_len + 1 + 16");

        // Verify structure: tag_bytes + ':' + uuid_bytes
        assert!(key.starts_with(tag.as_bytes()));
        assert_eq!(key[9], b':');
    }

    #[test]
    fn test_format_source_key() {
        let id = uuid::Uuid::new_v4();
        let source = "web-scraper";

        let key = format_source_key(source, &id);

        assert_eq!(
            key.len(),
            11 + 1 + 16,
            "Source key should be source_len + 1 + 16"
        );

        // Verify structure: source_bytes + ':' + uuid_bytes
        assert!(key.starts_with(source.as_bytes()));
        assert_eq!(key[11], b':');
    }

    // =========================================================================
    // Performance Sanity Tests (TASK-M02-017)
    // =========================================================================

    #[test]
    fn test_store_get_performance_sanity() {
        println!("=== PERFORMANCE: store_node + get_node timing ===");
        let (_tmp, db) = create_temp_db();
        let node = create_valid_test_node();

        // Warm up
        db.store_node(&node).unwrap();
        db.get_node(&node.id).unwrap();
        db.delete_node(&node.id, false).unwrap();

        // Time store
        let node2 = create_valid_test_node();
        let start = std::time::Instant::now();
        db.store_node(&node2).unwrap();
        let store_time = start.elapsed();

        // Time get
        let start = std::time::Instant::now();
        let _ = db.get_node(&node2.id).unwrap();
        let get_time = start.elapsed();

        println!("  store_node: {:?}", store_time);
        println!("  get_node: {:?}", get_time);

        // Sanity check: should be < 100ms at least (real target is <1ms/<500μs)
        assert!(
            store_time.as_millis() < 100,
            "store_node too slow: {:?}",
            store_time
        );
        assert!(
            get_time.as_millis() < 100,
            "get_node too slow: {:?}",
            get_time
        );

        println!("RESULT: PASS - Performance within sanity bounds");
    }
}
