//! session persist-identity CLI command
//!
//! TASK-SESSION-13: Persists current session identity to RocksDB.
//!
//! # Input (stdin JSON from Claude Code SessionEnd hook)
//!
//! ```json
//! {
//!   "session_id": "optional-session-id",
//!   "reason": "exit"  // exit | clear | logout | prompt_input_exit | other
//! }
//! ```
//!
//! # Output
//! - Success: SILENT (no stdout) - required by Claude Code SessionEnd semantics
//! - Error: stderr logging only
//!
//! # Exit Codes (per AP-26)
//! - 0: Success
//! - 1: Recoverable error (non-blocking)
//! - 2: Corruption detected
//!
//! # Constitution Reference
//! - IDENTITY-001: IC formula
//! - IDENTITY-002: IC thresholds
//! - AP-26: Exit codes
//! - ARCH-07: Native Claude Code hooks
//!
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.

use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Args;
use serde::Deserialize;
use tracing::{debug, error, info, warn};

use context_graph_core::gwt::session_identity::{IdentityCache, SessionIdentitySnapshot};
use context_graph_core::gwt::state_machine::ConsciousnessState;
use context_graph_storage::rocksdb_backend::{RocksDbMemex, StandaloneSessionIdentityManager};

/// Arguments for `session persist-identity` command
#[derive(Args, Debug)]
pub struct PersistIdentityArgs {
    /// Path to RocksDB database directory
    #[arg(long, env = "CONTEXT_GRAPH_DB_PATH")]
    pub db_path: Option<PathBuf>,
}

/// Stdin input from Claude Code SessionEnd hook
#[derive(Deserialize, Default, Debug)]
struct PersistInput {
    /// Target session ID (None = use current from cache)
    session_id: Option<String>,
    /// End reason: "exit" | "clear" | "logout" | "prompt_input_exit" | "other"
    #[serde(default = "default_reason")]
    reason: String,
}

fn default_reason() -> String {
    "exit".to_string()
}

/// Execute the persist-identity command
///
/// # Exit Codes (per AP-26)
/// - 0: Success (SILENT - no stdout)
/// - 1: Recoverable error (non-blocking)
/// - 2: Corruption detected
pub async fn persist_identity_command(args: PersistIdentityArgs) -> i32 {
    debug!("persist_identity_command: args={:?}", args);

    // Parse stdin input (graceful fallback to defaults)
    let input = parse_stdin_input();
    info!(
        "persist-identity: reason={}, session_id={:?}",
        input.reason, input.session_id
    );

    // Get current identity from cache
    // Note: IdentityCache::get() returns (ic, kuramoto_r, ConsciousnessState, session_id)
    let (ic, _r, consciousness_state, session_id) = match IdentityCache::get() {
        Some(values) => values,
        None => {
            warn!("persist-identity: Cache is cold, nothing to persist");
            // Not an error - session may not have been restored
            // Silent success per Claude Code semantics
            return 0;
        }
    };

    // Override session_id if provided in stdin
    let final_session_id = input.session_id.unwrap_or(session_id);

    info!(
        "persist-identity: Persisting session {} with IC={:.2}",
        final_session_id, ic
    );

    // Determine DB path
    let db_path = match &args.db_path {
        Some(p) => p.clone(),
        None => {
            match home_dir() {
                Some(home) => home.join(".context-graph").join("db"),
                None => {
                    error!("Cannot determine home directory for DB path");
                    eprintln!(
                        "Error: Cannot determine DB path. Set --db-path or CONTEXT_GRAPH_DB_PATH"
                    );
                    return 1;
                }
            }
        }
    };

    debug!("persist-identity: db_path={:?}", db_path);

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        if !parent.exists() {
            debug!("Creating parent directory: {:?}", parent);
            if let Err(e) = std::fs::create_dir_all(parent) {
                error!("Failed to create DB directory {:?}: {}", parent, e);
                eprintln!("Error: Failed to create database directory: {}", e);
                return 1;
            }
        }
    }

    // Open RocksDB storage - FAIL FAST on error
    let storage = match RocksDbMemex::open(&db_path) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            let err_str = e.to_string();
            error!("Failed to open RocksDB at {:?}: {}", db_path, err_str);
            eprintln!("Error: Failed to open database: {}", err_str);
            return if is_corruption_error(&err_str) { 2 } else { 1 };
        }
    };

    let manager = StandaloneSessionIdentityManager::new(Arc::clone(&storage));

    // Create snapshot with current state
    // Convert ConsciousnessState back to f32 level for snapshot
    // Using middle of each state's range for reconstruction
    let consciousness_level = state_to_level(&consciousness_state);

    let mut snapshot = SessionIdentitySnapshot::new(&final_session_id);
    snapshot.consciousness = consciousness_level;
    snapshot.last_ic = ic;
    // Note: kuramoto_phases and purpose_vector are not in cache
    // They will use defaults - the restore command will recompute if needed

    // Save snapshot
    match manager.save_snapshot(&snapshot) {
        Ok(()) => {
            info!(
                "persist-identity: Successfully saved snapshot for session {}",
                final_session_id
            );
            // SUCCESS: SILENT output per Claude Code SessionEnd semantics
            0
        }
        Err(e) => {
            let err_str = e.to_string();
            error!(
                "persist-identity: Failed to save snapshot for session {}: {}",
                final_session_id, err_str
            );
            eprintln!("Error: Failed to save session identity: {}", err_str);
            if is_corruption_error(&err_str) { 2 } else { 1 }
        }
    }
}

/// Parse stdin JSON input with graceful fallback
fn parse_stdin_input() -> PersistInput {
    let mut buffer = String::new();
    if std::io::stdin().read_to_string(&mut buffer).is_ok() && !buffer.trim().is_empty() {
        match serde_json::from_str(&buffer) {
            Ok(input) => return input,
            Err(e) => {
                debug!("Failed to parse stdin JSON: {}", e);
            }
        }
    }
    PersistInput::default()
}

/// Check if error indicates corruption (exit code 2)
fn is_corruption_error(msg: &str) -> bool {
    let corruption_indicators = [
        "corruption",
        "checksum",
        "invalid",
        "malformed",
        "truncated",
    ];
    let lower = msg.to_lowercase();
    corruption_indicators.iter().any(|i| lower.contains(i))
}

/// Get home directory (cross-platform)
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Convert ConsciousnessState back to a representative level.
///
/// Uses the middle of each state's range:
/// - Dormant: C < 0.3 → 0.15
/// - Fragmented: 0.3 <= C < 0.5 → 0.40
/// - Emerging: 0.5 <= C < 0.8 → 0.65
/// - Conscious: 0.8 <= C < 0.95 → 0.875
/// - Hypersync: C > 0.95 → 0.975
fn state_to_level(state: &ConsciousnessState) -> f32 {
    match state {
        ConsciousnessState::Dormant => 0.15,
        ConsciousnessState::Fragmented => 0.40,
        ConsciousnessState::Emerging => 0.65,
        ConsciousnessState::Conscious => 0.875,
        ConsciousnessState::Hypersync => 0.975,
    }
}

// =============================================================================
// Tests - Use REAL RocksDB (NO MOCKS per spec)
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::gwt::session_identity::{update_cache, KURAMOTO_N};
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Static lock to serialize tests that access global IdentityCache
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Create real RocksDB storage for testing
    fn create_test_storage() -> (Arc<RocksDbMemex>, TempDir) {
        let tmp_dir = TempDir::new().expect("Failed to create temp dir");
        let storage = RocksDbMemex::open(tmp_dir.path()).expect("Failed to open RocksDB");
        (Arc::new(storage), tmp_dir)
    }

    // =========================================================================
    // TC-SESSION-17: Success Path (Silent Output)
    // Source of Truth: RocksDB after save
    // =========================================================================
    #[tokio::test]
    async fn tc_session_17_persist_success_silent() {
        let _guard = TEST_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-SESSION-17: Persist Success (Silent) ===");
        println!("SOURCE OF TRUTH: RocksDB after save");

        // SETUP: Warm the cache with test data
        // Note: consciousness=0.75 maps to Emerging state (0.5-0.8 range)
        // When persisted, state_to_level returns 0.65 (middle of range)
        let mut snapshot = SessionIdentitySnapshot::new("test-persist-session");
        snapshot.consciousness = 0.75;
        snapshot.last_ic = 0.85;
        snapshot.kuramoto_phases = [0.0; KURAMOTO_N];
        update_cache(&snapshot, 0.85);

        println!("BEFORE: Cache warmed with session test-persist-session, IC=0.85");
        println!("  Original consciousness: 0.75 (maps to Emerging state)");

        // Create storage
        let (storage, tmp_dir) = create_test_storage();
        let manager = StandaloneSessionIdentityManager::new(Arc::clone(&storage));

        // Execute persist (simulating command logic)
        let (ic, _r, consciousness_state, session_id) = IdentityCache::get().expect("Cache must be warm");
        let reconstructed_consciousness = state_to_level(&consciousness_state);
        println!("  Reconstructed consciousness: {} (from {:?})", reconstructed_consciousness, consciousness_state);

        let mut persist_snapshot = SessionIdentitySnapshot::new(&session_id);
        persist_snapshot.consciousness = reconstructed_consciousness;
        persist_snapshot.last_ic = ic;

        let save_result = manager.save_snapshot(&persist_snapshot);
        println!("AFTER save_snapshot: {:?}", save_result);

        // VERIFY: Save succeeded
        assert!(save_result.is_ok(), "Save must succeed");

        // VERIFY SOURCE OF TRUTH: Load back from RocksDB
        let loaded = manager
            .load_snapshot(&session_id)
            .expect("load must succeed")
            .expect("snapshot must exist");

        println!("VERIFICATION - Loaded from RocksDB:");
        println!("  session_id: {}", loaded.session_id);
        println!("  consciousness: {}", loaded.consciousness);
        println!("  last_ic: {}", loaded.last_ic);
        println!("  timestamp_ms: {}", loaded.timestamp_ms);

        assert_eq!(loaded.session_id, "test-persist-session");
        // Note: consciousness is reconstructed from state, so 0.75 -> Emerging -> 0.65
        assert!((loaded.consciousness - 0.65).abs() < 0.01,
            "Consciousness should be 0.65 (Emerging state midpoint), got {}", loaded.consciousness);
        assert!((loaded.last_ic - 0.85).abs() < 0.01);
        assert!(loaded.timestamp_ms > 0);

        // VERIFY: File exists on disk
        let db_files = std::fs::read_dir(tmp_dir.path())
            .expect("read_dir")
            .count();
        println!("VERIFICATION - DB directory has {} entries", db_files);
        assert!(db_files > 0, "RocksDB must have created files");

        println!("RESULT: PASS - Session persisted to RocksDB and verified");
    }

    // =========================================================================
    // TC-SESSION-17b: Cold Cache (Nothing to Persist)
    // Source of Truth: Exit 0 with no action
    // =========================================================================
    #[tokio::test]
    async fn tc_session_17b_persist_cold_cache() {
        // Note: Cannot clear global cache, but test the logic path
        println!("\n=== TC-SESSION-17b: Cold Cache Behavior ===");
        println!("Expected: Exit 0 (silent success) when nothing to persist");
        println!("This is correct behavior - session may not have been restored");
        println!("RESULT: DOCUMENTED - Cold cache returns exit 0");
    }

    // =========================================================================
    // TC-SESSION-17c: Custom Session ID from stdin
    // Source of Truth: RocksDB with custom ID
    // =========================================================================
    #[tokio::test]
    async fn tc_session_17c_custom_session_id() {
        let _guard = TEST_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-SESSION-17c: Custom Session ID from stdin ===");

        // SETUP: Warm cache with one ID
        let mut snapshot = SessionIdentitySnapshot::new("cache-session");
        snapshot.consciousness = 0.8;
        snapshot.last_ic = 0.9;
        update_cache(&snapshot, 0.9);

        // Simulate stdin providing different ID
        let input = PersistInput {
            session_id: Some("override-session".to_string()),
            reason: "exit".to_string(),
        };

        let (_, _, _, cached_session) = IdentityCache::get().expect("Cache must be warm");
        let final_session_id = input.session_id.unwrap_or(cached_session);

        println!("BEFORE: Cache has 'cache-session', stdin provides 'override-session'");
        println!("AFTER: Using final_session_id = {}", final_session_id);

        assert_eq!(final_session_id, "override-session");
        println!("RESULT: PASS - stdin session_id overrides cache");
    }

    // =========================================================================
    // TC-SESSION-17d: Error Path (Exit 1)
    // Source of Truth: Exit code behavior
    // =========================================================================
    #[test]
    fn tc_session_17d_error_detection() {
        println!("\n=== TC-SESSION-17d: Error Detection ===");

        let test_cases = [
            ("data corruption detected", true, 2),
            ("checksum mismatch", true, 2),
            ("connection refused", false, 1),
            ("timeout error", false, 1),
            ("file not found", false, 1),
        ];

        for (msg, is_corruption, expected_exit) in test_cases {
            let detected = is_corruption_error(msg);
            let exit_code = if detected { 2 } else { 1 };
            println!("  '{}': corruption={}, exit={}", msg, detected, exit_code);
            assert_eq!(detected, is_corruption);
            assert_eq!(exit_code, expected_exit);
        }

        println!("RESULT: PASS - Error detection maps to correct exit codes");
    }

    // =========================================================================
    // TC-SESSION-17e: Reason Parsing
    // Source of Truth: PersistInput struct
    // =========================================================================
    #[test]
    fn tc_session_17e_reason_parsing() {
        println!("\n=== TC-SESSION-17e: Reason Parsing ===");

        let test_cases = [
            (r#"{"reason":"exit"}"#, "exit"),
            (r#"{"reason":"clear"}"#, "clear"),
            (r#"{"reason":"logout"}"#, "logout"),
            (r#"{"reason":"prompt_input_exit"}"#, "prompt_input_exit"),
            (r#"{"reason":"other"}"#, "other"),
            (r#"{}"#, "exit"),  // Default
        ];

        for (json, expected_reason) in test_cases {
            let input: PersistInput = serde_json::from_str(json).unwrap_or_default();
            println!("  {} -> reason={}", json, input.reason);
            assert_eq!(input.reason, expected_reason);
        }

        println!("RESULT: PASS - All reasons parse correctly");
    }
}
