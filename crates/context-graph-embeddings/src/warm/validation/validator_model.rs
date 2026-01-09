//! Full model validation logic.
//!
//! Performs the complete validation pipeline for warm-loaded models.

use crate::warm::error::WarmError;
use crate::warm::handle::ModelHandle;

use super::config::TestInferenceConfig;
use super::result::ValidationResult;
use super::validator::WarmValidator;

impl WarmValidator {
    /// Perform full model validation.
    ///
    /// Runs all validation stages and aggregates results:
    /// 1. Dimension validation
    /// 2. Weight validation (via checksum check)
    /// 3. Inference validation
    #[must_use]
    pub fn validate_model(
        &self,
        config: &TestInferenceConfig,
        _handle: &ModelHandle,
        output: &[f32],
    ) -> ValidationResult {
        let start = std::time::Instant::now();

        // Stage 1: Validate dimensions
        if self
            .validate_dimensions(&config.model_id, config.expected_dimension, output.len())
            .is_err()
        {
            let error = WarmError::ModelDimensionMismatch {
                model_id: config.model_id.clone(),
                expected: config.expected_dimension,
                actual: output.len(),
            };
            return ValidationResult::failure(
                config.model_id.clone(),
                false,
                true,
                false,
                start.elapsed().as_millis() as u64,
                error,
            );
        }

        // Stage 2: Validate weights
        if self
            .validate_weights_finite_for_model(&config.model_id, output)
            .is_err()
        {
            let error = Self::find_weight_error(config, output);
            return ValidationResult::failure(
                config.model_id.clone(),
                true,
                false,
                false,
                start.elapsed().as_millis() as u64,
                error,
            );
        }

        // Stage 3: Validate inference
        let inference_valid = config.reference_output.as_ref().is_none_or(|reference| {
            self.compare_output_for_model(
                &config.model_id,
                output,
                reference,
                self.default_tolerance(),
            )
            .is_ok()
        });

        let inference_time_ms = start.elapsed().as_millis() as u64;

        if !inference_valid {
            let error = Self::find_inference_error(config, output, self.default_tolerance());
            return ValidationResult::failure(
                config.model_id.clone(),
                true,
                true,
                false,
                inference_time_ms,
                error,
            );
        }

        // Check inference time
        if inference_time_ms > config.max_inference_ms {
            return ValidationResult::failure(
                config.model_id.clone(),
                true,
                true,
                false,
                inference_time_ms,
                WarmError::ModelValidationFailed {
                    model_id: config.model_id.clone(),
                    reason: format!(
                        "Inference time {inference_time_ms}ms exceeded maximum {}ms",
                        config.max_inference_ms
                    ),
                    expected_output: Some(format!("<= {}ms", config.max_inference_ms)),
                    actual_output: Some(format!("{inference_time_ms}ms")),
                },
            );
        }

        ValidationResult::success(config.model_id.clone(), inference_time_ms)
    }

    fn find_weight_error(config: &TestInferenceConfig, output: &[f32]) -> WarmError {
        if let Some((idx, _)) = output.iter().enumerate().find(|(_, v)| v.is_nan()) {
            WarmError::ModelValidationFailed {
                model_id: config.model_id.clone(),
                reason: format!("NaN value found at output index {idx}"),
                expected_output: Some("finite values".to_string()),
                actual_output: Some("NaN".to_string()),
            }
        } else if let Some((idx, val)) = output.iter().enumerate().find(|(_, v)| v.is_infinite()) {
            let sign = if val.is_sign_positive() {
                "+Inf"
            } else {
                "-Inf"
            };
            WarmError::ModelValidationFailed {
                model_id: config.model_id.clone(),
                reason: format!("Infinite value found at output index {idx}"),
                expected_output: Some("finite values".to_string()),
                actual_output: Some(sign.to_string()),
            }
        } else {
            WarmError::ModelValidationFailed {
                model_id: config.model_id.clone(),
                reason: "Unknown weight validation failure".to_string(),
                expected_output: None,
                actual_output: None,
            }
        }
    }

    fn find_inference_error(
        config: &TestInferenceConfig,
        output: &[f32],
        tolerance: f32,
    ) -> WarmError {
        let reference = config.reference_output.as_ref().unwrap();
        let (idx, (actual_val, ref_val)) = output
            .iter()
            .zip(reference.iter())
            .enumerate()
            .find(|(_, (a, r))| (**a - **r).abs() > tolerance)
            .map(|(i, (a, r))| (i, (*a, *r)))
            .unwrap_or((0, (0.0, 0.0)));

        WarmError::ModelValidationFailed {
            model_id: config.model_id.clone(),
            reason: format!("Output mismatch at index {idx}: expected {ref_val}, got {actual_val}"),
            expected_output: Some(format!("{ref_val}")),
            actual_output: Some(format!("{actual_val}")),
        }
    }
}
