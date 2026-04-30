//! Test Cases for Diagnostics Module
//!
//! Comprehensive tests for the warm model diagnostic system.

use super::*;
use crate::warm::config::WarmConfig;
use crate::warm::cuda_alloc::GpuInfo;
use crate::warm::error::WarmError;
use crate::warm::handle::ModelHandle;
use crate::warm::loader::WarmLoader;
use crate::warm::registry::{EMBEDDING_MODEL_IDS, TOTAL_MODEL_COUNT};
use crate::warm::state::WarmModelState;

/// Create a test config that doesn't require real files.
#[allow(clippy::field_reassign_with_default)]
fn test_config() -> WarmConfig {
    let mut config = WarmConfig::default();
    config.enable_test_inference = true;
    config
}

#[test]
fn test_diagnostic_report_structure() {
    let config = test_config();
    let loader = WarmLoader::new(config).expect("Failed to create loader");
    let report = WarmDiagnostics::generate_report(&loader);

    assert!(!report.timestamp.is_empty());
    assert!(!report.system.hostname.is_empty() || report.system.hostname == "unknown");
    assert!(!report.system.os.is_empty());
    assert!(!report.models.is_empty());
    assert_eq!(report.models.len(), TOTAL_MODEL_COUNT);
}

#[test]
fn test_system_info_populated() {
    let system_info = SystemInfo::gather();
    assert!(!system_info.hostname.is_empty());
    assert!(system_info.os.contains(std::env::consts::OS));
    assert!(system_info.os.contains(std::env::consts::ARCH));
}

#[test]
fn test_gpu_diagnostics_from_info() {
    let gpu_info = GpuInfo::new(
        0,
        "NVIDIA GeForce RTX 5090".to_string(),
        (12, 0),
        32 * 1024 * 1024 * 1024,
        "13.2.0".to_string(),
    );

    let diagnostics = GpuDiagnostics::from_gpu_info(&gpu_info, 8 * 1024 * 1024 * 1024);

    assert_eq!(diagnostics.device_id, 0);
    assert_eq!(diagnostics.name, "NVIDIA GeForce RTX 5090");
    assert_eq!(diagnostics.compute_capability, "12.0");
    assert_eq!(diagnostics.total_vram_bytes, 32 * 1024 * 1024 * 1024);
    assert_eq!(diagnostics.available_vram_bytes, 8 * 1024 * 1024 * 1024);
    assert_eq!(diagnostics.driver_version, "13.2.0");
}

#[test]
fn test_memory_diagnostics_from_pools() {
    let config = test_config();
    let loader = WarmLoader::new(config).expect("Failed to create loader");
    let report = WarmDiagnostics::generate_report(&loader);

    assert_eq!(
        report.memory.model_pool_capacity_bytes,
        24 * 1024 * 1024 * 1024
    );
    assert_eq!(report.memory.model_pool_used_bytes, 0);
    assert_eq!(
        report.memory.working_pool_capacity_bytes,
        8 * 1024 * 1024 * 1024
    );
    assert_eq!(report.memory.total_allocations, 0);
}

#[test]
fn test_model_diagnostic_warm() {
    let state = WarmModelState::Warm;
    let handle_info = Some((0x7f8000000000u64, 629145600usize, 0xDEADBEEFu64));
    let diagnostic = ModelDiagnostic::from_state("E1_Semantic", &state, 629145600, handle_info);

    assert_eq!(diagnostic.model_id, "E1_Semantic");
    assert_eq!(diagnostic.state, "Warm");
    assert_eq!(diagnostic.expected_bytes, 629145600);
    assert_eq!(diagnostic.allocated_bytes, Some(629145600));
    assert_eq!(diagnostic.vram_ptr, Some("0x00007f8000000000".to_string()));
    assert_eq!(diagnostic.checksum, Some("0x00000000DEADBEEF".to_string()));
    assert!(diagnostic.error_message.is_none());
}

#[test]
fn test_model_diagnostic_failed() {
    let state = WarmModelState::Failed {
        error_code: 102,
        error_message: "CUDA allocation failed".to_string(),
    };
    let diagnostic = ModelDiagnostic::from_state("E1_Semantic", &state, 629145600, None);

    assert_eq!(diagnostic.model_id, "E1_Semantic");
    assert_eq!(diagnostic.state, "Failed");
    assert_eq!(diagnostic.expected_bytes, 629145600);
    assert!(diagnostic.allocated_bytes.is_none());
    assert!(diagnostic.vram_ptr.is_none());
    assert!(diagnostic.checksum.is_none());
    assert_eq!(
        diagnostic.error_message,
        Some("CUDA allocation failed".to_string())
    );
}

#[test]
fn test_json_serialization() {
    let config = test_config();
    let loader = WarmLoader::new(config).expect("Failed to create loader");
    let json = WarmDiagnostics::to_json(&loader).expect("Failed to serialize to JSON");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("Failed to parse JSON");

    assert!(parsed.get("timestamp").is_some());
    assert!(parsed.get("system").is_some());
    assert!(parsed.get("memory").is_some());
    assert!(parsed.get("models").is_some());
    assert!(parsed.get("errors").is_some());

    let models = parsed.get("models").unwrap().as_array().unwrap();
    assert_eq!(models.len(), TOTAL_MODEL_COUNT);
}

#[test]
fn test_status_line_format() {
    let config = test_config();
    let loader = WarmLoader::new(config).expect("Failed to create loader");
    let status = WarmDiagnostics::status_line(&loader);

    assert!(status.contains("WARM:"));
    assert!(status.contains("models"));
    assert!(status.contains("VRAM"));
    // Total models is dynamic based on TOTAL_MODEL_COUNT (13)
    assert!(status.contains(&format!("LOADING: 0/{}", TOTAL_MODEL_COUNT)));
}

#[test]
fn test_error_diagnostic_from_error() {
    let error = WarmError::CudaInitFailed {
        cuda_error: "Driver not found".to_string(),
        driver_version: String::new(),
        gpu_name: String::new(),
    };
    let diagnostic = ErrorDiagnostic::from_error(&error);

    assert_eq!(diagnostic.error_code, "ERR-WARM-CUDA-INIT");
    assert_eq!(diagnostic.category, "CUDA");
    assert_eq!(diagnostic.exit_code, 106);
    assert!(diagnostic.message.contains("Driver not found"));
}

#[test]
fn test_report_warm_and_failed_counts() {
    let mut report = WarmDiagnosticReport::empty();

    report.models.push(ModelDiagnostic {
        model_id: "model1".to_string(),
        state: "Warm".to_string(),
        expected_bytes: 1000,
        allocated_bytes: Some(1000),
        vram_ptr: None,
        checksum: None,
        error_message: None,
    });

    report.models.push(ModelDiagnostic {
        model_id: "model2".to_string(),
        state: "Failed".to_string(),
        expected_bytes: 1000,
        allocated_bytes: None,
        vram_ptr: None,
        checksum: None,
        error_message: Some("Error".to_string()),
    });

    report.models.push(ModelDiagnostic {
        model_id: "model3".to_string(),
        state: "Warm".to_string(),
        expected_bytes: 1000,
        allocated_bytes: Some(1000),
        vram_ptr: None,
        checksum: None,
        error_message: None,
    });

    assert_eq!(report.warm_count(), 2);
    assert_eq!(report.failed_count(), 1);
}

#[test]
fn test_report_has_errors() {
    let mut report = WarmDiagnosticReport::empty();
    assert!(!report.has_errors());

    report.errors.push(ErrorDiagnostic {
        error_code: "ERR-TEST".to_string(),
        category: "TEST".to_string(),
        message: "Test error".to_string(),
        exit_code: 1,
    });

    assert!(report.has_errors());
}

#[test]
fn test_model_diagnostic_loading_state() {
    let state = WarmModelState::Loading {
        progress_percent: 75,
        bytes_loaded: 500_000_000,
    };
    let diagnostic = ModelDiagnostic::from_state("E1_Semantic", &state, 629145600, None);
    assert_eq!(diagnostic.state, "Loading (75%)");
}

#[test]
fn test_model_diagnostic_validating_state() {
    let state = WarmModelState::Validating;
    let diagnostic = ModelDiagnostic::from_state("E1_Semantic", &state, 629145600, None);
    assert_eq!(diagnostic.state, "Validating");
}

#[test]
fn test_full_report_with_warm_models() {
    let config = test_config();
    let loader = WarmLoader::new(config).expect("Failed to create loader");

    {
        let mut registry = loader.registry().write().unwrap();
        for model_id in EMBEDDING_MODEL_IDS.iter() {
            registry.start_loading(model_id).unwrap();
            registry.mark_validating(model_id).unwrap();
            let handle = ModelHandle::new(0x1000, 1024, 0, 0xDEAD);
            registry.mark_warm(model_id, handle).unwrap();
        }
    }

    let report = WarmDiagnostics::generate_report(&loader);
    assert_eq!(report.warm_count(), TOTAL_MODEL_COUNT);
    assert_eq!(report.failed_count(), 0);
    assert!(!report.has_errors());

    let status = WarmDiagnostics::status_line(&loader);
    assert!(status.contains("OK"));
    // Total models is dynamic based on TOTAL_MODEL_COUNT (13)
    assert!(status.contains(&format!("{}/{}", TOTAL_MODEL_COUNT, TOTAL_MODEL_COUNT)));
}

#[test]
fn test_timestamp_format() {
    let report = WarmDiagnosticReport::empty();
    assert!(report.timestamp.contains('T'));
    assert!(report.timestamp.ends_with('Z'));
    assert_eq!(report.timestamp.len(), 24);
}
