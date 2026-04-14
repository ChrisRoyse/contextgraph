//! Training data export types and computation helpers.
//!
//! Defines [`TrainingRecord`] and supporting types ([`TrainingEdge`],
//! [`CausalLabel`], [`TopicMembership`]) that package a single memory's
//! full multi-dimensional label space into a bincode-serializable record.
//!
//! Records are persisted to `CF_TRAINING_RECORDS` in the teleological store.
//! See `context_graph_storage::teleological::rocksdb_store::export` for the
//! on-disk format and iteration helpers.
//!
//! # Content
//!
//! Each record bundles:
//! - Identity + provenance (UUID, content, importance, timestamps, source)
//! - All 13 embedding vectors (dense, sparse, token-level)
//! - 13D topic profile (per-embedder alignment)
//! - 78 synergy-weighted cross-correlations
//! - 6D group alignments (Factual/Temporal/Causal/Relational/Qualitative/Implementation)
//! - Outgoing + incoming typed edges with per-embedder scores
//! - K-NN neighbors per embedder
//! - LLM-discovered causal relationships (both directions)
//! - Topic memberships
//!
//! # Versioning
//!
//! [`TRAINING_RECORD_VERSION`] is a single version byte prepended before the
//! bincode-encoded record on disk. Deserialization rejects mismatched versions
//! with `CoreError::SerializationError` — no automatic migration.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::teleological::groups::GroupAlignments;
use crate::teleological::synergy_matrix::SynergyMatrix;
use crate::teleological::types::NUM_EMBEDDERS;
use crate::types::fingerprint::{SemanticFingerprint, SparseVector};

#[cfg(test)]
mod tests;

/// Current on-disk version byte for `TrainingRecord`.
///
/// Bumped on breaking layout changes. Deserialization rejects mismatches;
/// no automatic migration is supported.
pub const TRAINING_RECORD_VERSION: u8 = 1;

/// Expected length of the `cross_correlations` vector: C(13, 2) = 78.
pub const NUM_CROSS_CORRELATIONS: usize = 78;

/// Expected length of `group_alignments`: one per [`GroupType`].
pub const NUM_GROUP_ALIGNMENTS: usize = 6;

/// A single training record containing all multi-dimensional data for one memory.
///
/// Serialized with bincode (positional format — do NOT use `skip_serializing_if`
/// per constitution; add new fields at the end only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingRecord {
    // --- Identity ---
    pub memory_id: Uuid,
    pub content: String,
    pub importance: f32,
    pub created_at: DateTime<Utc>,
    pub session_id: Option<String>,

    // --- Source provenance ---
    pub source_type: Option<String>,
    pub source_path: Option<String>,

    // --- Dense embeddings (all L2-normalized; empty Vec when includeEmbeddings=false) ---
    pub e1_semantic: Vec<f32>,            // 1024D
    pub e2_temporal_recent: Vec<f32>,     // 512D
    pub e3_temporal_periodic: Vec<f32>,   // 512D
    pub e4_temporal_positional: Vec<f32>, // 512D
    pub e5_causal_cause: Vec<f32>,        // 768D (asymmetric)
    pub e5_causal_effect: Vec<f32>,       // 768D (asymmetric)
    pub e7_code: Vec<f32>,                // 1536D
    pub e8_graph_source: Vec<f32>,        // 1024D (asymmetric)
    pub e8_graph_target: Vec<f32>,        // 1024D (asymmetric)
    pub e9_hdc: Vec<f32>,                 // 1024D
    pub e10_paraphrase: Vec<f32>,         // 768D (asymmetric)
    pub e10_context: Vec<f32>,            // 768D (asymmetric)
    pub e11_entity: Vec<f32>,             // 768D

    // --- Sparse embeddings (empty when includeSparseVectors=false) ---
    pub e6_sparse_indices: Vec<u16>,
    pub e6_sparse_values: Vec<f32>,
    pub e13_splade_indices: Vec<u16>,
    pub e13_splade_values: Vec<f32>,

    // --- Token-level embeddings (empty when includeTokenEmbeddings=false) ---
    /// E12 ColBERT: variable token count × 128D per token.
    pub e12_token_embeddings: Vec<Vec<f32>>,

    // --- Teleological fusion (derived) ---
    /// 13D topic profile: per-embedder alignment scores (magnitude-based).
    pub topic_profile: [f32; NUM_EMBEDDERS],
    /// 78 synergy-weighted pairwise interactions. Order: (0,1), (0,2), ..., (11,12).
    pub cross_correlations: Vec<f32>,
    /// 6D group alignments: Factual, Temporal, Causal, Relational, Qualitative, Implementation.
    pub group_alignments: [f32; NUM_GROUP_ALIGNMENTS],

    // --- Graph structure ---
    /// Typed edges where this memory is the source.
    pub outgoing_edges: Vec<TrainingEdge>,
    /// Typed edges where this memory is the target.
    pub incoming_edges: Vec<TrainingEdge>,
    /// K-NN neighbors per embedder index 0..13; empty inner vecs for embedders
    /// without K-NN data (E6/E12/E13 and for fingerprints the background
    /// graph builder has not yet processed).
    pub knn_neighbors: Vec<Vec<KnnNeighbor>>,

    // --- Causal labels ---
    /// LLM-discovered causal relationships where this memory is the cause.
    pub causal_effects: Vec<CausalLabel>,
    /// LLM-discovered causal relationships where this memory is the effect.
    pub causal_causes: Vec<CausalLabel>,

    // --- Cluster memberships ---
    pub topic_memberships: Vec<TopicMembership>,
}

/// Compact training-time representation of a typed graph edge.
///
/// Direction meaning: `Symmetric` = 0, `Forward` = 1, `Backward` = 2 (matches
/// [`crate::graph_linking::DirectedRelation`] repr).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingEdge {
    /// Edge type as `GraphLinkEdgeType` repr(u8). 0=SemanticSimilar through 7=MultiAgreement.
    pub edge_type: u8,
    /// Peer node: target for outgoing edges, source for incoming edges.
    pub peer_id: Uuid,
    pub weight: f32,
    pub direction: u8,
    pub agreement_count: u8,
    pub embedder_scores: [f32; NUM_EMBEDDERS],
}

/// A single K-NN neighbor in a specific embedder space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnnNeighbor {
    pub target_id: Uuid,
    pub similarity: f32,
}

/// LLM-discovered causal relationship label.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalLabel {
    /// The other end of the relationship (UUID of the effect when this memory
    /// is the cause, or UUID of the cause when this memory is the effect).
    pub related_memory_id: Uuid,
    pub description: String,
    /// "cause" when this memory produces the other; "effect" when the other produces this.
    pub direction: String,
    pub confidence: f32,
}

/// Topic this memory belongs to, with the topic's 13D alignment profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMembership {
    pub topic_id: String,
    pub topic_label: Option<String>,
    pub topic_profile: [f32; NUM_EMBEDDERS],
    pub membership_probability: f32,
}

// =============================================================================
// Computation helpers
// =============================================================================

/// Compute a 13D topic profile from a [`SemanticFingerprint`] as per-embedder
/// vector magnitudes (each normalized so the resulting profile lives in [0, 1]).
///
/// Used when the caller does not have access to the `CF_TOPIC_PROFILES` CF or
/// wants a deterministic derivation from the fingerprint.
///
/// The computation:
/// 1. For dense vectors: `min(1.0, l2_norm / sqrt(dim))` — a unit L2-normalized
///    vector yields 1.0 / sqrt(dim) · sqrt(dim) = 1.0 (normalized vectors map to 1.0).
/// 2. For sparse vectors (E6/E13): non-zero term ratio `nnz / vocab_size`, clamped.
/// 3. For token-level (E12): average token norm, clamped.
///
/// For asymmetric embedders (E5/E8/E10), the cause/source/paraphrase side is used.
pub fn compute_topic_profile_from_fingerprint(
    fp: &SemanticFingerprint,
) -> [f32; NUM_EMBEDDERS] {
    let mut profile = [0.0f32; NUM_EMBEDDERS];
    profile[0] = dense_alignment(&fp.e1_semantic);
    profile[1] = dense_alignment(&fp.e2_temporal_recent);
    profile[2] = dense_alignment(&fp.e3_temporal_periodic);
    profile[3] = dense_alignment(&fp.e4_temporal_positional);
    profile[4] = dense_alignment(&fp.e5_causal_as_cause);
    profile[5] = sparse_alignment(&fp.e6_sparse);
    profile[6] = dense_alignment(&fp.e7_code);
    profile[7] = dense_alignment(&fp.e8_graph_as_source);
    profile[8] = dense_alignment(&fp.e9_hdc);
    profile[9] = dense_alignment(&fp.e10_multimodal_paraphrase);
    profile[10] = dense_alignment(&fp.e11_entity);
    profile[11] = token_level_alignment(&fp.e12_late_interaction);
    profile[12] = sparse_alignment(&fp.e13_splade);
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
    // Unit L2-normalized vectors yield ~1.0; near-zero norms yield near-0.0.
    norm_sq.sqrt().clamp(0.0, 1.0)
}

fn sparse_alignment(sparse: &SparseVector) -> f32 {
    let nnz = sparse.indices.len();
    if nnz == 0 {
        return 0.0;
    }
    let sum_sq: f32 = sparse.values.iter().map(|x| x * x).sum();
    if !sum_sq.is_finite() || sum_sq <= 0.0 {
        return 0.0;
    }
    // Presence-based alignment: sqrt(sum of squared values), clamped.
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

/// Compute the 78 synergy-weighted cross-correlations.
///
/// For each unique pair (i, j) with i < j in 0..13:
///
/// ```text
/// cross_corr[(i,j)] = alignment_i * alignment_j * synergy(i, j)
/// ```
///
/// Pair ordering matches [`crate::teleological::TeleologicalVector`] and the
/// training-record contract: (0,1), (0,2), …, (0,12), (1,2), …, (11,12).
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

/// Compute the 6D group alignments from a 13D topic profile.
///
/// Delegates to [`GroupAlignments::from_alignments`] which averages within
/// each group per teleoplan.md Section 3.2:
/// Factual=E1/E12/E13, Temporal=E2/E3, Causal=E4/E7,
/// Relational=E5/E8/E9, Qualitative=E10/E11, Implementation=E6.
pub fn compute_group_alignments(
    topic_profile: &[f32; NUM_EMBEDDERS],
) -> [f32; NUM_GROUP_ALIGNMENTS] {
    GroupAlignments::from_alignments(topic_profile, None).as_array()
}
