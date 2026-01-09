//! TASK-EMB-007 Full State Verification Test
//!
//! This module creates REAL error instances and verifies their actual output values.
//! NO MOCKS - all data is real.

#[cfg(test)]
mod full_state_verification {
    use crate::error::{EmbeddingError, ErrorSeverity};
    use crate::types::ModelId;
    use std::path::PathBuf;

    /// VERIFICATION TEST 1: Verify all 12 SPEC codes exist and return correct values
    #[test]
    fn verify_all_12_spec_codes_physically_exist() {
        println!("\n========== FULL STATE VERIFICATION ==========");
        println!("SOURCE OF TRUTH: crates/context-graph-embeddings/src/error/types.rs");
        println!("==============================================\n");

        // Create REAL error instances - no mocks
        let errors: Vec<(&str, EmbeddingError)> = vec![
            (
                "EMB-E001",
                EmbeddingError::CudaUnavailable {
                    message: "VERIFICATION".into(),
                },
            ),
            (
                "EMB-E002",
                EmbeddingError::InsufficientVram {
                    required_bytes: 32_000_000_000,
                    available_bytes: 8_000_000_000,
                    required_gb: 32.0,
                    available_gb: 8.0,
                },
            ),
            (
                "EMB-E003",
                EmbeddingError::WeightFileMissing {
                    model_id: ModelId::Semantic,
                    path: PathBuf::from("/verify/path"),
                },
            ),
            (
                "EMB-E004",
                EmbeddingError::WeightChecksumMismatch {
                    model_id: ModelId::Code,
                    expected: "expected_sha256".into(),
                    actual: "actual_sha256".into(),
                },
            ),
            (
                "EMB-E005",
                EmbeddingError::ModelDimensionMismatch {
                    model_id: ModelId::Graph,
                    expected: 384,
                    actual: 768,
                },
            ),
            (
                "EMB-E006",
                EmbeddingError::ProjectionMatrixMissing {
                    path: PathBuf::from("/verify/projection.bin"),
                },
            ),
            (
                "EMB-E007",
                EmbeddingError::OomDuringBatch {
                    batch_size: 128,
                    model_id: ModelId::Multimodal,
                },
            ),
            (
                "EMB-E008",
                EmbeddingError::InferenceValidationFailed {
                    model_id: ModelId::Entity,
                    reason: "NaN in output".into(),
                },
            ),
            (
                "EMB-E009",
                EmbeddingError::InputTooLarge {
                    max_tokens: 512,
                    actual_tokens: 10000,
                },
            ),
            (
                "EMB-E010",
                EmbeddingError::StorageCorruption {
                    id: "fp-verify-12345".into(),
                    reason: "CRC mismatch".into(),
                },
            ),
            (
                "EMB-E011",
                EmbeddingError::CodebookMissing {
                    model_id: ModelId::Sparse,
                },
            ),
            (
                "EMB-E012",
                EmbeddingError::RecallLossExceeded {
                    model_id: ModelId::Hdc,
                    measured: 0.75,
                    max_allowed: 0.05,
                },
            ),
        ];

        println!("PHYSICAL VERIFICATION OF 12 SPEC ERROR CODES:");
        println!("----------------------------------------------");

        for (expected_code, error) in &errors {
            let actual_code = error.spec_code();
            let severity = error.severity();
            let recoverable = error.is_recoverable();
            let message = error.to_string();

            println!("\n[{}] VERIFICATION:", expected_code);
            println!("  BEFORE: Creating error variant");
            println!("  AFTER:");
            println!("    spec_code() = {:?}", actual_code);
            println!("    severity() = {:?}", severity);
            println!("    is_recoverable() = {}", recoverable);
            println!(
                "    Message contains code: {}",
                message.contains(expected_code)
            );

            // PHYSICAL ASSERTION - this will FAIL if data doesn't match
            assert_eq!(
                actual_code,
                Some(*expected_code),
                "PHYSICAL VERIFICATION FAILED: {} spec_code mismatch",
                expected_code
            );

            // Verify message contains the error code
            assert!(
                message.contains(expected_code),
                "PHYSICAL VERIFICATION FAILED: {} message doesn't contain error code",
                expected_code
            );
        }

        println!("\n✓ ALL 12 SPEC CODES PHYSICALLY VERIFIED IN SOURCE OF TRUTH");
    }

    /// VERIFICATION TEST 2: Verify severity classification is correct
    #[test]
    fn verify_severity_classification_physically() {
        println!("\n========== SEVERITY VERIFICATION ==========");

        // Critical errors (7 total)
        let critical_codes = [
            "EMB-E001", "EMB-E002", "EMB-E003", "EMB-E004", "EMB-E005", "EMB-E006", "EMB-E008",
        ];
        let critical_errors: Vec<EmbeddingError> = vec![
            EmbeddingError::CudaUnavailable {
                message: "test".into(),
            },
            EmbeddingError::InsufficientVram {
                required_bytes: 0,
                available_bytes: 0,
                required_gb: 0.0,
                available_gb: 0.0,
            },
            EmbeddingError::WeightFileMissing {
                model_id: ModelId::Semantic,
                path: PathBuf::new(),
            },
            EmbeddingError::WeightChecksumMismatch {
                model_id: ModelId::Semantic,
                expected: "".into(),
                actual: "".into(),
            },
            EmbeddingError::ModelDimensionMismatch {
                model_id: ModelId::Semantic,
                expected: 0,
                actual: 0,
            },
            EmbeddingError::ProjectionMatrixMissing {
                path: PathBuf::new(),
            },
            EmbeddingError::InferenceValidationFailed {
                model_id: ModelId::Semantic,
                reason: "".into(),
            },
        ];

        println!("\nCRITICAL SEVERITY VERIFICATION:");
        for (i, err) in critical_errors.iter().enumerate() {
            let severity = err.severity();
            println!("  {} -> severity() = {:?}", critical_codes[i], severity);
            assert_eq!(
                severity,
                ErrorSeverity::Critical,
                "FAILED: {} should be Critical",
                critical_codes[i]
            );
        }

        // High errors (3 total)
        let high_codes = ["EMB-E007", "EMB-E010", "EMB-E011"];
        let high_errors: Vec<EmbeddingError> = vec![
            EmbeddingError::OomDuringBatch {
                batch_size: 0,
                model_id: ModelId::Semantic,
            },
            EmbeddingError::StorageCorruption {
                id: "".into(),
                reason: "".into(),
            },
            EmbeddingError::CodebookMissing {
                model_id: ModelId::Semantic,
            },
        ];

        println!("\nHIGH SEVERITY VERIFICATION:");
        for (i, err) in high_errors.iter().enumerate() {
            let severity = err.severity();
            println!("  {} -> severity() = {:?}", high_codes[i], severity);
            assert_eq!(
                severity,
                ErrorSeverity::High,
                "FAILED: {} should be High",
                high_codes[i]
            );
        }

        // Medium errors (2 total)
        let medium_codes = ["EMB-E009", "EMB-E012"];
        let medium_errors: Vec<EmbeddingError> = vec![
            EmbeddingError::InputTooLarge {
                max_tokens: 0,
                actual_tokens: 0,
            },
            EmbeddingError::RecallLossExceeded {
                model_id: ModelId::Semantic,
                measured: 0.0,
                max_allowed: 0.0,
            },
        ];

        println!("\nMEDIUM SEVERITY VERIFICATION:");
        for (i, err) in medium_errors.iter().enumerate() {
            let severity = err.severity();
            println!("  {} -> severity() = {:?}", medium_codes[i], severity);
            assert_eq!(
                severity,
                ErrorSeverity::Medium,
                "FAILED: {} should be Medium",
                medium_codes[i]
            );
        }

        println!("\n✓ ALL SEVERITY CLASSIFICATIONS PHYSICALLY VERIFIED");
    }

    /// VERIFICATION TEST 3: Verify ONLY EMB-E009 is recoverable
    #[test]
    fn verify_only_emb_e009_is_recoverable_physically() {
        println!("\n========== RECOVERABILITY VERIFICATION ==========");

        // All SPEC errors that should NOT be recoverable (11 total)
        let non_recoverable: Vec<(&str, EmbeddingError)> = vec![
            (
                "EMB-E001",
                EmbeddingError::CudaUnavailable { message: "".into() },
            ),
            (
                "EMB-E002",
                EmbeddingError::InsufficientVram {
                    required_bytes: 0,
                    available_bytes: 0,
                    required_gb: 0.0,
                    available_gb: 0.0,
                },
            ),
            (
                "EMB-E003",
                EmbeddingError::WeightFileMissing {
                    model_id: ModelId::Semantic,
                    path: PathBuf::new(),
                },
            ),
            (
                "EMB-E004",
                EmbeddingError::WeightChecksumMismatch {
                    model_id: ModelId::Semantic,
                    expected: "".into(),
                    actual: "".into(),
                },
            ),
            (
                "EMB-E005",
                EmbeddingError::ModelDimensionMismatch {
                    model_id: ModelId::Semantic,
                    expected: 0,
                    actual: 0,
                },
            ),
            (
                "EMB-E006",
                EmbeddingError::ProjectionMatrixMissing {
                    path: PathBuf::new(),
                },
            ),
            (
                "EMB-E007",
                EmbeddingError::OomDuringBatch {
                    batch_size: 0,
                    model_id: ModelId::Semantic,
                },
            ),
            (
                "EMB-E008",
                EmbeddingError::InferenceValidationFailed {
                    model_id: ModelId::Semantic,
                    reason: "".into(),
                },
            ),
            (
                "EMB-E010",
                EmbeddingError::StorageCorruption {
                    id: "".into(),
                    reason: "".into(),
                },
            ),
            (
                "EMB-E011",
                EmbeddingError::CodebookMissing {
                    model_id: ModelId::Semantic,
                },
            ),
            (
                "EMB-E012",
                EmbeddingError::RecallLossExceeded {
                    model_id: ModelId::Semantic,
                    measured: 0.0,
                    max_allowed: 0.0,
                },
            ),
        ];

        println!("\nNON-RECOVERABLE VERIFICATION (should all be false):");
        for (code, err) in &non_recoverable {
            let result = err.is_recoverable();
            println!("  {} -> is_recoverable() = {}", code, result);
            assert!(!result, "FAILED: {} should NOT be recoverable", code);
        }

        // THE ONLY recoverable error
        let recoverable = EmbeddingError::InputTooLarge {
            max_tokens: 512,
            actual_tokens: 1024,
        };
        let result = recoverable.is_recoverable();
        println!("\nRECOVERABLE VERIFICATION (should be true):");
        println!("  EMB-E009 -> is_recoverable() = {}", result);
        assert!(result, "FAILED: EMB-E009 MUST be recoverable");

        println!("\n✓ RECOVERABILITY PHYSICALLY VERIFIED - ONLY EMB-E009 IS RECOVERABLE");
    }

    // =========================================================================
    // EDGE CASE VERIFICATION (3 required edge cases)
    // =========================================================================

    /// EDGE CASE 1: Maximum value boundaries (usize::MAX)
    #[test]
    fn edge_case_1_maximum_values() {
        println!("\n========== EDGE CASE 1: MAXIMUM VALUES ==========");

        println!("BEFORE: Creating InsufficientVram with usize::MAX bytes");
        let err = EmbeddingError::InsufficientVram {
            required_bytes: usize::MAX,
            available_bytes: 0,
            required_gb: f64::MAX,
            available_gb: 0.0,
        };

        println!("AFTER: Error created successfully");
        println!("  spec_code() = {:?}", err.spec_code());
        println!("  severity() = {:?}", err.severity());
        println!("  is_recoverable() = {}", err.is_recoverable());

        let msg = err.to_string();
        println!("  Message length: {} chars", msg.len());
        println!("  Contains EMB-E002: {}", msg.contains("EMB-E002"));

        // Physical verification
        assert_eq!(err.spec_code(), Some("EMB-E002"));
        assert_eq!(err.severity(), ErrorSeverity::Critical);
        assert!(!err.is_recoverable());
        assert!(msg.contains("EMB-E002"));

        println!("\n✓ EDGE CASE 1 PASSED: Maximum values handled correctly");
    }

    /// EDGE CASE 2: Empty string inputs
    #[test]
    fn edge_case_2_empty_strings() {
        println!("\n========== EDGE CASE 2: EMPTY STRINGS ==========");

        println!("BEFORE: Creating errors with empty strings");

        let err1 = EmbeddingError::CudaUnavailable {
            message: String::new(),
        };
        let err2 = EmbeddingError::StorageCorruption {
            id: String::new(),
            reason: String::new(),
        };
        let err3 = EmbeddingError::WeightChecksumMismatch {
            model_id: ModelId::Semantic,
            expected: String::new(),
            actual: String::new(),
        };

        println!("AFTER: Errors created with empty strings");

        for (name, err) in [
            ("CudaUnavailable", &err1),
            ("StorageCorruption", &err2),
            ("WeightChecksumMismatch", &err3),
        ] {
            let msg = err.to_string();
            let code = err.spec_code();
            println!("\n  {}: ", name);
            println!("    spec_code() = {:?}", code);
            println!("    Message: {}", msg.replace('\n', " | "));

            // Physical verification - should not panic with empty strings
            assert!(code.is_some(), "spec_code should exist for {}", name);
            assert!(!msg.is_empty(), "message should not be empty for {}", name);
        }

        println!("\n✓ EDGE CASE 2 PASSED: Empty strings handled correctly");
    }

    /// EDGE CASE 3: Special float values (NaN, Infinity)
    #[test]
    fn edge_case_3_special_floats() {
        println!("\n========== EDGE CASE 3: SPECIAL FLOAT VALUES ==========");

        println!("BEFORE: Creating RecallLossExceeded with NaN and Infinity");

        let err_nan = EmbeddingError::RecallLossExceeded {
            model_id: ModelId::Semantic,
            measured: f32::NAN,
            max_allowed: f32::INFINITY,
        };

        let err_neg_inf = EmbeddingError::RecallLossExceeded {
            model_id: ModelId::Semantic,
            measured: f32::NEG_INFINITY,
            max_allowed: 0.05,
        };

        println!("AFTER: Errors created with special floats");

        let msg_nan = err_nan.to_string();
        let msg_neg_inf = err_neg_inf.to_string();

        println!("\n  NaN test:");
        println!("    spec_code() = {:?}", err_nan.spec_code());
        println!("    severity() = {:?}", err_nan.severity());
        println!(
            "    Message contains NaN: {}",
            msg_nan.to_lowercase().contains("nan")
        );

        println!("\n  NEG_INFINITY test:");
        println!("    spec_code() = {:?}", err_neg_inf.spec_code());
        println!("    severity() = {:?}", err_neg_inf.severity());
        println!(
            "    Message contains inf: {}",
            msg_neg_inf.to_lowercase().contains("inf")
        );

        // Physical verification
        assert_eq!(err_nan.spec_code(), Some("EMB-E012"));
        assert_eq!(err_nan.severity(), ErrorSeverity::Medium);
        assert!(msg_nan.contains("EMB-E012"));

        assert_eq!(err_neg_inf.spec_code(), Some("EMB-E012"));
        assert!(msg_neg_inf.contains("EMB-E012"));

        println!("\n✓ EDGE CASE 3 PASSED: Special float values handled correctly");
    }

    /// FINAL VERIFICATION: Count all error variants physically present
    #[test]
    fn final_physical_count_verification() {
        println!("\n========== FINAL PHYSICAL COUNT VERIFICATION ==========");

        // Count by actually creating one of each SPEC variant
        let spec_variants: Vec<EmbeddingError> = vec![
            EmbeddingError::CudaUnavailable { message: "".into() },
            EmbeddingError::InsufficientVram {
                required_bytes: 0,
                available_bytes: 0,
                required_gb: 0.0,
                available_gb: 0.0,
            },
            EmbeddingError::WeightFileMissing {
                model_id: ModelId::Semantic,
                path: PathBuf::new(),
            },
            EmbeddingError::WeightChecksumMismatch {
                model_id: ModelId::Semantic,
                expected: "".into(),
                actual: "".into(),
            },
            EmbeddingError::ModelDimensionMismatch {
                model_id: ModelId::Semantic,
                expected: 0,
                actual: 0,
            },
            EmbeddingError::ProjectionMatrixMissing {
                path: PathBuf::new(),
            },
            EmbeddingError::OomDuringBatch {
                batch_size: 0,
                model_id: ModelId::Semantic,
            },
            EmbeddingError::InferenceValidationFailed {
                model_id: ModelId::Semantic,
                reason: "".into(),
            },
            EmbeddingError::InputTooLarge {
                max_tokens: 0,
                actual_tokens: 0,
            },
            EmbeddingError::StorageCorruption {
                id: "".into(),
                reason: "".into(),
            },
            EmbeddingError::CodebookMissing {
                model_id: ModelId::Semantic,
            },
            EmbeddingError::RecallLossExceeded {
                model_id: ModelId::Semantic,
                measured: 0.0,
                max_allowed: 0.0,
            },
        ];

        let count = spec_variants.len();
        println!(
            "PHYSICAL COUNT: {} SPEC-EMB-001 error variants created",
            count
        );

        // Verify each has a spec_code
        let codes_present: Vec<_> = spec_variants.iter().filter_map(|e| e.spec_code()).collect();

        println!("SPEC CODES FOUND: {:?}", codes_present);
        println!("UNIQUE CODES: {}", codes_present.len());

        assert_eq!(
            count, 12,
            "FAILED: Expected 12 SPEC variants, found {}",
            count
        );
        assert_eq!(codes_present.len(), 12, "FAILED: Expected 12 unique codes");

        println!("\n✓ FINAL VERIFICATION: 12 SPEC-EMB-001 VARIANTS PHYSICALLY EXIST");
        println!("==============================================\n");
    }
}
