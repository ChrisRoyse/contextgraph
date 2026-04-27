//! Unit tests for matrix strategy search - SearchMatrix and correlation tests.
//!
//! # Design Philosophy
//!
//! **FAIL FAST. NO FALLBACKS.**

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use uuid::Uuid;

    use context_graph_core::types::fingerprint::NUM_EMBEDDERS;

    use crate::teleological::indexes::{EmbedderIndex, EmbedderIndexRegistry};
    use crate::teleological::search::matrix::strategy_search::pearson_correlation_matched;
    use crate::teleological::search::matrix::{MatrixStrategySearch, SearchMatrix};

    // ========== SEARCH MATRIX TESTS ==========

    #[test]
    fn test_zeros_matrix() {
        println!("=== TEST: SearchMatrix::zeros creates all-zero matrix ===");
        println!("BEFORE: Creating zeros matrix");

        let m = SearchMatrix::zeros();

        println!("AFTER: matrix created");
        for i in 0..NUM_EMBEDDERS {
            for j in 0..NUM_EMBEDDERS {
                assert_eq!(m.get(i, j), 0.0);
            }
        }
        assert!(m.is_diagonal()); // All zeros is considered diagonal
        assert!(!m.has_cross_correlations());
        assert_eq!(m.sparsity(), 1.0);
        assert!(m.active_embedders().is_empty());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_identity_matrix() {
        println!("=== TEST: SearchMatrix::identity has 1.0 on diagonal ===");

        let m = SearchMatrix::identity();

        for i in 0..NUM_EMBEDDERS {
            for j in 0..NUM_EMBEDDERS {
                if i == j {
                    assert_eq!(m.get(i, j), 1.0);
                } else {
                    assert_eq!(m.get(i, j), 0.0);
                }
            }
        }
        assert!(m.is_diagonal());
        assert!(!m.has_cross_correlations());
        assert_eq!(m.active_embedders().len(), NUM_EMBEDDERS);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_uniform_matrix() {
        println!("=== TEST: SearchMatrix::uniform has 1/14 everywhere ===");

        let m = SearchMatrix::uniform();
        let expected = 1.0 / NUM_EMBEDDERS as f32;

        for i in 0..NUM_EMBEDDERS {
            for j in 0..NUM_EMBEDDERS {
                assert!((m.get(i, j) - expected).abs() < 1e-6);
            }
        }
        assert!(!m.is_diagonal());
        assert!(m.has_cross_correlations());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_predefined_semantic_focused() {
        println!("=== TEST: SearchMatrix::semantic_focused structure ===");

        let m = SearchMatrix::semantic_focused();
        assert_eq!(m.get(0, 0), 1.0, "E1Semantic should have weight 1.0");
        assert_eq!(m.get(4, 4), 0.3, "E5Causal should have weight 0.3");
        assert_eq!(m.get(0, 4), 0.2, "E1-E5 cross should be 0.2");
        assert_eq!(m.get(4, 0), 0.2, "E5-E1 cross should be 0.2");
        assert!(m.has_cross_correlations());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_predefined_code_heavy() {
        println!("=== TEST: SearchMatrix::code_heavy structure ===");

        let m = SearchMatrix::code_heavy();
        assert_eq!(m.get(6, 6), 1.0, "E7Code should have weight 1.0");
        assert_eq!(m.get(0, 0), 0.3, "E1Semantic should have weight 0.3");
        assert_eq!(m.get(0, 6), 0.2, "E1-E7 cross should be 0.2");
        assert_eq!(m.get(6, 0), 0.2, "E7-E1 cross should be 0.2");
        assert!(m.has_cross_correlations());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_predefined_temporal_aware() {
        println!("=== TEST: SearchMatrix::temporal_aware structure ===");

        let m = SearchMatrix::temporal_aware();
        assert_eq!(m.get(0, 0), 0.5, "E1Semantic should have weight 0.5");
        assert_eq!(m.get(1, 1), 0.8, "E2TemporalRecent should have weight 0.8");
        assert_eq!(
            m.get(2, 2),
            0.8,
            "E3TemporalPeriodic should have weight 0.8"
        );
        assert_eq!(
            m.get(3, 3),
            0.8,
            "E4TemporalPositional should have weight 0.8"
        );
        assert_eq!(m.get(1, 2), 0.1, "E2-E3 cross should be 0.1");
        assert_eq!(m.get(2, 1), 0.1, "E3-E2 cross should be 0.1");
        assert!(m.has_cross_correlations());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_predefined_balanced() {
        println!("=== TEST: SearchMatrix::balanced structure ===");

        let m = SearchMatrix::balanced();
        assert_eq!(m.get(0, 0), 0.1); // E1
        assert_eq!(m.get(1, 1), 0.1); // E2
        assert_eq!(m.get(6, 6), 0.1); // E7
        assert_eq!(m.get(5, 5), 0.0, "E6Sparse should have weight 0.0");
        assert_eq!(
            m.get(11, 11),
            0.0,
            "E12LateInteraction should have weight 0.0"
        );
        assert_eq!(m.get(12, 12), 0.0, "E13Splade should have weight 0.0");
        assert!(m.is_diagonal(), "Balanced should be diagonal-only");

        println!("RESULT: PASS");
    }

    #[test]
    fn test_predefined_entity_focused() {
        println!("=== TEST: SearchMatrix::entity_focused structure ===");

        let m = SearchMatrix::entity_focused();
        assert_eq!(m.get(10, 10), 1.0, "E11Entity should have weight 1.0");
        assert_eq!(m.get(0, 0), 0.4, "E1Semantic should have weight 0.4");
        assert_eq!(m.get(7, 7), 0.3, "E8Graph should have weight 0.3");
        assert!(m.is_diagonal());

        println!("RESULT: PASS");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_matrix_get_out_of_bounds_panics() {
        println!("=== TEST: SearchMatrix::get out of bounds panics ===");
        let m = SearchMatrix::zeros();
        m.get(NUM_EMBEDDERS, 0); // Should panic
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_matrix_set_out_of_bounds_panics() {
        println!("=== TEST: SearchMatrix::set out of bounds panics ===");
        let mut m = SearchMatrix::zeros();
        m.set(0, NUM_EMBEDDERS, 1.0); // Should panic
    }

    #[test]
    fn test_matrix_sparsity() {
        println!("=== TEST: SearchMatrix::sparsity calculation ===");

        let zeros = SearchMatrix::zeros();
        assert_eq!(zeros.sparsity(), 1.0);

        let identity = SearchMatrix::identity();
        let total_cells = (NUM_EMBEDDERS * NUM_EMBEDDERS) as f32;
        let expected = (total_cells - NUM_EMBEDDERS as f32) / total_cells;
        assert!((identity.sparsity() - expected).abs() < 1e-4);

        let uniform = SearchMatrix::uniform();
        assert_eq!(uniform.sparsity(), 0.0);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_matrix_active_embedders() {
        println!("=== TEST: SearchMatrix::active_embedders ===");

        let zeros = SearchMatrix::zeros();
        assert!(zeros.active_embedders().is_empty());

        let identity = SearchMatrix::identity();
        assert_eq!(identity.active_embedders().len(), NUM_EMBEDDERS);

        let mut custom = SearchMatrix::zeros();
        custom.set(0, 0, 1.0);
        custom.set(6, 6, 0.5);
        let active = custom.active_embedders();
        assert_eq!(active.len(), 2);
        assert!(active.contains(&0));
        assert!(active.contains(&6));

        println!("RESULT: PASS");
    }

    #[test]
    fn test_matrix_diagonal_for_embedder() {
        println!("=== TEST: SearchMatrix::diagonal for EmbedderIndex ===");

        let m = SearchMatrix::code_heavy();
        assert_eq!(m.diagonal(EmbedderIndex::E7Code), 1.0);
        assert_eq!(m.diagonal(EmbedderIndex::E1Semantic), 0.3);
        assert_eq!(m.diagonal(EmbedderIndex::E8Graph), 0.0);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_matrix_default_is_balanced() {
        println!("=== TEST: SearchMatrix::default is balanced ===");

        let default = SearchMatrix::default();
        let balanced = SearchMatrix::balanced();
        assert_eq!(default, balanced);

        println!("RESULT: PASS");
    }

    // ========== MATRIX ANALYSIS TESTS ==========

    #[test]
    fn test_matrix_analysis() {
        println!("=== TEST: MatrixAnalysis structure ===");

        let registry = Arc::new(EmbedderIndexRegistry::new());
        let search = MatrixStrategySearch::new(registry);

        let matrix = SearchMatrix::code_heavy();
        let analysis = search.analyze_matrix(&matrix);

        assert!(!analysis.is_diagonal, "code_heavy has cross-correlations");
        assert!(analysis.has_cross_correlations);
        assert!(analysis.sparsity > 0.9, "code_heavy is mostly sparse");
        assert!(analysis.active_embedders.contains(&0)); // E1
        assert!(analysis.active_embedders.contains(&6)); // E7
        assert_eq!(analysis.cross_correlation_count, 2); // E1-E7 and E7-E1

        println!("RESULT: PASS");
    }

    // ========== PEARSON CORRELATION TESTS ==========

    #[test]
    fn test_pearson_correlation_perfect() {
        println!("=== TEST: Pearson correlation perfect positive ===");

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let scores_a = vec![(id1, 0.1), (id2, 0.5), (id3, 0.9)];
        let scores_b = vec![(id1, 0.2), (id2, 0.6), (id3, 1.0)];

        let r = pearson_correlation_matched(&scores_a, &scores_b);
        println!("Pearson r = {:.4}", r);
        assert!(r > 0.99, "Perfect positive correlation expected");

        println!("RESULT: PASS");
    }

    #[test]
    fn test_pearson_correlation_negative() {
        println!("=== TEST: Pearson correlation negative ===");

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        let scores_a = vec![(id1, 0.1), (id2, 0.5), (id3, 0.9)];
        let scores_b = vec![(id1, 0.9), (id2, 0.5), (id3, 0.1)];

        let r = pearson_correlation_matched(&scores_a, &scores_b);
        println!("Pearson r = {:.4}", r);
        assert!(r < -0.99, "Perfect negative correlation expected");

        println!("RESULT: PASS");
    }

    #[test]
    fn test_pearson_correlation_no_common_ids() {
        println!("=== TEST: Pearson correlation no common IDs ===");

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();
        let id4 = Uuid::new_v4();

        let scores_a = vec![(id1, 0.5), (id2, 0.6)];
        let scores_b = vec![(id3, 0.5), (id4, 0.6)];

        let r = pearson_correlation_matched(&scores_a, &scores_b);
        assert_eq!(r, 0.0, "No common IDs should return 0");

        println!("RESULT: PASS");
    }

    // ========== VERIFICATION LOG ==========
}
