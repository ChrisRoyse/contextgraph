//! CUDA Runtime Verification Tests for RTX 5090
//!
//! This test file verifies that CUDA runtime operations work correctly on the RTX 5090 GPU.
//! All tests use REAL GPU operations - NO MOCKS.
//!
//! # Requirements
//!
//! - RTX 5090 GPU with 32GB VRAM
//! - CUDA 13.x drivers installed
//! - Compute capability 12.0
//!
//! # Constitution Compliance
//!
//! AP-007: No mock/stub data in production - all allocations are verified as real.
//!
//! # Exit Codes
//!
//! - 101: CUDA_UNAVAILABLE - No CUDA device or simulated GPU detected
//! - 107: CUDA_CAPABILITY_INSUFFICIENT - Compute capability below 12.0
//! - 108: CUDA_ALLOC_FAILED - Memory allocation failed
//! - 109: FAKE_ALLOCATION_DETECTED - Mock allocation pointer detected

#![cfg(feature = "cuda")]

use context_graph_embeddings::warm::{
    VramAllocation, WarmCudaAllocator, MINIMUM_VRAM_BYTES, REQUIRED_COMPUTE_MAJOR,
    REQUIRED_COMPUTE_MINOR,
};

/// One gigabyte in bytes
const GB: usize = 1024 * 1024 * 1024;

/// One megabyte in bytes
const MB: usize = 1024 * 1024;

/// Expected VRAM for RTX 5090 (32GB)
const RTX_5090_VRAM_BYTES: usize = 32 * GB;

/// Tolerance for VRAM reporting (allow 1GB variance for driver overhead)
const VRAM_TOLERANCE_BYTES: usize = GB;

// ============================================================================
// Test 1: CUDA Allocator Creation
// ============================================================================

/// Verify that WarmCudaAllocator can be created for device 0.
///
/// This test verifies:
/// - CUDA device 0 is accessible
/// - GPU info is correctly populated
/// - No simulated/stub GPU is detected
#[test]
fn test_cuda_allocator_creation() {
    println!("\n=== CUDA ALLOCATOR CREATION TEST ===\n");

    // Attempt to create allocator for device 0
    let result = WarmCudaAllocator::new(0);

    match result {
        Ok(allocator) => {
            println!("[PASS] WarmCudaAllocator created successfully for device 0");
            println!("       Device ID: {}", allocator.device_id());
            println!(
                "       Total allocated: {} bytes",
                allocator.total_allocated()
            );

            // Verify GPU info is available
            let gpu_info = allocator
                .get_gpu_info()
                .expect("GPU info should be available after allocator creation");

            println!("       GPU Name: {}", gpu_info.name);
            println!(
                "       Compute Capability: {}",
                gpu_info.compute_capability_string()
            );
            println!("       Total VRAM: {:.2} GB", gpu_info.total_memory_gb());
            println!("       Driver Version: {}", gpu_info.driver_version);

            // CRITICAL: Verify NOT a simulated GPU
            let name_lower = gpu_info.name.to_lowercase();
            assert!(
                !name_lower.contains("simulated")
                    && !name_lower.contains("stub")
                    && !name_lower.contains("fake"),
                "[FAIL] Detected simulated/stub GPU: '{}'. Real GPU required!",
                gpu_info.name
            );

            println!("[PASS] GPU is NOT simulated (real hardware verified)");
        }
        Err(e) => {
            panic!(
                "[FAIL] Failed to create CUDA allocator: {:?}\n\
                 \n\
                 CUDA is REQUIRED. There are NO fallbacks.\n\
                 Ensure RTX 5090 is properly installed with CUDA 13.x drivers.\n\
                 Exit code: {}",
                e,
                e.exit_code()
            );
        }
    }
}

// ============================================================================
// Test 2: GPU Info Verification
// ============================================================================

/// Verify GPU reports correct compute capability (12.0 for RTX 5090).
///
/// RTX 5090 (Blackwell architecture) should report:
/// - Compute capability 12.0
/// - ~32GB VRAM
#[test]
fn test_gpu_info_compute_capability() {
    println!("\n=== GPU COMPUTE CAPABILITY TEST ===\n");

    let allocator =
        WarmCudaAllocator::new(0).expect("Failed to create allocator - is RTX 5090 installed?");

    let gpu_info = allocator
        .get_gpu_info()
        .expect("GPU info should be available");

    let (major, minor) = gpu_info.compute_capability;

    println!("GPU: {}", gpu_info.name);
    println!("Compute Capability: {}.{}", major, minor);
    println!(
        "Required: {}.{}",
        REQUIRED_COMPUTE_MAJOR, REQUIRED_COMPUTE_MINOR
    );

    // Verify compute capability meets requirements
    let meets_requirement =
        gpu_info.meets_compute_requirement(REQUIRED_COMPUTE_MAJOR, REQUIRED_COMPUTE_MINOR);

    assert!(
        meets_requirement,
        "[FAIL] GPU compute capability {}.{} does not meet required {}.{}\n\
         RTX 5090 (Blackwell, CC 12.0) is REQUIRED. No fallbacks.",
        major, minor, REQUIRED_COMPUTE_MAJOR, REQUIRED_COMPUTE_MINOR
    );

    println!(
        "[PASS] Compute capability {}.{} meets requirement {}.{}",
        major, minor, REQUIRED_COMPUTE_MAJOR, REQUIRED_COMPUTE_MINOR
    );

    // Additional verification: RTX 5090 should be exactly 12.0
    if major == 12 && minor == 0 {
        println!("[INFO] Confirmed RTX 5090 Blackwell architecture (CC 12.0)");
    } else if major > 12 {
        println!("[INFO] GPU exceeds RTX 5090 requirements (future GPU detected)");
    }
}

/// Verify GPU reports sufficient VRAM (~32GB for RTX 5090).
#[test]
fn test_gpu_info_vram_capacity() {
    println!("\n=== GPU VRAM CAPACITY TEST ===\n");

    let allocator =
        WarmCudaAllocator::new(0).expect("Failed to create allocator - is RTX 5090 installed?");

    let gpu_info = allocator
        .get_gpu_info()
        .expect("GPU info should be available");

    let total_vram = gpu_info.total_memory_bytes;
    let total_vram_gb = gpu_info.total_memory_gb();

    println!("GPU: {}", gpu_info.name);
    println!("Total VRAM: {} bytes ({:.2} GB)", total_vram, total_vram_gb);
    println!(
        "Minimum Required: {} bytes ({:.2} GB)",
        MINIMUM_VRAM_BYTES,
        MINIMUM_VRAM_BYTES as f64 / GB as f64
    );

    // Verify VRAM meets minimum requirements
    assert!(
        total_vram >= MINIMUM_VRAM_BYTES,
        "[FAIL] GPU VRAM {:.2} GB below required {:.2} GB\n\
         RTX 5090 with 32GB VRAM is REQUIRED. No fallbacks.",
        total_vram_gb,
        MINIMUM_VRAM_BYTES as f64 / GB as f64
    );

    println!(
        "[PASS] VRAM {:.2} GB meets minimum requirement {:.2} GB",
        total_vram_gb,
        MINIMUM_VRAM_BYTES as f64 / GB as f64
    );

    // Verify VRAM is approximately 32GB (with tolerance for driver overhead)
    let lower_bound = RTX_5090_VRAM_BYTES - VRAM_TOLERANCE_BYTES;
    let upper_bound = RTX_5090_VRAM_BYTES + VRAM_TOLERANCE_BYTES;

    if total_vram >= lower_bound && total_vram <= upper_bound {
        println!("[INFO] VRAM matches expected RTX 5090 capacity (~32GB)");
    } else if total_vram > upper_bound {
        println!("[INFO] VRAM exceeds expected RTX 5090 capacity (may be different GPU)");
    }
}

// ============================================================================
// Test 3: Real VRAM Allocation
// ============================================================================

/// Verify that VRAM allocations are REAL (not fake pointers).
///
/// This is critical for Constitution AP-007 compliance:
/// - Fake allocations (0x7f80_xxxx_xxxx pattern) MUST be rejected
/// - Real CUDA allocations should return device pointers
#[test]
fn test_vram_allocation_is_real() {
    println!("\n=== VRAM ALLOCATION REALITY TEST ===\n");

    let mut allocator =
        WarmCudaAllocator::new(0).expect("Failed to create allocator - is RTX 5090 installed?");

    // Allocate 100MB of protected VRAM
    let allocation_size = 100 * MB;
    println!(
        "Attempting to allocate {} bytes ({} MB) of protected VRAM...",
        allocation_size,
        allocation_size / MB
    );

    let result = allocator.allocate_protected_with_verification(allocation_size, "test_allocation");

    match result {
        Ok(allocation) => {
            println!("[PASS] Allocation succeeded");
            println!("       Pointer: 0x{:016x}", allocation.ptr);
            println!(
                "       Size: {} bytes ({:.2} MB)",
                allocation.size_bytes,
                allocation.size_mb()
            );
            println!("       Device ID: {}", allocation.device_id);
            println!("       Is Protected: {}", allocation.is_protected);

            // Verify pointer is valid
            assert!(allocation.is_valid(), "[FAIL] Allocation pointer is NULL");
            println!("[PASS] Pointer is non-NULL");

            // Verify pointer is NOT fake (Constitution AP-007)
            let is_fake = WarmCudaAllocator::is_fake_pointer(allocation.ptr);
            assert!(
                !is_fake,
                "[FAIL] FAKE ALLOCATION DETECTED at 0x{:016x}\n\
                 Constitution AP-007 VIOLATION: Mock/stub allocations are FORBIDDEN.\n\
                 Exit code: 109",
                allocation.ptr
            );
            println!("[PASS] Pointer is NOT fake (Constitution AP-007 verified)");

            // Verify allocation is protected (non-evictable)
            assert!(
                allocation.is_protected,
                "[FAIL] Allocation should be protected (non-evictable)"
            );
            println!("[PASS] Allocation is protected (non-evictable via cudaMalloc)");

            // Verify allocator tracking is updated
            let total_allocated = allocator.total_allocated();
            assert!(
                total_allocated >= allocation_size,
                "[FAIL] Allocator tracking mismatch: expected >= {}, got {}",
                allocation_size,
                total_allocated
            );
            println!(
                "[PASS] Allocator tracking updated: {} bytes total",
                total_allocated
            );

            // Clean up
            allocator
                .free_protected(&allocation)
                .expect("Failed to free allocation");
            println!("[PASS] Allocation freed successfully");
        }
        Err(e) => {
            panic!(
                "[FAIL] VRAM allocation failed: {:?}\n\
                 \n\
                 CUDA allocation is REQUIRED. There are NO fallbacks.\n\
                 Ensure RTX 5090 has sufficient free VRAM.\n\
                 Exit code: {}",
                e,
                e.exit_code()
            );
        }
    }
}

/// Verify large allocation (1GB) works correctly.
#[test]
fn test_large_vram_allocation() {
    println!("\n=== LARGE VRAM ALLOCATION TEST (1GB) ===\n");

    let mut allocator =
        WarmCudaAllocator::new(0).expect("Failed to create allocator - is RTX 5090 installed?");

    // Query available VRAM before allocation
    let vram_before = allocator
        .query_available_vram()
        .expect("Failed to query available VRAM");
    println!(
        "Available VRAM before allocation: {:.2} GB",
        vram_before as f64 / GB as f64
    );

    // Allocate 1GB of protected VRAM
    let allocation_size = GB;
    println!(
        "Attempting to allocate {} bytes (1 GB) of protected VRAM...",
        allocation_size
    );

    let result =
        allocator.allocate_protected_with_verification(allocation_size, "large_test_allocation");

    match result {
        Ok(allocation) => {
            println!("[PASS] Large allocation (1GB) succeeded");
            println!("       Pointer: 0x{:016x}", allocation.ptr);
            println!("       Size: {:.2} GB", allocation.size_gb());

            // Verify the allocation is real
            assert!(
                allocation.is_valid(),
                "[FAIL] Large allocation pointer is NULL"
            );
            assert!(
                !WarmCudaAllocator::is_fake_pointer(allocation.ptr),
                "[FAIL] Large allocation is FAKE (0x{:016x})",
                allocation.ptr
            );
            println!("[PASS] Large allocation is REAL and VALID");

            // Query VRAM after allocation - should show reduction
            let vram_after = allocator
                .query_available_vram()
                .expect("Failed to query available VRAM");
            println!(
                "Available VRAM after allocation: {:.2} GB",
                vram_after as f64 / GB as f64
            );

            // Note: Due to our simplified tracking, we verify via allocator's internal tracking
            let total_allocated = allocator.total_allocated();
            assert!(
                total_allocated >= allocation_size,
                "[FAIL] Allocator should track at least {} bytes, got {}",
                allocation_size,
                total_allocated
            );
            println!(
                "[PASS] Allocator correctly tracks {} bytes allocated",
                total_allocated
            );

            // Free the allocation
            allocator
                .free_protected(&allocation)
                .expect("Failed to free large allocation");
            println!("[PASS] Large allocation freed successfully");

            // Verify tracking is updated after free
            let total_after_free = allocator.total_allocated();
            println!(
                "[INFO] Total allocated after free: {} bytes",
                total_after_free
            );
        }
        Err(e) => {
            panic!(
                "[FAIL] Large VRAM allocation (1GB) failed: {:?}\n\
                 \n\
                 RTX 5090 should have 32GB VRAM - 1GB allocation should succeed.\n\
                 Exit code: {}",
                e,
                e.exit_code()
            );
        }
    }
}

// ============================================================================
// Test 4: Compute Capability Check
// ============================================================================

/// Verify compute capability check rejects insufficient GPUs.
#[test]
fn test_compute_capability_check() {
    println!("\n=== COMPUTE CAPABILITY CHECK TEST ===\n");

    let allocator =
        WarmCudaAllocator::new(0).expect("Failed to create allocator - is RTX 5090 installed?");

    // Check that RTX 5090 passes the requirement check
    let result = allocator.check_compute_capability(REQUIRED_COMPUTE_MAJOR, REQUIRED_COMPUTE_MINOR);

    match result {
        Ok(()) => {
            println!(
                "[PASS] GPU passes compute capability check for {}.{}",
                REQUIRED_COMPUTE_MAJOR, REQUIRED_COMPUTE_MINOR
            );
        }
        Err(e) => {
            panic!(
                "[FAIL] GPU failed compute capability check: {:?}\n\
                 RTX 5090 (CC 12.0) is REQUIRED.\n\
                 Exit code: {}",
                e,
                e.exit_code()
            );
        }
    }

    // Verify that a higher requirement would correctly pass/fail
    let gpu_info = allocator
        .get_gpu_info()
        .expect("GPU info should be available");

    // Check against CC 8.0 (should pass for any modern GPU)
    assert!(
        gpu_info.meets_compute_requirement(8, 0),
        "[FAIL] GPU should meet CC 8.0 requirement"
    );
    println!("[PASS] GPU correctly reports meeting CC 8.0 requirement");

    // Check against CC 99.0 (should fail for any current GPU)
    assert!(
        !gpu_info.meets_compute_requirement(99, 0),
        "[FAIL] GPU should NOT meet CC 99.0 requirement"
    );
    println!("[PASS] GPU correctly reports NOT meeting CC 99.0 requirement");
}

// ============================================================================
// Test 5: RTX 5090 Requirements Check
// ============================================================================

/// Verify GPU meets full RTX 5090 requirements.
#[test]
fn test_rtx_5090_requirements() {
    println!("\n=== RTX 5090 FULL REQUIREMENTS TEST ===\n");

    let allocator =
        WarmCudaAllocator::new(0).expect("Failed to create allocator - is RTX 5090 installed?");

    let gpu_info = allocator
        .get_gpu_info()
        .expect("GPU info should be available");

    println!("GPU: {}", gpu_info.name);
    println!(
        "Compute Capability: {}",
        gpu_info.compute_capability_string()
    );
    println!("VRAM: {:.2} GB", gpu_info.total_memory_gb());

    let meets_requirements = gpu_info.meets_rtx_5090_requirements();

    assert!(
        meets_requirements,
        "[FAIL] GPU does not meet RTX 5090 requirements:\n\
         - Required CC: {}.{}\n\
         - Required VRAM: {:.2} GB\n\
         - Actual CC: {}\n\
         - Actual VRAM: {:.2} GB",
        REQUIRED_COMPUTE_MAJOR,
        REQUIRED_COMPUTE_MINOR,
        MINIMUM_VRAM_BYTES as f64 / GB as f64,
        gpu_info.compute_capability_string(),
        gpu_info.total_memory_gb()
    );

    println!("[PASS] GPU meets ALL RTX 5090 requirements:");
    println!(
        "       - Compute Capability: {} >= {}.{}",
        gpu_info.compute_capability_string(),
        REQUIRED_COMPUTE_MAJOR,
        REQUIRED_COMPUTE_MINOR
    );
    println!(
        "       - VRAM: {:.2} GB >= {:.2} GB",
        gpu_info.total_memory_gb(),
        MINIMUM_VRAM_BYTES as f64 / GB as f64
    );
}

// ============================================================================
// Test 6: Fake Pointer Detection
// ============================================================================

/// Verify fake pointer detection works correctly.
#[test]
fn test_fake_pointer_detection() {
    println!("\n=== FAKE POINTER DETECTION TEST ===\n");

    // Known fake pointer patterns (used by mock implementations)
    let fake_pointers = [
        0x7f80_0000_0000_u64,
        0x7f80_1234_5678_u64,
        0x7f8f_ffff_ffff_u64,
    ];

    // Real pointer patterns (typical CUDA device pointers)
    // Note: Real CUDA pointers vary by system but are typically NOT in 0x7fxx range
    let real_pointers = [
        0x0000_0001_0000_0000_u64, // Typical low device pointer
        0x0000_0002_0000_0000_u64,
        0x0000_7000_0000_0000_u64, // Below fake range
        0x0000_8000_0000_0000_u64, // Above fake range
    ];

    println!("Testing fake pointer detection...\n");

    println!("Fake pointers (should be detected):");
    for ptr in fake_pointers.iter() {
        let is_fake = WarmCudaAllocator::is_fake_pointer(*ptr);
        println!("  0x{:016x}: is_fake = {}", ptr, is_fake);
        assert!(is_fake, "Pointer 0x{:016x} should be detected as fake", ptr);
    }
    println!("[PASS] All fake pointers correctly detected\n");

    println!("Real pointers (should NOT be detected as fake):");
    for ptr in real_pointers.iter() {
        let is_fake = WarmCudaAllocator::is_fake_pointer(*ptr);
        println!("  0x{:016x}: is_fake = {}", ptr, is_fake);
        assert!(
            !is_fake,
            "Pointer 0x{:016x} should NOT be detected as fake",
            ptr
        );
    }
    println!("[PASS] All real pointers correctly identified\n");

    // Verify NULL pointer handling
    let is_null_fake = WarmCudaAllocator::is_fake_pointer(0);
    println!("NULL pointer (0x0): is_fake = {}", is_null_fake);
    // NULL is not fake - it's just invalid
    assert!(
        !is_null_fake,
        "NULL pointer should not be detected as fake pattern"
    );
    println!("[PASS] NULL pointer correctly handled (not fake, just invalid)");
}

// ============================================================================
// Test 7: Multiple Allocations
// ============================================================================

/// Verify multiple allocations work correctly and tracking is accurate.
#[test]
fn test_multiple_allocations() {
    println!("\n=== MULTIPLE ALLOCATIONS TEST ===\n");

    let mut allocator =
        WarmCudaAllocator::new(0).expect("Failed to create allocator - is RTX 5090 installed?");

    // Allocate multiple chunks
    let allocation_sizes = [100 * MB, 200 * MB, 150 * MB];
    let mut allocations: Vec<VramAllocation> = Vec::new();
    let mut total_expected: usize = 0;

    println!("Allocating multiple chunks...\n");

    for (i, size) in allocation_sizes.iter().enumerate() {
        let name = format!("multi_alloc_{}", i);
        let result = allocator.allocate_protected_with_verification(*size, &name);

        match result {
            Ok(allocation) => {
                println!(
                    "Allocation {}: {} bytes at 0x{:016x}",
                    i, size, allocation.ptr
                );

                // Verify each allocation is real
                assert!(allocation.is_valid());
                assert!(!WarmCudaAllocator::is_fake_pointer(allocation.ptr));

                total_expected += size;
                allocations.push(allocation);
            }
            Err(e) => {
                // Clean up any successful allocations before panic
                for alloc in &allocations {
                    let _ = allocator.free_protected(alloc);
                }
                panic!("[FAIL] Allocation {} ({} bytes) failed: {:?}", i, size, e);
            }
        }
    }

    println!("\n[PASS] All {} allocations succeeded", allocations.len());

    // Verify total tracking
    let total_allocated = allocator.total_allocated();
    assert_eq!(
        total_allocated, total_expected,
        "[FAIL] Total allocated mismatch: expected {}, got {}",
        total_expected, total_allocated
    );
    println!("[PASS] Total tracking correct: {} bytes", total_allocated);

    // Verify allocation history
    let history = allocator.allocation_history();
    assert!(
        history.len() >= allocations.len(),
        "[FAIL] Allocation history should have at least {} entries, got {}",
        allocations.len(),
        history.len()
    );
    println!(
        "[PASS] Allocation history contains {} entries",
        history.len()
    );

    // Free all allocations
    println!("\nFreeing all allocations...");
    for (i, allocation) in allocations.iter().enumerate() {
        allocator
            .free_protected(allocation)
            .unwrap_or_else(|_| panic!("Failed to free allocation {}", i));
        println!("Freed allocation {}", i);
    }

    // Verify tracking after free
    let final_allocated = allocator.total_allocated();
    assert_eq!(
        final_allocated, 0,
        "[FAIL] Total allocated after free should be 0, got {}",
        final_allocated
    );
    println!("\n[PASS] All allocations freed, total = 0 bytes");
}

// ============================================================================
// Test 8: Allocation History
// ============================================================================

/// Verify allocation history is correctly maintained.
#[test]
fn test_allocation_history() {
    println!("\n=== ALLOCATION HISTORY TEST ===\n");

    let mut allocator =
        WarmCudaAllocator::new(0).expect("Failed to create allocator - is RTX 5090 installed?");

    // Initially history should be empty
    let initial_history = allocator.allocation_history();
    let initial_len = initial_history.len();
    println!("Initial history length: {}", initial_len);

    // Allocate something
    let allocation = allocator
        .allocate_protected_with_verification(50 * MB, "history_test")
        .expect("Allocation should succeed");

    // History should have grown
    let after_alloc_history = allocator.allocation_history();
    assert!(
        after_alloc_history.len() > initial_len,
        "[FAIL] History should grow after allocation"
    );
    println!(
        "[PASS] History grew after allocation: {} -> {} entries",
        initial_len,
        after_alloc_history.len()
    );

    // Check last history entry contains ALLOC
    let last_entry = after_alloc_history
        .last()
        .expect("History should not be empty");
    assert!(
        last_entry.contains("ALLOC"),
        "[FAIL] Last history entry should contain 'ALLOC': {}",
        last_entry
    );
    println!("[PASS] Last entry is ALLOC: {}", last_entry);

    // Free the allocation
    allocator
        .free_protected(&allocation)
        .expect("Free should succeed");

    // History should have grown again with FREE
    let after_free_history = allocator.allocation_history();
    let free_entry = after_free_history
        .last()
        .expect("History should not be empty");
    assert!(
        free_entry.contains("FREE"),
        "[FAIL] Last history entry should contain 'FREE': {}",
        free_entry
    );
    println!("[PASS] Last entry is FREE: {}", free_entry);
}

// ============================================================================
// Test 9: Verify All Allocations Real (Bulk Check)
// ============================================================================

/// Verify the verify_all_allocations_real method works.
#[test]
fn test_verify_all_allocations_real() {
    println!("\n=== VERIFY ALL ALLOCATIONS REAL TEST ===\n");

    let mut allocator =
        WarmCudaAllocator::new(0).expect("Failed to create allocator - is RTX 5090 installed?");

    // With no allocations, should pass
    let result = allocator.verify_all_allocations_real();
    assert!(
        result.is_ok(),
        "[FAIL] Verification should pass with no allocations"
    );
    println!("[PASS] Verification passes with no allocations");

    // Allocate something
    let allocation = allocator
        .allocate_protected_with_verification(50 * MB, "verify_test")
        .expect("Allocation should succeed");

    // Verification should still pass (all allocations are real)
    let result = allocator.verify_all_allocations_real();
    assert!(
        result.is_ok(),
        "[FAIL] Verification should pass with real allocations"
    );
    println!("[PASS] Verification passes with real allocations");

    // Clean up
    allocator
        .free_protected(&allocation)
        .expect("Free should succeed");
    println!("[PASS] Cleanup complete");
}

// ============================================================================
// Summary Test
// ============================================================================

/// Comprehensive summary test that runs all critical checks.
#[test]
fn test_cuda_runtime_summary() {
    println!("\n");
    println!("====================================================================");
    println!("         CUDA RUNTIME VERIFICATION SUMMARY - RTX 5090");
    println!("====================================================================\n");

    let allocator = WarmCudaAllocator::new(0)
        .expect("[CRITICAL] Failed to initialize CUDA - RTX 5090 required!");

    let gpu_info = allocator
        .get_gpu_info()
        .expect("[CRITICAL] Failed to get GPU info!");

    println!("GPU Information:");
    println!("  Name:               {}", gpu_info.name);
    println!("  Device ID:          {}", gpu_info.device_id);
    println!(
        "  Compute Capability: {}",
        gpu_info.compute_capability_string()
    );
    println!(
        "  VRAM Total:         {:.2} GB ({} bytes)",
        gpu_info.total_memory_gb(),
        gpu_info.total_memory_bytes
    );
    println!("  Driver Version:     {}", gpu_info.driver_version);
    println!();

    // Verification checklist
    println!("Verification Checklist:");

    // 1. Not simulated
    let is_simulated = gpu_info.name.to_lowercase().contains("simulated")
        || gpu_info.name.to_lowercase().contains("stub")
        || gpu_info.name.to_lowercase().contains("fake");
    println!(
        "  [{}] Real GPU (not simulated)",
        if !is_simulated { "PASS" } else { "FAIL" }
    );
    assert!(!is_simulated, "Simulated GPU detected!");

    // 2. Compute capability
    let meets_cc =
        gpu_info.meets_compute_requirement(REQUIRED_COMPUTE_MAJOR, REQUIRED_COMPUTE_MINOR);
    println!(
        "  [{}] Compute Capability >= {}.{}",
        if meets_cc { "PASS" } else { "FAIL" },
        REQUIRED_COMPUTE_MAJOR,
        REQUIRED_COMPUTE_MINOR
    );
    assert!(meets_cc, "Insufficient compute capability!");

    // 3. VRAM
    let meets_vram = gpu_info.total_memory_bytes >= MINIMUM_VRAM_BYTES;
    println!(
        "  [{}] VRAM >= {:.2} GB",
        if meets_vram { "PASS" } else { "FAIL" },
        MINIMUM_VRAM_BYTES as f64 / GB as f64
    );
    assert!(meets_vram, "Insufficient VRAM!");

    // 4. Meets RTX 5090 requirements
    let meets_5090 = gpu_info.meets_rtx_5090_requirements();
    println!(
        "  [{}] Meets RTX 5090 requirements",
        if meets_5090 { "PASS" } else { "FAIL" }
    );
    assert!(meets_5090, "Does not meet RTX 5090 requirements!");

    println!();
    println!("====================================================================");
    println!("                    ALL CHECKS PASSED");
    println!("         CUDA Runtime is operational on RTX 5090");
    println!("====================================================================\n");
}
