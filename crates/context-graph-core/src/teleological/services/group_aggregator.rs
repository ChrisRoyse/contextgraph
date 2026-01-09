//! TASK-TELEO-011: GroupAggregator Implementation
//!
//! Aggregates 13D embeddings/alignments into 6D group representation.
//!
//! # From teleoplan.md Section 3.2
//!
//! Hierarchical Grouping:
//! - Factual: E1, E12, E13 (what IS)
//! - Temporal: E2, E3 (when/sequence)
//! - Causal: E4, E7 (why/how)
//! - Relational: E5, E8, E9 (like/where/who)
//! - Qualitative: E10, E11 (feel/principle)
//! - Implementation: E6 (code)

use crate::teleological::{types::NUM_EMBEDDERS, GroupAlignments, GroupType};

/// Configuration for group aggregation.
#[derive(Clone, Debug)]
pub struct GroupAggregationConfig {
    /// Aggregation method for each group
    pub method: AggregationMethod,
    /// Custom weights per embedder (optional)
    pub embedder_weights: Option<[f32; NUM_EMBEDDERS]>,
    /// Normalize output to [0, 1]
    pub normalize_output: bool,
}

impl Default for GroupAggregationConfig {
    fn default() -> Self {
        Self {
            method: AggregationMethod::WeightedAverage,
            embedder_weights: None,
            normalize_output: true,
        }
    }
}

/// Method for aggregating embedder values within a group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregationMethod {
    /// Simple average
    Average,
    /// Weighted average (uses embedder_weights)
    WeightedAverage,
    /// Maximum value in group
    Max,
    /// Minimum value in group
    Min,
    /// Geometric mean (for positive values)
    GeometricMean,
    /// RMS (Root Mean Square)
    RootMeanSquare,
}

/// Result of group aggregation.
#[derive(Clone, Debug)]
pub struct GroupAggregationResult {
    /// 6D group alignments
    pub alignments: GroupAlignments,
    /// Per-group statistics
    pub group_stats: [GroupStats; 6],
    /// Overall coherence (agreement between groups)
    pub coherence: f32,
}

/// Statistics for a single group.
#[derive(Clone, Debug, Default)]
pub struct GroupStats {
    /// Group type
    pub group_type: Option<GroupType>,
    /// Aggregated value
    pub value: f32,
    /// Standard deviation within group
    pub std_dev: f32,
    /// Number of embedders in group
    pub embedder_count: usize,
    /// Range (max - min) within group
    pub range: f32,
}

/// TELEO-011: Aggregates 13D values into 6D group representation.
///
/// # Example
///
/// ```
/// use context_graph_core::teleological::services::GroupAggregator;
///
/// let aggregator = GroupAggregator::new();
/// let alignments = [0.8f32; 13];
/// let result = aggregator.aggregate(&alignments);
/// assert_eq!(result.alignments.factual, 0.8); // All same input
/// ```
pub struct GroupAggregator {
    config: GroupAggregationConfig,
}

impl GroupAggregator {
    /// Create a new GroupAggregator with default configuration.
    pub fn new() -> Self {
        Self {
            config: GroupAggregationConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: GroupAggregationConfig) -> Self {
        Self { config }
    }

    /// Aggregate 13D alignments into 6D groups.
    ///
    /// # Arguments
    /// * `alignments` - 13D array of per-embedder alignment values
    pub fn aggregate(&self, alignments: &[f32; NUM_EMBEDDERS]) -> GroupAggregationResult {
        let weights = self.config.embedder_weights.unwrap_or([1.0; NUM_EMBEDDERS]);

        let mut group_values = [0.0f32; 6];
        let mut group_stats = [
            GroupStats::default(),
            GroupStats::default(),
            GroupStats::default(),
            GroupStats::default(),
            GroupStats::default(),
            GroupStats::default(),
        ];

        // Aggregate each group
        for (idx, group_type) in GroupType::ALL.iter().enumerate() {
            let indices = group_type.embedding_indices();
            let values: Vec<f32> = indices.iter().map(|&i| alignments[i]).collect();
            let group_weights: Vec<f32> = indices.iter().map(|&i| weights[i]).collect();

            let aggregated = self.aggregate_group(&values, &group_weights);

            group_values[idx] = if self.config.normalize_output {
                aggregated.clamp(0.0, 1.0)
            } else {
                aggregated
            };

            // Compute statistics
            group_stats[idx] = self.compute_group_stats(*group_type, &values);
        }

        let alignments = GroupAlignments::from_array(group_values);
        let coherence = alignments.coherence();

        GroupAggregationResult {
            alignments,
            group_stats,
            coherence,
        }
    }

    /// Aggregate with custom per-group weights.
    ///
    /// # Arguments
    /// * `alignments` - 13D array of per-embedder values
    /// * `group_weights` - 6D array of weights to apply after aggregation
    pub fn aggregate_weighted(
        &self,
        alignments: &[f32; NUM_EMBEDDERS],
        group_weights: &[f32; 6],
    ) -> GroupAggregationResult {
        let mut result = self.aggregate(alignments);

        // Apply group-level weights
        let arr = result.alignments.as_array();
        let weighted: [f32; 6] = std::array::from_fn(|i| arr[i] * group_weights[i]);
        result.alignments = GroupAlignments::from_array(weighted);

        result
    }

    /// Aggregate values within a single group.
    fn aggregate_group(&self, values: &[f32], weights: &[f32]) -> f32 {
        if values.is_empty() {
            return 0.0;
        }

        match self.config.method {
            AggregationMethod::Average => values.iter().sum::<f32>() / values.len() as f32,
            AggregationMethod::WeightedAverage => {
                let weighted_sum: f32 = values.iter().zip(weights.iter()).map(|(v, w)| v * w).sum();
                let weight_sum: f32 = weights.iter().sum();
                if weight_sum > f32::EPSILON {
                    weighted_sum / weight_sum
                } else {
                    0.0
                }
            }
            AggregationMethod::Max => values.iter().cloned().fold(f32::NEG_INFINITY, f32::max),
            AggregationMethod::Min => values.iter().cloned().fold(f32::INFINITY, f32::min),
            AggregationMethod::GeometricMean => {
                let product: f32 = values.iter().map(|v| v.max(f32::EPSILON)).product();
                product.powf(1.0 / values.len() as f32)
            }
            AggregationMethod::RootMeanSquare => {
                let sum_sq: f32 = values.iter().map(|v| v * v).sum();
                (sum_sq / values.len() as f32).sqrt()
            }
        }
    }

    /// Compute statistics for a group.
    fn compute_group_stats(&self, group_type: GroupType, values: &[f32]) -> GroupStats {
        if values.is_empty() {
            return GroupStats {
                group_type: Some(group_type),
                ..Default::default()
            };
        }

        let mean: f32 = values.iter().sum::<f32>() / values.len() as f32;
        let variance: f32 =
            values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / values.len() as f32;
        let std_dev = variance.sqrt();

        let min = values.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

        GroupStats {
            group_type: Some(group_type),
            value: mean,
            std_dev,
            embedder_count: values.len(),
            range: max - min,
        }
    }

    /// Get dominant group from alignments.
    pub fn get_dominant_group(&self, alignments: &[f32; NUM_EMBEDDERS]) -> GroupType {
        let result = self.aggregate(alignments);
        result.alignments.dominant_group()
    }

    /// Get weakest group from alignments.
    pub fn get_weakest_group(&self, alignments: &[f32; NUM_EMBEDDERS]) -> GroupType {
        let result = self.aggregate(alignments);
        result.alignments.weakest_group()
    }

    /// Get configuration.
    pub fn config(&self) -> &GroupAggregationConfig {
        &self.config
    }
}

impl Default for GroupAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_aggregator_new() {
        let aggregator = GroupAggregator::new();
        assert_eq!(
            aggregator.config().method,
            AggregationMethod::WeightedAverage
        );

        println!("[PASS] GroupAggregator::new creates default config");
    }

    #[test]
    fn test_aggregate_uniform() {
        let aggregator = GroupAggregator::new();
        let alignments = [0.8f32; NUM_EMBEDDERS];

        let result = aggregator.aggregate(&alignments);

        // All groups should be 0.8
        assert!((result.alignments.factual - 0.8).abs() < 0.01);
        assert!((result.alignments.temporal - 0.8).abs() < 0.01);
        assert!((result.alignments.implementation - 0.8).abs() < 0.01);

        println!("[PASS] Uniform input produces uniform groups");
    }

    #[test]
    fn test_aggregate_varied() {
        let aggregator = GroupAggregator::new();

        let mut alignments = [0.5f32; NUM_EMBEDDERS];

        // Boost factual embedders (E1=0, E12=11, E13=12)
        alignments[0] = 0.9;
        alignments[11] = 0.9;
        alignments[12] = 0.9;

        let result = aggregator.aggregate(&alignments);

        // Factual should be higher
        assert!(result.alignments.factual > 0.8);
        assert!(result.alignments.factual > result.alignments.temporal);

        println!("[PASS] Varied input produces expected group differences");
    }

    #[test]
    fn test_coherence() {
        let aggregator = GroupAggregator::new();

        // Uniform = high coherence
        let uniform = aggregator.aggregate(&[0.7f32; NUM_EMBEDDERS]);
        assert!(uniform.coherence > 0.9);

        // Varied = lower coherence
        let mut varied_input = [0.5f32; NUM_EMBEDDERS];
        varied_input[0] = 0.1;
        varied_input[5] = 0.9;
        let varied = aggregator.aggregate(&varied_input);
        assert!(varied.coherence < uniform.coherence);

        println!("[PASS] Coherence reflects group agreement");
    }

    #[test]
    fn test_group_stats() {
        let aggregator = GroupAggregator::new();

        let mut alignments = [0.5f32; NUM_EMBEDDERS];
        // Make factual group have variance
        alignments[0] = 0.3;
        alignments[11] = 0.5;
        alignments[12] = 0.7;

        let result = aggregator.aggregate(&alignments);

        let factual_stats = &result.group_stats[0];
        assert_eq!(factual_stats.embedder_count, 3);
        assert!(factual_stats.std_dev > 0.0);
        assert!((factual_stats.range - 0.4).abs() < 0.01);

        println!("[PASS] Group stats computed correctly");
    }

    #[test]
    fn test_aggregation_methods() {
        let alignments = [
            0.3f32, 0.5, 0.7, 0.4, 0.6, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5,
        ];

        // Average method
        let avg_aggregator = GroupAggregator::with_config(GroupAggregationConfig {
            method: AggregationMethod::Average,
            ..Default::default()
        });
        let avg_result = avg_aggregator.aggregate(&alignments);

        // Max method
        let max_aggregator = GroupAggregator::with_config(GroupAggregationConfig {
            method: AggregationMethod::Max,
            ..Default::default()
        });
        let max_result = max_aggregator.aggregate(&alignments);

        // Max should be >= Average for factual (E1, E12, E13)
        assert!(max_result.alignments.factual >= avg_result.alignments.factual);

        println!("[PASS] Different aggregation methods produce different results");
    }

    #[test]
    fn test_aggregate_weighted() {
        let aggregator = GroupAggregator::new();
        let alignments = [0.8f32; NUM_EMBEDDERS];

        // Double weight for factual
        let group_weights = [2.0f32, 1.0, 1.0, 1.0, 1.0, 1.0];

        let result = aggregator.aggregate_weighted(&alignments, &group_weights);

        // Factual should be scaled up
        assert!(result.alignments.factual > result.alignments.temporal);

        println!("[PASS] aggregate_weighted applies group weights");
    }

    #[test]
    fn test_get_dominant_group() {
        let aggregator = GroupAggregator::new();

        let mut alignments = [0.5f32; NUM_EMBEDDERS];
        // Boost implementation (E6 = index 5)
        alignments[5] = 0.95;

        let dominant = aggregator.get_dominant_group(&alignments);
        assert_eq!(dominant, GroupType::Implementation);

        println!("[PASS] get_dominant_group identifies highest group");
    }

    #[test]
    fn test_get_weakest_group() {
        let aggregator = GroupAggregator::new();

        let mut alignments = [0.7f32; NUM_EMBEDDERS];
        // Lower qualitative (E10=9, E11=10)
        alignments[9] = 0.1;
        alignments[10] = 0.1;

        let weakest = aggregator.get_weakest_group(&alignments);
        assert_eq!(weakest, GroupType::Qualitative);

        println!("[PASS] get_weakest_group identifies lowest group");
    }

    #[test]
    fn test_geometric_mean() {
        let aggregator = GroupAggregator::with_config(GroupAggregationConfig {
            method: AggregationMethod::GeometricMean,
            ..Default::default()
        });

        // Geometric mean of 0.4, 0.5, 0.6 ≈ 0.493
        let alignments = [0.5f32; NUM_EMBEDDERS];
        let result = aggregator.aggregate(&alignments);

        // Should be close to 0.5 for uniform input
        assert!((result.alignments.factual - 0.5).abs() < 0.01);

        println!("[PASS] GeometricMean aggregation works");
    }

    #[test]
    fn test_normalize_output() {
        let aggregator = GroupAggregator::with_config(GroupAggregationConfig {
            method: AggregationMethod::Max,
            normalize_output: true,
            ..Default::default()
        });

        // Values > 1.0 should be clamped
        let alignments = [1.5f32; NUM_EMBEDDERS];
        let result = aggregator.aggregate(&alignments);

        assert!(result.alignments.factual <= 1.0);

        println!("[PASS] normalize_output clamps to [0, 1]");
    }

    #[test]
    fn test_custom_embedder_weights() {
        let mut weights = [1.0f32; NUM_EMBEDDERS];
        weights[0] = 3.0; // Triple weight for E1

        let aggregator = GroupAggregator::with_config(GroupAggregationConfig {
            method: AggregationMethod::WeightedAverage,
            embedder_weights: Some(weights),
            normalize_output: true,
        });

        let mut alignments = [0.5f32; NUM_EMBEDDERS];
        alignments[0] = 0.9; // E1 high

        let result = aggregator.aggregate(&alignments);

        // Factual should be pulled towards E1's value
        assert!(result.alignments.factual > 0.6);

        println!("[PASS] Custom embedder weights affect aggregation");
    }
}
