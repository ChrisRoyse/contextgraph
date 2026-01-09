//! TeleologicalMatrixSearch: Cross-correlation search across all 13 embedders.
//!
//! This module implements the "super search algorithm" for teleological vectors,
//! enabling multi-level cosine similarity comparisons across the full 13x13
//! embedding matrix.
//!
//! # Search Levels
//!
//! 1. **Full Matrix (13x13)**: Compare all 78 cross-correlations
//! 2. **Purpose Vector (13D)**: Compare per-embedder alignments to North Star
//! 3. **Group Level (6D)**: Compare 6 hierarchical group alignments
//! 4. **Single Embedder**: Compare specific embedder correlation patterns
//! 5. **Synergy-Weighted**: Use learned synergy matrix as similarity weights
//!
//! # Comparison Strategies
//!
//! - `Cosine`: Standard cosine similarity (normalized dot product)
//! - `Euclidean`: L2 distance (inverted to similarity)
//! - `SynergyWeighted`: Synergy matrix modulates importance
//! - `GroupHierarchical`: Aggregate by embedding groups
//! - `CrossCorrelationDominant`: Prioritize 78 pair interactions
//! - `TuckerCompressed`: Use Tucker decomposition for compressed comparison

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::comparison_error::{
    ComparisonValidationError, ComparisonValidationResult, WeightValues,
};
use super::groups::{GroupAlignments, GroupType, NUM_GROUPS};
use super::synergy_matrix::{SynergyMatrix, CROSS_CORRELATION_COUNT};
use super::types::NUM_EMBEDDERS;
use super::vector::TeleologicalVector;

/// Search strategy for comparing teleological vectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum SearchStrategy {
    /// Standard cosine similarity across all components
    #[default]
    Cosine,
    /// Euclidean distance (inverted to similarity)
    Euclidean,
    /// Synergy matrix modulates importance of each pair
    SynergyWeighted,
    /// Compare at group level only (6D)
    GroupHierarchical,
    /// Prioritize cross-correlation patterns (78D)
    CrossCorrelationDominant,
    /// Use Tucker core for compressed comparison (if available)
    TuckerCompressed,
    /// Adaptive: choose best strategy based on vector characteristics
    Adaptive,
}

/// Which components of the teleological vector to compare.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ComparisonScope {
    /// Compare all components: purpose + correlations + groups
    #[default]
    Full,
    /// Compare only the 13D purpose vector
    PurposeVectorOnly,
    /// Compare only the 78 cross-correlations
    CrossCorrelationsOnly,
    /// Compare only the 6D group alignments
    GroupAlignmentsOnly,
    /// Compare specific embedder pairs
    SpecificPairs(Vec<(usize, usize)>),
    /// Compare specific embedding groups
    SpecificGroups(Vec<GroupType>),
    /// Compare a single embedder's correlation pattern (all 12 pairs it's in)
    SingleEmbedderPattern(usize),
}

/// Weights for different comparison components in Full scope.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComponentWeights {
    /// Weight for purpose vector similarity (default 0.4)
    pub purpose_vector: f32,
    /// Weight for cross-correlation similarity (default 0.35)
    pub cross_correlations: f32,
    /// Weight for group alignments similarity (default 0.15)
    pub group_alignments: f32,
    /// Weight for confidence factor (default 0.1)
    pub confidence: f32,
}

impl Default for ComponentWeights {
    fn default() -> Self {
        Self {
            purpose_vector: 0.4,
            cross_correlations: 0.35,
            group_alignments: 0.15,
            confidence: 0.1,
        }
    }
}

impl ComponentWeights {
    /// Weights emphasizing cross-correlations (for teleological search)
    pub fn correlation_focused() -> Self {
        Self {
            purpose_vector: 0.25,
            cross_correlations: 0.55,
            group_alignments: 0.15,
            confidence: 0.05,
        }
    }

    /// Weights emphasizing purpose alignment (for goal-directed search)
    pub fn purpose_focused() -> Self {
        Self {
            purpose_vector: 0.6,
            cross_correlations: 0.2,
            group_alignments: 0.15,
            confidence: 0.05,
        }
    }

    /// Weights emphasizing group structure (for hierarchical search)
    pub fn group_focused() -> Self {
        Self {
            purpose_vector: 0.25,
            cross_correlations: 0.25,
            group_alignments: 0.45,
            confidence: 0.05,
        }
    }

    /// Tolerance for weight sum validation (0.1%)
    pub const WEIGHT_SUM_TOLERANCE: f32 = 0.001;

    /// Validate all weight invariants.
    ///
    /// Returns `Ok(())` if:
    /// - All weights are in [0.0, 1.0]
    /// - Weights sum to 1.0 (±tolerance)
    ///
    /// Returns detailed error describing exactly what failed.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::teleological::ComponentWeights;
    ///
    /// let valid = ComponentWeights::default();
    /// assert!(valid.validate().is_ok());
    ///
    /// let mut invalid = ComponentWeights::default();
    /// invalid.purpose_vector = 2.0; // Out of range
    /// assert!(invalid.validate().is_err());
    /// ```
    pub fn validate(&self) -> ComparisonValidationResult<()> {
        // Check individual weight ranges
        if !(0.0..=1.0).contains(&self.purpose_vector) {
            return Err(ComparisonValidationError::WeightOutOfRange {
                field_name: "purpose_vector",
                value: self.purpose_vector,
                min: 0.0,
                max: 1.0,
            });
        }

        if !(0.0..=1.0).contains(&self.cross_correlations) {
            return Err(ComparisonValidationError::WeightOutOfRange {
                field_name: "cross_correlations",
                value: self.cross_correlations,
                min: 0.0,
                max: 1.0,
            });
        }

        if !(0.0..=1.0).contains(&self.group_alignments) {
            return Err(ComparisonValidationError::WeightOutOfRange {
                field_name: "group_alignments",
                value: self.group_alignments,
                min: 0.0,
                max: 1.0,
            });
        }

        if !(0.0..=1.0).contains(&self.confidence) {
            return Err(ComparisonValidationError::WeightOutOfRange {
                field_name: "confidence",
                value: self.confidence,
                min: 0.0,
                max: 1.0,
            });
        }

        // Check sum
        let sum =
            self.purpose_vector + self.cross_correlations + self.group_alignments + self.confidence;

        if (sum - 1.0).abs() > Self::WEIGHT_SUM_TOLERANCE {
            return Err(ComparisonValidationError::WeightsNotNormalized {
                actual_sum: sum,
                expected_sum: 1.0,
                tolerance: Self::WEIGHT_SUM_TOLERANCE,
                weights: WeightValues {
                    purpose_vector: self.purpose_vector,
                    cross_correlations: self.cross_correlations,
                    group_alignments: self.group_alignments,
                    confidence: self.confidence,
                },
            });
        }

        Ok(())
    }

    /// Check if weights are valid (returns bool for simple checks).
    ///
    /// For detailed error information, use `validate()` instead.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }

    /// Normalize weights to sum to 1.0
    pub fn normalize(&mut self) {
        let sum =
            self.purpose_vector + self.cross_correlations + self.group_alignments + self.confidence;
        if sum > f32::EPSILON {
            self.purpose_vector /= sum;
            self.cross_correlations /= sum;
            self.group_alignments /= sum;
            self.confidence /= sum;
        }
    }
}

/// Configuration for teleological matrix search.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatrixSearchConfig {
    /// Search strategy to use
    pub strategy: SearchStrategy,
    /// Which components to compare
    pub scope: ComparisonScope,
    /// Component weights (for Full scope)
    pub weights: ComponentWeights,
    /// Optional synergy matrix for weighted comparisons
    pub synergy_matrix: Option<SynergyMatrix>,
    /// Minimum similarity threshold for results
    pub min_similarity: f32,
    /// Maximum number of results to return
    pub max_results: usize,
    /// Whether to compute per-component breakdown
    pub compute_breakdown: bool,
}

impl Default for MatrixSearchConfig {
    fn default() -> Self {
        Self {
            strategy: SearchStrategy::default(),
            scope: ComparisonScope::default(),
            weights: ComponentWeights::default(),
            synergy_matrix: None,
            min_similarity: 0.0,
            max_results: 100,
            compute_breakdown: false,
        }
    }
}

impl MatrixSearchConfig {
    /// Create config for correlation-focused search
    pub fn correlation_focused() -> Self {
        Self {
            strategy: SearchStrategy::CrossCorrelationDominant,
            scope: ComparisonScope::CrossCorrelationsOnly,
            weights: ComponentWeights::correlation_focused(),
            ..Default::default()
        }
    }

    /// Create config for purpose-aligned search
    pub fn purpose_aligned() -> Self {
        Self {
            strategy: SearchStrategy::Cosine,
            scope: ComparisonScope::PurposeVectorOnly,
            weights: ComponentWeights::purpose_focused(),
            ..Default::default()
        }
    }

    /// Create config for group-hierarchical search
    pub fn group_hierarchical() -> Self {
        Self {
            strategy: SearchStrategy::GroupHierarchical,
            scope: ComparisonScope::GroupAlignmentsOnly,
            weights: ComponentWeights::group_focused(),
            ..Default::default()
        }
    }

    /// Create config with synergy weighting
    pub fn with_synergy(synergy_matrix: SynergyMatrix) -> Self {
        Self {
            strategy: SearchStrategy::SynergyWeighted,
            synergy_matrix: Some(synergy_matrix),
            ..Default::default()
        }
    }

    /// Create config for adaptive search
    pub fn adaptive() -> Self {
        Self {
            strategy: SearchStrategy::Adaptive,
            compute_breakdown: true,
            ..Default::default()
        }
    }
}

/// Detailed breakdown of similarity components.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimilarityBreakdown {
    /// Overall similarity score [0, 1]
    pub overall: f32,
    /// Purpose vector similarity [0, 1]
    pub purpose_vector: f32,
    /// Cross-correlation similarity [0, 1]
    pub cross_correlations: f32,
    /// Group alignments similarity [0, 1]
    pub group_alignments: f32,
    /// Per-group similarity scores
    pub per_group: HashMap<GroupType, f32>,
    /// Per-embedder purpose alignment similarity
    pub per_embedder_purpose: [f32; NUM_EMBEDDERS],
    /// Top contributing cross-correlation pairs
    pub top_correlation_pairs: Vec<((usize, usize), f32)>,
    /// Strategy used for this comparison
    pub strategy_used: SearchStrategy,
}

impl Default for SimilarityBreakdown {
    fn default() -> Self {
        Self {
            overall: 0.0,
            purpose_vector: 0.0,
            cross_correlations: 0.0,
            group_alignments: 0.0,
            per_group: HashMap::new(),
            per_embedder_purpose: [0.0; NUM_EMBEDDERS],
            top_correlation_pairs: Vec::new(),
            strategy_used: SearchStrategy::default(),
        }
    }
}

/// TeleologicalMatrixSearch: The super search algorithm for 13-embedder teleological vectors.
///
/// Enables cross-correlation search across all 13 embedders at multiple levels,
/// with configurable comparison strategies and component weighting.
pub struct TeleologicalMatrixSearch {
    config: MatrixSearchConfig,
}

impl TeleologicalMatrixSearch {
    /// Create a new matrix search with default configuration.
    pub fn new() -> Self {
        Self {
            config: MatrixSearchConfig::default(),
        }
    }

    /// Create with specific configuration.
    pub fn with_config(config: MatrixSearchConfig) -> Self {
        Self { config }
    }

    /// Get current configuration.
    pub fn config(&self) -> &MatrixSearchConfig {
        &self.config
    }

    /// Set configuration.
    pub fn set_config(&mut self, config: MatrixSearchConfig) {
        self.config = config;
    }

    /// Compute similarity between two teleological vectors.
    ///
    /// Returns similarity score in [0, 1] where 1 = identical.
    pub fn similarity(&self, a: &TeleologicalVector, b: &TeleologicalVector) -> f32 {
        match self.config.strategy {
            SearchStrategy::Cosine => self.cosine_similarity(a, b),
            SearchStrategy::Euclidean => self.euclidean_similarity(a, b),
            SearchStrategy::SynergyWeighted => self.synergy_weighted_similarity(a, b),
            SearchStrategy::GroupHierarchical => self.group_hierarchical_similarity(a, b),
            SearchStrategy::CrossCorrelationDominant => self.cross_correlation_similarity(a, b),
            SearchStrategy::TuckerCompressed => self.tucker_similarity(a, b),
            SearchStrategy::Adaptive => self.adaptive_similarity(a, b),
        }
    }

    /// Compute similarity with full breakdown.
    pub fn similarity_with_breakdown(
        &self,
        a: &TeleologicalVector,
        b: &TeleologicalVector,
    ) -> SimilarityBreakdown {
        let mut breakdown = SimilarityBreakdown {
            strategy_used: self.config.strategy,
            ..Default::default()
        };

        // Purpose vector similarity
        breakdown.purpose_vector = self.compute_purpose_similarity(a, b);

        // Per-embedder purpose similarity
        for ((out, &av), &bv) in breakdown
            .per_embedder_purpose
            .iter_mut()
            .zip(a.purpose_vector.alignments.iter())
            .zip(b.purpose_vector.alignments.iter())
        {
            // Product similarity for aligned values
            *out = if av.signum() == bv.signum() {
                1.0 - (av - bv).abs() / 2.0
            } else {
                0.0
            };
        }

        // Cross-correlation similarity
        breakdown.cross_correlations = self.compute_correlation_similarity(a, b);

        // Find top contributing pairs
        let mut pairs_with_sim: Vec<((usize, usize), f32)> =
            Vec::with_capacity(CROSS_CORRELATION_COUNT);
        for (flat_idx, (&av, &bv)) in a
            .cross_correlations
            .iter()
            .zip(b.cross_correlations.iter())
            .enumerate()
        {
            let (i, j) = SynergyMatrix::flat_to_indices(flat_idx);
            // Contribution = product (high if both agree)
            let contrib = av * bv;
            pairs_with_sim.push(((i, j), contrib));
        }
        pairs_with_sim.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap_or(std::cmp::Ordering::Equal));
        breakdown.top_correlation_pairs = pairs_with_sim.into_iter().take(10).collect();

        // Group alignments similarity
        breakdown.group_alignments = a.group_alignments.similarity(&b.group_alignments);

        // Per-group similarity
        for group in GroupType::ALL {
            let ga = a.group_alignments.get(group);
            let gb = b.group_alignments.get(group);
            let sim = 1.0 - (ga - gb).abs();
            breakdown.per_group.insert(group, sim);
        }

        // Overall based on weights
        let w = &self.config.weights;
        breakdown.overall = w.purpose_vector * breakdown.purpose_vector
            + w.cross_correlations * breakdown.cross_correlations
            + w.group_alignments * breakdown.group_alignments
            + w.confidence * (a.confidence.min(b.confidence));

        breakdown
    }

    /// Cosine similarity across all components based on scope.
    fn cosine_similarity(&self, a: &TeleologicalVector, b: &TeleologicalVector) -> f32 {
        match &self.config.scope {
            ComparisonScope::Full => {
                let w = &self.config.weights;
                let pv_sim = self.compute_purpose_similarity(a, b);
                let cc_sim = self.compute_correlation_similarity(a, b);
                let ga_sim = a.group_alignments.similarity(&b.group_alignments);
                let conf_sim = a.confidence.min(b.confidence);

                w.purpose_vector * pv_sim
                    + w.cross_correlations * cc_sim
                    + w.group_alignments * ga_sim
                    + w.confidence * conf_sim
            }
            ComparisonScope::PurposeVectorOnly => self.compute_purpose_similarity(a, b),
            ComparisonScope::CrossCorrelationsOnly => self.compute_correlation_similarity(a, b),
            ComparisonScope::GroupAlignmentsOnly => {
                a.group_alignments.similarity(&b.group_alignments)
            }
            ComparisonScope::SpecificPairs(pairs) => {
                self.compute_specific_pairs_similarity(a, b, pairs)
            }
            ComparisonScope::SpecificGroups(groups) => {
                self.compute_specific_groups_similarity(a, b, groups)
            }
            ComparisonScope::SingleEmbedderPattern(embedder_idx) => {
                self.compute_single_embedder_pattern_similarity(a, b, *embedder_idx)
            }
        }
    }

    /// Euclidean distance converted to similarity [0, 1].
    fn euclidean_similarity(&self, a: &TeleologicalVector, b: &TeleologicalVector) -> f32 {
        let mut sum_sq = 0.0f32;

        // Purpose vector distance
        for (&av, &bv) in a
            .purpose_vector
            .alignments
            .iter()
            .zip(b.purpose_vector.alignments.iter())
        {
            let diff = av - bv;
            sum_sq += diff * diff;
        }

        // Cross-correlation distance
        for (&av, &bv) in a.cross_correlations.iter().zip(b.cross_correlations.iter()) {
            let diff = av - bv;
            sum_sq += diff * diff;
        }

        // Group alignment distance
        let ga = a.group_alignments.as_array();
        let gb = b.group_alignments.as_array();
        for (&av, &bv) in ga.iter().zip(gb.iter()) {
            let diff = av - bv;
            sum_sq += diff * diff;
        }

        // Convert distance to similarity: 1 / (1 + sqrt(distance))
        1.0 / (1.0 + sum_sq.sqrt())
    }

    /// Synergy-weighted similarity using the synergy matrix.
    fn synergy_weighted_similarity(&self, a: &TeleologicalVector, b: &TeleologicalVector) -> f32 {
        let synergy = match &self.config.synergy_matrix {
            Some(s) => s,
            None => return self.cosine_similarity(a, b), // Fall back to cosine
        };

        // Weight each cross-correlation by its synergy value
        let mut weighted_dot = 0.0f32;
        let mut weighted_norm_a = 0.0f32;
        let mut weighted_norm_b = 0.0f32;

        for (flat_idx, (&av, &bv)) in a
            .cross_correlations
            .iter()
            .zip(b.cross_correlations.iter())
            .enumerate()
        {
            let (i, j) = SynergyMatrix::flat_to_indices(flat_idx);
            let weight = synergy.get_weighted_synergy(i, j);

            weighted_dot += weight * av * bv;
            weighted_norm_a += weight * av * av;
            weighted_norm_b += weight * bv * bv;
        }

        let corr_sim = if weighted_norm_a > f32::EPSILON && weighted_norm_b > f32::EPSILON {
            weighted_dot / (weighted_norm_a.sqrt() * weighted_norm_b.sqrt())
        } else {
            0.0
        };

        // Combine with purpose vector similarity
        let pv_sim = self.compute_purpose_similarity(a, b);

        0.4 * pv_sim + 0.6 * corr_sim
    }

    /// Group-hierarchical similarity (compare at group level).
    fn group_hierarchical_similarity(&self, a: &TeleologicalVector, b: &TeleologicalVector) -> f32 {
        // First compute group-level similarity
        let group_sim = a.group_alignments.similarity(&b.group_alignments);

        // Then compare within-group correlation patterns
        let mut within_group_sim = 0.0f32;
        let mut group_count = 0;

        for group in GroupType::ALL {
            let indices = group.embedding_indices();
            if indices.len() < 2 {
                continue;
            }

            // For each pair within the group
            let mut group_corr_sim = 0.0f32;
            let mut pair_count = 0;

            for (k, &idx_k) in indices.iter().enumerate() {
                for &idx_l in indices.iter().skip(k + 1) {
                    let (lo, hi) = if idx_k < idx_l {
                        (idx_k, idx_l)
                    } else {
                        (idx_l, idx_k)
                    };

                    let av = a.get_correlation(lo, hi);
                    let bv = b.get_correlation(lo, hi);

                    // Product similarity
                    group_corr_sim += 1.0 - (av - bv).abs();
                    pair_count += 1;
                }
            }

            if pair_count > 0 {
                within_group_sim += group_corr_sim / pair_count as f32;
                group_count += 1;
            }
        }

        let within_sim = if group_count > 0 {
            within_group_sim / group_count as f32
        } else {
            1.0
        };

        0.6 * group_sim + 0.4 * within_sim
    }

    /// Cross-correlation dominant similarity (prioritize 78 pairs).
    fn cross_correlation_similarity(&self, a: &TeleologicalVector, b: &TeleologicalVector) -> f32 {
        let corr_sim = self.compute_correlation_similarity(a, b);
        let pv_sim = self.compute_purpose_similarity(a, b);

        // Heavy weight on correlations
        0.75 * corr_sim + 0.25 * pv_sim
    }

    /// Tucker compressed similarity (if available).
    fn tucker_similarity(&self, a: &TeleologicalVector, b: &TeleologicalVector) -> f32 {
        match (&a.tucker_core, &b.tucker_core) {
            (Some(ta), Some(tb)) => {
                // Compare Tucker core tensors
                if ta.ranks != tb.ranks {
                    // Different ranks - fall back to regular similarity
                    return self.cosine_similarity(a, b);
                }

                let mut dot = 0.0f32;
                let mut norm_a = 0.0f32;
                let mut norm_b = 0.0f32;

                for (&av, &bv) in ta.data.iter().zip(tb.data.iter()) {
                    dot += av * bv;
                    norm_a += av * av;
                    norm_b += bv * bv;
                }

                if norm_a > f32::EPSILON && norm_b > f32::EPSILON {
                    dot / (norm_a.sqrt() * norm_b.sqrt())
                } else {
                    0.0
                }
            }
            _ => self.cosine_similarity(a, b), // Fall back if Tucker not available
        }
    }

    /// Adaptive similarity: choose best strategy based on vector characteristics.
    fn adaptive_similarity(&self, a: &TeleologicalVector, b: &TeleologicalVector) -> f32 {
        // Analyze vector characteristics
        let a_density = a.correlation_density();
        let b_density = b.correlation_density();
        let avg_density = (a_density + b_density) / 2.0;

        let a_coherence = a.group_alignments.coherence();
        let b_coherence = b.group_alignments.coherence();
        let avg_coherence = (a_coherence + b_coherence) / 2.0;

        // Choose strategy based on characteristics
        if a.has_tucker_core() && b.has_tucker_core() {
            // Use Tucker if available
            self.tucker_similarity(a, b)
        } else if avg_density < 0.3 {
            // Sparse correlations - use purpose vector
            self.compute_purpose_similarity(a, b)
        } else if avg_coherence > 0.8 {
            // High coherence - use group hierarchical
            self.group_hierarchical_similarity(a, b)
        } else {
            // Default: full weighted similarity
            self.cosine_similarity(a, b)
        }
    }

    /// Compute purpose vector cosine similarity.
    fn compute_purpose_similarity(&self, a: &TeleologicalVector, b: &TeleologicalVector) -> f32 {
        a.purpose_vector.similarity(&b.purpose_vector)
    }

    /// Compute cross-correlation cosine similarity.
    fn compute_correlation_similarity(
        &self,
        a: &TeleologicalVector,
        b: &TeleologicalVector,
    ) -> f32 {
        let mut dot = 0.0f32;
        let mut norm_a = 0.0f32;
        let mut norm_b = 0.0f32;

        for (&av, &bv) in a.cross_correlations.iter().zip(b.cross_correlations.iter()) {
            dot += av * bv;
            norm_a += av * av;
            norm_b += bv * bv;
        }

        if norm_a > f32::EPSILON && norm_b > f32::EPSILON {
            dot / (norm_a.sqrt() * norm_b.sqrt())
        } else {
            0.0
        }
    }

    /// Compute similarity for specific embedding pairs.
    fn compute_specific_pairs_similarity(
        &self,
        a: &TeleologicalVector,
        b: &TeleologicalVector,
        pairs: &[(usize, usize)],
    ) -> f32 {
        if pairs.is_empty() {
            return 0.0;
        }

        let mut sum_sim = 0.0f32;

        for &(i, j) in pairs {
            let (lo, hi) = if i < j { (i, j) } else { (j, i) };
            let av = a.get_correlation(lo, hi);
            let bv = b.get_correlation(lo, hi);
            // Absolute difference similarity
            sum_sim += 1.0 - (av - bv).abs();
        }

        sum_sim / pairs.len() as f32
    }

    /// Compute similarity for specific embedding groups.
    fn compute_specific_groups_similarity(
        &self,
        a: &TeleologicalVector,
        b: &TeleologicalVector,
        groups: &[GroupType],
    ) -> f32 {
        if groups.is_empty() {
            return 0.0;
        }

        let mut sum_sim = 0.0f32;

        for &group in groups {
            let ga = a.group_alignments.get(group);
            let gb = b.group_alignments.get(group);
            sum_sim += 1.0 - (ga - gb).abs();
        }

        sum_sim / groups.len() as f32
    }

    /// Compute similarity for a single embedder's correlation pattern.
    ///
    /// Compares all 12 cross-correlations that involve the specified embedder.
    fn compute_single_embedder_pattern_similarity(
        &self,
        a: &TeleologicalVector,
        b: &TeleologicalVector,
        embedder_idx: usize,
    ) -> f32 {
        assert!(
            embedder_idx < NUM_EMBEDDERS,
            "FAIL FAST: embedder index {} out of bounds",
            embedder_idx
        );

        let mut sum_sim = 0.0f32;
        let mut count = 0;

        // All pairs involving this embedder
        for other in 0..NUM_EMBEDDERS {
            if other == embedder_idx {
                continue;
            }

            let (lo, hi) = if embedder_idx < other {
                (embedder_idx, other)
            } else {
                (other, embedder_idx)
            };

            let av = a.get_correlation(lo, hi);
            let bv = b.get_correlation(lo, hi);
            sum_sim += 1.0 - (av - bv).abs();
            count += 1;
        }

        if count > 0 {
            sum_sim / count as f32
        } else {
            0.0
        }
    }

    /// Search for similar vectors in a collection.
    ///
    /// Returns vector of (index, similarity) sorted by descending similarity.
    pub fn search(
        &self,
        query: &TeleologicalVector,
        candidates: &[TeleologicalVector],
    ) -> Vec<(usize, f32)> {
        let mut results: Vec<(usize, f32)> = candidates
            .iter()
            .enumerate()
            .map(|(idx, candidate)| (idx, self.similarity(query, candidate)))
            .filter(|(_, sim)| *sim >= self.config.min_similarity)
            .collect();

        // Sort by similarity descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Limit results
        results.truncate(self.config.max_results);

        results
    }

    /// Search with full breakdown for each result.
    pub fn search_with_breakdown(
        &self,
        query: &TeleologicalVector,
        candidates: &[TeleologicalVector],
    ) -> Vec<(usize, SimilarityBreakdown)> {
        let mut results: Vec<(usize, SimilarityBreakdown)> = candidates
            .iter()
            .enumerate()
            .map(|(idx, candidate)| (idx, self.similarity_with_breakdown(query, candidate)))
            .filter(|(_, breakdown)| breakdown.overall >= self.config.min_similarity)
            .collect();

        // Sort by overall similarity descending
        results.sort_by(|a, b| {
            b.1.overall
                .partial_cmp(&a.1.overall)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Limit results
        results.truncate(self.config.max_results);

        results
    }

    /// Compute pairwise similarity matrix for a collection.
    ///
    /// Returns N×N matrix where [i][j] = similarity(vectors[i], vectors[j]).
    #[allow(clippy::needless_range_loop)]
    pub fn pairwise_similarity_matrix(&self, vectors: &[TeleologicalVector]) -> Vec<Vec<f32>> {
        let n = vectors.len();
        let mut matrix = vec![vec![0.0f32; n]; n];

        for i in 0..n {
            matrix[i][i] = 1.0; // Self-similarity
            for j in (i + 1)..n {
                let sim = self.similarity(&vectors[i], &vectors[j]);
                matrix[i][j] = sim;
                matrix[j][i] = sim; // Symmetric
            }
        }

        matrix
    }

    /// Find clusters of similar vectors.
    ///
    /// Returns groups of vector indices that are mutually similar (above threshold).
    #[allow(clippy::needless_range_loop)]
    pub fn find_clusters(
        &self,
        vectors: &[TeleologicalVector],
        similarity_threshold: f32,
    ) -> Vec<Vec<usize>> {
        let n = vectors.len();
        let sim_matrix = self.pairwise_similarity_matrix(vectors);

        let mut visited = vec![false; n];
        let mut clusters = Vec::new();

        for i in 0..n {
            if visited[i] {
                continue;
            }

            // Start new cluster with this vector
            let mut cluster = vec![i];
            visited[i] = true;

            // Find all vectors similar to any in the cluster
            let mut frontier = vec![i];
            while let Some(current) = frontier.pop() {
                for (j, v) in visited.iter_mut().enumerate() {
                    if !*v && sim_matrix[current][j] >= similarity_threshold {
                        *v = true;
                        cluster.push(j);
                        frontier.push(j);
                    }
                }
            }

            clusters.push(cluster);
        }

        clusters
    }

    /// Compute centroid of a set of teleological vectors.
    pub fn compute_centroid(&self, vectors: &[TeleologicalVector]) -> TeleologicalVector {
        if vectors.is_empty() {
            return TeleologicalVector::default();
        }

        let n = vectors.len() as f32;

        // Average purpose vectors
        let mut avg_alignments = [0.0f32; NUM_EMBEDDERS];
        for v in vectors {
            for (avg, &val) in avg_alignments
                .iter_mut()
                .zip(v.purpose_vector.alignments.iter())
            {
                *avg += val;
            }
        }
        for avg in avg_alignments.iter_mut() {
            *avg /= n;
        }

        // Average cross-correlations
        let mut avg_correlations = vec![0.0f32; CROSS_CORRELATION_COUNT];
        for v in vectors {
            for (avg, &val) in avg_correlations.iter_mut().zip(v.cross_correlations.iter()) {
                *avg += val;
            }
        }
        for avg in avg_correlations.iter_mut() {
            *avg /= n;
        }

        // Average group alignments
        let mut avg_groups = [0.0f32; NUM_GROUPS];
        for v in vectors {
            let ga = v.group_alignments.as_array();
            for (avg, &val) in avg_groups.iter_mut().zip(ga.iter()) {
                *avg += val;
            }
        }
        for avg in avg_groups.iter_mut() {
            *avg /= n;
        }

        // Average confidence
        let avg_confidence: f32 = vectors.iter().map(|v| v.confidence).sum::<f32>() / n;

        use crate::types::fingerprint::PurposeVector;
        TeleologicalVector::with_all(
            PurposeVector::new(avg_alignments),
            avg_correlations,
            GroupAlignments::from_array(avg_groups),
            avg_confidence,
        )
    }

    /// Compare two vectors across ALL comparison scopes and return comprehensive analysis.
    pub fn comprehensive_comparison(
        &self,
        a: &TeleologicalVector,
        b: &TeleologicalVector,
    ) -> ComprehensiveComparison {
        // Full comparison
        let full = self.similarity_with_breakdown(a, b);

        // Purpose vector only
        let pv_config = MatrixSearchConfig {
            scope: ComparisonScope::PurposeVectorOnly,
            ..Default::default()
        };
        let pv_search = TeleologicalMatrixSearch::with_config(pv_config);
        let purpose_only = pv_search.similarity(a, b);

        // Cross-correlations only
        let cc_config = MatrixSearchConfig {
            scope: ComparisonScope::CrossCorrelationsOnly,
            ..Default::default()
        };
        let cc_search = TeleologicalMatrixSearch::with_config(cc_config);
        let correlations_only = cc_search.similarity(a, b);

        // Group alignments only
        let ga_config = MatrixSearchConfig {
            scope: ComparisonScope::GroupAlignmentsOnly,
            ..Default::default()
        };
        let ga_search = TeleologicalMatrixSearch::with_config(ga_config);
        let groups_only = ga_search.similarity(a, b);

        // Per-group comparisons
        let mut per_group = HashMap::new();
        for group in GroupType::ALL {
            let group_config = MatrixSearchConfig {
                scope: ComparisonScope::SpecificGroups(vec![group]),
                ..Default::default()
            };
            let group_search = TeleologicalMatrixSearch::with_config(group_config);
            per_group.insert(group, group_search.similarity(a, b));
        }

        // Per-embedder pattern comparisons
        let mut per_embedder_pattern = [0.0; NUM_EMBEDDERS];
        for (embedder, pattern) in per_embedder_pattern.iter_mut().enumerate() {
            let emb_config = MatrixSearchConfig {
                scope: ComparisonScope::SingleEmbedderPattern(embedder),
                ..Default::default()
            };
            let emb_search = TeleologicalMatrixSearch::with_config(emb_config);
            *pattern = emb_search.similarity(a, b);
        }

        // Tucker comparison (if available)
        let tucker = if a.has_tucker_core() && b.has_tucker_core() {
            let tucker_config = MatrixSearchConfig {
                strategy: SearchStrategy::TuckerCompressed,
                ..Default::default()
            };
            let tucker_search = TeleologicalMatrixSearch::with_config(tucker_config);
            Some(tucker_search.similarity(a, b))
        } else {
            None
        };

        ComprehensiveComparison {
            full,
            purpose_only,
            correlations_only,
            groups_only,
            per_group,
            per_embedder_pattern,
            tucker,
        }
    }
}

impl Default for TeleologicalMatrixSearch {
    fn default() -> Self {
        Self::new()
    }
}

/// Comprehensive comparison result across all scopes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComprehensiveComparison {
    /// Full comparison with breakdown
    pub full: SimilarityBreakdown,
    /// Purpose vector only similarity
    pub purpose_only: f32,
    /// Cross-correlations only similarity
    pub correlations_only: f32,
    /// Group alignments only similarity
    pub groups_only: f32,
    /// Per-group similarity
    pub per_group: HashMap<GroupType, f32>,
    /// Per-embedder correlation pattern similarity
    pub per_embedder_pattern: [f32; NUM_EMBEDDERS],
    /// Tucker compressed similarity (if available)
    pub tucker: Option<f32>,
}

impl Default for ComprehensiveComparison {
    fn default() -> Self {
        Self {
            full: SimilarityBreakdown::default(),
            purpose_only: 0.0,
            correlations_only: 0.0,
            groups_only: 0.0,
            per_group: HashMap::new(),
            per_embedder_pattern: [0.0; NUM_EMBEDDERS],
            tucker: None,
        }
    }
}

/// Embedder name constants for reference.
pub mod embedder_names {
    pub const E1_SEMANTIC: usize = 0;
    pub const E2_EPISODIC: usize = 1;
    pub const E3_TEMPORAL: usize = 2;
    pub const E4_CAUSAL: usize = 3;
    pub const E5_ANALOGICAL: usize = 4;
    pub const E6_CODE: usize = 5;
    pub const E7_PROCEDURAL: usize = 6;
    pub const E8_SPATIAL: usize = 7;
    pub const E9_SOCIAL: usize = 8;
    pub const E10_EMOTIONAL: usize = 9;
    pub const E11_ABSTRACT: usize = 10;
    pub const E12_FACTUAL: usize = 11;
    pub const E13_SPARSE: usize = 12;

    pub const ALL_NAMES: [&str; 13] = [
        "E1_Semantic",
        "E2_Episodic",
        "E3_Temporal",
        "E4_Causal",
        "E5_Analogical",
        "E6_Code",
        "E7_Procedural",
        "E8_Spatial",
        "E9_Social",
        "E10_Emotional",
        "E11_Abstract",
        "E12_Factual",
        "E13_Sparse",
    ];

    pub fn name(idx: usize) -> &'static str {
        if idx < 13 {
            ALL_NAMES[idx]
        } else {
            "Unknown"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::fingerprint::PurposeVector;

    fn make_test_vector(purpose_val: f32, corr_val: f32) -> TeleologicalVector {
        let pv = PurposeVector::new([purpose_val; NUM_EMBEDDERS]);
        let mut tv = TeleologicalVector::new(pv);
        for corr in tv.cross_correlations.iter_mut() {
            *corr = corr_val;
        }
        tv.group_alignments = GroupAlignments::new(
            purpose_val,
            purpose_val,
            purpose_val,
            purpose_val,
            purpose_val,
            purpose_val,
        );
        tv.confidence = 1.0; // Use 1.0 for test consistency
        tv
    }

    fn make_varied_test_vector(seed: u32) -> TeleologicalVector {
        // Create a vector with truly varied values based on seed
        // Use a simple pseudo-random approach
        let mut alignments = [0.0f32; NUM_EMBEDDERS];
        let mut state = seed;
        for alignment in alignments.iter_mut() {
            // Simple LCG for pseudo-random
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            *alignment = (state as f32 / u32::MAX as f32).max(0.05);
        }
        let pv = PurposeVector::new(alignments);
        let mut tv = TeleologicalVector::new(pv);
        for corr in tv.cross_correlations.iter_mut() {
            state = state.wrapping_mul(1103515245).wrapping_add(12345);
            *corr = (state as f32 / u32::MAX as f32).max(0.05);
        }
        tv.group_alignments = GroupAlignments::from_alignments(&alignments, None);
        tv.confidence = 1.0;
        tv
    }

    #[test]
    fn test_matrix_search_identical_vectors() {
        let search = TeleologicalMatrixSearch::new();
        let tv = make_test_vector(0.8, 0.6);

        let sim = search.similarity(&tv, &tv);
        assert!(
            (sim - 1.0).abs() < 0.02,
            "Self-similarity should be ~1.0, got {}",
            sim
        );

        println!("[PASS] Identical vectors have similarity ~1.0: {}", sim);
    }

    #[test]
    fn test_matrix_search_different_vectors() {
        let search = TeleologicalMatrixSearch::new();
        // Use varied vectors with different random seeds
        let tv1 = make_varied_test_vector(12345);
        let tv2 = make_varied_test_vector(98765);

        let sim = search.similarity(&tv1, &tv2);
        assert!(
            sim < 0.99,
            "Different vectors should have lower similarity, got {}",
            sim
        );

        println!("[PASS] Different vectors have lower similarity: {}", sim);
    }

    #[test]
    fn test_matrix_search_with_breakdown() {
        let search = TeleologicalMatrixSearch::new();
        let tv1 = make_test_vector(0.7, 0.5);
        let tv2 = make_test_vector(0.6, 0.4);

        let breakdown = search.similarity_with_breakdown(&tv1, &tv2);

        assert!(breakdown.overall > 0.0);
        assert!(breakdown.purpose_vector > 0.0);
        assert!(breakdown.cross_correlations > 0.0);
        assert!(breakdown.group_alignments > 0.0);
        assert!(!breakdown.per_group.is_empty());
        assert!(!breakdown.top_correlation_pairs.is_empty());

        println!("[PASS] Breakdown contains all components");
        println!("  overall: {:.4}", breakdown.overall);
        println!("  purpose_vector: {:.4}", breakdown.purpose_vector);
        println!("  cross_correlations: {:.4}", breakdown.cross_correlations);
        println!("  group_alignments: {:.4}", breakdown.group_alignments);
    }

    #[test]
    fn test_matrix_search_correlation_focused() {
        let config = MatrixSearchConfig::correlation_focused();
        let search = TeleologicalMatrixSearch::with_config(config);

        let tv1 = make_test_vector(0.5, 0.9);
        let tv2 = make_test_vector(0.5, 0.9);

        let sim = search.similarity(&tv1, &tv2);
        assert!(
            sim > 0.9,
            "Same correlations should have high similarity, got {}",
            sim
        );

        println!("[PASS] Correlation-focused search works");
    }

    #[test]
    fn test_matrix_search_group_hierarchical() {
        let config = MatrixSearchConfig::group_hierarchical();
        let search = TeleologicalMatrixSearch::with_config(config);

        let tv1 = make_test_vector(0.8, 0.5);
        let tv2 = make_test_vector(0.8, 0.5);

        let sim = search.similarity(&tv1, &tv2);
        assert!(sim > 0.9, "Same groups should have high similarity");

        println!("[PASS] Group hierarchical search works");
    }

    #[test]
    fn test_matrix_search_synergy_weighted() {
        let synergy = SynergyMatrix::with_base_synergies();
        let config = MatrixSearchConfig::with_synergy(synergy);
        let search = TeleologicalMatrixSearch::with_config(config);

        let tv1 = make_test_vector(0.7, 0.6);
        let tv2 = make_test_vector(0.7, 0.6);

        let sim = search.similarity(&tv1, &tv2);
        assert!(
            sim > 0.9,
            "Synergy-weighted similarity should be high for same vectors"
        );

        println!("[PASS] Synergy-weighted search works");
    }

    #[test]
    fn test_matrix_search_euclidean() {
        let config = MatrixSearchConfig {
            strategy: SearchStrategy::Euclidean,
            ..Default::default()
        };
        let search = TeleologicalMatrixSearch::with_config(config);

        let tv1 = make_test_vector(0.8, 0.6);
        let tv2 = make_test_vector(0.8, 0.6);

        let sim = search.similarity(&tv1, &tv2);
        assert!(
            (sim - 1.0).abs() < 0.01,
            "Identical vectors should have Euclidean similarity ~1.0"
        );

        println!("[PASS] Euclidean similarity works");
    }

    #[test]
    fn test_matrix_search_specific_pairs() {
        let config = MatrixSearchConfig {
            scope: ComparisonScope::SpecificPairs(vec![(0, 1), (0, 2), (1, 2)]),
            ..Default::default()
        };
        let search = TeleologicalMatrixSearch::with_config(config);

        let tv1 = make_test_vector(0.7, 0.5);
        let tv2 = make_test_vector(0.3, 0.5);

        let sim = search.similarity(&tv1, &tv2);
        assert!(
            (sim - 1.0).abs() < 0.01,
            "Same correlations should have similarity ~1.0, got {}",
            sim
        );

        println!("[PASS] Specific pairs comparison works");
    }

    #[test]
    fn test_matrix_search_single_embedder_pattern() {
        let config = MatrixSearchConfig {
            scope: ComparisonScope::SingleEmbedderPattern(0), // E1_Semantic
            ..Default::default()
        };
        let search = TeleologicalMatrixSearch::with_config(config);

        let tv = make_test_vector(0.7, 0.5);
        let sim = search.similarity(&tv, &tv);
        assert!(
            (sim - 1.0).abs() < 0.01,
            "Self similarity for embedder pattern should be ~1.0"
        );

        println!("[PASS] Single embedder pattern comparison works");
    }

    #[test]
    fn test_matrix_search_collection() {
        let search = TeleologicalMatrixSearch::new();
        let query = make_test_vector(0.8, 0.7);

        let candidates = vec![
            make_test_vector(0.8, 0.7),   // Identical - highest
            make_varied_test_vector(200), // Different pattern
            make_varied_test_vector(500), // Different pattern
        ];

        let results = search.search(&query, &candidates);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, 0); // Identical is most similar
                                     // Results are sorted by similarity descending
        assert!(
            results[0].1 >= results[1].1,
            "First result should have highest similarity"
        );

        println!("[PASS] Collection search returns sorted results");
    }

    #[test]
    fn test_matrix_search_with_threshold() {
        let config = MatrixSearchConfig {
            min_similarity: 0.5,
            ..Default::default()
        };
        let search = TeleologicalMatrixSearch::with_config(config);

        let query = make_test_vector(0.8, 0.7);
        let candidates = vec![
            make_test_vector(0.8, 0.7), // High similarity
            make_test_vector(0.1, 0.1), // Low similarity
        ];

        let results = search.search(&query, &candidates);

        assert!(!results.is_empty());
        for (_, sim) in &results {
            assert!(*sim >= 0.5, "All results should be above threshold");
        }

        println!("[PASS] Threshold filtering works");
    }

    #[test]
    fn test_pairwise_similarity_matrix() {
        let search = TeleologicalMatrixSearch::new();
        let vectors = vec![
            make_test_vector(0.8, 0.6),
            make_test_vector(0.7, 0.5),
            make_test_vector(0.6, 0.4),
        ];

        let matrix = search.pairwise_similarity_matrix(&vectors);

        assert_eq!(matrix.len(), 3);
        assert_eq!(matrix[0].len(), 3);

        // Diagonal should be 1.0
        for (i, row) in matrix.iter().enumerate() {
            assert!((row[i] - 1.0).abs() < 0.01);
        }

        // Should be symmetric
        for (i, row_i) in matrix.iter().enumerate() {
            for (j, &val) in row_i.iter().enumerate() {
                assert!((val - matrix[j][i]).abs() < 0.001);
            }
        }

        println!("[PASS] Pairwise similarity matrix is correct");
    }

    #[test]
    fn test_find_clusters() {
        let search = TeleologicalMatrixSearch::new();
        let vectors = vec![
            make_test_vector(0.9, 0.8),   // Very similar to each other
            make_test_vector(0.9, 0.8),   // Same as above (identical cluster)
            make_varied_test_vector(100), // Different pattern
            make_varied_test_vector(500), // Yet another different pattern
        ];

        // With a very high threshold, very similar vectors should cluster together
        let clusters = search.find_clusters(&vectors, 0.99);

        // At minimum, we should get some clusters
        assert!(!clusters.is_empty(), "Should find at least 1 cluster");

        println!("[PASS] Clustering works, found {} clusters", clusters.len());
    }

    #[test]
    fn test_compute_centroid() {
        let search = TeleologicalMatrixSearch::new();
        let vectors = vec![make_test_vector(0.8, 0.6), make_test_vector(0.6, 0.4)];

        let centroid = search.compute_centroid(&vectors);

        // Centroid should be average
        assert!((centroid.purpose_vector.alignments[0] - 0.7).abs() < 0.01);
        assert!((centroid.cross_correlations[0] - 0.5).abs() < 0.01);

        println!("[PASS] Centroid computation works");
    }

    #[test]
    fn test_comprehensive_comparison() {
        let search = TeleologicalMatrixSearch::new();
        let tv1 = make_test_vector(0.8, 0.6);
        let tv2 = make_test_vector(0.7, 0.5);

        let comp = search.comprehensive_comparison(&tv1, &tv2);

        assert!(comp.full.overall > 0.0);
        assert!(comp.purpose_only > 0.0);
        assert!(comp.correlations_only > 0.0);
        assert!(comp.groups_only > 0.0);
        assert!(!comp.per_group.is_empty());
        assert!(comp.per_embedder_pattern.iter().all(|&v| v > 0.0));

        println!("[PASS] Comprehensive comparison returns all metrics");
    }

    #[test]
    fn test_adaptive_strategy() {
        let config = MatrixSearchConfig::adaptive();
        let search = TeleologicalMatrixSearch::with_config(config);

        let tv1 = make_test_vector(0.8, 0.6);
        let tv2 = make_test_vector(0.7, 0.5);

        let sim = search.similarity(&tv1, &tv2);
        assert!(
            sim > 0.0,
            "Adaptive strategy should produce valid similarity"
        );

        println!("[PASS] Adaptive strategy works");
    }

    #[test]
    fn test_component_weights_validation() {
        use crate::teleological::comparison_error::ComparisonValidationError;

        let mut weights = ComponentWeights::default();
        assert!(
            weights.validate().is_ok(),
            "Default weights should sum to 1.0"
        );
        assert!(weights.is_valid(), "Default weights should be valid");

        weights.purpose_vector = 0.5;
        let err = weights.validate();
        assert!(err.is_err(), "Modified weights should not sum to 1.0");

        // Verify error type is WeightsNotNormalized
        match err {
            Err(ComparisonValidationError::WeightsNotNormalized { actual_sum, .. }) => {
                assert!((actual_sum - 1.1).abs() < 0.01, "Sum should be ~1.1");
                println!("  Got expected error with sum: {}", actual_sum);
            }
            _ => panic!("Expected WeightsNotNormalized error"),
        }

        weights.normalize();
        assert!(
            weights.validate().is_ok(),
            "Normalized weights should sum to 1.0"
        );
        assert!(weights.is_valid(), "Normalized weights should be valid");

        // Test out-of-range weight
        let bad_weights = ComponentWeights {
            confidence: -0.5,
            ..Default::default()
        };
        let range_err = bad_weights.validate();
        assert!(range_err.is_err(), "Negative weight should fail validation");
        match range_err {
            Err(ComparisonValidationError::WeightOutOfRange {
                field_name, value, ..
            }) => {
                assert_eq!(field_name, "confidence");
                assert!((value - (-0.5)).abs() < f32::EPSILON);
                println!("  Got expected range error for {}: {}", field_name, value);
            }
            _ => panic!("Expected WeightOutOfRange error"),
        }

        println!("[PASS] Component weights validation works");
    }

    #[test]
    fn test_embedder_names() {
        assert_eq!(embedder_names::name(0), "E1_Semantic");
        assert_eq!(embedder_names::name(5), "E6_Code");
        assert_eq!(embedder_names::name(12), "E13_Sparse");
        assert_eq!(embedder_names::name(99), "Unknown");

        println!("[PASS] Embedder names work correctly");
    }
}
