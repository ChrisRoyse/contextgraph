//! Serialization Roundtrip Tests

use super::helpers::*;
use context_graph_embeddings::{
    storage::NUM_EMBEDDERS, EmbedderQueryResult, MultiSpaceQueryResult, StoredQuantizedFingerprint,
};
use uuid::Uuid;

/// Test JSON serialization roundtrip preserves all fingerprint data.
#[test]
fn test_json_roundtrip_fingerprint() {
    let id = Uuid::new_v4();
    let embeddings = create_test_embeddings_with_deterministic_data(99);
    let topic_profile = create_topic_profile(99);
    let content_hash = create_content_hash(99);

    let original =
        StoredQuantizedFingerprint::new(id, embeddings.clone(), topic_profile, content_hash);

    // Serialize to JSON
    let json = serde_json::to_string(&original).expect("JSON serialization failed");

    // Deserialize from JSON
    let restored: StoredQuantizedFingerprint =
        serde_json::from_str(&json).expect("JSON deserialization failed");

    // Verify all fields match exactly
    assert_eq!(restored.id, original.id, "ID mismatch after roundtrip");
    assert_eq!(
        restored.version, original.version,
        "Version mismatch after roundtrip"
    );
    assert_eq!(
        restored.embeddings.len(),
        original.embeddings.len(),
        "Embeddings count mismatch"
    );

    for i in 0..NUM_EMBEDDERS as u8 {
        let orig_emb = original.get_embedding(i);
        let rest_emb = restored.get_embedding(i);
        assert_eq!(
            orig_emb.method, rest_emb.method,
            "Embedder {} method mismatch",
            i
        );
        assert_eq!(
            orig_emb.original_dim, rest_emb.original_dim,
            "Embedder {} dim mismatch",
            i
        );
        assert_eq!(orig_emb.data, rest_emb.data, "Embedder {} data mismatch", i);
    }

    assert_eq!(
        restored.topic_profile, original.topic_profile,
        "Topic profile mismatch"
    );
    assert_eq!(
        restored.content_hash, original.content_hash,
        "Content hash mismatch"
    );

    println!("[PASS] JSON roundtrip preserves all fingerprint data");
}

/// Test bincode serialization roundtrip preserves all data.
#[test]
fn test_bincode_roundtrip_fingerprint() {
    let original = StoredQuantizedFingerprint::new(
        Uuid::new_v4(),
        create_test_embeddings_with_deterministic_data(77),
        create_topic_profile(77),
        create_content_hash(77),
    );

    // Serialize to bincode
    let bytes = bincode::serialize(&original).expect("Bincode serialization failed");

    // Deserialize from bincode
    let restored: StoredQuantizedFingerprint =
        bincode::deserialize(&bytes).expect("Bincode deserialization failed");

    // Verify critical fields
    assert_eq!(
        restored.id, original.id,
        "ID mismatch after bincode roundtrip"
    );
    assert_eq!(restored.version, original.version, "Version mismatch");
    assert_eq!(
        restored.embeddings.len(),
        NUM_EMBEDDERS,
        "Must have one embedding per production slot"
    );
    assert_eq!(
        restored.topic_profile, original.topic_profile,
        "Topic profile mismatch"
    );
    assert_eq!(
        restored.content_hash, original.content_hash,
        "Content hash mismatch"
    );

    // Verify embedding data integrity
    for i in 0..NUM_EMBEDDERS as u8 {
        let orig_data = &original.get_embedding(i).data;
        let rest_data = &restored.get_embedding(i).data;
        assert_eq!(
            orig_data, rest_data,
            "Embedder {} data corrupted after bincode roundtrip",
            i
        );
    }

    println!(
        "[PASS] Bincode roundtrip preserves all fingerprint data ({} bytes)",
        bytes.len()
    );
}

/// Test EmbedderQueryResult serde roundtrip.
#[test]
fn test_embedder_query_result_roundtrip() {
    let original = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 5, 0.876_543_2, 42);

    let json = serde_json::to_string(&original).expect("Serialize");
    let restored: EmbedderQueryResult = serde_json::from_str(&json).expect("Deserialize");

    assert_eq!(restored.id, original.id);
    assert_eq!(restored.embedder_idx, original.embedder_idx);
    assert!((restored.similarity - original.similarity).abs() < f32::EPSILON);
    assert!((restored.distance - original.distance).abs() < f32::EPSILON);
    assert_eq!(restored.rank, original.rank);

    println!("[PASS] EmbedderQueryResult JSON roundtrip preserves all data");
}

/// Test MultiSpaceQueryResult bincode roundtrip.
/// Note: JSON does not support NaN values natively, so we use bincode for this test.
#[test]
fn test_multi_space_query_result_roundtrip() {
    let id = Uuid::new_v4();
    let results = vec![
        EmbedderQueryResult::from_similarity(id, 0, 0.9, 0),
        EmbedderQueryResult::from_similarity(id, 1, 0.85, 1),
        EmbedderQueryResult::from_similarity(id, 2, 0.8, 2),
    ];

    let original = MultiSpaceQueryResult::from_embedder_results(id, &results);

    // Use bincode instead of JSON because JSON doesn't support NaN
    let bytes = bincode::serialize(&original).expect("Serialize");
    let restored: MultiSpaceQueryResult = bincode::deserialize(&bytes).expect("Deserialize");

    assert_eq!(restored.id, original.id);
    assert_eq!(restored.embedder_count, original.embedder_count);
    assert!((restored.rrf_score - original.rrf_score).abs() < f32::EPSILON);

    // Check embedder similarities including NaN handling
    for i in 0..NUM_EMBEDDERS {
        let orig = original.embedder_similarities[i];
        let rest = restored.embedder_similarities[i];
        if orig.is_nan() {
            assert!(rest.is_nan(), "Embedder {} should be NaN", i);
        } else {
            assert!(
                (rest - orig).abs() < f32::EPSILON,
                "Embedder {} similarity mismatch",
                i
            );
        }
    }

    println!("[PASS] MultiSpaceQueryResult bincode roundtrip preserves all data (incl. NaN)");
}
