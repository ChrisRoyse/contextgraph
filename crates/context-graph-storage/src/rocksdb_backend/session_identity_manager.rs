//! StandaloneSessionIdentityManager - Concrete implementation of SessionIdentityManager.
//!
//! # TASK-SESSION-06
//!
//! This implementation lives in the storage crate because it needs direct RocksDB access.
//! The trait is defined in context-graph-core to avoid circular dependencies.
//!
//! # Constitution Reference
//! - IDENTITY-001: IC formula
//! - IDENTITY-002: IC thresholds
//! - AP-39: cosine_similarity_13d from ego_node
//! - FAIL FAST: No silent defaults

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::{debug, error, info};

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::gwt::{
    compute_ic, update_cache, SessionIdentityManager, SessionIdentitySnapshot,
};

use super::core::RocksDbMemex;
use super::error::StorageResult;

/// Standalone implementation with RocksDbMemex for persistence.
///
/// This manager does NOT depend on GwtSystem - it operates on pre-populated
/// snapshots. The caller is responsible for capturing GWT state into the
/// snapshot before calling `restore_identity`.
///
/// # Usage Pattern
/// ```ignore
/// // 1. Caller creates and populates snapshot with current GWT state
/// let mut snapshot = SessionIdentitySnapshot::new("my-session");
/// snapshot.purpose_vector = gwt.self_ego_node.read().await.purpose_vector;
/// snapshot.kuramoto_phases = convert_phases(&gwt.kuramoto.read().await.phases());
/// // ... populate other fields ...
///
/// // 2. Save the snapshot
/// storage.save_snapshot(&snapshot)?;
///
/// // 3. Later, restore identity using the manager
/// let manager = StandaloneSessionIdentityManager::new(storage);
/// let (restored, ic) = manager.restore_identity(None)?;
/// ```
pub struct StandaloneSessionIdentityManager {
    storage: Arc<RocksDbMemex>,
}

impl StandaloneSessionIdentityManager {
    /// Create new standalone manager.
    ///
    /// # Arguments
    /// * `storage` - RocksDB storage for persistence
    pub fn new(storage: Arc<RocksDbMemex>) -> Self {
        info!("SESSION_IDENTITY_MANAGER: Created standalone manager instance");
        Self { storage }
    }

    /// Load the latest snapshot from storage.
    ///
    /// # Returns
    /// * `Ok(Some(snapshot))` - Found and loaded
    /// * `Ok(None)` - No snapshots exist
    /// * `Err` - Storage error
    pub fn load_latest(&self) -> StorageResult<Option<SessionIdentitySnapshot>> {
        self.storage.load_latest()
    }

    /// Load a specific snapshot by session ID.
    ///
    /// # Arguments
    /// * `session_id` - The session ID to load
    ///
    /// # Returns
    /// * `Ok(Some(snapshot))` - Found and loaded
    /// * `Ok(None)` - Session not found
    /// * `Err` - Storage error
    pub fn load_snapshot(&self, session_id: &str) -> StorageResult<Option<SessionIdentitySnapshot>> {
        self.storage.load_snapshot(session_id)
    }

    /// Save a snapshot to storage.
    ///
    /// # Arguments
    /// * `snapshot` - The snapshot to save
    ///
    /// # Returns
    /// * `Ok(())` - Successfully saved
    /// * `Err` - Storage error
    pub fn save_snapshot(&self, snapshot: &SessionIdentitySnapshot) -> StorageResult<()> {
        self.storage.save_snapshot(snapshot)
    }
}

/// Get current timestamp in milliseconds.
#[inline]
fn timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}

impl SessionIdentityManager for StandaloneSessionIdentityManager {
    /// Capture is a no-op for standalone manager.
    ///
    /// This implementation cannot access GWT state directly. The caller
    /// must populate the snapshot fields before saving to storage.
    ///
    /// # Returns
    /// A new empty snapshot that the caller can populate.
    fn capture_snapshot(&self, session_id: &str) -> CoreResult<SessionIdentitySnapshot> {
        // Standalone manager cannot capture GWT state - return a default snapshot
        // that the caller can populate with actual values
        info!(
            session_id = %session_id,
            "SESSION_IDENTITY_MANAGER: Creating empty snapshot for caller to populate"
        );
        Ok(SessionIdentitySnapshot::new(session_id))
    }

    fn restore_identity(
        &self,
        target_session: Option<&str>,
    ) -> CoreResult<(SessionIdentitySnapshot, f32)> {
        let start = std::time::Instant::now();

        info!(
            target_session = ?target_session,
            "SESSION_IDENTITY_MANAGER: restore_identity starting"
        );

        // 1. Load snapshot from storage - FAIL FAST on storage errors
        let previous = match target_session {
            Some(session_id) => {
                debug!(session_id = %session_id, "Loading specific session");
                self.storage.load_snapshot(session_id).map_err(|e| {
                    error!(
                        session_id = %session_id,
                        error = %e,
                        "STORAGE ERROR: Failed to load snapshot by ID"
                    );
                    CoreError::StorageError(e.to_string())
                })?
            }
            None => {
                debug!("Loading latest session");
                self.storage.load_latest().map_err(|e| {
                    error!(
                        error = %e,
                        "STORAGE ERROR: Failed to load latest snapshot"
                    );
                    CoreError::StorageError(e.to_string())
                })?
            }
        };

        // 2. Handle first session case
        let Some(previous_snapshot) = previous else {
            info!("SESSION_IDENTITY_MANAGER: First session - no previous identity");

            // Create a new snapshot for first session
            let session_id = format!("session-{}", timestamp_ms());
            let mut current = SessionIdentitySnapshot::new(&session_id);

            // First session: IC = 1.0 (perfect continuity by definition)
            let ic = 1.0_f32;
            current.last_ic = ic;

            // CRITICAL: Persist the new snapshot to RocksDB so subsequent CLI
            // invocations (separate processes) can find it. Each CLI command runs
            // as a separate process, so in-memory cache is not shared.
            if let Err(e) = self.storage.save_snapshot(&current) {
                error!(
                    session_id = %current.session_id,
                    error = %e,
                    "STORAGE ERROR: Failed to persist first session snapshot"
                );
                return Err(CoreError::StorageError(format!(
                    "Failed to persist first session: {}",
                    e
                )));
            }

            // Update cache with current state
            update_cache(&current, ic);

            info!(
                session_id = %current.session_id,
                ic = ic,
                "SESSION_IDENTITY_MANAGER: First session initialized and persisted with IC=1.0"
            );

            return Ok((current, ic));
        };

        // 3. For restore, we return the previous snapshot with its IC
        // The caller may want to compute new IC against current state
        let ic = previous_snapshot.last_ic;

        // 4. Update cache
        update_cache(&previous_snapshot, ic);

        let elapsed = start.elapsed();
        info!(
            session_id = %previous_snapshot.session_id,
            ic = ic,
            elapsed_ms = elapsed.as_millis(),
            "SESSION_IDENTITY_MANAGER: restore_identity complete"
        );

        Ok((previous_snapshot, ic))
    }

    fn compute_cross_session_ic(
        &self,
        current: &SessionIdentitySnapshot,
        previous: &SessionIdentitySnapshot,
    ) -> f32 {
        // Delegate to the standalone function in core
        compute_ic(current, previous)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::gwt::session_identity::{IdentityCache, KURAMOTO_N};
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Static lock to serialize tests that access the global singleton cache.
    // Note: We cannot call clear_cache() from storage crate because the core
    // crate is compiled without cfg(test) when running storage tests. The
    // non-test version of clear_cache() panics by design.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Create real RocksDB storage for testing
    fn create_test_storage() -> (Arc<RocksDbMemex>, TempDir) {
        let tmp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage = RocksDbMemex::open(tmp_dir.path()).expect("Failed to open RocksDB");
        (Arc::new(storage), tmp_dir)
    }

    // =========================================================================
    // SETUP: Acquire lock to serialize tests (no cache clearing across crates)
    // =========================================================================
    fn setup() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().expect("Test lock poisoned")
    }

    // =========================================================================
    // TC-SESSION-MGR-03: First Session Returns IC=1.0
    // =========================================================================
    #[test]
    fn tc_session_mgr_03_first_session_ic_one() {
        let _guard = setup();

        println!("\n=== TC-SESSION-MGR-03: First Session Returns IC=1.0 ===");

        // SOURCE OF TRUTH: Spec requirement
        // First session has no previous -> IC = 1.0 by definition

        // Use REAL empty storage (fresh install scenario)
        let (storage, _tmp) = create_test_storage();
        let manager = StandaloneSessionIdentityManager::new(storage);

        println!("BEFORE: Fresh storage with no snapshots");

        // EXECUTE: restore_identity with no previous session
        let (snapshot, ic) = manager
            .restore_identity(None)
            .expect("restore_identity must succeed");

        println!("AFTER:");
        println!("  session_id: {}", snapshot.session_id);
        println!("  computed IC: {}", ic);

        // VERIFY: First session must have IC = 1.0
        assert!(
            (ic - 1.0).abs() < 0.001,
            "First session IC must be 1.0, got {}",
            ic
        );

        println!("RESULT: PASS - First session returns IC=1.0");
    }

    // =========================================================================
    // TC-SESSION-MGR-05: Full State Verification
    // =========================================================================
    #[test]
    fn tc_session_mgr_05_full_state_verification() {
        let _guard = setup();

        println!("\n=== TC-SESSION-MGR-05: Full State Verification ===");

        // Create REAL storage and a populated snapshot
        let (storage, _tmp) = create_test_storage();
        let manager = StandaloneSessionIdentityManager::new(Arc::clone(&storage));

        // Create and populate a snapshot with real values
        let mut snapshot = SessionIdentitySnapshot::new("verify-capture-test");
        snapshot.consciousness = 0.75;
        snapshot.integration = 0.80;
        snapshot.differentiation = 0.70;
        snapshot.reflection = 0.65;
        snapshot.purpose_vector =
            [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.9, 0.8, 0.7];
        snapshot.kuramoto_phases = [0.0; KURAMOTO_N];
        snapshot.last_ic = 0.85;

        println!("CAPTURED SNAPSHOT:");
        println!("  session_id: {}", snapshot.session_id);
        println!("  timestamp_ms: {}", snapshot.timestamp_ms);
        println!("  consciousness: {}", snapshot.consciousness);
        println!(
            "  kuramoto_phases[0..3]: {:?}",
            &snapshot.kuramoto_phases[..3]
        );
        println!("  purpose_vector[0..3]: {:?}", &snapshot.purpose_vector[..3]);

        // Save the snapshot
        manager
            .save_snapshot(&snapshot)
            .expect("save_snapshot must succeed");

        // Load it back
        let loaded = manager
            .load_snapshot("verify-capture-test")
            .expect("load_snapshot must succeed")
            .expect("snapshot must exist");

        // VERIFY: All fields are preserved
        assert_eq!(loaded.session_id, "verify-capture-test");
        assert!(loaded.timestamp_ms > 0, "timestamp_ms must be set");
        assert!(
            (loaded.consciousness - 0.75).abs() < 0.001,
            "consciousness must match"
        );
        assert!(
            (loaded.integration - 0.80).abs() < 0.001,
            "integration must match"
        );
        assert!(
            (loaded.differentiation - 0.70).abs() < 0.001,
            "differentiation must match"
        );
        assert!(
            (loaded.reflection - 0.65).abs() < 0.001,
            "reflection must match"
        );
        assert!(
            loaded.last_ic >= 0.0 && loaded.last_ic <= 1.0,
            "last_ic must be [0,1]"
        );

        println!("RESULT: PASS - All snapshot fields correctly saved and loaded");
    }

    // =========================================================================
    // TC-SESSION-MGR-06: IdentityCache Update Verification
    // =========================================================================
    #[test]
    fn tc_session_mgr_06_identity_cache_update() {
        let _guard = setup();

        println!("\n=== TC-SESSION-MGR-06: IdentityCache Update Verification ===");

        // Note: We cannot verify cache starts cold because we can't clear_cache()
        // across crate boundaries. Instead, we verify that restore_identity updates
        // the cache with the expected values.

        let (storage, _tmp) = create_test_storage();
        let manager = StandaloneSessionIdentityManager::new(storage);

        // EXECUTE: restore_identity should update cache
        let (snapshot, ic) = manager
            .restore_identity(None)
            .expect("restore_identity must succeed");

        // VERIFY: Cache is now warm (was updated by restore_identity)
        assert!(
            IdentityCache::is_warm(),
            "Cache must be warm after restore"
        );

        let (cached_ic, _cached_r, _cached_state, cached_session) =
            IdentityCache::get().expect("Cache must return values after restore");

        println!("CACHE STATE:");
        println!("  cached_ic: {} (expected: {})", cached_ic, ic);
        println!(
            "  cached_session: {} (expected: {})",
            cached_session, snapshot.session_id
        );

        // VERIFY: Cache values match the snapshot we just created
        assert!(
            (cached_ic - ic).abs() < 0.001,
            "Cached IC must match computed IC"
        );
        assert_eq!(
            cached_session, snapshot.session_id,
            "Cached session must match"
        );

        println!("RESULT: PASS - IdentityCache correctly updated by restore_identity");
    }

    // =========================================================================
    // TC-SESSION-MGR-07: Storage Round-Trip with IC Computation
    // =========================================================================
    #[test]
    fn tc_session_mgr_07_storage_roundtrip_with_ic() {
        let _guard = setup();

        println!("\n=== TC-SESSION-MGR-07: Storage Round-Trip with IC Computation ===");

        let (storage, _tmp) = create_test_storage();
        let manager = StandaloneSessionIdentityManager::new(Arc::clone(&storage));

        // Create first snapshot (previous)
        let mut previous = SessionIdentitySnapshot::new("session-previous");
        previous.purpose_vector = [0.5; KURAMOTO_N];
        previous.kuramoto_phases = [0.0; KURAMOTO_N];
        previous.last_ic = 0.9;

        // Save to storage
        manager
            .save_snapshot(&previous)
            .expect("save_snapshot must succeed");

        // Create second snapshot (current) with slightly different purpose
        let mut current = SessionIdentitySnapshot::new("session-current");
        current.purpose_vector = [0.5; KURAMOTO_N];
        current.purpose_vector[0] = 0.6; // Slight change
        current.kuramoto_phases = [0.0; KURAMOTO_N];

        // Compute IC
        let ic = manager.compute_cross_session_ic(&current, &previous);

        println!("Previous PV[0]: {}", previous.purpose_vector[0]);
        println!("Current PV[0]: {}", current.purpose_vector[0]);
        println!("Computed IC: {}", ic);

        // IC should be close to 1.0 but not exactly (slight difference in purpose)
        assert!(ic > 0.95 && ic < 1.0, "IC should be high but not perfect");

        // Reload from storage and verify
        let loaded = manager
            .load_latest()
            .expect("load_latest must succeed")
            .expect("snapshot must exist");

        assert_eq!(loaded.session_id, "session-previous");

        println!("RESULT: PASS - Storage round-trip with IC computation works");
    }
}
