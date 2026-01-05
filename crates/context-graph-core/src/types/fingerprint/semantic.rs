//! SemanticFingerprint: The core 12-embedding array data structure.
//!
//! This module provides the foundational data structure for the Teleological Vector Architecture.
//! It stores all 12 embedding types WITHOUT fusion to preserve full semantic information.
//!
//! # Design Philosophy
//!
//! **NO FUSION**: Each embedding space is preserved independently for:
//! - Per-space HNSW search (12x independent indexes)
//! - Per-space Johari quadrant classification
//! - Per-space teleological alignment computation
//! - 100% information preservation
//!
//! # Storage
//!
//! Typical storage is ~46KB per fingerprint (vs ~6KB fused = 67% info loss avoided).
//!
//! # Embedding Dimensions
//!
//! | Embedding | Model | Dimensions |
//! |-----------|-------|------------|
//! | E1 | e5-large-v2 | 1024 |
//! | E2 | Exponential Decay | 512 |
//! | E3 | Fourier Periodic | 512 |
//! | E4 | Sinusoidal PE | 512 |
//! | E5 | Longformer SCM | 768 |
//! | E6 | SPLADE (Sparse) | ~1500 active / 30522 vocab |
//! | E7 | CodeT5p | 256 |
//! | E8 | MiniLM (Graph) | 384 |
//! | E9 | HDC | 10000 |
//! | E10 | CLIP | 768 |
//! | E11 | MiniLM (Entity) | 384 |
//! | E12 | ColBERT (Late-Interaction) | 128 per token |

use serde::{Deserialize, Serialize};

use super::sparse::SparseVector;

// ============================================================================
// DIMENSION CONSTANTS
// ============================================================================

/// E1: Semantic (e5-large-v2) embedding dimension.
pub const E1_DIM: usize = 1024;

/// E2: Temporal-Recent (exponential decay) embedding dimension.
pub const E2_DIM: usize = 512;

/// E3: Temporal-Periodic (Fourier) embedding dimension.
pub const E3_DIM: usize = 512;

/// E4: Temporal-Positional (sinusoidal PE) embedding dimension.
pub const E4_DIM: usize = 512;

/// E5: Causal (Longformer SCM) embedding dimension.
pub const E5_DIM: usize = 768;

/// E6: Sparse lexical (SPLADE) vocabulary size.
pub const E6_SPARSE_VOCAB: usize = 30_522;

/// E7: Code (CodeT5p) embedding dimension.
pub const E7_DIM: usize = 256;

/// E8: Graph (MiniLM for structure) embedding dimension.
pub const E8_DIM: usize = 384;

/// E9: HDC (10K-bit hyperdimensional) embedding dimension.
pub const E9_DIM: usize = 10_000;

/// E10: Multimodal (CLIP) embedding dimension.
pub const E10_DIM: usize = 768;

/// E11: Entity (MiniLM for facts) embedding dimension.
pub const E11_DIM: usize = 384;

/// E12: Late-Interaction (ColBERT) per-token embedding dimension.
pub const E12_TOKEN_DIM: usize = 128;

/// Total dense dimensions (excluding E6 sparse and E12 variable-length).
///
/// Calculated as: E1 + E2 + E3 + E4 + E5 + E7 + E8 + E9 + E10 + E11
/// = 1024 + 512 + 512 + 512 + 768 + 256 + 384 + 10000 + 768 + 384 = 15120
pub const TOTAL_DENSE_DIMS: usize =
    E1_DIM + E2_DIM + E3_DIM + E4_DIM + E5_DIM + E7_DIM + E8_DIM + E9_DIM + E10_DIM + E11_DIM;

// ============================================================================
// EMBEDDING SLICE
// ============================================================================

/// Reference type for returning embedding slices without copying.
///
/// This enum allows uniform access to all 12 embedding types while preserving
/// their different representations (dense, sparse, token-level).
#[derive(Debug)]
pub enum EmbeddingSlice<'a> {
    /// Dense embedding as a contiguous f32 slice.
    Dense(&'a [f32]),

    /// Sparse embedding (E6 SPLADE).
    Sparse(&'a SparseVector),

    /// Token-level embedding (E12 ColBERT) - variable number of 128D tokens.
    TokenLevel(&'a [Vec<f32>]),
}

// ============================================================================
// SEMANTIC FINGERPRINT
// ============================================================================

/// SemanticFingerprint: Stores all 12 embeddings without fusion.
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
/// 1. Enable serde serialization for large embeddings (E9 has 10000 dims)
/// 2. Avoid stack overflow with large arrays
/// 3. Maintain flexibility for future dimension changes
///
/// Dimension validation is performed via `validate()` and construction methods.
///
/// # Fields
///
/// | Field | Embedding Type | Dimensions | Description |
/// |-------|---------------|------------|-------------|
/// | e1_semantic | Dense | 1024 | e5-large-v2 semantic |
/// | e2_temporal_recent | Dense | 512 | Exponential decay |
/// | e3_temporal_periodic | Dense | 512 | Fourier periodic |
/// | e4_temporal_positional | Dense | 512 | Sinusoidal PE |
/// | e5_causal | Dense | 768 | Longformer SCM |
/// | e6_sparse | Sparse | ~1500/30522 | SPLADE lexical |
/// | e7_code | Dense | 256 | CodeT5p |
/// | e8_graph | Dense | 384 | MiniLM graph |
/// | e9_hdc | Dense | 10000 | Hyperdimensional |
/// | e10_multimodal | Dense | 768 | CLIP |
/// | e11_entity | Dense | 384 | MiniLM entity |
/// | e12_late_interaction | Token-level | 128/token | ColBERT |
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFingerprint {
    /// E1: Semantic (e5-large-v2) - 1024D dense embedding.
    ///
    /// Captures deep semantic meaning using sentence transformers.
    pub e1_semantic: Vec<f32>,

    /// E2: Temporal-Recent (exponential decay) - 512D dense embedding.
    ///
    /// Encodes recency with exponential decay weighting.
    pub e2_temporal_recent: Vec<f32>,

    /// E3: Temporal-Periodic (Fourier) - 512D dense embedding.
    ///
    /// Captures periodic patterns (daily, weekly, seasonal) via Fourier analysis.
    pub e3_temporal_periodic: Vec<f32>,

    /// E4: Temporal-Positional (sinusoidal PE) - 512D dense embedding.
    ///
    /// Transformer-style positional encoding for absolute position.
    pub e4_temporal_positional: Vec<f32>,

    /// E5: Causal (Longformer SCM) - 768D dense embedding.
    ///
    /// Captures causal relationships via structural causal model.
    pub e5_causal: Vec<f32>,

    /// E6: Sparse Lexical (SPLADE) - sparse vector with ~1500 active of 30522 vocab.
    ///
    /// Explicit term matching with learned expansion for lexical retrieval.
    pub e6_sparse: SparseVector,

    /// E7: Code (CodeT5p) - 256D dense embedding.
    ///
    /// Specialized for code understanding and semantic similarity.
    pub e7_code: Vec<f32>,

    /// E8: Graph (MiniLM for structure) - 384D dense embedding.
    ///
    /// Captures structural relationships in knowledge graphs.
    pub e8_graph: Vec<f32>,

    /// E9: HDC (10K-bit hyperdimensional) - 10000D dense embedding.
    ///
    /// Hyperdimensional computing for compositional semantics.
    pub e9_hdc: Vec<f32>,

    /// E10: Multimodal (CLIP) - 768D dense embedding.
    ///
    /// Shared embedding space for text and images.
    pub e10_multimodal: Vec<f32>,

    /// E11: Entity (MiniLM for facts) - 384D dense embedding.
    ///
    /// Specialized for named entity and fact retrieval.
    pub e11_entity: Vec<f32>,

    /// E12: Late-Interaction (ColBERT) - 128D per token, variable token count.
    ///
    /// Token-level embeddings for maximum matching (MaxSim) retrieval.
    /// Each inner Vec has exactly E12_TOKEN_DIM (128) elements.
    pub e12_late_interaction: Vec<Vec<f32>>,
}

impl SemanticFingerprint {
    /// Create a zeroed fingerprint (all embeddings initialized to 0.0).
    ///
    /// This is useful for:
    /// - Initialization before population
    /// - Testing and benchmarking
    /// - Default placeholder values
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::types::fingerprint::SemanticFingerprint;
    ///
    /// let fp = SemanticFingerprint::zeroed();
    /// assert_eq!(fp.token_count(), 0);
    /// assert!(fp.validate().is_ok());
    /// ```
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
        }
    }

    /// Get embedding by index (0-11).
    ///
    /// Returns a reference to the embedding at the given index wrapped in an
    /// [`EmbeddingSlice`] to handle different embedding types uniformly.
    ///
    /// # Arguments
    ///
    /// * `idx` - Embedding index from 0 to 11 (inclusive)
    ///
    /// # Returns
    ///
    /// * `Some(EmbeddingSlice)` - The embedding slice if index is valid
    /// * `None` - If index is out of bounds (>= 12)
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::types::fingerprint::{SemanticFingerprint, EmbeddingSlice, E1_DIM};
    ///
    /// let fp = SemanticFingerprint::zeroed();
    ///
    /// // Get E1 (semantic) - returns Dense slice
    /// if let Some(EmbeddingSlice::Dense(slice)) = fp.get_embedding(0) {
    ///     assert_eq!(slice.len(), E1_DIM);
    /// }
    ///
    /// // Get E6 (sparse) - returns Sparse
    /// if let Some(EmbeddingSlice::Sparse(sparse)) = fp.get_embedding(5) {
    ///     assert_eq!(sparse.nnz(), 0); // zeroed fingerprint has empty sparse
    /// }
    /// ```
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
            _ => None,
        }
    }

    /// Compute total storage size in bytes (heap allocations only).
    ///
    /// This calculates the memory footprint of all embeddings on the heap,
    /// not including the struct itself on the stack.
    ///
    /// # Calculation
    ///
    /// - Dense embeddings: dimension * sizeof(f32) = dimension * 4 bytes
    /// - E6 sparse: SparseVector::memory_size()
    /// - E12 tokens: token_count * 128 * sizeof(f32)
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::types::fingerprint::SemanticFingerprint;
    ///
    /// let fp = SemanticFingerprint::zeroed();
    /// let size = fp.storage_size();
    /// // Dense dims: 15120 * 4 = 60480 bytes
    /// // E6 sparse (empty): 0 bytes
    /// // E12 tokens (empty): 0 bytes
    /// assert_eq!(size, 60480);
    /// ```
    pub fn storage_size(&self) -> usize {
        // Dense embeddings
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

        // E6 sparse vector
        let sparse_size = self.e6_sparse.memory_size();

        // E12 late-interaction tokens
        let token_size: usize = self
            .e12_late_interaction
            .iter()
            .map(|t| t.len() * std::mem::size_of::<f32>())
            .sum();

        dense_size + sparse_size + token_size
    }

    /// Validate all embeddings have correct dimensions.
    ///
    /// This method performs a fail-fast validation of all embedding dimensions.
    /// Returns an error immediately if any dimension is incorrect.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - All embeddings have correct dimensions
    /// * `Err(String)` - Description of validation failure
    ///
    /// # Validation Rules
    ///
    /// 1. E1: Must have exactly 1024 dimensions
    /// 2. E2-E4: Must have exactly 512 dimensions each
    /// 3. E5: Must have exactly 768 dimensions
    /// 4. E6: Sparse indices must be < E6_SPARSE_VOCAB
    /// 5. E7: Must have exactly 256 dimensions
    /// 6. E8, E11: Must have exactly 384 dimensions each
    /// 7. E9: Must have exactly 10000 dimensions
    /// 8. E10: Must have exactly 768 dimensions
    /// 9. E12: Each token must have exactly 128 dimensions
    pub fn validate(&self) -> Result<(), String> {
        // E1: Semantic
        if self.e1_semantic.len() != E1_DIM {
            return Err(format!(
                "E1 semantic dimension mismatch: expected {}, got {}",
                E1_DIM,
                self.e1_semantic.len()
            ));
        }

        // E2: Temporal-Recent
        if self.e2_temporal_recent.len() != E2_DIM {
            return Err(format!(
                "E2 temporal_recent dimension mismatch: expected {}, got {}",
                E2_DIM,
                self.e2_temporal_recent.len()
            ));
        }

        // E3: Temporal-Periodic
        if self.e3_temporal_periodic.len() != E3_DIM {
            return Err(format!(
                "E3 temporal_periodic dimension mismatch: expected {}, got {}",
                E3_DIM,
                self.e3_temporal_periodic.len()
            ));
        }

        // E4: Temporal-Positional
        if self.e4_temporal_positional.len() != E4_DIM {
            return Err(format!(
                "E4 temporal_positional dimension mismatch: expected {}, got {}",
                E4_DIM,
                self.e4_temporal_positional.len()
            ));
        }

        // E5: Causal
        if self.e5_causal.len() != E5_DIM {
            return Err(format!(
                "E5 causal dimension mismatch: expected {}, got {}",
                E5_DIM,
                self.e5_causal.len()
            ));
        }

        // E6: Sparse - validate indices within vocabulary bounds
        for &idx in &self.e6_sparse.indices {
            if idx as usize >= E6_SPARSE_VOCAB {
                return Err(format!(
                    "E6 sparse index {} exceeds vocabulary size {}",
                    idx, E6_SPARSE_VOCAB
                ));
            }
        }
        // Also verify indices and values lengths match
        if self.e6_sparse.indices.len() != self.e6_sparse.values.len() {
            return Err(format!(
                "E6 sparse indices ({}) and values ({}) length mismatch",
                self.e6_sparse.indices.len(),
                self.e6_sparse.values.len()
            ));
        }

        // E7: Code
        if self.e7_code.len() != E7_DIM {
            return Err(format!(
                "E7 code dimension mismatch: expected {}, got {}",
                E7_DIM,
                self.e7_code.len()
            ));
        }

        // E8: Graph
        if self.e8_graph.len() != E8_DIM {
            return Err(format!(
                "E8 graph dimension mismatch: expected {}, got {}",
                E8_DIM,
                self.e8_graph.len()
            ));
        }

        // E9: HDC
        if self.e9_hdc.len() != E9_DIM {
            return Err(format!(
                "E9 hdc dimension mismatch: expected {}, got {}",
                E9_DIM,
                self.e9_hdc.len()
            ));
        }

        // E10: Multimodal
        if self.e10_multimodal.len() != E10_DIM {
            return Err(format!(
                "E10 multimodal dimension mismatch: expected {}, got {}",
                E10_DIM,
                self.e10_multimodal.len()
            ));
        }

        // E11: Entity
        if self.e11_entity.len() != E11_DIM {
            return Err(format!(
                "E11 entity dimension mismatch: expected {}, got {}",
                E11_DIM,
                self.e11_entity.len()
            ));
        }

        // E12: Late-Interaction - each token must have E12_TOKEN_DIM dimensions
        for (i, token) in self.e12_late_interaction.iter().enumerate() {
            if token.len() != E12_TOKEN_DIM {
                return Err(format!(
                    "E12 late_interaction token {} dimension mismatch: expected {}, got {}",
                    i,
                    E12_TOKEN_DIM,
                    token.len()
                ));
            }
        }

        Ok(())
    }

    /// Get the number of tokens in E12 late-interaction embedding.
    ///
    /// # Returns
    ///
    /// The number of token embeddings stored in E12. Each token has
    /// [`E12_TOKEN_DIM`] (128) dimensions.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_core::types::fingerprint::SemanticFingerprint;
    ///
    /// let fp = SemanticFingerprint::zeroed();
    /// assert_eq!(fp.token_count(), 0);
    /// ```
    #[inline]
    pub fn token_count(&self) -> usize {
        self.e12_late_interaction.len()
    }

    /// Get embedding name by index.
    ///
    /// # Returns
    ///
    /// Human-readable name for the embedding at the given index, or None if out of bounds.
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
            _ => None,
        }
    }

    /// Get embedding dimension by index.
    ///
    /// # Returns
    ///
    /// The dimension of the embedding at the given index. For E6 (sparse),
    /// returns the vocabulary size. For E12 (token-level), returns the
    /// per-token dimension.
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
            _ => None,
        }
    }
}

impl Default for SemanticFingerprint {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl PartialEq for SemanticFingerprint {
    fn eq(&self, other: &Self) -> bool {
        // Compare all embeddings
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
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_fingerprint_zeroed() {
        let fp = SemanticFingerprint::zeroed();

        // Verify all dense embeddings are zeroed
        assert!(fp.e1_semantic.iter().all(|&v| v == 0.0));
        assert!(fp.e2_temporal_recent.iter().all(|&v| v == 0.0));
        assert!(fp.e3_temporal_periodic.iter().all(|&v| v == 0.0));
        assert!(fp.e4_temporal_positional.iter().all(|&v| v == 0.0));
        assert!(fp.e5_causal.iter().all(|&v| v == 0.0));
        assert!(fp.e7_code.iter().all(|&v| v == 0.0));
        assert!(fp.e8_graph.iter().all(|&v| v == 0.0));
        assert!(fp.e9_hdc.iter().all(|&v| v == 0.0));
        assert!(fp.e10_multimodal.iter().all(|&v| v == 0.0));
        assert!(fp.e11_entity.iter().all(|&v| v == 0.0));

        // Verify sparse and token embeddings are empty
        assert!(fp.e6_sparse.is_empty());
        assert!(fp.e12_late_interaction.is_empty());
    }

    #[test]
    fn test_semantic_fingerprint_dimensions() {
        let fp = SemanticFingerprint::zeroed();

        // Verify all dimensions match constants
        assert_eq!(fp.e1_semantic.len(), E1_DIM);
        assert_eq!(fp.e2_temporal_recent.len(), E2_DIM);
        assert_eq!(fp.e3_temporal_periodic.len(), E3_DIM);
        assert_eq!(fp.e4_temporal_positional.len(), E4_DIM);
        assert_eq!(fp.e5_causal.len(), E5_DIM);
        assert_eq!(fp.e7_code.len(), E7_DIM);
        assert_eq!(fp.e8_graph.len(), E8_DIM);
        assert_eq!(fp.e9_hdc.len(), E9_DIM);
        assert_eq!(fp.e10_multimodal.len(), E10_DIM);
        assert_eq!(fp.e11_entity.len(), E11_DIM);

        // Verify total dense dimensions constant
        let expected_total = E1_DIM + E2_DIM + E3_DIM + E4_DIM + E5_DIM + E7_DIM + E8_DIM + E9_DIM
            + E10_DIM
            + E11_DIM;
        assert_eq!(TOTAL_DENSE_DIMS, expected_total);
        assert_eq!(TOTAL_DENSE_DIMS, 15120);
    }

    #[test]
    fn test_semantic_fingerprint_get_embedding() {
        let fp = SemanticFingerprint::zeroed();

        // Test all 12 embeddings are accessible
        for idx in 0..12 {
            assert!(
                fp.get_embedding(idx).is_some(),
                "Embedding {} should be accessible",
                idx
            );
        }

        // Test out of bounds returns None
        assert!(fp.get_embedding(12).is_none());
        assert!(fp.get_embedding(100).is_none());
    }

    #[test]
    fn test_semantic_fingerprint_get_embedding_types() {
        let fp = SemanticFingerprint::zeroed();

        // E0-E4 should be Dense
        for idx in 0..5 {
            match fp.get_embedding(idx) {
                Some(EmbeddingSlice::Dense(_)) => {}
                _ => panic!("E{} should be Dense", idx + 1),
            }
        }

        // E5 should be Sparse (index 5)
        match fp.get_embedding(5) {
            Some(EmbeddingSlice::Sparse(_)) => {}
            _ => panic!("E6 should be Sparse"),
        }

        // E6-E10 should be Dense (indices 6-10)
        for idx in 6..11 {
            match fp.get_embedding(idx) {
                Some(EmbeddingSlice::Dense(_)) => {}
                _ => panic!("E{} should be Dense", idx + 1),
            }
        }

        // E11 should be TokenLevel (index 11)
        match fp.get_embedding(11) {
            Some(EmbeddingSlice::TokenLevel(_)) => {}
            _ => panic!("E12 should be TokenLevel"),
        }
    }

    #[test]
    fn test_semantic_fingerprint_storage_size_zeroed() {
        let fp = SemanticFingerprint::zeroed();
        let size = fp.storage_size();

        // Dense dims: 15120 * 4 = 60480 bytes
        // Sparse (empty): 0 bytes
        // Tokens (empty): 0 bytes
        let expected = TOTAL_DENSE_DIMS * std::mem::size_of::<f32>();
        assert_eq!(expected, 60480);
        assert_eq!(size, expected);
    }

    #[test]
    fn test_semantic_fingerprint_storage_size_with_sparse() {
        let mut fp = SemanticFingerprint::zeroed();

        // Add some sparse entries
        fp.e6_sparse = SparseVector::new(vec![1, 10, 100, 1000], vec![0.1, 0.2, 0.3, 0.4])
            .expect("valid sparse vector");

        let size = fp.storage_size();

        // Dense: 60480 bytes
        // Sparse: 4 indices * 2 bytes + 4 values * 4 bytes = 8 + 16 = 24 bytes
        // Tokens: 0 bytes
        let expected = 60480 + 24;
        assert_eq!(size, expected);
    }

    #[test]
    fn test_semantic_fingerprint_storage_size_with_tokens() {
        let mut fp = SemanticFingerprint::zeroed();

        // Add 10 tokens
        fp.e12_late_interaction = vec![vec![0.0; E12_TOKEN_DIM]; 10];

        let size = fp.storage_size();

        // Dense: 60480 bytes
        // Sparse: 0 bytes
        // Tokens: 10 * 128 * 4 = 5120 bytes
        let expected = 60480 + 5120;
        assert_eq!(size, expected);
    }

    #[test]
    fn test_semantic_fingerprint_typical_storage_size() {
        let mut fp = SemanticFingerprint::zeroed();

        // Typical sparse: ~1500 active entries
        let indices: Vec<u16> = (0..1500_u16).map(|i| i * 20).collect();
        let values: Vec<f32> = vec![0.1; 1500];
        fp.e6_sparse = SparseVector::new(indices, values).expect("valid sparse vector");

        // Typical tokens: ~50 tokens
        fp.e12_late_interaction = vec![vec![0.0; E12_TOKEN_DIM]; 50];

        let size = fp.storage_size();

        // Dense: 60480 bytes
        // Sparse: 1500 * 2 + 1500 * 4 = 3000 + 6000 = 9000 bytes
        // Tokens: 50 * 128 * 4 = 25600 bytes
        let expected = 60480 + 9000 + 25600;
        assert_eq!(size, expected);

        // Should be approximately 95KB for typical usage
        assert!(size > 90_000);
        assert!(size < 100_000);
    }

    #[test]
    fn test_semantic_fingerprint_serialization_roundtrip() {
        let mut fp = SemanticFingerprint::zeroed();

        // Set some non-zero values
        fp.e1_semantic[0] = 1.0;
        fp.e1_semantic[100] = 2.5;
        fp.e5_causal[50] = 3.14;
        fp.e9_hdc[9999] = -1.0;

        // Add sparse entries
        fp.e6_sparse = SparseVector::new(vec![100, 200, 300], vec![0.5, 0.6, 0.7])
            .expect("valid sparse vector");

        // Add tokens
        let mut token = vec![0.0_f32; E12_TOKEN_DIM];
        token[0] = 1.0;
        token[127] = -1.0;
        fp.e12_late_interaction = vec![token; 3];

        // Serialize and deserialize with bincode
        let bytes = bincode::serialize(&fp).expect("serialization should succeed");
        let restored: SemanticFingerprint =
            bincode::deserialize(&bytes).expect("deserialization should succeed");

        // Verify equality
        assert_eq!(fp, restored);

        // Verify specific values
        assert_eq!(restored.e1_semantic[0], 1.0);
        assert_eq!(restored.e1_semantic[100], 2.5);
        assert_eq!(restored.e5_causal[50], 3.14);
        assert_eq!(restored.e9_hdc[9999], -1.0);
        assert_eq!(restored.e6_sparse.nnz(), 3);
        assert_eq!(restored.token_count(), 3);
    }

    #[test]
    fn test_semantic_fingerprint_token_count() {
        let mut fp = SemanticFingerprint::zeroed();
        assert_eq!(fp.token_count(), 0);

        fp.e12_late_interaction = vec![vec![0.0; E12_TOKEN_DIM]; 5];
        assert_eq!(fp.token_count(), 5);

        fp.e12_late_interaction = vec![vec![0.0; E12_TOKEN_DIM]; 100];
        assert_eq!(fp.token_count(), 100);
    }

    #[test]
    fn test_semantic_fingerprint_validate() {
        // Valid fingerprint should pass validation
        let fp = SemanticFingerprint::zeroed();
        assert!(fp.validate().is_ok());

        // Fingerprint with valid sparse entries should pass
        let mut fp2 = SemanticFingerprint::zeroed();
        fp2.e6_sparse = SparseVector::new(vec![100, 200, 30521], vec![0.1, 0.2, 0.3])
            .expect("valid sparse vector");
        assert!(fp2.validate().is_ok());

        // Test with valid E12 tokens
        let mut fp3 = SemanticFingerprint::zeroed();
        fp3.e12_late_interaction = vec![vec![0.0; E12_TOKEN_DIM]; 10];
        assert!(fp3.validate().is_ok());
    }

    #[test]
    fn test_semantic_fingerprint_validate_dimension_errors() {
        // Test E1 dimension mismatch
        let mut fp = SemanticFingerprint::zeroed();
        fp.e1_semantic = vec![0.0; 100]; // Wrong dimension
        let err = fp.validate().unwrap_err();
        assert!(err.contains("E1 semantic dimension mismatch"));

        // Test E9 dimension mismatch (the largest embedding)
        let mut fp2 = SemanticFingerprint::zeroed();
        fp2.e9_hdc = vec![0.0; 1000]; // Wrong dimension
        let err2 = fp2.validate().unwrap_err();
        assert!(err2.contains("E9 hdc dimension mismatch"));

        // Test E12 token dimension mismatch
        let mut fp3 = SemanticFingerprint::zeroed();
        fp3.e12_late_interaction = vec![vec![0.0; 64]]; // Wrong token dimension
        let err3 = fp3.validate().unwrap_err();
        assert!(err3.contains("E12 late_interaction token 0 dimension mismatch"));
    }

    #[test]
    fn test_semantic_fingerprint_default() {
        let fp1 = SemanticFingerprint::default();
        let fp2 = SemanticFingerprint::zeroed();
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_semantic_fingerprint_partial_eq() {
        let fp1 = SemanticFingerprint::zeroed();
        let fp2 = SemanticFingerprint::zeroed();
        assert_eq!(fp1, fp2);

        let mut fp3 = SemanticFingerprint::zeroed();
        fp3.e1_semantic[0] = 1.0;
        assert_ne!(fp1, fp3);
    }

    #[test]
    fn test_embedding_name() {
        assert_eq!(
            SemanticFingerprint::embedding_name(0),
            Some("E1_Semantic")
        );
        assert_eq!(
            SemanticFingerprint::embedding_name(5),
            Some("E6_Sparse_Lexical")
        );
        assert_eq!(
            SemanticFingerprint::embedding_name(11),
            Some("E12_Late_Interaction")
        );
        assert_eq!(SemanticFingerprint::embedding_name(12), None);
    }

    #[test]
    fn test_embedding_dim() {
        assert_eq!(SemanticFingerprint::embedding_dim(0), Some(E1_DIM));
        assert_eq!(SemanticFingerprint::embedding_dim(5), Some(E6_SPARSE_VOCAB));
        assert_eq!(SemanticFingerprint::embedding_dim(11), Some(E12_TOKEN_DIM));
        assert_eq!(SemanticFingerprint::embedding_dim(12), None);
    }

    #[test]
    fn test_dimension_constants() {
        // Verify all dimension constants match the specification
        assert_eq!(E1_DIM, 1024);
        assert_eq!(E2_DIM, 512);
        assert_eq!(E3_DIM, 512);
        assert_eq!(E4_DIM, 512);
        assert_eq!(E5_DIM, 768);
        assert_eq!(E6_SPARSE_VOCAB, 30_522);
        assert_eq!(E7_DIM, 256);
        assert_eq!(E8_DIM, 384);
        assert_eq!(E9_DIM, 10_000);
        assert_eq!(E10_DIM, 768);
        assert_eq!(E11_DIM, 384);
        assert_eq!(E12_TOKEN_DIM, 128);

        // Verify TOTAL_DENSE_DIMS calculation
        let calculated = E1_DIM
            + E2_DIM
            + E3_DIM
            + E4_DIM
            + E5_DIM
            + E7_DIM
            + E8_DIM
            + E9_DIM
            + E10_DIM
            + E11_DIM;
        assert_eq!(TOTAL_DENSE_DIMS, calculated);
    }
}
