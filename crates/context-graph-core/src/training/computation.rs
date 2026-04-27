//! Pure computation helpers for the training-record fusion features.
//!
//! Functions here are stateless and side-effect free:
//! - [`topic_profile_or_fallback`] — resolve the 14D topic profile, preferring
//!   the per-memory profile stored in `CF_TOPIC_PROFILES`; falls back to a
//!   presence-indicator derivation when that row is missing.
//! - [`compute_cross_correlations`] — 91 synergy-weighted pairwise products.
//! - [`compute_group_alignments`] — 6D group aggregation.
//!
//! None of these helpers read from the database. The caller is responsible for
//! looking up `CF_TOPIC_PROFILES` (when available) and passing the result as
//! the `stored_profile` argument.

use crate::teleological::groups::GroupAlignments;
use crate::teleological::synergy_matrix::SynergyMatrix;
use crate::teleological::types::NUM_EMBEDDERS;
use crate::types::fingerprint::{SemanticFingerprint, SparseVector};

use super::{NUM_CROSS_CORRELATIONS, NUM_GROUP_ALIGNMENTS};

/// Resolve the 14D topic profile for a memory.
///
/// **Preferred path** (fix #2 per `tasks/lessons.md`): when the caller has
/// already read `CF_TOPIC_PROFILES` for this memory, pass it as
/// `stored_profile = Some(profile)`. The returned profile is exactly those
/// bytes.
///
/// **Fallback path**: when the row is missing (legacy or newly-stored memory
/// that predates topic-profile persistence), this function derives a
/// *presence* indicator per embedder. This is **not** a real topical
/// alignment — it is a uniform scalar that signals "some vector is present".
/// Callers should treat it as a fallback only.
///
/// Fallback derivation (`stored_profile = None`):
/// - Dense vectors (E1/E2/E3/E4/E5_cause/E7/E8_source/E9/E10_paraphrase/E11/E14):
///   `l2_norm.clamp(0, 1)` — a unit-normalized vector yields ~1.0, empty yields 0.0.
/// - Sparse vectors (E6/E13): `sqrt(sum_sq).clamp(0, 1)`.
/// - Token-level (E12): mean of non-zero per-token dense alignments.
///
/// For asymmetric embedders the cause/source/paraphrase side is used.
pub fn topic_profile_or_fallback(
    stored_profile: Option<[f32; NUM_EMBEDDERS]>,
    fingerprint: &SemanticFingerprint,
) -> [f32; NUM_EMBEDDERS] {
    if let Some(p) = stored_profile {
        return p;
    }
    let mut profile = [0.0f32; NUM_EMBEDDERS];
    profile[0] = dense_alignment(&fingerprint.e1_semantic);
    profile[1] = dense_alignment(&fingerprint.e2_temporal_recent);
    profile[2] = dense_alignment(&fingerprint.e3_temporal_periodic);
    profile[3] = dense_alignment(&fingerprint.e4_temporal_positional);
    profile[4] = dense_alignment(&fingerprint.e5_causal_as_cause);
    profile[5] = sparse_alignment(&fingerprint.e6_sparse);
    profile[6] = dense_alignment(&fingerprint.e7_code);
    profile[7] = dense_alignment(&fingerprint.e8_graph_as_source);
    profile[8] = dense_alignment(&fingerprint.e9_hdc);
    profile[9] = dense_alignment(&fingerprint.e10_multimodal_paraphrase);
    profile[10] = dense_alignment(&fingerprint.e11_entity);
    profile[11] = token_level_alignment(&fingerprint.e12_late_interaction);
    profile[12] = sparse_alignment(&fingerprint.e13_splade);
    profile[13] = dense_alignment(&fingerprint.e14_bge_m3_dense);
    profile
}

fn dense_alignment(vec: &[f32]) -> f32 {
    if vec.is_empty() {
        return 0.0;
    }
    let norm_sq: f32 = vec.iter().map(|x| x * x).sum();
    if !norm_sq.is_finite() || norm_sq <= 0.0 {
        return 0.0;
    }
    norm_sq.sqrt().clamp(0.0, 1.0)
}

fn sparse_alignment(sparse: &SparseVector) -> f32 {
    if sparse.indices.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = sparse.values.iter().map(|x| x * x).sum();
    if !sum_sq.is_finite() || sum_sq <= 0.0 {
        return 0.0;
    }
    sum_sq.sqrt().clamp(0.0, 1.0)
}

fn token_level_alignment(tokens: &[Vec<f32>]) -> f32 {
    if tokens.is_empty() {
        return 0.0;
    }
    let mut total = 0.0f32;
    let mut counted = 0usize;
    for tok in tokens {
        let a = dense_alignment(tok);
        if a > 0.0 {
            total += a;
            counted += 1;
        }
    }
    if counted == 0 {
        0.0
    } else {
        (total / counted as f32).clamp(0.0, 1.0)
    }
}

/// Compute the 91 synergy-weighted cross-correlations.
///
/// For each unique pair (i, j) with i < j in 0..14:
///
/// ```text
/// cross_corr[(i,j)] = alignment_i * alignment_j * synergy(i, j)
/// ```
///
/// Pair ordering matches [`crate::teleological::TeleologicalVector`] and the
/// training-record contract: (0,1), (0,2), ..., (0,13), (1,2), ..., (12,13).
/// Returns exactly [`NUM_CROSS_CORRELATIONS`] values in [0, 1].
pub fn compute_cross_correlations(
    topic_profile: &[f32; NUM_EMBEDDERS],
    synergy: &SynergyMatrix,
) -> Vec<f32> {
    let mut out = Vec::with_capacity(NUM_CROSS_CORRELATIONS);
    for i in 0..NUM_EMBEDDERS {
        for j in (i + 1)..NUM_EMBEDDERS {
            let a_i = topic_profile[i].clamp(0.0, 1.0);
            let a_j = topic_profile[j].clamp(0.0, 1.0);
            let w = synergy.get_synergy(i, j).clamp(0.0, 1.0);
            out.push(a_i * a_j * w);
        }
    }
    debug_assert_eq!(out.len(), NUM_CROSS_CORRELATIONS);
    out
}

/// Compute the 6D group alignments from a 14D topic profile.
///
/// Delegates to [`GroupAlignments::from_alignments`] which averages within
/// each group per teleoplan.md Section 3.2:
/// Factual=E1/E12/E13/E14, Temporal=E2/E3, Causal=E4/E7,
/// Relational=E5/E8/E9, Qualitative=E10/E11, Implementation=E6.
pub fn compute_group_alignments(
    topic_profile: &[f32; NUM_EMBEDDERS],
) -> [f32; NUM_GROUP_ALIGNMENTS] {
    GroupAlignments::from_alignments(topic_profile, None).as_array()
}
