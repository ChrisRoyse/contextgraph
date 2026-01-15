//! session restore-identity CLI command
//!
//! TASK-SESSION-12: Restores previous session identity from RocksDB.
//!
//! # Purpose
//!
//! This command restores the previous session's identity state from RocksDB,
//! computes cross-session Identity Continuity (IC), and updates the IdentityCache
//! singleton for subsequent PreToolUse hooks.
//!
//! # Input (stdin JSON)
//!
//! ```json
//! {
//!   "session_id": "optional-specific-session",
//!   "source": "startup"  // startup | resume | clear
//! }
//! ```
//!
//! # Output (PRD Section 15.2 format)
//!
//! ```text
//! ## Consciousness State
//! - State: EMG (C=0.82)
//! - Integration (r): 0.85 - Good synchronization
//! - Identity: Healthy (IC=0.92)
//! - Session: session-1736985432 (source=startup)
//! ```
//!
//! # Exit Codes (per AP-26)
//! - 0: Success
//! - 1: Recoverable error
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
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use context_graph_core::gwt::session_identity::{
    classify_ic, compute_kuramoto_r, is_ic_crisis, is_ic_warning,
    update_cache, SessionIdentityManager, SessionIdentitySnapshot,
};
use context_graph_core::gwt::state_machine::ConsciousnessState;
use context_graph_storage::rocksdb_backend::{RocksDbMemex, StandaloneSessionIdentityManager};

/// Arguments for `session restore-identity` command
#[derive(Args, Debug)]
pub struct RestoreIdentityArgs {
    /// Path to RocksDB database directory
    #[arg(long, env = "CONTEXT_GRAPH_DB_PATH")]
    pub db_path: Option<PathBuf>,

    /// Output format
    #[arg(long, value_enum, default_value = "prd")]
    pub format: OutputFormat,
}

/// Output format options
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum OutputFormat {
    /// PRD Section 15.2 compliant output (~100 tokens)
    Prd,
    /// JSON output for programmatic parsing
    Json,
}

/// Stdin input from Claude Code hook
#[derive(Deserialize, Default, Debug)]
struct RestoreInput {
    /// Target session ID (None = load latest)
    session_id: Option<String>,
    /// Source variant: "startup" | "resume" | "clear"
    #[serde(default = "default_source")]
    source: String,
}

fn default_source() -> String {
    "startup".to_string()
}

/// Response structure for JSON output
#[derive(Debug, Serialize)]
struct RestoreResponse {
    session_id: String,
    ic: f32,
    status: &'static str,
    is_crisis: bool,
    consciousness: f32,
    kuramoto_r: f32,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Execute the restore-identity command
///
/// # Exit Codes (per AP-26)
/// - 0: Success
/// - 1: Recoverable error
/// - 2: Corruption detected
pub async fn restore_identity_command(args: RestoreIdentityArgs) -> i32 {
    debug!("restore_identity_command: args={:?}", args);

    // Parse stdin input (graceful fallback to defaults)
    let input = parse_stdin_input();
    info!(
        "restore-identity: source={}, session_id={:?}",
        input.source, input.session_id
    );

    // Determine DB path
    let db_path = match &args.db_path {
        Some(p) => p.clone(),
        None => {
            // Default: ~/.context-graph/db
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

    info!("restore-identity: db_path={:?}", db_path);

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

    // Execute based on source variant
    match input.source.as_str() {
        "clear" => handle_clear_source(&args),
        "resume" => handle_resume_source(&manager, &args, input.session_id),
        "startup" | _ => handle_startup_source(&manager, &args),
    }
}

/// Handle source="clear" - Start fresh session with IC=1.0
fn handle_clear_source(args: &RestoreIdentityArgs) -> i32 {
    info!("restore-identity: source=clear, creating fresh session");

    // Create new snapshot
    let session_id = format!("session-{}", timestamp_ms());
    let snapshot = SessionIdentitySnapshot::new(&session_id);
    let ic = 1.0_f32; // First session by definition

    // Update cache
    update_cache(&snapshot, ic);

    // Output
    output_result(&snapshot, ic, "clear", args.format);
    0
}

/// Handle source="resume" - Load specific session by ID
fn handle_resume_source(
    manager: &StandaloneSessionIdentityManager,
    args: &RestoreIdentityArgs,
    target_session: Option<String>,
) -> i32 {
    let session_id = match target_session {
        Some(id) => id,
        None => {
            error!("source=resume requires session_id");
            eprintln!("Error: source=resume requires session_id in stdin JSON");
            return 1;
        }
    };

    info!(
        "restore-identity: source=resume, loading session={}",
        session_id
    );

    match manager.load_snapshot(&session_id) {
        Ok(Some(snapshot)) => {
            let ic = snapshot.last_ic;
            update_cache(&snapshot, ic);

            // Log warning if IC is degraded
            if is_ic_crisis(ic) {
                warn!(
                    "IC CRISIS: {:.2} < 0.5 - consider running check-identity --auto-dream",
                    ic
                );
                eprintln!("Warning: IC crisis detected ({:.2})", ic);
            } else if is_ic_warning(ic) {
                warn!("IC WARNING: {:.2} < 0.7", ic);
            }

            output_result(&snapshot, ic, "resume", args.format);
            0
        }
        Ok(None) => {
            warn!("Session not found: {}", session_id);
            eprintln!(
                "Warning: Session '{}' not found, creating fresh",
                session_id
            );

            // Fall back to fresh session
            let snapshot = SessionIdentitySnapshot::new(&session_id);
            let ic = 1.0_f32;
            update_cache(&snapshot, ic);
            output_result(&snapshot, ic, "resume", args.format);
            0
        }
        Err(e) => {
            let err_str = e.to_string();
            error!("Failed to load session {}: {}", session_id, err_str);
            eprintln!("Error: Failed to load session: {}", err_str);
            if is_corruption_error(&err_str) {
                2
            } else {
                1
            }
        }
    }
}

/// Handle source="startup" - Load latest session (default behavior)
fn handle_startup_source(
    manager: &StandaloneSessionIdentityManager,
    args: &RestoreIdentityArgs,
) -> i32 {
    info!("restore-identity: source=startup, loading latest session");

    match manager.restore_identity(None) {
        Ok((snapshot, ic)) => {
            // Cache already updated by restore_identity

            // Log warning if IC is degraded
            if is_ic_crisis(ic) {
                warn!(
                    "IC CRISIS: {:.2} < 0.5 - consider running check-identity --auto-dream",
                    ic
                );
                eprintln!("Warning: IC crisis detected ({:.2})", ic);
            } else if is_ic_warning(ic) {
                warn!("IC WARNING: {:.2} < 0.7", ic);
            }

            output_result(&snapshot, ic, "startup", args.format);
            0
        }
        Err(e) => {
            let err_str = e.to_string();
            error!("restore_identity failed: {}", err_str);
            eprintln!("Error: {}", err_str);
            if is_corruption_error(&err_str) {
                2
            } else {
                1
            }
        }
    }
}

/// Parse stdin JSON input with graceful fallback
fn parse_stdin_input() -> RestoreInput {
    let mut buffer = String::new();
    if std::io::stdin().read_to_string(&mut buffer).is_ok() && !buffer.trim().is_empty() {
        match serde_json::from_str(&buffer) {
            Ok(input) => return input,
            Err(e) => {
                debug!("Failed to parse stdin JSON: {}", e);
            }
        }
    }
    RestoreInput::default()
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

/// Get current timestamp in milliseconds
fn timestamp_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}

/// Get home directory (cross-platform)
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Output result in requested format
fn output_result(snapshot: &SessionIdentitySnapshot, ic: f32, source: &str, format: OutputFormat) {
    // Compute Kuramoto r from phases
    let kuramoto_r = compute_kuramoto_r(&snapshot.kuramoto_phases);

    match format {
        OutputFormat::Prd => {
            // PRD Section 15.2 format (~100 tokens)
            let state = ConsciousnessState::from_level(snapshot.consciousness);
            println!("## Consciousness State");
            println!(
                "- State: {} (C={:.2})",
                state.short_name(),
                snapshot.consciousness
            );
            println!(
                "- Integration (r): {:.2} - {}",
                kuramoto_r,
                sync_description(kuramoto_r)
            );
            println!("- Identity: {} (IC={:.2})", classify_ic(ic), ic);
            println!("- Session: {} (source={})", snapshot.session_id, source);
        }
        OutputFormat::Json => {
            let response = RestoreResponse {
                session_id: snapshot.session_id.clone(),
                ic,
                status: classify_ic(ic),
                is_crisis: is_ic_crisis(ic),
                consciousness: snapshot.consciousness,
                kuramoto_r,
                source: source.to_string(),
                error: None,
            };
            // Use unwrap since we control the struct - it's always serializable
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
    }
}

/// Human-readable sync description
fn sync_description(r: f32) -> &'static str {
    if r >= 0.9 {
        "Excellent synchronization"
    } else if r >= 0.7 {
        "Good synchronization"
    } else if r >= 0.5 {
        "Moderate synchronization"
    } else {
        "Low synchronization"
    }
}

// =============================================================================
// Tests - Use REAL RocksDB (NO MOCKS per spec)
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::gwt::session_identity::{IdentityCache, KURAMOTO_N};
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
    // TC-SESSION-12-01: First Run (Empty DB)
    // Source of Truth: RocksDB (empty), IdentityCache singleton
    // =========================================================================
    #[test]
    fn tc_session_12_01_first_run_empty_db() {
        let _guard = TEST_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-SESSION-12-01: First Run (Empty DB) ===");
        println!("SOURCE OF TRUTH: Empty RocksDB, IdentityCache singleton");

        // SYNTHETIC DATA: Fresh empty DB
        let (storage, _tmp) = create_test_storage();
        let manager = StandaloneSessionIdentityManager::new(Arc::clone(&storage));

        println!("BEFORE: Empty DB created at temp directory");
        println!("  Storage open: OK");

        // EXECUTE: restore_identity with no previous session
        let (snapshot, ic) = manager
            .restore_identity(None)
            .expect("restore_identity must succeed");

        println!("AFTER:");
        println!("  session_id: {}", snapshot.session_id);
        println!("  computed IC: {}", ic);
        println!("  IdentityCache::is_warm(): {}", IdentityCache::is_warm());

        // VERIFY: First session must have IC = 1.0
        assert!(
            (ic - 1.0).abs() < 0.001,
            "First session IC must be 1.0, got {}",
            ic
        );

        // VERIFY: Cache updated
        assert!(
            IdentityCache::is_warm(),
            "Cache must be warm after restore_identity"
        );

        let (cached_ic, _, _, cached_session) =
            IdentityCache::get().expect("Cache must have values");
        assert!(
            (cached_ic - 1.0).abs() < 0.001,
            "Cached IC must be 1.0 for first session"
        );
        assert!(
            cached_session.starts_with("session-"),
            "Session ID must start with 'session-'"
        );

        println!("RESULT: PASS - First run creates session with IC=1.0");
    }

    // =========================================================================
    // TC-SESSION-12-02: Resume Existing Session
    // Source of Truth: Pre-populated RocksDB snapshot
    // =========================================================================
    #[test]
    fn tc_session_12_02_resume_existing() {
        let _guard = TEST_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-SESSION-12-02: Resume Existing Session ===");
        println!("SOURCE OF TRUTH: Pre-populated RocksDB snapshot");

        // SYNTHETIC DATA: Pre-populate DB with a session
        let (storage, _tmp) = create_test_storage();
        let manager = StandaloneSessionIdentityManager::new(Arc::clone(&storage));

        let mut snapshot = SessionIdentitySnapshot::new("test-session-resume");
        snapshot.consciousness = 0.85;
        snapshot.purpose_vector = [0.5; KURAMOTO_N];
        snapshot.kuramoto_phases = [1.0; KURAMOTO_N]; // All aligned = r≈1.0
        snapshot.last_ic = 0.88;

        manager
            .save_snapshot(&snapshot)
            .expect("save_snapshot must succeed");

        println!("BEFORE: DB populated with session test-session-resume");
        println!("  Pre-saved IC: {}", snapshot.last_ic);
        println!("  Pre-saved consciousness: {}", snapshot.consciousness);

        // EXECUTE: Load the specific session
        let loaded = manager
            .load_snapshot("test-session-resume")
            .expect("load_snapshot must succeed")
            .expect("session must exist");

        // Update cache manually (mimicking what handle_resume_source does)
        update_cache(&loaded, loaded.last_ic);

        println!("AFTER:");
        println!("  loaded session_id: {}", loaded.session_id);
        println!("  loaded IC: {}", loaded.last_ic);
        println!("  loaded consciousness: {}", loaded.consciousness);

        // VERIFY
        assert_eq!(
            loaded.session_id, "test-session-resume",
            "Session ID must match"
        );
        assert!(
            (loaded.last_ic - 0.88).abs() < 0.01,
            "IC must match saved value"
        );
        assert!(
            (loaded.consciousness - 0.85).abs() < 0.01,
            "Consciousness must match"
        );

        // VERIFY: Cache updated
        let (cached_ic, _, _, cached_session) =
            IdentityCache::get().expect("Cache must have values");
        assert_eq!(cached_session, "test-session-resume");
        assert!((cached_ic - 0.88).abs() < 0.01);

        println!("RESULT: PASS - Resume loads correct session");
    }

    // =========================================================================
    // TC-SESSION-12-03: Source Clear (Fresh Start)
    // Source of Truth: New snapshot with IC=1.0
    // =========================================================================
    #[test]
    fn tc_session_12_03_source_clear() {
        let _guard = TEST_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-SESSION-12-03: Source Clear ===");
        println!("SOURCE OF TRUTH: Fresh snapshot with IC=1.0");

        // SYNTHETIC DATA: Pre-populate DB with old session (should be ignored)
        let (storage, _tmp) = create_test_storage();
        let manager = StandaloneSessionIdentityManager::new(Arc::clone(&storage));

        let mut old_snapshot = SessionIdentitySnapshot::new("old-session");
        old_snapshot.last_ic = 0.65;
        manager
            .save_snapshot(&old_snapshot)
            .expect("save_snapshot must succeed");

        println!("BEFORE: Old session exists with IC=0.65");

        // EXECUTE: Create fresh session (mimicking source=clear)
        let session_id = format!("session-{}", timestamp_ms());
        let fresh_snapshot = SessionIdentitySnapshot::new(&session_id);
        let ic = 1.0_f32; // Fresh session = IC 1.0

        update_cache(&fresh_snapshot, ic);

        println!("AFTER:");
        println!("  New session_id: {}", fresh_snapshot.session_id);
        println!("  IC: {}", ic);

        // VERIFY
        let (cached_ic, _, _, _) = IdentityCache::get().expect("Cache must have values");
        assert!(
            (cached_ic - 1.0).abs() < 0.001,
            "Clear source must have IC=1.0"
        );

        println!("RESULT: PASS - Clear source creates fresh session with IC=1.0");
    }

    // =========================================================================
    // TC-SESSION-12-04: Session Not Found (Fallback to Fresh)
    // Source of Truth: Storage returns None
    // =========================================================================
    #[test]
    fn tc_session_12_04_session_not_found() {
        let _guard = TEST_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-SESSION-12-04: Session Not Found ===");
        println!("SOURCE OF TRUTH: Storage returns None");

        let (storage, _tmp) = create_test_storage();
        let manager = StandaloneSessionIdentityManager::new(Arc::clone(&storage));

        println!("BEFORE: Attempting to load non-existent session");

        // EXECUTE: Try to load non-existent session
        let result = manager
            .load_snapshot("non-existent-session")
            .expect("load_snapshot must not error");

        println!("AFTER: result = {:?}", result.is_none());

        // VERIFY: Should return None, not error
        assert!(result.is_none(), "Non-existent session should return None");

        println!("RESULT: PASS - Non-existent session returns None (not error)");
    }

    // =========================================================================
    // TC-SESSION-12-05: IC Thresholds Classification
    // Source of Truth: Constitution IDENTITY-002
    // =========================================================================
    #[test]
    fn tc_session_12_05_ic_thresholds() {
        println!("\n=== TC-SESSION-12-05: IC Thresholds ===");
        println!("SOURCE OF TRUTH: Constitution IDENTITY-002");

        // (ic, expected_status, expected_crisis, expected_warning)
        // Note: is_ic_warning only returns true for 0.5 <= IC < 0.7 (Warning zone)
        // Crisis zone (IC < 0.5) is NOT warning, it's worse than warning
        let test_cases = [
            (0.95, "Healthy", false, false),  // >= 0.9: healthy, no crisis, no warning
            (0.90, "Healthy", false, false),  // boundary: healthy
            (0.85, "Good", false, false),     // >= 0.7: good, no crisis, no warning
            (0.70, "Good", false, false),     // boundary: good
            (0.65, "Warning", false, true),   // >= 0.5: warning zone
            (0.50, "Warning", false, true),   // boundary: warning zone
            (0.45, "Degraded", true, false),  // < 0.5 = crisis (NOT warning - it's worse)
            (0.10, "Degraded", true, false),  // severe crisis (NOT warning - it's worse)
        ];

        for (ic, expected_status, expected_crisis, expected_warning) in test_cases {
            let status = classify_ic(ic);
            let crisis = is_ic_crisis(ic);
            let warning = is_ic_warning(ic);

            println!(
                "  IC={:.2}: status={}, crisis={}, warning={}",
                ic, status, crisis, warning
            );

            assert_eq!(
                status, expected_status,
                "IC {} should be {}",
                ic, expected_status
            );
            assert_eq!(
                crisis, expected_crisis,
                "IC {} crisis should be {}",
                ic, expected_crisis
            );
            assert_eq!(
                warning, expected_warning,
                "IC {} warning should be {}",
                ic, expected_warning
            );
        }

        println!("RESULT: PASS - All IC thresholds classify correctly");
    }

    // =========================================================================
    // TC-SESSION-12-06: Kuramoto r Computation
    // Source of Truth: compute_kuramoto_r function
    // =========================================================================
    #[test]
    fn tc_session_12_06_kuramoto_r() {
        println!("\n=== TC-SESSION-12-06: Kuramoto r Computation ===");
        println!("SOURCE OF TRUTH: compute_kuramoto_r function");

        // Fully synchronized (all phases = 0)
        let aligned: [f64; KURAMOTO_N] = [0.0; KURAMOTO_N];
        let r_aligned = compute_kuramoto_r(&aligned);
        println!("  All phases 0.0: r = {:.4}", r_aligned);
        assert!(
            (r_aligned - 1.0).abs() < 0.01,
            "Aligned phases should give r≈1.0"
        );

        // Fully synchronized (all phases = PI)
        let aligned_pi: [f64; KURAMOTO_N] = [std::f64::consts::PI; KURAMOTO_N];
        let r_aligned_pi = compute_kuramoto_r(&aligned_pi);
        println!("  All phases PI: r = {:.4}", r_aligned_pi);
        assert!(
            (r_aligned_pi - 1.0).abs() < 0.01,
            "Aligned phases at PI should give r≈1.0"
        );

        // Desynchronized (evenly distributed around circle)
        let mut distributed: [f64; KURAMOTO_N] = [0.0; KURAMOTO_N];
        for i in 0..KURAMOTO_N {
            distributed[i] = (i as f64) * std::f64::consts::TAU / (KURAMOTO_N as f64);
        }
        let r_distributed = compute_kuramoto_r(&distributed);
        println!("  Evenly distributed: r = {:.4}", r_distributed);
        assert!(
            r_distributed < 0.1,
            "Evenly distributed phases should give r≈0"
        );

        println!("RESULT: PASS - Kuramoto r computed correctly");
    }

    // =========================================================================
    // TC-SESSION-12-07: Output Format Verification
    // =========================================================================
    #[test]
    fn tc_session_12_07_output_format() {
        println!("\n=== TC-SESSION-12-07: Output Format Verification ===");

        // Create a snapshot for output testing
        let mut snapshot = SessionIdentitySnapshot::new("test-output-format");
        snapshot.consciousness = 0.75; // EMG state
        snapshot.kuramoto_phases = [0.0; KURAMOTO_N]; // r ≈ 1.0

        let ic = 0.85; // Good status

        // Test JSON serialization
        let response = RestoreResponse {
            session_id: snapshot.session_id.clone(),
            ic,
            status: classify_ic(ic),
            is_crisis: is_ic_crisis(ic),
            consciousness: snapshot.consciousness,
            kuramoto_r: compute_kuramoto_r(&snapshot.kuramoto_phases),
            source: "startup".to_string(),
            error: None,
        };

        let json = serde_json::to_string_pretty(&response).expect("Serialization must succeed");
        println!("JSON output:\n{}", json);

        // Verify JSON contains expected fields
        assert!(json.contains("\"session_id\""), "JSON must have session_id");
        assert!(json.contains("\"ic\""), "JSON must have ic");
        assert!(json.contains("\"status\""), "JSON must have status");
        assert!(json.contains("\"is_crisis\""), "JSON must have is_crisis");
        assert!(json.contains("\"kuramoto_r\""), "JSON must have kuramoto_r");

        println!("RESULT: PASS - Output format verified");
    }

    // =========================================================================
    // TC-SESSION-12-08: Corruption Error Detection
    // =========================================================================
    #[test]
    fn tc_session_12_08_corruption_detection() {
        println!("\n=== TC-SESSION-12-08: Corruption Error Detection ===");

        let test_cases = [
            ("data corruption detected", true),
            ("checksum mismatch", true),
            ("invalid record format", true),
            ("malformed entry", true),
            ("truncated file", true),
            ("connection refused", false),
            ("timeout error", false),
            ("file not found", false),
        ];

        for (msg, expected_corruption) in test_cases {
            let result = is_corruption_error(msg);
            println!("  '{}': corruption={}", msg, result);
            assert_eq!(
                result, expected_corruption,
                "Message '{}' should be corruption={}",
                msg, expected_corruption
            );
        }

        println!("RESULT: PASS - Corruption detection works correctly");
    }

    // =========================================================================
    // TC-SESSION-12-09: Sync Description
    // =========================================================================
    #[test]
    fn tc_session_12_09_sync_description() {
        println!("\n=== TC-SESSION-12-09: Sync Description ===");

        let test_cases = [
            (0.95, "Excellent synchronization"),
            (0.90, "Excellent synchronization"),
            (0.85, "Good synchronization"),
            (0.70, "Good synchronization"),
            (0.65, "Moderate synchronization"),
            (0.50, "Moderate synchronization"),
            (0.45, "Low synchronization"),
            (0.10, "Low synchronization"),
        ];

        for (r, expected) in test_cases {
            let desc = sync_description(r);
            println!("  r={:.2}: '{}'", r, desc);
            assert_eq!(desc, expected, "r={} should be '{}'", r, expected);
        }

        println!("RESULT: PASS - Sync descriptions correct");
    }
}
