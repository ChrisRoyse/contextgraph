//! TASK-EMB-020 Physical Verification Tests
//!
//! These tests verify the QuantizationRouter implementation meets all requirements:
//! - Binary quantization works end-to-end for E9_Hdc
//! - PQ8/Float8 return QuantizerNotImplemented error
//! - SparseNative rejects dense vectors
//! - All 13 ModelIds have method assignments
//!
//! Per Constitution AP-007: NO STUB DATA. All tests use real algorithms.

use context_graph_embeddings::quantization::{QuantizationMethod, QuantizationRouter};
use context_graph_embeddings::types::ModelId;
use context_graph_embeddings::EmbeddingError;

// ============================================================================
// Physical Verification: File Existence
// ============================================================================

/// PV-001: Verify QuantizationRouter is accessible from public API.
#[test]
fn pv_001_router_is_public() {
    // This test compiles only if QuantizationRouter is publicly exported
    let _router = QuantizationRouter::new();
}

// ============================================================================
// Physical Verification: Binary Quantization for E9_Hdc
// ============================================================================

/// PV-002: Binary quantization produces correct compressed size.
///
/// Source of Truth: QuantizedEmbedding.data.len() == original_dim / 8
#[test]
fn pv_002_binary_quantization_produces_correct_size() {
    let router = QuantizationRouter::new();

    // E9_Hdc: 10,000-bit HDC vector
    let hdc_embedding: Vec<f32> = (0..10000)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();

    let quantized = router
        .quantize(ModelId::Hdc, &hdc_embedding)
        .expect("Binary quantization should succeed");

    // PHYSICAL VERIFICATION:
    // 10,000 bits / 8 = 1250 bytes
    println!("PV-002 Evidence:");
    println!("  Input dimension: {}", hdc_embedding.len());
    println!("  Output bytes: {}", quantized.data.len());
    println!("  Expected bytes: {}", 10000 / 8);
    println!("  Method: {:?}", quantized.method);

    assert_eq!(quantized.data.len(), 1250);
    assert_eq!(quantized.method, QuantizationMethod::Binary);
    assert_eq!(quantized.original_dim, 10000);
}

/// PV-003: Binary roundtrip preserves sign information.
///
/// Source of Truth: Dequantized signs match original signs.
#[test]
fn pv_003_binary_roundtrip_preserves_signs() {
    let router = QuantizationRouter::new();

    // Create known pattern: alternating +/-
    let original: Vec<f32> = (0..1024)
        .map(|i| if i % 2 == 0 { 0.75 } else { -0.75 })
        .collect();

    let quantized = router
        .quantize(ModelId::Hdc, &original)
        .expect("quantize");
    let reconstructed = router
        .dequantize(ModelId::Hdc, &quantized)
        .expect("dequantize");

    // PHYSICAL VERIFICATION: Count sign matches
    let mut matches = 0;
    let mut mismatches = 0;

    for (i, (&orig, &recon)) in original.iter().zip(reconstructed.iter()).enumerate() {
        let orig_positive = orig >= 0.0;
        let recon_positive = recon >= 0.0;
        if orig_positive == recon_positive {
            matches += 1;
        } else {
            mismatches += 1;
            println!(
                "  Mismatch at {}: orig={}, recon={}",
                i, orig, recon
            );
        }
    }

    println!("PV-003 Evidence:");
    println!("  Total elements: {}", original.len());
    println!("  Sign matches: {}", matches);
    println!("  Sign mismatches: {}", mismatches);

    assert_eq!(mismatches, 0, "All signs should match");
}

// ============================================================================
// Physical Verification: PQ8/Float8 Return QuantizerNotImplemented
// ============================================================================

/// PV-004: PQ8 quantization returns QuantizerNotImplemented error.
///
/// Affected models: E1 (Semantic), E5 (Causal), E7 (Code), E10 (Multimodal)
#[test]
fn pv_004_pq8_returns_not_implemented() {
    let router = QuantizationRouter::new();

    let pq8_models = [
        (ModelId::Semantic, "E1"),
        (ModelId::Causal, "E5"),
        (ModelId::Code, "E7"),
        (ModelId::Multimodal, "E10"),
    ];

    println!("PV-004 Evidence:");

    for (model_id, name) in pq8_models {
        let embedding = vec![0.5f32; 1024];
        let result = router.quantize(model_id, &embedding);

        assert!(result.is_err(), "{} should fail", name);

        match result.unwrap_err() {
            EmbeddingError::QuantizerNotImplemented { model_id: m, method } => {
                println!("  {} ({:?}): QuantizerNotImplemented, method={}", name, m, method);
                assert_eq!(method, "PQ8");
            }
            e => panic!("Expected QuantizerNotImplemented for {}, got {:?}", name, e),
        }
    }
}

/// PV-005: Float8E4M3 quantization returns QuantizerNotImplemented error.
///
/// Affected models: E2-E4 (Temporal), E8 (Graph), E11 (Entity)
#[test]
fn pv_005_float8_returns_not_implemented() {
    let router = QuantizationRouter::new();

    let float8_models = [
        (ModelId::TemporalRecent, "E2"),
        (ModelId::TemporalPeriodic, "E3"),
        (ModelId::TemporalPositional, "E4"),
        (ModelId::Graph, "E8"),
        (ModelId::Entity, "E11"),
    ];

    println!("PV-005 Evidence:");

    for (model_id, name) in float8_models {
        let embedding = vec![0.5f32; 512];
        let result = router.quantize(model_id, &embedding);

        assert!(result.is_err(), "{} should fail", name);

        match result.unwrap_err() {
            EmbeddingError::QuantizerNotImplemented { model_id: m, method } => {
                println!("  {} ({:?}): QuantizerNotImplemented, method={}", name, m, method);
                assert_eq!(method, "Float8E4M3");
            }
            e => panic!("Expected QuantizerNotImplemented for {}, got {:?}", name, e),
        }
    }
}

// ============================================================================
// Physical Verification: SparseNative Rejects Dense
// ============================================================================

/// PV-006: Sparse models reject dense quantization path.
///
/// Affected models: E6 (Sparse), E13 (Splade)
#[test]
fn pv_006_sparse_rejects_dense_vectors() {
    let router = QuantizationRouter::new();

    let sparse_models = [(ModelId::Sparse, "E6"), (ModelId::Splade, "E13")];

    println!("PV-006 Evidence:");

    for (model_id, name) in sparse_models {
        let embedding = vec![0.0f32; 30522]; // SPLADE vocab size
        let result = router.quantize(model_id, &embedding);

        assert!(result.is_err(), "{} should fail", name);

        match result.unwrap_err() {
            EmbeddingError::InvalidModelInput { model_id: m, reason } => {
                println!("  {} ({:?}): InvalidModelInput, reason={}", name, m, reason);
                assert!(reason.contains("Sparse"));
            }
            e => panic!("Expected InvalidModelInput for {}, got {:?}", name, e),
        }
    }
}

// ============================================================================
// Physical Verification: All 13 ModelIds Have Assignments
// ============================================================================

/// PV-007: All 13 ModelIds have quantization method assignments.
///
/// Source of Truth: No panic when calling method_for on any ModelId.
#[test]
fn pv_007_all_model_ids_have_method_assignments() {
    let router = QuantizationRouter::new();

    let all_models = ModelId::all();
    assert_eq!(all_models.len(), 13, "Expected exactly 13 ModelId variants");

    println!("PV-007 Evidence:");
    println!("  Total ModelIds: {}", all_models.len());
    println!("  Assignments:");

    for model_id in all_models {
        let method = router.method_for(*model_id);
        let can_quantize = router.can_quantize(*model_id);

        println!(
            "    {:?}: {:?} (can_quantize={})",
            model_id, method, can_quantize
        );
    }

    // Verify expected methods per Constitution
    assert_eq!(router.method_for(ModelId::Semantic), QuantizationMethod::PQ8);
    assert_eq!(router.method_for(ModelId::TemporalRecent), QuantizationMethod::Float8E4M3);
    assert_eq!(router.method_for(ModelId::Hdc), QuantizationMethod::Binary);
    assert_eq!(router.method_for(ModelId::Sparse), QuantizationMethod::SparseNative);
    assert_eq!(router.method_for(ModelId::LateInteraction), QuantizationMethod::TokenPruning);
}

// ============================================================================
// Physical Verification: Compression Ratios
// ============================================================================

/// PV-008: Binary quantization achieves 32x compression.
///
/// 10,000 f32 values = 40,000 bytes → 1,250 bytes = 32x
#[test]
fn pv_008_binary_achieves_32x_compression() {
    let router = QuantizationRouter::new();

    let dim = 10000;
    let embedding: Vec<f32> = (0..dim).map(|i| (i as f32) / 100.0).collect();

    let original_size = dim * std::mem::size_of::<f32>();
    let quantized = router.quantize(ModelId::Hdc, &embedding).expect("quantize");
    let compressed_size = quantized.data.len();

    let compression_ratio = original_size as f64 / compressed_size as f64;

    println!("PV-008 Evidence:");
    println!("  Original size: {} bytes ({} f32s)", original_size, dim);
    println!("  Compressed size: {} bytes", compressed_size);
    println!("  Compression ratio: {:.1}x", compression_ratio);

    // Binary: 1 bit per f32 = 32x compression
    assert!(
        compression_ratio > 31.0 && compression_ratio < 33.0,
        "Expected ~32x compression, got {:.1}x",
        compression_ratio
    );
}
