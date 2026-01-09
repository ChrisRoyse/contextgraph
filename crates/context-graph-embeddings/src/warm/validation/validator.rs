//! Core validation logic for warm-loaded models.
//!
//! Performs comprehensive validation of models after weight loading
//! to ensure correctness before marking as Warm.

use crate::warm::error::{WarmError, WarmResult};
use crate::warm::handle::ModelHandle;

use super::comparisons::compare_output_impl;
use super::config::TestInferenceConfig;

/// Validator for warm-loaded models.
///
/// Performs comprehensive validation of models after weight loading
/// to ensure correctness before marking as Warm.
///
/// # Example
///
/// ```rust,ignore
/// let validator = WarmValidator::new();
///
/// // Validate dimensions
/// validator.validate_dimensions("E1_Semantic", 1024, output.len())?;
///
/// // Validate weights
/// validator.validate_weights_finite(&weights)?;
///
/// // Full validation
/// let result = validator.validate_model(&config, &handle, &output);
/// if !result.is_valid() {
///     return Err(result.error.unwrap());
/// }
/// ```
#[derive(Debug, Default)]
pub struct WarmValidator {
    /// Default tolerance for floating-point comparisons.
    default_tolerance: f32,
}

impl WarmValidator {
    /// Default tolerance for output comparison (1e-5).
    pub const DEFAULT_TOLERANCE: f32 = 1e-5;

    /// Create a new validator with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            default_tolerance: Self::DEFAULT_TOLERANCE,
        }
    }

    /// Create a validator with custom tolerance.
    #[must_use]
    pub fn with_tolerance(tolerance: f32) -> Self {
        Self {
            default_tolerance: tolerance,
        }
    }

    /// Get the default tolerance value.
    #[must_use]
    pub fn default_tolerance(&self) -> f32 {
        self.default_tolerance
    }

    /// Validate that output dimensions match expected values.
    ///
    /// # Arguments
    ///
    /// * `model_id` - Model identifier for error reporting
    /// * `expected` - Expected output dimension
    /// * `actual` - Actual output dimension
    ///
    /// # Errors
    ///
    /// Returns `WarmError::ModelDimensionMismatch` if dimensions don't match.
    pub fn validate_dimensions(
        &self,
        model_id: &str,
        expected: usize,
        actual: usize,
    ) -> WarmResult<()> {
        if expected != actual {
            return Err(WarmError::ModelDimensionMismatch {
                model_id: model_id.to_string(),
                expected,
                actual,
            });
        }
        Ok(())
    }

    /// Validate that weights contain no NaN or Inf values.
    ///
    /// Model weights with NaN or Inf indicate corruption or failed loading.
    /// This validation catches silent failures that might otherwise produce
    /// garbage inference output.
    ///
    /// # Errors
    ///
    /// Returns `WarmError::ModelValidationFailed` if any weight is NaN or Inf.
    pub fn validate_weights_finite(&self, weights: &[f32]) -> WarmResult<()> {
        for (idx, &weight) in weights.iter().enumerate() {
            if weight.is_nan() {
                return Err(WarmError::ModelValidationFailed {
                    model_id: "unknown".to_string(),
                    reason: format!("NaN value found at weight index {idx}"),
                    expected_output: Some("finite values".to_string()),
                    actual_output: Some("NaN".to_string()),
                });
            }
            if weight.is_infinite() {
                return Err(WarmError::ModelValidationFailed {
                    model_id: "unknown".to_string(),
                    reason: format!("Infinite value found at weight index {idx}"),
                    expected_output: Some("finite values".to_string()),
                    actual_output: Some(
                        if weight.is_sign_positive() {
                            "+Inf"
                        } else {
                            "-Inf"
                        }
                        .to_string(),
                    ),
                });
            }
        }
        Ok(())
    }

    /// Validate that weights contain no NaN or Inf values (with model ID).
    ///
    /// Same as `validate_weights_finite` but includes model ID in error.
    pub fn validate_weights_finite_for_model(
        &self,
        model_id: &str,
        weights: &[f32],
    ) -> WarmResult<()> {
        for (idx, &weight) in weights.iter().enumerate() {
            if weight.is_nan() {
                return Err(WarmError::ModelValidationFailed {
                    model_id: model_id.to_string(),
                    reason: format!("NaN value found at weight index {idx}"),
                    expected_output: Some("finite values".to_string()),
                    actual_output: Some("NaN".to_string()),
                });
            }
            if weight.is_infinite() {
                return Err(WarmError::ModelValidationFailed {
                    model_id: model_id.to_string(),
                    reason: format!("Infinite value found at weight index {idx}"),
                    expected_output: Some("finite values".to_string()),
                    actual_output: Some(
                        if weight.is_sign_positive() {
                            "+Inf"
                        } else {
                            "-Inf"
                        }
                        .to_string(),
                    ),
                });
            }
        }
        Ok(())
    }

    /// Validate weight checksum matches expected value.
    ///
    /// The checksum is computed during weight loading and stored in the
    /// `ModelHandle`. This validation ensures weight integrity.
    ///
    /// # Errors
    ///
    /// Returns `WarmError::ModelValidationFailed` if checksums don't match.
    pub fn validate_weight_checksum(&self, handle: &ModelHandle, expected: u64) -> WarmResult<()> {
        let actual = handle.weight_checksum();
        if expected != actual {
            return Err(WarmError::ModelValidationFailed {
                model_id: "unknown".to_string(),
                reason: "Weight checksum mismatch".to_string(),
                expected_output: Some(format!("0x{expected:016x}")),
                actual_output: Some(format!("0x{actual:016x}")),
            });
        }
        Ok(())
    }

    /// Validate weight checksum matches expected value (with model ID).
    ///
    /// Same as `validate_weight_checksum` but includes model ID in error.
    pub fn validate_weight_checksum_for_model(
        &self,
        model_id: &str,
        handle: &ModelHandle,
        expected: u64,
    ) -> WarmResult<()> {
        let actual = handle.weight_checksum();
        if expected != actual {
            return Err(WarmError::ModelValidationFailed {
                model_id: model_id.to_string(),
                reason: "Weight checksum mismatch".to_string(),
                expected_output: Some(format!("0x{expected:016x}")),
                actual_output: Some(format!("0x{actual:016x}")),
            });
        }
        Ok(())
    }

    /// Run test inference validation.
    ///
    /// Validates that inference output meets the configuration requirements:
    /// - Correct output dimension
    /// - No NaN/Inf in output
    /// - If reference output provided, values match within tolerance
    ///
    /// # Errors
    ///
    /// Returns `WarmError::ModelValidationFailed` or `WarmError::ModelDimensionMismatch`.
    pub fn run_test_inference(
        &self,
        config: &TestInferenceConfig,
        output: &[f32],
    ) -> WarmResult<()> {
        // Validate output dimension
        self.validate_dimensions(&config.model_id, config.expected_dimension, output.len())?;

        // Validate output contains no NaN/Inf
        self.validate_weights_finite_for_model(&config.model_id, output)?;

        // If reference output provided, compare values
        if let Some(ref reference) = config.reference_output {
            self.compare_output(output, reference, self.default_tolerance)?;
        }

        Ok(())
    }

    /// Compare actual output to reference output within tolerance.
    ///
    /// Uses element-wise absolute difference comparison. All elements must
    /// be within tolerance for validation to pass.
    ///
    /// # Errors
    ///
    /// Returns `WarmError::ModelValidationFailed` if:
    /// - Output lengths differ
    /// - Any element differs by more than tolerance
    pub fn compare_output(
        &self,
        actual: &[f32],
        reference: &[f32],
        tolerance: f32,
    ) -> WarmResult<()> {
        compare_output_impl("unknown", actual, reference, tolerance)
    }

    /// Compare actual output to reference output within tolerance (with model ID).
    pub fn compare_output_for_model(
        &self,
        model_id: &str,
        actual: &[f32],
        reference: &[f32],
        tolerance: f32,
    ) -> WarmResult<()> {
        compare_output_impl(model_id, actual, reference, tolerance)
    }
}
