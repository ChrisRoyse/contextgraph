//! Validation and error tests for SynergyMatrix.

use crate::teleological::synergy_matrix::SynergyMatrix;

#[test]
fn test_synergy_matrix_validate() {
    use crate::teleological::comparison_error::ComparisonValidationError;

    // Valid matrix should pass
    let matrix = SynergyMatrix::with_base_synergies();
    assert!(matrix.validate().is_ok(), "Base synergies should be valid");
    assert!(matrix.is_valid(), "is_valid() should return true");

    // All predefined matrices should be valid
    assert!(SynergyMatrix::semantic_focused().validate().is_ok());
    assert!(SynergyMatrix::code_heavy().validate().is_ok());
    assert!(SynergyMatrix::temporal_focused().validate().is_ok());
    assert!(SynergyMatrix::causal_reasoning().validate().is_ok());
    assert!(SynergyMatrix::relational().validate().is_ok());
    assert!(SynergyMatrix::qualitative().validate().is_ok());
    assert!(SynergyMatrix::balanced().validate().is_ok());
    assert!(SynergyMatrix::identity().validate().is_ok());

    // Test invalid matrix: asymmetric
    let mut asymmetric = SynergyMatrix::with_base_synergies();
    asymmetric.values[0][5] = 0.8; // Only change one direction
    let err = asymmetric.validate();
    assert!(err.is_err(), "Asymmetric matrix should fail");
    match err {
        Err(ComparisonValidationError::MatrixNotSymmetric { row, col, .. }) => {
            assert_eq!(row, 0);
            assert_eq!(col, 5);
        }
        _ => panic!("Expected MatrixNotSymmetric error"),
    }

    // Test invalid matrix: bad diagonal
    let mut bad_diag = SynergyMatrix::with_base_synergies();
    bad_diag.values[3][3] = 0.5;
    let err = bad_diag.validate();
    assert!(err.is_err(), "Bad diagonal should fail");
    match err {
        Err(ComparisonValidationError::DiagonalNotUnity { index, actual, .. }) => {
            assert_eq!(index, 3);
            assert!((actual - 0.5).abs() < f32::EPSILON);
        }
        _ => panic!("Expected DiagonalNotUnity error"),
    }

    // Test invalid matrix: out of range
    let mut out_of_range = SynergyMatrix::with_base_synergies();
    out_of_range.values[2][7] = 1.5;
    out_of_range.values[7][2] = 1.5; // Keep symmetric
    let err = out_of_range.validate();
    assert!(err.is_err(), "Out of range value should fail");
    match err {
        Err(ComparisonValidationError::SynergyOutOfRange { value, .. }) => {
            assert_eq!(value, 1.5);
        }
        _ => panic!("Expected SynergyOutOfRange error"),
    }
}

#[test]
#[should_panic(expected = "FAIL FAST")]
fn test_synergy_matrix_flat_to_indices_out_of_bounds() {
    let _ = SynergyMatrix::flat_to_indices(91);
}
