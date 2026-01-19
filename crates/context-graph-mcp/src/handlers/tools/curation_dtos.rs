//! DTOs for curation-related MCP tools.
//!
//! Per PRD v6 Section 10.3, these DTOs support:
//! - forget_concept: Soft-delete a memory with 30-day recovery
//! - boost_importance: Adjust memory importance score
//!
//! Constitution References:
//! - SEC-06: Soft delete 30-day recovery
//! - BR-MCP-001: forget_concept uses soft delete by default
//! - BR-MCP-002: boost_importance clamps final value to [0.0, 1.0]
//!
//! Note: These DTOs are used by curation_tools.rs implementation.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Soft delete recovery period in days per SEC-06.
pub const SOFT_DELETE_RECOVERY_DAYS: i64 = 30;

/// Minimum importance value.
pub const MIN_IMPORTANCE: f32 = 0.0;

/// Maximum importance value.
pub const MAX_IMPORTANCE: f32 = 1.0;

/// Minimum delta value for boost_importance.
pub const MIN_DELTA: f32 = -1.0;

/// Maximum delta value for boost_importance.
pub const MAX_DELTA: f32 = 1.0;

// ============================================================================
// REQUEST DTOs
// ============================================================================

/// Request parameters for forget_concept tool.
///
/// # Example JSON
/// ```json
/// {"node_id": "550e8400-e29b-41d4-a716-446655440000", "soft_delete": true}
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct ForgetConceptRequest {
    /// UUID of the memory to forget (required)
    /// Must be a valid UUID string
    pub node_id: String,

    /// Use soft delete with 30-day recovery window (default true per SEC-06)
    /// If false, memory is permanently deleted with no recovery option
    #[serde(default = "default_soft_delete")]
    pub soft_delete: bool,
}

fn default_soft_delete() -> bool {
    true
}

impl Default for ForgetConceptRequest {
    fn default() -> Self {
        Self {
            node_id: String::new(),
            soft_delete: true, // Per SEC-06 and BR-MCP-001
        }
    }
}

impl ForgetConceptRequest {
    /// Validate the request parameters and return parsed UUID.
    ///
    /// # Errors
    /// Returns an error message if node_id is not a valid UUID.
    pub fn validate(&self) -> Result<Uuid, String> {
        Uuid::parse_str(&self.node_id)
            .map_err(|e| format!("Invalid UUID format for node_id '{}': {}", self.node_id, e))
    }
}

/// Request parameters for boost_importance tool.
///
/// # Example JSON
/// ```json
/// {"node_id": "550e8400-e29b-41d4-a716-446655440000", "delta": 0.2}
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct BoostImportanceRequest {
    /// UUID of the memory to modify (required)
    pub node_id: String,

    /// Importance adjustment (-1.0 to 1.0)
    /// - Positive values increase importance
    /// - Negative values decrease importance
    /// - Final value is clamped to [0.0, 1.0] per BR-MCP-002
    pub delta: f32,
}

impl Default for BoostImportanceRequest {
    fn default() -> Self {
        Self {
            node_id: String::new(),
            delta: 0.0,
        }
    }
}

impl BoostImportanceRequest {
    /// Validate the request parameters and return parsed UUID.
    ///
    /// # Errors
    /// Returns an error message if:
    /// - delta is NaN or infinite
    /// - delta is outside [-1.0, 1.0]
    /// - node_id is not a valid UUID
    pub fn validate(&self) -> Result<Uuid, String> {
        // Check for NaN or infinity first (per AP-10: No NaN/Infinity in similarity scores)
        if self.delta.is_nan() || self.delta.is_infinite() {
            return Err("delta must be a finite number".to_string());
        }

        // Validate delta range
        if self.delta < MIN_DELTA || self.delta > MAX_DELTA {
            return Err(format!(
                "delta must be between {} and {}, got {}",
                MIN_DELTA, MAX_DELTA, self.delta
            ));
        }

        // Validate UUID
        Uuid::parse_str(&self.node_id)
            .map_err(|e| format!("Invalid UUID format for node_id '{}': {}", self.node_id, e))
    }

    /// Apply the delta to an importance value, clamping to [0.0, 1.0].
    ///
    /// Per BR-MCP-002: boost_importance clamps final value to [0.0, 1.0]
    ///
    /// # Returns
    /// Tuple of (new_importance, was_clamped)
    pub fn apply_delta(&self, current_importance: f32) -> (f32, bool) {
        let raw = current_importance + self.delta;
        let clamped = raw.clamp(MIN_IMPORTANCE, MAX_IMPORTANCE);
        let was_clamped = (raw - clamped).abs() > f32::EPSILON;
        (clamped, was_clamped)
    }
}

// ============================================================================
// RESPONSE DTOs
// ============================================================================

/// Response for forget_concept tool.
#[derive(Debug, Clone, Serialize)]
pub struct ForgetConceptResponse {
    /// UUID of the forgotten memory
    pub forgotten_id: Uuid,

    /// Whether soft delete was used
    pub soft_deleted: bool,

    /// When the memory can be recovered until (if soft deleted)
    /// Per SEC-06: 30 days from deletion
    /// Only present if soft_deleted is true
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recoverable_until: Option<DateTime<Utc>>,
}

impl ForgetConceptResponse {
    /// Create a response for a soft delete operation.
    ///
    /// Computes recovery deadline as 30 days from now per SEC-06.
    pub fn soft_deleted(id: Uuid) -> Self {
        Self {
            forgotten_id: id,
            soft_deleted: true,
            recoverable_until: Some(compute_recovery_deadline(Utc::now())),
        }
    }

    /// Create a response for a soft delete operation with a specific deletion time.
    ///
    /// Utility function for testing or when the deletion timestamp is known.
    #[allow(dead_code)]
    pub fn soft_deleted_at(id: Uuid, deleted_at: DateTime<Utc>) -> Self {
        Self {
            forgotten_id: id,
            soft_deleted: true,
            recoverable_until: Some(compute_recovery_deadline(deleted_at)),
        }
    }

    /// Create a response for a hard delete operation.
    pub fn hard_deleted(id: Uuid) -> Self {
        Self {
            forgotten_id: id,
            soft_deleted: false,
            recoverable_until: None,
        }
    }
}

/// Compute the recovery deadline from a deletion timestamp.
///
/// Per SEC-06: 30-day recovery window.
pub fn compute_recovery_deadline(deleted_at: DateTime<Utc>) -> DateTime<Utc> {
    deleted_at + Duration::days(SOFT_DELETE_RECOVERY_DAYS)
}

/// Response for boost_importance tool.
#[derive(Debug, Clone, Serialize)]
pub struct BoostImportanceResponse {
    /// UUID of the modified memory
    pub node_id: Uuid,

    /// Importance value before modification
    pub old_importance: f32,

    /// Importance value after modification (clamped to [0.0, 1.0])
    pub new_importance: f32,

    /// Whether the final value was clamped
    /// True if (old + delta) was outside [0.0, 1.0]
    pub clamped: bool,
}

impl BoostImportanceResponse {
    /// Create a new response with computed values.
    pub fn new(node_id: Uuid, old_importance: f32, delta: f32) -> Self {
        let raw_new = old_importance + delta;
        let new_importance = raw_new.clamp(MIN_IMPORTANCE, MAX_IMPORTANCE);
        let clamped = (raw_new - new_importance).abs() > f32::EPSILON;

        Self {
            node_id,
            old_importance,
            new_importance,
            clamped,
        }
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ===== ForgetConceptRequest Tests =====

    #[test]
    fn test_forget_concept_request_defaults() {
        let json = r#"{"node_id": "550e8400-e29b-41d4-a716-446655440000"}"#;
        let req: ForgetConceptRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.node_id, "550e8400-e29b-41d4-a716-446655440000");
        assert!(
            req.soft_delete,
            "soft_delete should default to true per SEC-06"
        );
        println!("[PASS] ForgetConceptRequest defaults soft_delete to true per SEC-06");
    }

    #[test]
    fn test_forget_concept_request_hard_delete() {
        let json = r#"{"node_id": "550e8400-e29b-41d4-a716-446655440000", "soft_delete": false}"#;
        let req: ForgetConceptRequest = serde_json::from_str(json).unwrap();

        assert!(!req.soft_delete);
        println!("[PASS] ForgetConceptRequest accepts soft_delete=false");
    }

    #[test]
    fn test_forget_concept_request_validation_valid() {
        let req = ForgetConceptRequest {
            node_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            soft_delete: true,
        };

        let result = req.validate();
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
        );
        println!("[PASS] ForgetConceptRequest validates correct UUID");
    }

    #[test]
    fn test_forget_concept_request_validation_invalid_uuid() {
        let req = ForgetConceptRequest {
            node_id: "not-a-valid-uuid".to_string(),
            soft_delete: true,
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid UUID format"));
        println!("[PASS] ForgetConceptRequest rejects invalid UUID");
    }

    #[test]
    fn test_forget_concept_request_validation_empty_uuid() {
        let req = ForgetConceptRequest {
            node_id: "".to_string(),
            soft_delete: true,
        };

        let result = req.validate();
        assert!(result.is_err());
        println!("[PASS] ForgetConceptRequest rejects empty UUID");
    }

    // ===== BoostImportanceRequest Tests =====

    #[test]
    fn test_boost_importance_request() {
        let json = r#"{"node_id": "550e8400-e29b-41d4-a716-446655440000", "delta": 0.3}"#;
        let req: BoostImportanceRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.node_id, "550e8400-e29b-41d4-a716-446655440000");
        assert!((req.delta - 0.3).abs() < f32::EPSILON);
        println!("[PASS] BoostImportanceRequest parses correctly");
    }

    #[test]
    fn test_boost_importance_negative_delta() {
        let json = r#"{"node_id": "test-id", "delta": -0.5}"#;
        let req: BoostImportanceRequest = serde_json::from_str(json).unwrap();

        assert!((req.delta - (-0.5)).abs() < f32::EPSILON);
        // Note: validation will fail on "test-id" not being a UUID
        println!("[PASS] BoostImportanceRequest accepts negative delta");
    }

    #[test]
    fn test_boost_importance_validation_valid() {
        let req = BoostImportanceRequest {
            node_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            delta: 0.3,
        };

        let result = req.validate();
        assert!(result.is_ok());
        println!("[PASS] BoostImportanceRequest validates correct input");
    }

    #[test]
    fn test_boost_importance_validation_delta_too_high() {
        let req = BoostImportanceRequest {
            node_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            delta: 1.5,
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("delta must be between"));
        println!("[PASS] BoostImportanceRequest rejects delta > 1.0");
    }

    #[test]
    fn test_boost_importance_validation_delta_too_low() {
        let req = BoostImportanceRequest {
            node_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            delta: -1.5,
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("delta must be between"));
        println!("[PASS] BoostImportanceRequest rejects delta < -1.0");
    }

    #[test]
    fn test_boost_importance_apply_delta_no_clamp() {
        let req = BoostImportanceRequest {
            node_id: "test".to_string(),
            delta: 0.2,
        };

        let (new_val, clamped) = req.apply_delta(0.5);
        assert!((new_val - 0.7).abs() < f32::EPSILON);
        assert!(!clamped);
        println!("[PASS] apply_delta works without clamping");
    }

    #[test]
    fn test_boost_importance_apply_delta_clamp_high() {
        let req = BoostImportanceRequest {
            node_id: "test".to_string(),
            delta: 0.5,
        };

        let (new_val, clamped) = req.apply_delta(0.8);
        assert!((new_val - 1.0).abs() < f32::EPSILON);
        assert!(clamped);
        println!("[PASS] apply_delta clamps at 1.0 per BR-MCP-002");
    }

    #[test]
    fn test_boost_importance_apply_delta_clamp_low() {
        let req = BoostImportanceRequest {
            node_id: "test".to_string(),
            delta: -0.5,
        };

        let (new_val, clamped) = req.apply_delta(0.2);
        assert!((new_val - 0.0).abs() < f32::EPSILON);
        assert!(clamped);
        println!("[PASS] apply_delta clamps at 0.0 per BR-MCP-002");
    }

    // ===== ForgetConceptResponse Tests =====

    #[test]
    fn test_forget_concept_response_serialization_soft() {
        let recovery_time = Utc::now() + Duration::days(30);
        let response = ForgetConceptResponse {
            forgotten_id: Uuid::nil(),
            soft_deleted: true,
            recoverable_until: Some(recovery_time),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"soft_deleted\":true"));
        assert!(json.contains("\"recoverable_until\""));
        println!("[PASS] ForgetConceptResponse serializes soft delete correctly");
    }

    #[test]
    fn test_forget_concept_response_serialization_hard() {
        let response = ForgetConceptResponse {
            forgotten_id: Uuid::nil(),
            soft_deleted: false,
            recoverable_until: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"soft_deleted\":false"));
        assert!(
            !json.contains("\"recoverable_until\""),
            "recoverable_until should be skipped when None"
        );
        println!("[PASS] ForgetConceptResponse skips recoverable_until when None");
    }

    #[test]
    fn test_forget_concept_response_soft_deleted_factory() {
        let id = Uuid::new_v4();
        let response = ForgetConceptResponse::soft_deleted(id);

        assert_eq!(response.forgotten_id, id);
        assert!(response.soft_deleted);
        assert!(response.recoverable_until.is_some());

        // Verify recovery is ~30 days from now
        let recovery = response.recoverable_until.unwrap();
        let expected = Utc::now() + Duration::days(30);
        let diff = (recovery - expected).num_seconds().abs();
        assert!(diff < 5, "Recovery time should be ~30 days from now");
        println!("[PASS] ForgetConceptResponse::soft_deleted sets 30-day recovery per SEC-06");
    }

    #[test]
    fn test_forget_concept_response_hard_deleted_factory() {
        let id = Uuid::new_v4();
        let response = ForgetConceptResponse::hard_deleted(id);

        assert_eq!(response.forgotten_id, id);
        assert!(!response.soft_deleted);
        assert!(response.recoverable_until.is_none());
        println!("[PASS] ForgetConceptResponse::hard_deleted works");
    }

    // ===== BoostImportanceResponse Tests =====

    #[test]
    fn test_boost_importance_response_serialization() {
        let response = BoostImportanceResponse {
            node_id: Uuid::nil(),
            old_importance: 0.5,
            new_importance: 0.7,
            clamped: false,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"old_importance\":0.5"));
        assert!(json.contains("\"new_importance\":0.7"));
        assert!(json.contains("\"clamped\":false"));
        println!("[PASS] BoostImportanceResponse serializes correctly");
    }

    #[test]
    fn test_boost_importance_response_clamped() {
        let response = BoostImportanceResponse {
            node_id: Uuid::nil(),
            old_importance: 0.9,
            new_importance: 1.0, // Was clamped from 1.1
            clamped: true,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"clamped\":true"));
        println!("[PASS] BoostImportanceResponse indicates clamping");
    }

    #[test]
    fn test_boost_importance_response_new_factory() {
        let id = Uuid::new_v4();

        // Normal case - no clamping
        let response = BoostImportanceResponse::new(id, 0.5, 0.2);
        assert!((response.old_importance - 0.5).abs() < f32::EPSILON);
        assert!((response.new_importance - 0.7).abs() < f32::EPSILON);
        assert!(!response.clamped);

        // Clamp at max
        let response = BoostImportanceResponse::new(id, 0.9, 0.5);
        assert!((response.new_importance - 1.0).abs() < f32::EPSILON);
        assert!(response.clamped);

        // Clamp at min
        let response = BoostImportanceResponse::new(id, 0.1, -0.5);
        assert!((response.new_importance - 0.0).abs() < f32::EPSILON);
        assert!(response.clamped);

        println!("[PASS] BoostImportanceResponse::new computes clamping correctly");
    }

    // ===== Edge Case Tests =====

    #[test]
    fn test_delta_at_boundaries() {
        // Exactly at +1.0
        let req = BoostImportanceRequest {
            node_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            delta: 1.0,
        };
        assert!(req.validate().is_ok());

        // Exactly at -1.0
        let req = BoostImportanceRequest {
            node_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            delta: -1.0,
        };
        assert!(req.validate().is_ok());

        println!("[PASS] Delta boundary values are accepted");
    }

    #[test]
    fn test_delta_zero() {
        let req = BoostImportanceRequest {
            node_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            delta: 0.0,
        };

        assert!(req.validate().is_ok());

        let (new_val, clamped) = req.apply_delta(0.5);
        assert!((new_val - 0.5).abs() < f32::EPSILON);
        assert!(!clamped);

        println!("[PASS] Zero delta is valid and doesn't change importance");
    }

    #[test]
    fn test_recovery_deadline_calculation() {
        let now = Utc::now();
        let deadline = compute_recovery_deadline(now);

        let diff_days = (deadline - now).num_days();
        assert_eq!(diff_days, 30);
        println!("[PASS] Recovery deadline is 30 days per SEC-06");
    }

    // ===== Constitution Compliance Tests =====

    #[test]
    fn test_constants_match_constitution() {
        // SEC-06: 30-day recovery
        assert_eq!(SOFT_DELETE_RECOVERY_DAYS, 30);

        // BR-MCP-002: Importance clamped to [0.0, 1.0]
        assert!((MIN_IMPORTANCE - 0.0).abs() < f32::EPSILON);
        assert!((MAX_IMPORTANCE - 1.0).abs() < f32::EPSILON);

        // Delta range for boost_importance
        assert!((MIN_DELTA - (-1.0)).abs() < f32::EPSILON);
        assert!((MAX_DELTA - 1.0).abs() < f32::EPSILON);

        println!("[PASS] Constants match constitution requirements");
    }

    #[test]
    fn test_soft_delete_is_default() {
        // BR-MCP-001: forget_concept uses soft delete by default
        let json = r#"{"node_id": "550e8400-e29b-41d4-a716-446655440000"}"#;
        let req: ForgetConceptRequest = serde_json::from_str(json).unwrap();
        assert!(
            req.soft_delete,
            "soft_delete must default to true per BR-MCP-001"
        );
        println!("[PASS] Soft delete is default per BR-MCP-001");
    }

    #[test]
    fn test_importance_clamping_enforced() {
        // BR-MCP-002: boost_importance clamps final value to [0.0, 1.0]
        let req = BoostImportanceRequest {
            node_id: "test".to_string(),
            delta: 100.0, // Extreme value, but we clamp the result not the input
        };

        // Even if we somehow got an extreme delta, apply_delta clamps correctly
        // Note: validate() would reject delta=100.0, but apply_delta is separate
        let (new_val, clamped) = req.apply_delta(0.5);
        // 0.5 + 100.0 = 100.5 -> clamped to 1.0
        assert!((new_val - 1.0).abs() < f32::EPSILON);
        assert!(clamped);
        println!("[PASS] Importance clamping enforced per BR-MCP-002");
    }

    // ===== Default Impl Tests =====

    #[test]
    fn test_forget_concept_request_default_impl() {
        let req = ForgetConceptRequest::default();
        assert!(req.node_id.is_empty());
        assert!(
            req.soft_delete,
            "Default should use soft_delete=true per BR-MCP-001"
        );
        println!("[PASS] ForgetConceptRequest::default() uses soft_delete=true");
    }

    #[test]
    fn test_boost_importance_request_default_impl() {
        let req = BoostImportanceRequest::default();
        assert!(req.node_id.is_empty());
        assert!((req.delta - 0.0).abs() < f32::EPSILON);
        println!("[PASS] BoostImportanceRequest::default() uses delta=0.0");
    }

    // ===== NaN/Infinity Tests (AP-10 compliance) =====

    #[test]
    fn test_boost_importance_rejects_nan() {
        let req = BoostImportanceRequest {
            node_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            delta: f32::NAN,
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("finite number"));
        println!("[PASS] BoostImportanceRequest rejects NaN delta per AP-10");
    }

    #[test]
    fn test_boost_importance_rejects_infinity() {
        let req = BoostImportanceRequest {
            node_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            delta: f32::INFINITY,
        };

        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("finite number"));

        let req_neg = BoostImportanceRequest {
            node_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            delta: f32::NEG_INFINITY,
        };

        let result_neg = req_neg.validate();
        assert!(result_neg.is_err());
        assert!(result_neg.unwrap_err().contains("finite number"));
        println!("[PASS] BoostImportanceRequest rejects infinity delta per AP-10");
    }
}
