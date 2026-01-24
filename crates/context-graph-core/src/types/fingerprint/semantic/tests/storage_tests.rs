//! Storage and serialization tests for SemanticFingerprint.

use crate::types::fingerprint::semantic::*;
use crate::types::fingerprint::SparseVector;

// E5, E8, and E10 now use dual vectors for asymmetric similarity.
// New dense size = TOTAL_DENSE_DIMS + E5_DIM + E8_DIM + E10_DIM = 7808 + 768 + 384 + 768 = 9728
// Storage = 9728 * 4 = 38912 bytes
const NEW_DENSE_STORAGE: usize =
    (TOTAL_DENSE_DIMS + E5_DIM + E8_DIM + E10_DIM) * std::mem::size_of::<f32>();

#[test]
fn test_semantic_fingerprint_storage_size_zeroed() {
    let fp = SemanticFingerprint::zeroed();
    let size = fp.storage_size();

    // With dual E5, E8, and E10 vectors, storage includes both directions
    // TOTAL_DENSE_DIMS = 7808, + E5_DIM + E8_DIM + E10_DIM = 9728
    // 9728 * 4 bytes = 38912
    assert_eq!(NEW_DENSE_STORAGE, 38912);
    assert_eq!(size, NEW_DENSE_STORAGE);
}

#[test]
fn test_semantic_fingerprint_storage_size_with_sparse() {
    let mut fp = SemanticFingerprint::zeroed();

    fp.e6_sparse = SparseVector::new(vec![1, 10, 100, 1000], vec![0.1, 0.2, 0.3, 0.4])
        .expect("valid sparse vector");

    let size = fp.storage_size();

    // 38912 (dense with dual E5/E8/E10) + 24 (4 indices * 2 bytes + 4 values * 4 bytes = 8 + 16 = 24)
    let expected = NEW_DENSE_STORAGE + 24;
    assert_eq!(size, expected);
}

#[test]
fn test_semantic_fingerprint_storage_size_with_tokens() {
    let mut fp = SemanticFingerprint::zeroed();

    fp.e12_late_interaction = vec![vec![0.0; E12_TOKEN_DIM]; 10];

    let size = fp.storage_size();

    // 38912 (dense with dual E5/E8/E10) + 10 tokens * 128 dims * 4 bytes = 38912 + 5120
    let expected = NEW_DENSE_STORAGE + 5120;
    assert_eq!(size, expected);
}

#[test]
fn test_semantic_fingerprint_typical_storage_size() {
    let mut fp = SemanticFingerprint::zeroed();

    let indices: Vec<u16> = (0..1500_u16).map(|i| i * 20).collect();
    let values: Vec<f32> = vec![0.1; 1500];
    fp.e6_sparse = SparseVector::new(indices, values).expect("valid sparse vector");

    fp.e12_late_interaction = vec![vec![0.0; E12_TOKEN_DIM]; 50];

    let size = fp.storage_size();

    // 38912 (dense with dual E5/E8/E10) + 9000 (1500 sparse entries) + 25600 (50 tokens * 128 * 4)
    let expected = NEW_DENSE_STORAGE + 9000 + 25600;
    assert_eq!(size, expected);

    // Updated bounds for dual E5/E8/E10 vectors (expected: 73512 bytes)
    assert!(size > 70_000);
    assert!(size < 80_000);
}

#[test]
fn test_semantic_fingerprint_serialization_roundtrip() {
    let mut fp = SemanticFingerprint::zeroed();

    fp.e1_semantic[0] = 1.0;
    fp.e1_semantic[100] = 2.5;
    // E5 now uses dual vectors for asymmetric causal similarity
    fp.e5_causal_as_cause[50] = 3.125;
    fp.e5_causal_as_effect[50] = 3.125;
    fp.e9_hdc[1023] = -1.0; // E9 now has 1024 dims (projected)

    fp.e6_sparse =
        SparseVector::new(vec![100, 200, 300], vec![0.5, 0.6, 0.7]).expect("valid sparse vector");

    fp.e13_splade =
        SparseVector::new(vec![500, 1000, 1500], vec![0.8, 0.9, 1.0]).expect("valid sparse vector");

    let mut token = vec![0.0_f32; E12_TOKEN_DIM];
    token[0] = 1.0;
    token[127] = -1.0;
    fp.e12_late_interaction = vec![token; 3];

    let bytes = bincode::serialize(&fp).expect("serialization should succeed");
    let restored: SemanticFingerprint =
        bincode::deserialize(&bytes).expect("deserialization should succeed");

    assert_eq!(fp, restored);

    assert_eq!(restored.e1_semantic[0], 1.0);
    assert_eq!(restored.e1_semantic[100], 2.5);
    assert_eq!(restored.e5_causal_as_cause[50], 3.125);
    assert_eq!(restored.e5_causal_as_effect[50], 3.125);
    assert_eq!(restored.e9_hdc[1023], -1.0);
    assert_eq!(restored.e6_sparse.nnz(), 3);
    assert_eq!(restored.e13_splade.nnz(), 3);
    assert_eq!(restored.token_count(), 3);
}
