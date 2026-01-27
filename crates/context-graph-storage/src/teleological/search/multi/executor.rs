//! Multi-embedder search executor.
//!
//! Contains the core `MultiEmbedderSearch` struct that coordinates
//! parallel HNSW searches across multiple embedder indexes.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use rayon::prelude::*;
use uuid::Uuid;

use super::super::super::indexes::{EmbedderIndex, EmbedderIndexRegistry};
use super::super::error::{SearchError, SearchResult};
use super::super::result::EmbedderSearchHit;
use super::super::single::SingleEmbedderSearch;
use super::types::{
    AggregatedHit, AggregationStrategy, MultiEmbedderSearchConfig, MultiEmbedderSearchResults,
    NormalizationStrategy, PerEmbedderResults,
};

// ============================================================================
// MULTI-EMBEDDER SEARCH
// ============================================================================

/// Multi-embedder parallel HNSW search.
///
/// Searches multiple embedder indexes in parallel using rayon, then aggregates
/// results according to the configured normalization and aggregation strategies.
///
/// # Thread Safety
///
/// Uses rayon for parallel execution. Thread count is configurable.
///
/// # Example
///
/// ```no_run
/// use context_graph_storage::teleological::search::{
///     MultiEmbedderSearch, MultiEmbedderSearchConfig,
///     NormalizationStrategy, AggregationStrategy,
/// };
/// use context_graph_storage::teleological::indexes::{
///     EmbedderIndex, EmbedderIndexRegistry,
/// };
/// use std::sync::Arc;
/// use std::collections::HashMap;
///
/// let registry = Arc::new(EmbedderIndexRegistry::new());
/// let search = MultiEmbedderSearch::new(registry);
///
/// let mut queries = HashMap::new();
/// queries.insert(EmbedderIndex::E1Semantic, vec![0.5f32; 1024]);
/// queries.insert(EmbedderIndex::E8Graph, vec![0.5f32; 1024]);
///
/// let results = search.search(queries, 10, None);
/// ```
pub struct MultiEmbedderSearch {
    single_search: SingleEmbedderSearch,
    config: MultiEmbedderSearchConfig,
}

impl MultiEmbedderSearch {
    /// Create with default configuration.
    ///
    /// # Arguments
    ///
    /// * `registry` - Registry containing all HNSW indexes
    pub fn new(registry: Arc<EmbedderIndexRegistry>) -> Self {
        Self {
            single_search: SingleEmbedderSearch::new(registry),
            config: MultiEmbedderSearchConfig::default(),
        }
    }

    /// Create with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `registry` - Registry containing all HNSW indexes
    /// * `config` - Custom search configuration
    pub fn with_config(
        registry: Arc<EmbedderIndexRegistry>,
        config: MultiEmbedderSearchConfig,
    ) -> Self {
        Self {
            single_search: SingleEmbedderSearch::new(registry),
            config,
        }
    }

    /// Search multiple embedders in parallel.
    ///
    /// # Arguments
    ///
    /// * `queries` - Map of embedder -> query vector
    /// * `k` - Number of results per embedder
    /// * `threshold` - Minimum similarity threshold (None = no threshold)
    ///
    /// # Returns
    ///
    /// Aggregated search results with per-embedder breakdown.
    ///
    /// # Errors
    ///
    /// - `SearchError::EmptyQuery` if queries map is empty
    /// - Other errors from individual embedder searches (first error wins)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use context_graph_storage::teleological::search::MultiEmbedderSearch;
    /// use context_graph_storage::teleological::indexes::{
    ///     EmbedderIndex, EmbedderIndexRegistry,
    /// };
    /// use std::sync::Arc;
    /// use std::collections::HashMap;
    ///
    /// let registry = Arc::new(EmbedderIndexRegistry::new());
    /// let search = MultiEmbedderSearch::new(registry);
    ///
    /// let mut queries = HashMap::new();
    /// queries.insert(EmbedderIndex::E1Semantic, vec![0.5f32; 1024]);
    /// queries.insert(EmbedderIndex::E8Graph, vec![0.5f32; 1024]);
    ///
    /// let results = search.search(queries, 10, Some(0.5));
    /// ```
    pub fn search(
        &self,
        queries: HashMap<EmbedderIndex, Vec<f32>>,
        k: usize,
        threshold: Option<f32>,
    ) -> SearchResult<MultiEmbedderSearchResults> {
        let start = Instant::now();

        // FAIL FAST: Validate inputs
        if queries.is_empty() {
            return Err(SearchError::Store(
                "FAIL FAST: queries map is empty - no embedders to search".to_string(),
            ));
        }

        // Validate all queries before starting parallel search
        for (embedder, query) in &queries {
            self.validate_query(*embedder, query)?;
        }

        let embedders_searched: Vec<EmbedderIndex> = queries.keys().copied().collect();

        // Execute parallel search using rayon
        let search_results: Vec<SearchResult<(EmbedderIndex, PerEmbedderResults)>> = queries
            .into_par_iter()
            .map(|(embedder, query)| {
                let embedder_start = Instant::now();
                let effective_k = self.get_k_for_embedder(embedder, k);

                let result = self
                    .single_search
                    .search(embedder, &query, effective_k, threshold)?;

                let per_results = PerEmbedderResults {
                    embedder,
                    count: result.len(),
                    latency_us: embedder_start.elapsed().as_micros() as u64,
                    hits: result.hits,
                };

                Ok((embedder, per_results))
            })
            .collect();

        // FAIL FAST: Check for any errors
        // Pre-allocate for number of embedders (max 13) to avoid reallocations
        let mut per_embedder: HashMap<EmbedderIndex, PerEmbedderResults> =
            HashMap::with_capacity(embedders_searched.len());
        for result in search_results {
            let (embedder, results) = result?;
            per_embedder.insert(embedder, results);
        }

        // Aggregate results
        let aggregated_hits = self.aggregate_results(
            &per_embedder,
            &self.config.normalization,
            &self.config.aggregation,
        );

        Ok(MultiEmbedderSearchResults {
            aggregated_hits,
            per_embedder,
            total_latency_us: start.elapsed().as_micros() as u64,
            embedders_searched,
            normalization_used: self.config.normalization,
            aggregation_used: self.config.aggregation.clone(),
        })
    }

    /// Search with configuration overrides.
    ///
    /// # Arguments
    ///
    /// * `queries` - Map of embedder -> query vector
    /// * `k` - Number of results per embedder
    /// * `threshold` - Minimum similarity threshold
    /// * `normalization` - Override normalization strategy
    /// * `aggregation` - Override aggregation strategy
    pub fn search_with_options(
        &self,
        queries: HashMap<EmbedderIndex, Vec<f32>>,
        k: usize,
        threshold: Option<f32>,
        normalization: NormalizationStrategy,
        aggregation: AggregationStrategy,
    ) -> SearchResult<MultiEmbedderSearchResults> {
        let start = Instant::now();

        if queries.is_empty() {
            return Err(SearchError::Store(
                "FAIL FAST: queries map is empty - no embedders to search".to_string(),
            ));
        }

        for (embedder, query) in &queries {
            self.validate_query(*embedder, query)?;
        }

        let embedders_searched: Vec<EmbedderIndex> = queries.keys().copied().collect();

        let search_results: Vec<SearchResult<(EmbedderIndex, PerEmbedderResults)>> = queries
            .into_par_iter()
            .map(|(embedder, query)| {
                let embedder_start = Instant::now();
                let effective_k = self.get_k_for_embedder(embedder, k);

                let result = self
                    .single_search
                    .search(embedder, &query, effective_k, threshold)?;

                let per_results = PerEmbedderResults {
                    embedder,
                    count: result.len(),
                    latency_us: embedder_start.elapsed().as_micros() as u64,
                    hits: result.hits,
                };

                Ok((embedder, per_results))
            })
            .collect();

        // Pre-allocate for number of embedders (max 13) to avoid reallocations
        let mut per_embedder: HashMap<EmbedderIndex, PerEmbedderResults> =
            HashMap::with_capacity(embedders_searched.len());
        for result in search_results {
            let (embedder, results) = result?;
            per_embedder.insert(embedder, results);
        }

        let aggregated_hits = self.aggregate_results(&per_embedder, &normalization, &aggregation);

        Ok(MultiEmbedderSearchResults {
            aggregated_hits,
            per_embedder,
            total_latency_us: start.elapsed().as_micros() as u64,
            embedders_searched,
            normalization_used: normalization,
            aggregation_used: aggregation,
        })
    }

    /// Validate query vector for an embedder. FAIL FAST on invalid input.
    pub(crate) fn validate_query(
        &self,
        embedder: EmbedderIndex,
        query: &[f32],
    ) -> SearchResult<()> {
        // Check embedder supports HNSW
        if !embedder.uses_hnsw() {
            return Err(SearchError::UnsupportedEmbedder { embedder });
        }

        // Check empty
        if query.is_empty() {
            return Err(SearchError::EmptyQuery { embedder });
        }

        // Check dimension
        if let Some(expected_dim) = embedder.dimension() {
            if query.len() != expected_dim {
                return Err(SearchError::DimensionMismatch {
                    embedder,
                    expected: expected_dim,
                    actual: query.len(),
                });
            }
        }

        // Check for NaN/Inf
        for (i, &v) in query.iter().enumerate() {
            if !v.is_finite() {
                return Err(SearchError::InvalidVector {
                    embedder,
                    message: format!("Non-finite value at index {}: {}", i, v),
                });
            }
        }

        Ok(())
    }

    /// Get effective k for an embedder (uses override if available).
    fn get_k_for_embedder(&self, embedder: EmbedderIndex, default: usize) -> usize {
        self.config
            .per_embedder_k
            .as_ref()
            .and_then(|map| map.get(&embedder).copied())
            .unwrap_or(default)
    }

    /// Aggregate results from multiple embedders.
    pub(crate) fn aggregate_results(
        &self,
        per_embedder: &HashMap<EmbedderIndex, PerEmbedderResults>,
        normalization: &NormalizationStrategy,
        aggregation: &AggregationStrategy,
    ) -> Vec<AggregatedHit> {
        // Step 1: Normalize scores within each embedder
        let normalized: HashMap<EmbedderIndex, Vec<(Uuid, f32, f32)>> = per_embedder
            .iter()
            .map(|(embedder, results)| {
                let normalized = self.normalize_scores(&results.hits, normalization);
                (*embedder, normalized)
            })
            .collect();

        // Step 2: Group by ID across embedders
        // Pre-allocate based on total hits across all embedders
        let total_hits: usize = per_embedder.values().map(|r| r.hits.len()).sum();
        let mut id_scores: HashMap<Uuid, Vec<(EmbedderIndex, f32, f32)>> =
            HashMap::with_capacity(total_hits);
        for (embedder, scores) in &normalized {
            for (id, original, norm) in scores {
                id_scores
                    .entry(*id)
                    .or_default()
                    .push((*embedder, *original, *norm));
            }
        }

        // Step 3: Aggregate scores for each ID
        let mut aggregated: Vec<AggregatedHit> = id_scores
            .into_iter()
            .map(|(id, contributions)| {
                let aggregated_score = self.aggregate_score(&contributions, aggregation);
                AggregatedHit {
                    id,
                    aggregated_score,
                    contributing_embedders: contributions,
                }
            })
            .collect();

        // Step 4: Sort by aggregated score descending
        aggregated.sort_by(|a, b| {
            b.aggregated_score
                .partial_cmp(&a.aggregated_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        aggregated
    }

    /// Normalize scores within a single embedder's results.
    pub(crate) fn normalize_scores(
        &self,
        hits: &[EmbedderSearchHit],
        strategy: &NormalizationStrategy,
    ) -> Vec<(Uuid, f32, f32)> {
        if hits.is_empty() {
            return vec![];
        }

        match strategy {
            NormalizationStrategy::None => hits
                .iter()
                .map(|h| (h.id, h.similarity, h.similarity))
                .collect(),

            NormalizationStrategy::MinMax => {
                let (min, max) = hits.iter().fold((f32::MAX, f32::MIN), |(min, max), h| {
                    (min.min(h.similarity), max.max(h.similarity))
                });
                let range = max - min;

                if range < 1e-9 {
                    // All scores are the same - normalize to 1.0
                    return hits.iter().map(|h| (h.id, h.similarity, 1.0)).collect();
                }

                hits.iter()
                    .map(|h| {
                        let norm = (h.similarity - min) / range;
                        (h.id, h.similarity, norm)
                    })
                    .collect()
            }

            NormalizationStrategy::ZScore => {
                let n = hits.len() as f32;
                let mean: f32 = hits.iter().map(|h| h.similarity).sum::<f32>() / n;
                let variance: f32 = hits
                    .iter()
                    .map(|h| (h.similarity - mean).powi(2))
                    .sum::<f32>()
                    / n;
                let stddev = variance.sqrt();

                if stddev < 1e-9 {
                    // All scores are the same
                    return hits.iter().map(|h| (h.id, h.similarity, 0.0)).collect();
                }

                hits.iter()
                    .map(|h| {
                        let norm = (h.similarity - mean) / stddev;
                        // Clamp to reasonable range [-3, 3] for interpretability
                        let clamped = norm.clamp(-3.0, 3.0);
                        // Scale to [0, 1] for aggregation compatibility
                        let scaled = (clamped + 3.0) / 6.0;
                        (h.id, h.similarity, scaled)
                    })
                    .collect()
            }

            NormalizationStrategy::RankNorm => hits
                .iter()
                .enumerate()
                .map(|(rank, h)| {
                    let norm = 1.0 / (rank + 1) as f32;
                    (h.id, h.similarity, norm)
                })
                .collect(),
        }
    }

    /// Aggregate scores from multiple embedders for a single ID.
    pub(crate) fn aggregate_score(
        &self,
        contributions: &[(EmbedderIndex, f32, f32)], // (embedder, original, normalized)
        strategy: &AggregationStrategy,
    ) -> f32 {
        if contributions.is_empty() {
            return 0.0;
        }

        match strategy {
            AggregationStrategy::Max => contributions
                .iter()
                .map(|(_, _, norm)| *norm)
                .fold(f32::MIN, f32::max),

            AggregationStrategy::Sum => contributions.iter().map(|(_, _, norm)| *norm).sum(),

            AggregationStrategy::Mean => {
                let sum: f32 = contributions.iter().map(|(_, _, norm)| *norm).sum();
                sum / contributions.len() as f32
            }

            AggregationStrategy::WeightedSum(weights) => {
                let mut total = 0.0;
                let mut weight_sum = 0.0;
                for (embedder, _, norm) in contributions {
                    let weight = weights.get(embedder).copied().unwrap_or(1.0);
                    total += norm * weight;
                    weight_sum += weight;
                }
                if weight_sum > 0.0 {
                    total / weight_sum
                } else {
                    0.0
                }
            }
        }
    }

    /// Get the underlying registry.
    pub fn registry(&self) -> &Arc<EmbedderIndexRegistry> {
        self.single_search.registry()
    }

    /// Get the configuration.
    pub fn config(&self) -> &MultiEmbedderSearchConfig {
        &self.config
    }
}
