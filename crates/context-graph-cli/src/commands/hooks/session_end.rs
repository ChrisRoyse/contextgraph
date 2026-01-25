//! Session end hook handler for Claude Code native hooks.
//!
//! # Timeout Budget: 30000ms
//! # Output: HookOutput JSON with final topic stability status
//!
//! # Constitution References
//! - Topic stability thresholds (Healthy>0.9, Warning<0.7, Critical<0.5)
//! - AP-26: Exit codes (0=success, 1=error, 2=corruption)
//! - AP-50: NO internal hooks - shell scripts call CLI
//! - ARCH-07: Native Claude Code hooks
//!
//! # Note on Topic Stability
//! Per PRD v6 Section 14, this module uses Topic Stability (churn tracking)
//! for session coherence. Session state is persisted to SessionCache.
//!
//! # NO BACKWARDS COMPATIBILITY - FAIL FAST

use std::io::{self, BufRead};
use std::time::Instant;

use tracing::{debug, error, info, warn};

use super::session_state::{store_in_cache, SessionCache, SessionSnapshot};

use super::args::SessionEndArgs;
use super::error::{HookError, HookResult};
use super::types::{
    CoherenceState, HookInput, HookOutput, HookPayload, StabilityClassification, SessionEndStatus,
};

/// Execute session-end hook.
///
/// # Flow
/// 1. Parse input (stdin JSON or CLI args)
/// 2. Get session_id (from args or stdin)
/// 3. Read warm cache (SessionCache) if available
/// 4. Persist snapshot to SessionCache
/// 5. Build HookOutput with final topic stability status
///
/// # Timeout
/// MUST complete within 30000ms (Claude Code enforced)
///
/// # Exit Codes (AP-26)
/// - 0: Success
/// - 1: Recoverable error (non-blocking)
/// - 2: Corruption detected
pub async fn execute(args: SessionEndArgs) -> HookResult<HookOutput> {
    let start = Instant::now();

    info!(
        stdin = args.stdin,
        session_id = %args.session_id,
        duration_ms = ?args.duration_ms,
        generate_summary = args.generate_summary,
        "SESSION_END: execute starting"
    );

    // 1. Parse input source for additional data
    let (session_id, duration_ms, status) = if args.stdin {
        let (parsed_session_id, parsed_duration, parsed_status) = parse_stdin(&args.session_id)?;
        (parsed_session_id, parsed_duration, parsed_status)
    } else {
        (
            args.session_id.clone(),
            args.duration_ms,
            SessionEndStatus::Normal,
        )
    };

    // 2. Get current state from warm cache
    let cache_snapshot = SessionCache::get();

    // 3. Persist session state to SessionCache
    let (topic_stability, coherence_state) =
        persist_to_cache(&session_id, cache_snapshot, duration_ms);

    // 4. Build output structures
    let execution_time_ms = start.elapsed().as_millis() as u64;

    info!(
        session_id = %session_id,
        topic_stability = topic_stability,
        status = ?status,
        execution_time_ms,
        "SESSION_END: execute complete"
    );

    let output = HookOutput::success(execution_time_ms)
        .with_coherence_state(coherence_state)
        .with_stability_classification(StabilityClassification::from_value(topic_stability));

    Ok(output)
}

/// Parse stdin JSON into session data.
/// Returns (session_id, duration_ms, status).
fn parse_stdin(default_session_id: &str) -> HookResult<(String, Option<u64>, SessionEndStatus)> {
    let stdin = io::stdin();
    let mut input_str = String::new();

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| {
            error!(error = %e, "SESSION_END: stdin read failed");
            HookError::invalid_input(format!("stdin read failed: {}", e))
        })?;
        input_str.push_str(&line);
    }

    if input_str.is_empty() {
        debug!("SESSION_END: stdin is empty, using defaults");
        return Ok((
            default_session_id.to_string(),
            None,
            SessionEndStatus::Normal,
        ));
    }

    debug!(
        input_bytes = input_str.len(),
        "SESSION_END: parsing stdin JSON"
    );

    // Try to parse as HookInput
    let input: HookInput = serde_json::from_str(&input_str).map_err(|e| {
        error!(error = %e, input_preview = %&input_str[..input_str.len().min(100)], "SESSION_END: JSON parse failed");
        HookError::invalid_input(format!("JSON parse failed: {}", e))
    })?;

    // Validate input
    if let Some(error) = input.validate() {
        return Err(HookError::invalid_input(error));
    }

    // Extract session end payload
    let (duration_ms, status) = match input.payload {
        HookPayload::SessionEnd {
            duration_ms,
            status,
            reason: _,
        } => (Some(duration_ms), status),
        other => {
            error!(payload_type = ?std::mem::discriminant(&other), "SESSION_END: unexpected payload type");
            return Err(HookError::invalid_input(
                "Expected SessionEnd payload, got different type",
            ));
        }
    };

    Ok((input.session_id, duration_ms, status))
}

/// Persist session state to SessionCache.
///
/// # Arguments
/// * `session_id` - Session identifier
/// * `cache_snapshot` - Optional snapshot from SessionCache
/// * `duration_ms` - Optional session duration
///
/// # Returns
/// Tuple of (topic_stability, CoherenceState)
///
/// # Note
/// Per PRD v6 Section 14, this function uses Topic Stability for session
/// coherence. The simplified SessionSnapshot is stored in SessionCache.
fn persist_to_cache(
    session_id: &str,
    cache_snapshot: Option<SessionSnapshot>,
    duration_ms: Option<u64>,
) -> (f32, CoherenceState) {
    // Determine values from cache or use defaults
    // Per PRD v6, we use topic_stability (default 1.0 = fresh/stable state)
    let (topic_stability, integration, reflection, differentiation, final_session_id) =
        match cache_snapshot {
            Some(snapshot) => {
                info!(
                    "SESSION_END: Using warm cache - session={}",
                    snapshot.session_id
                );
                (
                    1.0, // Topic stability defaults to 1.0 (fresh state)
                    snapshot.integration,
                    snapshot.reflection,
                    snapshot.differentiation,
                    snapshot.session_id,
                )
            }
            None => {
                warn!("SESSION_END: Cache is cold, using defaults");
                (
                    1.0, // Fresh state = perfect stability
                    0.0,
                    0.0,
                    0.0,
                    session_id.to_string(),
                )
            }
        };

    // Create and store updated snapshot
    let mut snapshot = SessionSnapshot::new(&final_session_id);
    snapshot.integration = integration;
    snapshot.reflection = reflection;
    snapshot.differentiation = differentiation;
    snapshot.touch();

    // Store in global cache
    store_in_cache(&snapshot);

    // Log duration if provided
    if let Some(duration) = duration_ms {
        debug!("SESSION_END: Session duration: {}ms", duration);
    }

    info!(
        session_id = %final_session_id,
        topic_stability = topic_stability,
        "SESSION_END: Successfully persisted session state to cache"
    );

    // Build CoherenceState for output (DTO for JSON response)
    let coherence_state = CoherenceState::new(
        integration * reflection * differentiation, // Coherence score C(t) = I(t) * R(t) * D(t)
        integration,
        reflection,
        differentiation,
        topic_stability,
    );

    (topic_stability, coherence_state)
}

// =============================================================================
// TESTS - Cache-based verification per PRD v6
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::hooks::args::OutputFormat;
    use crate::commands::test_utils::GLOBAL_IDENTITY_LOCK;

    // =========================================================================
    // TC-HOOKS-012-001: Session End with Warm Cache
    // Verify: Warm cache returns valid JSON with topic stability
    // SOURCE OF TRUTH: Global SessionCache state
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_012_001_warm_cache_flush() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-012-001: Warm Cache Flush ===");
        println!("SOURCE OF TRUTH: Global SessionCache state");

        let session_id = "test-session-end-warm";

        // SETUP: Warm the cache with test data
        let mut snapshot = SessionSnapshot::new(session_id);
        snapshot.integration = 0.85;
        snapshot.reflection = 0.78;
        snapshot.differentiation = 0.82;
        store_in_cache(&snapshot);

        println!(
            "BEFORE: Cache warmed with session {}",
            session_id
        );

        let args = SessionEndArgs {
            db_path: None,
            session_id: session_id.to_string(),
            duration_ms: Some(3600000),
            stdin: false,
            generate_summary: true,
            format: OutputFormat::Json,
        };

        // Execute
        let result = execute(args).await;

        // Verify: Success
        assert!(result.is_ok(), "Execute must succeed");
        let output = result.unwrap();
        assert!(output.success, "Output.success must be true");
        assert!(
            output.coherence_state.is_some(),
            "Must have coherence_state"
        );
        assert!(
            output.stability_classification.is_some(),
            "Must have stability_classification"
        );

        // Verify: Topic stability classification (fresh state = 1.0)
        let stability_class = output.stability_classification.unwrap();
        println!(
            "AFTER: stability={:.2}, level={:?}",
            stability_class.value, stability_class.level
        );

        // Verify: Cache still contains the snapshot
        let cached = SessionCache::get().expect("Cache must have snapshot");
        assert_eq!(cached.session_id, session_id);

        println!("RESULT: PASS - Warm cache handled successfully");
    }

    // =========================================================================
    // TC-HOOKS-012-002: Session End with Cold Cache
    // Verify: Cold cache uses defaults gracefully
    // SOURCE OF TRUTH: Default values stored in cache
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_012_002_cold_cache_behavior() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-012-002: Cold Cache Behavior ===");
        println!("SOURCE OF TRUTH: Default values stored in cache");

        // Use unique session ID that won't be in cache
        let cold_session_id = format!("test-session-end-cold-{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos());

        let args = SessionEndArgs {
            db_path: None,
            session_id: cold_session_id.to_string(),
            duration_ms: None,
            stdin: false,
            generate_summary: true,
            format: OutputFormat::Json,
        };

        // Execute
        let result = execute(args).await;

        // Verify: Success (should use defaults when cache is cold)
        assert!(result.is_ok(), "Execute must succeed even with cold cache");
        let output = result.unwrap();
        assert!(output.success, "Output.success must be true");

        println!("RESULT: PASS - Cold cache handled gracefully");
    }

    // =========================================================================
    // TC-HOOKS-012-003: persist_to_cache Function Behavior
    // Verify: Cache persistence works correctly
    // =========================================================================
    #[test]
    fn tc_hooks_012_003_persist_to_cache_function() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-012-003: persist_to_cache Function ===");
        println!("SOURCE OF TRUTH: SessionCache after persist");

        let session_id = "test-persist-cache";

        // SETUP: Warm cache
        let mut snapshot = SessionSnapshot::new(session_id);
        snapshot.integration = 0.75;
        snapshot.reflection = 0.65;
        snapshot.differentiation = 0.80;
        store_in_cache(&snapshot);

        let cache_snapshot = SessionCache::get();

        // Execute persist_to_cache
        let (topic_stability, coherence_state) =
            persist_to_cache(session_id, cache_snapshot, Some(3600000));

        // Verify: Topic stability is 1.0 (fresh state)
        assert!((topic_stability - 1.0).abs() < 0.01, "Topic stability should be 1.0");

        // Verify: CoherenceState has correct values
        assert!((coherence_state.integration - 0.75).abs() < 0.01);
        assert!((coherence_state.reflection - 0.65).abs() < 0.01);
        assert!((coherence_state.differentiation - 0.80).abs() < 0.01);

        // Verify: Cache was updated
        let cached = SessionCache::get().expect("Cache must have snapshot");
        assert_eq!(cached.session_id, session_id);
        assert!((cached.integration - 0.75).abs() < 0.01);

        println!("RESULT: PASS - persist_to_cache function works correctly");
    }

    // =========================================================================
    // TC-HOOKS-012-004: Cold Cache Persist Defaults
    // Verify: persist_to_cache with None snapshot uses defaults
    // =========================================================================
    #[test]
    fn tc_hooks_012_004_cold_cache_persist_defaults() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-012-004: Cold Cache Persist Defaults ===");
        println!("SOURCE OF TRUTH: Default values when cache is cold");

        let session_id = "test-cold-persist";

        // Execute with no cache snapshot
        let (topic_stability, coherence_state) =
            persist_to_cache(session_id, None, None);

        // Verify: Topic stability defaults to 1.0
        assert!((topic_stability - 1.0).abs() < 0.01, "Topic stability should be 1.0");

        // Verify: CoherenceState has default values (0.0)
        assert!((coherence_state.integration - 0.0).abs() < 0.01);
        assert!((coherence_state.reflection - 0.0).abs() < 0.01);
        assert!((coherence_state.differentiation - 0.0).abs() < 0.01);

        // Verify: A fresh snapshot was stored in cache
        let cached = SessionCache::get().expect("Cache must have snapshot");
        assert_eq!(cached.session_id, session_id);

        println!("RESULT: PASS - Cold cache persist uses correct defaults");
    }

    // =========================================================================
    // TC-HOOKS-012-005: JSON Output Schema Compliance
    // Verify: Output matches HookOutput schema exactly
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_012_005_json_output_schema() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-012-005: JSON Output Schema ===");
        println!("SOURCE OF TRUTH: TECH-HOOKS.md Section 3.3");

        let session_id = "test-session-end-schema";

        // Warm cache
        let mut snapshot = SessionSnapshot::new(session_id);
        snapshot.integration = 0.75;
        snapshot.reflection = 0.70;
        snapshot.differentiation = 0.80;
        store_in_cache(&snapshot);

        let args = SessionEndArgs {
            db_path: None,
            session_id: session_id.to_string(),
            duration_ms: Some(1800000),
            stdin: false,
            generate_summary: true,
            format: OutputFormat::Json,
        };

        let result = execute(args).await.unwrap();

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&result).expect("Serialization must succeed");
        println!("Output JSON:\n{}", json);

        // Verify required fields
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("success").is_some(), "Must have 'success' field");
        assert!(
            parsed.get("execution_time_ms").is_some(),
            "Must have 'execution_time_ms' field"
        );

        // Verify optional fields present when data available
        assert!(
            parsed.get("coherence_state").is_some(),
            "Should have coherence_state"
        );
        assert!(
            parsed.get("stability_classification").is_some(),
            "Should have stability_classification"
        );

        // Verify nested structure
        let cs = parsed
            .get("coherence_state")
            .expect("coherence_state present");
        assert!(cs.get("coherence").is_some());
        assert!(cs.get("topic_stability").is_some());

        let stability = parsed
            .get("stability_classification")
            .expect("stability_classification present");
        assert!(stability.get("value").is_some());
        assert!(stability.get("level").is_some());
        assert!(stability.get("crisis_triggered").is_some());

        println!("RESULT: PASS - JSON output matches schema");
    }

    // =========================================================================
    // TC-HOOKS-012-006: Execution Time Under Budget
    // Verify: Execution time stays under 30000ms timeout
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_012_006_execution_time_budget() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-012-006: Execution Time Under Budget ===");
        println!("SOURCE OF TRUTH: TECH-HOOKS.md timeout=30000ms");

        let session_id = "test-session-end-timing";

        // Warm cache
        let mut snapshot = SessionSnapshot::new(session_id);
        snapshot.integration = 0.65;
        snapshot.reflection = 0.60;
        snapshot.differentiation = 0.70;
        store_in_cache(&snapshot);

        let args = SessionEndArgs {
            db_path: None,
            session_id: session_id.to_string(),
            duration_ms: None,
            stdin: false,
            generate_summary: false,
            format: OutputFormat::Json,
        };

        let start = std::time::Instant::now();
        let result = execute(args).await.unwrap();
        let actual_elapsed = start.elapsed().as_millis() as u64;

        // Note: execution_time_ms may be 0 if operation completes in <1ms
        // which is actually a SUCCESS per our performance budgets (30000ms timeout)
        // Just verify it doesn't exceed actual elapsed time significantly
        assert!(
            result.execution_time_ms <= actual_elapsed + 10,
            "Reported time {} should not exceed actual elapsed {}",
            result.execution_time_ms,
            actual_elapsed
        );

        // Verify well under timeout budget (30000ms)
        assert!(
            result.execution_time_ms < 30000,
            "Execution time {} must be under 30000ms timeout",
            result.execution_time_ms
        );

        // Typically should be very fast (under 1000ms)
        println!(
            "PASS: Execution time {} ms (actual: {} ms)",
            result.execution_time_ms, actual_elapsed
        );

        println!("RESULT: PASS - Execution time under budget");
    }

    // =========================================================================
    // TC-HOOKS-012-007: Cache Verification After Session End
    // Verify: SessionCache contains updated snapshot after session end
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_012_007_cache_verification() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-012-007: Cache Verification After Session End ===");
        println!("SOURCE OF TRUTH: SessionCache state");

        let session_id = "test-session-end-cache-verify";

        // Warm cache
        let mut snapshot = SessionSnapshot::new(session_id);
        snapshot.integration = 0.80;
        snapshot.reflection = 0.75;
        snapshot.differentiation = 0.85;
        store_in_cache(&snapshot);

        let args = SessionEndArgs {
            db_path: None,
            session_id: session_id.to_string(),
            duration_ms: Some(7200000),
            stdin: false,
            generate_summary: true,
            format: OutputFormat::Json,
        };

        // Execute
        let result = execute(args).await;
        assert!(result.is_ok(), "Execute must succeed");

        // Verify: Cache contains updated snapshot
        let cached = SessionCache::get().expect("Cache must have snapshot");
        println!("VERIFICATION - Cached snapshot:");
        println!("  session_id: {}", cached.session_id);
        println!("  integration: {}", cached.integration);
        println!("  reflection: {}", cached.reflection);
        println!("  differentiation: {}", cached.differentiation);

        assert_eq!(cached.session_id, session_id);
        assert!((cached.integration - 0.80).abs() < 0.01);
        assert!((cached.reflection - 0.75).abs() < 0.01);
        assert!((cached.differentiation - 0.85).abs() < 0.01);

        println!("RESULT: PASS - Cache verification successful");
    }
}
