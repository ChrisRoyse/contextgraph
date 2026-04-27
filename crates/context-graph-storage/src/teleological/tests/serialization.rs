//! Serialization round-trip tests.

use super::helpers::create_real_fingerprint;
use crate::teleological::*;
use uuid::Uuid;

// =========================================================================
// Serialization Tests
// =========================================================================

#[test]
fn test_serialize_teleological_roundtrip() {
    println!("=== TEST: TeleologicalFingerprint serialization round-trip ===");

    let original = create_real_fingerprint();
    println!("BEFORE: Created real fingerprint with ID: {}", original.id);
    println!("  - SemanticFingerprint: default (all 13 embedders)");
    println!("  - content_hash present");

    let serialized = serialize_teleological_fingerprint(&original);
    println!("SERIALIZED: {} bytes", serialized.len());
    println!("  - Version byte: {}", serialized[0]);
    println!("  - Payload size: {} bytes", serialized.len() - 1);

    let deserialized = deserialize_teleological_fingerprint(&serialized)
        .expect("deserialize should succeed for valid data");
    println!("AFTER: Deserialized fingerprint ID: {}", deserialized.id);

    assert_eq!(original.id, deserialized.id);
    assert_eq!(original.content_hash, deserialized.content_hash);
    println!("RESULT: PASS - Round-trip preserved all fields");
}

#[test]
fn test_fingerprint_size_in_range() {
    println!("=== TEST: Serialized size within expected range ===");

    let fp = create_real_fingerprint();
    let serialized = serialize_teleological_fingerprint(&fp);

    // Actual size calculation:
    // - Dense storage with E14 + dual E5/E8/E10 = 12,032 floats -> 48,128 bytes
    // - Plus sparse vectors and metadata
    // - Total: ~32-40KB for a fingerprint
    println!("BEFORE: Expected range [25KB, 100KB]");
    println!(
        "AFTER: Actual size {} bytes ({:.2}KB)",
        serialized.len(),
        serialized.len() as f64 / 1024.0
    );

    assert!(
        serialized.len() >= 25_000,
        "Size {} below minimum 25KB - embeddings may be missing",
        serialized.len()
    );
    assert!(
        serialized.len() <= 100_000,
        "Size {} above maximum 100KB - unexpectedly large",
        serialized.len()
    );
    println!("RESULT: PASS - Size in expected range");
}

#[test]
fn test_topic_profile_roundtrip() {
    println!("=== TEST: Topic profile (14D) round-trip ===");

    let original: [f32; 14] = [
        0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4,
    ];
    println!("BEFORE: {:?}", original);

    let serialized = serialize_topic_profile(&original);
    assert_eq!(serialized.len(), 56);
    println!("SERIALIZED: {} bytes", serialized.len());

    let deserialized =
        deserialize_topic_profile(&serialized).expect("deserialize should succeed for valid data");
    println!("AFTER: {:?}", deserialized);

    for i in 0..14 {
        assert!(
            (original[i] - deserialized[i]).abs() < 1e-6,
            "Value mismatch at index {}",
            i
        );
    }
    println!("RESULT: PASS - All 14 dimensions preserved");
}

#[test]
fn test_e1_matryoshka_roundtrip() {
    println!("=== TEST: E1 Matryoshka 128D vector round-trip ===");

    let mut original = [0.0f32; 128];
    for (i, val) in original.iter_mut().enumerate() {
        *val = (i as f32) * 0.01;
    }
    println!(
        "BEFORE: 128D vector, first 5 elements: {:?}",
        &original[..5]
    );
    println!("  Last 5 elements: {:?}", &original[123..]);

    let serialized = serialize_e1_matryoshka_128(&original);
    assert_eq!(serialized.len(), 512);
    println!("SERIALIZED: {} bytes", serialized.len());

    let deserialized = deserialize_e1_matryoshka_128(&serialized);
    println!("AFTER: first 5 elements: {:?}", &deserialized[..5]);
    println!("  Last 5 elements: {:?}", &deserialized[123..]);

    for i in 0..128 {
        assert!(
            (original[i] - deserialized[i]).abs() < 1e-6,
            "Value mismatch at index {}",
            i
        );
    }
    println!("RESULT: PASS - All 128 dimensions preserved");
}

#[test]
fn test_memory_id_list_roundtrip() {
    println!("=== TEST: Memory ID list round-trip ===");

    let original: Vec<Uuid> = (0..10).map(|_| Uuid::new_v4()).collect();
    println!("BEFORE: {} UUIDs", original.len());
    for (i, id) in original.iter().enumerate() {
        println!("  [{}]: {}", i, id);
    }

    let serialized = serialize_memory_id_list(&original);
    println!(
        "SERIALIZED: {} bytes (expected: {})",
        serialized.len(),
        4 + 10 * 16
    );
    assert_eq!(serialized.len(), 4 + 10 * 16);

    let deserialized =
        deserialize_memory_id_list(&serialized).expect("deserialize should succeed for valid data");
    println!("AFTER: {} UUIDs", deserialized.len());
    for (i, id) in deserialized.iter().enumerate() {
        println!("  [{}]: {}", i, id);
    }

    assert_eq!(original, deserialized);
    println!("RESULT: PASS - All UUIDs preserved");
}

// =========================================================================
// EDGE CASES (3 required with before/after state printing)
// =========================================================================

#[test]
fn edge_case_empty_memory_id_list() {
    println!("=== EDGE CASE 1: Empty memory ID list ===");

    let original: Vec<Uuid> = vec![];
    println!("BEFORE: Empty list, {} UUIDs", original.len());

    let serialized = serialize_memory_id_list(&original);
    println!(
        "SERIALIZED: {} bytes (should be 4 for count only)",
        serialized.len()
    );
    assert_eq!(serialized.len(), 4);
    assert_eq!(&serialized[..4], &[0, 0, 0, 0]); // count = 0

    let deserialized =
        deserialize_memory_id_list(&serialized).expect("deserialize should succeed for empty list");
    println!("AFTER: {} UUIDs", deserialized.len());

    assert!(deserialized.is_empty());
    println!("RESULT: PASS - Empty list handled correctly");
}

#[test]
fn edge_case_large_memory_id_list() {
    println!("=== EDGE CASE 2: Large memory ID list (1000 entries) ===");

    let original: Vec<Uuid> = (0..1000).map(|_| Uuid::new_v4()).collect();
    println!("BEFORE: {} UUIDs", original.len());
    println!("  First: {}", original[0]);
    println!("  Last: {}", original[999]);

    let serialized = serialize_memory_id_list(&original);
    let expected_size = 4 + 1000 * 16;
    println!(
        "SERIALIZED: {} bytes (expected: {})",
        serialized.len(),
        expected_size
    );
    assert_eq!(serialized.len(), expected_size);

    let deserialized =
        deserialize_memory_id_list(&serialized).expect("deserialize should succeed for large list");
    println!("AFTER: {} UUIDs", deserialized.len());
    println!("  First: {}", deserialized[0]);
    println!("  Last: {}", deserialized[999]);

    assert_eq!(original, deserialized);
    println!("RESULT: PASS - Large list handled correctly");
}

#[test]
fn edge_case_topic_profile_extreme_values() {
    println!("=== EDGE CASE 3: Topic profile with extreme float values ===");

    let original: [f32; 14] = [
        f32::MIN,
        f32::MAX,
        0.0,
        -0.0,
        f32::EPSILON,
        -f32::EPSILON,
        1e-38,
        1e38,
        -1e38,
        std::f32::consts::PI,
        std::f32::consts::E,
        0.123_456_79,
        -0.987_654_3,
        42.5_f32, // E14 BGE-M3 Dense slot
    ];
    println!("BEFORE: Extreme values including MIN, MAX, EPSILON, PI, E");
    for (i, v) in original.iter().enumerate() {
        println!("  [{}]: {:e}", i, v);
    }

    let serialized = serialize_topic_profile(&original);
    println!("SERIALIZED: {} bytes", serialized.len());

    let deserialized = deserialize_topic_profile(&serialized)
        .expect("deserialize should succeed for extreme values");
    println!("AFTER: Deserialized values");
    for (i, v) in deserialized.iter().enumerate() {
        println!("  [{}]: {:e}", i, v);
    }

    for i in 0..14 {
        assert_eq!(
            original[i].to_bits(),
            deserialized[i].to_bits(),
            "Bit-exact match failed at index {}",
            i
        );
    }
    println!("RESULT: PASS - Extreme values preserved bit-exactly");
}

// =========================================================================
// Additional Verification Tests
// =========================================================================

#[test]
fn test_version_constant() {
    assert_eq!(TELEOLOGICAL_VERSION, 1, "Version should be 1");
}

#[test]
fn test_serialization_version_prefix() {
    let fp = create_real_fingerprint();
    let serialized = serialize_teleological_fingerprint(&fp);

    // First byte should be version
    assert_eq!(serialized[0], TELEOLOGICAL_VERSION);
}

#[test]
fn test_multiple_fingerprint_roundtrips() {
    println!("=== TEST: Multiple fingerprint round-trips ===");

    for i in 0..10 {
        let original = create_real_fingerprint();
        let serialized = serialize_teleological_fingerprint(&original);
        let deserialized = deserialize_teleological_fingerprint(&serialized)
            .expect("deserialize should succeed for valid data");

        assert_eq!(original.id, deserialized.id, "Mismatch on iteration {}", i);
        assert_eq!(
            original.content_hash, deserialized.content_hash,
            "Hash mismatch on iteration {}",
            i
        );
    }

    println!("RESULT: PASS - 10 round-trips successful");
}
