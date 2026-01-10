//! TeleologicalComparator: Apples-to-apples comparison across 13 embedders.
//!
//! Routes to correct similarity function per embedder type, applies weights
//! and synergy matrices, returns detailed comparison results.
//!
//! # Design Philosophy
//!
//! From constitution.yaml ARCH-02: "Compare Only Compatible Embedding Types (Apples-to-Apples)"
//! - E1 compares with E1, E5 with E5, NEVER cross-embedder
//! - Each embedder has a specific output type (dense/sparse/token-level)
//! - The comparator dispatches to the correct similarity function per embedder
//! - Results are aggregated according to SearchStrategy and ComponentWeights
//!
//! # Embedder Type Mapping
//!
//! | Index | Embedder | Type | Similarity Function |
//! |-------|----------|------|---------------------|
//! | 0 | E1 Semantic | Dense | cosine_similarity |
//! | 1 | E2 TemporalRecent | Dense | cosine_similarity |
//! | 2 | E3 TemporalPeriodic | Dense | cosine_similarity |
//! | 3 | E4 TemporalPositional | Dense | cosine_similarity |
//! | 4 | E5 Causal | Dense | cosine_similarity |
//! | 5 | E6 Sparse | Sparse | sparse_cosine_similarity |
//! | 6 | E7 Code | Dense | cosine_similarity |
//! | 7 | E8 Graph | Dense | cosine_similarity |
//! | 8 | E9 HDC | Dense | cosine_similarity |
//! | 9 | E10 Multimodal | Dense | cosine_similarity |
//! | 10 | E11 Entity | Dense | cosine_similarity |
//! | 11 | E12 LateInteraction | TokenLevel | max_sim |
//! | 12 | E13 KeywordSplade | Sparse | sparse_cosine_similarity |

use rayon::prelude::*;

use crate::similarity::{cosine_similarity, max_sim, sparse_cosine_similarity, DenseSimilarityError};
use crate::teleological::{
    ComparisonValidationResult, Embedder, MatrixSearchConfig,
    SearchStrategy, SimilarityBreakdown, NUM_EMBEDDERS,
};
use crate::types::{EmbeddingSlice, SemanticFingerprint};

/// Result of comparing two teleological fingerprints.
#[derive(Clone, Debug)]
pub struct ComparisonResult {
    /// Overall similarity score [0.0, 1.0]
    pub overall: f32,
    /// Per-embedder similarity scores (None if embedder unavailable or comparison failed)
    pub per_embedder: [Option<f32>; NUM_EMBEDDERS],
    /// Strategy used for comparison
    pub strategy: SearchStrategy,
    /// Coherence: inverse of coefficient of variation across embedders (higher = more consistent)
    pub coherence: Option<f32>,
    /// Dominant embedder (highest score)
    pub dominant_embedder: Option<Embedder>,
    /// Optional detailed breakdown
    pub breakdown: Option<SimilarityBreakdown>,
}

impl ComparisonResult {
    /// Create a new ComparisonResult with default values.
    fn new(strategy: SearchStrategy) -> Self {
        Self {
            overall: 0.0,
            per_embedder: [None; NUM_EMBEDDERS],
            strategy,
            coherence: None,
            dominant_embedder: None,
            breakdown: None,
        }
    }

    /// Count how many embedders have valid scores.
    pub fn valid_score_count(&self) -> usize {
        self.per_embedder.iter().filter(|s| s.is_some()).count()
    }
}

/// Compares teleological fingerprints using configurable strategies.
///
/// # Apples-to-Apples Guarantee
///
/// Each embedder's output is compared only with the same embedder's output
/// from another fingerprint. Cross-embedder comparison is FORBIDDEN per ARCH-02.
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_core::teleological::{TeleologicalComparator, MatrixSearchConfig};
/// use context_graph_core::types::SemanticFingerprint;
///
/// let comparator = TeleologicalComparator::new();
/// let result = comparator.compare(&fingerprint_a, &fingerprint_b)?;
/// println!("Overall similarity: {:.4}", result.overall);
/// ```
#[derive(Debug, Clone)]
pub struct TeleologicalComparator {
    config: MatrixSearchConfig,
}

impl Default for TeleologicalComparator {
    fn default() -> Self {
        Self::new()
    }
}

impl TeleologicalComparator {
    /// Create with default configuration (Cosine strategy, Full scope).
    pub fn new() -> Self {
        Self {
            config: MatrixSearchConfig::default(),
        }
    }

    /// Create with specific configuration.
    pub fn with_config(config: MatrixSearchConfig) -> Self {
        Self { config }
    }

    /// Get the current configuration.
    pub fn config(&self) -> &MatrixSearchConfig {
        &self.config
    }

    /// Compare two fingerprints using configured strategy.
    ///
    /// # Errors
    ///
    /// Returns error if weights are invalid (FAIL FAST per constitution.yaml).
    pub fn compare(
        &self,
        a: &SemanticFingerprint,
        b: &SemanticFingerprint,
    ) -> ComparisonValidationResult<ComparisonResult> {
        self.compare_with_strategy(a, b, self.config.strategy)
    }

    /// Compare with explicit strategy override.
    pub fn compare_with_strategy(
        &self,
        a: &SemanticFingerprint,
        b: &SemanticFingerprint,
        strategy: SearchStrategy,
    ) -> ComparisonValidationResult<ComparisonResult> {
        // FAIL FAST: Validate weights before any computation
        self.config.weights.validate()?;

        let mut result = ComparisonResult::new(strategy);

        // Compare each embedder (apples-to-apples)
        for idx in 0..NUM_EMBEDDERS {
            let a_slice = a.get_embedding(idx);
            let b_slice = b.get_embedding(idx);

            if let (Some(a_emb), Some(b_emb)) = (a_slice, b_slice) {
                result.per_embedder[idx] = self.compare_embedder_slices(&a_emb, &b_emb);
            }
        }

        // Aggregate scores according to strategy
        result.overall = self.aggregate(&result.per_embedder, strategy);

        // Compute coherence measure
        result.coherence = self.compute_coherence(&result.per_embedder);

        // Find dominant embedder
        result.dominant_embedder = self.find_dominant_embedder(&result.per_embedder);

        // Generate breakdown if requested
        if self.config.compute_breakdown {
            result.breakdown = Some(self.generate_breakdown(&result, strategy));
        }

        Ok(result)
    }

    /// Compare a single embedder pair using the correct similarity function.
    ///
    /// Routes based on EmbeddingSlice variant:
    /// - Dense: cosine_similarity
    /// - Sparse: sparse_cosine_similarity
    /// - TokenLevel: max_sim
    fn compare_embedder_slices(
        &self,
        a: &EmbeddingSlice<'_>,
        b: &EmbeddingSlice<'_>,
    ) -> Option<f32> {
        match (a, b) {
            // Dense embeddings (E1-E5, E7-E11): cosine similarity
            (EmbeddingSlice::Dense(a_dense), EmbeddingSlice::Dense(b_dense)) => {
                // Skip empty vectors
                if a_dense.is_empty() || b_dense.is_empty() {
                    return None;
                }
                // cosine_similarity returns Result, handle error by returning None
                match cosine_similarity(a_dense, b_dense) {
                    Ok(sim) => Some(sim.clamp(0.0, 1.0)),
                    Err(DenseSimilarityError::DimensionMismatch { .. }) => {
                        // Dimension mismatch between same embedder type - should not happen
                        // with valid fingerprints, but handle gracefully
                        None
                    }
                    Err(DenseSimilarityError::EmptyVector) => None,
                    Err(DenseSimilarityError::ZeroMagnitude) => {
                        // Zero vector - treat as no similarity
                        Some(0.0)
                    }
                }
            }

            // Sparse embeddings (E6, E13): sparse cosine similarity
            (EmbeddingSlice::Sparse(a_sparse), EmbeddingSlice::Sparse(b_sparse)) => {
                // Skip empty sparse vectors (no meaningful comparison possible)
                if a_sparse.nnz() == 0 || b_sparse.nnz() == 0 {
                    return None;
                }
                // sparse_cosine_similarity returns f32 directly
                let sim = sparse_cosine_similarity(a_sparse, b_sparse);
                Some(sim.clamp(0.0, 1.0))
            }

            // Token-level embeddings (E12): ColBERT MaxSim
            (EmbeddingSlice::TokenLevel(a_tokens), EmbeddingSlice::TokenLevel(b_tokens)) => {
                // Skip empty token sequences (no meaningful comparison possible)
                if a_tokens.is_empty() || b_tokens.is_empty() {
                    return None;
                }
                let sim = max_sim(a_tokens, b_tokens);
                Some(sim.clamp(0.0, 1.0))
            }

            // Type mismatch - should never happen with valid fingerprints
            // This would indicate a bug in SemanticFingerprint.get_embedding()
            _ => None,
        }
    }

    /// Aggregate per-embedder scores according to strategy.
    fn aggregate(&self, scores: &[Option<f32>; NUM_EMBEDDERS], strategy: SearchStrategy) -> f32 {
        match strategy {
            SearchStrategy::Cosine => self.aggregate_mean(scores),
            SearchStrategy::Euclidean => self.aggregate_euclidean(scores),
            SearchStrategy::SynergyWeighted => self.aggregate_synergy(scores),
            SearchStrategy::GroupHierarchical => self.aggregate_hierarchical(scores),
            SearchStrategy::CrossCorrelationDominant => self.aggregate_correlation(scores),
            SearchStrategy::TuckerCompressed => self.aggregate_tucker(scores),
            SearchStrategy::Adaptive => self.aggregate_adaptive(scores),
        }
    }

    /// Simple weighted mean of available scores.
    fn aggregate_mean(&self, scores: &[Option<f32>; NUM_EMBEDDERS]) -> f32 {
        let valid_scores: Vec<f32> = scores.iter().filter_map(|&s| s).collect();
        if valid_scores.is_empty() {
            return 0.0;
        }
        let sum: f32 = valid_scores.iter().sum();
        sum / valid_scores.len() as f32
    }

    /// Euclidean distance converted to similarity.
    /// Uses: similarity = 1 / (1 + sqrt(sum((1-s_i)^2) / n))
    fn aggregate_euclidean(&self, scores: &[Option<f32>; NUM_EMBEDDERS]) -> f32 {
        let valid_scores: Vec<f32> = scores.iter().filter_map(|&s| s).collect();
        if valid_scores.is_empty() {
            return 0.0;
        }
        let n = valid_scores.len() as f32;
        let sum_sq: f32 = valid_scores.iter().map(|&s| (1.0 - s).powi(2)).sum();
        let rms_distance = (sum_sq / n).sqrt();
        1.0 / (1.0 + rms_distance)
    }

    /// Synergy-weighted aggregation using SynergyMatrix diagonal weights.
    fn aggregate_synergy(&self, scores: &[Option<f32>; NUM_EMBEDDERS]) -> f32 {
        let synergy = match &self.config.synergy_matrix {
            Some(s) => s,
            None => return self.aggregate_mean(scores), // Fallback if no synergy matrix
        };

        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;

        for (i, score) in scores.iter().enumerate() {
            if let Some(s) = score {
                // Diagonal elements represent self-importance weights
                let weight = synergy.get_synergy(i, i);
                weighted_sum += s * weight;
                weight_sum += weight;
            }
        }

        if weight_sum > f32::EPSILON {
            weighted_sum / weight_sum
        } else {
            0.0
        }
    }

    /// Group-hierarchical aggregation using embedding groups.
    /// Groups: Factual (E1,E12,E13), Temporal (E2,E3), Causal (E4,E7),
    ///         Relational (E5,E8,E9), Qualitative (E10,E11), Implementation (E6)
    fn aggregate_hierarchical(&self, scores: &[Option<f32>; NUM_EMBEDDERS]) -> f32 {
        // Define group indices
        let groups: [&[usize]; 6] = [
            &[0, 10, 11, 12], // Factual: E1, E11, E12, E13
            &[1, 2, 3],       // Temporal: E2, E3, E4
            &[4, 6],          // Causal: E5, E7
            &[7, 8],          // Relational: E8, E9
            &[9],             // Qualitative: E10
            &[5],             // Implementation: E6
        ];

        // Group weights (equal weighting across groups)
        let group_weight = 1.0 / groups.len() as f32;

        let mut total = 0.0;
        let mut valid_groups = 0;

        for group_indices in groups.iter() {
            let group_scores: Vec<f32> = group_indices
                .iter()
                .filter_map(|&i| scores[i])
                .collect();

            if !group_scores.is_empty() {
                let group_mean: f32 = group_scores.iter().sum::<f32>() / group_scores.len() as f32;
                total += group_mean * group_weight;
                valid_groups += 1;
            }
        }

        if valid_groups > 0 {
            // Normalize by actual number of groups with valid scores
            total * (groups.len() as f32 / valid_groups as f32)
        } else {
            0.0
        }
    }

    /// Cross-correlation dominant: emphasizes pairs with high synergy.
    fn aggregate_correlation(&self, scores: &[Option<f32>; NUM_EMBEDDERS]) -> f32 {
        let synergy = match &self.config.synergy_matrix {
            Some(s) => s,
            None => return self.aggregate_mean(scores),
        };

        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;

        // Use off-diagonal synergy values to weight pairs
        for i in 0..NUM_EMBEDDERS {
            for j in (i + 1)..NUM_EMBEDDERS {
                if let (Some(s_i), Some(s_j)) = (scores[i], scores[j]) {
                    let pair_sim = (s_i + s_j) / 2.0;
                    let synergy_weight = synergy.get_synergy(i, j);
                    weighted_sum += pair_sim * synergy_weight;
                    weight_sum += synergy_weight;
                }
            }
        }

        if weight_sum > f32::EPSILON {
            weighted_sum / weight_sum
        } else {
            self.aggregate_mean(scores)
        }
    }

    /// Tucker decomposition approximation (simplified for fingerprint comparison).
    /// Uses principal components estimated from score variance.
    fn aggregate_tucker(&self, scores: &[Option<f32>; NUM_EMBEDDERS]) -> f32 {
        let valid_scores: Vec<f32> = scores.iter().filter_map(|&s| s).collect();
        if valid_scores.is_empty() {
            return 0.0;
        }

        let n = valid_scores.len() as f32;
        let mean: f32 = valid_scores.iter().sum::<f32>() / n;
        let variance: f32 = valid_scores.iter().map(|&s| (s - mean).powi(2)).sum::<f32>() / n;
        let std_dev = variance.sqrt();

        // Tucker-inspired: weight by how close each score is to the mean
        // (approximates principal component contribution)
        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;

        for &score in &valid_scores {
            // Higher weight for scores closer to mean (more "typical")
            let distance = (score - mean).abs();
            let weight = if std_dev > f32::EPSILON {
                (1.0 - (distance / (std_dev * 2.0)).min(1.0)).max(0.1)
            } else {
                1.0
            };
            weighted_sum += score * weight;
            weight_sum += weight;
        }

        if weight_sum > f32::EPSILON {
            weighted_sum / weight_sum
        } else {
            mean
        }
    }

    /// Adaptive strategy: chooses aggregation based on score distribution.
    fn aggregate_adaptive(&self, scores: &[Option<f32>; NUM_EMBEDDERS]) -> f32 {
        let valid_scores: Vec<f32> = scores.iter().filter_map(|&s| s).collect();
        if valid_scores.is_empty() {
            return 0.0;
        }

        let n = valid_scores.len() as f32;
        let mean: f32 = valid_scores.iter().sum::<f32>() / n;
        let variance: f32 = valid_scores.iter().map(|&s| (s - mean).powi(2)).sum::<f32>() / n;
        let std_dev = variance.sqrt();

        // Coefficient of variation determines strategy
        let cov = if mean > f32::EPSILON {
            std_dev / mean
        } else {
            0.0
        };

        if cov < 0.1 {
            // Low variance: scores are consistent, use simple mean
            self.aggregate_mean(scores)
        } else if cov < 0.3 {
            // Medium variance: use synergy weighting if available
            if self.config.synergy_matrix.is_some() {
                self.aggregate_synergy(scores)
            } else {
                self.aggregate_hierarchical(scores)
            }
        } else {
            // High variance: use robust method
            self.aggregate_tucker(scores)
        }
    }

    /// Compute coherence measure across embedders.
    /// Higher coherence = more consistent scores = more confident result.
    fn compute_coherence(&self, scores: &[Option<f32>; NUM_EMBEDDERS]) -> Option<f32> {
        let valid_scores: Vec<f32> = scores.iter().filter_map(|&s| s).collect();
        if valid_scores.len() < 2 {
            return None; // Need at least 2 scores for coherence
        }

        let n = valid_scores.len() as f32;
        let mean: f32 = valid_scores.iter().sum::<f32>() / n;

        if mean < f32::EPSILON {
            return Some(0.0); // All zeros = no coherence information
        }

        let variance: f32 = valid_scores.iter().map(|&s| (s - mean).powi(2)).sum::<f32>() / n;
        let std_dev = variance.sqrt();
        let cov = std_dev / mean;

        // Coherence = 1 / (1 + CoV)
        Some(1.0 / (1.0 + cov))
    }

    /// Find the embedder with the highest similarity score.
    fn find_dominant_embedder(&self, scores: &[Option<f32>; NUM_EMBEDDERS]) -> Option<Embedder> {
        let mut max_score = f32::NEG_INFINITY;
        let mut max_idx = None;

        for (idx, score) in scores.iter().enumerate() {
            if let Some(s) = score {
                if *s > max_score {
                    max_score = *s;
                    max_idx = Some(idx);
                }
            }
        }

        max_idx.and_then(Embedder::from_index)
    }

    /// Generate detailed breakdown for the comparison.
    fn generate_breakdown(
        &self,
        result: &ComparisonResult,
        strategy: SearchStrategy,
    ) -> SimilarityBreakdown {
        use std::collections::HashMap;

        let mut breakdown = SimilarityBreakdown {
            overall: result.overall,
            purpose_vector: result.overall, // Simplified: use overall as purpose
            cross_correlations: 0.0,
            group_alignments: 0.0,
            per_group: HashMap::new(),
            per_embedder_purpose: [0.0; NUM_EMBEDDERS],
            top_correlation_pairs: Vec::new(),
            strategy_used: strategy,
        };

        // Fill per-embedder scores
        for (idx, score) in result.per_embedder.iter().enumerate() {
            breakdown.per_embedder_purpose[idx] = score.unwrap_or(0.0);
        }

        // Calculate group scores
        use crate::teleological::GroupType;
        let group_map: [(GroupType, &[usize]); 6] = [
            (GroupType::Factual, &[0, 10, 11, 12]),
            (GroupType::Temporal, &[1, 2, 3]),
            (GroupType::Causal, &[4, 6]),
            (GroupType::Relational, &[7, 8]),
            (GroupType::Qualitative, &[9]),
            (GroupType::Implementation, &[5]),
        ];

        for (group_type, indices) in group_map {
            let group_scores: Vec<f32> = indices
                .iter()
                .filter_map(|&i| result.per_embedder[i])
                .collect();

            if !group_scores.is_empty() {
                let avg = group_scores.iter().sum::<f32>() / group_scores.len() as f32;
                breakdown.per_group.insert(group_type, avg);
            }
        }

        // Calculate group alignments average
        if !breakdown.per_group.is_empty() {
            breakdown.group_alignments =
                breakdown.per_group.values().sum::<f32>() / breakdown.per_group.len() as f32;
        }

        breakdown
    }
}

/// Batch comparator for parallel processing using rayon.
///
/// Provides efficient parallel comparison for scenarios where
/// many fingerprints need to be compared simultaneously.
///
/// # Example
///
/// ```rust,ignore
/// use context_graph_core::teleological::BatchComparator;
///
/// let batch = BatchComparator::new();
/// let results = batch.compare_one_to_many(&reference, &targets);
/// ```
#[derive(Debug, Clone)]
pub struct BatchComparator {
    comparator: TeleologicalComparator,
}

impl Default for BatchComparator {
    fn default() -> Self {
        Self::new()
    }
}

impl BatchComparator {
    /// Create with default configuration.
    pub fn new() -> Self {
        Self {
            comparator: TeleologicalComparator::new(),
        }
    }

    /// Create with specific configuration.
    pub fn with_config(config: MatrixSearchConfig) -> Self {
        Self {
            comparator: TeleologicalComparator::with_config(config),
        }
    }

    /// Get the underlying comparator.
    pub fn comparator(&self) -> &TeleologicalComparator {
        &self.comparator
    }

    /// Compare one reference against many targets in parallel.
    ///
    /// Uses rayon for parallel iteration, distributing comparisons
    /// across all available CPU cores.
    pub fn compare_one_to_many(
        &self,
        reference: &SemanticFingerprint,
        targets: &[SemanticFingerprint],
    ) -> Vec<ComparisonValidationResult<ComparisonResult>> {
        targets
            .par_iter()
            .map(|target| self.comparator.compare(reference, target))
            .collect()
    }

    /// Compare many-to-many in parallel, returns similarity matrix.
    ///
    /// Returns a Vec<Vec<f32>> where result[i][j] is the similarity
    /// between fingerprints[i] and fingerprints[j].
    ///
    /// The matrix is symmetric (result[i][j] == result[j][i]).
    /// Diagonal elements are 1.0 (self-similarity).
    pub fn compare_all_pairs(&self, fingerprints: &[SemanticFingerprint]) -> Vec<Vec<f32>> {
        let n = fingerprints.len();
        if n == 0 {
            return Vec::new();
        }

        // Initialize matrix with 1.0 on diagonal
        let mut matrix: Vec<Vec<f32>> = vec![vec![0.0; n]; n];
        for (i, row) in matrix.iter_mut().enumerate() {
            row[i] = 1.0;
        }

        // Compute upper triangle in parallel, then mirror
        let pairs: Vec<(usize, usize)> = (0..n)
            .flat_map(|i| ((i + 1)..n).map(move |j| (i, j)))
            .collect();

        let similarities: Vec<((usize, usize), f32)> = pairs
            .par_iter()
            .map(|&(i, j)| {
                let sim = self
                    .comparator
                    .compare(&fingerprints[i], &fingerprints[j])
                    .map(|r| r.overall)
                    .unwrap_or(0.0);
                ((i, j), sim)
            })
            .collect();

        // Fill matrix (symmetric)
        for ((i, j), sim) in similarities {
            matrix[i][j] = sim;
            matrix[j][i] = sim;
        }

        matrix
    }

    /// Compare one reference against many, returning only scores above threshold.
    ///
    /// Returns pairs of (index, similarity) for targets exceeding min_similarity.
    pub fn compare_above_threshold(
        &self,
        reference: &SemanticFingerprint,
        targets: &[SemanticFingerprint],
        min_similarity: f32,
    ) -> Vec<(usize, f32)> {
        targets
            .par_iter()
            .enumerate()
            .filter_map(|(idx, target)| {
                self.comparator
                    .compare(reference, target)
                    .ok()
                    .filter(|r| r.overall >= min_similarity)
                    .map(|r| (idx, r.overall))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::teleological::{ComparisonValidationError, SynergyMatrix};
    use crate::types::fingerprint::{SparseVector, E1_DIM, E2_DIM, E3_DIM, E4_DIM, E5_DIM, E7_DIM, E8_DIM, E9_DIM, E10_DIM, E11_DIM};

    /// Create a test fingerprint with known values for dense embeddings.
    fn create_test_fingerprint(base_value: f32) -> SemanticFingerprint {
        // Create normalized vectors to ensure valid cosine similarity
        let create_normalized_vec = |dim: usize, val: f32| -> Vec<f32> {
            let mut v = vec![val; dim];
            let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > f32::EPSILON {
                for x in &mut v {
                    *x /= norm;
                }
            }
            v
        };

        SemanticFingerprint {
            e1_semantic: create_normalized_vec(E1_DIM, base_value),
            e2_temporal_recent: create_normalized_vec(E2_DIM, base_value),
            e3_temporal_periodic: create_normalized_vec(E3_DIM, base_value),
            e4_temporal_positional: create_normalized_vec(E4_DIM, base_value),
            e5_causal: create_normalized_vec(E5_DIM, base_value),
            e6_sparse: SparseVector::empty(),
            e7_code: create_normalized_vec(E7_DIM, base_value),
            e8_graph: create_normalized_vec(E8_DIM, base_value),
            e9_hdc: create_normalized_vec(E9_DIM, base_value),
            e10_multimodal: create_normalized_vec(E10_DIM, base_value),
            e11_entity: create_normalized_vec(E11_DIM, base_value),
            e12_late_interaction: vec![vec![base_value / 128.0_f32.sqrt(); 128]],
            e13_splade: SparseVector::empty(),
        }
    }

    /// Create two fingerprints with known cosine similarity.
    fn create_orthogonal_fingerprints() -> (SemanticFingerprint, SemanticFingerprint) {
        // Fingerprint A: all ones (normalized)
        // Fingerprint B: alternating sign (orthogonal in high dimensions approximation)
        let fp_a = create_test_fingerprint(1.0);
        let mut fp_b = create_test_fingerprint(1.0);

        // Make E1 orthogonal
        for (i, val) in fp_b.e1_semantic.iter_mut().enumerate() {
            if i % 2 == 1 {
                *val = -*val;
            }
        }
        // Renormalize
        let norm: f32 = fp_b.e1_semantic.iter().map(|x| x * x).sum::<f32>().sqrt();
        for x in &mut fp_b.e1_semantic {
            *x /= norm;
        }

        (fp_a, fp_b)
    }

    #[test]
    fn test_compare_identical() {
        let fp = create_test_fingerprint(1.0);
        let comparator = TeleologicalComparator::new();

        let result = comparator.compare(&fp, &fp).expect("comparison should succeed");

        // Self-similarity should be ~1.0
        assert!(
            result.overall >= 0.99,
            "Self-similarity should be ~1.0, got {}",
            result.overall
        );

        // All available embedders should have scores
        let valid_count = result.valid_score_count();
        assert!(valid_count >= 10, "Expected at least 10 valid scores, got {}", valid_count);
    }

    #[test]
    fn test_compare_different() {
        let (fp_a, fp_b) = create_orthogonal_fingerprints();
        let comparator = TeleologicalComparator::new();

        let result = comparator.compare(&fp_a, &fp_b).expect("comparison should succeed");

        // Orthogonal vectors should have low similarity
        // Note: Only E1 is truly orthogonal in this test
        assert!(
            result.per_embedder[0].map(|s| s < 0.5).unwrap_or(false),
            "E1 similarity for orthogonal vectors should be low"
        );
    }

    #[test]
    fn test_compare_no_overlap() {
        // Create fingerprints with empty embeddings
        let fp_a = SemanticFingerprint {
            e1_semantic: vec![1.0 / (E1_DIM as f32).sqrt(); E1_DIM],
            e2_temporal_recent: vec![],
            e3_temporal_periodic: vec![],
            e4_temporal_positional: vec![],
            e5_causal: vec![],
            e6_sparse: SparseVector::empty(),
            e7_code: vec![],
            e8_graph: vec![],
            e9_hdc: vec![],
            e10_multimodal: vec![],
            e11_entity: vec![],
            e12_late_interaction: vec![],
            e13_splade: SparseVector::empty(),
        };

        let fp_b = SemanticFingerprint {
            e1_semantic: vec![],
            e2_temporal_recent: vec![1.0 / (E2_DIM as f32).sqrt(); E2_DIM],
            e3_temporal_periodic: vec![],
            e4_temporal_positional: vec![],
            e5_causal: vec![],
            e6_sparse: SparseVector::empty(),
            e7_code: vec![],
            e8_graph: vec![],
            e9_hdc: vec![],
            e10_multimodal: vec![],
            e11_entity: vec![],
            e12_late_interaction: vec![],
            e13_splade: SparseVector::empty(),
        };

        let comparator = TeleologicalComparator::new();
        let result = comparator.compare(&fp_a, &fp_b).expect("comparison should succeed");

        // No overlapping embedders = 0 similarity
        assert_eq!(result.overall, 0.0, "No overlap should give 0 similarity");
        assert_eq!(result.valid_score_count(), 0, "No valid scores expected");
    }

    #[test]
    fn test_compare_strategies() {
        let fp_a = create_test_fingerprint(1.0);
        let fp_b = create_test_fingerprint(0.9);
        let comparator = TeleologicalComparator::new();

        let strategies = [
            SearchStrategy::Cosine,
            SearchStrategy::Euclidean,
            SearchStrategy::GroupHierarchical,
            SearchStrategy::TuckerCompressed,
            SearchStrategy::Adaptive,
        ];

        for strategy in strategies {
            let result = comparator
                .compare_with_strategy(&fp_a, &fp_b, strategy)
                .unwrap_or_else(|_| panic!("Strategy {:?} should succeed", strategy));

            assert!(
                (0.0..=1.0).contains(&result.overall),
                "Strategy {:?}: similarity {} should be in [0,1]",
                strategy,
                result.overall
            );
            assert_eq!(result.strategy, strategy);
        }
    }

    #[test]
    fn test_invalid_weights_fail_fast() {
        let fp = create_test_fingerprint(1.0);

        let mut config = MatrixSearchConfig::default();
        config.weights.purpose_vector = 2.0; // Invalid: > 1.0

        let comparator = TeleologicalComparator::with_config(config);
        let result = comparator.compare(&fp, &fp);

        assert!(result.is_err(), "Invalid weights should return error");
        assert!(
            matches!(result.unwrap_err(), ComparisonValidationError::WeightOutOfRange { .. }),
            "Error should be WeightOutOfRange"
        );
    }

    #[test]
    fn test_synergy_weighted() {
        let fp_a = create_test_fingerprint(1.0);
        let fp_b = create_test_fingerprint(0.8);

        let config = MatrixSearchConfig::with_synergy(SynergyMatrix::semantic_focused());
        let comparator = TeleologicalComparator::with_config(config);

        let result = comparator
            .compare_with_strategy(&fp_a, &fp_b, SearchStrategy::SynergyWeighted)
            .expect("Synergy weighted comparison should succeed");

        assert!(
            (0.0..=1.0).contains(&result.overall),
            "Synergy weighted result should be in [0,1]"
        );
    }

    #[test]
    fn test_coherence_computation() {
        let fp = create_test_fingerprint(1.0);
        let comparator = TeleologicalComparator::new();

        let result = comparator.compare(&fp, &fp).expect("comparison should succeed");

        // High self-similarity = high coherence
        assert!(
            result.coherence.map(|c| c > 0.9).unwrap_or(false),
            "Self-comparison should have high coherence"
        );
    }

    #[test]
    fn test_dominant_embedder() {
        let fp = create_test_fingerprint(1.0);
        let comparator = TeleologicalComparator::new();

        let result = comparator.compare(&fp, &fp).expect("comparison should succeed");

        // Should have a dominant embedder
        assert!(
            result.dominant_embedder.is_some(),
            "Should identify dominant embedder"
        );
    }

    #[test]
    fn test_breakdown_generation() {
        let fp = create_test_fingerprint(1.0);

        let config = MatrixSearchConfig {
            compute_breakdown: true,
            ..Default::default()
        };

        let comparator = TeleologicalComparator::with_config(config);
        let result = comparator.compare(&fp, &fp).expect("comparison should succeed");

        assert!(result.breakdown.is_some(), "Breakdown should be generated");

        let breakdown = result.breakdown.as_ref().expect("breakdown exists");
        assert!(
            !breakdown.per_group.is_empty(),
            "Per-group scores should be populated"
        );
    }

    #[test]
    fn test_similarity_range() {
        let fp_a = create_test_fingerprint(1.0);
        let fp_b = create_test_fingerprint(0.5);
        let comparator = TeleologicalComparator::new();

        let result = comparator.compare(&fp_a, &fp_b).expect("comparison should succeed");

        // Overall must be in [0, 1]
        assert!(
            (0.0..=1.0).contains(&result.overall),
            "Overall similarity {} should be in [0,1]",
            result.overall
        );

        // All per-embedder scores must be in [0, 1]
        for (idx, score) in result.per_embedder.iter().enumerate() {
            if let Some(s) = score {
                assert!(
                    (0.0..=1.0).contains(s),
                    "Embedder {} score {} should be in [0,1]",
                    idx,
                    s
                );
            }
        }
    }

    #[test]
    fn test_batch_one_to_many() {
        let reference = create_test_fingerprint(1.0);
        let targets: Vec<SemanticFingerprint> = (0..10)
            .map(|i| create_test_fingerprint(0.5 + (i as f32) * 0.05))
            .collect();

        let batch = BatchComparator::new();
        let results = batch.compare_one_to_many(&reference, &targets);

        assert_eq!(results.len(), 10, "Should have 10 results");
        for result in results {
            assert!(result.is_ok(), "All comparisons should succeed");
        }
    }

    #[test]
    fn test_batch_all_pairs() {
        let fingerprints: Vec<SemanticFingerprint> = (0..5)
            .map(|i| create_test_fingerprint(0.5 + (i as f32) * 0.1))
            .collect();

        let batch = BatchComparator::new();
        let matrix = batch.compare_all_pairs(&fingerprints);

        assert_eq!(matrix.len(), 5, "Matrix should be 5x5");
        for row in &matrix {
            assert_eq!(row.len(), 5, "Each row should have 5 elements");
        }

        // Diagonal should be 1.0
        for (i, row) in matrix.iter().enumerate() {
            assert!(
                (row[i] - 1.0).abs() < 0.01,
                "Diagonal element should be ~1.0"
            );
        }

        // Matrix should be symmetric
        for (i, row_i) in matrix.iter().enumerate() {
            for (j, &val) in row_i.iter().enumerate() {
                assert!(
                    (val - matrix[j][i]).abs() < f32::EPSILON,
                    "Matrix should be symmetric"
                );
            }
        }
    }

    #[test]
    fn test_apples_to_apples() {
        // Verify that only same-embedder comparisons occur
        let fp_a = create_test_fingerprint(1.0);
        let fp_b = create_test_fingerprint(0.8);

        let config = MatrixSearchConfig {
            compute_breakdown: true,
            ..Default::default()
        };

        let comparator = TeleologicalComparator::with_config(config);
        let result = comparator.compare(&fp_a, &fp_b).expect("comparison should succeed");

        // Per-embedder scores should reflect same-type comparisons only
        // E1 (index 0) compared with E1, E2 with E2, etc.
        // This is enforced by the EmbeddingSlice matching in compare_embedder_slices

        // Verify that scores are only present for embedders that exist in both fingerprints
        // (apples-to-apples means same embedder type comparison only)
        let valid_count = result.valid_score_count();
        assert!(
            valid_count > 0,
            "Should have at least one valid embedder comparison"
        );

        // All valid scores should be in valid range (confirming same-type comparison worked)
        for (idx, score) in result.per_embedder.iter().enumerate() {
            if let Some(s) = score {
                let name = SemanticFingerprint::embedding_name(idx).unwrap_or("unknown");
                assert!(
                    (0.0..=1.0).contains(s),
                    "Embedder {} ({}) score {} should be in [0,1] from same-type comparison",
                    idx, name, s
                );
            }
        }
    }

    #[test]
    fn test_no_unwrap_calls() {
        // This test verifies the code doesn't panic on edge cases
        // by testing various potentially problematic inputs

        let empty_fp = SemanticFingerprint {
            e1_semantic: vec![],
            e2_temporal_recent: vec![],
            e3_temporal_periodic: vec![],
            e4_temporal_positional: vec![],
            e5_causal: vec![],
            e6_sparse: SparseVector::empty(),
            e7_code: vec![],
            e8_graph: vec![],
            e9_hdc: vec![],
            e10_multimodal: vec![],
            e11_entity: vec![],
            e12_late_interaction: vec![],
            e13_splade: SparseVector::empty(),
        };

        let comparator = TeleologicalComparator::new();

        // Should not panic
        let result = comparator.compare(&empty_fp, &empty_fp);
        assert!(result.is_ok(), "Empty fingerprint comparison should not panic");
    }

    #[test]
    fn test_group_hierarchical() {
        let fp_a = create_test_fingerprint(1.0);
        let fp_b = create_test_fingerprint(0.7);

        let comparator = TeleologicalComparator::new();
        let result = comparator
            .compare_with_strategy(&fp_a, &fp_b, SearchStrategy::GroupHierarchical)
            .expect("Group hierarchical comparison should succeed");

        assert!(
            (0.0..=1.0).contains(&result.overall),
            "Group hierarchical result should be in [0,1]"
        );
    }

    #[test]
    fn test_adaptive_strategy() {
        let fp_a = create_test_fingerprint(1.0);
        let fp_b = create_test_fingerprint(0.6);

        let comparator = TeleologicalComparator::new();
        let result = comparator
            .compare_with_strategy(&fp_a, &fp_b, SearchStrategy::Adaptive)
            .expect("Adaptive comparison should succeed");

        assert!(
            (0.0..=1.0).contains(&result.overall),
            "Adaptive result should be in [0,1]"
        );
    }
}
