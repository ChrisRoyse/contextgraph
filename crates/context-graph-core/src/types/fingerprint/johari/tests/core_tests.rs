//! Tests for core JohariFingerprint functionality.

use crate::types::fingerprint::johari::JohariFingerprint;
use crate::types::fingerprint::purpose::NUM_EMBEDDERS;
use crate::types::JohariQuadrant;

// ===== zeroed() Tests =====

#[test]
fn test_zeroed() {
    let jf = JohariFingerprint::zeroed();

    // All quadrant weights should be [0,0,0,0]
    for embedder_idx in 0..NUM_EMBEDDERS {
        assert_eq!(
            jf.quadrants[embedder_idx],
            [0.0, 0.0, 0.0, 0.0],
            "Embedder {} quadrants should be all zeros",
            embedder_idx
        );
    }

    // All confidence should be 0.0
    for embedder_idx in 0..NUM_EMBEDDERS {
        assert_eq!(
            jf.confidence[embedder_idx], 0.0,
            "Embedder {} confidence should be 0.0",
            embedder_idx
        );
    }

    // All transition probabilities should be uniform (0.25)
    for embedder_idx in 0..NUM_EMBEDDERS {
        for from_quad in 0..4 {
            for to_quad in 0..4 {
                assert!(
                    (jf.transition_probs[embedder_idx][from_quad][to_quad] - 0.25).abs()
                        < f32::EPSILON,
                    "Transition prob [{}][{}][{}] should be 0.25",
                    embedder_idx,
                    from_quad,
                    to_quad
                );
            }
        }
    }

    println!(
        "[PASS] zeroed() creates valid fingerprint with zero quadrants and uniform transitions"
    );
}

// NOTE: stub() method has been removed as part of backwards compat cleanup
// See: docs2/codestate/sherlockplans/agent7-phase1-cleanup-report.md

// ===== Default and PartialEq Tests =====

#[test]
fn test_default() {
    let jf = JohariFingerprint::default();
    let zeroed = JohariFingerprint::zeroed();

    assert_eq!(jf, zeroed, "Default should equal zeroed");

    println!("[PASS] Default implements zeroed()");
}

#[test]
fn test_partial_eq() {
    let jf1 = JohariFingerprint::zeroed();
    let jf2 = JohariFingerprint::zeroed();

    assert_eq!(jf1, jf2, "Two zeroed fingerprints should be equal");

    let mut jf3 = JohariFingerprint::zeroed();
    jf3.set_quadrant(0, 1.0, 0.0, 0.0, 0.0, 1.0);

    assert_ne!(jf1, jf3, "Different fingerprints should not be equal");

    println!("[PASS] PartialEq works correctly with epsilon tolerance");
}

// ===== Edge Case Tests =====

#[test]
fn test_edge_case_all_zero_weights() {
    let mut jf = JohariFingerprint::zeroed();

    println!("BEFORE set_quadrant: quadrants[0] = {:?}", jf.quadrants[0]);

    jf.set_quadrant(0, 0.0, 0.0, 0.0, 0.0, 0.5);

    println!(
        "AFTER set_quadrant(all zeros): quadrants[0] = {:?}",
        jf.quadrants[0]
    );

    assert!((jf.quadrants[0].iter().sum::<f32>() - 1.0).abs() < 0.001);
    println!("[PASS] All-zero weights normalized to uniform distribution");
}

#[test]
fn test_edge_case_max_embedder_index() {
    let mut jf = JohariFingerprint::zeroed();

    println!("BEFORE: Setting embedder 12 (E13_SPLADE)");

    jf.set_quadrant(12, 1.0, 0.0, 0.0, 0.0, 1.0); // E13 = index 12

    println!("AFTER: quadrants[12] = {:?}", jf.quadrants[12]);

    assert_eq!(jf.dominant_quadrant(12), JohariQuadrant::Open);
    println!("[PASS] Maximum embedder index 12 works correctly");
}

#[test]
#[should_panic(expected = "embedder_idx")]
fn test_edge_case_out_of_bounds_panics() {
    let mut jf = JohariFingerprint::zeroed();

    println!("BEFORE: Attempting to set embedder 13 (out of bounds)");

    // This MUST panic - no silent failure
    jf.set_quadrant(13, 1.0, 0.0, 0.0, 0.0, 1.0);

    // Should never reach here
    println!("ERROR: Did not panic on out-of-bounds index!");
}
