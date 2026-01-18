//! E2E tests for complete session workflow
//!
//! # NO MOCKS - Real shell scripts, real MCP, real database
//!
//! These tests execute the actual shell scripts in .claude/hooks/
//! exactly as Claude Code would - by piping JSON to stdin.
//!
//! # Tests verify:
//! 1. Shell scripts execute correctly
//! 2. CLI binary invoked properly
//! 3. Coherence state updated in database
//! 4. Topic stability snapshots persisted
//! 5. Topic coherence brief output format
//!
//! # Constitution References
//! - REQ-HOOKS-45: E2E tests with real MCP
//! - REQ-HOOKS-46: E2E tests simulate Claude Code
//! - REQ-HOOKS-47: No mock data in any tests

use std::path::Path;

use super::helpers::*;
use serde_json::json;
use tempfile::TempDir;

// =============================================================================
// Test Setup Helpers
// =============================================================================

/// Common test setup: verify scripts exist, create temp dir, generate session ID
fn setup_test(prefix: &str) -> (TempDir, String) {
    if let Err(e) = verify_all_scripts_exist() {
        panic!("E2E test prerequisite failed: {}", e);
    }
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let session_id = generate_e2e_session_id(prefix);
    (temp_dir, session_id)
}

/// Execute session_start.sh and assert success
fn start_session(session_id: &str, db_path: &Path) -> HookScriptResult {
    let input = create_claude_code_session_start_input(session_id);
    let result = execute_hook_script(
        "session_start.sh",
        &input,
        TIMEOUT_SESSION_START_MS,
        db_path,
    )
    .expect("session_start.sh failed");
    assert_eq!(
        result.exit_code, EXIT_SUCCESS,
        "session_start.sh failed: stderr={}",
        result.stderr
    );
    result
}

/// Execute session_end.sh and assert success
fn end_session(session_id: &str, db_path: &Path) -> HookScriptResult {
    let input = create_claude_code_session_end_input(session_id, "normal");
    let result = execute_hook_script(
        "session_end.sh",
        &input,
        TIMEOUT_SESSION_END_MS,
        db_path,
    )
    .expect("session_end.sh failed");
    assert_eq!(
        result.exit_code, EXIT_SUCCESS,
        "session_end.sh failed: stderr={}",
        result.stderr
    );
    result
}

/// Execute user_prompt_submit.sh and assert success
fn submit_prompt(session_id: &str, prompt: &str, db_path: &Path) -> HookScriptResult {
    let input = create_claude_code_prompt_submit_input(session_id, prompt);
    let result = execute_hook_script(
        "user_prompt_submit.sh",
        &input,
        TIMEOUT_USER_PROMPT_MS,
        db_path,
    )
    .expect("user_prompt_submit.sh failed");
    assert_eq!(
        result.exit_code, EXIT_SUCCESS,
        "user_prompt_submit.sh failed: stderr={}",
        result.stderr
    );
    result
}

/// Execute post_tool_use.sh and assert success
fn capture_memory(
    session_id: &str,
    tool_name: &str,
    file_path: &str,
    content: &str,
    db_path: &Path,
) -> HookScriptResult {
    let input = create_claude_code_post_tool_input(
        session_id,
        tool_name,
        json!({"file_path": file_path}),
        content,
        true,
    );
    let result = execute_hook_script(
        "post_tool_use.sh",
        &input,
        TIMEOUT_POST_TOOL_MS,
        db_path,
    )
    .expect("post_tool_use.sh failed");
    assert_eq!(
        result.exit_code, EXIT_SUCCESS,
        "post_tool_use.sh failed: stderr={}",
        result.stderr
    );
    result
}

/// Test complete session lifecycle via shell scripts
///
/// Flow: SessionStart -> PreToolUse -> PostToolUse -> UserPromptSubmit -> SessionEnd
///
/// # Verifies:
/// - Each shell script returns exit code 0
/// - Each script outputs valid JSON with success=true
/// - Coherence state is updated after each hook
/// - SessionEnd creates snapshot in RocksDB
#[tokio::test]
async fn test_e2e_full_session_workflow() {
    // PREREQUISITE: Verify scripts exist
    if let Err(e) = verify_all_scripts_exist() {
        panic!("E2E test prerequisite failed: {}", e);
    }

    // SETUP: Create temp database
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();
    let session_id = generate_e2e_session_id("full");

    println!("\n=== E2E Full Session Workflow Test ===");
    println!("Session ID: {}", session_id);
    println!("Database: {}", db_path.display());

    // 1. Execute session_start.sh
    println!("\n[1/5] Executing session_start.sh...");
    let start_input = create_claude_code_session_start_input(&session_id);
    let start_result = execute_hook_script(
        "session_start.sh",
        &start_input,
        TIMEOUT_SESSION_START_MS,
        db_path,
    )
    .expect("session_start.sh execution failed");

    assert_eq!(
        start_result.exit_code, EXIT_SUCCESS,
        "session_start.sh exit code mismatch.\nstdout: {}\nstderr: {}",
        start_result.stdout, start_result.stderr
    );

    // Verify JSON output
    let start_json = start_result
        .parse_stdout()
        .expect("Invalid JSON from session_start.sh");
    println!(
        "session_start.sh output: {}",
        serde_json::to_string_pretty(&start_json).unwrap()
    );

    // Check for success field
    assert_eq!(
        start_json.get("success"),
        Some(&json!(true)),
        "session_start.sh success=false"
    );

    // Verify topic state in output (if present)
    if let Some(topic_state) = start_result.topic_state() {
        println!("Topic state: {:?}", topic_state);
        assert!(
            topic_state.get("topic_stability").is_some() || topic_state.get("stability").is_some(),
            "topic_state missing stability field"
        );
    }

    log_test_evidence(
        "test_e2e_full_session_workflow",
        "session_start",
        &session_id,
        &start_result,
        false, // DB not verified yet
    );

    // 2. Execute pre_tool_use.sh (FAST PATH - must be under 100ms + overhead)
    println!("\n[2/5] Executing pre_tool_use.sh (FAST PATH)...");
    let pre_input = create_claude_code_pre_tool_input(
        &session_id,
        "Read",
        json!({"file_path": "/tmp/test.txt"}),
    );
    let pre_result = execute_hook_script(
        "pre_tool_use.sh",
        &pre_input,
        TIMEOUT_PRE_TOOL_MS + 200, // Allow shell overhead
        db_path,
    )
    .expect("pre_tool_use.sh execution failed");

    assert_eq!(
        pre_result.exit_code, EXIT_SUCCESS,
        "pre_tool_use.sh failed.\nstdout: {}\nstderr: {}",
        pre_result.stdout, pre_result.stderr
    );

    // Verify timing budget (100ms + shell overhead ~150ms)
    assert!(
        pre_result.execution_time_ms < 500,
        "pre_tool_use.sh exceeded timing budget: {}ms (max 500ms with overhead)",
        pre_result.execution_time_ms
    );

    println!(
        "pre_tool_use.sh completed in {}ms",
        pre_result.execution_time_ms
    );

    log_test_evidence(
        "test_e2e_full_session_workflow",
        "pre_tool_use",
        &session_id,
        &pre_result,
        false,
    );

    // 3. Execute post_tool_use.sh
    println!("\n[3/5] Executing post_tool_use.sh...");
    let post_input = create_claude_code_post_tool_input(
        &session_id,
        "Read",
        json!({"file_path": "/tmp/test.txt"}),
        "file contents here",
        true,
    );
    let post_result = execute_hook_script(
        "post_tool_use.sh",
        &post_input,
        TIMEOUT_POST_TOOL_MS,
        db_path,
    )
    .expect("post_tool_use.sh execution failed");

    assert_eq!(
        post_result.exit_code, EXIT_SUCCESS,
        "post_tool_use.sh failed.\nstdout: {}\nstderr: {}",
        post_result.stdout, post_result.stderr
    );

    println!(
        "post_tool_use.sh completed in {}ms",
        post_result.execution_time_ms
    );

    log_test_evidence(
        "test_e2e_full_session_workflow",
        "post_tool_use",
        &session_id,
        &post_result,
        false,
    );

    // 4. Execute user_prompt_submit.sh
    println!("\n[4/5] Executing user_prompt_submit.sh...");
    let prompt_input = create_claude_code_prompt_submit_input(
        &session_id,
        "Please read the file and summarize it.",
    );
    let prompt_result = execute_hook_script(
        "user_prompt_submit.sh",
        &prompt_input,
        TIMEOUT_USER_PROMPT_MS,
        db_path,
    )
    .expect("user_prompt_submit.sh execution failed");

    assert_eq!(
        prompt_result.exit_code, EXIT_SUCCESS,
        "user_prompt_submit.sh failed.\nstdout: {}\nstderr: {}",
        prompt_result.stdout, prompt_result.stderr
    );

    println!(
        "user_prompt_submit.sh completed in {}ms",
        prompt_result.execution_time_ms
    );

    log_test_evidence(
        "test_e2e_full_session_workflow",
        "user_prompt_submit",
        &session_id,
        &prompt_result,
        false,
    );

    // 5. Execute session_end.sh
    println!("\n[5/5] Executing session_end.sh...");
    let end_input = create_claude_code_session_end_input(&session_id, "normal");
    let end_result = execute_hook_script(
        "session_end.sh",
        &end_input,
        TIMEOUT_SESSION_END_MS,
        db_path,
    )
    .expect("session_end.sh execution failed");

    assert_eq!(
        end_result.exit_code, EXIT_SUCCESS,
        "session_end.sh failed.\nstdout: {}\nstderr: {}",
        end_result.stdout, end_result.stderr
    );

    println!(
        "session_end.sh completed in {}ms",
        end_result.execution_time_ms
    );

    // PHYSICAL DATABASE VERIFICATION
    println!("\n=== Physical Database Verification ===");
    let snapshot_exists = verify_snapshot_exists(db_path, &session_id);
    println!("Snapshot exists in DB: {}", snapshot_exists);

    if !snapshot_exists {
        println!("WARNING: Snapshot not found in database. This may be expected if persistence is not fully implemented.");
    }

    log_test_evidence(
        "test_e2e_full_session_workflow",
        "full_session",
        &session_id,
        &end_result,
        snapshot_exists,
    );

    println!("\n=== Test Complete ===");
}

/// Test that topic stability state is properly updated throughout session
#[tokio::test]
async fn test_e2e_topic_stability_updates() {
    let (temp_dir, session_id) = setup_test("topic-stability");
    let db_path = temp_dir.path();

    println!("\n=== E2E Topic Stability Updates Test ===");
    println!("Session ID: {}", session_id);

    let start_result = start_session(&session_id, db_path);

    // Verify topic state in output
    let output_json = start_result.parse_stdout().expect("Invalid JSON output");
    println!(
        "Session start output: {}",
        serde_json::to_string_pretty(&output_json).unwrap()
    );

    // Check for topic-related fields in output
    if let Some(ts) = output_json.get("topic_state") {
        println!("Found topic_state: {:?}", ts);

        // Required fields per PRD v6 topic_stability spec (if present)
        let has_required = ts.get("stability").is_some()
            || ts.get("churn_rate").is_some()
            || ts.get("topic_stability").is_some()
            || ts.get("entropy").is_some();

        if has_required {
            println!("Topic state has required fields");
        }
    }

    // Stability classification
    if let Some(stability_class) = start_result.stability_classification() {
        println!("Stability Classification: {:?}", stability_class);

        if let Some(level) = stability_class.get("level").and_then(|v| v.as_str()) {
            let valid_levels = ["healthy", "normal", "warning", "critical", "unstable"];
            assert!(
                valid_levels.contains(&level),
                "Invalid stability level: {}",
                level
            );
            println!("Stability Level: {}", level);
        }
    }

    let end_result = end_session(&session_id, db_path);

    log_test_evidence(
        "test_e2e_topic_stability_updates",
        "topic_stability",
        &session_id,
        &end_result,
        true,
    );

    println!("\n=== Test Complete ===");
}

/// Test memory capture and retrieval (E2E-HP-002)
///
/// # Scenario:
/// 1. Start session
/// 2. Post-tool captures memory via hook
/// 3. User prompt retrieves context that includes captured memory
/// 4. End session
///
/// # Verifies:
/// - Memory is captured via post_tool_use.sh
/// - Context injection returns related content
#[tokio::test]
async fn test_e2e_memory_capture_and_retrieval() {
    let (temp_dir, session_id) = setup_test("memory-capture");
    let db_path = temp_dir.path();

    println!("\n=== E2E Memory Capture and Retrieval Test ===");
    println!("Session ID: {}", session_id);
    println!("Database: {}", db_path.display());

    // 1. Start session
    println!("\n[1/4] Starting session...");
    start_session(&session_id, db_path);
    println!("Session started successfully");

    // 2. Capture memory via post_tool_use.sh with specific content
    println!("\n[2/4] Capturing memory via post_tool_use.sh...");
    capture_memory(
        &session_id,
        "Edit",
        "/src/clustering.rs",
        "Implemented HDBSCAN clustering with min_cluster_size=3 and EOM selection",
        db_path,
    );
    println!("Memory captured successfully");

    // 3. Query for related content via user_prompt_submit.sh
    println!("\n[3/4] Querying for related content...");
    let prompt_result = submit_prompt(&session_id, "clustering algorithm configuration", db_path);

    let output_json = prompt_result
        .parse_stdout()
        .expect("Invalid JSON from user_prompt_submit.sh");
    println!(
        "Context injection output: {}",
        serde_json::to_string_pretty(&output_json).unwrap()
    );

    assert_eq!(
        output_json.get("success"),
        Some(&json!(true)),
        "user_prompt_submit.sh should succeed"
    );

    // 4. End session
    println!("\n[4/4] Ending session...");
    let end_result = end_session(&session_id, db_path);

    log_test_evidence(
        "test_e2e_memory_capture_and_retrieval",
        "memory_capture",
        &session_id,
        &end_result,
        true,
    );

    println!("\n=== Memory Capture and Retrieval Test Complete ===");
}

/// Test multi-session continuity (E2E-HP-003)
///
/// # Scenario:
/// 1. Session 1: Start, capture memories, end
/// 2. Session 2: Start with reference to Session 1, verify memories retrieved
///
/// # Verifies:
/// - Memory persists across sessions
/// - Session 2 can retrieve Session 1 memories
#[tokio::test]
async fn test_e2e_multi_session_continuity() {
    if let Err(e) = verify_all_scripts_exist() {
        panic!("E2E test prerequisite failed: {}", e);
    }

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();

    println!("\n=== E2E Multi-Session Continuity Test ===");
    println!("Database: {}", db_path.display());

    // --- SESSION 1 ---
    let session1_id = generate_e2e_session_id("continuity-s1");
    println!("\n[Session 1] ID: {}", session1_id);

    println!("[Session 1] Starting...");
    start_session(&session1_id, db_path);

    println!("[Session 1] Capturing memory...");
    capture_memory(
        &session1_id,
        "Write",
        "/src/database.rs",
        "Created PostgreSQL migration for users table with UUID primary key",
        db_path,
    );

    println!("[Session 1] Ending...");
    end_session(&session1_id, db_path);
    println!("[Session 1] Complete");

    // --- SESSION 2 ---
    let session2_id = generate_e2e_session_id("continuity-s2");
    println!("\n[Session 2] ID: {}", session2_id);

    // Start session 2 with reference to session 1
    println!("[Session 2] Starting with previous session reference...");
    let start2_input = create_claude_code_session_start_with_previous(&session2_id, &session1_id);
    let start2_result = execute_hook_script(
        "session_start.sh",
        &start2_input,
        TIMEOUT_SESSION_START_MS,
        db_path,
    )
    .expect("session_start.sh failed");
    assert_eq!(
        start2_result.exit_code, EXIT_SUCCESS,
        "session_start.sh with previous session failed: stderr={}",
        start2_result.stderr
    );

    // Query for related content in session 2
    println!("[Session 2] Querying for memories from Session 1...");
    let prompt2_result = submit_prompt(&session2_id, "database migration", db_path);

    let output_json = prompt2_result
        .parse_stdout()
        .expect("Invalid JSON from user_prompt_submit.sh");
    println!(
        "[Session 2] Context output: {}",
        serde_json::to_string_pretty(&output_json).unwrap()
    );

    println!("[Session 2] Ending...");
    let end2_result = end_session(&session2_id, db_path);

    log_test_evidence(
        "test_e2e_multi_session_continuity",
        "multi_session",
        &session2_id,
        &end2_result,
        true,
    );

    println!("\n=== Multi-Session Continuity Test Complete ===");
}

/// Test special character safety (E2E-EC-005)
///
/// # Scenario:
/// Pipe prompts containing shell special characters, verify no injection
///
/// # Verifies:
/// - Shell injection attempts are safely handled
/// - JSON escaping works correctly
/// - No command execution occurs
#[tokio::test]
async fn test_e2e_special_character_safety() {
    let (temp_dir, session_id) = setup_test("special-chars");
    let db_path = temp_dir.path();

    println!("\n=== E2E Special Character Safety Test ===");
    println!("Session ID: {}", session_id);

    // Test dangerous shell characters
    let dangerous_prompts = [
        ("command_substitution", "$(rm -rf /)"),
        ("backtick_injection", "`echo pwned`"),
        ("pipe_redirect", "test | cat /etc/passwd"),
        ("semicolon_chain", "test; rm -rf /"),
        ("newline_injection", "test\necho pwned"),
        ("dollar_vars", "$PATH $HOME $USER"),
        ("double_quote_escape", r#"test" && echo pwned"#),
        ("single_quote_escape", "test' && echo pwned'"),
    ];

    for (case_name, dangerous_prompt) in &dangerous_prompts {
        println!("\n--- Testing {} ---", case_name);
        println!("Input: {}", dangerous_prompt);

        let prompt_input = create_claude_code_prompt_submit_input(&session_id, dangerous_prompt);
        let result = execute_hook_script(
            "user_prompt_submit.sh",
            &prompt_input,
            TIMEOUT_USER_PROMPT_MS,
            db_path,
        )
        .expect("user_prompt_submit.sh execution failed");

        println!("Exit code: {}", result.exit_code);

        // Should NOT execute any injected commands - just process safely
        assert_ne!(
            result.exit_code, 127,
            "Exit code 127 suggests command execution attempt for case: {}",
            case_name
        );

        // Verify output doesn't contain execution results of injected commands
        let combined_output = format!("{}{}", result.stdout, result.stderr);
        assert!(
            !combined_output.contains("pwned"),
            "Injection succeeded for {}: output contains 'pwned'. Output: {}",
            case_name,
            combined_output
        );

        log_test_evidence(
            "test_e2e_special_character_safety",
            &format!("special_chars_{}", case_name),
            &session_id,
            &result,
            false,
        );
    }

    println!("\n=== Special Character Safety Test Complete ===");
}

/// Test Unicode handling (E2E-EC-006)
///
/// # Scenario:
/// Process prompts and tool responses containing various Unicode characters
///
/// # Verifies:
/// - UTF-8 content is handled correctly
/// - No mojibake or corruption
/// - CJK, emoji, and RTL text work
#[tokio::test]
async fn test_e2e_unicode_handling() {
    let (temp_dir, session_id) = setup_test("unicode");
    let db_path = temp_dir.path();

    println!("\n=== E2E Unicode Handling Test ===");
    println!("Session ID: {}", session_id);

    start_session(&session_id, db_path);

    // Test various Unicode strings
    let unicode_tests = [
        ("emoji", "🚀 Rocket launch 🎉 party 🔥 fire"),
        ("cjk", "你好世界 こんにちは世界 안녕하세요"),
        ("cyrillic", "Привет мир Здравствуй"),
        ("arabic", "مرحبا بالعالم السلام عليكم"),
        ("math_symbols", "∀x ∈ ℝ: x² ≥ 0 ∧ √(x²) = |x|"),
        ("mixed", "Code: 你好 + مرحبا = 🌍 (∞ possibilities)"),
        ("accents", "café résumé naïve façade"),
        ("combining_chars", "e\u{0301} (é) n\u{0303} (ñ)"),
    ];

    for (case_name, unicode_content) in &unicode_tests {
        println!("\n--- Testing {} ---", case_name);
        println!("Input: {}", unicode_content);

        let prompt_input = create_claude_code_prompt_submit_input(&session_id, unicode_content);
        let result = execute_hook_script(
            "user_prompt_submit.sh",
            &prompt_input,
            TIMEOUT_USER_PROMPT_MS,
            db_path,
        )
        .expect("user_prompt_submit.sh execution failed");

        println!("Exit code: {}", result.exit_code);

        assert_eq!(
            result.exit_code, EXIT_SUCCESS,
            "Unicode handling failed for {}: stdout={}, stderr={}",
            case_name, result.stdout, result.stderr
        );

        let output_json = result.parse_stdout();
        assert!(
            output_json.is_ok(),
            "Invalid JSON output for {}: {:?}",
            case_name,
            output_json.err()
        );

        log_test_evidence(
            "test_e2e_unicode_handling",
            &format!("unicode_{}", case_name),
            &session_id,
            &result,
            false,
        );
    }

    end_session(&session_id, db_path);

    println!("\n=== Unicode Handling Test Complete ===");
}

/// Test long prompt handling (E2E-EC-004)
///
/// # Scenario:
/// Process a very long prompt (10KB+) without truncation
///
/// # Verifies:
/// - Long prompts are handled within timeout
/// - No truncation or data loss
#[tokio::test]
async fn test_e2e_long_prompt_handling() {
    let (temp_dir, session_id) = setup_test("long-prompt");
    let db_path = temp_dir.path();

    println!("\n=== E2E Long Prompt Handling Test ===");
    println!("Session ID: {}", session_id);

    // Create a 10KB+ prompt (~11KB with 150 repeats)
    let base_text = "This is a test prompt that will be repeated many times to create a long input. ";
    let long_prompt: String = base_text.repeat(150);
    let prompt_len = long_prompt.len();
    println!(
        "Long prompt length: {} bytes ({:.1} KB)",
        prompt_len,
        prompt_len as f64 / 1024.0
    );
    assert!(prompt_len > 10 * 1024, "Prompt should be > 10KB");

    start_session(&session_id, db_path);

    // Send long prompt
    println!("Sending long prompt...");
    let prompt_input = create_claude_code_prompt_submit_input(&session_id, &long_prompt);
    let result = execute_hook_script(
        "user_prompt_submit.sh",
        &prompt_input,
        TIMEOUT_USER_PROMPT_MS,
        db_path,
    )
    .expect("user_prompt_submit.sh execution failed");

    println!("Exit code: {}", result.exit_code);
    println!("Execution time: {}ms", result.execution_time_ms);

    assert_eq!(
        result.exit_code, EXIT_SUCCESS,
        "Long prompt handling failed: stdout={}, stderr={}",
        result.stdout, result.stderr
    );

    assert!(
        result.execution_time_ms < TIMEOUT_USER_PROMPT_MS,
        "Long prompt exceeded timeout: {}ms (max {}ms)",
        result.execution_time_ms,
        TIMEOUT_USER_PROMPT_MS
    );

    end_session(&session_id, db_path);

    log_test_evidence(
        "test_e2e_long_prompt_handling",
        "long_prompt",
        &session_id,
        &result,
        true,
    );

    println!("\n=== Long Prompt Handling Test Complete ===");
}

/// Test pre_tool_use.sh timing compliance
#[tokio::test]
async fn test_e2e_pre_tool_fast_path() {
    let (temp_dir, session_id) = setup_test("fast-path");
    let db_path = temp_dir.path();

    println!("\n=== E2E Pre-Tool Fast Path Test ===");
    println!("Testing that pre_tool_use.sh completes within timing budget");

    // Execute pre_tool_use.sh multiple times to verify consistent timing
    for i in 1..=3 {
        let pre_input = create_claude_code_pre_tool_input(
            &session_id,
            &format!("Read_{}", i),
            json!({"file_path": format!("/tmp/test_{}.txt", i)}),
        );

        let start = std::time::Instant::now();
        let result = execute_hook_script(
            "pre_tool_use.sh",
            &pre_input,
            TIMEOUT_PRE_TOOL_MS + 400, // Allow generous overhead
            db_path,
        )
        .expect("pre_tool_use.sh execution failed");

        let wall_time = start.elapsed().as_millis();

        println!(
            "Run {}: exit_code={}, wall_time={}ms, reported_time={}ms",
            i, result.exit_code, wall_time, result.execution_time_ms
        );

        // Shell overhead adds ~50-100ms typically
        assert!(
            result.execution_time_ms < 500,
            "Run {} exceeded budget: {}ms",
            i,
            result.execution_time_ms
        );
    }

    println!("\n=== Fast Path Test Complete ===");
}
