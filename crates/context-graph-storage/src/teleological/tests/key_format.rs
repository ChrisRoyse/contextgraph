//! Key encoding/decoding tests.

use crate::teleological::*;
use uuid::Uuid;

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
fn test_topic_profile_key_format() {
    let id = Uuid::new_v4();
    let key = topic_profile_key(&id);

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
// Content Key Tests
// =========================================================================

#[test]
fn test_content_key_format() {
    println!("=== TEST: content_key format (TASK-CONTENT-002) ===");

    let id = Uuid::new_v4();
    let key = schema::content_key(&id);

    println!("UUID: {}", id);
    println!("Key: {:02x?}", key);
    println!("Key length: {} bytes", key.len());

    assert_eq!(key.len(), 16, "Content key must be 16 bytes (UUID)");
    assert_eq!(key, *id.as_bytes(), "Key must equal UUID bytes");
}

#[test]
fn test_content_key_roundtrip() {
    println!("=== TEST: content_key roundtrip ===");

    let test_uuids = vec![Uuid::nil(), Uuid::max(), Uuid::new_v4(), Uuid::new_v4()];

    for id in test_uuids {
        let key = schema::content_key(&id);
        let parsed = schema::parse_content_key(&key);
        assert_eq!(id, parsed, "Round-trip failed for UUID {}", id);
    }

    println!("RESULT: PASS - All content key round-trips successful");
}

// =========================================================================
// Additional Verification Tests
// =========================================================================

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
