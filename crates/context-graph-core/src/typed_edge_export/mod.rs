//! Wire types for `export_typed_edges_corpus` (Feature 1 of the typed-edges
//! training-data factory).
//!
//! Each [`TypedEdgeTrainingRecord`] carries one
//! `(source_memory_id, target_memory_id, edge_type)` observation plus the
//! full per-embedder similarity profile, optional content / source-metadata
//! join, optional mechanism-type join (for `CausalChain`), and an optional
//! [`LLMValidationSummary`] populated when the LLM edge-validation feedback
//! loop has issued a verdict for the same edge.
//!
//! Records are persisted to the new `CF_TYPED_EDGE_RECORDS` column family
//! as `[version_byte = TYPED_EDGE_RECORD_VERSION][bincode-encoded record]`.
//!
//! # Versioning
//!
//! [`TYPED_EDGE_RECORD_VERSION`] is a single version byte prepended before
//! the bincode payload on disk. Decoders must reject any other value with
//! `CoreError::SerializationError` — no automatic migration. Bincode
//! positional layout means new fields **only** append at the end.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::teleological::types::NUM_EMBEDDERS;

/// Current on-disk version byte for [`TypedEdgeTrainingRecord`] payloads in
/// `CF_TYPED_EDGE_RECORDS`.
///
/// Bumped on breaking layout changes; deserialization rejects mismatches with
/// `CoreError::SerializationError`. No automatic migration is supported.
pub const TYPED_EDGE_RECORD_VERSION: u8 = 1;

/// One row of the exported typed-edge training corpus.
///
/// Serialized with bincode (positional). New fields go at the end only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TypedEdgeTrainingRecord {
    /// Source memory UUID (anchor end of the edge).
    pub source_memory_id: Uuid,
    /// Target memory UUID (peer end of the edge).
    pub target_memory_id: Uuid,
    /// Edge type as `GraphLinkEdgeType::as_u8()` (0..=7).
    pub edge_type: u8,
    /// Snake-case enum name; lets consumers filter without rebuilding the enum.
    pub edge_type_name: String,
    /// Computed weight in `[0.0, 1.0]`.
    pub weight: f32,
    /// Direction encoding: `0=Symmetric, 1=Forward, 2=Backward` matching
    /// [`crate::graph_linking::DirectedRelation`].
    pub direction: u8,
    /// Per-embedder similarity profile in SRC-3 `[0, 1]` convention.
    pub embedder_scores: [f32; NUM_EMBEDDERS],
    /// Number of embedders that agreed (above threshold) on this edge.
    pub agreement_count: u8,
    /// Bitset of agreeing embedders (bit 0 = E1, bit 12 = E13).
    pub agreeing_embedders: u16,

    // Content join — optional; all empty `String` / `None` when caller asks to skip.
    /// Source memory's content text, or empty when the caller suppressed
    /// content joining.
    pub source_content: String,
    /// Target memory's content text, or empty when suppressed.
    pub target_content: String,
    /// Source memory's session id, joined from `SourceMetadata`.
    pub source_session_id: Option<String>,
    /// Target memory's session id, joined from `SourceMetadata`.
    pub target_session_id: Option<String>,
    /// Source memory's `SourceMetadata::source_type`.
    pub source_type: Option<String>,
    /// Target memory's `SourceMetadata::source_type`.
    pub target_type: Option<String>,

    /// Mechanism sub-type for `CausalChain` edges, joined from
    /// `CF_CAUSAL_RELATIONSHIPS` when resolvable. `None` for non-causal edges
    /// or when no causal record exists.
    pub mechanism_type: Option<String>,

    /// LLM validation join — `None` when no validation row exists in
    /// `CF_TYPED_EDGE_VALIDATIONS` for this edge.
    pub llm_validation: Option<LLMValidationSummary>,

    /// Wall-clock time when this record was emitted by the exporter.
    pub exported_at: DateTime<Utc>,
    /// Free-form generator tag (e.g. `"typed_edge_export_v1"`).
    pub exporter_version: String,
}

/// Compact view of the LLM verdict for embedding inside a
/// [`TypedEdgeTrainingRecord`]. Mirrors the relevant fields of the full
/// `LLMEdgeValidation` row (defined in
/// `crate::llm_edge_validation::LLMEdgeValidation`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LLMValidationSummary {
    /// Wall-clock time of the LLM validation call.
    pub validated_at: DateTime<Utc>,
    /// Final verdict emitted by the LLM.
    pub verdict: LLMVerdict,
    /// LLM confidence in `[0.0, 1.0]`.
    pub confidence: f32,
    /// 1-3 sentence rationale from the grammar-constrained LLM output.
    pub rationale: String,
    /// Validator tag or external validator version.
    pub validator_version: String,
}

/// LLM verdict on a single typed edge.
///
/// Serialized via bincode (variant index + payload for `Reclassify`). Any
/// addition or reordering of variants requires bumping
/// [`crate::llm_edge_validation::LLM_EDGE_VALIDATION_VERSION`] and
/// [`TYPED_EDGE_RECORD_VERSION`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LLMVerdict {
    /// LLM confirmed the auto-derived edge.
    Valid,
    /// LLM rejected the auto-derived edge.
    Invalid,
    /// LLM accepted that the two memories are linked but classified the
    /// relationship under a different `GraphLinkEdgeType`.
    Reclassify {
        /// New edge type as `GraphLinkEdgeType::as_u8()` (0..=7).
        new_edge_type: u8,
    },
}

impl LLMVerdict {
    /// Snake-case label suitable for logging / JSON output.
    pub fn as_str(&self) -> &'static str {
        match self {
            LLMVerdict::Valid => "valid",
            LLMVerdict::Invalid => "invalid",
            LLMVerdict::Reclassify { .. } => "reclassify",
        }
    }
}

#[cfg(test)]
mod tests;
