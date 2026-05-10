//! Quantization method tests for all embedder models.
//!
//! Tests that each model uses the correct quantization method as specified
//! in the Constitution.

use context_graph_embeddings::{ModelId, QuantizationMethod};

use super::constants::EXPECTED_QUANTIZATION;

/// Test PQ8 models: E1, E5, E7, E10, production Kepler, and E14.
#[test]
fn test_pq8_models() {
    let pq8_models = [
        (ModelId::Semantic, "E1"),
        (ModelId::Causal, "E5"),
        (ModelId::Code, "E7"),
        (ModelId::Contextual, "E10"),
        (ModelId::Kepler, "Kepler"),
        (ModelId::BgeM3Dense, "E14"),
    ];

    for (model_id, label) in pq8_models {
        assert_eq!(
            QuantizationMethod::for_model_id(model_id),
            QuantizationMethod::PQ8,
            "{} {:?} should use PQ8 quantization",
            label,
            model_id
        );
    }
    println!("[PASS] All PQ8 models (E1, E5, E7, E10, Kepler, E14) verified");
}

/// Test Float8E4M3 models: E2, E3, E4, E8, E11.
#[test]
fn test_float8_models() {
    let float8_models = [
        (ModelId::TemporalRecent, "E2"),
        (ModelId::TemporalPeriodic, "E3"),
        (ModelId::TemporalPositional, "E4"),
        (ModelId::Graph, "E8"),
        (ModelId::Entity, "E11"),
    ];

    for (model_id, label) in float8_models {
        assert_eq!(
            QuantizationMethod::for_model_id(model_id),
            QuantizationMethod::Float8E4M3,
            "{} {:?} should use Float8E4M3 quantization",
            label,
            model_id
        );
    }
    println!("[PASS] All Float8E4M3 models (E2, E3, E4, E8, E11) verified");
}

/// Test Binary model: E9 Hdc.
#[test]
fn test_binary_model() {
    assert_eq!(
        QuantizationMethod::for_model_id(ModelId::Hdc),
        QuantizationMethod::Binary,
        "E9 Hdc should use Binary quantization"
    );
    println!("[PASS] Binary model (E9) verified");
}

/// Test SparseNative models: E6, E13.
#[test]
fn test_sparse_native_models() {
    let sparse_models = [(ModelId::Sparse, "E6"), (ModelId::Splade, "E13")];

    for (model_id, label) in sparse_models {
        assert_eq!(
            QuantizationMethod::for_model_id(model_id),
            QuantizationMethod::SparseNative,
            "{} {:?} should use SparseNative quantization",
            label,
            model_id
        );
    }
    println!("[PASS] All SparseNative models (E6, E13) verified");
}

/// Test TokenPruning model: E12 LateInteraction.
#[test]
fn test_token_pruning_model() {
    assert_eq!(
        QuantizationMethod::for_model_id(ModelId::LateInteraction),
        QuantizationMethod::TokenPruning,
        "E12 LateInteraction should use TokenPruning quantization"
    );
    println!("[PASS] TokenPruning model (E12) verified");
}

/// Test ALL quantization methods match expected values in one sweep.
/// FAIL FAST: Any single mismatch panics with details.
#[test]
fn test_all_quantization_methods_match_constitution() {
    for (model_id, expected_method) in &EXPECTED_QUANTIZATION {
        let actual_method = QuantizationMethod::for_model_id(*model_id);
        assert_eq!(
            actual_method, *expected_method,
            "Quantization method mismatch for {:?}: expected {:?}, got {:?}",
            model_id, expected_method, actual_method
        );
    }
    println!(
        "[PASS] All {} quantization methods match Constitution",
        EXPECTED_QUANTIZATION.len()
    );
}

/// Test compression ratios match Constitution specifications.
#[test]
fn test_quantization_compression_ratios() {
    assert_eq!(
        QuantizationMethod::PQ8.compression_ratio(),
        32.0,
        "PQ8 compression ratio should be 32x"
    );
    assert_eq!(
        QuantizationMethod::Float8E4M3.compression_ratio(),
        4.0,
        "Float8E4M3 compression ratio should be 4x"
    );
    assert_eq!(
        QuantizationMethod::Binary.compression_ratio(),
        32.0,
        "Binary compression ratio should be 32x"
    );
    assert_eq!(
        QuantizationMethod::SparseNative.compression_ratio(),
        1.0,
        "SparseNative compression ratio should be 1.0 (variable)"
    );
    assert_eq!(
        QuantizationMethod::TokenPruning.compression_ratio(),
        2.0,
        "TokenPruning compression ratio should be 2x"
    );
    println!("[PASS] All quantization compression ratios verified");
}

/// Test maximum recall loss values match Constitution specifications.
#[test]
fn test_quantization_max_recall_loss() {
    assert_eq!(
        QuantizationMethod::PQ8.max_recall_loss(),
        0.05,
        "PQ8 max recall loss should be 5%"
    );
    assert_eq!(
        QuantizationMethod::Float8E4M3.max_recall_loss(),
        0.003,
        "Float8E4M3 max recall loss should be 0.3%"
    );
    assert_eq!(
        QuantizationMethod::Binary.max_recall_loss(),
        0.10,
        "Binary max recall loss should be 10%"
    );
    assert_eq!(
        QuantizationMethod::SparseNative.max_recall_loss(),
        0.0,
        "SparseNative max recall loss should be 0%"
    );
    assert_eq!(
        QuantizationMethod::TokenPruning.max_recall_loss(),
        0.02,
        "TokenPruning max recall loss should be 2%"
    );
    println!("[PASS] All quantization max recall loss values verified");
}
