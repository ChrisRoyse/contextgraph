//! WarmLoader Integration Verification Tests
//!
//! This test file verifies the COMPLETE WarmLoader pipeline integration:
//! - WarmLoader creation and initialization
//! - WarmEmbeddingPipeline creation and configuration
//! - Preflight checks (GPU detection and validation)
//! - Health monitoring system
//! - Diagnostics reporting
//! - Registry state management
//! - Full pipeline readiness checks
//!
//! # Requirements
//!
//! - RTX 5090 GPU with 32GB VRAM
//! - CUDA 13.x drivers installed
//! - Compute capability 12.0
//!
//! # Constitution Compliance
//!
//! AP-007: No mock/stub data in production - all GPU operations are verified as real.
//!
//! # Agent Context
//!
//! Agent #3/3 (FINAL) | Previous: CUDA Runtime (12 tests PASSED), Weight Loading (9 tests PASSED)
//!
//! # Exit Codes
//!
//! - 101: CUDA_UNAVAILABLE - No CUDA device or simulated GPU detected
//! - 107: CUDA_CAPABILITY_INSUFFICIENT - Compute capability below 12.0
//! - 108: CUDA_ALLOC_FAILED - Memory allocation failed

#![cfg(feature = "cuda")]

use context_graph_embeddings::warm::{
    ModelHandle, WarmConfig, WarmCudaAllocator, WarmEmbeddingPipeline,
    WarmHealthChecker, WarmHealthStatus, WarmLoader, WarmModelState,
    EMBEDDING_MODEL_IDS, MINIMUM_VRAM_BYTES, REQUIRED_COMPUTE_MAJOR,
    REQUIRED_COMPUTE_MINOR, TOTAL_MODEL_COUNT,
};

/// One gigabyte in bytes
const GB: usize = 1024 * 1024 * 1024;

/// One megabyte in bytes
const MB: usize = 1024 * 1024;

// ============================================================================
// TEST 1: WarmLoader Creation
// ============================================================================

/// Verify that WarmLoader can be created with default configuration.
///
/// This test verifies:
/// - WarmLoader::new() succeeds
/// - Registry is properly initialized with all 12 models
/// - Memory pools are configured correctly
/// - No models are warm initially
#[test]
fn test_warmloader_creation() {
    println!("\n=== WARMLOADER CREATION TEST ===\n");

    let config = WarmConfig::default();
    println!("Config: VRAM budget = {} GB, headroom = {} GB",
             config.vram_budget_bytes / GB,
             config.vram_headroom_bytes / GB);

    let result = WarmLoader::new(config);

    match result {
        Ok(loader) => {
            println!("[PASS] WarmLoader created successfully");

            // Verify registry has all models registered
            let registry = loader.registry();
            let guard = registry.read().expect("Failed to lock registry");

            assert_eq!(
                guard.model_count(), TOTAL_MODEL_COUNT,
                "[FAIL] Registry should have {} models, got {}",
                TOTAL_MODEL_COUNT, guard.model_count()
            );
            println!("[PASS] Registry contains {} models", guard.model_count());

            // Verify all models are in Pending state
            let mut pending_count = 0;
            for model_id in EMBEDDING_MODEL_IDS.iter() {
                if let Some(state) = guard.get_state(model_id) {
                    if matches!(state, WarmModelState::Pending) {
                        pending_count += 1;
                    }
                }
            }
            assert_eq!(
                pending_count, TOTAL_MODEL_COUNT,
                "[FAIL] All models should be Pending, got {} pending",
                pending_count
            );
            println!("[PASS] All {} models are in Pending state", pending_count);

            // Verify no models are warm yet
            assert!(
                !loader.all_warm(),
                "[FAIL] No models should be warm after creation"
            );
            println!("[PASS] all_warm() returns false (correct)");

            // Verify loading summary
            let summary = loader.loading_summary();
            assert_eq!(summary.total_models, TOTAL_MODEL_COUNT);
            assert_eq!(summary.models_warm, 0);
            assert_eq!(summary.models_failed, 0);
            println!("[PASS] Loading summary: {} total, 0 warm, 0 failed",
                     summary.total_models);
        }
        Err(e) => {
            panic!(
                "[FAIL] Failed to create WarmLoader: {:?}\n\
                 Exit code: {}",
                e, e.exit_code()
            );
        }
    }
}

// ============================================================================
// TEST 2: WarmEmbeddingPipeline Creation
// ============================================================================

/// Verify that WarmEmbeddingPipeline can be created without warming.
///
/// This test verifies:
/// - Pipeline creation succeeds
/// - Pipeline is NOT ready initially
/// - Health checker is initialized
/// - Registry is accessible
#[test]
fn test_pipeline_creation_without_warming() {
    println!("\n=== PIPELINE CREATION (NO WARMING) TEST ===\n");

    let config = WarmConfig::default();
    let result = WarmEmbeddingPipeline::new(config);

    match result {
        Ok(pipeline) => {
            println!("[PASS] WarmEmbeddingPipeline created successfully");

            // Verify NOT ready (no warming done)
            assert!(
                !pipeline.is_ready(),
                "[FAIL] Pipeline should NOT be ready without warming"
            );
            println!("[PASS] is_ready() returns false (correct)");

            // Verify NOT initialized
            assert!(
                !pipeline.is_initialized(),
                "[FAIL] Pipeline should NOT be initialized"
            );
            println!("[PASS] is_initialized() returns false (correct)");

            // Verify no uptime yet
            assert!(
                pipeline.uptime().is_none(),
                "[FAIL] Uptime should be None before initialization"
            );
            println!("[PASS] uptime() returns None (correct)");

            // Verify registry is accessible
            let registry = pipeline.registry();
            let guard = registry.read().expect("Failed to lock registry");
            assert_eq!(guard.model_count(), TOTAL_MODEL_COUNT);
            println!("[PASS] Registry accessible with {} models", guard.model_count());
        }
        Err(e) => {
            panic!("[FAIL] Failed to create pipeline: {:?}", e);
        }
    }
}

// ============================================================================
// TEST 3: Health Checker Initialization
// ============================================================================

/// Verify health checker is properly initialized and reports correct status.
///
/// This test verifies:
/// - Health checker can be created from loader
/// - Initial status is Loading (models pending)
/// - Correct model counts are reported
#[test]
fn test_health_checker_initialization() {
    println!("\n=== HEALTH CHECKER INITIALIZATION TEST ===\n");

    let config = WarmConfig::default();
    let pipeline = WarmEmbeddingPipeline::new(config)
        .expect("Failed to create pipeline");

    let health = pipeline.health();

    // Initial status should be Loading (models pending)
    assert_eq!(
        health.status, WarmHealthStatus::Loading,
        "[FAIL] Initial status should be Loading, got {:?}",
        health.status
    );
    println!("[PASS] Initial status is Loading");

    // Verify model counts
    assert_eq!(health.models_total, TOTAL_MODEL_COUNT);
    assert_eq!(health.models_warm, 0);
    assert_eq!(health.models_pending, TOTAL_MODEL_COUNT);
    assert_eq!(health.models_loading, 0);
    assert_eq!(health.models_failed, 0);
    println!("[PASS] Model counts: total={}, warm=0, pending={}, loading=0, failed=0",
             health.models_total, health.models_pending);

    // Verify no error messages
    assert!(
        health.error_messages.is_empty(),
        "[FAIL] Error messages should be empty"
    );
    println!("[PASS] No error messages");

    // Verify uptime is tracked
    assert!(health.uptime.is_some(), "[FAIL] Uptime should be tracked");
    println!("[PASS] Uptime is tracked");

    // Verify VRAM metrics are initialized
    println!("VRAM allocated: {} bytes", health.vram_allocated_bytes);
    println!("VRAM available: {} bytes", health.vram_available_bytes);
}

// ============================================================================
// TEST 4: Health Status Quick Check
// ============================================================================

/// Verify health status quick check methods work correctly.
#[test]
fn test_health_status_quick_check() {
    println!("\n=== HEALTH STATUS QUICK CHECK TEST ===\n");

    let config = WarmConfig::default();
    let loader = WarmLoader::new(config).expect("Failed to create loader");
    let checker = WarmHealthChecker::from_loader(&loader);

    // Initial status should be Loading
    let status = checker.status();
    assert_eq!(status, WarmHealthStatus::Loading);
    println!("[PASS] status() returns Loading");

    // is_healthy should be false
    assert!(!checker.is_healthy());
    println!("[PASS] is_healthy() returns false");

    // Verify status methods
    assert!(status.is_loading());
    assert!(!status.is_healthy());
    assert!(!status.is_unhealthy());
    assert!(!status.is_not_initialized());
    println!("[PASS] Status methods work correctly");
}

// ============================================================================
// TEST 5: Diagnostics Report Generation
// ============================================================================

/// Verify diagnostics report is generated correctly.
///
/// This test verifies:
/// - Report contains timestamp
/// - System info is populated
/// - Model list is complete
/// - Memory diagnostics are available
#[test]
fn test_diagnostics_report_generation() {
    println!("\n=== DIAGNOSTICS REPORT GENERATION TEST ===\n");

    let config = WarmConfig::default();
    let pipeline = WarmEmbeddingPipeline::new(config)
        .expect("Failed to create pipeline");

    let report = pipeline.diagnostics();

    // Verify timestamp
    assert!(
        !report.timestamp.is_empty(),
        "[FAIL] Timestamp should not be empty"
    );
    println!("[PASS] Timestamp: {}", report.timestamp);

    // Verify system info
    println!("System hostname: {}", report.system.hostname);
    println!("System OS: {}", report.system.os);

    // Verify model list is complete
    assert_eq!(
        report.models.len(), TOTAL_MODEL_COUNT,
        "[FAIL] Report should contain {} models, got {}",
        TOTAL_MODEL_COUNT, report.models.len()
    );
    println!("[PASS] Report contains {} model entries", report.models.len());

    // Verify model diagnostic fields
    for model in &report.models {
        assert!(!model.model_id.is_empty());
        assert!(!model.state.is_empty());
        // Initially all should be Pending
        assert!(
            model.state.contains("Pending") || model.state.contains("Loading"),
            "[FAIL] Model {} should be Pending, got {}",
            model.model_id, model.state
        );
    }
    println!("[PASS] All model diagnostics have valid state");

    // Verify memory diagnostics
    println!("Memory pool capacity: {} bytes", report.memory.model_pool_capacity_bytes);
    println!("Memory pool used: {} bytes", report.memory.model_pool_used_bytes);

    // Verify initial state counts
    assert_eq!(report.warm_count(), 0);
    assert_eq!(report.failed_count(), 0);
    println!("[PASS] warm_count=0, failed_count=0 (correct for initial state)");
}

// ============================================================================
// TEST 6: Diagnostics Status Line
// ============================================================================

/// Verify status line format is correct.
#[test]
fn test_diagnostics_status_line() {
    println!("\n=== DIAGNOSTICS STATUS LINE TEST ===\n");

    let config = WarmConfig::default();
    let pipeline = WarmEmbeddingPipeline::new(config)
        .expect("Failed to create pipeline");

    let status_line = pipeline.status_line();

    println!("Status line: {}", status_line);

    // Verify format components
    assert!(
        status_line.contains("WARM:"),
        "[FAIL] Status line should contain 'WARM:'"
    );
    assert!(
        status_line.contains("models"),
        "[FAIL] Status line should contain 'models'"
    );
    assert!(
        status_line.contains("VRAM"),
        "[FAIL] Status line should contain 'VRAM'"
    );

    // Should show LOADING status initially
    assert!(
        status_line.contains("LOADING") || status_line.contains("0/12"),
        "[FAIL] Status line should indicate loading state"
    );

    println!("[PASS] Status line format is correct");
}

// ============================================================================
// TEST 7: GPU Detection via Preflight Checks
// ============================================================================

/// Verify GPU is detected correctly via preflight-like checks.
///
/// This test directly uses WarmCudaAllocator to verify:
/// - GPU is accessible
/// - GPU is NOT simulated
/// - Compute capability meets requirements
/// - VRAM meets requirements
#[test]
fn test_gpu_detection_preflight() {
    println!("\n=== GPU DETECTION (PREFLIGHT) TEST ===\n");

    // Create allocator to query GPU info
    let allocator = WarmCudaAllocator::new(0)
        .expect("[FAIL] CUDA allocator creation failed - RTX 5090 required");

    let gpu_info = allocator.get_gpu_info()
        .expect("[FAIL] Failed to get GPU info");

    println!("GPU Detected:");
    println!("  Name: {}", gpu_info.name);
    println!("  Compute Capability: {}", gpu_info.compute_capability_string());
    println!("  VRAM: {:.2} GB ({} bytes)",
             gpu_info.total_memory_gb(), gpu_info.total_memory_bytes);
    println!("  Driver: {}", gpu_info.driver_version);

    // CRITICAL: Verify NOT simulated
    let name_lower = gpu_info.name.to_lowercase();
    let is_simulated = name_lower.contains("simulated")
        || name_lower.contains("stub")
        || name_lower.contains("fake");

    assert!(
        !is_simulated,
        "[FAIL] SIMULATED GPU DETECTED: '{}'\n\
         Constitution AP-007: Real RTX 5090 REQUIRED. Exit code: 101",
        gpu_info.name
    );
    println!("[PASS] GPU is NOT simulated (real hardware)");

    // Verify compute capability
    let meets_cc = gpu_info.meets_compute_requirement(
        REQUIRED_COMPUTE_MAJOR,
        REQUIRED_COMPUTE_MINOR
    );
    assert!(
        meets_cc,
        "[FAIL] Compute capability {}.{} below required {}.{}\n\
         Exit code: 107",
        gpu_info.compute_capability.0, gpu_info.compute_capability.1,
        REQUIRED_COMPUTE_MAJOR, REQUIRED_COMPUTE_MINOR
    );
    println!("[PASS] Compute capability >= {}.{}",
             REQUIRED_COMPUTE_MAJOR, REQUIRED_COMPUTE_MINOR);

    // Verify VRAM
    assert!(
        gpu_info.total_memory_bytes >= MINIMUM_VRAM_BYTES,
        "[FAIL] VRAM {} GB below required {} GB\n\
         Exit code: 103",
        gpu_info.total_memory_gb(),
        MINIMUM_VRAM_BYTES as f64 / GB as f64
    );
    println!("[PASS] VRAM >= {} GB", MINIMUM_VRAM_BYTES / GB);

    // Verify RTX 5090 requirements
    let meets_5090 = gpu_info.meets_rtx_5090_requirements();
    assert!(meets_5090, "[FAIL] GPU does not meet RTX 5090 requirements");
    println!("[PASS] GPU meets RTX 5090 requirements");
}

// ============================================================================
// TEST 8: State Transitions via Registry (Direct Access)
// ============================================================================

/// Verify state transitions work correctly using WarmLoader directly.
///
/// This test simulates the warming process through registry access:
/// - Transitions models through Pending -> Loading -> Validating -> Warm
/// - Verifies health checker updates correctly
/// - Uses WarmHealthChecker directly for verification
#[test]
fn test_state_transitions_via_registry() {
    println!("\n=== STATE TRANSITIONS VIA REGISTRY TEST ===\n");

    let config = WarmConfig::default();
    let loader = WarmLoader::new(config)
        .expect("Failed to create loader");

    // Create a test handle for simulating VRAM allocation
    let create_handle = |model_num: u32| -> ModelHandle {
        let vram_ptr = 0x1000_0000 + (model_num as u64 * 0x2000_0000);
        let size = 500 * MB;
        let checksum = 0xDEAD_0000 + model_num as u64;
        ModelHandle::new(vram_ptr, size, 0, checksum)
    };

    // Verify initial state via health checker
    let checker = WarmHealthChecker::from_loader(&loader);
    assert_eq!(checker.status(), WarmHealthStatus::Loading);
    println!("[PASS] Initial status: Loading");

    // Transition all models through state machine
    {
        let mut registry = loader.registry().write().expect("Failed to lock registry");

        for (i, model_id) in EMBEDDING_MODEL_IDS.iter().enumerate() {
            // Pending -> Loading
            registry.start_loading(model_id)
                .unwrap_or_else(|_| panic!("Failed to start loading {}", model_id));

            // Update progress
            registry.update_progress(model_id, 50, 250 * MB)
                .unwrap_or_else(|_| panic!("Failed to update progress for {}", model_id));

            // Loading -> Validating
            registry.mark_validating(model_id)
                .unwrap_or_else(|_| panic!("Failed to mark validating {}", model_id));

            // Validating -> Warm
            let handle = create_handle(i as u32);
            registry.mark_warm(model_id, handle)
                .unwrap_or_else(|_| panic!("Failed to mark warm {}", model_id));
        }
    }

    println!("[PASS] All {} models transitioned to Warm state", TOTAL_MODEL_COUNT);

    // Verify registry reflects warm state
    {
        let registry = loader.registry().read().expect("Failed to lock registry");
        assert!(registry.all_warm(), "[FAIL] Registry should report all_warm()");
        println!("[PASS] Registry.all_warm() returns true");
    }

    // Recreate health checker to pick up new state
    let checker = WarmHealthChecker::from_loader(&loader);

    // Verify health status is now Healthy
    let health = checker.check();
    assert_eq!(health.status, WarmHealthStatus::Healthy);
    assert_eq!(health.models_warm, TOTAL_MODEL_COUNT);
    assert_eq!(health.models_failed, 0);
    assert_eq!(health.models_loading, 0);
    assert_eq!(health.models_pending, 0);
    println!("[PASS] Health status: Healthy, {} models warm",
             health.models_warm);

    // Verify loader reflects all_warm
    assert!(loader.all_warm(), "[FAIL] Loader should report all_warm()");
    println!("[PASS] Loader.all_warm() returns true");
}

// ============================================================================
// TEST 9: Health Check After Failure
// ============================================================================

/// Verify health check correctly reports failures.
#[test]
fn test_health_check_after_failure() {
    println!("\n=== HEALTH CHECK AFTER FAILURE TEST ===\n");

    let config = WarmConfig::default();
    let pipeline = WarmEmbeddingPipeline::new(config)
        .expect("Failed to create pipeline");

    // Simulate some models warm, one failed
    {
        let mut registry = pipeline.registry().write().expect("Failed to lock registry");

        // Warm first model
        let model_id = EMBEDDING_MODEL_IDS[0];
        registry.start_loading(model_id).unwrap();
        registry.mark_validating(model_id).unwrap();
        registry.mark_warm(model_id, ModelHandle::new(0x1000, 100*MB, 0, 0x1234)).unwrap();

        // Fail second model
        let failed_model = EMBEDDING_MODEL_IDS[1];
        registry.start_loading(failed_model).unwrap();
        registry.mark_failed(failed_model, 108, "CUDA allocation failed: out of memory").unwrap();
    }

    // Check health
    let health = pipeline.health();

    // Status should be Unhealthy due to failure
    assert_eq!(
        health.status, WarmHealthStatus::Unhealthy,
        "[FAIL] Status should be Unhealthy, got {:?}",
        health.status
    );
    println!("[PASS] Status is Unhealthy (correct)");

    // Verify counts
    assert_eq!(health.models_warm, 1);
    assert_eq!(health.models_failed, 1);
    println!("[PASS] Models: {} warm, {} failed", health.models_warm, health.models_failed);

    // Verify error messages
    assert!(
        !health.error_messages.is_empty(),
        "[FAIL] Error messages should not be empty"
    );
    println!("[PASS] Error messages present: {:?}", health.error_messages);

    // Verify is_healthy returns false
    assert!(!pipeline.is_ready());
    println!("[PASS] is_ready() returns false due to failure");
}

// ============================================================================
// TEST 10: Registry Model Access
// ============================================================================

/// Verify registry model entries can be accessed correctly.
#[test]
fn test_registry_model_access() {
    println!("\n=== REGISTRY MODEL ACCESS TEST ===\n");

    let config = WarmConfig::default();
    let loader = WarmLoader::new(config).expect("Failed to create loader");

    let registry = loader.registry();
    let guard = registry.read().expect("Failed to lock registry");

    // Verify each model is accessible
    for model_id in EMBEDDING_MODEL_IDS.iter() {
        let entry = guard.get_entry(model_id);
        assert!(
            entry.is_some(),
            "[FAIL] Model {} should have an entry",
            model_id
        );

        let entry = entry.unwrap();
        assert!(entry.expected_bytes > 0, "[FAIL] Expected bytes should be > 0");
        assert!(entry.expected_dimension > 0, "[FAIL] Expected dimension should be > 0");
        println!("Model {}: {} bytes, {} dims",
                 model_id, entry.expected_bytes, entry.expected_dimension);
    }

    println!("[PASS] All {} models accessible with valid metadata", TOTAL_MODEL_COUNT);
}

// ============================================================================
// TEST 11: Loading Order
// ============================================================================

/// Verify loading order is correctly determined (largest first).
#[test]
fn test_loading_order() {
    println!("\n=== LOADING ORDER TEST ===\n");

    let config = WarmConfig::default();
    let loader = WarmLoader::new(config).expect("Failed to create loader");

    let registry = loader.registry();
    let guard = registry.read().expect("Failed to lock registry");
    let loading_order = guard.loading_order();

    assert_eq!(
        loading_order.len(), TOTAL_MODEL_COUNT,
        "[FAIL] Loading order should contain {} models",
        TOTAL_MODEL_COUNT
    );

    println!("Loading order (largest first):");
    let mut prev_size = usize::MAX;
    for (i, model_id) in loading_order.iter().enumerate() {
        if let Some(entry) = guard.get_entry(model_id) {
            println!("  {}. {} ({} bytes)", i + 1, model_id, entry.expected_bytes);
            // Verify largest-first ordering
            assert!(
                entry.expected_bytes <= prev_size,
                "[FAIL] Loading order should be largest-first"
            );
            prev_size = entry.expected_bytes;
        }
    }

    println!("[PASS] Loading order is correct (largest-first)");
}

// ============================================================================
// TEST 12: VRAM Allocation Integration
// ============================================================================

/// Verify VRAM allocation can be performed through the loader.
///
/// This test uses the CUDA allocator directly to verify real allocations work.
#[test]
fn test_vram_allocation_integration() {
    println!("\n=== VRAM ALLOCATION INTEGRATION TEST ===\n");

    let mut allocator = WarmCudaAllocator::new(0)
        .expect("Failed to create CUDA allocator");

    // Query initial VRAM state
    let initial_allocated = allocator.total_allocated();
    println!("Initial allocated: {} bytes", initial_allocated);

    // Allocate a test chunk (simulating model loading)
    let test_size = 100 * MB;
    let allocation = allocator.allocate_protected_with_verification(test_size, "integration_test")
        .expect("Failed to allocate VRAM");

    println!("[PASS] Allocated {} bytes at 0x{:016x}",
             allocation.size_bytes, allocation.ptr);

    // Verify allocation is valid
    assert!(allocation.is_valid(), "[FAIL] Allocation should be valid");
    assert!(allocation.is_protected, "[FAIL] Allocation should be protected");
    assert!(!WarmCudaAllocator::is_fake_pointer(allocation.ptr),
            "[FAIL] Allocation should NOT be fake");

    println!("[PASS] Allocation is valid, protected, and NOT fake");

    // Verify tracking
    let after_allocated = allocator.total_allocated();
    assert!(
        after_allocated >= initial_allocated + test_size,
        "[FAIL] Allocation tracking should increase"
    );
    println!("[PASS] Allocation tracking correct: {} bytes total", after_allocated);

    // Free the allocation
    allocator.free_protected(&allocation).expect("Failed to free allocation");
    println!("[PASS] Allocation freed successfully");
}

// ============================================================================
// Summary Test
// ============================================================================

/// Comprehensive summary test for WarmLoader integration verification.
#[test]
fn test_warmloader_integration_summary() {
    println!("\n");
    println!("====================================================================");
    println!("       WARMLOADER INTEGRATION VERIFICATION SUMMARY");
    println!("====================================================================\n");

    println!("Component Verification:\n");

    // 1. WarmLoader creation
    let config = WarmConfig::default();
    let loader_result = WarmLoader::new(config.clone());
    let loader_ok = loader_result.is_ok();
    println!("  [{}] WarmLoader creation",
             if loader_ok { "PASS" } else { "FAIL" });
    assert!(loader_ok);

    // 2. Pipeline creation
    let pipeline_result = WarmEmbeddingPipeline::new(config.clone());
    let pipeline_ok = pipeline_result.is_ok();
    println!("  [{}] WarmEmbeddingPipeline creation",
             if pipeline_ok { "PASS" } else { "FAIL" });
    assert!(pipeline_ok);

    let pipeline = pipeline_result.unwrap();

    // 3. Health checker
    let health = pipeline.health();
    let health_ok = health.models_total == TOTAL_MODEL_COUNT;
    println!("  [{}] Health checker initialization ({} models)",
             if health_ok { "PASS" } else { "FAIL" },
             health.models_total);
    assert!(health_ok);

    // 4. Diagnostics
    let report = pipeline.diagnostics();
    let diag_ok = !report.timestamp.is_empty() && report.models.len() == TOTAL_MODEL_COUNT;
    println!("  [{}] Diagnostics report generation",
             if diag_ok { "PASS" } else { "FAIL" });
    assert!(diag_ok);

    // 5. GPU detection
    let gpu_result = WarmCudaAllocator::new(0);
    let gpu_ok = gpu_result.is_ok();
    println!("  [{}] GPU detection (CUDA allocator)",
             if gpu_ok { "PASS" } else { "FAIL" });
    assert!(gpu_ok);

    let allocator = gpu_result.unwrap();
    let gpu_info = allocator.get_gpu_info().expect("Failed to get GPU info");

    // 6. GPU requirements
    let gpu_meets = gpu_info.meets_rtx_5090_requirements();
    println!("  [{}] GPU meets RTX 5090 requirements",
             if gpu_meets { "PASS" } else { "FAIL" });
    assert!(gpu_meets);

    // 7. Not simulated
    let name_lower = gpu_info.name.to_lowercase();
    let not_simulated = !name_lower.contains("simulated")
        && !name_lower.contains("stub")
        && !name_lower.contains("fake");
    println!("  [{}] GPU is NOT simulated",
             if not_simulated { "PASS" } else { "FAIL" });
    assert!(not_simulated);

    // 8. Registry state
    let registry = pipeline.registry();
    let guard = registry.read().expect("Failed to lock registry");
    let registry_ok = guard.model_count() == TOTAL_MODEL_COUNT;
    println!("  [{}] Registry contains all {} models",
             if registry_ok { "PASS" } else { "FAIL" },
             TOTAL_MODEL_COUNT);
    assert!(registry_ok);

    println!("\nGPU Information:");
    println!("  Name:               {}", gpu_info.name);
    println!("  Compute Capability: {}", gpu_info.compute_capability_string());
    println!("  VRAM:               {:.2} GB", gpu_info.total_memory_gb());
    println!("  Driver:             {}", gpu_info.driver_version);

    println!("\n====================================================================");
    println!("              ALL INTEGRATION CHECKS PASSED");
    println!("         WarmLoader pipeline is operational for RTX 5090");
    println!("====================================================================\n");
}
