//! Binary quantization for E9_HDC (Hyperdimensional Computing).
//!
//! Implements Constitution `embeddings.quantization.Binary`:
//! - 32x compression (8 f32 values -> 1 byte)
//! - Hamming distance for similarity
//! - <1ms latency requirement
//!
//! # Algorithm
//!
//! 1. Binarize: value >= threshold -> 1, else -> 0
//! 2. Pack 8 bits per byte (MSB first)
//! 3. Store packed bytes + threshold in metadata
//!
//! # Error Codes
//!
//! - EMB-E012: Binary quantization failed (exit 117)

use super::types::{BinaryEncoder, QuantizationMetadata, QuantizationMethod, QuantizedEmbedding};
use tracing::{error, info, instrument};

/// Error type for binary quantization operations.
#[derive(Debug, thiserror::Error)]
pub enum BinaryQuantizationError {
    /// Input embedding is empty.
    #[error("[EMB-E012] BINARY_QUANTIZATION_FAILED: Empty input embedding")]
    EmptyInput,

    /// Input contains NaN or Infinity values.
    #[error("[EMB-E012] BINARY_QUANTIZATION_FAILED: Input contains NaN/Inf at index {index}, value={value}")]
    InvalidValue {
        /// Index of the invalid value.
        index: usize,
        /// The invalid value (NaN or Infinity).
        value: f32,
    },

    /// Packed data length mismatch during dequantization.
    #[error("[EMB-E012] BINARY_DEQUANTIZATION_FAILED: Expected {expected} bytes, got {actual}")]
    DataLengthMismatch {
        /// Expected number of bytes.
        expected: usize,
        /// Actual number of bytes received.
        actual: usize,
    },

    /// Metadata type mismatch.
    #[error("[EMB-E012] BINARY_DEQUANTIZATION_FAILED: Expected Binary metadata, got different variant")]
    MetadataMismatch,
}

impl BinaryEncoder {
    /// Create a new binary encoder instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Binarize and pack an f32 embedding into bytes.
    ///
    /// # Algorithm
    ///
    /// 1. Validate input (no NaN/Inf, non-empty)
    /// 2. Compute threshold (default: 0.0 for sign-based binarization)
    /// 3. For each value: bit = 1 if value >= threshold, else 0
    /// 4. Pack 8 bits per byte, MSB first
    /// 5. Pad final byte with zeros if needed
    ///
    /// # Arguments
    ///
    /// * `embedding` - Input f32 embedding vector
    /// * `threshold` - Optional custom threshold (default: 0.0)
    ///
    /// # Returns
    ///
    /// `QuantizedEmbedding` with packed binary data.
    ///
    /// # Errors
    ///
    /// - `EmptyInput` if embedding is empty
    /// - `InvalidValue` if any value is NaN or Infinity
    ///
    /// # Constitution Compliance
    ///
    /// - 32x compression: 1024 f32 (4096 bytes) -> 128 bytes
    /// - <1ms latency for typical HDC dimensions
    #[instrument(skip(self, embedding), fields(dim = embedding.len()))]
    pub fn quantize(
        &self,
        embedding: &[f32],
        threshold: Option<f32>,
    ) -> Result<QuantizedEmbedding, BinaryQuantizationError> {
        // Validate input: empty check
        if embedding.is_empty() {
            error!(
                target: "quantization::binary",
                code = "EMB-E012",
                "Empty input embedding"
            );
            return Err(BinaryQuantizationError::EmptyInput);
        }

        // Validate input: check for NaN/Inf (fail fast with exact index/value)
        for (i, &val) in embedding.iter().enumerate() {
            if !val.is_finite() {
                error!(
                    target: "quantization::binary",
                    code = "EMB-E012",
                    index = i,
                    value = %val,
                    "Invalid value in embedding"
                );
                return Err(BinaryQuantizationError::InvalidValue { index: i, value: val });
            }
        }

        let thresh = threshold.unwrap_or(0.0);
        let original_dim = embedding.len();

        // Calculate packed size: ceil(dim / 8)
        let packed_len = original_dim.div_ceil(8);
        let mut packed = vec![0u8; packed_len];

        // Pack bits: MSB first within each byte
        // Bit 7 of byte 0 corresponds to index 0 of input
        // Bit 6 of byte 0 corresponds to index 1 of input
        // ...
        // Bit 0 of byte 0 corresponds to index 7 of input
        for (i, &val) in embedding.iter().enumerate() {
            if val >= thresh {
                let byte_idx = i / 8;
                let bit_idx = 7 - (i % 8); // MSB first
                packed[byte_idx] |= 1 << bit_idx;
            }
        }

        info!(
            target: "quantization::binary",
            code = "EMB-I012",
            original_dim = original_dim,
            packed_bytes = packed_len,
            threshold = thresh,
            compression_ratio = (original_dim * 4) as f32 / packed_len as f32,
            "Binary quantization complete"
        );

        Ok(QuantizedEmbedding {
            method: QuantizationMethod::Binary,
            original_dim,
            data: packed,
            metadata: QuantizationMetadata::Binary { threshold: thresh },
        })
    }

    /// Dequantize packed binary data back to approximate f32 values.
    ///
    /// # Algorithm
    ///
    /// 1. Validate data length matches expected
    /// 2. Unpack bits from bytes (MSB first)
    /// 3. Convert: 1 -> +1.0, 0 -> -1.0 (bipolar representation)
    ///
    /// # Arguments
    ///
    /// * `quantized` - The quantized embedding to reconstruct
    ///
    /// # Returns
    ///
    /// Reconstructed f32 vector with bipolar values (+1.0 or -1.0).
    ///
    /// # Errors
    ///
    /// - `DataLengthMismatch` if packed data doesn't match original_dim
    /// - `MetadataMismatch` if metadata is not Binary variant
    #[instrument(skip(self, quantized), fields(dim = quantized.original_dim))]
    pub fn dequantize(
        &self,
        quantized: &QuantizedEmbedding,
    ) -> Result<Vec<f32>, BinaryQuantizationError> {
        // Validate metadata type
        let _threshold = match &quantized.metadata {
            QuantizationMetadata::Binary { threshold } => *threshold,
            _ => {
                error!(
                    target: "quantization::binary",
                    code = "EMB-E012",
                    "Expected Binary metadata"
                );
                return Err(BinaryQuantizationError::MetadataMismatch);
            }
        };

        // Validate data length
        let expected_bytes = quantized.original_dim.div_ceil(8);
        if quantized.data.len() != expected_bytes {
            error!(
                target: "quantization::binary",
                code = "EMB-E012",
                expected = expected_bytes,
                actual = quantized.data.len(),
                "Data length mismatch"
            );
            return Err(BinaryQuantizationError::DataLengthMismatch {
                expected: expected_bytes,
                actual: quantized.data.len(),
            });
        }

        let mut result = Vec::with_capacity(quantized.original_dim);

        // Unpack bits: MSB first
        for i in 0..quantized.original_dim {
            let byte_idx = i / 8;
            let bit_idx = 7 - (i % 8);
            let bit = (quantized.data[byte_idx] >> bit_idx) & 1;
            // Bipolar: 1 -> +1.0, 0 -> -1.0
            result.push(if bit == 1 { 1.0 } else { -1.0 });
        }

        info!(
            target: "quantization::binary",
            code = "EMB-I013",
            original_dim = quantized.original_dim,
            "Binary dequantization complete"
        );

        Ok(result)
    }

    /// Compute Hamming distance between two binary-quantized embeddings.
    ///
    /// Hamming distance = number of differing bits.
    /// For normalized similarity: sim = 1 - (hamming_dist / total_bits)
    ///
    /// # Arguments
    ///
    /// * `a` - First quantized embedding
    /// * `b` - Second quantized embedding
    ///
    /// # Returns
    ///
    /// Number of differing bits.
    ///
    /// # Panics
    ///
    /// Panics if embeddings have different dimensions (programming error).
    #[must_use]
    pub fn hamming_distance(a: &QuantizedEmbedding, b: &QuantizedEmbedding) -> usize {
        assert_eq!(
            a.original_dim, b.original_dim,
            "Dimension mismatch: {} vs {}",
            a.original_dim, b.original_dim
        );
        assert_eq!(
            a.data.len(),
            b.data.len(),
            "Data length mismatch: {} vs {}",
            a.data.len(),
            b.data.len()
        );

        // Count differing bits using XOR + popcount
        a.data
            .iter()
            .zip(b.data.iter())
            .map(|(&x, &y)| (x ^ y).count_ones() as usize)
            .sum()
    }

    /// Compute normalized Hamming similarity (0.0 to 1.0).
    ///
    /// similarity = 1 - (hamming_distance / total_bits)
    ///
    /// # Arguments
    ///
    /// * `a` - First quantized embedding
    /// * `b` - Second quantized embedding
    ///
    /// # Returns
    ///
    /// Similarity in range [0.0, 1.0] where 1.0 = identical.
    #[must_use]
    pub fn hamming_similarity(a: &QuantizedEmbedding, b: &QuantizedEmbedding) -> f32 {
        let distance = Self::hamming_distance(a, b);
        let total_bits = a.original_dim;
        1.0 - (distance as f32 / total_bits as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================
    // REAL DATA TESTS - NO MOCKS
    // ==========================================

    #[test]
    fn test_quantize_basic() {
        let encoder = BinaryEncoder::new();
        let input = vec![0.5, -0.3, 0.1, -0.8, 0.0, 0.9, -0.1, 0.2];

        let result = encoder
            .quantize(&input, None)
            .expect("quantization should succeed");

        assert_eq!(result.method, QuantizationMethod::Binary);
        assert_eq!(result.original_dim, 8);
        assert_eq!(result.data.len(), 1); // 8 bits = 1 byte

        // Verify threshold stored
        match &result.metadata {
            QuantizationMetadata::Binary { threshold } => {
                assert!((*threshold - 0.0).abs() < f32::EPSILON);
            }
            _ => panic!("Wrong metadata variant"),
        }

        // VERIFICATION: Check actual bit pattern
        // Input: [0.5, -0.3, 0.1, -0.8, 0.0, 0.9, -0.1, 0.2]
        // Bits:  [1,   0,    1,   0,    1,   1,   0,    1] (>= 0.0)
        // MSB first: 10101101 = 0xAD = 173
        assert_eq!(result.data[0], 0b10101101, "Bit pattern mismatch");
    }

    #[test]
    fn test_quantize_custom_threshold() {
        let encoder = BinaryEncoder::new();
        let input = vec![0.5, 0.3, 0.1, 0.8];

        let result = encoder
            .quantize(&input, Some(0.4))
            .expect("quantization should succeed");

        // Bits with threshold 0.4: [1, 0, 0, 1] (only 0.5 and 0.8 >= 0.4)
        // MSB first in one byte: [1,0,0,1,0,0,0,0] = 0b10010000 = 144
        assert_eq!(result.data[0], 0b10010000);
    }

    #[test]
    fn test_roundtrip_preserves_signs() {
        let encoder = BinaryEncoder::new();
        let input: Vec<f32> = (0..1024)
            .map(|i| if i % 3 == 0 { 0.5 } else { -0.5 })
            .collect();

        let quantized = encoder.quantize(&input, None).expect("quantize");
        let reconstructed = encoder.dequantize(&quantized).expect("dequantize");

        // VERIFICATION: All signs must match
        for (i, (&orig, &recon)) in input.iter().zip(reconstructed.iter()).enumerate() {
            let orig_sign = orig >= 0.0;
            let recon_sign = recon >= 0.0;
            assert_eq!(
                orig_sign, recon_sign,
                "Sign mismatch at index {}: orig={}, recon={}",
                i, orig, recon
            );
        }
    }

    #[test]
    fn test_compression_ratio_32x() {
        let encoder = BinaryEncoder::new();
        let input = vec![0.0f32; 1024]; // 1024 f32 = 4096 bytes

        let result = encoder.quantize(&input, None).expect("quantize");

        // Expected: 1024 bits = 128 bytes
        assert_eq!(result.data.len(), 128);

        // Compression ratio: 4096 / 128 = 32x
        let ratio = result.compression_ratio();
        assert!(
            (ratio - 32.0).abs() < 0.1,
            "Expected 32x compression, got {}x",
            ratio
        );
    }

    #[test]
    fn test_hamming_distance_identical() {
        let encoder = BinaryEncoder::new();
        let input = vec![0.5, -0.3, 0.1, -0.8];

        let a = encoder.quantize(&input, None).expect("quantize");
        let b = encoder.quantize(&input, None).expect("quantize");

        let distance = BinaryEncoder::hamming_distance(&a, &b);
        assert_eq!(distance, 0, "Identical inputs should have distance 0");
    }

    #[test]
    fn test_hamming_distance_opposite() {
        let encoder = BinaryEncoder::new();
        let a_input = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let b_input = vec![-1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0];

        let a = encoder.quantize(&a_input, None).expect("quantize");
        let b = encoder.quantize(&b_input, None).expect("quantize");

        let distance = BinaryEncoder::hamming_distance(&a, &b);
        assert_eq!(distance, 8, "Opposite inputs should have distance = dim");
    }

    #[test]
    fn test_hamming_similarity() {
        let encoder = BinaryEncoder::new();
        let a_input = vec![1.0; 8];
        let b_input = vec![1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0];

        let a = encoder.quantize(&a_input, None).expect("quantize");
        let b = encoder.quantize(&b_input, None).expect("quantize");

        let sim = BinaryEncoder::hamming_similarity(&a, &b);
        assert!(
            (sim - 0.5).abs() < 0.01,
            "Expected 50% similarity, got {}",
            sim
        );
    }

    // ==========================================
    // ERROR CASE TESTS - FAIL FAST VERIFICATION
    // ==========================================

    #[test]
    fn test_empty_input_fails() {
        let encoder = BinaryEncoder::new();
        let result = encoder.quantize(&[], None);

        assert!(result.is_err());
        match result.unwrap_err() {
            BinaryQuantizationError::EmptyInput => {}
            e => panic!("Expected EmptyInput, got {:?}", e),
        }
    }

    #[test]
    fn test_nan_input_fails() {
        let encoder = BinaryEncoder::new();
        let input = vec![1.0, f32::NAN, 2.0];
        let result = encoder.quantize(&input, None);

        assert!(result.is_err());
        match result.unwrap_err() {
            BinaryQuantizationError::InvalidValue { index, .. } => {
                assert_eq!(index, 1, "NaN should be detected at index 1");
            }
            e => panic!("Expected InvalidValue, got {:?}", e),
        }
    }

    #[test]
    fn test_infinity_input_fails() {
        let encoder = BinaryEncoder::new();
        let input = vec![1.0, 2.0, f32::INFINITY];
        let result = encoder.quantize(&input, None);

        assert!(result.is_err());
        match result.unwrap_err() {
            BinaryQuantizationError::InvalidValue { index, .. } => {
                assert_eq!(index, 2, "Infinity should be detected at index 2");
            }
            e => panic!("Expected InvalidValue, got {:?}", e),
        }
    }

    #[test]
    fn test_dequantize_wrong_metadata_fails() {
        let bad_quantized = QuantizedEmbedding {
            method: QuantizationMethod::Binary,
            original_dim: 8,
            data: vec![0xFF],
            metadata: QuantizationMetadata::Float8 {
                scale: 1.0,
                bias: 0.0,
            },
        };

        let encoder = BinaryEncoder::new();
        let result = encoder.dequantize(&bad_quantized);

        assert!(result.is_err());
        match result.unwrap_err() {
            BinaryQuantizationError::MetadataMismatch => {}
            e => panic!("Expected MetadataMismatch, got {:?}", e),
        }
    }

    #[test]
    fn test_dequantize_wrong_data_length_fails() {
        let bad_quantized = QuantizedEmbedding {
            method: QuantizationMethod::Binary,
            original_dim: 16, // Should need 2 bytes
            data: vec![0xFF], // Only 1 byte
            metadata: QuantizationMetadata::Binary { threshold: 0.0 },
        };

        let encoder = BinaryEncoder::new();
        let result = encoder.dequantize(&bad_quantized);

        assert!(result.is_err());
        match result.unwrap_err() {
            BinaryQuantizationError::DataLengthMismatch { expected, actual } => {
                assert_eq!(expected, 2);
                assert_eq!(actual, 1);
            }
            e => panic!("Expected DataLengthMismatch, got {:?}", e),
        }
    }

    // ==========================================
    // EDGE CASES - BOUNDARY CONDITIONS
    // ==========================================

    #[test]
    fn test_single_element() {
        let encoder = BinaryEncoder::new();
        let input = vec![0.5];

        let result = encoder.quantize(&input, None).expect("quantize");
        assert_eq!(result.data.len(), 1);
        assert_eq!(result.data[0], 0b10000000); // MSB set, rest zeros

        let reconstructed = encoder.dequantize(&result).expect("dequantize");
        assert_eq!(reconstructed.len(), 1);
        assert_eq!(reconstructed[0], 1.0);
    }

    #[test]
    fn test_dimension_not_multiple_of_8() {
        let encoder = BinaryEncoder::new();
        let input = vec![1.0; 13]; // 13 elements, needs 2 bytes

        let result = encoder.quantize(&input, None).expect("quantize");
        assert_eq!(result.data.len(), 2);

        let reconstructed = encoder.dequantize(&result).expect("dequantize");
        assert_eq!(reconstructed.len(), 13);
    }

    #[test]
    fn test_all_positive_values() {
        let encoder = BinaryEncoder::new();
        let input = vec![1.0; 8];

        let result = encoder.quantize(&input, None).expect("quantize");
        assert_eq!(result.data[0], 0xFF); // All bits set
    }

    #[test]
    fn test_all_negative_values() {
        let encoder = BinaryEncoder::new();
        let input = vec![-1.0; 8];

        let result = encoder.quantize(&input, None).expect("quantize");
        assert_eq!(result.data[0], 0x00); // No bits set
    }

    #[test]
    fn test_exactly_at_threshold() {
        let encoder = BinaryEncoder::new();
        // Value exactly at threshold should be counted as 1
        let input = vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

        let result = encoder.quantize(&input, Some(0.0)).expect("quantize");
        // All values >= 0.0, so all bits = 1
        assert_eq!(result.data[0], 0xFF);
    }

    #[test]
    fn test_large_dimension_10k() {
        let encoder = BinaryEncoder::new();
        let input: Vec<f32> = (0..10000)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();

        let result = encoder.quantize(&input, None).expect("quantize");

        // 10000 bits = 1250 bytes
        assert_eq!(result.data.len(), 1250);

        // Verify compression ratio
        let ratio = result.compression_ratio();
        assert!(ratio > 31.0 && ratio < 33.0, "Expected ~32x compression");

        // Verify roundtrip
        let reconstructed = encoder.dequantize(&result).expect("dequantize");
        assert_eq!(reconstructed.len(), 10000);
    }

    #[test]
    fn test_negative_infinity_input_fails() {
        let encoder = BinaryEncoder::new();
        let input = vec![1.0, f32::NEG_INFINITY, 2.0];
        let result = encoder.quantize(&input, None);

        assert!(result.is_err());
        match result.unwrap_err() {
            BinaryQuantizationError::InvalidValue { index, .. } => {
                assert_eq!(index, 1, "NEG_INFINITY should be detected at index 1");
            }
            e => panic!("Expected InvalidValue, got {:?}", e),
        }
    }

    #[test]
    fn test_bipolar_reconstruction_values() {
        let encoder = BinaryEncoder::new();
        let input = vec![0.5, -0.5]; // positive, negative

        let quantized = encoder.quantize(&input, None).expect("quantize");
        let reconstructed = encoder.dequantize(&quantized).expect("dequantize");

        // Should reconstruct as bipolar: +1.0 and -1.0
        assert_eq!(reconstructed[0], 1.0);
        assert_eq!(reconstructed[1], -1.0);
    }
}
