//! Main WarmLoader engine - the orchestrator for warm model loading.
//!
//! Coordinates the loading of all embedding models into VRAM using:
//! - [`WarmModelRegistry`] for state machine tracking
//! - [`WarmMemoryPools`] for VRAM allocation management
//! - [`WarmCudaAllocator`] for protected CUDA allocations
//! - [`WarmValidator`] for model validation

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use crate::warm::config::WarmConfig;
use crate::warm::cuda_alloc::{GpuInfo, WarmCudaAllocator};
use crate::warm::error::{WarmError, WarmResult};
use crate::warm::memory_pool::WarmMemoryPools;
use crate::warm::registry::{
    SharedWarmRegistry, WarmModelRegistry, EMBEDDING_MODEL_IDS, TOTAL_MODEL_COUNT,
};
use crate::warm::validation::WarmValidator;

use super::constants::{DEFAULT_EMBEDDING_DIMENSION, GB, MODEL_SIZES};
use super::operations::{load_single_model, mark_model_failed, verify_all_warm};
#[cfg(feature = "cuda")]
use super::preflight::{initialize_cuda_allocator, run_preflight_checks};
use super::summary::LoadingSummary;

#[cfg(test)]
use super::operations::allocate_model_vram;

/// Main orchestrator for warm model loading.
///
/// # Fail-Fast Behavior
///
/// On ANY error during loading, the loader:
/// 1. Logs comprehensive diagnostic information
/// 2. Calls `std::process::exit()` with the appropriate error code
///
/// There is NO partial loading mode. The system either has all models
/// warm and ready, or it terminates.
pub struct WarmLoader {
    /// Configuration for loading.
    config: WarmConfig,
    /// Thread-safe registry for model state tracking.
    registry: SharedWarmRegistry,
    /// Dual-pool VRAM management (model + working).
    memory_pools: WarmMemoryPools,
    /// CUDA allocator for protected allocations.
    #[allow(dead_code)]
    cuda_allocator: Option<WarmCudaAllocator>,
    /// Model validator for dimension/weight/inference checks.
    validator: WarmValidator,
    /// Ordered list of model IDs for loading sequence.
    loading_order: Vec<String>,
    /// Start time of loading operation.
    start_time: Option<Instant>,
    /// GPU information (cached after pre-flight).
    gpu_info: Option<GpuInfo>,
}

impl WarmLoader {
    /// Create a new loader with the given configuration.
    ///
    /// Initializes all components but does NOT begin loading.
    /// Call [`load_all_models()`](Self::load_all_models) to start the loading process.
    pub fn new(config: WarmConfig) -> WarmResult<Self> {
        tracing::info!("Creating WarmLoader with config: {:?}", config);

        // Create the registry and register all models
        let mut registry = WarmModelRegistry::new();
        Self::register_all_models(&mut registry)?;

        // Get the loading order (largest first)
        let loading_order = registry.loading_order();

        tracing::info!(
            "Registered {} models, loading order: {:?}",
            registry.model_count(),
            loading_order
        );

        // Create memory pools from config
        let memory_pools = WarmMemoryPools::new(config.clone());

        tracing::info!(
            "Memory pools initialized: model={}GB, working={}GB",
            memory_pools.model_pool_capacity() / GB,
            memory_pools.working_pool_capacity() / GB
        );

        // Create validator
        let validator = WarmValidator::new();

        Ok(Self {
            config,
            registry: Arc::new(RwLock::new(registry)),
            memory_pools,
            cuda_allocator: None,
            validator,
            loading_order,
            start_time: None,
            gpu_info: None,
        })
    }

    /// Register all 12 embedding models in the registry.
    pub(crate) fn register_all_models(registry: &mut WarmModelRegistry) -> WarmResult<()> {
        let size_map: HashMap<&str, usize> = MODEL_SIZES.iter().copied().collect();

        for model_id in EMBEDDING_MODEL_IDS {
            let size = size_map.get(model_id).copied().unwrap_or(500 * 1024 * 1024);
            registry.register_model(model_id, size, DEFAULT_EMBEDDING_DIMENSION)?;
        }

        Ok(())
    }

    /// Load all models into VRAM.
    ///
    /// This is the main entry point for warm loading.
    ///
    /// # CUDA Requirement
    ///
    /// This function requires the `cuda` feature to be enabled.
    /// Without CUDA, warm loading fails fast per Constitution AP-007.
    #[cfg(feature = "cuda")]
    pub fn load_all_models(&mut self) -> WarmResult<()> {
        self.start_time = Some(Instant::now());

        tracing::info!(
            "Starting warm model loading for {} models",
            TOTAL_MODEL_COUNT
        );

        // Step 1: Pre-flight checks
        if let Err(e) = run_preflight_checks(&self.config, &mut self.gpu_info) {
            Self::handle_fatal_error(&e);
        }

        // Step 2: Initialize CUDA allocator
        match initialize_cuda_allocator(&self.config) {
            Ok(allocator) => self.cuda_allocator = Some(allocator),
            Err(e) => Self::handle_fatal_error(&e),
        }

        // Step 3: Load each model in order using REAL CUDA allocation
        // Get mutable reference to cuda_allocator - REQUIRED, no fallback
        let cuda_allocator =
            self.cuda_allocator
                .as_mut()
                .ok_or_else(|| WarmError::CudaInitFailed {
                    cuda_error: "CUDA allocator not initialized - CUDA is REQUIRED (RTX 5090)"
                        .to_string(),
                    driver_version: String::new(),
                    gpu_name: String::new(),
                });

        let cuda_allocator = match cuda_allocator {
            Ok(alloc) => alloc,
            Err(e) => Self::handle_fatal_error(&e),
        };

        for model_id in self.loading_order.clone() {
            if let Err(e) = load_single_model(
                &model_id,
                &self.config,
                &self.registry,
                &mut self.memory_pools,
                cuda_allocator,
                &self.validator,
            ) {
                let _ = mark_model_failed(&model_id, &e, &self.registry);
                Self::handle_fatal_error(&e);
            }
        }

        // Step 4: Final verification
        if let Err(e) = verify_all_warm(&self.loading_order, &self.registry, TOTAL_MODEL_COUNT) {
            Self::handle_fatal_error(&e);
        }

        let duration = self.start_time.map(|t| t.elapsed());
        tracing::info!(
            "All {} models loaded successfully in {:?}",
            TOTAL_MODEL_COUNT,
            duration
        );

        Ok(())
    }

    /// Handle a fatal error by logging and exiting.
    fn handle_fatal_error(error: &WarmError) -> ! {
        tracing::error!(
            exit_code = error.exit_code(),
            category = %error.category(),
            error_code = %error.error_code(),
            "FATAL: Warm model loading failed"
        );

        tracing::error!("Error details: {}", error);
        std::process::exit(error.exit_code())
    }

    /// Get a reference to the registry.
    #[must_use]
    pub fn registry(&self) -> &SharedWarmRegistry {
        &self.registry
    }

    /// Get a reference to the memory pools.
    #[must_use]
    pub fn memory_pools(&self) -> &WarmMemoryPools {
        &self.memory_pools
    }

    /// Get a mutable reference to the memory pools.
    #[must_use]
    pub fn memory_pools_mut(&mut self) -> &mut WarmMemoryPools {
        &mut self.memory_pools
    }

    /// Check if all models are warm.
    #[must_use]
    pub fn all_warm(&self) -> bool {
        self.registry.read().map(|r| r.all_warm()).unwrap_or(false)
    }

    /// Get a summary of the loading state.
    #[must_use]
    pub fn loading_summary(&self) -> LoadingSummary {
        let registry = match self.registry.read() {
            Ok(r) => r,
            Err(_) => return LoadingSummary::empty(),
        };

        let mut model_states = HashMap::new();
        let mut models_warm = 0;
        let mut models_failed = 0;
        let mut models_loading = 0;

        for model_id in &self.loading_order {
            if let Some(state) = registry.get_state(model_id) {
                if state.is_warm() {
                    models_warm += 1;
                } else if state.is_failed() {
                    models_failed += 1;
                } else if state.is_loading() {
                    models_loading += 1;
                }
                model_states.insert(model_id.clone(), state);
            }
        }

        let total_vram_allocated = self
            .memory_pools
            .list_model_allocations()
            .iter()
            .map(|a| a.size_bytes)
            .sum();

        let loading_duration = self.start_time.map(|t| t.elapsed());

        LoadingSummary {
            total_models: registry.model_count(),
            models_warm,
            models_failed,
            models_loading,
            total_vram_allocated,
            loading_duration,
            model_states,
        }
    }

    /// Get the configuration.
    #[must_use]
    pub fn config(&self) -> &WarmConfig {
        &self.config
    }

    /// Get cached GPU info (if available).
    #[must_use]
    pub fn gpu_info(&self) -> Option<&GpuInfo> {
        self.gpu_info.as_ref()
    }

    // ========================================================================
    // Methods exposed for testing (pub(crate) visibility)
    // ========================================================================

    /// Run pre-flight checks (exposed for testing).
    #[cfg(all(test, feature = "cuda"))]
    pub(crate) fn run_preflight_checks(&mut self) -> WarmResult<()> {
        run_preflight_checks(&self.config, &mut self.gpu_info)
    }

    /// Initialize CUDA allocator for testing.
    ///
    /// # CRITICAL: Real CUDA Required
    ///
    /// This initializes a real WarmCudaAllocator using Candle.
    /// Per Constitution AP-007, no fake allocators are allowed.
    #[cfg(all(test, feature = "cuda"))]
    pub(crate) fn initialize_cuda_for_test(&mut self) -> WarmResult<()> {
        // Run preflight first if not done
        if self.gpu_info.is_none() {
            self.run_preflight_checks()?;
        }

        // Initialize CUDA allocator (real Candle-based allocation)
        match initialize_cuda_allocator(&self.config) {
            Ok(allocator) => {
                self.cuda_allocator = Some(allocator);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Allocate model VRAM (exposed for testing).
    ///
    /// # CRITICAL: Requires CUDA
    ///
    /// This method uses real CUDA allocation. If CUDA is not initialized,
    /// it will return an error. No fake pointers are generated.
    #[cfg(test)]
    pub(crate) fn allocate_model_vram(
        &mut self,
        model_id: &str,
        size_bytes: usize,
    ) -> WarmResult<u64> {
        let cuda_allocator =
            self.cuda_allocator
                .as_mut()
                .ok_or_else(|| WarmError::CudaInitFailed {
                    cuda_error: "CUDA allocator not initialized for test".to_string(),
                    driver_version: String::new(),
                    gpu_name: String::new(),
                })?;
        allocate_model_vram(model_id, size_bytes, &mut self.memory_pools, cuda_allocator)
    }
}
