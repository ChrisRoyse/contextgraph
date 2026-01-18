//! MCP Protocol Compliance Unit Tests
//!
//! TASK-S001: Updated to use TeleologicalMemoryStore and MultiArrayEmbeddingProvider.
//! TASK-S003: Added GoalAlignmentCalculator and GoalHierarchy for purpose operations.
//! NO BACKWARDS COMPATIBILITY with legacy MemoryStore.
//!
//! Tests verify compliance with MCP protocol version 2024-11-05
//! Reference: https://spec.modelcontextprotocol.io/specification/2024-11-05/
//!
//! # Test Helper Variants
//!
//! This module provides three categories of test helpers:
//!
//! ## Fast In-Memory Helpers (Unit Tests)
//! - `create_test_handlers()` - Uses InMemoryTeleologicalStore with stubs
//! - `create_test_handlers_no_goals()` - Same but with empty goal hierarchy
//!
//! ## Real Storage Helpers (Integration Tests)
//! - `create_test_handlers_with_rocksdb()` - Uses RocksDbTeleologicalStore with tempdir
//! - `create_test_handlers_with_rocksdb_no_goals()` - Same but with empty goal hierarchy
//! - `create_test_handlers_with_rocksdb_store_access()` - Same but returns store for FSV assertions
//!
//! ## Real GPU Embedding Helpers (FSV Tests) - Feature-gated: `cuda`
//! - `create_test_handlers_with_real_embeddings()` - Uses ProductionMultiArrayProvider (GPU)
//! - `create_test_handlers_with_real_embeddings_store_access()` - Same but returns store for FSV
//!
//! # TempDir Lifecycle
//!
//! The RocksDB helpers return `(Handlers, TempDir)`. The TempDir MUST be kept alive
//! for the duration of the test - dropping it will delete the database directory
//! and cause "Invalid argument" errors from RocksDB.
//!
//! ```ignore
//! #[tokio::test]
//! async fn test_with_real_storage() {
//!     let (handlers, _tempdir) = create_test_handlers_with_rocksdb();
//!     // _tempdir keeps the database alive until end of test
//!     // ... test code ...
//! } // _tempdir dropped here, cleaning up the database
//! ```

mod cognitive_pulse;
mod content_storage_verification;
mod curation_tools_fsv;
mod dream_tools_integration;
mod semantic_search_skill_verification;
mod error_codes;
mod full_state_verification_search;
mod initialize;
mod inject_synthetic_data;
mod manual_fsv_verification;
mod memory;
mod phase1_manual_store_verify;
mod phase2_search_retrieval;
mod phase7_final_state_verification;
mod search;
mod stubfix_fsv_tests;
mod task_emb_024_verification;
mod tcp_transport_integration;
mod tools_call;
mod tools_list;
mod topic_tools;
mod topic_tools_fsv;

use std::sync::Arc;

use parking_lot::RwLock;
use tempfile::TempDir;

use context_graph_core::monitoring::{LayerStatusProvider, StubLayerStatusProvider};
use context_graph_core::purpose::{GoalDiscoveryMetadata, GoalHierarchy, GoalLevel, GoalNode};
use context_graph_core::stubs::{
    InMemoryTeleologicalStore, StubMultiArrayProvider, StubUtlProcessor,
};
use context_graph_core::traits::{
    MultiArrayEmbeddingProvider, TeleologicalMemoryStore, UtlProcessor,
};
use context_graph_core::types::fingerprint::SemanticFingerprint;
use context_graph_storage::teleological::RocksDbTeleologicalStore;

// TASK-P3-01: Import real embedding provider for FSV tests (feature-gated)
#[cfg(feature = "cuda")]
use context_graph_embeddings::{GpuConfig, ProductionMultiArrayProvider};

// TASK-P3-01: Import PathBuf for models directory resolution
#[cfg(feature = "cuda")]
use std::path::PathBuf;

// TASK-WARM-LOAD: Warm model cache for GPU embedding tests
// Prevents OOM by loading models ONCE and sharing across all tests
#[cfg(feature = "cuda")]
use tokio::sync::OnceCell;

/// Global warm-loaded model cache.
///
/// RTX 5090 32GB VRAM - models should be warm-loaded ONCE and shared.
/// This prevents CUDA OOM when tests run in parallel, each trying to load
/// all 13 embedding models (~20GB total) from scratch.
///
/// FAIL FAST: If initial load fails, all tests using real embeddings will fail.
#[cfg(feature = "cuda")]
static WARM_MODEL_CACHE: OnceCell<Arc<dyn MultiArrayEmbeddingProvider>> = OnceCell::const_new();

/// Get or initialize the warm-loaded embedding provider.
///
/// This function ensures models are loaded exactly ONCE into GPU VRAM and
/// shared across all tests. The Arc allows multiple tests to hold references
/// to the same provider instance.
///
/// # RTX 5090 32GB Configuration
///
/// With 32GB VRAM, all 13 embedding models fit comfortably:
/// - Semantic (384D) + Temporal (3x) + Causal + Code + Graph + HDC + Multimodal
/// - Entity + LateInteraction + SPLADE + Sparse
/// - Total ~18-20GB loaded, leaving headroom for activations
///
/// # Returns
///
/// `Arc<dyn MultiArrayEmbeddingProvider>` - Cloned reference to the cached provider
///
/// # Panics
///
/// Panics if:
/// - CUDA GPU not available
/// - Models directory missing or incomplete
/// - GPU OOM (should NOT happen with 32GB VRAM)
#[cfg(feature = "cuda")]
async fn get_warm_loaded_provider() -> Arc<dyn MultiArrayEmbeddingProvider> {
    WARM_MODEL_CACHE
        .get_or_init(|| async {
            let models_dir = resolve_test_models_path();
            tracing::info!(
                "WARM LOAD: Initializing global embedding provider from {:?}",
                models_dir
            );
            tracing::info!("WARM LOAD: This loads 13 models ONCE, shared across all tests");

            let provider =
                ProductionMultiArrayProvider::new(models_dir.clone(), GpuConfig::default())
                    .await
                    .unwrap_or_else(|_| {
                        panic!(
                            "WARM LOAD FAILED: Could not create ProductionMultiArrayProvider. \
                     Ensure models exist at {:?} and RTX 5090 GPU is available with CUDA.",
                            models_dir
                        )
                    });

            tracing::info!("WARM LOAD: All 13 embedding models loaded into VRAM successfully");
            Arc::new(provider) as Arc<dyn MultiArrayEmbeddingProvider>
        })
        .await
        .clone()
}

use crate::adapters::UtlProcessorAdapter;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcRequest};

// ============================================================================
// MCP Response Parsing Helpers
// ============================================================================

/// Extract parsed data from MCP tool response.
///
/// MCP tool responses wrap data in: `{ "content": [{ "type": "text", "text": "{...json...}" }] }`
/// This helper extracts and parses the inner JSON from the text field.
///
/// # Arguments
///
/// * `result` - The `result` field from JsonRpcResponse
///
/// # Returns
///
/// Parsed JSON value from content[0].text
///
/// # Panics
///
/// Panics if:
/// - result doesn't have `content` array
/// - content[0] doesn't have `text` field
/// - text field isn't valid JSON
pub(crate) fn extract_mcp_tool_data(result: &serde_json::Value) -> serde_json::Value {
    // Check if this is an error response (MCP format)
    if let Some(is_error) = result.get("isError").and_then(|v| v.as_bool()) {
        if is_error {
            let error_text = result
                .get("content")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("Unknown error");
            panic!("MCP tool returned error: {}", error_text);
        }
    }

    // Check if result has MCP content wrapper format: { "content": [{ "text": "..." }] }
    if let Some(content) = result.get("content").and_then(|v| v.as_array()) {
        // MCP wrapped format - extract from content[0].text
        let text = content[0]
            .get("text")
            .and_then(|v| v.as_str())
            .expect("content[0] must have text field");
        serde_json::from_str(text).expect("text field must be valid JSON")
    } else {
        // Direct result format - data is already unwrapped
        // This happens when handler returns data directly via JsonRpcResponse::success
        result.clone()
    }
}

/// Create test handlers with real stub implementations (no mocks).
///
/// TASK-S001: Uses TeleologicalMemoryStore and MultiArrayEmbeddingProvider.
/// TASK-GAP-001: Updated to use Handlers::with_all() after PRD v6 refactor.
/// NO legacy MemoryStore support.
pub(crate) fn create_test_handlers() -> Handlers {
    let teleological_store: Arc<dyn TeleologicalMemoryStore> =
        Arc::new(InMemoryTeleologicalStore::new());
    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(StubUtlProcessor::new());
    let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> =
        Arc::new(StubMultiArrayProvider::new());
    let goal_hierarchy = create_test_hierarchy();
    let layer_status_provider: Arc<dyn LayerStatusProvider> = Arc::new(StubLayerStatusProvider);
    Handlers::with_all(
        teleological_store,
        utl_processor,
        multi_array_provider,
        Arc::new(RwLock::new(goal_hierarchy)),
        layer_status_provider,
    )
}

/// Create test handlers WITHOUT top-level goals (for testing error cases).
pub(crate) fn create_test_handlers_no_goals() -> Handlers {
    let teleological_store: Arc<dyn TeleologicalMemoryStore> =
        Arc::new(InMemoryTeleologicalStore::new());
    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(StubUtlProcessor::new());
    let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> =
        Arc::new(StubMultiArrayProvider::new());
    let goal_hierarchy = GoalHierarchy::new(); // Empty hierarchy
    let layer_status_provider: Arc<dyn LayerStatusProvider> = Arc::new(StubLayerStatusProvider);
    Handlers::with_all(
        teleological_store,
        utl_processor,
        multi_array_provider,
        Arc::new(RwLock::new(goal_hierarchy)),
        layer_status_provider,
    )
}

// ============================================================================
// Real RocksDB Test Helpers (Integration Testing)
// ============================================================================

/// Create test handlers with REAL RocksDbTeleologicalStore for integration testing.
///
/// Uses `tempfile::TempDir` for automatic cleanup after tests complete.
/// The returned TempDir MUST be kept alive for the duration of the test -
/// dropping it deletes the database directory.
///
/// # Returns
///
/// `(Handlers, TempDir)` - The Handlers instance and the TempDir that owns the database.
///
/// # Components
///
/// - **Storage**: RocksDbTeleologicalStore (17 column families, real persistence)
/// - **UTL**: UtlProcessorAdapter with defaults (real UTL computation)
/// - **Embeddings**: StubMultiArrayProvider (until GPU embedding ready - FAIL FAST on embed ops)
/// - **Alignment**: DefaultAlignmentCalculator (real cosine similarity)
/// - **Goals**: Test hierarchy with strategic goals
///
/// # HNSW Initialization
///
/// This function **MUST** be async to initialize HNSW indexes before use.
/// Without HNSW initialization, store operations will fail with "Index not initialized" errors.
///
/// # Example
///
/// ```ignore
/// #[tokio::test]
/// async fn test_with_real_storage() {
///     let (handlers, _tempdir) = create_test_handlers_with_rocksdb().await;
///     // _tempdir keeps the database alive until end of test
///
///     // Store and retrieve operations use real RocksDB with initialized HNSW
///     let result = handlers.handle_memory_store(...).await;
///     assert!(result.is_ok());
/// }
/// ```
///
/// # Panics
///
/// Panics if TempDir creation, RocksDB opening, or HNSW initialization fails.
/// This is intentional - tests should fail immediately if infrastructure cannot be set up.
pub(crate) async fn create_test_handlers_with_rocksdb() -> (Handlers, TempDir) {
    let tempdir = TempDir::new().expect("Failed to create temp directory for RocksDB test");
    let db_path = tempdir.path().join("test_rocksdb");

    // Open RocksDB store
    let rocksdb_store = RocksDbTeleologicalStore::open(&db_path)
        .expect("Failed to open RocksDbTeleologicalStore in test");

    // Note: EmbedderIndexRegistry is initialized in constructor

    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(rocksdb_store);

    // Use real UTL processor adapter for live computation
    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(UtlProcessorAdapter::with_defaults());

    // Still use StubMultiArrayProvider until GPU embedding is ready
    // This will FAIL FAST on actual embedding operations - tests must not rely on embeddings
    let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> =
        Arc::new(StubMultiArrayProvider::new());

    let goal_hierarchy = create_test_hierarchy();
    let layer_status_provider: Arc<dyn LayerStatusProvider> = Arc::new(StubLayerStatusProvider);

    let handlers = Handlers::with_all(
        teleological_store,
        utl_processor,
        multi_array_provider,
        Arc::new(RwLock::new(goal_hierarchy)),
        layer_status_provider,
    );

    (handlers, tempdir)
}

/// Create test handlers with REAL RocksDbTeleologicalStore but NO top-level goals.
///
/// Same as `create_test_handlers_with_rocksdb()` but with an empty goal hierarchy.
/// Use this for testing error cases where Strategic goals are required but missing.
///
/// # Returns
///
/// `(Handlers, TempDir)` - The Handlers instance and the TempDir that owns the database.
///
/// # Example
///
/// ```ignore
/// #[tokio::test]
/// async fn test_missing_strategic_goal_error() {
///     let (handlers, _tempdir) = create_test_handlers_with_rocksdb_no_goals().await;
///
///     // Should fail because no Strategic goal is configured
///     let result = handlers.handle_purpose_align(...).await;
///     assert!(result.error.is_some());
/// }
/// ```
#[allow(dead_code)]
pub(crate) async fn create_test_handlers_with_rocksdb_no_goals() -> (Handlers, TempDir) {
    let tempdir = TempDir::new().expect("Failed to create temp directory for RocksDB test");
    let db_path = tempdir.path().join("test_rocksdb");

    // Open RocksDB store
    let rocksdb_store = RocksDbTeleologicalStore::open(&db_path)
        .expect("Failed to open RocksDbTeleologicalStore in test");

    // Note: EmbedderIndexRegistry is initialized in constructor

    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(rocksdb_store);

    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(UtlProcessorAdapter::with_defaults());
    let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> =
        Arc::new(StubMultiArrayProvider::new());
    let goal_hierarchy = GoalHierarchy::new(); // Empty hierarchy - no Strategic goals
    let layer_status_provider: Arc<dyn LayerStatusProvider> = Arc::new(StubLayerStatusProvider);

    let handlers = Handlers::with_all(
        teleological_store,
        utl_processor,
        multi_array_provider,
        Arc::new(RwLock::new(goal_hierarchy)),
        layer_status_provider,
    );

    (handlers, tempdir)
}

/// Create a test goal hierarchy with 3 levels.
/// TASK-P0-001: Updated for 3-level hierarchy (Strategic → Tactical → Immediate)
///
/// Hierarchy:
/// - Strategic: "Build the best ML learning system"
///   - Tactical: "Implement semantic search"
///     - Immediate: "Add vector similarity"
/// - Strategic: "Enhance user experience"
pub(crate) fn create_test_hierarchy() -> GoalHierarchy {
    let mut hierarchy = GoalHierarchy::new();

    // Create test discovery metadata for autonomous goals
    let discovery = GoalDiscoveryMetadata::bootstrap();

    // Strategic goal 1 (top-level, no parent)
    let s1_goal = GoalNode::autonomous_goal(
        "Build the best ML learning system".into(),
        GoalLevel::Strategic,
        SemanticFingerprint::zeroed(),
        discovery.clone(),
    )
    .expect("Failed to create Strategic goal 1");
    let s1_id = s1_goal.id;
    hierarchy
        .add_goal(s1_goal)
        .expect("Failed to add Strategic goal 1");

    // Strategic goal 2 (top-level, no parent)
    let s2_goal = GoalNode::autonomous_goal(
        "Enhance user experience".into(),
        GoalLevel::Strategic,
        SemanticFingerprint::zeroed(),
        discovery.clone(),
    )
    .expect("Failed to create Strategic goal 2");
    hierarchy
        .add_goal(s2_goal)
        .expect("Failed to add Strategic goal 2");

    // Tactical goal - child of Strategic goal 1
    let t1_goal = GoalNode::child_goal(
        "Implement semantic search".into(),
        GoalLevel::Tactical,
        s1_id,
        SemanticFingerprint::zeroed(),
        discovery.clone(),
    )
    .expect("Failed to create tactical goal");
    let t1_id = t1_goal.id;
    hierarchy
        .add_goal(t1_goal)
        .expect("Failed to add tactical goal");

    // Immediate goal - child of Tactical goal
    let i1_goal = GoalNode::child_goal(
        "Add vector similarity".into(),
        GoalLevel::Immediate,
        t1_id,
        SemanticFingerprint::zeroed(),
        discovery,
    )
    .expect("Failed to create immediate goal");
    hierarchy
        .add_goal(i1_goal)
        .expect("Failed to add immediate goal");

    hierarchy
}

/// Create test handlers with REAL RocksDbTeleologicalStore and EXPOSED store reference.
///
/// This is for Full State Verification tests that need to directly inspect
/// the underlying store to verify data was actually persisted.
///
/// # Returns
///
/// `(Handlers, Arc<dyn TeleologicalMemoryStore>, TempDir)`
/// - Handlers instance for dispatching MCP requests
/// - Direct reference to the store for FSV assertions
/// - TempDir that MUST be kept alive for the duration of the test
///
/// # Example
///
/// ```ignore
/// #[tokio::test]
/// async fn test_fsv_store_verification() {
///     let (handlers, store, _tempdir) = create_test_handlers_with_rocksdb_store_access().await;
///
///     // Store via MCP handler
///     let response = handlers.dispatch(store_request).await;
///
///     // Verify directly in store (FSV)
///     let count = store.count().await.expect("count works");
///     assert_eq!(count, 1, "Must have stored 1 fingerprint");
/// }
/// ```
pub(crate) async fn create_test_handlers_with_rocksdb_store_access(
) -> (Handlers, Arc<dyn TeleologicalMemoryStore>, TempDir) {
    let tempdir = TempDir::new().expect("Failed to create temp directory for RocksDB test");
    let db_path = tempdir.path().join("test_rocksdb_fsv");

    // Open RocksDB store
    let rocksdb_store = RocksDbTeleologicalStore::open(&db_path)
        .expect("Failed to open RocksDbTeleologicalStore in FSV test");

    // Note: EmbedderIndexRegistry is initialized in constructor

    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(rocksdb_store);

    // Use real UTL processor adapter for live computation
    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(UtlProcessorAdapter::with_defaults());

    // Still use StubMultiArrayProvider until GPU embedding is ready
    // This will FAIL FAST on actual embedding operations - tests must not rely on embeddings
    let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> =
        Arc::new(StubMultiArrayProvider::new());

    let goal_hierarchy = create_test_hierarchy();
    let layer_status_provider: Arc<dyn LayerStatusProvider> = Arc::new(StubLayerStatusProvider);

    let handlers = Handlers::with_all(
        Arc::clone(&teleological_store),
        utl_processor,
        multi_array_provider,
        Arc::new(RwLock::new(goal_hierarchy)),
        layer_status_provider,
    );

    (handlers, teleological_store, tempdir)
}

// ============================================================================
// TASK-P3-01: Real GPU Embedding Test Helpers (FSV Integration Testing)
// ============================================================================

/// Create test handlers with REAL ProductionMultiArrayProvider for Full State Verification.
///
/// This helper is feature-gated behind `cuda` feature and requires:
/// - NVIDIA CUDA GPU with 8GB+ VRAM
/// - Models pre-downloaded to `./models` directory
/// - CUDA toolkit installed and configured
///
/// # When to Use
///
/// Use this helper ONLY for Full State Verification (FSV) tests that need to verify:
/// - Actual embedding generation produces correct dimensions
/// - Real semantic similarity computations
/// - GPU acceleration performance characteristics
/// - End-to-end embedding pipeline correctness
///
/// For fast unit tests that don't need real embeddings, use:
/// - `create_test_handlers()` - Fast in-memory with stubs
/// - `create_test_handlers_with_rocksdb()` - Real storage, stub embeddings
///
/// # Returns
///
/// `(Handlers, TempDir)` - The Handlers instance and the TempDir that owns the database.
///
/// # Panics
///
/// Panics if:
/// - CUDA GPU is not available
/// - Models directory doesn't exist or is missing models
/// - RocksDB fails to open
/// - HNSW initialization fails
///
/// # Example
///
/// ```ignore
/// #[tokio::test]
/// #[cfg(feature = "cuda")]
/// async fn test_fsv_with_real_embeddings() {
///     let (handlers, _tempdir) = create_test_handlers_with_real_embeddings().await;
///
///     // This test uses REAL GPU embeddings
///     let response = handlers.dispatch(store_request).await;
///
///     // Verify embeddings have correct dimensions
///     assert!(response.result.is_some());
/// }
/// ```
#[cfg(feature = "cuda")]
pub(crate) async fn create_test_handlers_with_real_embeddings() -> (Handlers, TempDir) {
    let tempdir = TempDir::new().expect("Failed to create temp directory for RocksDB FSV test");
    let db_path = tempdir.path().join("test_rocksdb_fsv_real_embeddings");

    // Open RocksDB store
    let rocksdb_store = RocksDbTeleologicalStore::open(&db_path)
        .expect("Failed to open RocksDbTeleologicalStore in FSV test with real embeddings");

    // Note: EmbedderIndexRegistry is initialized in constructor

    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(rocksdb_store);

    // Use real UTL processor adapter for live computation
    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(UtlProcessorAdapter::with_defaults());

    // TASK-WARM-LOAD: Use WARM-LOADED embedding provider from global cache
    // RTX 5090 32GB - models loaded ONCE, shared across all tests
    // This prevents CUDA OOM when tests run in parallel
    let multi_array_provider = get_warm_loaded_provider().await;

    let goal_hierarchy = create_test_hierarchy();
    let layer_status_provider: Arc<dyn LayerStatusProvider> = Arc::new(StubLayerStatusProvider);

    let handlers = Handlers::with_all(
        teleological_store,
        utl_processor,
        multi_array_provider,
        Arc::new(RwLock::new(goal_hierarchy)),
        layer_status_provider,
    );

    (handlers, tempdir)
}

/// Create test handlers with REAL embeddings and exposed store reference for FSV assertions.
///
/// Same as `create_test_handlers_with_real_embeddings()` but also returns a direct
/// reference to the teleological store for Full State Verification assertions.
///
/// # Returns
///
/// `(Handlers, Arc<dyn TeleologicalMemoryStore>, TempDir)`
/// - Handlers instance for dispatching MCP requests
/// - Direct reference to the store for FSV assertions (verify data was persisted)
/// - TempDir that MUST be kept alive for the duration of the test
#[cfg(feature = "cuda")]
#[allow(dead_code)]
pub(crate) async fn create_test_handlers_with_real_embeddings_store_access(
) -> (Handlers, Arc<dyn TeleologicalMemoryStore>, TempDir) {
    let tempdir = TempDir::new().expect("Failed to create temp directory for FSV test");
    let db_path = tempdir
        .path()
        .join("test_rocksdb_fsv_real_embeddings_store");

    // Open RocksDB store
    let rocksdb_store = RocksDbTeleologicalStore::open(&db_path)
        .expect("Failed to open RocksDbTeleologicalStore in FSV test");

    // Note: EmbedderIndexRegistry is initialized in constructor

    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(rocksdb_store);

    // Use real UTL processor adapter
    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(UtlProcessorAdapter::with_defaults());

    // TASK-WARM-LOAD: Use WARM-LOADED embedding provider from global cache
    // RTX 5090 32GB - models loaded ONCE, shared across all tests
    let multi_array_provider = get_warm_loaded_provider().await;

    let goal_hierarchy = create_test_hierarchy();
    let layer_status_provider: Arc<dyn LayerStatusProvider> = Arc::new(StubLayerStatusProvider);

    let handlers = Handlers::with_all(
        Arc::clone(&teleological_store),
        utl_processor,
        multi_array_provider,
        Arc::new(RwLock::new(goal_hierarchy)),
        layer_status_provider,
    );

    (handlers, teleological_store, tempdir)
}

/// Resolve models directory for tests.
///
/// Priority:
/// 1. `CONTEXT_GRAPH_MODELS_PATH` environment variable
/// 2. Default: `./models` relative to workspace root
#[cfg(feature = "cuda")]
fn resolve_test_models_path() -> PathBuf {
    if let Ok(env_path) = std::env::var("CONTEXT_GRAPH_MODELS_PATH") {
        return PathBuf::from(env_path);
    }
    // Navigate from crate directory to workspace root
    // CARGO_MANIFEST_DIR = crates/context-graph-mcp
    // Workspace root = ../../ from there
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));

    // Go up to workspace root (crates/context-graph-mcp -> crates -> root)
    let workspace_root = manifest_dir
        .parent() // -> crates/
        .and_then(|p| p.parent()) // -> workspace root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    workspace_root.join("models")
}

/// Create a JSON-RPC request for testing.
pub(crate) fn make_request(
    method: &str,
    id: Option<JsonRpcId>,
    params: Option<serde_json::Value>,
) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id,
        method: method.to_string(),
        params,
    }
}

// ============================================================================
// TASK-GAP-001: Removed dead GWT/MetaUtl code (deleted in commit fab0622)
// ============================================================================
// The following functions were removed as they referenced deleted modules:
// - create_test_handlers_with_warm_gwt()
// - create_test_handlers_with_warm_gwt_rocksdb()
// - create_test_handlers_with_all_components()
//
// These functions used:
// - crate::handlers::core::MetaUtlTracker (deleted)
// - crate::handlers::gwt_providers (deleted)
// - crate::handlers::gwt_traits (deleted)
// - Handlers::with_gwt() (deleted)
// - Handlers::with_gwt_and_subsystems() (deleted)
//
// When GWT/MetaUtl is reimplemented per PRD v6, new test helpers will be added.
