//! Comprehensive validation test for dimension system.
//!
//! Master validation test that runs all critical checks in one place.

use context_graph_embeddings::dimensions::{MODEL_COUNT, TOTAL_DIMENSION};
use context_graph_embeddings::{ModelId, QuantizationMethod};

use super::constants::{EXPECTED_NATIVE_DIMS, EXPECTED_PROJECTED_DIMS, EXPECTED_QUANTIZATION};

/// Master validation test - runs all critical checks in one place.
/// FAIL FAST on any Constitution violation.
#[test]
fn test_comprehensive_dimension_validation() {
    println!("=== COMPREHENSIVE DIMENSION VALIDATION ===\n");

    // 1. Verify MODEL_COUNT
    assert_eq!(MODEL_COUNT, 13, "MODEL_COUNT must be 13");
    println!("[1/6] MODEL_COUNT = 13");

    // 2. Verify TOTAL_DIMENSION
    assert_eq!(TOTAL_DIMENSION, 11264, "TOTAL_DIMENSION must be 11264");
    println!("[2/6] TOTAL_DIMENSION = 11264");

    // 3. Verify all native dimensions
    for (model_id, expected) in &EXPECTED_NATIVE_DIMS {
        let actual = model_id.dimension();
        assert_eq!(
            actual, *expected,
            "{:?} native dimension mismatch",
            model_id
        );
    }
    println!("[3/6] All 13 native dimensions verified");

    // 4. Verify all projected dimensions
    for (model_id, expected) in &EXPECTED_PROJECTED_DIMS {
        let actual = model_id.projected_dimension();
        assert_eq!(
            actual, *expected,
            "{:?} projected dimension mismatch",
            model_id
        );
    }
    println!("[4/6] All 13 projected dimensions verified");

    // 5. Verify all quantization methods
    for (model_id, expected) in &EXPECTED_QUANTIZATION {
        let actual = QuantizationMethod::for_model_id(*model_id);
        assert_eq!(
            actual, *expected,
            "{:?} quantization method mismatch",
            model_id
        );
    }
    println!("[5/6] All 13 quantization methods verified");

    // 6. Verify sum consistency
    let sum: usize = ModelId::all().iter().map(|m| m.projected_dimension()).sum();
    assert_eq!(sum, TOTAL_DIMENSION, "Sum mismatch with TOTAL_DIMENSION");
    println!("[6/6] Sum of projected dimensions = TOTAL_DIMENSION");

    println!("\n=== ALL VALIDATIONS PASSED ===");
    println!("  - 13 ModelId variants verified");
    println!("  - 13 native dimensions verified");
    println!("  - 13 projected dimensions verified");
    println!("  - 13 quantization methods verified");
    println!("  - TOTAL_DIMENSION = 11264 verified");
}
