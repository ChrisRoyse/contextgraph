//! Equality and clone tests for PoincarePoint.

use crate::hyperbolic::PoincarePoint;

// ============================================================================
// EQUALITY TESTS
// ============================================================================

#[test]
fn test_equality_same_coords() {
    let coords = [0.1f32; 64];
    let a = PoincarePoint::from_coords(coords);
    let b = PoincarePoint::from_coords(coords);
    assert_eq!(a, b);
}

#[test]
fn test_inequality_different_coords() {
    let a = PoincarePoint::from_coords([0.1f32; 64]);
    let mut coords = [0.1f32; 64];
    coords[0] = 0.2;
    let b = PoincarePoint::from_coords(coords);
    assert_ne!(a, b);
}

// ============================================================================
// CLONE TESTS
// ============================================================================

#[test]
fn test_clone_independent() {
    let coords = [0.1f32; 64];
    let original = PoincarePoint::from_coords(coords);
    let cloned = original.clone();
    // Verify clone has same values as original
    assert_eq!(cloned.coords[0], 0.1, "Clone must have same values");
    // Verify original is independent - modifying a separate copy doesn't affect original
    let mut modified = cloned;
    modified.coords[0] = 0.9;
    // Use modified to ensure it's read
    assert_eq!(modified.coords[0], 0.9, "Modified value should be 0.9");
    assert_eq!(
        original.coords[0], 0.1,
        "Original must be independent from modifications"
    );
}
