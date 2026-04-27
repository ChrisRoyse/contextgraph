//! Similarity calculation functions for the in-memory teleological store.
//!
//! This module contains cosine similarity and semantic score computation functions
//! used by the search operations.
//!
//! IMPORTANT: These functions MUST match the production scoring in
//! context-graph-storage/src/teleological/rocksdb_store/search.rs lines 446-466.

use crate::types::fingerprint::{SemanticFingerprint, NUM_EMBEDDERS};

/// Compute cosine similarity between two dense vectors.
/// Returns [0,1] via SRC-3 normalization: (raw+1)/2.
pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;

    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom < f32::EPSILON {
        0.0
    } else {
        // Normalize from [-1,1] to [0,1] to match production (SRC-3: (raw+1)/2)
        (dot / denom + 1.0) / 2.0
    }
}

/// Compute semantic similarity across all embedders.
///
/// MATCHES production scoring at search.rs:446-466:
/// - E2: cosine similarity (E2 vectors are now unique per memory — bug fixed)
/// - E5: returns -1.0 sentinel (AP-77: causal requires direction; stub has no direction context)
/// - E6/E13: sparse cosine normalized to [0,1] via (raw+1)/2
/// - E8: asymmetric cross-pair max: max(source→target, target→source)
/// - E10: asymmetric cross-pair max: max(paraphrase→context, context→paraphrase)
pub(crate) fn compute_semantic_scores(
    query: &SemanticFingerprint,
    target: &SemanticFingerprint,
) -> [f32; NUM_EMBEDDERS] {
    let mut scores = [0.0_f32; NUM_EMBEDDERS];

    // E1: Semantic
    scores[0] = cosine_similarity(&query.e1_semantic, &target.e1_semantic);

    // E2: Temporal Recent — cosine similarity (E2 vectors are now unique per memory)
    scores[1] = cosine_similarity(&query.e2_temporal_recent, &target.e2_temporal_recent);

    // E3: Temporal Periodic
    scores[2] = cosine_similarity(&query.e3_temporal_periodic, &target.e3_temporal_periodic);

    // E4: Temporal Positional
    scores[3] = cosine_similarity(
        &query.e4_temporal_positional,
        &target.e4_temporal_positional,
    );

    // E5: AP-77 FIX: Return -1.0 sentinel (causal requires direction context which
    // the in-memory stub does not have). Production returns -1.0 via
    // compute_similarity_for_space_with_direction when direction is Unknown.
    // The -1.0 sentinel causes fusion to skip E5 via suppress_degenerate_weights.
    scores[4] = -1.0;

    // E6: SEARCH-1 FIX: Normalize sparse cosine [-1,1] to [0,1]
    scores[5] = (query.e6_sparse.cosine_similarity(&target.e6_sparse) + 1.0) / 2.0;

    // E7: Code
    scores[6] = cosine_similarity(&query.e7_code, &target.e7_code);

    // E8: STOR-H1 FIX: Asymmetric — max of both cross-directions
    scores[7] = cosine_similarity(query.get_e8_as_source(), target.get_e8_as_target()).max(
        cosine_similarity(query.get_e8_as_target(), target.get_e8_as_source()),
    );

    // E9: HDC
    scores[8] = cosine_similarity(&query.e9_hdc, &target.e9_hdc);

    // E10: STOR-H1 FIX: Asymmetric — max of both cross-directions
    scores[9] = cosine_similarity(query.get_e10_as_paraphrase(), target.get_e10_as_context()).max(
        cosine_similarity(query.get_e10_as_context(), target.get_e10_as_paraphrase()),
    );

    // E11: Entity
    scores[10] = cosine_similarity(&query.e11_entity, &target.e11_entity);

    // E12: Late Interaction (simplified: average token similarities)
    scores[11] =
        compute_late_interaction_score(&query.e12_late_interaction, &target.e12_late_interaction);

    // E13: SEARCH-1 FIX: Normalize sparse cosine [-1,1] to [0,1]
    scores[12] = (query.e13_splade.cosine_similarity(&target.e13_splade) + 1.0) / 2.0;

    // E14: BGE-M3 dense
    scores[13] = cosine_similarity(&query.e14_bge_m3_dense, &target.e14_bge_m3_dense);

    scores
}

/// Compute ColBERT-style late interaction score (MaxSim).
pub(crate) fn compute_late_interaction_score(
    query_tokens: &[Vec<f32>],
    target_tokens: &[Vec<f32>],
) -> f32 {
    if query_tokens.is_empty() || target_tokens.is_empty() {
        return 0.0;
    }

    // MaxSim: for each query token, find max similarity to any target token
    let mut total = 0.0_f32;
    for q_tok in query_tokens {
        let max_sim = target_tokens
            .iter()
            .map(|t_tok| cosine_similarity(q_tok, t_tok))
            .fold(f32::NEG_INFINITY, f32::max);
        if max_sim.is_finite() {
            total += max_sim;
        }
    }

    total / query_tokens.len() as f32
}
