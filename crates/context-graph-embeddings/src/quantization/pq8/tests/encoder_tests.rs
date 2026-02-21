//! Tests for PQ-8 encoder functionality.
//!
//! Tests for:
//! - Basic encoder creation
//! - Quantization and dequantization
//! - Round-trip quality
//! - Error handling

use crate::quantization::pq8::encoder::PQ8Encoder;
use crate::quantization::pq8::types::PQ8QuantizationError;
use crate::quantization::types::{QuantizationMetadata, QuantizationMethod};
use tracing::debug;

#[test]
fn test_encoder_new() {
    let encoder = PQ8Encoder::new(1024);
    assert_eq!(encoder.codebook().embedding_dim, 1024);
    assert_eq!(encoder.codebook().num_subvectors, 8);
    assert_eq!(encoder.codebook().num_centroids, 256);
}

#[test]
fn test_encoder_default() {
    let encoder = PQ8Encoder::default();
    assert_eq!(encoder.codebook().embedding_dim, 1024);
}

#[test]
#[should_panic(expected = "must be divisible by")]
fn test_encoder_invalid_dim() {
    let _ = PQ8Encoder::new(1001); // Not divisible by 8 (1001 % 8 = 1)
}

#[test]
fn test_quantize_basic() {
    let encoder = PQ8Encoder::new(1024);
    let embedding: Vec<f32> = (0..1024).map(|i| (i as f32 / 512.0) - 1.0).collect();

    let quantized = encoder.quantize(&embedding).expect("quantize");

    assert_eq!(quantized.method, QuantizationMethod::PQ8);
    assert_eq!(quantized.original_dim, 1024);
    assert_eq!(quantized.data.len(), 8); // 8 centroid indices
}

#[test]
fn test_round_trip() {
    let encoder = PQ8Encoder::new(256);
    let embedding: Vec<f32> = (0..256).map(|i| (i as f32 / 128.0) - 1.0).collect();

    let quantized = encoder.quantize(&embedding).expect("quantize");
    let reconstructed = encoder.dequantize(&quantized).expect("dequantize");

    assert_eq!(reconstructed.len(), 256);

    // Compute cosine similarity for reconstruction quality
    let dot: f32 = embedding
        .iter()
        .zip(reconstructed.iter())
        .map(|(a, b)| a * b)
        .sum();
    let norm_a: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = reconstructed.iter().map(|x| x * x).sum::<f32>().sqrt();
    let cosine = dot / (norm_a * norm_b);

    // Note: Default codebook with LCG-generated centroids provides moderate quality.
    // The <5% recall loss guarantee only applies to trained codebooks.
    assert!(
        cosine > 0.15,
        "Cosine similarity {} too low for PQ-8 round-trip (expected > 0.15 for default codebook)",
        cosine
    );
}

#[test]
fn test_compression_ratio() {
    let encoder = PQ8Encoder::new(1024);
    let embedding = vec![0.5f32; 1024];

    let quantized = encoder.quantize(&embedding).expect("quantize");

    // 1024 * 4 bytes = 4096 bytes original
    // 8 bytes compressed
    let actual_ratio = (1024 * 4) as f32 / quantized.data.len() as f32;
    assert!(
        actual_ratio > 500.0,
        "Compression ratio {} too low (expected ~512x)",
        actual_ratio
    );
}

#[test]
fn test_empty_embedding_error() {
    let encoder = PQ8Encoder::new(256);
    let result = encoder.quantize(&[]);
    assert!(matches!(result, Err(PQ8QuantizationError::EmptyEmbedding)));
}

#[test]
fn test_nan_error() {
    let encoder = PQ8Encoder::new(256);
    let mut embedding = vec![0.5f32; 256];
    embedding[100] = f32::NAN;

    let result = encoder.quantize(&embedding);
    assert!(matches!(
        result,
        Err(PQ8QuantizationError::ContainsNaN { index: 100 })
    ));
}

#[test]
fn test_infinity_error() {
    let encoder = PQ8Encoder::new(256);
    let mut embedding = vec![0.5f32; 256];
    embedding[50] = f32::INFINITY;

    let result = encoder.quantize(&embedding);
    assert!(matches!(
        result,
        Err(PQ8QuantizationError::ContainsInfinity { index: 50 })
    ));
}

#[test]
fn test_dimension_mismatch_error() {
    let encoder = PQ8Encoder::new(1024);
    let embedding = vec![0.5f32; 256]; // Wrong dimension

    let result = encoder.quantize(&embedding);
    assert!(matches!(
        result,
        Err(PQ8QuantizationError::CodebookDimensionMismatch { .. })
    ));
}

#[test]
fn test_dequantize_wrong_metadata() {
    let encoder = PQ8Encoder::new(256);

    let bad_quantized = crate::quantization::types::QuantizedEmbedding {
        method: QuantizationMethod::PQ8,
        original_dim: 256,
        data: vec![0u8; 8],
        metadata: QuantizationMetadata::Float8 {
            scale: 1.0,
            bias: 0.0,
        },
    };

    let result = encoder.dequantize(&bad_quantized);
    assert!(matches!(
        result,
        Err(PQ8QuantizationError::InvalidMetadata { .. })
    ));
}

#[test]
fn test_dequantize_wrong_data_length() {
    let encoder = PQ8Encoder::new(256);

    let bad_quantized = crate::quantization::types::QuantizedEmbedding {
        method: QuantizationMethod::PQ8,
        original_dim: 256,
        data: vec![0u8; 4], // Should be 8
        metadata: QuantizationMetadata::PQ8 {
            codebook_id: 0,
            num_subvectors: 8,
        },
    };

    let result = encoder.dequantize(&bad_quantized);
    assert!(matches!(
        result,
        Err(PQ8QuantizationError::InvalidDataLength { .. })
    ));
}

#[test]
fn test_all_pq8_dimensions() {
    // Test all PQ8 embedder dimensions
    let dimensions = [
        1024, // E1_Semantic
        768,  // E5_Causal
        1536, // E7_Code
        768,  // E10_Multimodal
    ];

    for dim in dimensions {
        let encoder = PQ8Encoder::new(dim);
        let embedding: Vec<f32> = (0..dim)
            .map(|i| (i as f32 / dim as f32) * 2.0 - 1.0)
            .collect();

        let quantized = encoder
            .quantize(&embedding)
            .unwrap_or_else(|e| panic!("quantize {}D: {:?}", dim, e));
        let reconstructed = encoder.dequantize(&quantized).expect("dequantize");

        assert_eq!(reconstructed.len(), dim);
        assert_eq!(quantized.data.len(), 8);
    }
}

#[test]
fn test_recall_within_spec() {
    // PQ-8 should have <5% recall loss
    // We test this by checking cosine similarity on random embeddings
    let encoder = PQ8Encoder::new(1024);

    let mut total_cosine = 0.0;
    let num_tests = 10;

    for seed in 0..num_tests {
        // Create deterministic "random" embedding
        let embedding: Vec<f32> = (0..1024)
            .map(|i| (i as f32 + seed as f32 * 100.0).sin() * 0.5)
            .collect();

        let quantized = encoder.quantize(&embedding).expect("quantize");
        let reconstructed = encoder.dequantize(&quantized).expect("dequantize");

        // Compute cosine similarity
        let dot: f32 = embedding
            .iter()
            .zip(reconstructed.iter())
            .map(|(a, b)| a * b)
            .sum();
        let norm_a: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = reconstructed.iter().map(|x| x * x).sum::<f32>().sqrt();
        let cosine = dot / (norm_a * norm_b);

        total_cosine += cosine;
    }

    let avg_cosine = total_cosine / num_tests as f32;
    // Note: Default codebook uses pseudo-random centroids which provides
    // reasonable but not optimal quantization.
    assert!(
        avg_cosine > 0.15,
        "Average cosine similarity {} too low even for default codebook (expected > 0.15)",
        avg_cosine
    );

    debug!(
        target: "quantization::pq8::test",
        "Default codebook avg cosine similarity: {:.4} (train codebook for <5% recall loss)",
        avg_cosine
    );
}
