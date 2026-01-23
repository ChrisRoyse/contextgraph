//! Multi-Array Embedding Provider for 13-embedding SemanticFingerprint generation.
//!
//! This module defines the [`MultiArrayEmbeddingProvider`] trait that orchestrates
//! 13 individual embedders to produce a complete [`SemanticFingerprint`].
//!
//! # Design Philosophy
//!
//! **NO FUSION** - Each embedding space is preserved independently for:
//! 1. Per-space HNSW search (13 independent indexes)
//! 2. Per-space confidence classification
//! 3. Full information preservation (~46KB vs ~6KB fused)
//! 4. Auditability - trace which embedder contributed to ranking
//!
//! # Architecture
//!
//! ```text
//! MultiArrayEmbeddingProvider
//!     |-- calls 10 dense SingleEmbedder instances (E1-E5, E7-E11)
//!     |-- calls 2 SparseEmbedder instances (E6, E13 - SPLADE)
//!     |-- calls TokenEmbedder for E12 (ColBERT late-interaction)
//!
//!     Returns: SemanticFingerprint with all 13 embeddings
//! ```
//!
//! # Performance Targets (from constitution.yaml)
//!
//! - Single content embedding: <30ms for all 13 embeddings
//! - Batch embedding (64 items): <100ms total per item
//!
//! # 5-Stage Pipeline Integration
//!
//! - **Stage 1**: E13 SPLADE for initial recall (sparse retrieval)
//! - **Stage 2**: E1 Matryoshka 128D for fast dense filtering
//! - **Stage 3**: Full E1-E12 dense embeddings for precision
//! - **Stage 4**: E12 ColBERT for late interaction reranking
//! - **Stage 5**: Purpose vector from teleological computation
//!
//! # Example
//!
//! ```ignore
//! use context_graph_core::traits::{MultiArrayEmbeddingProvider, MultiArrayEmbeddingOutput};
//!
//! async fn generate_fingerprint<P: MultiArrayEmbeddingProvider>(
//!     provider: &P,
//!     content: &str,
//! ) -> MultiArrayEmbeddingOutput {
//!     let output = provider.embed_all(content).await.unwrap();
//!
//!     // Check performance
//!     assert!(output.is_within_latency_target());
//!
//!     // Access Stage 2 fast-filter embedding
//!     let e1_128 = output.e1_matryoshka_128();
//!     assert_eq!(e1_128.len(), 128);
//!
//!     output
//! }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

use crate::error::CoreResult;
use crate::types::fingerprint::{SemanticFingerprint, SparseVector, NUM_EMBEDDERS};

/// Output from multi-array embedding generation.
///
/// Contains the complete 13-embedding fingerprint plus performance metrics
/// for monitoring and optimization.
///
/// # Fields
///
/// - `fingerprint`: Complete 13-embedding [`SemanticFingerprint`] (E1-E13)
/// - `total_latency`: Wall-clock time for all 13 embeddings
/// - `per_embedder_latency`: Individual latency for each embedder (index 0 = E1)
/// - `model_ids`: Model identifiers used for each embedder slot
///
/// # Performance Monitoring
///
/// Use [`is_within_latency_target()`](Self::is_within_latency_target) to check
/// if the embedding met the 30ms performance target, and
/// [`slowest_embedder()`](Self::slowest_embedder) to identify optimization targets.
#[derive(Debug, Clone)]
pub struct MultiArrayEmbeddingOutput {
    /// Complete 13-embedding fingerprint (E1-E13).
    ///
    /// Contains all dense, sparse, and token-level embeddings
    /// without any fusion - each space preserved independently.
    pub fingerprint: SemanticFingerprint,

    /// Total wall-clock latency for generating all 13 embeddings.
    ///
    /// This includes parallel execution time and any coordination overhead.
    /// Target: <30ms (from constitution.yaml)
    pub total_latency: Duration,

    /// Per-embedder latencies for performance optimization.
    ///
    /// Index mapping:
    /// - 0: E1 Semantic (e5-large-v2)
    /// - 1: E2 Temporal-Recent
    /// - 2: E3 Temporal-Periodic
    /// - 3: E4 Temporal-Positional
    /// - 4: E5 Causal (Longformer)
    /// - 5: E6 Sparse (SPLADE)
    /// - 6: E7 Code (Qodo-Embed-1-1.5B)
    /// - 7: E8 Graph (MiniLM)
    /// - 8: E9 HDC
    /// - 9: E10 Multimodal (CLIP)
    /// - 10: E11 Entity (MiniLM)
    /// - 11: E12 Late-Interaction (ColBERT)
    /// - 12: E13 SPLADE v3
    pub per_embedder_latency: [Duration; NUM_EMBEDDERS],

    /// Model IDs used for each embedder slot.
    ///
    /// Useful for tracking which models generated embeddings
    /// and ensuring version consistency.
    pub model_ids: [String; NUM_EMBEDDERS],
}

impl MultiArrayEmbeddingOutput {
    /// Performance target: <30ms for all 13 embeddings.
    ///
    /// This target is derived from constitution.yaml requirements
    /// for real-time embedding generation.
    pub const TARGET_LATENCY_MS: u64 = 30;

    /// Check if total latency is within the 30ms target.
    ///
    /// # Returns
    ///
    /// `true` if `total_latency < 30ms`, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if output.is_within_latency_target() {
    ///     log::info!("Embedding generated within performance budget");
    /// } else {
    ///     log::warn!("Embedding exceeded 30ms target: {:?}", output.total_latency);
    /// }
    /// ```
    #[inline]
    pub fn is_within_latency_target(&self) -> bool {
        self.total_latency.as_millis() < Self::TARGET_LATENCY_MS as u128
    }

    /// Get E1 Matryoshka embedding truncated to 128D for Stage 2 fast filtering.
    ///
    /// Matryoshka embeddings are trained to be meaningful at multiple
    /// truncation points. The 128D prefix provides a fast approximation
    /// for initial candidate filtering before full similarity computation.
    ///
    /// # Returns
    ///
    /// A slice of the first 128 dimensions of the E1 semantic embedding.
    ///
    /// # Panics
    ///
    /// Panics if E1 embedding has fewer than 128 dimensions (should never
    /// happen with correctly initialized fingerprint - E1 is always 1024D).
    #[inline]
    pub fn e1_matryoshka_128(&self) -> &[f32] {
        &self.fingerprint.e1_semantic[..128]
    }

    /// Get the slowest embedder for optimization targeting.
    ///
    /// Identifies the embedder with the highest latency, which is
    /// typically the bottleneck for overall performance.
    ///
    /// # Returns
    ///
    /// A tuple of `(embedder_index, latency)` where `embedder_index`
    /// corresponds to E1 (0) through E13 (12).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let (idx, latency) = output.slowest_embedder();
    /// let name = SemanticFingerprint::embedding_name(idx).unwrap();
    /// log::info!("Bottleneck: {} took {:?}", name, latency);
    /// ```
    pub fn slowest_embedder(&self) -> (usize, Duration) {
        self.per_embedder_latency
            .iter()
            .enumerate()
            .max_by_key(|(_, d)| *d)
            .map(|(i, d)| (i, *d))
            .unwrap_or((0, Duration::ZERO))
    }

    /// Get the fastest embedder.
    ///
    /// Useful for understanding the performance distribution across embedders.
    pub fn fastest_embedder(&self) -> (usize, Duration) {
        self.per_embedder_latency
            .iter()
            .enumerate()
            .min_by_key(|(_, d)| *d)
            .map(|(i, d)| (i, *d))
            .unwrap_or((0, Duration::ZERO))
    }

    /// Calculate average latency across all embedders.
    pub fn average_embedder_latency(&self) -> Duration {
        let total_nanos: u128 = self.per_embedder_latency.iter().map(|d| d.as_nanos()).sum();
        Duration::from_nanos((total_nanos / NUM_EMBEDDERS as u128) as u64)
    }
}

/// Metadata for temporal embedding models (E2-E4).
///
/// Provides explicit context for temporal embedders rather than relying on
/// implicit time extraction. This is particularly important for E4 (V_ordering)
/// which should encode session sequence positions, not Unix timestamps.
///
/// # E4-FIX
///
/// This struct was added to fix E4 sequence embedding. Previously, E4 used
/// Unix timestamps (same as E2), making it a duplicate. With this metadata,
/// E4 now receives proper session sequence numbers.
///
/// # Usage
///
/// ```ignore
/// let metadata = EmbeddingMetadata {
///     session_id: Some("session-123".to_string()),
///     session_sequence: Some(42),  // 43rd memory in this session
///     timestamp: Some(Utc::now()),
/// };
///
/// let output = provider.embed_all_with_metadata(content, metadata).await?;
/// ```
#[derive(Debug, Clone, Default)]
pub struct EmbeddingMetadata {
    /// Session ID for session-scoped operations.
    ///
    /// Used to scope sequence numbers within a session.
    pub session_id: Option<String>,

    /// Session sequence number for E4 (V_ordering).
    ///
    /// Monotonically increasing within a session (0, 1, 2, ...).
    /// This is used by E4 instead of Unix timestamps to enable proper
    /// "before/after" queries within a session.
    pub session_sequence: Option<u64>,

    /// Explicit timestamp for E2 (V_freshness) and E3 (V_periodicity).
    ///
    /// If not provided, defaults to `Utc::now()`.
    pub timestamp: Option<DateTime<Utc>>,
}

impl EmbeddingMetadata {
    /// Create metadata with session sequence (preferred for E4).
    ///
    /// # Arguments
    /// * `session_id` - Session identifier
    /// * `sequence` - Sequence number within the session
    #[must_use]
    pub fn with_sequence(session_id: impl Into<String>, sequence: u64) -> Self {
        Self {
            session_id: Some(session_id.into()),
            session_sequence: Some(sequence),
            timestamp: Some(Utc::now()),
        }
    }

    /// Create metadata with explicit timestamp (for backward compatibility).
    #[must_use]
    pub fn with_timestamp(timestamp: DateTime<Utc>) -> Self {
        Self {
            session_id: None,
            session_sequence: None,
            timestamp: Some(timestamp),
        }
    }

    /// Format E4 instruction string for hybrid session+sequence mode.
    ///
    /// Returns "session:X sequence:N" if both session_id and session_sequence are set,
    /// enabling the E4 hybrid mode to generate session-specific signatures.
    /// Falls back to "sequence:N" or "epoch:N" for backward compatibility.
    ///
    /// # Output Examples
    ///
    /// - With session + sequence: `"session:abc123 sequence:42"`
    /// - With session + timestamp: `"session:abc123 epoch:1705315800"`
    /// - Without session (legacy): `"sequence:42"` or `"epoch:1705315800"`
    #[must_use]
    pub fn e4_instruction(&self) -> String {
        let mut parts = Vec::new();

        // Include session_id if present (for hybrid mode)
        if let Some(ref sess) = self.session_id {
            parts.push(format!("session:{}", sess));
        }

        // Include position (sequence or timestamp)
        if let Some(seq) = self.session_sequence {
            parts.push(format!("sequence:{}", seq));
        } else if let Some(ts) = self.timestamp {
            parts.push(format!("epoch:{}", ts.timestamp()));
        } else {
            parts.push(format!("epoch:{}", Utc::now().timestamp()));
        }

        parts.join(" ")
    }

    /// Format E2/E3 instruction string (always uses timestamp).
    #[must_use]
    pub fn temporal_instruction(&self) -> String {
        let ts = self.timestamp.unwrap_or_else(Utc::now);
        format!("epoch:{}", ts.timestamp())
    }
}

/// Multi-Array Embedding Provider trait.
///
/// Orchestrates 13 individual embedders to produce a complete [`SemanticFingerprint`].
/// This trait REPLACES the legacy single-vector `EmbeddingProvider` for all new code.
///
/// # Performance Targets (from constitution.yaml)
///
/// - Single content: <30ms for all 13 embeddings
/// - Batch (64 items): <100ms total per item
///
/// # Thread Safety
///
/// Requires `Send + Sync` for async task spawning across threads.
/// Implementations should parallelize embedding generation where possible.
///
/// # Object Safety
///
/// This trait is object-safe and can be used with `dyn MultiArrayEmbeddingProvider`.
///
/// # Example Implementation
///
/// ```ignore
/// use async_trait::async_trait;
/// use context_graph_core::traits::{
///     MultiArrayEmbeddingProvider, MultiArrayEmbeddingOutput
/// };
/// use context_graph_core::error::CoreResult;
///
/// struct ProductionProvider {
///     embedders: Vec<Box<dyn SingleEmbedder>>,
///     sparse_embedders: Vec<Box<dyn SparseEmbedder>>,
///     token_embedder: Box<dyn TokenEmbedder>,
/// }
///
/// #[async_trait]
/// impl MultiArrayEmbeddingProvider for ProductionProvider {
///     async fn embed_all(&self, content: &str) -> CoreResult<MultiArrayEmbeddingOutput> {
///         // Parallel execution of all 13 embedders
///         todo!()
///     }
///
///     // ... other methods
/// }
/// ```
#[async_trait]
pub trait MultiArrayEmbeddingProvider: Send + Sync {
    /// Generate complete 13-embedding fingerprint for content.
    ///
    /// Calls all 13 embedders (ideally in parallel) and combines results
    /// into a single [`SemanticFingerprint`].
    ///
    /// # Arguments
    ///
    /// * `content` - Text content to embed (must be non-empty)
    ///
    /// # Returns
    ///
    /// A [`MultiArrayEmbeddingOutput`] containing:
    /// - Complete 13-embedding fingerprint
    /// - Total and per-embedder latency metrics
    /// - Model IDs used
    ///
    /// # Errors
    ///
    /// Returns `CoreError` if:
    /// - Content is empty (`CoreError::ValidationError`)
    /// - Provider is not ready (`CoreError::Internal`)
    /// - Any embedder fails (propagated error)
    /// - Timeout exceeded
    ///
    /// # Performance
    ///
    /// Target latency: <30ms for all 13 embeddings (constitution.yaml)
    async fn embed_all(&self, content: &str) -> CoreResult<MultiArrayEmbeddingOutput>;

    /// Generate complete 13-embedding fingerprint with explicit metadata.
    ///
    /// This method allows passing explicit session sequence numbers for E4
    /// (V_ordering) embeddings, enabling proper "before/after" queries within
    /// a session.
    ///
    /// # Arguments
    ///
    /// * `content` - Text content to embed (must be non-empty)
    /// * `metadata` - Metadata for temporal embedders (E2-E4)
    ///
    /// # E4-FIX
    ///
    /// This method was added to fix E4 sequence embedding. The default
    /// implementation delegates to `embed_all()` for backward compatibility,
    /// but implementations should override this to use metadata.session_sequence
    /// for E4 embeddings.
    ///
    /// # Default Implementation
    ///
    /// Delegates to `embed_all()`, ignoring metadata. Override to use metadata.
    async fn embed_all_with_metadata(
        &self,
        content: &str,
        _metadata: EmbeddingMetadata,
    ) -> CoreResult<MultiArrayEmbeddingOutput> {
        // Default: ignore metadata and delegate to embed_all
        // Implementations should override to use metadata for E4
        self.embed_all(content).await
    }

    /// Generate fingerprints for multiple contents in batch.
    ///
    /// Batch processing is more efficient than individual calls,
    /// amortizing model loading and API overhead.
    ///
    /// # Arguments
    ///
    /// * `contents` - Slice of text content to embed
    ///
    /// # Returns
    ///
    /// A vector of [`MultiArrayEmbeddingOutput`] in the same order as input.
    ///
    /// # Performance Target
    ///
    /// 64 contents: <100ms per item average
    async fn embed_batch_all(
        &self,
        contents: &[String],
    ) -> CoreResult<Vec<MultiArrayEmbeddingOutput>>;

    /// Get expected dimensions for each embedder.
    ///
    /// Returns array where index matches embedder number:
    /// - 0: E1 (1024D semantic)
    /// - 1: E2 (512D temporal-recent)
    /// - ...
    /// - 5: E6 (0 - sparse variable)
    /// - ...
    /// - 11: E12 (128D per token)
    /// - 12: E13 (0 - sparse variable)
    ///
    /// Sparse embedders (E6, E13) return 0 since dimension is variable.
    ///
    /// # Default Implementation
    ///
    /// Returns the standard dimensions from the teleological architecture spec.
    fn dimensions(&self) -> [usize; NUM_EMBEDDERS] {
        [
            1024, // E1: Semantic (e5-large-v2)
            512,  // E2: Temporal-Recent
            512,  // E3: Temporal-Periodic
            512,  // E4: Temporal-Positional
            768,  // E5: Causal (Longformer)
            0,    // E6: Sparse (SPLADE) - variable
            1536, // E7: Code (Qodo-Embed-1-1.5B)
            384,  // E8: Graph (MiniLM)
            1024, // E9: HDC (projected)
            768,  // E10: Multimodal (CLIP)
            384,  // E11: Entity (MiniLM)
            128,  // E12: Late-Interaction (ColBERT per-token)
            0,    // E13: SPLADE v3 - variable
        ]
    }

    /// Get model IDs for each embedder slot.
    ///
    /// Used for tracking which models generated embeddings
    /// and ensuring version consistency across sessions.
    fn model_ids(&self) -> [&str; NUM_EMBEDDERS];

    /// Check if all 13 embedders are initialized and ready.
    ///
    /// # Returns
    ///
    /// `true` if all embedders have loaded their models and are
    /// ready to accept embedding requests.
    fn is_ready(&self) -> bool;

    /// Get health status for each embedder.
    ///
    /// Index mapping matches `dimensions()`:
    /// - 0: E1, 1: E2, ..., 12: E13
    ///
    /// # Returns
    ///
    /// Array of booleans indicating health status for each embedder.
    fn health_status(&self) -> [bool; NUM_EMBEDDERS];
}

/// Individual dense embedder trait for composition.
///
/// Wraps a single embedding model for use in [`MultiArrayEmbeddingProvider`].
/// Used for E1-E5, E7-E11 (10 dense embedders).
///
/// # Thread Safety
///
/// Requires `Send + Sync` for use in async contexts with work-stealing executors.
///
/// # Object Safety
///
/// This trait is object-safe and can be used with `dyn SingleEmbedder`.
///
/// # Example
///
/// ```ignore
/// struct NomicEmbedder {
///     model: Model,
///     dimension: usize,
/// }
///
/// #[async_trait]
/// impl SingleEmbedder for NomicEmbedder {
///     fn dimension(&self) -> usize { self.dimension }
///     fn model_id(&self) -> &str { "nomic-embed-v1.5" }
///     async fn embed(&self, content: &str) -> CoreResult<Vec<f32>> {
///         self.model.encode(content).await
///     }
///     fn is_ready(&self) -> bool { self.model.is_loaded() }
/// }
/// ```
#[async_trait]
pub trait SingleEmbedder: Send + Sync {
    /// Fixed embedding dimension for this model.
    ///
    /// All embeddings from this embedder will have exactly this dimension.
    fn dimension(&self) -> usize;

    /// Model identifier (e.g., "nomic-embed-v1.5", "e5-large-v2").
    ///
    /// Used for tracking and version consistency.
    fn model_id(&self) -> &str;

    /// Generate dense embedding vector.
    ///
    /// # Arguments
    ///
    /// * `content` - Text content to embed
    ///
    /// # Returns
    ///
    /// A dense f32 vector with length equal to `dimension()`.
    ///
    /// # Errors
    ///
    /// Returns `CoreError` if:
    /// - Content is empty
    /// - Model is not ready
    /// - Encoding fails
    async fn embed(&self, content: &str) -> CoreResult<Vec<f32>>;

    /// Check if model is loaded and ready.
    fn is_ready(&self) -> bool;
}

/// Sparse embedder trait for SPLADE-style embeddings.
///
/// Used for E6 (general sparse) and E13 (SPLADE v3 for Stage 1 recall).
/// Produces sparse vectors with vocabulary indices and activation values.
///
/// # Sparse Vector Format
///
/// Returns [`SparseVector`] with:
/// - `indices`: Sorted vocabulary indices (u16, max 30522)
/// - `values`: Activation values (f32) for each index
///
/// Typical sparsity: ~5% of vocabulary (1500 active indices).
///
/// # Thread Safety
///
/// Requires `Send + Sync` for async contexts.
///
/// # Object Safety
///
/// This trait is object-safe and can be used with `dyn SparseEmbedder`.
#[async_trait]
pub trait SparseEmbedder: Send + Sync {
    /// Vocabulary size for sparse vector indices.
    ///
    /// Standard SPLADE uses BERT vocabulary: 30,522 tokens.
    fn vocab_size(&self) -> usize;

    /// Model identifier (e.g., "splade-v3-doc", "splade-cocondenser").
    fn model_id(&self) -> &str;

    /// Generate sparse embedding with indices and values.
    ///
    /// # Arguments
    ///
    /// * `content` - Text content to embed
    ///
    /// # Returns
    ///
    /// A [`SparseVector`] with sorted indices and corresponding values.
    ///
    /// # Errors
    ///
    /// Returns `CoreError` if:
    /// - Content is empty
    /// - Model is not ready
    /// - Encoding fails
    async fn embed_sparse(&self, content: &str) -> CoreResult<SparseVector>;

    /// Check if model is loaded and ready.
    fn is_ready(&self) -> bool;
}

/// Token-level embedder trait for ColBERT-style embeddings.
///
/// Used for E12 - produces per-token embeddings for late interaction scoring.
/// Late interaction allows fine-grained matching between query and document tokens.
///
/// # ColBERT Architecture
///
/// ColBERT generates 128D embeddings for each token, enabling MaxSim scoring:
/// ```text
/// score = sum(max(q_i . d_j)) for all query tokens i
/// ```
///
/// # Thread Safety
///
/// Requires `Send + Sync` for async contexts.
///
/// # Object Safety
///
/// This trait is object-safe and can be used with `dyn TokenEmbedder`.
#[async_trait]
pub trait TokenEmbedder: Send + Sync {
    /// Dimension per token (E12 = 128D).
    ///
    /// Each token embedding will have exactly this dimension.
    fn token_dimension(&self) -> usize;

    /// Maximum tokens supported.
    ///
    /// Content will be truncated if it exceeds this limit.
    /// Typical values: 512 (BERT-base) or 256 (efficiency mode).
    fn max_tokens(&self) -> usize;

    /// Model identifier (e.g., "colbert-v2", "colbertv2.0").
    fn model_id(&self) -> &str;

    /// Generate per-token embeddings.
    ///
    /// # Arguments
    ///
    /// * `content` - Text content to tokenize and embed
    ///
    /// # Returns
    ///
    /// A vector of token embeddings, where each inner vector has
    /// length equal to `token_dimension()`.
    ///
    /// # Errors
    ///
    /// Returns `CoreError` if:
    /// - Content is empty
    /// - Model is not ready
    /// - Tokenization or encoding fails
    async fn embed_tokens(&self, content: &str) -> CoreResult<Vec<Vec<f32>>>;

    /// Check if model is loaded and ready.
    fn is_ready(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that latency target check works correctly for passing case.
    #[test]
    fn test_multi_array_output_within_latency_target() {
        let output = MultiArrayEmbeddingOutput {
            fingerprint: SemanticFingerprint::zeroed(),
            total_latency: Duration::from_millis(25), // Under 30ms
            per_embedder_latency: [Duration::from_millis(2); NUM_EMBEDDERS],
            model_ids: core::array::from_fn(|i| format!("model-e{}", i + 1)),
        };
        assert!(output.is_within_latency_target());
    }

    /// Test that latency target check works correctly for exceeding case.
    #[test]
    fn test_multi_array_output_exceeds_latency_target() {
        let output = MultiArrayEmbeddingOutput {
            fingerprint: SemanticFingerprint::zeroed(),
            total_latency: Duration::from_millis(50), // Over 30ms
            per_embedder_latency: [Duration::from_millis(4); NUM_EMBEDDERS],
            model_ids: core::array::from_fn(|i| format!("model-e{}", i + 1)),
        };
        assert!(!output.is_within_latency_target());
    }

    /// Test boundary case: exactly 30ms should NOT be within target (must be less than).
    #[test]
    fn test_multi_array_output_boundary_latency() {
        let output = MultiArrayEmbeddingOutput {
            fingerprint: SemanticFingerprint::zeroed(),
            total_latency: Duration::from_millis(30), // Exactly 30ms
            per_embedder_latency: [Duration::from_millis(2); NUM_EMBEDDERS],
            model_ids: core::array::from_fn(|_| String::new()),
        };
        // 30ms is NOT within target (must be strictly less than 30ms)
        assert!(!output.is_within_latency_target());
    }

    /// Test E1 Matryoshka truncation to 128D.
    #[test]
    fn test_e1_matryoshka_128_truncation() {
        let mut fp = SemanticFingerprint::zeroed();
        // Set known values in first 128 elements
        for i in 0..128 {
            fp.e1_semantic[i] = i as f32;
        }
        // Set different values beyond 128 to ensure we're truncating
        for i in 128..256 {
            fp.e1_semantic[i] = 999.0;
        }

        let output = MultiArrayEmbeddingOutput {
            fingerprint: fp,
            total_latency: Duration::ZERO,
            per_embedder_latency: [Duration::ZERO; NUM_EMBEDDERS],
            model_ids: core::array::from_fn(|_| String::new()),
        };

        let truncated = output.e1_matryoshka_128();
        assert_eq!(truncated.len(), 128);
        assert_eq!(truncated[0], 0.0);
        assert_eq!(truncated[127], 127.0);
        // Verify we got the first 128, not some other slice
        assert_eq!(truncated[64], 64.0);
    }

    /// Test that dimensions() returns correct values for all 13 embedders.
    #[test]
    fn test_dimensions_returns_correct_values() {
        // Create a mock implementation to test default
        struct MockProvider;

        #[async_trait]
        impl MultiArrayEmbeddingProvider for MockProvider {
            async fn embed_all(&self, _: &str) -> CoreResult<MultiArrayEmbeddingOutput> {
                unimplemented!()
            }
            async fn embed_batch_all(
                &self,
                _: &[String],
            ) -> CoreResult<Vec<MultiArrayEmbeddingOutput>> {
                unimplemented!()
            }
            fn model_ids(&self) -> [&str; NUM_EMBEDDERS] {
                [""; NUM_EMBEDDERS]
            }
            fn is_ready(&self) -> bool {
                true
            }
            fn health_status(&self) -> [bool; NUM_EMBEDDERS] {
                [true; NUM_EMBEDDERS]
            }
        }

        let dims = MockProvider.dimensions();

        // Verify all expected dimensions
        assert_eq!(dims[0], 1024, "E1 semantic should be 1024D");
        assert_eq!(dims[1], 512, "E2 temporal-recent should be 512D");
        assert_eq!(dims[2], 512, "E3 temporal-periodic should be 512D");
        assert_eq!(dims[3], 512, "E4 temporal-positional should be 512D");
        assert_eq!(dims[4], 768, "E5 causal should be 768D");
        assert_eq!(dims[5], 0, "E6 sparse should be 0 (variable)");
        assert_eq!(dims[6], 1536, "E7 code should be 1536D");
        assert_eq!(dims[7], 384, "E8 graph should be 384D");
        assert_eq!(dims[8], 1024, "E9 HDC should be 1024D (projected)");
        assert_eq!(dims[9], 768, "E10 multimodal should be 768D");
        assert_eq!(dims[10], 384, "E11 entity should be 384D");
        assert_eq!(
            dims[11], 128,
            "E12 late-interaction should be 128D per token"
        );
        assert_eq!(dims[12], 0, "E13 SPLADE should be 0 (variable)");
    }

    /// Test slowest_embedder() identification.
    #[test]
    fn test_slowest_embedder_identification() {
        let mut latencies = [Duration::from_millis(1); NUM_EMBEDDERS];
        latencies[8] = Duration::from_millis(10); // E9 HDC is slowest

        let output = MultiArrayEmbeddingOutput {
            fingerprint: SemanticFingerprint::zeroed(),
            total_latency: Duration::from_millis(20),
            per_embedder_latency: latencies,
            model_ids: core::array::from_fn(|_| String::new()),
        };

        let (idx, latency) = output.slowest_embedder();
        assert_eq!(idx, 8, "E9 should be identified as slowest");
        assert_eq!(latency, Duration::from_millis(10));
    }

    /// Test fastest_embedder() identification.
    #[test]
    fn test_fastest_embedder_identification() {
        let mut latencies = [Duration::from_millis(5); NUM_EMBEDDERS];
        latencies[6] = Duration::from_millis(1); // E7 Code is fastest

        let output = MultiArrayEmbeddingOutput {
            fingerprint: SemanticFingerprint::zeroed(),
            total_latency: Duration::from_millis(60),
            per_embedder_latency: latencies,
            model_ids: core::array::from_fn(|_| String::new()),
        };

        let (idx, latency) = output.fastest_embedder();
        assert_eq!(idx, 6, "E7 should be identified as fastest");
        assert_eq!(latency, Duration::from_millis(1));
    }

    /// Test average latency calculation.
    #[test]
    fn test_average_embedder_latency() {
        let latencies = [Duration::from_millis(2); NUM_EMBEDDERS]; // All 2ms

        let output = MultiArrayEmbeddingOutput {
            fingerprint: SemanticFingerprint::zeroed(),
            total_latency: Duration::from_millis(26),
            per_embedder_latency: latencies,
            model_ids: core::array::from_fn(|_| String::new()),
        };

        let avg = output.average_embedder_latency();
        assert_eq!(avg, Duration::from_millis(2));
    }

    /// Test that MultiArrayEmbeddingProvider trait is object-safe.
    /// This test verifies compilation - if it compiles, the trait is object-safe.
    #[test]
    fn test_multi_array_embedding_provider_object_safety() {
        fn _accepts_provider(_: &dyn MultiArrayEmbeddingProvider) {}
        // Compiles = object-safe
    }

    /// Test that SingleEmbedder trait is object-safe.
    #[test]
    fn test_single_embedder_object_safety() {
        fn _accepts_single(_: &dyn SingleEmbedder) {}
        // Compiles = object-safe
    }

    /// Test that SparseEmbedder trait is object-safe.
    #[test]
    fn test_sparse_embedder_object_safety() {
        fn _accepts_sparse(_: &dyn SparseEmbedder) {}
        // Compiles = object-safe
    }

    /// Test that TokenEmbedder trait is object-safe.
    #[test]
    fn test_token_embedder_object_safety() {
        fn _accepts_token(_: &dyn TokenEmbedder) {}
        // Compiles = object-safe
    }

    /// Test that NUM_EMBEDDERS constant is 13.
    #[test]
    fn test_num_embedders_is_13() {
        assert_eq!(NUM_EMBEDDERS, 13, "NUM_EMBEDDERS must be 13");
    }

    /// Test that SemanticFingerprint::zeroed() produces valid dimensions.
    #[test]
    fn test_zeroed_fingerprint_dimensions() {
        let fp = SemanticFingerprint::zeroed();
        assert_eq!(fp.e1_semantic.len(), 1024);
        assert_eq!(fp.e2_temporal_recent.len(), 512);
        assert_eq!(fp.e3_temporal_periodic.len(), 512);
        assert_eq!(fp.e4_temporal_positional.len(), 512);
        // E5 now uses dual vectors for asymmetric causal similarity
        assert_eq!(fp.e5_causal_as_cause.len(), 768);
        assert_eq!(fp.e5_causal_as_effect.len(), 768);
        assert!(fp.e5_causal.is_empty()); // Legacy field empty in new format
        assert!(fp.e6_sparse.is_empty()); // Sparse starts empty
        assert_eq!(fp.e7_code.len(), 1536);
        // E8 now uses dual vectors for asymmetric graph similarity
        assert_eq!(fp.e8_graph_as_source.len(), 384);
        assert_eq!(fp.e8_graph_as_target.len(), 384);
        assert!(fp.e8_graph.is_empty()); // Legacy field empty in new format
        assert_eq!(fp.e9_hdc.len(), 1024); // HDC projected
        // E10 now uses dual vectors for asymmetric intent/context similarity
        assert_eq!(fp.e10_multimodal_as_intent.len(), 768);
        assert_eq!(fp.e10_multimodal_as_context.len(), 768);
        assert!(fp.e10_multimodal.is_empty()); // Legacy field empty in new format
        assert_eq!(fp.e11_entity.len(), 384);
        assert!(fp.e12_late_interaction.is_empty()); // Token-level starts empty
        assert!(fp.e13_splade.is_empty()); // Sparse starts empty
    }

    /// Test TARGET_LATENCY_MS constant.
    #[test]
    fn test_target_latency_constant() {
        assert_eq!(
            MultiArrayEmbeddingOutput::TARGET_LATENCY_MS,
            30,
            "Target latency should be 30ms per constitution.yaml"
        );
    }

    /// Test model_ids array in output.
    #[test]
    fn test_model_ids_in_output() {
        let output = MultiArrayEmbeddingOutput {
            fingerprint: SemanticFingerprint::zeroed(),
            total_latency: Duration::ZERO,
            per_embedder_latency: [Duration::ZERO; NUM_EMBEDDERS],
            model_ids: [
                "e5-large-v2".to_string(),
                "exp-decay".to_string(),
                "fourier".to_string(),
                "sinusoidal-pe".to_string(),
                "longformer".to_string(),
                "splade".to_string(),
                "qodo-embed".to_string(),
                "minilm-graph".to_string(),
                "hdc-10k".to_string(),
                "clip".to_string(),
                "minilm-entity".to_string(),
                "colbert-v2".to_string(),
                "splade-v3".to_string(),
            ],
        };

        assert_eq!(output.model_ids[0], "e5-large-v2");
        assert_eq!(output.model_ids[11], "colbert-v2");
        assert_eq!(output.model_ids[12], "splade-v3");
        assert_eq!(output.model_ids.len(), NUM_EMBEDDERS);
    }

    // =========================================================================
    // E4 INSTRUCTION FORMAT TESTS (Critical for session clustering)
    // =========================================================================

    /// Test e4_instruction() with session_id and sequence (hybrid mode).
    ///
    /// This is the critical fix test - verifies session_id is NOT dropped.
    #[test]
    fn test_e4_instruction_with_session_and_sequence() {
        let metadata = EmbeddingMetadata::with_sequence("session-123", 42);
        let instruction = metadata.e4_instruction();

        assert!(
            instruction.contains("session:session-123"),
            "Should include session_id: {}",
            instruction
        );
        assert!(
            instruction.contains("sequence:42"),
            "Should include sequence: {}",
            instruction
        );
        assert_eq!(instruction, "session:session-123 sequence:42");
    }

    /// Test e4_instruction() without session_id (backward compatible legacy mode).
    #[test]
    fn test_e4_instruction_without_session() {
        let metadata = EmbeddingMetadata {
            session_id: None,
            session_sequence: Some(42),
            timestamp: None,
        };
        let instruction = metadata.e4_instruction();

        assert_eq!(instruction, "sequence:42");
        assert!(
            !instruction.contains("session:"),
            "Should NOT include session prefix when session_id is None"
        );
    }

    /// Test e4_instruction() with timestamp only (no session or sequence).
    #[test]
    fn test_e4_instruction_with_timestamp_only() {
        use chrono::TimeZone;
        let ts = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let expected_epoch = ts.timestamp(); // Get the actual epoch value
        let metadata = EmbeddingMetadata::with_timestamp(ts);
        let instruction = metadata.e4_instruction();

        assert!(instruction.starts_with("epoch:"), "Got: {}", instruction);
        assert!(
            !instruction.contains("session:"),
            "Should NOT include session prefix"
        );
        assert_eq!(instruction, format!("epoch:{}", expected_epoch));
    }

    /// Test e4_instruction() with session_id and timestamp (no sequence).
    #[test]
    fn test_e4_instruction_with_session_and_timestamp() {
        use chrono::TimeZone;
        let ts = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let expected_epoch = ts.timestamp(); // Get the actual epoch value
        let metadata = EmbeddingMetadata {
            session_id: Some("my-session".to_string()),
            session_sequence: None,
            timestamp: Some(ts),
        };
        let instruction = metadata.e4_instruction();

        assert!(
            instruction.contains("session:my-session"),
            "Got: {}",
            instruction
        );
        assert!(instruction.contains("epoch:"), "Got: {}", instruction);
        assert_eq!(
            instruction,
            format!("session:my-session epoch:{}", expected_epoch)
        );
    }

    /// Test e4_instruction() with UUID session_id (common real-world case).
    #[test]
    fn test_e4_instruction_with_uuid_session() {
        let uuid = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";
        let metadata = EmbeddingMetadata::with_sequence(uuid, 100);
        let instruction = metadata.e4_instruction();

        assert_eq!(
            instruction,
            "session:a1b2c3d4-e5f6-7890-abcd-ef1234567890 sequence:100"
        );
    }

    /// Test e4_instruction() with empty string session_id (edge case).
    #[test]
    fn test_e4_instruction_with_empty_session() {
        let metadata = EmbeddingMetadata::with_sequence("", 5);
        let instruction = metadata.e4_instruction();

        // Empty session_id is still included (it's Some(""))
        assert_eq!(instruction, "session: sequence:5");
    }
}
