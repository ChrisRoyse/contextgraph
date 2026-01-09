//! Comparison validation errors for teleological types.
//!
//! This module defines structured errors for validation failures in comparison types
//! (ComponentWeights, SynergyMatrix, SimilarityBreakdown). These errors are designed
//! to be actionable, providing exactly what failed and how to fix it.
//!
//! # Error Philosophy (per Constitution)
//!
//! - Errors MUST be specific and actionable
//! - No workarounds or fallbacks - fail fast with clear diagnostics
//! - Every error variant should tell the caller exactly what went wrong

use std::fmt;

/// Validation errors for comparison types.
///
/// Each variant captures the exact failure condition with all relevant data
/// needed for debugging and correction.
#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonValidationError {
    /// Weights do not sum to 1.0 (±tolerance)
    WeightsNotNormalized {
        /// Actual sum of all weights
        actual_sum: f32,
        /// Expected sum (1.0)
        expected_sum: f32,
        /// Tolerance used for comparison
        tolerance: f32,
        /// Individual weight values for debugging
        weights: WeightValues,
    },

    /// Individual weight is out of valid range [0.0, 1.0]
    WeightOutOfRange {
        /// Name of the weight field
        field_name: &'static str,
        /// The invalid value
        value: f32,
        /// Valid minimum (0.0)
        min: f32,
        /// Valid maximum (1.0)
        max: f32,
    },

    /// Synergy matrix is not symmetric within tolerance
    MatrixNotSymmetric {
        /// Row index where asymmetry was detected
        row: usize,
        /// Column index where asymmetry was detected
        col: usize,
        /// Value at [row][col]
        value_ij: f32,
        /// Value at [col][row]
        value_ji: f32,
        /// Maximum allowed difference
        tolerance: f32,
    },

    /// Diagonal value is not 1.0
    DiagonalNotUnity {
        /// Index on diagonal
        index: usize,
        /// Actual value
        actual: f32,
        /// Expected value (1.0)
        expected: f32,
        /// Tolerance used
        tolerance: f32,
    },

    /// Synergy value is out of valid range [0.0, 1.0]
    SynergyOutOfRange {
        /// Row index
        row: usize,
        /// Column index
        col: usize,
        /// The invalid value
        value: f32,
        /// Valid minimum (0.0)
        min: f32,
        /// Valid maximum (1.0)
        max: f32,
    },

    /// Similarity score is out of valid range [0.0, 1.0]
    SimilarityOutOfRange {
        /// Name of the similarity component
        component: &'static str,
        /// The invalid value
        value: f32,
    },

    /// Breakdown components don't reconcile with overall score
    BreakdownInconsistent {
        /// Computed overall from breakdown
        computed_overall: f32,
        /// Stored overall value
        stored_overall: f32,
        /// Maximum allowed difference
        tolerance: f32,
    },
}

/// Individual weight values for debugging WeightsNotNormalized errors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightValues {
    pub purpose_vector: f32,
    pub cross_correlations: f32,
    pub group_alignments: f32,
    pub confidence: f32,
}

impl fmt::Display for ComparisonValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComparisonValidationError::WeightsNotNormalized {
                actual_sum,
                expected_sum,
                tolerance,
                weights,
            } => {
                write!(
                    f,
                    "ComponentWeights not normalized: sum is {} (expected {} ±{}). \
                    Values: purpose_vector={}, cross_correlations={}, \
                    group_alignments={}, confidence={}. \
                    Fix: call normalize() or adjust weights to sum to 1.0",
                    actual_sum,
                    expected_sum,
                    tolerance,
                    weights.purpose_vector,
                    weights.cross_correlations,
                    weights.group_alignments,
                    weights.confidence
                )
            }

            ComparisonValidationError::WeightOutOfRange {
                field_name,
                value,
                min,
                max,
            } => {
                write!(
                    f,
                    "Weight '{}' is {} (must be in [{}, {}]). \
                    Fix: set {} to a value within the valid range",
                    field_name, value, min, max, field_name
                )
            }

            ComparisonValidationError::MatrixNotSymmetric {
                row,
                col,
                value_ij,
                value_ji,
                tolerance,
            } => {
                write!(
                    f,
                    "SynergyMatrix not symmetric: values[{}][{}]={} != values[{}][{}]={} \
                    (tolerance: {}). Fix: use set_synergy() which maintains symmetry automatically",
                    row, col, value_ij, col, row, value_ji, tolerance
                )
            }

            ComparisonValidationError::DiagonalNotUnity {
                index,
                actual,
                expected,
                tolerance,
            } => {
                write!(
                    f,
                    "SynergyMatrix diagonal[{}][{}]={} (expected {} ±{}). \
                    Fix: diagonal values must always be 1.0 (self-synergy)",
                    index, index, actual, expected, tolerance
                )
            }

            ComparisonValidationError::SynergyOutOfRange {
                row,
                col,
                value,
                min,
                max,
            } => {
                write!(
                    f,
                    "Synergy value at [{}, {}] is {} (must be in [{}, {}]). \
                    Fix: set synergy values only within the valid range",
                    row, col, value, min, max
                )
            }

            ComparisonValidationError::SimilarityOutOfRange { component, value } => {
                write!(
                    f,
                    "Similarity component '{}' is {} (must be in [0.0, 1.0]). \
                    Fix: ensure similarity calculations produce normalized values",
                    component, value
                )
            }

            ComparisonValidationError::BreakdownInconsistent {
                computed_overall,
                stored_overall,
                tolerance,
            } => {
                write!(
                    f,
                    "SimilarityBreakdown inconsistent: computed overall {} != stored {} \
                    (tolerance: {}). Fix: recompute breakdown with consistent weights",
                    computed_overall, stored_overall, tolerance
                )
            }
        }
    }
}

impl std::error::Error for ComparisonValidationError {}

/// Result type for comparison validation operations.
pub type ComparisonValidationResult<T> = Result<T, ComparisonValidationError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weights_not_normalized_error_display() {
        let err = ComparisonValidationError::WeightsNotNormalized {
            actual_sum: 1.5,
            expected_sum: 1.0,
            tolerance: 0.001,
            weights: WeightValues {
                purpose_vector: 0.5,
                cross_correlations: 0.5,
                group_alignments: 0.3,
                confidence: 0.2,
            },
        };

        let msg = err.to_string();
        assert!(msg.contains("1.5"));
        assert!(msg.contains("normalize()"));
        println!("[PASS] WeightsNotNormalized error: {}", msg);
    }

    #[test]
    fn test_weight_out_of_range_error_display() {
        let err = ComparisonValidationError::WeightOutOfRange {
            field_name: "purpose_vector",
            value: -0.5,
            min: 0.0,
            max: 1.0,
        };

        let msg = err.to_string();
        assert!(msg.contains("purpose_vector"));
        assert!(msg.contains("-0.5"));
        println!("[PASS] WeightOutOfRange error: {}", msg);
    }

    #[test]
    fn test_matrix_not_symmetric_error_display() {
        let err = ComparisonValidationError::MatrixNotSymmetric {
            row: 2,
            col: 5,
            value_ij: 0.8,
            value_ji: 0.3,
            tolerance: 0.001,
        };

        let msg = err.to_string();
        assert!(msg.contains("[2][5]"));
        assert!(msg.contains("set_synergy()"));
        println!("[PASS] MatrixNotSymmetric error: {}", msg);
    }

    #[test]
    fn test_diagonal_not_unity_error_display() {
        let err = ComparisonValidationError::DiagonalNotUnity {
            index: 3,
            actual: 0.9,
            expected: 1.0,
            tolerance: 0.0001,
        };

        let msg = err.to_string();
        assert!(msg.contains("[3][3]"));
        assert!(msg.contains("0.9"));
        println!("[PASS] DiagonalNotUnity error: {}", msg);
    }

    #[test]
    fn test_synergy_out_of_range_error_display() {
        let err = ComparisonValidationError::SynergyOutOfRange {
            row: 1,
            col: 4,
            value: 1.5,
            min: 0.0,
            max: 1.0,
        };

        let msg = err.to_string();
        assert!(msg.contains("[1, 4]"));
        assert!(msg.contains("1.5"));
        println!("[PASS] SynergyOutOfRange error: {}", msg);
    }

    #[test]
    fn test_similarity_out_of_range_error_display() {
        let err = ComparisonValidationError::SimilarityOutOfRange {
            component: "purpose_vector",
            value: 1.2,
        };

        let msg = err.to_string();
        assert!(msg.contains("purpose_vector"));
        assert!(msg.contains("1.2"));
        println!("[PASS] SimilarityOutOfRange error: {}", msg);
    }

    #[test]
    fn test_breakdown_inconsistent_error_display() {
        let err = ComparisonValidationError::BreakdownInconsistent {
            computed_overall: 0.75,
            stored_overall: 0.85,
            tolerance: 0.01,
        };

        let msg = err.to_string();
        assert!(msg.contains("0.75"));
        assert!(msg.contains("0.85"));
        println!("[PASS] BreakdownInconsistent error: {}", msg);
    }

    #[test]
    fn test_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ComparisonValidationError>();
        println!("[PASS] ComparisonValidationError is Send + Sync");
    }

    #[test]
    fn test_error_impl_std_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(ComparisonValidationError::WeightOutOfRange {
                field_name: "test",
                value: 2.0,
                min: 0.0,
                max: 1.0,
            });
        assert!(err.to_string().contains("test"));
        println!("[PASS] ComparisonValidationError implements std::error::Error");
    }
}
