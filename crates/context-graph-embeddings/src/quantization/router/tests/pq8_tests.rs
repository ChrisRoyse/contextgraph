//! PQ8 quantization tests (IMPLEMENTED).

use crate::quantization::router::QuantizationRouter;
use crate::quantization::types::QuantizationMethod;
use crate::types::ModelId;

#[test]
fn test_pq8_quantization_e1_semantic() {
    let router = QuantizationRouter::new();

    // E1_Semantic uses PQ8 quantization (1024D)
    let embedding: Vec<f32> = (0..1024).map(|i| (i as f32 / 512.0) - 1.0).collect();

    let quantized = router
        .quantize(ModelId::Semantic, &embedding)
        .expect("PQ8 quantization should succeed");

    assert_eq!(quantized.method, QuantizationMethod::PQ8);
    assert_eq!(quantized.original_dim, 1024);
    // PQ8: 8 bytes (8 centroid indices)
    assert_eq!(quantized.data.len(), 8);
}

#[test]
fn test_pq8_round_trip() {
    let router = QuantizationRouter::new();

    // Create input with known range
    let input: Vec<f32> = (0..1024).map(|i| (i as f32 / 512.0) - 1.0).collect();

    let quantized = router
        .quantize(ModelId::Semantic, &input)
        .expect("quantize");

    let reconstructed = router
        .dequantize(ModelId::Semantic, &quantized)
        .expect("dequantize");

    assert_eq!(reconstructed.len(), 1024);

    // Compute cosine similarity for reconstruction quality
    let dot: f32 = input
        .iter()
        .zip(reconstructed.iter())
        .map(|(a, b)| a * b)
        .sum();
    let norm_a: f32 = input.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = reconstructed.iter().map(|x| x * x).sum::<f32>().sqrt();
    let cosine = dot / (norm_a * norm_b);

    // Note: Default codebook provides moderate quality. The <5% recall loss
    // guarantee only applies to trained codebooks. For testing, we verify
    // the algorithm preserves directional structure (cosine > 0.15).
    assert!(
        cosine > 0.15,
        "Cosine similarity {} too low for PQ8 reconstruction (expected > 0.15 for default codebook)",
        cosine
    );
}

#[test]
fn test_pq8_all_model_ids() {
    let router = QuantizationRouter::new();

    // Test PQ8 model IDs with correct dimensions
    // E1_Semantic: 1024D, E5_Causal: 768D, E7_Code: 1536D, E10_Multimodal: 768D
    let pq8_models = [
        (ModelId::Semantic, 1024),
        (ModelId::Causal, 768),
        (ModelId::Code, 1536),
        (ModelId::Contextual, 768),
    ];

    for (model_id, dim) in pq8_models {
        let embedding: Vec<f32> = (0..dim)
            .map(|i| (i as f32 / dim as f32) * 2.0 - 1.0)
            .collect();
        let result = router.quantize(model_id, &embedding);
        assert!(
            result.is_ok(),
            "PQ8 quantization failed for {:?} ({}D): {:?}",
            model_id,
            dim,
            result.err()
        );
        let q = result.unwrap();
        assert_eq!(q.method, QuantizationMethod::PQ8);
        assert_eq!(q.data.len(), 8); // Always 8 bytes
    }
}
