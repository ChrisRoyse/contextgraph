//! Search operations for TeleologicalMemoryStore trait.
//!
//! Contains semantic, purpose, text, and sparse search implementations.
//!
//! # Multi-Space Search Strategies (TASK-MULTISPACE)
//!
//! Three search strategies are supported:
//!
//! - **E1Only**: Original E1-only HNSW search (backward compatible)
//! - **MultiSpace**: Weighted fusion of semantic embedders
//! - **Pipeline**: Full 3-stage retrieval
//!
//! ## Key Research Insights
//!
//! Temporal embedders (E2-E4) measure TIME proximity, not TOPIC similarity.
//! They are excluded from similarity scoring and applied as post-retrieval boosts.
//!
//! # Fusion Strategies (ARCH-18)
//!
//! When using MultiSpace or Pipeline strategies, score fusion can use:
//!
//! - **WeightedSum** (legacy): Simple weighted sum of similarity scores
//! - **WeightedRRF** (default per ARCH-18): Weighted Reciprocal Rank Fusion
//!
//! RRF formula: `RRF_score(d) = Sum(weight_i / (rank_i + k))`
//!
//! RRF is more robust to score distribution differences between embedders.
//!
//! # ARCH-16 Compliance
//!
//! E7 Code embedder uses query-type-aware similarity computation:
//! - **Code2Code**: Query is actual code syntax (e.g., "fn process<T>")
//! - **Text2Code**: Query is natural language about code (e.g., "batch function")
//! - **NonCode**: Query is not code-related
//!
//! When `query_text` is provided in search options, the query type is auto-detected.
//!
//! References:
//! - [Cascading Retrieval](https://www.pinecone.io/blog/cascading-retrieval/)
//! - [Fusion Analysis](https://dl.acm.org/doi/10.1145/3596512)
//! - [Elastic Weighted RRF](https://www.elastic.co/blog/weighted-reciprocal-rank-fusion-rrf)

use std::collections::{HashMap, HashSet};

use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::fusion::{EmbedderRanking, FusionStrategy, fuse_rankings};
use context_graph_core::causal::asymmetric::CausalDirection;
use context_graph_core::traits::{SearchStrategy, TeleologicalSearchOptions, TeleologicalSearchResult};
use context_graph_core::types::fingerprint::{SemanticFingerprint, SparseVector};

use crate::teleological::search::temporal_boost;

use crate::teleological::column_families::CF_E13_SPLADE_INVERTED;
use crate::teleological::indexes::{EmbedderIndex, EmbedderIndexOps};
use crate::teleological::schema::e13_splade_inverted_key;
use crate::teleological::serialization::{
    deserialize_memory_id_list, deserialize_teleological_fingerprint,
};

use super::store::RocksDbTeleologicalStore;
use super::types::TeleologicalStoreError;

use context_graph_core::code::CodeQueryType;
use context_graph_core::types::fingerprint::TeleologicalFingerprint;
use context_graph_core::weights::get_weight_profile;

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Sequential CPU computation of embedder scores for candidates.
/// Used when GPU batching is not available or for small candidate counts.
fn compute_scores_sequential(
    store: &RocksDbTeleologicalStore,
    query: &SemanticFingerprint,
    candidates: Vec<(Uuid, TeleologicalFingerprint)>,
    code_query_type: Option<CodeQueryType>,
    causal_direction: CausalDirection,
) -> Vec<(Uuid, [f32; 13], SemanticFingerprint)> {
    candidates
        .into_iter()
        .map(|(id, fp)| {
            let embedder_scores = if causal_direction != CausalDirection::Unknown {
                store.compute_embedder_scores_with_direction(
                    query,
                    &fp.semantic,
                    code_query_type,
                    causal_direction,
                )
            } else {
                store.compute_embedder_scores_with_code_query_type(
                    query,
                    &fp.semantic,
                    code_query_type,
                )
            };
            (id, embedder_scores, fp.semantic)
        })
        .collect()
}

// =============================================================================
// SEMANTIC EMBEDDER INDICES (for multi-space scoring)
// Per constitution: Temporal (E2-E4) have weight 0.0 in semantic search
// =============================================================================

/// Semantic embedder indices that contribute to similarity scoring.
/// Per constitution (ARCH-13, AP-71, AP-73, AP-74):
/// - E2-E4 (temporal) are EXCLUDED - used for post-retrieval boosts
/// - E12 (ColBERT) is EXCLUDED - used in Stage 3 rerank only (AP-73)
/// - E13 (SPLADE) is EXCLUDED - used in Stage 1 recall only (AP-74)
#[allow(dead_code)]
const SEMANTIC_EMBEDDER_INDICES: [usize; 6] = [
    0,  // E1 - Semantic
    4,  // E5 - Causal (asymmetric per ARCH-15)
    5,  // E6 - Sparse
    6,  // E7 - Code (query-type aware per ARCH-16)
    9,  // E10 - Multimodal
    10, // E11 - Entity (with entity linking per ARCH-17)
];

/// Default weights for semantic search profile.
/// Sum of non-zero weights = 1.0
/// E2-E4 (temporal) = 0.0 per research
const DEFAULT_SEMANTIC_WEIGHTS: [f32; 13] = [
    0.35, // E1 - Semantic (primary)
    0.0,  // E2 - Temporal Recent (metadata only)
    0.0,  // E3 - Temporal Periodic (metadata only)
    0.0,  // E4 - Temporal Positional (metadata only)
    0.15, // E5 - Causal
    0.05, // E6 - Sparse (keyword backup)
    0.20, // E7 - Code
    0.05, // E8 - Graph (relational)
    0.0,  // E9 - HDC (noise-robust backup, not used in fusion)
    0.15, // E10 - Multimodal
    0.05, // E11 - Entity (relational)
    0.0,  // E12 - Late Interaction (used in Stage 3 rerank only)
    0.0,  // E13 - SPLADE (used in Stage 1 recall only)
];

// =============================================================================
// PIPELINE CONFIGURATION
// Per AP-76: Stage 1 (sparse recall) → Stage 2 (dense scoring) → Stage 3 (rerank)
// =============================================================================

/// Stage 1 recall multiplier (how many candidates to retrieve)
const STAGE1_RECALL_MULTIPLIER: usize = 10;

/// Stage 2 scoring keeps this many candidates for optional re-ranking
const STAGE2_CANDIDATE_MULTIPLIER: usize = 3;

// =============================================================================
// WEIGHTED FUSION FUNCTIONS
// =============================================================================

/// Compute weighted fusion of embedder scores.
///
/// Only uses embedders with non-zero weights (semantic embedders).
/// Temporal embedders (E2-E4) have weight 0.0 per AP-71.
///
/// # Arguments
///
/// * `scores` - All 13 embedder similarity scores
/// * `weights` - Weight profile (must have 13 elements)
///
/// # Returns
///
/// Weighted average of scores (0.0-1.0)
fn compute_semantic_fusion(scores: &[f32; 13], weights: &[f32; 13]) -> f32 {
    let mut weighted_sum = 0.0f32;
    let mut weight_total = 0.0f32;

    for (&score, &weight) in scores.iter().zip(weights.iter()) {
        // Skip zero-weight embedders (temporal)
        if weight > 0.0 {
            weighted_sum += score * weight;
            weight_total += weight;
        }
    }

    if weight_total > 0.0 {
        weighted_sum / weight_total
    } else {
        // Fallback to E1 if all weights are zero
        scores[0]
    }
}

impl RocksDbTeleologicalStore {
    /// Search by semantic fingerprint (internal async wrapper).
    ///
    /// Supports three strategies:
    /// - `E1Only`: Original E1-only HNSW search (backward compatible)
    /// - `MultiSpace`: Weighted fusion of semantic embedders
    /// - `Pipeline`: Full 3-stage retrieval
    ///
    /// Also supports temporal options (ARCH-14):
    /// - E2 Recency: Decay functions, time windows, session filtering
    /// - E3 Periodic: Hour-of-day, day-of-week pattern matching
    /// - E4 Sequence: Before/after anchor memory retrieval
    pub(crate) async fn search_semantic_async(
        &self,
        query: &SemanticFingerprint,
        options: TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        debug!(
            "Searching semantic with strategy={:?}, top_k={}, min_similarity={}, temporal_weight={}",
            options.strategy, options.top_k, options.min_similarity,
            options.temporal_options.temporal_weight
        );

        // Branch based on search strategy
        let mut results = match options.strategy {
            SearchStrategy::E1Only => {
                self.search_e1_only(query, &options).await?
            }
            SearchStrategy::MultiSpace => {
                self.search_multi_space(query, &options).await?
            }
            SearchStrategy::Pipeline => {
                // Full 3-stage pipeline per AP-76:
                // Stage 1 (Recall): E13 SPLADE + E1 HNSW
                // Stage 2 (Scoring): Weighted fusion of semantic embedders
                // Stage 3 (Re-rank): Optional E12 ColBERT MaxSim
                self.search_pipeline(query, &options).await?
            }
        };

        // Apply time window filter if configured
        if let Some(ref window) = options.temporal_options.time_window {
            if window.is_defined() {
                temporal_boost::filter_by_time_window(&mut results, window, |r| {
                    r.fingerprint.created_at.timestamp_millis()
                });
            }
        }

        // Apply legacy recency boost if configured (backward compatibility)
        // Note: recency_boost is deprecated, use temporal_options.temporal_weight instead
        #[allow(deprecated)]
        if options.recency_boost > 0.0 {
            #[allow(deprecated)]
            self.apply_recency_boost(&mut results, query, options.recency_boost);
        }

        // Apply full temporal boost system (ARCH-14) if configured
        if options.temporal_options.has_any_boost() {
            self.apply_full_temporal_boosts(&mut results, query, &options).await?;
        }

        debug!("Semantic search returned {} results", results.len());
        Ok(results)
    }

    /// Original E1-only search (backward compatible).
    ///
    /// Uses only E1 Semantic HNSW index for ranking.
    /// Fastest option, suitable for simple queries.
    async fn search_e1_only(
        &self,
        query: &SemanticFingerprint,
        options: &TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        // Search E1 Semantic as primary
        let entry_embedder = EmbedderIndex::E1Semantic;
        let entry_index = self.index_registry.get(entry_embedder).ok_or_else(|| {
            CoreError::IndexError(format!("Index {:?} not found", entry_embedder))
        })?;

        // Search E1 semantic space with 2x top_k to allow filtering
        let k = (options.top_k * 2).max(20);
        let candidates = entry_index
            .search(&query.e1_semantic, k, None)
            .map_err(|e| {
                error!("E1 search failed: {}", e);
                CoreError::IndexError(e.to_string())
            })?;

        // Fetch full fingerprints for candidates
        let mut results = Vec::with_capacity(candidates.len());

        for (id, distance) in candidates {
            // Convert distance to similarity (HNSW returns distance, not similarity)
            let similarity = 1.0 - distance.min(1.0);

            // Skip soft-deleted
            if !options.include_deleted && self.is_soft_deleted(&id) {
                continue;
            }

            // Apply min_similarity filter
            if similarity < options.min_similarity {
                continue;
            }

            // Fetch full fingerprint from RocksDB
            if let Some(data) = self.get_fingerprint_raw(id)? {
                let fp = deserialize_teleological_fingerprint(&data);

                // Compute all 13 embedder scores with E7 query-type awareness (ARCH-16)
                // and direction-aware E5 similarity (ARCH-15, AP-77)
                let code_query_type = options.effective_code_query_type();
                let embedder_scores = if options.causal_direction != CausalDirection::Unknown {
                    self.compute_embedder_scores_with_direction(
                        query,
                        &fp.semantic,
                        code_query_type,
                        options.causal_direction,
                    )
                } else {
                    self.compute_embedder_scores_with_code_query_type(
                        query,
                        &fp.semantic,
                        code_query_type,
                    )
                };

                results.push(TeleologicalSearchResult::new(fp, similarity, embedder_scores));
            }
        }

        // Sort by similarity descending
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to top_k
        results.truncate(options.top_k);

        Ok(results)
    }

    /// Multi-space weighted fusion search.
    ///
    /// Uses weighted combination of semantic embedders (E1, E5, E7, E10).
    /// Temporal embedders (E2-E4) have weight 0.0 per AP-71.
    ///
    /// Supports two fusion strategies (ARCH-18):
    /// - `WeightedSum`: Legacy weighted sum of similarity scores
    /// - `WeightedRRF`: Weighted Reciprocal Rank Fusion (default)
    async fn search_multi_space(
        &self,
        query: &SemanticFingerprint,
        options: &TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        let weights = self.resolve_weights(options)?;
        let k = (options.top_k * 3).max(50);

        // =========================================================================
        // STEP 1: Get ranked results from each embedder for RRF fusion
        // =========================================================================
        let mut embedder_rankings: Vec<EmbedderRanking> = Vec::new();

        // E1 Semantic (primary dense retrieval)
        let entry_embedder = EmbedderIndex::E1Semantic;
        let entry_index = self.index_registry.get(entry_embedder).ok_or_else(|| {
            CoreError::IndexError(format!("Index {:?} not found", entry_embedder))
        })?;

        let e1_candidates = entry_index
            .search(&query.e1_semantic, k, None)
            .map_err(|e| {
                error!("E1 search failed: {}", e);
                CoreError::IndexError(e.to_string())
            })?;

        // Filter soft-deleted and convert distance to similarity
        let e1_ranked: Vec<(Uuid, f32)> = e1_candidates
            .into_iter()
            .filter(|(id, _)| options.include_deleted || !self.is_soft_deleted(id))
            .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
            .collect();

        if !e1_ranked.is_empty() {
            embedder_rankings.push(EmbedderRanking::new("E1", weights[0], e1_ranked));
        }

        // E5 Causal with asymmetric retrieval (ARCH-15, AP-77)
        // Direction-aware index selection:
        // - Cause-seeking queries (why X?) -> search E5CausalEffect using query.e5_as_cause
        // - Effect-seeking queries (what happens when X?) -> search E5CausalCause using query.e5_as_effect
        // - Unknown direction -> use legacy E5Causal index with active vector
        let e5_results = self.search_e5_asymmetric(query, options.causal_direction, k);
        if let Ok(e5_candidates) = e5_results {
            let e5_ranked: Vec<(Uuid, f32)> = e5_candidates
                .into_iter()
                .filter(|(id, _)| options.include_deleted || !self.is_soft_deleted(id))
                .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
                .collect();

            if !e5_ranked.is_empty() && weights[4] > 0.0 {
                let e5_label = match options.causal_direction {
                    CausalDirection::Cause => "E5_Cause",
                    CausalDirection::Effect => "E5_Effect",
                    CausalDirection::Unknown => "E5",
                };
                embedder_rankings.push(EmbedderRanking::new(e5_label, weights[4], e5_ranked));
            }
        }

        // E7 Code (if available)
        if let Some(e7_index) = self.index_registry.get(EmbedderIndex::E7Code) {
            if let Ok(e7_candidates) = e7_index.search(&query.e7_code, k, None) {
                let e7_ranked: Vec<(Uuid, f32)> = e7_candidates
                    .into_iter()
                    .filter(|(id, _)| options.include_deleted || !self.is_soft_deleted(id))
                    .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
                    .collect();

                if !e7_ranked.is_empty() && weights[6] > 0.0 {
                    embedder_rankings.push(EmbedderRanking::new("E7", weights[6], e7_ranked));
                }
            }
        }

        // E10 Multimodal (if available)
        // FIX: Use e10_active_vector() instead of legacy e10_multimodal field
        if let Some(e10_index) = self.index_registry.get(EmbedderIndex::E10Multimodal) {
            if let Ok(e10_candidates) = e10_index.search(query.e10_active_vector(), k, None) {
                let e10_ranked: Vec<(Uuid, f32)> = e10_candidates
                    .into_iter()
                    .filter(|(id, _)| options.include_deleted || !self.is_soft_deleted(id))
                    .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
                    .collect();

                if !e10_ranked.is_empty() && weights[9] > 0.0 {
                    embedder_rankings.push(EmbedderRanking::new("E10", weights[9], e10_ranked));
                }
            }
        }

        debug!(
            "Multi-space search: {} embedder rankings collected, fusion_strategy={:?}",
            embedder_rankings.len(),
            options.fusion_strategy
        );

        // =========================================================================
        // STEP 2: Fuse results using configured strategy (ARCH-18)
        // =========================================================================
        let fused_results = fuse_rankings(
            &embedder_rankings,
            options.fusion_strategy,
            options.top_k * 2, // Get extra candidates for filtering
        );

        debug!(
            "Fusion produced {} candidates using {:?}",
            fused_results.len(),
            options.fusion_strategy
        );

        // =========================================================================
        // STEP 3: Build final results with full embedder scores
        // =========================================================================
        let code_query_type = options.effective_code_query_type();
        let mut results = Vec::with_capacity(fused_results.len());

        for fused in fused_results {
            // Fetch full fingerprint from RocksDB
            if let Some(data) = self.get_fingerprint_raw(fused.doc_id)? {
                let fp = deserialize_teleological_fingerprint(&data);

                // Compute all 13 embedder scores with E7 query-type awareness (ARCH-16)
                // and direction-aware E5 similarity (ARCH-15, AP-77)
                let embedder_scores = if options.causal_direction != CausalDirection::Unknown {
                    self.compute_embedder_scores_with_direction(
                        query,
                        &fp.semantic,
                        code_query_type,
                        options.causal_direction,
                    )
                } else {
                    self.compute_embedder_scores_with_code_query_type(
                        query,
                        &fp.semantic,
                        code_query_type,
                    )
                };

                // For WeightedSum strategy, use the legacy score computation
                // For WeightedRRF, the fused_score is already the RRF score
                let final_score = match options.fusion_strategy {
                    FusionStrategy::WeightedSum => {
                        // Use direct weighted sum of computed embedder scores
                        compute_semantic_fusion(&embedder_scores, &weights)
                    }
                    FusionStrategy::WeightedRRF => {
                        // Use RRF score from fusion, normalized to [0, 1] range
                        // RRF scores are typically small (< 0.1), so we normalize
                        fused.fused_score * 10.0 // Scale for readability
                    }
                };

                // Apply min_similarity filter
                if final_score < options.min_similarity {
                    continue;
                }

                results.push(TeleologicalSearchResult::new(fp, final_score, embedder_scores));
            }
        }

        // Sort by fusion score descending
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to top_k
        results.truncate(options.top_k);

        debug!(
            "Multi-space search returned {} results with {:?} fusion",
            results.len(),
            options.fusion_strategy
        );
        Ok(results)
    }

    /// Full 3-stage pipeline search.
    ///
    /// Per AP-76: Stage 1 (sparse recall) → Stage 2 (dense scoring) → Stage 3 (optional rerank)
    ///
    /// # Stages
    ///
    /// 1. **Recall** (E13 SPLADE + E1 HNSW): Broad candidate generation
    /// 2. **Scoring** (Weighted fusion): Multi-space semantic scoring
    /// 3. **Re-rank** (E12 ColBERT MaxSim): Optional precise re-ranking
    ///
    /// # References
    ///
    /// - [Cascading Retrieval](https://www.pinecone.io/blog/cascading-retrieval/) - 48% improvement
    /// - [ColBERT Late Interaction](https://weaviate.io/blog/late-interaction-overview)
    async fn search_pipeline(
        &self,
        query: &SemanticFingerprint,
        options: &TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        let recall_k = options.top_k * STAGE1_RECALL_MULTIPLIER;
        let stage2_k = options.top_k * STAGE2_CANDIDATE_MULTIPLIER;

        info!(
            "Pipeline search: Stage1 recall_k={}, Stage2 candidates={}, final_k={}",
            recall_k, stage2_k, options.top_k
        );

        // =========================================================================
        // STAGE 1: FAST RECALL (E13 SPLADE inverted + E1 HNSW)
        // Per AP-75: E13 SPLADE for Stage 1 recall, NOT final ranking
        // =========================================================================
        let mut candidate_ids: HashSet<Uuid> = HashSet::new();

        // E13 SPLADE sparse recall (inverted index)
        if !query.e13_splade.is_empty() {
            match self.search_sparse_async(&query.e13_splade, recall_k).await {
                Ok(sparse_results) => {
                    let sparse_count = sparse_results.len();
                    candidate_ids.extend(sparse_results.into_iter().map(|(id, _)| id));
                    debug!("Stage 1: E13 SPLADE returned {} candidates", sparse_count);
                }
                Err(e) => {
                    warn!("Stage 1: E13 SPLADE search failed: {}, continuing with E1 only", e);
                }
            }
        } else {
            debug!("Stage 1: E13 SPLADE empty, skipping sparse recall");
        }

        // E1 Semantic HNSW recall (primary dense)
        let entry_embedder = EmbedderIndex::E1Semantic;
        if let Some(entry_index) = self.index_registry.get(entry_embedder) {
            match entry_index.search(&query.e1_semantic, recall_k, None) {
                Ok(e1_candidates) => {
                    let e1_count = e1_candidates.len();
                    candidate_ids.extend(e1_candidates.into_iter().map(|(id, _)| id));
                    debug!("Stage 1: E1 HNSW returned {} candidates", e1_count);
                }
                Err(e) => {
                    error!("Stage 1: E1 HNSW search failed: {}", e);
                    return Err(CoreError::IndexError(e.to_string()));
                }
            }
        } else {
            return Err(CoreError::IndexError("E1 Semantic index not found".into()));
        }

        // Also add candidates from E5 Causal for better recall on reasoning queries
        if let Some(e5_index) = self.index_registry.get(EmbedderIndex::E5Causal) {
            if let Ok(e5_candidates) = e5_index.search(query.e5_active_vector(), recall_k / 2, None) {
                let e5_count = e5_candidates.len();
                candidate_ids.extend(e5_candidates.into_iter().map(|(id, _)| id));
                debug!("Stage 1: E5 Causal returned {} additional candidates", e5_count);
            }
        }

        // Also add candidates from E7 Code for better recall on code queries
        if let Some(e7_index) = self.index_registry.get(EmbedderIndex::E7Code) {
            if let Ok(e7_candidates) = e7_index.search(&query.e7_code, recall_k / 2, None) {
                let e7_count = e7_candidates.len();
                candidate_ids.extend(e7_candidates.into_iter().map(|(id, _)| id));
                debug!("Stage 1: E7 Code returned {} additional candidates", e7_count);
            }
        }

        info!(
            "Stage 1 complete: {} unique candidates from E13+E1+E5+E7",
            candidate_ids.len()
        );

        if candidate_ids.is_empty() {
            debug!("Stage 1: No candidates found, returning empty results");
            return Ok(Vec::new());
        }

        // =========================================================================
        // STAGE 2: MULTI-SPACE SCORING (Weighted fusion of semantic embedders)
        // Per AP-73: Temporal (E2-E4) MUST NOT be used in similarity scoring
        // Per ARCH-16: E7 Code uses query-type-aware similarity
        // Per ARCH-18: Supports both WeightedSum and WeightedRRF fusion strategies
        // Per ARCH-GPU-06: Use GPU batch similarity when candidate count is high
        // =========================================================================
        let weights = self.resolve_weights(options)?;
        let code_query_type = options.effective_code_query_type();

        // First, fetch all fingerprints for valid candidates
        let mut valid_candidates: Vec<(Uuid, TeleologicalFingerprint)> =
            Vec::with_capacity(candidate_ids.len());

        for id in candidate_ids {
            // Skip soft-deleted
            if !options.include_deleted && self.is_soft_deleted(&id) {
                continue;
            }

            // Fetch full fingerprint from RocksDB
            if let Some(data) = self.get_fingerprint_raw(id)? {
                let fp = deserialize_teleological_fingerprint(&data);
                valid_candidates.push((id, fp));
            }
        }

        // Compute embedder scores - use GPU batch when available and beneficial
        let candidate_data: Vec<(Uuid, [f32; 13], SemanticFingerprint)> = {
            #[cfg(feature = "cuda")]
            {
                use context_graph_cuda::GPU_BATCH_THRESHOLD;

                if valid_candidates.len() >= GPU_BATCH_THRESHOLD {
                    // GPU batch path (ARCH-GPU-06)
                    debug!(
                        "Stage 2: Using GPU batch similarity for {} candidates",
                        valid_candidates.len()
                    );

                    // Collect references to semantic fingerprints
                    let semantic_refs: Vec<&SemanticFingerprint> = valid_candidates
                        .iter()
                        .map(|(_, fp)| &fp.semantic)
                        .collect();

                    // Compute all scores in batch on GPU
                    let all_scores = if options.causal_direction != CausalDirection::Unknown {
                        self.compute_embedder_scores_batch_with_direction(
                            query,
                            &semantic_refs,
                            code_query_type,
                            options.causal_direction,
                        )
                    } else {
                        self.compute_embedder_scores_batch(
                            query,
                            &semantic_refs,
                            code_query_type,
                        )
                    };

                    // Combine with fingerprint data
                    valid_candidates
                        .into_iter()
                        .zip(all_scores.into_iter())
                        .map(|((id, fp), scores)| (id, scores, fp.semantic))
                        .collect()
                } else {
                    // Fall through to CPU path for small batches
                    compute_scores_sequential(self, query, valid_candidates, code_query_type, options.causal_direction)
                }
            }

            #[cfg(not(feature = "cuda"))]
            {
                compute_scores_sequential(self, query, valid_candidates, code_query_type, options.causal_direction)
            }
        };

        // Build scored candidates based on fusion strategy (ARCH-18)
        let mut scored_candidates: Vec<(Uuid, f32, [f32; 13], SemanticFingerprint)> =
            match options.fusion_strategy {
                FusionStrategy::WeightedSum => {
                    // Legacy: Direct weighted sum of embedder scores
                    candidate_data
                        .into_iter()
                        .map(|(id, scores, semantic)| {
                            let fusion_score = compute_semantic_fusion(&scores, &weights);
                            (id, fusion_score, scores, semantic)
                        })
                        .filter(|(_, score, _, _)| *score >= options.min_similarity)
                        .collect()
                }
                FusionStrategy::WeightedRRF => {
                    // Build per-embedder rankings from candidates
                    let semantic_indices = [
                        (0, "E1", weights[0]),   // E1 Semantic
                        (4, "E5", weights[4]),   // E5 Causal
                        (6, "E7", weights[6]),   // E7 Code
                        (9, "E10", weights[9]),  // E10 Multimodal
                    ];

                    let mut embedder_rankings: Vec<EmbedderRanking> = Vec::new();

                    for (idx, name, weight) in semantic_indices {
                        if weight <= 0.0 {
                            continue;
                        }

                        // Sort candidates by this embedder's score
                        let mut ranked: Vec<(Uuid, f32)> = candidate_data
                            .iter()
                            .map(|(id, scores, _)| (*id, scores[idx]))
                            .collect();
                        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                        if !ranked.is_empty() {
                            embedder_rankings.push(EmbedderRanking::new(name, weight, ranked));
                        }
                    }

                    // Fuse using RRF
                    let fused = fuse_rankings(&embedder_rankings, FusionStrategy::WeightedRRF, stage2_k * 2);

                    // Map back to full candidate data
                    let candidate_map: HashMap<Uuid, ([f32; 13], SemanticFingerprint)> = candidate_data
                        .into_iter()
                        .map(|(id, scores, semantic)| (id, (scores, semantic)))
                        .collect();

                    fused
                        .into_iter()
                        .filter_map(|f| {
                            candidate_map.get(&f.doc_id).map(|(scores, semantic)| {
                                // Scale RRF score for readability (RRF scores are typically small)
                                let scaled_score = f.fused_score * 10.0;
                                (f.doc_id, scaled_score, *scores, semantic.clone())
                            })
                        })
                        .filter(|(_, score, _, _)| *score >= options.min_similarity)
                        .collect()
                }
            };

        // Sort by fusion score descending
        scored_candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Keep top candidates for Stage 3 (or final results if no rerank)
        scored_candidates.truncate(stage2_k);

        info!(
            "Stage 2 complete: {} candidates after {:?} fusion scoring",
            scored_candidates.len(),
            options.fusion_strategy
        );

        if scored_candidates.is_empty() {
            debug!("Stage 2: No candidates passed min_similarity filter");
            return Ok(Vec::new());
        }

        // =========================================================================
        // STAGE 3: OPTIONAL E12 COLBERT RE-RANKING (MaxSim)
        // Per AP-74: E12 ColBERT MUST only be used for re-ranking, NOT initial retrieval
        // =========================================================================
        let final_candidates = if options.enable_rerank && !query.e12_late_interaction.is_empty() {
            info!(
                "Stage 3: Applying E12 ColBERT MaxSim re-ranking (weight={})",
                options.rerank_weight
            );
            self.rerank_with_colbert(query, scored_candidates, options.top_k, options.rerank_weight)
        } else {
            if options.enable_rerank {
                debug!("Stage 3: Re-ranking requested but E12 query tokens empty, skipping");
            }
            // No re-ranking, just truncate to top_k
            scored_candidates.truncate(options.top_k);
            scored_candidates
        };

        info!(
            "Stage 3 complete: {} final results",
            final_candidates.len()
        );

        // =========================================================================
        // BUILD FINAL RESULTS
        // =========================================================================
        let mut results = Vec::with_capacity(final_candidates.len());

        for (id, score, embedder_scores, _semantic) in final_candidates {
            // Re-fetch the full TeleologicalFingerprint for the result
            if let Some(data) = self.get_fingerprint_raw(id)? {
                let fp = deserialize_teleological_fingerprint(&data);
                results.push(TeleologicalSearchResult::new(fp, score, embedder_scores));
            }
        }

        debug!("Pipeline search returned {} results", results.len());
        Ok(results)
    }

    /// Re-rank candidates using E12 ColBERT MaxSim scoring.
    ///
    /// Per AP-74: ColBERT is for re-ranking only.
    /// Uses token-level MaxSim for precise similarity scoring.
    ///
    /// # Arguments
    ///
    /// * `query` - Query semantic fingerprint with E12 tokens
    /// * `candidates` - Candidates from Stage 2 fusion
    /// * `top_k` - Number of results to return
    /// * `rerank_weight` - E12 weight for blending (0.0-1.0, default 0.4)
    fn rerank_with_colbert(
        &self,
        query: &SemanticFingerprint,
        mut candidates: Vec<(Uuid, f32, [f32; 13], SemanticFingerprint)>,
        top_k: usize,
        rerank_weight: f32,
    ) -> Vec<(Uuid, f32, [f32; 13], SemanticFingerprint)> {
        use crate::teleological::search::compute_maxsim_direct;

        // Pre-compute complementary weight for fusion score
        let fusion_weight = 1.0 - rerank_weight;

        // Compute MaxSim scores for all candidates
        for (_, score, embedder_scores, semantic) in candidates.iter_mut() {
            if !semantic.e12_late_interaction.is_empty() {
                // Compute E12 ColBERT MaxSim
                let maxsim_score = compute_maxsim_direct(
                    &query.e12_late_interaction,
                    &semantic.e12_late_interaction,
                );

                // Update E12 score in embedder_scores
                embedder_scores[11] = maxsim_score;

                // Blend Stage 2 score with MaxSim using configurable weight
                // Formula: final = fusion * (1 - weight) + maxsim * weight
                // This preserves semantic ranking while adding E12 precision
                let original_fusion = *score;
                let blended = original_fusion * fusion_weight + maxsim_score * rerank_weight;
                *score = blended;

                debug!(
                    "E12 rerank: fusion={:.4}, maxsim={:.4}, blended={:.4} (weight={})",
                    original_fusion, maxsim_score, blended, rerank_weight
                );
            }
        }

        // Re-sort by blended score
        candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to top_k
        candidates.truncate(top_k);
        candidates
    }

    /// Resolve weight profile to weight array.
    ///
    /// Returns DEFAULT_SEMANTIC_WEIGHTS if no profile specified.
    /// Returns error if specified profile is unknown (FAIL FAST).
    fn resolve_weights(&self, options: &TeleologicalSearchOptions) -> CoreResult<[f32; 13]> {
        match &options.weight_profile {
            Some(profile_name) => {
                let weights = get_weight_profile(profile_name).map_err(|e| {
                    error!(
                        profile = %profile_name,
                        error = %e,
                        "Failed to resolve weight profile - FAIL FAST"
                    );
                    CoreError::ValidationError {
                        field: "weight_profile".to_string(),
                        message: format!("Invalid weight profile: {}", e),
                    }
                })?;

                info!(profile = %profile_name, "Using weight profile");
                Ok(weights)
            }
            None => {
                debug!("No weight profile specified, using default semantic weights");
                Ok(DEFAULT_SEMANTIC_WEIGHTS)
            }
        }
    }

    /// Search E5 with asymmetric index selection (ARCH-15, AP-77).
    ///
    /// Implements direction-aware retrieval:
    /// - **Cause-seeking** (why X?): search E5CausalEffect index using query.e5_as_cause
    ///   - We want documents whose *effect* matches our *cause* query
    /// - **Effect-seeking** (what happens when X?): search E5CausalCause index using query.e5_as_effect
    ///   - We want documents whose *cause* matches our *effect* query
    /// - **Unknown direction**: use legacy E5Causal index with active vector
    ///
    /// # Arguments
    ///
    /// * `query` - Query semantic fingerprint
    /// * `direction` - Detected causal direction of the query
    /// * `k` - Number of candidates to retrieve
    ///
    /// # Returns
    ///
    /// List of (Uuid, distance) pairs from HNSW search
    ///
    /// # Index Selection Logic
    ///
    /// The key insight is that causal relationships are complementary:
    /// - A cause-seeking query ("why did X happen?") should find documents that describe effects
    /// - An effect-seeking query ("what happens when X?") should find documents that describe causes
    ///
    /// This is why we search the *opposite* index from the query direction.
    fn search_e5_asymmetric(
        &self,
        query: &SemanticFingerprint,
        direction: CausalDirection,
        k: usize,
    ) -> Result<Vec<(Uuid, f32)>, crate::teleological::indexes::IndexError> {
        match direction {
            CausalDirection::Cause => {
                // Cause-seeking query: search Effect index using cause vector
                // "Why X?" -> find documents whose effect matches our cause query
                if let Some(effect_index) = self.index_registry.get(EmbedderIndex::E5CausalEffect) {
                    let cause_vec = query.get_e5_as_cause();
                    if !cause_vec.is_empty() {
                        debug!(
                            "E5 asymmetric search: Cause-seeking query, searching E5CausalEffect index with cause vector ({}D)",
                            cause_vec.len()
                        );
                        return effect_index.search(cause_vec, k, None);
                    } else {
                        warn!("E5 asymmetric search: cause_vec is empty, falling back to legacy E5Causal");
                    }
                } else {
                    debug!("E5 asymmetric search: E5CausalEffect index not available, using legacy E5Causal");
                }
            }
            CausalDirection::Effect => {
                // Effect-seeking query: search Cause index using effect vector
                // "What happens when X?" -> find documents whose cause matches our effect query
                if let Some(cause_index) = self.index_registry.get(EmbedderIndex::E5CausalCause) {
                    let effect_vec = query.get_e5_as_effect();
                    if !effect_vec.is_empty() {
                        debug!(
                            "E5 asymmetric search: Effect-seeking query, searching E5CausalCause index with effect vector ({}D)",
                            effect_vec.len()
                        );
                        return cause_index.search(effect_vec, k, None);
                    } else {
                        warn!("E5 asymmetric search: effect_vec is empty, falling back to legacy E5Causal");
                    }
                } else {
                    debug!("E5 asymmetric search: E5CausalCause index not available, using legacy E5Causal");
                }
            }
            CausalDirection::Unknown => {
                // Unknown direction: use legacy E5Causal index
                debug!("E5 asymmetric search: Unknown direction, using legacy E5Causal index");
            }
        }

        // Fallback to legacy E5Causal index
        if let Some(e5_index) = self.index_registry.get(EmbedderIndex::E5Causal) {
            e5_index.search(query.e5_active_vector(), k, None)
        } else {
            debug!("E5 asymmetric search: No E5 indexes available");
            Ok(Vec::new())
        }
    }

    /// Apply recency boost POST-retrieval.
    ///
    /// Per ARCH-14: Temporal is a POST-retrieval boost, not similarity.
    /// Formula: final = semantic * (1.0 - boost) + temporal * boost
    fn apply_recency_boost(
        &self,
        results: &mut [TeleologicalSearchResult],
        _query: &SemanticFingerprint,
        boost_factor: f32,
    ) {
        debug!("Applying recency boost factor: {}", boost_factor);

        for result in results.iter_mut() {
            // E2 temporal recent score (index 1)
            let temporal_score = result.embedder_scores[1];

            // Blend semantic and temporal scores
            let original = result.similarity;
            result.similarity = original * (1.0 - boost_factor) + temporal_score * boost_factor;

            debug!(
                "Recency boost: {} -> {} (temporal: {})",
                original, result.similarity, temporal_score
            );
        }

        // Re-sort after boosting
        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Apply full temporal boost system POST-retrieval (ARCH-14).
    ///
    /// This method applies E2/E3/E4 temporal boosts based on the temporal_options:
    /// - E2 Recency: Decay functions (linear, exponential, step)
    /// - E3 Periodic: Hour-of-day, day-of-week pattern matching
    /// - E4 Sequence: Before/after anchor memory relationships
    ///
    /// # Arguments
    ///
    /// * `results` - Search results to boost (modified in place)
    /// * `query` - Query semantic fingerprint
    /// * `options` - Search options with temporal configuration
    ///
    /// # E4-FIX
    ///
    /// This method now uses `apply_temporal_boosts_v2` which properly fetches
    /// session_sequence from SourceMetadata for accurate before/after filtering.
    async fn apply_full_temporal_boosts(
        &self,
        results: &mut Vec<TeleologicalSearchResult>,
        query: &SemanticFingerprint,
        options: &TeleologicalSearchOptions,
    ) -> CoreResult<()> {
        debug!(
            "Applying full temporal boosts: weight={}, decay={:?}",
            options.temporal_options.temporal_weight,
            options.temporal_options.decay_function
        );

        if results.is_empty() {
            return Ok(());
        }

        // Collect fingerprints and timestamps for all results
        let mut fingerprints: HashMap<Uuid, SemanticFingerprint> = HashMap::new();
        let mut timestamps: HashMap<Uuid, i64> = HashMap::new();

        for result in results.iter() {
            let id = result.fingerprint.id;
            fingerprints.insert(id, result.fingerprint.semantic.clone());
            timestamps.insert(id, result.fingerprint.created_at.timestamp_millis());
        }

        // E4-FIX Phase 4: Batch fetch sequences for all results from SourceMetadata
        let result_ids: Vec<Uuid> = results.iter().map(|r| r.fingerprint.id).collect();
        let source_metadata_batch = self.get_source_metadata_batch_async(&result_ids).await?;

        let mut sequences: HashMap<Uuid, Option<u64>> = HashMap::new();
        for (id, maybe_meta) in result_ids.iter().zip(source_metadata_batch.into_iter()) {
            let seq = maybe_meta.and_then(|m| m.session_sequence);
            sequences.insert(*id, seq);
        }

        // If sequence options are set, fetch the anchor fingerprint and sequence
        let (anchor_fp, anchor_ts, anchor_seq) = if let Some(ref seq_opts) = options.temporal_options.sequence_options {
            if !seq_opts.anchor_id.is_nil() {
                // Try to fetch anchor fingerprint from storage
                match self.get_fingerprint_raw(seq_opts.anchor_id) {
                    Ok(Some(data)) => {
                        let anchor = deserialize_teleological_fingerprint(&data);
                        let ts = anchor.created_at.timestamp_millis();

                        // E4-FIX Phase 3: Also fetch anchor's sequence from SourceMetadata
                        let seq = match self.get_source_metadata_async(seq_opts.anchor_id).await {
                            Ok(Some(meta)) => meta.session_sequence,
                            _ => {
                                debug!(
                                    "Temporal boost: Could not fetch anchor source metadata {}, using timestamp fallback",
                                    seq_opts.anchor_id
                                );
                                None
                            }
                        };

                        (Some(anchor.semantic), Some(ts), seq)
                    }
                    _ => {
                        warn!(
                            "Temporal boost: Could not fetch anchor fingerprint {}, skipping sequence boost",
                            seq_opts.anchor_id
                        );
                        (None, None, None)
                    }
                }
            } else {
                (None, None, None)
            }
        } else {
            (None, None, None)
        };

        // E4-FIX: Apply temporal boosts using the v2 function with sequence support
        let _boost_data = temporal_boost::apply_temporal_boosts_v2(
            results,
            query,
            &options.temporal_options,
            &fingerprints,
            &timestamps,
            &sequences,
            anchor_fp.as_ref(),
            anchor_ts,
            anchor_seq,
        );

        debug!(
            "Temporal boosts v2 applied to {} results (anchor_seq={:?})",
            results.len(),
            anchor_seq
        );

        Ok(())
    }

    /// Search by text (internal async wrapper).
    pub(crate) async fn search_text_async(
        &self,
        _text: &str,
        options: TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        // Text search requires embedding generation, which is not available in storage layer
        // Return empty results with a warning
        warn!("search_text called but embedding generation not available in storage layer");
        warn!("Use embedding service to generate query embeddings, then call search_semantic");
        Ok(Vec::with_capacity(options.top_k))
    }

    /// Search by sparse vector (internal async wrapper).
    ///
    /// Uses BM25-IDF scoring per Phase 2 of E12/E13 integration plan:
    /// - IDF = ln((N - df + 0.5) / (df + 0.5) + 1)
    /// - Score = Σ(query_weight × IDF(term))
    ///
    /// This is BM25 without per-document TF saturation (posting lists only store doc IDs).
    /// Still provides significant improvement over simple term frequency scoring by
    /// properly weighting rare terms higher than common terms.
    pub(crate) async fn search_sparse_async(
        &self,
        sparse_query: &SparseVector,
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        debug!(
            "Searching sparse with {} active terms, top_k={} (BM25-IDF scoring)",
            sparse_query.nnz(),
            top_k
        );

        // Get total document count for IDF calculation FIRST (before any cf references)
        // This is an O(n) scan but cached in typical usage patterns
        let total_docs = self.count_async().await.unwrap_or(1) as f32;

        // Now get the column family (not held across await)
        let cf = self.get_cf(CF_E13_SPLADE_INVERTED)?;

        // First pass: collect posting lists and compute IDF weights
        // Store (term_id, query_weight, posting_list, idf) tuples
        struct TermData {
            query_weight: f32,
            doc_ids: Vec<Uuid>,
            idf: f32,
        }
        let mut term_data: Vec<TermData> = Vec::with_capacity(sparse_query.nnz());

        for (i, &term_id) in sparse_query.indices.iter().enumerate() {
            let term_key = e13_splade_inverted_key(term_id);
            let query_weight = sparse_query.values[i];

            if let Some(data) = self.db.get_cf(cf, term_key).map_err(|e| {
                TeleologicalStoreError::rocksdb_op("get", CF_E13_SPLADE_INVERTED, None, e)
            })? {
                let doc_ids = deserialize_memory_id_list(&data);
                let df = doc_ids.len() as f32;

                // BM25 IDF formula: ln((N - df + 0.5) / (df + 0.5) + 1)
                // This gives higher weight to rare terms (low df) and lower weight to common terms
                let idf = ((total_docs - df + 0.5) / (df + 0.5) + 1.0).ln();

                term_data.push(TermData {
                    query_weight,
                    doc_ids,
                    idf,
                });
            }
        }

        // Second pass: accumulate BM25-IDF scores per document
        let mut doc_scores: HashMap<Uuid, f32> = HashMap::new();

        for term in &term_data {
            // Pre-compute term contribution: query_weight × IDF
            let term_contribution = term.query_weight * term.idf;

            for doc_id in &term.doc_ids {
                // Skip soft-deleted documents
                if self.is_soft_deleted(doc_id) {
                    continue;
                }

                // BM25-IDF scoring: accumulate query_weight × IDF for each matching term
                *doc_scores.entry(*doc_id).or_insert(0.0) += term_contribution;
            }
        }

        // Sort by score descending
        let mut results: Vec<(Uuid, f32)> = doc_scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Truncate to top_k
        results.truncate(top_k);

        debug!(
            "Sparse search (BM25-IDF) returned {} results from {} terms, total_docs={}",
            results.len(),
            term_data.len(),
            total_docs
        );
        Ok(results)
    }
}
