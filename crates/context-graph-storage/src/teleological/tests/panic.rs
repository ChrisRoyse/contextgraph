//! Error behavior tests (formerly panic tests, now Result-returning).
//!
//! STG-01: Deserialization functions now return Result instead of panicking.

use crate::teleological::*;
use uuid::Uuid;

// =========================================================================
// DESERIALIZATION ERROR TESTS (Verify Result-based error handling)
// =========================================================================

#[test]
fn test_error_on_empty_fingerprint_data() {
    let result = deserialize_teleological_fingerprint(&[]);
    assert!(result.is_err(), "Expected Err for empty fingerprint data");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Empty data"),
        "Error message should mention empty data, got: {}",
        err_msg
    );
}

#[test]
fn test_error_on_wrong_version() {
    let mut data = vec![255u8]; // Wrong version
    data.extend(vec![0u8; 100]); // Garbage data
    let result = deserialize_teleological_fingerprint(&data);
    assert!(result.is_err(), "Expected Err for wrong version");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Version mismatch"),
        "Error message should mention version mismatch, got: {}",
        err_msg
    );
}

#[test]
fn test_error_on_wrong_topic_profile_size() {
    let result = deserialize_topic_profile(&[0u8; 55]); // Should be 56 post-E14
    assert!(result.is_err(), "Expected Err for wrong topic profile size");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("56 bytes"),
        "Error message should mention 56 bytes, got: {}",
        err_msg
    );
}

#[test]
#[should_panic(expected = "DESERIALIZATION ERROR")]
fn test_panic_on_wrong_e1_vector_size() {
    let _ = deserialize_e1_matryoshka_128(&[0u8; 500]); // Should be 512
}

#[test]
fn test_error_on_truncated_memory_id_list() {
    // Create valid list, then truncate
    let ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
    let serialized = serialize_memory_id_list(&ids);
    let truncated = &serialized[..serialized.len() - 10]; // Remove 10 bytes
    let result = deserialize_memory_id_list(truncated);
    assert!(result.is_err(), "Expected Err for truncated memory ID list");
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
// Content Key Panic Tests
// =========================================================================

#[test]
#[should_panic(expected = "STORAGE ERROR")]
fn test_content_key_parse_invalid_length_panics() {
    let invalid_key = [0u8; 8]; // Only 8 bytes, should be 16
    schema::parse_content_key(&invalid_key);
}
