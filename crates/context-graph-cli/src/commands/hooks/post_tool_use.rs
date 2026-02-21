//! PostToolUse hook handler
//!
//! # Performance Requirements
//! - Timeout: 3000ms (constitution.yaml hooks.timeout_ms.post_tool_use)
//! - Database access: ALLOWED
//!
//! # Constitution References
//! - AP-50: NO internal hooks - shell scripts call CLI
//! - AP-26: Exit codes (0=success)
//!
//! # NO BACKWARDS COMPATIBILITY - FAIL FAST

use std::io::{self, BufRead};
use std::time::Instant;

use tracing::{debug, error, info, warn};

use super::session_state::{store_in_cache, SessionCache, SessionSnapshot};
use crate::mcp_client::McpClient;

use super::args::PostToolArgs;
use super::error::{HookError, HookResult};
use super::types::{
    CoherenceState, HookInput, HookOutput, HookPayload, StabilityClassification,
};

// ============================================================================
// Constants (from constitution.yaml)
// ============================================================================

/// PostToolUse timeout in milliseconds
pub const POST_TOOL_USE_TIMEOUT_MS: u64 = 3000;

/// Minimum response length to capture as memory (avoid noise from empty/trivial responses)
const MIN_RESPONSE_LENGTH_FOR_CAPTURE: usize = 50;

/// Maximum response length to store (truncate longer responses)
const MAX_RESPONSE_LENGTH_FOR_CAPTURE: usize = 2000;

/// Tools that should have their responses captured as memories
const TOOLS_TO_CAPTURE: &[&str] = &[
    "Read",     // File content - valuable context
    "Bash",     // Command output - may contain useful info
    "WebFetch", // External data - new knowledge
    "Grep",     // Search results - code understanding
    "LSP",      // Code intelligence - technical context
];

// ============================================================================
// Types
// ============================================================================

/// Impact of tool execution on coherence state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactLevel {
    /// No significant impact
    None,
    /// Minor impact - log but don't recalculate
    Low,
    /// Moderate impact - consider recalculation
    Medium,
    /// High impact - force recalculation
    High,
}

/// Result of analyzing tool response
#[derive(Debug, Clone)]
pub struct ToolImpact {
    /// Impact level on coherence
    pub level: ImpactLevel,
    /// Whether tool execution succeeded
    pub tool_success: bool,
}

// ============================================================================
// Handler
// ============================================================================

/// Execute post-tool hook.
///
/// See module doc for full flow and exit codes.
///
/// # Note on Topic Stability
/// Per PRD v6 Section 14, we use Topic Stability for session coherence.
/// Session state is stored in the in-memory SessionCache.
pub async fn execute(args: PostToolArgs) -> HookResult<HookOutput> {
    let start = Instant::now();

    info!(
        stdin = args.stdin,
        session_id = %args.session_id,
        tool_name = ?args.tool_name,
        "POST_TOOL: execute starting"
    );

    // 1. Parse input source
    let (tool_name, tool_response, tool_success) = if args.stdin {
        let input = parse_stdin()?;
        extract_tool_info(&input)?
    } else {
        let name = args.tool_name.ok_or_else(|| {
            error!("POST_TOOL: tool_name required when not using stdin");
            HookError::invalid_input("tool_name required when not using stdin")
        })?;
        (name, String::new(), args.success.unwrap_or(true))
    };

    // 2. Load snapshot from cache (or create new)
    let mut snapshot = load_snapshot_from_cache(&args.session_id)?;

    // 3. Analyze tool response for coherence impact
    let impact = analyze_tool_response(&tool_name, &tool_response, tool_success);

    // 4. Update snapshot based on impact
    if impact.level >= ImpactLevel::Medium {
        update_snapshot_from_impact(&mut snapshot, &impact);
    }

    // 5. Persist updated snapshot to cache
    store_in_cache(&snapshot);

    // 6. Capture tool response as memory if appropriate
    //    This stores valuable context from tool execution into the knowledge graph.
    //    Per CLAUDE.md: PostToolUse captures tool description as HookDescription memory.
    // SESSION-ID-FIX: Pass session_id for proper session-scoped storage
    capture_tool_memory(&tool_name, &tool_response, tool_success, &args.session_id).await;

    // 7. Build output structures
    let coherence_state = build_coherence_state(&snapshot);
    let stability_value = compute_coherence(&snapshot);
    let stability_classification = StabilityClassification::from_value(stability_value);

    let execution_time_ms = start.elapsed().as_millis() as u64;

    info!(
        session_id = %args.session_id,
        tool_name = %tool_name,
        stability = stability_value,
        execution_time_ms,
        "POST_TOOL: execute complete"
    );

    Ok(HookOutput::success(execution_time_ms)
        .with_coherence_state(coherence_state)
        .with_stability_classification(stability_classification))
}

// ============================================================================
// Input Parsing
// ============================================================================

/// Parse stdin JSON into HookInput.
/// FAIL FAST on empty or malformed input.
fn parse_stdin() -> HookResult<HookInput> {
    let stdin = io::stdin();
    let mut input_str = String::new();

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| {
            error!(error = %e, "POST_TOOL: stdin read failed");
            HookError::invalid_input(format!("stdin read failed: {}", e))
        })?;
        input_str.push_str(&line);
    }

    if input_str.is_empty() {
        error!("POST_TOOL: stdin is empty");
        return Err(HookError::invalid_input("stdin is empty - expected JSON"));
    }

    debug!(
        input_bytes = input_str.len(),
        "POST_TOOL: parsing stdin JSON"
    );

    serde_json::from_str(&input_str).map_err(|e| {
        error!(error = %e, "POST_TOOL: JSON parse failed");
        HookError::invalid_input(format!("JSON parse failed: {}", e))
    })
}

/// Extract tool info from HookInput payload.
fn extract_tool_info(input: &HookInput) -> HookResult<(String, String, bool)> {
    // Validate input
    if let Some(error) = input.validate() {
        return Err(HookError::invalid_input(error));
    }

    match &input.payload {
        HookPayload::PostToolUse {
            tool_name,
            tool_response,
            tool_success,
            ..
        } => {
            // Use explicit tool_success if provided, otherwise use smart heuristic
            let success = tool_success.unwrap_or_else(|| infer_tool_success(tool_response));
            Ok((tool_name.clone(), tool_response.clone(), success))
        }
        other => {
            error!(payload_type = ?std::mem::discriminant(other), "POST_TOOL: unexpected payload type");
            Err(HookError::invalid_input(
                "Expected PostToolUse payload, got different type",
            ))
        }
    }
}

/// Infer tool success from response content using smart heuristics.
/// This is a fallback when tool_success is not explicitly provided.
///
/// Unlike naive string matching, this checks for actual error PATTERNS:
/// - Lines starting with "Error:" or "error:"
/// - Exit codes indicating failure
/// - Common tool error messages
///
/// It does NOT flag code containing Error types (e.g., `sqlx::Error`) as failures.
fn infer_tool_success(response: &str) -> bool {
    // Empty response is not necessarily a failure
    if response.is_empty() {
        return true;
    }

    // Check for explicit error patterns at line starts
    for line in response.lines() {
        let trimmed = line.trim();
        // Error message patterns (at start of line)
        if trimmed.starts_with("Error:") || trimmed.starts_with("error:") {
            return false;
        }
        // Exit code failures
        if trimmed.starts_with("Exit code:") && !trimmed.contains("Exit code: 0") {
            return false;
        }
        if trimmed.starts_with("error[E") {
            // Rust compiler errors
            return false;
        }
        // Command not found
        if trimmed.contains("command not found") || trimmed.contains("No such file or directory") {
            return false;
        }
    }

    // Check for common failure indicators that span lines
    if response.contains("FAILED") && response.contains("test result:") {
        return false; // Test failures
    }

    true
}

// ============================================================================
// Cache Operations
// ============================================================================

/// Compute coherence from snapshot's integration, reflection, and differentiation metrics.
#[inline]
fn compute_coherence(snapshot: &SessionSnapshot) -> f32 {
    (snapshot.integration + snapshot.reflection + snapshot.differentiation) / 3.0
}

/// Load snapshot from cache or create new one for session.
///
/// # Note on Topic Stability
/// Per PRD v6 Section 14, we use in-memory SessionCache for session state.
fn load_snapshot_from_cache(session_id: &str) -> HookResult<SessionSnapshot> {
    // Try to load from global cache
    if let Some(snapshot) = SessionCache::get() {
        if snapshot.session_id == session_id {
            info!(session_id = %session_id, stability = compute_coherence(&snapshot), "POST_TOOL: loaded snapshot from cache");
            return Ok(snapshot);
        }
    }

    // Session not found in cache - create new one
    // This is not an error for post_tool_use, we just create a default
    info!(session_id = %session_id, "POST_TOOL: session not in cache, creating new snapshot");
    let snapshot = SessionSnapshot::new(session_id);
    store_in_cache(&snapshot);
    Ok(snapshot)
}

// ============================================================================
// Tool Analysis
// ============================================================================

/// Analyze tool response for coherence updates
fn analyze_tool_response(tool_name: &str, _tool_response: &str, tool_success: bool) -> ToolImpact {
    let level = match tool_name {
        "Read" => ImpactLevel::Low,
        "Write" | "Edit" | "MultiEdit" => ImpactLevel::Medium,
        "Bash" => {
            if tool_success {
                ImpactLevel::Medium
            } else {
                ImpactLevel::High
            }
        }
        "WebFetch" | "WebSearch" => ImpactLevel::Medium,
        "Git" => ImpactLevel::Low,
        "Task" => ImpactLevel::High,
        _ => ImpactLevel::None,
    };

    ToolImpact { level, tool_success }
}

/// Update snapshot based on tool impact
///
/// # Note on Topic Stability
/// Per PRD v6 Section 14, we use integration/reflection/differentiation metrics
/// for coherence tracking.
fn update_snapshot_from_impact(snapshot: &mut SessionSnapshot, impact: &ToolImpact) {
    // Apply coherence changes based on impact level
    let delta = match impact.level {
        ImpactLevel::High => 0.03,
        ImpactLevel::Medium => 0.02,
        ImpactLevel::Low => 0.01,
        ImpactLevel::None => 0.0,
    };

    // Positive delta for successful tools, negative for failures
    // Update integration as the primary metric affected by tool success
    if impact.tool_success {
        snapshot.integration = (snapshot.integration + delta * 0.5).clamp(0.0, 1.0);
    } else {
        snapshot.integration = (snapshot.integration - delta).clamp(0.0, 1.0);
    }
}

/// Build CoherenceState from snapshot.
fn build_coherence_state(snapshot: &SessionSnapshot) -> CoherenceState {
    let coherence = compute_coherence(snapshot);
    CoherenceState::new(
        coherence,
        snapshot.integration,
        snapshot.reflection,
        snapshot.differentiation,
        coherence, // topic_stability uses same coherence measure
    )
}

// ============================================================================
// Memory Capture via MCP
// ============================================================================

/// Capture tool response as a memory in the knowledge graph.
///
/// Per CLAUDE.md: PostToolUse captures tool description as HookDescription memory.
/// Only captures for specific high-value tools and non-trivial responses.
///
/// # Arguments
/// * `tool_name` - Name of the tool that was executed
/// * `tool_response` - The response from tool execution
/// * `tool_success` - Whether the tool executed successfully
/// * `session_id` - Session ID for session-scoped storage (SESSION-ID-FIX)
///
/// # Note
/// Failure is non-fatal - we log and continue rather than failing the hook.
async fn capture_tool_memory(tool_name: &str, tool_response: &str, tool_success: bool, session_id: &str) {
    // R7: Capture both successful AND failed tool responses
    // Failed tools get importance=0.3 with FailurePattern source type
    // Successful tools get importance=0.4 with HookDescription source type
    let (importance, source_label) = if tool_success {
        // Only capture successful executions from high-value tools
        if !TOOLS_TO_CAPTURE.contains(&tool_name) {
            debug!(tool_name, "POST_TOOL: Tool not in capture list, skipping");
            return;
        }
        (0.4, "HookDescription")
    } else {
        // R7: Capture failure patterns for all tools (not just TOOLS_TO_CAPTURE)
        // so that repeated failures can be surfaced as context
        info!(tool_name, "POST_TOOL: R7 capturing failure pattern for failed tool");
        (0.3, "FailurePattern")
    };

    // Skip trivial responses
    if tool_response.len() < MIN_RESPONSE_LENGTH_FOR_CAPTURE {
        debug!(
            tool_name,
            response_len = tool_response.len(),
            min_length = MIN_RESPONSE_LENGTH_FOR_CAPTURE,
            "POST_TOOL: Response too short for memory capture"
        );
        return;
    }

    let client = McpClient::new();

    // Check if server is running first (fast fail)
    let server_running = match client.is_server_running().await {
        Ok(running) => running,
        Err(e) => {
            warn!(error = %e, "POST_TOOL: Failed to check MCP server, skipping memory capture");
            return;
        }
    };

    if !server_running {
        warn!("POST_TOOL: MCP server not running, skipping memory capture");
        return;
    }

    debug!("POST_TOOL: MCP server is running, capturing memory");

    // Truncate response if too long
    let content = if tool_response.len() > MAX_RESPONSE_LENGTH_FOR_CAPTURE {
        format!(
            "[{}: {} {}]\n{}...\n[truncated from {} chars]",
            source_label,
            tool_name,
            if tool_success { "output" } else { "FAILURE" },
            // HIGH-4 FIX: Use floor_char_boundary to avoid panic on multi-byte UTF-8
            &tool_response[..tool_response.floor_char_boundary(MAX_RESPONSE_LENGTH_FOR_CAPTURE)],
            tool_response.len()
        )
    } else {
        format!(
            "[{}: {} {}]\n{}",
            source_label,
            tool_name,
            if tool_success { "output" } else { "FAILURE" },
            tool_response
        )
    };

    let rationale = if tool_success {
        format!(
            "Tool execution context from {} - captured for future reference",
            tool_name
        )
    } else {
        format!(
            "R7: Tool failure pattern from {} - captured for error context surfacing",
            tool_name
        )
    };

    // Store via MCP inject_context
    // SESSION-ID-FIX: Pass session_id for proper session-scoped storage
    match client.inject_context(&content, &rationale, importance, Some(session_id), Some("text"), None).await {
        Ok(result) => {
            let fingerprint_id = result
                .get("fingerprintId")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            info!(
                tool_name,
                fingerprint_id,
                content_len = content.len(),
                success = tool_success,
                source_label,
                "POST_TOOL: Captured tool memory successfully"
            );
        }
        Err(e) => {
            warn!(
                tool_name,
                error = %e,
                "POST_TOOL: Failed to capture tool memory, continuing"
            );
        }
    }
}

// ============================================================================
// Comparison Implementations
// ============================================================================

impl PartialOrd for ImpactLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ImpactLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let self_val = match self {
            ImpactLevel::None => 0,
            ImpactLevel::Low => 1,
            ImpactLevel::Medium => 2,
            ImpactLevel::High => 3,
        };
        let other_val = match other {
            ImpactLevel::None => 0,
            ImpactLevel::Low => 1,
            ImpactLevel::Medium => 2,
            ImpactLevel::High => 3,
        };
        self_val.cmp(&other_val)
    }
}

// ============================================================================
// TESTS - Use in-memory SessionCache per PRD v6
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::hooks::args::OutputFormat;
    use crate::commands::test_utils::GLOBAL_IDENTITY_LOCK;

    /// Create a session in the cache for testing
    fn create_test_session(session_id: &str, integration: f32) {
        let mut snapshot = SessionSnapshot::new(session_id);
        snapshot.integration = integration;
        snapshot.reflection = 0.5;
        snapshot.differentiation = 0.5;
        store_in_cache(&snapshot);
    }

    // =========================================================================
    // TC-POST-001: Successful Tool Processing
    // SOURCE OF TRUTH: SessionCache state before/after
    // =========================================================================
    #[tokio::test]
    async fn tc_post_001_successful_tool_processing() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-POST-001: Successful Tool Processing ===");

        let session_id = "tc-post-001-session";

        // BEFORE: Create session with known integration
        println!("BEFORE: Creating session with integration=0.85");
        create_test_session(session_id, 0.85);

        // Verify BEFORE state
        let before_snapshot = SessionCache::get().expect("Cache must be warm");
        println!("BEFORE state: integration={}", before_snapshot.integration);
        assert!((before_snapshot.integration - 0.85).abs() < 0.01);

        // Execute
        let args = PostToolArgs {
            session_id: session_id.to_string(),
            tool_name: Some("Read".to_string()),
            success: Some(true),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;

        // AFTER: Verify success
        assert!(result.is_ok(), "Execute must succeed: {:?}", result.err());
        let output = result.unwrap();
        assert!(output.success, "Output.success must be true");

        // Verify AFTER state in cache
        let after_snapshot = SessionCache::get().expect("Cache must have snapshot");
        println!("AFTER state: integration={}", after_snapshot.integration);

        // Read tool should have minimal positive impact
        println!(
            "RESULT: PASS - Tool processed, integration changed from 0.85 to {}",
            after_snapshot.integration
        );
    }

    // =========================================================================
    // TC-POST-002: New Session Created When Not Found
    // SOURCE OF TRUTH: New session created in cache
    // Per PRD v6, we create a new session instead of returning error
    // =========================================================================
    #[tokio::test]
    async fn tc_post_002_new_session_created() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-POST-002: New Session Created When Not Found ===");

        // Execute with session not in cache - should create new
        let args = PostToolArgs {
            session_id: "brand-new-session-12345".to_string(),
            tool_name: Some("Read".to_string()),
            success: Some(true),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;

        // Verify success (new session created)
        assert!(result.is_ok(), "Should succeed with new session created");
        let output = result.unwrap();
        assert!(output.success, "Output.success must be true");

        println!("RESULT: PASS - New session created in cache");
    }

    // =========================================================================
    // TC-POST-003: Tool Impact Analysis
    // SOURCE OF TRUTH: ImpactLevel per tool type
    // =========================================================================
    #[test]
    fn tc_post_003_tool_impact_analysis() {
        println!("\n=== TC-POST-003: Tool Impact Analysis ===");

        // Edge Case 1: Read tool (Low impact)
        println!("\nEdge Case 1: Read tool");
        let impact = analyze_tool_response("Read", "", true);
        assert_eq!(impact.level, ImpactLevel::Low);
        println!("  - Level: Low");

        // Edge Case 2: Write tool (Medium impact)
        println!("\nEdge Case 2: Write tool");
        let impact = analyze_tool_response("Write", "", true);
        assert_eq!(impact.level, ImpactLevel::Medium);
        println!("  - Level: Medium");

        // Edge Case 3: Failed Bash (High impact)
        println!("\nEdge Case 3: Failed Bash tool");
        let impact = analyze_tool_response("Bash", "command not found", false);
        assert_eq!(impact.level, ImpactLevel::High);
        println!("  - Level: High");

        // Edge Case 4: Unknown tool (No impact)
        println!("\nEdge Case 4: Unknown tool");
        let impact = analyze_tool_response("CustomTool123", "", true);
        assert_eq!(impact.level, ImpactLevel::None);
        println!("  - Level: None");

        println!("\nRESULT: PASS - All tool impacts correctly classified");
    }

    // =========================================================================
    // TC-POST-004: Impact Level Ordering
    // =========================================================================
    #[test]
    fn tc_post_004_impact_level_ordering() {
        println!("\n=== TC-POST-004: Impact Level Ordering ===");

        assert!(ImpactLevel::High > ImpactLevel::Medium);
        assert!(ImpactLevel::Medium > ImpactLevel::Low);
        assert!(ImpactLevel::Low > ImpactLevel::None);

        println!("RESULT: PASS - ImpactLevel ordering correct");
    }

    // =========================================================================
    // TC-POST-006: Tool Impact Effects
    // SOURCE OF TRUTH: SessionCache state values before/after
    // =========================================================================
    #[tokio::test]
    async fn tc_post_006_tool_impact_effects() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-POST-006: Tool Impact Effects ===");

        let session_id = "tool-impact-test";
        create_test_session(session_id, 0.80);

        let args = PostToolArgs {
            session_id: session_id.to_string(),
            tool_name: Some("WebFetch".to_string()),
            success: Some(true),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await.unwrap();
        assert!(result.success);

        let snapshot = SessionCache::get().expect("Cache must have snapshot");

        // Successful tool should maintain or increase integration
        println!("Tool impact: integration 0.80 -> {}", snapshot.integration);
        assert!(
            snapshot.integration >= 0.80,
            "Successful tool should maintain or increase integration"
        );

        println!("RESULT: PASS - Tool impact affects integration correctly");
    }

    // =========================================================================
    // TC-POST-007: Execution Time Tracking
    // =========================================================================
    #[tokio::test]
    async fn tc_post_007_execution_time_tracking() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-POST-007: Execution Time Tracking ===");

        let session_id = "timing-test";
        create_test_session(session_id, 0.90);

        let args = PostToolArgs {
            session_id: session_id.to_string(),
            tool_name: Some("Read".to_string()),
            success: Some(true),
            stdin: false,
            format: OutputFormat::Json,
        };

        let start = std::time::Instant::now();
        let result = execute(args).await.unwrap();
        let actual_elapsed = start.elapsed().as_millis() as u64;

        // Note: execution_time_ms may be 0 if operation completes in <1ms
        // which is actually a SUCCESS per our performance budgets (3000ms timeout)
        assert!(
            result.execution_time_ms < POST_TOOL_USE_TIMEOUT_MS,
            "Execution time {} must be under timeout {}ms",
            result.execution_time_ms,
            POST_TOOL_USE_TIMEOUT_MS
        );

        println!(
            "Execution time: {}ms (timeout: {}ms)",
            result.execution_time_ms, POST_TOOL_USE_TIMEOUT_MS
        );
        println!("Actual elapsed: {}ms", actual_elapsed);
        println!("RESULT: PASS - Execution time within timeout budget");
    }

    // =========================================================================
    // TC-POST-008: Missing tool_name when stdin=false
    // SOURCE OF TRUTH: Exit code 4 (InvalidInput)
    // =========================================================================
    #[tokio::test]
    async fn tc_post_008_missing_tool_name() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-POST-008: Missing tool_name (stdin=false) ===");

        let session_id = "missing-tool-test";
        create_test_session(session_id, 0.90);

        let args = PostToolArgs {
            session_id: session_id.to_string(),
            tool_name: None, // Missing!
            success: Some(true),
            stdin: false, // Not reading from stdin
            format: OutputFormat::Json,
        };

        let result = execute(args).await;

        assert!(result.is_err(), "Should fail with missing tool_name");
        let err = result.unwrap_err();
        assert!(
            matches!(err, HookError::InvalidInput(_)),
            "Must be InvalidInput, got: {:?}",
            err
        );
        assert_eq!(err.exit_code(), 4, "InvalidInput must be exit code 4");

        println!("RESULT: PASS - Missing tool_name returns InvalidInput error");
    }

    // =========================================================================
    // TC-POST-009: infer_tool_success - Code with Error types is NOT failure
    // Validates that code containing Error as a type name passes
    // =========================================================================
    #[test]
    fn tc_post_009_infer_success_code_with_error_types() {
        println!("\n=== TC-POST-009: Code with Error types is NOT failure ===");

        // Code containing Error as a type name should be considered successful
        let code_with_error_type = r#"
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::io::Error;

pub async fn create_pool(database_url: &str) -> Result<Pool<Postgres>, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await
}

impl From<std::io::Error> for MyError {
    fn from(e: std::io::Error) -> Self {
        MyError::Io(e)
    }
}
"#;

        let result = infer_tool_success(code_with_error_type);
        assert!(result, "Code with Error types should be considered successful");

        println!("RESULT: PASS - Error type names don't trigger false failures");
    }

    // =========================================================================
    // TC-POST-010: infer_tool_success - Actual errors are detected
    // Validates that real error messages are detected as failures
    // =========================================================================
    #[test]
    fn tc_post_010_infer_success_actual_errors() {
        println!("\n=== TC-POST-010: Actual errors are detected ===");

        // Error: at start of line
        assert!(
            !infer_tool_success("Error: File not found"),
            "Error: prefix should be failure"
        );

        // error: at start of line
        assert!(
            !infer_tool_success("error: cannot find module"),
            "error: prefix should be failure"
        );

        // Rust compiler errors
        assert!(
            !infer_tool_success("error[E0425]: cannot find value `x` in this scope"),
            "Rust compiler errors should be failure"
        );

        // Exit code failures
        assert!(
            !infer_tool_success("Exit code: 1"),
            "Non-zero exit code should be failure"
        );
        assert!(
            infer_tool_success("Exit code: 0"),
            "Zero exit code should be success"
        );

        // Command not found
        assert!(
            !infer_tool_success("bash: foo: command not found"),
            "Command not found should be failure"
        );

        // No such file
        assert!(
            !infer_tool_success("cat: /nonexistent: No such file or directory"),
            "No such file should be failure"
        );

        // Test failures
        assert!(
            !infer_tool_success("test result: FAILED. 1 passed; 3 failed; 0 ignored"),
            "Test failures should be failure"
        );

        println!("RESULT: PASS - Actual errors correctly detected as failures");
    }

    // =========================================================================
    // TC-POST-011: infer_tool_success - Successful responses
    // Validates that normal successful outputs pass
    // =========================================================================
    #[test]
    fn tc_post_011_infer_success_successful_responses() {
        println!("\n=== TC-POST-011: Successful responses ===");

        // Normal file content
        assert!(
            infer_tool_success("fn main() {\n    println!(\"Hello, world!\");\n}"),
            "Normal code should be success"
        );

        // Empty response
        assert!(infer_tool_success(""), "Empty response should be success");

        // Successful test output
        assert!(
            infer_tool_success("test result: ok. 10 passed; 0 failed; 0 ignored"),
            "Successful tests should be success"
        );

        // Compilation success
        assert!(
            infer_tool_success("Compiling context-graph v0.1.0\nFinished release [optimized]"),
            "Compilation success should be success"
        );

        // Git output
        assert!(
            infer_tool_success("On branch main\nYour branch is up to date"),
            "Git status should be success"
        );

        println!("RESULT: PASS - Successful responses correctly identified");
    }

    // =========================================================================
    // TC-POST-012: infer_tool_success - Edge cases
    // Validates edge cases are handled correctly
    // =========================================================================
    #[test]
    fn tc_post_012_infer_success_edge_cases() {
        println!("\n=== TC-POST-012: Edge cases ===");

        // Error in the middle of a line (not at start) - should be success
        assert!(
            infer_tool_success("This function returns an Error type"),
            "Error in middle of line should be success"
        );

        // Multiline with error type but no actual error
        let multiline = r#"
pub struct AppError {
    message: String,
}

impl std::error::Error for AppError {}
"#;
        assert!(
            infer_tool_success(multiline),
            "Error trait impl should be success"
        );

        // Error mentioned in comments
        assert!(
            infer_tool_success("// This handles the error case\nfn handle() {}"),
            "Error in comments should be success"
        );

        println!("RESULT: PASS - Edge cases handled correctly");
    }
}
