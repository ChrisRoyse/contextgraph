//! Tests for WarmValidator dimension/weight/inference validation.

use super::helpers::test_handle_full;
use crate::warm::error::WarmError;
use crate::warm::validation::{TestInferenceConfig, TestInput, ValidationResult, WarmValidator};

#[test]
fn test_validator_default_tolerance() {
    let v = WarmValidator::new();
    // Just verify it's created successfully - tolerance is internal
    assert!(v.validate_dimensions("model", 1024, 1024).is_ok());
}

#[test]
fn test_validator_custom_tolerance() {
    let v = WarmValidator::with_tolerance(0.01);
    // Verify custom tolerance validator works
    assert!(v.validate_dimensions("model", 1024, 1024).is_ok());
}

#[test]
fn test_validate_dimensions_matching() {
    let v = WarmValidator::new();
    assert!(v.validate_dimensions("model", 1024, 1024).is_ok());
}

#[test]
fn test_validate_dimensions_mismatch() {
    let v = WarmValidator::new();
    let result = v.validate_dimensions("E1_Semantic", 1024, 512);

    assert!(result.is_err());
    match result.unwrap_err() {
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
fn test_validate_weights_finite_valid() {
    let v = WarmValidator::new();
    let weights = vec![0.1, -0.5, 1.0, -1.0, 0.0];
    assert!(v.validate_weights_finite(&weights).is_ok());
}

#[test]
fn test_validate_weights_finite_nan() {
    let v = WarmValidator::new();
    let weights = vec![0.1, f32::NAN, 0.3];
    let result = v.validate_weights_finite(&weights);

    assert!(result.is_err());
    match result.unwrap_err() {
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
    match result.unwrap_err() {
        WarmError::ModelValidationFailed { actual_output, .. } => {
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
    match result.unwrap_err() {
        WarmError::ModelValidationFailed { actual_output, .. } => {
            assert_eq!(actual_output.as_deref(), Some("-Inf"));
        }
        _ => panic!("Expected ModelValidationFailed error"),
    }
}

#[test]
fn test_validate_weight_checksum_matching() {
    let v = WarmValidator::new();
    let handle = test_handle_full(0x1000, 1024, 0, 0xdeadbeefcafebabe);
    assert!(v
        .validate_weight_checksum(&handle, 0xdeadbeefcafebabe)
        .is_ok());
}

#[test]
fn test_validate_weight_checksum_mismatch() {
    let v = WarmValidator::new();
    let handle = test_handle_full(0x1000, 1024, 0, 0xdeadbeefcafebabe);
    let result = v.validate_weight_checksum(&handle, 0x1111111111111111);

    assert!(result.is_err());
    match result.unwrap_err() {
        WarmError::ModelValidationFailed {
            reason,
            expected_output,
            actual_output,
            ..
        } => {
            assert!(reason.contains("checksum"));
            assert!(expected_output.unwrap().contains("1111111111111111"));
            assert!(actual_output.unwrap().contains("deadbeefcafebabe"));
        }
        _ => panic!("Expected ModelValidationFailed error"),
    }
}

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
    let output = vec![0.1, 0.25, 0.3];
    let reference = vec![0.1, 0.2, 0.3];
    let result = v.compare_output(&output, &reference, 1e-5);

    assert!(result.is_err());
    match result.unwrap_err() {
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
    match result.unwrap_err() {
        WarmError::ModelValidationFailed { reason, .. } => {
            assert!(reason.contains("length mismatch"));
        }
        _ => panic!("Expected ModelValidationFailed error"),
    }
}

#[test]
fn test_validate_model_success() {
    let v = WarmValidator::new();
    let config = TestInferenceConfig::for_embedding_model("E1_Semantic", 4);
    let handle = test_handle_full(0x1000, 16, 0, 0xabcd);
    let output = vec![0.1, 0.2, 0.3, 0.4];

    let result = v.validate_model(&config, &handle, &output);

    assert!(result.is_valid());
    assert!(result.dimension_valid);
    assert!(result.weights_valid);
    assert!(result.inference_valid);
    assert!(result.error.is_none());
}

#[test]
fn test_validate_model_dimension_failure() {
    let v = WarmValidator::new();
    let config = TestInferenceConfig::for_embedding_model("E1_Semantic", 1024);
    let handle = test_handle_full(0x1000, 2048, 0, 0xabcd);
    let output = vec![0.1; 512];

    let result = v.validate_model(&config, &handle, &output);

    assert!(!result.is_valid());
    assert!(!result.dimension_valid);
    assert!(matches!(
        result.error,
        Some(WarmError::ModelDimensionMismatch { .. })
    ));
}

#[test]
fn test_validate_model_nan_failure() {
    let v = WarmValidator::new();
    let config = TestInferenceConfig::for_embedding_model("E1_Semantic", 4);
    let handle = test_handle_full(0x1000, 16, 0, 0xabcd);
    let output = vec![0.1, f32::NAN, 0.3, 0.4];

    let result = v.validate_model(&config, &handle, &output);

    assert!(!result.is_valid());
    assert!(result.dimension_valid);
    assert!(!result.weights_valid);
    assert!(matches!(
        result.error,
        Some(WarmError::ModelValidationFailed { .. })
    ));
}

#[test]
fn test_test_inference_config_for_embedding() {
    let config = TestInferenceConfig::for_embedding_model("E1_Semantic", 1024);

    assert_eq!(config.model_id, "E1_Semantic");
    assert_eq!(config.expected_dimension, 1024);
    assert!(matches!(config.test_input, TestInput::Text(_)));
    assert!(config.reference_output.is_none());
    assert_eq!(config.max_inference_ms, 1000);
}

#[test]
fn test_test_input_types() {
    assert_eq!(TestInput::Text("hello".to_string()).description(), "text");
    assert_eq!(TestInput::Tokens(vec![1, 2, 3]).description(), "tokens");
    assert_eq!(
        TestInput::Embeddings(vec![0.1, 0.2]).description(),
        "embeddings"
    );

    assert_eq!(TestInput::Text("hello".to_string()).len(), 5);
    assert_eq!(TestInput::Tokens(vec![1, 2, 3]).len(), 3);
    assert!(!TestInput::Text("hello".to_string()).is_empty());
    assert!(TestInput::Text(String::new()).is_empty());
}

#[test]
fn test_validation_result_success() {
    let result = ValidationResult::success("model1".to_string(), 42);

    assert_eq!(result.model_id, "model1");
    assert!(result.is_valid());
    assert!(result.dimension_valid);
    assert!(result.weights_valid);
    assert!(result.inference_valid);
    assert_eq!(result.inference_time_ms, 42);
    assert!(result.error().is_none());
}

#[test]
fn test_validation_result_failure() {
    let error = WarmError::ModelDimensionMismatch {
        model_id: "model1".to_string(),
        expected: 100,
        actual: 50,
    };

    let result = ValidationResult::failure("model1".to_string(), false, true, false, 100, error);

    assert!(!result.is_valid());
    assert!(!result.dimension_valid);
    assert!(result.weights_valid);
    assert!(!result.inference_valid);
    assert!(result.error().is_some());
}
