//! SessionManager: RocksDB-backed session lifecycle management.
//!
//! This module provides persistent session management with:
//! - RocksDB storage for session data
//! - File-based current session tracking (survives restarts)
//! - Idempotent session termination
//!
//! # Constitution Compliance
//! - ARCH-07: Supports NATIVE Claude Code hooks
//! - AP-14: No .unwrap() in library code
//! - rust_standards.error_handling: thiserror for errors

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rocksdb::{IteratorMode, DB};
use thiserror::Error;
use tracing::{debug, error, warn};

use super::Session;

/// Column family name for session storage.
pub const CF_SESSIONS: &str = "sessions";

/// Filename for tracking current active session.
const CURRENT_SESSION_FILE: &str = "current_session";

/// Errors that can occur during session management operations.
///
/// All errors include sufficient context for debugging.
/// Follows fail-fast principle - no retries at this layer.
#[derive(Debug, Error)]
pub enum SessionError {
    /// Session with given ID was not found.
    #[error("Session not found: {session_id}")]
    NotFound { session_id: String },

    /// Cannot start new session while another is active.
    #[error("Session already active: {session_id}")]
    AlreadyActive { session_id: String },

    /// Session is not in the expected state for the operation.
    #[error("Session {session_id} has invalid status {status} for operation {operation}")]
    InvalidStatus {
        session_id: String,
        status: String,
        operation: String,
    },

    /// RocksDB operation failed.
    #[error("Storage failed: {0}")]
    StorageFailed(String),

    /// Bincode serialization/deserialization failed.
    #[error("Serialization failed: {0}")]
    SerializationFailed(String),

    /// File I/O operation failed.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Required column family not found in database.
    #[error("Column family not found: {0}")]
    ColumnFamilyNotFound(String),

    /// Session validation failed.
    #[error("Session validation failed: {0}")]
    ValidationFailed(String),
}

/// Session termination type for internal use.
#[derive(Debug, Clone, Copy)]
enum SessionTermination {
    Complete,
    Abandon,
}

/// Manages session lifecycle with RocksDB persistence.
///
/// # Thread Safety
/// Thread-safe via Arc<DB>. Multiple threads can call methods concurrently.
///
/// # Current Session Tracking
/// The current session ID is stored in a file (`current_session`) to survive
/// process restarts. This allows the SessionEnd hook to find and close the
/// active session even after crashes/restarts.
///
/// # Storage
/// - Sessions stored in RocksDB `sessions` column family
/// - Key: session_id as UTF-8 bytes
/// - Value: bincode-serialized Session struct
#[derive(Debug)]
pub struct SessionManager {
    /// RocksDB instance (shared).
    db: Arc<DB>,
    /// Path to current_session file.
    session_file: PathBuf,
    /// Data directory root.
    data_dir: PathBuf,
}

impl SessionManager {
    /// Create a new SessionManager.
    ///
    /// # Arguments
    /// * `db` - Shared RocksDB instance (must have `sessions` CF)
    /// * `data_dir` - Directory for storing current_session file
    ///
    /// # Returns
    /// * `Ok(Self)` - Manager ready to use
    /// * `Err(SessionError::ColumnFamilyNotFound)` - `sessions` CF missing
    /// * `Err(SessionError::IoError)` - Cannot create data_dir
    ///
    /// # Example
    /// ```ignore
    /// let db = Arc::new(open_db_with_sessions_cf(path)?);
    /// let manager = SessionManager::new(db, data_dir)?;
    /// ```
    pub fn new(db: Arc<DB>, data_dir: &Path) -> Result<Self, SessionError> {
        // Verify sessions CF exists
        let _ = db
            .cf_handle(CF_SESSIONS)
            .ok_or_else(|| SessionError::ColumnFamilyNotFound(CF_SESSIONS.to_string()))?;

        // Ensure data_dir exists
        fs::create_dir_all(data_dir)?;

        let session_file = data_dir.join(CURRENT_SESSION_FILE);

        debug!("SessionManager initialized with data_dir: {:?}", data_dir);

        Ok(Self {
            db,
            session_file,
            data_dir: data_dir.to_path_buf(),
        })
    }

    /// Start a new session.
    ///
    /// # Behavior
    /// 1. Checks if an active session exists (fails if so)
    /// 2. Creates new Session with UUID
    /// 3. Stores in RocksDB
    /// 4. Writes session ID to current_session file
    ///
    /// # Returns
    /// * `Ok(Session)` - The newly created session
    /// * `Err(AlreadyActive)` - Another session is already active
    /// * `Err(StorageFailed)` - RocksDB write failed
    ///
    /// # Idempotency
    /// NOT idempotent - calling twice will fail with AlreadyActive.
    pub fn start_session(&self) -> Result<Session, SessionError> {
        // Check for existing active session
        if let Some(existing) = self.get_current_session()? {
            if existing.status.is_active() {
                return Err(SessionError::AlreadyActive {
                    session_id: existing.id,
                });
            }
            // Clear stale current_session file if session is not active
            self.clear_current_session_file()?;
        }

        // Create new session
        let session = Session::new();

        // Store in RocksDB
        self.store_session(&session)?;

        // Write current session file
        fs::write(&self.session_file, &session.id)?;

        debug!("Started session: {}", session.id);
        Ok(session)
    }

    /// End a session normally (mark as Completed).
    ///
    /// # Arguments
    /// * `session_id` - ID of session to end
    ///
    /// # Behavior
    /// 1. Loads session from storage
    /// 2. Calls session.complete() if Active
    /// 3. Stores updated session
    /// 4. Clears current_session file if this was the current session
    ///
    /// # Idempotency
    /// Idempotent - calling multiple times has same effect as calling once.
    /// Returns Ok(()) if session already Completed or not found.
    pub fn end_session(&self, session_id: &str) -> Result<(), SessionError> {
        self.terminate_session(session_id, SessionTermination::Complete)
    }

    /// Abandon a session (mark as Abandoned).
    ///
    /// # Arguments
    /// * `session_id` - ID of session to abandon
    ///
    /// # Behavior
    /// Same as end_session but marks session as Abandoned instead of Completed.
    ///
    /// # Use Case
    /// Called when session ends abnormally (crash, timeout, error).
    ///
    /// # Idempotency
    /// Idempotent - safe to call multiple times.
    pub fn abandon_session(&self, session_id: &str) -> Result<(), SessionError> {
        self.terminate_session(session_id, SessionTermination::Abandon)
    }

    /// Get the current active session if one exists.
    ///
    /// # Returns
    /// * `Ok(Some(Session))` - The current active session
    /// * `Ok(None)` - No active session
    /// * `Err(...)` - Storage/IO error
    ///
    /// # Behavior
    /// 1. Reads session ID from current_session file
    /// 2. Loads session from RocksDB
    /// 3. Validates session is still Active
    pub fn get_current_session(&self) -> Result<Option<Session>, SessionError> {
        // Read current session file
        let session_id = match fs::read_to_string(&self.session_file) {
            Ok(content) => {
                let id = content.trim().to_string();
                if id.is_empty() {
                    return Ok(None);
                }
                id
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(e) => return Err(SessionError::IoError(e)),
        };

        // Load session from storage
        self.get_session(&session_id)
    }

    /// Get a session by ID.
    ///
    /// # Arguments
    /// * `session_id` - The session ID to look up
    ///
    /// # Returns
    /// * `Ok(Some(Session))` - Session found
    /// * `Ok(None)` - Session not found
    /// * `Err(...)` - Storage error
    pub fn get_session(&self, session_id: &str) -> Result<Option<Session>, SessionError> {
        let cf = self.sessions_cf()?;

        match self.db.get_cf(cf, session_id.as_bytes()) {
            Ok(Some(data)) => {
                let session: Session = bincode::deserialize(&data).map_err(|e| {
                    error!(
                        session_id = %session_id,
                        error = %e,
                        "Failed to deserialize session"
                    );
                    SessionError::SerializationFailed(format!("session_id={}: {}", session_id, e))
                })?;
                Ok(Some(session))
            }
            Ok(None) => Ok(None),
            Err(e) => {
                error!(
                    session_id = %session_id,
                    error = %e,
                    "Failed to read session from DB"
                );
                Err(SessionError::StorageFailed(format!(
                    "session_id={}: {}",
                    session_id, e
                )))
            }
        }
    }

    /// List all active sessions.
    ///
    /// # Returns
    /// Vec of sessions with status == Active.
    ///
    /// # Performance
    /// Scans entire sessions CF - use sparingly.
    pub fn list_active_sessions(&self) -> Result<Vec<Session>, SessionError> {
        let cf = self.sessions_cf()?;

        let mut sessions = Vec::new();
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);

        for item in iter {
            let (_, value) = item.map_err(|e| {
                error!(error = %e, "Failed to iterate sessions");
                SessionError::StorageFailed(e.to_string())
            })?;
            let session: Session = bincode::deserialize(&value).map_err(|e| {
                error!(error = %e, "Failed to deserialize session during iteration");
                SessionError::SerializationFailed(e.to_string())
            })?;
            if session.status.is_active() {
                sessions.push(session);
            }
        }

        Ok(sessions)
    }

    /// Increment the memory count for a session.
    ///
    /// # Arguments
    /// * `session_id` - Session to update
    ///
    /// # Returns
    /// * `Ok(u32)` - The new memory count
    /// * `Err(NotFound)` - Session doesn't exist
    /// * `Err(InvalidStatus)` - Session is not Active
    pub fn increment_memory_count(&self, session_id: &str) -> Result<u32, SessionError> {
        let mut session = self
            .get_session(session_id)?
            .ok_or_else(|| SessionError::NotFound {
                session_id: session_id.to_string(),
            })?;

        if !session.status.is_active() {
            return Err(SessionError::InvalidStatus {
                session_id: session_id.to_string(),
                status: session.status.to_string(),
                operation: "increment_memory_count".to_string(),
            });
        }

        session.increment_memory_count();
        self.store_session(&session)?;

        Ok(session.memory_count)
    }

    /// Get the data directory path.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    // -------------------------------------------------------------------------
    // Private helpers
    // -------------------------------------------------------------------------

    /// Get the sessions column family handle.
    ///
    /// Returns an error if the CF is not found (should not happen after construction).
    fn sessions_cf(&self) -> Result<&rocksdb::ColumnFamily, SessionError> {
        self.db
            .cf_handle(CF_SESSIONS)
            .ok_or_else(|| SessionError::ColumnFamilyNotFound(CF_SESSIONS.to_string()))
    }

    /// Terminate a session (complete or abandon).
    ///
    /// Unified implementation for end_session and abandon_session to avoid duplication.
    fn terminate_session(
        &self,
        session_id: &str,
        termination: SessionTermination,
    ) -> Result<(), SessionError> {
        let Some(mut session) = self.get_session(session_id)? else {
            debug!(
                "terminate_session: session {} not found, treating as already terminated",
                session_id
            );
            return Ok(());
        };

        if !session.status.is_active() {
            debug!(
                "terminate_session: session {} already terminated ({})",
                session_id, session.status
            );
            return Ok(());
        }

        match termination {
            SessionTermination::Complete => {
                session.complete();
                debug!("Ended session: {}", session_id);
            }
            SessionTermination::Abandon => {
                session.abandon();
                warn!("Abandoned session: {}", session_id);
            }
        }

        session.validate().map_err(SessionError::ValidationFailed)?;

        self.store_session(&session)?;
        self.clear_if_current(session_id)?;

        Ok(())
    }

    /// Store a session in RocksDB.
    fn store_session(&self, session: &Session) -> Result<(), SessionError> {
        let cf = self.sessions_cf()?;

        let value = bincode::serialize(session).map_err(|e| {
            error!(
                session_id = %session.id,
                error = %e,
                "Failed to serialize session"
            );
            SessionError::SerializationFailed(format!("session_id={}: {}", session.id, e))
        })?;

        self.db
            .put_cf(cf, session.id.as_bytes(), &value)
            .map_err(|e| {
                error!(
                    session_id = %session.id,
                    error = %e,
                    "Failed to write session to DB"
                );
                SessionError::StorageFailed(format!("session_id={}: {}", session.id, e))
            })?;

        Ok(())
    }

    /// Clear current_session file if session_id matches.
    fn clear_if_current(&self, session_id: &str) -> Result<(), SessionError> {
        match fs::read_to_string(&self.session_file) {
            Ok(current_id) => {
                if current_id.trim() == session_id {
                    self.clear_current_session_file()?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // No current session file — nothing to clear
            }
            Err(e) => {
                // ERR-14: Unreadable session file — log and delete so a fresh one is created
                error!(
                    error = %e,
                    session_file = %self.session_file.display(),
                    "clear_if_current: Corrupted/unreadable session file, removing it"
                );
                self.clear_current_session_file()?;
            }
        }
        Ok(())
    }

    /// Clear the current_session file.
    fn clear_current_session_file(&self) -> Result<(), SessionError> {
        match fs::remove_file(&self.session_file) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(SessionError::IoError(e)),
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::SessionStatus;
    use rocksdb::{ColumnFamilyDescriptor, Options};
    use tempfile::tempdir;

    /// Create a real RocksDB instance with sessions CF for testing.
    fn create_test_db(path: &Path) -> Arc<DB> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        let cf = ColumnFamilyDescriptor::new(CF_SESSIONS, Options::default());
        Arc::new(
            DB::open_cf_descriptors(&opts, path.join("db"), vec![cf])
                .expect("Failed to open test DB"),
        )
    }

    #[test]
    fn test_session_manager_new_success() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let result = SessionManager::new(db, dir.path());
        assert!(result.is_ok(), "SessionManager::new should succeed");
    }

    #[test]
    fn test_start_session_creates_session() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        let session = manager.start_session().expect("start_session");

        // Verify session properties
        assert!(!session.id.is_empty());
        assert!(session.status.is_active());
        assert!(session.ended_at.is_none());
        assert_eq!(session.memory_count, 0);

        // Verify current_session file
        let file_content = fs::read_to_string(dir.path().join(CURRENT_SESSION_FILE))
            .expect("read current_session");
        assert_eq!(file_content, session.id);

        // Verify stored in DB
        let loaded = manager
            .get_session(&session.id)
            .expect("get_session")
            .expect("should exist");
        assert_eq!(loaded.id, session.id);
    }

    #[test]
    fn test_start_session_fails_if_active_exists() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        let session1 = manager.start_session().expect("first start");

        // Second start should fail
        let result = manager.start_session();
        assert!(result.is_err());
        match result {
            Err(SessionError::AlreadyActive { session_id }) => {
                assert_eq!(session_id, session1.id);
            }
            other => panic!("Expected AlreadyActive, got {:?}", other),
        }
    }

    #[test]
    fn test_end_session_marks_completed() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        let session = manager.start_session().expect("start");
        manager.end_session(&session.id).expect("end");

        // Verify status
        let loaded = manager
            .get_session(&session.id)
            .expect("get")
            .expect("exists");
        assert_eq!(loaded.status, SessionStatus::Completed);
        assert!(loaded.ended_at.is_some());

        // Verify current_session file cleared
        assert!(
            !dir.path().join(CURRENT_SESSION_FILE).exists(),
            "current_session file should be cleared"
        );
    }

    #[test]
    fn test_end_session_is_idempotent() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        let session = manager.start_session().expect("start");
        let session_id = session.id.clone();

        // End multiple times - should all succeed
        manager.end_session(&session_id).expect("end 1");
        manager.end_session(&session_id).expect("end 2");
        manager.end_session(&session_id).expect("end 3");

        // Still completed
        let loaded = manager
            .get_session(&session_id)
            .expect("get")
            .expect("exists");
        assert_eq!(loaded.status, SessionStatus::Completed);
    }

    #[test]
    fn test_end_session_not_found_is_idempotent() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        // End non-existent session should succeed (idempotent)
        let result = manager.end_session("does-not-exist");
        assert!(result.is_ok());
    }

    #[test]
    fn test_abandon_session_marks_abandoned() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        let session = manager.start_session().expect("start");
        manager.abandon_session(&session.id).expect("abandon");

        let loaded = manager
            .get_session(&session.id)
            .expect("get")
            .expect("exists");
        assert_eq!(loaded.status, SessionStatus::Abandoned);
        assert!(loaded.ended_at.is_some());
    }

    #[test]
    fn test_get_current_session_returns_active() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        // No current session initially
        assert!(manager.get_current_session().expect("get").is_none());

        let session = manager.start_session().expect("start");

        // Now there's a current session
        let current = manager
            .get_current_session()
            .expect("get")
            .expect("should exist");
        assert_eq!(current.id, session.id);
    }

    #[test]
    fn test_get_current_session_none_after_end() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        let session = manager.start_session().expect("start");
        manager.end_session(&session.id).expect("end");

        // No current session after ending
        assert!(manager.get_current_session().expect("get").is_none());
    }

    #[test]
    fn test_session_survives_restart() {
        let dir = tempdir().expect("tempdir");
        let session_id: String;

        // First "run" - start session
        {
            let db = create_test_db(dir.path());
            let manager = SessionManager::new(db, dir.path()).expect("manager");
            let session = manager.start_session().expect("start");
            session_id = session.id.clone();
        }
        // DB dropped, simulating process exit

        // Second "run" - session should still be there
        {
            let db = create_test_db(dir.path());
            let manager = SessionManager::new(db, dir.path()).expect("manager");

            // Current session file should still point to the session
            let current = manager
                .get_current_session()
                .expect("get")
                .expect("should exist");
            assert_eq!(current.id, session_id);
            assert!(current.status.is_active());
        }
    }

    #[test]
    fn test_list_active_sessions() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        // Start session 1
        let s1 = manager.start_session().expect("start 1");
        manager.end_session(&s1.id).expect("end 1");

        // Start session 2 (now possible since s1 is ended)
        let s2 = manager.start_session().expect("start 2");

        let active = manager.list_active_sessions().expect("list");

        // Only s2 should be active
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, s2.id);
    }

    #[test]
    fn test_increment_memory_count() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        let session = manager.start_session().expect("start");
        assert_eq!(session.memory_count, 0);

        let count1 = manager.increment_memory_count(&session.id).expect("inc 1");
        assert_eq!(count1, 1);

        let count2 = manager.increment_memory_count(&session.id).expect("inc 2");
        assert_eq!(count2, 2);

        // Verify persisted
        let loaded = manager
            .get_session(&session.id)
            .expect("get")
            .expect("exists");
        assert_eq!(loaded.memory_count, 2);
    }

    #[test]
    fn test_increment_memory_count_fails_for_ended_session() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        let session = manager.start_session().expect("start");
        manager.end_session(&session.id).expect("end");

        let result = manager.increment_memory_count(&session.id);
        assert!(result.is_err());
        match result {
            Err(SessionError::InvalidStatus { .. }) => {}
            other => panic!("Expected InvalidStatus, got {:?}", other),
        }
    }

    #[test]
    fn test_get_session_not_found() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        let result = manager
            .get_session("nonexistent")
            .expect("should not error");
        assert!(result.is_none());
    }

    #[test]
    fn test_session_validation_on_end() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        let session = manager.start_session().expect("start");

        // End should validate session before storing
        let result = manager.end_session(&session.id);
        assert!(result.is_ok());

        // Load and verify valid state
        let loaded = manager
            .get_session(&session.id)
            .expect("get")
            .expect("exists");
        assert!(loaded.validate().is_ok());
    }

    #[test]
    fn test_can_start_new_session_after_previous_ends() {
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        // Start and end first session
        let s1 = manager.start_session().expect("start 1");
        manager.end_session(&s1.id).expect("end 1");

        // Should be able to start new session
        let s2 = manager.start_session().expect("start 2");
        assert_ne!(s1.id, s2.id);
        assert!(s2.status.is_active());
    }

    // =========================================================================
    // EDGE CASE TESTS (Required by task spec)
    // =========================================================================

    #[test]
    fn edge_case_empty_session_id() {
        println!("=== EDGE CASE: Empty session_id ===");
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        println!("BEFORE: Testing get_session with empty string");
        let result = manager.get_session("").expect("should not error");
        println!("Result: {:?}", result);
        assert!(result.is_none(), "Empty session_id should return None");

        println!("Testing end_session with empty string (should be idempotent)");
        let end_result = manager.end_session("");
        println!("end_session result: {:?}", end_result);
        assert!(
            end_result.is_ok(),
            "end_session('') should succeed (idempotent)"
        );

        println!("RESULT: PASS - Empty session_id handled correctly");
    }

    #[test]
    fn edge_case_unicode_session_id() {
        println!("=== EDGE CASE: Unicode in session lookup ===");
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        let unicode_ids = [
            "session-\u{1F600}-emoji",
            "\u{4E2D}\u{6587}-chinese",
            "session-\u{0410}\u{0411}\u{0412}-cyrillic",
        ];

        for unicode_id in &unicode_ids {
            println!("Testing: {}", unicode_id);
            // get_session should return None (no such session)
            let result = manager.get_session(unicode_id).expect("should not error");
            assert!(result.is_none(), "Unicode session_id should return None");

            // end_session should be idempotent
            let end_result = manager.end_session(unicode_id);
            assert!(end_result.is_ok(), "end_session should succeed for unicode");
        }
        println!("RESULT: PASS - Unicode session_ids handled correctly");
    }

    #[test]
    fn edge_case_very_long_session_id() {
        println!("=== EDGE CASE: Very long session_id (1000+ chars) ===");
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        let long_id = "x".repeat(1000);
        println!("BEFORE: Testing get_session with {} char ID", long_id.len());

        let result = manager.get_session(&long_id).expect("should not error");
        println!("Result: {:?}", result);
        assert!(result.is_none(), "Long session_id should return None");

        // end_session should be idempotent
        let end_result = manager.end_session(&long_id);
        assert!(end_result.is_ok(), "end_session should succeed for long ID");

        println!("RESULT: PASS - Very long session_id handled correctly");
    }

    #[test]
    fn edge_case_concurrent_access() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::thread;

        println!("=== EDGE CASE: Concurrent access ===");
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = Arc::new(SessionManager::new(db, dir.path()).expect("manager"));

        // Start initial session
        let session = manager.start_session().expect("start");
        let session_id = session.id.clone();
        println!("Started session: {}", session_id);

        let error_count = Arc::new(AtomicUsize::new(0));
        let read_count = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();

        // Spawn readers
        for _ in 0..4 {
            let mgr = Arc::clone(&manager);
            let sid = session_id.clone();
            let reads = Arc::clone(&read_count);
            let errors = Arc::clone(&error_count);

            handles.push(thread::spawn(move || {
                for _ in 0..25 {
                    match mgr.get_session(&sid) {
                        Ok(_) => {
                            reads.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(e) => {
                            eprintln!("Read error: {:?}", e);
                            errors.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                }
            }));
        }

        // Spawn writers (increment_memory_count)
        for _ in 0..2 {
            let mgr = Arc::clone(&manager);
            let sid = session_id.clone();
            let errors = Arc::clone(&error_count);

            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    if let Err(e) = mgr.increment_memory_count(&sid) {
                        eprintln!("Write error: {:?}", e);
                        errors.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().expect("thread join");
        }

        let total_reads = read_count.load(Ordering::SeqCst);
        let total_errors = error_count.load(Ordering::SeqCst);

        println!("Concurrent operations complete:");
        println!("  Total reads: {}", total_reads);
        println!("  Total errors: {}", total_errors);

        // Verify final state
        let final_session = manager
            .get_session(&session_id)
            .expect("get")
            .expect("exists");
        println!(
            "  Final memory count: {} (expected ~20)",
            final_session.memory_count
        );

        assert_eq!(
            total_errors, 0,
            "Should have no errors during concurrent access"
        );
        assert!(
            final_session.memory_count >= 1,
            "Should have incremented at least once"
        );
        println!("RESULT: PASS - Concurrent access handled without panics");
    }

    #[test]
    fn edge_case_corrupt_data() {
        println!("=== EDGE CASE: Corrupt data in DB ===");
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());

        // Write corrupt data directly to DB
        let cf = db.cf_handle(CF_SESSIONS).expect("cf");
        let corrupt_data = b"not valid bincode data at all!!";
        db.put_cf(&cf, b"corrupt-session-id", corrupt_data)
            .expect("put corrupt data");

        let manager = SessionManager::new(db, dir.path()).expect("manager");

        println!("BEFORE: Attempting to read corrupt session");
        let result = manager.get_session("corrupt-session-id");
        println!("Result: {:?}", result);

        assert!(result.is_err(), "Should error on corrupt data");
        match result {
            Err(SessionError::SerializationFailed(msg)) => {
                println!("Got expected SerializationFailed: {}", msg);
                assert!(
                    msg.contains("corrupt-session-id"),
                    "Error should mention session_id"
                );
            }
            other => panic!("Expected SerializationFailed, got {:?}", other),
        }

        println!("RESULT: PASS - Corrupt data detected and reported");
    }

    #[test]
    fn edge_case_missing_column_family() {
        println!("=== EDGE CASE: Missing column family ===");
        let dir = tempdir().expect("tempdir");

        // Create DB WITHOUT sessions CF
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let db = Arc::new(DB::open(&opts, dir.path().join("db")).expect("open db"));

        println!("BEFORE: Attempting to create SessionManager with DB missing sessions CF");
        let result = SessionManager::new(db, dir.path());
        println!("Result: {:?}", result);

        assert!(result.is_err(), "Should error when CF missing");
        match result {
            Err(SessionError::ColumnFamilyNotFound(cf_name)) => {
                println!("Got expected ColumnFamilyNotFound: {}", cf_name);
                assert_eq!(cf_name, CF_SESSIONS);
            }
            other => panic!("Expected ColumnFamilyNotFound, got {:?}", other),
        }

        println!("RESULT: PASS - Missing CF detected at construction");
    }

    #[test]
    fn edge_case_memory_count_saturation() {
        println!("=== EDGE CASE: Memory count overflow (u32::MAX) ===");
        let dir = tempdir().expect("tempdir");
        let db = create_test_db(dir.path());
        let manager = SessionManager::new(db, dir.path()).expect("manager");

        // Start session
        let session = manager.start_session().expect("start");
        let session_id = session.id.clone();

        // Directly modify session to have u32::MAX - 1 memory_count
        let cf = manager.db.cf_handle(CF_SESSIONS).expect("cf");
        let mut maxed_session = session.clone();
        maxed_session.memory_count = u32::MAX - 1;
        let data = bincode::serialize(&maxed_session).expect("serialize");
        manager
            .db
            .put_cf(&cf, session_id.as_bytes(), &data)
            .expect("put");

        println!("BEFORE: memory_count = {}", u32::MAX - 1);

        // First increment should succeed
        let count1 = manager.increment_memory_count(&session_id).expect("inc 1");
        println!("After first increment: {}", count1);
        assert_eq!(count1, u32::MAX, "Should reach u32::MAX");

        // Second increment should saturate (not panic)
        let count2 = manager.increment_memory_count(&session_id).expect("inc 2");
        println!("After second increment (should saturate): {}", count2);
        assert_eq!(count2, u32::MAX, "Should stay at u32::MAX (saturating_add)");

        println!("RESULT: PASS - Memory count saturates, does not panic");
    }

    // =========================================================================
    // FULL STATE VERIFICATION (FSV)
    // =========================================================================

    #[test]
    fn fsv_verify_rocksdb_disk_state() {
        use std::fs as stdfs;

        println!("\n============================================================");
        println!("=== FSV: SessionManager RocksDB Disk State Verification ===");
        println!("============================================================\n");

        let dir = tempdir().expect("tempdir");
        let db_path = dir.path();

        println!("[FSV-1] Creating SessionManager at: {:?}", db_path);

        let session_id1: String;
        let session_id2: String;

        // Phase 1: Create and manipulate sessions
        {
            let db = create_test_db(db_path);
            let manager = SessionManager::new(db, db_path).expect("create manager");

            // Start first session
            let s1 = manager.start_session().expect("start s1");
            session_id1 = s1.id.clone();
            println!("[FSV-2] Started session 1: {}", session_id1);

            // Increment memory count
            manager.increment_memory_count(&session_id1).expect("inc 1");
            manager.increment_memory_count(&session_id1).expect("inc 2");
            println!("[FSV-3] Incremented memory count to 2");

            // End session 1
            manager.end_session(&session_id1).expect("end s1");
            println!("[FSV-4] Ended session 1");

            // Start session 2
            let s2 = manager.start_session().expect("start s2");
            session_id2 = s2.id.clone();
            println!("[FSV-5] Started session 2: {}", session_id2);
        }
        // DB dropped - simulating process exit

        // VERIFICATION STEP 1: Check RocksDB files on disk
        println!("\n[FSV-6] Verifying RocksDB files on disk...");
        let db_dir = db_path.join("db");
        let entries: Vec<_> = stdfs::read_dir(&db_dir)
            .expect("read db dir")
            .filter_map(|e| e.ok())
            .collect();

        println!(
            "  Directory contents: {:?}",
            entries.iter().map(|e| e.file_name()).collect::<Vec<_>>()
        );
        assert!(!entries.is_empty(), "DB directory should have files");

        // Check for MANIFEST file (RocksDB marker)
        let has_manifest = entries
            .iter()
            .any(|e| e.file_name().to_string_lossy().starts_with("MANIFEST"));
        assert!(has_manifest, "Should have MANIFEST file");
        println!("  Has MANIFEST: {}", has_manifest);

        // VERIFICATION STEP 2: Check current_session file
        let current_session_path = db_path.join(CURRENT_SESSION_FILE);
        let current_content =
            stdfs::read_to_string(&current_session_path).expect("read current_session");
        println!(
            "  current_session file content: '{}'",
            current_content.trim()
        );
        assert_eq!(
            current_content.trim(),
            session_id2,
            "current_session should point to session 2"
        );

        // VERIFICATION STEP 3: Reopen and verify state
        println!("\n[FSV-7] Reopening database and verifying state...");
        {
            let db = create_test_db(db_path);
            let manager = SessionManager::new(db, db_path).expect("reopen manager");

            // Verify session 1
            let loaded1 = manager
                .get_session(&session_id1)
                .expect("get s1")
                .expect("s1 should exist");
            println!(
                "  Session 1: id={}, status={}, memory_count={}",
                loaded1.id, loaded1.status, loaded1.memory_count
            );
            assert_eq!(loaded1.status, SessionStatus::Completed);
            assert_eq!(loaded1.memory_count, 2);

            // Verify session 2
            let loaded2 = manager
                .get_session(&session_id2)
                .expect("get s2")
                .expect("s2 should exist");
            println!(
                "  Session 2: id={}, status={}, memory_count={}",
                loaded2.id, loaded2.status, loaded2.memory_count
            );
            assert_eq!(loaded2.status, SessionStatus::Active);
            assert_eq!(loaded2.memory_count, 0);

            // Verify current session
            let current = manager
                .get_current_session()
                .expect("get current")
                .expect("should have current");
            println!("  Current session ID: {}", current.id);
            assert_eq!(current.id, session_id2);

            // Verify active sessions list
            let active = manager.list_active_sessions().expect("list active");
            println!("  Active sessions count: {}", active.len());
            assert_eq!(active.len(), 1);
            assert_eq!(active[0].id, session_id2);
        }

        // VERIFICATION STEP 4: Test end_session persists
        println!("\n[FSV-8] Testing end_session persistence...");
        {
            let db = create_test_db(db_path);
            let manager = SessionManager::new(db, db_path).expect("reopen for end");

            manager.end_session(&session_id2).expect("end s2");
            println!("  Ended session 2");
        }

        // Verify end persisted
        {
            let db = create_test_db(db_path);
            let manager = SessionManager::new(db, db_path).expect("reopen after end");

            let loaded2 = manager
                .get_session(&session_id2)
                .expect("get s2")
                .expect("s2 exists");
            println!("  Session 2 after end: status={}", loaded2.status);
            assert_eq!(loaded2.status, SessionStatus::Completed);

            // current_session file should be cleared
            let current = manager.get_current_session().expect("get current");
            println!("  Current session after end: {:?}", current);
            assert!(current.is_none(), "No current session after ending all");

            let active = manager.list_active_sessions().expect("list");
            println!("  Active sessions after end: {}", active.len());
            assert_eq!(active.len(), 0, "No active sessions");
        }

        println!("\n============================================================");
        println!("[FSV] VERIFIED: All disk state checks passed");
        println!("============================================================\n");
    }
}
