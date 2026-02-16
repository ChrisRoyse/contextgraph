//! MemoryStore: RocksDB-backed storage for Memory structs.
//!
//! This module provides persistent storage for Memory structs with CRUD operations
//! and session-based indexing.
//!
//! # Architecture
//!
//! ```text
//! MemoryStore
//! ├── DB (RocksDB instance)
//! │   ├── CF: memories       - Primary Memory storage (key: UUID bytes)
//! │   └── CF: session_index  - Session -> Vec<UUID> index
//! └── Arc wrapper for thread-safe sharing
//! ```
//!
//! # Constitution Compliance
//!
//! - ARCH-01: TeleologicalArray is atomic (stored as part of Memory)
//! - AP-14: No .unwrap() in library code - all errors propagated
//! - rust_standards.error_handling: thiserror for library errors
//!
//! # Example
//!
//! ```ignore
//! use context_graph_core::memory::{Memory, MemoryStore, MemorySource, HookType};
//! use tempfile::tempdir;
//!
//! let tmp = tempdir().unwrap();
//! let store = MemoryStore::new(tmp.path()).unwrap();
//!
//! // Store a memory
//! store.store(&memory).unwrap();
//!
//! // Retrieve by ID
//! let retrieved = store.get(memory.id).unwrap();
//! ```

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteBatch, DB};
use thiserror::Error;
use tracing::error;
use uuid::Uuid;

use super::{Memory, MemorySource};

/// Column family name for primary Memory storage.
const CF_MEMORIES: &str = "memories";

/// Column family name for session -> memories index.
const CF_SESSION_INDEX: &str = "session_index";

/// Column family name for file_path -> memories index.
/// Used for efficient lookup and deletion of memories by source file.
const CF_FILE_INDEX: &str = "file_index";

/// Storage error types for MemoryStore operations.
///
/// All errors include enough context for debugging. Errors are fail-fast -
/// no retries or fallbacks at this layer.
///
/// # Constitution Compliance
///
/// - rust_standards.error_handling: thiserror for library errors
/// - AP-14: No .unwrap() - errors propagated via Result
#[derive(Debug, Error)]
pub enum StorageError {
    /// Database initialization failed.
    ///
    /// Occurs when RocksDB cannot be opened at the specified path.
    /// Common causes: path doesn't exist, permission denied, disk full.
    #[error("Database initialization failed: {0}")]
    InitFailed(String),

    /// Serialization or deserialization failed.
    ///
    /// Occurs when bincode cannot serialize/deserialize a Memory struct.
    /// Could indicate data corruption or schema mismatch.
    #[error("Serialization failed: {0}")]
    SerializationFailed(String),

    /// Database write operation failed.
    ///
    /// Occurs when RocksDB cannot write data.
    /// Common causes: disk full, I/O error, database closed.
    #[error("Database write failed: {0}")]
    WriteFailed(String),

    /// Database read operation failed.
    ///
    /// Occurs when RocksDB cannot read data.
    /// Common causes: I/O error, corruption, database closed.
    #[error("Database read failed: {0}")]
    ReadFailed(String),

    /// Required column family not found.
    ///
    /// Should never occur in normal operation - indicates database schema mismatch.
    #[error("Column family not found: {0}")]
    ColumnFamilyNotFound(String),
}

/// RocksDB-backed storage for Memory structs.
///
/// Provides CRUD operations with session-based indexing. Thread-safe via Arc<DB>.
///
/// # Column Families
///
/// - `memories`: Primary storage, key = UUID bytes (16 bytes), value = bincode(Memory)
/// - `session_index`: Secondary index, key = session_id bytes, value = bincode(Vec<Uuid>)
///
/// # Thread Safety
///
/// The underlying RocksDB is thread-safe. Multiple threads can call methods
/// on the same MemoryStore instance concurrently.
///
/// # Not Async
///
/// All methods are synchronous. For async contexts, wrap calls in `spawn_blocking`.
/// RocksDB-backed storage for Memory structs.
///
/// Provides CRUD operations with session-based indexing. Thread-safe via Arc<DB>
/// and Mutex-protected secondary index updates.
pub struct MemoryStore {
    /// RocksDB database instance wrapped in Arc for thread-safe sharing.
    db: Arc<DB>,
    /// DATA-6 FIX: Mutex to protect read-modify-write cycles on secondary indexes
    /// (session_index, file_index). Without this, concurrent writes to the same
    /// session can lose index entries via the classic lost-update pattern.
    /// Uses parking_lot::Mutex (non-poisoning) per constitution ARCH-CONC-01.
    index_lock: Mutex<()>,
}

impl std::fmt::Debug for MemoryStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryStore")
            .field("db", &"Arc<DB>")
            .finish()
    }
}

impl MemoryStore {
    /// Initialize RocksDB with column families.
    ///
    /// Creates a new database at the specified path with `memories` and `session_index`
    /// column families. Creates the directory if it doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the database directory
    ///
    /// # Returns
    ///
    /// * `Ok(Self)` - Successfully opened database
    /// * `Err(StorageError::InitFailed)` - Database could not be opened
    ///
    /// # Fails Fast
    ///
    /// Returns error immediately if path is invalid or DB cannot be opened.
    /// No fallbacks or retries.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use context_graph_core::memory::MemoryStore;
    /// use tempfile::tempdir;
    ///
    /// let tmp = tempdir().unwrap();
    /// let store = MemoryStore::new(tmp.path()).unwrap();
    /// ```
    pub fn new(path: &Path) -> Result<Self, StorageError> {
        let path_str = path.to_string_lossy().to_string();

        // Create DB options
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);

        // Create column family descriptors
        let cf_opts = Options::default();
        let cf_descriptors = vec![
            ColumnFamilyDescriptor::new(CF_MEMORIES, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_SESSION_INDEX, cf_opts.clone()),
            ColumnFamilyDescriptor::new(CF_FILE_INDEX, cf_opts),
        ];

        // Open database with column families
        let db = DB::open_cf_descriptors(&db_opts, &path_str, cf_descriptors).map_err(|e| {
            error!(
                path = %path_str,
                error = %e,
                "Failed to open RocksDB for MemoryStore"
            );
            StorageError::InitFailed(format!("path={}: {}", path_str, e))
        })?;

        Ok(Self {
            db: Arc::new(db),
            index_lock: Mutex::new(()),
        })
    }

    /// Store a Memory and update session index.
    ///
    /// Persists the Memory to the `memories` column family and updates the
    /// `session_index` to include this memory's ID under its session.
    ///
    /// # Arguments
    ///
    /// * `memory` - The Memory to store
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Memory stored successfully
    /// * `Err(StorageError)` - Storage failed (serialization or write error)
    ///
    /// # Atomicity
    ///
    /// The memory and index update are NOT in a single transaction. If the
    /// index update fails after the memory is written, the memory will exist
    /// but may not be found via session query. This is acceptable for Phase 1.
    ///
    /// # Idempotency
    ///
    /// Storing the same memory twice overwrites the previous value. The session
    /// index avoids duplicates.
    pub fn store(&self, memory: &Memory) -> Result<(), StorageError> {
        // Get column family handles
        let cf_memories = self
            .db
            .cf_handle(CF_MEMORIES)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound(CF_MEMORIES.to_string()))?;

        let cf_session_index = self
            .db
            .cf_handle(CF_SESSION_INDEX)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound(CF_SESSION_INDEX.to_string()))?;

        // Serialize memory
        let memory_bytes = bincode::serialize(memory).map_err(|e| {
            error!(
                memory_id = %memory.id,
                error = %e,
                "Failed to serialize Memory"
            );
            StorageError::SerializationFailed(format!("memory_id={}: {}", memory.id, e))
        })?;

        // DATA-1 + DATA-6 FIX: Use WriteBatch for atomicity and hold index_lock
        // to prevent lost-update race on secondary indexes.
        let _guard = self.index_lock.lock();

        let mut batch = WriteBatch::default();

        // Write memory to primary CF
        batch.put_cf(cf_memories, memory.id.as_bytes(), &memory_bytes);

        // Update session index (read-modify-write under lock)
        let session_key = memory.session_id.as_bytes();
        let mut session_ids: Vec<Uuid> = match self.db.get_cf(cf_session_index, session_key) {
            Ok(Some(bytes)) => bincode::deserialize(&bytes).map_err(|e| {
                error!(
                    session_id = %memory.session_id,
                    error = %e,
                    "Failed to deserialize session index"
                );
                StorageError::SerializationFailed(format!(
                    "session_index for '{}': {}",
                    memory.session_id, e
                ))
            })?,
            Ok(None) => Vec::new(),
            Err(e) => {
                error!(
                    session_id = %memory.session_id,
                    error = %e,
                    "Failed to read session index"
                );
                return Err(StorageError::ReadFailed(format!(
                    "session_index for '{}': {}",
                    memory.session_id, e
                )));
            }
        };

        if !session_ids.contains(&memory.id) {
            session_ids.push(memory.id);
            let index_bytes = bincode::serialize(&session_ids).map_err(|e| {
                StorageError::SerializationFailed(format!(
                    "session_index for '{}': {}",
                    memory.session_id, e
                ))
            })?;
            batch.put_cf(cf_session_index, session_key, &index_bytes);
        }

        // Update file index for MDFileChunk sources (also in same batch + lock)
        if let MemorySource::MDFileChunk { ref file_path, .. } = memory.source {
            self.add_to_file_index_batch(&mut batch, file_path, memory.id)?;
        }

        // Atomic commit — all-or-nothing
        self.db.write(batch).map_err(|e| {
            error!(
                memory_id = %memory.id,
                error = %e,
                "Failed to write atomic batch for Memory store"
            );
            StorageError::WriteFailed(format!("atomic_store memory_id={}: {}", memory.id, e))
        })?;

        Ok(())
    }

    /// Add a memory ID to the file index within a WriteBatch (for atomic store).
    /// Caller must hold index_lock.
    fn add_to_file_index_batch(
        &self,
        batch: &mut WriteBatch,
        file_path: &str,
        memory_id: Uuid,
    ) -> Result<(), StorageError> {
        let cf_file_index = self
            .db
            .cf_handle(CF_FILE_INDEX)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound(CF_FILE_INDEX.to_string()))?;

        let file_key = file_path.as_bytes();

        let mut file_ids: Vec<Uuid> = match self.db.get_cf(cf_file_index, file_key) {
            Ok(Some(bytes)) => bincode::deserialize(&bytes).map_err(|e| {
                error!(
                    file_path = %file_path,
                    error = %e,
                    "Failed to deserialize file index"
                );
                StorageError::SerializationFailed(format!("file_index for '{}': {}", file_path, e))
            })?,
            Ok(None) => Vec::new(),
            Err(e) => {
                error!(
                    file_path = %file_path,
                    error = %e,
                    "Failed to read file index"
                );
                return Err(StorageError::ReadFailed(format!(
                    "file_index for '{}': {}",
                    file_path, e
                )));
            }
        };

        if !file_ids.contains(&memory_id) {
            file_ids.push(memory_id);
            let index_bytes = bincode::serialize(&file_ids).map_err(|e| {
                StorageError::SerializationFailed(format!("file_index for '{}': {}", file_path, e))
            })?;
            batch.put_cf(cf_file_index, file_key, &index_bytes);
        }

        Ok(())
    }

    /// Get Memory by UUID.
    ///
    /// # Arguments
    ///
    /// * `id` - The UUID of the Memory to retrieve
    ///
    /// # Returns
    ///
    /// * `Ok(Some(Memory))` - Memory found and returned
    /// * `Ok(None)` - No Memory exists with this ID
    /// * `Err(StorageError)` - Read or deserialization failed
    pub fn get(&self, id: Uuid) -> Result<Option<Memory>, StorageError> {
        let cf_memories = self
            .db
            .cf_handle(CF_MEMORIES)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound(CF_MEMORIES.to_string()))?;

        match self.db.get_cf(cf_memories, id.as_bytes()) {
            Ok(Some(bytes)) => {
                let memory: Memory = bincode::deserialize(&bytes).map_err(|e| {
                    error!(
                        memory_id = %id,
                        error = %e,
                        "Failed to deserialize Memory"
                    );
                    StorageError::SerializationFailed(format!("memory_id={}: {}", id, e))
                })?;
                Ok(Some(memory))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                error!(
                    memory_id = %id,
                    error = %e,
                    "Failed to read Memory from DB"
                );
                Err(StorageError::ReadFailed(format!("memory_id={}: {}", id, e)))
            }
        }
    }

    /// Get all memories for a session.
    ///
    /// Retrieves all memories associated with the given session ID by looking up
    /// the session index and then fetching each memory.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session identifier
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<Memory>)` - All memories for the session (may be empty)
    /// * `Err(StorageError)` - Read or deserialization failed
    ///
    /// # Note
    ///
    /// If a memory ID in the index no longer exists (orphaned reference),
    /// it is silently skipped. This provides resilience against partial deletions.
    pub fn get_by_session(&self, session_id: &str) -> Result<Vec<Memory>, StorageError> {
        let cf_session_index = self
            .db
            .cf_handle(CF_SESSION_INDEX)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound(CF_SESSION_INDEX.to_string()))?;

        let session_key = session_id.as_bytes();

        // Read session index
        let memory_ids: Vec<Uuid> = match self.db.get_cf(cf_session_index, session_key) {
            Ok(Some(bytes)) => bincode::deserialize(&bytes).map_err(|e| {
                error!(
                    session_id = %session_id,
                    error = %e,
                    "Failed to deserialize session index"
                );
                StorageError::SerializationFailed(format!(
                    "session_index for '{}': {}",
                    session_id, e
                ))
            })?,
            Ok(None) => return Ok(Vec::new()),
            Err(e) => {
                error!(
                    session_id = %session_id,
                    error = %e,
                    "Failed to read session index"
                );
                return Err(StorageError::ReadFailed(format!(
                    "session_index for '{}': {}",
                    session_id, e
                )));
            }
        };

        // Fetch each memory, skipping orphaned references
        let mut memories = Vec::with_capacity(memory_ids.len());
        for id in memory_ids {
            if let Some(memory) = self.get(id)? {
                memories.push(memory);
            }
            // Silently skip if memory was deleted but index not updated
        }

        Ok(memories)
    }

    /// Count total memories in store.
    ///
    /// Iterates over the `memories` column family to count all entries.
    ///
    /// # Returns
    ///
    /// * `Ok(u64)` - Total count of memories
    /// * `Err(StorageError)` - Column family not found
    pub fn count(&self) -> Result<u64, StorageError> {
        let cf_memories = self
            .db
            .cf_handle(CF_MEMORIES)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound(CF_MEMORIES.to_string()))?;

        let iter = self.db.iterator_cf(cf_memories, IteratorMode::Start);
        let count = iter.count() as u64;

        Ok(count)
    }

    /// Delete a memory by ID.
    ///
    /// Removes the memory from the `memories` column family and updates the
    /// session index to remove the memory ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The UUID of the Memory to delete
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Memory was found and deleted
    /// * `Ok(false)` - No memory exists with this ID
    /// * `Err(StorageError)` - Delete operation failed
    pub fn delete(&self, id: Uuid) -> Result<bool, StorageError> {
        let cf_memories = self
            .db
            .cf_handle(CF_MEMORIES)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound(CF_MEMORIES.to_string()))?;

        let cf_session_index = self
            .db
            .cf_handle(CF_SESSION_INDEX)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound(CF_SESSION_INDEX.to_string()))?;

        // First, get the memory to find its session_id
        let memory = match self.get(id)? {
            Some(m) => m,
            None => return Ok(false),
        };

        // DATA-1 + DATA-6 FIX: Atomic delete with index_lock protection
        let _guard = self.index_lock.lock();
        let mut batch = WriteBatch::default();

        // Delete from memories CF
        batch.delete_cf(cf_memories, id.as_bytes());

        // Update session index atomically
        let session_key = memory.session_id.as_bytes();
        if let Some(bytes) = self
            .db
            .get_cf(cf_session_index, session_key)
            .map_err(|e| StorageError::ReadFailed(e.to_string()))?
        {
            let mut session_ids: Vec<Uuid> = bincode::deserialize(&bytes).map_err(|e| {
                StorageError::SerializationFailed(format!(
                    "session_index for '{}': {}",
                    memory.session_id, e
                ))
            })?;

            session_ids.retain(|&stored_id| stored_id != id);

            let index_bytes = bincode::serialize(&session_ids).map_err(|e| {
                StorageError::SerializationFailed(format!(
                    "session_index for '{}': {}",
                    memory.session_id, e
                ))
            })?;

            batch.put_cf(cf_session_index, session_key, &index_bytes);
        }

        // Atomic commit
        self.db.write(batch).map_err(|e| {
            error!(
                memory_id = %id,
                error = %e,
                "Failed to write atomic batch for Memory delete"
            );
            StorageError::WriteFailed(format!("atomic_delete memory_id={}: {}", id, e))
        })?;

        Ok(true)
    }

    /// Delete all memories associated with a file path.
    ///
    /// Used to clear stale embeddings when a markdown file is modified.
    /// This is a hard delete - data is permanently removed (no soft-delete
    /// 30-day recovery per SEC-06 since these are stale embeddings).
    ///
    /// # Arguments
    ///
    /// * `file_path` - The file path to delete memories for
    ///
    /// # Returns
    ///
    /// * `Ok(usize)` - Number of memories deleted
    /// * `Err(StorageError)` - Deletion failed
    ///
    /// # Atomicity Note
    ///
    /// This operation is NOT atomic. If it fails partway through,
    /// some memories may be deleted while others remain. The file
    /// index is cleared after all memory deletes succeed.
    pub fn delete_by_file_path(&self, file_path: &str) -> Result<usize, StorageError> {
        let cf_file_index = self
            .db
            .cf_handle(CF_FILE_INDEX)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound(CF_FILE_INDEX.to_string()))?;

        let file_key = file_path.as_bytes();

        // Get all memory IDs for this file
        let memory_ids: Vec<Uuid> = match self.db.get_cf(cf_file_index, file_key) {
            Ok(Some(bytes)) => bincode::deserialize(&bytes).map_err(|e| {
                error!(
                    file_path = %file_path,
                    error = %e,
                    "Failed to deserialize file index for delete"
                );
                StorageError::SerializationFailed(format!("file_index for '{}': {}", file_path, e))
            })?,
            Ok(None) => {
                // No memories for this file - nothing to delete
                return Ok(0);
            }
            Err(e) => {
                error!(
                    file_path = %file_path,
                    error = %e,
                    "Failed to read file index for delete"
                );
                return Err(StorageError::ReadFailed(format!(
                    "file_index for '{}': {}",
                    file_path, e
                )));
            }
        };

        // Delete each memory
        let mut deleted = 0;
        for id in &memory_ids {
            if self.delete(*id)? {
                deleted += 1;
            }
        }

        // Clear file index entry
        self.db
            .delete_cf(cf_file_index, file_key)
            .map_err(|e| {
                error!(
                    file_path = %file_path,
                    error = %e,
                    "Failed to delete file index entry"
                );
                StorageError::WriteFailed(format!("file_index delete for '{}': {}", file_path, e))
            })?;

        Ok(deleted)
    }

    /// Get all memories for a specific file path.
    ///
    /// Retrieves all memories associated with the given file path by looking up
    /// the file index and then fetching each memory.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The file path to get memories for
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<Memory>)` - All memories for the file (may be empty)
    /// * `Err(StorageError)` - Read or deserialization failed
    ///
    /// # Note
    ///
    /// If a memory ID in the index no longer exists (orphaned reference),
    /// it is silently skipped. This provides resilience against partial deletions.
    pub fn get_by_file_path(&self, file_path: &str) -> Result<Vec<Memory>, StorageError> {
        let cf_file_index = self
            .db
            .cf_handle(CF_FILE_INDEX)
            .ok_or_else(|| StorageError::ColumnFamilyNotFound(CF_FILE_INDEX.to_string()))?;

        let file_key = file_path.as_bytes();

        // Read file index
        let memory_ids: Vec<Uuid> = match self.db.get_cf(cf_file_index, file_key) {
            Ok(Some(bytes)) => bincode::deserialize(&bytes).map_err(|e| {
                error!(
                    file_path = %file_path,
                    error = %e,
                    "Failed to deserialize file index"
                );
                StorageError::SerializationFailed(format!("file_index for '{}': {}", file_path, e))
            })?,
            Ok(None) => return Ok(Vec::new()),
            Err(e) => {
                error!(
                    file_path = %file_path,
                    error = %e,
                    "Failed to read file index"
                );
                return Err(StorageError::ReadFailed(format!(
                    "file_index for '{}': {}",
                    file_path, e
                )));
            }
        };

        // Fetch each memory, skipping orphaned references
        let mut memories = Vec::with_capacity(memory_ids.len());
        for id in memory_ids {
            if let Some(memory) = self.get(id)? {
                memories.push(memory);
            }
            // Silently skip if memory was deleted but index not updated
        }

        Ok(memories)
    }

    /// Get the underlying database reference for testing/diagnostics.
    ///
    /// # Warning
    ///
    /// Direct DB access bypasses validation. Only use for tests and diagnostics.
    #[cfg(test)]
    #[doc(hidden)]
    pub fn db(&self) -> &DB {
        &self.db
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{HookType, MemorySource};
    use crate::types::fingerprint::SemanticFingerprint;
    use tempfile::tempdir;

    // Helper to create test fingerprint
    #[cfg(feature = "test-utils")]
    fn test_fingerprint() -> crate::types::fingerprint::TeleologicalArray {
        SemanticFingerprint::zeroed()
    }

    // Helper to create test memory
    #[cfg(feature = "test-utils")]
    fn create_test_memory(content: &str, session_id: &str) -> Memory {
        Memory::new(
            content.to_string(),
            MemorySource::HookDescription {
                hook_type: HookType::SessionStart,
                tool_name: None,
            },
            session_id.to_string(),
            test_fingerprint(),
            None,
        )
    }

    #[test]
    fn test_new_creates_db() {
        let tmp = tempdir().expect("create temp dir");
        let store = MemoryStore::new(tmp.path());
        assert!(store.is_ok(), "Failed to create MemoryStore: {:?}", store);

        // Verify DB files exist
        let db_path = tmp.path();
        assert!(db_path.exists(), "DB path should exist");

        // Check for RocksDB files
        let entries: Vec<_> = std::fs::read_dir(db_path)
            .expect("read dir")
            .filter_map(|e| e.ok())
            .collect();
        assert!(!entries.is_empty(), "DB directory should have files");
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_store_and_get() {
        let tmp = tempdir().expect("create temp dir");
        let store = MemoryStore::new(tmp.path()).expect("create store");

        let memory = create_test_memory("Test memory content", "session-001");
        let memory_id = memory.id;

        // Store
        store.store(&memory).expect("store memory");

        // Get
        let retrieved = store.get(memory_id).expect("get memory");
        assert!(retrieved.is_some(), "Memory should be found");

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.id, memory_id);
        assert_eq!(retrieved.content, "Test memory content");
        assert_eq!(retrieved.session_id, "session-001");
    }

    #[test]
    fn test_get_nonexistent() {
        let tmp = tempdir().expect("create temp dir");
        let store = MemoryStore::new(tmp.path()).expect("create store");

        let random_id = Uuid::new_v4();
        let result = store.get(random_id).expect("get should not error");
        assert!(result.is_none(), "Should return None for non-existent ID");
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_get_by_session() {
        let tmp = tempdir().expect("create temp dir");
        let store = MemoryStore::new(tmp.path()).expect("create store");

        // Store multiple memories in same session
        let mem1 = create_test_memory("Memory 1", "test-session");
        let mem2 = create_test_memory("Memory 2", "test-session");
        let mem3 = create_test_memory("Memory 3", "other-session");

        store.store(&mem1).expect("store mem1");
        store.store(&mem2).expect("store mem2");
        store.store(&mem3).expect("store mem3");

        // Get by session
        let test_mems = store
            .get_by_session("test-session")
            .expect("get by session");
        assert_eq!(test_mems.len(), 2, "Should have 2 memories in test-session");
        assert!(
            test_mems.iter().any(|m| m.id == mem1.id),
            "Should contain mem1"
        );
        assert!(
            test_mems.iter().any(|m| m.id == mem2.id),
            "Should contain mem2"
        );

        let other_mems = store.get_by_session("other-session").expect("get other");
        assert_eq!(other_mems.len(), 1, "Should have 1 memory in other-session");
        assert_eq!(other_mems[0].id, mem3.id);
    }

    #[test]
    fn test_get_by_session_empty() {
        let tmp = tempdir().expect("create temp dir");
        let store = MemoryStore::new(tmp.path()).expect("create store");

        let result = store.get_by_session("nonexistent-session").expect("get");
        assert!(
            result.is_empty(),
            "Should return empty vec for unknown session"
        );
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_count() {
        let tmp = tempdir().expect("create temp dir");
        let store = MemoryStore::new(tmp.path()).expect("create store");

        assert_eq!(store.count().expect("count"), 0, "Empty store count = 0");

        let mem1 = create_test_memory("Memory 1", "session");
        store.store(&mem1).expect("store");
        assert_eq!(store.count().expect("count"), 1, "After 1 store, count = 1");

        let mem2 = create_test_memory("Memory 2", "session");
        store.store(&mem2).expect("store");
        assert_eq!(
            store.count().expect("count"),
            2,
            "After 2 stores, count = 2"
        );
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_delete() {
        let tmp = tempdir().expect("create temp dir");
        let store = MemoryStore::new(tmp.path()).expect("create store");

        let memory = create_test_memory("To be deleted", "session");
        let memory_id = memory.id;

        store.store(&memory).expect("store");
        assert_eq!(store.count().expect("count"), 1);

        // Delete
        let deleted = store.delete(memory_id).expect("delete");
        assert!(deleted, "Should return true for successful delete");

        // Verify deleted
        let result = store.get(memory_id).expect("get");
        assert!(result.is_none(), "Memory should be gone after delete");
        assert_eq!(store.count().expect("count"), 0, "Count should be 0");

        // Verify session index updated
        let session_mems = store.get_by_session("session").expect("get session");
        assert!(session_mems.is_empty(), "Session should have no memories");
    }

    #[test]
    fn test_delete_nonexistent() {
        let tmp = tempdir().expect("create temp dir");
        let store = MemoryStore::new(tmp.path()).expect("create store");

        let random_id = Uuid::new_v4();
        let result = store.delete(random_id).expect("delete");
        assert!(!result, "Should return false for non-existent ID");
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_persistence_across_reopen() {
        let tmp = tempdir().expect("create temp dir");
        let path = tmp.path().to_path_buf();

        let memory_id;
        let memory_content = "Persistent memory content";

        // First instance: store memory
        {
            let store = MemoryStore::new(&path).expect("create store");
            let memory = create_test_memory(memory_content, "persist-session");
            memory_id = memory.id;
            store.store(&memory).expect("store");
        }
        // store is dropped here, DB closed

        // Second instance: verify data persists
        {
            let store = MemoryStore::new(&path).expect("reopen store");

            let retrieved = store.get(memory_id).expect("get");
            assert!(retrieved.is_some(), "Memory should persist across reopen");

            let retrieved = retrieved.unwrap();
            assert_eq!(retrieved.content, memory_content);
            assert_eq!(store.count().expect("count"), 1);

            let session_mems = store
                .get_by_session("persist-session")
                .expect("get session");
            assert_eq!(session_mems.len(), 1, "Session index should persist");
        }
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_multiple_sessions_isolation() {
        let tmp = tempdir().expect("create temp dir");
        let store = MemoryStore::new(tmp.path()).expect("create store");

        // Store memories in different sessions
        let session_a1 = create_test_memory("A1", "session-A");
        let session_a2 = create_test_memory("A2", "session-A");
        let session_b1 = create_test_memory("B1", "session-B");
        let session_c1 = create_test_memory("C1", "session-C");

        store.store(&session_a1).expect("store");
        store.store(&session_a2).expect("store");
        store.store(&session_b1).expect("store");
        store.store(&session_c1).expect("store");

        // Verify isolation
        let a_mems = store.get_by_session("session-A").expect("get A");
        assert_eq!(a_mems.len(), 2);

        let b_mems = store.get_by_session("session-B").expect("get B");
        assert_eq!(b_mems.len(), 1);

        let c_mems = store.get_by_session("session-C").expect("get C");
        assert_eq!(c_mems.len(), 1);

        let d_mems = store.get_by_session("session-D").expect("get D");
        assert_eq!(d_mems.len(), 0);
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_store_duplicate_id() {
        let tmp = tempdir().expect("create temp dir");
        let store = MemoryStore::new(tmp.path()).expect("create store");

        let memory = create_test_memory("Original content", "session");
        let memory_id = memory.id;
        store.store(&memory).expect("store");

        // Create new memory with same ID but different content
        let updated = Memory::with_id(
            memory_id,
            "Updated content".to_string(),
            MemorySource::HookDescription {
                hook_type: HookType::PostToolUse,
                tool_name: Some("Edit".to_string()),
            },
            "session".to_string(),
            test_fingerprint(),
            None,
        );

        store.store(&updated).expect("store updated");

        // Should only have 1 memory
        assert_eq!(store.count().expect("count"), 1);

        // Should have updated content
        let retrieved = store.get(memory_id).expect("get").expect("should exist");
        assert_eq!(retrieved.content, "Updated content");

        // Session index should not have duplicates
        let session_mems = store.get_by_session("session").expect("get session");
        assert_eq!(
            session_mems.len(),
            1,
            "Should not have duplicate in session"
        );
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn test_serialization_roundtrip() {
        let tmp = tempdir().expect("create temp dir");
        let store = MemoryStore::new(tmp.path()).expect("create store");

        // Create memory with all fields populated
        let memory = Memory::new(
            "Full memory test with all fields".to_string(),
            MemorySource::HookDescription {
                hook_type: HookType::UserPromptSubmit,
                tool_name: Some("Test Tool".to_string()),
            },
            "roundtrip-session".to_string(),
            test_fingerprint(),
            Some(crate::memory::ChunkMetadata {
                file_path: "/path/to/file.md".to_string(),
                chunk_index: 3,
                total_chunks: 10,
                word_offset: 150,
                char_offset: 1000,
                original_file_hash: "abc123def456".to_string(),
                start_line: 30,
                end_line: 50,
            }),
        );

        let memory_id = memory.id;
        store.store(&memory).expect("store");

        let retrieved = store.get(memory_id).expect("get").expect("should exist");

        // Verify all fields
        assert_eq!(retrieved.id, memory.id);
        assert_eq!(retrieved.content, memory.content);
        assert_eq!(retrieved.session_id, memory.session_id);
        assert_eq!(retrieved.word_count, memory.word_count);

        // Verify chunk metadata
        let meta = retrieved.chunk_metadata.expect("should have metadata");
        assert_eq!(meta.file_path, "/path/to/file.md");
        assert_eq!(meta.chunk_index, 3);
        assert_eq!(meta.total_chunks, 10);
    }

    // ========== EDGE CASE TESTS ==========

    #[test]
    fn edge_case_empty_database_operations() {
        println!("=== EDGE CASE: Empty database ===");
        println!("BEFORE: Fresh MemoryStore with no data");

        let tmp = tempdir().expect("create temp dir");
        let store = MemoryStore::new(tmp.path()).expect("create store");

        // get(random_uuid) should return Ok(None)
        let random_id = Uuid::new_v4();
        let get_result = store.get(random_id).expect("get should not error");
        println!("get(random_uuid): {:?}", get_result);
        assert!(get_result.is_none(), "get should return None");

        // get_by_session("unknown") should return Ok(vec![])
        let session_result = store.get_by_session("unknown").expect("get_by_session");
        println!("get_by_session('unknown'): {:?}", session_result);
        assert!(
            session_result.is_empty(),
            "get_by_session should return empty vec"
        );

        // count() should return Ok(0)
        let count = store.count().expect("count");
        println!("count(): {}", count);
        assert_eq!(count, 0, "count should return 0");

        println!("AFTER: Database state unchanged");
        println!("RESULT: PASS - Empty database edge case handled");
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn edge_case_large_content() {
        println!("=== EDGE CASE: Large content (10,000 chars) ===");

        let tmp = tempdir().expect("create temp dir");
        let store = MemoryStore::new(tmp.path()).expect("create store");

        // Create memory with MAX_CONTENT_LENGTH content
        let large_content = "x".repeat(crate::memory::MAX_CONTENT_LENGTH);
        println!("BEFORE: Creating memory with {} chars", large_content.len());

        let memory = Memory::new(
            large_content.clone(),
            MemorySource::HookDescription {
                hook_type: HookType::SessionStart,
                tool_name: None,
            },
            "large-session".to_string(),
            test_fingerprint(),
            None,
        );
        let memory_id = memory.id;

        store.store(&memory).expect("store large memory");
        println!("Stored memory ID: {}", memory_id);

        let retrieved = store.get(memory_id).expect("get").expect("should exist");
        println!(
            "AFTER: Retrieved content length: {}",
            retrieved.content.len()
        );

        assert_eq!(
            retrieved.content.len(),
            crate::memory::MAX_CONTENT_LENGTH,
            "Content should be preserved"
        );
        assert_eq!(retrieved.content, large_content);
        println!("RESULT: PASS - Large content handled correctly");
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn edge_case_special_chars_session_id() {
        println!("=== EDGE CASE: Special chars in session_id ===");

        let tmp = tempdir().expect("create temp dir");
        let store = MemoryStore::new(tmp.path()).expect("create store");

        let special_session = "test/session:with-special_chars.123";
        println!("BEFORE: Using session_id = '{}'", special_session);

        let memory = create_test_memory("Special session test", special_session);
        store.store(&memory).expect("store");

        let retrieved = store
            .get_by_session(special_session)
            .expect("get by session");
        println!("AFTER: Retrieved {} memories", retrieved.len());

        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].session_id, special_session);
        println!("RESULT: PASS - Special chars handled correctly");
    }

    #[test]
    fn edge_case_delete_nonexistent() {
        println!("=== EDGE CASE: Delete non-existent ===");

        let tmp = tempdir().expect("create temp dir");
        let store = MemoryStore::new(tmp.path()).expect("create store");

        println!("BEFORE: Empty store");
        let random_id = Uuid::new_v4();
        println!("Attempting to delete non-existent ID: {}", random_id);

        let result = store.delete(random_id).expect("delete should not error");
        println!("delete result: {}", result);

        assert!(!result, "Should return false for non-existent");
        assert_eq!(store.count().expect("count"), 0, "Count should still be 0");
        println!("RESULT: PASS - Delete non-existent handled correctly");
    }

    #[cfg(feature = "test-utils")]
    #[test]
    fn edge_case_concurrent_access() {
        use std::thread;

        println!("=== EDGE CASE: Basic concurrent access ===");

        let tmp = tempdir().expect("create temp dir");
        let store = Arc::new(MemoryStore::new(tmp.path()).expect("create store"));

        println!("BEFORE: Store with one memory");
        let initial_memory = create_test_memory("Initial", "concurrent");
        store.store(&initial_memory).expect("store");

        // Clone Arc for multiple threads
        let store1 = Arc::clone(&store);
        let store2 = Arc::clone(&store);

        let handle1 = thread::spawn(move || {
            for _ in 0..10 {
                let _ = store1.count();
                let _ = store1.get_by_session("concurrent");
            }
        });

        let handle2 = thread::spawn(move || {
            for _ in 0..10 {
                let mem = create_test_memory("Concurrent write", "concurrent");
                let _ = store2.store(&mem);
            }
        });

        handle1.join().expect("thread 1");
        handle2.join().expect("thread 2");

        println!("AFTER: Concurrent operations complete");
        let final_count = store.count().expect("count");
        println!("Final count: {} (expected >= 1)", final_count);

        assert!(final_count >= 1, "Should have at least initial memory");
        println!("RESULT: PASS - No panics during concurrent access");
    }

    /// Full State Verification (FSV) test per user requirements.
    /// Verifies actual database state on disk, not just return values.
    #[cfg(feature = "test-utils")]
    #[test]
    fn fsv_verify_rocksdb_disk_state() {
        use std::fs;

        println!("\n============================================================");
        println!("=== FSV: MemoryStore RocksDB Disk State Verification ===");
        println!("============================================================\n");

        let tmp = tempdir().expect("create temp dir");
        let db_path = tmp.path();

        println!("[FSV-1] Creating MemoryStore at: {:?}", db_path);

        // Store 3 memories across 2 sessions
        let memory1 = create_test_memory("FSV Memory One", "fsv-session-a");
        let memory2 = create_test_memory("FSV Memory Two", "fsv-session-a");
        let memory3 = create_test_memory("FSV Memory Three", "fsv-session-b");

        let id1 = memory1.id;
        let id2 = memory2.id;
        let id3 = memory3.id;

        {
            let store = MemoryStore::new(db_path).expect("create store");
            store.store(&memory1).expect("store 1");
            store.store(&memory2).expect("store 2");
            store.store(&memory3).expect("store 3");
            println!("[FSV-2] Stored 3 memories, IDs: {:?}", [id1, id2, id3]);
        }
        // Store dropped, DB closed

        // VERIFICATION STEP 1: Check that RocksDB files exist on disk
        println!("\n[FSV-3] Verifying RocksDB files on disk...");
        let entries: Vec<_> = fs::read_dir(db_path)
            .expect("read db dir")
            .filter_map(|e| e.ok())
            .collect();

        println!(
            "  Directory contents: {:?}",
            entries.iter().map(|e| e.file_name()).collect::<Vec<_>>()
        );
        assert!(!entries.is_empty(), "RocksDB directory should not be empty");

        // Check for typical RocksDB files
        let file_names: Vec<String> = entries
            .iter()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();

        // RocksDB creates MANIFEST, OPTIONS, and SST files
        let has_manifest = file_names.iter().any(|n| n.starts_with("MANIFEST"));
        let has_options = file_names.iter().any(|n| n.starts_with("OPTIONS"));
        println!("  Has MANIFEST file: {}", has_manifest);
        println!("  Has OPTIONS file: {}", has_options);

        assert!(has_manifest, "RocksDB MANIFEST file should exist");
        assert!(has_options, "RocksDB OPTIONS file should exist");

        // VERIFICATION STEP 2: Reopen and verify data
        println!("\n[FSV-4] Reopening database and verifying data...");
        {
            let store = MemoryStore::new(db_path).expect("reopen store");

            // Verify count
            let count = store.count().expect("count");
            println!("  Memory count: {}", count);
            assert_eq!(count, 3, "Should have 3 memories persisted");

            // Verify each memory by ID
            let r1 = store.get(id1).expect("get 1");
            let r2 = store.get(id2).expect("get 2");
            let r3 = store.get(id3).expect("get 3");

            assert!(r1.is_some(), "Memory 1 should exist");
            assert!(r2.is_some(), "Memory 2 should exist");
            assert!(r3.is_some(), "Memory 3 should exist");

            println!("  Memory 1 content: {:?}", r1.as_ref().map(|m| &m.content));
            println!("  Memory 2 content: {:?}", r2.as_ref().map(|m| &m.content));
            println!("  Memory 3 content: {:?}", r3.as_ref().map(|m| &m.content));

            // Verify session indexes
            let session_a = store
                .get_by_session("fsv-session-a")
                .expect("get session A");
            let session_b = store
                .get_by_session("fsv-session-b")
                .expect("get session B");

            println!("  Session A count: {}", session_a.len());
            println!("  Session B count: {}", session_b.len());

            assert_eq!(session_a.len(), 2, "Session A should have 2 memories");
            assert_eq!(session_b.len(), 1, "Session B should have 1 memory");

            // Verify content integrity
            assert_eq!(r1.unwrap().content, "FSV Memory One");
            assert_eq!(r2.unwrap().content, "FSV Memory Two");
            assert_eq!(r3.unwrap().content, "FSV Memory Three");
        }

        // VERIFICATION STEP 3: Test delete persists
        println!("\n[FSV-5] Testing delete persistence...");
        {
            let store = MemoryStore::new(db_path).expect("reopen for delete");
            let deleted = store.delete(id2).expect("delete");
            assert!(deleted, "Should successfully delete memory 2");
            println!("  Deleted memory 2: {}", deleted);
        }

        // Verify delete persisted after reopen
        {
            let store = MemoryStore::new(db_path).expect("reopen after delete");
            let count = store.count().expect("count");
            println!("  Count after delete: {}", count);
            assert_eq!(count, 2, "Should have 2 memories after delete");

            let r2 = store.get(id2).expect("get 2");
            assert!(r2.is_none(), "Memory 2 should be deleted");

            let session_a = store
                .get_by_session("fsv-session-a")
                .expect("get session A");
            println!("  Session A count after delete: {}", session_a.len());
            assert_eq!(
                session_a.len(),
                1,
                "Session A should have 1 memory after delete"
            );
        }

        println!("\n============================================================");
        println!("[FSV] VERIFIED: All disk state checks passed");
        println!("============================================================\n");
    }
}
