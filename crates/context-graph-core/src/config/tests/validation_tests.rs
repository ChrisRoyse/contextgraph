//! Tests for configuration validation.
//!
//! Includes Phase-aware safety validation tests that prevent
//! Production phase from using stub/in-memory backends.

use crate::config::{Config, EmbeddingConfig, IndexConfig, Phase, StorageConfig, UtlConfig};

#[test]
fn test_validation_passes() {
    let config = Config::default_config();
    assert!(config.validate().is_ok());
}

#[test]
fn test_validation_fails_zero_payload() {
    let mut config = Config::default_config();
    config.mcp.max_payload_size = 0;
    assert!(config.validate().is_err());
}

// =========================================================================
// TC-GHOST-006: Configuration & Infrastructure Tests - Validation
// =========================================================================

#[test]
fn test_config_validation_embedding_dimension_zero() {
    // TC-GHOST-006: Embedding dimension 0 must fail validation
    let mut config = Config::default_config();
    config.embedding.dimension = 0;

    let result = config.validate();
    assert!(
        result.is_err(),
        "Embedding dimension 0 must fail validation"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("embedding.dimension"),
        "Error must mention embedding.dimension"
    );
}

// =========================================================================
// Phase-Aware Safety Validation Tests (P0-FIX-2)
// Per Constitution AP-007: No stubs or fallbacks in production code paths
// =========================================================================

/// Test that Ghost phase allows stub configurations
#[test]
fn test_ghost_phase_allows_stubs() {
    let config = Config {
        phase: Phase::Ghost,
        embedding: EmbeddingConfig {
            model: "stub".to_string(),
            ..Default::default()
        },
        storage: StorageConfig {
            backend: "memory".to_string(),
            ..Default::default()
        },
        index: IndexConfig {
            backend: "memory".to_string(),
            ..Default::default()
        },
        utl: UtlConfig {
            mode: "stub".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    // Ghost phase should allow stubs (will warn to stderr but not fail)
    assert!(
        config.validate().is_ok(),
        "Ghost phase must allow stub configurations"
    );
    assert!(config.uses_stubs(), "Config should report using stubs");
    assert!(
        !config.is_production_safe(),
        "Config with stubs is not production-safe"
    );
}

/// Test that Development phase allows stub configurations with warnings
#[test]
fn test_development_phase_allows_stubs() {
    let config = Config {
        phase: Phase::Development,
        embedding: EmbeddingConfig {
            model: "stub".to_string(),
            ..Default::default()
        },
        storage: StorageConfig {
            backend: "memory".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    // Development phase should allow stubs (will warn to stderr but not fail)
    assert!(
        config.validate().is_ok(),
        "Development phase must allow stub configurations"
    );
    assert!(config.uses_stubs(), "Config should report using stubs");
}

/// Test that Production phase REJECTS stub embedding model
#[test]
fn test_production_phase_rejects_stub_embedding() {
    let config = Config {
        phase: Phase::Production,
        embedding: EmbeddingConfig {
            model: "stub".to_string(),
            ..Default::default()
        },
        storage: StorageConfig {
            backend: "rocksdb".to_string(),
            path: "/tmp/test".to_string(),
            compression: true,
        },
        index: IndexConfig {
            backend: "hnsw".to_string(),
            ..Default::default()
        },
        utl: UtlConfig {
            mode: "real".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    let result = config.validate();
    assert!(
        result.is_err(),
        "Production phase must reject stub embedding model"
    );

    let err = result.unwrap_err();
    let err_string = err.to_string();
    assert!(
        err_string.contains("PRODUCTION PHASE SAFETY VIOLATION"),
        "Error must mention production safety violation, got: {}",
        err_string
    );
    assert!(
        err_string.contains("embedding.model"),
        "Error must mention embedding.model, got: {}",
        err_string
    );
}

/// Test that Production phase REJECTS memory storage backend
#[test]
fn test_production_phase_rejects_memory_storage() {
    let config = Config {
        phase: Phase::Production,
        embedding: EmbeddingConfig {
            model: "multi_array_13".to_string(),
            ..Default::default()
        },
        storage: StorageConfig {
            backend: "memory".to_string(), // <-- DANGEROUS
            ..Default::default()
        },
        index: IndexConfig {
            backend: "hnsw".to_string(),
            ..Default::default()
        },
        utl: UtlConfig {
            mode: "real".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    let result = config.validate();
    assert!(
        result.is_err(),
        "Production phase must reject memory storage"
    );

    let err = result.unwrap_err();
    let err_string = err.to_string();
    assert!(
        err_string.contains("storage.backend"),
        "Error must mention storage.backend, got: {}",
        err_string
    );
    assert!(
        err_string.contains("ALL DATA WILL BE LOST"),
        "Error must explain the danger, got: {}",
        err_string
    );
}

/// Test that Production phase REJECTS memory index backend
#[test]
fn test_production_phase_rejects_memory_index() {
    let config = Config {
        phase: Phase::Production,
        embedding: EmbeddingConfig {
            model: "multi_array_13".to_string(),
            ..Default::default()
        },
        storage: StorageConfig {
            backend: "rocksdb".to_string(),
            path: "/tmp/test".to_string(),
            compression: true,
        },
        index: IndexConfig {
            backend: "memory".to_string(), // <-- DANGEROUS
            ..Default::default()
        },
        utl: UtlConfig {
            mode: "real".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    let result = config.validate();
    assert!(result.is_err(), "Production phase must reject memory index");

    let err = result.unwrap_err();
    let err_string = err.to_string();
    assert!(
        err_string.contains("index.backend"),
        "Error must mention index.backend, got: {}",
        err_string
    );
}

/// Test that Production phase REJECTS stub UTL mode
#[test]
fn test_production_phase_rejects_stub_utl() {
    let config = Config {
        phase: Phase::Production,
        embedding: EmbeddingConfig {
            model: "multi_array_13".to_string(),
            ..Default::default()
        },
        storage: StorageConfig {
            backend: "rocksdb".to_string(),
            path: "/tmp/test".to_string(),
            compression: true,
        },
        index: IndexConfig {
            backend: "hnsw".to_string(),
            ..Default::default()
        },
        utl: UtlConfig {
            mode: "stub".to_string(), // <-- DANGEROUS
            ..Default::default()
        },
        ..Default::default()
    };

    let result = config.validate();
    assert!(
        result.is_err(),
        "Production phase must reject stub UTL mode"
    );

    let err = result.unwrap_err();
    let err_string = err.to_string();
    assert!(
        err_string.contains("utl.mode"),
        "Error must mention utl.mode, got: {}",
        err_string
    );
}

/// Test that Production phase ACCEPTS valid production configuration
#[test]
fn test_production_phase_accepts_valid_config() {
    let config = Config {
        phase: Phase::Production,
        embedding: EmbeddingConfig {
            model: "multi_array_13".to_string(),
            dimension: 11264,
            max_input_length: 8191,
        },
        storage: StorageConfig {
            backend: "rocksdb".to_string(),
            path: "/tmp/test_storage".to_string(),
            compression: true,
        },
        index: IndexConfig {
            backend: "hnsw".to_string(),
            hnsw_m: 16,
            hnsw_ef_construction: 200,
        },
        utl: UtlConfig {
            mode: "real".to_string(),
            default_emotional_weight: 1.0,
            consolidation_threshold: 0.7,
        },
        ..Default::default()
    };

    let result = config.validate();
    assert!(
        result.is_ok(),
        "Production phase must accept valid config: {:?}",
        result.err()
    );
    assert!(
        !config.uses_stubs(),
        "Valid production config should not use stubs"
    );
    assert!(
        config.is_production_safe(),
        "Valid production config should be production-safe"
    );
}

/// Test that multiple dangerous configs are all reported in error
#[test]
fn test_production_phase_reports_all_dangerous_configs() {
    let config = Config {
        phase: Phase::Production,
        embedding: EmbeddingConfig {
            model: "stub".to_string(),
            ..Default::default()
        },
        storage: StorageConfig {
            backend: "memory".to_string(),
            ..Default::default()
        },
        index: IndexConfig {
            backend: "memory".to_string(),
            ..Default::default()
        },
        utl: UtlConfig {
            mode: "stub".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    let result = config.validate();
    assert!(result.is_err(), "Production must reject all-stub config");

    let err = result.unwrap_err();
    let err_string = err.to_string();

    // All dangerous configs should be mentioned
    assert!(
        err_string.contains("embedding.model"),
        "Should mention embedding"
    );
    assert!(
        err_string.contains("storage.backend"),
        "Should mention storage"
    );
    assert!(err_string.contains("index.backend"), "Should mention index");
    assert!(err_string.contains("utl.mode"), "Should mention utl");

    // Remediation should be included
    assert!(
        err_string.contains("REMEDIATION"),
        "Should include remediation steps"
    );
}

/// Test uses_stubs helper function
#[test]
fn test_uses_stubs_helper() {
    // All real backends
    let config_real = Config {
        embedding: EmbeddingConfig {
            model: "multi_array_13".to_string(),
            ..Default::default()
        },
        storage: StorageConfig {
            backend: "rocksdb".to_string(),
            path: "/tmp/test".to_string(),
            ..Default::default()
        },
        index: IndexConfig {
            backend: "hnsw".to_string(),
            ..Default::default()
        },
        utl: UtlConfig {
            mode: "real".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        !config_real.uses_stubs(),
        "Real config should not use stubs"
    );

    // Just one stub
    let config_one_stub = Config {
        embedding: EmbeddingConfig {
            model: "stub".to_string(), // <-- One stub
            ..Default::default()
        },
        storage: StorageConfig {
            backend: "rocksdb".to_string(),
            path: "/tmp/test".to_string(),
            ..Default::default()
        },
        index: IndexConfig {
            backend: "hnsw".to_string(),
            ..Default::default()
        },
        utl: UtlConfig {
            mode: "real".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        config_one_stub.uses_stubs(),
        "Config with any stub should return true"
    );
}

/// Test is_production_safe helper function
#[test]
fn test_is_production_safe_helper() {
    // Production phase, no stubs
    let config_safe = Config {
        phase: Phase::Production,
        embedding: EmbeddingConfig {
            model: "multi_array_13".to_string(),
            ..Default::default()
        },
        storage: StorageConfig {
            backend: "rocksdb".to_string(),
            path: "/tmp/test".to_string(),
            ..Default::default()
        },
        index: IndexConfig {
            backend: "hnsw".to_string(),
            ..Default::default()
        },
        utl: UtlConfig {
            mode: "real".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        config_safe.is_production_safe(),
        "Should be production safe"
    );

    // Production phase, with stubs - NOT safe
    let config_unsafe = Config {
        phase: Phase::Production,
        ..Default::default() // Uses stub defaults
    };
    assert!(
        !config_unsafe.is_production_safe(),
        "Production with stubs is not safe"
    );

    // Ghost phase, no stubs - still not "production safe" because phase is wrong
    let config_ghost = Config {
        phase: Phase::Ghost,
        embedding: EmbeddingConfig {
            model: "multi_array_13".to_string(),
            ..Default::default()
        },
        storage: StorageConfig {
            backend: "rocksdb".to_string(),
            path: "/tmp/test".to_string(),
            ..Default::default()
        },
        index: IndexConfig {
            backend: "hnsw".to_string(),
            ..Default::default()
        },
        utl: UtlConfig {
            mode: "real".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        !config_ghost.is_production_safe(),
        "Ghost phase is never production-safe"
    );
}

/// Test default config phase
#[test]
fn test_default_config_is_ghost_phase() {
    let config = Config::default();
    assert_eq!(
        config.phase,
        Phase::Ghost,
        "Default config should be Ghost phase"
    );
    assert!(config.uses_stubs(), "Default config should use stubs");
}
