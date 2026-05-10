//! Fingerprint Creation Tests

use super::helpers::*;
use context_graph_embeddings::{
    storage::NUM_EMBEDDERS, ModelId, QuantizationMethod, StoredQuantizedFingerprint,
    MAX_QUANTIZED_SIZE_BYTES, STORAGE_VERSION,
};
use uuid::Uuid;

/// Test creating fingerprint with all production embeddings succeeds.
#[test]
fn test_create_fingerprint_with_all_embeddings() {
    let id = Uuid::new_v4();
    let embeddings = create_test_embeddings_with_deterministic_data(42);
    let topic_profile = create_topic_profile(42);
    let content_hash = create_content_hash(42);

    let fp = StoredQuantizedFingerprint::new(id, embeddings, topic_profile, content_hash);

    // Verify all fields
    assert_eq!(fp.id, id, "ID must match");
    assert_eq!(
        fp.version, STORAGE_VERSION,
        "Version must match STORAGE_VERSION"
    );
    assert_eq!(
        fp.embeddings.len(),
        NUM_EMBEDDERS,
        "Must have exactly one embedding per production slot"
    );
    assert_eq!(
        fp.topic_profile.len(),
        NUM_EMBEDDERS,
        "Topic profile must have one dimension per production slot"
    );
    assert_eq!(fp.content_hash, content_hash, "Content hash must match");
    assert!(!fp.deleted, "New fingerprint should not be deleted");
    assert_eq!(
        fp.access_count, 0,
        "New fingerprint should have zero access count"
    );

    println!(
        "[PASS] Created fingerprint with all {} embeddings",
        NUM_EMBEDDERS
    );
}

/// Test that missing any single embedding panics.
#[test]
#[should_panic(expected = "CONSTRUCTION ERROR")]
fn test_panic_on_missing_embedder_0() {
    let mut embeddings = create_test_embeddings_with_deterministic_data(42);
    embeddings.remove(&0);

    let _ = StoredQuantizedFingerprint::new(
        Uuid::new_v4(),
        embeddings,
        create_topic_profile(42),
        create_content_hash(42),
    );
}

#[test]
#[should_panic(expected = "CONSTRUCTION ERROR")]
fn test_panic_on_missing_embedder_6() {
    let mut embeddings = create_test_embeddings_with_deterministic_data(42);
    embeddings.remove(&6);

    let _ = StoredQuantizedFingerprint::new(
        Uuid::new_v4(),
        embeddings,
        create_topic_profile(42),
        create_content_hash(42),
    );
}

#[test]
#[should_panic(expected = "CONSTRUCTION ERROR")]
fn test_panic_on_missing_embedder_12() {
    let mut embeddings = create_test_embeddings_with_deterministic_data(42);
    embeddings.remove(&12);

    let _ = StoredQuantizedFingerprint::new(
        Uuid::new_v4(),
        embeddings,
        create_topic_profile(42),
        create_content_hash(42),
    );
}

/// Test that each embedding has correct quantization method.
#[test]
fn test_embeddings_have_correct_quantization_methods() {
    let embeddings = create_test_embeddings_with_deterministic_data(42);

    let fp = StoredQuantizedFingerprint::new(
        Uuid::new_v4(),
        embeddings,
        create_topic_profile(42),
        create_content_hash(42),
    );

    // Verify each production slot uses Constitution-correct method.
    for (i, model_id) in ModelId::production().iter().copied().enumerate() {
        let expected_method = QuantizationMethod::for_model_id(model_id);
        let actual_method = fp.get_embedding(i as u8).method;
        assert_eq!(
            actual_method, expected_method,
            "Embedder {} should use {:?}, got {:?}",
            i, expected_method, actual_method
        );
    }

    assert!(
        fp.validate_quantization_methods(),
        "All quantization methods should be valid"
    );
    println!(
        "[PASS] All {} embeddings have correct quantization methods",
        NUM_EMBEDDERS
    );
}

/// Test topic_profile is stored correctly.
#[test]
fn test_topic_profile_storage() {
    let pv = [0.5f32; 14]; // Uniform purpose vector

    let fp = StoredQuantizedFingerprint::new(
        Uuid::new_v4(),
        create_test_embeddings_with_deterministic_data(42),
        pv,
        create_content_hash(42),
    );

    assert_eq!(
        fp.topic_profile, pv,
        "Topic profile must be stored correctly"
    );

    println!("[PASS] topic_profile stored correctly");
}

/// Test estimated size is within Constitution bounds.
#[test]
fn test_estimated_size_within_bounds() {
    let fp = StoredQuantizedFingerprint::new(
        Uuid::new_v4(),
        create_test_embeddings_with_deterministic_data(42),
        create_topic_profile(42),
        create_content_hash(42),
    );

    let size = fp.estimated_size_bytes();

    // Must be less than MAX_QUANTIZED_SIZE_BYTES (25KB)
    assert!(
        size < MAX_QUANTIZED_SIZE_BYTES,
        "Estimated size {} exceeds maximum {} bytes",
        size,
        MAX_QUANTIZED_SIZE_BYTES
    );

    // Should be reasonable (> 1KB for all that data)
    assert!(
        size > 1000,
        "Estimated size {} seems too small for {} embeddings",
        size,
        NUM_EMBEDDERS
    );

    println!("[PASS] Estimated size {} bytes is within bounds", size);
}
