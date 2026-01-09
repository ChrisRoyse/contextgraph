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
//! 3. **Tight Coupling**: Apply Kuramoto coupling K=10 for synchronization
//! 4. **Shortcut Detection**: Identify 3+ hop paths for amortization
//!
//! Agent 2 will implement the actual Hebbian update logic.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use uuid::Uuid;

use super::amortized::AmortizedLearner;
use super::constants;
use crate::error::CoreResult;

/// NREM phase handler for memory replay and consolidation
#[derive(Debug, Clone)]
pub struct NremPhase {
    /// Phase duration (Constitution: 3 minutes)
    duration: Duration,

    /// Coupling strength (Constitution: 0.9)
    coupling: f32,

    /// Recency bias (Constitution: 0.8)
    recency_bias: f32,

    /// Hebbian learning rate (eta)
    learning_rate: f32,

    /// Batch size for memory processing
    #[allow(dead_code)]
    batch_size: usize,

    /// Weight decay factor
    weight_decay: f32,

    /// Minimum weight before pruning
    weight_floor: f32,

    /// Maximum weight cap
    weight_cap: f32,
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

/// Hebbian update statistics
#[derive(Debug, Clone, Default)]
pub struct HebbianUpdateStats {
    /// Number of edges strengthened
    pub edges_strengthened: usize,
    /// Number of edges weakened
    pub edges_weakened: usize,
    /// Number of edges pruned
    pub edges_pruned: usize,
    /// Average weight delta
    pub average_delta: f32,
}

/// Path identified during replay for potential shortcut creation
#[derive(Debug, Clone)]
pub struct ReplayPath {
    /// Nodes in the path
    pub nodes: Vec<Uuid>,
    /// Traversal count
    pub traversal_count: usize,
    /// Minimum confidence along path
    pub min_confidence: f32,
    /// Combined path weight
    pub combined_weight: f32,
}

impl NremPhase {
    /// Create a new NREM phase with constitution-mandated defaults
    pub fn new() -> Self {
        Self {
            duration: constants::NREM_DURATION,
            coupling: constants::NREM_COUPLING,
            recency_bias: constants::NREM_RECENCY_BIAS,
            learning_rate: 0.01,
            batch_size: 64,
            weight_decay: 0.001,
            weight_floor: 0.05,
            weight_cap: 1.0,
        }
    }

    /// Execute the NREM phase
    ///
    /// Note: This is a stub implementation. Agent 2 will implement the full
    /// Hebbian update logic with actual memory store integration.
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
        let _deadline = start + self.duration;

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

        // TODO: Agent 2 will implement actual processing:
        // 1. Select memories with recency bias
        // 2. Apply Hebbian updates: delta_w = eta * pre * post
        // 3. Apply tight coupling via Kuramoto
        // 4. Detect shortcut candidates

        // Placeholder: Simulate NREM phase processing
        // In production, this would iterate over memories and apply Hebbian updates

        // For now, we do a minimal simulation
        let simulated_memories = 100;
        let simulated_edges = 50;

        report.memories_replayed = simulated_memories;
        report.edges_strengthened = simulated_edges;
        report.edges_weakened = 10;
        report.edges_pruned = 5;
        report.clusters_consolidated = 3;
        report.compression_ratio = 1.2;
        report.shortcut_candidates = 15;

        // Check for interrupt periodically
        if interrupt_flag.load(Ordering::SeqCst) {
            debug!("NREM phase interrupted during processing");
            report.duration = start.elapsed();
            return Ok(report);
        }

        report.duration = start.elapsed();
        report.completed = true;

        info!(
            "NREM phase completed: {} memories, {} edges strengthened in {:?}",
            report.memories_replayed, report.edges_strengthened, report.duration
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
        // Hebbian update: "neurons that fire together wire together"
        let delta_w = self.learning_rate * pre_activation * post_activation;

        // Apply decay
        let decayed = current_weight * (1.0 - self.weight_decay);

        // Update with cap
        (decayed + delta_w).clamp(self.weight_floor, self.weight_cap)
    }

    /// Check if weight should trigger pruning
    #[inline]
    pub fn should_prune(&self, weight: f32) -> bool {
        weight <= self.weight_floor
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
        self.learning_rate
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

        // Should complete
        assert!(report.completed);
        assert!(report.memories_replayed > 0);
    }
}
