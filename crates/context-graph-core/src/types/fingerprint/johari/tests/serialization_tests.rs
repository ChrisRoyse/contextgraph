//! Tests for JohariFingerprint serialization and validation.

use crate::types::fingerprint::johari::JohariFingerprint;
use crate::types::JohariQuadrant;

// ===== Compact Bytes Tests =====

#[test]
fn test_compact_bytes_roundtrip() {
    let mut jf = JohariFingerprint::zeroed();

    // Set various quadrants
    jf.set_quadrant(0, 1.0, 0.0, 0.0, 0.0, 1.0); // Open
    jf.set_quadrant(1, 0.0, 1.0, 0.0, 0.0, 1.0); // Hidden
    jf.set_quadrant(2, 0.0, 0.0, 1.0, 0.0, 1.0); // Blind
    jf.set_quadrant(3, 0.0, 0.0, 0.0, 1.0, 1.0); // Unknown

    let bytes = jf.to_compact_bytes();
    println!("Compact bytes: {:?}", bytes);

    let decoded = JohariFingerprint::from_compact_bytes(bytes);

    // Verify dominant quadrants match
    for i in 0..4 {
        assert_eq!(
            jf.dominant_quadrant(i),
            decoded.dominant_quadrant(i),
            "Embedder {} dominant mismatch",
            i
        );
    }

    println!("[PASS] compact_bytes roundtrip preserves dominant quadrants");
}

#[test]
fn test_compact_bytes_all_quadrants() {
    // Test each quadrant has unique encoding
    let quadrants = [
        JohariQuadrant::Open,
        JohariQuadrant::Hidden,
        JohariQuadrant::Blind,
        JohariQuadrant::Unknown,
    ];

    for (i, q) in quadrants.iter().enumerate() {
        let mut jf = JohariFingerprint::zeroed();
        jf.quadrants[0][i] = 1.0;
        jf.confidence[0] = 1.0;

        let bytes = jf.to_compact_bytes();
        let decoded = JohariFingerprint::from_compact_bytes(bytes);

        assert_eq!(decoded.dominant_quadrant(0), *q);
    }

    println!("[PASS] Each quadrant has unique encoding");
}

// ===== validate() Tests =====

#[test]
fn test_validate_valid() {
    let mut jf = JohariFingerprint::zeroed();
    jf.set_quadrant(0, 0.25, 0.25, 0.25, 0.25, 0.5);

    let result = jf.validate();
    assert!(
        result.is_ok(),
        "Valid fingerprint should pass: {:?}",
        result
    );

    println!("[PASS] validate() returns Ok for valid fingerprint");
}

#[test]
fn test_validate_invalid_sum() {
    let mut jf = JohariFingerprint::zeroed();
    jf.quadrants[0] = [0.1, 0.1, 0.1, 0.1]; // Sum = 0.4, not 1.0

    let result = jf.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("sum"));

    println!("[PASS] validate() catches invalid weight sum");
}

#[test]
fn test_validate_nan() {
    let mut jf = JohariFingerprint::zeroed();
    jf.quadrants[0][0] = f32::NAN;

    let result = jf.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("NaN"));

    println!("[PASS] validate() catches NaN values");
}

#[test]
fn test_validate_confidence_range() {
    let mut jf = JohariFingerprint::zeroed();
    jf.confidence[0] = 1.5; // Out of range

    let result = jf.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("confidence"));

    println!("[PASS] validate() catches confidence out of range");
}
