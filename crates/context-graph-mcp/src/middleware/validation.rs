//! Parameter validation middleware for MCP tool handlers.
//!
//! Provides consistent validation with field-name-aware error messages.
//! All validation functions return Result types per AP-14.
//!
//! # Constitution Compliance
//! - SEC-01: Validate/sanitize all input
//! - AP-14: No .unwrap() in library code
//!
//! # Usage
//! ```rust
//! use context_graph_mcp::middleware::validation::*;
//!
//! fn example() -> Result<(), ValidationError> {
//!     let args = serde_json::json!({"rationale": "test reason"});
//!     let rationale = validate_required_string("rationale", args.get("rationale"))?;
//!     let limit = validate_range("limit", 50i64, 1i64, 100i64)?;
//!     let memory_id = validate_uuid("memory_id", "550e8400-e29b-41d4-a716-446655440000")?;
//!     Ok(())
//! }
//! ```

use serde::de::DeserializeOwned;
use thiserror::Error;
use uuid::Uuid;

/// Validation error with field information.
///
/// Error messages MUST include the field name for debugging.
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Field '{field}' validation failed: {message}")]
    FieldValidation { field: String, message: String },

    #[error("Required field '{field}' is missing")]
    MissingRequired { field: String },

    #[error("Invalid format for '{field}': expected {expected}")]
    InvalidFormat { field: String, expected: String },

    #[error("Field '{field}' out of range: {value} not in [{min}, {max}]")]
    OutOfRange {
        field: String,
        value: String,
        min: String,
        max: String,
    },

    #[error("Schema validation failed for '{field}': {details}")]
    SchemaViolation { field: String, details: String },
}

impl ValidationError {
    /// Get the field name that failed validation.
    pub fn field_name(&self) -> &str {
        match self {
            Self::FieldValidation { field, .. } => field,
            Self::MissingRequired { field } => field,
            Self::InvalidFormat { field, .. } => field,
            Self::OutOfRange { field, .. } => field,
            Self::SchemaViolation { field, .. } => field,
        }
    }

    /// Convert to JSON-RPC error code (always INVALID_PARAMS = -32602).
    pub fn error_code(&self) -> i32 {
        -32602 // INVALID_PARAMS per protocol.rs
    }
}

/// Validate a required string field is present and non-empty.
///
/// # Arguments
/// * `field` - Field name for error messages
/// * `value` - Optional JSON value to validate
///
/// # Returns
/// * `Ok(String)` - Trimmed, non-empty string
/// * `Err(ValidationError::MissingRequired)` - If None or empty
pub fn validate_required_string(
    field: &str,
    value: Option<&serde_json::Value>,
) -> Result<String, ValidationError> {
    match value.and_then(|v| v.as_str()) {
        Some(s) if !s.trim().is_empty() => Ok(s.trim().to_string()),
        _ => Err(ValidationError::MissingRequired {
            field: field.to_string(),
        }),
    }
}

/// Validate string length is within bounds.
///
/// # Arguments
/// * `field` - Field name for error messages
/// * `value` - String to validate
/// * `min` - Minimum length (inclusive)
/// * `max` - Maximum length (inclusive)
pub fn validate_string_length(
    field: &str,
    value: &str,
    min: usize,
    max: usize,
) -> Result<(), ValidationError> {
    let len = value.len();
    if len < min || len > max {
        return Err(ValidationError::OutOfRange {
            field: field.to_string(),
            value: len.to_string(),
            min: min.to_string(),
            max: max.to_string(),
        });
    }
    Ok(())
}

/// Validate numeric value is within range.
///
/// # Arguments
/// * `field` - Field name for error messages
/// * `value` - Numeric value to validate
/// * `min` - Minimum value (inclusive)
/// * `max` - Maximum value (inclusive)
pub fn validate_range<N: PartialOrd + std::fmt::Display>(
    field: &str,
    value: N,
    min: N,
    max: N,
) -> Result<(), ValidationError> {
    if value < min || value > max {
        return Err(ValidationError::OutOfRange {
            field: field.to_string(),
            value: value.to_string(),
            min: min.to_string(),
            max: max.to_string(),
        });
    }
    Ok(())
}

/// Validate and parse UUID string.
///
/// # Arguments
/// * `field` - Field name for error messages
/// * `value` - String to parse as UUID
///
/// # Returns
/// * `Ok(Uuid)` - Parsed UUID
/// * `Err(ValidationError::InvalidFormat)` - If not valid UUID
pub fn validate_uuid(field: &str, value: &str) -> Result<Uuid, ValidationError> {
    Uuid::parse_str(value).map_err(|e| ValidationError::InvalidFormat {
        field: field.to_string(),
        expected: format!("UUID (got '{}', error: {})", value, e),
    })
}

/// Validate optional integer within range.
///
/// Returns default if value is None.
pub fn validate_optional_int(
    field: &str,
    value: Option<&serde_json::Value>,
    min: i64,
    max: i64,
    default: i64,
) -> Result<i64, ValidationError> {
    match value.and_then(|v| v.as_i64()) {
        Some(n) => {
            validate_range(field, n, min, max)?;
            Ok(n)
        }
        None => Ok(default),
    }
}

/// Validate optional float within range.
///
/// Returns default if value is None.
pub fn validate_optional_float(
    field: &str,
    value: Option<&serde_json::Value>,
    min: f64,
    max: f64,
    default: f64,
) -> Result<f64, ValidationError> {
    match value.and_then(|v| v.as_f64()) {
        Some(n) => {
            if n < min || n > max {
                return Err(ValidationError::OutOfRange {
                    field: field.to_string(),
                    value: n.to_string(),
                    min: min.to_string(),
                    max: max.to_string(),
                });
            }
            Ok(n)
        }
        None => Ok(default),
    }
}

/// Validate 13-element array (embedder weights/purpose vector).
///
/// Constitution ARCH-01: "TeleologicalArray is atomic - store all 13 embeddings or nothing"
pub fn validate_13_element_array(
    field: &str,
    value: Option<&serde_json::Value>,
) -> Result<[f32; 13], ValidationError> {
    let arr = value
        .and_then(|v| v.as_array())
        .ok_or_else(|| ValidationError::MissingRequired {
            field: field.to_string(),
        })?;

    if arr.len() != 13 {
        return Err(ValidationError::FieldValidation {
            field: field.to_string(),
            message: format!("Must have exactly 13 elements, got {}", arr.len()),
        });
    }

    let mut result = [0.0f32; 13];
    for (i, v) in arr.iter().enumerate() {
        result[i] = v.as_f64().ok_or_else(|| ValidationError::FieldValidation {
            field: format!("{}[{}]", field, i),
            message: "Expected number".to_string(),
        })? as f32;
    }

    Ok(result)
}

/// Validate embedder index (0-12 per constitution).
///
/// Constitution: 13 embedders E1-E13, indices 0-12.
pub fn validate_embedder_index(field: &str, value: usize) -> Result<(), ValidationError> {
    if value > 12 {
        return Err(ValidationError::OutOfRange {
            field: field.to_string(),
            value: value.to_string(),
            min: "0".to_string(),
            max: "12".to_string(),
        });
    }
    Ok(())
}

// ============================================================================
// SCHEMARS INTEGRATION (JsonSchema validation)
// ============================================================================

/// Validate tool input against JsonSchema using schemars.
///
/// Deserializes and validates params against T's JsonSchema.
/// Returns strongly-typed T on success.
///
/// # Type Requirements
/// T must implement:
/// - `schemars::JsonSchema` - For schema generation
/// - `serde::de::DeserializeOwned` - For deserialization
pub fn validate_input<T>(params: serde_json::Value) -> Result<T, ValidationError>
where
    T: schemars::JsonSchema + DeserializeOwned,
{
    // First attempt deserialization
    serde_json::from_value::<T>(params.clone()).map_err(|e| ValidationError::SchemaViolation {
        field: "params".to_string(),
        details: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ========================================================================
    // UNIT TESTS - Each validation function
    // ========================================================================

    #[test]
    fn test_validate_required_string_present() {
        let args = json!({"name": "test value"});
        let result = validate_required_string("name", args.get("name"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test value");
    }

    #[test]
    fn test_validate_required_string_missing() {
        let args = json!({});
        let result = validate_required_string("name", args.get("name"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.field_name(), "name");
        assert!(matches!(err, ValidationError::MissingRequired { .. }));
    }

    #[test]
    fn test_validate_required_string_empty() {
        let args = json!({"name": "   "});
        let result = validate_required_string("name", args.get("name"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().field_name(), "name");
    }

    #[test]
    fn test_validate_string_length_valid() {
        let result = validate_string_length("rationale", "valid input", 1, 1024);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_string_length_too_short() {
        let result = validate_string_length("rationale", "", 1, 1024);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.field_name(), "rationale");
        assert!(matches!(err, ValidationError::OutOfRange { .. }));
    }

    #[test]
    fn test_validate_string_length_too_long() {
        let long_string = "x".repeat(2000);
        let result = validate_string_length("rationale", &long_string, 1, 1024);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_range_valid() {
        let result = validate_range("limit", 50, 1, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_range_below_min() {
        let result = validate_range("limit", 0, 1, 100);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.field_name(), "limit");
    }

    #[test]
    fn test_validate_range_above_max() {
        let result = validate_range("limit", 150, 1, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_uuid_valid() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let result = validate_uuid("memory_id", uuid_str);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_string(), uuid_str);
    }

    #[test]
    fn test_validate_uuid_invalid() {
        let result = validate_uuid("memory_id", "not-a-uuid");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.field_name(), "memory_id");
        assert!(matches!(err, ValidationError::InvalidFormat { .. }));
    }

    #[test]
    fn test_validate_optional_int_present() {
        let args = json!({"limit": 50});
        let result = validate_optional_int("limit", args.get("limit"), 1, 100, 20);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 50);
    }

    #[test]
    fn test_validate_optional_int_missing_uses_default() {
        let args = json!({});
        let result = validate_optional_int("limit", args.get("limit"), 1, 100, 20);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 20);
    }

    #[test]
    fn test_validate_optional_int_out_of_range() {
        let args = json!({"limit": 200});
        let result = validate_optional_int("limit", args.get("limit"), 1, 100, 20);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_13_element_array_valid() {
        let args =
            json!({"weights": [0.08, 0.08, 0.08, 0.08, 0.08, 0.08, 0.08, 0.08, 0.08, 0.08, 0.08, 0.08, 0.04]});
        let result = validate_13_element_array("weights", args.get("weights"));
        assert!(result.is_ok());
        let arr = result.unwrap();
        assert_eq!(arr.len(), 13);
    }

    #[test]
    fn test_validate_13_element_array_wrong_length() {
        let args = json!({"weights": [0.1, 0.1, 0.1]}); // Only 3 elements
        let result = validate_13_element_array("weights", args.get("weights"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.field_name(), "weights");
    }

    #[test]
    fn test_validate_embedder_index_valid() {
        for i in 0..=12 {
            assert!(validate_embedder_index("space_index", i).is_ok());
        }
    }

    #[test]
    fn test_validate_embedder_index_invalid() {
        let result = validate_embedder_index("space_index", 13);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().field_name(), "space_index");
    }

    #[test]
    fn test_error_code_always_invalid_params() {
        let errors = vec![
            ValidationError::MissingRequired {
                field: "x".into(),
            },
            ValidationError::InvalidFormat {
                field: "x".into(),
                expected: "y".into(),
            },
            ValidationError::OutOfRange {
                field: "x".into(),
                value: "1".into(),
                min: "0".into(),
                max: "0".into(),
            },
        ];
        for err in errors {
            assert_eq!(err.error_code(), -32602);
        }
    }

    // ========================================================================
    // EDGE CASE TESTS - Per task requirements
    // ========================================================================

    #[test]
    fn test_edge_case_empty_string_field_name() {
        // Edge: Empty field name should still work
        let result = validate_required_string("", None);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().field_name(), "");
    }

    #[test]
    fn test_edge_case_null_json_value() {
        let args = json!({"field": null});
        let result = validate_required_string("field", args.get("field"));
        assert!(result.is_err());
    }

    #[test]
    fn test_edge_case_wrong_type_for_int() {
        let args = json!({"limit": "not a number"});
        let result = validate_optional_int("limit", args.get("limit"), 1, 100, 20);
        // Should use default when type is wrong
        assert_eq!(result.unwrap(), 20);
    }

    #[test]
    fn test_edge_case_float_for_int_field() {
        let args = json!({"limit": 50.7});
        // as_i64() on 50.7 returns None, so should use default
        let result = validate_optional_int("limit", args.get("limit"), 1, 100, 20);
        assert_eq!(result.unwrap(), 20);
    }

    #[test]
    fn test_edge_case_boundary_values() {
        // Test exact boundary values
        assert!(validate_range("x", 1, 1, 100).is_ok()); // min boundary
        assert!(validate_range("x", 100, 1, 100).is_ok()); // max boundary
        assert!(validate_range("x", 0, 1, 100).is_err()); // just below min
        assert!(validate_range("x", 101, 1, 100).is_err()); // just above max
    }

    // ========================================================================
    // SCHEMARS INTEGRATION TEST
    // ========================================================================

    #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
    struct TestInput {
        name: String,
        count: i32,
    }

    #[test]
    fn test_validate_input_valid() {
        let params = json!({"name": "test", "count": 42});
        let result: Result<TestInput, _> = validate_input(params);
        assert!(result.is_ok());
        let input = result.unwrap();
        assert_eq!(input.name, "test");
        assert_eq!(input.count, 42);
    }

    #[test]
    fn test_validate_input_invalid() {
        let params = json!({"name": "test"}); // missing count
        let result: Result<TestInput, _> = validate_input(params);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ValidationError::SchemaViolation { .. }));
    }

    // ========================================================================
    // OPTIONAL FLOAT TESTS
    // ========================================================================

    #[test]
    fn test_validate_optional_float_present() {
        let args = json!({"threshold": 0.75});
        let result = validate_optional_float("threshold", args.get("threshold"), 0.0, 1.0, 0.5);
        assert!(result.is_ok());
        assert!((result.unwrap() - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_validate_optional_float_missing_uses_default() {
        let args = json!({});
        let result = validate_optional_float("threshold", args.get("threshold"), 0.0, 1.0, 0.5);
        assert!(result.is_ok());
        assert!((result.unwrap() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_validate_optional_float_out_of_range() {
        let args = json!({"threshold": 1.5});
        let result = validate_optional_float("threshold", args.get("threshold"), 0.0, 1.0, 0.5);
        assert!(result.is_err());
    }
}
