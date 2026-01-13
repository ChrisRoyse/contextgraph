//! MCP handlers for Meta-UTL self-correction tools.
//!
//! TASK-METAUTL-P0-005: Exposes meta-learning status, recalibration, and event log.

use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::core::event_log::EventLogQuery;
use super::core::meta_utl_service::{LambdaWeightsPair, MetaLearningService};
use super::core::{Domain, MetaLearningEventType};

/// Error types for meta-learning MCP handlers.
#[derive(Debug, Error)]
pub enum MetaLearningMcpError {
    #[error("Invalid event type '{value}'. Valid types: lambda_adjustment, bayesian_escalation, accuracy_alert, accuracy_recovery, weight_clamped")]
    InvalidEventType { value: String },

    #[error("Invalid domain '{value}'. Valid domains: code, medical, legal, creative, research, general")]
    InvalidDomain { value: String },

    #[error("Invalid timestamp format '{value}'. Expected ISO 8601 (e.g., 2024-01-15T10:30:00Z)")]
    InvalidTimestamp { value: String },

    #[allow(dead_code)]
    #[error("Pagination offset {offset} exceeds total count {total}")]
    InvalidPaginationOffset { offset: usize, total: usize },

    #[allow(dead_code)]
    #[error("Service unavailable: {reason}")]
    ServiceUnavailable { reason: String },

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for meta-learning handlers.
pub type McpResult<T> = Result<T, MetaLearningMcpError>;

/// Input for get_meta_learning_status tool.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GetMetaLearningStatusInput {
    /// Include detailed accuracy history (optional)
    #[serde(default)]
    pub include_accuracy_history: bool,
    /// Include per-embedder breakdown (optional)
    #[serde(default)]
    pub include_embedder_breakdown: bool,
}

/// Output for get_meta_learning_status tool.
#[derive(Debug, Clone, Serialize)]
pub struct MetaLearningStatusOutput {
    /// Whether self-correction is enabled
    pub enabled: bool,
    /// Current global accuracy (rolling average)
    pub current_accuracy: f32,
    /// Consecutive low accuracy cycle count
    pub consecutive_low_count: u32,
    /// Current lambda weights
    pub current_lambdas: LambdaValues,
    /// Base lambda weights (from lifecycle)
    pub base_lambdas: LambdaValues,
    /// Deviation from base weights
    pub lambda_deviation: LambdaValues,
    /// Current escalation status
    pub escalation_status: String,
    /// Total adjustments made
    pub adjustment_count: u64,
    /// Recent events count (last 24h)
    pub recent_events_count: usize,
    /// Last adjustment timestamp (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_adjustment_at: Option<String>,
    /// Accuracy history (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accuracy_history: Option<Vec<f32>>,
    /// Per-embedder accuracy (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedder_accuracy: Option<Vec<f32>>,
}

/// Lambda weight values.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LambdaValues {
    pub lambda_s: f32,
    pub lambda_c: f32,
}

impl From<LambdaWeightsPair> for LambdaValues {
    fn from(pair: LambdaWeightsPair) -> Self {
        Self {
            lambda_s: pair.lambda_s,
            lambda_c: pair.lambda_c,
        }
    }
}

impl From<context_graph_utl::lifecycle::LifecycleLambdaWeights> for LambdaValues {
    fn from(weights: context_graph_utl::lifecycle::LifecycleLambdaWeights) -> Self {
        Self {
            lambda_s: weights.lambda_s(),
            lambda_c: weights.lambda_c(),
        }
    }
}

/// Input for trigger_lambda_recalibration tool.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TriggerRecalibrationInput {
    /// Force Bayesian optimization (skip gradient check)
    #[serde(default)]
    pub force_bayesian: bool,
    /// Target domain for calibration (optional)
    pub domain: Option<String>,
    /// Dry run - compute but don't apply
    #[serde(default)]
    pub dry_run: bool,
}

/// Output for trigger_lambda_recalibration tool.
#[derive(Debug, Clone, Serialize)]
pub struct RecalibrationOutput {
    /// Whether recalibration succeeded
    pub success: bool,
    /// Adjustment applied (or that would be applied)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjustment: Option<AdjustmentDetails>,
    /// New lambda values
    pub new_lambdas: LambdaValues,
    /// Previous lambda values
    pub previous_lambdas: LambdaValues,
    /// Method used (gradient or bayesian)
    pub method: String,
    /// Bayesian optimization iterations (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bo_iterations: Option<usize>,
    /// Expected improvement (if BO)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_improvement: Option<f32>,
    /// Whether this was a dry run
    pub dry_run: bool,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Adjustment details.
#[derive(Debug, Clone, Serialize)]
pub struct AdjustmentDetails {
    pub delta_s: f32,
    pub delta_c: f32,
    pub alpha: f32,
    pub trigger_error: f32,
}

/// Input for get_meta_learning_log tool.
#[derive(Debug, Clone, Deserialize)]
pub struct GetMetaLearningLogInput {
    /// Start time (ISO 8601)
    pub start_time: Option<String>,
    /// End time (ISO 8601)
    pub end_time: Option<String>,
    /// Filter by event type
    pub event_type: Option<String>,
    /// Filter by domain
    pub domain: Option<String>,
    /// Maximum events to return
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Offset for pagination
    #[serde(default)]
    pub offset: usize,
}

fn default_limit() -> usize {
    100
}

impl Default for GetMetaLearningLogInput {
    fn default() -> Self {
        Self {
            start_time: None,
            end_time: None,
            event_type: None,
            domain: None,
            limit: default_limit(),
            offset: 0,
        }
    }
}

/// Output for get_meta_learning_log tool.
#[derive(Debug, Clone, Serialize)]
pub struct MetaLearningLogOutput {
    /// Events matching query
    pub events: Vec<MetaLearningEventOutput>,
    /// Total count (before limit/offset)
    pub total_count: usize,
    /// Whether there are more events
    pub has_more: bool,
    /// Query execution time (ms)
    pub query_time_ms: u32,
}

/// Serializable event representation.
#[derive(Debug, Clone, Serialize)]
pub struct MetaLearningEventOutput {
    pub timestamp: String,
    pub event_type: String,
    pub embedder_index: Option<usize>,
    pub previous_value: Option<f32>,
    pub new_value: Option<f32>,
    pub description: Option<String>,
    pub accuracy: Option<f32>,
    pub domain: Option<String>,
}

// ============================================================================
// Handler implementations
// ============================================================================

/// Handle get_meta_learning_status tool.
///
/// Returns current state of the Meta-UTL self-correction system.
pub fn handle_get_meta_learning_status(
    input: GetMetaLearningStatusInput,
    service: &MetaLearningService,
) -> McpResult<MetaLearningStatusOutput> {
    let current = service.current_lambdas();
    let base = service.base_lambdas();

    let current_lambdas = LambdaValues::from(current);
    let base_lambdas = LambdaValues::from(base);
    let lambda_deviation = LambdaValues {
        lambda_s: current_lambdas.lambda_s - base_lambdas.lambda_s,
        lambda_c: current_lambdas.lambda_c - base_lambdas.lambda_c,
    };

    let last_adjustment_at = service
        .last_adjustment()
        .map(|a| a.timestamp.to_rfc3339());

    let accuracy_history = if input.include_accuracy_history {
        let history = service.accuracy_history();
        if history.is_empty() {
            None
        } else {
            Some(history)
        }
    } else {
        None
    };

    let embedder_accuracy = if input.include_embedder_breakdown {
        service.embedder_accuracies().map(|arr| arr.to_vec())
    } else {
        None
    };

    Ok(MetaLearningStatusOutput {
        enabled: service.is_enabled(),
        current_accuracy: service.current_accuracy(),
        consecutive_low_count: service.consecutive_low_count(),
        current_lambdas,
        base_lambdas,
        lambda_deviation,
        escalation_status: format!("{:?}", service.escalation_status()),
        adjustment_count: service.adjustment_count(),
        recent_events_count: service.recent_events(24).len(),
        last_adjustment_at,
        accuracy_history,
        embedder_accuracy,
    })
}

/// Handle trigger_lambda_recalibration tool.
///
/// Manually triggers lambda weight recalibration.
pub fn handle_trigger_lambda_recalibration(
    input: TriggerRecalibrationInput,
    service: &mut MetaLearningService,
) -> McpResult<RecalibrationOutput> {
    // Validate domain if provided
    if let Some(ref domain_str) = input.domain {
        // Parse to validate, but we don't use it currently
        let _ = parse_domain(domain_str)?;
    }

    let result = service
        .trigger_recalibration(input.force_bayesian, input.dry_run)
        .map_err(|e| MetaLearningMcpError::Internal(e.to_string()))?;

    let adjustment = result.adjustment.map(|a| AdjustmentDetails {
        delta_s: a.delta_lambda_s,
        delta_c: a.delta_lambda_c,
        alpha: a.alpha,
        trigger_error: a.trigger_error,
    });

    Ok(RecalibrationOutput {
        success: result.success,
        adjustment,
        new_lambdas: LambdaValues::from(result.new_weights),
        previous_lambdas: LambdaValues::from(result.previous_weights),
        method: result.method.to_string(),
        bo_iterations: result.bo_iterations,
        expected_improvement: result.expected_improvement,
        dry_run: input.dry_run,
        error: result.error,
    })
}

/// Handle get_meta_learning_log tool.
///
/// Queries the meta-learning event log.
pub fn handle_get_meta_learning_log(
    input: GetMetaLearningLogInput,
    service: &MetaLearningService,
) -> McpResult<MetaLearningLogOutput> {
    let start_time = Instant::now();

    // Build query
    let mut query = EventLogQuery::new();

    // Parse and apply time range
    let start_dt = if let Some(ref start) = input.start_time {
        Some(parse_timestamp(start)?)
    } else {
        None
    };

    let end_dt = if let Some(ref end) = input.end_time {
        Some(parse_timestamp(end)?)
    } else {
        None
    };

    if let (Some(start), Some(end)) = (start_dt, end_dt) {
        query = query.time_range(start, end);
    } else if let Some(start) = start_dt {
        query = query.time_range(start, Utc::now());
    } else if let Some(end) = end_dt {
        query = query.time_range(DateTime::<Utc>::MIN_UTC, end);
    }

    // Apply event type filter
    if let Some(ref event_type) = input.event_type {
        let et = parse_event_type(event_type)?;
        query = query.event_type(et);
    }

    // Apply domain filter
    if let Some(ref domain) = input.domain {
        let d = parse_domain(domain)?;
        query = query.domain(d);
    }

    // Get total count first (without pagination)
    let total_query = query.clone();
    let all_events = service.query_events(&total_query);
    let total_count = all_events.len();

    // Apply pagination
    query = query.limit(input.limit).offset(input.offset);
    let events = service.query_events(&query);

    // Convert to output format
    let event_outputs: Vec<MetaLearningEventOutput> = events
        .iter()
        .map(|e| MetaLearningEventOutput {
            timestamp: e.created_at.to_rfc3339(),
            event_type: format!("{:?}", e.event_type),
            embedder_index: e.embedder_index,
            previous_value: e.previous_value,
            new_value: e.new_value,
            description: e.description.clone(),
            accuracy: e.accuracy,
            domain: Some(format!("{:?}", e.domain)),
        })
        .collect();

    let query_time_ms = start_time.elapsed().as_millis() as u32;
    let has_more = input.offset + events.len() < total_count;

    Ok(MetaLearningLogOutput {
        events: event_outputs,
        total_count,
        has_more,
        query_time_ms,
    })
}

// ============================================================================
// Parsing helpers
// ============================================================================

/// Parse ISO 8601 timestamp.
pub fn parse_timestamp(s: &str) -> McpResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            // Try parsing without timezone (assume UTC)
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
                .map(|ndt| ndt.and_utc())
        })
        .map_err(|_| MetaLearningMcpError::InvalidTimestamp {
            value: s.to_string(),
        })
}

/// Parse event type string to enum.
pub fn parse_event_type(s: &str) -> McpResult<MetaLearningEventType> {
    match s.to_lowercase().as_str() {
        "lambda_adjustment" => Ok(MetaLearningEventType::LambdaAdjustment),
        "bayesian_escalation" => Ok(MetaLearningEventType::BayesianEscalation),
        "accuracy_alert" => Ok(MetaLearningEventType::AccuracyAlert),
        "accuracy_recovery" => Ok(MetaLearningEventType::AccuracyRecovery),
        "weight_clamped" => Ok(MetaLearningEventType::WeightClamped),
        _ => Err(MetaLearningMcpError::InvalidEventType {
            value: s.to_string(),
        }),
    }
}

/// Parse domain string to enum.
pub fn parse_domain(s: &str) -> McpResult<Domain> {
    match s.to_lowercase().as_str() {
        "code" => Ok(Domain::Code),
        "medical" => Ok(Domain::Medical),
        "legal" => Ok(Domain::Legal),
        "creative" => Ok(Domain::Creative),
        "research" => Ok(Domain::Research),
        "general" => Ok(Domain::General),
        _ => Err(MetaLearningMcpError::InvalidDomain {
            value: s.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_status_basic() {
        let service = MetaLearningService::with_defaults();
        let input = GetMetaLearningStatusInput::default();

        let result = handle_get_meta_learning_status(input, &service);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.enabled);
        assert!(output.current_accuracy >= 0.0 && output.current_accuracy <= 1.0);
    }

    #[test]
    fn test_get_status_with_options() {
        let service = MetaLearningService::with_defaults();
        let input = GetMetaLearningStatusInput {
            include_accuracy_history: true,
            include_embedder_breakdown: true,
        };

        let result = handle_get_meta_learning_status(input, &service);

        assert!(result.is_ok());
        let output = result.unwrap();
        // May or may not have data depending on service state
        assert!(output.enabled);
    }

    #[test]
    fn test_recalibration_dry_run() {
        let mut service = MetaLearningService::with_defaults();
        let original_lambdas = LambdaValues::from(service.current_lambdas());

        let input = TriggerRecalibrationInput {
            force_bayesian: false,
            domain: None,
            dry_run: true,
        };

        let result = handle_trigger_lambda_recalibration(input, &mut service);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.dry_run);

        // Verify no state change
        let after_lambdas = LambdaValues::from(service.current_lambdas());
        assert!(
            (original_lambdas.lambda_s - after_lambdas.lambda_s).abs() < 0.001,
            "Dry run mutated lambda_s!"
        );
    }

    #[test]
    fn test_recalibration_gradient() {
        let mut service = MetaLearningService::with_defaults();

        let input = TriggerRecalibrationInput {
            force_bayesian: false,
            domain: None,
            dry_run: false,
        };

        let result = handle_trigger_lambda_recalibration(input, &mut service);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.dry_run);
        assert!(output.method == "gradient" || output.method == "none");
    }

    #[test]
    fn test_recalibration_bayesian() {
        let mut service = MetaLearningService::with_defaults();

        let input = TriggerRecalibrationInput {
            force_bayesian: true,
            domain: None,
            dry_run: false,
        };

        let result = handle_trigger_lambda_recalibration(input, &mut service);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.method, "bayesian");
        assert!(output.bo_iterations.is_some());
    }

    #[test]
    fn test_log_query_basic() {
        let service = MetaLearningService::with_defaults();

        let input = GetMetaLearningLogInput::default();

        let result = handle_get_meta_learning_log(input, &service);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.events.len() <= 100);
        assert!(output.query_time_ms < 1000);
    }

    #[test]
    fn test_log_query_with_filters() {
        let service = MetaLearningService::with_defaults();

        let input = GetMetaLearningLogInput {
            event_type: Some("lambda_adjustment".to_string()),
            domain: Some("code".to_string()),
            limit: 10,
            offset: 0,
            ..Default::default()
        };

        let result = handle_get_meta_learning_log(input, &service);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.events.len() <= 10);
    }

    #[test]
    fn test_parse_timestamp_valid() {
        let result = parse_timestamp("2024-01-15T10:30:00Z");
        assert!(result.is_ok());

        let result = parse_timestamp("2024-01-15T10:30:00+00:00");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        let result = parse_timestamp("not-a-timestamp");
        assert!(result.is_err());
        match result {
            Err(MetaLearningMcpError::InvalidTimestamp { value }) => {
                assert_eq!(value, "not-a-timestamp");
            }
            _ => panic!("Expected InvalidTimestamp error"),
        }
    }

    #[test]
    fn test_parse_event_type_valid() {
        assert!(parse_event_type("lambda_adjustment").is_ok());
        assert!(parse_event_type("bayesian_escalation").is_ok());
        assert!(parse_event_type("accuracy_alert").is_ok());
        assert!(parse_event_type("accuracy_recovery").is_ok());
        assert!(parse_event_type("weight_clamped").is_ok());
    }

    #[test]
    fn test_parse_event_type_invalid() {
        let result = parse_event_type("invalid_type");
        assert!(result.is_err());
        match result {
            Err(MetaLearningMcpError::InvalidEventType { value }) => {
                assert_eq!(value, "invalid_type");
            }
            _ => panic!("Expected InvalidEventType error"),
        }
    }

    #[test]
    fn test_parse_domain_valid() {
        assert_eq!(parse_domain("code").unwrap(), Domain::Code);
        assert_eq!(parse_domain("medical").unwrap(), Domain::Medical);
        assert_eq!(parse_domain("legal").unwrap(), Domain::Legal);
        assert_eq!(parse_domain("creative").unwrap(), Domain::Creative);
        assert_eq!(parse_domain("research").unwrap(), Domain::Research);
        assert_eq!(parse_domain("general").unwrap(), Domain::General);
    }

    #[test]
    fn test_parse_domain_invalid() {
        let result = parse_domain("invalid_domain");
        assert!(result.is_err());
        match result {
            Err(MetaLearningMcpError::InvalidDomain { value }) => {
                assert_eq!(value, "invalid_domain");
            }
            _ => panic!("Expected InvalidDomain error"),
        }
    }

    #[test]
    fn test_fsv_edge_case_dry_run_no_mutation() {
        let mut service = MetaLearningService::with_defaults();

        // BEFORE STATE
        let before_weights = LambdaValues::from(service.current_lambdas());
        let before_adjustment_count = service.adjustment_count();
        println!(
            "FSV BEFORE: weights={:?}, adjustment_count={}",
            before_weights, before_adjustment_count
        );

        // ACTION: Dry run recalibration
        let input = TriggerRecalibrationInput {
            force_bayesian: false,
            domain: None,
            dry_run: true,
        };
        let output = handle_trigger_lambda_recalibration(input, &mut service).unwrap();

        // AFTER STATE (FSV)
        let after_weights = LambdaValues::from(service.current_lambdas());
        let after_adjustment_count = service.adjustment_count();
        println!(
            "FSV AFTER: weights={:?}, adjustment_count={}",
            after_weights, after_adjustment_count
        );
        println!("FSV OUTPUT: dry_run={}, success={}", output.dry_run, output.success);

        // VERIFY: State unchanged
        assert!(output.dry_run, "FSV: Output should indicate dry_run=true");
        assert!(
            (before_weights.lambda_s - after_weights.lambda_s).abs() < 0.001,
            "FSV: Dry run mutated lambda_s!"
        );
        assert_eq!(
            before_adjustment_count, after_adjustment_count,
            "FSV: Dry run changed adjustment_count!"
        );
        println!("FSV: Dry run verified - no mutation");
    }

    #[test]
    fn test_fsv_edge_case_invalid_input() {
        let service = MetaLearningService::with_defaults();

        // BEFORE STATE
        println!("FSV BEFORE: Testing invalid inputs");

        // ACTION: Query with invalid event type
        let input = GetMetaLearningLogInput {
            event_type: Some("invalid_event_type".to_string()),
            ..Default::default()
        };
        let result = handle_get_meta_learning_log(input, &service);

        // AFTER STATE (FSV)
        println!("FSV AFTER: result.is_err()={}", result.is_err());

        // VERIFY: Should fail fast with clear error
        assert!(result.is_err(), "FSV: Invalid event_type should return error");
        let err = result.unwrap_err();
        let err_msg = format!("{:?}", err);
        assert!(
            err_msg.contains("InvalidEventType"),
            "FSV: Error message should mention invalid event_type: {}",
            err_msg
        );
        println!("FSV: Invalid input rejected with error");
    }

    #[test]
    fn test_fsv_verify_status_tool() {
        let service = MetaLearningService::with_defaults();

        // 1. Call MCP tool
        let input = GetMetaLearningStatusInput::default();
        let output = handle_get_meta_learning_status(input, &service).unwrap();

        // 2. INSPECT: Read actual state directly
        let tracker_accuracy = service.current_accuracy();
        let tracker_consecutive = service.consecutive_low_count();
        let tracker_enabled = service.is_enabled();

        // 3. VERIFY: Compare tool output with actual state
        assert!(
            (output.current_accuracy - tracker_accuracy).abs() < 0.001,
            "FSV: Status tool accuracy {} does not match tracker {}",
            output.current_accuracy,
            tracker_accuracy
        );
        assert_eq!(
            output.consecutive_low_count, tracker_consecutive,
            "FSV: Status tool consecutive_low_count mismatch"
        );
        assert_eq!(
            output.enabled, tracker_enabled,
            "FSV: Status tool enabled mismatch"
        );
        println!("FSV: Status tool verified against actual state");
    }
}
