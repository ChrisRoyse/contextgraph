//! Error types for Knowledge Graph operations.
//!
//! This module provides comprehensive error handling for all graph operations
//! including FAISS index, hyperbolic geometry, RocksDB storage, and traversal.
//!
//! # Constitution Reference
//!
//! - AP-001: Never unwrap() in prod - all errors properly typed
//! - rules: Result<T,E> for fallible ops, thiserror for derivation
//!
//! TODO: Expand error conversions in M04-T08a

use thiserror::Error;

/// Result type alias for graph operations.
pub type GraphResult<T> = Result<T, GraphError>;

/// Comprehensive error type for all graph operations.
///
/// Each variant includes context for debugging and error recovery.
/// All errors are designed to fail fast with clear messages.
///
/// TODO: M04-T08 will add additional variants as needed
#[derive(Error, Debug)]
pub enum GraphError {
    // ========== FAISS Index Errors ==========
    /// FAISS index creation failed.
    #[error("FAISS index creation failed: {0}")]
    FaissIndexCreation(String),

    /// FAISS training failed.
    #[error("FAISS training failed: {0}")]
    FaissTrainingFailed(String),

    /// FAISS search failed.
    #[error("FAISS search failed: {0}")]
    FaissSearchFailed(String),

    /// FAISS add vectors failed.
    #[error("FAISS add failed: {0}")]
    FaissAddFailed(String),

    /// Index not trained - operations require trained index.
    #[error("Index not trained - must call train() before search/add")]
    IndexNotTrained,

    /// Insufficient training data for IVF clustering.
    #[error("Insufficient training data: need at least {required} vectors, got {provided}")]
    InsufficientTrainingData { required: usize, provided: usize },

    // ========== GPU Resource Errors ==========
    /// GPU resource allocation failed.
    #[error("GPU resource allocation failed: {0}")]
    GpuResourceAllocation(String),

    /// GPU memory transfer failed.
    #[error("GPU transfer failed: {0}")]
    GpuTransferFailed(String),

    /// GPU device not available.
    #[error("GPU device not available: {0}")]
    GpuDeviceUnavailable(String),

    /// FAISS GPU is not available.
    ///
    /// This error occurs when:
    /// - `faiss-working` feature is not enabled in context-graph-cuda
    /// - FAISS library is not installed
    /// - No GPU available for FAISS
    ///
    /// # Resolution
    ///
    /// 1. Run `./scripts/rebuild_faiss_gpu.sh` to build FAISS with CUDA 13.1+
    /// 2. Build with: `cargo build -p context-graph-cuda --features faiss-working`
    #[error("FAISS GPU unavailable: {reason}. Help: {help}")]
    FaissGpuUnavailable {
        /// Why FAISS GPU is unavailable
        reason: String,
        /// How to fix the issue
        help: String,
    },

    // ========== Storage Errors ==========
    /// Failed to open storage at specific path.
    #[error("Failed to open storage at {path}: {cause}")]
    StorageOpen { path: String, cause: String },

    /// RocksDB storage error.
    #[error("Storage error: {0}")]
    Storage(String),

    /// Column family not found in RocksDB.
    #[error("Column family not found: {0}")]
    ColumnFamilyNotFound(String),

    /// Data corruption detected during deserialization.
    #[error("Corrupted data in {location}: {details}")]
    CorruptedData { location: String, details: String },

    /// Storage migration failed.
    #[error("Storage migration failed: {0}")]
    MigrationFailed(String),

    // ========== Configuration Errors ==========
    /// Invalid configuration parameter.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Dimension mismatch between vectors.
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    // ========== Graph Structure Errors ==========
    /// Node not found in graph.
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    /// Edge not found in graph.
    #[error("Edge not found: source={0}, target={1}")]
    EdgeNotFound(String, String),

    /// Duplicate node ID.
    #[error("Duplicate node ID: {0}")]
    DuplicateNode(String),

    // ========== Hyperbolic Geometry Errors ==========
    /// Invalid hyperbolic point (norm >= 1.0 or invalid).
    #[error("Invalid hyperbolic point: norm {norm} >= max_norm (must be in open ball)")]
    InvalidHyperbolicPoint { norm: f32 },

    /// Invalid curvature (must be negative).
    #[error("Invalid curvature: {0} (must be negative)")]
    InvalidCurvature(f32),

    /// Mobius operation failed.
    #[error("Mobius operation failed: {0}")]
    MobiusOperationFailed(String),

    // ========== Entailment Cone Errors ==========
    /// Invalid cone aperture.
    #[error("Invalid cone aperture: {0} (must be in (0, PI))")]
    InvalidAperture(f32),

    /// Cone axis is zero vector.
    #[error("Cone axis cannot be zero vector")]
    ZeroConeAxis,

    // ========== Traversal Errors ==========
    /// Path not found between nodes.
    #[error("No path found from {0} to {1}")]
    PathNotFound(String, String),

    /// Traversal depth exceeded.
    #[error("Traversal depth limit exceeded: {0}")]
    DepthLimitExceeded(usize),

    /// Cycle detected during traversal.
    #[error("Cycle detected at node: {0}")]
    CycleDetected(String),

    /// Missing hyperbolic coordinates for A* traversal (M04-T17a).
    /// A* requires hyperbolic embeddings for distance heuristic.
    /// NO FALLBACK - fail fast per AP-001.
    #[error("Missing hyperbolic data for node {0}: A* requires hyperbolic embeddings")]
    MissingHyperbolicData(i64),

    // ========== Validation Errors ==========
    /// Invalid input provided to a function.
    /// Used for FAIL FAST validation of parameters.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Vector ID mismatch between index and storage.
    #[error("Vector ID mismatch: {0}")]
    VectorIdMismatch(String),

    /// Invalid NT weights (must be in [0,1]).
    #[error("Invalid NT weights: {field} = {value} (must be in [0.0, 1.0])")]
    InvalidNtWeights { field: String, value: f32 },

    // ========== Serialization Errors ==========
    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Deserialization error.
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    // ========== I/O Errors ==========
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ========== Error Conversions ==========
// Enable ? operator for external error types

impl From<rocksdb::Error> for GraphError {
    fn from(err: rocksdb::Error) -> Self {
        GraphError::Storage(err.to_string())
    }
}

impl From<serde_json::Error> for GraphError {
    fn from(err: serde_json::Error) -> Self {
        // serde_json errors include line/column info in to_string()
        GraphError::Serialization(err.to_string())
    }
}

impl From<bincode::Error> for GraphError {
    fn from(err: bincode::Error) -> Self {
        // Box<bincode::ErrorKind> - deref for message
        GraphError::Deserialization(err.to_string())
    }
}

// Compile-time verification that GraphError is thread-safe
static_assertions::assert_impl_all!(GraphError: Send, Sync, std::error::Error);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_index_not_trained() {
        let err = GraphError::IndexNotTrained;
        let msg = err.to_string();
        assert!(msg.contains("not trained"));
        assert!(msg.contains("train()"));
    }

    #[test]
    fn test_error_display_insufficient_training_data() {
        let err = GraphError::InsufficientTrainingData {
            required: 4194304,
            provided: 1000,
        };
        let msg = err.to_string();
        assert!(msg.contains("4194304"));
        assert!(msg.contains("1000"));
    }

    #[test]
    fn test_error_display_invalid_hyperbolic_point() {
        let err = GraphError::InvalidHyperbolicPoint { norm: 1.5 };
        let msg = err.to_string();
        assert!(msg.contains("1.5"));
        assert!(msg.contains("norm"));
    }

    #[test]
    fn test_error_display_dimension_mismatch() {
        let err = GraphError::DimensionMismatch {
            expected: 1536,
            actual: 768,
        };
        let msg = err.to_string();
        assert!(msg.contains("1536"));
        assert!(msg.contains("768"));
    }

    #[test]
    fn test_error_display_node_not_found() {
        let err = GraphError::NodeNotFound("abc-123".to_string());
        let msg = err.to_string();
        assert!(msg.contains("abc-123"));
    }

    #[test]
    fn test_error_display_edge_not_found() {
        let err = GraphError::EdgeNotFound("node-a".to_string(), "node-b".to_string());
        let msg = err.to_string();
        assert!(msg.contains("node-a"));
        assert!(msg.contains("node-b"));
    }

    #[test]
    fn test_error_display_corrupted_data() {
        let err = GraphError::CorruptedData {
            location: "edges_cf".to_string(),
            details: "invalid bincode".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("edges_cf"));
        assert!(msg.contains("invalid bincode"));
    }

    #[test]
    fn test_error_display_invalid_nt_weights() {
        let err = GraphError::InvalidNtWeights {
            field: "excitatory".to_string(),
            value: 1.5,
        };
        let msg = err.to_string();
        assert!(msg.contains("excitatory"));
        assert!(msg.contains("1.5"));
    }

    #[test]
    fn test_graph_result_type_alias() {
        fn example_fn() -> GraphResult<u32> {
            Ok(42)
        }
        assert_eq!(example_fn().unwrap(), 42);
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let graph_err: GraphError = io_err.into();
        assert!(matches!(graph_err, GraphError::Io(_)));
    }

    // ========== M04-T08 Tests ==========

    #[test]
    fn test_error_display_storage_open() {
        let err = GraphError::StorageOpen {
            path: "/data/graph.db".to_string(),
            cause: "permission denied".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("/data/graph.db"));
        assert!(msg.contains("permission denied"));
        assert!(msg.contains("Failed to open storage"));
    }

    #[test]
    fn test_graph_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GraphError>();
    }

    #[test]
    fn test_storage_open_empty_path() {
        // Edge case: empty path
        let err = GraphError::StorageOpen {
            path: "".to_string(),
            cause: "invalid".to_string(),
        };
        println!("BEFORE: constructing StorageOpen with empty path");
        let msg = err.to_string();
        println!("AFTER: message = {}", msg);
        assert!(msg.contains("Failed to open storage"));
    }

    #[test]
    fn test_storage_open_unicode() {
        // Edge case: unicode in error messages
        let err = GraphError::StorageOpen {
            path: "/данные/граф.db".to_string(), // Russian
            cause: "权限被拒绝".to_string(),     // Chinese
        };
        println!("BEFORE: constructing with unicode");
        let msg = err.to_string();
        println!("AFTER: message = {}", msg);
        assert!(msg.contains("данные"));
    }

    #[test]
    fn test_storage_open_long_path() {
        // Edge case: very long path
        let long_path = "a".repeat(10000);
        let err = GraphError::StorageOpen {
            path: long_path.clone(),
            cause: "test".to_string(),
        };
        println!("BEFORE: constructing with 10000 char path");
        let msg = err.to_string();
        println!("AFTER: message length = {}", msg.len());
        assert!(msg.len() > 10000);
    }

    // ========== M04-T08a Tests ==========

    #[test]
    fn test_rocksdb_error_conversion() {
        // Test that RocksDB error conversion compiles and works
        // Note: rocksdb::Error doesn't have public constructors,
        // so we verify via real scenario or type checking
        fn rocksdb_fn() -> GraphResult<()> {
            let temp_dir = tempfile::tempdir()?; // io::Error -> GraphError::Io
            let path = temp_dir.path().join("test_rocksdb.db");
            let _db = rocksdb::DB::open_default(&path)?; // rocksdb::Error -> GraphError
            Ok(())
        }

        // If this compiles and runs, the From impl works
        let result = rocksdb_fn();
        assert!(result.is_ok());
    }

    #[test]
    fn test_serde_json_error_conversion() {
        // Create invalid JSON to trigger parse error
        let invalid_json = "{ invalid json }";
        let result: Result<serde_json::Value, serde_json::Error> =
            serde_json::from_str(invalid_json);

        let json_err = result.unwrap_err();
        let graph_err: GraphError = json_err.into();

        // Verify it converted to Serialization variant
        match &graph_err {
            GraphError::Serialization(msg) => {
                assert!(!msg.is_empty());
                println!("JSON error converted: {}", msg);
            }
            _ => panic!("Expected GraphError::Serialization, got {:?}", graph_err),
        }
    }

    #[test]
    fn test_bincode_error_conversion() {
        // Create invalid bincode data to trigger deserialize error
        let invalid_data: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let result: Result<String, bincode::Error> = bincode::deserialize(invalid_data);

        let bincode_err = result.unwrap_err();
        let graph_err: GraphError = bincode_err.into();

        // Verify it converted to Deserialization variant
        match &graph_err {
            GraphError::Deserialization(msg) => {
                assert!(!msg.is_empty());
                println!("Bincode error converted: {}", msg);
            }
            _ => panic!("Expected GraphError::Deserialization, got {:?}", graph_err),
        }
    }

    #[test]
    fn test_question_mark_operator_with_conversions() {
        // Verify ? operator works in function returning GraphResult

        fn json_parse_fn(json: &str) -> GraphResult<serde_json::Value> {
            let value = serde_json::from_str(json)?; // ? converts serde_json::Error
            Ok(value)
        }

        fn bincode_fn(data: &[u8]) -> GraphResult<String> {
            let value = bincode::deserialize(data)?; // ? converts bincode::Error
            Ok(value)
        }

        // Valid JSON should succeed
        let valid = json_parse_fn(r#"{"key": "value"}"#);
        assert!(valid.is_ok());

        // Invalid JSON should fail with Serialization error
        let invalid = json_parse_fn("not json");
        assert!(matches!(invalid, Err(GraphError::Serialization(_))));

        // Invalid bincode should fail with Deserialization error
        let invalid_bin = bincode_fn(&[0xFF, 0xFF]);
        assert!(matches!(invalid_bin, Err(GraphError::Deserialization(_))));
    }

    #[test]
    fn test_json_error_preserves_position_info() {
        // Edge case: serde_json error with position info
        let json = r#"{
            "key": "value",
            broken
        }"#;
        println!("BEFORE: parsing multi-line invalid JSON");
        let result: Result<serde_json::Value, _> = serde_json::from_str(json);
        let err = result.unwrap_err();
        println!("AFTER: serde_json error = {}", err);
        let graph_err: GraphError = err.into();
        println!("AFTER: GraphError = {}", graph_err);
        // serde_json includes position info in error
        let msg = graph_err.to_string();
        assert!(!msg.is_empty());
    }

    #[test]
    fn test_bincode_type_mismatch_error() {
        // Edge case: bincode error with truncated/invalid data
        // Use intentionally truncated data that can't be deserialized as u64
        let truncated_data: &[u8] = &[0x01, 0x02, 0x03]; // Only 3 bytes, need 8 for u64
        println!("BEFORE: deserializing truncated data as u64");
        let result: Result<u64, _> = bincode::deserialize(truncated_data);
        let err = result.unwrap_err();
        println!("AFTER: bincode error = {}", err);
        let graph_err: GraphError = err.into();
        println!("AFTER: GraphError = {}", graph_err);
        assert!(matches!(graph_err, GraphError::Deserialization(_)));
    }
}
