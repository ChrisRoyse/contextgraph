//! TeleologicalFingerprint type definition.
//!
//! This module contains the struct definition for the complete teleological fingerprint.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::fingerprint::{SemanticFingerprint, SparseVector};

/// Number of embedder dimensions in the teleological signature vector.
pub const TELEOLOGICAL_VECTOR_DIM: usize = 13;

/// Complete teleological fingerprint for a memory node.
///
/// This struct combines semantic content with metadata for tracking
/// and retrieval.
///
/// From constitution.yaml:
/// - Expected size: ~46KB per node (+ ~2KB if e6_sparse is present)
///
/// # E6 Sparse Vector
///
/// The optional `e6_sparse` field stores the original sparse vector from E6
/// (V_selectivity) embedder before projection to dense. This enables:
/// - Stage 1 sparse recall via inverted index
/// - Exact keyword matching for technical queries
/// - E6 tie-breaking when E1 scores are close
///
/// See docs/e6upgrade.md for the full E6 enhancement proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleologicalFingerprint {
    /// Unique identifier for this fingerprint (UUID v4)
    pub id: Uuid,

    /// The 13-embedding semantic fingerprint (from TASK-F001)
    pub semantic: SemanticFingerprint,

    /// SHA-256 hash of the source content (32 bytes)
    pub content_hash: [u8; 32],

    /// When this fingerprint was created
    pub created_at: DateTime<Utc>,

    /// When this fingerprint was last updated
    pub last_updated: DateTime<Utc>,

    /// Number of times this memory has been accessed
    pub access_count: u64,

    /// Importance score [0.0, 1.0] for memory prioritization.
    /// Used by consolidation, boost_importance, and dream phases.
    /// Default: 0.5
    pub importance: f32,

    /// When this memory was last accessed (read) for importance decay.
    /// Default: same as `created_at`. Updated on search result retrieval.
    /// Used for computing effective_importance = importance * 0.5^(days_since_access / 30).
    ///
    /// NOTE: This field uses `#[serde(skip)]` because TeleologicalFingerprint is
    /// serialized with bincode (positional format). Adding a new field to the
    /// bincode layout would break deserialization of all existing data. The field
    /// exists only in memory and resets to `Utc::now()` on deserialization.
    /// For persistent access tracking, use a separate CF in the future.
    #[serde(skip, default = "default_last_accessed_at")]
    pub last_accessed_at: DateTime<Utc>,

    /// Original E6 sparse vector for Stage 1 recall and keyword matching.
    ///
    /// This field is optional for backward compatibility with existing fingerprints.
    /// New fingerprints should populate this during embedding generation.
    ///
    /// - Typical size: ~235 active terms (~1.4KB)
    /// - Used for: inverted index recall, exact term matching, tie-breaking
    /// - NOT used for: semantic similarity (use projected dense in `semantic`)
    ///
    /// NOTE: We use #[serde(default)] but NOT skip_serializing_if because bincode
    /// uses a fixed format and doesn't support field skipping. All fields must
    /// be serialized for bincode compatibility.
    #[serde(default)]
    pub e6_sparse: Option<SparseVector>,
}

/// Default for `last_accessed_at` when deserializing legacy fingerprints
/// that lack this field. Uses `Utc::now()` which is conservative -- old
/// memories appear "just accessed" rather than unfairly penalized.
fn default_last_accessed_at() -> DateTime<Utc> {
    Utc::now()
}

impl TeleologicalFingerprint {
    /// Compute 13D teleological signature vector.
    ///
    /// Each dimension is the L2 norm of the corresponding embedder's vector,
    /// providing a compact representation of the memory's "shape" across all
    /// 13 embedding spaces. This enables ultra-fast first-pass candidate
    /// filtering before full multi-embedder search.
    ///
    /// - Dense embedders (E1, E2-E5, E7-E11): L2 norm of the dense vector
    /// - Sparse embedders (E6, E13): number of active terms normalized to [0, 1]
    /// - Token embedder (E12): average L2 norm across all tokens
    ///
    /// For asymmetric embedders (E5, E8, E10), the "active" vector is used
    /// (cause/source/paraphrase side, with legacy fallback).
    pub fn teleological_vector(&self) -> [f32; TELEOLOGICAL_VECTOR_DIM] {
        let s = &self.semantic;
        [
            l2_norm(&s.e1_semantic),                 // E1  Semantic
            l2_norm(&s.e2_temporal_recent),          // E2  Temporal-Recent
            l2_norm(&s.e3_temporal_periodic),        // E3  Temporal-Periodic
            l2_norm(&s.e4_temporal_positional),      // E4  Temporal-Positional
            l2_norm(s.e5_active_vector()),           // E5  Causal (active side)
            sparse_norm(&s.e6_sparse),               // E6  Sparse Lexical
            l2_norm(&s.e7_code),                     // E7  Code
            l2_norm(s.e8_active_vector()),           // E8  Graph (active side)
            l2_norm(&s.e9_hdc),                      // E9  HDC
            l2_norm(s.e10_active_vector()),          // E10 Multimodal (active side)
            l2_norm(&s.e11_entity),                  // E11 Entity
            token_avg_norm(&s.e12_late_interaction), // E12 Late-Interaction
            sparse_norm(&s.e13_splade),              // E13 SPLADE
        ]
    }
}

/// L2 norm of a dense vector.
///
/// Returns 0.0 for empty vectors (graceful handling of missing embeddings).
#[inline]
fn l2_norm(v: &[f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = v.iter().map(|x| x * x).sum();
    sum_sq.sqrt()
}

/// Sparse vector norm: number of active terms / 1000 (normalized to reasonable range).
///
/// Dividing by 1000 keeps the value in a comparable magnitude to dense L2 norms
/// (typical SPLADE has ~200-1500 active terms, so this yields 0.2-1.5).
#[inline]
fn sparse_norm(sv: &SparseVector) -> f32 {
    sv.indices.len() as f32 / 1000.0
}

/// Average L2 norm across token embeddings (E12 ColBERT).
///
/// Returns 0.0 when there are no tokens (empty content).
#[inline]
fn token_avg_norm(tokens: &[Vec<f32>]) -> f32 {
    if tokens.is_empty() {
        return 0.0;
    }
    let sum: f32 = tokens.iter().map(|t| l2_norm(t)).sum();
    sum / tokens.len() as f32
}
