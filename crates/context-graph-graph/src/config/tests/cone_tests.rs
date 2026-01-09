//! Tests for ConeConfig.

use crate::config::ConeConfig;

#[test]
fn test_cone_config_default_values() {
    let config = ConeConfig::default();

    // Verify all 5 fields match spec
    assert_eq!(config.min_aperture, 0.1, "min_aperture must be 0.1");
    assert_eq!(config.max_aperture, 1.5, "max_aperture must be 1.5");
    assert_eq!(config.base_aperture, 1.0, "base_aperture must be 1.0");
    assert_eq!(config.aperture_decay, 0.85, "aperture_decay must be 0.85");
    assert_eq!(
        config.membership_threshold, 0.7,
        "membership_threshold must be 0.7"
    );
}

#[test]
fn test_cone_config_field_constraints() {
    let config = ConeConfig::default();

    // Verify logical relationships
    assert!(config.min_aperture > 0.0, "min_aperture must be positive");
    assert!(config.max_aperture > config.min_aperture, "max > min");
    assert!(config.base_aperture >= config.min_aperture, "base >= min");
    assert!(config.base_aperture <= config.max_aperture, "base <= max");
    assert!(
        config.aperture_decay > 0.0 && config.aperture_decay < 1.0,
        "decay in (0,1)"
    );
    assert!(
        config.membership_threshold > 0.0 && config.membership_threshold < 1.0,
        "threshold in (0,1)"
    );
}

#[test]
fn test_compute_aperture_depth_zero() {
    let config = ConeConfig::default();
    // depth=0: base_aperture * 0.85^0 = 1.0 * 1 = 1.0
    assert_eq!(config.compute_aperture(0), 1.0);
}

#[test]
fn test_compute_aperture_depth_one() {
    let config = ConeConfig::default();
    // depth=1: 1.0 * 0.85^1 = 0.85
    let result = config.compute_aperture(1);
    assert!(
        (result - 0.85).abs() < 1e-6,
        "Expected 0.85, got {}",
        result
    );
}

#[test]
fn test_compute_aperture_depth_two() {
    let config = ConeConfig::default();
    // depth=2: 1.0 * 0.85^2 = 0.7225
    let result = config.compute_aperture(2);
    assert!(
        (result - 0.7225).abs() < 1e-6,
        "Expected 0.7225, got {}",
        result
    );
}

#[test]
fn test_compute_aperture_clamps_to_min() {
    let config = ConeConfig::default();
    // Very deep: should clamp to min_aperture = 0.1
    // 1.0 * 0.85^100 ~ 3.6e-8, clamped to 0.1
    assert_eq!(config.compute_aperture(100), 0.1);
}

#[test]
fn test_compute_aperture_clamps_to_max() {
    // Config where base > max (shouldn't happen in practice but test clamping)
    let config = ConeConfig {
        min_aperture: 0.1,
        max_aperture: 0.5,
        base_aperture: 1.0, // Exceeds max
        aperture_decay: 0.85,
        membership_threshold: 0.7,
    };
    // depth=0: raw=1.0, clamped to max=0.5
    assert_eq!(config.compute_aperture(0), 0.5);
}

#[test]
fn test_cone_validate_default_passes() {
    let config = ConeConfig::default();
    assert!(config.validate().is_ok(), "Default config must be valid");
}

#[test]
fn test_cone_validate_min_aperture_zero_fails() {
    let config = ConeConfig {
        min_aperture: 0.0,
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("min_aperture"));
}

#[test]
fn test_cone_validate_min_aperture_negative_fails() {
    let config = ConeConfig {
        min_aperture: -0.1,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_cone_validate_max_less_than_min_fails() {
    let config = ConeConfig {
        min_aperture: 1.0,
        max_aperture: 0.5, // Less than min
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("max_aperture"));
}

#[test]
fn test_cone_validate_max_equals_min_fails() {
    let config = ConeConfig {
        min_aperture: 0.5,
        max_aperture: 0.5, // Equal, should fail (must be greater)
        base_aperture: 0.5,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_cone_validate_decay_zero_fails() {
    let config = ConeConfig {
        aperture_decay: 0.0,
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("aperture_decay"));
}

#[test]
fn test_cone_validate_decay_one_fails() {
    let config = ConeConfig {
        aperture_decay: 1.0, // Boundary, excluded
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_cone_validate_decay_greater_than_one_fails() {
    let config = ConeConfig {
        aperture_decay: 1.5,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_cone_validate_threshold_zero_fails() {
    let config = ConeConfig {
        membership_threshold: 0.0,
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("membership_threshold"));
}

#[test]
fn test_cone_validate_threshold_one_fails() {
    let config = ConeConfig {
        membership_threshold: 1.0,
        ..Default::default()
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_cone_validate_nan_fields_fail() {
    // Test each field with NaN
    let configs = [
        ConeConfig {
            min_aperture: f32::NAN,
            ..Default::default()
        },
        ConeConfig {
            max_aperture: f32::NAN,
            ..Default::default()
        },
        ConeConfig {
            base_aperture: f32::NAN,
            ..Default::default()
        },
        ConeConfig {
            aperture_decay: f32::NAN,
            ..Default::default()
        },
        ConeConfig {
            membership_threshold: f32::NAN,
            ..Default::default()
        },
    ];

    for (i, config) in configs.iter().enumerate() {
        assert!(
            config.validate().is_err(),
            "Config {} with NaN should fail validation",
            i
        );
    }
}

#[test]
fn test_cone_config_serialization_roundtrip() {
    let config = ConeConfig::default();
    let json = serde_json::to_string(&config).expect("Serialization failed");
    let deserialized: ConeConfig = serde_json::from_str(&json).expect("Deserialization failed");
    assert_eq!(config, deserialized);
}

#[test]
fn test_cone_config_json_has_all_fields() {
    let json = serde_json::to_string_pretty(&ConeConfig::default()).unwrap();
    for field in [
        "min_aperture",
        "max_aperture",
        "base_aperture",
        "aperture_decay",
        "membership_threshold",
    ] {
        assert!(
            json.contains(&format!("\"{}\":", field)),
            "JSON must contain {}",
            field
        );
    }
}

#[test]
fn test_cone_config_equality() {
    let a = ConeConfig::default();
    let b = ConeConfig::default();
    assert_eq!(a, b, "Two default configs must be equal");

    let c = ConeConfig {
        min_aperture: 0.2, // Different
        ..Default::default()
    };
    assert_ne!(a, c, "Different configs must not be equal");
}

#[test]
fn test_cone_validate_base_below_min_fails() {
    let config = ConeConfig {
        min_aperture: 0.5,
        max_aperture: 1.5,
        base_aperture: 0.3, // Below min
        aperture_decay: 0.85,
        membership_threshold: 0.7,
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("base_aperture"));
}

#[test]
fn test_cone_validate_base_above_max_fails() {
    let config = ConeConfig {
        min_aperture: 0.1,
        max_aperture: 0.8,
        base_aperture: 1.0, // Above max
        aperture_decay: 0.85,
        membership_threshold: 0.7,
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("base_aperture"));
}
