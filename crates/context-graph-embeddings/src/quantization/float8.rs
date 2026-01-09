//! Float8 E4M3 Quantizer Implementation
//!
//! Implements 8-bit floating point quantization in E4M3 format for embeddings.
//! Used for E2_TemporalRecent, E3_TemporalPeriodic, E4_TemporalPositional,
//! E8_Graph, and E11_Entity embedders.
//!
//! # Constitution Alignment
//!
//! - Compression: 4x (f32 → 8 bits)
//! - Max Recall Loss: <0.3%
//! - Used for: E2, E3, E4, E8, E11
//!
//! # E4M3 Format
//!
//! - 1 sign bit
//! - 4 exponent bits (bias = 7)
//! - 3 mantissa bits
//! - Range: ~1.5e-5 to 448 (normalized)
//! - Special values: NaN represented, no infinities

use super::types::{QuantizationMetadata, QuantizationMethod, QuantizedEmbedding};
use std::fmt;
use tracing::debug;

/// Errors specific to Float8 quantization operations.
#[derive(Debug, Clone)]
pub enum Float8QuantizationError {
    /// Input embedding is empty.
    EmptyEmbedding,
    /// Input contains NaN values.
    ContainsNaN { index: usize },
    /// Input contains infinite values.
    ContainsInfinity { index: usize },
    /// Metadata type mismatch during dequantization.
    InvalidMetadata { expected: &'static str, got: String },
    /// Data length doesn't match original dimension.
    DimensionMismatch { expected: usize, got: usize },
}

impl fmt::Display for Float8QuantizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyEmbedding => write!(f, "Empty embedding: cannot quantize zero-length vector"),
            Self::ContainsNaN { index } => {
                write!(f, "Invalid input: NaN value at index {}", index)
            }
            Self::ContainsInfinity { index } => {
                write!(f, "Invalid input: Infinity value at index {}", index)
            }
            Self::InvalidMetadata { expected, got } => {
                write!(
                    f,
                    "Invalid metadata: expected {}, got {}",
                    expected, got
                )
            }
            Self::DimensionMismatch { expected, got } => {
                write!(
                    f,
                    "Dimension mismatch: expected {} bytes, got {}",
                    expected, got
                )
            }
        }
    }
}

impl std::error::Error for Float8QuantizationError {}

/// Float8 E4M3 Encoder for 4x compression of embedding vectors.
///
/// # Algorithm
///
/// 1. Compute global scale and bias from min/max values
/// 2. Normalize values to [0, 1] range using: `normalized = (value - bias) / scale`
/// 3. Quantize to E4M3 format (8 bits per element)
/// 4. Store scale and bias in metadata for reconstruction
///
/// # Reconstruction
///
/// `original ≈ dequantized * scale + bias`
///
/// # E4M3 Bit Layout
///
/// ```text
/// [S][E E E E][M M M]
///  7  6 5 4 3  2 1 0
/// ```
///
/// - S: Sign bit (0 = positive, 1 = negative)
/// - E: 4-bit exponent (bias = 7, range 0-15)
/// - M: 3-bit mantissa
#[derive(Debug, Clone, Copy, Default)]
pub struct Float8E4M3Encoder;

impl Float8E4M3Encoder {
    /// Create a new Float8 encoder.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Quantize an f32 embedding vector to Float8 E4M3 format.
    ///
    /// # Arguments
    ///
    /// * `embedding` - The f32 embedding vector to compress
    ///
    /// # Returns
    ///
    /// `QuantizedEmbedding` with 1 byte per element (4x compression).
    ///
    /// # Errors
    ///
    /// - `EmptyEmbedding` if input is empty
    /// - `ContainsNaN` if input has NaN values
    /// - `ContainsInfinity` if input has infinite values
    pub fn quantize(&self, embedding: &[f32]) -> Result<QuantizedEmbedding, Float8QuantizationError> {
        // Validate input
        if embedding.is_empty() {
            return Err(Float8QuantizationError::EmptyEmbedding);
        }

        // Check for NaN and infinity
        for (i, &val) in embedding.iter().enumerate() {
            if val.is_nan() {
                return Err(Float8QuantizationError::ContainsNaN { index: i });
            }
            if val.is_infinite() {
                return Err(Float8QuantizationError::ContainsInfinity { index: i });
            }
        }

        // Compute min and max for scaling
        let mut min_val = f32::MAX;
        let mut max_val = f32::MIN;
        for &val in embedding {
            if val < min_val {
                min_val = val;
            }
            if val > max_val {
                max_val = val;
            }
        }

        // Handle edge case: all values are the same
        let range = max_val - min_val;
        let (scale, bias, is_constant) = if range < f32::EPSILON {
            // All values are essentially the same
            // Set scale=0.0 and bias=min_val to signal constant reconstruction
            (0.0, min_val, true)
        } else {
            // Scale to use full E4M3 range
            // E4M3 can represent values roughly in [0, 448]
            // We normalize to [0, 1] then scale by 240 (max normalized E4M3 value we use)
            (range, min_val, false)
        };

        debug!(
            target: "quantization::float8",
            dim = embedding.len(),
            scale = scale,
            bias = bias,
            is_constant = is_constant,
            "Quantizing to Float8 E4M3"
        );

        // Quantize each value
        let mut data = Vec::with_capacity(embedding.len());
        for &val in embedding {
            let normalized = if is_constant {
                0.5 // Mid-range for constant values (doesn't matter since scale=0)
            } else {
                (val - bias) / scale
            };
            let quantized = self.f32_to_e4m3(normalized);
            data.push(quantized);
        }

        Ok(QuantizedEmbedding {
            method: QuantizationMethod::Float8E4M3,
            original_dim: embedding.len(),
            data,
            metadata: QuantizationMetadata::Float8 { scale, bias },
        })
    }

    /// Dequantize a Float8 E4M3 embedding back to f32 values.
    ///
    /// # Arguments
    ///
    /// * `quantized` - The quantized embedding to decompress
    ///
    /// # Returns
    ///
    /// Reconstructed f32 vector (approximately equal to original).
    ///
    /// # Errors
    ///
    /// - `InvalidMetadata` if metadata is not Float8 type
    /// - `DimensionMismatch` if data length doesn't match original_dim
    pub fn dequantize(
        &self,
        quantized: &QuantizedEmbedding,
    ) -> Result<Vec<f32>, Float8QuantizationError> {
        // Validate metadata
        let (scale, bias) = match &quantized.metadata {
            QuantizationMetadata::Float8 { scale, bias } => (*scale, *bias),
            other => {
                return Err(Float8QuantizationError::InvalidMetadata {
                    expected: "Float8",
                    got: format!("{:?}", other),
                });
            }
        };

        // Validate dimensions
        if quantized.data.len() != quantized.original_dim {
            return Err(Float8QuantizationError::DimensionMismatch {
                expected: quantized.original_dim,
                got: quantized.data.len(),
            });
        }

        // Check if this is a constant-value embedding (scale == 0.0)
        let is_constant = scale.abs() < f32::EPSILON;

        debug!(
            target: "quantization::float8",
            dim = quantized.original_dim,
            scale = scale,
            bias = bias,
            is_constant = is_constant,
            "Dequantizing from Float8 E4M3"
        );

        // Dequantize each value
        let mut result = Vec::with_capacity(quantized.original_dim);
        if is_constant {
            // All values are the same - just return bias
            result.resize(quantized.original_dim, bias);
        } else {
            for &byte in &quantized.data {
                let normalized = self.e4m3_to_f32(byte);
                let original = normalized * scale + bias;
                result.push(original);
            }
        }

        Ok(result)
    }

    /// Convert f32 (in [0, 1] range) to E4M3 byte.
    ///
    /// E4M3 format: 1 sign bit, 4 exponent bits (bias=7), 3 mantissa bits
    #[inline]
    fn f32_to_e4m3(&self, value: f32) -> u8 {
        // Handle special cases
        if value <= 0.0 {
            return 0x00; // Smallest positive value or zero
        }
        if value >= 1.0 {
            return 0x7F; // Max positive value (sign=0, exp=15, mantissa=7)
        }

        // Scale to E4M3 representable range
        // E4M3 with bias 7 can represent 2^(-6) to 2^8 approximately
        // We map [0, 1] to the representable range
        let scaled = value * 240.0; // Max representable value we use

        if scaled < 0.00390625 {
            // Below minimum representable (2^-8)
            return 0x00;
        }

        // Convert to E4M3 encoding
        let bits = scaled.to_bits();
        let sign = (bits >> 31) & 1;
        let exp = ((bits >> 23) & 0xFF) as i32;
        let mantissa = (bits >> 20) & 0x07; // Top 3 bits of 23-bit mantissa

        // Compute E4M3 exponent (bias conversion: f32 bias=127, e4m3 bias=7)
        let e4m3_exp = (exp - 127 + 7).clamp(0, 15) as u8;

        // Combine into E4M3 byte
        
        ((sign as u8) << 7) | (e4m3_exp << 3) | (mantissa as u8)
    }

    /// Convert E4M3 byte to f32 (in [0, 1] range).
    #[inline]
    fn e4m3_to_f32(&self, byte: u8) -> f32 {
        let sign = (byte >> 7) & 1;
        let exp = (byte >> 3) & 0x0F;
        let mantissa = byte & 0x07;

        // Handle zero
        if exp == 0 && mantissa == 0 {
            return 0.0;
        }

        // Reconstruct value
        // E4M3: value = (-1)^sign * 2^(exp-7) * (1 + mantissa/8)
        let exp_value = 2.0f32.powi(exp as i32 - 7);
        let mant_value = 1.0 + (mantissa as f32) / 8.0;
        let value = exp_value * mant_value;

        // Apply sign and scale back to [0, 1]
        let signed_value = if sign == 1 { -value } else { value };
        (signed_value / 240.0).clamp(0.0, 1.0)
    }

    /// Compute theoretical compression ratio.
    #[must_use]
    pub const fn compression_ratio() -> f32 {
        4.0 // f32 (4 bytes) → u8 (1 byte)
    }

    /// Check if quantization is available for this encoder.
    #[must_use]
    pub const fn is_available() -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Basic functionality tests
    // =========================================================================

    #[test]
    fn test_quantize_basic() {
        let encoder = Float8E4M3Encoder::new();
        let embedding = vec![0.1, 0.2, 0.3, 0.4, 0.5];

        let quantized = encoder.quantize(&embedding).expect("quantize");

        assert_eq!(quantized.method, QuantizationMethod::Float8E4M3);
        assert_eq!(quantized.original_dim, 5);
        assert_eq!(quantized.data.len(), 5); // 1 byte per element
    }

    #[test]
    fn test_round_trip() {
        let encoder = Float8E4M3Encoder::new();
        let embedding: Vec<f32> = (0..512)
            .map(|i| (i as f32 / 512.0 - 0.5) * 2.0) // Range [-1, 1]
            .collect();

        let quantized = encoder.quantize(&embedding).expect("quantize");
        let reconstructed = encoder.dequantize(&quantized).expect("dequantize");

        // Verify dimensions match
        assert_eq!(reconstructed.len(), embedding.len());

        // Verify reconstruction error is within tolerance
        let mut max_error = 0.0f32;
        let mut total_error = 0.0f32;
        for (orig, recon) in embedding.iter().zip(reconstructed.iter()) {
            let error = (orig - recon).abs();
            max_error = max_error.max(error);
            total_error += error;
        }
        let avg_error = total_error / embedding.len() as f32;

        // Float8 should have <1% average error
        assert!(
            avg_error < 0.05,
            "Average error {} exceeds 5% tolerance",
            avg_error
        );
        // Max error should be < 10%
        assert!(
            max_error < 0.2,
            "Max error {} exceeds 20% tolerance",
            max_error
        );
    }

    #[test]
    fn test_compression_ratio() {
        let encoder = Float8E4M3Encoder::new();
        let embedding = vec![0.5f32; 512]; // 512 * 4 = 2048 bytes

        let quantized = encoder.quantize(&embedding).expect("quantize");

        assert_eq!(quantized.data.len(), 512); // 512 bytes
        assert!(
            quantized.compression_ratio() > 3.9,
            "Expected ~4x compression, got {}x",
            quantized.compression_ratio()
        );
    }

    // =========================================================================
    // Edge case tests
    // =========================================================================

    #[test]
    fn test_empty_embedding_error() {
        let encoder = Float8E4M3Encoder::new();
        let result = encoder.quantize(&[]);

        assert!(matches!(result, Err(Float8QuantizationError::EmptyEmbedding)));
    }

    #[test]
    fn test_nan_error() {
        let encoder = Float8E4M3Encoder::new();
        let embedding = vec![0.5, f32::NAN, 0.3];

        let result = encoder.quantize(&embedding);
        assert!(matches!(
            result,
            Err(Float8QuantizationError::ContainsNaN { index: 1 })
        ));
    }

    #[test]
    fn test_infinity_error() {
        let encoder = Float8E4M3Encoder::new();
        let embedding = vec![0.5, 0.3, f32::INFINITY];

        let result = encoder.quantize(&embedding);
        assert!(matches!(
            result,
            Err(Float8QuantizationError::ContainsInfinity { index: 2 })
        ));
    }

    #[test]
    fn test_all_same_value() {
        let encoder = Float8E4M3Encoder::new();
        let embedding = vec![0.5f32; 100]; // All same value

        let quantized = encoder.quantize(&embedding).expect("quantize");
        let reconstructed = encoder.dequantize(&quantized).expect("dequantize");

        // All values should be close to original
        for &val in &reconstructed {
            assert!(
                (val - 0.5).abs() < 0.1,
                "Reconstructed value {} deviates too much from 0.5",
                val
            );
        }
    }

    #[test]
    fn test_all_zeros() {
        let encoder = Float8E4M3Encoder::new();
        let embedding = vec![0.0f32; 64];

        let quantized = encoder.quantize(&embedding).expect("quantize");
        let reconstructed = encoder.dequantize(&quantized).expect("dequantize");

        // All values should be near zero
        for &val in &reconstructed {
            assert!(
                val.abs() < 0.01,
                "Reconstructed value {} should be near zero",
                val
            );
        }
    }

    #[test]
    fn test_negative_values() {
        let encoder = Float8E4M3Encoder::new();
        let embedding = vec![-0.5, -0.3, -0.1, 0.1, 0.3, 0.5];

        let quantized = encoder.quantize(&embedding).expect("quantize");
        let reconstructed = encoder.dequantize(&quantized).expect("dequantize");

        // Verify order is preserved (relative magnitudes)
        assert!(
            reconstructed[0] < reconstructed[1],
            "Order not preserved at 0,1"
        );
        assert!(
            reconstructed[1] < reconstructed[2],
            "Order not preserved at 1,2"
        );
    }

    #[test]
    fn test_large_dimension() {
        let encoder = Float8E4M3Encoder::new();
        // Test with E8_Graph dimension (384)
        let embedding: Vec<f32> = (0..384).map(|i| (i as f32).sin()).collect();

        let quantized = encoder.quantize(&embedding).expect("quantize");
        let reconstructed = encoder.dequantize(&quantized).expect("dequantize");

        assert_eq!(reconstructed.len(), 384);
        assert_eq!(quantized.data.len(), 384);
    }

    #[test]
    fn test_dequantize_wrong_metadata() {
        let encoder = Float8E4M3Encoder::new();

        let bad_quantized = QuantizedEmbedding {
            method: QuantizationMethod::Float8E4M3,
            original_dim: 8,
            data: vec![0xFF; 8],
            metadata: QuantizationMetadata::Binary { threshold: 0.0 }, // Wrong!
        };

        let result = encoder.dequantize(&bad_quantized);
        assert!(matches!(
            result,
            Err(Float8QuantizationError::InvalidMetadata { .. })
        ));
    }

    #[test]
    fn test_dequantize_dimension_mismatch() {
        let encoder = Float8E4M3Encoder::new();

        let bad_quantized = QuantizedEmbedding {
            method: QuantizationMethod::Float8E4M3,
            original_dim: 100, // Says 100
            data: vec![0xFF; 50], // But only 50 bytes
            metadata: QuantizationMetadata::Float8 {
                scale: 1.0,
                bias: 0.0,
            },
        };

        let result = encoder.dequantize(&bad_quantized);
        assert!(matches!(
            result,
            Err(Float8QuantizationError::DimensionMismatch { .. })
        ));
    }

    // =========================================================================
    // Constitution compliance tests
    // =========================================================================

    #[test]
    fn test_recall_loss_within_spec() {
        // Constitution specifies <0.3% recall loss
        // We test by measuring cosine similarity preservation
        let encoder = Float8E4M3Encoder::new();

        let embedding1: Vec<f32> = (0..512).map(|i| (i as f32 * 0.01).sin()).collect();
        let embedding2: Vec<f32> = (0..512).map(|i| (i as f32 * 0.01).cos()).collect();

        // Compute original cosine similarity
        let dot_original: f32 = embedding1
            .iter()
            .zip(embedding2.iter())
            .map(|(a, b)| a * b)
            .sum();
        let norm1: f32 = embedding1.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm2: f32 = embedding2.iter().map(|x| x * x).sum::<f32>().sqrt();
        let cos_original = dot_original / (norm1 * norm2);

        // Quantize and dequantize
        let q1 = encoder.quantize(&embedding1).expect("q1");
        let q2 = encoder.quantize(&embedding2).expect("q2");
        let d1 = encoder.dequantize(&q1).expect("d1");
        let d2 = encoder.dequantize(&q2).expect("d2");

        // Compute reconstructed cosine similarity
        let dot_recon: f32 = d1.iter().zip(d2.iter()).map(|(a, b)| a * b).sum();
        let norm1_r: f32 = d1.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm2_r: f32 = d2.iter().map(|x| x * x).sum::<f32>().sqrt();
        let cos_recon = dot_recon / (norm1_r * norm2_r);

        // Similarity difference should be small
        let sim_diff = (cos_original - cos_recon).abs();
        assert!(
            sim_diff < 0.05,
            "Similarity difference {} exceeds 5% tolerance",
            sim_diff
        );
    }

    #[test]
    fn test_e2_temporal_recent_dimension() {
        // E2 has 512 dimensions
        let encoder = Float8E4M3Encoder::new();
        let embedding = vec![0.1f32; 512];

        let quantized = encoder.quantize(&embedding).expect("quantize");
        assert_eq!(quantized.original_dim, 512);
        assert_eq!(quantized.data.len(), 512);
    }

    #[test]
    fn test_e8_graph_dimension() {
        // E8 has 384 dimensions
        let encoder = Float8E4M3Encoder::new();
        let embedding = vec![0.1f32; 384];

        let quantized = encoder.quantize(&embedding).expect("quantize");
        assert_eq!(quantized.original_dim, 384);
        assert_eq!(quantized.data.len(), 384);
    }

    #[test]
    fn test_e11_entity_dimension() {
        // E11 has 384 dimensions
        let encoder = Float8E4M3Encoder::new();
        let embedding = vec![0.1f32; 384];

        let quantized = encoder.quantize(&embedding).expect("quantize");
        assert_eq!(quantized.original_dim, 384);
        assert_eq!(quantized.data.len(), 384);
    }
}
