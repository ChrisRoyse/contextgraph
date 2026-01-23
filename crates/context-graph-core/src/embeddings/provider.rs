//! StubMultiArrayProvider implementation for testing.
//!
//! This module provides a concrete implementation of [`MultiArrayEmbeddingProvider`]
//! using stub embedders. Useful for testing the embedding pipeline without
//! requiring actual ML models.
//!
//! **NOT for production** - use real model implementations in production.
//!
//! # Architecture Reference
//!
//! From constitution.yaml (ARCH-01): "TeleologicalArray is atomic - store all 13 embeddings or nothing"
//! From constitution.yaml (ARCH-05): "All 13 embedders required - missing embedder is fatal error"
//! From constitution.yaml (AP-14): "No .unwrap() in library code"

use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::error::{CoreError, CoreResult};
use crate::traits::{
    MultiArrayEmbeddingOutput, MultiArrayEmbeddingProvider, SingleEmbedder, SparseEmbedder,
    TokenEmbedder,
};
use crate::types::fingerprint::{SemanticFingerprint, NUM_EMBEDDERS};

use super::stubs::{StubSingleEmbedder, StubSparseEmbedder, StubTokenEmbedder};

// ============================================================================
// Constants
// ============================================================================

/// Number of dense embedders (E1-E5, E7-E11).
const NUM_DENSE_EMBEDDERS: usize = 10;

/// Number of sparse embedders (E6, E13).
const NUM_SPARSE_EMBEDDERS: usize = 2;

/// Default timeout per individual embedder (5 seconds).
const DEFAULT_TIMEOUT_PER_EMBEDDER: Duration = Duration::from_secs(5);

/// Default total timeout for all embeddings (30 seconds).
const DEFAULT_TIMEOUT_TOTAL: Duration = Duration::from_secs(30);

// ============================================================================
// StubMultiArrayProvider
// ============================================================================

/// Stub implementation of [`MultiArrayEmbeddingProvider`] for testing.
///
/// Orchestrates 13 stub embedders to produce deterministic [`SemanticFingerprint`]
/// instances. Uses tokio::join! for parallel execution.
///
/// # Example
///
/// ```ignore
/// use context_graph_core::embeddings::provider::StubMultiArrayProvider;
/// use context_graph_core::traits::MultiArrayEmbeddingProvider;
///
/// #[tokio::main]
/// async fn main() {
///     let provider = StubMultiArrayProvider::new();
///     let output = provider.embed_all("test content").await.unwrap();
///     assert!(output.fingerprint.validate().is_ok());
/// }
/// ```
pub struct StubMultiArrayProvider {
    /// Dense embedders for E1-E5, E7-E11 (10 total).
    ///
    /// Index mapping:
    /// - 0: E1 (Semantic)
    /// - 1: E2 (TemporalRecent)
    /// - 2: E3 (TemporalPeriodic)
    /// - 3: E4 (TemporalPositional)
    /// - 4: E5 (Causal)
    /// - 5: E7 (Code)
    /// - 6: E8 (Emotional/Graph)
    /// - 7: E9 (Hdc)
    /// - 8: E10 (Multimodal)
    /// - 9: E11 (Entity)
    dense_embedders: [Arc<dyn SingleEmbedder>; NUM_DENSE_EMBEDDERS],

    /// Sparse embedders for E6, E13 (2 total).
    ///
    /// Index mapping:
    /// - 0: E6 (Sparse)
    /// - 1: E13 (KeywordSplade)
    sparse_embedders: [Arc<dyn SparseEmbedder>; NUM_SPARSE_EMBEDDERS],

    /// Token embedder for E12 (ColBERT late-interaction).
    token_embedder: Arc<dyn TokenEmbedder>,

    /// Timeout per individual embedder.
    ///
    /// Note: Timeout enforcement is not implemented in the stub provider.
    /// This field is retained for API compatibility with production providers
    /// that implement timeout-based cancellation.
    #[allow(dead_code)]
    timeout_per_embedder: Duration,

    /// Total timeout for all 13 embeddings.
    ///
    /// Note: Timeout enforcement is not implemented in the stub provider.
    /// This field is retained for API compatibility with production providers.
    #[allow(dead_code)]
    timeout_total: Duration,
}

impl StubMultiArrayProvider {
    /// Create a new provider with default stub embedders.
    ///
    /// All embedders are initialized and ready immediately.
    pub fn new() -> Self {
        Self {
            dense_embedders: [
                Arc::new(StubSingleEmbedder::for_e1()),
                Arc::new(StubSingleEmbedder::for_e2()),
                Arc::new(StubSingleEmbedder::for_e3()),
                Arc::new(StubSingleEmbedder::for_e4()),
                Arc::new(StubSingleEmbedder::for_e5()),
                Arc::new(StubSingleEmbedder::for_e7()),
                Arc::new(StubSingleEmbedder::for_e8()),
                Arc::new(StubSingleEmbedder::for_e9()),
                Arc::new(StubSingleEmbedder::for_e10()),
                Arc::new(StubSingleEmbedder::for_e11()),
            ],
            sparse_embedders: [
                Arc::new(StubSparseEmbedder::for_e6()),
                Arc::new(StubSparseEmbedder::for_e13()),
            ],
            token_embedder: Arc::new(StubTokenEmbedder::new()),
            timeout_per_embedder: DEFAULT_TIMEOUT_PER_EMBEDDER,
            timeout_total: DEFAULT_TIMEOUT_TOTAL,
        }
    }

    /// Create a provider with custom embedders.
    ///
    /// # Arguments
    ///
    /// * `dense_embedders` - 10 dense embedders in order: E1-E5, E7-E11
    /// * `sparse_embedders` - 2 sparse embedders: E6, E13
    /// * `token_embedder` - Token embedder for E12
    pub fn with_embedders(
        dense_embedders: [Arc<dyn SingleEmbedder>; NUM_DENSE_EMBEDDERS],
        sparse_embedders: [Arc<dyn SparseEmbedder>; NUM_SPARSE_EMBEDDERS],
        token_embedder: Arc<dyn TokenEmbedder>,
    ) -> Self {
        Self {
            dense_embedders,
            sparse_embedders,
            token_embedder,
            timeout_per_embedder: DEFAULT_TIMEOUT_PER_EMBEDDER,
            timeout_total: DEFAULT_TIMEOUT_TOTAL,
        }
    }

    /// Set custom timeouts.
    ///
    /// # Arguments
    ///
    /// * `per_embedder` - Timeout for each individual embedder
    /// * `total` - Total timeout for all 13 embeddings
    pub fn with_timeouts(mut self, per_embedder: Duration, total: Duration) -> Self {
        self.timeout_per_embedder = per_embedder;
        self.timeout_total = total;
        self
    }

    /// Embed content using all 13 embedders in parallel.
    ///
    /// Uses tokio::join! for true parallel execution. Returns individual
    /// latencies for each embedder plus total wall-clock time.
    async fn embed_all_parallel(
        &self,
        content: &str,
    ) -> CoreResult<(SemanticFingerprint, [Duration; NUM_EMBEDDERS])> {
        let start_total = Instant::now();

        // Create timing trackers for each embedder
        let mut latencies = [Duration::ZERO; NUM_EMBEDDERS];

        // Clone content for each embedder task
        let c = content.to_string();

        // Execute all embedders in parallel using tokio::join!
        // Dense embedders: E1-E5, E7-E11 (10 total)
        let (e1, e2, e3, e4, e5, e7, e8, e9, e10, e11) = {
            let (d0, d1, d2, d3, d4, d5, d6, d7, d8, d9) = tokio::join!(
                Self::timed_embed(&self.dense_embedders[0], &c), // E1
                Self::timed_embed(&self.dense_embedders[1], &c), // E2
                Self::timed_embed(&self.dense_embedders[2], &c), // E3
                Self::timed_embed(&self.dense_embedders[3], &c), // E4
                Self::timed_embed(&self.dense_embedders[4], &c), // E5
                Self::timed_embed(&self.dense_embedders[5], &c), // E7
                Self::timed_embed(&self.dense_embedders[6], &c), // E8
                Self::timed_embed(&self.dense_embedders[7], &c), // E9
                Self::timed_embed(&self.dense_embedders[8], &c), // E10
                Self::timed_embed(&self.dense_embedders[9], &c), // E11
            );
            (d0, d1, d2, d3, d4, d5, d6, d7, d8, d9)
        };

        // Sparse embedders: E6, E13 (2 total)
        let (e6, e13) = {
            let (s0, s1) = tokio::join!(
                Self::timed_sparse_embed(&self.sparse_embedders[0], &c), // E6
                Self::timed_sparse_embed(&self.sparse_embedders[1], &c), // E13
            );
            (s0, s1)
        };

        // Token embedder: E12
        let e12 = Self::timed_token_embed(&self.token_embedder, &c).await;

        // Collect results and latencies
        let (e1_vec, e1_lat) = e1?;
        let (e2_vec, e2_lat) = e2?;
        let (e3_vec, e3_lat) = e3?;
        let (e4_vec, e4_lat) = e4?;
        let (e5_vec, e5_lat) = e5?;
        let (e6_sparse, e6_lat) = e6?;
        let (e7_vec, e7_lat) = e7?;
        let (e8_vec, e8_lat) = e8?;
        let (e9_vec, e9_lat) = e9?;
        let (e10_vec, e10_lat) = e10?;
        let (e11_vec, e11_lat) = e11?;
        let (e12_tokens, e12_lat) = e12?;
        let (e13_sparse, e13_lat) = e13?;

        // Map latencies to embedder indices
        latencies[0] = e1_lat; // E1
        latencies[1] = e2_lat; // E2
        latencies[2] = e3_lat; // E3
        latencies[3] = e4_lat; // E4
        latencies[4] = e5_lat; // E5
        latencies[5] = e6_lat; // E6
        latencies[6] = e7_lat; // E7
        latencies[7] = e8_lat; // E8
        latencies[8] = e9_lat; // E9
        latencies[9] = e10_lat; // E10
        latencies[10] = e11_lat; // E11
        latencies[11] = e12_lat; // E12
        latencies[12] = e13_lat; // E13

        // Build fingerprint
        // For E5 and E8, we use the same vector for both roles in stub mode
        // (real models would produce different encodings via dual projections)
        let fingerprint = SemanticFingerprint {
            e1_semantic: e1_vec,
            e2_temporal_recent: e2_vec,
            e3_temporal_periodic: e3_vec,
            e4_temporal_positional: e4_vec,
            e5_causal_as_cause: e5_vec.clone(),
            e5_causal_as_effect: e5_vec,
            e5_causal: Vec::new(), // Empty - using new dual format
            e6_sparse,
            e7_code: e7_vec,
            e8_graph_as_source: e8_vec.clone(),
            e8_graph_as_target: e8_vec,
            e8_graph: Vec::new(), // Empty - using new dual format
            e9_hdc: e9_vec,
            e10_multimodal: e10_vec,
            e11_entity: e11_vec,
            e12_late_interaction: e12_tokens,
            e13_splade: e13_sparse,
        };

        // Validate fingerprint before returning (ARCH-01: atomic)
        fingerprint.validate().map_err(|e| {
            CoreError::ValidationError {
                field: "fingerprint".to_string(),
                message: format!("Validation failed after embedding: {}", e),
            }
        })?;

        // Verify total time
        let _total_elapsed = start_total.elapsed();

        Ok((fingerprint, latencies))
    }

    /// Timed wrapper for dense embedding.
    async fn timed_embed(
        embedder: &Arc<dyn SingleEmbedder>,
        content: &str,
    ) -> CoreResult<(Vec<f32>, Duration)> {
        let start = Instant::now();
        let result = embedder.embed(content).await?;
        Ok((result, start.elapsed()))
    }

    /// Timed wrapper for sparse embedding.
    async fn timed_sparse_embed(
        embedder: &Arc<dyn SparseEmbedder>,
        content: &str,
    ) -> CoreResult<(crate::types::fingerprint::SparseVector, Duration)> {
        let start = Instant::now();
        let result = embedder.embed_sparse(content).await?;
        Ok((result, start.elapsed()))
    }

    /// Timed wrapper for token embedding.
    async fn timed_token_embed(
        embedder: &Arc<dyn TokenEmbedder>,
        content: &str,
    ) -> CoreResult<(Vec<Vec<f32>>, Duration)> {
        let start = Instant::now();
        let result = embedder.embed_tokens(content).await?;
        Ok((result, start.elapsed()))
    }

    /// Collect model IDs from all embedders.
    fn collect_model_ids(&self) -> [String; NUM_EMBEDDERS] {
        [
            self.dense_embedders[0].model_id().to_string(), // E1
            self.dense_embedders[1].model_id().to_string(), // E2
            self.dense_embedders[2].model_id().to_string(), // E3
            self.dense_embedders[3].model_id().to_string(), // E4
            self.dense_embedders[4].model_id().to_string(), // E5
            self.sparse_embedders[0].model_id().to_string(), // E6
            self.dense_embedders[5].model_id().to_string(), // E7
            self.dense_embedders[6].model_id().to_string(), // E8
            self.dense_embedders[7].model_id().to_string(), // E9
            self.dense_embedders[8].model_id().to_string(), // E10
            self.dense_embedders[9].model_id().to_string(), // E11
            self.token_embedder.model_id().to_string(),     // E12
            self.sparse_embedders[1].model_id().to_string(), // E13
        ]
    }
}

impl Default for StubMultiArrayProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MultiArrayEmbeddingProvider for StubMultiArrayProvider {
    async fn embed_all(&self, content: &str) -> CoreResult<MultiArrayEmbeddingOutput> {
        // Validate input
        if content.is_empty() {
            return Err(CoreError::Embedding("Empty content provided".to_string()));
        }

        // Check readiness
        if !self.is_ready() {
            return Err(CoreError::Internal(
                "One or more embedders are not ready".to_string(),
            ));
        }

        let start = Instant::now();

        // Embed using all 13 embedders
        let (fingerprint, per_embedder_latency) = self.embed_all_parallel(content).await?;

        let total_latency = start.elapsed();

        Ok(MultiArrayEmbeddingOutput {
            fingerprint,
            total_latency,
            per_embedder_latency,
            model_ids: self.collect_model_ids(),
        })
    }

    async fn embed_batch_all(
        &self,
        contents: &[String],
    ) -> CoreResult<Vec<MultiArrayEmbeddingOutput>> {
        // Validate input
        if contents.is_empty() {
            return Ok(Vec::new());
        }

        // Process each content item
        // Note: In a production implementation, this would batch across embedders
        // For stub implementation, we process sequentially for simplicity
        let mut results = Vec::with_capacity(contents.len());
        for content in contents {
            let output = self.embed_all(content).await?;
            results.push(output);
        }

        Ok(results)
    }

    fn model_ids(&self) -> [&str; NUM_EMBEDDERS] {
        [
            self.dense_embedders[0].model_id(), // E1
            self.dense_embedders[1].model_id(), // E2
            self.dense_embedders[2].model_id(), // E3
            self.dense_embedders[3].model_id(), // E4
            self.dense_embedders[4].model_id(), // E5
            self.sparse_embedders[0].model_id(), // E6
            self.dense_embedders[5].model_id(), // E7
            self.dense_embedders[6].model_id(), // E8
            self.dense_embedders[7].model_id(), // E9
            self.dense_embedders[8].model_id(), // E10
            self.dense_embedders[9].model_id(), // E11
            self.token_embedder.model_id(),     // E12
            self.sparse_embedders[1].model_id(), // E13
        ]
    }

    fn is_ready(&self) -> bool {
        self.dense_embedders.iter().all(|e| e.is_ready())
            && self.sparse_embedders.iter().all(|e| e.is_ready())
            && self.token_embedder.is_ready()
    }

    fn health_status(&self) -> [bool; NUM_EMBEDDERS] {
        [
            self.dense_embedders[0].is_ready(), // E1
            self.dense_embedders[1].is_ready(), // E2
            self.dense_embedders[2].is_ready(), // E3
            self.dense_embedders[3].is_ready(), // E4
            self.dense_embedders[4].is_ready(), // E5
            self.sparse_embedders[0].is_ready(), // E6
            self.dense_embedders[5].is_ready(), // E7
            self.dense_embedders[6].is_ready(), // E8
            self.dense_embedders[7].is_ready(), // E9
            self.dense_embedders[8].is_ready(), // E10
            self.dense_embedders[9].is_ready(), // E11
            self.token_embedder.is_ready(),     // E12
            self.sparse_embedders[1].is_ready(), // E13
        ]
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Construction Tests
    // ========================================================================

    #[test]
    fn test_provider_creation() {
        let provider = StubMultiArrayProvider::new();
        assert!(provider.is_ready());
    }

    #[test]
    fn test_provider_default() {
        let provider = StubMultiArrayProvider::default();
        assert!(provider.is_ready());
    }

    #[test]
    fn test_provider_health_status() {
        let provider = StubMultiArrayProvider::new();
        let status = provider.health_status();
        assert_eq!(status.len(), NUM_EMBEDDERS);
        assert!(status.iter().all(|&s| s));
    }

    #[test]
    fn test_provider_model_ids() {
        let provider = StubMultiArrayProvider::new();
        let ids = provider.model_ids();
        assert_eq!(ids.len(), NUM_EMBEDDERS);

        // Verify expected model ID format
        assert_eq!(ids[0], "stub-e1");
        assert_eq!(ids[5], "stub-e6");
        assert_eq!(ids[11], "stub-e12");
        assert_eq!(ids[12], "stub-e13");
    }

    // ========================================================================
    // Embedding Tests
    // ========================================================================

    #[tokio::test]
    async fn test_embed_all_basic() {
        let provider = StubMultiArrayProvider::new();
        let result = provider.embed_all("test content").await;

        assert!(result.is_ok());
        let output = result.expect("should succeed");
        assert!(output.fingerprint.validate().is_ok());
    }

    #[tokio::test]
    async fn test_embed_all_dimensions() {
        let provider = StubMultiArrayProvider::new();
        let output = provider.embed_all("hello world").await.expect("success");
        let fp = &output.fingerprint;

        // Verify all dimensions
        assert_eq!(fp.e1_semantic.len(), 1024, "E1 should be 1024D");
        assert_eq!(fp.e2_temporal_recent.len(), 512, "E2 should be 512D");
        assert_eq!(fp.e3_temporal_periodic.len(), 512, "E3 should be 512D");
        assert_eq!(fp.e4_temporal_positional.len(), 512, "E4 should be 512D");
        // E5 now uses dual vectors for asymmetric causal similarity
        assert_eq!(fp.e5_causal_as_cause.len(), 768, "E5 cause should be 768D");
        assert_eq!(fp.e5_causal_as_effect.len(), 768, "E5 effect should be 768D");
        assert!(fp.e5_causal.is_empty(), "E5 legacy should be empty in new format");
        // E6 is sparse - check nnz instead
        assert!(fp.e6_sparse.nnz() > 0, "E6 should have active indices");
        assert_eq!(fp.e7_code.len(), 1536, "E7 should be 1536D");
        assert_eq!(fp.e8_graph.len(), 384, "E8 should be 384D");
        assert_eq!(fp.e9_hdc.len(), 1024, "E9 should be 1024D");
        assert_eq!(fp.e10_multimodal.len(), 768, "E10 should be 768D");
        assert_eq!(fp.e11_entity.len(), 384, "E11 should be 384D");
        // E12 should have tokens
        assert!(!fp.e12_late_interaction.is_empty(), "E12 should have tokens");
        for token in &fp.e12_late_interaction {
            assert_eq!(token.len(), 128, "E12 tokens should be 128D");
        }
        // E13 is sparse
        assert!(fp.e13_splade.nnz() > 0, "E13 should have active indices");
    }

    #[tokio::test]
    async fn test_embed_all_deterministic() {
        let provider = StubMultiArrayProvider::new();
        let content = "deterministic test content";

        let output1 = provider.embed_all(content).await.expect("first");
        let output2 = provider.embed_all(content).await.expect("second");

        assert_eq!(
            output1.fingerprint, output2.fingerprint,
            "Same content should produce same fingerprint"
        );
    }

    #[tokio::test]
    async fn test_embed_all_different_content() {
        let provider = StubMultiArrayProvider::new();

        let output1 = provider.embed_all("content A").await.expect("first");
        let output2 = provider.embed_all("content B").await.expect("second");

        assert_ne!(
            output1.fingerprint.e1_semantic, output2.fingerprint.e1_semantic,
            "Different content should produce different E1"
        );
    }

    #[tokio::test]
    async fn test_embed_all_empty_content_fails() {
        let provider = StubMultiArrayProvider::new();
        let result = provider.embed_all("").await;

        assert!(result.is_err(), "Empty content should fail");
        match result {
            Err(CoreError::Embedding(msg)) => {
                assert!(msg.contains("Empty"), "Error should mention empty: {}", msg);
            }
            other => panic!("Expected Embedding error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_embed_all_latency_tracking() {
        let provider = StubMultiArrayProvider::new();
        let output = provider.embed_all("test latency").await.expect("success");

        // Total latency should be tracked
        assert!(
            output.total_latency > Duration::ZERO,
            "Total latency should be positive"
        );

        // Per-embedder latencies should all be present
        assert_eq!(output.per_embedder_latency.len(), NUM_EMBEDDERS);
    }

    // ========================================================================
    // Batch Tests
    // ========================================================================

    #[tokio::test]
    async fn test_embed_batch_empty() {
        let provider = StubMultiArrayProvider::new();
        let result = provider.embed_batch_all(&[]).await;

        assert!(result.is_ok());
        assert!(result.expect("success").is_empty());
    }

    #[tokio::test]
    async fn test_embed_batch_multiple() {
        let provider = StubMultiArrayProvider::new();
        let contents = vec![
            "first content".to_string(),
            "second content".to_string(),
            "third content".to_string(),
        ];
        let result = provider.embed_batch_all(&contents).await;

        assert!(result.is_ok());
        let outputs = result.expect("success");
        assert_eq!(outputs.len(), 3);

        // Each should have valid fingerprint
        for output in &outputs {
            assert!(output.fingerprint.validate().is_ok());
        }
    }

    #[tokio::test]
    async fn test_embed_batch_deterministic() {
        let provider = StubMultiArrayProvider::new();
        let contents = vec!["a".to_string(), "b".to_string()];

        let batch1 = provider.embed_batch_all(&contents).await.expect("first");
        let batch2 = provider.embed_batch_all(&contents).await.expect("second");

        assert_eq!(batch1.len(), batch2.len());
        for (o1, o2) in batch1.iter().zip(batch2.iter()) {
            assert_eq!(o1.fingerprint, o2.fingerprint);
        }
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    #[tokio::test]
    async fn test_fingerprint_validation_after_embed() {
        let provider = StubMultiArrayProvider::new();
        let output = provider
            .embed_all("validation test")
            .await
            .expect("success");

        // Fingerprint should pass validation
        let validation = output.fingerprint.validate();
        assert!(
            validation.is_ok(),
            "Fingerprint should be valid: {:?}",
            validation
        );

        // Also check validate_all
        let all_validation = output.fingerprint.validate_all();
        assert!(
            all_validation.is_ok(),
            "validate_all should pass: {:?}",
            all_validation
        );
    }

    #[tokio::test]
    async fn test_model_ids_in_output() {
        let provider = StubMultiArrayProvider::new();
        let output = provider.embed_all("test model ids").await.expect("success");

        // Model IDs should match provider's model_ids()
        let expected_ids = provider.model_ids();
        for (i, (actual, expected)) in output.model_ids.iter().zip(expected_ids.iter()).enumerate()
        {
            assert_eq!(
                actual, *expected,
                "Model ID mismatch at index {}: {} != {}",
                i, actual, expected
            );
        }
    }

    #[tokio::test]
    async fn test_fingerprint_storage_size() {
        let provider = StubMultiArrayProvider::new();
        let output = provider.embed_all("storage test").await.expect("success");

        // Storage should be reasonable
        let size = output.fingerprint.storage_size();
        assert!(
            size > 0,
            "Storage size should be positive"
        );

        // Should be less than 100KB for typical content
        assert!(
            size < 100_000,
            "Storage size {} should be under 100KB",
            size
        );
    }

    #[test]
    fn test_provider_with_timeouts() {
        let provider = StubMultiArrayProvider::new()
            .with_timeouts(Duration::from_secs(10), Duration::from_secs(60));

        assert!(provider.is_ready());
    }

    #[tokio::test]
    async fn test_e1_matryoshka_truncation() {
        let provider = StubMultiArrayProvider::new();
        let output = provider.embed_all("matryoshka test").await.expect("success");

        // Test Matryoshka 128D truncation helper
        let truncated = output.e1_matryoshka_128();
        assert_eq!(truncated.len(), 128);

        // Should be first 128 elements of E1
        for i in 0..128 {
            assert_eq!(truncated[i], output.fingerprint.e1_semantic[i]);
        }
    }

    #[tokio::test]
    async fn test_slowest_embedder_identification() {
        let provider = StubMultiArrayProvider::new();
        let output = provider.embed_all("slowest test").await.expect("success");

        let (idx, latency) = output.slowest_embedder();
        assert!(idx < NUM_EMBEDDERS);
        assert!(latency >= Duration::ZERO);
    }

    #[tokio::test]
    async fn test_average_latency_calculation() {
        let provider = StubMultiArrayProvider::new();
        let output = provider.embed_all("average test").await.expect("success");

        let avg = output.average_embedder_latency();
        assert!(avg >= Duration::ZERO);
    }

    // ========================================================================
    // Edge Case Tests (Manual Testing with Synthetic Data)
    // ========================================================================

    #[tokio::test]
    async fn test_edge_case_single_character() {
        let provider = StubMultiArrayProvider::new();
        let output = provider.embed_all("a").await.expect("success");

        // Single character should still produce valid fingerprint
        assert!(output.fingerprint.validate().is_ok());
        assert_eq!(output.fingerprint.e1_semantic.len(), 1024);
        // E12 with single char should have 1 token
        assert_eq!(output.fingerprint.e12_late_interaction.len(), 1);
    }

    #[tokio::test]
    async fn test_edge_case_long_content() {
        let provider = StubMultiArrayProvider::new();
        // Very long content (1000 words)
        let words: Vec<&str> = (0..1000).map(|_| "word").collect();
        let content = words.join(" ");
        let output = provider.embed_all(&content).await.expect("success");

        assert!(output.fingerprint.validate().is_ok());
        // E12 should be capped at max_tokens (512)
        assert!(output.fingerprint.e12_late_interaction.len() <= 512);
    }

    #[tokio::test]
    async fn test_edge_case_unicode_content() {
        let provider = StubMultiArrayProvider::new();
        let output = provider
            .embed_all("こんにちは世界 🚀 émojis äöü 中文")
            .await
            .expect("success");

        assert!(output.fingerprint.validate().is_ok());
        // Unicode should not break the embedder
        assert!(!output.fingerprint.e1_semantic.is_empty());
    }

    #[tokio::test]
    async fn test_edge_case_whitespace_only() {
        let provider = StubMultiArrayProvider::new();
        // Whitespace-only should not be treated as empty
        // (split_whitespace().count() is 0, but content.is_empty() is false)
        let result = provider.embed_all("   \t\n  ").await;

        // This should succeed - whitespace is valid content
        // The implementation treats it as non-empty content
        assert!(result.is_ok());
        let output = result.expect("success");
        // E12 with only whitespace:
        // - split_whitespace().count() is 0
        // - But we do .max(1) so num_tokens = 1
        // This tests that minimal content is handled gracefully
        assert!(output.fingerprint.validate().is_ok());
        // Due to .max(1), we get 1 token even for whitespace-only
        assert_eq!(output.fingerprint.e12_late_interaction.len(), 1);
    }

    #[tokio::test]
    async fn test_edge_case_sparse_bounds() {
        let provider = StubMultiArrayProvider::new();
        let output = provider.embed_all("sparse bounds test").await.expect("success");

        // All sparse indices must be < 30522 (vocab size)
        for &idx in &output.fingerprint.e6_sparse.indices {
            assert!(
                (idx as usize) < 30_522,
                "E6 index {} out of bounds",
                idx
            );
        }
        for &idx in &output.fingerprint.e13_splade.indices {
            assert!(
                (idx as usize) < 30_522,
                "E13 index {} out of bounds",
                idx
            );
        }
    }

    #[tokio::test]
    async fn test_edge_case_determinism_across_providers() {
        // Same content should produce same fingerprint across different provider instances
        let provider1 = StubMultiArrayProvider::new();
        let provider2 = StubMultiArrayProvider::new();
        let content = "determinism across providers";

        let output1 = provider1.embed_all(content).await.expect("provider1");
        let output2 = provider2.embed_all(content).await.expect("provider2");

        assert_eq!(
            output1.fingerprint, output2.fingerprint,
            "Different provider instances should produce identical fingerprints"
        );
    }

    #[tokio::test]
    async fn test_edge_case_batch_with_empty_strings() {
        let provider = StubMultiArrayProvider::new();
        // Mixed content with empty string in the middle
        let contents = vec![
            "first".to_string(),
            "".to_string(), // Empty - should fail the whole batch
            "third".to_string(),
        ];
        let result = provider.embed_batch_all(&contents).await;

        // Batch should fail because one content is empty
        assert!(result.is_err(), "Batch with empty string should fail");
    }

    #[tokio::test]
    async fn test_edge_case_e12_token_dimensions() {
        let provider = StubMultiArrayProvider::new();
        let output = provider.embed_all("token dimension check test").await.expect("success");

        // Every token in E12 must be exactly 128D
        for (i, token) in output.fingerprint.e12_late_interaction.iter().enumerate() {
            assert_eq!(
                token.len(),
                128,
                "Token {} has dimension {} instead of 128",
                i,
                token.len()
            );
        }
    }

    #[tokio::test]
    async fn test_edge_case_values_in_range() {
        let provider = StubMultiArrayProvider::new();
        let output = provider.embed_all("value range check").await.expect("success");

        // Dense embeddings should have values in [-0.5, 0.5] range
        for (i, val) in output.fingerprint.e1_semantic.iter().enumerate() {
            assert!(
                *val >= -0.5 && *val <= 0.5,
                "E1[{}] = {} out of range [-0.5, 0.5]",
                i,
                val
            );
            assert!(val.is_finite(), "E1[{}] = {} is not finite", i, val);
        }

        // Sparse values should be non-negative
        for (i, val) in output.fingerprint.e6_sparse.values.iter().enumerate() {
            assert!(*val >= 0.0, "E6 sparse value[{}] = {} is negative", i, val);
            assert!(val.is_finite(), "E6 sparse value[{}] is not finite", i);
        }
    }
}
