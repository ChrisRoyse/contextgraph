//! Core quantization types aligned with Constitution.
//!
//! CRITICAL: These are DATA STRUCTURES ONLY. The actual quantization/dequantization
//! logic is implemented in Logic Layer tasks (TASK-EMB-016, 017, 018).

use crate::types::ModelId;
use serde::{Deserialize, Serialize};

/// Quantization methods aligned with Constitution `embeddings.quantization`.
///
/// # Constitution Alignment
/// - PQ_8: E1, E5, E7, E10 (32x compression, <5% recall impact)
/// - Float8: E2, E3, E4, E8, E11 (4x compression, <0.3% recall impact)
/// - Binary: E9 (32x compression, 5-10% recall impact)
/// - Sparse: E6, E13 (native format, 0% recall impact)
/// - TokenPruning: E12 (~50% compression, <2% recall impact)
///
/// # CRITICAL INVARIANT
/// Every embedder MUST use its assigned quantization method.
/// There is NO fallback to uncompressed float32 storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationMethod {
    /// Product Quantization with 8 subvectors, 256 centroids each.
    /// Used for: E1_Semantic, E5_Causal, E7_Code, E10_Multimodal
    /// Compression: 32x (e.g., 1024D f32 -> 8 bytes)
    PQ8,

    /// 8-bit floating point in E4M3 format (4-bit exponent, 3-bit mantissa).
    /// Used for: E2_TemporalRecent, E3_TemporalPeriodic, E4_TemporalPositional,
    ///           E8_Graph, E11_Entity
    /// Compression: 4x
    Float8E4M3,

    /// Binary quantization (sign bit only).
    /// Used for: E9_HDC (Hyperdimensional Computing)
    /// Compression: 32x
    Binary,

    /// Sparse format: indices + values for non-zero elements.
    /// Used for: E6_Sparse, E13_SPLADE
    /// Compression: native (depends on sparsity)
    SparseNative,

    /// Token pruning: keep top 50% tokens by importance score.
    /// Used for: E12_LateInteraction
    /// Compression: ~50%
    TokenPruning,
}

impl QuantizationMethod {
    /// Get quantization method for a given ModelId.
    ///
    /// This mapping is defined in Constitution `embeddings.quantization_by_embedder`.
    /// Every ModelId has exactly one assigned method - no exceptions.
    #[must_use]
    pub const fn for_model_id(model_id: ModelId) -> Self {
        match model_id {
            // PQ-8: Dense semantic embeddings
            ModelId::Semantic => Self::PQ8,   // E1
            ModelId::Causal => Self::PQ8,     // E5
            ModelId::Code => Self::PQ8,       // E7
            ModelId::Contextual => Self::PQ8, // E10

            // Float8: Temporal and graph embeddings
            ModelId::TemporalRecent => Self::Float8E4M3, // E2
            ModelId::TemporalPeriodic => Self::Float8E4M3, // E3
            ModelId::TemporalPositional => Self::Float8E4M3, // E4
            ModelId::Graph => Self::Float8E4M3,          // E8
            ModelId::Entity => Self::Float8E4M3,         // E11 (deprecated)
            ModelId::Kepler => Self::PQ8,                // E11 (new KEPLER, 768D like Causal)

            // Binary: Hyperdimensional computing
            ModelId::Hdc => Self::Binary, // E9

            // Sparse: Sparse vector formats
            ModelId::Sparse => Self::SparseNative, // E6
            ModelId::Splade => Self::SparseNative, // E13

            // Token pruning: Late interaction
            ModelId::LateInteraction => Self::TokenPruning, // E12
        }
    }

    /// Theoretical compression ratio from Constitution.
    #[must_use]
    pub const fn compression_ratio(&self) -> f32 {
        match self {
            Self::PQ8 => 32.0,
            Self::Float8E4M3 => 4.0,
            Self::Binary => 32.0,
            Self::SparseNative => 1.0, // Variable, depends on sparsity
            Self::TokenPruning => 2.0, // ~50%
        }
    }

    /// Maximum acceptable recall loss from Constitution.
    #[must_use]
    pub const fn max_recall_loss(&self) -> f32 {
        match self {
            Self::PQ8 => 0.05,          // <5%
            Self::Float8E4M3 => 0.003,  // <0.3%
            Self::Binary => 0.10,       // 5-10%
            Self::SparseNative => 0.0,  // 0%
            Self::TokenPruning => 0.02, // <2%
        }
    }
}

/// Quantized embedding ready for storage.
///
/// # Invariants
/// - `method` MUST match the encoding format of `data`
/// - `original_dim` allows validation during dequantization
/// - `data` contains compressed bytes (format depends on `method`)
///
/// # CRITICAL
/// The `data` bytes are NOT directly comparable for similarity.
/// You MUST dequantize before computing cosine similarity (except Binary).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizedEmbedding {
    /// Quantization method used (required for dequantization dispatch).
    pub method: QuantizationMethod,

    /// Original embedding dimension before quantization.
    pub original_dim: usize,

    /// Compressed embedding bytes. Format depends on `method`.
    pub data: Vec<u8>,

    /// Method-specific metadata for reconstruction.
    pub metadata: QuantizationMetadata,
}

impl QuantizedEmbedding {
    /// Compute compressed size in bytes.
    #[must_use]
    pub fn size_bytes(&self) -> usize {
        self.data.len()
    }

    /// Compute actual compression ratio vs float32 storage.
    #[must_use]
    pub fn compression_ratio(&self) -> f32 {
        let original_bytes = self.original_dim * 4; // f32 = 4 bytes
        original_bytes as f32 / self.data.len().max(1) as f32
    }
}

/// Method-specific metadata required for dequantization.
///
/// Each variant contains the parameters needed to reconstruct
/// an approximate f32 embedding from the compressed `data` bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantizationMetadata {
    /// PQ-8: Codebook identifier and subvector count.
    PQ8 {
        /// Codebook ID (for looking up trained centroids).
        codebook_id: u32,
        /// Number of subvectors (typically 8).
        num_subvectors: u8,
    },

    /// Float8: Scale and bias for denormalization.
    /// Reconstruction: original = quantized * scale + bias
    Float8 {
        /// Scaling factor.
        scale: f32,
        /// Bias offset.
        bias: f32,
    },

    /// Binary: Threshold used for binarization.
    /// Bit is 1 if original value >= threshold, else 0.
    Binary {
        /// Binarization threshold.
        threshold: f32,
    },

    /// Sparse: Vocabulary size and non-zero count.
    Sparse {
        /// Total vocabulary dimension.
        vocab_size: usize,
        /// Number of non-zero entries stored.
        nnz: usize,
    },

    /// Token pruning: Original vs kept token counts.
    TokenPruning {
        /// Original number of tokens.
        original_tokens: usize,
        /// Tokens kept after pruning (top 50% by importance).
        kept_tokens: usize,
        /// Importance threshold used for pruning.
        threshold: f32,
    },
}

/// PQ-8 codebook with 8 subvectors, 256 centroids each.
///
/// # Algorithm
/// 1. Split embedding into 8 subvectors of dimension D/8
/// 2. Each subvector quantized to nearest of 256 centroids
/// 3. Store 8 centroid indices (1 byte each) = 8 bytes total
///
/// # Note
/// This is a DATA STRUCTURE. The actual quantize/dequantize methods
/// are implemented in Logic Layer (TASK-EMB-016).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PQ8Codebook {
    /// Embedding dimension this codebook was trained for.
    pub embedding_dim: usize,

    /// Number of subvectors (typically 8).
    pub num_subvectors: usize,

    /// Centroids per subvector (typically 256).
    pub num_centroids: usize,

    /// Centroid vectors: [num_subvectors][num_centroids][subvector_dim]
    /// Shape example: [8][256][128] for 1024D embedding
    pub centroids: Vec<Vec<Vec<f32>>>,

    /// Unique codebook identifier (stored in QuantizationMetadata).
    pub codebook_id: u32,
}

/// Float8 E4M3 encoder (stateless).
///
/// E4M3 format: 1 sign bit, 4 exponent bits (bias 7), 3 mantissa bits.
/// Range: ~1.5e-5 to 448 (positive values).
///
/// # Note
/// This is a marker struct. Actual encoding logic is in Logic Layer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Float8Encoder;

/// Binary encoder (stateless).
///
/// Converts f32 values to single bits based on sign threshold.
///
/// # Note
/// This is a marker struct. Actual encoding logic is in Logic Layer.
#[derive(Debug, Clone, Copy, Default)]
pub struct BinaryEncoder;

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify QuantizationMethod returns correct method for all 13 embedders.
    #[test]
    fn test_method_for_all_model_ids() {
        // PQ-8 embedders (E1, E5, E7, E10)
        assert_eq!(
            QuantizationMethod::for_model_id(ModelId::Semantic),
            QuantizationMethod::PQ8
        );
        assert_eq!(
            QuantizationMethod::for_model_id(ModelId::Causal),
            QuantizationMethod::PQ8
        );
        assert_eq!(
            QuantizationMethod::for_model_id(ModelId::Code),
            QuantizationMethod::PQ8
        );
        assert_eq!(
            QuantizationMethod::for_model_id(ModelId::Contextual),
            QuantizationMethod::PQ8
        );

        // Float8 embedders (E2, E3, E4, E8, E11)
        assert_eq!(
            QuantizationMethod::for_model_id(ModelId::TemporalRecent),
            QuantizationMethod::Float8E4M3
        );
        assert_eq!(
            QuantizationMethod::for_model_id(ModelId::TemporalPeriodic),
            QuantizationMethod::Float8E4M3
        );
        assert_eq!(
            QuantizationMethod::for_model_id(ModelId::TemporalPositional),
            QuantizationMethod::Float8E4M3
        );
        assert_eq!(
            QuantizationMethod::for_model_id(ModelId::Graph),
            QuantizationMethod::Float8E4M3
        );
        assert_eq!(
            QuantizationMethod::for_model_id(ModelId::Entity),
            QuantizationMethod::Float8E4M3
        );

        // Binary embedder (E9)
        assert_eq!(
            QuantizationMethod::for_model_id(ModelId::Hdc),
            QuantizationMethod::Binary
        );

        // Sparse embedders (E6, E13)
        assert_eq!(
            QuantizationMethod::for_model_id(ModelId::Sparse),
            QuantizationMethod::SparseNative
        );
        assert_eq!(
            QuantizationMethod::for_model_id(ModelId::Splade),
            QuantizationMethod::SparseNative
        );

        // Token pruning embedder (E12)
        assert_eq!(
            QuantizationMethod::for_model_id(ModelId::LateInteraction),
            QuantizationMethod::TokenPruning
        );
    }

    /// Verify compression ratios match Constitution.
    #[test]
    fn test_compression_ratios() {
        assert_eq!(QuantizationMethod::PQ8.compression_ratio(), 32.0);
        assert_eq!(QuantizationMethod::Float8E4M3.compression_ratio(), 4.0);
        assert_eq!(QuantizationMethod::Binary.compression_ratio(), 32.0);
        assert_eq!(QuantizationMethod::SparseNative.compression_ratio(), 1.0);
        assert_eq!(QuantizationMethod::TokenPruning.compression_ratio(), 2.0);
    }

    /// Verify max recall loss values match Constitution.
    #[test]
    fn test_max_recall_loss() {
        assert_eq!(QuantizationMethod::PQ8.max_recall_loss(), 0.05);
        assert_eq!(QuantizationMethod::Float8E4M3.max_recall_loss(), 0.003);
        assert_eq!(QuantizationMethod::Binary.max_recall_loss(), 0.10);
        assert_eq!(QuantizationMethod::SparseNative.max_recall_loss(), 0.0);
        assert_eq!(QuantizationMethod::TokenPruning.max_recall_loss(), 0.02);
    }

    /// Verify QuantizedEmbedding size calculation.
    #[test]
    fn test_quantized_embedding_size() {
        let qe = QuantizedEmbedding {
            method: QuantizationMethod::PQ8,
            original_dim: 1024,
            data: vec![0u8; 8], // 8 bytes for PQ-8
            metadata: QuantizationMetadata::PQ8 {
                codebook_id: 1,
                num_subvectors: 8,
            },
        };

        assert_eq!(qe.size_bytes(), 8);
        // 1024 * 4 = 4096 bytes original, 8 bytes compressed = 512x
        assert!(qe.compression_ratio() > 500.0);
    }

    /// Verify serde serialization roundtrip works.
    #[test]
    fn test_serde_roundtrip() {
        let methods = [
            QuantizationMethod::PQ8,
            QuantizationMethod::Float8E4M3,
            QuantizationMethod::Binary,
            QuantizationMethod::SparseNative,
            QuantizationMethod::TokenPruning,
        ];

        for method in methods {
            let json = serde_json::to_string(&method).expect("serialize");
            let restored: QuantizationMethod = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(restored, method);
        }
    }

    /// Verify QuantizationMetadata serialization.
    #[test]
    fn test_metadata_serde() {
        let metadata = QuantizationMetadata::Float8 {
            scale: 0.5,
            bias: -1.0,
        };

        let json = serde_json::to_string(&metadata).expect("serialize");
        let restored: QuantizationMetadata = serde_json::from_str(&json).expect("deserialize");

        match restored {
            QuantizationMetadata::Float8 { scale, bias } => {
                assert!((scale - 0.5).abs() < f32::EPSILON);
                assert!((bias - (-1.0)).abs() < f32::EPSILON);
            }
            _ => panic!("Wrong metadata variant"),
        }
    }

    /// Verify all 14 ModelId variants are covered by iterating through ModelId::all().
    #[test]
    fn test_all_model_ids_covered() {
        let all_models = ModelId::all();
        assert_eq!(all_models.len(), 14, "Expected 14 ModelId variants");

        // Verify each has a quantization method (no panic)
        for model_id in all_models {
            let method = QuantizationMethod::for_model_id(*model_id);
            // Just verify it returns without panic
            let _ = method.compression_ratio();
            let _ = method.max_recall_loss();
        }
    }

    /// Edge case: Empty data vector (valid for all-zero sparse vectors).
    #[test]
    fn test_empty_data_vector() {
        let qe = QuantizedEmbedding {
            method: QuantizationMethod::SparseNative,
            original_dim: 30522,
            data: vec![], // Empty for all-zero sparse vector
            metadata: QuantizationMetadata::Sparse {
                vocab_size: 30522,
                nnz: 0,
            },
        };
        assert_eq!(qe.size_bytes(), 0);
        // compression_ratio() should handle this gracefully (divides by max(1, len))
        let ratio = qe.compression_ratio();
        assert!(ratio > 0.0, "Compression ratio should handle empty data");
    }

    /// Edge case: Large dimension handling without overflow.
    #[test]
    fn test_large_dimension_no_overflow() {
        let qe = QuantizedEmbedding {
            method: QuantizationMethod::Float8E4M3,
            original_dim: 1_000_000, // Large but reasonable
            data: vec![0u8; 1000],
            metadata: QuantizationMetadata::Float8 {
                scale: 1.0,
                bias: 0.0,
            },
        };
        // Should not panic
        let ratio = qe.compression_ratio();
        assert!(ratio > 0.0);
    }
}
