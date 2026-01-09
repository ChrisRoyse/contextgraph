//! Tests for WarmConfig defaults, environment loading, and validation.

use super::helpers::{test_config, GB};
use crate::warm::config::{QuantizationMode, WarmConfig};
use crate::warm::error::WarmError;

use std::path::PathBuf;

#[test]
fn test_default_values_rtx_5090() {
    let config = WarmConfig::default();

    assert_eq!(config.vram_budget_bytes, 24 * GB);
    assert_eq!(config.vram_headroom_bytes, 8 * GB);
    assert_eq!(config.model_weights_path, PathBuf::from("./models"));
    assert_eq!(
        config.diagnostic_dump_path,
        PathBuf::from("/var/log/context-graph")
    );
    assert_eq!(config.cuda_device_id, 0);
    assert!(config.enable_test_inference);
    assert_eq!(config.max_load_time_per_model_ms, 30_000);
    assert_eq!(config.quantization, QuantizationMode::Fp16);
}

#[test]
fn test_total_vram_required_32gb() {
    let config = WarmConfig::default();
    assert_eq!(config.total_vram_required(), 32 * GB);
}

#[test]
fn test_quantization_mode_memory_multipliers() {
    assert_eq!(QuantizationMode::Fp32.memory_multiplier(), 1.0);
    assert_eq!(QuantizationMode::Fp16.memory_multiplier(), 0.5);
    assert_eq!(QuantizationMode::Fp8.memory_multiplier(), 0.25);
}

#[test]
fn test_quantization_mode_as_str() {
    assert_eq!(QuantizationMode::Fp32.as_str(), "FP32");
    assert_eq!(QuantizationMode::Fp16.as_str(), "FP16");
    assert_eq!(QuantizationMode::Fp8.as_str(), "FP8");
}

#[test]
fn test_validate_zero_budget_fails() {
    let mut config = test_config();
    config.vram_budget_bytes = 0;

    let result = config.validate();
    assert!(result.is_err());

    if let Err(WarmError::InvalidConfig { field, reason }) = result {
        assert_eq!(field, "vram_budget_bytes");
        assert!(reason.contains("greater than 0"));
    } else {
        panic!("Expected InvalidConfig error");
    }
}

#[test]
#[allow(clippy::field_reassign_with_default)]
fn test_validate_missing_path_fails() {
    let mut config = WarmConfig::default();
    config.model_weights_path = PathBuf::from("/nonexistent/path/that/does/not/exist");

    let result = config.validate();
    assert!(result.is_err());

    if let Err(WarmError::InvalidConfig { field, .. }) = result {
        assert_eq!(field, "model_weights_path");
    } else {
        panic!("Expected InvalidConfig error");
    }
}

#[test]
fn test_validate_valid_config_succeeds() {
    let config = test_config();
    assert!(config.validate().is_ok());
}

#[test]
fn test_from_env_vram_budget() {
    std::env::set_var("WARM_VRAM_BUDGET_BYTES", "16000000000");
    let config = WarmConfig::from_env();
    assert_eq!(config.vram_budget_bytes, 16_000_000_000);
    std::env::remove_var("WARM_VRAM_BUDGET_BYTES");
}

// Note: CUDA_VISIBLE_DEVICES test is in config.rs to avoid env var interference

#[test]
fn test_from_env_test_inference_boolean_variants() {
    for (val, expected) in [
        ("true", true),
        ("TRUE", true),
        ("1", true),
        ("yes", true),
        ("on", true),
        ("false", false),
        ("0", false),
        ("no", false),
        ("off", false),
    ] {
        std::env::set_var("WARM_ENABLE_TEST_INFERENCE", val);
        let config = WarmConfig::from_env();
        assert_eq!(
            config.enable_test_inference, expected,
            "Failed for value '{}'",
            val
        );
    }
    std::env::remove_var("WARM_ENABLE_TEST_INFERENCE");
}
