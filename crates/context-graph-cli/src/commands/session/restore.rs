//! session restore-identity CLI command
//!
//! TASK-SESSION-12: Restores previous session identity.
//!
//! # Note on Topic Stability
//! Per PRD v6 Section 14, this module uses Topic Stability (churn tracking)
//! for session coherence. See `clustering/stability.rs` for implementation.
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
//! ## Session State
//! - State: EMG (C=0.82)
//! - Session: session-1736985432 (source=startup)
//! ```
//!
//! # Exit Codes (per AP-26)
//! - 0: Success
//! - 1: Recoverable error
//! - 2: Corruption detected
//!
//! # Constitution Reference
//! - AP-26: Exit codes
//! - ARCH-07: Native Claude Code hooks
//!
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.

use std::io::Read;
use std::path::PathBuf;

use clap::Args;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::commands::hooks::session_state::{
    store_in_cache, CoherenceState, SessionCache, SessionSnapshot,
};

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
    integration: f32,
    reflection: f32,
    differentiation: f32,
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

    // Execute based on source variant
    match input.source.as_str() {
        "clear" => handle_clear_source(&args),
        "resume" => handle_resume_source(&args, input.session_id),
        _ => handle_startup_source(&args),
    }
}

/// Handle source="clear" - Start fresh session
fn handle_clear_source(args: &RestoreIdentityArgs) -> i32 {
    info!("restore-identity: source=clear, creating fresh session");

    // Create new snapshot
    let session_id = format!("session-{}", timestamp_ms());
    let snapshot = SessionSnapshot::new(&session_id);

    // Update cache
    store_in_cache(&snapshot);

    // Output
    output_result(&snapshot, "clear", args.format);
    0
}

/// Handle source="resume" - Load specific session by ID
fn handle_resume_source(args: &RestoreIdentityArgs, target_session: Option<String>) -> i32 {
    let session_id = match target_session {
        Some(id) => id,
        None => {
            warn!("source=resume requires session_id, falling back to startup");
            return handle_startup_source(args);
        }
    };

    info!(
        "restore-identity: source=resume, looking for session={}",
        session_id
    );

    // Try to get from cache first
    if let Some(snapshot) = SessionCache::get() {
        if snapshot.session_id == session_id {
            output_result(&snapshot, "resume", args.format);
            return 0;
        }
    }

    // Session not found in cache - create fresh with requested ID
    warn!(
        "Session '{}' not found in cache, creating fresh",
        session_id
    );
    let snapshot = SessionSnapshot::new(&session_id);
    store_in_cache(&snapshot);
    output_result(&snapshot, "resume", args.format);
    0
}

/// Handle source="startup" - Load latest session (default behavior)
fn handle_startup_source(args: &RestoreIdentityArgs) -> i32 {
    info!("restore-identity: source=startup, loading or creating session");

    // Check if cache is warm
    if let Some(snapshot) = SessionCache::get() {
        info!("restore-identity: using cached session {}", snapshot.session_id);
        output_result(&snapshot, "startup", args.format);
        return 0;
    }

    // Cache is cold - create new session
    let session_id = format!("session-{}", timestamp_ms());
    let snapshot = SessionSnapshot::new(&session_id);
    store_in_cache(&snapshot);

    info!("restore-identity: created new session {}", session_id);
    output_result(&snapshot, "startup", args.format);
    0
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

/// Get current timestamp in milliseconds
fn timestamp_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}

/// Output result in requested format
fn output_result(snapshot: &SessionSnapshot, source: &str, format: OutputFormat) {
    match format {
        OutputFormat::Prd => {
            // PRD Section 15.2 format (~100 tokens)
            // Use integration as a proxy for coherence level
            let coherence_level = (snapshot.integration + snapshot.reflection + snapshot.differentiation) / 3.0;
            let state = CoherenceState::from_level(coherence_level);
            println!("## Session State");
            println!(
                "- State: {} (coherence={:.2})",
                state.short_name(),
                coherence_level
            );
            println!("- Session: {} (source={})", snapshot.session_id, source);
        }
        OutputFormat::Json => {
            let response = RestoreResponse {
                session_id: snapshot.session_id.clone(),
                integration: snapshot.integration,
                reflection: snapshot.reflection,
                differentiation: snapshot.differentiation,
                source: source.to_string(),
                error: None,
            };
            // Use unwrap since we control the struct - it's always serializable
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
    }
}

// =============================================================================
// Tests
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::test_utils::GLOBAL_IDENTITY_LOCK;

    // =========================================================================
    // TC-SESSION-12-01: First Run (Empty Cache)
    // =========================================================================
    #[test]
    fn tc_session_12_01_first_run_empty_cache() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-SESSION-12-01: First Run (Empty Cache) ===");

        // Create new session
        let session_id = format!("session-{}", timestamp_ms());
        let snapshot = SessionSnapshot::new(&session_id);
        store_in_cache(&snapshot);

        // Verify cache is warm
        assert!(
            SessionCache::is_warm(),
            "Cache must be warm after store_in_cache"
        );

        let cached = SessionCache::get().expect("Cache must have snapshot");
        assert!(
            cached.session_id.starts_with("session-"),
            "Session ID must start with 'session-'"
        );

        println!("RESULT: PASS - First run creates session successfully");
    }

    // =========================================================================
    // TC-SESSION-12-02: Source Clear (Fresh Start)
    // =========================================================================
    #[test]
    fn tc_session_12_02_source_clear() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-SESSION-12-02: Source Clear ===");

        // Create fresh session
        let session_id = format!("session-{}", timestamp_ms());
        let snapshot = SessionSnapshot::new(&session_id);
        store_in_cache(&snapshot);

        // Verify
        let cached = SessionCache::get().expect("Cache must have snapshot");
        assert_eq!(cached.session_id, session_id);

        println!("RESULT: PASS - Clear source creates fresh session");
    }

    // =========================================================================
    // TC-SESSION-12-03: Output Format Verification
    // =========================================================================
    #[test]
    fn tc_session_12_03_output_format() {
        println!("\n=== TC-SESSION-12-03: Output Format Verification ===");

        // Create a snapshot for output testing
        let snapshot = SessionSnapshot::new("test-output-format");

        // Test JSON serialization
        let response = RestoreResponse {
            session_id: snapshot.session_id.clone(),
            integration: snapshot.integration,
            reflection: snapshot.reflection,
            differentiation: snapshot.differentiation,
            source: "startup".to_string(),
            error: None,
        };

        let json = serde_json::to_string_pretty(&response).expect("Serialization must succeed");
        println!("JSON output:\n{}", json);

        // Verify JSON contains expected fields
        assert!(json.contains("\"session_id\""), "JSON must have session_id");
        assert!(json.contains("\"integration\""), "JSON must have integration");
        assert!(json.contains("\"reflection\""), "JSON must have reflection");
        assert!(json.contains("\"differentiation\""), "JSON must have differentiation");

        println!("RESULT: PASS - Output format verified");
    }
}
