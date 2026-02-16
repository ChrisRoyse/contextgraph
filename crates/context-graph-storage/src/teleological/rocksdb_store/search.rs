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
//! - **Pipeline**: 2-stage retrieval (E13 recall + multi-space scoring)
//!
//! ## Key Research Insights
//!
//! Temporal embedders (E2-E4) measure TIME proximity, not TOPIC similarity.
//! They are excluded from similarity scoring and applied as post-retrieval boosts.
//!
//! # Embedder-Specific Search (CRIT-06)
//!
//! When `embedder_indices` is set on search options, the search is routed to the
//! specific HNSW index(es) rather than always defaulting to E1. This enables
//! `search_by_embedder` to actually search the requested embedder's space.
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
use std::sync::Arc;
use std::sync::atomic::Ordering;
// P5: DashMap for lock-free concurrent soft-delete checks
use dashmap::DashMap;

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
///
/// P5: Uses DashMap for lock-free concurrent reads. No global RwLock contention
/// under concurrent search load.
fn is_soft_deleted_sync(soft_deleted: &Arc<DashMap<Uuid, i64>>, id: &Uuid) -> bool {
    soft_deleted.contains_key(id)
}

/// Get the query vector for a given embedder index (0-12).
///
/// Returns the appropriate vector slice from the SemanticFingerprint for searching
/// the embedder's HNSW index. Returns None for non-HNSW embedders (E6=5, E12=11, E13=12).
///
/// CRIT-06: This mapping is the single source of truth for embedder -> query vector routing.
fn get_query_vector_for_embedder<'a>(query: &'a SemanticFingerprint, embedder_idx: usize) -> Option<&'a [f32]> {
    match embedder_idx {
        0 => Some(&query.e1_semantic),              // E1 Semantic
        1 => Some(&query.e2_temporal_recent),       // E2 Temporal Recent
        2 => Some(&query.e3_temporal_periodic),     // E3 Temporal Periodic
        3 => Some(&query.e4_temporal_positional),   // E4 Temporal Positional
        4 => Some(query.e5_active_vector()),        // E5 Causal
        5 => None,                                  // E6 Sparse (NOT HNSW)
        6 => Some(&query.e7_code),                  // E7 Code
        7 => Some(query.e8_active_vector()),        // E8 Graph
        8 => Some(&query.e9_hdc),                   // E9 HDC
        9 => Some(query.e10_active_vector()),       // E10 Multimodal
        10 => Some(&query.e11_entity),              // E11 Entity
        11 => None,                                 // E12 LateInteraction (NOT HNSW)
        12 => None,                                 // E13 SPLADE (NOT HNSW)
        _ => None,
    }
}

/// Single-embedder search: search a specific HNSW index directly (CRIT-06).
///
/// When `embedder_indices` contains exactly one HNSW-capable index, this function
/// searches that specific index instead of always routing to E1.
fn search_single_embedder_sync(
    db: &Arc<DB>,
    index_registry: &Arc<EmbedderIndexRegistry>,
    soft_deleted: &Arc<DashMap<Uuid, i64>>,
    query: &SemanticFingerprint,
    options: &TeleologicalSearchOptions,
    embedder_idx: usize,
) -> CoreResult<Vec<TeleologicalSearchResult>> {
    let embedder = EmbedderIndex::from_index(embedder_idx);

    if !embedder.uses_hnsw() {
        return Err(CoreError::ValidationError {
            field: "embedder_indices".to_string(),
            message: format!(
                "Embedder index {} ({:?}) does not use HNSW and cannot be searched directly",
                embedder_idx, embedder
            ),
        });
    }

    let query_vec = get_query_vector_for_embedder(query, embedder_idx).ok_or_else(|| {
        CoreError::ValidationError {
            field: "embedder_indices".to_string(),
            message: format!("No query vector available for embedder index {}", embedder_idx),
        }
    })?;

    let entry_index = index_registry.get(embedder).ok_or_else(|| {
        CoreError::IndexError(format!("HNSW index {:?} not found in registry", embedder))
    })?;

    let k = (options.top_k * 2).max(20);
    let candidates = entry_index
        .search(query_vec, k, None)
        .map_err(|e| {
            error!("Single-embedder search for {:?} failed: {}", embedder, e);
            CoreError::IndexError(e.to_string())
        })?;

    debug!(
        "Single-embedder search: {:?} returned {} raw candidates",
        embedder,
        candidates.len()
    );

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
            let fp = deserialize_teleological_fingerprint(&data)?;
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

    debug!(
        "Single-embedder search: {:?} returned {} final results",
        embedder,
        results.len()
    );
    Ok(results)
}

/// Multi-embedder filtered search: MultiSpace but restricted to specific embedders (CRIT-06).
///
/// When `embedder_indices` contains multiple indices, this function runs MultiSpace
/// but only queries the specified HNSW indexes, ignoring all others.
fn search_filtered_multi_space_sync(
    db: &Arc<DB>,
    index_registry: &Arc<EmbedderIndexRegistry>,
    soft_deleted: &Arc<DashMap<Uuid, i64>>,
    query: &SemanticFingerprint,
    options: &TeleologicalSearchOptions,
    embedder_indices: &[usize],
) -> CoreResult<Vec<TeleologicalSearchResult>> {
    let weights = resolve_weights_sync(options)?;
    let k = (options.top_k * 3).max(50);

    let mut embedder_rankings: Vec<EmbedderRanking> = Vec::new();

    // Embedder name lookup for ranking labels
    let embedder_names: [&str; 13] = [
        "E1", "E2", "E3", "E4", "E5", "E6", "E7", "E8", "E9", "E10", "E11", "E12", "E13",
    ];

    for &idx in embedder_indices {
        if idx >= 13 {
            return Err(CoreError::ValidationError {
                field: "embedder_indices".to_string(),
                message: format!("Embedder index {} out of range (0-12)", idx),
            });
        }

        let base_embedder = EmbedderIndex::from_index(idx);
        if !base_embedder.uses_hnsw() {
            debug!("Skipping non-HNSW embedder {:?} in filtered multi-space", base_embedder);
            continue;
        }

        // Direction-aware E5 routing (Gap 1 fix): use directional index when causal direction known
        let (embedder, query_vec) = if idx == 4 {
            match options.causal_direction {
                CausalDirection::Cause => (EmbedderIndex::E5CausalEffect, query.get_e5_as_cause()),
                CausalDirection::Effect => (EmbedderIndex::E5CausalCause, query.get_e5_as_effect()),
                _ => match get_query_vector_for_embedder(query, idx) {
                    Some(v) => (base_embedder, v),
                    None => continue,
                },
            }
        } else {
            match get_query_vector_for_embedder(query, idx) {
                Some(v) => (base_embedder, v),
                None => continue,
            }
        };

        if let Some(index) = index_registry.get(embedder) {
            match index.search(query_vec, k, None) {
                Ok(candidates) => {
                    let ranked: Vec<(Uuid, f32)> = candidates
                        .into_iter()
                        .filter(|(id, _)| options.include_deleted || !is_soft_deleted_sync(soft_deleted, id))
                        .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
                        .collect();

                    if !ranked.is_empty() && weights[idx] > 0.0 {
                        embedder_rankings.push(EmbedderRanking::new(embedder_names[idx], weights[idx], ranked));
                    }
                }
                Err(e) => {
                    error!(
                        embedder = embedder_names[idx],
                        error = %e,
                        "HNSW search failed for embedder in filtered multi-space — degraded results"
                    );
                }
            }
        } else if idx == 4 && !matches!(options.causal_direction, CausalDirection::Unknown) {
            warn!(
                direction = ?options.causal_direction,
                index = ?embedder,
                "Direction-aware E5 HNSW index not found in registry; falling back to no E5 results"
            );
        }
    }

    debug!(
        "Filtered multi-space search: {} embedder rankings from {:?}",
        embedder_rankings.len(),
        embedder_indices
    );

    let fused_results = fuse_rankings(&embedder_rankings, options.fusion_strategy, options.top_k * 2);

    let code_query_type = options.effective_code_query_type();

    // Two-pass scoring: collect all embedder scores first, then suppress degenerate weights
    // Pass 1: Collect candidates with their embedder scores
    let mut candidates: Vec<(TeleologicalFingerprint, [f32; 13])> = Vec::with_capacity(fused_results.len());
    for fused in fused_results {
        if let Some(data) = get_fingerprint_raw_sync(db, fused.doc_id)? {
            let fp = deserialize_teleological_fingerprint(&data)?;
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
            candidates.push((fp, embedder_scores));
        }
    }

    // Suppress degenerate embedder weights based on cross-candidate variance
    let all_scores: Vec<[f32; 13]> = candidates.iter().map(|(_, s)| *s).collect();
    let adjusted_weights = suppress_degenerate_weights(&all_scores, &weights);

    // Pass 2: Score with adjusted weights
    // RRF is used for CANDIDATE SELECTION (which docs to retrieve from
    // multiple HNSW indexes), but the user-facing similarity score must
    // be a weighted cosine similarity in the 0.0-1.0 range.
    let mut results = Vec::with_capacity(candidates.len());
    for (fp, embedder_scores) in candidates {
        let final_score = compute_semantic_fusion(&embedder_scores, &adjusted_weights);
        if final_score < options.min_similarity {
            continue;
        }
        results.push(TeleologicalSearchResult::new(fp, final_score, embedder_scores));
    }

    results.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(options.top_k);

    debug!(
        "Filtered multi-space search returned {} results",
        results.len()
    );
    Ok(results)
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
        // AP-77: E5 MUST NOT use symmetric cosine — causal is directional.
        // Without an explicit direction, E5 cannot provide meaningful signal.
        // Use compute_embedder_scores_with_direction_sync when direction is known.
        0.0,
        // SEARCH-1 FIX: Normalize sparse cosine from [-1,1] to [0,1] to match
        // dense embedders which use (raw+1)/2. Without this, E6/E13 are
        // systematically under-weighted in fusion (0.8 raw vs 0.9 normalized).
        (query.e6_sparse.cosine_similarity(&stored.e6_sparse) + 1.0) / 2.0,
        e7_score,
        compute_cosine_similarity(query.e8_active_vector(), stored.e8_active_vector()),
        compute_cosine_similarity(&query.e9_hdc, &stored.e9_hdc),
        compute_cosine_similarity(query.e10_active_vector(), stored.e10_active_vector()),
        compute_cosine_similarity(&query.e11_entity, &stored.e11_entity),
        compute_maxsim_direct(&query.e12_late_interaction, &stored.e12_late_interaction),
        (query.e13_splade.cosine_similarity(&stored.e13_splade) + 1.0) / 2.0,
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
        // SEARCH-1 FIX: Normalize sparse cosine [-1,1] → [0,1]
        (query.e6_sparse.cosine_similarity(&stored.e6_sparse) + 1.0) / 2.0,
        e7_score,
        compute_cosine_similarity(query.e8_active_vector(), stored.e8_active_vector()),
        compute_cosine_similarity(&query.e9_hdc, &stored.e9_hdc),
        compute_cosine_similarity(query.e10_active_vector(), stored.e10_active_vector()),
        compute_cosine_similarity(&query.e11_entity, &stored.e11_entity),
        compute_maxsim_direct(&query.e12_late_interaction, &stored.e12_late_interaction),
        // SEARCH-1 FIX: Normalize sparse cosine [-1,1] → [0,1]
        (query.e13_splade.cosine_similarity(&stored.e13_splade) + 1.0) / 2.0,
    ]
}

/// E1-only search (sync version for spawn_blocking).
/// P5: Uses Arc<DashMap> for lock-free concurrent soft-delete checks.
fn search_e1_only_sync(
    db: &Arc<DB>,
    index_registry: &Arc<EmbedderIndexRegistry>,
    soft_deleted: &Arc<DashMap<Uuid, i64>>,
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
            let fp = deserialize_teleological_fingerprint(&data)?;
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
/// P5: Uses Arc<DashMap> for lock-free concurrent soft-delete checks.
fn search_multi_space_sync(
    db: &Arc<DB>,
    index_registry: &Arc<EmbedderIndexRegistry>,
    soft_deleted: &Arc<DashMap<Uuid, i64>>,
    query: &SemanticFingerprint,
    options: &TeleologicalSearchOptions,
) -> CoreResult<Vec<TeleologicalSearchResult>> {
    let weights = resolve_weights_sync(options)?;
    let k = (options.top_k * 3).max(50);

    let mut embedder_rankings: Vec<EmbedderRanking> = Vec::new();
    // SEARCH-4: Track embedders that failed HNSW search for operational visibility
    let mut degraded_embedders: Vec<&str> = Vec::new();

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

    // E5 Causal — direction-aware HNSW retrieval (Gap 1 fix)
    // When causal direction is known, query the directional HNSW index:
    //   Cause-seeking: query E5CausalEffect with cause vector (find memories whose effect matches our cause)
    //   Effect-seeking: query E5CausalCause with effect vector (find memories whose cause matches our effect)
    //   Unknown: legacy E5Causal with active vector
    let (e5_hnsw_idx, e5_query_vec) = match options.causal_direction {
        CausalDirection::Cause => (EmbedderIndex::E5CausalEffect, query.get_e5_as_cause()),
        CausalDirection::Effect => (EmbedderIndex::E5CausalCause, query.get_e5_as_effect()),
        _ => (EmbedderIndex::E5Causal, query.e5_active_vector()),
    };
    if let Some(e5_index) = index_registry.get(e5_hnsw_idx) {
        match e5_index.search(e5_query_vec, k, None) {
            Ok(e5_candidates) => {
                let e5_ranked: Vec<(Uuid, f32)> = e5_candidates
                    .into_iter()
                    .filter(|(id, _)| options.include_deleted || !is_soft_deleted_sync(soft_deleted, id))
                    .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
                    .collect();

                if !e5_ranked.is_empty() && weights[4] > 0.0 {
                    embedder_rankings.push(EmbedderRanking::new("E5", weights[4], e5_ranked));
                }
            }
            Err(e) => {
                degraded_embedders.push("E5");
                error!(
                    index = ?e5_hnsw_idx,
                    error = %e,
                    "E5 HNSW search failed in multi-space — degraded results"
                );
            }
        }
    } else if !matches!(options.causal_direction, CausalDirection::Unknown) {
        warn!(
            direction = ?options.causal_direction,
            index = ?e5_hnsw_idx,
            "Direction-aware E5 HNSW index not found in registry; falling back to no E5 results"
        );
    }

    // E7 Code
    if let Some(e7_index) = index_registry.get(EmbedderIndex::E7Code) {
        match e7_index.search(&query.e7_code, k, None) {
            Ok(e7_candidates) => {
                let e7_ranked: Vec<(Uuid, f32)> = e7_candidates
                    .into_iter()
                    .filter(|(id, _)| options.include_deleted || !is_soft_deleted_sync(soft_deleted, id))
                    .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
                    .collect();

                if !e7_ranked.is_empty() && weights[6] > 0.0 {
                    embedder_rankings.push(EmbedderRanking::new("E7", weights[6], e7_ranked));
                }
            }
            Err(e) => {
                degraded_embedders.push("E7");
                error!(
                    error = %e,
                    "E7 HNSW search failed in multi-space — degraded results"
                );
            }
        }
    }

    // E10 Multimodal
    // SEARCH-7: E10 uses paraphrase/doc asymmetric mode. The default e10_active_vector()
    // returns the "paraphrase" vector for queries, matching against stored "doc" vectors.
    // This is correct per ARCH-28: queries are paraphrased versions of stored documents,
    // so query=paraphrase and stored=doc aligns with the intended retrieval direction.
    if let Some(e10_index) = index_registry.get(EmbedderIndex::E10Multimodal) {
        match e10_index.search(query.e10_active_vector(), k, None) {
            Ok(e10_candidates) => {
                let e10_ranked: Vec<(Uuid, f32)> = e10_candidates
                    .into_iter()
                    .filter(|(id, _)| options.include_deleted || !is_soft_deleted_sync(soft_deleted, id))
                    .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
                    .collect();

                if !e10_ranked.is_empty() && weights[9] > 0.0 {
                    embedder_rankings.push(EmbedderRanking::new("E10", weights[9], e10_ranked));
                }
            }
            Err(e) => {
                degraded_embedders.push("E10");
                error!(
                    error = %e,
                    "E10 HNSW search failed in multi-space — degraded results"
                );
            }
        }
    }

    // E8 Graph (connectivity/structure embeddings, 1024D HNSW)
    // SEARCH-7: E8 uses source/target asymmetric mode. The default e8_active_vector()
    // returns the "source" vector for queries. This is correct because most queries
    // seek "what is connected to X?" (find targets of source). The stored vectors
    // contain the "target" representation, so query=source, stored=target is the
    // natural retrieval direction for structural relationship discovery.
    if let Some(e8_index) = index_registry.get(EmbedderIndex::E8Graph) {
        match e8_index.search(query.e8_active_vector(), k, None) {
            Ok(e8_candidates) => {
                let e8_ranked: Vec<(Uuid, f32)> = e8_candidates
                    .into_iter()
                    .filter(|(id, _)| options.include_deleted || !is_soft_deleted_sync(soft_deleted, id))
                    .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
                    .collect();

                if !e8_ranked.is_empty() && weights[7] > 0.0 {
                    embedder_rankings.push(EmbedderRanking::new("E8", weights[7], e8_ranked));
                }
            }
            Err(e) => {
                degraded_embedders.push("E8");
                error!(
                    error = %e,
                    "E8 HNSW search failed in multi-space — degraded results"
                );
            }
        }
    }

    // E11 Entity (KEPLER entity embeddings, 768D HNSW)
    if let Some(e11_index) = index_registry.get(EmbedderIndex::E11Entity) {
        match e11_index.search(&query.e11_entity, k, None) {
            Ok(e11_candidates) => {
                let e11_ranked: Vec<(Uuid, f32)> = e11_candidates
                    .into_iter()
                    .filter(|(id, _)| options.include_deleted || !is_soft_deleted_sync(soft_deleted, id))
                    .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
                    .collect();

                if !e11_ranked.is_empty() && weights[10] > 0.0 {
                    embedder_rankings.push(EmbedderRanking::new("E11", weights[10], e11_ranked));
                }
            }
            Err(e) => {
                degraded_embedders.push("E11");
                error!(
                    error = %e,
                    "E11 HNSW search failed in multi-space — degraded results"
                );
            }
        }
    }

    // SEARCH-2: E9 HDC (hyperdimensional computing, 1024D HNSW)
    // E9 provides noise-robust similarity via holographic reduced representations.
    // Included when weight > 0 (e.g., typo_tolerant profile sets E9=0.15).
    if let Some(e9_index) = index_registry.get(EmbedderIndex::E9HDC) {
        match e9_index.search(&query.e9_hdc, k, None) {
            Ok(e9_candidates) => {
                let e9_ranked: Vec<(Uuid, f32)> = e9_candidates
                    .into_iter()
                    .filter(|(id, _)| options.include_deleted || !is_soft_deleted_sync(soft_deleted, id))
                    .map(|(id, dist)| (id, 1.0 - dist.min(1.0)))
                    .collect();

                if !e9_ranked.is_empty() && weights[8] > 0.0 {
                    embedder_rankings.push(EmbedderRanking::new("E9", weights[8], e9_ranked));
                }
            }
            Err(e) => {
                degraded_embedders.push("E9");
                error!(
                    error = %e,
                    "E9 HNSW search failed in multi-space — degraded results"
                );
            }
        }
    }

    // SEARCH-4: Log summary of degraded embedders for operational visibility
    if !degraded_embedders.is_empty() {
        warn!(
            degraded = ?degraded_embedders,
            "Multi-space search completed with degraded embedders — results may be incomplete"
        );
    }

    debug!(
        "Multi-space search: {} embedder rankings collected, fusion_strategy={:?}",
        embedder_rankings.len(),
        options.fusion_strategy
    );

    let fused_results = fuse_rankings(&embedder_rankings, options.fusion_strategy, options.top_k * 2);

    let code_query_type = options.effective_code_query_type();

    // Two-pass scoring: collect all embedder scores first, then suppress degenerate weights
    // Pass 1: Collect candidates with their embedder scores
    let mut candidates: Vec<(TeleologicalFingerprint, [f32; 13])> = Vec::with_capacity(fused_results.len());
    for fused in fused_results {
        if let Some(data) = get_fingerprint_raw_sync(db, fused.doc_id)? {
            let fp = deserialize_teleological_fingerprint(&data)?;
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
            candidates.push((fp, embedder_scores));
        }
    }

    // Suppress degenerate embedder weights based on cross-candidate variance
    let all_scores: Vec<[f32; 13]> = candidates.iter().map(|(_, s)| *s).collect();
    let adjusted_weights = suppress_degenerate_weights(&all_scores, &weights);

    // Pass 2: Score with adjusted weights
    // RRF is used for CANDIDATE SELECTION (which docs to retrieve from
    // multiple HNSW indexes), but the user-facing similarity score must
    // be a weighted cosine similarity in the 0.0-1.0 range.
    let mut results = Vec::with_capacity(candidates.len());
    for (fp, embedder_scores) in candidates {
        let final_score = compute_semantic_fusion(&embedder_scores, &adjusted_weights);
        if final_score < options.min_similarity {
            continue;
        }
        results.push(TeleologicalSearchResult::new(fp, final_score, embedder_scores));
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

/// Pipeline search: 2-stage retrieval (sync version for spawn_blocking).
///
/// Stage 1: Fast recall via E13 SPLADE + E1/E5/E7/E8/E11 HNSW.
/// Stage 2: Multi-space scoring with weighted fusion across all candidates.
///
/// Note: E12 MaxSim reranking (Stage 3) is NOT implemented. See AP-74.
///
/// P1: Takes total_doc_count for O(1) IDF in sparse search stage.
/// P5: Uses DashMap for lock-free soft-delete checks.
fn search_pipeline_sync(
    db: &Arc<DB>,
    index_registry: &Arc<EmbedderIndexRegistry>,
    soft_deleted: &Arc<DashMap<Uuid, i64>>,
    query: &SemanticFingerprint,
    options: &TeleologicalSearchOptions,
    total_doc_count: usize,
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
        match search_sparse_sync(&**db, &query.e13_splade, recall_k, soft_deleted, total_doc_count) {
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

    // E5 Causal — direction-aware HNSW retrieval for pipeline search (Gap 1 fix)
    let (e5_pipeline_idx, e5_pipeline_vec) = match options.causal_direction {
        CausalDirection::Cause => (EmbedderIndex::E5CausalEffect, query.get_e5_as_cause()),
        CausalDirection::Effect => (EmbedderIndex::E5CausalCause, query.get_e5_as_effect()),
        _ => (EmbedderIndex::E5Causal, query.e5_active_vector()),
    };
    if let Some(e5_index) = index_registry.get(e5_pipeline_idx) {
        match e5_index.search(e5_pipeline_vec, recall_k / 2, None) {
            Ok(e5_candidates) => {
                let e5_count = e5_candidates.len();
                candidate_ids.extend(e5_candidates.into_iter().map(|(id, _)| id));
                debug!("Stage 1: E5 Causal ({:?}) returned {} additional candidates", e5_pipeline_idx, e5_count);
            }
            Err(e) => {
                warn!("Stage 1: E5 Causal search failed: {}, continuing without E5 candidates", e);
            }
        }
    } else if !matches!(options.causal_direction, CausalDirection::Unknown) {
        warn!(
            direction = ?options.causal_direction,
            index = ?e5_pipeline_idx,
            "Direction-aware E5 HNSW index not found in registry; falling back to no E5 candidates"
        );
    }

    // E7 Code
    if let Some(e7_index) = index_registry.get(EmbedderIndex::E7Code) {
        match e7_index.search(&query.e7_code, recall_k / 2, None) {
            Ok(e7_candidates) => {
                let e7_count = e7_candidates.len();
                candidate_ids.extend(e7_candidates.into_iter().map(|(id, _)| id));
                debug!("Stage 1: E7 Code returned {} additional candidates", e7_count);
            }
            Err(e) => {
                warn!("Stage 1: E7 Code search failed: {}, continuing without E7 candidates", e);
            }
        }
    }

    // E8 Graph (connectivity/structure)
    if let Some(e8_index) = index_registry.get(EmbedderIndex::E8Graph) {
        match e8_index.search(query.e8_active_vector(), recall_k / 2, None) {
            Ok(e8_candidates) => {
                let e8_count = e8_candidates.len();
                candidate_ids.extend(e8_candidates.into_iter().map(|(id, _)| id));
                debug!("Stage 1: E8 Graph returned {} additional candidates", e8_count);
            }
            Err(e) => {
                warn!("Stage 1: E8 Graph search failed: {}, continuing without E8 candidates", e);
            }
        }
    }

    // E11 Entity (KEPLER entity embeddings)
    if let Some(e11_index) = index_registry.get(EmbedderIndex::E11Entity) {
        match e11_index.search(&query.e11_entity, recall_k / 2, None) {
            Ok(e11_candidates) => {
                let e11_count = e11_candidates.len();
                candidate_ids.extend(e11_candidates.into_iter().map(|(id, _)| id));
                debug!("Stage 1: E11 Entity returned {} additional candidates", e11_count);
            }
            Err(e) => {
                warn!("Stage 1: E11 Entity search failed: {}, continuing without E11 candidates", e);
            }
        }
    }

    // SEARCH-2: E9 HDC (noise-robust hyperdimensional, 1024D HNSW)
    // Contributes candidates when weight > 0 (e.g., typo_tolerant profile).
    if let Some(e9_index) = index_registry.get(EmbedderIndex::E9HDC) {
        match e9_index.search(&query.e9_hdc, recall_k / 2, None) {
            Ok(e9_candidates) => {
                let e9_count = e9_candidates.len();
                candidate_ids.extend(e9_candidates.into_iter().map(|(id, _)| id));
                debug!("Stage 1: E9 HDC returned {} additional candidates", e9_count);
            }
            Err(e) => {
                warn!("Stage 1: E9 HDC search failed: {}, continuing without E9 candidates", e);
            }
        }
    }

    info!(
        "Stage 1 complete: {} unique candidates from E13+E1+E5+E7+E8+E9+E11",
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
            let fp = deserialize_teleological_fingerprint(&data)?;
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

    // Suppress degenerate embedder weights based on cross-candidate variance
    let all_scores: Vec<[f32; 13]> = candidate_data.iter().map(|(_, s, _)| *s).collect();
    let adjusted_weights = suppress_degenerate_weights(&all_scores, &weights);

    let mut scored_candidates: Vec<(Uuid, f32, [f32; 13], SemanticFingerprint)> =
        match options.fusion_strategy {
            FusionStrategy::WeightedSum => candidate_data
                .into_iter()
                .map(|(id, scores, semantic)| {
                    let fusion_score = compute_semantic_fusion(&scores, &adjusted_weights);
                    (id, fusion_score, scores, semantic)
                })
                .filter(|(_, score, _, _)| *score >= options.min_similarity)
                .collect(),
            FusionStrategy::WeightedRRF | FusionStrategy::ScoreWeightedRRF => {
                // SEARCH-5: Build embedder list dynamically from weights instead of
                // hardcoding indices. Excludes:
                //   - E2/E3/E4 (indices 1,2,3): temporal, not topical similarity (ARCH-25)
                //   - E12 (index 11): reranking only, not for RRF fusion (AP-74)
                let embedder_names_all: [&str; 13] = [
                    "E1", "E2", "E3", "E4", "E5", "E6", "E7", "E8", "E9", "E10", "E11", "E12", "E13",
                ];
                let excluded_from_rrf: [usize; 4] = [1, 2, 3, 11]; // E2,E3,E4 temporal + E12 rerank-only
                let semantic_indices: Vec<(usize, &str, f32)> = adjusted_weights.iter().enumerate()
                    .filter(|(idx, &w)| w > 0.0 && !excluded_from_rrf.contains(idx))
                    .map(|(idx, &w)| (idx, embedder_names_all[idx], w))
                    .collect();

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

                // Use ScoreWeightedRRF when requested: E5 contribution preserves score magnitude
                let fused = fuse_rankings(&embedder_rankings, options.fusion_strategy, stage2_k * 2);

                let mut candidate_map: HashMap<Uuid, ([f32; 13], SemanticFingerprint)> = candidate_data
                    .into_iter()
                    .map(|(id, scores, semantic)| (id, (scores, semantic)))
                    .collect();

                // P7: Use remove() to take ownership instead of get()+clone().
                // Avoids cloning ~63KB SemanticFingerprint per result.
                fused
                    .into_iter()
                    .filter_map(|f| {
                        candidate_map.remove(&f.doc_id).map(|(scores, semantic)| {
                            (f.doc_id, f.fused_score, scores, semantic)
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

    // STAGE 3: E12 COLBERT MAXSIM RERANKING (AP-74)
    // If enabled, compute MaxSim between query tokens and candidate e12_late_interaction tokens,
    // then interpolate with stage2 fusion score.
    if options.enable_rerank && !query.e12_late_interaction.is_empty() {
        let rerank_weight = options.rerank_weight;
        let mut reranked = Vec::with_capacity(scored_candidates.len());

        for (id, stage2_score, embedder_scores, semantic) in scored_candidates {
            let maxsim_score = if !semantic.e12_late_interaction.is_empty() {
                context_graph_core::retrieval::distance::max_sim(
                    &query.e12_late_interaction,
                    &semantic.e12_late_interaction,
                )
            } else {
                0.0
            };

            // Interpolate: final = (1 - weight) * stage2 + weight * maxsim
            let final_score = (1.0 - rerank_weight) * stage2_score + rerank_weight * maxsim_score;
            reranked.push((id, final_score, embedder_scores, semantic));
        }

        reranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        info!(
            "Stage 3 E12 MaxSim rerank complete: {} candidates reranked (weight={})",
            reranked.len(),
            rerank_weight
        );
        scored_candidates = reranked;
    }

    scored_candidates.truncate(options.top_k);

    debug!(
        "Pipeline search: {} final results (rerank={})",
        scored_candidates.len(),
        options.enable_rerank
    );

    // BUILD FINAL RESULTS
    // SEARCH-6: Re-read TeleologicalFingerprint from RocksDB for the full structure
    // (includes metadata fields beyond SemanticFingerprint), but carry the semantic
    // fingerprint forward to avoid a redundant deserialization of the embedding vectors.
    // The full TeleologicalFingerprint is needed for id, purpose, created_at, etc.
    let mut results = Vec::with_capacity(scored_candidates.len());

    for (id, score, embedder_scores, semantic) in scored_candidates {
        if let Some(data) = get_fingerprint_raw_sync(db, id)? {
            let mut fp = deserialize_teleological_fingerprint(&data)?;
            // Replace the deserialized semantic with the one we already have,
            // avoiding double-deserialization of the ~63KB embedding vectors.
            fp.semantic = semantic;
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
///
/// P1: Takes `total_doc_count` as parameter instead of doing O(n) full-iterator scan.
/// P5: Uses DashMap for lock-free soft-delete checks.
fn search_sparse_sync(
    db: &DB,
    sparse_query: &SparseVector,
    top_k: usize,
    soft_deleted: &Arc<DashMap<Uuid, i64>>,
    total_doc_count: usize,
) -> CoreResult<Vec<(Uuid, f32)>> {
    // P1: total_doc_count is O(1) atomic read (was O(n) iterator scan)
    let total_docs = total_doc_count.max(1) as f32;
    debug!(
        "Searching sparse with {} active terms, top_k={}, total_docs={} (BM25-IDF scoring)",
        sparse_query.nnz(),
        top_k,
        total_docs
    );

    let cf = db.cf_handle(CF_E13_SPLADE_INVERTED).ok_or_else(|| {
        CoreError::Internal("CF_E13_SPLADE_INVERTED not found".to_string())
    })?;

    struct TermData {
        query_weight: f32,
        doc_ids: Vec<Uuid>,
        idf: f32,
    }

    // P8: Batch-read all posting lists via multi_get_cf (was: per-term sequential get_cf).
    // For 100-term sparse queries, reduces from 100 individual reads to 1 batch read.
    let term_keys: Vec<[u8; 2]> = sparse_query.indices.iter()
        .map(|&term_id| e13_splade_inverted_key(term_id))
        .collect();
    let results = db.multi_get_cf(
        term_keys.iter().map(|k| (cf, k.as_slice()))
    );

    let mut term_data: Vec<TermData> = Vec::with_capacity(sparse_query.nnz());
    for (i, result) in results.into_iter().enumerate() {
        let query_weight = sparse_query.values[i];
        let data = result.map_err(|e| {
            CoreError::StorageError(format!("Failed to get E13 SPLADE term: {}", e))
        })?;
        if let Some(data) = data {
            let doc_ids = deserialize_memory_id_list(&data)?;
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
/// SEARCH-3: Kept in sync with the "semantic_search" named profile in
/// `context_graph_core::weights::WEIGHT_PROFILES`. Any changes there must
/// be mirrored here to avoid silent weight divergence.
/// Sum of non-zero weights = 1.0
/// E2-E4 (temporal) = 0.0 per research
const DEFAULT_SEMANTIC_WEIGHTS: [f32; 13] = [
    0.33, // E1 - Semantic (primary)
    0.0,  // E2 - Temporal Recent (metadata only)
    0.0,  // E3 - Temporal Periodic (metadata only)
    0.0,  // E4 - Temporal Positional (metadata only)
    0.15, // E5 - Causal
    0.05, // E6 - Sparse (keyword backup)
    0.20, // E7 - Code
    0.05, // E8 - Graph (relational)
    0.02, // E9 - HDC (noise-robust backup for typo tolerance)
    0.15, // E10 - Multimodal
    0.05, // E11 - Entity (relational)
    0.0,  // E12 - Late Interaction (used in Stage 3 rerank only)
    0.0,  // E13 - SPLADE (used in Stage 1 recall only)
];

// =============================================================================
// PIPELINE CONFIGURATION
// 2-stage pipeline: Stage 1 (sparse+dense recall) -> Stage 2 (multi-space scoring)
// E12 MaxSim reranking (Stage 3) is not yet implemented (AP-74).
// =============================================================================

/// Stage 1 recall multiplier (how many candidates to retrieve)
const STAGE1_RECALL_MULTIPLIER: usize = 10;

/// Stage 2 scoring candidate multiplier (intermediate set size before final truncation)
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
/// Suppress embedders with near-zero score variance before fusion.
///
/// If an embedder produces nearly identical scores for all candidates,
/// it contributes noise, not signal. Reduce its weight by SUPPRESSION_FACTOR.
/// This is defense-in-depth against degenerate embedders (e.g. E5 anisotropy).
fn suppress_degenerate_weights(
    all_scores: &[[f32; 13]],
    weights: &[f32; 13],
) -> [f32; 13] {
    const MIN_VARIANCE: f32 = 0.001;
    const SUPPRESSION_FACTOR: f32 = 0.25;

    if all_scores.len() < 3 {
        return *weights;
    }

    let mut adjusted = *weights;

    for idx in 0..13 {
        if weights[idx] <= 0.0 {
            continue;
        }

        // F-8 fix: detect all-zero scores BEFORE filtering.
        // If every candidate scored 0.0 for this embedder, the embedder produced
        // no signal — suppress its weight to prevent denominator inflation in fusion.
        let all_zero = all_scores.iter().all(|s| s[idx] == 0.0);
        if all_zero {
            tracing::debug!(
                embedder_idx = idx,
                original_weight = weights[idx],
                "Suppressing all-zero embedder weight (no signal produced)"
            );
            adjusted[idx] = 0.0;
            continue;
        }

        // P2: Welford's online algorithm — single-pass mean & variance.
        // Eliminates the second pass over all candidates for each embedder.
        let mut count = 0u32;
        let mut mean = 0.0f32;
        let mut m2 = 0.0f32;
        for s in all_scores.iter().map(|s| s[idx]).filter(|s| *s > 0.0) {
            count += 1;
            let delta = s - mean;
            mean += delta / count as f32;
            let delta2 = s - mean;
            m2 += delta * delta2;
        }
        if count < 3 {
            continue;
        }
        // SRC-8: Bessel's correction for unbiased sample variance.
        // Population variance (m2/count) underestimates true variance for small samples.
        let variance = m2 / (count - 1).max(1) as f32;
        if variance < MIN_VARIANCE {
            tracing::debug!(
                embedder_idx = idx,
                variance = variance,
                original_weight = weights[idx],
                suppressed_weight = weights[idx] * SUPPRESSION_FACTOR,
                "Suppressing degenerate embedder weight (variance < {MIN_VARIANCE})"
            );
            adjusted[idx] *= SUPPRESSION_FACTOR;
        }
    }

    adjusted
}

fn compute_semantic_fusion(scores: &[f32; 13], weights: &[f32; 13]) -> f32 {
    let mut weighted_sum = 0.0f32;
    let mut weight_total = 0.0f32;

    for (&score, &weight) in scores.iter().zip(weights.iter()) {
        // Only skip embedders with zero weight (not participating in this profile).
        // Zero SCORE is valid signal ("no match for this embedder") and MUST count
        // toward the denominator. Excluding zero scores inflated all fusion scores
        // by 15-33% (SRC-2 audit finding).
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
    /// - `Pipeline`: 2-stage retrieval (E13 recall + multi-space scoring)
    ///
    /// # Embedder-Specific Routing (CRIT-06)
    ///
    /// When `embedder_indices` is set on options, the search is routed to the specific
    /// HNSW index(es) regardless of the strategy field:
    /// - Single HNSW index: Direct search in that embedder's space
    /// - Multiple HNSW indices: Filtered multi-space search across only those embedders
    ///
    /// # Temporal Options (ARCH-14)
    ///
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
            "Searching semantic with strategy={:?}, top_k={}, min_similarity={}, embedder_indices={:?}, temporal_weight={}",
            options.strategy, options.top_k, options.min_similarity,
            options.embedder_indices,
            options.temporal_options.temporal_weight
        );

        // Clone Arc-wrapped fields for spawn_blocking closure
        // P5: Arc::clone for DashMap is 8 bytes (vs 5-478KB HashMap clone)
        let db = Arc::clone(&self.db);
        let index_registry = Arc::clone(&self.index_registry);
        let soft_deleted = Arc::clone(&self.soft_deleted);
        // P3: Wrap query in Arc to avoid cloning ~63KB SemanticFingerprint
        let query_arc = Arc::new(query.clone());
        let options_clone = options.clone();
        // P1: Read total_doc_count atomically (O(1) vs O(n) iterator)
        let total_docs = self.total_doc_count.load(Ordering::Relaxed);

        // Move synchronous search work to blocking thread pool
        let mut results = tokio::task::spawn_blocking(move || {
            let query_clone = &*query_arc;
            // CRIT-06: When embedder_indices is set, route to specific HNSW index(es)
            // instead of always defaulting to E1 or the strategy-based dispatch.
            if !options_clone.embedder_indices.is_empty() {
                let indices = &options_clone.embedder_indices;
                if indices.len() == 1 {
                    // Single embedder: search that specific HNSW index directly
                    debug!(
                        "Embedder-specific routing: single embedder index {}",
                        indices[0]
                    );
                    return search_single_embedder_sync(
                        &db, &index_registry, &soft_deleted, &query_clone, &options_clone,
                        indices[0],
                    );
                } else {
                    // Multiple embedders: filtered multi-space across only those embedders
                    debug!(
                        "Embedder-specific routing: filtered multi-space with {:?}",
                        indices
                    );
                    return search_filtered_multi_space_sync(
                        &db, &index_registry, &soft_deleted, &query_clone, &options_clone,
                        indices,
                    );
                }
            }

            // Standard strategy-based dispatch when no specific embedders requested
            match options_clone.strategy {
                SearchStrategy::E1Only => {
                    search_e1_only_sync(&db, &index_registry, &soft_deleted, &query_clone, &options_clone)
                }
                SearchStrategy::MultiSpace => {
                    search_multi_space_sync(&db, &index_registry, &soft_deleted, &query_clone, &options_clone)
                }
                SearchStrategy::Pipeline => {
                    warn!(
                        "Pipeline strategy uses 2-stage retrieval (E13 recall + multi-space scoring). \
                         E12 MaxSim reranking is not yet implemented."
                    );
                    search_pipeline_sync(&db, &index_registry, &soft_deleted, query_clone, &options_clone, total_docs)
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

        // Apply full temporal boost system (ARCH-14) if configured
        if options.temporal_options.has_any_boost() {
            self.apply_full_temporal_boosts(&mut results, query, &options).await?;
        }

        debug!("Semantic search returned {} results", results.len());
        Ok(results)
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
                        let anchor = match deserialize_teleological_fingerprint(&data) {
                            Ok(fp) => fp,
                            Err(e) => {
                                warn!(
                                    "Temporal boost: Could not deserialize anchor fingerprint {}: {}, skipping sequence boost",
                                    seq_opts.anchor_id, e
                                );
                                return Ok(());
                            }
                        };
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

        // P1: O(1) atomic read instead of O(n) count_async
        let total_doc_count = self.total_doc_count.load(Ordering::Relaxed);

        // Clone Arc-wrapped fields for spawn_blocking closure
        // P5: Arc::clone for DashMap is cheap (no HashMap clone)
        let db = Arc::clone(&self.db);
        let soft_deleted = Arc::clone(&self.soft_deleted);
        let sparse_query = sparse_query.clone();

        // Move synchronous RocksDB I/O to blocking thread pool
        tokio::task::spawn_blocking(move || {
            search_sparse_sync(&*db, &sparse_query, top_k, &soft_deleted, total_doc_count)
        })
        .await
        .map_err(|e| CoreError::Internal(format!("spawn_blocking failed: {}", e)))?
    }
}
