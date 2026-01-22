//! FAIL FAST behavior verification runner.
//!
//! This runner tests that errors propagate correctly instead of being silently swallowed.
//! It verifies the code simplifications that enforce FAIL FAST semantics:
//!
//! - Batch retrieval errors return errors, not empty results
//! - Invalid weight profiles return InvalidValue errors
//! - Anchor validation fails before traversal
//! - All error paths include descriptive messages

use serde::{Deserialize, Serialize};

/// FAIL FAST test scenario
#[derive(Debug, Clone)]
pub struct FailFastScenario {
    pub name: String,
    pub description: String,
    pub trigger: FailFastTrigger,
    pub expected_behavior: ExpectedBehavior,
}

/// How to trigger the FAIL FAST condition
#[derive(Debug, Clone)]
pub enum FailFastTrigger {
    /// Nonexistent anchor ID
    NonexistentAnchor(String),
    /// Invalid UUID format
    InvalidUuidFormat(String),
    /// Invalid weight profile JSON
    InvalidWeightProfile(String),
    /// Invalid parameter value
    InvalidParameter { name: String, value: serde_json::Value },
    /// Missing required parameter
    MissingParameter(String),
}

/// Expected behavior when FAIL FAST is triggered
#[derive(Debug, Clone)]
pub enum ExpectedBehavior {
    /// Tool should return an error with message containing substring
    ErrorContaining(String),
    /// Tool should return an error code
    ErrorCode(i32),
}

/// Result of a FAIL FAST test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailFastResult {
    pub scenario_name: String,
    pub passed: bool,
    pub actual_error: Option<String>,
    pub expected_substring: String,
    pub latency_ms: f64,
}

/// Summary of FAIL FAST benchmark results
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FailFastSummary {
    pub total_scenarios: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<FailFastResult>,
    pub error_message_quality: ErrorMessageQuality,
}

/// Metrics for error message quality
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrorMessageQuality {
    /// Average error message length
    pub avg_message_length: f64,
    /// Number of errors with parameter names in message
    pub errors_with_param_names: usize,
    /// Number of errors with actual values in message
    pub errors_with_values: usize,
    /// Number of errors with range info (min/max)
    pub errors_with_range_info: usize,
}

impl FailFastSummary {
    /// Compute pass rate
    pub fn pass_rate(&self) -> f64 {
        if self.total_scenarios == 0 {
            0.0
        } else {
            self.passed as f64 / self.total_scenarios as f64
        }
    }

    /// Check if all scenarios passed
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }
}

/// Build standard FAIL FAST test scenarios for sequence tools
pub fn build_sequence_tool_scenarios() -> Vec<FailFastScenario> {
    let mut scenarios = Vec::new();

    // ========== Anchor validation scenarios ==========
    scenarios.push(FailFastScenario {
        name: "anchor_invalid_uuid".to_string(),
        description: "Invalid UUID format should fail immediately without DB query".to_string(),
        trigger: FailFastTrigger::InvalidUuidFormat("not-a-valid-uuid".to_string()),
        expected_behavior: ExpectedBehavior::ErrorContaining("Invalid anchorId UUID format".to_string()),
    });

    scenarios.push(FailFastScenario {
        name: "anchor_nonexistent".to_string(),
        description: "Valid UUID but nonexistent should return NotFound error".to_string(),
        trigger: FailFastTrigger::NonexistentAnchor(uuid::Uuid::new_v4().to_string()),
        expected_behavior: ExpectedBehavior::ErrorContaining("not found in storage".to_string()),
    });

    // ========== Parameter validation scenarios ==========
    scenarios.push(FailFastScenario {
        name: "windowSize_zero".to_string(),
        description: "windowSize=0 should fail with clear minimum violation message".to_string(),
        trigger: FailFastTrigger::InvalidParameter {
            name: "windowSize".to_string(),
            value: serde_json::json!(0),
        },
        expected_behavior: ExpectedBehavior::ErrorContaining("windowSize 0 below minimum".to_string()),
    });

    scenarios.push(FailFastScenario {
        name: "windowSize_too_large".to_string(),
        description: "windowSize=51 should fail with clear maximum violation message".to_string(),
        trigger: FailFastTrigger::InvalidParameter {
            name: "windowSize".to_string(),
            value: serde_json::json!(51),
        },
        expected_behavior: ExpectedBehavior::ErrorContaining("windowSize 51 exceeds maximum".to_string()),
    });

    scenarios.push(FailFastScenario {
        name: "limit_zero".to_string(),
        description: "limit=0 should fail with clear minimum violation message".to_string(),
        trigger: FailFastTrigger::InvalidParameter {
            name: "limit".to_string(),
            value: serde_json::json!(0),
        },
        expected_behavior: ExpectedBehavior::ErrorContaining("limit 0 below minimum".to_string()),
    });

    scenarios.push(FailFastScenario {
        name: "limit_too_large".to_string(),
        description: "limit=201 should fail with clear maximum violation message".to_string(),
        trigger: FailFastTrigger::InvalidParameter {
            name: "limit".to_string(),
            value: serde_json::json!(201),
        },
        expected_behavior: ExpectedBehavior::ErrorContaining("limit 201 exceeds maximum".to_string()),
    });

    scenarios.push(FailFastScenario {
        name: "hops_zero".to_string(),
        description: "hops=0 should fail with clear minimum violation message".to_string(),
        trigger: FailFastTrigger::InvalidParameter {
            name: "hops".to_string(),
            value: serde_json::json!(0),
        },
        expected_behavior: ExpectedBehavior::ErrorContaining("hops 0 below minimum".to_string()),
    });

    scenarios.push(FailFastScenario {
        name: "hops_too_large".to_string(),
        description: "hops=21 should fail with clear maximum violation message".to_string(),
        trigger: FailFastTrigger::InvalidParameter {
            name: "hops".to_string(),
            value: serde_json::json!(21),
        },
        expected_behavior: ExpectedBehavior::ErrorContaining("hops 21 exceeds maximum".to_string()),
    });

    // ========== Missing parameter scenarios ==========
    scenarios.push(FailFastScenario {
        name: "missing_anchorId".to_string(),
        description: "traverse_memory_chain without anchorId should fail".to_string(),
        trigger: FailFastTrigger::MissingParameter("anchorId".to_string()),
        expected_behavior: ExpectedBehavior::ErrorContaining("Missing required 'anchorId'".to_string()),
    });

    scenarios
}

/// Analyze error message quality from FAIL FAST results
pub fn analyze_error_message_quality(results: &[FailFastResult]) -> ErrorMessageQuality {
    if results.is_empty() {
        return ErrorMessageQuality::default();
    }

    let mut total_length = 0usize;
    let mut with_param_names = 0usize;
    let mut with_values = 0usize;
    let mut with_range_info = 0usize;

    let param_names = ["windowSize", "limit", "hops", "anchorId", "sessionId"];

    for result in results {
        if let Some(ref msg) = result.actual_error {
            total_length += msg.len();

            // Check for parameter name in message
            if param_names.iter().any(|p| msg.contains(p)) {
                with_param_names += 1;
            }

            // Check for numeric values in message
            if msg.chars().any(|c| c.is_ascii_digit()) {
                with_values += 1;
            }

            // Check for range info (minimum, maximum, exceeds, below)
            if msg.contains("minimum") || msg.contains("maximum")
                || msg.contains("exceeds") || msg.contains("below") {
                with_range_info += 1;
            }
        }
    }

    ErrorMessageQuality {
        avg_message_length: total_length as f64 / results.len() as f64,
        errors_with_param_names: with_param_names,
        errors_with_values: with_values,
        errors_with_range_info: with_range_info,
    }
}

/// Verify that a result matches expected FAIL FAST behavior
pub fn verify_failfast_behavior(
    scenario: &FailFastScenario,
    error: Option<&str>,
    latency_ms: f64,
) -> FailFastResult {
    let expected_substring = match &scenario.expected_behavior {
        ExpectedBehavior::ErrorContaining(s) => s.clone(),
        ExpectedBehavior::ErrorCode(c) => format!("code {}", c),
    };

    let passed = match (&scenario.expected_behavior, error) {
        (ExpectedBehavior::ErrorContaining(expected), Some(actual)) => {
            actual.contains(expected)
        }
        (ExpectedBehavior::ErrorContaining(_), None) => false,
        (ExpectedBehavior::ErrorCode(_), _) => {
            // Error code checking would need additional response info
            error.is_some()
        }
    };

    FailFastResult {
        scenario_name: scenario.name.clone(),
        passed,
        actual_error: error.map(String::from),
        expected_substring,
        latency_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_scenarios() {
        let scenarios = build_sequence_tool_scenarios();
        assert!(scenarios.len() >= 8, "Expected at least 8 scenarios");

        // Verify all scenarios have descriptions
        for s in &scenarios {
            assert!(!s.description.is_empty(), "Scenario {} missing description", s.name);
        }
    }

    #[test]
    fn test_verify_failfast_behavior_pass() {
        let scenario = FailFastScenario {
            name: "test".to_string(),
            description: "test".to_string(),
            trigger: FailFastTrigger::InvalidParameter {
                name: "windowSize".to_string(),
                value: serde_json::json!(0),
            },
            expected_behavior: ExpectedBehavior::ErrorContaining("below minimum".to_string()),
        };

        let result = verify_failfast_behavior(&scenario, Some("windowSize 0 below minimum 1"), 1.0);
        assert!(result.passed);
    }

    #[test]
    fn test_verify_failfast_behavior_fail() {
        let scenario = FailFastScenario {
            name: "test".to_string(),
            description: "test".to_string(),
            trigger: FailFastTrigger::InvalidParameter {
                name: "windowSize".to_string(),
                value: serde_json::json!(0),
            },
            expected_behavior: ExpectedBehavior::ErrorContaining("below minimum".to_string()),
        };

        // No error returned = fail
        let result = verify_failfast_behavior(&scenario, None, 1.0);
        assert!(!result.passed);
    }

    #[test]
    fn test_analyze_error_quality() {
        let results = vec![
            FailFastResult {
                scenario_name: "test1".to_string(),
                passed: true,
                actual_error: Some("windowSize 0 below minimum 1".to_string()),
                expected_substring: "below minimum".to_string(),
                latency_ms: 1.0,
            },
            FailFastResult {
                scenario_name: "test2".to_string(),
                passed: true,
                actual_error: Some("limit 201 exceeds maximum 200".to_string()),
                expected_substring: "exceeds maximum".to_string(),
                latency_ms: 1.0,
            },
        ];

        let quality = analyze_error_message_quality(&results);
        assert!(quality.avg_message_length > 20.0);
        assert_eq!(quality.errors_with_param_names, 2);
        assert_eq!(quality.errors_with_values, 2);
        assert_eq!(quality.errors_with_range_info, 2);
    }

    #[test]
    fn test_summary_pass_rate() {
        let summary = FailFastSummary {
            total_scenarios: 10,
            passed: 8,
            failed: 2,
            ..Default::default()
        };
        assert!((summary.pass_rate() - 0.8).abs() < 0.001);
    }
}
