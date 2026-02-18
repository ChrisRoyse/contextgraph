//! SemanticFingerprint struct and implementation.
//!
//! # Architecture Reference
//!
//! From constitution.yaml (ARCH-01): "TeleologicalArray is the Atomic Storage Unit"
//! From constitution.yaml (ARCH-05): "All 13 Embedders Must Be Present"
//!
//! This module implements the canonical 13-embedding array storage structure.
//! The SemanticFingerprint IS the TeleologicalArray - they are the same type.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::constants::{
    E10_DIM, E11_DIM, E12_TOKEN_DIM, E13_SPLADE_VOCAB, E1_DIM, E2_DIM, E3_DIM, E4_DIM, E5_DIM,
    E6_SPARSE_VOCAB, E7_DIM, E8_DIM, E9_DIM,
};
use super::slice::EmbeddingSlice;
use crate::teleological::Embedder;
use crate::types::fingerprint::SparseVector;

/// Type alias for specification alignment.
///
/// Per TASK-CORE-003 decision: SemanticFingerprint IS the TeleologicalArray.
/// This alias provides documentation alignment with constitution.yaml.
pub type TeleologicalArray = SemanticFingerprint;

/// Reference to an embedding that may be dense, sparse, or token-level.
///
/// This enum provides type-safe access to embeddings via the Embedder enum,
/// preserving the different representation types without data copying.
///
/// # Design
///
/// Different embedders produce different output formats:
/// - Dense: Fixed-length f32 vectors (E1, E2-E5, E7-E11)
/// - Sparse: Index-value pairs for SPLADE vocabularies (E6, E13)
/// - TokenLevel: Variable-length sequences of per-token embeddings (E12)
#[derive(Debug)]
pub enum EmbeddingRef<'a> {
    /// Dense embedding as a contiguous f32 slice (E1, E2-E5, E7-E11).
    Dense(&'a [f32]),
    /// Sparse embedding reference (E6 SPLADE, E13 KeywordSPLADE).
    Sparse(&'a SparseVector),
    /// Token-level embedding (E12 ColBERT) - variable number of 128D tokens.
    TokenLevel(&'a [Vec<f32>]),
}

/// Errors from SemanticFingerprint validation.
///
/// All validation errors contain detailed context for debugging,
/// following constitution.yaml AP-14: "No .unwrap() in library code".
#[derive(Debug, Clone, Error)]
pub enum ValidationError {
    /// A dense embedding has incorrect dimensions.
    #[error("Dimension mismatch for {embedder}: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// The embedder that failed validation
        embedder: Embedder,
        /// Expected dimension size
        expected: usize,
        /// Actual dimension size
        actual: usize,
    },

    /// An embedding vector is empty when it should have content.
    /// Note: E12 (token-level) with 0 tokens is valid (empty content).
    /// Note: Sparse vectors with 0 active entries are valid.
    #[error("Empty dense embedding for {embedder}: expected {expected} dimensions")]
    EmptyDenseEmbedding {
        /// The embedder that has an empty embedding
        embedder: Embedder,
        /// Expected dimension size
        expected: usize,
    },

    /// A token in the late-interaction embedding has wrong dimensions.
    #[error(
        "Token {token_index} dimension mismatch for {embedder}: expected {expected}, got {actual}"
    )]
    TokenDimensionMismatch {
        /// The embedder (always LateInteraction)
        embedder: Embedder,
        /// Index of the invalid token
        token_index: usize,
        /// Expected per-token dimension (128)
        expected: usize,
        /// Actual token dimension
        actual: usize,
    },

    /// A sparse index exceeds the vocabulary size for the embedder.
    #[error("Sparse index {index} exceeds vocabulary size {vocab_size} for {embedder}")]
    SparseIndexOutOfBounds {
        /// The embedder with the out-of-bounds index
        embedder: Embedder,
        /// The invalid index value
        index: u32,
        /// The maximum valid vocabulary size
        vocab_size: usize,
    },

    /// Sparse indices and values vectors have different lengths.
    #[error("Sparse indices ({indices_len}) and values ({values_len}) length mismatch for {embedder}")]
    SparseIndicesValuesMismatch {
        /// The embedder with mismatched sparse data
        embedder: Embedder,
        /// Length of indices vector
        indices_len: usize,
        /// Length of values vector
        values_len: usize,
    },
}

/// SemanticFingerprint: Stores all 13 embeddings without fusion.
///
/// # Philosophy
///
/// **NO FUSION.** Each embedding space preserved independently for:
/// - Per-space HNSW search
/// - Per-space teleological alignment
/// - 100% information preservation
///
/// # Storage
///
/// Typical storage: ~46KB (vs 6KB fused = 67% info loss)
///
/// # Design Note
///
/// Uses `Vec<f32>` instead of fixed-size arrays to:
/// 1. Enable serde serialization for embeddings (E9 has 1024 projected dims)
/// 2. Avoid stack overflow with large arrays
/// 3. Maintain flexibility for future dimension changes
///
/// Dimension validation is performed via `validate()` and construction methods.
///
/// # IMPORTANT: No Default Implementation
///
/// This struct intentionally does NOT implement `Default` to prevent accidental
/// creation of all-zero fingerprints that pass validation but cause silent failures
/// in search and alignment operations. Use [`Self::zeroed()`] explicitly when you
/// need a placeholder fingerprint (e.g., in tests), but be aware that zeroed data
/// should never be used in production workflows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFingerprint {
    /// E1: Semantic (e5-large-v2) - 1024D dense embedding.
    pub e1_semantic: Vec<f32>,

    /// E2: Temporal-Recent (exponential decay) - 512D dense embedding.
    pub e2_temporal_recent: Vec<f32>,

    /// E3: Temporal-Periodic (Fourier) - 512D dense embedding.
    pub e3_temporal_periodic: Vec<f32>,

    /// E4: Temporal-Positional (sinusoidal PE) - 512D dense embedding.
    pub e4_temporal_positional: Vec<f32>,

    /// E5: Causal (Longformer SCM) - 768D dense embedding (as cause).
    ///
    /// This vector encodes the text as a potential cause in causal relationships.
    /// For asymmetric similarity: query_as_cause is compared against doc_as_effect.
    pub e5_causal_as_cause: Vec<f32>,

    /// E5: Causal (Longformer SCM) - 768D dense embedding (as effect).
    ///
    /// This vector encodes the text as a potential effect in causal relationships.
    /// For asymmetric similarity: query_as_effect is compared against doc_as_cause.
    pub e5_causal_as_effect: Vec<f32>,

    /// E5: Causal - Legacy single vector (deprecated, kept for backward compatibility).
    ///
    /// New code should use `e5_causal_as_cause` and `e5_causal_as_effect` instead.
    /// This field is populated during deserialization of old fingerprints.
    #[serde(default)]
    pub e5_causal: Vec<f32>,

    /// E6: Sparse Lexical (SPLADE) - sparse vector with ~1500 active of 30522 vocab.
    pub e6_sparse: SparseVector,

    /// E7: Code (Qodo-Embed-1-1.5B) - 1536D dense embedding.
    pub e7_code: Vec<f32>,

    /// E8: Graph (e5-large-v2) - 1024D dense embedding (as source).
    ///
    /// This vector encodes the text as a potential source in graph relationships.
    /// For asymmetric similarity: query_as_source is compared against doc_as_target.
    /// Example: "Module A imports B" → A is the source.
    pub e8_graph_as_source: Vec<f32>,

    /// E8: Graph (e5-large-v2) - 1024D dense embedding (as target).
    ///
    /// This vector encodes the text as a potential target in graph relationships.
    /// For asymmetric similarity: query_as_target is compared against doc_as_source.
    /// Example: "Module X is imported by A, B" → X is the target.
    pub e8_graph_as_target: Vec<f32>,

    /// E8: Graph - Legacy single vector (deprecated, kept for backward compatibility).
    ///
    /// New code should use `e8_graph_as_source` and `e8_graph_as_target` instead.
    /// This field is populated during deserialization of old fingerprints.
    #[serde(default)]
    pub e8_graph: Vec<f32>,

    /// E9: HDC (projected from 10K-bit hyperdimensional) - 1024D dense embedding.
    pub e9_hdc: Vec<f32>,

    /// E10: Multimodal (CLIP/MPNet) - 768D dense embedding (paraphrase side).
    ///
    /// This vector encodes the text for paraphrase detection in asymmetric retrieval.
    /// For asymmetric similarity: query paraphrase vector is compared against doc context vector.
    /// Example: "User wants to fix the bug" → captures the expressed meaning.
    pub e10_multimodal_paraphrase: Vec<f32>,

    /// E10: Multimodal (CLIP/MPNet) - 768D dense embedding (as context).
    ///
    /// This vector encodes the text as providing context in relationships.
    /// For asymmetric similarity: query_as_context is compared against doc_as_paraphrase.
    /// Example: "The database connection failed" → captures background context.
    pub e10_multimodal_as_context: Vec<f32>,

    /// E11: Entity (KEPLER for knowledge graph) - 768D dense embedding.
    pub e11_entity: Vec<f32>,

    /// E12: Late-Interaction (ColBERT) - 128D per token, variable token count.
    pub e12_late_interaction: Vec<Vec<f32>>,

    /// E13: SPLADE v3 sparse embedding for lexical-semantic hybrid search.
    pub e13_splade: SparseVector,
}

impl SemanticFingerprint {
    /// Create a zeroed fingerprint (all embeddings initialized to 0.0).
    ///
    /// # ⚠️ TEST ONLY - AP-007 ENFORCED
    ///
    /// This method is **only available in test builds** because:
    /// - Zero vectors have undefined cosine similarity (0/0 = NaN)
    /// - HNSW search returns unpredictable results with zero-magnitude vectors
    /// - Production code MUST use real embeddings from the GPU pipeline
    ///
    /// If you need this in production, you're doing something wrong.
    /// Use the embedding pipeline to compute real vectors instead.
    #[cfg(any(test, feature = "test-utils"))]
    #[must_use = "zeroed fingerprints should be used explicitly and with caution"]
    pub fn zeroed() -> Self {
        Self {
            e1_semantic: vec![0.0; E1_DIM],
            e2_temporal_recent: vec![0.0; E2_DIM],
            e3_temporal_periodic: vec![0.0; E3_DIM],
            e4_temporal_positional: vec![0.0; E4_DIM],
            e5_causal_as_cause: vec![0.0; E5_DIM],
            e5_causal_as_effect: vec![0.0; E5_DIM],
            e5_causal: Vec::new(), // Legacy field, empty by default
            e6_sparse: SparseVector::empty(),
            e7_code: vec![0.0; E7_DIM],
            e8_graph_as_source: vec![0.0; E8_DIM],
            e8_graph_as_target: vec![0.0; E8_DIM],
            e8_graph: Vec::new(), // Legacy field, empty by default
            e9_hdc: vec![0.0; E9_DIM],
            e10_multimodal_paraphrase: vec![0.0; E10_DIM],
            e10_multimodal_as_context: vec![0.0; E10_DIM],
            e11_entity: vec![0.0; E11_DIM],
            e12_late_interaction: Vec::new(),
            e13_splade: SparseVector::empty(),
        }
    }

    /// Get embedding by index (0-12).
    ///
    /// For E5 (index 4), returns the `e5_causal_as_cause` vector by default.
    /// Use `get_e5_as_effect()` for asymmetric similarity comparisons.
    ///
    /// For E8 (index 7), returns the `e8_graph_as_source` vector by default.
    /// Use `get_e8_as_target()` for asymmetric similarity comparisons.
    pub fn get_embedding(&self, idx: usize) -> Option<EmbeddingSlice<'_>> {
        match idx {
            0 => Some(EmbeddingSlice::Dense(&self.e1_semantic)),
            1 => Some(EmbeddingSlice::Dense(&self.e2_temporal_recent)),
            2 => Some(EmbeddingSlice::Dense(&self.e3_temporal_periodic)),
            3 => Some(EmbeddingSlice::Dense(&self.e4_temporal_positional)),
            4 => Some(EmbeddingSlice::Dense(self.e5_active_vector())),
            5 => Some(EmbeddingSlice::Sparse(&self.e6_sparse)),
            6 => Some(EmbeddingSlice::Dense(&self.e7_code)),
            7 => Some(EmbeddingSlice::Dense(self.e8_active_vector())),
            8 => Some(EmbeddingSlice::Dense(&self.e9_hdc)),
            9 => Some(EmbeddingSlice::Dense(self.e10_active_vector())),
            10 => Some(EmbeddingSlice::Dense(&self.e11_entity)),
            11 => Some(EmbeddingSlice::TokenLevel(&self.e12_late_interaction)),
            12 => Some(EmbeddingSlice::Sparse(&self.e13_splade)),
            _ => None,
        }
    }

    /// Get the active E5 causal vector for standard operations.
    ///
    /// Returns `e5_causal_as_cause` if populated, otherwise falls back to
    /// the legacy `e5_causal` field for backward compatibility.
    ///
    /// This is the default E5 vector used for symmetric similarity comparisons
    /// (when causal direction is unknown or not relevant).
    #[inline]
    pub fn e5_active_vector(&self) -> &[f32] {
        if !self.e5_causal_as_cause.is_empty() {
            &self.e5_causal_as_cause
        } else {
            &self.e5_causal
        }
    }

    /// Get E5 causal embedding encoded as a potential cause.
    ///
    /// Use this for asymmetric similarity when the query represents a cause.
    #[inline]
    pub fn get_e5_as_cause(&self) -> &[f32] {
        if !self.e5_causal_as_cause.is_empty() {
            &self.e5_causal_as_cause
        } else {
            &self.e5_causal
        }
    }

    /// Get E5 causal embedding encoded as a potential effect.
    ///
    /// Use this for asymmetric similarity when the query represents an effect.
    #[inline]
    pub fn get_e5_as_effect(&self) -> &[f32] {
        if !self.e5_causal_as_effect.is_empty() {
            &self.e5_causal_as_effect
        } else {
            &self.e5_causal
        }
    }

    /// Check if this fingerprint has asymmetric E5 causal vectors.
    ///
    /// Returns `true` if both `e5_causal_as_cause` and `e5_causal_as_effect`
    /// are populated, `false` if only the legacy `e5_causal` field is used.
    #[inline]
    pub fn has_asymmetric_e5(&self) -> bool {
        !self.e5_causal_as_cause.is_empty() && !self.e5_causal_as_effect.is_empty()
    }

    // =========================================================================
    // E8 GRAPH DUAL VECTOR METHODS (Following E5 Causal Pattern)
    // =========================================================================

    /// Get the active E8 graph vector for standard operations.
    ///
    /// Returns `e8_graph_as_source` if populated, otherwise falls back to
    /// the legacy `e8_graph` field for backward compatibility.
    ///
    /// This is the default E8 vector used for symmetric similarity comparisons
    /// (when graph direction is unknown or not relevant).
    #[inline]
    pub fn e8_active_vector(&self) -> &[f32] {
        if !self.e8_graph_as_source.is_empty() {
            &self.e8_graph_as_source
        } else {
            &self.e8_graph
        }
    }

    /// Get E8 graph embedding encoded as a potential source.
    ///
    /// Use this for asymmetric similarity when the query represents a source.
    /// (e.g., "What does module X import?")
    #[inline]
    pub fn get_e8_as_source(&self) -> &[f32] {
        if !self.e8_graph_as_source.is_empty() {
            &self.e8_graph_as_source
        } else {
            &self.e8_graph
        }
    }

    /// Get E8 graph embedding encoded as a potential target.
    ///
    /// Use this for asymmetric similarity when the query represents a target.
    /// (e.g., "What imports module X?")
    #[inline]
    pub fn get_e8_as_target(&self) -> &[f32] {
        if !self.e8_graph_as_target.is_empty() {
            &self.e8_graph_as_target
        } else {
            &self.e8_graph
        }
    }

    /// Check if this fingerprint has asymmetric E8 graph vectors.
    ///
    /// Returns `true` if both `e8_graph_as_source` and `e8_graph_as_target`
    /// are populated, `false` if only the legacy `e8_graph` field is used.
    #[inline]
    pub fn has_asymmetric_e8(&self) -> bool {
        !self.e8_graph_as_source.is_empty() && !self.e8_graph_as_target.is_empty()
    }

    /// Migrate a legacy fingerprint to the new dual E8 format.
    ///
    /// If the fingerprint has only the legacy `e8_graph` field populated,
    /// this method copies it to both `e8_graph_as_source` and `e8_graph_as_target`.
    ///
    /// This is useful when loading old fingerprints that need to work with
    /// asymmetric similarity computation.
    pub fn migrate_legacy_e8(&mut self) {
        if !self.e8_graph.is_empty()
            && self.e8_graph_as_source.is_empty()
            && self.e8_graph_as_target.is_empty()
        {
            // Copy legacy E8 to both source and target vectors
            self.e8_graph_as_source = self.e8_graph.clone();
            self.e8_graph_as_target = self.e8_graph.clone();
        }
    }

    /// Check if this fingerprint uses the legacy single E8 format.
    ///
    /// Returns `true` if the fingerprint has only `e8_graph` populated
    /// and both `e8_graph_as_source` and `e8_graph_as_target` are empty.
    #[inline]
    pub fn is_legacy_e8_format(&self) -> bool {
        !self.e8_graph.is_empty()
            && self.e8_graph_as_source.is_empty()
            && self.e8_graph_as_target.is_empty()
    }

    // =========================================================================
    // E10 MULTIMODAL DUAL VECTOR METHODS (Following E5/E8 Pattern)
    // =========================================================================

    /// Get the active E10 multimodal vector for standard operations.
    ///
    /// Returns the `e10_multimodal_paraphrase` vector.
    ///
    /// This is the default E10 vector used for symmetric similarity comparisons
    /// (when paraphrase/context direction is unknown or not relevant).
    #[inline]
    pub fn e10_active_vector(&self) -> &[f32] {
        &self.e10_multimodal_paraphrase
    }

    /// Get E10 multimodal embedding (paraphrase side).
    ///
    /// Use this for asymmetric similarity when the query represents expressed meaning.
    /// (e.g., "User wants to accomplish X")
    #[inline]
    pub fn get_e10_as_paraphrase(&self) -> &[f32] {
        &self.e10_multimodal_paraphrase
    }

    /// Get E10 multimodal embedding encoded as providing context.
    ///
    /// Use this for asymmetric similarity when the query represents context.
    /// (e.g., "The background situation is Y")
    #[inline]
    pub fn get_e10_as_context(&self) -> &[f32] {
        &self.e10_multimodal_as_context
    }

    /// Check if this fingerprint has asymmetric E10 multimodal vectors.
    ///
    /// Returns `true` if both `e10_multimodal_paraphrase` and `e10_multimodal_as_context`
    /// are populated.
    #[inline]
    pub fn has_asymmetric_e10(&self) -> bool {
        !self.e10_multimodal_paraphrase.is_empty() && !self.e10_multimodal_as_context.is_empty()
    }

    /// Compute total storage size in bytes (heap allocations only).
    pub fn storage_size(&self) -> usize {
        let dense_size = (self.e1_semantic.len()
            + self.e2_temporal_recent.len()
            + self.e3_temporal_periodic.len()
            + self.e4_temporal_positional.len()
            + self.e5_causal_as_cause.len()
            + self.e5_causal_as_effect.len()
            + self.e5_causal.len() // Legacy field
            + self.e7_code.len()
            + self.e8_graph_as_source.len()
            + self.e8_graph_as_target.len()
            + self.e8_graph.len() // Legacy field
            + self.e9_hdc.len()
            + self.e10_multimodal_paraphrase.len()
            + self.e10_multimodal_as_context.len()
            + self.e11_entity.len())
            * std::mem::size_of::<f32>();

        let e6_sparse_size = self.e6_sparse.memory_size();
        let token_size: usize = self
            .e12_late_interaction
            .iter()
            .map(|t| t.len() * std::mem::size_of::<f32>())
            .sum();
        let e13_sparse_size = self.e13_splade.memory_size();

        dense_size + e6_sparse_size + token_size + e13_sparse_size
    }

    /// Get the number of tokens in E12 late-interaction embedding.
    #[inline]
    pub fn token_count(&self) -> usize {
        self.e12_late_interaction.len()
    }

    /// Get the number of non-zero entries in E13 SPLADE embedding.
    #[inline]
    pub fn e13_splade_nnz(&self) -> usize {
        self.e13_splade.nnz()
    }

    /// Get embedding name by index.
    pub fn embedding_name(idx: usize) -> Option<&'static str> {
        match idx {
            0 => Some("E1_Semantic"),
            1 => Some("E2_Temporal_Recent"),
            2 => Some("E3_Temporal_Periodic"),
            3 => Some("E4_Temporal_Positional"),
            4 => Some("E5_Causal"),
            5 => Some("E6_Sparse_Lexical"),
            6 => Some("E7_Code"),
            7 => Some("E8_Graph"),
            8 => Some("E9_HDC"),
            9 => Some("E10_Multimodal"),
            10 => Some("E11_Entity"),
            11 => Some("E12_Late_Interaction"),
            12 => Some("E13_SPLADE"),
            _ => None,
        }
    }

    /// Get embedding dimension by index.
    pub fn embedding_dim(idx: usize) -> Option<usize> {
        match idx {
            0 => Some(E1_DIM),
            1 => Some(E2_DIM),
            2 => Some(E3_DIM),
            3 => Some(E4_DIM),
            4 => Some(E5_DIM),
            5 => Some(E6_SPARSE_VOCAB),
            6 => Some(E7_DIM),
            7 => Some(E8_DIM),
            8 => Some(E9_DIM),
            9 => Some(E10_DIM),
            10 => Some(E11_DIM),
            11 => Some(E12_TOKEN_DIM),
            12 => Some(E13_SPLADE_VOCAB),
            _ => None,
        }
    }

    /// Get an embedding reference by Embedder enum (type-safe access).
    ///
    /// This method provides type-safe access to embeddings using the canonical
    /// Embedder enum from teleological::embedder. It never fails since all 13
    /// embedders are always present.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::types::fingerprint::{SemanticFingerprint, EmbeddingRef};
    /// use context_graph_core::teleological::Embedder;
    ///
    /// # #[cfg(feature = "test-utils")]
    /// # {
    /// let fp = SemanticFingerprint::zeroed();
    /// match fp.get(Embedder::Semantic) {
    ///     EmbeddingRef::Dense(data) => assert_eq!(data.len(), 1024),
    ///     _ => panic!("E1 should be dense"),
    /// }
    /// # }
    /// ```
    pub fn get(&self, embedder: Embedder) -> EmbeddingRef<'_> {
        match embedder {
            Embedder::Semantic => EmbeddingRef::Dense(&self.e1_semantic),
            Embedder::TemporalRecent => EmbeddingRef::Dense(&self.e2_temporal_recent),
            Embedder::TemporalPeriodic => EmbeddingRef::Dense(&self.e3_temporal_periodic),
            Embedder::TemporalPositional => EmbeddingRef::Dense(&self.e4_temporal_positional),
            Embedder::Causal => EmbeddingRef::Dense(self.e5_active_vector()),
            Embedder::Sparse => EmbeddingRef::Sparse(&self.e6_sparse),
            Embedder::Code => EmbeddingRef::Dense(&self.e7_code),
            Embedder::Graph => EmbeddingRef::Dense(self.e8_active_vector()),
            Embedder::Hdc => EmbeddingRef::Dense(&self.e9_hdc),
            Embedder::Multimodal => EmbeddingRef::Dense(self.e10_active_vector()),
            Embedder::Entity => EmbeddingRef::Dense(&self.e11_entity),
            Embedder::LateInteraction => EmbeddingRef::TokenLevel(&self.e12_late_interaction),
            Embedder::KeywordSplade => EmbeddingRef::Sparse(&self.e13_splade),
        }
    }

    /// Check if all dense embeddings have valid (non-zero) dimensions.
    ///
    /// Returns `true` if all 10 dense embeddings have their expected dimensions.
    /// Sparse embeddings (E6, E13) are always considered complete (can be empty).
    /// Token-level embedding (E12) is always considered complete (can have 0 tokens).
    ///
    /// For E5 Causal, accepts either:
    /// - Both e5_causal_as_cause and e5_causal_as_effect populated (new format)
    /// - Only e5_causal populated (legacy format)
    ///
    /// For E8 Graph, accepts either:
    /// - Both e8_graph_as_source and e8_graph_as_target populated (new format)
    /// - Only e8_graph populated (legacy format)
    ///
    /// For E10 Multimodal:
    /// - Both e10_multimodal_paraphrase and e10_multimodal_as_context must be populated
    ///
    /// # Returns
    ///
    /// `true` if all dense embeddings have correct dimensions, `false` otherwise.
    pub fn is_complete(&self) -> bool {
        // E5 check: either new dual vectors OR legacy single vector
        let e5_complete = (self.e5_causal_as_cause.len() == E5_DIM
            && self.e5_causal_as_effect.len() == E5_DIM)
            || self.e5_causal.len() == E5_DIM;

        // E8 check: either new dual vectors OR legacy single vector
        let e8_complete = (self.e8_graph_as_source.len() == E8_DIM
            && self.e8_graph_as_target.len() == E8_DIM)
            || self.e8_graph.len() == E8_DIM;

        // E10 check: both paraphrase and context vectors must be populated
        let e10_complete = self.e10_multimodal_paraphrase.len() == E10_DIM
            && self.e10_multimodal_as_context.len() == E10_DIM;

        self.e1_semantic.len() == E1_DIM
            && self.e2_temporal_recent.len() == E2_DIM
            && self.e3_temporal_periodic.len() == E3_DIM
            && self.e4_temporal_positional.len() == E4_DIM
            && e5_complete
            && self.e7_code.len() == E7_DIM
            && e8_complete
            && self.e9_hdc.len() == E9_DIM
            && e10_complete
            && self.e11_entity.len() == E11_DIM
    }

    /// Compute total storage size in bytes.
    ///
    /// This is an alias for `storage_size()` for specification alignment.
    /// Returns the total heap allocation for all embeddings.
    #[inline]
    pub fn storage_bytes(&self) -> usize {
        self.storage_size()
    }

    /// Validate all embeddings with detailed error reporting (alias for validate).
    ///
    /// This is an alias for [`Self::validate()`] for API compatibility.
    /// Both methods perform identical validation with fail-fast semantics.
    #[inline]
    pub fn validate_strict(&self) -> Result<(), ValidationError> {
        self.validate()
    }

    /// Convert fingerprint to dense array for cluster_manager insertion.
    ///
    /// TASK-FIX-CLUSTERING: Required for topic detection via MultiSpaceClusterManager.
    ///
    /// This method converts all 13 embeddings to dense Vec<f32> format:
    /// - Dense embeddings (E1-E5, E7-E11): Cloned directly
    /// - Sparse embeddings (E6, E13): Converted via SparseVector::to_dense()
    /// - Token-level (E12): Mean-pooled across tokens to single 128D vector
    ///
    /// # Performance
    ///
    /// This is an expensive operation due to sparse->dense expansion.
    /// E6 and E13 expand from ~1500 active values to 30522 dimensions each (~240KB).
    /// Use only for clustering operations, not for routine storage/retrieval.
    ///
    /// # Returns
    ///
    /// Array of 13 dense vectors with expected dimensions:
    /// - E1: 1024D, E2-E4: 512D each, E5: 768D
    /// - E6: 30522D (sparse converted), E7: 1536D
    /// - E8: 1024D, E9: 1024D, E10: 768D, E11: 768D
    /// - E12: 128D (mean-pooled), E13: 30522D (sparse converted)
    pub fn to_cluster_array(&self) -> [Vec<f32>; 13] {
        // E12: Mean-pool tokens to single 128D vector
        // If no tokens, return zero vector
        let e12_pooled = if self.e12_late_interaction.is_empty() {
            vec![0.0; E12_TOKEN_DIM]
        } else {
            let num_tokens = self.e12_late_interaction.len() as f32;
            let mut pooled = vec![0.0; E12_TOKEN_DIM];
            for token in &self.e12_late_interaction {
                for (i, &val) in token.iter().enumerate() {
                    if i < E12_TOKEN_DIM {
                        pooled[i] += val;
                    }
                }
            }
            // Mean by dividing by token count
            for val in &mut pooled {
                *val /= num_tokens;
            }
            pooled
        };

        [
            self.e1_semantic.clone(),
            self.e2_temporal_recent.clone(),
            self.e3_temporal_periodic.clone(),
            self.e4_temporal_positional.clone(),
            self.e5_active_vector().to_vec(), // Use active E5 vector
            self.e6_sparse.to_dense(E6_SPARSE_VOCAB),
            self.e7_code.clone(),
            self.e8_active_vector().to_vec(), // Use active E8 vector
            self.e9_hdc.clone(),
            self.e10_active_vector().to_vec(), // Use active E10 vector
            self.e11_entity.clone(),
            e12_pooled,
            self.e13_splade.to_dense(E13_SPLADE_VOCAB),
        ]
    }

    /// Migrate a legacy fingerprint to the new dual E5 format.
    ///
    /// If the fingerprint has only the legacy `e5_causal` field populated,
    /// this method copies it to both `e5_causal_as_cause` and `e5_causal_as_effect`.
    ///
    /// This is useful when loading old fingerprints that need to work with
    /// asymmetric similarity computation.
    pub fn migrate_legacy_e5(&mut self) {
        if !self.e5_causal.is_empty()
            && self.e5_causal_as_cause.is_empty()
            && self.e5_causal_as_effect.is_empty()
        {
            // Copy legacy E5 to both cause and effect vectors
            self.e5_causal_as_cause = self.e5_causal.clone();
            self.e5_causal_as_effect = self.e5_causal.clone();
        }
    }

    /// Check if this fingerprint uses the legacy single E5 format.
    ///
    /// Returns `true` if the fingerprint has only `e5_causal` populated
    /// and both `e5_causal_as_cause` and `e5_causal_as_effect` are empty.
    #[inline]
    pub fn is_legacy_e5_format(&self) -> bool {
        !self.e5_causal.is_empty()
            && self.e5_causal_as_cause.is_empty()
            && self.e5_causal_as_effect.is_empty()
    }
}

// NOTE: Default is intentionally NOT implemented for SemanticFingerprint.
// All-zero fingerprints pass validation but cause silent failures in search/alignment.
// Use SemanticFingerprint::zeroed() explicitly when placeholder data is needed.

impl PartialEq for SemanticFingerprint {
    fn eq(&self, other: &Self) -> bool {
        self.e1_semantic == other.e1_semantic
            && self.e2_temporal_recent == other.e2_temporal_recent
            && self.e3_temporal_periodic == other.e3_temporal_periodic
            && self.e4_temporal_positional == other.e4_temporal_positional
            && self.e5_causal_as_cause == other.e5_causal_as_cause
            && self.e5_causal_as_effect == other.e5_causal_as_effect
            && self.e5_causal == other.e5_causal
            && self.e6_sparse == other.e6_sparse
            && self.e7_code == other.e7_code
            && self.e8_graph_as_source == other.e8_graph_as_source
            && self.e8_graph_as_target == other.e8_graph_as_target
            && self.e8_graph == other.e8_graph
            && self.e9_hdc == other.e9_hdc
            && self.e10_multimodal_paraphrase == other.e10_multimodal_paraphrase
            && self.e10_multimodal_as_context == other.e10_multimodal_as_context
            && self.e11_entity == other.e11_entity
            && self.e12_late_interaction == other.e12_late_interaction
            && self.e13_splade == other.e13_splade
    }
}
