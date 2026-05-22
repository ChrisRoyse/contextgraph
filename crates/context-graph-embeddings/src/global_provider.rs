//! Global warm-loaded embedding provider singleton.
//!
//! CRITICAL: All code MUST use this singleton. Direct model loading is FORBIDDEN.
//!
//! # Design
//!
//! This module provides a global singleton that ensures:
//! 1. All 13 embedding models are loaded ONCE at startup into VRAM
//! 2. Tests and production code use the SAME warm models
//! 3. No cold loading overhead in tests or runtime
//!
//! # Usage
//!
//! ```rust,ignore
//! // At startup (MCP server, CLI, etc.)
//! initialize_global_warm_provider().await?;
//!
//! // Anywhere that needs embeddings
//! let provider = get_warm_provider()?;
//! let output = provider.embed_all("text").await?;
//! ```
//!
//! # Error Behavior
//!
//! - `NotInitialized`: Call `initialize_global_warm_provider()` first
//! - `ProviderBusy`: Provider lock contention, retry later
//! - `InitializationFailed`: Warm loading failed (CUDA, models, etc.)
//!
//! # Thread Safety
//!
//! - `OnceLock` ensures single initialization
//! - `RwLock` allows concurrent reads during embedding
//! - All operations are `Send + Sync`

use std::sync::Arc;
use std::sync::OnceLock;

use tokio::sync::RwLock;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::models::pretrained::{CausalModel, GraphModel};
use crate::provider::ProductionMultiArrayProvider;
use context_graph_core::traits::MultiArrayEmbeddingProvider;

/// Global warm provider singleton - initialized ONCE, used everywhere.
///
/// Structure: `OnceLock<Arc<RwLock<ProviderState>>>`
/// - `OnceLock`: Ensures single initialization across all threads
/// - `Arc`: Allows cloning for async tasks
/// - `RwLock`: Allows concurrent reads, exclusive writes
/// - `ProviderState`: Holds provider and error state
static GLOBAL_WARM_PROVIDER: OnceLock<Arc<RwLock<ProviderState>>> = OnceLock::new();

/// Internal state for the global provider.
///
/// State machine:
/// - `provider: None, init_error: None` -> Not initialized
/// - `provider: None, init_error: Some` -> Initialization failed
/// - `provider: Some, init_error: None` -> Ready
#[derive(Default)]
struct ProviderState {
    /// The actual provider as trait object, if initialized successfully
    provider: Option<Arc<dyn MultiArrayEmbeddingProvider>>,
    /// The concrete provider for accessing models directly.
    /// Stored separately to allow model extraction without downcasting.
    concrete_provider: Option<Arc<ProductionMultiArrayProvider>>,
    /// Error message if initialization failed
    init_error: Option<String>,
}

// =============================================================================
// CUDA-specific implementation - Uses ProductionMultiArrayProvider directly
// =============================================================================

#[cfg(feature = "candle")]
mod cuda_impl {
    use super::*;
    use crate::config::GpuConfig;
    use crate::provider::ProductionMultiArrayProvider;
    use std::path::{Path, PathBuf};

    /// Resolve models directory path.
    ///
    /// Priority:
    /// 1. CONTEXT_GRAPH_MODELS_PATH environment variable
    /// 2. CONTEXTGRAPH_MODELS_ROOT environment variable
    /// 3. Canonical aiwonder hot model registry path
    fn resolve_models_dir() -> EmbeddingResult<PathBuf> {
        if let Ok(path) = std::env::var("CONTEXT_GRAPH_MODELS_PATH") {
            let p = PathBuf::from(&path);
            tracing::info!("Using models path from CONTEXT_GRAPH_MODELS_PATH: {:?}", p);
            return validate_model_root(p, "CONTEXT_GRAPH_MODELS_PATH");
        }

        if let Ok(path) = std::env::var("CONTEXTGRAPH_MODELS_ROOT") {
            let p = PathBuf::from(&path);
            tracing::info!("Using models path from CONTEXTGRAPH_MODELS_ROOT: {:?}", p);
            return validate_model_root(p, "CONTEXTGRAPH_MODELS_ROOT");
        }

        let canonical = PathBuf::from("/zfs/hot/contextgraph/models");
        if canonical.join("mejepa_models_config.toml").is_file() {
            tracing::info!("Using canonical ME-JEPA models path: {:?}", canonical);
            return validate_model_root(canonical, "canonical_model_root");
        }

        let default_path = PathBuf::from("/zfs/hot/contextgraph/models");
        tracing::info!("Using default models path: {:?}", default_path);
        validate_model_root(default_path, "default_model_root")
    }

    pub(super) fn validate_model_root(
        path: PathBuf,
        field: &'static str,
    ) -> EmbeddingResult<PathBuf> {
        if !path.is_absolute() {
            return Err(model_root_error(field, &path));
        }
        let allowed_roots = [
            Path::new("/zfs/hot/contextgraph"),
            Path::new("/zfs/archive/contextgraph"),
            Path::new("/home/croyse/.cache/contextgraph"),
        ];
        if allowed_roots.iter().any(|root| path.starts_with(root)) {
            return Ok(path);
        }
        Err(model_root_error(field, &path))
    }

    fn model_root_error(field: &'static str, path: &Path) -> EmbeddingError {
        EmbeddingError::InternalError {
            message: format!(
                "MEJEPA_MODEL_ROOT_NOT_AIWONDER: {field}={} is not an aiwonder model root; \
                 use /zfs/hot/contextgraph, /zfs/archive/contextgraph, or explicit aiwonder scratch",
                path.display()
            ),
        }
    }

    /// Initialize the global warm provider (CUDA version).
    ///
    /// Uses ProductionMultiArrayProvider which loads all 13 models eagerly.
    /// NO STUBS - real GPU inference with fail-fast error handling.
    ///
    /// MUST be called ONCE at startup before any embedding operations.
    /// Subsequent calls are no-ops if initialization succeeded.
    pub async fn initialize_global_warm_provider_impl() -> EmbeddingResult<()> {
        initialize_global_warm_provider_with_models_dir_impl(resolve_models_dir()?).await
    }

    /// Initialize the global warm provider from an explicitly resolved model path.
    ///
    /// This is the production entry point for MCP startup: the caller resolves
    /// the canonical model asset directory once and the singleton uses that exact
    /// source of truth. No executable-relative probing or direct-loader fallback.
    pub async fn initialize_global_warm_provider_with_models_dir_impl(
        models_dir: PathBuf,
    ) -> EmbeddingResult<()> {
        let models_dir = validate_model_root(models_dir, "models_dir")?;
        let slot =
            GLOBAL_WARM_PROVIDER.get_or_init(|| Arc::new(RwLock::new(ProviderState::default())));

        let mut guard = slot.write().await;

        // Already initialized successfully
        if guard.provider.is_some() {
            tracing::debug!("Global warm provider already initialized, skipping");
            return Ok(());
        }

        // Previous initialization attempt failed - don't retry
        if let Some(ref err) = guard.init_error {
            return Err(EmbeddingError::InternalError {
                message: format!(
                    "Global warm provider initialization previously failed: {}",
                    err
                ),
            });
        }

        tracing::info!("Initializing global warm provider with ProductionMultiArrayProvider...");
        tracing::info!(
            "Loading active production embedding models to GPU VRAM (E5 retired; this takes 20-30 seconds)..."
        );

        let gpu_config = GpuConfig::default();

        // Verify models directory exists
        if !models_dir.exists() {
            let err_msg = format!(
                "Models directory not found: {:?}. Ensure /zfs/hot/contextgraph/models exists or set an allowed aiwonder model root.",
                models_dir
            );
            tracing::error!("{}", err_msg);
            guard.init_error = Some(err_msg.clone());
            return Err(EmbeddingError::InternalError { message: err_msg });
        }

        // Create ProductionMultiArrayProvider - this loads all production models.
        // NO FALLBACKS - if this fails, the system is not functional
        let provider = match ProductionMultiArrayProvider::new(models_dir.clone(), gpu_config).await
        {
            Ok(p) => p,
            Err(e) => {
                let err_msg = format!(
                    "Failed to create ProductionMultiArrayProvider: {}. \
                     Models dir: {:?}. Ensure all production model files exist and CUDA GPU is available.",
                    e, models_dir
                );
                tracing::error!("{}", err_msg);
                guard.init_error = Some(err_msg.clone());
                return Err(e);
            }
        };

        // Verify provider is ready
        if !provider.is_ready() {
            let health = provider.health_status();
            let ready_count = health.iter().filter(|&&h| h).count();
            let err_msg = format!(
                "ProductionMultiArrayProvider not ready after initialization: {}/{} models ready",
                ready_count,
                health.len()
            );
            tracing::error!("{}", err_msg);
            guard.init_error = Some(err_msg.clone());
            return Err(EmbeddingError::InternalError { message: err_msg });
        }

        // Store the provider (both concrete and trait object)
        let concrete_provider = Arc::new(provider);
        guard.concrete_provider = Some(Arc::clone(&concrete_provider));
        guard.provider = Some(concrete_provider);

        tracing::info!(
            "Global warm provider initialized successfully - active production models loaded to VRAM; E5 retired"
        );
        Ok(())
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Initialize the global warm provider.
///
/// MUST be called ONCE at startup before any embedding operations.
/// Subsequent calls are no-ops if initialization succeeded.
///
/// # CUDA Requirement
///
/// This function requires the `cuda` feature. Per Constitution AP-007,
/// CUDA is mandatory - there are NO CPU fallbacks.
///
/// # Errors
///
/// - `InitializationFailed`: If warm loading fails (CUDA unavailable, model files missing, etc.)
///
/// # Panics
///
/// Does NOT panic. All errors are returned as `EmbeddingResult`.
#[cfg(feature = "candle")]
pub async fn initialize_global_warm_provider() -> EmbeddingResult<()> {
    cuda_impl::initialize_global_warm_provider_impl().await
}

/// Initialize the global warm provider with an explicitly resolved model directory.
#[cfg(feature = "candle")]
pub async fn initialize_global_warm_provider_with_models_dir(
    models_dir: impl Into<std::path::PathBuf>,
) -> EmbeddingResult<()> {
    cuda_impl::initialize_global_warm_provider_with_models_dir_impl(models_dir.into()).await
}

/// Initialize the global warm provider (non-CUDA stub).
///
/// When CUDA is not available, this returns an error per Constitution AP-007.
#[cfg(not(feature = "candle"))]
pub async fn initialize_global_warm_provider() -> EmbeddingResult<()> {
    Err(EmbeddingError::CudaUnavailable {
        message: "Global warm provider requires CUDA feature. Build with --features cuda"
            .to_string(),
    })
}

/// Initialize the global warm provider with an explicitly resolved model directory.
#[cfg(not(feature = "candle"))]
pub async fn initialize_global_warm_provider_with_models_dir(
    _models_dir: impl Into<std::path::PathBuf>,
) -> EmbeddingResult<()> {
    Err(EmbeddingError::CudaUnavailable {
        message: "Global warm provider requires CUDA feature. Build with --features cuda"
            .to_string(),
    })
}

/// Get the global warm provider.
///
/// FAILS FAST if not initialized - call `initialize_global_warm_provider()` first.
///
/// # Errors
///
/// - `NotInitialized`: Provider not initialized yet
/// - `ProviderBusy`: Lock contention (rare, retry)
/// - `InitializationFailed`: Previous initialization failed
///
/// # Example
///
/// ```rust,ignore
/// let provider = get_warm_provider()?;
/// let output = provider.embed_all("some text").await?;
/// ```
pub fn get_warm_provider() -> EmbeddingResult<Arc<dyn MultiArrayEmbeddingProvider>> {
    let slot = GLOBAL_WARM_PROVIDER.get().ok_or_else(|| {
        EmbeddingError::InternalError {
            message: "Global warm provider not initialized. Call initialize_global_warm_provider() first.".to_string(),
        }
    })?;

    // Use try_read for non-blocking check
    let guard = slot.try_read().map_err(|_| EmbeddingError::InternalError {
        message: "Global warm provider is busy (lock contention). Retry later.".to_string(),
    })?;

    // Check if initialization failed
    if let Some(ref err) = guard.init_error {
        return Err(EmbeddingError::InternalError {
            message: format!("Global warm provider initialization failed: {}", err),
        });
    }

    // Get the provider
    guard
        .provider
        .as_ref()
        .map(Arc::clone)
        .ok_or_else(|| EmbeddingError::InternalError {
            message: "Global warm provider not initialized. Call initialize_global_warm_provider() first.".to_string(),
        })
}

/// Check if warm provider is initialized (non-blocking).
///
/// Returns `true` only if initialization succeeded and provider is available.
/// Returns `false` if:
/// - Not yet initialized
/// - Initialization in progress
/// - Initialization failed
pub fn is_warm_initialized() -> bool {
    GLOBAL_WARM_PROVIDER
        .get()
        .and_then(|slot| slot.try_read().ok())
        .map(|guard| guard.provider.is_some())
        .unwrap_or(false)
}

/// Get initialization status message (for diagnostics).
///
/// Returns human-readable status:
/// - "Not initialized"
/// - "Initialization in progress"
/// - "Initialization failed: <error>"
/// - "Ready (active models warm; E5 retired)"
pub fn warm_status_message() -> String {
    match GLOBAL_WARM_PROVIDER.get() {
        None => "Not initialized".to_string(),
        Some(slot) => {
            match slot.try_read() {
                Ok(guard) => {
                    if guard.provider.is_some() {
                        "Ready (active models warm; E5 retired)".to_string()
                    } else if let Some(ref err) = guard.init_error {
                        format!("Initialization failed: {}", err)
                    } else {
                        // State slot exists but no provider and no error - not yet initialized
                        "Not initialized".to_string()
                    }
                }
                // Lock contention means initialization is likely in progress
                Err(_) => "Initialization in progress".to_string(),
            }
        }
    }
}

/// Get the CausalModel from the global warm provider.
///
/// This provides direct access to the E5 CausalModel for use by CausalDiscoveryService.
/// The model is already loaded and ready for use.
///
/// # Returns
///
/// `Ok(Arc<CausalModel>)` if the provider is initialized
/// `Err` if the provider is not initialized or initialization failed
///
/// # Example
///
/// ```rust,ignore
/// let causal_model = get_warm_causal_model()?;
/// let service = CausalDiscoveryService::with_models(llm, causal_model, config);
/// ```
pub fn get_warm_causal_model() -> EmbeddingResult<Arc<CausalModel>> {
    Err(EmbeddingError::InternalError {
        message: "E5 causal embedder is retired and disabled; no warm CausalModel is loaded"
            .to_string(),
    })
}

/// Get the GraphModel from the global warm provider.
///
/// This provides direct access to the E8 GraphModel for deterministic graph activation.
/// The model is already loaded and ready for use.
///
/// # Returns
///
/// `Ok(Arc<GraphModel>)` if the provider is initialized
/// `Err` if the provider is not initialized or initialization failed
///
/// # Example
///
/// ```rust,ignore
/// let graph_model = get_warm_graph_model()?;
/// let graph_model = get_warm_graph_model()?;
/// ```
pub fn get_warm_graph_model() -> EmbeddingResult<Arc<GraphModel>> {
    let slot = GLOBAL_WARM_PROVIDER.get().ok_or_else(|| EmbeddingError::InternalError {
        message: "Global warm provider not initialized. Call initialize_global_warm_provider() first.".to_string(),
    })?;

    let guard = slot.try_read().map_err(|_| EmbeddingError::InternalError {
        message: "Global warm provider is busy (lock contention). Retry later.".to_string(),
    })?;

    if let Some(ref err) = guard.init_error {
        return Err(EmbeddingError::InternalError {
            message: format!("Global warm provider initialization failed: {}", err),
        });
    }

    guard
        .concrete_provider
        .as_ref()
        .map(|p| p.graph_model())
        .ok_or_else(|| EmbeddingError::InternalError {
            message: "Global warm provider not initialized. Call initialize_global_warm_provider() first.".to_string(),
        })
}

/// Reset the global provider (for testing only).
///
/// # Safety
///
/// This is intended ONLY for tests that need to reinitialize the provider.
/// Using this in production could lead to race conditions.
#[cfg(test)]
pub async fn reset_global_provider_for_testing() {
    if let Some(slot) = GLOBAL_WARM_PROVIDER.get() {
        let mut guard = slot.write().await;
        *guard = ProviderState::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_warm_initialized_before_init() {
        // Before any initialization, should return false
        // Note: This test might fail if run after other tests that initialize
        // In real test suite, use reset_global_provider_for_testing()
        let _ = is_warm_initialized(); // Just verify it doesn't panic
    }

    #[test]
    fn test_warm_status_message_before_init() {
        let status = warm_status_message();
        // Should be one of the expected states
        assert!(
            status == "Not initialized"
                || status.starts_with("Ready")
                || status.starts_with("Initialization"),
            "Unexpected status: {}",
            status
        );
    }

    #[test]
    fn test_get_warm_provider_before_init() {
        // May or may not be initialized depending on test order
        // Just verify it doesn't panic
        let result = get_warm_provider();
        // Either succeeds or returns a sensible error
        if let Err(e) = result {
            let msg = e.to_string();
            assert!(
                msg.contains("not initialized") || msg.contains("failed"),
                "Unexpected error: {}",
                msg
            );
        }
    }

    #[cfg(feature = "candle")]
    #[test]
    fn test_model_root_guard_rejects_non_aiwonder_root() {
        let err = cuda_impl::validate_model_root(PathBuf::from("/tmp/contextgraph/models"), "test")
            .expect_err("non-aiwonder root must not be accepted as a model root");
        assert!(err.to_string().contains("MEJEPA_MODEL_ROOT_NOT_AIWONDER"));
    }

    #[cfg(feature = "candle")]
    #[test]
    fn test_model_root_guard_accepts_aiwonder_hot_root() {
        let path =
            cuda_impl::validate_model_root(PathBuf::from("/zfs/hot/contextgraph/models"), "test")
                .expect("aiwonder hot model root must be accepted");
        assert_eq!(path, PathBuf::from("/zfs/hot/contextgraph/models"));
    }

    #[cfg(feature = "candle")]
    #[test]
    fn test_model_root_guard_accepts_archive_and_scratch_roots() {
        for root in [
            "/zfs/archive/contextgraph/models",
            "/home/croyse/.cache/contextgraph/models",
        ] {
            let path = cuda_impl::validate_model_root(PathBuf::from(root), "test")
                .expect("allowed aiwonder model roots must be accepted");
            assert_eq!(path, PathBuf::from(root));
        }
    }

    #[cfg(feature = "candle")]
    #[test]
    fn test_model_root_guard_rejects_local_and_sibling_roots() {
        for root in [
            "models",
            "/zfs/hot/contextgraph-bad/models",
        ] {
            let err = cuda_impl::validate_model_root(PathBuf::from(root), "test")
                .expect_err("non-aiwonder model roots must be rejected");
            assert!(err.to_string().contains("MEJEPA_MODEL_ROOT_NOT_AIWONDER"));
        }
    }
}
