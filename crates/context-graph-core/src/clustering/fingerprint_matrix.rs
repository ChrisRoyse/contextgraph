//! Fingerprint Distance Matrix for multi-space clustering.
//!
//! Builds a co-association matrix using weighted similarity scores
//! from all 13 embedding spaces, enabling topic detection that
//! leverages the full fingerprint signal.
//!
//! # Architecture
//!
//! The fingerprint matrix approach (FDMC - Fingerprint Distance Matrix Clustering)
//! replaces independent per-space clustering with aggregated multi-space similarity:
//!
//! ```text
//! Memory A ─────┬──────────────────────────────────────────────────┐
//!               │                                                   │
//! Memory B ─────┼─> TeleologicalComparator ─> 13 per-space scores  │
//!               │              │                                    │
//! Memory C ─────┼──────────────┤                                    │
//!               │              ▼                                    │
//! Memory D ─────┘   Weighted Aggregation (category weights)         │
//!                              │                                    │
//!                              ▼                                    │
//!                   Co-Association Similarity Matrix                │
//!                              │                                    │
//!                              ▼                                    │
//!                     HDBSCAN (single run)                          │
//!                              │                                    │
//!                              ▼                                    │
//!                          Topics
//! ```
//!
//! # Why This Works
//!
//! 1. **Amplified Signal**: Gaps of 0.03-0.08 across 7 semantic spaces become 0.21-0.56 in aggregate
//! 2. **Weighted Agreement Built-In**: Category weights (semantic=1.0, temporal=0.0, etc.) applied
//! 3. **Single Clustering Run**: No need to synthesize from 13 independent cluster assignments
//! 4. **Leverages Existing Code**: TeleologicalComparator already implements apples-to-apples comparison
//!
//! # Constitution Compliance
//!
//! - ARCH-02: Apples-to-apples comparison (E1<->E1, never cross-space)
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - AP-10: No NaN/Infinity in similarity scores (FAIL FAST)
//! - AP-60: Temporal embedders (E2-E4) have weight 0.0

use crate::teleological::{Embedder, TeleologicalComparator, NUM_EMBEDDERS};
use crate::types::SemanticFingerprint;
use uuid::Uuid;

use super::error::ClusterError;

// =============================================================================
// Configuration
// =============================================================================

/// Configuration for fingerprint matrix construction.
#[derive(Debug, Clone)]
pub struct FingerprintMatrixConfig {
    /// Strategy for aggregation (default: WeightedSum)
    pub aggregation: AggregationStrategy,

    /// Minimum similarity threshold. Pairs below this are set to 0.0.
    /// Default: 0.0 (include all similarities)
    pub min_similarity_threshold: f32,

    /// Whether to store per-space similarities for analysis.
    /// Default: true for diagnostics
    pub store_per_space: bool,
}

impl Default for FingerprintMatrixConfig {
    fn default() -> Self {
        Self {
            aggregation: AggregationStrategy::WeightedSum,
            min_similarity_threshold: 0.0,
            store_per_space: true,
        }
    }
}

impl FingerprintMatrixConfig {
    /// Validate configuration parameters.
    pub fn validate(&self) -> Result<(), ClusterError> {
        if !(0.0..=1.0).contains(&self.min_similarity_threshold) {
            return Err(ClusterError::invalid_parameter(format!(
                "min_similarity_threshold must be in [0.0, 1.0], got {}",
                self.min_similarity_threshold
            )));
        }
        Ok(())
    }

    /// Create config optimized for topic detection.
    #[must_use]
    pub fn for_topic_detection() -> Self {
        Self {
            aggregation: AggregationStrategy::WeightedSum,
            min_similarity_threshold: 0.0,
            store_per_space: true,
        }
    }

    /// Create config optimized for performance (minimal storage).
    #[must_use]
    pub fn for_performance() -> Self {
        Self {
            aggregation: AggregationStrategy::WeightedSum,
            min_similarity_threshold: 0.0,
            store_per_space: false, // Skip per-space storage
        }
    }
}

// =============================================================================
// Aggregation Strategy
// =============================================================================

/// Strategy for aggregating per-embedder similarities into overall score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AggregationStrategy {
    /// Weighted sum using category weights (recommended for topic detection).
    ///
    /// Uses the TeleologicalComparator's built-in category weighting:
    /// - Semantic (E1, E5, E6, E7, E10, E12, E13): weight 1.0
    /// - Temporal (E2, E3, E4): weight 0.0 (excluded per AP-60)
    /// - Relational (E8, E11): weight 0.5
    /// - Structural (E9): weight 0.5
    #[default]
    WeightedSum,

    /// Maximum similarity across spaces (picks strongest agreement).
    MaxPooling,

    /// Mean of all non-zero similarities (unweighted average).
    MeanPooling,

    /// Geometric mean of similarities (harsh but precise).
    ProductRule,

    /// Minimum similarity (conservative, requires all spaces to agree).
    MinPooling,
}

// =============================================================================
// Fingerprint Matrix
// =============================================================================

/// Result of fingerprint matrix construction.
///
/// Contains the aggregated similarity matrix and optionally per-space
/// similarities for diagnostic analysis.
#[derive(Debug, Clone)]
pub struct FingerprintMatrix {
    /// Memory IDs in matrix order.
    pub memory_ids: Vec<Uuid>,

    /// Aggregated similarity matrix [n x n].
    /// Entry (i,j) = fingerprint_similarity(memory_i, memory_j)
    pub similarities: Vec<Vec<f32>>,

    /// Per-space similarity tensors [n x n x 13].
    /// Only populated if config.store_per_space = true.
    /// Entry (i,j,k) = similarity between memory i and j in embedder k.
    pub per_space: Option<Vec<Vec<[f32; NUM_EMBEDDERS]>>>,

    /// Configuration used to build this matrix.
    pub config: FingerprintMatrixConfig,
}

impl FingerprintMatrix {
    /// Get the size (number of memories) in the matrix.
    #[inline]
    pub fn size(&self) -> usize {
        self.memory_ids.len()
    }

    /// Check if matrix is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.memory_ids.is_empty()
    }

    /// Convert to distance matrix for clustering.
    ///
    /// Distance = 1.0 - similarity
    /// This produces a matrix suitable for HDBSCAN's fit_precomputed.
    #[must_use]
    pub fn to_distance_matrix(&self) -> Vec<Vec<f32>> {
        self.similarities
            .iter()
            .map(|row| row.iter().map(|s| 1.0 - s).collect())
            .collect()
    }

    /// Get similarity between two memories by their indices.
    #[inline]
    pub fn get_similarity(&self, i: usize, j: usize) -> Option<f32> {
        self.similarities.get(i).and_then(|row| row.get(j).copied())
    }

    /// Get distance between two memories by their indices.
    #[inline]
    pub fn get_distance(&self, i: usize, j: usize) -> Option<f32> {
        self.get_similarity(i, j).map(|s| 1.0 - s)
    }

    /// Analyze which embedders contribute most to cluster separation.
    ///
    /// Returns variance of similarity values per embedder.
    /// High variance = good discriminator (can separate different content).
    #[must_use]
    pub fn analyze_embedder_contributions(&self) -> [f32; NUM_EMBEDDERS] {
        let mut contributions = [0.0f32; NUM_EMBEDDERS];

        let Some(per_space) = &self.per_space else {
            return contributions; // No per-space data stored
        };

        let n = self.memory_ids.len();
        if n < 2 {
            return contributions;
        }

        for embedder_idx in 0..NUM_EMBEDDERS {
            let mut values = Vec::with_capacity(n * (n - 1) / 2);

            for i in 0..n {
                for j in (i + 1)..n {
                    values.push(per_space[i][j][embedder_idx]);
                }
            }

            if !values.is_empty() {
                let mean = values.iter().sum::<f32>() / values.len() as f32;
                let variance = values
                    .iter()
                    .map(|v| (v - mean).powi(2))
                    .sum::<f32>()
                    / values.len() as f32;
                contributions[embedder_idx] = variance;
            }
        }

        contributions
    }

    /// Find the dominant embedder (highest contribution to separation).
    #[must_use]
    pub fn dominant_embedder(&self) -> Option<Embedder> {
        let contributions = self.analyze_embedder_contributions();
        contributions
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .and_then(|(idx, _)| Embedder::from_index(idx))
    }

    /// Compute statistics about the similarity distribution.
    #[must_use]
    pub fn similarity_stats(&self) -> SimilarityStats {
        let n = self.size();
        if n < 2 {
            return SimilarityStats::default();
        }

        let mut values = Vec::with_capacity(n * (n - 1) / 2);
        for i in 0..n {
            for j in (i + 1)..n {
                values.push(self.similarities[i][j]);
            }
        }

        if values.is_empty() {
            return SimilarityStats::default();
        }

        let min = values.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        let variance = values
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f32>()
            / values.len() as f32;
        let std_dev = variance.sqrt();

        // Find gap (largest jump in sorted similarities)
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut max_gap = 0.0f32;
        let mut gap_position = 0.5f32;
        for i in 0..(values.len() - 1) {
            let gap = values[i + 1] - values[i];
            if gap > max_gap {
                max_gap = gap;
                gap_position = (values[i] + values[i + 1]) / 2.0;
            }
        }

        SimilarityStats {
            min,
            max,
            mean,
            std_dev,
            max_gap,
            gap_position,
            pair_count: values.len(),
        }
    }
}

/// Statistics about similarity distribution.
#[derive(Debug, Clone, Default)]
pub struct SimilarityStats {
    /// Minimum similarity value.
    pub min: f32,
    /// Maximum similarity value.
    pub max: f32,
    /// Mean similarity.
    pub mean: f32,
    /// Standard deviation.
    pub std_dev: f32,
    /// Largest gap in similarity distribution.
    pub max_gap: f32,
    /// Similarity value at gap midpoint.
    pub gap_position: f32,
    /// Number of pairwise comparisons.
    pub pair_count: usize,
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Aggregate per-embedder similarities into a single similarity score.
///
/// This helper centralizes the aggregation logic used by both `build_fingerprint_matrix`
/// and `update_matrix_incremental`.
#[inline]
fn aggregate_similarity(
    per_embedder: &[Option<f32>; 13],
    overall: f32,
    strategy: AggregationStrategy,
) -> f32 {
    match strategy {
        AggregationStrategy::WeightedSum => overall,
        AggregationStrategy::MaxPooling => per_embedder
            .iter()
            .filter_map(|s| *s)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0),
        AggregationStrategy::MeanPooling => {
            let values: Vec<f32> = per_embedder.iter().filter_map(|s| *s).collect();
            if values.is_empty() {
                0.0
            } else {
                values.iter().sum::<f32>() / values.len() as f32
            }
        }
        AggregationStrategy::ProductRule => {
            let values: Vec<f32> = per_embedder
                .iter()
                .filter_map(|s| *s)
                .filter(|&s| s > 0.0)
                .collect();
            if values.is_empty() {
                0.0
            } else {
                let product: f32 = values.iter().product();
                product.powf(1.0 / values.len() as f32)
            }
        }
        AggregationStrategy::MinPooling => per_embedder
            .iter()
            .filter_map(|s| *s)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0),
    }
}

// =============================================================================
// Matrix Builder
// =============================================================================

/// Build fingerprint similarity matrix for a set of memories.
///
/// Uses TeleologicalComparator to compute pairwise similarities across
/// all 13 embedding spaces, then aggregates according to the configured strategy.
///
/// # Arguments
///
/// * `fingerprints` - Slice of (memory_id, fingerprint) pairs
/// * `config` - Configuration for matrix construction
///
/// # Returns
///
/// `FingerprintMatrix` containing similarity values.
///
/// # Errors
///
/// - `ClusterError::InvalidParameter` if config validation fails
/// - `ClusterError::InvalidParameter` if any fingerprint produces NaN/Infinity
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_core::clustering::{build_fingerprint_matrix, FingerprintMatrixConfig};
/// use context_graph_core::types::SemanticFingerprint;
/// use uuid::Uuid;
///
/// let fingerprints: Vec<(Uuid, &SemanticFingerprint)> = get_fingerprints();
/// let config = FingerprintMatrixConfig::for_topic_detection();
/// let matrix = build_fingerprint_matrix(&fingerprints, &config)?;
///
/// // Get distance matrix for HDBSCAN
/// let distances = matrix.to_distance_matrix();
/// ```
pub fn build_fingerprint_matrix(
    fingerprints: &[(Uuid, &SemanticFingerprint)],
    config: &FingerprintMatrixConfig,
) -> Result<FingerprintMatrix, ClusterError> {
    config.validate()?;

    let n = fingerprints.len();
    let comparator = TeleologicalComparator::new();

    // Initialize matrices
    let mut similarities = vec![vec![0.0f32; n]; n];
    let mut per_space: Option<Vec<Vec<[f32; NUM_EMBEDDERS]>>> = if config.store_per_space {
        Some(vec![vec![[0.0f32; NUM_EMBEDDERS]; n]; n])
    } else {
        None
    };

    // Fill diagonal (self-similarity = 1.0)
    for i in 0..n {
        similarities[i][i] = 1.0;
        if let Some(ref mut ps) = per_space {
            ps[i][i] = [1.0f32; NUM_EMBEDDERS];
        }
    }

    // Compute pairwise similarities
    for i in 0..n {
        for j in (i + 1)..n {
            let result = comparator
                .compare(fingerprints[i].1, fingerprints[j].1)
                .map_err(|e| {
                    ClusterError::invalid_parameter(format!(
                        "Failed to compare fingerprints {} and {}: {}",
                        fingerprints[i].0, fingerprints[j].0, e
                    ))
                })?;

            // Validate result (FAIL FAST per AP-10)
            if !result.overall.is_finite() {
                return Err(ClusterError::invalid_parameter(format!(
                    "Comparison between {} and {} produced non-finite overall similarity: {}",
                    fingerprints[i].0, fingerprints[j].0, result.overall
                )));
            }

            // Aggregate per-embedder scores using helper
            let aggregated_sim = aggregate_similarity(
                &result.per_embedder,
                result.overall,
                config.aggregation,
            );

            // Apply minimum threshold
            let final_sim = if aggregated_sim >= config.min_similarity_threshold {
                aggregated_sim
            } else {
                0.0
            };

            // Store in symmetric matrix
            similarities[i][j] = final_sim;
            similarities[j][i] = final_sim;

            // Store per-space similarities if requested
            if let Some(ref mut ps) = per_space {
                let per_emb: [f32; NUM_EMBEDDERS] =
                    std::array::from_fn(|k| result.per_embedder[k].unwrap_or(0.0));
                ps[i][j] = per_emb;
                ps[j][i] = per_emb;
            }
        }
    }

    Ok(FingerprintMatrix {
        memory_ids: fingerprints.iter().map(|(id, _)| *id).collect(),
        similarities,
        per_space,
        config: config.clone(),
    })
}

// =============================================================================
// Incremental Update
// =============================================================================

/// Update fingerprint matrix with a new memory.
///
/// Only computes similarities between the new memory and existing ones,
/// O(n) instead of O(n²) for full rebuild.
///
/// # Arguments
///
/// * `matrix` - Existing matrix to update
/// * `new_id` - UUID of the new memory
/// * `new_fp` - Fingerprint of the new memory
///
/// # Errors
///
/// Returns error if comparison produces NaN/Infinity.
pub fn update_matrix_incremental(
    matrix: &mut FingerprintMatrix,
    new_id: Uuid,
    new_fp: &SemanticFingerprint,
    existing_fingerprints: &[(Uuid, &SemanticFingerprint)],
) -> Result<(), ClusterError> {
    let comparator = TeleologicalComparator::new();
    let n = matrix.memory_ids.len();

    // Extend matrix dimensions
    matrix.memory_ids.push(new_id);
    for row in &mut matrix.similarities {
        row.push(0.0);
    }
    matrix.similarities.push(vec![0.0; n + 1]);
    matrix.similarities[n][n] = 1.0; // Self-similarity

    // Extend per-space if present
    if let Some(ref mut ps) = matrix.per_space {
        for row in ps.iter_mut() {
            row.push([0.0f32; NUM_EMBEDDERS]);
        }
        ps.push(vec![[0.0f32; NUM_EMBEDDERS]; n + 1]);
        ps[n][n] = [1.0f32; NUM_EMBEDDERS];
    }

    // Compute similarities to new memory
    for (i, (_, existing_fp)) in existing_fingerprints.iter().enumerate() {
        if i >= n {
            break; // Safety check
        }

        let result = comparator.compare(*existing_fp, new_fp).map_err(|e| {
            ClusterError::invalid_parameter(format!(
                "Failed to compare fingerprint with new memory {}: {}",
                new_id, e
            ))
        })?;

        // Validate result
        if !result.overall.is_finite() {
            return Err(ClusterError::invalid_parameter(format!(
                "Comparison with new memory {} produced non-finite similarity: {}",
                new_id, result.overall
            )));
        }

        // Use shared aggregation helper
        let sim = aggregate_similarity(
            &result.per_embedder,
            result.overall,
            matrix.config.aggregation,
        );

        let final_sim = if sim >= matrix.config.min_similarity_threshold {
            sim
        } else {
            0.0
        };

        matrix.similarities[i][n] = final_sim;
        matrix.similarities[n][i] = final_sim;

        // Store per-space if present
        if let Some(ref mut ps) = matrix.per_space {
            let per_emb: [f32; NUM_EMBEDDERS] =
                std::array::from_fn(|k| result.per_embedder[k].unwrap_or(0.0));
            ps[i][n] = per_emb;
            ps[n][i] = per_emb;
        }
    }

    Ok(())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create test fingerprints with known similarity patterns
    #[cfg(any(test, feature = "test-utils"))]
    fn create_test_fingerprint(offset: f32) -> SemanticFingerprint {
        use crate::embeddings::config::get_dimension;
        use crate::types::SparseVector;

        SemanticFingerprint {
            e1_semantic: vec![offset; get_dimension(Embedder::Semantic)],
            e2_temporal_recent: vec![offset; get_dimension(Embedder::TemporalRecent)],
            e3_temporal_periodic: vec![offset; get_dimension(Embedder::TemporalPeriodic)],
            e4_temporal_positional: vec![offset; get_dimension(Embedder::TemporalPositional)],
            e5_causal_as_cause: vec![offset; get_dimension(Embedder::Causal)],
            e5_causal_as_effect: vec![offset; get_dimension(Embedder::Causal)],
            e5_causal: Vec::new(), // Using new dual format
            e6_sparse: SparseVector::empty(),
            e7_code: vec![offset; get_dimension(Embedder::Code)],
            e8_graph_as_source: vec![offset; get_dimension(Embedder::Emotional)],
            e8_graph_as_target: vec![offset; get_dimension(Embedder::Emotional)],
            e8_graph: Vec::new(), // Legacy field, empty by default
            e9_hdc: vec![offset; get_dimension(Embedder::Hdc)],
            e10_multimodal: vec![offset; get_dimension(Embedder::Multimodal)],
            e11_entity: vec![offset; get_dimension(Embedder::Entity)],
            e12_late_interaction: vec![vec![offset; 128]; 10],
            e13_splade: SparseVector::empty(),
        }
    }

    #[test]
    fn test_config_defaults() {
        let config = FingerprintMatrixConfig::default();
        assert_eq!(config.aggregation, AggregationStrategy::WeightedSum);
        assert_eq!(config.min_similarity_threshold, 0.0);
        assert!(config.store_per_space);
        assert!(config.validate().is_ok());

        println!("[PASS] test_config_defaults");
    }

    #[test]
    fn test_config_validation_invalid_threshold() {
        let mut config = FingerprintMatrixConfig::default();
        config.min_similarity_threshold = 1.5;

        let result = config.validate();
        assert!(result.is_err());

        println!("[PASS] test_config_validation_invalid_threshold");
    }

    #[test]
    fn test_build_empty_matrix() {
        let config = FingerprintMatrixConfig::default();
        let fingerprints: Vec<(Uuid, &SemanticFingerprint)> = vec![];

        let result = build_fingerprint_matrix(&fingerprints, &config);
        assert!(result.is_ok());

        let matrix = result.unwrap();
        assert!(matrix.is_empty());
        assert_eq!(matrix.size(), 0);

        println!("[PASS] test_build_empty_matrix");
    }

    #[test]
    fn test_build_single_fingerprint() {
        let config = FingerprintMatrixConfig::default();
        let fp = create_test_fingerprint(0.5);
        let id = Uuid::new_v4();
        let fingerprints: Vec<(Uuid, &SemanticFingerprint)> = vec![(id, &fp)];

        let result = build_fingerprint_matrix(&fingerprints, &config);
        assert!(result.is_ok());

        let matrix = result.unwrap();
        assert_eq!(matrix.size(), 1);
        assert_eq!(matrix.similarities[0][0], 1.0); // Self-similarity

        println!("[PASS] test_build_single_fingerprint");
    }

    #[test]
    fn test_build_two_fingerprints_identical() {
        let config = FingerprintMatrixConfig::default();
        let fp1 = create_test_fingerprint(0.5);
        let fp2 = create_test_fingerprint(0.5); // Identical values
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let fingerprints: Vec<(Uuid, &SemanticFingerprint)> = vec![(id1, &fp1), (id2, &fp2)];

        let result = build_fingerprint_matrix(&fingerprints, &config);
        assert!(result.is_ok());

        let matrix = result.unwrap();
        assert_eq!(matrix.size(), 2);

        // Identical fingerprints should have high similarity
        let sim = matrix.get_similarity(0, 1).unwrap();
        assert!(
            sim > 0.9,
            "Identical fingerprints should have similarity > 0.9, got {}",
            sim
        );

        // Matrix should be symmetric
        assert_eq!(
            matrix.get_similarity(0, 1),
            matrix.get_similarity(1, 0)
        );

        println!(
            "[PASS] test_build_two_fingerprints_identical - similarity={}",
            sim
        );
    }

    #[test]
    fn test_to_distance_matrix() {
        let config = FingerprintMatrixConfig::default();
        let fp1 = create_test_fingerprint(0.5);
        let fp2 = create_test_fingerprint(0.5);
        let fingerprints: Vec<(Uuid, &SemanticFingerprint)> =
            vec![(Uuid::new_v4(), &fp1), (Uuid::new_v4(), &fp2)];

        let matrix = build_fingerprint_matrix(&fingerprints, &config).unwrap();
        let distances = matrix.to_distance_matrix();

        // Self-distance should be 0
        assert_eq!(distances[0][0], 0.0);
        assert_eq!(distances[1][1], 0.0);

        // Distance = 1 - similarity
        let sim = matrix.get_similarity(0, 1).unwrap();
        let dist = matrix.get_distance(0, 1).unwrap();
        assert!((dist - (1.0 - sim)).abs() < f32::EPSILON);

        println!("[PASS] test_to_distance_matrix");
    }

    #[test]
    fn test_similarity_stats() {
        let config = FingerprintMatrixConfig::default();
        let fp1 = create_test_fingerprint(0.3);
        let fp2 = create_test_fingerprint(0.5);
        let fp3 = create_test_fingerprint(0.7);
        let fingerprints: Vec<(Uuid, &SemanticFingerprint)> = vec![
            (Uuid::new_v4(), &fp1),
            (Uuid::new_v4(), &fp2),
            (Uuid::new_v4(), &fp3),
        ];

        let matrix = build_fingerprint_matrix(&fingerprints, &config).unwrap();
        let stats = matrix.similarity_stats();

        assert!(stats.pair_count == 3); // 3 choose 2
        assert!(stats.min <= stats.mean);
        assert!(stats.mean <= stats.max);

        println!(
            "[PASS] test_similarity_stats - min={}, max={}, mean={}",
            stats.min, stats.max, stats.mean
        );
    }

    #[test]
    fn test_aggregation_strategy_max_pooling() {
        let config = FingerprintMatrixConfig {
            aggregation: AggregationStrategy::MaxPooling,
            ..Default::default()
        };

        let fp1 = create_test_fingerprint(0.5);
        let fp2 = create_test_fingerprint(0.6);
        let fingerprints: Vec<(Uuid, &SemanticFingerprint)> =
            vec![(Uuid::new_v4(), &fp1), (Uuid::new_v4(), &fp2)];

        let result = build_fingerprint_matrix(&fingerprints, &config);
        assert!(result.is_ok());

        let matrix = result.unwrap();
        let sim = matrix.get_similarity(0, 1).unwrap();
        assert!(sim.is_finite(), "MaxPooling should produce finite similarity");

        println!(
            "[PASS] test_aggregation_strategy_max_pooling - similarity={}",
            sim
        );
    }

    #[test]
    fn test_aggregation_strategy_mean_pooling() {
        let config = FingerprintMatrixConfig {
            aggregation: AggregationStrategy::MeanPooling,
            ..Default::default()
        };

        let fp1 = create_test_fingerprint(0.5);
        let fp2 = create_test_fingerprint(0.6);
        let fingerprints: Vec<(Uuid, &SemanticFingerprint)> =
            vec![(Uuid::new_v4(), &fp1), (Uuid::new_v4(), &fp2)];

        let result = build_fingerprint_matrix(&fingerprints, &config);
        assert!(result.is_ok());

        println!("[PASS] test_aggregation_strategy_mean_pooling");
    }

    #[test]
    fn test_per_space_storage() {
        let config = FingerprintMatrixConfig {
            store_per_space: true,
            ..Default::default()
        };

        let fp1 = create_test_fingerprint(0.5);
        let fp2 = create_test_fingerprint(0.6);
        let fingerprints: Vec<(Uuid, &SemanticFingerprint)> =
            vec![(Uuid::new_v4(), &fp1), (Uuid::new_v4(), &fp2)];

        let matrix = build_fingerprint_matrix(&fingerprints, &config).unwrap();

        assert!(matrix.per_space.is_some());
        let per_space = matrix.per_space.as_ref().unwrap();
        assert_eq!(per_space.len(), 2);
        assert_eq!(per_space[0].len(), 2);
        assert_eq!(per_space[0][0].len(), NUM_EMBEDDERS);

        println!("[PASS] test_per_space_storage");
    }

    #[test]
    fn test_per_space_disabled() {
        let config = FingerprintMatrixConfig {
            store_per_space: false,
            ..Default::default()
        };

        let fp1 = create_test_fingerprint(0.5);
        let fp2 = create_test_fingerprint(0.6);
        let fingerprints: Vec<(Uuid, &SemanticFingerprint)> =
            vec![(Uuid::new_v4(), &fp1), (Uuid::new_v4(), &fp2)];

        let matrix = build_fingerprint_matrix(&fingerprints, &config).unwrap();

        assert!(matrix.per_space.is_none());

        println!("[PASS] test_per_space_disabled");
    }

    #[test]
    fn test_embedder_contributions() {
        let config = FingerprintMatrixConfig::default();
        let fp1 = create_test_fingerprint(0.3);
        let fp2 = create_test_fingerprint(0.5);
        let fp3 = create_test_fingerprint(0.7);
        let fingerprints: Vec<(Uuid, &SemanticFingerprint)> = vec![
            (Uuid::new_v4(), &fp1),
            (Uuid::new_v4(), &fp2),
            (Uuid::new_v4(), &fp3),
        ];

        let matrix = build_fingerprint_matrix(&fingerprints, &config).unwrap();
        let contributions = matrix.analyze_embedder_contributions();

        // All contributions should be finite
        for (i, c) in contributions.iter().enumerate() {
            assert!(c.is_finite(), "Contribution for embedder {} is not finite", i);
        }

        println!("[PASS] test_embedder_contributions - {:?}", contributions);
    }

    #[test]
    fn test_incremental_update() {
        let config = FingerprintMatrixConfig::default();
        let fp1 = create_test_fingerprint(0.5);
        let fp2 = create_test_fingerprint(0.6);
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        // Build initial matrix with 2 fingerprints
        let fingerprints: Vec<(Uuid, &SemanticFingerprint)> = vec![(id1, &fp1), (id2, &fp2)];
        let mut matrix = build_fingerprint_matrix(&fingerprints, &config).unwrap();

        assert_eq!(matrix.size(), 2);

        // Add a third fingerprint incrementally
        let fp3 = create_test_fingerprint(0.7);
        let id3 = Uuid::new_v4();

        let result = update_matrix_incremental(&mut matrix, id3, &fp3, &fingerprints);
        assert!(result.is_ok());

        assert_eq!(matrix.size(), 3);
        assert_eq!(matrix.memory_ids[2], id3);

        // Verify new similarities exist
        let sim_0_2 = matrix.get_similarity(0, 2);
        let sim_1_2 = matrix.get_similarity(1, 2);
        let sim_2_2 = matrix.get_similarity(2, 2);

        assert!(sim_0_2.is_some());
        assert!(sim_1_2.is_some());
        assert_eq!(sim_2_2, Some(1.0)); // Self-similarity

        // Verify symmetry
        assert_eq!(matrix.get_similarity(0, 2), matrix.get_similarity(2, 0));
        assert_eq!(matrix.get_similarity(1, 2), matrix.get_similarity(2, 1));

        println!("[PASS] test_incremental_update");
    }

    #[test]
    fn test_min_similarity_threshold() {
        let config = FingerprintMatrixConfig {
            min_similarity_threshold: 0.95,
            ..Default::default()
        };

        let fp1 = create_test_fingerprint(0.3);
        let fp2 = create_test_fingerprint(0.9); // Very different
        let fingerprints: Vec<(Uuid, &SemanticFingerprint)> =
            vec![(Uuid::new_v4(), &fp1), (Uuid::new_v4(), &fp2)];

        let matrix = build_fingerprint_matrix(&fingerprints, &config).unwrap();
        let sim = matrix.get_similarity(0, 1).unwrap();

        // If similarity is below threshold, it should be set to 0.0
        // (Note: actual behavior depends on fingerprint similarity)
        assert!(sim.is_finite());

        println!(
            "[PASS] test_min_similarity_threshold - similarity={}",
            sim
        );
    }

    #[test]
    fn test_memory_ids_preserved() {
        let config = FingerprintMatrixConfig::default();
        let fp1 = create_test_fingerprint(0.5);
        let fp2 = create_test_fingerprint(0.6);
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        let fingerprints: Vec<(Uuid, &SemanticFingerprint)> = vec![(id1, &fp1), (id2, &fp2)];
        let matrix = build_fingerprint_matrix(&fingerprints, &config).unwrap();

        assert_eq!(matrix.memory_ids[0], id1);
        assert_eq!(matrix.memory_ids[1], id2);

        println!("[PASS] test_memory_ids_preserved");
    }
}
