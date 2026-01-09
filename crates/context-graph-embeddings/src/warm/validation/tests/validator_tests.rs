//! Tests for WarmValidator basic validation methods.

use crate::warm::error::WarmError;
use crate::warm::handle::ModelHandle;
use crate::warm::validation::{TestInferenceConfig, TestInput, WarmValidator};

// === WarmValidator Construction Tests ===

#[test]
fn test_validator_new() {
    let v = WarmValidator::new();
    assert!((v.default_tolerance() - WarmValidator::DEFAULT_TOLERANCE).abs() < 1e-10);
}

#[test]
fn test_validator_with_tolerance() {
    let v = WarmValidator::with_tolerance(0.01);
    assert!((v.default_tolerance() - 0.01).abs() < 1e-10);
}

// === validate_dimensions Tests ===

#[test]
fn test_validate_dimensions_matching() {
    let v = WarmValidator::new();
    assert!(v.validate_dimensions("model", 1024, 1024).is_ok());
}

#[test]
fn test_validate_dimensions_mismatched() {
    let v = WarmValidator::new();
    let result = v.validate_dimensions("E1_Semantic", 1024, 512);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        WarmError::ModelDimensionMismatch {
            model_id,
            expected,
            actual,
        } => {
            assert_eq!(model_id, "E1_Semantic");
            assert_eq!(expected, 1024);
            assert_eq!(actual, 512);
        }
        _ => panic!("Expected ModelDimensionMismatch error"),
    }
}

#[test]
fn test_validate_dimensions_zero() {
    let v = WarmValidator::new();
    assert!(v.validate_dimensions("model", 0, 0).is_ok());
    assert!(v.validate_dimensions("model", 0, 1).is_err());
}

// === validate_weights_finite Tests ===

#[test]
fn test_validate_weights_finite_valid() {
    let v = WarmValidator::new();
    let weights = vec![0.1, -0.5, 1.0, -1.0, 0.0];
    assert!(v.validate_weights_finite(&weights).is_ok());
}

#[test]
fn test_validate_weights_finite_empty() {
    let v = WarmValidator::new();
    assert!(v.validate_weights_finite(&[]).is_ok());
}

#[test]
fn test_validate_weights_finite_nan() {
    let v = WarmValidator::new();
    let weights = vec![0.1, f32::NAN, 0.3];
    let result = v.validate_weights_finite(&weights);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        WarmError::ModelValidationFailed {
            reason,
            actual_output,
            ..
        } => {
            assert!(reason.contains("NaN"));
            assert!(reason.contains("index 1"));
            assert_eq!(actual_output.as_deref(), Some("NaN"));
        }
        _ => panic!("Expected ModelValidationFailed error"),
    }
}

#[test]
fn test_validate_weights_finite_positive_inf() {
    let v = WarmValidator::new();
    let weights = vec![0.1, f32::INFINITY, 0.3];
    let result = v.validate_weights_finite(&weights);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        WarmError::ModelValidationFailed {
            reason,
            actual_output,
            ..
        } => {
            assert!(reason.contains("Infinite"));
            assert_eq!(actual_output.as_deref(), Some("+Inf"));
        }
        _ => panic!("Expected ModelValidationFailed error"),
    }
}

#[test]
fn test_validate_weights_finite_negative_inf() {
    let v = WarmValidator::new();
    let weights = vec![0.1, f32::NEG_INFINITY, 0.3];
    let result = v.validate_weights_finite(&weights);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        WarmError::ModelValidationFailed { actual_output, .. } => {
            assert_eq!(actual_output.as_deref(), Some("-Inf"));
        }
        _ => panic!("Expected ModelValidationFailed error"),
    }
}

// === validate_weight_checksum Tests ===

#[test]
fn test_validate_weight_checksum_matching() {
    let v = WarmValidator::new();
    let handle = ModelHandle::new(0x1000, 1024, 0, 0xdead_beef_cafe_babe);
    assert!(v
        .validate_weight_checksum(&handle, 0xdead_beef_cafe_babe)
        .is_ok());
}

#[test]
fn test_validate_weight_checksum_mismatched() {
    let v = WarmValidator::new();
    let handle = ModelHandle::new(0x1000, 1024, 0, 0xdead_beef_cafe_babe);
    let result = v.validate_weight_checksum(&handle, 0x1111_1111_1111_1111);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        WarmError::ModelValidationFailed {
            reason,
            expected_output,
            actual_output,
            ..
        } => {
            assert!(reason.contains("checksum"));
            assert!(expected_output
                .as_ref()
                .unwrap()
                .contains("1111111111111111"));
            assert!(actual_output.as_ref().unwrap().contains("deadbeefcafebabe"));
        }
        _ => panic!("Expected ModelValidationFailed error"),
    }
}

// === compare_output Tests ===

#[test]
fn test_compare_output_identical() {
    let v = WarmValidator::new();
    let output = vec![0.1, 0.2, 0.3];
    let reference = vec![0.1, 0.2, 0.3];
    assert!(v.compare_output(&output, &reference, 1e-5).is_ok());
}

#[test]
fn test_compare_output_within_tolerance() {
    let v = WarmValidator::new();
    let output = vec![0.10001, 0.20001, 0.30001];
    let reference = vec![0.1, 0.2, 0.3];
    assert!(v.compare_output(&output, &reference, 1e-3).is_ok());
}

#[test]
fn test_compare_output_outside_tolerance() {
    let v = WarmValidator::new();
    let output = vec![0.1, 0.25, 0.3]; // 0.25 != 0.2
    let reference = vec![0.1, 0.2, 0.3];
    let result = v.compare_output(&output, &reference, 1e-5);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        WarmError::ModelValidationFailed { reason, .. } => {
            assert!(reason.contains("index 1"));
        }
        _ => panic!("Expected ModelValidationFailed error"),
    }
}

#[test]
fn test_compare_output_length_mismatch() {
    let v = WarmValidator::new();
    let output = vec![0.1, 0.2];
    let reference = vec![0.1, 0.2, 0.3];
    let result = v.compare_output(&output, &reference, 1e-5);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        WarmError::ModelValidationFailed { reason, .. } => {
            assert!(reason.contains("length mismatch"));
        }
        _ => panic!("Expected ModelValidationFailed error"),
    }
}

#[test]
fn test_compare_output_empty() {
    let v = WarmValidator::new();
    assert!(v.compare_output(&[], &[], 1e-5).is_ok());
}

// === run_test_inference Tests ===

#[test]
fn test_run_test_inference_valid() {
    let v = WarmValidator::new();
    let config = TestInferenceConfig::for_embedding_model("TestModel", 4);
    let output = vec![0.1, 0.2, 0.3, 0.4];
    assert!(v.run_test_inference(&config, &output).is_ok());
}

#[test]
fn test_run_test_inference_wrong_dimension() {
    let v = WarmValidator::new();
    let config = TestInferenceConfig::for_embedding_model("TestModel", 4);
    let output = vec![0.1, 0.2, 0.3];
    let result = v.run_test_inference(&config, &output);

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        WarmError::ModelDimensionMismatch { .. }
    ));
}

#[test]
fn test_run_test_inference_with_reference() {
    let v = WarmValidator::new();
    let reference = vec![0.1, 0.2, 0.3];
    let config = TestInferenceConfig::with_reference(
        "TestModel",
        3,
        TestInput::Text("test".to_string()),
        reference,
        1000,
    );
    let output = vec![0.1, 0.2, 0.3];
    assert!(v.run_test_inference(&config, &output).is_ok());
}
