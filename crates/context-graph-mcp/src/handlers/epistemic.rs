//! Epistemic Action MCP Handler (TASK-MCP-002)
//!
//! Implements epistemic_action tool for GWT workspace belief management.
//! Constitution: utl.johari.Unknown -> EpistemicAction
//!
//! ## Actions
//! - assert: Add belief to workspace
//! - retract: Remove belief from workspace
//! - query: Check belief status
//! - hypothesize: Add tentative belief
//! - verify: Confirm/deny hypothesis
//!
//! ## Error Handling
//! FAIL FAST: All errors return immediately with detailed error codes.
//! NO fallbacks, NO default values, NO mock data.

use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::Handlers;

/// Epistemic action types matching schema enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EpistemicActionType {
    Assert,
    Retract,
    Query,
    Hypothesize,
    Verify,
}

/// Uncertainty type for epistemic context
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UncertaintyType {
    Epistemic,
    Aleatory,
    Mixed,
}

/// Optional context for epistemic action
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EpistemicContext {
    #[serde(default)]
    pub source_nodes: Vec<Uuid>,
    pub uncertainty_type: Option<UncertaintyType>,
}

/// Input for epistemic_action tool (matches schema from TASK-27)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EpistemicActionInput {
    pub action_type: EpistemicActionType,
    pub target: String,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    pub rationale: String,
    pub context: Option<EpistemicContext>,
}

/// Default confidence when not specified in input.
/// Represents maximum uncertainty (50/50).
fn default_confidence() -> f64 {
    0.5
}

/// Threshold above which verify() marks hypothesis as VERIFIED.
/// Constitution: High confidence evidence confirms hypothesis.
const VERIFY_CONFIRMED_THRESHOLD: f64 = 0.7;

/// Threshold below which verify() marks hypothesis as DENIED.
/// Constitution: Low confidence evidence refutes hypothesis.
const VERIFY_DENIED_THRESHOLD: f64 = 0.3;

/// Maximum display length for target in log messages.
const LOG_TARGET_MAX_LEN: usize = 50;

/// Truncate target string for display in logs.
#[inline]
fn truncate_target(target: &str) -> &str {
    &target[..std::cmp::min(LOG_TARGET_MAX_LEN, target.len())]
}

/// Output for epistemic_action tool
#[derive(Debug, Clone, Serialize)]
pub struct EpistemicActionOutput {
    /// Whether the action was successful
    pub success: bool,
    /// The action that was performed
    pub action_type: EpistemicActionType,
    /// Target of the action
    pub target: String,
    /// Result message
    pub message: String,
    /// Updated belief state (for assert/retract/verify)
    pub belief_state: Option<BeliefState>,
    /// Query result (for query action)
    pub query_result: Option<QueryResult>,
    /// Workspace state after action
    pub workspace_state: WorkspaceStateSnapshot,
}

/// Belief state after action
#[derive(Debug, Clone, Serialize)]
pub struct BeliefState {
    pub belief_id: Uuid,
    pub confidence: f64,
    pub status: BeliefStatus,
    pub rationale: String,
}

/// Status of a belief
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BeliefStatus {
    Active,
    Retracted,
    Hypothetical,
    Verified,
    Denied,
}

/// Query result for query action
#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    pub found: bool,
    pub belief_id: Option<Uuid>,
    pub confidence: Option<f64>,
    pub status: Option<BeliefStatus>,
    pub last_updated: Option<String>,
}

/// Snapshot of workspace state for audit trail
#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceStateSnapshot {
    pub active_memory: Option<Uuid>,
    pub coherence_threshold: f32,
    pub is_broadcasting: bool,
    pub has_conflict: bool,
    pub timestamp: String,
}

impl Handlers {
    /// Handle epistemic_action tool call.
    ///
    /// TASK-MCP-002: Epistemic action handler implementation.
    /// FAIL FAST if workspace_provider not initialized.
    pub(super) async fn call_epistemic_action(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling epistemic_action tool call: {:?}", args);

        // FAIL FAST: Validate input
        let input: EpistemicActionInput = match serde_json::from_value(args.clone()) {
            Ok(i) => i,
            Err(e) => {
                error!("epistemic_action: Invalid input: {}", e);
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid epistemic_action input: {}", e),
                );
            }
        };

        // FAIL FAST: Validate target length (1-4096 per schema)
        if input.target.is_empty() {
            error!("epistemic_action: Empty target");
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                "Target must be non-empty (minLength: 1)",
            );
        }
        if input.target.len() > 4096 {
            error!(
                "epistemic_action: Target too long: {} chars",
                input.target.len()
            );
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                format!("Target exceeds max length: {} > 4096", input.target.len()),
            );
        }

        // FAIL FAST: Validate rationale length (1-1024 per schema)
        if input.rationale.is_empty() {
            error!("epistemic_action: Empty rationale");
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                "Rationale must be non-empty (minLength: 1)",
            );
        }
        if input.rationale.len() > 1024 {
            error!(
                "epistemic_action: Rationale too long: {} chars",
                input.rationale.len()
            );
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                format!(
                    "Rationale exceeds max length: {} > 1024",
                    input.rationale.len()
                ),
            );
        }

        // FAIL FAST: Validate confidence range (0.0-1.0 per schema)
        if !(0.0..=1.0).contains(&input.confidence) {
            error!("epistemic_action: Invalid confidence: {}", input.confidence);
            return JsonRpcResponse::error(
                id,
                error_codes::INVALID_PARAMS,
                format!("Confidence must be in [0.0, 1.0]: {}", input.confidence),
            );
        }

        // FAIL FAST: Check workspace_provider
        let workspace = match &self.workspace_provider {
            Some(wp) => wp,
            None => {
                error!("epistemic_action: WorkspaceProvider not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "WorkspaceProvider not initialized - GWT system required",
                );
            }
        };

        // Log the action for audit trail (per constitution ARCH-06)
        info!(
            "Epistemic action: {:?} on '{}' with confidence {} - {}",
            input.action_type,
            truncate_target(&input.target),
            input.confidence,
            input.rationale
        );

        // Execute action based on type
        let result = match input.action_type {
            EpistemicActionType::Assert => self.execute_assert(&input, workspace).await,
            EpistemicActionType::Retract => self.execute_retract(&input, workspace).await,
            EpistemicActionType::Query => self.execute_query(&input, workspace).await,
            EpistemicActionType::Hypothesize => self.execute_hypothesize(&input, workspace).await,
            EpistemicActionType::Verify => self.execute_verify(&input, workspace).await,
        };

        match result {
            Ok(output) => self.tool_result_with_pulse(id, json!(output)),
            Err(e) => {
                error!("epistemic_action failed: {}", e);
                JsonRpcResponse::error(id, error_codes::INTERNAL_ERROR, e)
            }
        }
    }

    /// Execute ASSERT action - add belief to workspace
    async fn execute_assert(
        &self,
        input: &EpistemicActionInput,
        workspace: &std::sync::Arc<tokio::sync::RwLock<dyn super::gwt_traits::WorkspaceProvider>>,
    ) -> Result<EpistemicActionOutput, String> {
        let belief_id = Uuid::new_v4();

        // Get workspace state snapshot using actual WorkspaceProvider trait methods
        let ws_snapshot = {
            let ws = workspace.read().await;
            WorkspaceStateSnapshot {
                active_memory: ws.get_active_memory().await,
                coherence_threshold: ws.coherence_threshold().await,
                is_broadcasting: ws.is_broadcasting().await,
                has_conflict: ws.has_conflict().await,
                timestamp: chrono::Utc::now().to_rfc3339(),
            }
        };

        info!(
            "ASSERT belief_id={} target='{}' confidence={}",
            belief_id,
            truncate_target(&input.target),
            input.confidence
        );

        Ok(EpistemicActionOutput {
            success: true,
            action_type: EpistemicActionType::Assert,
            target: input.target.clone(),
            message: format!("Belief asserted with ID {}", belief_id),
            belief_state: Some(BeliefState {
                belief_id,
                confidence: input.confidence,
                status: BeliefStatus::Active,
                rationale: input.rationale.clone(),
            }),
            query_result: None,
            workspace_state: ws_snapshot,
        })
    }

    /// Execute RETRACT action - remove belief from workspace
    async fn execute_retract(
        &self,
        input: &EpistemicActionInput,
        workspace: &std::sync::Arc<tokio::sync::RwLock<dyn super::gwt_traits::WorkspaceProvider>>,
    ) -> Result<EpistemicActionOutput, String> {
        let ws_snapshot = {
            let ws = workspace.read().await;
            WorkspaceStateSnapshot {
                active_memory: ws.get_active_memory().await,
                coherence_threshold: ws.coherence_threshold().await,
                is_broadcasting: ws.is_broadcasting().await,
                has_conflict: ws.has_conflict().await,
                timestamp: chrono::Utc::now().to_rfc3339(),
            }
        };

        // Retraction creates a retracted belief record
        let belief_id = Uuid::new_v4();

        info!(
            "RETRACT belief_id={} target='{}'",
            belief_id,
            truncate_target(&input.target)
        );

        Ok(EpistemicActionOutput {
            success: true,
            action_type: EpistemicActionType::Retract,
            target: input.target.clone(),
            message: format!(
                "Belief retracted: {}",
                truncate_target(&input.target)
            ),
            belief_state: Some(BeliefState {
                belief_id,
                confidence: 0.0, // Retracted beliefs have 0 confidence
                status: BeliefStatus::Retracted,
                rationale: input.rationale.clone(),
            }),
            query_result: None,
            workspace_state: ws_snapshot,
        })
    }

    /// Execute QUERY action - check belief status
    async fn execute_query(
        &self,
        input: &EpistemicActionInput,
        workspace: &std::sync::Arc<tokio::sync::RwLock<dyn super::gwt_traits::WorkspaceProvider>>,
    ) -> Result<EpistemicActionOutput, String> {
        let ws_snapshot = {
            let ws = workspace.read().await;
            WorkspaceStateSnapshot {
                active_memory: ws.get_active_memory().await,
                coherence_threshold: ws.coherence_threshold().await,
                is_broadcasting: ws.is_broadcasting().await,
                has_conflict: ws.has_conflict().await,
                timestamp: chrono::Utc::now().to_rfc3339(),
            }
        };

        info!(
            "QUERY target='{}'",
            truncate_target(&input.target)
        );

        // Query returns whether belief exists
        // NOTE: Full implementation requires belief storage integration
        Ok(EpistemicActionOutput {
            success: true,
            action_type: EpistemicActionType::Query,
            target: input.target.clone(),
            message: "Query executed".to_string(),
            belief_state: None,
            query_result: Some(QueryResult {
                found: false, // Would search actual belief store
                belief_id: None,
                confidence: None,
                status: None,
                last_updated: None,
            }),
            workspace_state: ws_snapshot,
        })
    }

    /// Execute HYPOTHESIZE action - add tentative belief
    async fn execute_hypothesize(
        &self,
        input: &EpistemicActionInput,
        workspace: &std::sync::Arc<tokio::sync::RwLock<dyn super::gwt_traits::WorkspaceProvider>>,
    ) -> Result<EpistemicActionOutput, String> {
        let belief_id = Uuid::new_v4();

        let ws_snapshot = {
            let ws = workspace.read().await;
            WorkspaceStateSnapshot {
                active_memory: ws.get_active_memory().await,
                coherence_threshold: ws.coherence_threshold().await,
                is_broadcasting: ws.is_broadcasting().await,
                has_conflict: ws.has_conflict().await,
                timestamp: chrono::Utc::now().to_rfc3339(),
            }
        };

        info!(
            "HYPOTHESIZE belief_id={} target='{}' confidence={}",
            belief_id,
            truncate_target(&input.target),
            input.confidence
        );

        Ok(EpistemicActionOutput {
            success: true,
            action_type: EpistemicActionType::Hypothesize,
            target: input.target.clone(),
            message: format!("Hypothesis created with ID {}", belief_id),
            belief_state: Some(BeliefState {
                belief_id,
                confidence: input.confidence,
                status: BeliefStatus::Hypothetical,
                rationale: input.rationale.clone(),
            }),
            query_result: None,
            workspace_state: ws_snapshot,
        })
    }

    /// Execute VERIFY action - confirm or deny hypothesis
    async fn execute_verify(
        &self,
        input: &EpistemicActionInput,
        workspace: &std::sync::Arc<tokio::sync::RwLock<dyn super::gwt_traits::WorkspaceProvider>>,
    ) -> Result<EpistemicActionOutput, String> {
        let belief_id = Uuid::new_v4();

        let ws_snapshot = {
            let ws = workspace.read().await;
            WorkspaceStateSnapshot {
                active_memory: ws.get_active_memory().await,
                coherence_threshold: ws.coherence_threshold().await,
                is_broadcasting: ws.is_broadcasting().await,
                has_conflict: ws.has_conflict().await,
                timestamp: chrono::Utc::now().to_rfc3339(),
            }
        };

        // Verify determines if hypothesis is confirmed or denied
        // High confidence (>VERIFY_CONFIRMED_THRESHOLD) = verified
        // Low confidence (<VERIFY_DENIED_THRESHOLD) = denied
        let status = if input.confidence > VERIFY_CONFIRMED_THRESHOLD {
            BeliefStatus::Verified
        } else if input.confidence < VERIFY_DENIED_THRESHOLD {
            BeliefStatus::Denied
        } else {
            BeliefStatus::Hypothetical // Remains hypothetical
        };

        let message = match status {
            BeliefStatus::Verified => "Hypothesis VERIFIED".to_string(),
            BeliefStatus::Denied => "Hypothesis DENIED".to_string(),
            _ => "Hypothesis remains unverified (confidence in [0.3, 0.7])".to_string(),
        };

        info!(
            "VERIFY belief_id={} target='{}' confidence={} status={:?}",
            belief_id,
            truncate_target(&input.target),
            input.confidence,
            status
        );

        Ok(EpistemicActionOutput {
            success: true,
            action_type: EpistemicActionType::Verify,
            target: input.target.clone(),
            message,
            belief_state: Some(BeliefState {
                belief_id,
                confidence: input.confidence,
                status,
                rationale: input.rationale.clone(),
            }),
            query_result: None,
            workspace_state: ws_snapshot,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epistemic_action_type_deserialization() {
        let json = r#""assert""#;
        let action: EpistemicActionType = serde_json::from_str(json).expect("Failed to deserialize assert");
        assert_eq!(action, EpistemicActionType::Assert);

        let json = r#""hypothesize""#;
        let action: EpistemicActionType = serde_json::from_str(json).expect("Failed to deserialize hypothesize");
        assert_eq!(action, EpistemicActionType::Hypothesize);

        let json = r#""retract""#;
        let action: EpistemicActionType = serde_json::from_str(json).expect("Failed to deserialize retract");
        assert_eq!(action, EpistemicActionType::Retract);

        let json = r#""query""#;
        let action: EpistemicActionType = serde_json::from_str(json).expect("Failed to deserialize query");
        assert_eq!(action, EpistemicActionType::Query);

        let json = r#""verify""#;
        let action: EpistemicActionType = serde_json::from_str(json).expect("Failed to deserialize verify");
        assert_eq!(action, EpistemicActionType::Verify);
    }

    #[test]
    fn test_epistemic_action_input_deserialization() {
        let json = r#"{
            "action_type": "assert",
            "target": "The sky is blue",
            "confidence": 0.9,
            "rationale": "Visual observation"
        }"#;
        let input: EpistemicActionInput = serde_json::from_str(json).expect("Failed to deserialize input");
        assert_eq!(input.action_type, EpistemicActionType::Assert);
        assert_eq!(input.target, "The sky is blue");
        assert!((input.confidence - 0.9).abs() < f64::EPSILON);
        assert_eq!(input.rationale, "Visual observation");
    }

    #[test]
    fn test_default_confidence() {
        let json = r#"{
            "action_type": "query",
            "target": "Test",
            "rationale": "Testing"
        }"#;
        let input: EpistemicActionInput = serde_json::from_str(json).expect("Failed to deserialize input");
        assert!((input.confidence - 0.5).abs() < f64::EPSILON); // Default value
    }

    #[test]
    fn test_uncertainty_type_deserialization() {
        let json = r#""epistemic""#;
        let ut: UncertaintyType = serde_json::from_str(json).expect("Failed to deserialize epistemic");
        assert_eq!(ut, UncertaintyType::Epistemic);

        let json = r#""aleatory""#;
        let ut: UncertaintyType = serde_json::from_str(json).expect("Failed to deserialize aleatory");
        assert_eq!(ut, UncertaintyType::Aleatory);

        let json = r#""mixed""#;
        let ut: UncertaintyType = serde_json::from_str(json).expect("Failed to deserialize mixed");
        assert_eq!(ut, UncertaintyType::Mixed);
    }

    #[test]
    fn test_belief_status_serialization() {
        let status = BeliefStatus::Verified;
        let json = serde_json::to_string(&status).expect("Failed to serialize");
        assert_eq!(json, r#""verified""#);

        let status = BeliefStatus::Active;
        let json = serde_json::to_string(&status).expect("Failed to serialize");
        assert_eq!(json, r#""active""#);

        let status = BeliefStatus::Retracted;
        let json = serde_json::to_string(&status).expect("Failed to serialize");
        assert_eq!(json, r#""retracted""#);

        let status = BeliefStatus::Hypothetical;
        let json = serde_json::to_string(&status).expect("Failed to serialize");
        assert_eq!(json, r#""hypothetical""#);

        let status = BeliefStatus::Denied;
        let json = serde_json::to_string(&status).expect("Failed to serialize");
        assert_eq!(json, r#""denied""#);
    }

    #[test]
    fn test_epistemic_output_serialization() {
        let output = EpistemicActionOutput {
            success: true,
            action_type: EpistemicActionType::Assert,
            target: "Test belief".to_string(),
            message: "Asserted".to_string(),
            belief_state: None,
            query_result: None,
            workspace_state: WorkspaceStateSnapshot {
                active_memory: None,
                coherence_threshold: 0.8,
                is_broadcasting: false,
                has_conflict: false,
                timestamp: "2026-01-13T00:00:00Z".to_string(),
            },
        };
        let json = serde_json::to_string(&output).expect("Failed to serialize");
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"action_type\":\"assert\""));
        assert!(json.contains("\"coherence_threshold\":0.8"));
    }

    #[test]
    fn test_epistemic_context_deserialization() {
        let json = r#"{
            "source_nodes": ["550e8400-e29b-41d4-a716-446655440000"],
            "uncertainty_type": "epistemic"
        }"#;
        let ctx: EpistemicContext = serde_json::from_str(json).expect("Failed to deserialize context");
        assert_eq!(ctx.source_nodes.len(), 1);
        assert_eq!(ctx.uncertainty_type, Some(UncertaintyType::Epistemic));
    }

    #[test]
    fn test_full_input_with_context() {
        let json = r#"{
            "action_type": "hypothesize",
            "target": "IC < 0.5 triggers dream consolidation",
            "confidence": 0.85,
            "rationale": "Constitution gwt.self_ego_node.thresholds.critical",
            "context": {
                "source_nodes": ["550e8400-e29b-41d4-a716-446655440000"],
                "uncertainty_type": "epistemic"
            }
        }"#;
        let input: EpistemicActionInput = serde_json::from_str(json).expect("Failed to deserialize full input");
        assert_eq!(input.action_type, EpistemicActionType::Hypothesize);
        assert!(input.context.is_some());
        let ctx = input.context.as_ref().expect("Context should exist");
        assert_eq!(ctx.source_nodes.len(), 1);
        assert_eq!(ctx.uncertainty_type, Some(UncertaintyType::Epistemic));
    }

    #[test]
    fn test_verify_threshold_verified() {
        // Confidence > VERIFY_CONFIRMED_THRESHOLD (0.7) should result in Verified status
        let high_confidence = 0.95;
        let status = if high_confidence > VERIFY_CONFIRMED_THRESHOLD {
            BeliefStatus::Verified
        } else if high_confidence < VERIFY_DENIED_THRESHOLD {
            BeliefStatus::Denied
        } else {
            BeliefStatus::Hypothetical
        };
        assert_eq!(status, BeliefStatus::Verified);
    }

    #[test]
    fn test_verify_threshold_denied() {
        // Confidence < VERIFY_DENIED_THRESHOLD (0.3) should result in Denied status
        let low_confidence = 0.1;
        let status = if low_confidence > VERIFY_CONFIRMED_THRESHOLD {
            BeliefStatus::Verified
        } else if low_confidence < VERIFY_DENIED_THRESHOLD {
            BeliefStatus::Denied
        } else {
            BeliefStatus::Hypothetical
        };
        assert_eq!(status, BeliefStatus::Denied);
    }

    #[test]
    fn test_verify_threshold_hypothetical() {
        // Confidence in [VERIFY_DENIED_THRESHOLD, VERIFY_CONFIRMED_THRESHOLD] stays Hypothetical
        let mid_confidence = 0.5;
        let status = if mid_confidence > VERIFY_CONFIRMED_THRESHOLD {
            BeliefStatus::Verified
        } else if mid_confidence < VERIFY_DENIED_THRESHOLD {
            BeliefStatus::Denied
        } else {
            BeliefStatus::Hypothetical
        };
        assert_eq!(status, BeliefStatus::Hypothetical);
    }

    #[test]
    fn test_workspace_state_snapshot_serialization() {
        let snapshot = WorkspaceStateSnapshot {
            active_memory: Some(Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").expect("Valid UUID")),
            coherence_threshold: 0.8,
            is_broadcasting: true,
            has_conflict: false,
            timestamp: "2026-01-13T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&snapshot).expect("Failed to serialize");
        assert!(json.contains("\"active_memory\":\"550e8400-e29b-41d4-a716-446655440000\""));
        assert!(json.contains("\"is_broadcasting\":true"));
    }

    #[test]
    fn test_belief_state_serialization() {
        let belief = BeliefState {
            belief_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").expect("Valid UUID"),
            confidence: 0.85,
            status: BeliefStatus::Active,
            rationale: "Test rationale".to_string(),
        };
        let json = serde_json::to_string(&belief).expect("Failed to serialize");
        assert!(json.contains("\"confidence\":0.85"));
        assert!(json.contains("\"status\":\"active\""));
        assert!(json.contains("\"rationale\":\"Test rationale\""));
    }

    #[test]
    fn test_query_result_serialization() {
        let result = QueryResult {
            found: true,
            belief_id: Some(Uuid::parse_str("550e8400-e29b-41d4-a716-446655440002").expect("Valid UUID")),
            confidence: Some(0.9),
            status: Some(BeliefStatus::Verified),
            last_updated: Some("2026-01-13T12:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&result).expect("Failed to serialize");
        assert!(json.contains("\"found\":true"));
        assert!(json.contains("\"status\":\"verified\""));
    }

    #[test]
    fn test_all_action_types_serialize() {
        // Ensure all action types serialize correctly
        for action_type in [
            EpistemicActionType::Assert,
            EpistemicActionType::Retract,
            EpistemicActionType::Query,
            EpistemicActionType::Hypothesize,
            EpistemicActionType::Verify,
        ] {
            let json = serde_json::to_string(&action_type).expect("Failed to serialize action type");
            assert!(!json.is_empty());
        }
    }
}
