//! NaN/Infinity Prevention and Validation Tests
//!
//! Tests for parameter validation and error handling

use context_graph_utl::{
    compute_learning_magnitude, compute_learning_magnitude_validated, UtlError,
};

// =============================================================================
// NaN/INFINITY PREVENTION TESTS
// =============================================================================

#[test]
fn test_nan_infinity_prevention_edge_cases() {
    // Empty/zero inputs should not produce NaN/Infinity
    let result = compute_learning_magnitude_validated(0.0, 0.0, 1.0, 0.0);
    assert!(result.is_ok(), "zero inputs should not error");
    let value = result.unwrap();
    assert!(!value.is_nan(), "zero inputs should not produce NaN");
    assert!(
        !value.is_infinite(),
        "zero inputs should not produce Infinity"
    );

    // Boundary values
    for &val in &[0.0_f32, 0.5, 1.0] {
        let result = compute_learning_magnitude(val, val, 1.0, val.min(std::f32::consts::PI));
        assert!(!result.is_nan(), "NaN detected for input {}", val);
        assert!(!result.is_infinite(), "Infinity detected for input {}", val);
    }
}

#[test]
fn test_error_handling_out_of_bounds_delta_s() {
    // delta_s negative
    let result = compute_learning_magnitude_validated(-0.1, 0.5, 1.0, 0.5);
    assert!(result.is_err(), "negative delta_s must fail");
    match result.unwrap_err() {
        UtlError::InvalidParameter { name, .. } => assert_eq!(name, "delta_s"),
        e => panic!("Expected InvalidParameter error for delta_s, got {:?}", e),
    }

    // delta_s > 1
    let result = compute_learning_magnitude_validated(1.1, 0.5, 1.0, 0.5);
    assert!(result.is_err(), "delta_s > 1 must fail");
    match result.unwrap_err() {
        UtlError::InvalidParameter { name, .. } => assert_eq!(name, "delta_s"),
        e => panic!("Expected InvalidParameter error for delta_s, got {:?}", e),
    }
}

#[test]
fn test_error_handling_out_of_bounds_delta_c() {
    // delta_c negative
    let result = compute_learning_magnitude_validated(0.5, -0.1, 1.0, 0.5);
    assert!(result.is_err(), "negative delta_c must fail");
    match result.unwrap_err() {
        UtlError::InvalidParameter { name, .. } => assert_eq!(name, "delta_c"),
        e => panic!("Expected InvalidParameter error for delta_c, got {:?}", e),
    }

    // delta_c > 1
    let result = compute_learning_magnitude_validated(0.5, 1.1, 1.0, 0.5);
    assert!(result.is_err(), "delta_c > 1 must fail");
}

#[test]
fn test_error_handling_out_of_bounds_w_e() {
    // w_e < 0.5
    let result = compute_learning_magnitude_validated(0.5, 0.5, 0.4, 0.5);
    assert!(result.is_err(), "w_e < 0.5 must fail");
    match result.unwrap_err() {
        UtlError::InvalidParameter { name, .. } => assert_eq!(name, "w_e"),
        e => panic!("Expected InvalidParameter error for w_e, got {:?}", e),
    }

    // w_e > 1.5
    let result = compute_learning_magnitude_validated(0.5, 0.5, 1.6, 0.5);
    assert!(result.is_err(), "w_e > 1.5 must fail");
}

#[test]
fn test_error_handling_out_of_bounds_phi() {
    // phi negative
    let result = compute_learning_magnitude_validated(0.5, 0.5, 1.0, -0.1);
    assert!(result.is_err(), "negative phi must fail");
    match result.unwrap_err() {
        UtlError::InvalidParameter { name, .. } => assert_eq!(name, "phi"),
        e => panic!("Expected InvalidParameter error for phi, got {:?}", e),
    }

    // phi > π
    let result = compute_learning_magnitude_validated(0.5, 0.5, 1.0, 4.0);
    assert!(result.is_err(), "phi > π must fail");
}
