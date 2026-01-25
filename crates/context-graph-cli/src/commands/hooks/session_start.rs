//! Session start hook handler for Claude Code native hooks.
//!
//! # Timeout Budget: 5000ms
//! # Output: ~100 tokens coherence status
//!
//! # Constitution References
//! - Topic Stability: Per PRD v6 Section 14, used for session coherence
//! - AP-50: NO internal hooks - shell scripts call CLI
//!
//! # Note on Topic Stability
//! Per PRD v6 Section 14, Topic Stability (churn tracking) is used for
//! session coherence. See `clustering/stability.rs` for implementation.
//!
//! # NO BACKWARDS COMPATIBILITY - FAIL FAST

use std::io::{self, BufRead};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tracing::{debug, error, info, warn};

use super::session_state::{store_in_cache, SessionCache, SessionSnapshot, NUM_EMBEDDERS};

use super::args::SessionStartArgs;
use super::error::{HookError, HookResult};
use super::types::{
    CoherenceState, DriftMetrics, HookInput, HookOutput, HookPayload, StabilityClassification,
};

/// Execute session-start hook.
///
/// # Flow
/// 1. Parse input (stdin JSON or CLI args)
/// 2. Load or create SessionSnapshot from cache
/// 3. Link to previous session if provided (from cache)
/// 4. Build CoherenceState and StabilityClassification
/// 5. Return HookOutput as JSON to stdout
///
/// # Note on Storage
/// Per PRD v6 Section 14, session identity is managed via the in-memory
/// `SessionCache` singleton. Database persistence was removed to simplify
/// the architecture.
///
/// # Timeout
/// MUST complete within 5000ms (Claude Code enforced)
///
/// # Exit Codes
/// - 0: Success
/// - 2: Timeout
/// - 4: Invalid input
pub async fn execute(args: SessionStartArgs) -> HookResult<HookOutput> {
    let start = Instant::now();

    info!(
        stdin = args.stdin,
        session_id = ?args.session_id,
        previous_session_id = ?args.previous_session_id,
        "SESSION_START: execute starting"
    );

    // 1. Parse input source
    let (session_id, previous_session_id) = if args.stdin {
        let input = parse_stdin()?;
        extract_session_ids(&input)?
    } else {
        (args.session_id, args.previous_session_id)
    };

    // 2. Generate session_id if not provided
    let session_id = session_id.unwrap_or_else(|| {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let id = format!("session-{}", timestamp_ms);
        info!(generated_session_id = %id, "SESSION_START: generated new session_id");
        id
    });

    // 3. Load or create snapshot from cache
    let (snapshot, drift_metrics) =
        load_or_create_snapshot(&session_id, previous_session_id.as_deref());

    // 4. Build output structures
    let coherence_state = build_coherence_state(&snapshot);
    // Use average of integration, reflection, differentiation as stability proxy
    let stability_value = (snapshot.integration + snapshot.reflection + snapshot.differentiation) / 3.0;
    let stability_classification = StabilityClassification::from_value(stability_value);

    // 5. Build final output
    let execution_time_ms = start.elapsed().as_millis() as u64;

    info!(
        session_id = %session_id,
        stability = stability_value,
        drift_metrics = ?drift_metrics,
        execution_time_ms,
        "SESSION_START: execute complete"
    );

    // 6. Return output with drift metrics if available
    let mut output = HookOutput::success(execution_time_ms)
        .with_coherence_state(coherence_state)
        .with_stability_classification(stability_classification);

    if let Some(metrics) = drift_metrics {
        output = output.with_drift_metrics(metrics);
    }

    Ok(output)
}

/// Parse stdin JSON into HookInput.
/// FAIL FAST on empty or malformed input.
fn parse_stdin() -> HookResult<HookInput> {
    let stdin = io::stdin();
    let mut input_str = String::new();

    for line in stdin.lock().lines() {
        let line = line.map_err(|e| {
            error!(error = %e, "SESSION_START: stdin read failed");
            HookError::invalid_input(format!("stdin read failed: {}", e))
        })?;
        input_str.push_str(&line);
    }

    if input_str.is_empty() {
        error!("SESSION_START: stdin is empty");
        return Err(HookError::invalid_input("stdin is empty - expected JSON"));
    }

    debug!(
        input_bytes = input_str.len(),
        "SESSION_START: parsing stdin JSON"
    );

    serde_json::from_str(&input_str).map_err(|e| {
        error!(error = %e, input_preview = %&input_str[..input_str.len().min(100)], "SESSION_START: JSON parse failed");
        HookError::invalid_input(format!("JSON parse failed: {}", e))
    })
}

/// Extract session IDs from HookInput payload.
fn extract_session_ids(input: &HookInput) -> HookResult<(Option<String>, Option<String>)> {
    // Validate input
    if let Some(error) = input.validate() {
        return Err(HookError::invalid_input(error));
    }

    let session_id = Some(input.session_id.clone());

    let previous_session_id = match &input.payload {
        HookPayload::SessionStart {
            previous_session_id,
            ..
        } => previous_session_id.clone(),
        other => {
            error!(payload_type = ?std::mem::discriminant(other), "SESSION_START: unexpected payload type");
            return Err(HookError::invalid_input(
                "Expected SessionStart payload, got different type",
            ));
        }
    };

    Ok((session_id, previous_session_id))
}

/// Load existing snapshot or create new one.
/// Links to previous session if provided and computes drift metrics.
///
/// # Returns
/// Tuple of (SessionSnapshot, Option<DriftMetrics>)
/// - DriftMetrics is Some when linking to a previous session succeeded
/// - DriftMetrics is None for new sessions or when previous session not found
///
/// # Note on Topic Stability
/// Per PRD v6 Section 14, Topic Stability (churn tracking) is used for
/// session coherence. Drift metrics use integration, reflection, and
/// differentiation values.
///
/// # Note on Storage
/// Per PRD v6, session state uses the in-memory SessionCache singleton.
/// Database persistence was removed to simplify the architecture.
fn load_or_create_snapshot(
    session_id: &str,
    previous_session_id: Option<&str>,
) -> (SessionSnapshot, Option<DriftMetrics>) {
    // Try to load existing snapshot from cache
    if let Some(cached) = SessionCache::get() {
        if cached.session_id == session_id {
            info!(session_id = %session_id, "SESSION_START: loaded existing snapshot from cache");
            // No drift metrics for resumed sessions (same session, not linked)
            return (cached, None);
        }
    }

    debug!(session_id = %session_id, "SESSION_START: no existing snapshot in cache, creating new");

    // Create new snapshot
    let mut snapshot = SessionSnapshot::new(session_id);
    let mut drift_metrics: Option<DriftMetrics> = None;

    // Link to previous session if provided (check cache)
    if let Some(prev_id) = previous_session_id {
        // Try to get previous session from cache
        if let Some(cached) = SessionCache::get() {
            if cached.session_id == prev_id {
                let prev_stability = (cached.integration + cached.reflection + cached.differentiation) / 3.0;
                info!(
                    session_id = %session_id,
                    previous_session_id = %prev_id,
                    previous_stability = prev_stability,
                    "SESSION_START: linking to previous session from cache"
                );

                // Restore identity state from previous session
                // Per PRD v6, we copy topic_profile and metrics
                snapshot.topic_profile = cached.topic_profile;
                snapshot.integration = cached.integration;
                snapshot.reflection = cached.reflection;
                snapshot.differentiation = cached.differentiation;

                // Copy trajectory (up to MAX_TRAJECTORY_SIZE)
                for pv in cached.trajectory.iter() {
                    snapshot.append_to_trajectory(*pv);
                }

                snapshot.previous_session_id = Some(prev_id.to_string());

                let restored_stability = (snapshot.integration + snapshot.reflection + snapshot.differentiation) / 3.0;
                info!(
                    session_id = %session_id,
                    restored_stability = restored_stability,
                    "SESSION_START: identity state restored from previous session"
                );

                // Compute drift metrics when linking sessions
                drift_metrics = Some(compute_drift_metrics(&snapshot, &cached));
            } else {
                warn!(
                    previous_session_id = %prev_id,
                    "SESSION_START: previous session not found in cache, starting fresh"
                );
            }
        } else {
            warn!(
                previous_session_id = %prev_id,
                "SESSION_START: cache is cold, cannot link to previous session"
            );
        }
    }

    // Update the global cache with the new snapshot
    store_in_cache(&snapshot);

    let stability = (snapshot.integration + snapshot.reflection + snapshot.differentiation) / 3.0;
    info!(
        session_id = %session_id,
        stability = stability,
        drift_detected = drift_metrics.is_some(),
        "SESSION_START: created new snapshot and stored in cache"
    );

    (snapshot, drift_metrics)
}

/// Build CoherenceState from snapshot.
///
/// # Note on Topic Stability
/// Per PRD v6 Section 14, Topic Stability is used for session coherence.
/// The topic_stability field is computed from the average of integration,
/// reflection, and differentiation.
fn build_coherence_state(snapshot: &SessionSnapshot) -> CoherenceState {
    // Compute coherence level as average of metrics
    let coherence_level = (snapshot.integration + snapshot.reflection + snapshot.differentiation) / 3.0;
    CoherenceState::new(
        coherence_level, // coherence derived from integration metrics
        snapshot.integration,
        snapshot.reflection,
        snapshot.differentiation,
        coherence_level, // topic_stability uses same coherence measure
    )
}

// =============================================================================
// Drift Metrics Computation (TASK-HOOKS-013)
// =============================================================================

/// Compute cosine distance between two topic profiles.
///
/// # Arguments
/// * `current` - Current session's topic profile [NUM_EMBEDDERS]
/// * `previous` - Previous session's topic profile [NUM_EMBEDDERS]
///
/// # Returns
/// Cosine distance in range [0.0, 2.0] where:
/// - 0.0 = identical vectors (perfectly aligned)
/// - 1.0 = orthogonal vectors
/// - 2.0 = opposite vectors (completely misaligned)
///
/// # Formula
/// `distance = 1 - cosine_similarity`
/// `cosine_similarity = (a · b) / (||a|| * ||b||)`
///
/// # Edge Cases
/// - Returns 0.0 if either vector has zero magnitude (identical = no drift)
///
/// # Example
/// ```ignore
/// let v1 = [0.5; 13];
/// let v2 = [0.5; 13];
/// assert_eq!(cosine_distance(&v1, &v2), 0.0); // Identical
/// ```
fn cosine_distance(current: &[f32; NUM_EMBEDDERS], previous: &[f32; NUM_EMBEDDERS]) -> f32 {
    // Compute dot product and magnitudes
    let mut dot_product: f64 = 0.0;
    let mut mag_current: f64 = 0.0;
    let mut mag_previous: f64 = 0.0;

    for i in 0..NUM_EMBEDDERS {
        let c = current[i] as f64;
        let p = previous[i] as f64;
        dot_product += c * p;
        mag_current += c * c;
        mag_previous += p * p;
    }

    mag_current = mag_current.sqrt();
    mag_previous = mag_previous.sqrt();

    // Handle zero magnitude edge case
    if mag_current < f64::EPSILON || mag_previous < f64::EPSILON {
        return 0.0; // No drift if either vector is zero
    }

    // Cosine similarity in range [-1, 1]
    let cosine_similarity = dot_product / (mag_current * mag_previous);

    // Clamp to handle floating point precision issues
    let cosine_similarity = cosine_similarity.clamp(-1.0, 1.0);

    // Convert to distance: 1 - similarity gives range [0, 2]
    let distance = 1.0 - cosine_similarity;

    distance as f32
}


/// Compute comprehensive drift metrics between current and previous session snapshots.
///
/// # Arguments
/// * `current` - Current session's identity snapshot
/// * `previous` - Previous session's identity snapshot
///
/// # Returns
/// DriftMetrics containing:
/// - stability_delta: Change in stability (current - previous coherence)
/// - purpose_drift: Cosine distance between topic profiles [0.0, 2.0]
/// - time_since_snapshot_ms: Time elapsed since previous snapshot
/// - coherence_phase_drift: Mean absolute change in metrics
///
/// # Note on Topic Stability
/// Per PRD v6 Section 14, stability delta measures change in the average
/// of integration, reflection, and differentiation.
fn compute_drift_metrics(
    current: &SessionSnapshot,
    previous: &SessionSnapshot,
) -> DriftMetrics {
    // Compute stability as average of integration, reflection, differentiation
    let current_stability = (current.integration + current.reflection + current.differentiation) / 3.0;
    let previous_stability = (previous.integration + previous.reflection + previous.differentiation) / 3.0;

    // Stability delta: positive = improvement, negative = degradation
    let stability_delta = current_stability - previous_stability;

    // Purpose vector drift (cosine distance)
    let purpose_drift = cosine_distance(&current.topic_profile, &previous.topic_profile);

    // Time since previous snapshot
    let time_since_snapshot_ms = (current.timestamp_ms as i64) - (previous.timestamp_ms as i64);

    // Compute coherence drift from metrics differences
    let integration_diff = (current.integration - previous.integration).abs();
    let reflection_diff = (current.reflection - previous.reflection).abs();
    let differentiation_diff = (current.differentiation - previous.differentiation).abs();
    let coherence_phase_drift = ((integration_diff + reflection_diff + differentiation_diff) / 3.0) as f64;

    let metrics = DriftMetrics {
        stability_delta,
        purpose_drift,
        time_since_snapshot_ms,
        coherence_phase_drift,
    };

    // Log drift detection results
    if metrics.is_crisis_drift() {
        warn!(
            stability_delta = stability_delta,
            purpose_drift = purpose_drift,
            coherence_phase_drift = coherence_phase_drift,
            time_since_ms = time_since_snapshot_ms,
            "SESSION_START: CRISIS DRIFT DETECTED"
        );
    } else if metrics.is_warning_drift() {
        info!(
            stability_delta = stability_delta,
            purpose_drift = purpose_drift,
            "SESSION_START: Warning drift detected"
        );
    } else {
        debug!(
            stability_delta = stability_delta,
            purpose_drift = purpose_drift,
            "SESSION_START: Normal drift metrics"
        );
    }

    metrics
}

// =============================================================================
// TESTS - Cache-based verification per PRD v6 Section 14
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::hooks::args::OutputFormat;
    use crate::commands::test_utils::GLOBAL_IDENTITY_LOCK;

    // =========================================================================
    // TC-HOOKS-006-001: New Session Creation
    // Verify: New snapshot created, stored in cache, output valid
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_006_001_new_session_creation() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-006-001: New Session Creation ===");

        let session_id = "test-session-001";

        let args = SessionStartArgs {
            db_path: None,
            session_id: Some(session_id.to_string()),
            previous_session_id: None,
            stdin: false,
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

        // Verify: Data stored in cache
        assert!(SessionCache::is_warm(), "Cache must be warm");
        let cached = SessionCache::get().expect("Cache must have snapshot");
        assert_eq!(cached.session_id, session_id, "Session ID must match");

        println!("PASS: New session created and stored in cache");
    }

    // =========================================================================
    // TC-HOOKS-006-002: Session Linking (Previous Session from cache)
    // Verify: identity state restored when previous session is in cache
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_006_002_session_linking() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-006-002: Session Linking ===");

        let prev_id = "previous-session";
        let new_id = "new-session";

        // Setup: Create previous session with known identity state in cache
        let prev_purpose = [0.5_f32; 13];
        let prev_integration = 0.75;
        let prev_reflection = 0.65;
        let prev_differentiation = 0.55;
        {
            let mut prev_snapshot = SessionSnapshot::new(prev_id);
            prev_snapshot.topic_profile = prev_purpose;
            prev_snapshot.integration = prev_integration;
            prev_snapshot.reflection = prev_reflection;
            prev_snapshot.differentiation = prev_differentiation;
            store_in_cache(&prev_snapshot);
        }

        // Execute: Create new session linked to previous
        let args = SessionStartArgs {
            db_path: None,
            session_id: Some(new_id.to_string()),
            previous_session_id: Some(prev_id.to_string()),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;
        assert!(result.is_ok(), "Execute must succeed");

        // Verify: Session stored in cache with identity state restored
        let cached = SessionCache::get().expect("Cache must have snapshot");
        // Note: Cache stores the NEW session, which has identity state restored from previous
        // The new session ID is now current in cache
        assert_eq!(cached.session_id, new_id, "New session ID must be in cache");

        // Verify identity state is restored from previous session
        assert_eq!(
            cached.topic_profile, prev_purpose,
            "topic_profile MUST be restored from previous session"
        );
        assert!(
            (cached.integration - prev_integration).abs() < 0.001,
            "integration MUST be restored from previous session"
        );
        assert!(
            (cached.reflection - prev_reflection).abs() < 0.001,
            "reflection MUST be restored from previous session"
        );
        assert!(
            (cached.differentiation - prev_differentiation).abs() < 0.001,
            "differentiation MUST be restored from previous session"
        );

        println!("PASS: Session linked to previous with identity state restored");
        println!(
            "  topic_profile restored: {:?}",
            &cached.topic_profile[0..3]
        );
        println!("  integration: {}", cached.integration);
        println!("  reflection: {}", cached.reflection);
        println!("  differentiation: {}", cached.differentiation);
    }

    // =========================================================================
    // TC-HOOKS-006-003: Auto-Generate Session ID
    // Verify: Session ID generated when not provided
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_006_003_auto_generate_session_id() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-006-003: Auto-Generate Session ID ===");

        let args = SessionStartArgs {
            db_path: None,
            session_id: None, // Should auto-generate
            previous_session_id: None,
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;
        assert!(result.is_ok(), "Execute must succeed with generated ID");

        // Verify: Session exists in cache with generated ID
        assert!(SessionCache::is_warm(), "Cache must be warm");
        let cached = SessionCache::get().expect("Cache must have snapshot");
        assert!(
            cached.session_id.starts_with("session-"),
            "Generated ID should have prefix"
        );

        println!("PASS: Session ID auto-generated: {}", cached.session_id);
    }

    // =========================================================================
    // TC-HOOKS-006-004: Missing Previous Session (Graceful)
    // Verify: When previous_session_id doesn't exist, continue without linking
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_006_004_missing_previous_session() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-006-004: Missing Previous Session ===");

        let args = SessionStartArgs {
            db_path: None,
            session_id: Some("new-session".to_string()),
            previous_session_id: Some("nonexistent-session".to_string()),
            stdin: false,
            format: OutputFormat::Json,
        };

        // Should succeed despite missing previous session
        let result = execute(args).await;
        assert!(
            result.is_ok(),
            "Execute must succeed despite missing previous"
        );

        // Verify: New session created and stored in cache
        let cached = SessionCache::get().expect("Cache must have snapshot");
        assert_eq!(cached.session_id, "new-session", "Session ID must match");
        // Note: We log a warning but don't fail

        println!("PASS: Handled missing previous session gracefully");
    }

    // =========================================================================
    // TC-HOOKS-006-006: JSON Output Schema Compliance
    // Verify: Output matches HookOutput schema exactly
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_006_006_json_output_schema() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-006-006: JSON Output Schema ===");

        let args = SessionStartArgs {
            db_path: None,
            session_id: Some("schema-test".to_string()),
            previous_session_id: None,
            stdin: false,
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

        println!("PASS: JSON output matches schema");
    }

    // =========================================================================
    // TC-HOOKS-006-007: Execution Time Tracking
    // Verify: execution_time_ms reflects actual elapsed time
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_006_007_execution_time_tracking() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-006-007: Execution Time Tracking ===");

        let args = SessionStartArgs {
            db_path: None,
            session_id: Some("timing-test".to_string()),
            previous_session_id: None,
            stdin: false,
            format: OutputFormat::Json,
        };

        let start = std::time::Instant::now();
        let result = execute(args).await.unwrap();
        let actual_elapsed = start.elapsed().as_millis() as u64;

        // Note: execution_time_ms may be 0 if operation completes in <1ms
        // which is actually a SUCCESS per our performance budgets
        assert!(
            result.execution_time_ms <= actual_elapsed + 10, // Allow small margin
            "Reported time {} should not exceed actual elapsed {}",
            result.execution_time_ms,
            actual_elapsed
        );

        // Verify within timeout budget (5000ms)
        assert!(
            result.execution_time_ms < 5000,
            "Execution time {} must be under 5000ms timeout",
            result.execution_time_ms
        );

        println!(
            "PASS: Execution time {} ms (actual: {} ms)",
            result.execution_time_ms, actual_elapsed
        );
    }

    // =========================================================================
    // TC-HOOKS-013: Drift Metrics Computation Tests
    // All tests use SessionCache per PRD v6 Section 14
    // =========================================================================

    // =========================================================================
    // TC-HOOKS-013-01: Drift metrics computed when linking sessions
    // Verify: drift_metrics is Some with valid values when previous exists
    // UPDATED: stability_delta should be ~0.0 because identity state is RESTORED
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_013_01_drift_metrics_computed_when_linking() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-013-01: Drift Metrics Computed When Linking ===");

        let prev_id = "prev-session-drift";
        let new_id = "new-session-drift";

        // Setup: Create previous session snapshot with known values in cache
        {
            let mut prev_snapshot = SessionSnapshot::new(prev_id);
            prev_snapshot.topic_profile = [0.5; 13];
            prev_snapshot.integration = 0.8;
            prev_snapshot.reflection = 0.7;
            prev_snapshot.differentiation = 0.6;
            store_in_cache(&prev_snapshot);
        }

        // Execute: Create new session linked to previous
        let args = SessionStartArgs {
            db_path: None,
            session_id: Some(new_id.to_string()),
            previous_session_id: Some(prev_id.to_string()),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;
        assert!(result.is_ok(), "Execute must succeed");
        let output = result.unwrap();

        // Verify: drift_metrics is present
        assert!(
            output.drift_metrics.is_some(),
            "drift_metrics must be Some when linking sessions"
        );
        let drift = output.drift_metrics.unwrap();

        // Verify: all fields are valid
        assert!(
            drift.purpose_drift >= 0.0 && drift.purpose_drift <= 2.0,
            "purpose_drift {} must be in [0.0, 2.0]",
            drift.purpose_drift
        );
        // Note: time_since_snapshot_ms may be 0 if operations complete in <1ms
        // which is valid - the session was just stored and immediately retrieved
        assert!(
            drift.time_since_snapshot_ms < 60000, // Should be under 1 minute
            "time_since_snapshot_ms {} should be reasonable",
            drift.time_since_snapshot_ms
        );
        assert!(
            drift.coherence_phase_drift >= 0.0,
            "coherence_phase_drift {} must be non-negative",
            drift.coherence_phase_drift
        );

        // After identity restoration, stability_delta should be ~0.0
        // (identical metrics = no stability change)
        assert!(
            drift.stability_delta.abs() < 0.1,
            "stability_delta MUST be ~0.0 after identity restoration, got {} (bug if < -0.3)",
            drift.stability_delta
        );

        // purpose_drift should be ~0.0 (vectors are identical after copy)
        assert!(
            drift.purpose_drift < 0.01,
            "purpose_drift MUST be ~0.0 after identity restoration, got {}",
            drift.purpose_drift
        );

        // coherence_phase_drift should be ~0.0 (metrics are identical after copy)
        assert!(
            drift.coherence_phase_drift < 0.1,
            "coherence_phase_drift MUST be ~0.0 after identity restoration, got {}",
            drift.coherence_phase_drift
        );

        println!("PASS: Drift metrics computed with valid values");
        println!("  stability_delta: {} (MUST be ~0.0)", drift.stability_delta);
        println!("  purpose_drift: {} (MUST be ~0.0)", drift.purpose_drift);
        println!("  time_since_snapshot_ms: {}", drift.time_since_snapshot_ms);
        println!(
            "  coherence_phase_drift: {} (MUST be ~0.0)",
            drift.coherence_phase_drift
        );
    }

    // =========================================================================
    // TC-HOOKS-013-02: No drift metrics for new session
    // Verify: drift_metrics is None when no previous_session_id
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_013_02_no_drift_metrics_for_new_session() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-013-02: No Drift Metrics for New Session ===");

        let args = SessionStartArgs {
            db_path: None,
            session_id: Some("brand-new-session".to_string()),
            previous_session_id: None,
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;
        assert!(result.is_ok(), "Execute must succeed");
        let output = result.unwrap();

        // Verify: No drift metrics for new session
        assert!(
            output.drift_metrics.is_none(),
            "drift_metrics must be None for new session"
        );
        assert!(output.success, "success must be true");

        println!("PASS: No drift metrics for new session");
    }

    // =========================================================================
    // TC-HOOKS-013-03: Drift metrics when previous session not found
    // Verify: drift_metrics is None when previous doesn't exist
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_013_03_drift_metrics_when_previous_not_found() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-013-03: Drift Metrics When Previous Not Found ===");

        let args = SessionStartArgs {
            db_path: None,
            session_id: Some("orphan-session".to_string()),
            previous_session_id: Some("nonexistent-session-xyz".to_string()),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;
        assert!(
            result.is_ok(),
            "Execute must succeed despite missing previous"
        );
        let output = result.unwrap();

        // Verify: No drift metrics when previous not found
        assert!(
            output.drift_metrics.is_none(),
            "drift_metrics must be None when previous not found"
        );
        assert!(output.success, "success must be true");

        println!("PASS: Handled missing previous session gracefully");
    }

    // =========================================================================
    // TC-HOOKS-013-04: Cosine distance edge cases
    // Verify: cosine_distance function handles edge cases
    // =========================================================================
    #[test]
    fn tc_hooks_013_04_cosine_distance_edge_cases() {
        println!("\n=== TC-HOOKS-013-04: Cosine Distance Edge Cases ===");

        // Test 1: Identical vectors -> distance 0.0
        let a = [
            1.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ];
        let b = [
            1.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ];
        let dist = cosine_distance(&a, &b);
        assert!(
            (dist - 0.0).abs() < 0.001,
            "Identical vectors should have distance 0.0, got {}",
            dist
        );
        println!("  Identical vectors: distance = {}", dist);

        // Test 2: Opposite vectors -> distance 2.0
        let a = [
            1.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ];
        let b = [
            -1.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ];
        let dist = cosine_distance(&a, &b);
        assert!(
            (dist - 2.0).abs() < 0.001,
            "Opposite vectors should have distance 2.0, got {}",
            dist
        );
        println!("  Opposite vectors: distance = {}", dist);

        // Test 3: Zero vectors -> distance 0.0 (graceful handling)
        let a = [0.0_f32; 13];
        let b = [0.0_f32; 13];
        let dist = cosine_distance(&a, &b);
        assert!(
            dist == 0.0,
            "Zero vectors should have distance 0.0, got {}",
            dist
        );
        println!("  Zero vectors: distance = {}", dist);

        // Test 4: Proportional vectors -> distance 0.0
        let a = [1.0_f32; 13];
        let b = [0.5_f32; 13];
        let dist = cosine_distance(&a, &b);
        assert!(
            (dist - 0.0).abs() < 0.001,
            "Proportional vectors should have distance ~0.0, got {}",
            dist
        );
        println!("  Proportional vectors: distance = {}", dist);

        println!("PASS: Cosine distance handles edge cases");
    }

    // =========================================================================
    // TC-HOOKS-013-06: Crisis drift triggers error log
    // Verify: is_crisis_drift returns true for stability_delta < -0.3
    // =========================================================================
    #[test]
    fn tc_hooks_013_06_crisis_drift_detection() {
        println!("\n=== TC-HOOKS-013-06: Crisis Drift Detection ===");

        // Verify: DriftMetrics correctly identifies crisis
        let drift = DriftMetrics {
            stability_delta: -0.4, // 40% stability drop
            purpose_drift: 0.0,
            time_since_snapshot_ms: 1000,
            coherence_phase_drift: 0.0,
        };

        assert!(
            drift.is_crisis_drift(),
            "stability_delta {} should be crisis (< -0.3)",
            drift.stability_delta
        );
        assert!(drift.is_warning_drift(), "crisis is also warning level");

        println!(
            "PASS: Crisis drift correctly detected for stability_delta = {}",
            drift.stability_delta
        );
    }

    // =========================================================================
    // TC-HOOKS-013-07: Warning drift triggers warn log
    // Verify: is_warning_drift returns true for stability_delta < -0.1
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_013_07_warning_drift_detection() {
        println!("\n=== TC-HOOKS-013-07: Warning Drift Detection ===");

        // Test warning level: -0.15 (15% drop)
        let warning_drift = DriftMetrics {
            stability_delta: -0.15, // 15% drop
            purpose_drift: 0.0,
            time_since_snapshot_ms: 1000,
            coherence_phase_drift: 0.0,
        };

        assert!(
            warning_drift.is_warning_drift(),
            "stability_delta {} should be warning (< -0.1)",
            warning_drift.stability_delta
        );
        assert!(
            !warning_drift.is_crisis_drift(),
            "stability_delta {} should NOT be crisis (>= -0.3)",
            warning_drift.stability_delta
        );
        println!(
            "  Warning drift: stability_delta = {}, is_warning = true, is_crisis = false",
            warning_drift.stability_delta
        );

        // Test healthy level: -0.05 (5% drop)
        let healthy_drift = DriftMetrics {
            stability_delta: -0.05, // 5% drop
            purpose_drift: 0.0,
            time_since_snapshot_ms: 1000,
            coherence_phase_drift: 0.0,
        };

        assert!(
            !healthy_drift.is_warning_drift(),
            "stability_delta {} should NOT be warning (>= -0.1)",
            healthy_drift.stability_delta
        );
        assert!(
            !healthy_drift.is_crisis_drift(),
            "stability_delta {} should NOT be crisis (>= -0.3)",
            healthy_drift.stability_delta
        );
        println!(
            "  Healthy drift: stability_delta = {}, is_warning = false, is_crisis = false",
            healthy_drift.stability_delta
        );

        // Test positive drift (improvement)
        let positive_drift = DriftMetrics {
            stability_delta: 0.1, // 10% improvement
            purpose_drift: 0.0,
            time_since_snapshot_ms: 1000,
            coherence_phase_drift: 0.0,
        };

        assert!(
            !positive_drift.is_warning_drift(),
            "positive stability_delta {} should NOT be warning",
            positive_drift.stability_delta
        );
        assert!(
            !positive_drift.is_crisis_drift(),
            "positive stability_delta {} should NOT be crisis",
            positive_drift.stability_delta
        );
        println!(
            "  Positive drift: stability_delta = {}, is_warning = false, is_crisis = false",
            positive_drift.stability_delta
        );

        println!("PASS: Warning drift correctly detected at threshold boundaries");
    }

    // =========================================================================
    // TC-HOOKS-017: Session State Restoration Tests
    // Tests verify session state fields are correctly restored from previous session
    // Fields tested: topic_profile, integration, reflection, differentiation, trajectory
    // All tests use SessionCache per PRD v6 Section 14
    // =========================================================================

    // =========================================================================
    // TC-HOOKS-017-01: Complete identity state restoration
    // Verify: All identity fields are copied from previous to new session
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_017_01_complete_identity_state_restoration() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-017-01: Complete Identity State Restoration ===");

        let prev_id = "session-with-identity";
        let new_id = "restored-session";

        // Setup: Create previous session with identity state in cache
        let prev_purpose = [
            0.3_f32, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.35, 0.45, 0.55,
        ];
        let prev_integration = 0.72;
        let prev_reflection = 0.68;
        let prev_differentiation = 0.59;
        {
            let mut prev_snapshot = SessionSnapshot::new(prev_id);
            prev_snapshot.topic_profile = prev_purpose;
            prev_snapshot.integration = prev_integration;
            prev_snapshot.reflection = prev_reflection;
            prev_snapshot.differentiation = prev_differentiation;
            // Add trajectory entries
            prev_snapshot.append_to_trajectory([0.1; 13]);
            prev_snapshot.append_to_trajectory([0.2; 13]);
            store_in_cache(&prev_snapshot);
        }

        // Execute: Create new session linked to previous
        let args = SessionStartArgs {
            db_path: None,
            session_id: Some(new_id.to_string()),
            previous_session_id: Some(prev_id.to_string()),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;
        assert!(result.is_ok(), "Execute must succeed");

        // VERIFY: Identity fields restored from previous session (now in cache)
        let new_snapshot = SessionCache::get().expect("Cache must have snapshot");

        // Verify topic_profile
        assert_eq!(
            new_snapshot.topic_profile, prev_purpose,
            "topic_profile MUST be restored"
        );

        // Verify integration
        assert!(
            (new_snapshot.integration - prev_integration).abs() < 0.001,
            "integration MUST be restored, got {} expected {}",
            new_snapshot.integration,
            prev_integration
        );

        // Verify reflection
        assert!(
            (new_snapshot.reflection - prev_reflection).abs() < 0.001,
            "reflection MUST be restored, got {} expected {}",
            new_snapshot.reflection,
            prev_reflection
        );

        // Verify differentiation
        assert!(
            (new_snapshot.differentiation - prev_differentiation).abs() < 0.001,
            "differentiation MUST be restored, got {} expected {}",
            new_snapshot.differentiation,
            prev_differentiation
        );

        // Verify trajectory copied
        assert!(
            new_snapshot.trajectory.len() >= 2,
            "trajectory MUST be restored, got {} entries",
            new_snapshot.trajectory.len()
        );

        println!("PASS: All identity state fields restored from previous session");
        println!(
            "  topic_profile: {:?}...",
            &new_snapshot.topic_profile[0..3]
        );
        println!("  integration: {}", new_snapshot.integration);
        println!("  reflection: {}", new_snapshot.reflection);
        println!("  differentiation: {}", new_snapshot.differentiation);
        println!("  trajectory.len: {}", new_snapshot.trajectory.len());
    }

    // =========================================================================
    // TC-HOOKS-017-02: Stability healthy after restoration
    // Verify: Coherence computed from integration/reflection/differentiation is healthy
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_017_02_stability_healthy_after_restoration() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-017-02: Stability Healthy After Restoration ===");

        let prev_id = "healthy-session";
        let new_id = "should-be-healthy-too";

        // Setup: Create previous session with healthy metrics in cache
        {
            let mut prev_snapshot = SessionSnapshot::new(prev_id);
            prev_snapshot.topic_profile = [0.5; 13]; // Non-zero vector
            prev_snapshot.integration = 0.85;
            prev_snapshot.reflection = 0.80;
            prev_snapshot.differentiation = 0.75;
            store_in_cache(&prev_snapshot);
        }

        // Execute: Create new session linked to previous
        let args = SessionStartArgs {
            db_path: None,
            session_id: Some(new_id.to_string()),
            previous_session_id: Some(prev_id.to_string()),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;
        assert!(result.is_ok(), "Execute must succeed");
        let output = result.unwrap();

        // VERIFY: Stability classification shows healthy status
        assert!(
            output.stability_classification.is_some(),
            "stability_classification must be present"
        );
        let stability = output.stability_classification.unwrap();

        // Coherence = (integration + reflection + differentiation) / 3 = (0.85 + 0.80 + 0.75) / 3 = 0.8
        // Should be healthy (>= 0.7)
        assert!(
            stability.value > 0.7,
            "Stability MUST be healthy (>0.7) after restoration, got {}",
            stability.value
        );

        // Verify level indicates healthy
        assert!(
            stability.value >= 0.7,
            "Stability level MUST indicate healthy, got value {}",
            stability.value
        );

        // VERIFY: Cache has correct restored values
        let new_snapshot = SessionCache::get().expect("Cache must have snapshot");

        let coherence =
            (new_snapshot.integration + new_snapshot.reflection + new_snapshot.differentiation)
                / 3.0;
        assert!(
            coherence > 0.7,
            "Computed coherence MUST be >0.7, got {}",
            coherence
        );

        println!("PASS: Stability is healthy after session restoration");
        println!("  Stability value: {}", stability.value);
        println!("  Cache coherence: {}", coherence);
    }

    // =========================================================================
    // TC-HOOKS-017-03: Drift metrics near zero after restoration
    // Verify: drift_metrics.stability_delta is near zero (identical vectors)
    // =========================================================================
    #[tokio::test]
    async fn tc_hooks_017_03_stability_delta_near_zero() {
        let _guard = GLOBAL_IDENTITY_LOCK.lock().expect("Test lock poisoned");
        println!("\n=== TC-HOOKS-017-03: Stability Delta Near Zero ===");

        let prev_id = "stable-session";
        let new_id = "continued-session";

        // Setup: Create previous session in cache
        {
            let mut prev_snapshot = SessionSnapshot::new(prev_id);
            prev_snapshot.topic_profile = [0.6; 13];
            prev_snapshot.integration = 0.75;
            prev_snapshot.reflection = 0.70;
            prev_snapshot.differentiation = 0.65;
            store_in_cache(&prev_snapshot);
        }

        // Execute: Create new session linked to previous
        let args = SessionStartArgs {
            db_path: None,
            session_id: Some(new_id.to_string()),
            previous_session_id: Some(prev_id.to_string()),
            stdin: false,
            format: OutputFormat::Json,
        };

        let result = execute(args).await;
        assert!(result.is_ok(), "Execute must succeed");
        let output = result.unwrap();

        // VERIFY: drift_metrics shows near-zero delta
        assert!(
            output.drift_metrics.is_some(),
            "drift_metrics must be present"
        );
        let drift = output.drift_metrics.unwrap();

        // stability_delta should be ~0.0 (vectors identical after copy)
        assert!(
            drift.stability_delta.abs() < 0.1,
            "stability_delta MUST be ~0.0 after restoration, got {}",
            drift.stability_delta
        );

        // Also verify not a crisis or warning
        assert!(
            !drift.is_crisis_drift(),
            "MUST NOT be crisis drift after restoration, stability_delta={}",
            drift.stability_delta
        );
        assert!(
            !drift.is_warning_drift(),
            "MUST NOT be warning drift after restoration, stability_delta={}",
            drift.stability_delta
        );

        println!("PASS: Stability delta is ~0.0 after restoration");
        println!("  stability_delta: {} (MUST be ~0.0)", drift.stability_delta);
        println!("  is_crisis_drift: false");
        println!("  is_warning_drift: false");
    }
}
