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
use crate::types::fingerprint::{SparseVector, SparseVectorError};

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

    /// A sparse vector failed validation.
    #[error("Sparse vector validation failed for {embedder}: {source}")]
    SparseVectorError {
        /// The embedder with invalid sparse data
        embedder: Embedder,
        /// The underlying sparse vector error
        #[source]
        source: SparseVectorError,
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
}

/// SemanticFingerprint: Stores all 13 embeddings without fusion.
///
/// # Philosophy
///
/// **NO FUSION.** Each embedding space preserved independently for:
/// - Per-space HNSW search
/// - Per-space Johari classification
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

    /// E5: Causal (Longformer SCM) - 768D dense embedding.
    pub e5_causal: Vec<f32>,

    /// E6: Sparse Lexical (SPLADE) - sparse vector with ~1500 active of 30522 vocab.
    pub e6_sparse: SparseVector,

    /// E7: Code (Qodo-Embed-1-1.5B) - 1536D dense embedding.
    pub e7_code: Vec<f32>,

    /// E8: Graph (MiniLM for structure) - 384D dense embedding.
    pub e8_graph: Vec<f32>,

    /// E9: HDC (projected from 10K-bit hyperdimensional) - 1024D dense embedding.
    pub e9_hdc: Vec<f32>,

    /// E10: Multimodal (CLIP) - 768D dense embedding.
    pub e10_multimodal: Vec<f32>,

    /// E11: Entity (MiniLM for facts) - 384D dense embedding.
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
            e5_causal: vec![0.0; E5_DIM],
            e6_sparse: SparseVector::empty(),
            e7_code: vec![0.0; E7_DIM],
            e8_graph: vec![0.0; E8_DIM],
            e9_hdc: vec![0.0; E9_DIM],
            e10_multimodal: vec![0.0; E10_DIM],
            e11_entity: vec![0.0; E11_DIM],
            e12_late_interaction: Vec::new(),
            e13_splade: SparseVector::empty(),
        }
    }

    /// Get embedding by index (0-12).
    pub fn get_embedding(&self, idx: usize) -> Option<EmbeddingSlice<'_>> {
        match idx {
            0 => Some(EmbeddingSlice::Dense(&self.e1_semantic)),
            1 => Some(EmbeddingSlice::Dense(&self.e2_temporal_recent)),
            2 => Some(EmbeddingSlice::Dense(&self.e3_temporal_periodic)),
            3 => Some(EmbeddingSlice::Dense(&self.e4_temporal_positional)),
            4 => Some(EmbeddingSlice::Dense(&self.e5_causal)),
            5 => Some(EmbeddingSlice::Sparse(&self.e6_sparse)),
            6 => Some(EmbeddingSlice::Dense(&self.e7_code)),
            7 => Some(EmbeddingSlice::Dense(&self.e8_graph)),
            8 => Some(EmbeddingSlice::Dense(&self.e9_hdc)),
            9 => Some(EmbeddingSlice::Dense(&self.e10_multimodal)),
            10 => Some(EmbeddingSlice::Dense(&self.e11_entity)),
            11 => Some(EmbeddingSlice::TokenLevel(&self.e12_late_interaction)),
            12 => Some(EmbeddingSlice::Sparse(&self.e13_splade)),
            _ => None,
        }
    }

    /// Compute total storage size in bytes (heap allocations only).
    pub fn storage_size(&self) -> usize {
        let dense_size = (self.e1_semantic.len()
            + self.e2_temporal_recent.len()
            + self.e3_temporal_periodic.len()
            + self.e4_temporal_positional.len()
            + self.e5_causal.len()
            + self.e7_code.len()
            + self.e8_graph.len()
            + self.e9_hdc.len()
            + self.e10_multimodal.len()
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
            Embedder::Causal => EmbeddingRef::Dense(&self.e5_causal),
            Embedder::Sparse => EmbeddingRef::Sparse(&self.e6_sparse),
            Embedder::Code => EmbeddingRef::Dense(&self.e7_code),
            Embedder::Graph => EmbeddingRef::Dense(&self.e8_graph),
            Embedder::Hdc => EmbeddingRef::Dense(&self.e9_hdc),
            Embedder::Multimodal => EmbeddingRef::Dense(&self.e10_multimodal),
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
    /// # Returns
    ///
    /// `true` if all dense embeddings have correct dimensions, `false` otherwise.
    pub fn is_complete(&self) -> bool {
        self.e1_semantic.len() == E1_DIM
            && self.e2_temporal_recent.len() == E2_DIM
            && self.e3_temporal_periodic.len() == E3_DIM
            && self.e4_temporal_positional.len() == E4_DIM
            && self.e5_causal.len() == E5_DIM
            && self.e7_code.len() == E7_DIM
            && self.e8_graph.len() == E8_DIM
            && self.e9_hdc.len() == E9_DIM
            && self.e10_multimodal.len() == E10_DIM
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

    /// Validate all embeddings with detailed error reporting.
    ///
    /// This method performs comprehensive validation of all 13 embeddings
    /// and returns a structured error on failure.
    ///
    /// # Validation Rules
    ///
    /// 1. Dense embeddings (E1-E5, E7-E11): Must have exact dimensions
    /// 2. Sparse embeddings (E6, E13): Indices must be in-bounds and sorted
    /// 3. Token-level embedding (E12): Each token must have 128 dimensions
    ///
    /// # Returns
    ///
    /// * `Ok(())` - All embeddings are valid
    /// * `Err(ValidationError)` - First validation failure found (fail-fast)
    pub fn validate_strict(&self) -> Result<(), ValidationError> {
        // E1: Semantic
        if self.e1_semantic.len() != E1_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::Semantic,
                expected: E1_DIM,
                actual: self.e1_semantic.len(),
            });
        }

        // E2: Temporal Recent
        if self.e2_temporal_recent.len() != E2_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::TemporalRecent,
                expected: E2_DIM,
                actual: self.e2_temporal_recent.len(),
            });
        }

        // E3: Temporal Periodic
        if self.e3_temporal_periodic.len() != E3_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::TemporalPeriodic,
                expected: E3_DIM,
                actual: self.e3_temporal_periodic.len(),
            });
        }

        // E4: Temporal Positional
        if self.e4_temporal_positional.len() != E4_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::TemporalPositional,
                expected: E4_DIM,
                actual: self.e4_temporal_positional.len(),
            });
        }

        // E5: Causal
        if self.e5_causal.len() != E5_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::Causal,
                expected: E5_DIM,
                actual: self.e5_causal.len(),
            });
        }

        // E6: Sparse - validate index bounds and sortedness
        self.validate_sparse_vector(Embedder::Sparse, &self.e6_sparse, E6_SPARSE_VOCAB)?;

        // E7: Code
        if self.e7_code.len() != E7_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::Code,
                expected: E7_DIM,
                actual: self.e7_code.len(),
            });
        }

        // E8: Graph
        if self.e8_graph.len() != E8_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::Graph,
                expected: E8_DIM,
                actual: self.e8_graph.len(),
            });
        }

        // E9: HDC
        if self.e9_hdc.len() != E9_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::Hdc,
                expected: E9_DIM,
                actual: self.e9_hdc.len(),
            });
        }

        // E10: Multimodal
        if self.e10_multimodal.len() != E10_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::Multimodal,
                expected: E10_DIM,
                actual: self.e10_multimodal.len(),
            });
        }

        // E11: Entity
        if self.e11_entity.len() != E11_DIM {
            return Err(ValidationError::DimensionMismatch {
                embedder: Embedder::Entity,
                expected: E11_DIM,
                actual: self.e11_entity.len(),
            });
        }

        // E12: Late Interaction - each token must have 128 dimensions
        for (i, token) in self.e12_late_interaction.iter().enumerate() {
            if token.len() != E12_TOKEN_DIM {
                return Err(ValidationError::TokenDimensionMismatch {
                    embedder: Embedder::LateInteraction,
                    token_index: i,
                    expected: E12_TOKEN_DIM,
                    actual: token.len(),
                });
            }
        }

        // E13: Keyword SPLADE
        self.validate_sparse_vector(Embedder::KeywordSplade, &self.e13_splade, E13_SPLADE_VOCAB)?;

        Ok(())
    }

    /// Validate a sparse vector for the specified embedder.
    fn validate_sparse_vector(
        &self,
        embedder: Embedder,
        sparse: &SparseVector,
        vocab_size: usize,
    ) -> Result<(), ValidationError> {
        // Check indices/values length match
        if sparse.indices.len() != sparse.values.len() {
            return Err(ValidationError::SparseVectorError {
                embedder,
                source: SparseVectorError::LengthMismatch {
                    indices_len: sparse.indices.len(),
                    values_len: sparse.values.len(),
                },
            });
        }

        // Check indices are in bounds and sorted
        let mut prev: Option<u16> = None;
        for &idx in &sparse.indices {
            // Bounds check
            if idx as usize >= vocab_size {
                return Err(ValidationError::SparseVectorError {
                    embedder,
                    source: SparseVectorError::IndexOutOfBounds {
                        index: idx as usize,
                        max: vocab_size - 1,
                    },
                });
            }
            // Sorted check
            if let Some(p) = prev {
                if idx <= p {
                    return Err(ValidationError::SparseVectorError {
                        embedder,
                        source: SparseVectorError::UnsortedOrDuplicate { index: idx },
                    });
                }
            }
            prev = Some(idx);
        }

        Ok(())
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
            && self.e5_causal == other.e5_causal
            && self.e6_sparse == other.e6_sparse
            && self.e7_code == other.e7_code
            && self.e8_graph == other.e8_graph
            && self.e9_hdc == other.e9_hdc
            && self.e10_multimodal == other.e10_multimodal
            && self.e11_entity == other.e11_entity
            && self.e12_late_interaction == other.e12_late_interaction
            && self.e13_splade == other.e13_splade
    }
}
