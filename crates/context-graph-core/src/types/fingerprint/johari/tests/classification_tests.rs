//! Tests for JohariFingerprint classification methods.

use crate::types::fingerprint::johari::JohariFingerprint;
use crate::types::fingerprint::purpose::NUM_EMBEDDERS;
use crate::types::JohariQuadrant;

// Real entropy/coherence values from UTL theory
const REAL_ENTROPY_LOW: f32 = 0.3; // Below threshold
const REAL_ENTROPY_HIGH: f32 = 0.7; // Above threshold
const REAL_COHERENCE_LOW: f32 = 0.3;
const REAL_COHERENCE_HIGH: f32 = 0.7;

// ===== classify_quadrant() Tests =====

#[test]
fn test_classify_quadrant_open() {
    // Low entropy, High coherence -> Open
    let q = JohariFingerprint::classify_quadrant(REAL_ENTROPY_LOW, REAL_COHERENCE_HIGH);
    assert_eq!(q, JohariQuadrant::Open);
    println!(
        "[PASS] classify_quadrant({}, {}) returns Open",
        REAL_ENTROPY_LOW, REAL_COHERENCE_HIGH
    );
}

#[test]
fn test_classify_quadrant_hidden() {
    // Low entropy, Low coherence -> Hidden
    let q = JohariFingerprint::classify_quadrant(REAL_ENTROPY_LOW, REAL_COHERENCE_LOW);
    assert_eq!(q, JohariQuadrant::Hidden);
    println!(
        "[PASS] classify_quadrant({}, {}) returns Hidden",
        REAL_ENTROPY_LOW, REAL_COHERENCE_LOW
    );
}

#[test]
fn test_classify_quadrant_blind() {
    // High entropy, Low coherence -> Blind
    let q = JohariFingerprint::classify_quadrant(REAL_ENTROPY_HIGH, REAL_COHERENCE_LOW);
    assert_eq!(q, JohariQuadrant::Blind);
    println!(
        "[PASS] classify_quadrant({}, {}) returns Blind",
        REAL_ENTROPY_HIGH, REAL_COHERENCE_LOW
    );
}

#[test]
fn test_classify_quadrant_unknown() {
    // High entropy, High coherence -> Unknown
    let q = JohariFingerprint::classify_quadrant(REAL_ENTROPY_HIGH, REAL_COHERENCE_HIGH);
    assert_eq!(q, JohariQuadrant::Unknown);
    println!(
        "[PASS] classify_quadrant({}, {}) returns Unknown",
        REAL_ENTROPY_HIGH, REAL_COHERENCE_HIGH
    );
}

#[test]
fn test_classify_quadrant_boundary() {
    // At exactly 0.5, 0.5 - should be deterministic
    let q = JohariFingerprint::classify_quadrant(0.5, 0.5);
    // entropy < 0.5 is false (0.5 is not < 0.5), coherence > 0.5 is false
    // So: (false, false) -> Blind
    assert_eq!(q, JohariQuadrant::Blind);
    println!("[PASS] classify_quadrant(0.5, 0.5) returns deterministic result (Blind)");
}

// ===== set_quadrant() Tests =====

#[test]
fn test_set_quadrant_normalizes() {
    let mut jf = JohariFingerprint::zeroed();

    println!("BEFORE set_quadrant: quadrants[0] = {:?}", jf.quadrants[0]);

    jf.set_quadrant(0, 1.0, 2.0, 3.0, 4.0, 0.8);

    println!(
        "AFTER set_quadrant([1,2,3,4]): quadrants[0] = {:?}",
        jf.quadrants[0]
    );

    let sum: f32 = jf.quadrants[0].iter().sum();
    assert!((sum - 1.0).abs() < 0.001, "Sum should be 1.0, got {}", sum);

    // Expected: 1/10, 2/10, 3/10, 4/10
    assert!((jf.quadrants[0][0] - 0.1).abs() < 0.001);
    assert!((jf.quadrants[0][1] - 0.2).abs() < 0.001);
    assert!((jf.quadrants[0][2] - 0.3).abs() < 0.001);
    assert!((jf.quadrants[0][3] - 0.4).abs() < 0.001);

    assert_eq!(jf.confidence[0], 0.8);

    println!("[PASS] set_quadrant normalizes weights to sum=1.0");
}

#[test]
fn test_set_quadrant_all_zero() {
    let mut jf = JohariFingerprint::zeroed();

    println!("BEFORE set_quadrant: quadrants[0] = {:?}", jf.quadrants[0]);

    jf.set_quadrant(0, 0.0, 0.0, 0.0, 0.0, 0.5);

    println!(
        "AFTER set_quadrant(all zeros): quadrants[0] = {:?}",
        jf.quadrants[0]
    );

    // All zero should become uniform [0.25, 0.25, 0.25, 0.25]
    for weight in jf.quadrants[0].iter() {
        assert!(
            (weight - 0.25).abs() < 0.001,
            "Weight should be 0.25, got {}",
            weight
        );
    }

    let sum: f32 = jf.quadrants[0].iter().sum();
    assert!((sum - 1.0).abs() < 0.001);

    println!("[PASS] All-zero weights normalized to uniform distribution");
}

#[test]
#[should_panic(expected = "negative")]
fn test_set_quadrant_negative_panics() {
    let mut jf = JohariFingerprint::zeroed();

    println!("BEFORE: Attempting to set negative weight");

    // This MUST panic - no silent failure
    jf.set_quadrant(0, -1.0, 0.0, 0.0, 0.0, 0.5);

    println!("ERROR: Did not panic on negative weight!");
}

#[test]
#[should_panic(expected = "embedder_idx")]
fn test_set_quadrant_out_of_bounds_panics() {
    let mut jf = JohariFingerprint::zeroed();

    println!("BEFORE: Attempting to set embedder 13 (out of bounds)");

    // This MUST panic
    jf.set_quadrant(13, 1.0, 0.0, 0.0, 0.0, 1.0);

    println!("ERROR: Did not panic on out-of-bounds index!");
}

// ===== dominant_quadrant() Tests =====

#[test]
fn test_dominant_quadrant() {
    let mut jf = JohariFingerprint::zeroed();
    jf.set_quadrant(0, 0.5, 0.3, 0.1, 0.1, 1.0);

    let dom = jf.dominant_quadrant(0);
    assert_eq!(dom, JohariQuadrant::Open);

    println!("[PASS] dominant_quadrant returns highest weight quadrant");
}

#[test]
fn test_dominant_quadrant_tie() {
    let mut jf = JohariFingerprint::zeroed();
    // Set equal weights for Open and Hidden
    jf.quadrants[0] = [0.5, 0.5, 0.0, 0.0];

    let dom = jf.dominant_quadrant(0);
    // First wins in tie
    assert_eq!(dom, JohariQuadrant::Open);

    println!("[PASS] dominant_quadrant returns first quadrant in tie");
}

#[test]
fn test_dominant_quadrant_zeroed_returns_unknown() {
    let jf = JohariFingerprint::zeroed();

    // Zeroed embedders (all weights = 0) should return Unknown (frontier state)
    for i in 0..NUM_EMBEDDERS {
        assert_eq!(
            jf.dominant_quadrant(i),
            JohariQuadrant::Unknown,
            "Zeroed embedder {} should have Unknown as dominant",
            i
        );
    }

    println!("[PASS] dominant_quadrant returns Unknown for zeroed embedders");
}

#[test]
#[should_panic(expected = "embedder_idx")]
fn test_dominant_quadrant_out_of_bounds() {
    let jf = JohariFingerprint::zeroed();
    let _ = jf.dominant_quadrant(13);
}

// ===== find_by_quadrant() Tests =====

#[test]
fn test_find_by_quadrant() {
    let mut jf = JohariFingerprint::zeroed();

    // Set E1 (index 0) to Open
    jf.set_quadrant(0, 1.0, 0.0, 0.0, 0.0, 1.0);
    // Set E5 (index 4) to Blind
    jf.set_quadrant(4, 0.0, 0.0, 1.0, 0.0, 1.0);
    // Set remaining embedders to Hidden (so they don't default to Unknown)
    for i in [1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12] {
        jf.set_quadrant(i, 0.0, 1.0, 0.0, 0.0, 1.0); // Hidden
    }

    let open_embedders = jf.find_by_quadrant(JohariQuadrant::Open);
    assert_eq!(open_embedders, vec![0]);

    let blind_embedders = jf.find_by_quadrant(JohariQuadrant::Blind);
    assert_eq!(blind_embedders, vec![4]);

    // Verify the others are Hidden
    let hidden_embedders = jf.find_by_quadrant(JohariQuadrant::Hidden);
    assert_eq!(hidden_embedders.len(), 11); // All except 0 and 4

    println!("[PASS] find_by_quadrant returns correct embedder indices");
}
