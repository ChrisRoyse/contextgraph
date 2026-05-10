//! Router initialization and method routing tests.

use crate::quantization::router::QuantizationRouter;
use crate::quantization::types::QuantizationMethod;
use crate::types::ModelId;

// =========================================================================
// Router initialization tests
// =========================================================================

#[test]
fn test_router_new() {
    let router = QuantizationRouter::new();
    // Verify it initializes without panic
    assert!(router.can_quantize(ModelId::Hdc));
}

#[test]
fn test_router_default() {
    let router = QuantizationRouter::default();
    assert!(router.can_quantize(ModelId::Hdc));
}

// =========================================================================
// Method routing tests
// =========================================================================

#[test]
fn test_all_model_ids_have_method() {
    let router = QuantizationRouter::new();

    // Verify all ModelIds return a valid method (no panic)
    for model_id in ModelId::all() {
        let method = router.method_for(*model_id);
        // Just verify it doesn't panic and returns something
        let _ = method.compression_ratio();
    }

    // Verify specific mappings per Constitution
    assert_eq!(
        router.method_for(ModelId::Semantic),
        QuantizationMethod::PQ8
    );
    assert_eq!(router.method_for(ModelId::Causal), QuantizationMethod::PQ8);
    assert_eq!(router.method_for(ModelId::Code), QuantizationMethod::PQ8);
    assert_eq!(
        router.method_for(ModelId::Contextual),
        QuantizationMethod::PQ8
    );
    assert_eq!(router.method_for(ModelId::Kepler), QuantizationMethod::PQ8);
    assert_eq!(
        router.method_for(ModelId::BgeM3Dense),
        QuantizationMethod::PQ8
    );

    assert_eq!(
        router.method_for(ModelId::TemporalRecent),
        QuantizationMethod::Float8E4M3
    );
    assert_eq!(
        router.method_for(ModelId::TemporalPeriodic),
        QuantizationMethod::Float8E4M3
    );
    assert_eq!(
        router.method_for(ModelId::TemporalPositional),
        QuantizationMethod::Float8E4M3
    );
    assert_eq!(
        router.method_for(ModelId::Graph),
        QuantizationMethod::Float8E4M3
    );
    assert_eq!(
        router.method_for(ModelId::Entity),
        QuantizationMethod::Float8E4M3
    );

    assert_eq!(router.method_for(ModelId::Hdc), QuantizationMethod::Binary);

    assert_eq!(
        router.method_for(ModelId::Sparse),
        QuantizationMethod::SparseNative
    );
    assert_eq!(
        router.method_for(ModelId::Splade),
        QuantizationMethod::SparseNative
    );

    assert_eq!(
        router.method_for(ModelId::LateInteraction),
        QuantizationMethod::TokenPruning
    );
}

// =========================================================================
// can_quantize tests
// =========================================================================

#[test]
fn test_can_quantize() {
    let router = QuantizationRouter::new();

    // Binary: implemented
    assert!(router.can_quantize(ModelId::Hdc));

    // PQ8: IMPLEMENTED
    assert!(router.can_quantize(ModelId::Semantic));
    assert!(router.can_quantize(ModelId::Causal));
    assert!(router.can_quantize(ModelId::Code));
    assert!(router.can_quantize(ModelId::Contextual));
    assert!(router.can_quantize(ModelId::Kepler));
    assert!(router.can_quantize(ModelId::BgeM3Dense));

    // Float8: IMPLEMENTED
    assert!(router.can_quantize(ModelId::TemporalRecent));
    assert!(router.can_quantize(ModelId::TemporalPeriodic));
    assert!(router.can_quantize(ModelId::TemporalPositional));
    assert!(router.can_quantize(ModelId::Graph));
    assert!(router.can_quantize(ModelId::Entity));

    // Sparse: invalid path (not a dense quantization)
    assert!(!router.can_quantize(ModelId::Sparse));
    assert!(!router.can_quantize(ModelId::Splade));

    // TokenPruning: out of scope
    assert!(!router.can_quantize(ModelId::LateInteraction));
}

// =========================================================================
// expected_size tests
// =========================================================================

#[test]
fn test_expected_size_binary() {
    let router = QuantizationRouter::new();

    // Binary: ceil(dim / 8)
    assert_eq!(router.expected_size(ModelId::Hdc, 10000), 1250);
    assert_eq!(router.expected_size(ModelId::Hdc, 1024), 128);
    assert_eq!(router.expected_size(ModelId::Hdc, 8), 1);
    assert_eq!(router.expected_size(ModelId::Hdc, 9), 2);
}

#[test]
fn test_expected_size_float8() {
    let router = QuantizationRouter::new();

    // Float8: 1 byte per element
    assert_eq!(router.expected_size(ModelId::TemporalRecent, 512), 512);
    assert_eq!(router.expected_size(ModelId::Graph, 1024), 1024); // e5-large-v2 (upgraded from MiniLM 384D)
}

#[test]
fn test_expected_size_pq8() {
    let router = QuantizationRouter::new();

    // PQ8: always 8 bytes (8 subvectors)
    assert_eq!(router.expected_size(ModelId::Semantic, 1024), 8);
    assert_eq!(router.expected_size(ModelId::Code, 1536), 8);
}

#[test]
fn test_expected_size_sparse_unknown() {
    let router = QuantizationRouter::new();

    // Sparse: Variable, returns 0
    assert_eq!(router.expected_size(ModelId::Sparse, 30522), 0);
    assert_eq!(router.expected_size(ModelId::Splade, 30522), 0);
}

#[test]
fn test_expected_size_token_pruning_unknown() {
    let router = QuantizationRouter::new();

    // TokenPruning: Variable, returns 0
    assert_eq!(router.expected_size(ModelId::LateInteraction, 128), 0);
}
