//! Test Data Generation Helpers (NO MOCKS - Real Types with Deterministic Data)

use context_graph_embeddings::{
    storage::NUM_EMBEDDERS, ModelId, QuantizationMetadata, QuantizationMethod, QuantizedEmbedding,
};
use std::collections::HashMap;

/// Creates valid test embeddings for all production embedders with Constitution-correct methods.
/// Each embedding uses deterministic data that can be verified after roundtrip.
pub fn create_test_embeddings_with_deterministic_data(seed: u8) -> HashMap<u8, QuantizedEmbedding> {
    let mut map = HashMap::new();

    for (i, model_id) in ModelId::production().iter().copied().enumerate() {
        assert!(
            i < NUM_EMBEDDERS,
            "production() returned more models than NUM_EMBEDDERS"
        );
        let slot = i as u8;
        let method = QuantizationMethod::for_model_id(model_id);

        // Create method-appropriate test data with deterministic pattern
        let (dim, data, metadata) = match method {
            QuantizationMethod::PQ8 => {
                // PQ8: 8 bytes of centroid indices
                let data: Vec<u8> = (0..8u8)
                    .map(|j| seed.wrapping_add(slot).wrapping_add(j))
                    .collect();
                let dim = model_id.dimension();
                let metadata = QuantizationMetadata::PQ8 {
                    codebook_id: slot as u32 + seed as u32 * 100,
                    num_subvectors: 8,
                };
                (dim, data, metadata)
            }
            QuantizationMethod::Float8E4M3 => {
                // Float8: 1 byte per dimension (compressed from f32)
                let dim = model_id.dimension();
                let data: Vec<u8> = (0..dim)
                    .map(|j| {
                        ((seed as usize).wrapping_add(slot as usize).wrapping_add(j) & 0xFF) as u8
                    })
                    .collect();
                let metadata = QuantizationMetadata::Float8 {
                    scale: 1.0 + (seed as f32 * 0.1),
                    bias: seed as f32 * 0.01,
                };
                (dim, data, metadata)
            }
            QuantizationMethod::Binary => {
                // Binary: 10000 bits = 1250 bytes for E9 HDC
                let dim = 10000;
                let data: Vec<u8> = (0..1250)
                    .map(|j| {
                        ((seed as usize).wrapping_add(slot as usize).wrapping_add(j) & 0xFF) as u8
                    })
                    .collect();
                let metadata = QuantizationMetadata::Binary {
                    threshold: 0.0 + seed as f32 * 0.001,
                };
                (dim, data, metadata)
            }
            QuantizationMethod::SparseNative => {
                // Sparse: Variable size based on nnz (100 entries typical for test)
                let nnz = 100;
                // Each sparse entry: 4 bytes index + 4 bytes value = 8 bytes
                let data: Vec<u8> = (0..(nnz * 8))
                    .map(|j| {
                        ((seed as usize).wrapping_add(slot as usize).wrapping_add(j) & 0xFF) as u8
                    })
                    .collect();
                let metadata = QuantizationMetadata::Sparse {
                    vocab_size: 30522,
                    nnz,
                };
                (30522, data, metadata)
            }
            QuantizationMethod::TokenPruning => {
                // TokenPruning: ~50% of tokens kept, 128D per token, ~64 tokens
                let kept_tokens = 64;
                let data: Vec<u8> = (0..(kept_tokens * 128))
                    .map(|j| {
                        ((seed as usize).wrapping_add(slot as usize).wrapping_add(j) & 0xFF) as u8
                    })
                    .collect();
                let metadata = QuantizationMetadata::TokenPruning {
                    original_tokens: 128,
                    kept_tokens,
                    threshold: 0.5 + seed as f32 * 0.01,
                };
                (128, data, metadata)
            }
        };

        map.insert(
            slot,
            QuantizedEmbedding {
                method,
                original_dim: dim,
                data,
                metadata,
            },
        );
    }

    assert_eq!(
        map.len(),
        NUM_EMBEDDERS,
        "test helper must create one embedding per production storage slot"
    );
    map
}

/// Create deterministic topic profile based on seed.
pub fn create_topic_profile(seed: u8) -> [f32; 14] {
    let mut pv = [0.0f32; 14];
    for (i, val) in pv.iter_mut().enumerate() {
        // Generate values in [0.3, 0.9] range
        *val = 0.3 + ((seed as f32 + i as f32 * 0.05) % 0.6);
    }
    pv
}

/// Create deterministic content hash based on seed.
pub fn create_content_hash(seed: u8) -> [u8; 32] {
    let mut hash = [0u8; 32];
    for (i, byte) in hash.iter_mut().enumerate() {
        *byte = seed.wrapping_add(i as u8).wrapping_mul(17);
    }
    hash
}
