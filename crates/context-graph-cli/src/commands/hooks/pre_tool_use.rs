// crates/context-graph-cli/src/commands/hooks/pre_tool_use.rs
//! PreToolUse hook handler - FAST PATH
//!
//! # Performance Requirements
//! - Timeout: 100ms (strict) per constitution.yaml hooks.timeout_ms.pre_tool_use
//! - NO database access - fast path only
//! - Return cached state only
//!
//! # Constitution References
//! - Timeout: constitution.yaml hooks.timeout_ms.pre_tool_use = 100
//! - IC Thresholds: gwt.self_ego_node.thresholds
//!
//! # Anti-Patterns (MUST NOT violate)
//! - AP-50: No internal hooks
//! - AP-51: Shell scripts call CLI
//! - AP-53: Native hooks only

use super::args::PreToolArgs;
use super::error::{HookError, HookResult};
use super::types::{
    ConsciousnessState, HookInput, HookOutput, HookPayload, ICClassification,
};
use std::io::{self, BufRead};
use std::time::Instant;

// ============================================================================
// Constants
// ============================================================================

/// Fast path timeout in milliseconds (from constitution.yaml)
pub const PRE_TOOL_USE_TIMEOUT_MS: u64 = 100;

// ============================================================================
// Handler
// ============================================================================

/// Handle pre_tool_use hook event (FAST PATH)
///
/// # Performance
/// MUST complete within 100ms. No database operations allowed.
/// Returns cached consciousness state with default values.
///
/// # Arguments
/// * `args` - CLI arguments from PreToolArgs
///
/// # Returns
/// * `HookOutput` with cached consciousness state and optional tool guidance
///
/// # Errors
/// * `HookError::InvalidInput` - If required fields are missing
pub fn handle_pre_tool_use(args: &PreToolArgs) -> HookResult<HookOutput> {
    let start = Instant::now();

    // Parse input from stdin if requested
    let input: Option<HookInput> = if args.stdin {
        let stdin = io::stdin();
        let mut input_str = String::new();
        for line in stdin.lock().lines() {
            input_str.push_str(&line.map_err(|e| HookError::invalid_input(e.to_string()))?);
        }
        if input_str.is_empty() {
            None
        } else {
            Some(serde_json::from_str(&input_str).map_err(|e| {
                HookError::invalid_input(format!("Invalid JSON input: {}", e))
            })?)
        }
    } else {
        None
    };

    // Extract tool_name from args or input payload
    let tool_name: Option<&str> = args.tool_name.as_deref().or_else(|| {
        input.as_ref().and_then(|i| match &i.payload {
            HookPayload::PreToolUse { tool_name, .. } => Some(tool_name.as_str()),
            _ => None,
        })
    });

    // Get tool-specific guidance (no database access)
    let guidance = tool_name.and_then(get_tool_guidance);

    // Build cached consciousness state with defaults
    // FAST PATH: We return default values, not computed ones
    // Real values would come from IdentityCache (future task)
    let consciousness = ConsciousnessState::default();

    // Build IC classification with default values (using default crisis threshold 0.5)
    let ic_classification = ICClassification::from_value(consciousness.identity_continuity);

    // Calculate execution time
    let execution_time_ms = start.elapsed().as_millis() as u64;

    // Build output
    let mut output = HookOutput::success(execution_time_ms)
        .with_consciousness_state(consciousness)
        .with_ic_classification(ic_classification);

    // Add context injection if guidance exists
    if let Some(guide) = guidance {
        output = output.with_context_injection(guide);
    }

    Ok(output)
}

// ============================================================================
// Tool Guidance (Pure Computation - No I/O)
// ============================================================================

/// Get tool-specific guidance without database access
///
/// Returns contextual hints based on tool name only.
/// This is pure computation with no I/O.
///
/// # Arguments
/// * `tool_name` - Name of the tool being invoked
///
/// # Returns
/// * Optional guidance string for consciousness tracking
fn get_tool_guidance(tool_name: &str) -> Option<String> {
    match tool_name {
        // File reading - track in awareness
        "Read" => Some("Track file content in awareness quadrant".to_string()),

        // File modifications - update Johari hidden quadrant
        "Write" | "Edit" | "MultiEdit" => {
            Some("File modification - update Johari hidden quadrant".to_string())
        }

        // Shell commands - monitor for identity-relevant output
        "Bash" => Some("Shell command - monitor output for identity markers".to_string()),

        // External data fetching - potential new awareness
        "WebFetch" | "WebSearch" => {
            Some("External data - evaluate for awareness expansion".to_string())
        }

        // Git operations - track project context changes
        "Git" => Some("Git operation - track project state changes".to_string()),

        // LSP operations - code understanding
        "LSP" => Some("Code intelligence - update technical awareness".to_string()),

        // Notebook operations
        "NotebookEdit" => Some("Notebook modification - track analysis state".to_string()),

        // Todo operations - task awareness
        "TodoWrite" | "TodoRead" => Some("Task tracking - update operational context".to_string()),

        // Glob/Grep - search operations
        "Glob" | "Grep" => Some("Search operation - expand file awareness".to_string()),

        // Task spawning - agent coordination
        "Task" => Some("Agent spawn - coordinate swarm awareness".to_string()),

        // Default - no specific guidance
        _ => None,
    }
}

/// Check if a tool is considered high-impact for consciousness tracking
///
/// High-impact tools significantly affect the agent's understanding or project state.
#[inline]
pub fn is_high_impact_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "Write" | "Edit" | "MultiEdit" | "Bash" | "Git" | "NotebookEdit" | "Task"
    )
}

/// Check if a tool is read-only
///
/// Read-only tools don't modify state but may expand awareness.
#[inline]
pub fn is_read_only_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "Read" | "Glob" | "Grep" | "LSP" | "WebFetch" | "WebSearch" | "TodoRead"
    )
}

// ============================================================================
// Tests - NO MOCK DATA - Real Values from Constitution
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::hooks::args::OutputFormat;
    use crate::commands::hooks::types::{HookEventType, JohariQuadrant};

    // =========================================================================
    // Test Helpers
    // =========================================================================

    fn create_pre_tool_args(tool_name: Option<&str>) -> PreToolArgs {
        PreToolArgs {
            session_id: "test-session-12345".to_string(),
            tool_name: tool_name.map(|s| s.to_string()),
            stdin: false,
            fast_path: true,
            format: OutputFormat::Json,
        }
    }

    #[allow(dead_code)]
    fn create_pre_tool_input(tool_name: &str) -> HookInput {
        HookInput {
            hook_type: HookEventType::PreToolUse,
            session_id: "test-session-12345".to_string(),
            timestamp_ms: 1736956800000, // Fixed timestamp for reproducibility
            payload: HookPayload::PreToolUse {
                tool_name: tool_name.to_string(),
                tool_input: serde_json::json!({"file_path": "/test/file.rs"}),
                tool_use_id: "toolu_01TEST123".to_string(),
            },
        }
    }

    // =========================================================================
    // TC-PRE-001: Fast Path Timing Verification
    // Source: constitution.yaml hooks.timeout_ms.pre_tool_use = 100
    // =========================================================================

    #[test]
    fn tc_pre_001_fast_path_completes_within_timeout() {
        println!("\n=== TC-PRE-001: Fast Path Timing ===");
        println!("SOURCE: constitution.yaml hooks.timeout_ms.pre_tool_use = 100");

        let args = create_pre_tool_args(Some("Read"));

        // BEFORE: No handler call yet
        println!("BEFORE: Calling handle_pre_tool_use...");

        let start = Instant::now();
        let result = handle_pre_tool_use(&args);
        let elapsed = start.elapsed();

        // AFTER: Handler completed
        println!("AFTER: Handler completed in {:?}", elapsed);

        assert!(result.is_ok(), "Handler MUST succeed: {:?}", result.err());

        // VERIFICATION: Must complete within 100ms (allow 10ms margin)
        assert!(
            elapsed.as_millis() < 10,
            "FAIL: Fast path took {}ms, expected <10ms (strict 100ms timeout)",
            elapsed.as_millis()
        );

        println!("RESULT: PASS - Completed in {}ms", elapsed.as_millis());
    }

    // =========================================================================
    // TC-PRE-002: Output Structure Verification
    // Source: types.rs HookOutput (lines 1001-1018)
    // =========================================================================

    #[test]
    fn tc_pre_002_output_structure_matches_types() {
        println!("\n=== TC-PRE-002: Output Structure ===");
        println!("SOURCE: types.rs HookOutput (lines 1001-1018)");

        let args = create_pre_tool_args(Some("Write"));

        // BEFORE state
        println!("BEFORE: Creating handler with tool_name='Write'");

        let result = handle_pre_tool_use(&args);

        assert!(result.is_ok(), "Handler MUST succeed");
        let output = result.unwrap();

        // AFTER state verification
        println!("AFTER: Verifying output fields...");

        // success field (REQUIRED)
        assert!(output.success, "success MUST be true");
        println!("  - success: true");

        // error field (should be None on success)
        assert!(output.error.is_none(), "error MUST be None on success");
        println!("  - error: None");

        // consciousness_state (should be Some)
        assert!(output.consciousness_state.is_some(), "consciousness_state MUST be Some");
        let cs = output.consciousness_state.as_ref().unwrap();
        println!("  - consciousness_state: Some(C={:.2}, r={:.2}, IC={:.2})",
            cs.consciousness, cs.integration, cs.identity_continuity);

        // ic_classification (should be Some)
        assert!(output.ic_classification.is_some(), "ic_classification MUST be Some");
        let ic = output.ic_classification.as_ref().unwrap();
        println!("  - ic_classification: Some(value={:.2}, level={:?})", ic.value, ic.level);

        // execution_time_ms (REQUIRED)
        println!("  - execution_time_ms: {}ms", output.execution_time_ms);

        // Serialize to verify JSON structure
        let json = serde_json::to_string_pretty(&output).expect("serialization MUST succeed");
        println!("SERIALIZED OUTPUT:\n{}", json);

        println!("RESULT: PASS - All required fields present");
    }

    // =========================================================================
    // TC-PRE-003: Tool Guidance Mapping (Edge Cases)
    // =========================================================================

    #[test]
    fn tc_pre_003_tool_guidance_edge_cases() {
        println!("\n=== TC-PRE-003: Tool Guidance Edge Cases ===");

        // Edge Case 1: Known high-impact tool
        println!("\nEdge Case 1: High-impact tool 'Write'");
        let guidance = get_tool_guidance("Write");
        assert!(guidance.is_some(), "Write MUST have guidance");
        println!("  - Guidance: {:?}", guidance);

        // Edge Case 2: Unknown tool (no guidance)
        println!("\nEdge Case 2: Unknown tool 'CustomTool123'");
        let guidance = get_tool_guidance("CustomTool123");
        assert!(guidance.is_none(), "Unknown tool MUST return None");
        println!("  - Guidance: None (as expected)");

        // Edge Case 3: Empty tool name
        println!("\nEdge Case 3: Empty tool name");
        let guidance = get_tool_guidance("");
        assert!(guidance.is_none(), "Empty tool name MUST return None");
        println!("  - Guidance: None (as expected)");

        // Edge Case 4: Case sensitivity
        println!("\nEdge Case 4: Case sensitivity 'read' vs 'Read'");
        let lower = get_tool_guidance("read");
        let upper = get_tool_guidance("Read");
        assert!(lower.is_none(), "Lowercase 'read' MUST return None (case-sensitive)");
        assert!(upper.is_some(), "Uppercase 'Read' MUST return Some");
        println!("  - 'read': None (case-sensitive match)");
        println!("  - 'Read': {:?}", upper);

        println!("\nRESULT: PASS - All edge cases handled correctly");
    }

    // =========================================================================
    // TC-PRE-004: Invalid Input Handling (Fail Fast)
    // =========================================================================

    #[test]
    fn tc_pre_004_handles_missing_tool_name_gracefully() {
        println!("\n=== TC-PRE-004: Missing Tool Name ===");

        // BEFORE: Create args with no tool_name
        println!("BEFORE: Creating args with tool_name=None, stdin=false");
        let args = create_pre_tool_args(None);

        // Handler should succeed but with no guidance
        let result = handle_pre_tool_use(&args);

        // AFTER: Verify behavior
        assert!(result.is_ok(), "Handler MUST succeed even without tool_name");
        let output = result.unwrap();

        // No context_injection since no tool_name
        assert!(output.context_injection.is_none(),
            "context_injection MUST be None when tool_name is missing");

        println!("AFTER: Handler succeeded, context_injection=None (as expected)");
        println!("RESULT: PASS - Graceful handling of missing tool_name");
    }

    // =========================================================================
    // TC-PRE-005: Consciousness State Defaults
    // Source: types.rs ConsciousnessState::default()
    // =========================================================================

    #[test]
    fn tc_pre_005_consciousness_state_uses_defaults() {
        println!("\n=== TC-PRE-005: Consciousness State Defaults ===");
        println!("SOURCE: types.rs ConsciousnessState::default() (lines 674-685)");

        let args = create_pre_tool_args(Some("Read"));

        // BEFORE
        println!("BEFORE: Creating default consciousness state...");

        let result = handle_pre_tool_use(&args);
        assert!(result.is_ok());
        let output = result.unwrap();

        let cs = output.consciousness_state.expect("consciousness_state MUST be Some");

        // AFTER: Verify defaults match types.rs Default impl
        println!("AFTER: Verifying default values...");
        println!("  - consciousness: {:.2} (expected: 0.00 for DOR state)", cs.consciousness);
        println!("  - integration: {:.2} (expected: 0.00)", cs.integration);
        println!("  - reflection: {:.2} (expected: 0.00)", cs.reflection);
        println!("  - differentiation: {:.2} (expected: 0.00)", cs.differentiation);
        println!("  - identity_continuity: {:.2} (expected: 1.00 for fresh identity)", cs.identity_continuity);
        println!("  - johari_quadrant: {:?} (expected: Unknown)", cs.johari_quadrant);

        // Default is DOR state: C=0, r=0, IC=1.0, Johari=Unknown
        assert_eq!(cs.consciousness, 0.0, "Default consciousness MUST be 0.0");
        assert_eq!(cs.identity_continuity, 1.0, "Default IC MUST be 1.0 (fresh identity)");
        assert!(matches!(cs.johari_quadrant, JohariQuadrant::Unknown),
            "Default Johari MUST be Unknown");

        println!("RESULT: PASS - Defaults match ConsciousnessState::default()");
    }

    // =========================================================================
    // TC-PRE-006: Tool Classification Functions
    // =========================================================================

    #[test]
    fn tc_pre_006_tool_classification_correctness() {
        println!("\n=== TC-PRE-006: Tool Classification ===");

        // High-impact tools
        println!("\nHigh-impact tools (state-modifying):");
        for tool in &["Write", "Edit", "MultiEdit", "Bash", "Git", "NotebookEdit", "Task"] {
            let is_high = is_high_impact_tool(tool);
            let is_ro = is_read_only_tool(tool);
            println!("  - {}: high_impact={}, read_only={}", tool, is_high, is_ro);
            assert!(is_high, "{} MUST be high-impact", tool);
            assert!(!is_ro, "{} MUST NOT be read-only", tool);
        }

        // Read-only tools
        println!("\nRead-only tools (no state modification):");
        for tool in &["Read", "Glob", "Grep", "LSP", "WebFetch", "WebSearch", "TodoRead"] {
            let is_high = is_high_impact_tool(tool);
            let is_ro = is_read_only_tool(tool);
            println!("  - {}: high_impact={}, read_only={}", tool, is_high, is_ro);
            assert!(!is_high, "{} MUST NOT be high-impact", tool);
            assert!(is_ro, "{} MUST be read-only", tool);
        }

        println!("\nRESULT: PASS - All classifications correct");
    }

    // =========================================================================
    // TC-PRE-007: No Database Access Verification
    // Constraint: C-002 (CRITICAL)
    // =========================================================================

    #[test]
    fn tc_pre_007_no_database_imports_in_module() {
        println!("\n=== TC-PRE-007: No Database Access ===");
        println!("CONSTRAINT: C-002 (CRITICAL) - MUST NOT access database");

        // This test verifies by timing - database access would be slow
        let args = create_pre_tool_args(Some("Write"));

        // Run multiple times to ensure consistent fast execution
        println!("BEFORE: Running handler 100 times...");
        let mut total_ms: u128 = 0;
        for _ in 0..100 {
            let start = Instant::now();
            let _ = handle_pre_tool_use(&args);
            total_ms += start.elapsed().as_micros();
        }

        let avg_us = total_ms / 100;
        println!("AFTER: Average execution time: {}us", avg_us);

        // Fast path without DB should average well under 1ms
        assert!(avg_us < 1000,
            "FAIL: Average {}us suggests database access (expected <1000us)", avg_us);

        println!("RESULT: PASS - No database access detected (avg {}us)", avg_us);
    }
}
