//! Stub embedder implementations for testing.
//!
//! These produce deterministic embeddings based on content hash.
//! Same input always produces same output - useful for testing.
//!
//! **NOT for production** - use real model implementations.
//!
//! # Architecture Reference
//!
//! From constitution.yaml (ARCH-01): "TeleologicalArray is atomic - store all 13 embeddings or nothing"
//! From constitution.yaml (AP-14): "No .unwrap() in library code"

use async_trait::async_trait;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::embeddings::EMBEDDER_CONFIGS;
use crate::error::{CoreError, CoreResult};
use crate::teleological::Embedder;
use crate::traits::{SingleEmbedder, SparseEmbedder, TokenEmbedder};
use crate::types::fingerprint::SparseVector;

// ============================================================================
// Constants
// ============================================================================

/// Minimum number of active indices for sparse vectors.
const MIN_SPARSE_ACTIVE: usize = 5;

/// Maximum number of active indices for sparse vectors (matches MAX_SPARSE_ACTIVE).
const MAX_SPARSE_ACTIVE: usize = 1500;

/// Modulo for determining sparse vector density based on content length.
const SPARSE_DENSITY_MODULO: usize = 100;

/// Prime number for hash distribution in sparse indices.
const HASH_PRIME: u64 = 7919;

/// Golden ratio constant for token hash mixing (provides good bit distribution).
const GOLDEN_RATIO_PRIME: u64 = 0x9e3779b97f4a7c15;

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate deterministic hash from content.
fn content_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Generate deterministic f32 from hash and index.
///
/// Maps to [-0.5, 0.5] range to approximate normalized embeddings.
fn hash_to_f32(hash: u64, index: usize) -> f32 {
    let combined = hash.wrapping_add(index as u64);
    // Map to [-0.5, 0.5] range
    ((combined as f64 / u64::MAX as f64) - 0.5) as f32
}

// ============================================================================
// StubSingleEmbedder
// ============================================================================

/// Stub dense embedder for testing.
///
/// Produces deterministic embeddings based on content hash.
/// Used for E1-E5, E7-E11 (10 dense embedders).
///
/// # Example
///
/// ```ignore
/// use context_graph_core::embeddings::stubs::StubSingleEmbedder;
///
/// let embedder = StubSingleEmbedder::for_e1();
/// let embedding = embedder.embed("test content").await.unwrap();
/// assert_eq!(embedding.len(), 1024); // E1 is 1024D
/// ```
pub struct StubSingleEmbedder {
    embedder: Embedder,
    dimension: usize,
    model_id: String,
}

impl StubSingleEmbedder {
    /// Create a new stub embedder for the specified embedder type.
    ///
    /// # Arguments
    ///
    /// * `embedder` - The embedder type (must be a dense embedder)
    ///
    /// # Panics
    ///
    /// Does not panic, but will produce incorrect results if used with
    /// sparse (E6, E13) or token-level (E12) embedders.
    pub fn new(embedder: Embedder) -> Self {
        let config = &EMBEDDER_CONFIGS[embedder.index()];
        Self {
            embedder,
            dimension: config.dimension,
            model_id: format!("stub-{}", embedder.short_name().to_lowercase()),
        }
    }

    /// Create stub embedder for E1 (Semantic, 1024D).
    pub fn for_e1() -> Self {
        Self::new(Embedder::Semantic)
    }

    /// Create stub embedder for E2 (TemporalRecent, 512D).
    pub fn for_e2() -> Self {
        Self::new(Embedder::TemporalRecent)
    }

    /// Create stub embedder for E3 (TemporalPeriodic, 512D).
    pub fn for_e3() -> Self {
        Self::new(Embedder::TemporalPeriodic)
    }

    /// Create stub embedder for E4 (TemporalPositional, 512D).
    pub fn for_e4() -> Self {
        Self::new(Embedder::TemporalPositional)
    }

    /// Create stub embedder for E5 (Causal, 768D).
    pub fn for_e5() -> Self {
        Self::new(Embedder::Causal)
    }

    /// Create stub embedder for E7 (Code, 1536D).
    pub fn for_e7() -> Self {
        Self::new(Embedder::Code)
    }

    /// Create stub embedder for E8 (Emotional/Graph, 384D).
    ///
    /// Note: The field in SemanticFingerprint is `e8_graph`, but the
    /// canonical enum name is `Embedder::Emotional`.
    pub fn for_e8() -> Self {
        Self::new(Embedder::Emotional)
    }

    /// Create stub embedder for E9 (HDC, 1024D projected).
    pub fn for_e9() -> Self {
        Self::new(Embedder::Hdc)
    }

    /// Create stub embedder for E10 (Multimodal, 768D).
    pub fn for_e10() -> Self {
        Self::new(Embedder::Multimodal)
    }

    /// Create stub embedder for E11 (Entity/KEPLER, 768D).
    pub fn for_e11() -> Self {
        Self::new(Embedder::Entity)
    }

    /// Get the embedder type this stub represents.
    pub fn embedder(&self) -> Embedder {
        self.embedder
    }
}

#[async_trait]
impl SingleEmbedder for StubSingleEmbedder {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }

    async fn embed(&self, content: &str) -> CoreResult<Vec<f32>> {
        let hash = content_hash(content);
        let embedding: Vec<f32> = (0..self.dimension)
            .map(|i| hash_to_f32(hash, i))
            .collect();
        Ok(embedding)
    }

    fn is_ready(&self) -> bool {
        true
    }
}

// ============================================================================
// StubSparseEmbedder
// ============================================================================

/// Stub sparse embedder for E6 and E13.
///
/// Produces deterministic sparse vectors based on content hash.
/// The number of active indices is proportional to content length
/// (with minimum 5 and maximum 1500 to match typical SPLADE behavior).
///
/// # Example
///
/// ```ignore
/// use context_graph_core::embeddings::stubs::StubSparseEmbedder;
///
/// let embedder = StubSparseEmbedder::for_e6();
/// let sparse = embedder.embed_sparse("test content").await.unwrap();
/// assert!(sparse.nnz() > 0);
/// ```
pub struct StubSparseEmbedder {
    embedder: Embedder,
    vocab_size: usize,
    model_id: String,
}

impl StubSparseEmbedder {
    /// Create a new stub sparse embedder.
    ///
    /// # Arguments
    ///
    /// * `embedder` - The embedder type (should be E6 or E13)
    pub fn new(embedder: Embedder) -> Self {
        let vocab_size = match embedder {
            Embedder::Sparse | Embedder::KeywordSplade => 30_522,
            _ => 30_522, // Default to BERT vocab
        };
        Self {
            embedder,
            vocab_size,
            model_id: format!("stub-{}", embedder.short_name().to_lowercase()),
        }
    }

    /// Create stub sparse embedder for E6 (Sparse Lexical).
    pub fn for_e6() -> Self {
        Self::new(Embedder::Sparse)
    }

    /// Create stub sparse embedder for E13 (KeywordSplade).
    pub fn for_e13() -> Self {
        Self::new(Embedder::KeywordSplade)
    }

    /// Get the embedder type this stub represents.
    pub fn embedder(&self) -> Embedder {
        self.embedder
    }
}

#[async_trait]
impl SparseEmbedder for StubSparseEmbedder {
    fn vocab_size(&self) -> usize {
        self.vocab_size
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }

    async fn embed_sparse(&self, content: &str) -> CoreResult<SparseVector> {
        if content.is_empty() {
            return Ok(SparseVector::empty());
        }

        let hash = content_hash(content);
        // Generate ~5% active indices (typical SPLADE sparsity)
        // Minimum 5 active, maximum 1500 (matches MAX_SPARSE_ACTIVE)
        let num_active =
            (content.len() % SPARSE_DENSITY_MODULO).clamp(MIN_SPARSE_ACTIVE, MAX_SPARSE_ACTIVE);

        let mut indices: Vec<u16> = Vec::with_capacity(num_active);
        let mut values: Vec<f32> = Vec::with_capacity(num_active);

        for i in 0..num_active {
            // Use prime multiplier for better distribution
            let idx = ((hash.wrapping_add(i as u64 * HASH_PRIME) as usize) % self.vocab_size)
                as u16;
            let val = (hash_to_f32(hash, i) + 0.5).abs(); // Positive activation [0, 1]
            indices.push(idx);
            values.push(val);
        }

        // Sort indices (required by SparseVector)
        let mut pairs: Vec<_> = indices.into_iter().zip(values).collect();
        pairs.sort_by_key(|(idx, _)| *idx);
        // Remove duplicates (keep first occurrence)
        pairs.dedup_by_key(|(idx, _)| *idx);

        let (indices, values): (Vec<_>, Vec<_>) = pairs.into_iter().unzip();

        SparseVector::new(indices, values).map_err(|e| {
            CoreError::Embedding(format!(
                "Sparse vector creation failed for {:?}: {}",
                self.embedder, e
            ))
        })
    }

    fn is_ready(&self) -> bool {
        true
    }
}

// ============================================================================
// StubTokenEmbedder
// ============================================================================

/// Stub token embedder for E12 (ColBERT late-interaction).
///
/// Produces deterministic per-token embeddings based on content hash.
/// Token count is approximately 1 per whitespace-separated word.
///
/// # Example
///
/// ```ignore
/// use context_graph_core::embeddings::stubs::StubTokenEmbedder;
///
/// let embedder = StubTokenEmbedder::new();
/// let tokens = embedder.embed_tokens("hello world").await.unwrap();
/// assert_eq!(tokens.len(), 2); // Two words = two tokens
/// assert_eq!(tokens[0].len(), 128); // 128D per token
/// ```
pub struct StubTokenEmbedder {
    token_dim: usize,
    max_tokens: usize,
    model_id: String,
}

impl StubTokenEmbedder {
    /// Create a new stub token embedder with default settings.
    ///
    /// - Token dimension: 128 (ColBERT standard)
    /// - Max tokens: 512 (BERT-base limit)
    pub fn new() -> Self {
        Self {
            token_dim: 128,
            max_tokens: 512,
            model_id: "stub-e12".to_string(),
        }
    }

    /// Create with custom settings.
    ///
    /// # Arguments
    ///
    /// * `token_dim` - Dimension per token (standard: 128)
    /// * `max_tokens` - Maximum number of tokens to generate
    pub fn with_settings(token_dim: usize, max_tokens: usize) -> Self {
        Self {
            token_dim,
            max_tokens,
            model_id: "stub-e12".to_string(),
        }
    }
}

impl Default for StubTokenEmbedder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TokenEmbedder for StubTokenEmbedder {
    fn token_dimension(&self) -> usize {
        self.token_dim
    }

    fn max_tokens(&self) -> usize {
        self.max_tokens
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }

    async fn embed_tokens(&self, content: &str) -> CoreResult<Vec<Vec<f32>>> {
        if content.is_empty() {
            return Ok(Vec::new());
        }

        let hash = content_hash(content);
        // Approximate tokenization: ~1 token per whitespace-separated word
        let word_count = content.split_whitespace().count().max(1);
        let num_tokens = word_count.min(self.max_tokens);

        let token_embeddings: Vec<Vec<f32>> = (0..num_tokens)
            .map(|t| {
                // Each token gets a unique hash using XOR with golden ratio constant
                // This ensures tokens have visibly different embeddings
                let token_hash = hash ^ (t as u64).wrapping_mul(GOLDEN_RATIO_PRIME);
                (0..self.token_dim)
                    .map(|d| hash_to_f32(token_hash, d))
                    .collect()
            })
            .collect();

        Ok(token_embeddings)
    }

    fn is_ready(&self) -> bool {
        true
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // StubSingleEmbedder Tests
    // ========================================================================

    #[tokio::test]
    async fn test_stub_single_embedder_dimension() {
        let e1 = StubSingleEmbedder::for_e1();
        assert_eq!(e1.dimension(), 1024);

        let e7 = StubSingleEmbedder::for_e7();
        assert_eq!(e7.dimension(), 1536);

        let e8 = StubSingleEmbedder::for_e8();
        assert_eq!(e8.dimension(), 384);
    }

    #[tokio::test]
    async fn test_stub_single_embedder_embed() {
        let embedder = StubSingleEmbedder::for_e1();
        let result = embedder.embed("test content").await;

        assert!(result.is_ok());
        let embedding = result.expect("should succeed");
        assert_eq!(embedding.len(), 1024);
    }

    #[tokio::test]
    async fn test_stub_single_embedder_deterministic() {
        let embedder = StubSingleEmbedder::for_e1();
        let content = "deterministic test content";

        let result1 = embedder.embed(content).await.expect("first embed");
        let result2 = embedder.embed(content).await.expect("second embed");

        assert_eq!(result1, result2, "Same content should produce same embedding");
    }

    #[tokio::test]
    async fn test_stub_single_embedder_different_content() {
        let embedder = StubSingleEmbedder::for_e1();

        let result1 = embedder.embed("content A").await.expect("embed A");
        let result2 = embedder.embed("content B").await.expect("embed B");

        assert_ne!(
            result1, result2,
            "Different content should produce different embeddings"
        );
    }

    #[tokio::test]
    async fn test_stub_single_embedder_empty_content() {
        let embedder = StubSingleEmbedder::for_e1();
        let result = embedder.embed("").await;

        assert!(result.is_ok());
        let embedding = result.expect("should succeed");
        assert_eq!(embedding.len(), 1024);
        // Empty content produces consistent hash-based output
    }

    #[tokio::test]
    async fn test_stub_single_embedder_is_ready() {
        let embedder = StubSingleEmbedder::for_e1();
        assert!(embedder.is_ready());
    }

    #[tokio::test]
    async fn test_stub_single_embedder_model_id() {
        let embedder = StubSingleEmbedder::for_e1();
        assert_eq!(embedder.model_id(), "stub-e1");

        let e7 = StubSingleEmbedder::for_e7();
        assert_eq!(e7.model_id(), "stub-e7");
    }

    #[tokio::test]
    async fn test_stub_single_embedder_all_types() {
        // Verify all factory methods work and produce correct dimensions
        assert_eq!(StubSingleEmbedder::for_e1().dimension(), 1024);
        assert_eq!(StubSingleEmbedder::for_e2().dimension(), 512);
        assert_eq!(StubSingleEmbedder::for_e3().dimension(), 512);
        assert_eq!(StubSingleEmbedder::for_e4().dimension(), 512);
        assert_eq!(StubSingleEmbedder::for_e5().dimension(), 768);
        assert_eq!(StubSingleEmbedder::for_e7().dimension(), 1536);
        assert_eq!(StubSingleEmbedder::for_e8().dimension(), 384);
        assert_eq!(StubSingleEmbedder::for_e9().dimension(), 1024);
        assert_eq!(StubSingleEmbedder::for_e10().dimension(), 768);
        assert_eq!(StubSingleEmbedder::for_e11().dimension(), 768); // KEPLER
    }

    // ========================================================================
    // StubSparseEmbedder Tests
    // ========================================================================

    #[tokio::test]
    async fn test_stub_sparse_embedder_vocab_size() {
        let e6 = StubSparseEmbedder::for_e6();
        assert_eq!(e6.vocab_size(), 30_522);

        let e13 = StubSparseEmbedder::for_e13();
        assert_eq!(e13.vocab_size(), 30_522);
    }

    #[tokio::test]
    async fn test_stub_sparse_embedder_embed() {
        let embedder = StubSparseEmbedder::for_e6();
        let result = embedder.embed_sparse("test content").await;

        assert!(result.is_ok());
        let sparse = result.expect("should succeed");
        assert!(sparse.nnz() > 0);
        // Indices should be within vocab bounds
        for &idx in &sparse.indices {
            assert!(
                (idx as usize) < 30_522,
                "Index {} out of bounds",
                idx
            );
        }
    }

    #[tokio::test]
    async fn test_stub_sparse_embedder_empty_content() {
        let embedder = StubSparseEmbedder::for_e6();
        let result = embedder.embed_sparse("").await;

        assert!(result.is_ok());
        let sparse = result.expect("should succeed");
        assert_eq!(sparse.nnz(), 0, "Empty content should produce empty sparse vector");
    }

    #[tokio::test]
    async fn test_stub_sparse_embedder_deterministic() {
        let embedder = StubSparseEmbedder::for_e6();
        let content = "deterministic sparse test";

        let result1 = embedder.embed_sparse(content).await.expect("first");
        let result2 = embedder.embed_sparse(content).await.expect("second");

        assert_eq!(result1.indices, result2.indices);
        assert_eq!(result1.values, result2.values);
    }

    #[tokio::test]
    async fn test_stub_sparse_embedder_sorted_indices() {
        let embedder = StubSparseEmbedder::for_e6();
        let result = embedder.embed_sparse("test for sorted indices").await;

        let sparse = result.expect("should succeed");
        // Verify indices are sorted
        for i in 1..sparse.indices.len() {
            assert!(
                sparse.indices[i] > sparse.indices[i - 1],
                "Indices must be sorted ascending"
            );
        }
    }

    #[tokio::test]
    async fn test_stub_sparse_embedder_positive_values() {
        let embedder = StubSparseEmbedder::for_e6();
        let result = embedder.embed_sparse("test positive values").await;

        let sparse = result.expect("should succeed");
        for &val in &sparse.values {
            assert!(val >= 0.0, "Sparse values should be non-negative");
        }
    }

    // ========================================================================
    // StubTokenEmbedder Tests
    // ========================================================================

    #[tokio::test]
    async fn test_stub_token_embedder_dimension() {
        let embedder = StubTokenEmbedder::new();
        assert_eq!(embedder.token_dimension(), 128);
        assert_eq!(embedder.max_tokens(), 512);
    }

    #[tokio::test]
    async fn test_stub_token_embedder_embed() {
        let embedder = StubTokenEmbedder::new();
        let result = embedder.embed_tokens("hello world").await;

        assert!(result.is_ok());
        let tokens = result.expect("should succeed");
        assert_eq!(tokens.len(), 2, "Two words should produce two tokens");
        assert_eq!(tokens[0].len(), 128, "Each token should be 128D");
        assert_eq!(tokens[1].len(), 128);
    }

    #[tokio::test]
    async fn test_stub_token_embedder_empty_content() {
        let embedder = StubTokenEmbedder::new();
        let result = embedder.embed_tokens("").await;

        assert!(result.is_ok());
        let tokens = result.expect("should succeed");
        assert!(tokens.is_empty(), "Empty content should produce no tokens");
    }

    #[tokio::test]
    async fn test_stub_token_embedder_deterministic() {
        let embedder = StubTokenEmbedder::new();
        let content = "deterministic token test";

        let result1 = embedder.embed_tokens(content).await.expect("first");
        let result2 = embedder.embed_tokens(content).await.expect("second");

        assert_eq!(result1, result2);
    }

    #[tokio::test]
    async fn test_stub_token_embedder_max_tokens() {
        let embedder = StubTokenEmbedder::with_settings(128, 5);
        // Many words, but limited to 5 tokens
        let content = "one two three four five six seven eight nine ten";
        let result = embedder.embed_tokens(content).await.expect("should succeed");

        assert_eq!(result.len(), 5, "Should be limited to max_tokens");
    }

    #[tokio::test]
    async fn test_stub_token_embedder_different_tokens() {
        let embedder = StubTokenEmbedder::new();
        let result = embedder.embed_tokens("hello world").await.expect("success");

        // Different tokens should have different embeddings
        assert_ne!(result[0], result[1], "Different tokens should differ");
    }

    #[test]
    fn test_stub_token_embedder_default() {
        let embedder = StubTokenEmbedder::default();
        assert_eq!(embedder.token_dimension(), 128);
        assert_eq!(embedder.max_tokens(), 512);
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    #[tokio::test]
    async fn test_all_stub_embedders_ready() {
        assert!(StubSingleEmbedder::for_e1().is_ready());
        assert!(StubSparseEmbedder::for_e6().is_ready());
        assert!(StubTokenEmbedder::new().is_ready());
    }

    #[tokio::test]
    async fn test_hash_distribution() {
        // Verify hash function produces reasonable distribution
        let embedder = StubSingleEmbedder::for_e1();
        let embedding = embedder.embed("test").await.expect("success");

        // Check values are in expected range [-0.5, 0.5]
        for &val in &embedding {
            assert!(
                val >= -0.5 && val <= 0.5,
                "Value {} out of expected range",
                val
            );
        }
    }
}
