//! Core tests for SemanticFingerprint basic functionality.

use crate::types::fingerprint::semantic::*;
use crate::types::fingerprint::SparseVector;

#[test]
fn test_semantic_fingerprint_zeroed() {
    let fp = SemanticFingerprint::zeroed();

    assert!(fp.e1_semantic.iter().all(|&v| v == 0.0));
    assert!(fp.e2_temporal_recent.iter().all(|&v| v == 0.0));
    assert!(fp.e3_temporal_periodic.iter().all(|&v| v == 0.0));
    assert!(fp.e4_temporal_positional.iter().all(|&v| v == 0.0));
    // E5 now uses dual vectors for asymmetric causal similarity
    assert!(fp.e5_causal_as_cause.iter().all(|&v| v == 0.0));
    assert!(fp.e5_causal_as_effect.iter().all(|&v| v == 0.0));
    assert!(fp.e5_causal.is_empty()); // Legacy field is empty
    assert!(fp.e7_code.iter().all(|&v| v == 0.0));
    // E8 now uses dual vectors for asymmetric graph similarity
    assert!(fp.e8_graph_as_source.iter().all(|&v| v == 0.0));
    assert!(fp.e8_graph_as_target.iter().all(|&v| v == 0.0));
    assert!(fp.e8_graph.is_empty()); // Legacy field is empty
    assert!(fp.e9_hdc.iter().all(|&v| v == 0.0));
    // E10 now uses dual vectors for asymmetric intent/context similarity
    assert!(fp.e10_multimodal_as_intent.iter().all(|&v| v == 0.0));
    assert!(fp.e10_multimodal_as_context.iter().all(|&v| v == 0.0));
    assert!(fp.e10_multimodal.is_empty()); // Legacy field is empty
    assert!(fp.e11_entity.iter().all(|&v| v == 0.0));

    assert!(fp.e6_sparse.is_empty());
    assert!(fp.e12_late_interaction.is_empty());
    assert!(fp.e13_splade.is_empty());
}

#[test]
fn test_semantic_fingerprint_dimensions() {
    let fp = SemanticFingerprint::zeroed();

    assert_eq!(fp.e1_semantic.len(), E1_DIM);
    assert_eq!(fp.e2_temporal_recent.len(), E2_DIM);
    assert_eq!(fp.e3_temporal_periodic.len(), E3_DIM);
    assert_eq!(fp.e4_temporal_positional.len(), E4_DIM);
    // E5 now uses dual vectors for asymmetric causal similarity
    assert_eq!(fp.e5_causal_as_cause.len(), E5_DIM);
    assert_eq!(fp.e5_causal_as_effect.len(), E5_DIM);
    assert!(fp.e5_causal.is_empty(), "Legacy e5_causal should be empty in new format");
    assert_eq!(fp.e7_code.len(), E7_DIM);
    // E8 now uses dual vectors for asymmetric graph similarity
    assert_eq!(fp.e8_graph_as_source.len(), E8_DIM);
    assert_eq!(fp.e8_graph_as_target.len(), E8_DIM);
    assert!(fp.e8_graph.is_empty(), "Legacy e8_graph should be empty in new format");
    assert_eq!(fp.e9_hdc.len(), E9_DIM);
    // E10 now uses dual vectors for asymmetric intent/context similarity
    assert_eq!(fp.e10_multimodal_as_intent.len(), E10_DIM);
    assert_eq!(fp.e10_multimodal_as_context.len(), E10_DIM);
    assert!(fp.e10_multimodal.is_empty(), "Legacy e10_multimodal should be empty in new format");
    assert_eq!(fp.e11_entity.len(), E11_DIM);

    let expected_total =
        E1_DIM + E2_DIM + E3_DIM + E4_DIM + E5_DIM + E7_DIM + E8_DIM + E9_DIM + E10_DIM + E11_DIM;
    assert_eq!(TOTAL_DENSE_DIMS, expected_total);
    // 1024 + 512 + 512 + 512 + 768 + 1536 + 1024 + 1024 + 768 + 768 = 8448
    // (E8 upgraded from 384D to 1024D)
    assert_eq!(TOTAL_DENSE_DIMS, 8448);
}

#[test]
fn test_semantic_fingerprint_get_embedding() {
    let fp = SemanticFingerprint::zeroed();

    for idx in 0..NUM_EMBEDDERS {
        assert!(
            fp.get_embedding(idx).is_some(),
            "Embedding {} should be accessible",
            idx
        );
    }

    assert!(fp.get_embedding(13).is_none());
    assert!(fp.get_embedding(100).is_none());
}

#[test]
fn test_semantic_fingerprint_get_embedding_types() {
    let fp = SemanticFingerprint::zeroed();

    for idx in 0..5 {
        match fp.get_embedding(idx) {
            Some(EmbeddingSlice::Dense(_)) => {}
            _ => panic!("E{} should be Dense", idx + 1),
        }
    }

    match fp.get_embedding(5) {
        Some(EmbeddingSlice::Sparse(_)) => {}
        _ => panic!("E6 should be Sparse"),
    }

    for idx in 6..11 {
        match fp.get_embedding(idx) {
            Some(EmbeddingSlice::Dense(_)) => {}
            _ => panic!("E{} should be Dense", idx + 1),
        }
    }

    match fp.get_embedding(11) {
        Some(EmbeddingSlice::TokenLevel(_)) => {}
        _ => panic!("E12 should be TokenLevel"),
    }

    match fp.get_embedding(12) {
        Some(EmbeddingSlice::Sparse(_)) => {}
        _ => panic!("E13 should be Sparse"),
    }
}

#[test]
fn test_semantic_fingerprint_token_count() {
    let mut fp = SemanticFingerprint::zeroed();
    assert_eq!(fp.token_count(), 0);

    fp.e12_late_interaction = vec![vec![0.0; E12_TOKEN_DIM]; 5];
    assert_eq!(fp.token_count(), 5);

    fp.e12_late_interaction = vec![vec![0.0; E12_TOKEN_DIM]; 100];
    assert_eq!(fp.token_count(), 100);
}

// NOTE: test_semantic_fingerprint_default was removed because SemanticFingerprint
// no longer implements Default. This is intentional - all-zero fingerprints
// pass validation but cause silent failures in search/alignment operations.
// Use SemanticFingerprint::zeroed() explicitly when placeholder data is needed.

#[test]
fn test_semantic_fingerprint_partial_eq() {
    let fp1 = SemanticFingerprint::zeroed();
    let fp2 = SemanticFingerprint::zeroed();
    assert_eq!(fp1, fp2);

    let mut fp3 = SemanticFingerprint::zeroed();
    fp3.e1_semantic[0] = 1.0;
    assert_ne!(fp1, fp3);
}

#[test]
fn test_embedding_name() {
    assert_eq!(SemanticFingerprint::embedding_name(0), Some("E1_Semantic"));
    assert_eq!(
        SemanticFingerprint::embedding_name(5),
        Some("E6_Sparse_Lexical")
    );
    assert_eq!(
        SemanticFingerprint::embedding_name(11),
        Some("E12_Late_Interaction")
    );
    assert_eq!(SemanticFingerprint::embedding_name(12), Some("E13_SPLADE"));
    assert_eq!(SemanticFingerprint::embedding_name(13), None);
}

#[test]
fn test_embedding_dim() {
    assert_eq!(SemanticFingerprint::embedding_dim(0), Some(E1_DIM));
    assert_eq!(SemanticFingerprint::embedding_dim(5), Some(E6_SPARSE_VOCAB));
    assert_eq!(SemanticFingerprint::embedding_dim(11), Some(E12_TOKEN_DIM));
    assert_eq!(
        SemanticFingerprint::embedding_dim(12),
        Some(E13_SPLADE_VOCAB)
    );
    assert_eq!(SemanticFingerprint::embedding_dim(13), None);
}

#[test]
fn test_dimension_constants() {
    assert_eq!(E1_DIM, 1024);
    assert_eq!(E2_DIM, 512);
    assert_eq!(E3_DIM, 512);
    assert_eq!(E4_DIM, 512);
    assert_eq!(E5_DIM, 768);
    assert_eq!(E6_SPARSE_VOCAB, 30_522);
    assert_eq!(E7_DIM, 1536);
    assert_eq!(E8_DIM, 1024); // Upgraded from MiniLM (384D) to e5-large-v2 (1024D)
    assert_eq!(E9_DIM, 1024); // HDC projected dimension
    assert_eq!(E10_DIM, 768);
    assert_eq!(E11_DIM, 768); // KEPLER (was 384 with MiniLM)
    assert_eq!(E12_TOKEN_DIM, 128);
    assert_eq!(E13_SPLADE_VOCAB, 30_522);
    assert_eq!(NUM_EMBEDDERS, 13);

    let calculated =
        E1_DIM + E2_DIM + E3_DIM + E4_DIM + E5_DIM + E7_DIM + E8_DIM + E9_DIM + E10_DIM + E11_DIM;
    assert_eq!(TOTAL_DENSE_DIMS, calculated);
}

#[test]
fn test_e13_splade_nnz() {
    let mut fp = SemanticFingerprint::zeroed();
    assert_eq!(fp.e13_splade_nnz(), 0);

    fp.e13_splade = SparseVector::new(vec![100, 200, 300, 400, 500], vec![0.1, 0.2, 0.3, 0.4, 0.5])
        .expect("valid sparse vector");
    assert_eq!(fp.e13_splade_nnz(), 5);
}
