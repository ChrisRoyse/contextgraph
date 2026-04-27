//! Training-data export types and computation helpers.
//!
//! Defines [`TrainingRecord`] (**v3**) and supporting types ([`TrainingEdge`],
//! [`CausalLabel`], [`TopicMembership`], [`TemporalLabels`]) that package a
//! single memory's full multi-dimensional label space into a bincode-
//! serializable record.
//!
//! Records are persisted to `CF_TRAINING_RECORDS` in the teleological store.
//! See `context_graph_storage::teleological::rocksdb_store::export` for the
//! on-disk format and iteration helpers.
//!
//! # Content
//!
//! Each record bundles:
//! - Identity + provenance (UUID, content, importance, timestamps, source)
//! - All 14 embedding vectors (dense, sparse, token-level)
//! - 14D topic profile (per-embedder alignment) — loaded from `CF_TOPIC_PROFILES`
//!   when available, otherwise falls back to per-embedder vector-presence.
//! - 91 synergy-weighted cross-correlations
//! - 6D group alignments (Factual/Temporal/Causal/Relational/Qualitative/Implementation)
//! - Outgoing + incoming typed edges with per-embedder scores
//! - K-NN neighbors per embedder
//! - LLM-discovered causal relationships (both directions)
//! - Topic memberships
//! - Temporal labels (Phase 5: hour/day/month buckets, age, session position)
//! - Optional Tucker-core (Phase 4 hook; populated by Agent D later)
//! - **v2 (typed-edges feature)**: 8-dim `edge_type_distribution` —
//!   per-`GraphLinkEdgeType` count of outgoing edges. Indices follow
//!   [`crate::graph_linking::GraphLinkEdgeType::as_u8`]:
//!   `[SemanticSimilar, CodeRelated, EntityShared, CausalChain,
//!    GraphConnected, ParaphraseAligned, KeywordOverlap, MultiAgreement]`.
//!   See [`edge_signature::compute_edge_type_distribution`].
//!
//! # Versioning
//!
//! [`TRAINING_RECORD_VERSION`] is a single version byte prepended before the
//! bincode-encoded record on disk. Deserialization rejects mismatched versions
//! with `CoreError::SerializationError` — no automatic migration. **v1
//! v1/v2 records are unreadable by the v3 decoder; re-export is required.**

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::teleological::types::{TuckerCore, NUM_EMBEDDERS};

pub mod computation;
pub mod edge_signature;
pub mod temporal;

#[cfg(test)]
mod tests;

pub use computation::{
    compute_cross_correlations, compute_group_alignments, topic_profile_or_fallback,
};
pub use edge_signature::{compute_edge_type_distribution, NUM_EDGE_TYPE_DISTRIBUTION};
pub use temporal::{extract_temporal_labels, PeriodicBucket, TemporalLabels};

/// Current on-disk version byte for `TrainingRecord`.
///
/// Bumped on breaking layout changes. Deserialization rejects mismatches;
/// no automatic migration is supported.
///
/// **v3 (current)**: adds `TrainingRecord::e14_bge_m3_dense`.
/// v1/v2 readers cannot decode v3 payloads (and vice versa) - re-export required.
pub const TRAINING_RECORD_VERSION: u8 = 3;

/// Expected length of the `cross_correlations` vector: C(14, 2) = 91.
pub const NUM_CROSS_CORRELATIONS: usize = crate::teleological::CROSS_CORRELATION_COUNT;

/// Expected length of `group_alignments`: one per group (Factual, Temporal,
/// Causal, Relational, Qualitative, Implementation).
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
    /// SHA-256 hash of the source content, copied from TeleologicalFingerprint.
    pub content_hash: Option<[u8; 32]>,

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
    pub e14_bge_m3_dense: Vec<f32>,       // 1024D

    // --- Sparse embeddings (empty when includeSparseVectors=false) ---
    pub e6_sparse_indices: Vec<u16>,
    pub e6_sparse_values: Vec<f32>,
    pub e13_splade_indices: Vec<u16>,
    pub e13_splade_values: Vec<f32>,

    // --- Token-level embeddings (empty when includeTokenEmbeddings=false) ---
    /// E12 ColBERT: variable token count × 128D per token.
    pub e12_token_embeddings: Vec<Vec<f32>>,

    // --- Teleological fusion (derived) ---
    /// 14D topic profile: per-embedder alignment scores.
    ///
    /// Preferred source is `CF_TOPIC_PROFILES`; see
    /// [`topic_profile_or_fallback`].
    pub topic_profile: [f32; NUM_EMBEDDERS],
    /// 91 synergy-weighted pairwise interactions. Order: (0,1), (0,2), ..., (12,13).
    pub cross_correlations: Vec<f32>,
    /// 6D group alignments: Factual, Temporal, Causal, Relational, Qualitative, Implementation.
    pub group_alignments: [f32; NUM_GROUP_ALIGNMENTS],

    // --- Graph structure ---
    /// Typed edges where this memory is the source.
    pub outgoing_edges: Vec<TrainingEdge>,
    /// Typed edges where this memory is the target.
    pub incoming_edges: Vec<TrainingEdge>,
    /// K-NN neighbors per embedder index 0..13; always 14 outer vecs, may be
    /// empty inside when K-NN data is absent for the corresponding embedder.
    pub knn_neighbors: Vec<Vec<KnnNeighbor>>,

    // --- Causal labels ---
    /// LLM-discovered causal relationships where this memory is the cause.
    pub causal_effects: Vec<CausalLabel>,
    /// LLM-discovered causal relationships where this memory is the effect.
    pub causal_causes: Vec<CausalLabel>,

    // --- Cluster memberships ---
    pub topic_memberships: Vec<TopicMembership>,

    // --- Phase 5: Temporal labels ---
    /// Derived temporal features for time-aware training. `None` when
    /// `includeTemporalLabels=false` was passed to `export_training_corpus`.
    pub temporal_labels: Option<TemporalLabels>,

    // --- Phase 4: Tucker-core hook ---
    /// Placeholder populated by Agent D (Tucker-core export). Always `None`
    /// after a Phase-1 export.
    pub tucker_core: Option<TuckerCore>,

    // --- v2: Typed-edge relational signature ---
    /// 8-dim count of outgoing edges by `GraphLinkEdgeType`.
    ///
    /// Indices follow [`crate::graph_linking::GraphLinkEdgeType::as_u8`]:
    /// `[0]=SemanticSimilar, [1]=CodeRelated, [2]=EntityShared,
    /// [3]=CausalChain, [4]=GraphConnected, [5]=ParaphraseAligned,
    /// [6]=KeywordOverlap, [7]=MultiAgreement`.
    ///
    /// Computed by [`compute_edge_type_distribution`] over the same outgoing-
    /// edges iterator that fills `outgoing_edges`. The sum of this array
    /// equals `outgoing_edges.len()` modulo `u32::MAX` saturation. Bincode
    /// positional layout: this field MUST stay at the end (append-only).
    pub edge_type_distribution: [u32; NUM_EDGE_TYPE_DISTRIBUTION],
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
///
/// Per lesson 5 from `tasks/lessons.md`: `get_causal_relationships_by_source`
/// returns records whose `source_fingerprint_id == fp.id` by construction;
/// setting the peer to that ID means every peer is self. The `rel_id` (the
/// relationship UUID) is the stable join key; `related_memory_id` stays
/// `Uuid::nil()` unless the peer end is resolvable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalLabel {
    /// Peer end of the relationship when resolvable. `Uuid::nil()` when the
    /// causal record does not carry the other end's fingerprint id.
    pub related_memory_id: Uuid,
    /// Always the `CausalRelationship.id` so downstream tooling can join back
    /// against `CF_CAUSAL_RELATIONSHIPS`.
    pub rel_id: Uuid,
    /// "cause_statement -> effect_statement. explanation" concatenation.
    pub description: String,
    /// `"cause"` when this memory produces the other; `"effect"` when the
    /// other produces this.
    pub direction: String,
    pub confidence: f32,
    /// Mechanism classification from the LLM discovery step
    /// ("direct" / "mediated" / "feedback" / "temporal").
    pub mechanism_type: Option<String>,
}

/// Topic this memory belongs to, with the topic's 14D alignment profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMembership {
    pub topic_id: String,
    pub topic_label: Option<String>,
    pub topic_profile: [f32; NUM_EMBEDDERS],
    pub membership_probability: f32,
}
