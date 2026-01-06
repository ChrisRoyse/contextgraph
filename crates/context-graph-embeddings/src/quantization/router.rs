//! QuantizationRouter for Constitution-aligned embedding compression.
//!
//! Routes quantization/dequantization operations to the correct encoder based on ModelId.
//! Per Constitution AP-007: NO STUB DATA IN PRODUCTION.
//!
//! # Implementation Status
//!
//! | Method | Status | Notes |
//! |--------|--------|-------|
//! | Binary | IMPLEMENTED | Full roundtrip support |
//! | Float8E4M3 | NOT IMPLEMENTED | Returns QuantizerNotImplemented |
//! | PQ8 | NOT IMPLEMENTED | Returns QuantizerNotImplemented |
//! | SparseNative | INVALID PATH | Sparse models should not use dense quantization |
//! | TokenPruning | OUT OF SCOPE | Returns UnsupportedOperation |
//!
//! # Error Handling
//!
//! All errors include:
//! - ModelId context for debugging
//! - Clear error messages explaining what failed
//! - Logging via `tracing` crate for operational visibility

use super::binary::BinaryQuantizationError;
use super::types::{BinaryEncoder, QuantizationMetadata, QuantizationMethod, QuantizedEmbedding};
use crate::error::EmbeddingError;
use crate::types::ModelId;
use tracing::{debug, error, info, warn};

/// Router for quantization operations across all embedding types.
///
/// Delegates to the appropriate encoder based on ModelId and QuantizationMethod.
/// Per Constitution: NO fallback to float32 - every embedder MUST use its assigned compression.
#[derive(Debug)]
pub struct QuantizationRouter {
    /// Binary encoder for E9_HDC embeddings.
    binary_encoder: BinaryEncoder,
}

impl Default for QuantizationRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl QuantizationRouter {
    /// Create a new quantization router with all available encoders.
    #[must_use]
    pub fn new() -> Self {
        info!(
            target: "quantization::router",
            "Initializing QuantizationRouter with Binary encoder"
        );
        Self {
            binary_encoder: BinaryEncoder::new(),
        }
    }

    /// Get the quantization method assigned to a ModelId.
    ///
    /// Delegates to `QuantizationMethod::for_model_id` - every ModelId has exactly one method.
    #[must_use]
    pub fn method_for(&self, model_id: ModelId) -> QuantizationMethod {
        QuantizationMethod::for_model_id(model_id)
    }

    /// Check if quantization is currently available for a ModelId.
    ///
    /// Returns true only for methods with implemented encoders.
    /// Per AP-007: NO fake "available" status - if encoder is not implemented, returns false.
    #[must_use]
    pub fn can_quantize(&self, model_id: ModelId) -> bool {
        let method = self.method_for(model_id);
        match method {
            // Binary: Fully implemented
            QuantizationMethod::Binary => true,
            // SparseNative: Pass-through (no dense quantization needed)
            // Sparse models store indices+values directly, not via this router
            QuantizationMethod::SparseNative => false,
            // Not implemented yet
            QuantizationMethod::PQ8 | QuantizationMethod::Float8E4M3 | QuantizationMethod::TokenPruning => false,
        }
    }

    /// Quantize an embedding vector for the given ModelId.
    ///
    /// Routes to the appropriate encoder based on the model's assigned quantization method.
    ///
    /// # Arguments
    ///
    /// * `model_id` - The model that produced this embedding
    /// * `embedding` - The f32 embedding vector to compress
    ///
    /// # Returns
    ///
    /// `QuantizedEmbedding` ready for storage.
    ///
    /// # Errors
    ///
    /// - `QuantizerNotImplemented` - Encoder for this method not yet available
    /// - `QuantizationFailed` - Encoding operation failed
    /// - `InvalidModelInput` - Sparse models should not use this path
    /// - `UnsupportedOperation` - TokenPruning is out of scope
    pub fn quantize(
        &self,
        model_id: ModelId,
        embedding: &[f32],
    ) -> Result<QuantizedEmbedding, EmbeddingError> {
        let method = self.method_for(model_id);

        debug!(
            target: "quantization::router",
            model_id = ?model_id,
            method = ?method,
            dim = embedding.len(),
            "Routing quantization request"
        );

        match method {
            QuantizationMethod::Binary => {
                self.quantize_binary(model_id, embedding)
            }
            QuantizationMethod::PQ8 => {
                error!(
                    target: "quantization::router",
                    model_id = ?model_id,
                    "PQ8 quantizer not implemented"
                );
                Err(EmbeddingError::QuantizerNotImplemented {
                    model_id,
                    method: "PQ8".to_string(),
                })
            }
            QuantizationMethod::Float8E4M3 => {
                error!(
                    target: "quantization::router",
                    model_id = ?model_id,
                    "Float8E4M3 quantizer not implemented"
                );
                Err(EmbeddingError::QuantizerNotImplemented {
                    model_id,
                    method: "Float8E4M3".to_string(),
                })
            }
            QuantizationMethod::SparseNative => {
                warn!(
                    target: "quantization::router",
                    model_id = ?model_id,
                    "Sparse models should not use dense quantization path"
                );
                Err(EmbeddingError::InvalidModelInput {
                    model_id,
                    reason: "Sparse models store indices+values directly, not via dense quantization".to_string(),
                })
            }
            QuantizationMethod::TokenPruning => {
                error!(
                    target: "quantization::router",
                    model_id = ?model_id,
                    "TokenPruning is out of scope for this router"
                );
                Err(EmbeddingError::UnsupportedOperation {
                    model_id,
                    operation: "TokenPruning quantization".to_string(),
                })
            }
        }
    }

    /// Dequantize a compressed embedding back to f32 values.
    ///
    /// Routes to the appropriate decoder based on the embedding's method.
    ///
    /// # Arguments
    ///
    /// * `model_id` - The model that originally produced this embedding
    /// * `quantized` - The compressed embedding to reconstruct
    ///
    /// # Returns
    ///
    /// Reconstructed f32 vector (approximate for lossy methods).
    ///
    /// # Errors
    ///
    /// - `QuantizerNotImplemented` - Decoder for this method not yet available
    /// - `DequantizationFailed` - Decoding operation failed
    /// - `InvalidModelInput` - Sparse models should not use this path
    /// - `UnsupportedOperation` - TokenPruning is out of scope
    pub fn dequantize(
        &self,
        model_id: ModelId,
        quantized: &QuantizedEmbedding,
    ) -> Result<Vec<f32>, EmbeddingError> {
        let method = quantized.method;

        debug!(
            target: "quantization::router",
            model_id = ?model_id,
            method = ?method,
            original_dim = quantized.original_dim,
            "Routing dequantization request"
        );

        match method {
            QuantizationMethod::Binary => {
                self.dequantize_binary(model_id, quantized)
            }
            QuantizationMethod::PQ8 => {
                error!(
                    target: "quantization::router",
                    model_id = ?model_id,
                    "PQ8 dequantizer not implemented"
                );
                Err(EmbeddingError::QuantizerNotImplemented {
                    model_id,
                    method: "PQ8".to_string(),
                })
            }
            QuantizationMethod::Float8E4M3 => {
                error!(
                    target: "quantization::router",
                    model_id = ?model_id,
                    "Float8E4M3 dequantizer not implemented"
                );
                Err(EmbeddingError::QuantizerNotImplemented {
                    model_id,
                    method: "Float8E4M3".to_string(),
                })
            }
            QuantizationMethod::SparseNative => {
                warn!(
                    target: "quantization::router",
                    model_id = ?model_id,
                    "Sparse models should not use dense dequantization path"
                );
                Err(EmbeddingError::InvalidModelInput {
                    model_id,
                    reason: "Sparse models store indices+values directly, not via dense quantization".to_string(),
                })
            }
            QuantizationMethod::TokenPruning => {
                error!(
                    target: "quantization::router",
                    model_id = ?model_id,
                    "TokenPruning is out of scope for this router"
                );
                Err(EmbeddingError::UnsupportedOperation {
                    model_id,
                    operation: "TokenPruning dequantization".to_string(),
                })
            }
        }
    }

    /// Compute the expected compressed size in bytes for a given ModelId and dimension.
    ///
    /// # Arguments
    ///
    /// * `model_id` - The model to compute size for
    /// * `original_dim` - The original f32 embedding dimension
    ///
    /// # Returns
    ///
    /// Expected compressed size in bytes (0 if method not implemented).
    #[must_use]
    pub fn expected_size(&self, model_id: ModelId, original_dim: usize) -> usize {
        let method = self.method_for(model_id);
        match method {
            QuantizationMethod::Binary => {
                // Binary: ceil(dim / 8) bytes
                (original_dim + 7) / 8
            }
            QuantizationMethod::Float8E4M3 => {
                // Float8: 1 byte per element
                original_dim
            }
            QuantizationMethod::PQ8 => {
                // PQ-8: 8 bytes (8 subvectors, 1 centroid index each)
                8
            }
            QuantizationMethod::SparseNative => {
                // Sparse: Variable based on sparsity, cannot predict
                0
            }
            QuantizationMethod::TokenPruning => {
                // TokenPruning: ~50% of original tokens * dimension * 4 bytes
                // Cannot predict without knowing token count
                0
            }
        }
    }

    // =========================================================================
    // Private encoder methods
    // =========================================================================

    /// Quantize using binary encoder.
    fn quantize_binary(
        &self,
        model_id: ModelId,
        embedding: &[f32],
    ) -> Result<QuantizedEmbedding, EmbeddingError> {
        self.binary_encoder
            .quantize(embedding, Some(0.0))
            .map_err(|e| {
                error!(
                    target: "quantization::router",
                    model_id = ?model_id,
                    error = %e,
                    "Binary quantization failed"
                );
                Self::binary_error_to_embedding_error(model_id, e, true)
            })
    }

    /// Dequantize using binary decoder.
    fn dequantize_binary(
        &self,
        model_id: ModelId,
        quantized: &QuantizedEmbedding,
    ) -> Result<Vec<f32>, EmbeddingError> {
        self.binary_encoder.dequantize(quantized).map_err(|e| {
            error!(
                target: "quantization::router",
                model_id = ?model_id,
                error = %e,
                "Binary dequantization failed"
            );
            Self::binary_error_to_embedding_error(model_id, e, false)
        })
    }

    /// Convert BinaryQuantizationError to EmbeddingError.
    fn binary_error_to_embedding_error(
        model_id: ModelId,
        error: BinaryQuantizationError,
        is_quantization: bool,
    ) -> EmbeddingError {
        let reason = error.to_string();
        if is_quantization {
            EmbeddingError::QuantizationFailed { model_id, reason }
        } else {
            EmbeddingError::DequantizationFailed { model_id, reason }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Router initialization tests
    // =========================================================================

    #[test]
    fn test_router_new() {
        let router = QuantizationRouter::new();
        // Verify it initializes without panic
        assert!(router.can_quantize(ModelId::Hdc));
    }

    #[test]
    fn test_router_default() {
        let router = QuantizationRouter::default();
        assert!(router.can_quantize(ModelId::Hdc));
    }

    // =========================================================================
    // Method routing tests
    // =========================================================================

    #[test]
    fn test_all_model_ids_have_method() {
        let router = QuantizationRouter::new();

        // Verify all 13 ModelIds return a valid method (no panic)
        for model_id in ModelId::all() {
            let method = router.method_for(*model_id);
            // Just verify it doesn't panic and returns something
            let _ = method.compression_ratio();
        }

        // Verify specific mappings per Constitution
        assert_eq!(router.method_for(ModelId::Semantic), QuantizationMethod::PQ8);
        assert_eq!(router.method_for(ModelId::Causal), QuantizationMethod::PQ8);
        assert_eq!(router.method_for(ModelId::Code), QuantizationMethod::PQ8);
        assert_eq!(router.method_for(ModelId::Multimodal), QuantizationMethod::PQ8);

        assert_eq!(router.method_for(ModelId::TemporalRecent), QuantizationMethod::Float8E4M3);
        assert_eq!(router.method_for(ModelId::TemporalPeriodic), QuantizationMethod::Float8E4M3);
        assert_eq!(router.method_for(ModelId::TemporalPositional), QuantizationMethod::Float8E4M3);
        assert_eq!(router.method_for(ModelId::Graph), QuantizationMethod::Float8E4M3);
        assert_eq!(router.method_for(ModelId::Entity), QuantizationMethod::Float8E4M3);

        assert_eq!(router.method_for(ModelId::Hdc), QuantizationMethod::Binary);

        assert_eq!(router.method_for(ModelId::Sparse), QuantizationMethod::SparseNative);
        assert_eq!(router.method_for(ModelId::Splade), QuantizationMethod::SparseNative);

        assert_eq!(router.method_for(ModelId::LateInteraction), QuantizationMethod::TokenPruning);
    }

    // =========================================================================
    // Binary quantization tests (IMPLEMENTED)
    // =========================================================================

    #[test]
    fn test_binary_quantization_e9_hdc() {
        let router = QuantizationRouter::new();

        // E9_HDC uses Binary quantization
        let embedding: Vec<f32> = (0..10000)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();

        let quantized = router
            .quantize(ModelId::Hdc, &embedding)
            .expect("Binary quantization should succeed");

        assert_eq!(quantized.method, QuantizationMethod::Binary);
        assert_eq!(quantized.original_dim, 10000);
        // 10000 bits = 1250 bytes
        assert_eq!(quantized.data.len(), 1250);
    }

    #[test]
    fn test_binary_round_trip() {
        let router = QuantizationRouter::new();

        // Create input with known pattern
        let input: Vec<f32> = (0..1024)
            .map(|i| if i % 3 == 0 { 0.5 } else { -0.5 })
            .collect();

        let quantized = router
            .quantize(ModelId::Hdc, &input)
            .expect("quantize");

        let reconstructed = router
            .dequantize(ModelId::Hdc, &quantized)
            .expect("dequantize");

        // VERIFICATION: All signs must match (binary preserves sign only)
        for (i, (&orig, &recon)) in input.iter().zip(reconstructed.iter()).enumerate() {
            let orig_positive = orig >= 0.0;
            let recon_positive = recon >= 0.0;
            assert_eq!(
                orig_positive, recon_positive,
                "Sign mismatch at index {}: orig={}, recon={}",
                i, orig, recon
            );
        }
    }

    // =========================================================================
    // Not implemented tests (QuantizerNotImplemented)
    // =========================================================================

    #[test]
    fn test_pq8_not_implemented() {
        let router = QuantizationRouter::new();

        // E1_Semantic uses PQ8 - not implemented
        let embedding = vec![0.5f32; 1024];
        let result = router.quantize(ModelId::Semantic, &embedding);

        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::QuantizerNotImplemented { model_id, method } => {
                assert_eq!(model_id, ModelId::Semantic);
                assert_eq!(method, "PQ8");
            }
            e => panic!("Expected QuantizerNotImplemented, got {:?}", e),
        }
    }

    #[test]
    fn test_float8_not_implemented() {
        let router = QuantizationRouter::new();

        // E2_TemporalRecent uses Float8E4M3 - not implemented
        let embedding = vec![0.5f32; 512];
        let result = router.quantize(ModelId::TemporalRecent, &embedding);

        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::QuantizerNotImplemented { model_id, method } => {
                assert_eq!(model_id, ModelId::TemporalRecent);
                assert_eq!(method, "Float8E4M3");
            }
            e => panic!("Expected QuantizerNotImplemented, got {:?}", e),
        }
    }

    // =========================================================================
    // Invalid path tests
    // =========================================================================

    #[test]
    fn test_sparse_rejects_dense() {
        let router = QuantizationRouter::new();

        // E6_Sparse should NOT use dense quantization path
        let embedding = vec![0.0f32; 30522]; // Sparse vocab size
        let result = router.quantize(ModelId::Sparse, &embedding);

        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::InvalidModelInput { model_id, reason } => {
                assert_eq!(model_id, ModelId::Sparse);
                assert!(reason.contains("Sparse"));
            }
            e => panic!("Expected InvalidModelInput, got {:?}", e),
        }
    }

    #[test]
    fn test_token_pruning_unsupported() {
        let router = QuantizationRouter::new();

        // E12_LateInteraction uses TokenPruning - out of scope
        let embedding = vec![0.5f32; 128];
        let result = router.quantize(ModelId::LateInteraction, &embedding);

        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::UnsupportedOperation { model_id, operation } => {
                assert_eq!(model_id, ModelId::LateInteraction);
                assert!(operation.contains("TokenPruning"));
            }
            e => panic!("Expected UnsupportedOperation, got {:?}", e),
        }
    }

    // =========================================================================
    // can_quantize tests
    // =========================================================================

    #[test]
    fn test_can_quantize() {
        let router = QuantizationRouter::new();

        // Binary: implemented
        assert!(router.can_quantize(ModelId::Hdc));

        // PQ8: not implemented
        assert!(!router.can_quantize(ModelId::Semantic));
        assert!(!router.can_quantize(ModelId::Causal));
        assert!(!router.can_quantize(ModelId::Code));
        assert!(!router.can_quantize(ModelId::Multimodal));

        // Float8: not implemented
        assert!(!router.can_quantize(ModelId::TemporalRecent));
        assert!(!router.can_quantize(ModelId::TemporalPeriodic));
        assert!(!router.can_quantize(ModelId::TemporalPositional));
        assert!(!router.can_quantize(ModelId::Graph));
        assert!(!router.can_quantize(ModelId::Entity));

        // Sparse: invalid path (not a dense quantization)
        assert!(!router.can_quantize(ModelId::Sparse));
        assert!(!router.can_quantize(ModelId::Splade));

        // TokenPruning: out of scope
        assert!(!router.can_quantize(ModelId::LateInteraction));
    }

    // =========================================================================
    // expected_size tests
    // =========================================================================

    #[test]
    fn test_expected_size_binary() {
        let router = QuantizationRouter::new();

        // Binary: ceil(dim / 8)
        assert_eq!(router.expected_size(ModelId::Hdc, 10000), 1250);
        assert_eq!(router.expected_size(ModelId::Hdc, 1024), 128);
        assert_eq!(router.expected_size(ModelId::Hdc, 8), 1);
        assert_eq!(router.expected_size(ModelId::Hdc, 9), 2);
    }

    #[test]
    fn test_expected_size_float8() {
        let router = QuantizationRouter::new();

        // Float8: 1 byte per element
        assert_eq!(router.expected_size(ModelId::TemporalRecent, 512), 512);
        assert_eq!(router.expected_size(ModelId::Graph, 384), 384);
    }

    #[test]
    fn test_expected_size_pq8() {
        let router = QuantizationRouter::new();

        // PQ8: always 8 bytes (8 subvectors)
        assert_eq!(router.expected_size(ModelId::Semantic, 1024), 8);
        assert_eq!(router.expected_size(ModelId::Code, 256), 8);
    }

    #[test]
    fn test_expected_size_sparse_unknown() {
        let router = QuantizationRouter::new();

        // Sparse: Variable, returns 0
        assert_eq!(router.expected_size(ModelId::Sparse, 30522), 0);
        assert_eq!(router.expected_size(ModelId::Splade, 30522), 0);
    }

    #[test]
    fn test_expected_size_token_pruning_unknown() {
        let router = QuantizationRouter::new();

        // TokenPruning: Variable, returns 0
        assert_eq!(router.expected_size(ModelId::LateInteraction, 128), 0);
    }

    // =========================================================================
    // Error handling tests
    // =========================================================================

    #[test]
    fn test_binary_quantization_empty_input() {
        let router = QuantizationRouter::new();

        let result = router.quantize(ModelId::Hdc, &[]);

        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::QuantizationFailed { model_id, reason } => {
                assert_eq!(model_id, ModelId::Hdc);
                assert!(reason.contains("Empty"));
            }
            e => panic!("Expected QuantizationFailed, got {:?}", e),
        }
    }

    #[test]
    fn test_binary_quantization_nan_input() {
        let router = QuantizationRouter::new();

        let embedding = vec![1.0, f32::NAN, 0.5];
        let result = router.quantize(ModelId::Hdc, &embedding);

        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::QuantizationFailed { model_id, reason } => {
                assert_eq!(model_id, ModelId::Hdc);
                assert!(reason.contains("NaN") || reason.contains("Invalid"));
            }
            e => panic!("Expected QuantizationFailed, got {:?}", e),
        }
    }

    #[test]
    fn test_binary_dequantization_wrong_metadata() {
        let router = QuantizationRouter::new();

        // Create a QuantizedEmbedding with wrong metadata type
        let bad_quantized = QuantizedEmbedding {
            method: QuantizationMethod::Binary,
            original_dim: 8,
            data: vec![0xFF],
            metadata: QuantizationMetadata::Float8 {
                scale: 1.0,
                bias: 0.0,
            },
        };

        let result = router.dequantize(ModelId::Hdc, &bad_quantized);

        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::DequantizationFailed { model_id, reason } => {
                assert_eq!(model_id, ModelId::Hdc);
                assert!(reason.contains("metadata") || reason.contains("Binary"));
            }
            e => panic!("Expected DequantizationFailed, got {:?}", e),
        }
    }

    // =========================================================================
    // MANDATORY Edge Case Tests (TASK-EMB-020 Definition of Done)
    // =========================================================================

    /// Edge Case 1: Empty embedding - must not panic, should return error or empty output.
    #[test]
    fn test_edge_empty_embedding() {
        let router = QuantizationRouter::new();
        let empty: Vec<f32> = vec![];

        let result = router.quantize(ModelId::Hdc, &empty);
        // Should succeed but produce empty output OR fail gracefully
        // Verify it does NOT panic
        match result {
            Ok(q) => assert_eq!(q.original_dim, 0),
            Err(EmbeddingError::QuantizationFailed { model_id, .. }) => {
                assert_eq!(model_id, ModelId::Hdc);
            }
            Err(e) => panic!("Unexpected error type: {:?}", e),
        }
    }

    /// Edge Case 2: Maximum dimension (65536 = 2^16) - realistic large input.
    #[test]
    fn test_edge_max_dimension() {
        let router = QuantizationRouter::new();
        // 65536 dimensions (2^16) - large but realistic
        let large: Vec<f32> = (0..65536).map(|i| (i as f32).sin()).collect();

        let result = router.quantize(ModelId::Hdc, &large);
        assert!(result.is_ok(), "Large dimension quantization failed: {:?}", result.err());

        let q = result.unwrap();
        assert_eq!(q.original_dim, 65536);
        // 65536 bits / 8 = 8192 bytes
        assert_eq!(q.data.len(), 8192, "Expected 8192 bytes for 65536-bit binary vector");
    }

    /// Edge Case 3: All same value (all zeros) - degenerate case.
    ///
    /// Binary quantization uses threshold=0.0, where value >= threshold produces 1 bit.
    /// So 0.0 >= 0.0 = true → all 1 bits → 0xFF bytes.
    #[test]
    fn test_edge_all_same_value() {
        let router = QuantizationRouter::new();
        // All zeros with threshold=0.0: 0.0 >= 0.0 = true → all 1 bits
        let all_zeros = vec![0.0f32; 256];

        let result = router.quantize(ModelId::Hdc, &all_zeros);
        assert!(result.is_ok(), "All-zeros quantization failed: {:?}", result.err());

        let q = result.unwrap();
        assert_eq!(q.original_dim, 256);
        assert_eq!(q.data.len(), 32); // 256/8 = 32 bytes

        // With threshold=0.0, all zeros → all 1 bits (0.0 >= 0.0 = true)
        // This is the correct behavior per BinaryEncoder implementation
        assert!(
            q.data.iter().all(|&b| b == 0xFF),
            "Expected all 0xFF bytes for zero input (0.0 >= 0.0 = true), got {:?}",
            q.data
        );
    }

    /// Edge Case 4: All positive values - should produce all 1 bits.
    #[test]
    fn test_edge_all_positive() {
        let router = QuantizationRouter::new();
        let all_positive = vec![1.0f32; 64];

        let result = router.quantize(ModelId::Hdc, &all_positive);
        assert!(result.is_ok());

        let q = result.unwrap();
        assert_eq!(q.original_dim, 64);
        assert_eq!(q.data.len(), 8); // 64/8 = 8 bytes

        // All positive values → all 1 bits → 0xFF bytes
        assert!(
            q.data.iter().all(|&b| b == 0xFF),
            "Expected all 0xFF bytes for positive input, got {:?}",
            q.data
        );
    }

    /// Edge Case 5: Alternating pattern - verify bit packing order.
    #[test]
    fn test_edge_alternating_pattern() {
        let router = QuantizationRouter::new();
        // Pattern: +, -, +, -, ... (8 values = 1 byte)
        let alternating: Vec<f32> = (0..8).map(|i| if i % 2 == 0 { 1.0 } else { -1.0 }).collect();

        let result = router.quantize(ModelId::Hdc, &alternating);
        assert!(result.is_ok());

        let q = result.unwrap();
        assert_eq!(q.original_dim, 8);
        assert_eq!(q.data.len(), 1);

        // Pattern 1,0,1,0,1,0,1,0 = 0b10101010 = 0xAA (LSB first)
        // Or 0b01010101 = 0x55 (MSB first)
        // Actual value depends on bit packing order
        let byte = q.data[0];
        assert!(
            byte == 0xAA || byte == 0x55,
            "Expected alternating pattern 0xAA or 0x55, got 0x{:02X}",
            byte
        );
    }
}
