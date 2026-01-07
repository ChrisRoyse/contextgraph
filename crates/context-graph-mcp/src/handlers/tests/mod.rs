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
//! This module provides two categories of test helpers:
//!
//! ## Fast In-Memory Helpers (Unit Tests)
//! - `create_test_handlers()` - Uses InMemoryTeleologicalStore with stubs
//! - `create_test_handlers_no_north_star()` - Same but with empty goal hierarchy
//!
//! ## Real Storage Helpers (Integration Tests)
//! - `create_test_handlers_with_rocksdb()` - Uses RocksDbTeleologicalStore with tempdir
//! - `create_test_handlers_with_rocksdb_no_north_star()` - Same but with empty goal hierarchy
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
mod error_codes;
mod full_state_verification;
mod full_state_verification_johari;
mod full_state_verification_meta_utl;
mod full_state_verification_search;
mod full_state_verification_purpose;
mod initialize;
mod integration_e2e;
mod manual_fsv_purpose;
mod manual_fsv_verification;
mod memory;
mod meta_cognitive;
mod purpose;
mod search;
mod task_emb_024_verification;
mod tools_list;
mod tools_call;
mod utl;

use std::sync::Arc;

use tempfile::TempDir;

use context_graph_core::alignment::{DefaultAlignmentCalculator, GoalAlignmentCalculator};
use context_graph_core::purpose::{GoalHierarchy, GoalId, GoalLevel, GoalNode};
use context_graph_core::stubs::{InMemoryTeleologicalStore, StubMultiArrayProvider, StubUtlProcessor};
use context_graph_core::traits::{MultiArrayEmbeddingProvider, TeleologicalMemoryStore, UtlProcessor};
use context_graph_storage::teleological::RocksDbTeleologicalStore;

use crate::adapters::UtlProcessorAdapter;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcRequest};

/// Create test handlers with real stub implementations (no mocks).
///
/// TASK-S001: Uses TeleologicalMemoryStore and MultiArrayEmbeddingProvider.
/// TASK-S003: Uses DefaultAlignmentCalculator and test GoalHierarchy.
/// NO legacy MemoryStore support.
pub(crate) fn create_test_handlers() -> Handlers {
    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(InMemoryTeleologicalStore::new());
    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(StubUtlProcessor::new());
    let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> = Arc::new(StubMultiArrayProvider::new());
    let alignment_calculator: Arc<dyn GoalAlignmentCalculator> = Arc::new(DefaultAlignmentCalculator::new());
    let goal_hierarchy = create_test_hierarchy();
    Handlers::new(teleological_store, utl_processor, multi_array_provider, alignment_calculator, goal_hierarchy)
}

/// Create test handlers WITHOUT a North Star goal (for testing error cases).
pub(crate) fn create_test_handlers_no_north_star() -> Handlers {
    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(InMemoryTeleologicalStore::new());
    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(StubUtlProcessor::new());
    let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> = Arc::new(StubMultiArrayProvider::new());
    let alignment_calculator: Arc<dyn GoalAlignmentCalculator> = Arc::new(DefaultAlignmentCalculator::new());
    let goal_hierarchy = GoalHierarchy::new(); // Empty hierarchy
    Handlers::new(teleological_store, utl_processor, multi_array_provider, alignment_calculator, goal_hierarchy)
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
/// - **Goals**: Test hierarchy with North Star
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

    // CRITICAL: Initialize HNSW indexes BEFORE wrapping in Arc<dyn>
    // Without this, store operations fail with "Index for E1Semantic not initialized"
    rocksdb_store
        .initialize_hnsw()
        .await
        .expect("Failed to initialize HNSW indexes in test");

    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(rocksdb_store);

    // Use real UTL processor adapter for live computation
    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(UtlProcessorAdapter::with_defaults());

    // Still use StubMultiArrayProvider until GPU embedding is ready
    // This will FAIL FAST on actual embedding operations - tests must not rely on embeddings
    let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> =
        Arc::new(StubMultiArrayProvider::new());

    let alignment_calculator: Arc<dyn GoalAlignmentCalculator> =
        Arc::new(DefaultAlignmentCalculator::new());
    let goal_hierarchy = create_test_hierarchy();

    let handlers = Handlers::new(
        teleological_store,
        utl_processor,
        multi_array_provider,
        alignment_calculator,
        goal_hierarchy,
    );

    (handlers, tempdir)
}

/// Create test handlers with REAL RocksDbTeleologicalStore but NO North Star goal.
///
/// Same as `create_test_handlers_with_rocksdb()` but with an empty goal hierarchy.
/// Use this for testing error cases where North Star is required but missing.
///
/// # Returns
///
/// `(Handlers, TempDir)` - The Handlers instance and the TempDir that owns the database.
///
/// # Example
///
/// ```ignore
/// #[tokio::test]
/// async fn test_missing_north_star_error() {
///     let (handlers, _tempdir) = create_test_handlers_with_rocksdb_no_north_star().await;
///
///     // Should fail because no North Star is configured
///     let result = handlers.handle_purpose_align(...).await;
///     assert!(result.error.is_some());
/// }
/// ```
pub(crate) async fn create_test_handlers_with_rocksdb_no_north_star() -> (Handlers, TempDir) {
    let tempdir = TempDir::new().expect("Failed to create temp directory for RocksDB test");
    let db_path = tempdir.path().join("test_rocksdb");

    // Open RocksDB store
    let rocksdb_store = RocksDbTeleologicalStore::open(&db_path)
        .expect("Failed to open RocksDbTeleologicalStore in test");

    // CRITICAL: Initialize HNSW indexes BEFORE wrapping in Arc<dyn>
    rocksdb_store
        .initialize_hnsw()
        .await
        .expect("Failed to initialize HNSW indexes in test");

    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(rocksdb_store);

    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(UtlProcessorAdapter::with_defaults());
    let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> =
        Arc::new(StubMultiArrayProvider::new());
    let alignment_calculator: Arc<dyn GoalAlignmentCalculator> =
        Arc::new(DefaultAlignmentCalculator::new());
    let goal_hierarchy = GoalHierarchy::new(); // Empty hierarchy - no North Star

    let handlers = Handlers::new(
        teleological_store,
        utl_processor,
        multi_array_provider,
        alignment_calculator,
        goal_hierarchy,
    );

    (handlers, tempdir)
}

/// Create a test goal hierarchy with North Star and sub-goals.
///
/// Hierarchy:
/// - NorthStar: "Build the best ML learning system"
///   - Strategic: "Improve retrieval accuracy"
///     - Tactical: "Implement semantic search"
///       - Immediate: "Add vector similarity"
///   - Strategic: "Enhance user experience"
pub(crate) fn create_test_hierarchy() -> GoalHierarchy {
    let mut hierarchy = GoalHierarchy::new();

    // Create embedding that varies by dimension for distinctiveness
    let ns_embedding: Vec<f32> = (0..1024)
        .map(|i| (i as f32 / 1024.0).sin() * 0.8)
        .collect();

    // North Star
    hierarchy
        .add_goal(GoalNode::north_star(
            "ns_ml_system",
            "Build the best ML learning system",
            ns_embedding.clone(),
            vec!["ml".into(), "learning".into(), "system".into()],
        ))
        .expect("Failed to add North Star");

    // Strategic goal 1
    hierarchy
        .add_goal(GoalNode::child(
            "s1_retrieval",
            "Improve retrieval accuracy",
            GoalLevel::Strategic,
            GoalId::new("ns_ml_system"),
            ns_embedding.clone(),
            0.8,
            vec!["retrieval".into(), "accuracy".into()],
        ))
        .expect("Failed to add strategic goal 1");

    // Strategic goal 2
    hierarchy
        .add_goal(GoalNode::child(
            "s2_ux",
            "Enhance user experience",
            GoalLevel::Strategic,
            GoalId::new("ns_ml_system"),
            ns_embedding.clone(),
            0.7,
            vec!["ux".into(), "user".into()],
        ))
        .expect("Failed to add strategic goal 2");

    // Tactical goal
    hierarchy
        .add_goal(GoalNode::child(
            "t1_semantic",
            "Implement semantic search",
            GoalLevel::Tactical,
            GoalId::new("s1_retrieval"),
            ns_embedding.clone(),
            0.6,
            vec!["semantic".into(), "search".into()],
        ))
        .expect("Failed to add tactical goal");

    // Immediate goal
    hierarchy
        .add_goal(GoalNode::child(
            "i1_vector",
            "Add vector similarity",
            GoalLevel::Immediate,
            GoalId::new("t1_semantic"),
            ns_embedding,
            0.5,
            vec!["vector".into(), "similarity".into()],
        ))
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
pub(crate) async fn create_test_handlers_with_rocksdb_store_access() -> (
    Handlers,
    Arc<dyn TeleologicalMemoryStore>,
    TempDir,
) {
    let tempdir = TempDir::new().expect("Failed to create temp directory for RocksDB test");
    let db_path = tempdir.path().join("test_rocksdb_fsv");

    // Open RocksDB store
    let rocksdb_store = RocksDbTeleologicalStore::open(&db_path)
        .expect("Failed to open RocksDbTeleologicalStore in FSV test");

    // CRITICAL: Initialize HNSW indexes BEFORE wrapping in Arc<dyn>
    rocksdb_store
        .initialize_hnsw()
        .await
        .expect("Failed to initialize HNSW indexes in FSV test");

    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(rocksdb_store);

    // Use real UTL processor adapter for live computation
    let utl_processor: Arc<dyn UtlProcessor> = Arc::new(UtlProcessorAdapter::with_defaults());

    // Still use StubMultiArrayProvider until GPU embedding is ready
    // This will FAIL FAST on actual embedding operations - tests must not rely on embeddings
    let multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider> =
        Arc::new(StubMultiArrayProvider::new());

    let alignment_calculator: Arc<dyn GoalAlignmentCalculator> =
        Arc::new(DefaultAlignmentCalculator::new());
    let goal_hierarchy = create_test_hierarchy();

    let handlers = Handlers::new(
        Arc::clone(&teleological_store),
        utl_processor,
        multi_array_provider,
        alignment_calculator,
        goal_hierarchy,
    );

    (handlers, teleological_store, tempdir)
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
