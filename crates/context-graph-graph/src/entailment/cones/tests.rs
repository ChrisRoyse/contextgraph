//! Tests for EntailmentCone.
//!
//! MUST USE REAL DATA, NO MOCKS (per constitution REQ-KG-TEST)

#![allow(clippy::field_reassign_with_default)]

use super::*;
use crate::config::ConeConfig;
use crate::hyperbolic::poincare::PoincarePoint;

// ========== CONSTRUCTION TESTS ==========

/// Test creation with default ConeConfig
#[test]
fn test_new_with_default_config() {
    let config = ConeConfig::default();
    let apex = PoincarePoint::origin();
    let cone = EntailmentCone::new(apex, 0, &config).expect("Should create valid cone");

    assert!(cone.is_valid());
    assert_eq!(cone.depth, 0);
    assert_eq!(cone.aperture, config.base_aperture);
    assert_eq!(cone.aperture_factor, 1.0);
}

/// Test aperture decay with depth
#[test]
fn test_aperture_decay_with_depth() {
    let config = ConeConfig::default();
    let apex = PoincarePoint::origin();

    let cone_d0 = EntailmentCone::new(apex.clone(), 0, &config).expect("valid");
    let cone_d1 = EntailmentCone::new(apex.clone(), 1, &config).expect("valid");
    let cone_d5 = EntailmentCone::new(apex.clone(), 5, &config).expect("valid");

    // Deeper = narrower aperture
    assert!(
        cone_d0.aperture > cone_d1.aperture,
        "depth 0 should have wider aperture than depth 1"
    );
    assert!(
        cone_d1.aperture > cone_d5.aperture,
        "depth 1 should have wider aperture than depth 5"
    );

    // Verify formula: base * decay^depth
    let expected_d1 = config.base_aperture * config.aperture_decay;
    assert!(
        (cone_d1.aperture - expected_d1).abs() < 1e-6,
        "Expected {}, got {}",
        expected_d1,
        cone_d1.aperture
    );
}

/// Test very deep node clamps to min_aperture
#[test]
fn test_deep_node_min_aperture() {
    let config = ConeConfig::default();
    let apex = PoincarePoint::origin();
    let cone = EntailmentCone::new(apex, 100, &config).expect("valid");

    assert_eq!(
        cone.aperture, config.min_aperture,
        "Very deep node should clamp to min_aperture"
    );
}

/// Test with_aperture constructor
#[test]
fn test_with_aperture_constructor() {
    let apex = PoincarePoint::origin();
    let cone = EntailmentCone::with_aperture(apex, 0.5, 3).expect("valid");

    assert_eq!(cone.aperture, 0.5);
    assert_eq!(cone.depth, 3);
    assert_eq!(cone.aperture_factor, 1.0);
    assert!(cone.is_valid());
}

// ========== EFFECTIVE APERTURE TESTS ==========

/// Test effective_aperture with factor
#[test]
fn test_effective_aperture() {
    let mut cone = EntailmentCone::default();
    cone.aperture = 0.5;
    cone.aperture_factor = 1.5;

    let effective = cone.effective_aperture();
    assert!(
        (effective - 0.75).abs() < 1e-6,
        "Expected 0.75, got {}",
        effective
    );
}

/// Test effective_aperture clamping to pi/2
#[test]
fn test_effective_aperture_clamp_max() {
    let mut cone = EntailmentCone::default();
    cone.aperture = 1.5;
    cone.aperture_factor = 2.0;

    let effective = cone.effective_aperture();
    assert!(
        (effective - std::f32::consts::FRAC_PI_2).abs() < 1e-6,
        "Should clamp to π/2, got {}",
        effective
    );
}

/// Test effective_aperture clamping to minimum
#[test]
fn test_effective_aperture_clamp_min() {
    let mut cone = EntailmentCone::default();
    cone.aperture = 1e-8;
    cone.aperture_factor = 0.5;

    let effective = cone.effective_aperture();
    assert!(
        effective >= 1e-6,
        "Should clamp to minimum epsilon, got {}",
        effective
    );
}

// ========== VALIDATION TESTS ==========

/// Test invalid apex rejection (FAIL FAST)
#[test]
fn test_invalid_apex_fails_fast() {
    use crate::error::GraphError;

    let config = ConeConfig::default();
    // Create point with norm >= 1
    let mut coords = [0.0f32; 64];
    coords[0] = 1.0; // norm = 1.0, invalid for Poincare ball
    let invalid_apex = PoincarePoint::from_coords(coords);

    let result = EntailmentCone::new(invalid_apex, 0, &config);
    assert!(result.is_err(), "Should reject invalid apex");

    match result {
        Err(GraphError::InvalidHyperbolicPoint { norm }) => {
            assert!(
                (norm - 1.0).abs() < 1e-6,
                "Should report correct norm in error"
            );
        }
        _ => panic!("Expected InvalidHyperbolicPoint error"),
    }
}

/// Test with_aperture validation - zero aperture
#[test]
fn test_with_aperture_zero_fails() {
    use crate::error::GraphError;

    let apex = PoincarePoint::origin();
    let result = EntailmentCone::with_aperture(apex, 0.0, 0);
    assert!(matches!(result, Err(GraphError::InvalidAperture(_))));
}

/// Test with_aperture validation - negative aperture
#[test]
fn test_with_aperture_negative_fails() {
    use crate::error::GraphError;

    let apex = PoincarePoint::origin();
    let result = EntailmentCone::with_aperture(apex, -0.5, 0);
    assert!(matches!(result, Err(GraphError::InvalidAperture(_))));
}

/// Test with_aperture validation - aperture > π/2
#[test]
fn test_with_aperture_exceeds_max_fails() {
    use crate::error::GraphError;

    let apex = PoincarePoint::origin();
    let result = EntailmentCone::with_aperture(apex, 2.0, 0);
    assert!(matches!(result, Err(GraphError::InvalidAperture(_))));
}

/// Test validate() returns specific errors
#[test]
fn test_validate_specific_errors() {
    use crate::error::GraphError;

    let mut cone = EntailmentCone::default();
    assert!(cone.validate().is_ok());

    // Invalid aperture_factor below 0.5
    cone.aperture_factor = 0.3;
    let result = cone.validate();
    assert!(matches!(result, Err(GraphError::InvalidConfig(_))));

    // Invalid aperture_factor above 2.0
    cone.aperture_factor = 2.5;
    let result = cone.validate();
    assert!(matches!(result, Err(GraphError::InvalidConfig(_))));
}

/// Test is_valid returns false for invalid configurations
#[test]
fn test_is_valid_invalid_configurations() {
    let mut cone = EntailmentCone::default();
    assert!(cone.is_valid());

    // Invalid aperture
    cone.aperture = 0.0;
    assert!(!cone.is_valid(), "Zero aperture should be invalid");

    cone.aperture = 1.0;
    cone.aperture_factor = 0.4;
    assert!(
        !cone.is_valid(),
        "aperture_factor below 0.5 should be invalid"
    );

    cone.aperture_factor = 2.1;
    assert!(
        !cone.is_valid(),
        "aperture_factor above 2.0 should be invalid"
    );
}

// ========== SERIALIZATION TESTS ==========

/// Test serialization roundtrip preserves all fields
#[test]
fn test_serde_roundtrip() {
    let config = ConeConfig::default();
    let mut coords = [0.0f32; 64];
    coords[0] = 0.5;
    coords[1] = 0.3;
    let apex = PoincarePoint::from_coords(coords);

    let original = EntailmentCone::new(apex, 5, &config).expect("valid");

    let serialized = bincode::serialize(&original).expect("Serialization failed");
    let deserialized: EntailmentCone =
        bincode::deserialize(&serialized).expect("Deserialization failed");

    assert_eq!(original.depth, deserialized.depth);
    assert!((original.aperture - deserialized.aperture).abs() < 1e-6);
    assert!((original.aperture_factor - deserialized.aperture_factor).abs() < 1e-6);
    assert!(deserialized.is_valid());
}

/// Test serialized size is approximately 268 bytes
#[test]
fn test_serialized_size() {
    let cone = EntailmentCone::default();
    let serialized = bincode::serialize(&cone).expect("Serialization failed");

    // 256 (coords) + 4 (aperture) + 4 (factor) + 4 (depth) = 268
    // bincode may add small overhead
    assert!(
        serialized.len() >= 268,
        "Serialized size should be at least 268 bytes, got {}",
        serialized.len()
    );
    assert!(
        serialized.len() <= 280,
        "Serialized size should not exceed 280 bytes, got {}",
        serialized.len()
    );
}

// ========== DEFAULT TESTS ==========

/// Test Default creates valid cone
#[test]
fn test_default_is_valid() {
    let cone = EntailmentCone::default();
    assert!(cone.is_valid());
    assert!(cone.validate().is_ok());
    assert_eq!(cone.depth, 0);
    assert_eq!(cone.aperture, 1.0);
    assert_eq!(cone.aperture_factor, 1.0);
}

/// Test default apex is at origin
#[test]
fn test_default_apex_is_origin() {
    let cone = EntailmentCone::default();
    assert_eq!(cone.apex.norm(), 0.0);
}

// ========== EDGE CASE TESTS ==========

/// Test apex at origin edge case
#[test]
fn test_apex_at_origin() {
    let config = ConeConfig::default();
    let origin = PoincarePoint::origin();

    // Should create valid cone (degenerate but allowed)
    let cone = EntailmentCone::new(origin, 0, &config).expect("valid");
    assert!(cone.is_valid());
    assert_eq!(cone.apex.norm(), 0.0);
}

/// Test apex near boundary
#[test]
fn test_apex_near_boundary() {
    let config = ConeConfig::default();
    // Create point with norm ≈ 0.99 (near but inside boundary)
    let scale = 0.99 / (64.0_f32).sqrt();
    let coords = [scale; 64];
    let apex = PoincarePoint::from_coords(coords);

    assert!(apex.is_valid(), "Apex should be valid");
    let cone = EntailmentCone::new(apex, 0, &config).expect("valid");
    assert!(cone.is_valid());
}

/// Test aperture at maximum (π/2)
#[test]
fn test_aperture_at_max() {
    let apex = PoincarePoint::origin();
    let cone = EntailmentCone::with_aperture(apex, std::f32::consts::FRAC_PI_2, 0).expect("valid");
    assert!(cone.is_valid());
    assert_eq!(cone.aperture, std::f32::consts::FRAC_PI_2);
}

/// Test aperture_factor at bounds
#[test]
fn test_aperture_factor_at_bounds() {
    let mut cone = EntailmentCone::default();

    // At lower bound
    cone.aperture_factor = 0.5;
    assert!(cone.is_valid());
    assert!(cone.validate().is_ok());

    // At upper bound
    cone.aperture_factor = 2.0;
    assert!(cone.is_valid());
    assert!(cone.validate().is_ok());
}

/// Integration test with real ConeConfig values
#[test]
fn test_real_config_integration() {
    // Use actual default values from config.rs
    let config = ConeConfig {
        min_aperture: 0.1,
        max_aperture: 1.5,
        base_aperture: 1.0,
        aperture_decay: 0.85,
        membership_threshold: 0.7,
    };

    let apex = PoincarePoint::origin();

    // Test multiple depths
    for depth in [0, 1, 5, 10, 20] {
        let cone = EntailmentCone::new(apex.clone(), depth, &config).expect("valid");
        assert!(cone.is_valid(), "Cone at depth {} should be valid", depth);
        assert!(
            cone.aperture >= config.min_aperture,
            "Aperture at depth {} should be >= min_aperture",
            depth
        );
        assert!(
            cone.aperture <= config.max_aperture,
            "Aperture at depth {} should be <= max_aperture",
            depth
        );
    }
}

/// Test Clone trait
#[test]
fn test_clone_is_independent() {
    let config = ConeConfig::default();
    let mut coords = [0.0f32; 64];
    coords[0] = 0.3;
    let apex = PoincarePoint::from_coords(coords);

    let original = EntailmentCone::new(apex, 5, &config).expect("valid");
    let cloned = original.clone();

    // Clone should have identical values
    assert_eq!(original.aperture, cloned.aperture);
    assert_eq!(original.depth, cloned.depth);
    assert_eq!(original.aperture_factor, cloned.aperture_factor);

    // Verify expected aperture: 1.0 * 0.85^5 = 0.4437
    let expected_aperture = 1.0 * 0.85_f32.powi(5);
    assert!(
        (original.aperture - expected_aperture).abs() < 0.01,
        "Expected {}, got {}",
        expected_aperture,
        original.aperture
    );
    assert_eq!(original.depth, 5);
}

/// Test Debug trait
#[test]
fn test_debug_output() {
    let cone = EntailmentCone::default();
    let debug_str = format!("{:?}", cone);
    assert!(debug_str.contains("EntailmentCone"));
    assert!(debug_str.contains("apex"));
    assert!(debug_str.contains("aperture"));
    assert!(debug_str.contains("aperture_factor"));
    assert!(debug_str.contains("depth"));
}

/// Test with various valid apex positions
#[test]
fn test_various_apex_positions() {
    let config = ConeConfig::default();

    // Near origin
    let mut coords1 = [0.0f32; 64];
    coords1[0] = 0.01;
    let apex1 = PoincarePoint::from_coords(coords1);
    assert!(EntailmentCone::new(apex1, 0, &config).is_ok());

    // Medium distance
    let mut coords2 = [0.0f32; 64];
    coords2[0] = 0.5;
    coords2[1] = 0.3;
    let apex2 = PoincarePoint::from_coords(coords2);
    assert!(EntailmentCone::new(apex2, 0, &config).is_ok());

    // Near boundary
    let mut coords3 = [0.0f32; 64];
    coords3[0] = 0.9;
    let apex3 = PoincarePoint::from_coords(coords3);
    assert!(EntailmentCone::new(apex3, 0, &config).is_ok());
}

/// Test depth affects aperture monotonically
#[test]
fn test_depth_aperture_monotonicity() {
    let config = ConeConfig::default();
    let apex = PoincarePoint::origin();

    let mut prev_aperture = f32::INFINITY;
    for depth in 0..20 {
        let cone = EntailmentCone::new(apex.clone(), depth, &config).expect("valid");
        assert!(
            cone.aperture <= prev_aperture,
            "Aperture should decrease or stay same with depth"
        );
        prev_aperture = cone.aperture;
    }
}

/// Test PartialEq for PoincarePoint in apex
#[test]
fn test_apex_equality() {
    let config = ConeConfig::default();
    let apex = PoincarePoint::origin();

    let cone1 = EntailmentCone::new(apex.clone(), 0, &config).expect("valid");
    let cone2 = EntailmentCone::new(apex, 0, &config).expect("valid");

    assert_eq!(cone1.apex, cone2.apex);
    assert_eq!(cone1.aperture, cone2.aperture);
    assert_eq!(cone1.depth, cone2.depth);
}

/// Test NaN handling in apex
#[test]
fn test_nan_apex_rejected() {
    let config = ConeConfig::default();
    let mut coords = [0.0f32; 64];
    coords[0] = f32::NAN;
    let apex = PoincarePoint::from_coords(coords);

    // NaN norm < 1.0 is false, so should be rejected
    let result = EntailmentCone::new(apex, 0, &config);
    assert!(result.is_err(), "NaN apex should be rejected");
}

/// Test infinity handling in apex
#[test]
fn test_infinity_apex_rejected() {
    let config = ConeConfig::default();
    let mut coords = [0.0f32; 64];
    coords[0] = f32::INFINITY;
    let apex = PoincarePoint::from_coords(coords);

    let result = EntailmentCone::new(apex, 0, &config);
    assert!(result.is_err(), "Infinity apex should be rejected");
}
