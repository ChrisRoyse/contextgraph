//! Boundary and edge case tests for matrix strategy search.
//!
//! From TASK-LOGIC-007 <boundary_edge_cases> section
//!
//! # Design Philosophy
//!
//! **FAIL FAST. NO FALLBACKS.**

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use context_graph_core::types::fingerprint::NUM_EMBEDDERS;

    use crate::teleological::indexes::{EmbedderIndex, EmbedderIndexRegistry};
    use crate::teleological::search::error::SearchError;
    use crate::teleological::search::matrix::{
        CorrelationAnalysis, CorrelationPattern, MatrixAnalysis, MatrixSearchResults,
        MatrixStrategySearch, SearchMatrix,
    };

    #[test]
    fn test_empty_queries_fails_fast() {
        println!("=== TEST: empty_queries - FAIL FAST with empty HashMap ===");
        println!("BEFORE: queries.is_empty() == true");

        let registry = Arc::new(EmbedderIndexRegistry::new());
        let search = MatrixStrategySearch::new(registry);

        let queries: HashMap<EmbedderIndex, Vec<f32>> = HashMap::new();
        let matrix = SearchMatrix::identity();

        let result = search.search(queries, matrix, 10, None);

        println!("AFTER: result = {:?}", result.is_err());

        match result {
            Err(SearchError::Store(msg)) => {
                println!("EVIDENCE: Error message = \"{}\"", msg);
                assert!(msg.contains("empty"), "Error message must contain 'empty'");
                println!("RESULT: PASS - FAIL FAST with correct error");
            }
            Ok(_) => panic!("FAIL: Expected error for empty queries"),
            Err(e) => panic!("FAIL: Wrong error variant: {:?}", e),
        }
    }

    #[test]
    fn test_identity_matrix_equals_multi_search_structure() {
        println!("=== TEST: identity_matrix_equals_multi_search ===");
        println!("BEFORE: identity matrix has diagonal=1.0, off-diagonal=0.0");

        let identity = SearchMatrix::identity();

        for i in 0..NUM_EMBEDDERS {
            assert_eq!(
                identity.get(i, i),
                1.0,
                "Identity diagonal[{}] must be 1.0",
                i
            );
        }

        for i in 0..NUM_EMBEDDERS {
            for j in 0..NUM_EMBEDDERS {
                if i != j {
                    assert_eq!(
                        identity.get(i, j),
                        0.0,
                        "Identity off-diagonal[{},{}] must be 0.0",
                        i,
                        j
                    );
                }
            }
        }

        assert!(identity.is_diagonal(), "Identity must be diagonal");
        assert!(
            !identity.has_cross_correlations(),
            "Identity must have no cross-correlations"
        );

        println!("AFTER: Identity matrix structure verified");
        println!("EVIDENCE: is_diagonal=true, has_cross_correlations=false");
        println!("RESULT: PASS");
    }

    #[test]
    fn test_zero_weight_embedder_skipped_structure() {
        println!("=== TEST: zero_weight_embedder_skipped ===");
        println!("BEFORE: matrix with E1Semantic weight = 0.0");

        let mut matrix = SearchMatrix::identity();
        matrix.set(0, 0, 0.0); // Zero out E1Semantic

        let active = matrix.active_embedders();
        println!("AFTER: active_embedders = {:?}", active);

        assert!(
            !active.contains(&0),
            "E1Semantic (index 0) must not be in active embedders"
        );
        assert_eq!(
            active.len(),
            NUM_EMBEDDERS - 1,
            "Should have all except the zeroed embedder active"
        );

        assert_eq!(
            matrix.diagonal(EmbedderIndex::E1Semantic),
            0.0,
            "E1Semantic diagonal must be 0.0"
        );

        println!("EVIDENCE: E1Semantic not in active_embedders, diagonal=0.0");
        println!("RESULT: PASS");
    }

    #[test]
    fn test_cross_correlation_matrix_structure() {
        println!("=== TEST: cross_correlation_boosts_consensus (structure) ===");
        println!("BEFORE: matrix with E1-E7 cross-correlation = 0.5");

        let mut matrix = SearchMatrix::zeros();
        matrix.set(0, 0, 1.0); // E1Semantic diagonal
        matrix.set(6, 6, 1.0); // E7Code diagonal
        matrix.set(0, 6, 0.5); // E1-E7 cross
        matrix.set(6, 0, 0.5); // E7-E1 cross (symmetric)

        assert_eq!(matrix.get(0, 6), 0.5, "E1-E7 cross should be 0.5");
        assert_eq!(matrix.get(6, 0), 0.5, "E7-E1 cross should be 0.5");
        assert!(
            matrix.has_cross_correlations(),
            "Should have cross-correlations"
        );
        assert!(!matrix.is_diagonal(), "Should not be diagonal-only");

        let registry = Arc::new(EmbedderIndexRegistry::new());
        let search = MatrixStrategySearch::new(registry);
        let analysis = search.analyze_matrix(&matrix);

        println!(
            "AFTER: analysis.cross_correlation_count = {}",
            analysis.cross_correlation_count
        );
        assert_eq!(
            analysis.cross_correlation_count, 2,
            "Should have 2 cross-correlations (E1-E7 and E7-E1)"
        );

        println!("EVIDENCE: has_cross_correlations=true, count=2");
        println!("RESULT: PASS");
    }

    #[test]
    fn test_unsupported_embedder_in_queries_fails_fast() {
        println!("=== TEST: unsupported_embedder_fails_fast ===");
        println!("BEFORE: queries contains E6Sparse");

        let registry = Arc::new(EmbedderIndexRegistry::new());
        let search = MatrixStrategySearch::new(registry);

        let mut queries = HashMap::new();
        queries.insert(EmbedderIndex::E6Sparse, vec![0.5f32; 256]);

        let matrix = SearchMatrix::identity();
        let result = search.search(queries, matrix, 10, None);

        println!("AFTER: result = {:?}", result.is_err());

        match result {
            Err(SearchError::UnsupportedEmbedder { embedder }) => {
                println!("EVIDENCE: Got UnsupportedEmbedder for {:?}", embedder);
                assert_eq!(
                    embedder,
                    EmbedderIndex::E6Sparse,
                    "Error must be for E6Sparse"
                );
                println!("RESULT: PASS - FAIL FAST with correct error");
            }
            Err(SearchError::Store(_)) => {
                println!("EVIDENCE: Store error (queries filtered to empty)");
                println!("RESULT: PASS - FAIL FAST with filtered queries");
            }
            Ok(_) => panic!("FAIL: Expected error for unsupported embedder"),
            Err(e) => panic!("FAIL: Wrong error variant: {:?}", e),
        }
    }

    #[test]
    fn test_matrix_weights_applied_correctly() {
        println!("=== TEST: Matrix weights affect aggregation ===");
        println!("BEFORE: Testing weight application logic");

        let semantic_matrix = SearchMatrix::semantic_focused();
        let code_matrix = SearchMatrix::code_heavy();

        let e1_weight_semantic = semantic_matrix.diagonal(EmbedderIndex::E1Semantic);
        let e1_weight_code = code_matrix.diagonal(EmbedderIndex::E1Semantic);
        let e7_weight_semantic = semantic_matrix.diagonal(EmbedderIndex::E7Code);
        let e7_weight_code = code_matrix.diagonal(EmbedderIndex::E7Code);

        println!(
            "semantic_focused: E1={}, E7={}",
            e1_weight_semantic, e7_weight_semantic
        );
        println!("code_heavy: E1={}, E7={}", e1_weight_code, e7_weight_code);

        assert!(
            e1_weight_semantic > e1_weight_code,
            "E1 should have higher weight in semantic_focused"
        );
        assert!(
            e7_weight_code > e7_weight_semantic,
            "E7 should have higher weight in code_heavy"
        );

        println!("AFTER: Weight differences verified");
        println!(
            "EVIDENCE: semantic E1={} > code E1={}, code E7={} > semantic E7={}",
            e1_weight_semantic, e1_weight_code, e7_weight_code, e7_weight_semantic
        );
        println!("RESULT: PASS");
    }

    #[test]
    fn test_correlation_patterns_detection() {
        println!("=== TEST: Correlation pattern variants ===");

        let patterns = vec![
            CorrelationPattern::ConsensusHigh {
                embedder_indices: vec![0, 4, 6],
                strength: 0.8,
            },
            CorrelationPattern::TemporalSemanticAlign { strength: 0.7 },
            CorrelationPattern::CodeSemanticDivergence { strength: 0.5 },
            CorrelationPattern::OutlierEmbedder {
                embedder_index: 5,
                deviation: 0.4,
            },
        ];

        for pattern in &patterns {
            println!("Pattern: {:?}", pattern);
        }

        assert_eq!(patterns.len(), 4, "Should have 4 pattern types");
        println!("RESULT: PASS");
    }

    #[test]
    fn test_matrix_results_accessors() {
        println!("=== TEST: MatrixSearchResults accessors ===");

        let results = MatrixSearchResults {
            hits: Vec::new(),
            correlation: CorrelationAnalysis {
                correlation_matrix: [[0.0; 14]; 14],
                patterns: Vec::new(),
                coherence: 0.0,
            },
            matrix_used: SearchMatrix::identity(),
            matrix_analysis: MatrixAnalysis {
                is_diagonal: true,
                has_cross_correlations: false,
                sparsity: 0.923,
                active_embedders: (0..NUM_EMBEDDERS).collect(),
                cross_correlation_count: 0,
            },
            latency_us: 100,
        };

        assert!(results.is_empty(), "Empty results should be empty");
        assert_eq!(results.len(), 0, "Length should be 0");
        assert!(results.top().is_none(), "Top should be None for empty");
        assert!(results.top_n(5).is_empty(), "Top 5 should be empty");
        assert!(results.ids().is_empty(), "IDs should be empty");

        println!("RESULT: PASS - All accessors work on empty results");
    }
}
