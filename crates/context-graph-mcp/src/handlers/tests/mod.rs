//! MCP Protocol Compliance Tests
//!
//! Default protocol tests use REAL RocksDB storage plus a fail-closed embedding
//! provider that returns an explicit error if a protocol-only test accidentally
//! reaches an embedding path. Tests that verify memory/search embedding behavior
//! use `create_test_handlers_with_real_embeddings()` and are explicit integration
//! tests requiring production model assets.
//!
//! Tests verify compliance with MCP protocol version 2024-11-05
//! Reference: https://spec.modelcontextprotocol.io/specification/2024-11-05/
//!
//! # Test Helpers
//!
//! - `create_protocol_test_handlers()` - Real RocksDB + fail-closed embeddings
//! - `create_test_handlers()` - Real RocksDB + real GPU embeddings
//! - `create_test_handlers_with_real_embeddings()` - Alias for create_test_handlers()
//!
//! # TempDir Lifecycle
//!
//! All helpers return `(Handlers, TempDir)`. The TempDir MUST be kept alive
//! for the duration of the test - dropping it deletes the database directory.
//!
//! ```ignore
//! #[tokio::test]
//! async fn test_example() {
//!     let (handlers, _tempdir) = create_test_handlers().await;
//!     // _tempdir keeps the database alive until end of test
//! }
//! ```

mod dynamicjepa_tools;
mod error_codes;
mod initialize;
mod search_periodic_test;
mod tcp_transport_integration;
mod tools_call;
mod tools_list;
mod topic_tools;

use std::sync::Arc;

use async_trait::async_trait;
use tempfile::TempDir;

use context_graph_core::monitoring::{HardcodedActiveLayerStatusProvider, LayerStatusProvider};
use context_graph_core::traits::{
    EmbeddingMetadata, MultiArrayEmbeddingOutput, MultiArrayEmbeddingProvider,
    TeleologicalMemoryStore,
};
use context_graph_core::types::fingerprint::NUM_EMBEDDERS;
use context_graph_core::{CoreError, CoreResult};
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use context_graph_storage::{BackgroundGraphBuilder, EdgeRepository, GraphBuilderConfig};

use context_graph_embeddings::{
    get_warm_provider, initialize_global_warm_provider, is_warm_initialized, warm_status_message,
    GpuConfig, ProductionMultiArrayProvider,
};

use std::path::PathBuf;

use tokio::sync::OnceCell;

/// Global warm-loaded model cache.
///
/// RTX 5090 32GB VRAM - models are warm-loaded ONCE and shared across ALL tests.
/// This prevents CUDA OOM when tests run in parallel, each trying to load
/// all 14 embedding models (~20GB total) from scratch.
///
/// FAIL FAST: If initial load fails, ALL tests will fail - no stubs, no fallbacks.
static WARM_MODEL_CACHE: OnceCell<Arc<dyn MultiArrayEmbeddingProvider>> = OnceCell::const_new();

/// Get or initialize the warm-loaded embedding provider.
///
/// Uses the global warm provider singleton from global_provider.rs.
/// Models are loaded exactly ONCE into GPU VRAM and shared across ALL tests.
///
/// # Panics
///
/// Panics if CUDA GPU not available, models directory missing, or GPU OOM.
async fn get_warm_loaded_provider() -> Arc<dyn MultiArrayEmbeddingProvider> {
    WARM_MODEL_CACHE
        .get_or_init(|| async {
            // TASK-EMB-016: First try to use global warm provider singleton
            if is_warm_initialized() {
                tracing::info!(
                    "WARM LOAD: Using existing global warm provider (already initialized)"
                );
                match get_warm_provider() {
                    Ok(provider) => {
                        tracing::info!(
                            "WARM LOAD: Retrieved global warm provider successfully"
                        );
                        return provider;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "WARM LOAD: Failed to get global warm provider: {}. Falling back to direct load.",
                            e
                        );
                    }
                }
            }

            // Try to initialize the global warm provider
            tracing::info!(
                "WARM LOAD: Attempting to initialize global warm provider..."
            );
            match initialize_global_warm_provider().await {
                Ok(()) => {
                    tracing::info!(
                        "WARM LOAD: Global warm provider initialized successfully"
                    );
                    match get_warm_provider() {
                        Ok(provider) => {
                            tracing::info!(
                                "WARM LOAD: All 14 embedding models ready via global warm provider"
                            );
                            return provider;
                        }
                        Err(e) => {
                            tracing::warn!(
                                "WARM LOAD: Global warm provider init succeeded but get failed: {}. Status: {}",
                                e,
                                warm_status_message()
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "WARM LOAD: Global warm provider initialization failed: {}. Falling back to ProductionMultiArrayProvider.",
                        e
                    );
                }
            }

            // Fall back to direct ProductionMultiArrayProvider if global warm provider fails
            let models_dir = resolve_test_models_path();
            tracing::info!(
                "WARM LOAD: Falling back to direct ProductionMultiArrayProvider from {:?}",
                models_dir
            );

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

            tracing::info!("WARM LOAD: All 14 embedding models loaded into VRAM successfully (fallback)");
            Arc::new(provider) as Arc<dyn MultiArrayEmbeddingProvider>
        })
        .await
        .clone()
}

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcRequest};

// ============================================================================
// Shared Read-Only Handlers for Protocol Tests
// ============================================================================
//
// PERF: Tests that exercise only MCP protocol dispatch (initialize, tools/list,
// error codes, etc.) don't mutate RocksDB state, so they can share ONE handler
// instance across all tests. This amortizes the RocksDB open from N opens to
// exactly 1 and deliberately avoids unrelated production model loading.
//
// Tests that WRITE to storage (store_memory, merge, delete, forget, etc.) MUST
// continue to use an isolated handler for source-of-truth verification.

static SHARED_READONLY_HANDLERS: OnceCell<(Arc<Handlers>, Arc<TempDir>)> = OnceCell::const_new();

/// Get a shared read-only Handlers instance for protocol-only tests.
///
/// ALL callers share the same Handlers, RocksDB, fail-closed embedding provider,
/// and TempDir.
/// This is ONLY safe for tests that do not persistently mutate storage state.
///
/// Returns `Arc<Handlers>` so the caller can `.dispatch()` without ownership juggling.
///
/// # When to use this
///
/// - Tests that only call `initialize`, `notifications/initialized`, `tools/list`
/// - Tests that verify error codes for unknown methods / unknown tools / bad params
/// - Tests that validate request ID echoing
///
/// # When NOT to use this
///
/// - Tests that write memories (`store_memory`, `merge_memories`, etc.)
/// - Tests that inspect RocksDB state after writes
/// - Tests that directly inspect RocksDB state after writes
pub(crate) async fn shared_readonly_handlers() -> Arc<Handlers> {
    let (handlers, _tempdir) = SHARED_READONLY_HANDLERS
        .get_or_init(|| async {
            let (handlers, tempdir) = create_protocol_test_handlers().await;
            (Arc::new(handlers), Arc::new(tempdir))
        })
        .await;
    Arc::clone(handlers)
}

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

/// Embedding provider for protocol-only tests.
///
/// This is not a fake success path: every embedding attempt returns a structured
/// error. Protocol tests use it so they fail loudly if they cross into memory
/// graph behavior that requires production model assets.
struct FailClosedEmbeddingProvider {
    reason: &'static str,
}

impl FailClosedEmbeddingProvider {
    fn new(reason: &'static str) -> Self {
        Self { reason }
    }

    fn error(&self, operation: &str) -> CoreError {
        CoreError::FeatureDisabled {
            feature: format!(
                "{operation}: production embeddings are not configured for this protocol-only test harness; {}",
                self.reason
            ),
        }
    }
}

#[async_trait]
impl MultiArrayEmbeddingProvider for FailClosedEmbeddingProvider {
    async fn embed_all(&self, content: &str) -> CoreResult<MultiArrayEmbeddingOutput> {
        tracing::error!(
            content_len = content.len(),
            "protocol-only test attempted embed_all without production models"
        );
        Err(self.error("embed_all"))
    }

    async fn embed_batch_all(
        &self,
        contents: &[String],
        _metadata: &[EmbeddingMetadata],
    ) -> CoreResult<Vec<MultiArrayEmbeddingOutput>> {
        tracing::error!(
            batch_len = contents.len(),
            "protocol-only test attempted embed_batch_all without production models"
        );
        Err(self.error("embed_batch_all"))
    }

    fn model_ids(&self) -> [&str; NUM_EMBEDDERS] {
        ["embedding-disabled-for-protocol-tests"; NUM_EMBEDDERS]
    }

    fn is_ready(&self) -> bool {
        false
    }

    fn health_status(&self) -> [bool; NUM_EMBEDDERS] {
        [false; NUM_EMBEDDERS]
    }
}

/// Create protocol test handlers with REAL RocksDB storage and fail-closed embeddings.
///
/// Use this for tests whose source of truth is JSON-RPC/MCP protocol behavior,
/// tool schema definitions, validation, or DynamicJEPA databases selected by an
/// explicit `dbPath`. If the tested code tries to embed content, the provider
/// returns a precise error instead of manufacturing vectors.
pub(crate) async fn create_protocol_test_handlers() -> (Handlers, TempDir) {
    let tempdir =
        TempDir::new().expect("Failed to create temp directory for protocol RocksDB test");
    let db_path = tempdir.path().join("test_protocol_rocksdb");

    let rocksdb_store = RocksDbTeleologicalStore::open(&db_path)
        .expect("Failed to open RocksDbTeleologicalStore in protocol test");
    let (handlers, _) = create_protocol_handlers_from_store(rocksdb_store).await;

    (handlers, tempdir)
}

pub(crate) async fn create_protocol_handlers_from_store(
    rocksdb_store: RocksDbTeleologicalStore,
) -> (Handlers, Arc<dyn TeleologicalMemoryStore>) {
    create_handlers_from_store_with_provider(
        rocksdb_store,
        Arc::new(FailClosedEmbeddingProvider::new(
            "use create_test_handlers_with_real_embeddings() for memory/search integration tests",
        )),
    )
    .await
}

/// Create test handlers with REAL RocksDB storage and REAL GPU embeddings.
///
/// ALL tests use real implementations - no stubs, no mocks, no workarounds.
/// Requires CUDA GPU with models loaded into VRAM via warm provider.
///
/// # Returns
///
/// `(Handlers, TempDir)` - The Handlers instance and TempDir that owns the database.
/// The TempDir MUST be kept alive for the duration of the test.
///
/// # Panics
///
/// Panics if CUDA GPU not available, models not loaded, or RocksDB fails to open.
pub(crate) async fn create_test_handlers() -> (Handlers, TempDir) {
    let tempdir = TempDir::new().expect("Failed to create temp directory for RocksDB test");
    let db_path = tempdir.path().join("test_rocksdb");

    let rocksdb_store = RocksDbTeleologicalStore::open(&db_path)
        .expect("Failed to open RocksDbTeleologicalStore in test");
    let (handlers, _) = create_handlers_from_store(rocksdb_store).await;

    (handlers, tempdir)
}

// ============================================================================
// Real GPU Embedding Test Helpers (FSV Integration Testing)
// ============================================================================

/// Create test handlers with REAL RocksDB + REAL GPU embeddings.
///
/// Uses the global warm-loaded embedding provider for fast initialization.
///
/// # Returns
///
/// `(Handlers, TempDir)` - The Handlers instance and TempDir that owns the database.
/// The TempDir MUST be kept alive for the duration of the test.
pub(crate) async fn create_test_handlers_with_real_embeddings() -> (Handlers, TempDir) {
    let tempdir = TempDir::new().expect("Failed to create temp directory for RocksDB FSV test");
    let db_path = tempdir.path().join("test_rocksdb_fsv_real_embeddings");

    // Open RocksDB store
    let rocksdb_store = RocksDbTeleologicalStore::open(&db_path)
        .expect("Failed to open RocksDbTeleologicalStore in FSV test with real embeddings");

    let (handlers, _) = create_handlers_from_store(rocksdb_store).await;

    (handlers, tempdir)
}

async fn create_handlers_from_store(
    rocksdb_store: RocksDbTeleologicalStore,
) -> (Handlers, Arc<dyn TeleologicalMemoryStore>) {
    // TASK-WARM-LOAD: Use WARM-LOADED embedding provider from global cache.
    // RTX 5090 32GB - models loaded ONCE, shared across real integration tests.
    let multi_array_provider = get_warm_loaded_provider().await;
    create_handlers_from_store_with_provider(rocksdb_store, multi_array_provider).await
}

async fn create_handlers_from_store_with_provider(
    rocksdb_store: RocksDbTeleologicalStore,
    multi_array_provider: Arc<dyn MultiArrayEmbeddingProvider>,
) -> (Handlers, Arc<dyn TeleologicalMemoryStore>) {
    let db_arc = rocksdb_store.db_arc();

    let teleological_store: Arc<dyn TeleologicalMemoryStore> = Arc::new(rocksdb_store);

    let layer_status_provider: Arc<dyn LayerStatusProvider> =
        Arc::new(HardcodedActiveLayerStatusProvider);

    let handlers = {
        let edge_repository = EdgeRepository::new(db_arc);
        let graph_builder = Arc::new(BackgroundGraphBuilder::new(
            edge_repository.clone(),
            Arc::clone(&teleological_store),
            Arc::clone(&multi_array_provider),
            GraphBuilderConfig::default(),
        ));

        Handlers::without_llm(
            Arc::clone(&teleological_store),
            multi_array_provider,
            layer_status_provider,
            edge_repository,
            graph_builder,
            Arc::new(context_graph_embeddings::provider::NoOpCausalHintProvider),
        )
        .expect("Default cluster manager should always succeed in tests")
    };

    (handlers, teleological_store)
}

/// Resolve models directory for tests.
///
/// Priority:
/// 1. `CONTEXT_GRAPH_MODELS_PATH` environment variable
/// 2. Default: `./models` relative to workspace root
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
// TASK-GAP-001: Removed obsolete test helper code
// ============================================================================
// Legacy test helpers were removed as they referenced deleted modules.
// Current tests use the simplified handler construction per constitution v6.
