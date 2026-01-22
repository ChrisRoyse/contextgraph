//! Validation metrics for measuring input validation performance and correctness.
//!
//! This module provides metrics for benchmarking the code simplification changes:
//! - Validation overhead measurement
//! - FAIL FAST behavior tracking
//! - Error message quality assessment

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Validation metrics for a benchmark run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationMetrics {
    /// Total number of validation tests run
    pub total_tests: usize,

    /// Number of tests that passed
    pub passed: usize,

    /// Number of tests that failed
    pub failed: usize,

    /// Pass rate as a percentage
    pub pass_rate: f64,

    /// Average latency for valid inputs (ms)
    pub avg_valid_latency_ms: f64,

    /// Average latency for invalid inputs (ms)
    pub avg_invalid_latency_ms: f64,

    /// P50 latency for validation path (ms)
    pub validation_p50_ms: f64,

    /// P99 latency for validation path (ms)
    pub validation_p99_ms: f64,

    /// P50 baseline latency for valid inputs (ms)
    pub baseline_p50_ms: f64,

    /// P99 baseline latency for valid inputs (ms)
    pub baseline_p99_ms: f64,

    /// Validation overhead percentage
    pub overhead_percent: f64,
}

impl ValidationMetrics {
    /// Compute validation metrics from raw measurements.
    pub fn from_measurements(
        total_tests: usize,
        passed: usize,
        valid_latencies: &[f64],
        invalid_latencies: &[f64],
    ) -> Self {
        let failed = total_tests.saturating_sub(passed);
        let pass_rate = if total_tests > 0 {
            (passed as f64 / total_tests as f64) * 100.0
        } else {
            0.0
        };

        let avg_valid_latency_ms = mean(valid_latencies);
        let avg_invalid_latency_ms = mean(invalid_latencies);

        let (baseline_p50_ms, baseline_p99_ms) = percentiles(valid_latencies);
        let (validation_p50_ms, validation_p99_ms) = percentiles(invalid_latencies);

        let overhead_percent = if baseline_p50_ms > 0.0 {
            ((validation_p50_ms - baseline_p50_ms) / baseline_p50_ms) * 100.0
        } else {
            0.0
        };

        Self {
            total_tests,
            passed,
            failed,
            pass_rate,
            avg_valid_latency_ms,
            avg_invalid_latency_ms,
            validation_p50_ms,
            validation_p99_ms,
            baseline_p50_ms,
            baseline_p99_ms,
            overhead_percent,
        }
    }

    /// Check if validation overhead is acceptable (< 1ms typically)
    pub fn overhead_acceptable(&self, max_overhead_ms: f64) -> bool {
        self.validation_p50_ms - self.baseline_p50_ms < max_overhead_ms
    }

    /// Check if all tests passed
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }

    /// Get a quality score (0-1) for validation implementation
    pub fn quality_score(&self) -> f64 {
        // 70% for correctness (pass rate), 30% for performance (low overhead)
        let correctness_score = self.pass_rate / 100.0;
        let performance_score = if self.overhead_percent <= 0.0 {
            1.0 // Negative overhead = faster validation
        } else if self.overhead_percent <= 10.0 {
            1.0 - (self.overhead_percent / 100.0)
        } else if self.overhead_percent <= 50.0 {
            0.5 - (self.overhead_percent - 10.0) / 100.0
        } else {
            0.0
        };

        0.7 * correctness_score + 0.3 * performance_score
    }
}

/// Per-tool validation results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolValidationMetrics {
    /// Tool name
    pub tool_name: String,

    /// Tests passed for this tool
    pub passed: usize,

    /// Tests failed for this tool
    pub failed: usize,

    /// Individual test results
    pub test_results: Vec<TestCaseResult>,

    /// Latencies for tests on this tool
    pub latencies_ms: Vec<f64>,
}

impl ToolValidationMetrics {
    /// Create new metrics for a tool
    pub fn new(tool_name: &str) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            ..Default::default()
        }
    }

    /// Add a test result
    pub fn add_result(&mut self, result: TestCaseResult) {
        if result.passed {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
        self.latencies_ms.push(result.latency_ms);
        self.test_results.push(result);
    }

    /// Get pass rate for this tool
    pub fn pass_rate(&self) -> f64 {
        let total = self.passed + self.failed;
        if total > 0 {
            (self.passed as f64 / total as f64) * 100.0
        } else {
            0.0
        }
    }
}

/// Result of a single test case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseResult {
    /// Test case name
    pub name: String,

    /// Whether the test passed
    pub passed: bool,

    /// Latency in milliseconds
    pub latency_ms: f64,

    /// Error message if any
    pub error_message: Option<String>,

    /// Expected error substring if this was an error test
    pub expected_error: Option<String>,

    /// Whether this tested valid or invalid input
    pub input_valid: bool,
}

impl TestCaseResult {
    /// Create a passing result for valid input
    pub fn valid_pass(name: &str, latency_ms: f64) -> Self {
        Self {
            name: name.to_string(),
            passed: true,
            latency_ms,
            error_message: None,
            expected_error: None,
            input_valid: true,
        }
    }

    /// Create a passing result for invalid input (correct error returned)
    pub fn invalid_pass(name: &str, latency_ms: f64, expected: &str, actual: &str) -> Self {
        Self {
            name: name.to_string(),
            passed: true,
            latency_ms,
            error_message: Some(actual.to_string()),
            expected_error: Some(expected.to_string()),
            input_valid: false,
        }
    }

    /// Create a failing result
    pub fn fail(name: &str, latency_ms: f64, reason: &str, input_valid: bool) -> Self {
        Self {
            name: name.to_string(),
            passed: false,
            latency_ms,
            error_message: Some(reason.to_string()),
            expected_error: None,
            input_valid,
        }
    }
}

/// Boundary test configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryTestConfig {
    /// Parameter name being tested
    pub parameter: String,

    /// Minimum valid value
    pub min: u64,

    /// Maximum valid value
    pub max: u64,

    /// Default value when not specified
    pub default: u64,

    /// Test values to try
    pub test_values: Vec<BoundaryTestValue>,
}

/// A single boundary test value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryTestValue {
    /// The value to test
    pub value: Option<u64>,

    /// Whether this should be valid
    pub expected_valid: bool,

    /// Description of this test
    pub description: String,
}

impl BoundaryTestConfig {
    /// Create standard boundary tests for a parameter
    pub fn standard(parameter: &str, min: u64, max: u64, default: u64) -> Self {
        Self {
            parameter: parameter.to_string(),
            min,
            max,
            default,
            test_values: vec![
                BoundaryTestValue {
                    value: Some(0),
                    expected_valid: min == 0,
                    description: "zero value".to_string(),
                },
                BoundaryTestValue {
                    value: Some(min),
                    expected_valid: true,
                    description: "at minimum".to_string(),
                },
                BoundaryTestValue {
                    value: Some(min.saturating_sub(1)),
                    expected_valid: min == 0,
                    description: "below minimum".to_string(),
                },
                BoundaryTestValue {
                    value: Some(max),
                    expected_valid: true,
                    description: "at maximum".to_string(),
                },
                BoundaryTestValue {
                    value: Some(max + 1),
                    expected_valid: false,
                    description: "above maximum".to_string(),
                },
                BoundaryTestValue {
                    value: None,
                    expected_valid: true,
                    description: "null/default".to_string(),
                },
            ],
        }
    }
}

/// Pre-built boundary test configs for sequence tools.
pub fn sequence_tool_boundary_configs() -> HashMap<String, Vec<BoundaryTestConfig>> {
    let mut configs = HashMap::new();

    configs.insert(
        "get_conversation_context".to_string(),
        vec![BoundaryTestConfig::standard("windowSize", 1, 50, 10)],
    );

    configs.insert(
        "get_session_timeline".to_string(),
        vec![BoundaryTestConfig::standard("limit", 1, 200, 50)],
    );

    configs.insert(
        "traverse_memory_chain".to_string(),
        vec![BoundaryTestConfig::standard("hops", 1, 20, 5)],
    );

    configs
}

// Helper functions

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn percentiles(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let p50_idx = ((sorted.len() - 1) as f64 * 0.50).round() as usize;
    let p99_idx = ((sorted.len() - 1) as f64 * 0.99).round() as usize;

    (
        sorted[p50_idx.min(sorted.len() - 1)],
        sorted[p99_idx.min(sorted.len() - 1)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_metrics_from_measurements() {
        let valid_latencies = vec![10.0, 12.0, 11.0, 13.0, 10.5];
        let invalid_latencies = vec![1.0, 1.2, 0.9, 1.1, 1.0];

        let metrics = ValidationMetrics::from_measurements(10, 8, &valid_latencies, &invalid_latencies);

        assert_eq!(metrics.total_tests, 10);
        assert_eq!(metrics.passed, 8);
        assert_eq!(metrics.failed, 2);
        assert!((metrics.pass_rate - 80.0).abs() < 0.001);
    }

    #[test]
    fn test_validation_metrics_overhead() {
        let metrics = ValidationMetrics {
            baseline_p50_ms: 10.0,
            validation_p50_ms: 11.0,
            overhead_percent: 10.0,
            ..Default::default()
        };

        // 1ms max overhead = 11-10 = 1 = acceptable
        assert!(metrics.overhead_acceptable(1.5));
        assert!(!metrics.overhead_acceptable(0.5));
    }

    #[test]
    fn test_tool_validation_metrics() {
        let mut tool_metrics = ToolValidationMetrics::new("test_tool");

        tool_metrics.add_result(TestCaseResult::valid_pass("test1", 10.0));
        tool_metrics.add_result(TestCaseResult::fail("test2", 5.0, "failed", true));

        assert_eq!(tool_metrics.passed, 1);
        assert_eq!(tool_metrics.failed, 1);
        assert!((tool_metrics.pass_rate() - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_boundary_test_config() {
        let config = BoundaryTestConfig::standard("windowSize", 1, 50, 10);

        assert_eq!(config.min, 1);
        assert_eq!(config.max, 50);
        assert_eq!(config.default, 10);
        assert!(config.test_values.len() >= 5);

        // Check specific test cases
        let below_min = config.test_values.iter().find(|t| t.description == "below minimum");
        assert!(below_min.is_some());
        assert!(!below_min.unwrap().expected_valid);
    }

    #[test]
    fn test_sequence_tool_configs() {
        let configs = sequence_tool_boundary_configs();

        assert!(configs.contains_key("get_conversation_context"));
        assert!(configs.contains_key("get_session_timeline"));
        assert!(configs.contains_key("traverse_memory_chain"));
    }

    #[test]
    fn test_quality_score() {
        // Perfect: 100% pass rate, no overhead
        let perfect = ValidationMetrics {
            pass_rate: 100.0,
            overhead_percent: 0.0,
            ..Default::default()
        };
        assert!((perfect.quality_score() - 1.0).abs() < 0.001);

        // 80% pass rate, 10% overhead
        let good = ValidationMetrics {
            pass_rate: 80.0,
            overhead_percent: 10.0,
            ..Default::default()
        };
        let score = good.quality_score();
        assert!(score > 0.5 && score < 0.9);
    }
}
