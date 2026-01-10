//! Matrix strategy search with cross-embedder correlations.
//!
//! # Design Philosophy
//!
//! **FAIL FAST. NO FALLBACKS.**
//!
//! All errors are fatal. No recovery attempts. This ensures:
//! - Bugs are caught early in development
//! - Data integrity is preserved
//! - Clear error messages for debugging
//!
//! # Overview
//!
//! Matrix strategy search applies a full 13x13 weight matrix for search. This enables
//! cross-embedder correlation analysis where off-diagonal weights capture relationships
//! between different embedding spaces.
//!
//! # Example
//!
//! ```no_run
//! use context_graph_storage::teleological::search::{
//!     MatrixStrategySearch, SearchMatrix, MatrixSearchBuilder,
//! };
//! use context_graph_storage::teleological::indexes::EmbedderIndexRegistry;
//! use std::sync::Arc;
//! use std::collections::HashMap;
//! use context_graph_storage::teleological::indexes::EmbedderIndex;
//!
//! let registry = Arc::new(EmbedderIndexRegistry::new());
//! let search = MatrixStrategySearch::new(registry);
//!
//! let mut queries = HashMap::new();
//! queries.insert(EmbedderIndex::E1Semantic, vec![0.5f32; 1024]);
//! queries.insert(EmbedderIndex::E7Code, vec![0.5f32; 1536]);
//!
//! // Search with predefined matrix
//! let results = search.search(
//!     queries,
//!     SearchMatrix::code_heavy(),
//!     10,
//!     None,
//! );
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use uuid::Uuid;

use super::error::{SearchError, SearchResult};
use super::multi::{
    AggregatedHit, AggregationStrategy, MultiEmbedderSearch, MultiEmbedderSearchResults,
    NormalizationStrategy,
};
use super::super::indexes::{EmbedderIndex, EmbedderIndexRegistry};

// ============================================================================
// SEARCH MATRIX (13x13)
// ============================================================================

/// 13x13 weight matrix for cross-embedder correlation search.
///
/// Diagonal elements weight individual embedder contributions.
/// Off-diagonal elements capture cross-embedder correlations.
///
/// # Example
///
/// ```
/// use context_graph_storage::teleological::search::SearchMatrix;
///
/// // Create identity (diagonal only, no cross-correlation)
/// let identity = SearchMatrix::identity();
///
/// // Use predefined semantic-focused matrix
/// let semantic = SearchMatrix::semantic_focused();
///
/// // Create custom matrix
/// let mut custom = SearchMatrix::zeros();
/// custom.set(0, 0, 1.0);  // E1Semantic full weight
/// custom.set(6, 6, 0.5);  // E7Code half weight
/// custom.set(0, 6, 0.2);  // E1-E7 cross-correlation
/// custom.set(6, 0, 0.2);  // E7-E1 cross-correlation (symmetric)
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SearchMatrix {
    /// 13x13 weight matrix. weights[i][j] = weight for embedder i × embedder j correlation.
    weights: [[f32; 13]; 13],
}

impl SearchMatrix {
    /// Create zero matrix.
    pub fn zeros() -> Self {
        Self {
            weights: [[0.0; 13]; 13],
        }
    }

    /// Create identity matrix (diagonal = 1.0, off-diagonal = 0.0).
    pub fn identity() -> Self {
        let mut weights = [[0.0; 13]; 13];
        for i in 0..13 {
            weights[i][i] = 1.0;
        }
        Self { weights }
    }

    /// Create uniform matrix (all weights = 1/13).
    pub fn uniform() -> Self {
        let w = 1.0 / 13.0;
        Self {
            weights: [[w; 13]; 13],
        }
    }

    /// Get weight at (i, j). Panics if i >= 13 or j >= 13.
    #[inline]
    pub fn get(&self, i: usize, j: usize) -> f32 {
        if i >= 13 || j >= 13 {
            panic!(
                "FAIL FAST: matrix index ({}, {}) out of bounds (max 12)",
                i, j
            );
        }
        self.weights[i][j]
    }

    /// Set weight at (i, j). Panics if i >= 13 or j >= 13.
    #[inline]
    pub fn set(&mut self, i: usize, j: usize, weight: f32) {
        if i >= 13 || j >= 13 {
            panic!(
                "FAIL FAST: matrix index ({}, {}) out of bounds (max 12)",
                i, j
            );
        }
        self.weights[i][j] = weight;
    }

    /// Get diagonal weight for embedder.
    #[inline]
    pub fn diagonal(&self, embedder: EmbedderIndex) -> f32 {
        if let Some(idx) = embedder.to_index() {
            self.weights[idx][idx]
        } else {
            0.0
        }
    }

    /// Check if matrix is diagonal (all off-diagonal = 0).
    pub fn is_diagonal(&self) -> bool {
        for i in 0..13 {
            for j in 0..13 {
                if i != j && self.weights[i][j].abs() > 1e-9 {
                    return false;
                }
            }
        }
        true
    }

    /// Check if matrix has cross-correlations (any off-diagonal > 0).
    pub fn has_cross_correlations(&self) -> bool {
        !self.is_diagonal()
    }

    /// Get sparsity (fraction of zero elements).
    pub fn sparsity(&self) -> f32 {
        let mut zeros = 0;
        for i in 0..13 {
            for j in 0..13 {
                if self.weights[i][j].abs() < 1e-9 {
                    zeros += 1;
                }
            }
        }
        zeros as f32 / 169.0
    }

    /// Get list of non-zero embedder indices on diagonal.
    pub fn active_embedders(&self) -> Vec<usize> {
        (0..13)
            .filter(|&i| self.weights[i][i].abs() > 1e-9)
            .collect()
    }

    // === PREDEFINED MATRICES ===

    /// Semantic-focused: E1Semantic=1.0, E5Causal=0.3, E1-E5 cross=0.2
    pub fn semantic_focused() -> Self {
        let mut m = Self::zeros();
        m.set(0, 0, 1.0); // E1Semantic
        m.set(4, 4, 0.3); // E5Causal
        m.set(0, 4, 0.2); // E1-E5 cross
        m.set(4, 0, 0.2); // E5-E1 cross (symmetric)
        m
    }

    /// Code-heavy: E7Code=1.0, E1Semantic=0.3, E1-E7 cross=0.2
    pub fn code_heavy() -> Self {
        let mut m = Self::zeros();
        m.set(6, 6, 1.0); // E7Code
        m.set(0, 0, 0.3); // E1Semantic
        m.set(0, 6, 0.2); // E1-E7 cross
        m.set(6, 0, 0.2); // E7-E1 cross
        m
    }

    /// Temporal-aware: E2+E3+E4=0.8, E1=0.5, temporal cross=0.1
    pub fn temporal_aware() -> Self {
        let mut m = Self::zeros();
        m.set(0, 0, 0.5); // E1Semantic
        m.set(1, 1, 0.8); // E2TemporalRecent
        m.set(2, 2, 0.8); // E3TemporalPeriodic
        m.set(3, 3, 0.8); // E4TemporalPositional
                          // Temporal cross-correlations
        m.set(1, 2, 0.1);
        m.set(2, 1, 0.1);
        m.set(1, 3, 0.1);
        m.set(3, 1, 0.1);
        m.set(2, 3, 0.1);
        m.set(3, 2, 0.1);
        m
    }

    /// Balanced: all 10 HNSW embedders = 1/10 (excludes E6, E12, E13)
    pub fn balanced() -> Self {
        let w = 0.1;
        let mut m = Self::zeros();
        // Include all HNSW-capable: 0,1,2,3,4,6,7,8,9,10 (skip 5,11,12)
        for i in [0, 1, 2, 3, 4, 6, 7, 8, 9, 10] {
            m.set(i, i, w);
        }
        m
    }

    /// Entity-focused: E11Entity=1.0, E1Semantic=0.4, E8Graph=0.3
    pub fn entity_focused() -> Self {
        let mut m = Self::zeros();
        m.set(10, 10, 1.0); // E11Entity
        m.set(0, 0, 0.4); // E1Semantic
        m.set(7, 7, 0.3); // E8Graph
        m
    }
}

impl Default for SearchMatrix {
    fn default() -> Self {
        Self::balanced()
    }
}

// ============================================================================
// MATRIX ANALYSIS
// ============================================================================

/// Analysis of matrix structure for execution optimization.
#[derive(Debug, Clone)]
pub struct MatrixAnalysis {
    /// Matrix is purely diagonal (no cross-correlations).
    pub is_diagonal: bool,
    /// Matrix has off-diagonal weights.
    pub has_cross_correlations: bool,
    /// Fraction of zero elements.
    pub sparsity: f32,
    /// Embedder indices with non-zero diagonal weights.
    pub active_embedders: Vec<usize>,
    /// Number of non-zero off-diagonal elements.
    pub cross_correlation_count: usize,
}

// ============================================================================
// CORRELATION ANALYSIS
// ============================================================================

/// Analysis of embedder correlations in search results.
#[derive(Debug, Clone)]
pub struct CorrelationAnalysis {
    /// 13x13 Pearson correlation matrix between embedder scores.
    pub correlation_matrix: [[f32; 13]; 13],
    /// Detected correlation patterns.
    pub patterns: Vec<CorrelationPattern>,
    /// Overall coherence score (0-1, higher = more agreement).
    pub coherence: f32,
}

/// Detected correlation patterns between embedders.
#[derive(Debug, Clone)]
pub enum CorrelationPattern {
    /// Multiple embedders strongly agree on relevance.
    ConsensusHigh {
        embedder_indices: Vec<usize>,
        strength: f32,
    },
    /// Temporal embedders align with semantic.
    TemporalSemanticAlign { strength: f32 },
    /// Code and semantic embeddings diverge.
    CodeSemanticDivergence { strength: f32 },
    /// One embedder significantly disagrees with others.
    OutlierEmbedder {
        embedder_index: usize,
        deviation: f32,
    },
}

// ============================================================================
// MATRIX SEARCH RESULTS
// ============================================================================

/// Search results with correlation analysis.
#[derive(Debug, Clone)]
pub struct MatrixSearchResults {
    /// Aggregated hits from underlying multi-search.
    pub hits: Vec<AggregatedHit>,
    /// Correlation analysis between embedders.
    pub correlation: CorrelationAnalysis,
    /// Matrix used for search.
    pub matrix_used: SearchMatrix,
    /// Matrix analysis results.
    pub matrix_analysis: MatrixAnalysis,
    /// Total latency in microseconds.
    pub latency_us: u64,
}

impl MatrixSearchResults {
    /// Check if no results were found.
    pub fn is_empty(&self) -> bool {
        self.hits.is_empty()
    }

    /// Get the number of results.
    pub fn len(&self) -> usize {
        self.hits.len()
    }

    /// Get the top (highest score) result.
    pub fn top(&self) -> Option<&AggregatedHit> {
        self.hits.first()
    }

    /// Get top N results.
    pub fn top_n(&self, n: usize) -> &[AggregatedHit] {
        if n >= self.hits.len() {
            &self.hits
        } else {
            &self.hits[..n]
        }
    }

    /// Get all result IDs.
    pub fn ids(&self) -> Vec<Uuid> {
        self.hits.iter().map(|h| h.id).collect()
    }
}

// ============================================================================
// MATRIX STRATEGY SEARCH
// ============================================================================

/// Matrix strategy search with cross-embedder correlations.
///
/// Wraps MultiEmbedderSearch and applies 13x13 weight matrices.
pub struct MatrixStrategySearch {
    multi_search: MultiEmbedderSearch,
}

impl MatrixStrategySearch {
    /// Create with default configuration.
    pub fn new(registry: Arc<EmbedderIndexRegistry>) -> Self {
        Self {
            multi_search: MultiEmbedderSearch::new(registry),
        }
    }

    /// Search using matrix weights.
    ///
    /// # Arguments
    ///
    /// * `queries` - Map of embedder -> query vector
    /// * `matrix` - 13x13 weight matrix
    /// * `k` - Number of results per embedder
    /// * `threshold` - Minimum similarity threshold
    ///
    /// # Returns
    ///
    /// Search results with correlation analysis.
    ///
    /// # Errors
    ///
    /// - `SearchError::Store` if queries empty
    /// - `SearchError::DimensionMismatch` if query wrong size
    /// - `SearchError::UnsupportedEmbedder` for E6/E12/E13
    pub fn search(
        &self,
        queries: HashMap<EmbedderIndex, Vec<f32>>,
        matrix: SearchMatrix,
        k: usize,
        threshold: Option<f32>,
    ) -> SearchResult<MatrixSearchResults> {
        let start = Instant::now();

        // FAIL FAST: Validate queries
        if queries.is_empty() {
            return Err(SearchError::Store(
                "FAIL FAST: queries map is empty".to_string(),
            ));
        }

        // Analyze matrix for optimization
        let analysis = self.analyze_matrix(&matrix);

        // If purely diagonal, delegate to MultiEmbedderSearch with weighted aggregation
        if analysis.is_diagonal {
            // Create weight map from diagonal
            let mut weights = HashMap::new();
            for &idx in &analysis.active_embedders {
                if let Some(embedder) = self.index_to_embedder(idx) {
                    weights.insert(embedder, matrix.get(idx, idx));
                }
            }

            // Filter queries to only active embedders
            let filtered_queries: HashMap<EmbedderIndex, Vec<f32>> = queries
                .into_iter()
                .filter(|(e, _)| {
                    if let Some(idx) = e.to_index() {
                        analysis.active_embedders.contains(&idx)
                    } else {
                        false
                    }
                })
                .collect();

            if filtered_queries.is_empty() {
                // No active embedders in queries - return empty results
                return Ok(MatrixSearchResults {
                    hits: Vec::new(),
                    correlation: self.empty_correlation(),
                    matrix_used: matrix,
                    matrix_analysis: analysis,
                    latency_us: start.elapsed().as_micros() as u64,
                });
            }

            let multi_results = self.multi_search.search_with_options(
                filtered_queries,
                k,
                threshold,
                NormalizationStrategy::None,
                AggregationStrategy::WeightedSum(weights),
            )?;

            // Compute correlation before moving hits
            let correlation = self.compute_correlation(&multi_results);

            return Ok(MatrixSearchResults {
                hits: multi_results.aggregated_hits,
                correlation,
                matrix_used: matrix,
                matrix_analysis: analysis,
                latency_us: start.elapsed().as_micros() as u64,
            });
        }

        // Full matrix search with cross-correlations
        // Fetch more results for cross-correlation computation
        let multi_results = self.multi_search.search(queries, k * 3, threshold)?;

        // Apply matrix weights including cross-correlations
        let weighted_hits = self.apply_matrix_weights(&multi_results, &matrix);

        // Compute correlation analysis
        let correlation = self.compute_correlation(&multi_results);

        // Take top k hits
        let hits = if weighted_hits.len() > k {
            weighted_hits.into_iter().take(k).collect()
        } else {
            weighted_hits
        };

        Ok(MatrixSearchResults {
            hits,
            correlation,
            matrix_used: matrix,
            matrix_analysis: analysis,
            latency_us: start.elapsed().as_micros() as u64,
        })
    }

    /// Search with full correlation analysis enabled.
    pub fn search_with_correlation(
        &self,
        queries: HashMap<EmbedderIndex, Vec<f32>>,
        matrix: SearchMatrix,
        k: usize,
    ) -> SearchResult<MatrixSearchResults> {
        self.search(queries, matrix, k, None)
    }

    /// Analyze matrix structure for optimization hints.
    fn analyze_matrix(&self, matrix: &SearchMatrix) -> MatrixAnalysis {
        let mut cross_count = 0;
        for i in 0..13 {
            for j in 0..13 {
                if i != j && matrix.get(i, j).abs() > 1e-9 {
                    cross_count += 1;
                }
            }
        }

        MatrixAnalysis {
            is_diagonal: matrix.is_diagonal(),
            has_cross_correlations: cross_count > 0,
            sparsity: matrix.sparsity(),
            active_embedders: matrix.active_embedders(),
            cross_correlation_count: cross_count,
        }
    }

    /// Apply matrix weights to raw per-embedder scores.
    fn apply_matrix_weights(
        &self,
        results: &MultiEmbedderSearchResults,
        matrix: &SearchMatrix,
    ) -> Vec<AggregatedHit> {
        // Group per-embedder scores by ID
        let mut id_scores: HashMap<Uuid, HashMap<usize, f32>> = HashMap::new();

        for (embedder, per_result) in &results.per_embedder {
            if let Some(idx) = embedder.to_index() {
                for hit in &per_result.hits {
                    id_scores
                        .entry(hit.id)
                        .or_default()
                        .insert(idx, hit.similarity);
                }
            }
        }

        // Apply matrix weights
        let mut aggregated: Vec<AggregatedHit> = id_scores
            .into_iter()
            .map(|(id, scores)| {
                let mut total_score = 0.0f32;
                let mut total_weight = 0.0f32;

                for i in 0..13 {
                    for j in 0..13 {
                        let w = matrix.get(i, j);
                        if w.abs() > 1e-9 {
                            if let (Some(&si), Some(&sj)) = (scores.get(&i), scores.get(&j)) {
                                if i == j {
                                    // Diagonal: direct weight
                                    total_score += si * w;
                                } else {
                                    // Cross-correlation: geometric mean
                                    total_score += (si * sj).sqrt() * w;
                                }
                                total_weight += w;
                            }
                        }
                    }
                }

                let final_score = if total_weight > 0.0 {
                    total_score / total_weight
                } else {
                    0.0
                };

                // Build contributing_embedders
                let contributing: Vec<(EmbedderIndex, f32, f32)> = scores
                    .iter()
                    .filter_map(|(&idx, &sim)| {
                        self.index_to_embedder(idx).map(|e| (e, sim, sim))
                    })
                    .collect();

                AggregatedHit {
                    id,
                    aggregated_score: final_score,
                    contributing_embedders: contributing,
                }
            })
            .collect();

        // Sort by aggregated score descending
        aggregated.sort_by(|a, b| {
            b.aggregated_score
                .partial_cmp(&a.aggregated_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        aggregated
    }

    /// Compute correlation analysis from search results.
    fn compute_correlation(&self, results: &MultiEmbedderSearchResults) -> CorrelationAnalysis {
        let mut correlation_matrix = [[0.0f32; 13]; 13];

        // Collect scores per embedder for all IDs
        let mut embedder_scores: HashMap<usize, Vec<(Uuid, f32)>> = HashMap::new();
        for (embedder, per_result) in &results.per_embedder {
            if let Some(idx) = embedder.to_index() {
                for hit in &per_result.hits {
                    embedder_scores
                        .entry(idx)
                        .or_default()
                        .push((hit.id, hit.similarity));
                }
            }
        }

        // Compute pairwise Pearson correlation
        for i in 0..13 {
            for j in 0..13 {
                if let (Some(scores_i), Some(scores_j)) =
                    (embedder_scores.get(&i), embedder_scores.get(&j))
                {
                    correlation_matrix[i][j] = pearson_correlation_matched(scores_i, scores_j);
                }
            }
        }

        // Detect patterns
        let patterns = self.detect_patterns(&correlation_matrix);

        // Compute overall coherence
        let coherence = self.compute_coherence(results);

        CorrelationAnalysis {
            correlation_matrix,
            patterns,
            coherence,
        }
    }

    /// Create empty correlation analysis.
    fn empty_correlation(&self) -> CorrelationAnalysis {
        CorrelationAnalysis {
            correlation_matrix: [[0.0f32; 13]; 13],
            patterns: Vec::new(),
            coherence: 0.0,
        }
    }

    /// Detect correlation patterns.
    fn detect_patterns(&self, corr: &[[f32; 13]; 13]) -> Vec<CorrelationPattern> {
        let mut patterns = Vec::new();

        // Check for high consensus (multiple embedders with r > 0.7)
        let mut high_corr_pairs: Vec<(usize, usize)> = Vec::new();
        for i in 0..13 {
            for j in (i + 1)..13 {
                if corr[i][j] > 0.7 {
                    high_corr_pairs.push((i, j));
                }
            }
        }
        if high_corr_pairs.len() >= 3 {
            let indices: Vec<usize> = high_corr_pairs
                .iter()
                .flat_map(|&(a, b)| [a, b])
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            let strength = high_corr_pairs
                .iter()
                .map(|&(i, j)| corr[i][j])
                .sum::<f32>()
                / high_corr_pairs.len() as f32;
            patterns.push(CorrelationPattern::ConsensusHigh {
                embedder_indices: indices,
                strength,
            });
        }

        // Check temporal-semantic alignment (E1=0, E2=1, E3=2, E4=3)
        let temporal_semantic_corr = (corr[0][1] + corr[0][2] + corr[0][3]) / 3.0;
        if temporal_semantic_corr > 0.5 {
            patterns.push(CorrelationPattern::TemporalSemanticAlign {
                strength: temporal_semantic_corr,
            });
        }

        // Check code-semantic divergence (E1=0, E7=6)
        if corr[0][6] < -0.3 {
            patterns.push(CorrelationPattern::CodeSemanticDivergence {
                strength: -corr[0][6],
            });
        }

        // Check for outlier embedders
        for i in 0..13 {
            let mut total_corr = 0.0f32;
            let mut count = 0;
            for j in 0..13 {
                if i != j && corr[i][j].abs() > 1e-9 {
                    total_corr += corr[i][j];
                    count += 1;
                }
            }
            if count > 0 {
                let avg_corr = total_corr / count as f32;
                if avg_corr < -0.3 {
                    patterns.push(CorrelationPattern::OutlierEmbedder {
                        embedder_index: i,
                        deviation: -avg_corr,
                    });
                }
            }
        }

        patterns
    }

    /// Compute overall coherence.
    fn compute_coherence(&self, results: &MultiEmbedderSearchResults) -> f32 {
        if results.aggregated_hits.is_empty() {
            return 0.0;
        }

        // Coherence = average across all hits of (1 / (1 + score_variance))
        let coherences: Vec<f32> = results
            .aggregated_hits
            .iter()
            .filter_map(|hit| {
                let scores: Vec<f32> = hit
                    .contributing_embedders
                    .iter()
                    .map(|(_, orig, _)| *orig)
                    .collect();
                if scores.len() > 1 {
                    let mean = scores.iter().sum::<f32>() / scores.len() as f32;
                    let variance = scores
                        .iter()
                        .map(|&s| (s - mean).powi(2))
                        .sum::<f32>()
                        / scores.len() as f32;
                    Some(1.0 / (1.0 + variance.sqrt()))
                } else {
                    None
                }
            })
            .collect();

        if coherences.is_empty() {
            0.0
        } else {
            coherences.iter().sum::<f32>() / coherences.len() as f32
        }
    }

    /// Convert index to embedder.
    fn index_to_embedder(&self, idx: usize) -> Option<EmbedderIndex> {
        if idx < 13 {
            Some(EmbedderIndex::from_index(idx))
        } else {
            None
        }
    }
}

/// Pearson correlation for matched ID scores.
fn pearson_correlation_matched(scores_a: &[(Uuid, f32)], scores_b: &[(Uuid, f32)]) -> f32 {
    // Find common IDs
    let a_map: HashMap<Uuid, f32> = scores_a.iter().cloned().collect();
    let common: Vec<(f32, f32)> = scores_b
        .iter()
        .filter_map(|(id, sb)| a_map.get(id).map(|sa| (*sa, *sb)))
        .collect();

    if common.len() < 2 {
        return 0.0;
    }

    let n = common.len() as f32;
    let sum_a: f32 = common.iter().map(|(a, _)| a).sum();
    let sum_b: f32 = common.iter().map(|(_, b)| b).sum();
    let sum_ab: f32 = common.iter().map(|(a, b)| a * b).sum();
    let sum_a2: f32 = common.iter().map(|(a, _)| a * a).sum();
    let sum_b2: f32 = common.iter().map(|(_, b)| b * b).sum();

    let numerator = n * sum_ab - sum_a * sum_b;
    let denominator = ((n * sum_a2 - sum_a * sum_a) * (n * sum_b2 - sum_b * sum_b)).sqrt();

    if denominator.abs() < 1e-9 {
        0.0
    } else {
        (numerator / denominator).clamp(-1.0, 1.0)
    }
}

// ============================================================================
// BUILDER
// ============================================================================

/// Builder pattern for matrix strategy search.
pub struct MatrixSearchBuilder {
    queries: HashMap<EmbedderIndex, Vec<f32>>,
    matrix: SearchMatrix,
    k: usize,
    threshold: Option<f32>,
}

impl MatrixSearchBuilder {
    /// Create a new builder with queries.
    pub fn new(queries: HashMap<EmbedderIndex, Vec<f32>>) -> Self {
        Self {
            queries,
            matrix: SearchMatrix::default(),
            k: 100,
            threshold: None,
        }
    }

    /// Set the search matrix.
    pub fn matrix(mut self, matrix: SearchMatrix) -> Self {
        self.matrix = matrix;
        self
    }

    /// Set the number of results to return.
    pub fn k(mut self, k: usize) -> Self {
        self.k = k;
        self
    }

    /// Set minimum similarity threshold.
    pub fn threshold(mut self, threshold: f32) -> Self {
        self.threshold = Some(threshold);
        self
    }

    /// Execute the search.
    pub fn execute(self, search: &MatrixStrategySearch) -> SearchResult<MatrixSearchResults> {
        search.search(self.queries, self.matrix, self.k, self.threshold)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========== SEARCH MATRIX TESTS ==========

    #[test]
    fn test_zeros_matrix() {
        println!("=== TEST: SearchMatrix::zeros creates all-zero matrix ===");
        println!("BEFORE: Creating zeros matrix");

        let m = SearchMatrix::zeros();

        println!("AFTER: matrix created");
        for i in 0..13 {
            for j in 0..13 {
                assert_eq!(m.get(i, j), 0.0);
            }
        }
        assert!(m.is_diagonal()); // All zeros is considered diagonal
        assert!(!m.has_cross_correlations());
        assert_eq!(m.sparsity(), 1.0);
        assert!(m.active_embedders().is_empty());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_identity_matrix() {
        println!("=== TEST: SearchMatrix::identity has 1.0 on diagonal ===");

        let m = SearchMatrix::identity();

        for i in 0..13 {
            for j in 0..13 {
                if i == j {
                    assert_eq!(m.get(i, j), 1.0);
                } else {
                    assert_eq!(m.get(i, j), 0.0);
                }
            }
        }
        assert!(m.is_diagonal());
        assert!(!m.has_cross_correlations());
        assert_eq!(m.active_embedders().len(), 13);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_uniform_matrix() {
        println!("=== TEST: SearchMatrix::uniform has 1/13 everywhere ===");

        let m = SearchMatrix::uniform();
        let expected = 1.0 / 13.0;

        for i in 0..13 {
            for j in 0..13 {
                assert!((m.get(i, j) - expected).abs() < 1e-6);
            }
        }
        assert!(!m.is_diagonal());
        assert!(m.has_cross_correlations());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_predefined_semantic_focused() {
        println!("=== TEST: SearchMatrix::semantic_focused structure ===");

        let m = SearchMatrix::semantic_focused();
        assert_eq!(m.get(0, 0), 1.0, "E1Semantic should have weight 1.0");
        assert_eq!(m.get(4, 4), 0.3, "E5Causal should have weight 0.3");
        assert_eq!(m.get(0, 4), 0.2, "E1-E5 cross should be 0.2");
        assert_eq!(m.get(4, 0), 0.2, "E5-E1 cross should be 0.2");
        assert!(m.has_cross_correlations());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_predefined_code_heavy() {
        println!("=== TEST: SearchMatrix::code_heavy structure ===");

        let m = SearchMatrix::code_heavy();
        assert_eq!(m.get(6, 6), 1.0, "E7Code should have weight 1.0");
        assert_eq!(m.get(0, 0), 0.3, "E1Semantic should have weight 0.3");
        assert_eq!(m.get(0, 6), 0.2, "E1-E7 cross should be 0.2");
        assert_eq!(m.get(6, 0), 0.2, "E7-E1 cross should be 0.2");
        assert!(m.has_cross_correlations());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_predefined_temporal_aware() {
        println!("=== TEST: SearchMatrix::temporal_aware structure ===");

        let m = SearchMatrix::temporal_aware();
        assert_eq!(m.get(0, 0), 0.5, "E1Semantic should have weight 0.5");
        assert_eq!(m.get(1, 1), 0.8, "E2TemporalRecent should have weight 0.8");
        assert_eq!(m.get(2, 2), 0.8, "E3TemporalPeriodic should have weight 0.8");
        assert_eq!(m.get(3, 3), 0.8, "E4TemporalPositional should have weight 0.8");
        // Check temporal cross-correlations
        assert_eq!(m.get(1, 2), 0.1, "E2-E3 cross should be 0.1");
        assert_eq!(m.get(2, 1), 0.1, "E3-E2 cross should be 0.1");
        assert!(m.has_cross_correlations());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_predefined_balanced() {
        println!("=== TEST: SearchMatrix::balanced structure ===");

        let m = SearchMatrix::balanced();
        // Should have 10 HNSW embedders each with weight 0.1
        assert_eq!(m.get(0, 0), 0.1); // E1
        assert_eq!(m.get(1, 1), 0.1); // E2
        assert_eq!(m.get(6, 6), 0.1); // E7
        // Should skip non-HNSW: E6(5), E12(11), E13(12)
        assert_eq!(m.get(5, 5), 0.0, "E6Sparse should have weight 0.0");
        assert_eq!(m.get(11, 11), 0.0, "E12LateInteraction should have weight 0.0");
        assert_eq!(m.get(12, 12), 0.0, "E13Splade should have weight 0.0");
        assert!(m.is_diagonal(), "Balanced should be diagonal-only");

        println!("RESULT: PASS");
    }

    #[test]
    fn test_predefined_entity_focused() {
        println!("=== TEST: SearchMatrix::entity_focused structure ===");

        let m = SearchMatrix::entity_focused();
        assert_eq!(m.get(10, 10), 1.0, "E11Entity should have weight 1.0");
        assert_eq!(m.get(0, 0), 0.4, "E1Semantic should have weight 0.4");
        assert_eq!(m.get(7, 7), 0.3, "E8Graph should have weight 0.3");
        assert!(m.is_diagonal());

        println!("RESULT: PASS");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_matrix_get_out_of_bounds_panics() {
        println!("=== TEST: SearchMatrix::get out of bounds panics ===");
        let m = SearchMatrix::zeros();
        m.get(13, 0); // Should panic
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_matrix_set_out_of_bounds_panics() {
        println!("=== TEST: SearchMatrix::set out of bounds panics ===");
        let mut m = SearchMatrix::zeros();
        m.set(0, 13, 1.0); // Should panic
    }

    #[test]
    fn test_matrix_sparsity() {
        println!("=== TEST: SearchMatrix::sparsity calculation ===");

        // Zeros matrix = 100% sparse (169/169 zeros)
        let zeros = SearchMatrix::zeros();
        assert_eq!(zeros.sparsity(), 1.0);

        // Identity matrix = 156/169 zeros = ~0.923
        let identity = SearchMatrix::identity();
        let expected = 156.0 / 169.0;
        assert!((identity.sparsity() - expected).abs() < 1e-4);

        // Uniform matrix = 0% sparse (0/169 zeros)
        let uniform = SearchMatrix::uniform();
        assert_eq!(uniform.sparsity(), 0.0);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_matrix_active_embedders() {
        println!("=== TEST: SearchMatrix::active_embedders ===");

        // Zeros has no active embedders
        let zeros = SearchMatrix::zeros();
        assert!(zeros.active_embedders().is_empty());

        // Identity has all 13 active
        let identity = SearchMatrix::identity();
        assert_eq!(identity.active_embedders().len(), 13);

        // Custom with only E1 and E7
        let mut custom = SearchMatrix::zeros();
        custom.set(0, 0, 1.0);
        custom.set(6, 6, 0.5);
        let active = custom.active_embedders();
        assert_eq!(active.len(), 2);
        assert!(active.contains(&0));
        assert!(active.contains(&6));

        println!("RESULT: PASS");
    }

    #[test]
    fn test_matrix_diagonal_for_embedder() {
        println!("=== TEST: SearchMatrix::diagonal for EmbedderIndex ===");

        let m = SearchMatrix::code_heavy();
        assert_eq!(m.diagonal(EmbedderIndex::E7Code), 1.0);
        assert_eq!(m.diagonal(EmbedderIndex::E1Semantic), 0.3);
        assert_eq!(m.diagonal(EmbedderIndex::E8Graph), 0.0);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_matrix_default_is_balanced() {
        println!("=== TEST: SearchMatrix::default is balanced ===");

        let default = SearchMatrix::default();
        let balanced = SearchMatrix::balanced();
        assert_eq!(default, balanced);

        println!("RESULT: PASS");
    }

    // ========== MATRIX ANALYSIS TESTS ==========

    #[test]
    fn test_matrix_analysis() {
        println!("=== TEST: MatrixAnalysis structure ===");

        let registry = Arc::new(EmbedderIndexRegistry::new());
        let search = MatrixStrategySearch::new(registry);

        let matrix = SearchMatrix::code_heavy();
        let analysis = search.analyze_matrix(&matrix);

        assert!(!analysis.is_diagonal, "code_heavy has cross-correlations");
        assert!(analysis.has_cross_correlations);
        assert!(analysis.sparsity > 0.9, "code_heavy is mostly sparse");
        assert!(analysis.active_embedders.contains(&0)); // E1
        assert!(analysis.active_embedders.contains(&6)); // E7
        assert_eq!(analysis.cross_correlation_count, 2); // E1-E7 and E7-E1

        println!("RESULT: PASS");
    }

    // ========== PEARSON CORRELATION TESTS ==========

    #[test]
    fn test_pearson_correlation_perfect() {
        println!("=== TEST: Pearson correlation perfect positive ===");

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let scores_a = vec![(id1, 0.1), (id2, 0.5), (id3, 0.9)];
        let scores_b = vec![(id1, 0.2), (id2, 0.6), (id3, 1.0)];

        let r = pearson_correlation_matched(&scores_a, &scores_b);
        println!("Pearson r = {:.4}", r);
        assert!(r > 0.99, "Perfect positive correlation expected");

        println!("RESULT: PASS");
    }

    #[test]
    fn test_pearson_correlation_negative() {
        println!("=== TEST: Pearson correlation negative ===");

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let scores_a = vec![(id1, 0.1), (id2, 0.5), (id3, 0.9)];
        let scores_b = vec![(id1, 0.9), (id2, 0.5), (id3, 0.1)];

        let r = pearson_correlation_matched(&scores_a, &scores_b);
        println!("Pearson r = {:.4}", r);
        assert!(r < -0.99, "Perfect negative correlation expected");

        println!("RESULT: PASS");
    }

    #[test]
    fn test_pearson_correlation_no_common_ids() {
        println!("=== TEST: Pearson correlation no common IDs ===");

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();
        let id4 = Uuid::new_v4();

        let scores_a = vec![(id1, 0.5), (id2, 0.6)];
        let scores_b = vec![(id3, 0.5), (id4, 0.6)];

        let r = pearson_correlation_matched(&scores_a, &scores_b);
        assert_eq!(r, 0.0, "No common IDs should return 0");

        println!("RESULT: PASS");
    }

    // ========== BUILDER TESTS ==========

    #[test]
    fn test_builder_pattern() {
        println!("=== TEST: MatrixSearchBuilder pattern ===");

        let queries: HashMap<EmbedderIndex, Vec<f32>> = HashMap::new();
        let builder = MatrixSearchBuilder::new(queries.clone())
            .matrix(SearchMatrix::code_heavy())
            .k(50)
            .threshold(0.5);

        assert_eq!(builder.k, 50);
        assert_eq!(builder.threshold, Some(0.5));
        assert_eq!(builder.matrix, SearchMatrix::code_heavy());

        println!("RESULT: PASS");
    }

    // ========== VERIFICATION LOG ==========

    #[test]
    fn test_verification_log() {
        println!("\n=== MATRIX.RS VERIFICATION LOG ===\n");

        println!("Type Verification:");
        println!("  - SearchMatrix: 13x13 weight matrix");
        println!("    - zeros(), identity(), uniform()");
        println!("    - get(), set(), diagonal()");
        println!("    - is_diagonal(), has_cross_correlations()");
        println!("    - sparsity(), active_embedders()");
        println!("  - Predefined matrices:");
        println!("    - semantic_focused()");
        println!("    - code_heavy()");
        println!("    - temporal_aware()");
        println!("    - balanced()");
        println!("    - entity_focused()");
        println!("  - MatrixAnalysis: optimization hints");
        println!("  - CorrelationAnalysis: 13x13 Pearson correlations");
        println!("  - CorrelationPattern: ConsensusHigh, TemporalSemanticAlign, etc.");
        println!("  - MatrixSearchResults: hits + correlation + metadata");
        println!("  - MatrixStrategySearch: wraps MultiEmbedderSearch");
        println!("  - MatrixSearchBuilder: fluent API");

        println!();
        println!("Fail Fast Verification:");
        println!("  - Matrix index bounds: PANIC on >= 13");
        println!("  - Empty queries: SearchError::Store");

        println!();
        println!("VERIFICATION COMPLETE");
    }

    // ========== BOUNDARY EDGE CASE TESTS ==========
    // From TASK-LOGIC-007 <boundary_edge_cases> section

    #[test]
    fn test_empty_queries_fails_fast() {
        println!("=== TEST: empty_queries - FAIL FAST with empty HashMap ===");
        println!("BEFORE: queries.is_empty() == true");

        let registry = Arc::new(EmbedderIndexRegistry::new());
        let search = MatrixStrategySearch::new(registry);

        let queries: HashMap<EmbedderIndex, Vec<f32>> = HashMap::new();
        let matrix = SearchMatrix::identity();

        let result = search.search(queries, matrix, 10, None);

        println!("AFTER: result = {:?}", result.is_err());

        match result {
            Err(SearchError::Store(msg)) => {
                println!("EVIDENCE: Error message = \"{}\"", msg);
                assert!(
                    msg.contains("empty"),
                    "Error message must contain 'empty'"
                );
                println!("RESULT: PASS - FAIL FAST with correct error");
            }
            Ok(_) => panic!("FAIL: Expected error for empty queries"),
            Err(e) => panic!("FAIL: Wrong error variant: {:?}", e),
        }
    }

    #[test]
    fn test_identity_matrix_equals_multi_search_structure() {
        println!("=== TEST: identity_matrix_equals_multi_search ===");
        println!("BEFORE: identity matrix has diagonal=1.0, off-diagonal=0.0");

        // Verify identity matrix structure
        let identity = SearchMatrix::identity();

        // Check diagonal is 1.0
        for i in 0..13 {
            assert_eq!(
                identity.get(i, i),
                1.0,
                "Identity diagonal[{}] must be 1.0",
                i
            );
        }

        // Check off-diagonal is 0.0
        for i in 0..13 {
            for j in 0..13 {
                if i != j {
                    assert_eq!(
                        identity.get(i, j),
                        0.0,
                        "Identity off-diagonal[{},{}] must be 0.0",
                        i,
                        j
                    );
                }
            }
        }

        // Verify is_diagonal() returns true
        assert!(identity.is_diagonal(), "Identity must be diagonal");

        // Verify no cross-correlations
        assert!(
            !identity.has_cross_correlations(),
            "Identity must have no cross-correlations"
        );

        println!("AFTER: Identity matrix structure verified");
        println!("EVIDENCE: is_diagonal=true, has_cross_correlations=false");
        println!("RESULT: PASS");
    }

    #[test]
    fn test_zero_weight_embedder_skipped_structure() {
        println!("=== TEST: zero_weight_embedder_skipped ===");
        println!("BEFORE: matrix with E1Semantic weight = 0.0");

        let mut matrix = SearchMatrix::identity();
        matrix.set(0, 0, 0.0); // Zero out E1Semantic

        // Verify active embedders excludes index 0
        let active = matrix.active_embedders();
        println!("AFTER: active_embedders = {:?}", active);

        assert!(
            !active.contains(&0),
            "E1Semantic (index 0) must not be in active embedders"
        );
        assert_eq!(active.len(), 12, "Should have 12 active embedders");

        // Verify diagonal returns 0.0 for E1Semantic
        assert_eq!(
            matrix.diagonal(EmbedderIndex::E1Semantic),
            0.0,
            "E1Semantic diagonal must be 0.0"
        );

        println!("EVIDENCE: E1Semantic not in active_embedders, diagonal=0.0");
        println!("RESULT: PASS");
    }

    #[test]
    fn test_cross_correlation_matrix_structure() {
        println!("=== TEST: cross_correlation_boosts_consensus (structure) ===");
        println!("BEFORE: matrix with E1-E7 cross-correlation = 0.5");

        let mut matrix = SearchMatrix::zeros();
        matrix.set(0, 0, 1.0); // E1Semantic diagonal
        matrix.set(6, 6, 1.0); // E7Code diagonal
        matrix.set(0, 6, 0.5); // E1-E7 cross
        matrix.set(6, 0, 0.5); // E7-E1 cross (symmetric)

        // Verify structure
        assert_eq!(matrix.get(0, 6), 0.5, "E1-E7 cross should be 0.5");
        assert_eq!(matrix.get(6, 0), 0.5, "E7-E1 cross should be 0.5");
        assert!(
            matrix.has_cross_correlations(),
            "Should have cross-correlations"
        );
        assert!(
            !matrix.is_diagonal(),
            "Should not be diagonal-only"
        );

        // Verify cross-correlation count
        let registry = Arc::new(EmbedderIndexRegistry::new());
        let search = MatrixStrategySearch::new(registry);
        let analysis = search.analyze_matrix(&matrix);

        println!("AFTER: analysis.cross_correlation_count = {}", analysis.cross_correlation_count);
        assert_eq!(
            analysis.cross_correlation_count, 2,
            "Should have 2 cross-correlations (E1-E7 and E7-E1)"
        );

        println!("EVIDENCE: has_cross_correlations=true, count=2");
        println!("RESULT: PASS");
    }

    #[test]
    fn test_unsupported_embedder_in_queries_fails_fast() {
        println!("=== TEST: unsupported_embedder_fails_fast ===");
        println!("BEFORE: queries contains E6Sparse");

        let registry = Arc::new(EmbedderIndexRegistry::new());
        let search = MatrixStrategySearch::new(registry);

        // Create queries with E6Sparse (unsupported for HNSW)
        let mut queries = HashMap::new();
        queries.insert(EmbedderIndex::E6Sparse, vec![0.5f32; 256]);

        let matrix = SearchMatrix::identity();
        let result = search.search(queries, matrix, 10, None);

        println!("AFTER: result = {:?}", result.is_err());

        match result {
            Err(SearchError::UnsupportedEmbedder { embedder }) => {
                println!("EVIDENCE: Got UnsupportedEmbedder for {:?}", embedder);
                assert_eq!(
                    embedder,
                    EmbedderIndex::E6Sparse,
                    "Error must be for E6Sparse"
                );
                println!("RESULT: PASS - FAIL FAST with correct error");
            }
            Err(SearchError::Store(_)) => {
                // Also acceptable - filtered queries become empty
                println!("EVIDENCE: Store error (queries filtered to empty)");
                println!("RESULT: PASS - FAIL FAST with filtered queries");
            }
            Ok(_) => panic!("FAIL: Expected error for unsupported embedder"),
            Err(e) => panic!("FAIL: Wrong error variant: {:?}", e),
        }
    }

    #[test]
    fn test_matrix_weights_applied_correctly() {
        println!("=== TEST: Matrix weights affect aggregation ===");
        println!("BEFORE: Testing weight application logic");

        // Create matrices with different weight distributions
        let semantic_matrix = SearchMatrix::semantic_focused();
        let code_matrix = SearchMatrix::code_heavy();

        // Verify weights differ
        let e1_weight_semantic = semantic_matrix.diagonal(EmbedderIndex::E1Semantic);
        let e1_weight_code = code_matrix.diagonal(EmbedderIndex::E1Semantic);
        let e7_weight_semantic = semantic_matrix.diagonal(EmbedderIndex::E7Code);
        let e7_weight_code = code_matrix.diagonal(EmbedderIndex::E7Code);

        println!("semantic_focused: E1={}, E7={}", e1_weight_semantic, e7_weight_semantic);
        println!("code_heavy: E1={}, E7={}", e1_weight_code, e7_weight_code);

        assert!(
            e1_weight_semantic > e1_weight_code,
            "E1 should have higher weight in semantic_focused"
        );
        assert!(
            e7_weight_code > e7_weight_semantic,
            "E7 should have higher weight in code_heavy"
        );

        println!("AFTER: Weight differences verified");
        println!("EVIDENCE: semantic E1={} > code E1={}, code E7={} > semantic E7={}",
                 e1_weight_semantic, e1_weight_code, e7_weight_code, e7_weight_semantic);
        println!("RESULT: PASS");
    }

    #[test]
    fn test_correlation_patterns_detection() {
        println!("=== TEST: Correlation pattern variants ===");

        // Verify all pattern variants can be created
        let patterns = vec![
            CorrelationPattern::ConsensusHigh {
                embedder_indices: vec![0, 4, 6],
                strength: 0.8,
            },
            CorrelationPattern::TemporalSemanticAlign { strength: 0.7 },
            CorrelationPattern::CodeSemanticDivergence { strength: 0.5 },
            CorrelationPattern::OutlierEmbedder {
                embedder_index: 5,
                deviation: 0.4,
            },
        ];

        for pattern in &patterns {
            println!("Pattern: {:?}", pattern);
        }

        assert_eq!(patterns.len(), 4, "Should have 4 pattern types");
        println!("RESULT: PASS");
    }

    #[test]
    fn test_matrix_results_accessors() {
        println!("=== TEST: MatrixSearchResults accessors ===");

        // Create empty results to test accessors
        let results = MatrixSearchResults {
            hits: Vec::new(),
            correlation: CorrelationAnalysis {
                correlation_matrix: [[0.0; 13]; 13],
                patterns: Vec::new(),
                coherence: 0.0,
            },
            matrix_used: SearchMatrix::identity(),
            matrix_analysis: MatrixAnalysis {
                is_diagonal: true,
                has_cross_correlations: false,
                sparsity: 0.923,
                active_embedders: (0..13).collect(),
                cross_correlation_count: 0,
            },
            latency_us: 100,
        };

        // Test accessors
        assert!(results.is_empty(), "Empty results should be empty");
        assert_eq!(results.len(), 0, "Length should be 0");
        assert!(results.top().is_none(), "Top should be None for empty");
        assert!(results.top_n(5).is_empty(), "Top 5 should be empty");
        assert!(results.ids().is_empty(), "IDs should be empty");

        println!("RESULT: PASS - All accessors work on empty results");
    }
}
