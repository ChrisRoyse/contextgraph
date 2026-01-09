//! Tests for CUDA allocation module.

use super::*;

// ============================================================================
// GpuInfo Tests
// ============================================================================

#[test]
fn test_gpu_info_construction() {
    let info = GpuInfo::new(
        0,
        "NVIDIA GeForce RTX 5090".to_string(),
        (12, 0),
        32 * GB,
        "13.1.0".to_string(),
    );

    assert_eq!(info.device_id, 0);
    assert_eq!(info.name, "NVIDIA GeForce RTX 5090");
    assert_eq!(info.compute_capability, (12, 0));
    assert_eq!(info.total_memory_bytes, 32 * GB);
    assert_eq!(info.driver_version, "13.1.0");
}

#[test]
fn test_gpu_info_default() {
    let info = GpuInfo::default();

    assert_eq!(info.device_id, 0);
    assert_eq!(info.name, "No GPU");
    assert_eq!(info.compute_capability, (0, 0));
    assert_eq!(info.total_memory_bytes, 0);
    assert_eq!(info.driver_version, "N/A");
}

#[test]
fn test_gpu_info_compute_capability_string() {
    let info = GpuInfo::new(
        0,
        "RTX 5090".to_string(),
        (12, 0),
        32 * GB,
        "13.1.0".to_string(),
    );

    assert_eq!(info.compute_capability_string(), "12.0");

    let info_89 = GpuInfo::new(
        0,
        "RTX 4090".to_string(),
        (8, 9),
        24 * GB,
        "12.0.0".to_string(),
    );

    assert_eq!(info_89.compute_capability_string(), "8.9");
}

#[test]
fn test_gpu_info_total_memory_gb() {
    let info = GpuInfo::new(
        0,
        "RTX 5090".to_string(),
        (12, 0),
        32 * GB,
        "13.1.0".to_string(),
    );

    assert!((info.total_memory_gb() - 32.0).abs() < 0.01);
}

#[test]
fn test_gpu_info_meets_compute_requirement() {
    let rtx_5090 = GpuInfo::new(
        0,
        "RTX 5090".to_string(),
        (12, 0),
        32 * GB,
        "13.1.0".to_string(),
    );

    // Exact match
    assert!(rtx_5090.meets_compute_requirement(12, 0));

    // Higher major version meets requirement
    assert!(rtx_5090.meets_compute_requirement(11, 0));
    assert!(rtx_5090.meets_compute_requirement(8, 9));

    // Same major, lower minor meets requirement
    // (12.0 >= 12.0, so this is true)

    // Higher requirement not met
    assert!(!rtx_5090.meets_compute_requirement(13, 0));
    assert!(!rtx_5090.meets_compute_requirement(12, 1));

    // RTX 4090 case
    let rtx_4090 = GpuInfo::new(
        0,
        "RTX 4090".to_string(),
        (8, 9),
        24 * GB,
        "12.0.0".to_string(),
    );

    assert!(rtx_4090.meets_compute_requirement(8, 9));
    assert!(rtx_4090.meets_compute_requirement(8, 0));
    assert!(rtx_4090.meets_compute_requirement(7, 5));
    assert!(!rtx_4090.meets_compute_requirement(12, 0));
}

#[test]
fn test_gpu_info_meets_rtx_5090_requirements() {
    let rtx_5090 = GpuInfo::new(
        0,
        "RTX 5090".to_string(),
        (12, 0),
        32 * GB,
        "13.1.0".to_string(),
    );

    assert!(rtx_5090.meets_rtx_5090_requirements());

    // Insufficient VRAM
    let low_vram = GpuInfo::new(
        0,
        "RTX 5090".to_string(),
        (12, 0),
        24 * GB, // Only 24GB
        "13.1.0".to_string(),
    );

    assert!(!low_vram.meets_rtx_5090_requirements());

    // Insufficient compute capability
    let old_gpu = GpuInfo::new(
        0,
        "RTX 4090".to_string(),
        (8, 9),
        32 * GB,
        "12.0.0".to_string(),
    );

    assert!(!old_gpu.meets_rtx_5090_requirements());
}

// ============================================================================
// VramAllocation Tests
// ============================================================================

#[test]
fn test_vram_allocation_protected() {
    let alloc = VramAllocation::new_protected(0x1000_0000, 800_000_000, 0);

    assert_eq!(alloc.ptr, 0x1000_0000);
    assert_eq!(alloc.size_bytes, 800_000_000);
    assert_eq!(alloc.device_id, 0);
    assert!(alloc.is_protected);
    assert!(alloc.is_valid());
}

#[test]
fn test_vram_allocation_evictable() {
    let alloc = VramAllocation::new_evictable(0x2000_0000, 1_000_000, 1);

    assert_eq!(alloc.ptr, 0x2000_0000);
    assert_eq!(alloc.size_bytes, 1_000_000);
    assert_eq!(alloc.device_id, 1);
    assert!(!alloc.is_protected);
    assert!(alloc.is_valid());
}

#[test]
fn test_vram_allocation_default() {
    let alloc = VramAllocation::default();

    assert_eq!(alloc.ptr, 0);
    assert_eq!(alloc.size_bytes, 0);
    assert_eq!(alloc.device_id, 0);
    assert!(!alloc.is_protected);
    assert!(!alloc.is_valid()); // Null pointer is invalid
}

#[test]
fn test_vram_allocation_size_conversions() {
    let alloc = VramAllocation::new_protected(0x1000, 1_073_741_824, 0); // 1GB

    assert!((alloc.size_mb() - 1024.0).abs() < 0.01);
    assert!((alloc.size_gb() - 1.0).abs() < 0.01);
}

// ============================================================================
// NOTE: Stub tests REMOVED - CUDA is ALWAYS required (RTX 5090)
// Per constitution: No fallback stubs, fail-fast architecture
// ============================================================================

// ============================================================================
// Helper Function Tests
// ============================================================================

#[test]
fn test_format_bytes() {
    assert_eq!(format_bytes(0), "0B");
    assert_eq!(format_bytes(512), "512B");
    assert_eq!(format_bytes(1024), "1.00KB");
    assert_eq!(format_bytes(1536), "1.50KB");
    assert_eq!(format_bytes(1024 * 1024), "1.00MB");
    assert_eq!(format_bytes(1500 * 1024 * 1024), "1.46GB");
    assert_eq!(format_bytes(32 * GB), "32.00GB");
}

// ============================================================================
// Constant Tests
// ============================================================================

#[test]
fn test_constants() {
    assert_eq!(REQUIRED_COMPUTE_MAJOR, 12);
    assert_eq!(REQUIRED_COMPUTE_MINOR, 0);
    // RTX 5090 has 32GB GDDR7 but reports ~31.84GB usable due to driver/OS reservations.
    // The minimum is set to 31GB to account for this variance.
    assert_eq!(MINIMUM_VRAM_BYTES, 31 * 1024 * 1024 * 1024);
}

// ============================================================================
// Fake Allocation Detection Tests (TASK-EMB-017)
// ============================================================================

#[test]
fn test_fake_allocation_constant() {
    // Verify the fake allocation pattern is correctly defined
    assert_eq!(FAKE_ALLOCATION_BASE_PATTERN, 0x7f80_0000_0000u64);
}

#[test]
fn test_is_fake_pointer_detects_fake_pattern() {
    // The exact fake pattern MUST be detected
    assert!(
        WarmCudaAllocator::is_fake_pointer(FAKE_ALLOCATION_BASE_PATTERN),
        "Fake pointer pattern 0x7f80_0000_0000 MUST be detected"
    );

    // Variations in the fake range should also be detected
    assert!(
        WarmCudaAllocator::is_fake_pointer(0x7f80_0000_1000),
        "Fake pointer with offset MUST be detected"
    );

    assert!(
        WarmCudaAllocator::is_fake_pointer(0x7f80_1234_5678),
        "Fake pointer variation MUST be detected"
    );
}

#[test]
fn test_is_fake_pointer_allows_real_pointers() {
    // Real CUDA pointers are typically in lower address ranges
    assert!(
        !WarmCudaAllocator::is_fake_pointer(0x0000_7fff_0000_1000),
        "Real low-address pointer should NOT be detected as fake"
    );

    // NULL pointer is not fake (it's invalid, but not fake)
    assert!(
        !WarmCudaAllocator::is_fake_pointer(0),
        "NULL pointer should NOT be detected as fake pattern"
    );

    // High device addresses that don't match fake pattern
    assert!(
        !WarmCudaAllocator::is_fake_pointer(0x0001_0000_0000_0000),
        "Different high address should NOT be detected as fake"
    );
}

#[test]
fn test_verify_real_allocation_rejects_null() {
    let result = WarmCudaAllocator::verify_real_allocation(0, "test_tensor");

    assert!(result.is_err(), "NULL pointer should be rejected");

    match result.unwrap_err() {
        crate::warm::error::WarmError::CudaAllocFailed { cuda_error, .. } => {
            assert!(cuda_error.contains("NULL"), "Error should mention NULL");
        }
        other => panic!("Expected CudaAllocFailed, got {:?}", other),
    }
}

#[test]
fn test_verify_real_allocation_rejects_fake_pattern() {
    let result =
        WarmCudaAllocator::verify_real_allocation(FAKE_ALLOCATION_BASE_PATTERN, "embedding_tensor");

    assert!(result.is_err(), "Fake pointer MUST be rejected");

    match result.unwrap_err() {
        crate::warm::error::WarmError::FakeAllocationDetected {
            detected_address,
            tensor_name,
            ..
        } => {
            assert_eq!(detected_address, FAKE_ALLOCATION_BASE_PATTERN);
            assert_eq!(tensor_name, "embedding_tensor");
        }
        other => panic!(
            "Expected FakeAllocationDetected (exit 109), got {:?}",
            other
        ),
    }
}

#[test]
fn test_verify_real_allocation_accepts_valid_pointer() {
    // A valid-looking CUDA pointer
    let valid_ptr = 0x0000_7fff_8000_0000u64;

    let result = WarmCudaAllocator::verify_real_allocation(valid_ptr, "real_tensor");

    assert!(
        result.is_ok(),
        "Valid pointer should be accepted: {:?}",
        result
    );
}

#[test]
fn test_fake_allocation_exit_code_is_109() {
    use crate::warm::error::WarmError;

    let error = WarmError::FakeAllocationDetected {
        detected_address: 0x7f80_0000_0000,
        tensor_name: "test".to_string(),
        expected_pattern: "real".to_string(),
    };

    assert_eq!(
        error.exit_code(),
        109,
        "FakeAllocationDetected MUST have exit code 109"
    );
    assert!(error.is_fatal(), "FakeAllocationDetected MUST be fatal");
    assert_eq!(
        error.category(),
        "FAKE_DETECTION",
        "FakeAllocationDetected MUST have FAKE_DETECTION category"
    );
}

#[test]
fn test_sin_wave_exit_code_is_110() {
    use crate::warm::error::WarmError;

    let error = WarmError::SinWaveOutputDetected {
        model_id: "test_model".to_string(),
        dominant_frequency_hz: 0.1,
        energy_concentration: 0.95,
        output_size: 768,
    };

    assert_eq!(
        error.exit_code(),
        110,
        "SinWaveOutputDetected MUST have exit code 110"
    );
    assert!(error.is_fatal(), "SinWaveOutputDetected MUST be fatal");
    assert_eq!(
        error.category(),
        "FAKE_DETECTION",
        "SinWaveOutputDetected MUST have FAKE_DETECTION category"
    );
}

// ============================================================================
// Sin Wave Energy Threshold Tests
// ============================================================================

#[test]
fn test_sin_wave_energy_threshold() {
    assert!(
        (SIN_WAVE_ENERGY_THRESHOLD - 0.80).abs() < 0.001,
        "Sin wave energy threshold should be 80%"
    );
}

#[test]
fn test_golden_similarity_threshold() {
    assert!(
        (GOLDEN_SIMILARITY_THRESHOLD - 0.99).abs() < 0.001,
        "Golden similarity threshold should be 0.99"
    );
}
