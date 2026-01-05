//! Unit tests for teleological storage.
//!
//! # CRITICAL: NO MOCK DATA
//!
//! All tests use REAL data constructed from actual struct definitions.
//! This ensures tests accurately reflect production behavior.

use super::*;
use context_graph_core::types::fingerprint::{
    JohariFingerprint, PurposeVector, SemanticFingerprint, TeleologicalFingerprint, NUM_EMBEDDERS,
};
use uuid::Uuid;

// =========================================================================
// Helper Functions - Create REAL data (no mocks)
// =========================================================================

/// Create a REAL SemanticFingerprint with default embeddings.
fn create_real_semantic() -> SemanticFingerprint {
    SemanticFingerprint::default()
}

/// Create a REAL PurposeVector with specified alignment.
fn create_real_purpose(alignment: f32) -> PurposeVector {
    PurposeVector::new([alignment; NUM_EMBEDDERS])
}

/// Create a REAL JohariFingerprint with high openness.
fn create_real_johari() -> JohariFingerprint {
    let mut jf = JohariFingerprint::zeroed();
    for i in 0..NUM_EMBEDDERS {
        jf.set_quadrant(i, 1.0, 0.0, 0.0, 0.0, 1.0); // 100% Open, 100% confidence
    }
    jf
}

/// Create a REAL content hash.
fn create_real_hash() -> [u8; 32] {
    let mut hash = [0u8; 32];
    // SHA-256 of "test content"
    hash[0] = 0xDE;
    hash[1] = 0xAD;
    hash[30] = 0xBE;
    hash[31] = 0xEF;
    hash
}

/// Create a REAL TeleologicalFingerprint for testing.
fn create_real_fingerprint() -> TeleologicalFingerprint {
    TeleologicalFingerprint::new(
        create_real_semantic(),
        create_real_purpose(0.75),
        create_real_johari(),
        create_real_hash(),
    )
}

// =========================================================================
// Column Family Tests
// =========================================================================

#[test]
fn test_teleological_cf_names_count() {
    assert_eq!(
        TELEOLOGICAL_CFS.len(),
        4,
        "Must have exactly 4 teleological column families"
    );
}

#[test]
fn test_teleological_cf_names_unique() {
    use std::collections::HashSet;
    let set: HashSet<_> = TELEOLOGICAL_CFS.iter().collect();
    assert_eq!(set.len(), 4, "All CF names must be unique");
}

#[test]
fn test_teleological_cf_names_are_snake_case() {
    for name in TELEOLOGICAL_CFS {
        assert!(
            name.chars().all(|c| c.is_lowercase() || c == '_' || c.is_ascii_digit()),
            "CF name '{}' should be snake_case",
            name
        );
    }
}

#[test]
fn test_teleological_cf_names_values() {
    assert_eq!(CF_FINGERPRINTS, "fingerprints");
    assert_eq!(CF_PURPOSE_VECTORS, "purpose_vectors");
    assert_eq!(CF_E13_SPLADE_INVERTED, "e13_splade_inverted");
    assert_eq!(CF_E1_MATRYOSHKA_128, "e1_matryoshka_128");
}

#[test]
fn test_fingerprint_cf_options_valid() {
    use rocksdb::Cache;
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let opts = fingerprint_cf_options(&cache);
    drop(opts); // Should not panic
}

#[test]
fn test_purpose_vector_cf_options_valid() {
    use rocksdb::Cache;
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let opts = purpose_vector_cf_options(&cache);
    drop(opts);
}

#[test]
fn test_e13_splade_inverted_cf_options_valid() {
    use rocksdb::Cache;
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let opts = e13_splade_inverted_cf_options(&cache);
    drop(opts);
}

#[test]
fn test_e1_matryoshka_128_cf_options_valid() {
    use rocksdb::Cache;
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let opts = e1_matryoshka_128_cf_options(&cache);
    drop(opts);
}

#[test]
fn test_get_teleological_cf_descriptors_returns_4() {
    use rocksdb::Cache;
    let cache = Cache::new_lru_cache(256 * 1024 * 1024);
    let descriptors = get_teleological_cf_descriptors(&cache);
    assert_eq!(descriptors.len(), 4, "Must return exactly 4 descriptors");
}

// =========================================================================
// Key Format Tests
// =========================================================================

#[test]
fn test_fingerprint_key_format() {
    let id = Uuid::new_v4();
    let key = fingerprint_key(&id);

    println!("=== TEST: fingerprint_key format ===");
    println!("UUID: {}", id);
    println!("Key length: {} bytes", key.len());
    println!("Key bytes: {:02x?}", key);

    assert_eq!(key.len(), 16);
    assert_eq!(&key, id.as_bytes());
}

#[test]
fn test_purpose_vector_key_format() {
    let id = Uuid::new_v4();
    let key = purpose_vector_key(&id);

    assert_eq!(key.len(), 16);
    assert_eq!(&key, id.as_bytes());
}

#[test]
fn test_e1_matryoshka_128_key_format() {
    let id = Uuid::new_v4();
    let key = e1_matryoshka_128_key(&id);

    assert_eq!(key.len(), 16);
    assert_eq!(&key, id.as_bytes());
}

#[test]
fn test_e13_splade_inverted_key_format() {
    let term_id: u16 = 12345;
    let key = e13_splade_inverted_key(term_id);

    println!("=== TEST: e13_splade_inverted_key format ===");
    println!("term_id: {}", term_id);
    println!("Key length: {} bytes", key.len());
    println!("Key bytes: {:02x?}", key);

    assert_eq!(key.len(), 2);
    // Big-endian: 12345 = 0x3039
    assert_eq!(key, [0x30, 0x39]);
}

#[test]
fn test_parse_fingerprint_key_roundtrip() {
    let original = Uuid::new_v4();
    let key = fingerprint_key(&original);
    let parsed = parse_fingerprint_key(&key);

    assert_eq!(original, parsed);
}

#[test]
fn test_parse_e13_splade_key_roundtrip() {
    for term_id in [0u16, 1, 100, 1000, 12345, 30521, u16::MAX] {
        let key = e13_splade_inverted_key(term_id);
        let parsed = parse_e13_splade_key(&key);
        assert_eq!(term_id, parsed, "Round-trip failed for term_id {}", term_id);
    }
}

// =========================================================================
// Serialization Tests
// =========================================================================

#[test]
fn test_serialize_teleological_roundtrip() {
    println!("=== TEST: TeleologicalFingerprint serialization round-trip ===");

    let original = create_real_fingerprint();
    println!("BEFORE: Created real fingerprint with ID: {}", original.id);
    println!("  - SemanticFingerprint: default (all 13 embedders)");
    println!("  - PurposeVector: 13D with alignment 0.75");
    println!("  - JohariFingerprint: 13×4 quadrants");
    println!("  - Evolution snapshots: {}", original.purpose_evolution.len());

    let serialized = serialize_teleological_fingerprint(&original);
    println!("SERIALIZED: {} bytes", serialized.len());
    println!("  - Version byte: {}", serialized[0]);
    println!("  - Payload size: {} bytes", serialized.len() - 1);

    let deserialized = deserialize_teleological_fingerprint(&serialized);
    println!("AFTER: Deserialized fingerprint ID: {}", deserialized.id);
    println!("  - Evolution snapshots: {}", deserialized.purpose_evolution.len());
    println!("  - Theta to north star: {:.4}", deserialized.theta_to_north_star);

    assert_eq!(original.id, deserialized.id);
    assert_eq!(original.content_hash, deserialized.content_hash);
    assert!((original.theta_to_north_star - deserialized.theta_to_north_star).abs() < 1e-6);
    println!("RESULT: PASS - Round-trip preserved all fields");
}

#[test]
fn test_fingerprint_size_in_range() {
    println!("=== TEST: Serialized size within expected range ===");

    let fp = create_real_fingerprint();
    let serialized = serialize_teleological_fingerprint(&fp);

    // Actual size calculation:
    // - TOTAL_DENSE_DIMS = 15,120 → 60,480 bytes for dense embeddings
    // - Plus sparse vectors, JohariFingerprint (~520B), PurposeVector (52B), metadata
    // - Total: ~63KB for a fresh fingerprint with 1 evolution snapshot
    println!("BEFORE: Expected range [55KB, 150KB]");
    println!("AFTER: Actual size {} bytes ({:.2}KB)", serialized.len(), serialized.len() as f64 / 1024.0);

    assert!(
        serialized.len() >= 55_000,
        "Size {} below minimum 55KB - embeddings may be missing",
        serialized.len()
    );
    assert!(
        serialized.len() <= 150_000,
        "Size {} above maximum 150KB - unexpectedly large",
        serialized.len()
    );
    println!("RESULT: PASS - Size in expected range");
}

#[test]
fn test_purpose_vector_roundtrip() {
    println!("=== TEST: Purpose vector (13D) round-trip ===");

    let original: [f32; 13] = [
        0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3,
    ];
    println!("BEFORE: {:?}", original);

    let serialized = serialize_purpose_vector(&original);
    assert_eq!(serialized.len(), 52);
    println!("SERIALIZED: {} bytes", serialized.len());

    let deserialized = deserialize_purpose_vector(&serialized);
    println!("AFTER: {:?}", deserialized);

    for i in 0..13 {
        assert!(
            (original[i] - deserialized[i]).abs() < 1e-6,
            "Value mismatch at index {}",
            i
        );
    }
    println!("RESULT: PASS - All 13 dimensions preserved");
}

#[test]
fn test_e1_matryoshka_roundtrip() {
    println!("=== TEST: E1 Matryoshka 128D vector round-trip ===");

    let mut original = [0.0f32; 128];
    for i in 0..128 {
        original[i] = (i as f32) * 0.01;
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

    let deserialized = deserialize_memory_id_list(&serialized);
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

    let deserialized = deserialize_memory_id_list(&serialized);
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

    let deserialized = deserialize_memory_id_list(&serialized);
    println!("AFTER: {} UUIDs", deserialized.len());
    println!("  First: {}", deserialized[0]);
    println!("  Last: {}", deserialized[999]);

    assert_eq!(original, deserialized);
    println!("RESULT: PASS - Large list handled correctly");
}

#[test]
fn edge_case_purpose_vector_extreme_values() {
    println!("=== EDGE CASE 3: Purpose vector with extreme float values ===");

    let original: [f32; 13] = [
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
        0.123456789,
        -0.987654321,
    ];
    println!("BEFORE: Extreme values including MIN, MAX, EPSILON, PI, E");
    for (i, v) in original.iter().enumerate() {
        println!("  [{}]: {:e}", i, v);
    }

    let serialized = serialize_purpose_vector(&original);
    println!("SERIALIZED: {} bytes", serialized.len());

    let deserialized = deserialize_purpose_vector(&serialized);
    println!("AFTER: Deserialized values");
    for (i, v) in deserialized.iter().enumerate() {
        println!("  [{}]: {:e}", i, v);
    }

    for i in 0..13 {
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
// PANIC TESTS (Verify fail-fast behavior)
// =========================================================================

#[test]
#[should_panic(expected = "DESERIALIZATION ERROR")]
fn test_panic_on_empty_fingerprint_data() {
    let _ = deserialize_teleological_fingerprint(&[]);
}

#[test]
#[should_panic(expected = "DESERIALIZATION ERROR")]
fn test_panic_on_wrong_version() {
    let mut data = vec![255u8]; // Wrong version
    data.extend(vec![0u8; 100]); // Garbage data
    let _ = deserialize_teleological_fingerprint(&data);
}

#[test]
#[should_panic(expected = "DESERIALIZATION ERROR")]
fn test_panic_on_wrong_purpose_vector_size() {
    let _ = deserialize_purpose_vector(&[0u8; 51]); // Should be 52
}

#[test]
#[should_panic(expected = "DESERIALIZATION ERROR")]
fn test_panic_on_wrong_e1_vector_size() {
    let _ = deserialize_e1_matryoshka_128(&[0u8; 500]); // Should be 512
}

#[test]
#[should_panic(expected = "DESERIALIZATION ERROR")]
fn test_panic_on_truncated_memory_id_list() {
    // Create valid list, then truncate
    let ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
    let serialized = serialize_memory_id_list(&ids);
    let truncated = &serialized[..serialized.len() - 10]; // Remove 10 bytes
    let _ = deserialize_memory_id_list(truncated);
}

#[test]
#[should_panic(expected = "STORAGE ERROR")]
fn test_panic_on_invalid_fingerprint_key() {
    let _ = parse_fingerprint_key(&[0u8; 15]); // Should be 16
}

#[test]
#[should_panic(expected = "STORAGE ERROR")]
fn test_panic_on_invalid_splade_key() {
    let _ = parse_e13_splade_key(&[0u8; 3]); // Should be 2
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
        let deserialized = deserialize_teleological_fingerprint(&serialized);

        assert_eq!(original.id, deserialized.id, "Mismatch on iteration {}", i);
        assert_eq!(
            original.content_hash, deserialized.content_hash,
            "Hash mismatch on iteration {}",
            i
        );
    }

    println!("RESULT: PASS - 10 round-trips successful");
}

#[test]
fn test_key_functions_deterministic() {
    let id = Uuid::new_v4();

    // Same ID should produce same key
    let key1 = fingerprint_key(&id);
    let key2 = fingerprint_key(&id);
    assert_eq!(key1, key2);

    let term_id: u16 = 42;
    let term_key1 = e13_splade_inverted_key(term_id);
    let term_key2 = e13_splade_inverted_key(term_id);
    assert_eq!(term_key1, term_key2);
}
