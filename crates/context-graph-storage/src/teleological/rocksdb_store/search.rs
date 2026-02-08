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
use std::sync::{Arc, RwLock};

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
use context_graph_core::weights::{get_weight_profile, validate_weights};

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

use rocksdb::DB;
use crate::teleological::column_families::CF_FINGERPRINTS;
use crate::teleological::schema::fingerprint_key;
use crate::teleological::indexes::EmbedderIndexRegistry;
use super::helpers::compute_cosine_similarity;

// =============================================================================
// SPAWN_BLOCKING SYNC FUNCTIONS
// These functions run in Tokio's blocking thread pool for parallel agent access
// =============================================================================

/// Get raw fingerprint bytes from RocksDB (sync version for spawn_blocking).
fn get_fingerprint_raw_sync(db: &DB, id: Uuid) -> Result<Option<Vec<u8>>, TeleologicalStoreError> {
    let cf = db.cf_handle(CF_FINGERPRINTS).ok_or_else(|| {
        TeleologicalStoreError::ColumnFamilyNotFound {
            name: CF_FINGERPRINTS.to_string(),
        }
    })?;
    let key = fingerprint_key(&id);
    db.get_cf(cf, key)
        .map_err(|e| TeleologicalStoreError::rocksdb_op("get", CF_FINGERPRINTS, Some(id), e))
}

/// Check if an ID is soft-deleted (sync version for spawn_blocking).
/// Takes Arc<RwLock<HashMap>> to avoid expensive HashMap cloning before spawn_blocking.
///
/// # Panics
///
/// Panics if the RwLock is poisoned, indicating a critical thread panic elsewhere.
/// This is intentional fail-fast behavior per project requirements.
fn is_soft_deleted_sync(soft_deleted: &Arc<RwLock<HashMap<Uuid, bool>>>, id: &Uuid) -> bool {
    soft_deleted
        .read()
        .expect("soft_deleted RwLock poisoned - a thread panicked while holding this lock")
        .get(id)
        .copied()
        .unwrap_or(false) // False here is correct: unknown IDs are not deleted
}

/// Compute embedder scores with code query type (pure function).
fn compute_embedder_scores_sync(
    query: &SemanticFingerprint,
    stored: &SemanticFingerprint,
    code_query_type: Option<CodeQueryType>,
) -> [f32; 13] {
    use crate::teleological::search::compute_maxsim_direct;
    use context_graph_core::code::compute_e7_similarity_with_query_type;

    let e7_score = match code_query_type {
        Some(query_type) => {
            compute_e7_similarity_with_query_type(&query.e7_code, &stored.e7_code, query_type)
        }
        None => compute_cosine_similarity(&query.e7_code, &stored.e7_code),
    };

    [
        compute_cosine_similarity(&query.e1_semantic, &stored.e1_semantic),
        compute_cosine_similarity(&query.e2_temporal_recent, &stored.e2_temporal_recent),
        compute_cosine_similarity(&query.e3_temporal_periodic, &stored.e3_temporal_periodic),
        compute_cosine_similarity(&query.e4_temporal_positional, &stored.e4_temporal_positional),
        compute_cosine_similarity(query.e5_active_vector(), stored.e5_active_vector()),
        query.e6_sparse.cosine_similarity(&stored.e6_sparse),
        e7_score,
        compute_cosine_similarity(query.e8_active_vector(), stored.e8_active_vector()),
        compute_cosine_similarity(&query.e9_hdc, &stored.e9_hdc),
        compute_cosine_similarity(query.e10_active_vector(), stored.e10_active_vector()),
        compute_cosine_similarity(&query.e11_entity, &stored.e11_entity),
        compute_maxsim_direct(&query.e12_late_interaction, &stored.e12_late_interaction),
        query.e13_splade.cosine_similarity(&stored.e13_splade),
    ]
}

/// Compute embedder scores with direction awareness (pure function).
fn compute_embedder_scores_with_direction_sync(
    query: &SemanticFingerprint,
    stored: &SemanticFingerprint,
    code_query_type: Option<CodeQueryType>,
    causal_direction: CausalDirection,
) -> [f32; 13] {
    use crate::teleological::search::compute_maxsim_direct;
    use context_graph_core::code::compute_e7_similarity_with_query_type;
    use context_graph_core::retrieval::distance::compute_similarity_for_space_with_direction;
    use context_graph_core::teleological::Embedder;

    let e7_score = match code_query_type {
        Some(query_type) => {
            compute_e7_similarity_with_query_type(&query.e7_code, &stored.e7_code, query_type)
        }
        None => compute_cosine_similarity(&query.e7_code, &stored.e7_code),
    };

    let e5_score = compute_similarity_for_space_with_direction(
        Embedder::Causal,
        query,
        stored,
        causal_direction,
    );

    [
        compute_cosine_similarity(&query.e1_semantic, &stored.e1_semantic),
        compute_cosine_similarity(&query.e2_temporal_recent, &stored.e2_temporal_recent),
        compute_cosine_similarity(&query.e3_temporal_periodic, &stored.e3_temporal_periodic),
        compute_cosine_similarity(&query.e4_temporal_positional, &stored.e4_temporal_positional),
        e5_score,
        query.e6_sparse.cosine_similarity(&stored.e6_sparse),
        e7_score,
        compute_cosine_similarity(query.e8_active_vector(), stored.e8_active_vector()),
        compute_cosine_similarity(&query.e9_hdc, &stored.e9_hdc),
        compute_cosine_similarity(query.e10_active_vector(), stored.e10_active_vector()),
        compute_cosine_similarity(&query.e11_entity, &stored.e11_entity),
        compute_maxsim_direct(&query.e12_late_interaction, &stored.e12_late_interaction),
        query.e13_splade.cosine_similarity(&stored.e13_splade),
    ]
}

/// E1-only search (sync version for spawn_blocking).
/// Takes Arc<RwLock<HashMap>> to avoid expensive HashMap cloning before spawn_blocking.
fn search_e1_only_sync(
    db: &Arc<DB>,
    index_registry: &Arc<EmbedderIndexRegistry>,
    soft_deleted: &Arc<RwLock<HashMap<Uuid, bool>>>,
    query: &SemanticFingerprint,
    options: &TeleologicalSearchOptions,
) -> CoreResult<Vec<TeleologicalSearchResult>> {
    let entry_embedder = EmbedderIndex::E1Semantic;
    let entry_index = index_registry.get(entry_embedder).ok_or_else(|| {
        CoreError::IndexError(format!("Index {:?} not found", entry_embedder))
    })?;

    let k = (options.top_k * 2).max(20);
    let candidates = entry_index
        .search(&query.e1_semantic, k, None)
        .map_err(|e| {
            error!("E1 search failed: {}", e);
            CoreError::IndexError(e.to_string())
        })?;

    let mut results = Vec::with_capacity(candidates.len());

    for (id, distance) in candidates {
        let similarity = 1.0 - distance.min(1.0);

        if !options.include_deleted && is_soft_deleted_sync(soft_deleted, &id) {
            continue;
        }

        if similarity < options.min_similarity {
            continue;
        }

        if let Some(data) = get_fingerprint_raw_sync(db, id)? {
            let fp = deserialize_teleological_fingerprint(&data);
            let code_query_type = options.effective_code_query_type();
            let embedder_scores = if options.causal_direction != CausalDirection::Unknown {
                compute_embedder_scores_with_direction_sync(
                    query,
                    &fp.semantic,
                    code_query_type,
                    options.causal_direction,
                )
            } else {
                compute_embedder_scores_sync(query, &fp.semantic, code_query_type)
            };
            results.push(TeleologicalSearchResult::new(fp, similarity, embedder_scores));
        }
    }

    results.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(options.top_k);

    Ok(results)
}

/// Multi-space search (sync version for spawn_blocking).
/// Takes Arc<RwLock<HashMap>> to avoid expensive HashMap cloning before spawn_blocking.
fn search_multi_space_sync(
    db: &Arc<DB>,
    index_registry: &Arc<EmbedderIndexRegistry>,
    soft_deleted: &Arc<RwLock<HashMap<Uuid, bool>>>,
    query: &SemanticFingerprint,
    options: &TeleologicalSearchOptions,
) -> CoreResult<Vec<TeleologicalSearchResult>> {
    let weights = resolve_weights_sync(options)?;
    let k = (options.top_k * 3).max(50);

    let mut embedder_rankings: Vec<EmbedderRanking> = Vec::new();

    // E1 Semantic
    let entry_embedder = EmbedderIndex::E1Semantic;
    let entry_index = index_registry.get(entry_embedder).ok_or_else(|| {
        CoreError::IndexError(format!("Index {:?} not found", entry_embedder))
    })?;

    let e1_candidates = entry_index
        .search(&query.e1_semantic, k, None)
        .map_err(|e| {
            error!("E1 search failed: {}", e);
            CoreError::IndexError(e.to_string())
        })?;

    let e1_ranked: Vec<(Uuid, f32)> = e1_candidates
        .into_iter()
        .filter(|(id, _)| options.include_deleted || !is_soft_deleted_sync(soft_deleted, id))
        .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
        .collect();

    if !e1_ranked.is_empty() {
        embedder_rankings.push(EmbedderRanking::new("E1", weights[0], e1_ranked));
    }

    // E5 Causal
    if let Some(e5_index) = index_registry.get(EmbedderIndex::E5Causal) {
        if let Ok(e5_candidates) = e5_index.search(query.e5_active_vector(), k, None) {
            let e5_ranked: Vec<(Uuid, f32)> = e5_candidates
                .into_iter()
                .filter(|(id, _)| options.include_deleted || !is_soft_deleted_sync(soft_deleted, id))
                .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
                .collect();

            if !e5_ranked.is_empty() && weights[4] > 0.0 {
                embedder_rankings.push(EmbedderRanking::new("E5", weights[4], e5_ranked));
            }
        }
    }

    // E7 Code
    if let Some(e7_index) = index_registry.get(EmbedderIndex::E7Code) {
        if let Ok(e7_candidates) = e7_index.search(&query.e7_code, k, None) {
            let e7_ranked: Vec<(Uuid, f32)> = e7_candidates
                .into_iter()
                .filter(|(id, _)| options.include_deleted || !is_soft_deleted_sync(soft_deleted, id))
                .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
                .collect();

            if !e7_ranked.is_empty() && weights[6] > 0.0 {
                embedder_rankings.push(EmbedderRanking::new("E7", weights[6], e7_ranked));
            }
        }
    }

    // E10 Multimodal
    if let Some(e10_index) = index_registry.get(EmbedderIndex::E10Multimodal) {
        if let Ok(e10_candidates) = e10_index.search(query.e10_active_vector(), k, None) {
            let e10_ranked: Vec<(Uuid, f32)> = e10_candidates
                .into_iter()
                .filter(|(id, _)| options.include_deleted || !is_soft_deleted_sync(soft_deleted, id))
                .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
                .collect();

            if !e10_ranked.is_empty() && weights[9] > 0.0 {
                embedder_rankings.push(EmbedderRanking::new("E10", weights[9], e10_ranked));
            }
        }
    }

    // E8 Graph (connectivity/structure embeddings, 1024D HNSW)
    if let Some(e8_index) = index_registry.get(EmbedderIndex::E8Graph) {
        if let Ok(e8_candidates) = e8_index.search(query.e8_active_vector(), k, None) {
            let e8_ranked: Vec<(Uuid, f32)> = e8_candidates
                .into_iter()
                .filter(|(id, _)| options.include_deleted || !is_soft_deleted_sync(soft_deleted, id))
                .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
                .collect();

            if !e8_ranked.is_empty() && weights[7] > 0.0 {
                embedder_rankings.push(EmbedderRanking::new("E8", weights[7], e8_ranked));
            }
        }
    }

    // E11 Entity (KEPLER entity embeddings, 768D HNSW)
    if let Some(e11_index) = index_registry.get(EmbedderIndex::E11Entity) {
        if let Ok(e11_candidates) = e11_index.search(&query.e11_entity, k, None) {
            let e11_ranked: Vec<(Uuid, f32)> = e11_candidates
                .into_iter()
                .filter(|(id, _)| options.include_deleted || !is_soft_deleted_sync(soft_deleted, id))
                .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
                .collect();

            if !e11_ranked.is_empty() && weights[10] > 0.0 {
                embedder_rankings.push(EmbedderRanking::new("E11", weights[10], e11_ranked));
            }
        }
    }

    debug!(
        "Multi-space search: {} embedder rankings collected, fusion_strategy={:?}",
        embedder_rankings.len(),
        options.fusion_strategy
    );

    let fused_results = fuse_rankings(&embedder_rankings, options.fusion_strategy, options.top_k * 2);

    let code_query_type = options.effective_code_query_type();
    let mut results = Vec::with_capacity(fused_results.len());

    for fused in fused_results {
        if let Some(data) = get_fingerprint_raw_sync(db, fused.doc_id)? {
            let fp = deserialize_teleological_fingerprint(&data);
            let embedder_scores = if options.causal_direction != CausalDirection::Unknown {
                compute_embedder_scores_with_direction_sync(
                    query,
                    &fp.semantic,
                    code_query_type,
                    options.causal_direction,
                )
            } else {
                compute_embedder_scores_sync(query, &fp.semantic, code_query_type)
            };

            let final_score = match options.fusion_strategy {
                FusionStrategy::WeightedSum => compute_semantic_fusion(&embedder_scores, &weights),
                FusionStrategy::WeightedRRF => fused.fused_score * 10.0,
            };

            if final_score < options.min_similarity {
                continue;
            }

            results.push(TeleologicalSearchResult::new(fp, final_score, embedder_scores));
        }
    }

    results.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(options.top_k);

    debug!(
        "Multi-space search returned {} results with {:?} fusion",
        results.len(),
        options.fusion_strategy
    );
    Ok(results)
}

/// Pipeline search (sync version for spawn_blocking).
/// Takes Arc<RwLock<HashMap>> to avoid expensive HashMap cloning before spawn_blocking.
fn search_pipeline_sync(
    db: &Arc<DB>,
    index_registry: &Arc<EmbedderIndexRegistry>,
    soft_deleted: &Arc<RwLock<HashMap<Uuid, bool>>>,
    query: &SemanticFingerprint,
    options: &TeleologicalSearchOptions,
) -> CoreResult<Vec<TeleologicalSearchResult>> {
    let recall_k = options.top_k * STAGE1_RECALL_MULTIPLIER;
    let stage2_k = options.top_k * STAGE2_CANDIDATE_MULTIPLIER;

    info!(
        "Pipeline search: Stage1 recall_k={}, Stage2 candidates={}, final_k={}",
        recall_k, stage2_k, options.top_k
    );

    // STAGE 1: FAST RECALL
    let mut candidate_ids: HashSet<Uuid> = HashSet::new();

    // E13 SPLADE sparse recall
    if !query.e13_splade.is_empty() {
        match search_sparse_sync(&**db, &query.e13_splade, recall_k, soft_deleted) {
            Ok(sparse_results) => {
                let sparse_count = sparse_results.len();
                candidate_ids.extend(sparse_results.into_iter().map(|(id, _)| id));
                debug!("Stage 1: E13 SPLADE returned {} candidates", sparse_count);
            }
            Err(e) => {
                warn!("Stage 1: E13 SPLADE search failed: {}, continuing with E1 only", e);
            }
        }
    }

    // E1 Semantic HNSW
    let entry_embedder = EmbedderIndex::E1Semantic;
    if let Some(entry_index) = index_registry.get(entry_embedder) {
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

    // E5 Causal
    if let Some(e5_index) = index_registry.get(EmbedderIndex::E5Causal) {
        if let Ok(e5_candidates) = e5_index.search(query.e5_active_vector(), recall_k / 2, None) {
            let e5_count = e5_candidates.len();
            candidate_ids.extend(e5_candidates.into_iter().map(|(id, _)| id));
            debug!("Stage 1: E5 Causal returned {} additional candidates", e5_count);
        }
    }

    // E7 Code
    if let Some(e7_index) = index_registry.get(EmbedderIndex::E7Code) {
        if let Ok(e7_candidates) = e7_index.search(&query.e7_code, recall_k / 2, None) {
            let e7_count = e7_candidates.len();
            candidate_ids.extend(e7_candidates.into_iter().map(|(id, _)| id));
            debug!("Stage 1: E7 Code returned {} additional candidates", e7_count);
        }
    }

    // E8 Graph (connectivity/structure)
    if let Some(e8_index) = index_registry.get(EmbedderIndex::E8Graph) {
        if let Ok(e8_candidates) = e8_index.search(query.e8_active_vector(), recall_k / 2, None) {
            let e8_count = e8_candidates.len();
            candidate_ids.extend(e8_candidates.into_iter().map(|(id, _)| id));
            debug!("Stage 1: E8 Graph returned {} additional candidates", e8_count);
        }
    }

    // E11 Entity (KEPLER entity embeddings)
    if let Some(e11_index) = index_registry.get(EmbedderIndex::E11Entity) {
        if let Ok(e11_candidates) = e11_index.search(&query.e11_entity, recall_k / 2, None) {
            let e11_count = e11_candidates.len();
            candidate_ids.extend(e11_candidates.into_iter().map(|(id, _)| id));
            debug!("Stage 1: E11 Entity returned {} additional candidates", e11_count);
        }
    }

    info!(
        "Stage 1 complete: {} unique candidates from E13+E1+E5+E7+E8+E11",
        candidate_ids.len()
    );

    if candidate_ids.is_empty() {
        debug!("Stage 1: No candidates found, returning empty results");
        return Ok(Vec::new());
    }

    // STAGE 2: MULTI-SPACE SCORING
    let weights = resolve_weights_sync(options)?;
    let code_query_type = options.effective_code_query_type();

    let mut valid_candidates: Vec<(Uuid, TeleologicalFingerprint)> = Vec::with_capacity(candidate_ids.len());

    for id in candidate_ids {
        if !options.include_deleted && is_soft_deleted_sync(soft_deleted, &id) {
            continue;
        }
        if let Some(data) = get_fingerprint_raw_sync(db, id)? {
            let fp = deserialize_teleological_fingerprint(&data);
            valid_candidates.push((id, fp));
        }
    }

    let candidate_data: Vec<(Uuid, [f32; 13], SemanticFingerprint)> = valid_candidates
        .into_iter()
        .map(|(id, fp)| {
            let embedder_scores = if options.causal_direction != CausalDirection::Unknown {
                compute_embedder_scores_with_direction_sync(
                    query,
                    &fp.semantic,
                    code_query_type,
                    options.causal_direction,
                )
            } else {
                compute_embedder_scores_sync(query, &fp.semantic, code_query_type)
            };
            (id, embedder_scores, fp.semantic)
        })
        .collect();

    let mut scored_candidates: Vec<(Uuid, f32, [f32; 13], SemanticFingerprint)> =
        match options.fusion_strategy {
            FusionStrategy::WeightedSum => candidate_data
                .into_iter()
                .map(|(id, scores, semantic)| {
                    let fusion_score = compute_semantic_fusion(&scores, &weights);
                    (id, fusion_score, scores, semantic)
                })
                .filter(|(_, score, _, _)| *score >= options.min_similarity)
                .collect(),
            FusionStrategy::WeightedRRF => {
                let semantic_indices = [
                    (0, "E1", weights[0]),
                    (4, "E5", weights[4]),
                    (6, "E7", weights[6]),
                    (7, "E8", weights[7]),
                    (9, "E10", weights[9]),
                    (10, "E11", weights[10]),
                ];

                let mut embedder_rankings: Vec<EmbedderRanking> = Vec::new();

                for (idx, name, weight) in semantic_indices {
                    if weight <= 0.0 {
                        continue;
                    }
                    let mut ranked: Vec<(Uuid, f32)> = candidate_data
                        .iter()
                        .map(|(id, scores, _)| (*id, scores[idx]))
                        .collect();
                    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                    if !ranked.is_empty() {
                        embedder_rankings.push(EmbedderRanking::new(name, weight, ranked));
                    }
                }

                let fused = fuse_rankings(&embedder_rankings, FusionStrategy::WeightedRRF, stage2_k * 2);

                let candidate_map: HashMap<Uuid, ([f32; 13], SemanticFingerprint)> = candidate_data
                    .into_iter()
                    .map(|(id, scores, semantic)| (id, (scores, semantic)))
                    .collect();

                fused
                    .into_iter()
                    .filter_map(|f| {
                        candidate_map.get(&f.doc_id).map(|(scores, semantic)| {
                            let scaled_score = f.fused_score * 10.0;
                            (f.doc_id, scaled_score, *scores, semantic.clone())
                        })
                    })
                    .filter(|(_, score, _, _)| *score >= options.min_similarity)
                    .collect()
            }
        };

    scored_candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
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

    // STAGE 3: Optional E12 re-ranking (skip in sync version for now)
    scored_candidates.truncate(options.top_k);

    info!("Stage 3 complete: {} final results", scored_candidates.len());

    // BUILD FINAL RESULTS
    let mut results = Vec::with_capacity(scored_candidates.len());

    for (id, score, embedder_scores, _semantic) in scored_candidates {
        if let Some(data) = get_fingerprint_raw_sync(db, id)? {
            let fp = deserialize_teleological_fingerprint(&data);
            results.push(TeleologicalSearchResult::new(fp, score, embedder_scores));
        }
    }

    debug!("Pipeline search returned {} results", results.len());
    Ok(results)
}

/// Resolve weight profile (sync version).
///
/// Priority: custom_weights > weight_profile > default.
/// After resolution, applies exclude_embedders (zero + renormalize).
fn resolve_weights_sync(options: &TeleologicalSearchOptions) -> CoreResult<[f32; 13]> {
    // Step 1: Resolve base weights (custom_weights overrides weight_profile)
    let mut weights = if let Some(custom) = &options.custom_weights {
        // Validate custom weights - FAIL FAST on invalid input
        validate_weights(custom).map_err(|e| {
            error!(
                error = %e,
                "Custom weights validation failed - FAIL FAST"
            );
            CoreError::ValidationError {
                field: "customWeights".to_string(),
                message: format!("Invalid custom weights: {}", e),
            }
        })?;
        info!("Using custom weight array");
        *custom
    } else {
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
                weights
            }
            None => {
                debug!("No weight profile specified, using default semantic weights");
                DEFAULT_SEMANTIC_WEIGHTS
            }
        }
    };

    // Step 2: Apply embedder exclusions (zero out + renormalize)
    if !options.exclude_embedders.is_empty() {
        for &idx in &options.exclude_embedders {
            if idx < 13 {
                weights[idx] = 0.0;
            } else {
                return Err(CoreError::ValidationError {
                    field: "excludeEmbedders".to_string(),
                    message: format!("Embedder index {} out of range (0-12)", idx),
                });
            }
        }
        // Renormalize remaining weights to sum to 1.0
        let sum: f32 = weights.iter().sum();
        if sum > 0.0 {
            for w in weights.iter_mut() {
                *w /= sum;
            }
        } else {
            return Err(CoreError::ValidationError {
                field: "excludeEmbedders".to_string(),
                message: "All embedders excluded - at least one must have weight > 0".to_string(),
            });
        }
        info!(
            excluded = ?options.exclude_embedders,
            "Applied embedder exclusions and renormalized weights"
        );
    }

    Ok(weights)
}

/// Sparse search (sync version for spawn_blocking).
/// Takes Arc<RwLock<HashMap>> to avoid expensive HashMap cloning before spawn_blocking.
fn search_sparse_sync(
    db: &DB,
    sparse_query: &SparseVector,
    top_k: usize,
    soft_deleted: &Arc<RwLock<HashMap<Uuid, bool>>>,
) -> CoreResult<Vec<(Uuid, f32)>> {
    debug!(
        "Searching sparse with {} active terms, top_k={} (BM25-IDF scoring)",
        sparse_query.nnz(),
        top_k
    );

    // Get total document count - approximate from fingerprints CF
    let cf_fp = db.cf_handle(CF_FINGERPRINTS).ok_or_else(|| {
        CoreError::Internal("CF_FINGERPRINTS not found".to_string())
    })?;
    let total_docs = db.iterator_cf(cf_fp, rocksdb::IteratorMode::Start).count() as f32;

    let cf = db.cf_handle(CF_E13_SPLADE_INVERTED).ok_or_else(|| {
        CoreError::Internal("CF_E13_SPLADE_INVERTED not found".to_string())
    })?;

    struct TermData {
        query_weight: f32,
        doc_ids: Vec<Uuid>,
        idf: f32,
    }
    let mut term_data: Vec<TermData> = Vec::with_capacity(sparse_query.nnz());

    for (i, &term_id) in sparse_query.indices.iter().enumerate() {
        let term_key = e13_splade_inverted_key(term_id);
        let query_weight = sparse_query.values[i];

        if let Some(data) = db.get_cf(cf, term_key).map_err(|e| {
            CoreError::StorageError(format!("Failed to get E13 SPLADE term: {}", e))
        })? {
            let doc_ids = deserialize_memory_id_list(&data);
            let df = doc_ids.len() as f32;
            let idf = ((total_docs - df + 0.5) / (df + 0.5) + 1.0).ln();
            term_data.push(TermData { query_weight, doc_ids, idf });
        }
    }

    let mut doc_scores: HashMap<Uuid, f32> = HashMap::new();

    for term in &term_data {
        let term_contribution = term.query_weight * term.idf;
        for doc_id in &term.doc_ids {
            if is_soft_deleted_sync(soft_deleted, doc_id) {
                continue;
            }
            *doc_scores.entry(*doc_id).or_insert(0.0) += term_contribution;
        }
    }

    let mut results: Vec<(Uuid, f32)> = doc_scores.into_iter().collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);

    debug!(
        "Sparse search (BM25-IDF) returned {} results from {} terms, total_docs={}",
        results.len(),
        term_data.len(),
        total_docs
    );
    Ok(results)
}

// =============================================================================
// SEMANTIC EMBEDDER WEIGHTS
// Per constitution: Temporal (E2-E4) have weight 0.0 in semantic search
// =============================================================================

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
    ///
    /// # Concurrency
    ///
    /// Uses `spawn_blocking` to move RocksDB I/O to Tokio's blocking thread pool,
    /// enabling parallel agent access without blocking the async runtime.
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

        // Clone Arc-wrapped fields for spawn_blocking closure
        // CRITICAL: Use Arc::clone for soft_deleted instead of cloning the HashMap
        // This avoids 5-478 KB of data cloning per search on the async runtime
        let db = Arc::clone(&self.db);
        let index_registry = Arc::clone(&self.index_registry);
        let soft_deleted = Arc::clone(&self.soft_deleted);
        let query_clone = query.clone();
        let options_clone = options.clone();

        // Move synchronous search work to blocking thread pool
        let mut results = tokio::task::spawn_blocking(move || {
            // Branch based on search strategy
            match options_clone.strategy {
                SearchStrategy::E1Only => {
                    search_e1_only_sync(&db, &index_registry, &soft_deleted, &query_clone, &options_clone)
                }
                SearchStrategy::MultiSpace => {
                    search_multi_space_sync(&db, &index_registry, &soft_deleted, &query_clone, &options_clone)
                }
                SearchStrategy::Pipeline => {
                    search_pipeline_sync(&db, &index_registry, &soft_deleted, &query_clone, &options_clone)
                }
            }
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))??;

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

    /// Search by text - NOT IMPLEMENTED at storage layer.
    ///
    /// Text search requires embedding generation, which is NOT available at the storage layer.
    /// The storage layer can only search using pre-computed embeddings.
    ///
    /// # Errors
    ///
    /// Always returns `CoreError::NotImplemented` with guidance on correct usage.
    ///
    /// # Correct Usage
    ///
    /// Instead of calling `search_text`, use the embedding service to generate embeddings
    /// from text, then call `search_semantic` or `search_multi_space` with those embeddings:
    ///
    /// ```ignore
    /// // WRONG: search_text("query") - will fail
    /// // RIGHT: Generate embeddings first, then search
    /// let embeddings = embedding_service.embed("query").await?;
    /// let results = store.search_semantic(&embeddings.e1, options).await?;
    /// ```
    pub(crate) async fn search_text_async(
        &self,
        text: &str,
        _options: TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        error!(
            query_text = %text,
            "search_text called on storage layer which cannot generate embeddings"
        );
        Err(CoreError::NotImplemented(
            "search_text is not available at the storage layer. \
             The storage layer can only search using pre-computed embeddings. \
             Use the MCP tool 'search_graph' which handles embedding generation, \
             or generate embeddings via the embedding service and call 'search_semantic' directly."
                .to_string(),
        ))
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
    ///
    /// # Concurrency
    ///
    /// Uses `spawn_blocking` to move RocksDB I/O to Tokio's blocking thread pool,
    /// preventing async runtime blocking.
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

        // Clone Arc-wrapped fields for spawn_blocking closure
        // CRITICAL: Use Arc::clone for soft_deleted instead of cloning the HashMap
        let db = Arc::clone(&self.db);
        let soft_deleted = Arc::clone(&self.soft_deleted);
        let sparse_query = sparse_query.clone();

        // Move synchronous RocksDB I/O to blocking thread pool
        tokio::task::spawn_blocking(move || {
            search_sparse_sync(&*db, &sparse_query, top_k, &soft_deleted)
                .map(|results| {
                    debug!(
                        "Sparse search (BM25-IDF) returned {} results, total_docs={}",
                        results.len(),
                        total_docs
                    );
                    results
                })
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))?
    }
}
