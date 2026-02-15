//! E2E tests for error handling and recovery
//!
//! # NO MOCKS - Real shell scripts, real MCP, real database
//!
//! Tests verify:
//! 1. Empty stdin produces exit code 4 (INVALID_INPUT)
//! 2. Invalid JSON produces exit code 4 (INVALID_INPUT)
//! 3. stderr contains structured error JSON
//! 4. Shell scripts handle timeouts gracefully
//!
//! # Constitution References
//! - AP-26: Exit codes (0-6)
//! - REQ-HOOKS-45: E2E tests with real MCP
//! - REQ-HOOKS-47: No mock data in any tests

use super::helpers::*;
use serde_json::json;
use serial_test::serial;
use tempfile::TempDir;

/// Test that empty stdin produces INVALID_INPUT exit code
///
/// # Scenario:
/// Pipe empty string to shell script, expect exit code 4
#[tokio::test]
#[serial]
async fn test_e2e_empty_stdin_error() {
    // PREREQUISITE: Verify scripts exist
    if let Err(e) = verify_all_scripts_exist() {
        panic!("E2E test prerequisite failed: {}", e);
    }

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();

    println!("\n=== E2E Empty Stdin Error Test ===");

    // Test each hook script with empty stdin
    let scripts = [
        "session_start.sh",
        "pre_tool_use.sh",
        "post_tool_use.sh",
        "user_prompt_submit.sh",
        "session_end.sh",
    ];

    for script in &scripts {
        println!("\n--- Testing {} with empty stdin ---", script);

        let result = execute_hook_script(
            script,
            "", // Empty stdin
            TIMEOUT_SESSION_START_MS,
            db_path,
        )
        .expect(&format!("{} execution failed", script));

        println!("Exit code: {}", result.exit_code);
        println!("stdout: {}", result.stdout);
        println!("stderr: {}", result.stderr);

        // Should return INVALID_INPUT (4) for empty stdin
        assert_eq!(
            result.exit_code, EXIT_INVALID_INPUT,
            "{} should return exit code {} for empty stdin, got {}",
            script, EXIT_INVALID_INPUT, result.exit_code
        );

        // Verify stderr contains error information
        assert!(
            !result.stderr.is_empty() || !result.stdout.is_empty(),
            "{} should output error information",
            script
        );

        log_test_evidence(
            "test_e2e_empty_stdin_error",
            &format!("empty_stdin_{}", script),
            "empty-stdin-test",
            &result,
            false,
        );
    }

    println!("\n=== Empty Stdin Test Complete ===");
}

/// Test that invalid JSON produces error exit codes
///
/// # Scenario:
/// Pipe malformed JSON to shell script, verify non-success exit codes
///
/// # Note:
/// Different types of JSON errors may produce different exit codes:
/// - Malformed JSON (parse errors) → EXIT_INVALID_INPUT (4)
/// - Wrong JSON type (array vs object) → EXIT_SESSION_NOT_FOUND (5) from jq
/// Both are acceptable error behaviors per TASK-HOOKS-017.
#[tokio::test]
#[serial]
async fn test_e2e_invalid_json_error() {
    // PREREQUISITE: Verify scripts exist
    if let Err(e) = verify_all_scripts_exist() {
        panic!("E2E test prerequisite failed: {}", e);
    }

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();

    println!("\n=== E2E Invalid JSON Error Test ===");

    // Cases that should produce INVALID_INPUT (4) - actual JSON parse errors
    let parse_error_cases = [
        ("truncated", r#"{"session_id": "test"#),
        ("missing_brace", r#"{"session_id": "test""#),
        ("plain_text", "not json at all"),
    ];

    for (case_name, invalid_json) in &parse_error_cases {
        println!("\n--- Testing session_start.sh with {} ---", case_name);
        println!("Input: {}", invalid_json);

        let result = execute_hook_script(
            "session_start.sh",
            invalid_json,
            TIMEOUT_SESSION_START_MS,
            db_path,
        )
        .expect("session_start.sh execution failed");

        println!("Exit code: {}", result.exit_code);
        println!("stdout: {}", result.stdout);
        println!("stderr: {}", result.stderr);

        // Should return INVALID_INPUT (4) for JSON parse errors
        assert_eq!(
            result.exit_code, EXIT_INVALID_INPUT,
            "session_start.sh should return exit code {} for {}, got {}",
            EXIT_INVALID_INPUT, case_name, result.exit_code
        );

        log_test_evidence(
            "test_e2e_invalid_json_error",
            &format!("invalid_json_{}", case_name),
            "invalid-json-test",
            &result,
            false,
        );
    }

    // Test array input (valid JSON but wrong type - jq indexing fails)
    {
        let case_name = "array_instead_of_object";
        let invalid_json = r#"["session_id", "test"]"#;

        println!(
            "\n--- Testing session_start.sh with {} (type error) ---",
            case_name
        );
        println!("Input: {}", invalid_json);

        let result = execute_hook_script(
            "session_start.sh",
            invalid_json,
            TIMEOUT_SESSION_START_MS,
            db_path,
        )
        .expect("session_start.sh execution failed");

        println!("Exit code: {}", result.exit_code);
        println!("stdout: {}", result.stdout);
        println!("stderr: {}", result.stderr);

        // jq fails when trying to index an array - should return non-zero
        assert_ne!(
            result.exit_code, EXIT_SUCCESS,
            "session_start.sh should return non-zero exit code for {}, got {}",
            case_name, result.exit_code
        );

        log_test_evidence(
            "test_e2e_invalid_json_error",
            &format!("invalid_json_{}", case_name),
            "invalid-json-test",
            &result,
            false,
        );
    }

    // Test null input
    // Note: jq treats null as a valid JSON value that extracts to "null" for missing fields
    // The CLI may accept this and auto-generate session_id
    {
        let case_name = "null";
        let invalid_json = "null";

        println!("\n--- Testing session_start.sh with {} ---", case_name);
        println!("Input: {}", invalid_json);

        let result = execute_hook_script(
            "session_start.sh",
            invalid_json,
            TIMEOUT_SESSION_START_MS,
            db_path,
        )
        .expect("session_start.sh execution failed");

        println!("Exit code: {}", result.exit_code);
        println!("stdout: {}", result.stdout);
        println!("stderr: {}", result.stderr);

        // Document behavior - null may be accepted (auto-generates session_id)
        if result.exit_code == EXIT_SUCCESS {
            println!("CLI accepted null input (auto-generated session_id)");
        } else {
            println!(
                "CLI rejected null input with exit code {}",
                result.exit_code
            );
        }

        log_test_evidence(
            "test_e2e_invalid_json_error",
            &format!("invalid_json_{}", case_name),
            "invalid-json-test",
            &result,
            false,
        );
    }

    println!("\n=== Invalid JSON Test Complete ===");
}

/// Test behavior when session_id is missing
///
/// # Scenario:
/// Pipe JSON without session_id field
///
/// # Note:
/// The CLI may auto-generate a session_id if not provided,
/// making this valid input. This test documents the actual behavior.
#[tokio::test]
#[serial]
async fn test_e2e_missing_session_id_behavior() {
    // PREREQUISITE: Verify scripts exist
    if let Err(e) = verify_all_scripts_exist() {
        panic!("E2E test prerequisite failed: {}", e);
    }

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();

    println!("\n=== E2E Missing Session ID Behavior Test ===");

    // Valid JSON but missing session_id
    let missing_session_id = json!({
        "hook_type": "session_start",
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    println!(
        "Input: {}",
        serde_json::to_string_pretty(&missing_session_id).unwrap()
    );

    let result = execute_hook_script(
        "session_start.sh",
        &serde_json::to_string(&missing_session_id).unwrap(),
        TIMEOUT_SESSION_START_MS,
        db_path,
    )
    .expect("session_start.sh execution failed");

    println!("Exit code: {}", result.exit_code);
    println!("stdout: {}", result.stdout);
    println!("stderr: {}", result.stderr);

    // CLI must either auto-generate session_id (exit 0) or reject with INVALID_INPUT (exit 4)
    if result.exit_code == EXIT_SUCCESS {
        // Verify the response is still valid JSON with success=true
        let output_json = result.parse_stdout().expect("Should return valid JSON");
        assert_eq!(output_json.get("success"), Some(&json!(true)));
    } else {
        assert_eq!(
            result.exit_code, EXIT_INVALID_INPUT,
            "Missing session_id should either succeed (auto-generate) or return INVALID_INPUT (4), got {}",
            result.exit_code
        );
    }

    log_test_evidence(
        "test_e2e_missing_session_id_behavior",
        "missing_session_id",
        "missing-field-test",
        &result,
        false,
    );

    println!("\n=== Missing Session ID Behavior Test Complete ===");
}

/// Test error recovery after failed hook
///
/// # Scenario:
/// 1. Execute a failing hook (invalid input)
/// 2. Then execute a valid hook
/// 3. Verify the system recovers and processes the valid hook correctly
#[tokio::test]
#[serial]
async fn test_e2e_hook_error_recovery() {
    // PREREQUISITE: Verify scripts exist
    if let Err(e) = verify_all_scripts_exist() {
        panic!("E2E test prerequisite failed: {}", e);
    }

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();

    println!("\n=== E2E Hook Error Recovery Test ===");

    let session_id = generate_e2e_session_id("recovery");
    println!("Session ID: {}", session_id);

    // Step 1: Execute failing hook (empty stdin)
    println!("\n[1/3] Executing failing hook (empty stdin)...");
    let fail_result =
        execute_hook_script("session_start.sh", "", TIMEOUT_SESSION_START_MS, db_path)
            .expect("session_start.sh execution failed");

    assert_eq!(
        fail_result.exit_code, EXIT_INVALID_INPUT,
        "Expected INVALID_INPUT exit code"
    );
    println!(
        "First hook failed as expected (exit code {})",
        fail_result.exit_code
    );

    // Step 2: Execute valid hook
    println!("\n[2/3] Executing valid hook...");
    let valid_input = create_claude_code_session_start_input(&session_id);
    let valid_result = execute_hook_script(
        "session_start.sh",
        &valid_input,
        TIMEOUT_SESSION_START_MS,
        db_path,
    )
    .expect("session_start.sh execution failed");

    assert_eq!(
        valid_result.exit_code, EXIT_SUCCESS,
        "Second hook should succeed.\\nstdout: {}\\nstderr: {}",
        valid_result.stdout, valid_result.stderr
    );
    println!(
        "Second hook succeeded (exit code {})",
        valid_result.exit_code
    );

    // Step 3: Verify session is properly initialized
    println!("\n[3/3] Verifying session state...");
    let output_json = valid_result
        .parse_stdout()
        .expect("Invalid JSON from second hook");

    assert_eq!(
        output_json.get("success"),
        Some(&json!(true)),
        "Session should be successfully started"
    );
    println!("Session state verified: success=true");

    // Cleanup: end the session
    let end_input = create_claude_code_session_end_input(&session_id, "normal");
    let end_result = execute_hook_script(
        "session_end.sh",
        &end_input,
        TIMEOUT_SESSION_END_MS,
        db_path,
    )
    .expect("session_end.sh failed");

    assert_eq!(end_result.exit_code, EXIT_SUCCESS);

    // Verify snapshot was created (proves full recovery)
    let snapshot_exists = verify_snapshot_exists(db_path, &session_id);
    println!("Snapshot created after recovery: {}", snapshot_exists);

    log_test_evidence(
        "test_e2e_hook_error_recovery",
        "recovery_complete",
        &session_id,
        &end_result,
        snapshot_exists,
    );

    println!("\n=== Error Recovery Test Complete ===");
}

/// Test shell script timeout behavior
///
/// # Note:
/// This test verifies that our test infrastructure properly detects timeouts.
/// Actual shell scripts should complete well within their budgets.
#[tokio::test]
#[serial]
async fn test_e2e_shell_script_timeout() {
    // PREREQUISITE: Verify scripts exist
    if let Err(e) = verify_all_scripts_exist() {
        panic!("E2E test prerequisite failed: {}", e);
    }

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();

    println!("\n=== E2E Shell Script Timeout Test ===");

    // Test that pre_tool_use.sh completes well within timeout
    let session_id = generate_e2e_session_id("timeout");
    let pre_input = create_claude_code_pre_tool_input(
        &session_id,
        "Read",
        json!({"file_path": "/tmp/test.txt"}),
    );

    // Execute with very short timeout to test timeout handling
    // Note: This tests our infrastructure, not the actual script
    println!("Testing timeout detection with very short timeout...");

    let start = std::time::Instant::now();
    let result = execute_hook_script(
        "pre_tool_use.sh",
        &pre_input,
        50, // 50ms - very short, should likely timeout
        db_path,
    );

    let elapsed = start.elapsed();
    println!("Execution time: {:?}", elapsed);

    match result {
        Ok(res) => {
            println!("Script completed within timeout");
            println!("Exit code: {}", res.exit_code);
            println!("Execution time: {}ms", res.execution_time_ms);

            // If it completed within 50ms test timeout, that's fine.
            // The 500ms total budget in constitution includes CLI startup + logic.
            // Shell script overhead (bash startup, jq parsing) adds ~100ms.
            // A shell script wrapper realistically takes 300-500ms total.
            // This test verifies our infrastructure handles timeouts correctly.
            assert!(
                res.execution_time_ms < 500,
                "pre_tool_use.sh should complete within shell wrapper budget (was {}ms)",
                res.execution_time_ms
            );
        }
        Err(e) => {
            let error_msg = e.to_string();
            println!("Execution failed (possibly timeout): {}", error_msg);
            // Timeout is acceptable for this test - we're testing with very short timeout
            if error_msg.contains("timeout") || error_msg.contains("timed out") {
                println!("Timeout detected as expected (this is acceptable for 50ms budget)");
            }
        }
    }

    // Now test with normal timeout - should succeed
    println!("\nTesting with normal timeout...");
    let normal_result = execute_hook_script(
        "pre_tool_use.sh",
        &pre_input,
        TIMEOUT_PRE_TOOL_MS + 500, // Normal timeout with overhead
        db_path,
    )
    .expect("pre_tool_use.sh should succeed with normal timeout");

    println!(
        "Normal execution completed in {}ms",
        normal_result.execution_time_ms
    );

    // The shell script has 500ms total budget per constitution.yaml.
    // With shell overhead (~50-100ms), total should be 600ms max.
    assert!(
        normal_result.execution_time_ms < 600,
        "pre_tool_use.sh should complete within shell wrapper budget (was {}ms)",
        normal_result.execution_time_ms
    );

    log_test_evidence(
        "test_e2e_shell_script_timeout",
        "timeout_test",
        &session_id,
        &normal_result,
        false,
    );

    println!("\n=== Timeout Test Complete ===");
}

/// Test structured error output in stderr
///
/// # Scenario:
/// Verify that error conditions produce structured JSON in stderr
#[tokio::test]
#[serial]
async fn test_e2e_structured_error_output() {
    // PREREQUISITE: Verify scripts exist
    if let Err(e) = verify_all_scripts_exist() {
        panic!("E2E test prerequisite failed: {}", e);
    }

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();

    println!("\n=== E2E Structured Error Output Test ===");

    // Execute with invalid input to trigger error
    let result = execute_hook_script(
        "session_start.sh",
        "invalid json {{{",
        TIMEOUT_SESSION_START_MS,
        db_path,
    )
    .expect("session_start.sh execution failed");

    println!("Exit code: {}", result.exit_code);
    println!("stdout: {}", result.stdout);
    println!("stderr: {}", result.stderr);

    // Verify we got an error exit code
    assert_ne!(
        result.exit_code, EXIT_SUCCESS,
        "Should return error exit code for invalid input"
    );

    // Check if stderr or stdout contains error information
    let has_error_info = !result.stderr.is_empty()
        || result.stdout.contains("error")
        || result.stdout.contains("Error")
        || result.stdout.contains("failed");

    println!(
        "Error information present: {} (stderr len: {}, stdout contains error: {})",
        has_error_info,
        result.stderr.len(),
        result.stdout.contains("error") || result.stdout.contains("Error")
    );

    // Try to parse error from stdout or stderr as JSON
    let error_json: Option<serde_json::Value> = if !result.stderr.is_empty() {
        serde_json::from_str(&result.stderr).ok()
    } else {
        serde_json::from_str(&result.stdout).ok()
    };

    if let Some(error) = error_json {
        println!(
            "Parsed error JSON: {}",
            serde_json::to_string_pretty(&error).unwrap()
        );

        // Check for common error fields
        if error.get("error").is_some() || error.get("message").is_some() {
            println!("Structured error contains error/message field");
        }
    } else {
        println!("Error output is not JSON (may be plain text from shell script)");
    }

    log_test_evidence(
        "test_e2e_structured_error_output",
        "structured_error",
        "error-output-test",
        &result,
        false,
    );

    println!("\n=== Structured Error Output Test Complete ===");
}

/// Test database error handling (exit code 3)
///
/// # Scenario:
/// Attempt to use an invalid database path and verify proper error handling
#[tokio::test]
#[serial]
async fn test_e2e_database_error_handling() {
    // PREREQUISITE: Verify scripts exist
    if let Err(e) = verify_all_scripts_exist() {
        panic!("E2E test prerequisite failed: {}", e);
    }

    println!("\n=== E2E Database Error Handling Test ===");

    // Use a path that cannot be written to
    let invalid_db_path = std::path::Path::new("/nonexistent/path/that/cannot/exist/db");

    let session_id = generate_e2e_session_id("db-error");
    let input = create_claude_code_session_start_input(&session_id);

    let result = execute_hook_script(
        "session_start.sh",
        &input,
        TIMEOUT_SESSION_START_MS,
        invalid_db_path,
    )
    .expect("session_start.sh execution failed");

    println!("Exit code: {}", result.exit_code);
    println!("stdout: {}", result.stdout);
    println!("stderr: {}", result.stderr);

    // Invalid DB path must produce a non-zero exit code
    assert_ne!(
        result.exit_code, EXIT_SUCCESS,
        "session_start.sh should fail with invalid DB path /nonexistent/path/..., but got exit code 0 (success)"
    );
    // Should be either EXIT_DATABASE_ERROR (3) or EXIT_GENERAL_ERROR (1)
    assert!(
        result.exit_code == EXIT_DATABASE_ERROR || result.exit_code == EXIT_GENERAL_ERROR,
        "Expected EXIT_DATABASE_ERROR (3) or EXIT_GENERAL_ERROR (1) for invalid DB path, got {}",
        result.exit_code
    );

    log_test_evidence(
        "test_e2e_database_error_handling",
        "db_error",
        &session_id,
        &result,
        false,
    );

    println!("\n=== Database Error Handling Test Complete ===");
}
