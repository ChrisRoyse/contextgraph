//! Persisted LLM verdict on one typed edge (Feature 4 of the typed-edges
//! training-data factory).
//!
//! Each [`LLMEdgeValidation`] row carries the LLM's verdict, confidence,
//! rationale, the edge weight at validation time, the validator model tag,
//! and a SHA-256 hash of the prompt actually sent to the LLM (so a future
//! change in prompt template is detectable).
//!
//! Rows are persisted to the new `CF_TYPED_EDGE_VALIDATIONS` column family
//! keyed by the same 33-byte composite key shape used for
//! `CF_TYPED_EDGE_RECORDS` (`[source: 16][target: 16][edge_type: u8]`).
//! Wire format: `[version_byte = LLM_EDGE_VALIDATION_VERSION][bincode-encoded
//! LLMEdgeValidation]`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::typed_edge_export::LLMVerdict;

/// Current on-disk version byte for [`LLMEdgeValidation`] payloads in
/// `CF_TYPED_EDGE_VALIDATIONS`.
///
/// Bumped on breaking layout changes; deserialization rejects mismatches with
/// `CoreError::SerializationError`. No automatic migration is supported.
pub const LLM_EDGE_VALIDATION_VERSION: u8 = 1;

/// An LLM-issued verdict on a single typed edge, persisted to
/// `CF_TYPED_EDGE_VALIDATIONS` keyed by `[source: 16][target: 16][edge_type:
/// u8]`.
///
/// Bincode positional layout: any new fields must be appended at the end and
/// require a [`LLM_EDGE_VALIDATION_VERSION`] bump if they break decode of
/// existing rows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LLMEdgeValidation {
    /// Wall-clock time of the LLM validation call.
    pub validated_at: DateTime<Utc>,
    /// LLM verdict (Valid / Invalid / Reclassify).
    pub verdict: LLMVerdict,
    /// LLM confidence in `[0.0, 1.0]`.
    pub confidence: f32,
    /// 1-3 sentence justification from the grammar-constrained LLM output.
    pub rationale: String,
    /// Edge weight at the moment of validation (so drift from the auto-derived
    /// embedder ensemble can be audited later).
    pub auto_derived_weight: f32,
    /// Validator model tag or external validator version.
    pub validator_version: String,
    /// SHA-256 of the prompt actually sent to the LLM. Reproducibility hook —
    /// any change in the prompt template is detectable as a drift in this
    /// field.
    pub prompt_hash: [u8; 32],
}

#[cfg(test)]
mod tests;
