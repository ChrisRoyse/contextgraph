//! Edge Case Tests

use super::helpers::*;
use context_graph_embeddings::{storage::NUM_EMBEDDERS, IndexEntry, StoredQuantizedFingerprint};
use uuid::Uuid;

/// Test get_embedding panics for invalid index.
#[test]
#[should_panic(expected = "STORAGE ERROR")]
fn test_get_embedding_invalid_index() {
    let fp = StoredQuantizedFingerprint::new(
        Uuid::new_v4(),
        create_test_embeddings_with_deterministic_data(42),
        create_topic_profile(42),
        create_content_hash(42),
    );

    let _ = fp.get_embedding(99); // Invalid index
}

/// Test very small vectors in IndexEntry.
#[test]
fn test_small_vector_values() {
    let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![1e-10, 1e-10, 1e-10]);
    let normalized = entry.normalized();

    // Should still produce valid normalized vector
    let norm: f32 = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 1e-3 || norm == 0.0,
        "Normalized very small vector should have unit norm or be zero"
    );

    println!("[PASS] Small vector values handled correctly");
}

/// Test very large vector values in IndexEntry.
#[test]
fn test_large_vector_values() {
    let entry = IndexEntry::new(Uuid::new_v4(), 0, vec![1e10, 1e10, 1e10]);
    let sim = entry.cosine_similarity(&[1e10, 1e10, 1e10]);

    assert!(
        (sim - 1.0).abs() < 1e-3,
        "Large identical vectors should have similarity ~1.0, got {}",
        sim
    );

    println!("[PASS] Large vector values handled correctly");
}

/// Test fingerprint with all equal purpose values.
#[test]
fn test_uniform_topic_profile() {
    let fp = StoredQuantizedFingerprint::new(
        Uuid::new_v4(),
        create_test_embeddings_with_deterministic_data(42),
        [0.5f32; 14],
        create_content_hash(42),
    );

    // Verify topic_profile is correctly stored
    assert_eq!(fp.topic_profile, [0.5f32; 14]);

    println!("[PASS] Uniform topic profile handled correctly");
}

/// Test multiple roundtrips don't accumulate errors.
#[test]
fn test_multiple_roundtrips_no_drift() {
    let original = StoredQuantizedFingerprint::new(
        Uuid::new_v4(),
        create_test_embeddings_with_deterministic_data(42),
        create_topic_profile(42),
        create_content_hash(42),
    );

    // 10 roundtrips
    let mut current = original.clone();
    for _ in 0..10 {
        let json = serde_json::to_string(&current).expect("Serialize");
        current = serde_json::from_str(&json).expect("Deserialize");
    }

    // Verify no drift
    assert_eq!(current.id, original.id, "ID drifted after 10 roundtrips");
    assert_eq!(
        current.content_hash, original.content_hash,
        "Content hash drifted"
    );
    assert_eq!(
        current.topic_profile, original.topic_profile,
        "Purpose vector drifted"
    );

    for i in 0..NUM_EMBEDDERS as u8 {
        let orig_data = &original.get_embedding(i).data;
        let curr_data = &current.get_embedding(i).data;
        assert_eq!(
            orig_data, curr_data,
            "Embedder {} data drifted after 10 roundtrips",
            i
        );
    }

    println!("[PASS] 10 roundtrips with no data drift");
}
