//! Tests for WarmError exit codes, categories, and fatal vs non-fatal errors.

use super::helpers::{GB, MB};
use crate::warm::error::{WarmError, WarmResult};

#[test]
fn test_exit_codes_fatal_101_to_110() {
    let fatal_errors: Vec<(i32, WarmError)> = vec![
        (
            101,
            WarmError::ModelFileMissing {
                model_id: "E1_Semantic".to_string(),
                path: "/models/semantic.bin".to_string(),
            },
        ),
        (
            102,
            WarmError::ModelLoadFailed {
                model_id: "E2_Temporal".to_string(),
                reason: "Checksum mismatch".to_string(),
                bytes_read: 1024,
                file_size: 10240,
            },
        ),
        (
            103,
            WarmError::ModelValidationFailed {
                model_id: "E3_Causal".to_string(),
                reason: "Test inference produced NaN".to_string(),
                expected_output: Some("[0.1, 0.2]".to_string()),
                actual_output: Some("[NaN, NaN]".to_string()),
            },
        ),
        (
            104,
            WarmError::VramInsufficientTotal {
                required_bytes: 32 * GB,
                available_bytes: 24 * GB,
                required_gb: 32.0,
                available_gb: 24.0,
                model_breakdown: vec![("E1".to_string(), 1024)],
            },
        ),
        (
            105,
            WarmError::VramInsufficientHeadroom {
                model_bytes: 24 * GB,
                available_bytes: 28 * GB,
                headroom_required: 8 * GB,
                model_gb: 24.0,
                available_gb: 28.0,
                headroom_gb: 8.0,
            },
        ),
        (
            106,
            WarmError::CudaInitFailed {
                cuda_error: "CUDA driver not found".to_string(),
                driver_version: "".to_string(),
                gpu_name: "".to_string(),
            },
        ),
        (
            107,
            WarmError::CudaCapabilityInsufficient {
                actual_cc: "8.6".to_string(),
                required_cc: "12.0".to_string(),
                gpu_name: "RTX 3090".to_string(),
            },
        ),
        (
            108,
            WarmError::CudaAllocFailed {
                requested_bytes: GB,
                cuda_error: "CUDA_ERROR_OUT_OF_MEMORY".to_string(),
                vram_free: Some(512 * MB),
                allocation_history: vec!["model_1: 2GB".to_string()],
            },
        ),
        (
            109,
            WarmError::FakeAllocationDetected {
                detected_address: 0x7f80_0000_0000,
                tensor_name: "embedding_weights".to_string(),
                expected_pattern: "real CUDA device pointer".to_string(),
            },
        ),
        (
            110,
            WarmError::SinWaveOutputDetected {
                model_id: "E1_Semantic".to_string(),
                dominant_frequency_hz: 0.1,
                energy_concentration: 0.95,
                output_size: 768,
            },
        ),
    ];

    for (expected_code, err) in fatal_errors {
        assert_eq!(
            err.exit_code(),
            expected_code,
            "Exit code mismatch for {:?}",
            err.category()
        );
        assert!(
            err.is_fatal(),
            "Expected fatal for exit code {}",
            expected_code
        );
    }
}

#[test]
fn test_exit_codes_non_fatal() {
    let non_fatal_errors: Vec<WarmError> = vec![
        WarmError::ModelAlreadyRegistered {
            model_id: "E1".to_string(),
        },
        WarmError::ModelNotRegistered {
            model_id: "E1".to_string(),
        },
        WarmError::InvalidConfig {
            field: "vram_budget".to_string(),
            reason: "must be > 0".to_string(),
        },
        WarmError::RegistryLockPoisoned,
        WarmError::WorkingMemoryExhausted {
            requested_bytes: 1024,
            available_bytes: 512,
        },
        WarmError::CudaNotAvailable,
        WarmError::CudaQueryFailed {
            error: "Query failed".to_string(),
        },
        WarmError::DiagnosticDumpFailed {
            reason: "Permission denied".to_string(),
        },
        WarmError::LoadTimeout {
            model_id: "E1".to_string(),
            timeout_ms: 30000,
        },
        WarmError::VramAllocationFailed {
            requested_bytes: 1024,
            available_bytes: 512,
            error: "Pool exhausted".to_string(),
        },
    ];

    for err in non_fatal_errors {
        assert_eq!(
            err.exit_code(),
            1,
            "Expected exit code 1 for {:?}",
            err.category()
        );
        assert!(
            !err.is_fatal(),
            "Expected non-fatal for {:?}",
            err.category()
        );
    }
}

#[test]
fn test_error_categories() {
    assert_eq!(
        WarmError::ModelFileMissing {
            model_id: "E1".to_string(),
            path: "/".to_string()
        }
        .category(),
        "MODEL_FILE"
    );
    assert_eq!(
        WarmError::CudaInitFailed {
            cuda_error: "".to_string(),
            driver_version: "".to_string(),
            gpu_name: "".to_string()
        }
        .category(),
        "CUDA"
    );
    assert_eq!(
        WarmError::VramInsufficientTotal {
            required_bytes: 0,
            available_bytes: 0,
            required_gb: 0.0,
            available_gb: 0.0,
            model_breakdown: vec![]
        }
        .category(),
        "VRAM"
    );
}

#[test]
fn test_error_display_messages() {
    let err = WarmError::ModelFileMissing {
        model_id: "E1_Semantic".to_string(),
        path: "/models/semantic.bin".to_string(),
    };
    assert_eq!(
        format!("{}", err),
        "Model file missing: E1_Semantic not found at /models/semantic.bin"
    );

    let err = WarmError::VramInsufficientTotal {
        required_bytes: 32 * GB,
        available_bytes: 24 * GB,
        required_gb: 32.0,
        available_gb: 24.0,
        model_breakdown: vec![],
    };
    let msg = format!("{}", err);
    assert!(msg.contains("32.00GB"));
    assert!(msg.contains("24.00GB"));
}

#[test]
fn test_error_codes() {
    assert_eq!(
        WarmError::ModelFileMissing {
            model_id: "".to_string(),
            path: "".to_string()
        }
        .error_code(),
        "ERR-WARM-MODEL-MISSING"
    );
    assert_eq!(
        WarmError::CudaInitFailed {
            cuda_error: "".to_string(),
            driver_version: "".to_string(),
            gpu_name: "".to_string()
        }
        .error_code(),
        "ERR-WARM-CUDA-INIT"
    );
    assert_eq!(
        WarmError::ModelDimensionMismatch {
            model_id: "".to_string(),
            expected: 0,
            actual: 0
        }
        .error_code(),
        "ERR-WARM-MODEL-DIMENSION"
    );
    assert_eq!(
        WarmError::FakeAllocationDetected {
            detected_address: 0x7f80_0000_0000,
            tensor_name: "".to_string(),
            expected_pattern: "".to_string()
        }
        .error_code(),
        "ERR-WARM-FAKE-ALLOC"
    );
    assert_eq!(
        WarmError::SinWaveOutputDetected {
            model_id: "".to_string(),
            dominant_frequency_hz: 0.0,
            energy_concentration: 0.0,
            output_size: 0
        }
        .error_code(),
        "ERR-WARM-SINWAVE-OUTPUT"
    );
}

#[test]
fn test_warm_result_type_alias() {
    fn returns_ok() -> WarmResult<i32> {
        Ok(42)
    }

    fn returns_err() -> WarmResult<i32> {
        Err(WarmError::CudaNotAvailable)
    }

    assert_eq!(returns_ok().unwrap(), 42);
    assert!(returns_err().is_err());
}
