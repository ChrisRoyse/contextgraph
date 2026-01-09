//! Health Checker Service
//!
//! Provides the main health check service for querying the warm loading system state.

use std::time::{Duration, Instant};

use crate::warm::loader::WarmLoader;
use crate::warm::memory_pool::WarmMemoryPools;
use crate::warm::registry::SharedWarmRegistry;
use crate::warm::state::WarmModelState;

use super::check::WarmHealthCheck;
use super::status::WarmHealthStatus;

/// Health check service for the warm loading system.
///
/// Provides methods to query the current health status of all loaded models.
/// Thread-safe for concurrent access from monitoring systems.
///
/// # Example
///
/// ```rust,ignore
/// let checker = WarmHealthChecker::from_loader(&loader);
///
/// // Quick status check (no allocations)
/// match checker.status() {
///     WarmHealthStatus::Healthy => serve_requests(),
///     WarmHealthStatus::Loading => wait_for_startup(),
///     WarmHealthStatus::Unhealthy => alert_oncall(),
///     WarmHealthStatus::NotInitialized => panic!("System not initialized"),
/// }
///
/// // Detailed check with metrics
/// let health = checker.check();
/// prometheus::gauge!("models_warm").set(health.models_warm as f64);
/// ```
pub struct WarmHealthChecker {
    /// Shared registry for model state access.
    registry: SharedWarmRegistry,

    /// Memory pools for VRAM usage queries.
    memory_pools: WarmMemoryPools,

    /// Start time for uptime tracking.
    start_time: Instant,
}

impl WarmHealthChecker {
    /// Create a health checker from a [`WarmLoader`].
    ///
    /// Extracts the registry and memory pools from the loader.
    /// The checker maintains references to these components for querying.
    ///
    /// # Arguments
    ///
    /// * `loader` - The warm loader to monitor
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let loader = WarmLoader::new(config)?;
    /// let checker = WarmHealthChecker::from_loader(&loader);
    /// ```
    #[must_use]
    pub fn from_loader(loader: &WarmLoader) -> Self {
        Self {
            registry: loader.registry().clone(),
            memory_pools: loader.memory_pools().clone(),
            start_time: Instant::now(),
        }
    }

    /// Create a health checker directly from components.
    ///
    /// Useful for testing or when constructing the checker independently.
    ///
    /// # Arguments
    ///
    /// * `registry` - Shared registry for model state access
    /// * `memory_pools` - Memory pools for VRAM queries
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let registry = Arc::new(RwLock::new(WarmModelRegistry::new()));
    /// let pools = WarmMemoryPools::rtx_5090();
    /// let checker = WarmHealthChecker::new(registry, pools);
    /// ```
    #[must_use]
    pub fn new(registry: SharedWarmRegistry, memory_pools: WarmMemoryPools) -> Self {
        Self {
            registry,
            memory_pools,
            start_time: Instant::now(),
        }
    }

    /// Perform a detailed health check.
    ///
    /// Queries the registry for model states and memory pools for VRAM usage.
    /// Returns a comprehensive [`WarmHealthCheck`] result.
    ///
    /// # Thread Safety
    ///
    /// Acquires a read lock on the registry. Safe for concurrent calls.
    /// Returns `NotInitialized` status if the lock is poisoned.
    ///
    /// # Returns
    ///
    /// A [`WarmHealthCheck`] containing status, model counts, and VRAM metrics.
    #[must_use]
    pub fn check(&self) -> WarmHealthCheck {
        let last_check = Instant::now();
        let uptime = Some(self.start_time.elapsed());

        // Attempt to read the registry
        let registry = match self.registry.read() {
            Ok(r) => r,
            Err(_) => {
                // Lock poisoned - return not initialized
                return WarmHealthCheck {
                    status: WarmHealthStatus::NotInitialized,
                    uptime,
                    last_check,
                    ..WarmHealthCheck::not_initialized()
                };
            }
        };

        // Check if any models are registered
        let models_total = registry.model_count();
        if models_total == 0 {
            return WarmHealthCheck {
                status: WarmHealthStatus::NotInitialized,
                models_total: 0,
                uptime,
                last_check,
                ..WarmHealthCheck::not_initialized()
            };
        }

        // Count models in each state
        let mut models_warm = 0usize;
        let mut models_loading = 0usize;
        let mut models_failed = 0usize;
        let mut models_pending = 0usize;
        let mut error_messages = Vec::new();

        // Iterate through all model states
        for entry in registry.loading_order() {
            if let Some(state) = registry.get_state(&entry) {
                match &state {
                    WarmModelState::Warm => models_warm += 1,
                    WarmModelState::Loading { .. } | WarmModelState::Validating => {
                        models_loading += 1;
                    }
                    WarmModelState::Failed {
                        error_code,
                        error_message,
                    } => {
                        models_failed += 1;
                        error_messages
                            .push(format!("{}: [{}] {}", entry, error_code, error_message));
                    }
                    WarmModelState::Pending => models_pending += 1,
                }
            }
        }

        // Determine overall status
        let status = if models_failed > 0 {
            WarmHealthStatus::Unhealthy
        } else if models_loading > 0 || models_pending > 0 {
            // If any model is still pending or loading, we're in loading state
            if models_warm == 0 && models_loading == 0 {
                // All pending, nothing started
                WarmHealthStatus::Loading
            } else {
                WarmHealthStatus::Loading
            }
        } else if models_warm == models_total {
            WarmHealthStatus::Healthy
        } else {
            // Unexpected state - should not happen
            WarmHealthStatus::NotInitialized
        };

        // Get VRAM metrics from memory pools
        let model_allocations = self.memory_pools.list_model_allocations();
        let vram_allocated_bytes: usize = model_allocations.iter().map(|a| a.size_bytes).sum();
        let vram_available_bytes = self.memory_pools.available_model_bytes();

        // Get working memory metrics
        let working_memory_available_bytes = self.memory_pools.available_working_bytes();
        let working_memory_total = self.memory_pools.working_pool_capacity();
        let working_memory_allocated_bytes =
            working_memory_total.saturating_sub(working_memory_available_bytes);

        WarmHealthCheck {
            status,
            models_total,
            models_warm,
            models_loading,
            models_failed,
            models_pending,
            vram_allocated_bytes,
            vram_available_bytes,
            working_memory_allocated_bytes,
            working_memory_available_bytes,
            uptime,
            last_check,
            error_messages,
        }
    }

    /// Quick status check without detailed metrics.
    ///
    /// More efficient than [`check()`](Self::check) when only the status is needed.
    ///
    /// # Returns
    ///
    /// The current [`WarmHealthStatus`].
    #[must_use]
    pub fn status(&self) -> WarmHealthStatus {
        // Attempt to read the registry
        let registry = match self.registry.read() {
            Ok(r) => r,
            Err(_) => return WarmHealthStatus::NotInitialized,
        };

        // Check if any models are registered
        if registry.model_count() == 0 {
            return WarmHealthStatus::NotInitialized;
        }

        // Quick check: any failed?
        if registry.any_failed() {
            return WarmHealthStatus::Unhealthy;
        }

        // Quick check: all warm?
        if registry.all_warm() {
            return WarmHealthStatus::Healthy;
        }

        // Otherwise, still loading
        WarmHealthStatus::Loading
    }

    /// Check if the system is healthy.
    ///
    /// Convenience method equivalent to `self.status().is_healthy()`.
    ///
    /// # Returns
    ///
    /// `true` if all models are in `Warm` state.
    #[inline]
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.status().is_healthy()
    }

    /// Get the uptime since the checker was created.
    ///
    /// # Returns
    ///
    /// Duration since the health checker was instantiated.
    #[must_use]
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get a reference to the registry.
    ///
    /// For advanced use cases requiring direct registry access.
    #[must_use]
    pub fn registry(&self) -> &SharedWarmRegistry {
        &self.registry
    }

    /// Get a reference to the memory pools.
    ///
    /// For advanced use cases requiring direct memory pool access.
    #[must_use]
    pub fn memory_pools(&self) -> &WarmMemoryPools {
        &self.memory_pools
    }
}

impl Clone for WarmHealthChecker {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
            memory_pools: self.memory_pools.clone(),
            start_time: self.start_time,
        }
    }
}
