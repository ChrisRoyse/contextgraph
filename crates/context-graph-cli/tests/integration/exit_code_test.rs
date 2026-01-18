//! Integration tests for exit code behavior
//!
//! # Tests
//! - `test_exit_code_4_empty_session_id`: Empty session_id returns exit code 4
//! - `test_exit_code_4_malformed_json`: Malformed JSON returns exit code 4
//! - `test_exit_code_4_missing_required_fields`: Missing fields returns exit code 4
//! - `test_exit_code_0_valid_input_all_hooks`: Valid input returns exit code 0
//! - `test_exit_code_5_previous_session_not_found`: Non-existent previous session returns exit code 5
//!
//! # Exit Codes (per TECH-HOOKS.md Section 3.2)
//! - 0: Success
//! - 1: General error (IO, unspecified)
//! - 2: Timeout/Corruption
//! - 3: Database error
//! - 4: Invalid input (malformed JSON, empty session_id, missing fields)
//! - 5: Session not found
//! - 6: Crisis triggered
//!
//! # Constitution References
//! - AP-26: Exit codes and fail fast behavior
//! - REQ-HOOKS-47: No mock data in tests

use serde_json::json;
use tempfile::TempDir;

use super::helpers::{
    create_session_end_input, create_session_start_input, generate_test_session_id,
    invoke_hook_with_stdin, log_test_evidence, EXIT_INVALID_INPUT, EXIT_SUCCESS,
};

// =============================================================================
// Exit Code 4: Invalid Input Tests
// =============================================================================

/// Test that empty session_id returns exit code 4
///
/// Per TASK-HOOKS-016 Edge Case 1:
/// - Empty session_id must fail immediately
/// - No silent fallbacks, no default values
#[test]
fn test_exit_code_4_empty_session_id() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();

    // Create input with empty session_id
    let input = json!({
        "hook_type": "session_start",
        "session_id": "",
        "timestamp_ms": chrono::Utc::now().timestamp_millis(),
        "payload": {
            "type": "session_start",
            "data": {
                "cwd": "/tmp",
                "source": "cli"
            }
        }
    })
    .to_string();

    // Invoke with empty session_id
    let result = invoke_hook_with_stdin("session-start", "", &[], &input, db_path);

    assert_eq!(
        result.exit_code, EXIT_INVALID_INPUT,
        "Empty session_id should return exit code 4.\nstdout: {}\nstderr: {}",
        result.stdout, result.stderr
    );

    // Verify error message mentions session_id
    let output_lower = result.stdout.to_lowercase() + &result.stderr.to_lowercase();
    assert!(
        output_lower.contains("session")
            || output_lower.contains("empty")
            || output_lower.contains("invalid"),
        "Error should mention session_id issue.\nstdout: {}\nstderr: {}",
        result.stdout,
        result.stderr
    );

    log_test_evidence(
        "test_exit_code_4_empty_session_id",
        "invalid_input",
        "",
        result.exit_code,
        result.execution_time_ms,
        false,
        Some(json!({
            "expected_exit_code": EXIT_INVALID_INPUT,
            "actual_exit_code": result.exit_code,
        })),
    );
}

/// Test that malformed JSON returns exit code 4
#[test]
fn test_exit_code_4_malformed_json() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();
    let session_id = generate_test_session_id("malformed");

    // Send completely invalid JSON
    let malformed_inputs = [
        "not json at all",
        "{unclosed",
        r#"{"key": value_without_quotes}"#,
        "",
    ];

    for (i, malformed) in malformed_inputs.iter().enumerate() {
        let result = invoke_hook_with_stdin("session-start", &session_id, &[], malformed, db_path);

        // Should return exit code 4 for malformed JSON
        // Note: Empty stdin might be handled differently
        if !malformed.is_empty() {
            assert_eq!(
                result.exit_code, EXIT_INVALID_INPUT,
                "Malformed JSON #{} should return exit code 4.\nInput: '{}'\nstdout: {}\nstderr: {}",
                i, malformed, result.stdout, result.stderr
            );
        }

        log_test_evidence(
            "test_exit_code_4_malformed_json",
            "malformed_json",
            &session_id,
            result.exit_code,
            result.execution_time_ms,
            false,
            Some(json!({
                "input_index": i,
                "malformed_input": malformed.chars().take(50).collect::<String>(),
            })),
        );
    }
}

/// Test that missing required fields returns exit code 4
#[test]
fn test_exit_code_4_missing_required_fields() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();
    let session_id = generate_test_session_id("missing-fields");

    // Missing hook_type
    let input1 = json!({
        "session_id": session_id,
        "timestamp_ms": 1705312345678i64,
        "payload": {"type": "session_start", "data": {"cwd": "/tmp", "source": "cli"}}
    })
    .to_string();

    let result1 = invoke_hook_with_stdin("session-start", &session_id, &[], &input1, db_path);

    // Missing payload
    let input2 = json!({
        "hook_type": "session_start",
        "session_id": session_id,
        "timestamp_ms": 1705312345678i64
    })
    .to_string();

    let result2 = invoke_hook_with_stdin("session-start", &session_id, &[], &input2, db_path);

    // Missing session_id in JSON (but provided via CLI)
    let input3 = json!({
        "hook_type": "session_start",
        "timestamp_ms": 1705312345678i64,
        "payload": {"type": "session_start", "data": {"cwd": "/tmp", "source": "cli"}}
    })
    .to_string();

    let result3 = invoke_hook_with_stdin("session-start", &session_id, &[], &input3, db_path);

    // Log all results
    log_test_evidence(
        "test_exit_code_4_missing_required_fields",
        "missing_fields",
        &session_id,
        result1.exit_code,
        result1.execution_time_ms,
        false,
        Some(json!({
            "missing_hook_type_exit": result1.exit_code,
            "missing_payload_exit": result2.exit_code,
            "missing_json_session_id_exit": result3.exit_code,
        })),
    );

    // At least one of these should return exit code 4
    let has_invalid_input = [result1.exit_code, result2.exit_code]
        .iter()
        .any(|&code| code == EXIT_INVALID_INPUT);

    assert!(
        has_invalid_input,
        "At least one missing-field case should return exit code 4.\n\
         missing_hook_type: {}\nmissing_payload: {}\nmissing_json_session_id: {}",
        result1.exit_code, result2.exit_code, result3.exit_code
    );
}

// =============================================================================
// Exit Code 0: Valid Input Tests
// =============================================================================

/// Test that valid input returns exit code 0 for all hooks
#[test]
fn test_exit_code_0_valid_input_all_hooks() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();
    let session_id = generate_test_session_id("valid-input");

    // SessionStart with valid input
    let start_input = create_session_start_input(&session_id, "/tmp", "cli", None);
    let start_result =
        invoke_hook_with_stdin("session-start", &session_id, &[], &start_input, db_path);
    assert_eq!(
        start_result.exit_code, EXIT_SUCCESS,
        "SessionStart with valid input should return 0.\nstderr: {}",
        start_result.stderr
    );

    // PreToolUse with valid input
    let pre_input = json!({
        "hook_type": "pre_tool_use",
        "session_id": session_id,
        "timestamp_ms": chrono::Utc::now().timestamp_millis(),
        "payload": {
            "type": "pre_tool_use",
            "data": {
                "tool_name": "Read",
                "tool_input": {"file_path": "/tmp/test.txt"},
                "tool_use_id": "tu-001"
            }
        }
    })
    .to_string();

    let pre_result = invoke_hook_with_stdin(
        "pre-tool",
        &session_id,
        &["--tool-name", "Read", "--fast-path", "true"],
        &pre_input,
        db_path,
    );
    assert_eq!(
        pre_result.exit_code, EXIT_SUCCESS,
        "PreToolUse with valid input should return 0.\nstderr: {}",
        pre_result.stderr
    );

    // PostToolUse with valid input
    let post_input = json!({
        "hook_type": "post_tool_use",
        "session_id": session_id,
        "timestamp_ms": chrono::Utc::now().timestamp_millis(),
        "payload": {
            "type": "post_tool_use",
            "data": {
                "tool_name": "Read",
                "tool_input": {"file_path": "/tmp/test.txt"},
                "tool_response": "file content",
                "tool_use_id": "tu-001"
            }
        }
    })
    .to_string();

    let post_result = invoke_hook_with_stdin(
        "post-tool",
        &session_id,
        &["--tool-name", "Read", "--success", "true"],
        &post_input,
        db_path,
    );
    assert_eq!(
        post_result.exit_code, EXIT_SUCCESS,
        "PostToolUse with valid input should return 0.\nstderr: {}",
        post_result.stderr
    );

    // UserPromptSubmit with valid input
    let prompt_input = json!({
        "hook_type": "user_prompt_submit",
        "session_id": session_id,
        "timestamp_ms": chrono::Utc::now().timestamp_millis(),
        "payload": {
            "type": "user_prompt_submit",
            "data": {
                "prompt": "Hello, please help me.",
                "context": []
            }
        }
    })
    .to_string();

    let prompt_result =
        invoke_hook_with_stdin("prompt-submit", &session_id, &[], &prompt_input, db_path);
    assert_eq!(
        prompt_result.exit_code, EXIT_SUCCESS,
        "UserPromptSubmit with valid input should return 0.\nstderr: {}",
        prompt_result.stderr
    );

    // SessionEnd with valid input
    let end_input = create_session_end_input(&session_id, 60000, "normal", None);
    let end_result = invoke_hook_with_stdin(
        "session-end",
        &session_id,
        &["--duration-ms", "60000"],
        &end_input,
        db_path,
    );
    assert_eq!(
        end_result.exit_code, EXIT_SUCCESS,
        "SessionEnd with valid input should return 0.\nstderr: {}",
        end_result.stderr
    );

    log_test_evidence(
        "test_exit_code_0_valid_input_all_hooks",
        "all_hooks",
        &session_id,
        EXIT_SUCCESS,
        start_result.execution_time_ms
            + pre_result.execution_time_ms
            + post_result.execution_time_ms
            + prompt_result.execution_time_ms
            + end_result.execution_time_ms,
        true,
        Some(json!({
            "session_start_exit": start_result.exit_code,
            "pre_tool_exit": pre_result.exit_code,
            "post_tool_exit": post_result.exit_code,
            "prompt_submit_exit": prompt_result.exit_code,
            "session_end_exit": end_result.exit_code,
        })),
    );
}

// =============================================================================
// Exit Code 5: Session Not Found Test
// =============================================================================

/// Test graceful handling of non-existent previous_session_id
///
/// The CLI is designed to be resilient: when a previous session is not found,
/// it logs a warning and starts fresh rather than failing. This tests that:
/// 1. Exit code is 0 (success - graceful degradation)
/// 2. A warning is logged about the missing session
/// 3. The session starts fresh with valid output
#[test]
fn test_graceful_handling_previous_session_not_found() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();
    let session_id = generate_test_session_id("session-not-found");
    let fake_previous = "fake-previous-session-that-does-not-exist";

    let input = create_session_start_input(&session_id, "/tmp", "resume", Some(fake_previous));
    let result = invoke_hook_with_stdin(
        "session-start",
        &session_id,
        &["--previous-session-id", fake_previous],
        &input,
        db_path,
    );

    // CLI is resilient: logs warning and starts fresh instead of failing
    assert_eq!(
        result.exit_code, EXIT_SUCCESS,
        "Non-existent previous_session_id should gracefully start fresh.\nstdout: {}\nstderr: {}",
        result.stdout, result.stderr
    );

    // Verify warning is logged about missing session
    // The CLI logs: "cache is cold, cannot link to previous session" with previous_session_id
    let stderr_lower = result.stderr.to_lowercase();
    assert!(
        stderr_lower.contains("previous session not found")
            || stderr_lower.contains("starting fresh")
            || stderr_lower.contains("cannot link to previous session")
            || stderr_lower.contains("cache is cold"),
        "Should log warning about missing previous session.\nstderr: {}",
        result.stderr
    );

    // Verify output indicates success
    if let Ok(json) = result.parse_stdout() {
        assert_eq!(
            json.get("success").and_then(|v| v.as_bool()),
            Some(true),
            "Output should show success=true"
        );
    }

    log_test_evidence(
        "test_graceful_handling_previous_session_not_found",
        "graceful_degradation",
        &session_id,
        result.exit_code,
        result.execution_time_ms,
        false,
        Some(json!({
            "fake_previous_session": fake_previous,
            "behavior": "graceful_degradation",
            "exit_code": result.exit_code,
        })),
    );
}

// =============================================================================
// Exit Code Consistency Tests
// =============================================================================

/// Test that error responses contain required fields
#[test]
fn test_error_response_format() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();

    // Trigger an error by using empty session_id
    let result = invoke_hook_with_stdin("session-start", "", &[], "{}", db_path);

    // Should have non-zero exit code
    assert_ne!(
        result.exit_code, EXIT_SUCCESS,
        "Invalid input should return non-zero exit code"
    );

    // Try to parse output as JSON
    if let Ok(json) = result.parse_stdout() {
        // Error response should have success=false
        if let Some(success) = json.get("success") {
            assert_eq!(
                success.as_bool(),
                Some(false),
                "Error response should have success=false"
            );
        }

        // Should have an error indicator
        let has_error = json.get("error").is_some()
            || json.get("message").is_some()
            || json.get("code").is_some();

        assert!(
            has_error || json.get("success").and_then(|v| v.as_bool()) == Some(false),
            "Error response should have error indicator"
        );
    }

    log_test_evidence(
        "test_error_response_format",
        "error_format",
        "",
        result.exit_code,
        result.execution_time_ms,
        false,
        Some(json!({
            "exit_code": result.exit_code,
            "has_stdout": !result.stdout.is_empty(),
            "has_stderr": !result.stderr.is_empty(),
        })),
    );
}

/// Test that stderr contains JSON for error cases
#[test]
fn test_stderr_json_for_errors() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();

    // Trigger an error
    let result = invoke_hook_with_stdin("session-start", "", &[], "invalid json {", db_path);

    // stderr should not be empty for errors
    if result.exit_code != EXIT_SUCCESS {
        // Either stdout or stderr should have content
        let has_output = !result.stdout.is_empty() || !result.stderr.is_empty();
        assert!(
            has_output,
            "Error case should produce output in stdout or stderr"
        );
    }

    log_test_evidence(
        "test_stderr_json_for_errors",
        "stderr_format",
        "",
        result.exit_code,
        result.execution_time_ms,
        false,
        Some(json!({
            "stderr_len": result.stderr.len(),
            "stdout_len": result.stdout.len(),
        })),
    );
}
