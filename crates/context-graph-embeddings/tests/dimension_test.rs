//! Integration tests for all 13 embedder dimension validations.
//!
//! FAIL FAST: Any dimension mismatch is a critical error requiring immediate fix.
//! NO FALLBACKS: All assertions must match Constitution exactly.
//!
//! # Constitution Reference
//!
//! | Model | Native Dim | Projected Dim | Quantization |
//! |-------|------------|---------------|--------------|
//! | E1 Semantic | 1024 | 1024 | PQ8 |
//! | E2 TemporalRecent | 512 | 512 | Float8E4M3 |
//! | E3 TemporalPeriodic | 512 | 512 | Float8E4M3 |
//! | E4 TemporalPositional | 512 | 512 | Float8E4M3 |
//! | E5 Causal | 768 | 768 | PQ8 |
//! | E6 Sparse | 30522 | 1536 | SparseNative |
//! | E7 Code | 1536 | 1536 | PQ8 |
//! | E8 Graph | 384 | 384 | Float8E4M3 |
//! | E9 Hdc | 10000 | 1024 | Binary |
//! | E10 Multimodal | 768 | 768 | PQ8 |
//! | E11 Entity | 384 | 384 | Float8E4M3 |
//! | E12 LateInteraction | 128 | 128 | TokenPruning |
//! | E13 Splade | 30522 | 1536 | SparseNative |
//!
//! TOTAL_DIMENSION = 9856

use context_graph_embeddings::{
    dimensions::{
        // Helper functions
        native_dimension_by_index,
        offset_by_index,
        projected_dimension_by_index,
        CAUSAL,
        CAUSAL_NATIVE,
        CODE,
        CODE_NATIVE,
        ENTITY,
        ENTITY_NATIVE,
        GRAPH,
        GRAPH_NATIVE,
        HDC,
        HDC_NATIVE,
        LATE_INTERACTION,
        LATE_INTERACTION_NATIVE,
        // Aggregate dimensions
        MODEL_COUNT,
        MULTIMODAL,
        MULTIMODAL_NATIVE,
        // Arrays
        NATIVE_DIMENSIONS,
        OFFSETS,
        PROJECTED_DIMENSIONS,
        // Projected dimensions
        SEMANTIC,
        // Native dimensions
        SEMANTIC_NATIVE,
        SPARSE,
        SPARSE_NATIVE,
        SPLADE,
        SPLADE_NATIVE,
        TEMPORAL_PERIODIC,
        TEMPORAL_PERIODIC_NATIVE,
        TEMPORAL_POSITIONAL,
        TEMPORAL_POSITIONAL_NATIVE,
        TEMPORAL_RECENT,
        TEMPORAL_RECENT_NATIVE,
        TOTAL_DIMENSION,
    },
    ModelId, QuantizationMethod,
};

// =============================================================================
// CONSTITUTION-DEFINED EXPECTED VALUES (Fail-Fast Reference)
// =============================================================================

/// Expected native dimensions for all 13 models.
/// These are the raw output dimensions from each model before projection.
const EXPECTED_NATIVE_DIMS: [(ModelId, usize); 13] = [
    (ModelId::Semantic, 1024),          // E1
    (ModelId::TemporalRecent, 512),     // E2
    (ModelId::TemporalPeriodic, 512),   // E3
    (ModelId::TemporalPositional, 512), // E4
    (ModelId::Causal, 768),             // E5
    (ModelId::Sparse, 30522),           // E6
    (ModelId::Code, 1536),              // E7
    (ModelId::Graph, 384),              // E8
    (ModelId::Hdc, 10000),              // E9
    (ModelId::Multimodal, 768),         // E10
    (ModelId::Entity, 384),             // E11
    (ModelId::LateInteraction, 128),    // E12
    (ModelId::Splade, 30522),           // E13
];

/// Expected projected dimensions for all 13 models.
/// These are the dimensions used for Multi-Array Storage.
const EXPECTED_PROJECTED_DIMS: [(ModelId, usize); 13] = [
    (ModelId::Semantic, 1024),          // E1 - no projection
    (ModelId::TemporalRecent, 512),     // E2 - no projection
    (ModelId::TemporalPeriodic, 512),   // E3 - no projection
    (ModelId::TemporalPositional, 512), // E4 - no projection
    (ModelId::Causal, 768),             // E5 - no projection
    (ModelId::Sparse, 1536),            // E6 - 30K -> 1536
    (ModelId::Code, 1536),              // E7 - native 1536D
    (ModelId::Graph, 384),              // E8 - no projection
    (ModelId::Hdc, 1024),               // E9 - 10K -> 1024
    (ModelId::Multimodal, 768),         // E10 - no projection
    (ModelId::Entity, 384),             // E11 - no projection
    (ModelId::LateInteraction, 128),    // E12 - no projection
    (ModelId::Splade, 1536),            // E13 - 30K -> 1536
];

/// Expected quantization methods for all 13 models.
const EXPECTED_QUANTIZATION: [(ModelId, QuantizationMethod); 13] = [
    (ModelId::Semantic, QuantizationMethod::PQ8), // E1
    (ModelId::TemporalRecent, QuantizationMethod::Float8E4M3), // E2
    (ModelId::TemporalPeriodic, QuantizationMethod::Float8E4M3), // E3
    (ModelId::TemporalPositional, QuantizationMethod::Float8E4M3), // E4
    (ModelId::Causal, QuantizationMethod::PQ8),   // E5
    (ModelId::Sparse, QuantizationMethod::SparseNative), // E6
    (ModelId::Code, QuantizationMethod::PQ8),     // E7
    (ModelId::Graph, QuantizationMethod::Float8E4M3), // E8
    (ModelId::Hdc, QuantizationMethod::Binary),   // E9
    (ModelId::Multimodal, QuantizationMethod::PQ8), // E10
    (ModelId::Entity, QuantizationMethod::Float8E4M3), // E11
    (ModelId::LateInteraction, QuantizationMethod::TokenPruning), // E12
    (ModelId::Splade, QuantizationMethod::SparseNative), // E13
];

/// Expected total dimension sum.
const EXPECTED_TOTAL_DIMENSION: usize = 9856;

/// Expected model count.
const EXPECTED_MODEL_COUNT: usize = 13;

// =============================================================================
// NATIVE DIMENSION TESTS
// =============================================================================

#[cfg(test)]
mod native_dimension_tests {
    use super::*;

    /// Test E1 Semantic native dimension matches Constitution.
    #[test]
    fn test_e1_semantic_native_dimension() {
        assert_eq!(
            ModelId::Semantic.dimension(),
            1024,
            "E1 Semantic: expected native dimension 1024"
        );
        assert_eq!(SEMANTIC_NATIVE, 1024, "SEMANTIC_NATIVE constant mismatch");
    }

    /// Test E2 TemporalRecent native dimension matches Constitution.
    #[test]
    fn test_e2_temporal_recent_native_dimension() {
        assert_eq!(
            ModelId::TemporalRecent.dimension(),
            512,
            "E2 TemporalRecent: expected native dimension 512"
        );
        assert_eq!(
            TEMPORAL_RECENT_NATIVE, 512,
            "TEMPORAL_RECENT_NATIVE constant mismatch"
        );
    }

    /// Test E3 TemporalPeriodic native dimension matches Constitution.
    #[test]
    fn test_e3_temporal_periodic_native_dimension() {
        assert_eq!(
            ModelId::TemporalPeriodic.dimension(),
            512,
            "E3 TemporalPeriodic: expected native dimension 512"
        );
        assert_eq!(
            TEMPORAL_PERIODIC_NATIVE, 512,
            "TEMPORAL_PERIODIC_NATIVE constant mismatch"
        );
    }

    /// Test E4 TemporalPositional native dimension matches Constitution.
    #[test]
    fn test_e4_temporal_positional_native_dimension() {
        assert_eq!(
            ModelId::TemporalPositional.dimension(),
            512,
            "E4 TemporalPositional: expected native dimension 512"
        );
        assert_eq!(
            TEMPORAL_POSITIONAL_NATIVE, 512,
            "TEMPORAL_POSITIONAL_NATIVE constant mismatch"
        );
    }

    /// Test E5 Causal native dimension matches Constitution.
    #[test]
    fn test_e5_causal_native_dimension() {
        assert_eq!(
            ModelId::Causal.dimension(),
            768,
            "E5 Causal: expected native dimension 768"
        );
        assert_eq!(CAUSAL_NATIVE, 768, "CAUSAL_NATIVE constant mismatch");
    }

    /// Test E6 Sparse native dimension matches Constitution.
    #[test]
    fn test_e6_sparse_native_dimension() {
        assert_eq!(
            ModelId::Sparse.dimension(),
            30522,
            "E6 Sparse: expected native dimension 30522 (SPLADE vocab)"
        );
        assert_eq!(SPARSE_NATIVE, 30522, "SPARSE_NATIVE constant mismatch");
    }

    /// Test E7 Code native dimension matches Constitution.
    #[test]
    fn test_e7_code_native_dimension() {
        assert_eq!(
            ModelId::Code.dimension(),
            1536,
            "E7 Code: expected native dimension 1536 (Qodo-Embed-1-1.5B)"
        );
        assert_eq!(CODE_NATIVE, 1536, "CODE_NATIVE constant mismatch");
    }

    /// Test E8 Graph native dimension matches Constitution.
    #[test]
    fn test_e8_graph_native_dimension() {
        assert_eq!(
            ModelId::Graph.dimension(),
            384,
            "E8 Graph: expected native dimension 384"
        );
        assert_eq!(GRAPH_NATIVE, 384, "GRAPH_NATIVE constant mismatch");
    }

    /// Test E9 Hdc native dimension matches Constitution.
    #[test]
    fn test_e9_hdc_native_dimension() {
        assert_eq!(
            ModelId::Hdc.dimension(),
            10000,
            "E9 Hdc: expected native dimension 10000 (10K-bit)"
        );
        assert_eq!(HDC_NATIVE, 10000, "HDC_NATIVE constant mismatch");
    }

    /// Test E10 Multimodal native dimension matches Constitution.
    #[test]
    fn test_e10_multimodal_native_dimension() {
        assert_eq!(
            ModelId::Multimodal.dimension(),
            768,
            "E10 Multimodal: expected native dimension 768 (CLIP)"
        );
        assert_eq!(
            MULTIMODAL_NATIVE, 768,
            "MULTIMODAL_NATIVE constant mismatch"
        );
    }

    /// Test E11 Entity native dimension matches Constitution.
    #[test]
    fn test_e11_entity_native_dimension() {
        assert_eq!(
            ModelId::Entity.dimension(),
            384,
            "E11 Entity: expected native dimension 384"
        );
        assert_eq!(ENTITY_NATIVE, 384, "ENTITY_NATIVE constant mismatch");
    }

    /// Test E12 LateInteraction native dimension matches Constitution.
    #[test]
    fn test_e12_late_interaction_native_dimension() {
        assert_eq!(
            ModelId::LateInteraction.dimension(),
            128,
            "E12 LateInteraction: expected native dimension 128 (per token)"
        );
        assert_eq!(
            LATE_INTERACTION_NATIVE, 128,
            "LATE_INTERACTION_NATIVE constant mismatch"
        );
    }

    /// Test E13 Splade native dimension matches Constitution.
    #[test]
    fn test_e13_splade_native_dimension() {
        assert_eq!(
            ModelId::Splade.dimension(),
            30522,
            "E13 Splade: expected native dimension 30522"
        );
        assert_eq!(SPLADE_NATIVE, 30522, "SPLADE_NATIVE constant mismatch");
    }

    /// Test ALL native dimensions match expected values in one sweep.
    /// FAIL FAST: Any single mismatch panics with details.
    #[test]
    fn test_all_native_dimensions_match_constitution() {
        for (model_id, expected_dim) in &EXPECTED_NATIVE_DIMS {
            let actual_dim = model_id.dimension();
            assert_eq!(
                actual_dim, *expected_dim,
                "Native dimension mismatch for {:?}: expected {}, got {}",
                model_id, expected_dim, actual_dim
            );
        }
        println!("[PASS] All 13 native dimensions match Constitution");
    }

    /// Test NATIVE_DIMENSIONS array matches ModelId::dimension() for all models.
    #[test]
    fn test_native_dimensions_array_consistency() {
        for (i, &expected) in NATIVE_DIMENSIONS.iter().enumerate() {
            let by_index = native_dimension_by_index(i);
            assert_eq!(
                by_index, expected,
                "NATIVE_DIMENSIONS[{}] ({}) != native_dimension_by_index({}) ({})",
                i, expected, i, by_index
            );
        }
        println!("[PASS] NATIVE_DIMENSIONS array consistent with helper function");
    }
}

// =============================================================================
// PROJECTED DIMENSION TESTS
// =============================================================================

#[cfg(test)]
mod projected_dimension_tests {
    use super::*;

    /// Test E1 Semantic projected dimension (no projection needed).
    #[test]
    fn test_e1_semantic_projected_dimension() {
        assert_eq!(
            ModelId::Semantic.projected_dimension(),
            1024,
            "E1 Semantic: expected projected dimension 1024"
        );
        assert_eq!(SEMANTIC, 1024, "SEMANTIC constant mismatch");
    }

    /// Test E2 TemporalRecent projected dimension (no projection needed).
    #[test]
    fn test_e2_temporal_recent_projected_dimension() {
        assert_eq!(
            ModelId::TemporalRecent.projected_dimension(),
            512,
            "E2 TemporalRecent: expected projected dimension 512"
        );
        assert_eq!(TEMPORAL_RECENT, 512, "TEMPORAL_RECENT constant mismatch");
    }

    /// Test E3 TemporalPeriodic projected dimension (no projection needed).
    #[test]
    fn test_e3_temporal_periodic_projected_dimension() {
        assert_eq!(
            ModelId::TemporalPeriodic.projected_dimension(),
            512,
            "E3 TemporalPeriodic: expected projected dimension 512"
        );
        assert_eq!(
            TEMPORAL_PERIODIC, 512,
            "TEMPORAL_PERIODIC constant mismatch"
        );
    }

    /// Test E4 TemporalPositional projected dimension (no projection needed).
    #[test]
    fn test_e4_temporal_positional_projected_dimension() {
        assert_eq!(
            ModelId::TemporalPositional.projected_dimension(),
            512,
            "E4 TemporalPositional: expected projected dimension 512"
        );
        assert_eq!(
            TEMPORAL_POSITIONAL, 512,
            "TEMPORAL_POSITIONAL constant mismatch"
        );
    }

    /// Test E5 Causal projected dimension (no projection needed).
    #[test]
    fn test_e5_causal_projected_dimension() {
        assert_eq!(
            ModelId::Causal.projected_dimension(),
            768,
            "E5 Causal: expected projected dimension 768"
        );
        assert_eq!(CAUSAL, 768, "CAUSAL constant mismatch");
    }

    /// Test E6 Sparse projected dimension (30K -> 1536 projection).
    #[test]
    fn test_e6_sparse_projected_dimension() {
        assert_eq!(
            ModelId::Sparse.projected_dimension(),
            1536,
            "E6 Sparse: expected projected dimension 1536 (from 30522)"
        );
        assert_eq!(SPARSE, 1536, "SPARSE constant mismatch");
        // Verify compression ratio
        let ratio = SPARSE_NATIVE as f64 / SPARSE as f64;
        assert!(
            ratio > 19.0 && ratio < 20.0,
            "E6 Sparse projection ratio ~19.8x expected, got {}",
            ratio
        );
    }

    /// Test E7 Code projected dimension (1536D native, no projection needed).
    #[test]
    fn test_e7_code_projected_dimension() {
        assert_eq!(
            ModelId::Code.projected_dimension(),
            1536,
            "E7 Code: expected projected dimension 1536 (Qodo-Embed native)"
        );
        assert_eq!(CODE, 1536, "CODE constant mismatch");
        // Verify no expansion needed (1:1 ratio)
        assert_eq!(
            CODE, CODE_NATIVE,
            "E7 Code should have no projection (native 1536D)"
        );
    }

    /// Test E8 Graph projected dimension (no projection needed).
    #[test]
    fn test_e8_graph_projected_dimension() {
        assert_eq!(
            ModelId::Graph.projected_dimension(),
            384,
            "E8 Graph: expected projected dimension 384"
        );
        assert_eq!(GRAPH, 384, "GRAPH constant mismatch");
    }

    /// Test E9 Hdc projected dimension (10K -> 1024 projection).
    #[test]
    fn test_e9_hdc_projected_dimension() {
        assert_eq!(
            ModelId::Hdc.projected_dimension(),
            1024,
            "E9 Hdc: expected projected dimension 1024 (from 10000)"
        );
        assert_eq!(HDC, 1024, "HDC constant mismatch");
        // Verify compression ratio
        let ratio = HDC_NATIVE as f64 / HDC as f64;
        assert!(
            ratio > 9.0 && ratio < 10.0,
            "E9 Hdc projection ratio ~9.77x expected, got {}",
            ratio
        );
    }

    /// Test E10 Multimodal projected dimension (no projection needed).
    #[test]
    fn test_e10_multimodal_projected_dimension() {
        assert_eq!(
            ModelId::Multimodal.projected_dimension(),
            768,
            "E10 Multimodal: expected projected dimension 768"
        );
        assert_eq!(MULTIMODAL, 768, "MULTIMODAL constant mismatch");
    }

    /// Test E11 Entity projected dimension (no projection needed).
    #[test]
    fn test_e11_entity_projected_dimension() {
        assert_eq!(
            ModelId::Entity.projected_dimension(),
            384,
            "E11 Entity: expected projected dimension 384"
        );
        assert_eq!(ENTITY, 384, "ENTITY constant mismatch");
    }

    /// Test E12 LateInteraction projected dimension (pooled to single vector).
    #[test]
    fn test_e12_late_interaction_projected_dimension() {
        assert_eq!(
            ModelId::LateInteraction.projected_dimension(),
            128,
            "E12 LateInteraction: expected projected dimension 128"
        );
        assert_eq!(LATE_INTERACTION, 128, "LATE_INTERACTION constant mismatch");
    }

    /// Test E13 Splade projected dimension (30K -> 1536 projection).
    #[test]
    fn test_e13_splade_projected_dimension() {
        assert_eq!(
            ModelId::Splade.projected_dimension(),
            1536,
            "E13 Splade: expected projected dimension 1536"
        );
        assert_eq!(SPLADE, 1536, "SPLADE projected constant mismatch");
        // Verify compression ratio
        let ratio = SPLADE_NATIVE as f64 / SPLADE as f64;
        assert!(
            ratio > 19.0 && ratio < 20.0,
            "E13 Splade projection ratio ~19.8x expected, got {}",
            ratio
        );
    }

    /// Test ALL projected dimensions match expected values in one sweep.
    /// FAIL FAST: Any single mismatch panics with details.
    #[test]
    fn test_all_projected_dimensions_match_constitution() {
        for (model_id, expected_dim) in &EXPECTED_PROJECTED_DIMS {
            let actual_dim = model_id.projected_dimension();
            assert_eq!(
                actual_dim, *expected_dim,
                "Projected dimension mismatch for {:?}: expected {}, got {}",
                model_id, expected_dim, actual_dim
            );
        }
        println!("[PASS] All 13 projected dimensions match Constitution");
    }

    /// Test PROJECTED_DIMENSIONS array consistency.
    #[test]
    fn test_projected_dimensions_array_consistency() {
        for (i, &expected) in PROJECTED_DIMENSIONS.iter().enumerate() {
            let by_index = projected_dimension_by_index(i);
            assert_eq!(
                by_index, expected,
                "PROJECTED_DIMENSIONS[{}] ({}) != projected_dimension_by_index({}) ({})",
                i, expected, i, by_index
            );
        }
        println!("[PASS] PROJECTED_DIMENSIONS array consistent with helper function");
    }

    /// Test PROJECTED_DIMENSIONS array sum equals TOTAL_DIMENSION.
    #[test]
    fn test_projected_dimensions_array_sum() {
        let sum: usize = PROJECTED_DIMENSIONS.iter().sum();
        assert_eq!(
            sum, TOTAL_DIMENSION,
            "Sum of PROJECTED_DIMENSIONS ({}) != TOTAL_DIMENSION ({})",
            sum, TOTAL_DIMENSION
        );
        println!("[PASS] PROJECTED_DIMENSIONS sum equals TOTAL_DIMENSION");
    }
}

// =============================================================================
// QUANTIZATION METHOD TESTS
// =============================================================================

#[cfg(test)]
mod quantization_method_tests {
    use super::*;

    /// Test PQ8 models: E1, E5, E7, E10.
    #[test]
    fn test_pq8_models() {
        let pq8_models = [
            (ModelId::Semantic, "E1"),
            (ModelId::Causal, "E5"),
            (ModelId::Code, "E7"),
            (ModelId::Multimodal, "E10"),
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
        println!("[PASS] All PQ8 models (E1, E5, E7, E10) verified");
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
        println!("[PASS] All 13 quantization methods match Constitution");
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
}

// =============================================================================
// AGGREGATE DIMENSION TESTS
// =============================================================================

#[cfg(test)]
mod aggregate_dimension_tests {
    use super::*;

    /// Test TOTAL_DIMENSION constant equals expected value.
    #[test]
    fn test_total_dimension_constant() {
        assert_eq!(
            TOTAL_DIMENSION, EXPECTED_TOTAL_DIMENSION,
            "TOTAL_DIMENSION should be {} but is {}",
            EXPECTED_TOTAL_DIMENSION, TOTAL_DIMENSION
        );
        println!("[PASS] TOTAL_DIMENSION = {} verified", TOTAL_DIMENSION);
    }

    /// Test MODEL_COUNT constant equals expected value.
    #[test]
    fn test_model_count_constant() {
        assert_eq!(
            MODEL_COUNT, EXPECTED_MODEL_COUNT,
            "MODEL_COUNT should be {} but is {}",
            EXPECTED_MODEL_COUNT, MODEL_COUNT
        );
        println!("[PASS] MODEL_COUNT = {} verified", MODEL_COUNT);
    }

    /// Test ModelId::all() returns exactly 13 models.
    #[test]
    fn test_model_id_all_count() {
        let all_models = ModelId::all();
        assert_eq!(
            all_models.len(),
            EXPECTED_MODEL_COUNT,
            "ModelId::all() should return {} models, got {}",
            EXPECTED_MODEL_COUNT,
            all_models.len()
        );
        println!("[PASS] ModelId::all() returns 13 models");
    }

    /// Test manual sum of projected dimensions equals TOTAL_DIMENSION.
    #[test]
    fn test_manual_sum_equals_total() {
        let manual_sum = SEMANTIC
            + TEMPORAL_RECENT
            + TEMPORAL_PERIODIC
            + TEMPORAL_POSITIONAL
            + CAUSAL
            + SPARSE
            + CODE
            + GRAPH
            + HDC
            + MULTIMODAL
            + ENTITY
            + LATE_INTERACTION
            + SPLADE;

        assert_eq!(
            manual_sum, TOTAL_DIMENSION,
            "Manual sum ({}) != TOTAL_DIMENSION ({})",
            manual_sum, TOTAL_DIMENSION
        );
        println!("[PASS] Manual sum of all projected dimensions = TOTAL_DIMENSION");
    }

    /// Test sum from ModelId iteration equals TOTAL_DIMENSION.
    #[test]
    fn test_model_id_iteration_sum() {
        let sum: usize = ModelId::all().iter().map(|m| m.projected_dimension()).sum();

        assert_eq!(
            sum, TOTAL_DIMENSION,
            "Sum from ModelId::all().projected_dimension() ({}) != TOTAL_DIMENSION ({})",
            sum, TOTAL_DIMENSION
        );
        println!("[PASS] Sum from ModelId iteration = TOTAL_DIMENSION");
    }

    /// Test TOTAL_DIMENSION breakdown matches documented calculation.
    #[test]
    fn test_total_dimension_breakdown() {
        // Constitution: 1024 + 512 + 512 + 512 + 768 + 1536 + 768 + 384 + 1024 + 768 + 384 + 128 + 1536 = 9856
        let expected_breakdown =
            1024 + 512 + 512 + 512 + 768 + 1536 + 768 + 384 + 1024 + 768 + 384 + 128 + 1536;
        assert_eq!(
            expected_breakdown, EXPECTED_TOTAL_DIMENSION,
            "Documented breakdown sum {} != expected {}",
            expected_breakdown, EXPECTED_TOTAL_DIMENSION
        );
        assert_eq!(
            TOTAL_DIMENSION, expected_breakdown,
            "TOTAL_DIMENSION {} != documented breakdown {}",
            TOTAL_DIMENSION, expected_breakdown
        );
        println!("[PASS] TOTAL_DIMENSION breakdown calculation verified");
    }
}

// =============================================================================
// OFFSET TESTS
// =============================================================================

#[cfg(test)]
mod offset_tests {
    use super::*;

    /// Test first offset is zero.
    #[test]
    fn test_first_offset_is_zero() {
        assert_eq!(offset_by_index(0), 0, "First offset must be 0");
        assert_eq!(OFFSETS[0], 0, "OFFSETS[0] must be 0");
        println!("[PASS] First offset is 0");
    }

    /// Test last offset + dimension equals TOTAL_DIMENSION.
    #[test]
    fn test_last_offset_plus_dimension_equals_total() {
        let last_index = MODEL_COUNT - 1; // 12
        let last_offset = offset_by_index(last_index);
        let last_dim = projected_dimension_by_index(last_index);
        let computed_total = last_offset + last_dim;

        assert_eq!(
            computed_total, TOTAL_DIMENSION,
            "offset[12] + dim[12] ({} + {}) = {} != TOTAL_DIMENSION ({})",
            last_offset, last_dim, computed_total, TOTAL_DIMENSION
        );
        println!("[PASS] Last offset + last dimension = TOTAL_DIMENSION");
    }

    /// Test each offset equals sum of previous dimensions.
    #[test]
    fn test_offset_cumulative_sum() {
        let mut cumulative: usize = 0;

        for i in 0..MODEL_COUNT {
            let offset = offset_by_index(i);
            assert_eq!(
                offset, cumulative,
                "offset_by_index({}) = {} but cumulative sum = {}",
                i, offset, cumulative
            );
            cumulative += projected_dimension_by_index(i);
        }

        // After all, cumulative should equal TOTAL_DIMENSION
        assert_eq!(
            cumulative, TOTAL_DIMENSION,
            "Final cumulative {} != TOTAL_DIMENSION {}",
            cumulative, TOTAL_DIMENSION
        );
        println!("[PASS] All offsets are correct cumulative sums");
    }

    /// Test OFFSETS array matches offset_by_index function.
    #[test]
    fn test_offsets_array_consistency() {
        for (i, &offset) in OFFSETS.iter().enumerate() {
            assert_eq!(
                offset,
                offset_by_index(i),
                "OFFSETS[{}] ({}) != offset_by_index({}) ({})",
                i,
                offset,
                i,
                offset_by_index(i)
            );
        }
        println!("[PASS] OFFSETS array matches offset_by_index function");
    }

    /// Test specific key offsets from Constitution.
    #[test]
    fn test_key_offsets() {
        // E1 Semantic: 0
        assert_eq!(offset_by_index(0), 0, "E1 offset should be 0");

        // E2 TemporalRecent: 1024
        assert_eq!(offset_by_index(1), 1024, "E2 offset should be 1024");

        // E5 Causal: 1024 + 512*3 = 2560
        assert_eq!(offset_by_index(4), 2560, "E5 offset should be 2560");

        // E6 Sparse: 2560 + 768 = 3328
        assert_eq!(offset_by_index(5), 3328, "E6 offset should be 3328");

        // E13 Splade: 9856 - 1536 = 8320
        assert_eq!(offset_by_index(12), 8320, "E13 offset should be 8320");

        println!("[PASS] Key offsets verified");
    }
}

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    /// Test invalid index for projected_dimension_by_index panics.
    #[test]
    #[should_panic(expected = "Invalid model index")]
    fn test_projected_dimension_invalid_index_panics() {
        let _ = projected_dimension_by_index(13);
    }

    /// Test invalid index for native_dimension_by_index panics.
    #[test]
    #[should_panic(expected = "Invalid model index")]
    fn test_native_dimension_invalid_index_panics() {
        let _ = native_dimension_by_index(13);
    }

    /// Test invalid index for offset_by_index panics.
    #[test]
    #[should_panic(expected = "Invalid model index")]
    fn test_offset_invalid_index_panics() {
        let _ = offset_by_index(13);
    }

    /// Test large invalid index for projected_dimension_by_index panics.
    #[test]
    #[should_panic(expected = "Invalid model index")]
    fn test_projected_dimension_large_index_panics() {
        let _ = projected_dimension_by_index(255);
    }

    /// Test no model has zero dimension.
    #[test]
    fn test_no_zero_dimensions() {
        for model_id in ModelId::all() {
            assert!(
                model_id.dimension() > 0,
                "{:?} has zero native dimension",
                model_id
            );
            assert!(
                model_id.projected_dimension() > 0,
                "{:?} has zero projected dimension",
                model_id
            );
        }
        println!("[PASS] No model has zero dimension");
    }

    /// Test projected >= native for models requiring expansion (E7 Code only).
    #[test]
    fn test_projection_directions() {
        for model_id in ModelId::all() {
            let native = model_id.dimension();
            let projected = model_id.projected_dimension();

            match model_id {
                // E7 Code: no projection needed (native 1536D from Qodo-Embed)
                ModelId::Code => {
                    assert_eq!(
                        projected, native,
                        "E7 Code should have no projection: {} == {} expected",
                        projected, native
                    );
                }
                // E6, E9, E13: compression
                ModelId::Sparse | ModelId::Hdc | ModelId::Splade => {
                    assert!(
                        projected < native,
                        "{:?} should compress: {} < {} expected",
                        model_id,
                        projected,
                        native
                    );
                }
                // All others: no projection
                _ => {
                    assert_eq!(
                        projected, native,
                        "{:?} should have no projection: {} == {} expected",
                        model_id, projected, native
                    );
                }
            }
        }
        println!("[PASS] Projection directions verified for all models");
    }

    /// Test boundary values at dimension limits.
    #[test]
    fn test_dimension_boundary_values() {
        // Minimum dimension is 128 (E12 LateInteraction)
        let min_dim = ModelId::all()
            .iter()
            .map(|m| m.projected_dimension())
            .min()
            .unwrap();
        assert_eq!(min_dim, 128, "Minimum projected dimension should be 128");

        // Maximum native dimension is 30522 (E6 Sparse, E13 Splade)
        let max_native = ModelId::all().iter().map(|m| m.dimension()).max().unwrap();
        assert_eq!(
            max_native, 30522,
            "Maximum native dimension should be 30522"
        );

        // Maximum projected dimension is 1536 (E6 Sparse, E13 Splade)
        let max_projected = ModelId::all()
            .iter()
            .map(|m| m.projected_dimension())
            .max()
            .unwrap();
        assert_eq!(
            max_projected, 1536,
            "Maximum projected dimension should be 1536"
        );

        println!("[PASS] Dimension boundary values verified");
    }

    /// Test arrays have correct length.
    #[test]
    fn test_array_lengths() {
        assert_eq!(
            NATIVE_DIMENSIONS.len(),
            MODEL_COUNT,
            "NATIVE_DIMENSIONS length mismatch"
        );
        assert_eq!(
            PROJECTED_DIMENSIONS.len(),
            MODEL_COUNT,
            "PROJECTED_DIMENSIONS length mismatch"
        );
        assert_eq!(OFFSETS.len(), MODEL_COUNT, "OFFSETS length mismatch");
        assert_eq!(
            ModelId::all().len(),
            MODEL_COUNT,
            "ModelId::all() length mismatch"
        );
        println!("[PASS] All arrays have length {}", MODEL_COUNT);
    }
}

// =============================================================================
// MODEL METADATA TESTS
// =============================================================================

#[cfg(test)]
mod model_metadata_tests {
    use super::*;

    /// Test ModelId repr(u8) values match index order.
    #[test]
    fn test_model_id_repr_order() {
        assert_eq!(ModelId::Semantic as u8, 0, "Semantic should be 0");
        assert_eq!(
            ModelId::TemporalRecent as u8,
            1,
            "TemporalRecent should be 1"
        );
        assert_eq!(
            ModelId::TemporalPeriodic as u8,
            2,
            "TemporalPeriodic should be 2"
        );
        assert_eq!(
            ModelId::TemporalPositional as u8,
            3,
            "TemporalPositional should be 3"
        );
        assert_eq!(ModelId::Causal as u8, 4, "Causal should be 4");
        assert_eq!(ModelId::Sparse as u8, 5, "Sparse should be 5");
        assert_eq!(ModelId::Code as u8, 6, "Code should be 6");
        assert_eq!(ModelId::Graph as u8, 7, "Graph should be 7");
        assert_eq!(ModelId::Hdc as u8, 8, "Hdc should be 8");
        assert_eq!(ModelId::Multimodal as u8, 9, "Multimodal should be 9");
        assert_eq!(ModelId::Entity as u8, 10, "Entity should be 10");
        assert_eq!(
            ModelId::LateInteraction as u8,
            11,
            "LateInteraction should be 11"
        );
        assert_eq!(ModelId::Splade as u8, 12, "Splade should be 12");
        println!("[PASS] ModelId repr(u8) values match E1-E13 order");
    }

    /// Test is_custom() classification.
    #[test]
    fn test_is_custom_classification() {
        let custom_models = [
            ModelId::TemporalRecent,
            ModelId::TemporalPeriodic,
            ModelId::TemporalPositional,
            ModelId::Hdc,
        ];

        for model_id in ModelId::all() {
            let expected_custom = custom_models.contains(model_id);
            assert_eq!(
                model_id.is_custom(),
                expected_custom,
                "{:?}.is_custom() should be {}",
                model_id,
                expected_custom
            );
            assert_eq!(
                model_id.is_pretrained(),
                !expected_custom,
                "{:?}.is_pretrained() should be {}",
                model_id,
                !expected_custom
            );
        }
        println!("[PASS] is_custom() and is_pretrained() classifications verified");
    }

    /// Test custom models count.
    #[test]
    fn test_custom_models_count() {
        let custom_count: usize = ModelId::custom().count();
        assert_eq!(
            custom_count, 4,
            "Expected 4 custom models, got {}",
            custom_count
        );

        let pretrained_count: usize = ModelId::pretrained().count();
        assert_eq!(
            pretrained_count, 9,
            "Expected 9 pretrained models, got {}",
            pretrained_count
        );

        assert_eq!(
            custom_count + pretrained_count,
            MODEL_COUNT,
            "Custom + pretrained should equal MODEL_COUNT"
        );
        println!("[PASS] 4 custom + 9 pretrained = 13 total models");
    }
}

// =============================================================================
// COMPREHENSIVE VALIDATION TEST
// =============================================================================

#[cfg(test)]
mod comprehensive_validation {
    use super::*;

    /// Master validation test - runs all critical checks in one place.
    /// FAIL FAST on any Constitution violation.
    #[test]
    fn test_comprehensive_dimension_validation() {
        println!("=== COMPREHENSIVE DIMENSION VALIDATION ===\n");

        // 1. Verify MODEL_COUNT
        assert_eq!(MODEL_COUNT, 13, "MODEL_COUNT must be 13");
        println!("[1/6] MODEL_COUNT = 13");

        // 2. Verify TOTAL_DIMENSION
        assert_eq!(TOTAL_DIMENSION, 9856, "TOTAL_DIMENSION must be 9856");
        println!("[2/6] TOTAL_DIMENSION = 9856");

        // 3. Verify all native dimensions
        for (model_id, expected) in &EXPECTED_NATIVE_DIMS {
            let actual = model_id.dimension();
            assert_eq!(
                actual, *expected,
                "{:?} native dimension mismatch",
                model_id
            );
        }
        println!("[3/6] All 13 native dimensions verified");

        // 4. Verify all projected dimensions
        for (model_id, expected) in &EXPECTED_PROJECTED_DIMS {
            let actual = model_id.projected_dimension();
            assert_eq!(
                actual, *expected,
                "{:?} projected dimension mismatch",
                model_id
            );
        }
        println!("[4/6] All 13 projected dimensions verified");

        // 5. Verify all quantization methods
        for (model_id, expected) in &EXPECTED_QUANTIZATION {
            let actual = QuantizationMethod::for_model_id(*model_id);
            assert_eq!(
                actual, *expected,
                "{:?} quantization method mismatch",
                model_id
            );
        }
        println!("[5/6] All 13 quantization methods verified");

        // 6. Verify sum consistency
        let sum: usize = ModelId::all().iter().map(|m| m.projected_dimension()).sum();
        assert_eq!(sum, TOTAL_DIMENSION, "Sum mismatch with TOTAL_DIMENSION");
        println!("[6/6] Sum of projected dimensions = TOTAL_DIMENSION");

        println!("\n=== ALL VALIDATIONS PASSED ===");
        println!("  - 13 ModelId variants verified");
        println!("  - 13 native dimensions verified");
        println!("  - 13 projected dimensions verified");
        println!("  - 13 quantization methods verified");
        println!("  - TOTAL_DIMENSION = 9856 verified");
    }
}
