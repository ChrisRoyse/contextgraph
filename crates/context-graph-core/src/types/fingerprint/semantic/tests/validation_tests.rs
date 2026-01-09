//! Validation tests for SemanticFingerprint.

use crate::types::fingerprint::semantic::*;
use crate::types::fingerprint::SparseVector;

#[test]
fn test_semantic_fingerprint_validate() {
    let fp = SemanticFingerprint::zeroed();
    assert!(fp.validate().is_ok());

    let mut fp2 = SemanticFingerprint::zeroed();
    fp2.e6_sparse =
        SparseVector::new(vec![100, 200, 30521], vec![0.1, 0.2, 0.3]).expect("valid sparse vector");
    assert!(fp2.validate().is_ok());

    let mut fp3 = SemanticFingerprint::zeroed();
    fp3.e12_late_interaction = vec![vec![0.0; E12_TOKEN_DIM]; 10];
    assert!(fp3.validate().is_ok());

    let mut fp4 = SemanticFingerprint::zeroed();
    fp4.e13_splade =
        SparseVector::new(vec![100, 200, 30521], vec![0.1, 0.2, 0.3]).expect("valid sparse vector");
    assert!(fp4.validate().is_ok());
}

#[test]
fn test_semantic_fingerprint_validate_dimension_errors() {
    let mut fp = SemanticFingerprint::zeroed();
    fp.e1_semantic = vec![0.0; 100];
    let err = fp.validate().unwrap_err();
    assert!(err.contains("E1 semantic dimension mismatch"));

    let mut fp2 = SemanticFingerprint::zeroed();
    fp2.e9_hdc = vec![0.0; 1000];
    let err2 = fp2.validate().unwrap_err();
    assert!(err2.contains("E9 hdc dimension mismatch"));

    let mut fp3 = SemanticFingerprint::zeroed();
    fp3.e12_late_interaction = vec![vec![0.0; 64]];
    let err3 = fp3.validate().unwrap_err();
    assert!(err3.contains("E12 late_interaction token 0 dimension mismatch"));
}
