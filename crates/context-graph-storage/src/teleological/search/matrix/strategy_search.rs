//! MatrixStrategySearch: main search implementation with cross-embedder correlations.
//!
//! # Design Philosophy
//!
//! **FAIL FAST. NO FALLBACKS.**
//!
//! All errors are fatal. No recovery attempts.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use uuid::Uuid;

use context_graph_core::types::fingerprint::NUM_EMBEDDERS;

use super::analysis::{CorrelationAnalysis, CorrelationPattern, MatrixAnalysis};
use super::results::MatrixSearchResults;
use super::search_matrix::SearchMatrix;
use crate::teleological::indexes::{EmbedderIndex, EmbedderIndexRegistry};
use crate::teleological::search::error::{SearchError, SearchResult};
use crate::teleological::search::multi::{
    AggregatedHit, AggregationStrategy, MultiEmbedderSearch, MultiEmbedderSearchResults,
    NormalizationStrategy,
};

// ============================================================================
// MATRIX STRATEGY SEARCH
// ============================================================================

/// Matrix strategy search with cross-embedder correlations.
///
/// Wraps MultiEmbedderSearch and applies 14x14 weight matrices.
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
    /// * `matrix` - 14x14 weight matrix
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
            // Create weight map from diagonal.
            let mut weights = HashMap::with_capacity(NUM_EMBEDDERS);
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
    pub(crate) fn analyze_matrix(&self, matrix: &SearchMatrix) -> MatrixAnalysis {
        let mut cross_count = 0;
        for i in 0..NUM_EMBEDDERS {
            for j in 0..NUM_EMBEDDERS {
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
        // Group per-embedder scores by ID - pre-allocate based on aggregated hits count
        let mut id_scores: HashMap<Uuid, HashMap<usize, f32>> =
            HashMap::with_capacity(results.aggregated_hits.len());

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

                for i in 0..NUM_EMBEDDERS {
                    for j in 0..NUM_EMBEDDERS {
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
                    .filter_map(|(&idx, &sim)| self.index_to_embedder(idx).map(|e| (e, sim, sim)))
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
        let mut correlation_matrix = [[0.0f32; 14]; 14];

        // Collect scores per embedder for all IDs.
        let mut embedder_scores: HashMap<usize, Vec<(Uuid, f32)>> =
            HashMap::with_capacity(NUM_EMBEDDERS);
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
        for i in 0..NUM_EMBEDDERS {
            for j in 0..NUM_EMBEDDERS {
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
            correlation_matrix: [[0.0f32; 14]; 14],
            patterns: Vec::new(),
            coherence: 0.0,
        }
    }

    /// Detect correlation patterns.
    fn detect_patterns(&self, corr: &[[f32; 14]; 14]) -> Vec<CorrelationPattern> {
        let mut patterns = Vec::new();

        // Check for high consensus (multiple embedders with r > 0.7)
        let mut high_corr_pairs: Vec<(usize, usize)> = Vec::new();
        for i in 0..NUM_EMBEDDERS {
            for j in (i + 1)..NUM_EMBEDDERS {
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
        for i in 0..NUM_EMBEDDERS {
            let mut total_corr = 0.0f32;
            let mut count = 0;
            for j in 0..NUM_EMBEDDERS {
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
                    let variance = scores.iter().map(|&s| (s - mean).powi(2)).sum::<f32>()
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
        if idx < NUM_EMBEDDERS {
            Some(EmbedderIndex::from_index(idx))
        } else {
            None
        }
    }
}

/// Pearson correlation for matched ID scores.
pub(crate) fn pearson_correlation_matched(
    scores_a: &[(Uuid, f32)],
    scores_b: &[(Uuid, f32)],
) -> f32 {
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
