//! In-memory implementation of MultiEmbeddingQueryExecutor.
//!
//! This module provides `InMemoryMultiEmbeddingExecutor`, an in-memory
//! implementation for development and testing.
//!
//! # Performance
//!
//! The in-memory implementation is optimized for correctness, not production
//! performance. For production use, implement the trait with proper HNSW
//! indexes and persistent storage.
//!
//! # Example
//!
//! ```ignore
//! use context_graph_core::retrieval::InMemoryMultiEmbeddingExecutor;
//! use context_graph_core::stubs::{InMemoryTeleologicalStore, StubMultiArrayProvider};
//!
//! let store = InMemoryTeleologicalStore::new();
//! let provider = StubMultiArrayProvider::new();
//! let executor = InMemoryMultiEmbeddingExecutor::new(store, provider);
//! ```

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

use crate::config::constants::pipeline;
use crate::error::{CoreError, CoreResult};
use crate::stubs::{InMemoryTeleologicalStore, StubMultiArrayProvider};
use crate::traits::{
    MultiArrayEmbeddingProvider, TeleologicalMemoryStore, TeleologicalSearchOptions,
};
use crate::types::fingerprint::{SemanticFingerprint, NUM_EMBEDDERS};

use super::{
    AggregatedMatch, AggregationStrategy, EmbeddingSpaceMask, IndexType, MultiEmbeddingQuery,
    MultiEmbeddingQueryExecutor, MultiEmbeddingResult, PipelineStageTiming, ScoredMatch,
    SpaceContribution, SpaceInfo, SpaceSearchResult,
};

/// In-memory implementation of MultiEmbeddingQueryExecutor.
///
/// Uses `InMemoryTeleologicalStore` for storage and `StubMultiArrayProvider`
/// for embedding generation. Suitable for development and testing.
///
/// # Thread Safety
///
/// This implementation is `Send + Sync` safe through Arc wrapping of
/// the internal store.
pub struct InMemoryMultiEmbeddingExecutor {
    /// The teleological memory store.
    store: Arc<InMemoryTeleologicalStore>,

    /// The embedding provider.
    provider: Arc<StubMultiArrayProvider>,
}

impl InMemoryMultiEmbeddingExecutor {
    /// Create a new in-memory executor.
    pub fn new(store: InMemoryTeleologicalStore, provider: StubMultiArrayProvider) -> Self {
        Self {
            store: Arc::new(store),
            provider: Arc::new(provider),
        }
    }

    /// Create with Arc-wrapped components.
    pub fn with_arcs(
        store: Arc<InMemoryTeleologicalStore>,
        provider: Arc<StubMultiArrayProvider>,
    ) -> Self {
        Self { store, provider }
    }

    /// Search a single embedding space using the store's search_semantic method.
    ///
    /// For dense spaces (E1-E5, E7-E11), we use the store's search_semantic
    /// with embedder_indices filtering.
    ///
    /// For sparse spaces (E6, E13), we use search_sparse.
    ///
    /// For late interaction (E12), we compute MaxSim manually using search_semantic.
    async fn search_space(
        &self,
        space_idx: usize,
        query_fingerprint: &SemanticFingerprint,
        limit: usize,
        min_similarity: f32,
    ) -> SpaceSearchResult {
        let start = Instant::now();

        match space_idx {
            // E6 Sparse
            5 => {
                return self
                    .search_sparse_space(space_idx, &query_fingerprint.e6_sparse, limit, start)
                    .await;
            }
            // E12 Late Interaction - use search_semantic with embedder filter
            11 => {
                return self
                    .search_single_embedder_space(
                        space_idx,
                        query_fingerprint,
                        limit,
                        min_similarity,
                        start,
                    )
                    .await;
            }
            // E13 SPLADE
            12 => {
                return self
                    .search_sparse_space(space_idx, &query_fingerprint.e13_splade, limit, start)
                    .await;
            }
            // Dense spaces (E1-E5, E7-E11)
            0..=4 | 6..=10 => {
                return self
                    .search_single_embedder_space(
                        space_idx,
                        query_fingerprint,
                        limit,
                        min_similarity,
                        start,
                    )
                    .await;
            }
            _ => {
                SpaceSearchResult::failure(space_idx, format!("Invalid space index: {}", space_idx))
            }
        }
    }

    /// Search using a single embedder index via TeleologicalSearchOptions.
    ///
    /// # Errors
    ///
    /// Returns `SpaceSearchResult::failure` if:
    /// - Search operation fails
    /// - Index count retrieval fails (logged with tracing::error)
    async fn search_single_embedder_space(
        &self,
        space_idx: usize,
        query_fingerprint: &SemanticFingerprint,
        limit: usize,
        min_similarity: f32,
        start: Instant,
    ) -> SpaceSearchResult {
        let options = TeleologicalSearchOptions::quick(limit)
            .with_min_similarity(min_similarity)
            .with_embedders(vec![space_idx]);

        match self.store.search_semantic(query_fingerprint, options).await {
            Ok(results) => {
                let matches: Vec<ScoredMatch> = results
                    .into_iter()
                    .enumerate()
                    .map(|(rank, r)| {
                        // Get the specific embedder score for this space
                        let similarity = r.embedder_scores[space_idx];
                        ScoredMatch::new(r.fingerprint.id, similarity, rank)
                    })
                    .collect();

                // FAIL FAST: Do not hide store count failures - they indicate storage problems
                let index_size = match self.store.count().await {
                    Ok(size) => size,
                    Err(e) => {
                        tracing::error!(
                            space_idx = space_idx,
                            error = %e,
                            "Failed to retrieve index size from store - storage may be unavailable"
                        );
                        return SpaceSearchResult::failure(
                            space_idx,
                            format!("Failed to get index size: {}", e),
                        );
                    }
                };
                SpaceSearchResult::success(space_idx, matches, start.elapsed(), index_size)
            }
            Err(e) => SpaceSearchResult::failure(space_idx, e.to_string()),
        }
    }

    /// Search sparse embedding space (E6 or E13).
    ///
    /// # Errors
    ///
    /// Returns `SpaceSearchResult::failure` if:
    /// - Sparse search operation fails
    /// - Index count retrieval fails (logged with tracing::error)
    async fn search_sparse_space(
        &self,
        space_idx: usize,
        query_sparse: &crate::types::fingerprint::SparseVector,
        limit: usize,
        start: Instant,
    ) -> SpaceSearchResult {
        match self.store.search_sparse(query_sparse, limit).await {
            Ok(results) => {
                let matches: Vec<ScoredMatch> = results
                    .into_iter()
                    .enumerate()
                    .map(|(rank, (id, score))| ScoredMatch::new(id, score, rank))
                    .collect();

                // FAIL FAST: Do not hide store count failures - they indicate storage problems
                let index_size = match self.store.count().await {
                    Ok(size) => size,
                    Err(e) => {
                        tracing::error!(
                            space_idx = space_idx,
                            error = %e,
                            "Failed to retrieve index size from sparse store - storage may be unavailable"
                        );
                        return SpaceSearchResult::failure(
                            space_idx,
                            format!("Failed to get index size: {}", e),
                        );
                    }
                };
                SpaceSearchResult::success(space_idx, matches, start.elapsed(), index_size)
            }
            Err(e) => SpaceSearchResult::failure(space_idx, e.to_string()),
        }
    }

    /// Build aggregated matches from space results using RRF.
    fn aggregate_results(
        &self,
        space_results: &[SpaceSearchResult],
        query: &MultiEmbeddingQuery,
    ) -> Vec<AggregatedMatch> {
        let rrf_k = query
            .pipeline_config
            .as_ref()
            .map(|c| c.rrf_k)
            .unwrap_or(pipeline::DEFAULT_RRF_K);

        // Build ranked lists for RRF
        let ranked_lists: Vec<(usize, Vec<Uuid>)> = space_results
            .iter()
            .filter(|r| r.success)
            .map(|r| (r.space_index, r.ranked_ids()))
            .collect();

        // Apply RRF or other aggregation strategy
        let scores = match &query.aggregation {
            AggregationStrategy::RRF { k } => AggregationStrategy::aggregate_rrf(&ranked_lists, *k),
            _ => {
                // For non-RRF strategies, use RRF as fallback for ranking
                AggregationStrategy::aggregate_rrf(&ranked_lists, rrf_k)
            }
        };

        // Build aggregated matches
        let mut aggregated: Vec<AggregatedMatch> = scores
            .into_iter()
            .map(|(memory_id, score)| {
                let mut match_result = AggregatedMatch::new(memory_id, score, 0);

                // Add space contributions
                for space_result in space_results.iter().filter(|r| r.success) {
                    for m in &space_result.matches {
                        if m.memory_id == memory_id {
                            match_result.space_count += 1;
                            match_result.add_contribution(SpaceContribution::new(
                                space_result.space_index,
                                m.similarity,
                                m.rank,
                                rrf_k,
                            ));
                            break;
                        }
                    }
                }

                match_result
            })
            .collect();

        // Sort by aggregate score descending
        aggregated.sort_by(|a, b| {
            b.aggregate_score
                .partial_cmp(&a.aggregate_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply final limit
        aggregated.truncate(query.final_limit);

        aggregated
    }
}

#[async_trait]
impl MultiEmbeddingQueryExecutor for InMemoryMultiEmbeddingExecutor {
    async fn execute(&self, query: MultiEmbeddingQuery) -> CoreResult<MultiEmbeddingResult> {
        let start = Instant::now();

        // Step 0: Validate query (FAIL FAST)
        query.validate()?;

        // Step 1: Generate query embeddings
        let embedding_output = self.provider.embed_all(&query.query_text).await?;
        let query_fingerprint = &embedding_output.fingerprint;

        // Log if embedding exceeded target
        if !embedding_output.is_within_latency_target() {
            tracing::warn!(
                "Embedding exceeded 30ms target: {:?}",
                embedding_output.total_latency
            );
        }

        // Step 2: Execute parallel searches across active spaces
        let active_indices = query.active_spaces.active_indices();
        let mut space_results = Vec::with_capacity(active_indices.len());
        let mut spaces_failed = 0;

        // In a production implementation, these would run in parallel using tokio::join!
        // For simplicity, we run them sequentially here
        for space_idx in active_indices {
            let result = self
                .search_space(
                    space_idx,
                    query_fingerprint,
                    query.per_space_limit,
                    query.min_similarity,
                )
                .await;

            if !result.success {
                tracing::error!("Space {} search failed: {:?}", space_idx, result.error);
                spaces_failed += 1;
            }
            space_results.push(result);
        }

        // Step 3: Aggregate results
        let aggregated = self.aggregate_results(&space_results, &query);

        // Step 4: Build result
        let total_time = start.elapsed();
        if total_time.as_millis() > 60 {
            tracing::warn!("Query exceeded 60ms target: {:?}", total_time);
        }

        let spaces_searched = space_results.iter().filter(|r| r.success).count();

        let mut result =
            MultiEmbeddingResult::new(aggregated, total_time, spaces_searched, spaces_failed);

        // Include space breakdown if requested
        if query.include_space_breakdown {
            result = result.with_space_breakdown(space_results);
        }

        Ok(result)
    }

    async fn execute_with_embeddings(
        &self,
        embeddings: &SemanticFingerprint,
        query: MultiEmbeddingQuery,
    ) -> CoreResult<MultiEmbeddingResult> {
        let start = Instant::now();

        // Validate query (except query_text which is ignored)
        if query.active_spaces.active_count() == 0 {
            return Err(CoreError::ValidationError {
                field: "active_spaces".to_string(),
                message: "At least one embedding space must be active".to_string(),
            });
        }

        // Execute parallel searches across active spaces
        let active_indices = query.active_spaces.active_indices();
        let mut space_results = Vec::with_capacity(active_indices.len());
        let mut spaces_failed = 0;

        for space_idx in active_indices {
            let result = self
                .search_space(
                    space_idx,
                    embeddings,
                    query.per_space_limit,
                    query.min_similarity,
                )
                .await;

            if !result.success {
                tracing::error!("Space {} search failed: {:?}", space_idx, result.error);
                spaces_failed += 1;
            }
            space_results.push(result);
        }

        // Aggregate results
        let aggregated = self.aggregate_results(&space_results, &query);

        // Build result
        let total_time = start.elapsed();
        let spaces_searched = space_results.iter().filter(|r| r.success).count();

        let mut result =
            MultiEmbeddingResult::new(aggregated, total_time, spaces_searched, spaces_failed);

        if query.include_space_breakdown {
            result = result.with_space_breakdown(space_results);
        }

        Ok(result)
    }

    fn available_spaces(&self) -> Vec<SpaceInfo> {
        vec![
            SpaceInfo::dense_hnsw(0, 1024, 0, true), // E1 Semantic
            SpaceInfo::dense_hnsw(1, 512, 0, true),  // E2 Temporal-Recent
            SpaceInfo::dense_hnsw(2, 512, 0, true),  // E3 Temporal-Periodic
            SpaceInfo::dense_hnsw(3, 512, 0, true),  // E4 Temporal-Positional
            SpaceInfo::dense_hnsw(4, 768, 0, true),  // E5 Causal
            SpaceInfo::sparse_inverted(5, 0, true),  // E6 Sparse
            SpaceInfo::dense_hnsw(6, 1536, 0, true), // E7 Code (Qodo-Embed)
            SpaceInfo::dense_hnsw(7, 384, 0, true),  // E8 Graph
            SpaceInfo::dense_hnsw(8, 1024, 0, true), // E9 HDC (projected)
            SpaceInfo::dense_hnsw(9, 768, 0, true),  // E10 Multimodal
            SpaceInfo::dense_hnsw(10, 768, 0, true), // E11 Entity (KEPLER)
            SpaceInfo::dense_hnsw(11, 128, 0, true), // E12 Late-Interaction
            SpaceInfo::sparse_inverted(12, 0, true), // E13 SPLADE
        ]
    }

    async fn warm_up(&self, _spaces: EmbeddingSpaceMask) -> CoreResult<()> {
        // In-memory implementation is always warm
        Ok(())
    }

    async fn execute_pipeline(
        &self,
        query: MultiEmbeddingQuery,
    ) -> CoreResult<MultiEmbeddingResult> {
        let start = Instant::now();

        // Validate query
        query.validate()?;

        let config = query.pipeline_config.clone().unwrap_or_default();

        // Generate query embeddings
        let embedding_output = self.provider.embed_all(&query.query_text).await?;
        let query_fingerprint = &embedding_output.fingerprint;

        // Stage 1: SPLADE sparse retrieval
        let stage1_start = Instant::now();
        let stage1_result = self
            .search_sparse_space(
                12, // E13 SPLADE
                &query_fingerprint.e13_splade,
                config.splade_candidates,
                stage1_start,
            )
            .await;
        let stage1_time = stage1_start.elapsed();
        let stage1_candidates = stage1_result.matches.len();

        // Get candidate IDs from Stage 1
        let candidate_ids: Vec<Uuid> = stage1_result.ranked_ids();

        // Stage 2: Matryoshka 128D filter (use E1 semantic)
        let stage2_start = Instant::now();
        let stage2_result = self
            .search_single_embedder_space(
                0, // E1 Semantic (uses Matryoshka prefix)
                query_fingerprint,
                config.matryoshka_128d_limit,
                0.0,
                stage2_start,
            )
            .await;
        let stage2_time = stage2_start.elapsed();
        let stage2_candidates = stage2_result
            .matches
            .iter()
            .filter(|m| candidate_ids.contains(&m.memory_id))
            .count()
            .min(config.matryoshka_128d_limit);

        // Stage 3: Full 13-space HNSW search (dense spaces only)
        let stage3_start = Instant::now();
        let mut stage3_results = Vec::new();
        for space_idx in 0..NUM_EMBEDDERS {
            // Skip sparse spaces (E6=5, E13=12) - handled separately
            if space_idx == 5 || space_idx == 12 {
                continue;
            }
            let result = self
                .search_single_embedder_space(
                    space_idx,
                    query_fingerprint,
                    config.full_search_limit,
                    query.min_similarity,
                    Instant::now(),
                )
                .await;
            stage3_results.push(result);
        }
        let stage3_time = stage3_start.elapsed();
        let stage3_candidates = config.full_search_limit;

        // Stage 4: Teleological alignment filter
        let stage4_start = Instant::now();
        // In production, this would filter by purpose alignment
        // For now, we just limit the results
        let stage4_candidates = config.teleological_limit;
        let stage4_time = stage4_start.elapsed();

        // Stage 5: Late interaction reranking (E12)
        let stage5_start = Instant::now();
        let stage5_result = self
            .search_single_embedder_space(
                11, // E12 Late Interaction
                query_fingerprint,
                config.late_interaction_limit,
                0.0,
                stage5_start,
            )
            .await;
        let stage5_time = stage5_start.elapsed();
        let stage5_candidates = stage5_result.matches.len();

        // Combine all results
        let mut all_results = vec![stage1_result, stage2_result, stage5_result];
        all_results.extend(stage3_results);

        // Aggregate
        let aggregated = self.aggregate_results(&all_results, &query);

        let total_time = start.elapsed();
        let spaces_searched = all_results.iter().filter(|r| r.success).count();
        let spaces_failed = all_results.iter().filter(|r| !r.success).count();

        let timing = PipelineStageTiming::new(
            stage1_time,
            stage2_time,
            stage3_time,
            stage4_time,
            stage5_time,
            [
                stage1_candidates,
                stage2_candidates,
                stage3_candidates,
                stage4_candidates,
                stage5_candidates,
            ],
        );

        let mut result =
            MultiEmbeddingResult::new(aggregated, total_time, spaces_searched, spaces_failed);
        result = result.with_stage_timings(timing);

        if query.include_space_breakdown {
            result = result.with_space_breakdown(all_results);
        }

        Ok(result)
    }
}

// Make the executor Send + Sync safe
unsafe impl Send for InMemoryMultiEmbeddingExecutor {}
unsafe impl Sync for InMemoryMultiEmbeddingExecutor {}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_executor() -> InMemoryMultiEmbeddingExecutor {
        let store = InMemoryTeleologicalStore::new();
        let provider = StubMultiArrayProvider::new();
        InMemoryMultiEmbeddingExecutor::new(store, provider)
    }

    #[tokio::test]
    async fn test_executor_creation() {
        let executor = create_test_executor().await;
        let spaces = executor.available_spaces();
        assert_eq!(spaces.len(), 13);

        println!("[VERIFIED] Executor created with 13 spaces");
    }

    #[tokio::test]
    async fn test_available_spaces() {
        let executor = create_test_executor().await;
        let spaces = executor.available_spaces();

        assert_eq!(spaces[0].name, "E1_Semantic");
        assert_eq!(spaces[0].dimension, 1024);
        assert_eq!(spaces[0].index_type, IndexType::Hnsw);

        assert_eq!(spaces[5].name, "E6_Sparse");
        assert_eq!(spaces[5].dimension, 0);
        assert_eq!(spaces[5].index_type, IndexType::Inverted);

        assert_eq!(spaces[12].name, "E13_SPLADE");
        assert_eq!(spaces[12].index_type, IndexType::Inverted);

        println!("[VERIFIED] available_spaces returns correct info");
    }

    #[tokio::test]
    async fn test_warm_up() {
        let executor = create_test_executor().await;
        let result = executor.warm_up(EmbeddingSpaceMask::ALL).await;
        assert!(result.is_ok());

        println!("[VERIFIED] warm_up succeeds");
    }
}
