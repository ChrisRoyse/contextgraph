//! Basic tests for teleological matrix search.
//!
//! Tests for core similarity computation and configuration.

use crate::teleological::groups::GroupAlignments;
use crate::teleological::matrix_search::{
    embedder_names, ComparisonScope, MatrixSearchConfig, SearchStrategy, TeleologicalMatrixSearch,
};
use crate::teleological::synergy_matrix::SynergyMatrix;
use crate::teleological::types::NUM_EMBEDDERS;
use crate::teleological::vector::TeleologicalVector;
use crate::teleological::TopicProfile;

pub(super) fn make_test_vector(purpose_val: f32, corr_val: f32) -> TeleologicalVector {
    let tp = TopicProfile::new([purpose_val; NUM_EMBEDDERS]);
    let mut tv = TeleologicalVector::new(tp);
    for corr in tv.cross_correlations.iter_mut() {
        *corr = corr_val;
    }
    tv.group_alignments = GroupAlignments::new(
        purpose_val,
        purpose_val,
        purpose_val,
        purpose_val,
        purpose_val,
        purpose_val,
    );
    tv.confidence = 1.0; // Use 1.0 for test consistency
    tv
}

pub(super) fn make_varied_test_vector(seed: u32) -> TeleologicalVector {
    let mut alignments = [0.0f32; NUM_EMBEDDERS];
    let mut state = seed;
    for alignment in alignments.iter_mut() {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        *alignment = (state as f32 / u32::MAX as f32).max(0.05);
    }
    let tp = TopicProfile::new(alignments);
    let mut tv = TeleologicalVector::new(tp);
    for corr in tv.cross_correlations.iter_mut() {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        *corr = (state as f32 / u32::MAX as f32).max(0.05);
    }
    tv.group_alignments = GroupAlignments::from_alignments(&alignments, None);
    tv.confidence = 1.0;
    tv
}

#[test]
fn test_matrix_search_identical_vectors() {
    let search = TeleologicalMatrixSearch::new();
    let tv = make_test_vector(0.8, 0.6);

    let sim = search.similarity(&tv, &tv);
    assert!(
        (sim - 1.0).abs() < 0.02,
        "Self-similarity should be ~1.0, got {}",
        sim
    );
}

#[test]
fn test_matrix_search_different_vectors() {
    let search = TeleologicalMatrixSearch::new();
    let tv1 = make_varied_test_vector(12345);
    let tv2 = make_varied_test_vector(98765);

    let sim = search.similarity(&tv1, &tv2);
    assert!(
        sim < 0.99,
        "Different vectors should have lower similarity, got {}",
        sim
    );
}

#[test]
fn test_matrix_search_with_breakdown() {
    let search = TeleologicalMatrixSearch::new();
    let tv1 = make_test_vector(0.7, 0.5);
    let tv2 = make_test_vector(0.6, 0.4);

    let breakdown = search.similarity_with_breakdown(&tv1, &tv2);

    assert!(breakdown.overall > 0.0);
    assert!(breakdown.topic_profile > 0.0);
    assert!(breakdown.cross_correlations > 0.0);
    assert!(breakdown.group_alignments > 0.0);
    assert!(!breakdown.per_group.is_empty());
    assert!(!breakdown.top_correlation_pairs.is_empty());
}

#[test]
fn test_matrix_search_correlation_focused() {
    let config = MatrixSearchConfig::correlation_focused();
    let search = TeleologicalMatrixSearch::with_config(config);

    let tv1 = make_test_vector(0.5, 0.9);
    let tv2 = make_test_vector(0.5, 0.9);

    let sim = search.similarity(&tv1, &tv2);
    assert!(
        sim > 0.9,
        "Same correlations should have high similarity, got {}",
        sim
    );
}

#[test]
fn test_matrix_search_group_hierarchical() {
    let config = MatrixSearchConfig::group_hierarchical();
    let search = TeleologicalMatrixSearch::with_config(config);

    let tv1 = make_test_vector(0.8, 0.5);
    let tv2 = make_test_vector(0.8, 0.5);

    let sim = search.similarity(&tv1, &tv2);
    assert!(sim > 0.9, "Same groups should have high similarity");
}

#[test]
fn test_matrix_search_synergy_weighted() {
    let synergy = SynergyMatrix::with_base_synergies();
    let config = MatrixSearchConfig::with_synergy(synergy);
    let search = TeleologicalMatrixSearch::with_config(config);

    let tv1 = make_test_vector(0.7, 0.6);
    let tv2 = make_test_vector(0.7, 0.6);

    let sim = search.similarity(&tv1, &tv2);
    assert!(
        sim > 0.9,
        "Synergy-weighted similarity should be high for same vectors"
    );
}

#[test]
fn test_matrix_search_euclidean() {
    let config = MatrixSearchConfig {
        strategy: SearchStrategy::Euclidean,
        ..Default::default()
    };
    let search = TeleologicalMatrixSearch::with_config(config);

    let tv1 = make_test_vector(0.8, 0.6);
    let tv2 = make_test_vector(0.8, 0.6);

    let sim = search.similarity(&tv1, &tv2);
    assert!(
        (sim - 1.0).abs() < 0.01,
        "Identical vectors should have Euclidean similarity ~1.0"
    );
}

#[test]
fn test_matrix_search_specific_pairs() {
    let config = MatrixSearchConfig {
        scope: ComparisonScope::SpecificPairs(vec![(0, 1), (0, 2), (1, 2)]),
        ..Default::default()
    };
    let search = TeleologicalMatrixSearch::with_config(config);

    let tv1 = make_test_vector(0.7, 0.5);
    let tv2 = make_test_vector(0.3, 0.5);

    let sim = search.similarity(&tv1, &tv2);
    assert!(
        (sim - 1.0).abs() < 0.01,
        "Same correlations should have similarity ~1.0, got {}",
        sim
    );
}

#[test]
fn test_matrix_search_single_embedder_pattern() {
    let config = MatrixSearchConfig {
        scope: ComparisonScope::SingleEmbedderPattern(0),
        ..Default::default()
    };
    let search = TeleologicalMatrixSearch::with_config(config);

    let tv = make_test_vector(0.7, 0.5);
    let sim = search.similarity(&tv, &tv);
    assert!(
        (sim - 1.0).abs() < 0.01,
        "Self similarity for embedder pattern should be ~1.0"
    );
}

#[test]
fn test_adaptive_strategy() {
    let config = MatrixSearchConfig::adaptive();
    let search = TeleologicalMatrixSearch::with_config(config);

    let tv1 = make_test_vector(0.8, 0.6);
    let tv2 = make_test_vector(0.7, 0.5);

    let sim = search.similarity(&tv1, &tv2);
    assert!(
        sim > 0.0,
        "Adaptive strategy should produce valid similarity"
    );
}

#[test]
fn test_embedder_names() {
    // Test canonical names matching Embedder::name()
    assert_eq!(embedder_names::name(0), "E1_Semantic");
    assert_eq!(embedder_names::name(1), "E2_Temporal_Recent");
    assert_eq!(embedder_names::name(2), "E3_Temporal_Periodic");
    assert_eq!(embedder_names::name(3), "E4_Temporal_Positional");
    assert_eq!(embedder_names::name(4), "E5_Causal");
    assert_eq!(embedder_names::name(5), "E6_Sparse_Lexical");
    assert_eq!(embedder_names::name(6), "E7_Code");
    assert_eq!(embedder_names::name(7), "E8_Graph");
    assert_eq!(embedder_names::name(8), "E9_HDC");
    assert_eq!(embedder_names::name(9), "E10_Multimodal");
    assert_eq!(embedder_names::name(10), "E11_Entity");
    assert_eq!(embedder_names::name(11), "E12_Late_Interaction");
    assert_eq!(embedder_names::name(12), "E13_SPLADE");
    assert_eq!(embedder_names::name(99), "Unknown");
}
