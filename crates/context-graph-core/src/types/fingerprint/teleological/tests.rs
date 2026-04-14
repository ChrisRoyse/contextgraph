//! Tests for TeleologicalFingerprint.

use chrono::Utc;
use uuid::Uuid;

use super::test_helpers::{make_test_hash, make_test_semantic};
use super::TeleologicalFingerprint;

// ===== Creation Tests =====

#[test]
fn test_teleological_new() {
    let semantic = make_test_semantic();
    let hash = make_test_hash();

    let before = Utc::now();
    let fp = TeleologicalFingerprint::new(semantic, hash);
    let after = Utc::now();

    // ID is valid UUID
    assert!(!fp.id.is_nil());

    // Timestamps are set
    assert!(fp.created_at >= before && fp.created_at <= after);
    assert!(fp.last_updated >= before && fp.last_updated <= after);

    // Access count starts at 0
    assert_eq!(fp.access_count, 0);

    // Hash is stored
    assert_eq!(fp.content_hash, hash);

    println!("[PASS] TeleologicalFingerprint::new creates valid fingerprint");
    println!("  - ID: {}", fp.id);
    println!("  - Created: {}", fp.created_at);
}

#[test]
fn test_teleological_with_id() {
    let specific_id = Uuid::new_v4();
    let fp = TeleologicalFingerprint::with_id(specific_id, make_test_semantic(), make_test_hash());

    assert_eq!(fp.id, specific_id);

    println!("[PASS] TeleologicalFingerprint::with_id uses provided ID");
}

// ===== Access Recording Tests =====

#[test]
fn test_teleological_record_access() {
    let mut fp = TeleologicalFingerprint::new(make_test_semantic(), make_test_hash());

    assert_eq!(fp.access_count, 0);
    let initial_updated = fp.last_updated;

    // Small delay to ensure timestamp difference (chrono nanosecond resolution)
    std::thread::sleep(std::time::Duration::from_millis(1));

    fp.record_access();
    assert_eq!(fp.access_count, 1);
    assert!(fp.last_updated > initial_updated);

    fp.record_access();
    assert_eq!(fp.access_count, 2);

    println!("[PASS] record_access increments count and updates timestamp");
}

// ===== Constants Tests =====

#[test]
fn test_teleological_constants() {
    assert_eq!(TeleologicalFingerprint::EXPECTED_SIZE_BYTES, 46_000);

    println!("[PASS] Constants match specification");
    println!(
        "  - EXPECTED_SIZE_BYTES: {}",
        TeleologicalFingerprint::EXPECTED_SIZE_BYTES
    );
}

// ===== Serialization Tests =====

#[test]
fn test_teleological_serialization() {
    let fp = TeleologicalFingerprint::new(make_test_semantic(), make_test_hash());

    // Test JSON serialization
    let json = serde_json::to_string(&fp).expect("Serialization should succeed");
    assert!(!json.is_empty());

    // Test deserialization
    let restored: TeleologicalFingerprint =
        serde_json::from_str(&json).expect("Deserialization should succeed");
    assert_eq!(restored.id, fp.id);

    println!("[PASS] TeleologicalFingerprint serializes/deserializes correctly");
}
