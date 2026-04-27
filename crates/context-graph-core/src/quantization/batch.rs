//! Batch quantization operations.
//!
//! TASK-L03: SPEC-E12-QUANT-001 REQ-E12-06: Batch quantization support.

use super::traits::Quantizable;
use super::types::{Precision, QuantizationError, QuantizedEmbedding};
use rayon::prelude::*;

/// Batch quantize multiple embeddings in parallel.
///
/// # Arguments
///
/// * `embeddings` - Slice of embeddings to quantize
/// * `precision` - Target precision
///
/// # Returns
///
/// Vec of quantization results (preserves order).
pub fn batch_quantize<T: Quantizable + Sync>(
    embeddings: &[T],
    precision: Precision,
) -> Vec<Result<QuantizedEmbedding, QuantizationError>> {
    embeddings
        .par_iter()
        .map(|e| e.quantize(precision))
        .collect()
}

/// Batch dequantize multiple embeddings in parallel.
///
/// # Arguments
///
/// * `quantized` - Slice of quantized embeddings
///
/// # Returns
///
/// Vec of dequantization results (preserves order).
pub fn batch_dequantize<T: Quantizable + Send>(
    quantized: &[QuantizedEmbedding],
) -> Vec<Result<T, QuantizationError>> {
    quantized.par_iter().map(T::dequantize).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::TokenPruningEmbedding;

    fn test_embedding(num_tokens: usize) -> TokenPruningEmbedding {
        let values: Vec<f32> = (0..(num_tokens * 128))
            .map(|i| (i as f32 / 1000.0) - 0.5)
            .collect();
        TokenPruningEmbedding::new(values, num_tokens)
    }

    #[test]
    fn test_batch_quantize() {
        let embeddings: Vec<_> = (0..5).map(|_| test_embedding(10)).collect();

        let results = batch_quantize(&embeddings, Precision::Int8);

        assert_eq!(results.len(), 5);
        for (i, result) in results.iter().enumerate() {
            let quantized = result
                .as_ref()
                .unwrap_or_else(|e| panic!("batch_quantize[{}] failed: {}", i, e));
            assert_eq!(quantized.precision, Precision::Int8);
            assert!(
                !quantized.data.is_empty(),
                "quantized data should not be empty"
            );
        }
    }

    #[test]
    fn test_batch_dequantize_round_trip() {
        let embeddings: Vec<_> = (0..5).map(|_| test_embedding(10)).collect();
        let quantized: Vec<_> = embeddings
            .iter()
            .map(|e| e.quantize(Precision::Int8).unwrap())
            .collect();

        let results: Vec<Result<TokenPruningEmbedding, _>> = batch_dequantize(&quantized);

        assert_eq!(results.len(), 5);
        for (i, result) in results.iter().enumerate() {
            let reconstructed = result
                .as_ref()
                .unwrap_or_else(|e| panic!("batch_dequantize[{}] failed: {}", i, e));
            // Verify round-trip preserves structure
            assert_eq!(
                reconstructed.num_tokens, embeddings[i].num_tokens,
                "round-trip should preserve token count"
            );
            assert_eq!(
                reconstructed.values.len(),
                embeddings[i].values.len(),
                "round-trip should preserve value count"
            );
            // Int8 quantization introduces error; verify values are close
            for (j, (orig, recon)) in embeddings[i]
                .values
                .iter()
                .zip(reconstructed.values.iter())
                .enumerate()
            {
                assert!(
                    (orig - recon).abs() < 0.01,
                    "round-trip error too large at [{}][{}]: orig={}, reconstructed={}",
                    i,
                    j,
                    orig,
                    recon
                );
            }
        }
    }

    #[test]
    fn test_batch_preserves_order() {
        // Create embeddings with distinct values
        let embeddings: Vec<_> = (0..10)
            .map(|i| {
                let values: Vec<f32> = (0..128).map(|j| i as f32 + j as f32 / 1000.0).collect();
                TokenPruningEmbedding::new(values, 1)
            })
            .collect();

        let quantized: Vec<_> = batch_quantize(&embeddings, Precision::Fp16);
        let reconstructed: Vec<Result<TokenPruningEmbedding, _>> = batch_dequantize(
            &quantized
                .iter()
                .map(|r| r.as_ref().unwrap().clone())
                .collect::<Vec<_>>(),
        );

        // Verify order preserved by checking first value of each embedding
        for (i, result) in reconstructed.iter().enumerate() {
            let emb = result.as_ref().unwrap();
            // First value should be approximately i + 0/1000 = i
            let expected = i as f32;
            assert!(
                (emb.values[0] - expected).abs() < 0.1,
                "Order not preserved: expected ~{}, got {}",
                expected,
                emb.values[0]
            );
        }
    }
}
