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
//! | Float8E4M3 | IMPLEMENTED | Full roundtrip support (4x compression) |
//! | PQ8 | IMPLEMENTED | Full roundtrip support (32x compression) |
//! | SparseNative | INVALID PATH | Sparse models should not use dense quantization |
//! | TokenPruning | OUT OF SCOPE | Returns UnsupportedOperation |
//!
//! # Error Handling
//!
//! All errors include:
//! - ModelId context for debugging
//! - Clear error messages explaining what failed
//! - Logging via `tracing` crate for operational visibility

mod encoders;

#[cfg(test)]
mod tests;

use super::float8::Float8E4M3Encoder;
use super::pq8::PQ8Encoder;
use super::types::{BinaryEncoder, QuantizationMethod, QuantizedEmbedding};
use crate::error::EmbeddingError;
use crate::types::ModelId;
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

/// Router for quantization operations across all embedding types.
///
/// Delegates to the appropriate encoder based on ModelId and QuantizationMethod.
/// Per Constitution: NO fallback to float32 - every embedder MUST use its assigned compression.
#[derive(Debug)]
pub struct QuantizationRouter {
    /// Binary encoder for E9_HDC embeddings.
    binary_encoder: BinaryEncoder,
    /// Float8 E4M3 encoder for E2, E3, E4, E8, E11 embeddings.
    float8_encoder: Float8E4M3Encoder,
    /// PQ8 encoders for E1, E5, E7, E10, Kepler embeddings (keyed by dimension).
    pq8_encoders: HashMap<usize, PQ8Encoder>,
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
            "Initializing QuantizationRouter with Binary, Float8E4M3, and PQ8 encoders"
        );

        // Pre-create PQ8 encoders for known dimensions (per Constitution)
        let mut pq8_encoders = HashMap::new();
        // E1_Semantic: 1024D
        pq8_encoders.insert(1024, PQ8Encoder::new(1024));
        // E5_Causal: 768D
        pq8_encoders.insert(768, PQ8Encoder::new(768));
        // E7_Code: 1536D
        pq8_encoders.insert(1536, PQ8Encoder::new(1536));
        // E10_Contextual + Kepler: 768D (already created for E5)

        Self {
            binary_encoder: BinaryEncoder::new(),
            float8_encoder: Float8E4M3Encoder::new(),
            pq8_encoders,
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
            // Float8E4M3: Fully implemented
            QuantizationMethod::Float8E4M3 => true,
            // PQ8: Fully implemented
            QuantizationMethod::PQ8 => true,
            // SparseNative: Pass-through (no dense quantization needed)
            // Sparse models store indices+values directly, not via this router
            QuantizationMethod::SparseNative => false,
            // Not implemented yet
            QuantizationMethod::TokenPruning => false,
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
            QuantizationMethod::Binary => self.quantize_binary(model_id, embedding),
            QuantizationMethod::PQ8 => self.quantize_pq8(model_id, embedding),
            QuantizationMethod::Float8E4M3 => self.quantize_float8(model_id, embedding),
            QuantizationMethod::SparseNative => {
                warn!(
                    target: "quantization::router",
                    model_id = ?model_id,
                    "Sparse models should not use dense quantization path"
                );
                Err(EmbeddingError::InvalidModelInput {
                    model_id,
                    reason:
                        "Sparse models store indices+values directly, not via dense quantization"
                            .to_string(),
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
            QuantizationMethod::Binary => self.dequantize_binary(model_id, quantized),
            QuantizationMethod::PQ8 => self.dequantize_pq8(model_id, quantized),
            QuantizationMethod::Float8E4M3 => self.dequantize_float8(model_id, quantized),
            QuantizationMethod::SparseNative => {
                warn!(
                    target: "quantization::router",
                    model_id = ?model_id,
                    "Sparse models should not use dense dequantization path"
                );
                Err(EmbeddingError::InvalidModelInput {
                    model_id,
                    reason:
                        "Sparse models store indices+values directly, not via dense quantization"
                            .to_string(),
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
                original_dim.div_ceil(8)
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
}
