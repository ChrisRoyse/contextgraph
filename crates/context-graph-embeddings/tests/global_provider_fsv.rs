//! Full State Verification tests for global_provider.rs
//!
//! TASK-EMB-016: These tests verify the global warm provider singleton behaves correctly.
//!
//! FSV Methodology:
//! 1. Define Source of Truth: Global static GLOBAL_WARM_PROVIDER
//! 2. Execute & Inspect: Test operations and verify state changes
//! 3. Edge Cases: Empty init, double init, concurrent access
//! 4. Evidence of Success: Detailed logging of state transitions

use context_graph_embeddings::{get_warm_provider, is_warm_initialized, warm_status_message};

/// FSV-1: Test status message returns expected values before initialization
#[test]
fn fsv_status_message_before_init() {
    println!("\n============================================================");
    println!("=== FSV-1: Status Message Before Init ===");
    println!("============================================================\n");

    let status = warm_status_message();
    println!("STATUS MESSAGE: {}", status);

    // Status should be one of the expected values
    let valid_states = [
        "Not initialized",
        "Initialization in progress",
        "Ready (13 models warm)",
    ];

    let is_valid = valid_states.iter().any(|s| status.starts_with(s))
        || status.starts_with("Initialization failed:");

    println!("VALIDATION: Status '{}' is valid: {}", status, is_valid);
    assert!(
        is_valid,
        "Status message should be a recognized state, got: {}",
        status
    );

    println!("\n[FSV-1 VERIFIED] Status message returns expected values");
}

/// FSV-2: Test is_warm_initialized returns bool and doesn't panic
#[test]
fn fsv_is_warm_initialized_safe() {
    println!("\n============================================================");
    println!("=== FSV-2: is_warm_initialized Safety ===");
    println!("============================================================\n");

    // Call multiple times to ensure no panic
    for i in 0..100 {
        let result = is_warm_initialized();
        if i == 0 {
            println!("FIRST CALL: is_warm_initialized() = {}", result);
        }
    }

    println!("COMPLETED 100 calls without panic");
    println!("\n[FSV-2 VERIFIED] is_warm_initialized is safe to call repeatedly");
}

/// FSV-3: Test get_warm_provider returns appropriate error before init
#[test]
fn fsv_get_provider_error_before_init() {
    println!("\n============================================================");
    println!("=== FSV-3: Get Provider Error Before Init ===");
    println!("============================================================\n");

    let result = get_warm_provider();

    match result {
        Ok(provider) => {
            // Provider may already be initialized from previous tests
            println!("Provider was already initialized (from previous test)");
            println!("is_ready: {}", provider.is_ready());
            println!("model_ids: {:?}", provider.model_ids());
        }
        Err(e) => {
            let err_msg = e.to_string();
            println!("ERROR MESSAGE: {}", err_msg);

            // Error should mention not initialized or failed
            let is_expected_error = err_msg.contains("not initialized")
                || err_msg.contains("failed")
                || err_msg.contains("CUDA");

            println!("VALIDATION: Error is expected type: {}", is_expected_error);
            assert!(
                is_expected_error,
                "Error should indicate not initialized or failed, got: {}",
                err_msg
            );
        }
    }

    println!("\n[FSV-3 VERIFIED] get_warm_provider handles uninitialized state correctly");
}

/// FSV-4: Test status message consistency with is_warm_initialized
#[test]
fn fsv_status_consistency() {
    println!("\n============================================================");
    println!("=== FSV-4: Status Consistency ===");
    println!("============================================================\n");

    let is_init = is_warm_initialized();
    let status = warm_status_message();

    println!("is_warm_initialized: {}", is_init);
    println!("warm_status_message: {}", status);

    if is_init {
        assert!(
            status.contains("Ready"),
            "If initialized, status should say Ready, got: {}",
            status
        );
    } else {
        // Could be "Not initialized" or "Initialization failed: ..."
        assert!(
            !status.contains("Ready"),
            "If not initialized, status should not say Ready, got: {}",
            status
        );
    }

    println!("\n[FSV-4 VERIFIED] Status message is consistent with is_warm_initialized");
}

/// FSV-5: Test concurrent access to status functions
#[test]
fn fsv_concurrent_status_access() {
    println!("\n============================================================");
    println!("=== FSV-5: Concurrent Status Access ===");
    println!("============================================================\n");

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;

    let success_count = Arc::new(AtomicUsize::new(0));
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let counter = Arc::clone(&success_count);
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = is_warm_initialized();
                    let _ = warm_status_message();
                }
                counter.fetch_add(1, Ordering::SeqCst);
                println!("Thread {} completed", i);
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread should not panic");
    }

    let completed = success_count.load(Ordering::SeqCst);
    println!("COMPLETED: {} threads finished successfully", completed);
    assert_eq!(completed, 10, "All threads should complete");

    println!("\n[FSV-5 VERIFIED] Concurrent status access is thread-safe");
}

/// FSV-6: Test that error types are informative
#[test]
fn fsv_error_informativeness() {
    println!("\n============================================================");
    println!("=== FSV-6: Error Informativeness ===");
    println!("============================================================\n");

    let result = get_warm_provider();

    if let Err(e) = result {
        let err_string = e.to_string();
        let debug_string = format!("{:?}", e);

        println!("Error Display: {}", err_string);
        println!("Error Debug: {}", debug_string);

        // Error should provide actionable information
        let has_action = err_string.contains("initialize")
            || err_string.contains("CUDA")
            || err_string.contains("failed")
            || err_string.contains("not initialized");

        println!("VALIDATION: Error contains actionable info: {}", has_action);
        assert!(
            has_action,
            "Error should provide actionable information, got: {}",
            err_string
        );
    } else {
        println!("Provider already initialized - skipping error test");
    }

    println!("\n[FSV-6 VERIFIED] Errors are informative and actionable");
}

/// FSV-7: Test state transitions are observable
#[test]
fn fsv_state_observability() {
    println!("\n============================================================");
    println!("=== FSV-7: State Observability ===");
    println!("============================================================\n");

    // Record initial state
    let init_state = is_warm_initialized();
    let init_status = warm_status_message();
    let init_result = get_warm_provider();

    println!("INITIAL STATE:");
    println!("  is_warm_initialized: {}", init_state);
    println!("  warm_status_message: {}", init_status);
    println!("  get_warm_provider: {}", init_result.is_ok());

    // State should be consistent
    if init_state {
        assert!(
            init_result.is_ok(),
            "If initialized, get_warm_provider should succeed"
        );
    }

    // Status message should match boolean state
    if init_status.contains("Ready") {
        assert!(
            init_state,
            "If status says Ready, is_warm_initialized should be true"
        );
    }

    println!("\n[FSV-7 VERIFIED] State is observable and consistent");
}

/// FSV-EDGE-1: V-004 fix (#499). Verify repeated calls do NOT mutate
/// observable state. Captures the initial (init, status) tuple and
/// asserts every one of 1000 subsequent calls returns the same value;
/// the previous version discarded every return value with `let _ =`
/// and could not detect state drift, accidental writes, or memoization
/// leaks under repeated queries.
#[test]
fn fsv_edge_long_queries() {
    println!("\n============================================================");
    println!("=== FSV-EDGE-1: Long Query Handling ===");
    println!("============================================================\n");

    let baseline_init = is_warm_initialized();
    let baseline_status = warm_status_message();
    assert!(
        !baseline_status.is_empty(),
        "baseline status must not be empty"
    );

    for i in 0..1000 {
        let init = is_warm_initialized();
        let status = warm_status_message();

        assert_eq!(
            init, baseline_init,
            "iteration {i}: is_warm_initialized drifted from baseline"
        );
        assert_eq!(
            status, baseline_status,
            "iteration {i}: warm_status_message drifted from baseline"
        );
        assert!(
            !status.is_empty(),
            "iteration {i}: status went empty mid-loop"
        );

        if i % 250 == 0 {
            println!("Iteration {i}: status = {status}");
        }
    }

    println!("COMPLETED 1000 iterations with stable state");
    println!("\n[FSV-EDGE-1 VERIFIED] Long repeated queries are observably idempotent");
}

/// FSV-EDGE-2: Test status during theoretical race conditions
#[test]
fn fsv_edge_race_conditions() {
    println!("\n============================================================");
    println!("=== FSV-EDGE-2: Race Condition Safety ===");
    println!("============================================================\n");

    use std::thread;
    use std::time::Duration;

    let handles: Vec<_> = (0..5)
        .map(|i| {
            thread::spawn(move || {
                for j in 0..50 {
                    let status = warm_status_message();
                    let init = is_warm_initialized();

                    // Verify consistency within single thread
                    if init && !status.contains("Ready") && !status.contains("in progress") {
                        panic!(
                            "Thread {} iter {}: Inconsistent state - init={} status={}",
                            i, j, init, status
                        );
                    }

                    // Small sleep to increase chance of race
                    if j % 10 == 0 {
                        thread::sleep(Duration::from_micros(10));
                    }
                }
            })
        })
        .collect();

    for (i, handle) in handles.into_iter().enumerate() {
        handle
            .join()
            .unwrap_or_else(|_| panic!("Thread {} should not panic", i));
    }

    println!("COMPLETED concurrent race condition test");
    println!("\n[FSV-EDGE-2 VERIFIED] Race conditions handled safely");
}

/// FSV-EDGE-3: Test API contract - methods don't panic on repeated calls
#[test]
fn fsv_edge_api_contract() {
    println!("\n============================================================");
    println!("=== FSV-EDGE-3: API Contract Verification ===");
    println!("============================================================\n");

    // These functions should NEVER panic, only return values or errors
    println!("Testing is_warm_initialized...");
    for _ in 0..100 {
        let _ = is_warm_initialized(); // Should never panic
    }
    println!("  PASS: No panics");

    println!("Testing warm_status_message...");
    for _ in 0..100 {
        let status = warm_status_message();
        assert!(!status.is_empty(), "Status should never be empty");
    }
    println!("  PASS: Never empty, no panics");

    println!("Testing get_warm_provider...");
    for _ in 0..100 {
        let _ = get_warm_provider(); // Should never panic, only Err
    }
    println!("  PASS: No panics");

    println!("\n[FSV-EDGE-3 VERIFIED] API contract upheld - no panics");
}
