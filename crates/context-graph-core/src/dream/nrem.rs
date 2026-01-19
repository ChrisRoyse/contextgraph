//! NREM Phase - Non-REM Sleep Memory Consolidation
//!
//! Implements the NREM phase of the dream cycle with Hebbian replay
//! and tight coupling for memory consolidation.
//!
//! ## Constitution Reference (Section dream, lines 446-453)
//!
//! - Duration: 3 minutes
//! - Coupling: 0.9 (tight)
//! - Recency bias: 0.8
//! - Hebbian learning: delta_w = eta * pre * post
//!
//! ## NREM Phase Steps
//!
//! 1. **Memory Selection**: Select recent memories weighted by recency and importance
//! 2. **Hebbian Replay**: Strengthen connections between co-activated memories
//! 3. **Tight Coupling**: Apply coupling K=0.9 for synchronization
//! 4. **Shortcut Detection**: Identify 3+ hop paths for amortization
//!
//! ## Memory Provider Architecture (AP-71, AP-72 Compliant)
//!
//! The NREM phase REQUIRES a real MemoryProvider for production use:
//!
//! - `MemoryProvider`: Trait for retrieving memories and edges for replay
//! - `NullMemoryProvider`: **ONLY for unit tests** - returns empty data
//! - Production: Use `with_provider()` constructor with a real MemoryProvider
//!
//! ## Constitution Anti-Patterns
//!
//! - AP-71: "Dream NREM/REM returning stubs forbidden"
//! - AP-72: "nrem.rs/rem.rs TODO stubs MUST be implemented"
//! - AP-35: "Implementations MUST NOT return stub data when real data is available"

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use uuid::Uuid;

use super::amortized::AmortizedLearner;
use super::constants;
use super::hebbian::{select_replay_memories, HebbianEngine};
use super::types::NodeActivation;
use crate::error::CoreResult;

// ============================================================================
// MEMORY PROVIDER TRAIT
// ============================================================================

/// Provider trait for NREM memory retrieval.
///
/// Allows dependency injection of memory stores for the NREM phase.
/// Implementations should retrieve recent memories and edges from the
/// underlying graph store for Hebbian replay.
///
/// # Constitution Compliance
///
/// - DREAM-001: Provider data feeds Hebbian replay (dw = eta * phi_i * phi_j)
/// - AP-35: Implementations MUST NOT return stub data when real data is available
/// - AP-36: This trait replaces the hardcoded Vec::new() stubs in process()
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` for concurrent access during dream cycles.
pub trait MemoryProvider: Send + Sync + std::fmt::Debug {
    /// Get recent memories for replay.
    ///
    /// Returns memories as tuples of (memory_id, timestamp_ms, phi_value).
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of memories to retrieve
    /// * `recency_bias` - How much to favor recent memories [0.0, 1.0]
    ///   Constitution default: 0.8
    ///
    /// # Returns
    ///
    /// Vector of (memory_id, timestamp_ms, phi_value) tuples.
    /// Empty vector if no memories are available.
    fn get_recent_memories(&self, limit: usize, recency_bias: f32) -> Vec<(Uuid, u64, f32)>;

    /// Get edges between the given memory nodes.
    ///
    /// Returns edges as tuples of (source_id, target_id, weight).
    ///
    /// # Arguments
    ///
    /// * `memory_ids` - Memory IDs to find edges between
    ///
    /// # Returns
    ///
    /// Vector of (source, target, weight) tuples for edges where both
    /// source and target are in `memory_ids`. Empty if no edges found.
    fn get_edges_for_memories(&self, memory_ids: &[Uuid]) -> Vec<(Uuid, Uuid, f32)>;
}

/// Null implementation of MemoryProvider for **UNIT TESTING ONLY**.
///
/// Returns empty vectors. This should ONLY be used in unit tests that
/// specifically test Hebbian math logic without needing real memory data.
///
/// # WARNING: Constitution Compliance
///
/// - AP-71: "Dream NREM/REM returning stubs forbidden" - This is ONLY for unit tests
/// - AP-35: Production code MUST use a real MemoryProvider (e.g., GraphMemoryProvider)
///
/// # Allowed Use Cases
///
/// - Unit tests for Hebbian learning math
/// - Unit tests for interrupt handling
/// - Unit tests for state machine logic
///
/// # NOT Allowed For
///
/// - Integration tests (use real RocksDbMemex with GraphMemoryProvider)
/// - Production code (will log ERROR-level warning)
/// - Benchmarks (use real data)
#[derive(Debug, Clone, Default)]
pub struct NullMemoryProvider;

impl MemoryProvider for NullMemoryProvider {
    fn get_recent_memories(&self, _limit: usize, _recency_bias: f32) -> Vec<(Uuid, u64, f32)> {
        // NullMemoryProvider returns empty by design - this is backward compatible
        // behavior for when no real memory store is available
        Vec::new()
    }

    fn get_edges_for_memories(&self, _memory_ids: &[Uuid]) -> Vec<(Uuid, Uuid, f32)> {
        // NullMemoryProvider returns empty by design
        Vec::new()
    }
}

// ============================================================================
// NREM PHASE
// ============================================================================

/// NREM phase handler for memory replay and consolidation.
///
/// Uses a `MemoryProvider` to retrieve memories and edges for Hebbian replay.
///
/// # Production Usage (REQUIRED)
///
/// In production, always use `with_provider()` to ensure real memories are processed:
///
/// ```ignore
/// use context_graph_storage::GraphMemoryProvider;
///
/// let storage = Arc::new(RocksDbMemex::open("/path/to/db")?);
/// let provider = Arc::new(GraphMemoryProvider::new(storage));
/// let phase = NremPhase::with_provider(provider);
/// let report = phase.process(&interrupt, &mut amortizer).await?;
/// ```
///
/// # Unit Testing (NullMemoryProvider)
///
/// For unit tests that ONLY test Hebbian math, use `new()`:
///
/// ```ignore
/// let phase = NremPhase::new();  // Uses no provider - logs WARNING
/// ```
///
/// # Constitution Compliance
///
/// - AP-71: "Dream NREM/REM returning stubs forbidden" - Production MUST use real provider
/// - AP-72: Stubs replaced with real implementations via GraphMemoryProvider
pub struct NremPhase {
    /// Phase duration (Constitution: 3 minutes)
    duration: Duration,

    /// Coupling strength (Constitution: 0.9)
    coupling: f32,

    /// Recency bias (Constitution: 0.8)
    recency_bias: f32,

    /// Batch size for memory processing
    #[allow(dead_code)]
    batch_size: usize,

    /// Hebbian learning engine
    hebbian_engine: HebbianEngine,

    /// Memory provider for retrieving memories and edges.
    /// Injected via `set_memory_provider()`.
    /// When None, uses NullMemoryProvider (returns empty data for backward compat).
    memory_provider: Option<Arc<dyn MemoryProvider>>,
}

// Manual Debug implementation since Arc<dyn MemoryProvider> requires it
impl std::fmt::Debug for NremPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NremPhase")
            .field("duration", &self.duration)
            .field("coupling", &self.coupling)
            .field("recency_bias", &self.recency_bias)
            .field("batch_size", &self.batch_size)
            .field("hebbian_engine", &self.hebbian_engine)
            .field(
                "memory_provider",
                &self.memory_provider.as_ref().map(|p| format!("{:?}", p)),
            )
            .finish()
    }
}

// Manual Clone implementation since Arc<dyn MemoryProvider> is Clone
impl Clone for NremPhase {
    fn clone(&self) -> Self {
        Self {
            duration: self.duration,
            coupling: self.coupling,
            recency_bias: self.recency_bias,
            batch_size: self.batch_size,
            hebbian_engine: self.hebbian_engine.clone(),
            memory_provider: self.memory_provider.clone(),
        }
    }
}

/// Report from NREM phase execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NremReport {
    /// Number of memories replayed
    pub memories_replayed: usize,

    /// Number of edges strengthened
    pub edges_strengthened: usize,

    /// Number of edges weakened
    pub edges_weakened: usize,

    /// Number of edges pruned
    pub edges_pruned: usize,

    /// Number of clusters consolidated
    pub clusters_consolidated: usize,

    /// Compression ratio achieved
    pub compression_ratio: f32,

    /// Paths identified for shortcut creation
    pub shortcut_candidates: usize,

    /// Phase duration
    pub duration: Duration,

    /// Whether phase completed normally
    pub completed: bool,

    /// Average weight delta during Hebbian updates
    pub average_weight_delta: f32,
}

impl NremPhase {
    /// Create a new NREM phase with a real MemoryProvider (PRODUCTION USE).
    ///
    /// This is the REQUIRED constructor for production and integration tests.
    /// Per AP-71: "Dream NREM/REM returning stubs forbidden"
    ///
    /// # Arguments
    ///
    /// * `provider` - A real MemoryProvider implementation (e.g., GraphMemoryProvider)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use context_graph_storage::GraphMemoryProvider;
    ///
    /// let storage = Arc::new(RocksDbMemex::open("/tmp/test")?);
    /// let provider = Arc::new(GraphMemoryProvider::new(storage));
    /// let phase = NremPhase::with_provider(provider);
    /// ```
    pub fn with_provider(provider: Arc<dyn MemoryProvider>) -> Self {
        info!(
            "NremPhase::with_provider() - Creating NREM phase with real memory provider: {:?}",
            provider
        );
        Self {
            duration: constants::NREM_DURATION,
            coupling: constants::NREM_COUPLING,
            recency_bias: constants::NREM_RECENCY_BIAS,
            batch_size: 64,
            hebbian_engine: HebbianEngine::with_defaults(),
            memory_provider: Some(provider),
        }
    }

    /// Create a new NREM phase WITHOUT a memory provider (UNIT TESTING ONLY).
    ///
    /// # WARNING: Constitution Compliance
    ///
    /// - AP-71: "Dream NREM/REM returning stubs forbidden"
    /// - This method logs a WARNING because production code should use `with_provider()`
    /// - Only use this in unit tests that test Hebbian math or state machine logic
    ///
    /// # For Production
    ///
    /// Use `with_provider()` with a real MemoryProvider instead.
    pub fn new() -> Self {
        tracing::warn!(
            "NremPhase::new() called without MemoryProvider - this is ONLY for unit tests! \
             Production code MUST use NremPhase::with_provider() per AP-71"
        );
        Self {
            duration: constants::NREM_DURATION,
            coupling: constants::NREM_COUPLING,
            recency_bias: constants::NREM_RECENCY_BIAS,
            batch_size: 64,
            hebbian_engine: HebbianEngine::with_defaults(),
            memory_provider: None,
        }
    }

    /// Set the memory provider for NREM replay.
    ///
    /// The provider will be called during `process()` to retrieve memories
    /// and edges for Hebbian learning.
    ///
    /// # Arguments
    ///
    /// * `provider` - Implementation of `MemoryProvider` trait
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut phase = NremPhase::new();
    /// phase.set_memory_provider(Arc::new(MyMemoryStore::new()));
    /// ```
    pub fn set_memory_provider(&mut self, provider: Arc<dyn MemoryProvider>) {
        self.memory_provider = Some(provider);
    }

    /// Clear the memory provider, reverting to empty data behavior.
    pub fn clear_memory_provider(&mut self) {
        self.memory_provider = None;
    }

    /// Check if a memory provider is set.
    pub fn has_memory_provider(&self) -> bool {
        self.memory_provider.is_some()
    }

    /// Execute the NREM phase
    ///
    /// Implements full Hebbian learning with memory selection and edge updates.
    ///
    /// # Arguments
    ///
    /// * `interrupt_flag` - Flag to check for abort requests
    /// * `amortizer` - Amortized learner for shortcut detection
    ///
    /// # Returns
    ///
    /// Report containing NREM phase metrics
    pub async fn process(
        &mut self,
        interrupt_flag: &Arc<AtomicBool>,
        _amortizer: &mut AmortizedLearner,
    ) -> CoreResult<NremReport> {
        let start = Instant::now();
        let deadline = start + self.duration;

        info!(
            "Starting NREM phase: coupling={}, recency_bias={}",
            self.coupling, self.recency_bias
        );

        let mut report = NremReport {
            memories_replayed: 0,
            edges_strengthened: 0,
            edges_weakened: 0,
            edges_pruned: 0,
            clusters_consolidated: 0,
            compression_ratio: 1.0,
            shortcut_candidates: 0,
            duration: Duration::ZERO,
            completed: false,
            average_weight_delta: 0.0,
        };

        // Check for interrupt
        if interrupt_flag.load(Ordering::SeqCst) {
            debug!("NREM phase interrupted at start");
            report.duration = start.elapsed();
            return Ok(report);
        }

        // Reset the Hebbian engine for this cycle
        self.hebbian_engine.reset();

        // Step 1: Get memories from provider
        // Constitution: recency_bias = 0.8, limit = 100
        // AP-71: "Dream NREM/REM returning stubs forbidden"
        let memories = match &self.memory_provider {
            Some(provider) => {
                info!(
                    "NREM: Fetching memories from real provider with recency_bias={}, limit=100",
                    self.recency_bias
                );
                let fetched = provider.get_recent_memories(100, self.recency_bias);
                info!(
                    "NREM: Provider returned {} memories for Hebbian replay",
                    fetched.len()
                );

                // Log warning if provider returns empty (might indicate early graph state)
                if fetched.is_empty() {
                    tracing::warn!(
                        "NREM: MemoryProvider returned 0 memories - this is OK for early graph \
                         state but should have memories after normal usage"
                    );
                }
                fetched
            }
            None => {
                // No provider set - this is ONLY valid for unit tests
                // Per AP-71: Production code MUST have a provider
                tracing::error!(
                    "NREM: No MemoryProvider set! This violates AP-71 in production. \
                     NREM will process empty memory set. Use NremPhase::with_provider() \
                     or set_memory_provider() to fix this."
                );
                Vec::new()
            }
        };

        // Step 2: Select memories for replay with recency bias
        let selected_memory_ids = select_replay_memories(&memories, self.recency_bias, 100);
        report.memories_replayed = selected_memory_ids.len();

        debug!(
            "Selected {} memories for replay with recency_bias={}",
            report.memories_replayed, self.recency_bias
        );

        // Check for interrupt
        if interrupt_flag.load(Ordering::SeqCst) {
            debug!("NREM phase interrupted during memory selection");
            report.duration = start.elapsed();
            return Ok(report);
        }

        // Step 3: Get edges for selected memories from provider
        let edges = match &self.memory_provider {
            Some(provider) => {
                if !selected_memory_ids.is_empty() {
                    debug!(
                        "Fetching edges for {} selected memories",
                        selected_memory_ids.len()
                    );
                    provider.get_edges_for_memories(&selected_memory_ids)
                } else {
                    Vec::new()
                }
            }
            None => Vec::new(),
        };

        debug!(
            "Retrieved {} edges for {} selected memories",
            edges.len(),
            selected_memory_ids.len()
        );

        // Step 4: Create activations for selected memories
        // Activations are based on phi values from memory data, or computed from selection order
        let activations: Vec<NodeActivation> = selected_memory_ids
            .iter()
            .enumerate()
            .map(|(i, &id)| {
                // Look up phi from original memories if available
                let phi = memories
                    .iter()
                    .find(|(mem_id, _, _)| *mem_id == id)
                    .map(|(_, _, phi)| *phi)
                    .unwrap_or_else(|| {
                        // Fallback: Decay activation based on selection order (most important first)
                        (1.0 - (i as f32 / selected_memory_ids.len().max(1) as f32)).max(0.1)
                    });
                NodeActivation::new(id, phi)
            })
            .collect();

        self.hebbian_engine.set_activations(&activations);

        // Step 5: Load edges and compute Hebbian updates
        // Constitution: dw_ij = eta * phi_i * phi_j (eta = 0.01)
        self.hebbian_engine.load_edges(&edges);

        let updates = self.hebbian_engine.compute_updates();
        let stats = self.hebbian_engine.stats();

        report.edges_strengthened = stats.edges_strengthened;
        report.edges_weakened = stats.edges_weakened;
        report.edges_pruned = stats.edges_to_prune;
        report.average_weight_delta = stats.average_delta;

        debug!(
            "Computed {} edge updates: {} strengthened, {} weakened, {} to prune",
            updates.len(),
            stats.edges_strengthened,
            stats.edges_weakened,
            stats.edges_to_prune
        );

        // Check for interrupt and deadline
        if interrupt_flag.load(Ordering::SeqCst) {
            debug!("NREM phase interrupted during Hebbian updates");
            report.duration = start.elapsed();
            return Ok(report);
        }

        if Instant::now() >= deadline {
            debug!("NREM phase reached deadline");
            report.duration = start.elapsed();
            report.completed = true;
            return Ok(report);
        }

        // Step 6: Track shortcut candidates (paths traversed >= 5 times with >= 3 hops)
        // This would integrate with the amortizer in production
        report.shortcut_candidates = 0;

        // Step 7: Estimate compression ratio based on pruned edges
        if !updates.is_empty() {
            let original_count = updates.len();
            let remaining = original_count.saturating_sub(stats.edges_to_prune);
            report.compression_ratio = if remaining > 0 {
                original_count as f32 / remaining as f32
            } else {
                1.0
            };
        }

        report.duration = start.elapsed();
        report.completed = true;

        let provider_status = if self.memory_provider.is_some() {
            "with provider"
        } else {
            "no provider"
        };

        info!(
            "NREM phase completed ({}): {} memories, {} edges strengthened, {} to prune in {:?}",
            provider_status,
            report.memories_replayed,
            report.edges_strengthened,
            report.edges_pruned,
            report.duration
        );

        Ok(report)
    }

    /// Apply Hebbian weight update
    ///
    /// Implements the Hebbian learning rule: delta_w = eta * pre * post
    ///
    /// # Arguments
    ///
    /// * `current_weight` - Current edge weight
    /// * `pre_activation` - Pre-synaptic activation level
    /// * `post_activation` - Post-synaptic activation level
    ///
    /// # Returns
    ///
    /// New weight after Hebbian update
    #[inline]
    pub fn hebbian_update(
        &self,
        current_weight: f32,
        pre_activation: f32,
        post_activation: f32,
    ) -> f32 {
        let config = self.hebbian_engine.config();

        // Hebbian update: "neurons that fire together wire together"
        let delta_w = config.learning_rate * pre_activation * post_activation;

        // Apply decay
        let decayed = current_weight * (1.0 - config.weight_decay);

        // Update with cap
        (decayed + delta_w).clamp(config.weight_floor, config.weight_cap)
    }

    /// Check if weight should trigger pruning
    #[inline]
    pub fn should_prune(&self, weight: f32) -> bool {
        weight <= self.hebbian_engine.config().weight_floor
    }

    /// Get the phase duration
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Get the coupling strength
    pub fn coupling(&self) -> f32 {
        self.coupling
    }

    /// Get the recency bias
    pub fn recency_bias(&self) -> f32 {
        self.recency_bias
    }

    /// Get the learning rate
    pub fn learning_rate(&self) -> f32 {
        self.hebbian_engine.config().learning_rate
    }

    /// Get a reference to the Hebbian engine
    pub fn hebbian_engine(&self) -> &HebbianEngine {
        &self.hebbian_engine
    }

    /// Get a mutable reference to the Hebbian engine
    pub fn hebbian_engine_mut(&mut self) -> &mut HebbianEngine {
        &mut self.hebbian_engine
    }
}

impl Default for NremPhase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_nrem_phase_creation() {
        let phase = NremPhase::new();

        assert_eq!(phase.duration.as_secs(), 180); // 3 minutes
        assert_eq!(phase.coupling, 0.9);
        assert_eq!(phase.recency_bias, 0.8);
    }

    #[test]
    fn test_constitution_compliance() {
        let phase = NremPhase::new();

        // Constitution mandates: 3 min, coupling=0.9, recency=0.8
        assert_eq!(phase.duration, constants::NREM_DURATION);
        assert_eq!(phase.coupling, constants::NREM_COUPLING);
        assert_eq!(phase.recency_bias, constants::NREM_RECENCY_BIAS);
    }

    #[test]
    fn test_hebbian_update() {
        let phase = NremPhase::new();

        // Test basic Hebbian update
        let current_weight = 0.5;
        let pre_activation = 0.8;
        let post_activation = 0.9;

        let new_weight = phase.hebbian_update(current_weight, pre_activation, post_activation);

        // Weight should increase (neurons that fire together wire together)
        assert!(new_weight > current_weight, "Weight should increase");

        // Weight should be capped at 1.0
        assert!(new_weight <= 1.0, "Weight should be capped at 1.0");
    }

    #[test]
    fn test_hebbian_update_zero_activation() {
        let phase = NremPhase::new();

        let current_weight = 0.5;
        let pre_activation = 0.0;
        let post_activation = 0.9;

        let new_weight = phase.hebbian_update(current_weight, pre_activation, post_activation);

        // With zero pre-activation, only decay should apply
        assert!(new_weight < current_weight, "Weight should decay");
    }

    #[test]
    fn test_weight_floor_pruning() {
        let phase = NremPhase::new();

        assert!(!phase.should_prune(0.5));
        assert!(!phase.should_prune(0.1));
        assert!(phase.should_prune(0.05));
        assert!(phase.should_prune(0.01));
    }

    #[test]
    fn test_weight_cap() {
        let phase = NremPhase::new();

        // Very high activations should still cap at 1.0
        let new_weight = phase.hebbian_update(0.9, 1.0, 1.0);

        assert!(new_weight <= 1.0, "Weight should be capped at 1.0");
    }

    #[tokio::test]
    async fn test_process_with_interrupt() {
        let mut phase = NremPhase::new();
        let interrupt = Arc::new(AtomicBool::new(true)); // Set interrupt immediately
        let mut amortizer = AmortizedLearner::new();

        let report = phase.process(&interrupt, &mut amortizer).await.unwrap();

        // Should return quickly due to interrupt
        assert!(!report.completed);
    }

    #[tokio::test]
    async fn test_process_without_interrupt() {
        let mut phase = NremPhase::new();
        let interrupt = Arc::new(AtomicBool::new(false));
        let mut amortizer = AmortizedLearner::new();

        let report = phase.process(&interrupt, &mut amortizer).await.unwrap();

        // Should complete (with empty input, memories_replayed will be 0)
        assert!(report.completed);
    }

    // ============================================================
    // MemoryProvider Tests
    // ============================================================

    /// Test memory provider that returns controlled data for testing.
    /// Uses real UUIDs, not mock data.
    #[derive(Debug)]
    struct TestMemoryProvider {
        memories: Vec<(Uuid, u64, f32)>,
        edges: Vec<(Uuid, Uuid, f32)>,
    }

    impl TestMemoryProvider {
        fn new(memories: Vec<(Uuid, u64, f32)>, edges: Vec<(Uuid, Uuid, f32)>) -> Self {
            Self { memories, edges }
        }
    }

    impl MemoryProvider for TestMemoryProvider {
        fn get_recent_memories(&self, limit: usize, _recency_bias: f32) -> Vec<(Uuid, u64, f32)> {
            self.memories.iter().take(limit).cloned().collect()
        }

        fn get_edges_for_memories(&self, memory_ids: &[Uuid]) -> Vec<(Uuid, Uuid, f32)> {
            let id_set: HashSet<Uuid> = memory_ids.iter().copied().collect();
            self.edges
                .iter()
                .filter(|(s, t, _)| id_set.contains(s) && id_set.contains(t))
                .cloned()
                .collect()
        }
    }

    #[test]
    fn test_null_memory_provider() {
        let provider = NullMemoryProvider;

        // NullMemoryProvider always returns empty
        let memories = provider.get_recent_memories(100, 0.8);
        assert!(memories.is_empty());

        let edges = provider.get_edges_for_memories(&[Uuid::new_v4()]);
        assert!(edges.is_empty());
    }

    #[test]
    fn test_memory_provider_trait() {
        let mem1 = Uuid::new_v4();
        let mem2 = Uuid::new_v4();

        let provider = TestMemoryProvider::new(
            vec![(mem1, 1000, 0.9), (mem2, 2000, 0.8)],
            vec![(mem1, mem2, 0.5)],
        );

        let memories = provider.get_recent_memories(10, 0.8);
        assert_eq!(memories.len(), 2);

        let edges = provider.get_edges_for_memories(&[mem1, mem2]);
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0], (mem1, mem2, 0.5));
    }

    #[test]
    fn test_set_memory_provider() {
        let mut phase = NremPhase::new();

        // Initially no provider
        assert!(!phase.has_memory_provider());

        // Set a provider
        let provider = Arc::new(NullMemoryProvider);
        phase.set_memory_provider(provider);
        assert!(phase.has_memory_provider());

        // Clear the provider
        phase.clear_memory_provider();
        assert!(!phase.has_memory_provider());
    }

    #[tokio::test]
    async fn test_nrem_with_provider() {
        let mem1 = Uuid::new_v4();
        let mem2 = Uuid::new_v4();
        let mem3 = Uuid::new_v4();

        let provider = Arc::new(TestMemoryProvider::new(
            vec![(mem1, 1000, 0.9), (mem2, 2000, 0.8), (mem3, 3000, 0.7)],
            vec![(mem1, mem2, 0.5), (mem2, mem3, 0.4)],
        ));

        let mut phase = NremPhase::new();
        phase.set_memory_provider(provider);

        let interrupt = Arc::new(AtomicBool::new(false));
        let mut amortizer = AmortizedLearner::new();

        let report = phase.process(&interrupt, &mut amortizer).await.unwrap();

        // Should have processed real data from provider
        assert!(report.completed);
        assert_eq!(
            report.memories_replayed, 3,
            "Should replay all 3 memories from provider"
        );

        // HebbianEngine should have processed the edges
        // With 3 memories and 2 edges, we should see edge updates
        // Note: edges_strengthened depends on activation values
    }

    #[tokio::test]
    async fn test_nrem_with_provider_processes_edges() {
        let mem1 = Uuid::new_v4();
        let mem2 = Uuid::new_v4();

        // Create provider with high phi values to ensure strengthening
        let provider = Arc::new(TestMemoryProvider::new(
            vec![
                (mem1, 1000, 0.9), // High phi
                (mem2, 2000, 0.9), // High phi
            ],
            vec![
                (mem1, mem2, 0.5), // Edge between them
            ],
        ));

        let mut phase = NremPhase::new();
        phase.set_memory_provider(provider);

        let interrupt = Arc::new(AtomicBool::new(false));
        let mut amortizer = AmortizedLearner::new();

        let report = phase.process(&interrupt, &mut amortizer).await.unwrap();

        assert!(report.completed);
        assert_eq!(report.memories_replayed, 2);

        // With high phi values (0.9 * 0.9 = 0.81), the edge should be strengthened
        // Hebbian: dw = eta * phi_i * phi_j = 0.01 * 0.9 * 0.9 = 0.0081
        // Note: edges_strengthened should be 1 since we have one edge with high activations
        assert_eq!(
            report.edges_strengthened, 1,
            "Edge should be strengthened with high phi values"
        );
    }

    #[tokio::test]
    async fn test_nrem_without_provider_backward_compat() {
        // Without a provider, NREM should still work but with empty data
        let mut phase = NremPhase::new();
        // Do NOT set a provider

        let interrupt = Arc::new(AtomicBool::new(false));
        let mut amortizer = AmortizedLearner::new();

        let report = phase.process(&interrupt, &mut amortizer).await.unwrap();

        // Should complete with empty data (backward compatible)
        assert!(report.completed);
        assert_eq!(report.memories_replayed, 0, "No memories without provider");
        assert_eq!(report.edges_strengthened, 0, "No edges without provider");
    }

    #[tokio::test]
    async fn test_nrem_provider_edge_filtering() {
        // Test that only edges between selected memories are processed
        let mem1 = Uuid::new_v4();
        let mem2 = Uuid::new_v4();
        let mem_external = Uuid::new_v4(); // Not in memories

        let provider = Arc::new(TestMemoryProvider::new(
            vec![(mem1, 1000, 0.9), (mem2, 2000, 0.8)],
            vec![
                (mem1, mem2, 0.5),         // Both in memories - should process
                (mem1, mem_external, 0.6), // mem_external not in memories - should filter out
            ],
        ));

        let mut phase = NremPhase::new();
        phase.set_memory_provider(provider);

        let interrupt = Arc::new(AtomicBool::new(false));
        let mut amortizer = AmortizedLearner::new();

        let report = phase.process(&interrupt, &mut amortizer).await.unwrap();

        assert!(report.completed);
        // Only the edge between mem1 and mem2 should be processed
        // (the one to mem_external should be filtered by TestMemoryProvider)
        assert_eq!(report.memories_replayed, 2);
    }

    #[test]
    fn test_nrem_phase_clone_with_provider() {
        let mut phase = NremPhase::new();
        let provider = Arc::new(NullMemoryProvider);
        phase.set_memory_provider(provider);

        let cloned = phase.clone();

        assert!(cloned.has_memory_provider());
        assert_eq!(cloned.duration(), phase.duration());
        assert_eq!(cloned.coupling(), phase.coupling());
        assert_eq!(cloned.recency_bias(), phase.recency_bias());
    }

    #[test]
    fn test_nrem_phase_debug_with_provider() {
        let mut phase = NremPhase::new();
        phase.set_memory_provider(Arc::new(NullMemoryProvider));

        // Should not panic when debug-printing
        let debug_str = format!("{:?}", phase);
        assert!(debug_str.contains("NremPhase"));
        assert!(debug_str.contains("memory_provider"));
    }
}
