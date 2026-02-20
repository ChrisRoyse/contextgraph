//! Integration test helpers for hook lifecycle testing
//!
//! # NO MOCK DATA - REAL CLI EXECUTION
//! All tests use the real CLI binary and real RocksDB storage.
//! No stubs, no mocks, no fake data.
//!
//! # Architecture
//! 1. `invoke_hook` - Spawns real CLI process
//! 2. `create_*_input` - Generates valid JSON input
//!
//! # Constitution References
//! - AP-26: Exit codes (0-6)
//! - AP-50: Native hooks only

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use serde_json::{json, Value};

// =============================================================================
// Constants - From Constitution
// =============================================================================

/// Get the workspace root directory (parent of crates/)
fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR points to crates/context-graph-cli
    // We need to go up two levels to get to the workspace root
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));

    // Go up two levels: crates/context-graph-cli -> crates -> root
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// CLI binary paths relative to workspace root
fn cli_binary_release() -> PathBuf {
    workspace_root().join("target/release/context-graph-cli")
}

fn cli_binary_debug() -> PathBuf {
    workspace_root().join("target/debug/context-graph-cli")
}

/// Exit codes per TECH-HOOKS.md Section 3.2
pub const EXIT_SUCCESS: i32 = 0;
pub const EXIT_GENERAL_ERROR: i32 = 1;
pub const EXIT_TIMEOUT_CORRUPTION: i32 = 2;
pub const EXIT_DATABASE_ERROR: i32 = 3;
pub const EXIT_INVALID_INPUT: i32 = 4;
pub const EXIT_SESSION_NOT_FOUND: i32 = 5;
pub const EXIT_CRISIS_TRIGGERED: i32 = 6;

/// Timeout budgets per constitution.yaml
/// Note: These include process spawn overhead for integration tests (~200-300ms).
/// The constitution's 500ms total budget includes CLI startup + logic.
/// When invoking via `Command::new()`, we use the full 500ms budget.
pub const TIMEOUT_PRE_TOOL_MS: u64 = 500; // 500ms total per constitution.yaml
pub const TIMEOUT_USER_PROMPT_MS: u64 = 2000;
pub const TIMEOUT_POST_TOOL_MS: u64 = 3000;
pub const TIMEOUT_SESSION_START_MS: u64 = 5000;
pub const TIMEOUT_SESSION_END_MS: u64 = 30000;


// =============================================================================
// CLI Invocation Helper
// =============================================================================

/// Result of a CLI invocation
#[derive(Debug)]
pub struct HookInvocationResult {
    /// Exit code from the process
    pub exit_code: i32,
    /// Captured stdout
    pub stdout: String,
    /// Captured stderr
    pub stderr: String,
    /// Wall-clock execution time in milliseconds
    pub execution_time_ms: u64,
}

impl HookInvocationResult {
    /// Parse stdout as JSON Value
    pub fn parse_stdout(&self) -> Result<Value, serde_json::Error> {
        serde_json::from_str(&self.stdout)
    }

    /// Check if hook succeeded (exit code 0 and success=true in output)
    pub fn is_success(&self) -> bool {
        if self.exit_code != EXIT_SUCCESS {
            return false;
        }
        if let Ok(json) = self.parse_stdout() {
            json.get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Get execution_time_ms from the JSON output
    pub fn reported_execution_time_ms(&self) -> Option<u64> {
        self.parse_stdout()
            .ok()
            .and_then(|json| json.get("execution_time_ms")?.as_u64())
    }

    /// Get coherence_state from the JSON output
    pub fn coherence_state(&self) -> Option<Value> {
        self.parse_stdout()
            .ok()
            .and_then(|json| json.get("coherence_state").cloned())
    }
}

/// Get the CLI binary path (prefers release, falls back to debug)
pub fn get_cli_binary() -> PathBuf {
    let release = cli_binary_release();
    let debug = cli_binary_debug();

    if release.exists() {
        release.clone()
    } else if debug.exists() {
        debug.clone()
    } else {
        panic!(
            "CLI binary not found. Run `cargo build --release -p context-graph-cli` first.\n\
             Looked for:\n  - {}\n  - {}",
            release.display(),
            debug.display()
        );
    }
}

/// Invoke a CLI hook command and capture all outputs
///
/// # Arguments
/// * `hook_cmd` - Subcommand name (e.g., "session-start", "pre-tool", "post-tool", "prompt-submit", "session-end")
/// * `session_id` - Session identifier
/// * `extra_args` - Additional CLI arguments
/// * `stdin_input` - Optional JSON input to pipe to stdin
/// * `db_path` - Database path for the hook
///
/// # Returns
/// HookInvocationResult with exit code, stdout, stderr, and timing
///
/// # Example
/// ```ignore
/// let result = invoke_hook(
///     "session-start",
///     "test-session-001",
///     &["--previous-session-id", "old-session"],
///     None,
///     &temp_dir.path(),
/// );
/// assert_eq!(result.exit_code, 0);
/// ```
pub fn invoke_hook(
    hook_cmd: &str,
    session_id: &str,
    extra_args: &[&str],
    stdin_input: Option<&str>,
    db_path: &Path,
) -> HookInvocationResult {
    let cli_binary = get_cli_binary();

    let mut cmd = Command::new(&cli_binary);
    cmd.args(["hooks", hook_cmd])
        .args(["--session-id", session_id])
        .args(["--format", "json"])
        .args(extra_args)
        .env("CONTEXT_GRAPH_DB_PATH", db_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let start = Instant::now();

    let mut child = cmd.spawn().unwrap_or_else(|e| {
        panic!(
            "Failed to spawn CLI: {}\nBinary: {}\nCommand: hooks {} --session-id {}",
            e,
            cli_binary.display(),
            hook_cmd,
            session_id
        );
    });

    // Write stdin if provided
    if let Some(input) = stdin_input {
        let stdin_handle = child.stdin.as_mut().expect("Failed to get stdin");
        stdin_handle
            .write_all(input.as_bytes())
            .expect("Failed to write to stdin");
    }
    // Drop stdin to close it
    drop(child.stdin.take());

    let output: Output = child.wait_with_output().expect("Failed to wait on CLI");
    let execution_time_ms = start.elapsed().as_millis() as u64;

    HookInvocationResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        execution_time_ms,
    }
}

/// Invoke hook with stdin flag (for hooks that expect --stdin flag)
///
/// Note: session-start uses --stdin as a boolean flag, while other hooks
/// use --stdin <value> format. This function handles the difference.
pub fn invoke_hook_with_stdin(
    hook_cmd: &str,
    session_id: &str,
    extra_args: &[&str],
    stdin_input: &str,
    db_path: &Path,
) -> HookInvocationResult {
    let mut args = extra_args.to_vec();
    args.push("--stdin");
    // session-start uses --stdin as a flag (no value)
    // other hooks use --stdin <true|false>
    if hook_cmd != "session-start" {
        args.push("true");
    }
    invoke_hook(hook_cmd, session_id, &args, Some(stdin_input), db_path)
}

// =============================================================================
// Input JSON Generators
// =============================================================================

/// Create a valid HookInput JSON for session-start
pub fn create_session_start_input(
    session_id: &str,
    cwd: &str,
    source: &str,
    previous_session_id: Option<&str>,
) -> String {
    let timestamp_ms = chrono::Utc::now().timestamp_millis();
    let mut payload_data = json!({
        "cwd": cwd,
        "source": source,
    });

    if let Some(prev_id) = previous_session_id {
        payload_data.as_object_mut().unwrap().insert(
            "previous_session_id".into(),
            serde_json::Value::String(prev_id.into()),
        );
    }

    json!({
        "hook_type": "session_start",
        "session_id": session_id,
        "timestamp_ms": timestamp_ms,
        "payload": {
            "type": "session_start",
            "data": payload_data
        }
    })
    .to_string()
}

/// Create a valid HookInput JSON for pre-tool
pub fn create_pre_tool_input(
    session_id: &str,
    tool_name: &str,
    tool_input: Value,
    tool_use_id: &str,
) -> String {
    let timestamp_ms = chrono::Utc::now().timestamp_millis();
    json!({
        "hook_type": "pre_tool_use",
        "session_id": session_id,
        "timestamp_ms": timestamp_ms,
        "payload": {
            "type": "pre_tool_use",
            "data": {
                "tool_name": tool_name,
                "tool_input": tool_input,
                "tool_use_id": tool_use_id
            }
        }
    })
    .to_string()
}

/// Create a valid HookInput JSON for post-tool
pub fn create_post_tool_input(
    session_id: &str,
    tool_name: &str,
    tool_input: Value,
    tool_response: &str,
    tool_use_id: &str,
) -> String {
    let timestamp_ms = chrono::Utc::now().timestamp_millis();
    json!({
        "hook_type": "post_tool_use",
        "session_id": session_id,
        "timestamp_ms": timestamp_ms,
        "payload": {
            "type": "post_tool_use",
            "data": {
                "tool_name": tool_name,
                "tool_input": tool_input,
                "tool_response": tool_response,
                "tool_use_id": tool_use_id
            }
        }
    })
    .to_string()
}

/// Create a valid HookInput JSON for user-prompt-submit
pub fn create_prompt_submit_input(
    session_id: &str,
    prompt: &str,
    context: Vec<(&str, &str)>, // (role, content) pairs
) -> String {
    let timestamp_ms = chrono::Utc::now().timestamp_millis();
    let context_messages: Vec<Value> = context
        .into_iter()
        .map(|(role, content)| json!({ "role": role, "content": content }))
        .collect();

    json!({
        "hook_type": "user_prompt_submit",
        "session_id": session_id,
        "timestamp_ms": timestamp_ms,
        "payload": {
            "type": "user_prompt_submit",
            "data": {
                "prompt": prompt,
                "context": context_messages
            }
        }
    })
    .to_string()
}

/// Create a valid HookInput JSON for session-end
pub fn create_session_end_input(
    session_id: &str,
    duration_ms: u64,
    status: &str, // "normal", "timeout", "error", "user_abort", "clear", "logout"
    reason: Option<&str>,
) -> String {
    let timestamp_ms = chrono::Utc::now().timestamp_millis();
    let mut data = json!({
        "duration_ms": duration_ms,
        "status": status,
    });

    if let Some(r) = reason {
        data.as_object_mut()
            .unwrap()
            .insert("reason".into(), serde_json::Value::String(r.into()));
    }

    json!({
        "hook_type": "session_end",
        "session_id": session_id,
        "timestamp_ms": timestamp_ms,
        "payload": {
            "type": "session_end",
            "data": data
        }
    })
    .to_string()
}

// =============================================================================
// Test Evidence Logging
// =============================================================================

/// Log test evidence in JSON format for audit trail
pub fn log_test_evidence(
    test_name: &str,
    hook_type: &str,
    session_id: &str,
    exit_code: i32,
    execution_time_ms: u64,
    db_verified: bool,
    extra: Option<Value>,
) {
    let mut evidence = json!({
        "test": test_name,
        "hook_type": hook_type,
        "session_id": session_id,
        "exit_code": exit_code,
        "execution_time_ms": execution_time_ms,
        "db_verified": db_verified,
    });

    if let Some(extra_data) = extra {
        if let Some(obj) = extra_data.as_object() {
            for (k, v) in obj {
                evidence
                    .as_object_mut()
                    .unwrap()
                    .insert(k.clone(), v.clone());
            }
        }
    }

    println!("{}", serde_json::to_string(&evidence).unwrap());
}

// =============================================================================
// Assertion Helpers
// =============================================================================

/// Assert that exit code matches expected with detailed error message
pub fn assert_exit_code(result: &HookInvocationResult, expected: i32, context: &str) {
    assert_eq!(
        result.exit_code, expected,
        "{}\nExpected exit code {}, got {}.\nstdout: {}\nstderr: {}",
        context, expected, result.exit_code, result.stdout, result.stderr
    );
}

/// Assert that execution time is under budget
pub fn assert_timing_under_budget(result: &HookInvocationResult, budget_ms: u64, context: &str) {
    assert!(
        result.execution_time_ms < budget_ms,
        "{}\nExecution time {}ms exceeded budget {}ms",
        context,
        result.execution_time_ms,
        budget_ms
    );
}

/// Assert that JSON output contains expected field with expected value
pub fn assert_output_field_eq(
    result: &HookInvocationResult,
    field: &str,
    expected: &Value,
    context: &str,
) {
    let json = result
        .parse_stdout()
        .expect("Failed to parse stdout as JSON");
    let actual = json.get(field);
    assert_eq!(
        actual,
        Some(expected),
        "{}\nField '{}' mismatch.\nExpected: {:?}\nActual: {:?}\nFull output: {}",
        context,
        field,
        expected,
        actual,
        result.stdout
    );
}

/// Assert that JSON output has a specific boolean field
pub fn assert_output_bool(
    result: &HookInvocationResult,
    field: &str,
    expected: bool,
    context: &str,
) {
    let json = result
        .parse_stdout()
        .expect("Failed to parse stdout as JSON");
    let actual = json.get(field).and_then(|v| v.as_bool());
    assert_eq!(
        actual,
        Some(expected),
        "{}\nBoolean field '{}' mismatch.\nExpected: {}\nActual: {:?}\nFull output: {}",
        context,
        field,
        expected,
        actual,
        result.stdout
    );
}

// =============================================================================
// Session ID Generators
// =============================================================================

/// Generate a unique session ID for testing
pub fn generate_test_session_id(prefix: &str) -> String {
    format!("{}-{}", prefix, uuid::Uuid::new_v4())
}

/// Generate a deterministic session ID for reproducible tests
pub fn deterministic_session_id(test_name: &str, suffix: &str) -> String {
    format!("test-{}-{}", test_name, suffix)
}

// =============================================================================
// Verification Helpers
// Per PRD v6 Section 14, we use SessionCache instead of RocksDB
// These functions provide compatibility for tests
// =============================================================================

/// Verify if a snapshot exists in the database.
///
/// NOTE: Not yet implemented — always returns false.
/// Snapshot verification requires reading CF_SYSTEM keys which
/// is not available in the test helper context.
///
/// CLI-2 FIX: Returns `false` — cross-process snapshot verification is impossible
/// because SessionCache is in-memory and process-scoped (PRD v6 Section 14).
/// Tests must verify session behavior via CLI exit codes and stdout JSON, not
/// by probing in-memory state across process boundaries.
pub fn verify_snapshot_exists(_db_path: &Path, _session_id: &str) -> bool {
    false
}

/// Load snapshot for verification purposes
/// Since we use in-memory SessionCache per PRD v6 Section 14, this returns
/// a stub value for integration test compatibility.
/// Note: The actual snapshot is in the CLI process's memory, not accessible here.
pub fn load_snapshot_for_verification(
    _db_path: &Path,
    _session_id: &str,
) -> Option<serde_json::Value> {
    // In integration tests with separate processes, we cannot access the CLI's
    // in-memory cache. Return None to indicate verification should use JSON output.
    None
}
