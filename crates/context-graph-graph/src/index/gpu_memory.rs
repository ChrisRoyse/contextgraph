//! GPU Memory Manager for VRAM budget tracking.
//!
//! Provides centralized allocation tracking to prevent GPU OOM conditions
//! when working with 10M+ vectors on RTX 5090 (32GB VRAM).
//!
//! # Constitution Reference
//!
//! - AP-015: GPU alloc without pool → use CUDA memory pool
//! - perf.memory.gpu: <24GB (8GB headroom)
//! - stack.gpu.vram: 32GB
//!
//! # Memory Budget
//!
//! ```text
//! +------------------+--------+
//! | FAISS Index      | 8GB    |
//! | Hyperbolic Coords| 2.5GB  |
//! | Entailment Cones | 2.7GB  |
//! | Working Memory   | 10.8GB |
//! +------------------+--------+
//! | Total Safe Limit | 24GB   |
//! +------------------+--------+
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::error::{GraphError, GraphResult};

/// Memory categories for GPU budget allocation.
///
/// Each category has a default budget that can be overridden via config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryCategory {
    /// FAISS IVF-PQ index structures (8GB default)
    FaissIndex,
    /// Poincare point coordinates (2.5GB default)
    HyperbolicCoords,
    /// Entailment cone data (2.7GB default)
    EntailmentCones,
    /// Temporary working memory (10.8GB default)
    WorkingMemory,
    /// Uncategorized allocations (512MB default)
    Other,
}

impl MemoryCategory {
    /// Get default budget for this category in bytes.
    pub const fn default_budget(&self) -> usize {
        match self {
            MemoryCategory::FaissIndex => 8 * 1024 * 1024 * 1024, // 8GB
            MemoryCategory::HyperbolicCoords => 2560 * 1024 * 1024, // 2.5GB
            MemoryCategory::EntailmentCones => 2764 * 1024 * 1024, // 2.7GB
            MemoryCategory::WorkingMemory => 10854 * 1024 * 1024, // 10.8GB
            MemoryCategory::Other => 512 * 1024 * 1024,           // 512MB
        }
    }

    /// Get human-readable name.
    pub const fn name(&self) -> &'static str {
        match self {
            MemoryCategory::FaissIndex => "FAISS Index",
            MemoryCategory::HyperbolicCoords => "Hyperbolic Coords",
            MemoryCategory::EntailmentCones => "Entailment Cones",
            MemoryCategory::WorkingMemory => "Working Memory",
            MemoryCategory::Other => "Other",
        }
    }
}

/// Handle to allocated GPU memory.
///
/// When dropped, automatically frees the allocation.
/// The handle holds an Arc to the manager inner state.
pub struct AllocationHandle {
    id: u64,
    size: usize,
    category: MemoryCategory,
    manager: Arc<Mutex<ManagerInner>>,
}

impl std::fmt::Debug for AllocationHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AllocationHandle")
            .field("id", &self.id)
            .field("size", &self.size)
            .field("category", &self.category)
            .finish()
    }
}

impl AllocationHandle {
    /// Get allocation size in bytes.
    #[inline]
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get allocation category.
    #[inline]
    pub fn category(&self) -> MemoryCategory {
        self.category
    }

    /// Get allocation ID.
    #[inline]
    pub fn id(&self) -> u64 {
        self.id
    }
}

impl Drop for AllocationHandle {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.manager.lock() {
            inner.free(self.id);
        }
    }
}

/// Configuration for GPU memory manager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuMemoryConfig {
    /// Total VRAM budget in bytes.
    /// Default: 24GB (safe limit for 32GB RTX 5090)
    pub total_budget: usize,

    /// Per-category budget overrides (bytes).
    pub category_budgets: HashMap<MemoryCategory, usize>,

    /// Allow over-allocation (FOR TESTING ONLY).
    /// Default: false (fail fast per AP-001)
    pub allow_overallocation: bool,

    /// Low memory threshold fraction (0.0-1.0).
    /// Triggers warnings when usage exceeds this.
    /// Default: 0.9 (90%)
    pub low_memory_threshold: f32,
}

impl Default for GpuMemoryConfig {
    fn default() -> Self {
        Self {
            total_budget: 24 * 1024 * 1024 * 1024, // 24GB safe limit
            category_budgets: HashMap::new(),
            allow_overallocation: false,
            low_memory_threshold: 0.9,
        }
    }
}

impl GpuMemoryConfig {
    /// Create config for RTX 5090 (24GB safe budget).
    pub fn rtx_5090() -> Self {
        Self::default()
    }

    /// Create config with custom total budget.
    pub fn with_budget(total_bytes: usize) -> Self {
        Self {
            total_budget: total_bytes,
            ..Default::default()
        }
    }

    /// Set category budget (builder pattern).
    pub fn category_budget(mut self, category: MemoryCategory, bytes: usize) -> Self {
        self.category_budgets.insert(category, bytes);
        self
    }

    /// Validate configuration.
    pub fn validate(&self) -> GraphResult<()> {
        if self.total_budget == 0 {
            return Err(GraphError::InvalidConfig(
                "total_budget must be > 0".to_string(),
            ));
        }
        if self.low_memory_threshold <= 0.0 || self.low_memory_threshold > 1.0 {
            return Err(GraphError::InvalidConfig(format!(
                "low_memory_threshold must be in (0, 1], got {}",
                self.low_memory_threshold
            )));
        }
        Ok(())
    }
}

/// Statistics about GPU memory usage.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Total bytes allocated.
    pub total_allocated: usize,
    /// Total budget in bytes.
    pub total_budget: usize,
    /// Number of active allocations.
    pub allocation_count: usize,
    /// Peak memory usage (bytes).
    pub peak_usage: usize,
    /// Per-category usage (bytes).
    pub category_usage: HashMap<MemoryCategory, usize>,
    /// Per-category budget (bytes).
    pub category_budget: HashMap<MemoryCategory, usize>,
}

impl MemoryStats {
    /// Get usage as percentage (0-100).
    pub fn usage_percent(&self) -> f32 {
        if self.total_budget > 0 {
            (self.total_allocated as f32 / self.total_budget as f32) * 100.0
        } else {
            0.0
        }
    }

    /// Check if low memory condition.
    pub fn is_low_memory(&self, threshold: f32) -> bool {
        self.usage_percent() / 100.0 > threshold
    }

    /// Get available bytes.
    pub fn available(&self) -> usize {
        self.total_budget.saturating_sub(self.total_allocated)
    }
}

/// Internal manager state (behind Mutex).
struct ManagerInner {
    config: GpuMemoryConfig,
    allocations: HashMap<u64, (usize, MemoryCategory)>,
    category_usage: HashMap<MemoryCategory, usize>,
    total_allocated: usize,
    peak_usage: usize,
    next_id: u64,
}

impl ManagerInner {
    fn new(config: GpuMemoryConfig) -> Self {
        Self {
            config,
            allocations: HashMap::new(),
            category_usage: HashMap::new(),
            total_allocated: 0,
            peak_usage: 0,
            next_id: 0,
        }
    }

    fn allocate(&mut self, size: usize, category: MemoryCategory) -> GraphResult<u64> {
        // Check total budget
        let new_total = self.total_allocated.saturating_add(size);
        if !self.config.allow_overallocation && new_total > self.config.total_budget {
            return Err(GraphError::GpuResourceAllocation(format!(
                "Allocation of {} bytes would exceed total budget ({}/{} bytes used)",
                size, self.total_allocated, self.config.total_budget
            )));
        }

        // Check category budget
        let category_budget = self
            .config
            .category_budgets
            .get(&category)
            .copied()
            .unwrap_or_else(|| category.default_budget());

        let current_category_usage = self.category_usage.get(&category).copied().unwrap_or(0);
        let new_category_total = current_category_usage.saturating_add(size);

        if !self.config.allow_overallocation && new_category_total > category_budget {
            return Err(GraphError::GpuResourceAllocation(format!(
                "Allocation of {} bytes in {:?} would exceed category budget ({}/{} bytes)",
                size, category, current_category_usage, category_budget
            )));
        }

        // Perform allocation
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);

        self.allocations.insert(id, (size, category));
        *self.category_usage.entry(category).or_insert(0) += size;
        self.total_allocated += size;
        self.peak_usage = self.peak_usage.max(self.total_allocated);

        Ok(id)
    }

    fn free(&mut self, id: u64) {
        if let Some((size, category)) = self.allocations.remove(&id) {
            self.total_allocated = self.total_allocated.saturating_sub(size);
            if let Some(usage) = self.category_usage.get_mut(&category) {
                *usage = usage.saturating_sub(size);
            }
        }
    }

    fn stats(&self) -> MemoryStats {
        let all_categories = [
            MemoryCategory::FaissIndex,
            MemoryCategory::HyperbolicCoords,
            MemoryCategory::EntailmentCones,
            MemoryCategory::WorkingMemory,
            MemoryCategory::Other,
        ];

        MemoryStats {
            total_allocated: self.total_allocated,
            total_budget: self.config.total_budget,
            allocation_count: self.allocations.len(),
            peak_usage: self.peak_usage,
            category_usage: self.category_usage.clone(),
            category_budget: all_categories
                .iter()
                .map(|&cat| {
                    let budget = self
                        .config
                        .category_budgets
                        .get(&cat)
                        .copied()
                        .unwrap_or_else(|| cat.default_budget());
                    (cat, budget)
                })
                .collect(),
        }
    }
}

/// GPU memory manager for VRAM budget tracking.
///
/// Provides centralized allocation tracking to prevent OOM conditions.
/// Thread-safe via Arc<Mutex<>>.
///
/// # Example
///
/// ```rust
/// use context_graph_graph::index::gpu_memory::{GpuMemoryManager, GpuMemoryConfig, MemoryCategory};
///
/// let manager = GpuMemoryManager::new(GpuMemoryConfig::rtx_5090()).unwrap();
///
/// // Allocate memory
/// let handle = manager.allocate(1024 * 1024, MemoryCategory::WorkingMemory).unwrap();
///
/// // Check stats
/// let stats = manager.stats();
/// println!("Using {} of {} bytes", stats.total_allocated, stats.total_budget);
///
/// // Memory freed automatically when handle drops
/// drop(handle);
/// assert_eq!(manager.used(), 0);
/// ```
#[derive(Clone)]
pub struct GpuMemoryManager {
    inner: Arc<Mutex<ManagerInner>>,
}

impl GpuMemoryManager {
    /// Create new memory manager with given configuration.
    ///
    /// # Errors
    ///
    /// Returns error if config validation fails.
    pub fn new(config: GpuMemoryConfig) -> GraphResult<Self> {
        config.validate()?;
        Ok(Self {
            inner: Arc::new(Mutex::new(ManagerInner::new(config))),
        })
    }

    /// Create manager for RTX 5090 (24GB safe budget).
    pub fn rtx_5090() -> GraphResult<Self> {
        Self::new(GpuMemoryConfig::rtx_5090())
    }

    /// Allocate GPU memory.
    ///
    /// Returns AllocationHandle that frees memory on drop.
    ///
    /// # Errors
    ///
    /// Returns `GraphError::GpuResourceAllocation` if:
    /// - Allocation exceeds total budget
    /// - Allocation exceeds category budget
    pub fn allocate(&self, size: usize, category: MemoryCategory) -> GraphResult<AllocationHandle> {
        let id = self
            .inner
            .lock()
            .map_err(|_| GraphError::GpuResourceAllocation("Lock poisoned".into()))?
            .allocate(size, category)?;

        Ok(AllocationHandle {
            id,
            size,
            category,
            manager: self.inner.clone(),
        })
    }

    /// Get available memory in bytes.
    ///
    /// # Panics
    /// Panics if the inner mutex is poisoned (another thread panicked while holding the lock).
    pub fn available(&self) -> usize {
        let inner = self.inner.lock()
            .expect("GpuMemoryManager mutex poisoned: another thread panicked");
        inner.config.total_budget.saturating_sub(inner.total_allocated)
    }

    /// Get used memory in bytes.
    ///
    /// # Panics
    /// Panics if the inner mutex is poisoned.
    pub fn used(&self) -> usize {
        let inner = self.inner.lock()
            .expect("GpuMemoryManager mutex poisoned: another thread panicked");
        inner.total_allocated
    }

    /// Get total budget in bytes.
    ///
    /// # Panics
    /// Panics if the inner mutex is poisoned.
    pub fn budget(&self) -> usize {
        let inner = self.inner.lock()
            .expect("GpuMemoryManager mutex poisoned: another thread panicked");
        inner.config.total_budget
    }

    /// Get memory statistics.
    ///
    /// # Panics
    /// Panics if the inner mutex is poisoned.
    pub fn stats(&self) -> MemoryStats {
        let inner = self.inner.lock()
            .expect("GpuMemoryManager mutex poisoned: another thread panicked");
        inner.stats()
    }

    /// Check if low memory condition (usage > threshold).
    ///
    /// # Panics
    /// Panics if the inner mutex is poisoned.
    pub fn is_low_memory(&self) -> bool {
        let inner = self.inner.lock()
            .expect("GpuMemoryManager mutex poisoned: another thread panicked");
        let threshold = inner.config.low_memory_threshold;
        let usage = inner.total_allocated as f32 / inner.config.total_budget as f32;
        usage > threshold
    }

    /// Get available memory in specific category.
    ///
    /// # Panics
    /// Panics if the inner mutex is poisoned.
    pub fn category_available(&self, category: MemoryCategory) -> usize {
        let inner = self.inner.lock()
            .expect("GpuMemoryManager mutex poisoned: another thread panicked");
        let budget = inner
            .config
            .category_budgets
            .get(&category)
            .copied()
            .unwrap_or_else(|| category.default_budget());
        let used = inner.category_usage.get(&category).copied().unwrap_or(0);
        budget.saturating_sub(used)
    }

    /// Try to allocate, returning None if insufficient memory.
    pub fn try_allocate(&self, size: usize, category: MemoryCategory) -> Option<AllocationHandle> {
        self.allocate(size, category).ok()
    }
}

impl std::fmt::Debug for GpuMemoryManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stats = self.stats();
        f.debug_struct("GpuMemoryManager")
            .field("used", &stats.total_allocated)
            .field("budget", &stats.total_budget)
            .field("allocations", &stats.allocation_count)
            .field("peak", &stats.peak_usage)
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_category_default_budgets() {
        assert_eq!(
            MemoryCategory::FaissIndex.default_budget(),
            8 * 1024 * 1024 * 1024
        );
        assert_eq!(
            MemoryCategory::HyperbolicCoords.default_budget(),
            2560 * 1024 * 1024
        );
        assert_eq!(
            MemoryCategory::EntailmentCones.default_budget(),
            2764 * 1024 * 1024
        );
        assert_eq!(
            MemoryCategory::WorkingMemory.default_budget(),
            10854 * 1024 * 1024
        );
        assert_eq!(MemoryCategory::Other.default_budget(), 512 * 1024 * 1024);
    }

    #[test]
    fn test_memory_category_names() {
        assert_eq!(MemoryCategory::FaissIndex.name(), "FAISS Index");
        assert_eq!(MemoryCategory::HyperbolicCoords.name(), "Hyperbolic Coords");
        assert_eq!(MemoryCategory::EntailmentCones.name(), "Entailment Cones");
        assert_eq!(MemoryCategory::WorkingMemory.name(), "Working Memory");
        assert_eq!(MemoryCategory::Other.name(), "Other");
    }

    #[test]
    fn test_config_default() {
        let config = GpuMemoryConfig::default();
        assert_eq!(config.total_budget, 24 * 1024 * 1024 * 1024);
        assert!(!config.allow_overallocation);
        assert!((config.low_memory_threshold - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_config_rtx_5090() {
        let config = GpuMemoryConfig::rtx_5090();
        assert_eq!(config.total_budget, 24 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_config_with_budget() {
        let config = GpuMemoryConfig::with_budget(1024 * 1024);
        assert_eq!(config.total_budget, 1024 * 1024);
    }

    #[test]
    fn test_config_category_budget_builder() {
        let config = GpuMemoryConfig::default().category_budget(MemoryCategory::FaissIndex, 1024);
        assert_eq!(
            config.category_budgets.get(&MemoryCategory::FaissIndex),
            Some(&1024)
        );
    }

    #[test]
    fn test_config_validation_zero_budget() {
        let config = GpuMemoryConfig::with_budget(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_threshold() {
        let mut config = GpuMemoryConfig::default();
        config.low_memory_threshold = 0.0;
        assert!(config.validate().is_err());

        config.low_memory_threshold = 1.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_memory_stats_usage_percent() {
        let stats = MemoryStats {
            total_allocated: 500,
            total_budget: 1000,
            ..Default::default()
        };
        assert!((stats.usage_percent() - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_memory_stats_available() {
        let stats = MemoryStats {
            total_allocated: 300,
            total_budget: 1000,
            ..Default::default()
        };
        assert_eq!(stats.available(), 700);
    }

    #[test]
    fn test_memory_stats_is_low_memory() {
        let stats = MemoryStats {
            total_allocated: 95,
            total_budget: 100,
            ..Default::default()
        };
        assert!(stats.is_low_memory(0.9));
        assert!(!stats.is_low_memory(0.99));
    }
}
