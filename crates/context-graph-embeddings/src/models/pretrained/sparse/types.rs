//! Types, constants, and data structures for the sparse embedding model.
//!
//! This module contains all core types used by the SPLADE sparse model:
//! - Constants for dimensions, tokens, and latency
//! - SparseVector for sparse representations
//! - MlmHeadWeights for MLM head parameters
//! - ModelState for internal state management

use candle_core::Tensor;
use tokenizers::Tokenizer;

use crate::gpu::BertWeights;

use super::projection::ProjectionMatrix;

/// Sparse vector dimension (BERT vocabulary size).
pub const SPARSE_VOCAB_SIZE: usize = 30522;

/// Hidden size for SPLADE's BERT backbone.
#[allow(dead_code)]
pub const SPARSE_HIDDEN_SIZE: usize = 768;

/// Maximum tokens for SPLADE model.
pub const SPARSE_MAX_TOKENS: usize = 512;

/// Latency budget in milliseconds (P95 target).
pub const SPARSE_LATENCY_BUDGET_MS: u32 = 10;

/// HuggingFace model repository name.
pub const SPARSE_MODEL_NAME: &str = "naver/splade-cocondenser-ensembledistil";

/// Native sparse dimension (same as vocab size for SPLADE).
pub const SPARSE_NATIVE_DIMENSION: usize = SPARSE_VOCAB_SIZE;

/// Projected dimension for multi-array storage compatibility.
/// Per Constitution E6_Sparse: "~30K 5%active" projects to 1536D.
/// Must match dimensions::constants::SPARSE and ModelId::Sparse.projected_dimension().
pub const SPARSE_PROJECTED_DIMENSION: usize = 1536;

/// Compile-time assertion: SPARSE_PROJECTED_DIMENSION must match the canonical value.
const _: () = assert!(
    SPARSE_PROJECTED_DIMENSION == 1536,
    "SPARSE_PROJECTED_DIMENSION must be 1536 per Constitution E6_Sparse"
);

/// Expected sparsity ratio (typically 99%+ zeros).
pub const SPARSE_EXPECTED_SPARSITY: f32 = 0.99;

/// Sparse vector output with term indices and weights.
///
/// # Constitution Alignment
/// - Dimension: SPARSE_VOCAB_SIZE (30522)
/// - Expected sparsity: ~95% zeros (~5% active)
/// - Output after projection: 1536D dense (via ProjectionMatrix)
///
/// # BREAKING CHANGE v4.0.0
/// `to_dense_projected()` has been REMOVED. The hash-based projection
/// (`idx % projected_dim`) destroyed semantic information and violated
/// Constitution AP-007 (no stub data in prod).
///
/// Use `ProjectionMatrix::project()` instead for learned sparse-to-dense
/// conversion that preserves semantic relationships.
#[derive(Debug, Clone, PartialEq)]
pub struct SparseVector {
    /// Token indices with non-zero weights (sorted ascending).
    pub indices: Vec<usize>,
    /// Corresponding weights for each index.
    pub weights: Vec<f32>,
    /// Total number of dimensions (vocabulary size = 30522).
    pub dimension: usize,
}

impl SparseVector {
    /// Create a new sparse vector.
    ///
    /// # Arguments
    /// * `indices` - Token indices with non-zero weights (should be sorted ascending)
    /// * `weights` - Corresponding weights for each index
    ///
    /// # Invariants
    /// - `indices.len() == weights.len()`
    /// - All indices < SPARSE_VOCAB_SIZE (30522)
    /// - Indices should be sorted ascending (for efficient CSR conversion)
    ///
    /// # Panics
    /// Debug builds will panic if `indices.len() != weights.len()`
    pub fn new(indices: Vec<usize>, weights: Vec<f32>) -> Self {
        debug_assert_eq!(
            indices.len(),
            weights.len(),
            "indices and weights must have same length"
        );
        Self {
            indices,
            weights,
            dimension: SPARSE_VOCAB_SIZE,
        }
    }

    /// Convert to CSR (Compressed Sparse Row) format for cuBLAS.
    ///
    /// CSR format is required for efficient sparse matrix-vector multiplication
    /// with `ProjectionMatrix` using cuBLAS `csrmm2` or similar operations.
    ///
    /// # Returns
    /// `(row_ptr, col_indices, values)` tuple for CSR representation:
    /// - `row_ptr`: Row pointers [0, nnz] for single-row sparse matrix
    /// - `col_indices`: Column indices (the token indices as i32)
    /// - `values`: Non-zero values (the weights)
    ///
    /// # Implementation Note
    /// For a single vector (1 row), CSR format is:
    /// - `row_ptr = [0, nnz]` where nnz = number of non-zero elements
    /// - `col_indices = indices` converted to i32
    /// - `values = weights`
    ///
    /// # Example
    /// ```rust,ignore
    /// let sparse = SparseVector::new(vec![10, 100, 500], vec![0.5, 0.3, 0.8]);
    /// let (row_ptr, col_idx, vals) = sparse.to_csr();
    /// assert_eq!(row_ptr, vec![0, 3]);
    /// assert_eq!(col_idx, vec![10, 100, 500]);
    /// assert_eq!(vals, vec![0.5, 0.3, 0.8]);
    /// ```
    pub fn to_csr(&self) -> (Vec<i32>, Vec<i32>, Vec<f32>) {
        let nnz = self.indices.len() as i32;
        let row_ptr = vec![0i32, nnz];
        let col_indices: Vec<i32> = self.indices.iter().map(|&i| i as i32).collect();
        let values = self.weights.clone();
        (row_ptr, col_indices, values)
    }

    /// Get number of non-zero elements.
    ///
    /// # Returns
    /// Count of active (non-zero weight) indices in this sparse vector.
    #[inline]
    pub fn nnz(&self) -> usize {
        self.indices.len()
    }

    /// Get sparsity as ratio of zeros (0.0 to 1.0).
    ///
    /// # Returns
    /// Sparsity ratio: `1.0 - (nnz / dimension)`
    ///
    /// # Example
    /// A vector with 150 non-zero elements out of 30522:
    /// `sparsity = 1.0 - 150/30522 = 0.9951` (~99.5% sparse)
    pub fn sparsity(&self) -> f32 {
        1.0 - (self.indices.len() as f32 / self.dimension as f32)
    }

    // =========================================================================
    // REMOVED: to_dense_projected()
    // =========================================================================
    // The hash-based projection (`idx % projected_dim`) has been DELETED.
    // It violated Constitution AP-007 by using stub/mock behavior in production.
    //
    // Hash collision example:
    //   Token "machine" (idx 3057) and "learning" (idx 4593) would collide
    //   if 3057 % 1536 == 4593 % 1536 (which destroys semantic meaning)
    //
    // Migration path:
    //   OLD: let dense = sparse.to_dense_projected(1536);
    //   NEW: let dense = projection_matrix.project(&sparse)?;
    //
    // See TASK-EMB-012 for ProjectionMatrix implementation.
    // =========================================================================
}

impl Default for SparseVector {
    fn default() -> Self {
        Self {
            indices: Vec::new(),
            weights: Vec::new(),
            dimension: SPARSE_VOCAB_SIZE,
        }
    }
}

/// MLM head weights for SPLADE vocabulary projection.
#[derive(Debug)]
pub struct MlmHeadWeights {
    /// Dense transform: [hidden_size, hidden_size]
    pub dense_weight: Tensor,
    /// Dense bias: [hidden_size]
    pub dense_bias: Tensor,
    /// LayerNorm weight: [hidden_size]
    pub layer_norm_weight: Tensor,
    /// LayerNorm bias: [hidden_size]
    pub layer_norm_bias: Tensor,
    /// Decoder/output projection: [hidden_size, vocab_size]
    pub decoder_weight: Tensor,
    /// Decoder bias: [vocab_size]
    pub decoder_bias: Tensor,
}

/// Internal state that varies based on feature flags.
#[allow(dead_code)]
pub(crate) enum ModelState {
    /// Unloaded - no weights in memory.
    Unloaded,

    /// Loaded with candle model and tokenizer (GPU-accelerated).
    Loaded {
        /// BERT model weights on GPU (boxed to reduce enum size).
        weights: Box<BertWeights>,
        /// HuggingFace tokenizer for text encoding (boxed to reduce enum size).
        tokenizer: Box<Tokenizer>,
        /// MLM head weights for vocabulary projection.
        mlm_head: MlmHeadWeights,
        /// Learned projection matrix for sparse-to-dense conversion.
        /// CRITICAL: Uses neural projection, NOT hash modulo (AP-007).
        projection: Box<ProjectionMatrix>,
    },
}
