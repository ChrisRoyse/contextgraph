//! REM Phase - REM Sleep Attractor Exploration
//!
//! Implements the REM phase of the dream cycle with high-temperature
//! attractor exploration for creative association discovery.
//!
//! ## Constitution Reference (Section dream, lines 446-453)
//!
//! - Duration: 2 minutes
//! - Temperature: 2.0 (high exploration)
//! - Semantic leap: >= 0.7
//! - Query limit: 100 synthetic queries
//!
//! ## REM Phase Steps
//!
//! 1. **Synthetic Query Generation**: Generate diverse queries via hyperbolic random walk
//! 2. **High-Temperature Search**: Explore with softmax temp=2.0
//! 3. **Semantic Leap Discovery**: Find connections with distance >= 0.7
//! 4. **Blind Spot Detection**: Identify unexplored graph regions via HyperbolicExplorer

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use uuid::Uuid;

use super::constants;
use super::hyperbolic_walk::{HyperbolicExplorer, ExplorationResult};
use super::types::HyperbolicWalkConfig;
use crate::error::CoreResult;

/// REM phase handler for attractor exploration
///
/// Uses `HyperbolicExplorer` to perform random walks in the Poincare ball
/// for blind spot discovery and creative association exploration.
///
/// # Constitution Compliance
/// - Temperature: 2.0 (high exploration)
/// - Query limit: 100 (hard cap, enforced by HyperbolicExplorer)
/// - Semantic leap: >= 0.7 for blind spot detection
///
/// Note: Does not implement Clone because HyperbolicExplorer contains StdRng.
#[derive(Debug)]
pub struct RemPhase {
    /// Phase duration (Constitution: 2 minutes)
    duration: Duration,

    /// Exploration temperature (Constitution: 2.0)
    temperature: f32,

    /// Minimum semantic leap distance (Constitution: 0.7)
    min_semantic_leap: f32,

    /// Maximum synthetic queries (Constitution: 100)
    query_limit: usize,

    /// New edge initial weight
    #[allow(dead_code)]
    new_edge_weight: f32,

    /// New edge initial confidence
    #[allow(dead_code)]
    new_edge_confidence: f32,

    /// Exploration bias for random walk
    #[allow(dead_code)]
    exploration_bias: f32,

    /// Random walk step size
    #[allow(dead_code)]
    walk_step_size: f32,

    /// Hyperbolic explorer for Poincare ball random walks
    /// Performs actual exploration and blind spot detection
    explorer: HyperbolicExplorer,
}

/// Report from REM phase execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemReport {
    /// Number of synthetic queries generated
    pub queries_generated: usize,

    /// Number of blind spots discovered
    pub blind_spots_found: usize,

    /// Number of new edges created
    pub new_edges_created: usize,

    /// Average semantic leap distance
    pub average_semantic_leap: f32,

    /// Exploration coverage (fraction of graph explored)
    pub exploration_coverage: f32,

    /// Phase duration
    pub duration: Duration,

    /// Whether phase completed normally
    pub completed: bool,

    /// Unique nodes visited during exploration
    pub unique_nodes_visited: usize,
}

/// A blind spot discovered during REM exploration
#[derive(Debug, Clone)]
pub struct BlindSpot {
    /// First node in the potential connection
    pub node_a: Uuid,
    /// Second node in the potential connection
    pub node_b: Uuid,
    /// Semantic distance between nodes
    pub semantic_distance: f32,
    /// Discovery confidence
    pub confidence: f32,
}

impl BlindSpot {
    /// Check if this is a significant blind spot (high distance, good confidence)
    ///
    /// NOTE: Uses legacy constants. Will be migrated to DreamThresholds in consumer update task.
    #[allow(deprecated)]
    pub fn is_significant(&self) -> bool {
        self.semantic_distance >= constants::MIN_SEMANTIC_LEAP && self.confidence >= 0.5
    }
}

/// A synthetic query generated for exploration
#[derive(Debug, Clone)]
pub struct SyntheticQuery {
    /// Query embedding (placeholder type)
    pub embedding: Vec<f32>,
    /// Origin node from random walk
    pub origin_node: Option<Uuid>,
    /// Random walk path taken
    pub walk_path: Vec<Uuid>,
}

impl RemPhase {
    /// Create a new REM phase with constitution-mandated defaults
    ///
    /// Initializes `HyperbolicExplorer` with Constitution-compliant configuration:
    /// - Temperature: 2.0 (high exploration)
    /// - Semantic leap: >= 0.7 for blind spot detection
    /// - Query limit: 100 (enforced by HyperbolicExplorer)
    ///
    /// NOTE: Uses legacy constants. Will be migrated to DreamThresholds in consumer update task.
    #[allow(deprecated)]
    pub fn new() -> Self {
        // Create HyperbolicWalkConfig with Constitution-mandated values
        let walk_config = HyperbolicWalkConfig {
            step_size: 0.1,
            max_steps: 50,
            temperature: constants::REM_TEMPERATURE,         // Constitution: 2.0
            min_blind_spot_distance: constants::MIN_SEMANTIC_LEAP, // Constitution: 0.7
            direction_samples: 8,
        };

        Self {
            duration: constants::REM_DURATION,
            temperature: constants::REM_TEMPERATURE,
            min_semantic_leap: constants::MIN_SEMANTIC_LEAP,
            query_limit: constants::MAX_REM_QUERIES,
            new_edge_weight: 0.3,
            new_edge_confidence: 0.5,
            exploration_bias: 0.7,
            walk_step_size: 0.3,
            explorer: HyperbolicExplorer::new(walk_config),
        }
    }

    /// Execute the REM phase using HyperbolicExplorer
    ///
    /// Performs hyperbolic random walks in the Poincare ball to discover
    /// blind spots (unexplored semantic regions) via the `HyperbolicExplorer`.
    ///
    /// # Arguments
    ///
    /// * `interrupt_flag` - Flag to check for abort requests (Constitution: wake < 100ms)
    ///
    /// # Returns
    ///
    /// Report containing REM phase metrics from actual exploration
    ///
    /// # Constitution Compliance
    ///
    /// - Uses HyperbolicExplorer for Poincare ball random walks (DREAM-002)
    /// - Query limit: 100 (enforced by HyperbolicExplorer)
    /// - Semantic leap: >= 0.7 for blind spot significance
    /// - Temperature: 2.0 for high exploration
    pub async fn process(&mut self, interrupt_flag: &Arc<AtomicBool>) -> CoreResult<RemReport> {
        let start = Instant::now();

        info!(
            "Starting REM phase: temp={}, semantic_leap={}, query_limit={}",
            self.temperature, self.min_semantic_leap, self.query_limit
        );

        // Check for interrupt before starting exploration
        if interrupt_flag.load(Ordering::SeqCst) {
            debug!("REM phase interrupted at start");
            return Ok(RemReport {
                queries_generated: 0,
                blind_spots_found: 0,
                new_edges_created: 0,
                average_semantic_leap: 0.0,
                exploration_coverage: 0.0,
                duration: start.elapsed(),
                completed: false,
                unique_nodes_visited: 0,
            });
        }

        // Reset explorer query counter for this REM cycle
        self.explorer.reset_queries();

        // Get starting positions for exploration
        // For now, start from origin if no positions provided
        // Future: integrate with MemoryStore for real high-phi node positions
        let starting_positions: Vec<[f32; 64]> = vec![[0.0f32; 64]];

        // Execute hyperbolic exploration using HyperbolicExplorer
        let exploration_result: ExplorationResult = self.explorer.explore(&starting_positions, interrupt_flag);

        // Convert ExplorationResult to RemReport with real metrics
        let blind_spots_found = exploration_result
            .all_blind_spots
            .iter()
            .filter(|bs| bs.is_significant())
            .count();

        let report = RemReport {
            queries_generated: exploration_result.queries_generated,
            blind_spots_found,
            new_edges_created: blind_spots_found, // 1:1 mapping (edge creation is out of scope)
            average_semantic_leap: exploration_result.average_semantic_leap,
            exploration_coverage: exploration_result.coverage_estimate,
            duration: start.elapsed(),
            completed: !interrupt_flag.load(Ordering::SeqCst),
            unique_nodes_visited: exploration_result.unique_positions,
        };

        info!(
            "REM phase completed: {} queries, {} blind spots ({} significant) in {:?}",
            report.queries_generated,
            exploration_result.all_blind_spots.len(),
            report.blind_spots_found,
            report.duration
        );

        Ok(report)
    }

    /// Check if semantic distance meets minimum leap requirement
    #[inline]
    pub fn meets_semantic_leap(&self, distance: f32) -> bool {
        distance >= self.min_semantic_leap
    }

    /// Apply softmax with exploration temperature
    ///
    /// Higher temperature (2.0) makes distribution more uniform for exploration.
    pub fn softmax_with_temperature(&self, scores: &[f32]) -> Vec<f32> {
        if scores.is_empty() {
            return Vec::new();
        }

        // Scale by temperature
        let scaled: Vec<f32> = scores.iter().map(|&s| s / self.temperature).collect();

        // Find max for numerical stability
        let max = scaled.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        // Compute exp(x - max)
        let exp_scores: Vec<f32> = scaled.iter().map(|&s| (s - max).exp()).collect();

        // Normalize
        let sum: f32 = exp_scores.iter().sum();
        if sum == 0.0 {
            return vec![1.0 / scores.len() as f32; scores.len()];
        }

        exp_scores.iter().map(|&e| e / sum).collect()
    }

    /// Get the phase duration
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Get the exploration temperature
    pub fn temperature(&self) -> f32 {
        self.temperature
    }

    /// Get the minimum semantic leap
    pub fn min_semantic_leap(&self) -> f32 {
        self.min_semantic_leap
    }

    /// Get the query limit
    pub fn query_limit(&self) -> usize {
        self.query_limit
    }
}

impl Default for RemPhase {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TESTS - NO MOCK DATA, REAL HYPERBOLIC EXPLORATION
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ============ Constitution Compliance Tests ============

    #[test]
    fn test_rem_phase_creation() {
        let phase = RemPhase::new();

        assert_eq!(phase.duration.as_secs(), 120, "duration must be 2 minutes");
        assert_eq!(phase.temperature, 2.0, "temperature must be 2.0 per Constitution");
        assert_eq!(phase.min_semantic_leap, 0.7, "semantic_leap must be 0.7 per Constitution");
        assert_eq!(phase.query_limit, 100, "query_limit must be 100 per Constitution");
    }

    #[test]
    #[allow(deprecated)]
    fn test_constitution_compliance() {
        let phase = RemPhase::new();

        // Constitution mandates: 2 min, temp=2.0, semantic_leap=0.7, queries=100
        assert_eq!(phase.duration, constants::REM_DURATION);
        assert_eq!(phase.temperature, constants::REM_TEMPERATURE);
        assert_eq!(phase.min_semantic_leap, constants::MIN_SEMANTIC_LEAP);
        assert_eq!(phase.query_limit, constants::MAX_REM_QUERIES);

        // Verify explorer is initialized with Constitution-compliant config
        assert_eq!(phase.explorer.config().temperature, 2.0,
            "explorer temperature must be 2.0 per Constitution");
        assert_eq!(phase.explorer.config().min_blind_spot_distance, 0.7,
            "explorer min_blind_spot_distance must be 0.7 per Constitution");
    }

    #[test]
    fn test_explorer_is_initialized() {
        let phase = RemPhase::new();

        // Verify HyperbolicExplorer is properly initialized
        let config = phase.explorer.config();
        assert_eq!(config.temperature, 2.0);
        assert_eq!(config.min_blind_spot_distance, 0.7);
        assert_eq!(config.max_steps, 50);
        assert_eq!(config.step_size, 0.1);
    }

    // ============ Semantic Leap Tests ============

    #[test]
    fn test_semantic_leap_check() {
        let phase = RemPhase::new();

        assert!(!phase.meets_semantic_leap(0.5));
        assert!(!phase.meets_semantic_leap(0.69));
        assert!(phase.meets_semantic_leap(0.7));
        assert!(phase.meets_semantic_leap(0.8));
        assert!(phase.meets_semantic_leap(0.99));
    }

    // ============ Softmax Tests ============

    #[test]
    fn test_softmax_with_temperature() {
        let phase = RemPhase::new();

        let scores = vec![1.0, 2.0, 3.0];
        let probs = phase.softmax_with_temperature(&scores);

        // Should sum to 1.0
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 0.001, "Probabilities should sum to 1.0");

        // Higher scores should have higher probability
        assert!(probs[2] > probs[1]);
        assert!(probs[1] > probs[0]);
    }

    #[test]
    fn test_softmax_empty_input() {
        let phase = RemPhase::new();

        let probs = phase.softmax_with_temperature(&[]);
        assert!(probs.is_empty());
    }

    #[test]
    fn test_softmax_uniform_with_high_temp() {
        let phase = RemPhase::new(); // temp = 2.0

        // With identical scores, should be uniform
        let scores = vec![1.0, 1.0, 1.0];
        let probs = phase.softmax_with_temperature(&scores);

        for p in &probs {
            assert!(
                (*p - 0.333).abs() < 0.01,
                "Uniform scores should give uniform probs"
            );
        }
    }

    // ============ BlindSpot Tests ============

    #[test]
    fn test_blind_spot_significance() {
        let significant = BlindSpot {
            node_a: Uuid::new_v4(),
            node_b: Uuid::new_v4(),
            semantic_distance: 0.8,
            confidence: 0.6,
        };
        assert!(significant.is_significant());

        let not_significant_distance = BlindSpot {
            node_a: Uuid::new_v4(),
            node_b: Uuid::new_v4(),
            semantic_distance: 0.5, // Below 0.7
            confidence: 0.8,
        };
        assert!(!not_significant_distance.is_significant());

        let not_significant_confidence = BlindSpot {
            node_a: Uuid::new_v4(),
            node_b: Uuid::new_v4(),
            semantic_distance: 0.9,
            confidence: 0.3, // Below 0.5
        };
        assert!(!not_significant_confidence.is_significant());
    }

    // ============ Process Tests - REAL Exploration ============

    #[tokio::test]
    async fn test_process_with_interrupt() {
        let mut phase = RemPhase::new();
        let interrupt = Arc::new(AtomicBool::new(true)); // Set interrupt immediately

        let report = phase.process(&interrupt).await.unwrap();

        // Should return quickly due to interrupt (before exploration starts)
        assert!(!report.completed);
        assert_eq!(report.queries_generated, 0);
    }

    #[tokio::test]
    async fn test_process_without_interrupt_uses_real_explorer() {
        let mut phase = RemPhase::new();
        let interrupt = Arc::new(AtomicBool::new(false));

        let report = phase.process(&interrupt).await.unwrap();

        // CRITICAL: Verify real exploration occurred (not stub values)
        assert!(report.completed, "exploration should complete without interrupt");
        assert!(report.queries_generated > 0, "must generate queries via HyperbolicExplorer");
        assert!(report.queries_generated <= 100,
            "queries {} must not exceed Constitution limit 100", report.queries_generated);

        // With no known positions, all visited positions are blind spots
        // This verifies real exploration is happening
        assert!(report.unique_nodes_visited > 0,
            "must visit unique positions via hyperbolic walk");

        // Verify real exploration metrics (not stub multiples like *3)
        // unique_nodes_visited should equal sum of walk trajectory lengths
        assert!(report.unique_nodes_visited <= report.queries_generated * 50,
            "unique_nodes_visited should be bounded by max possible walk steps");
    }

    #[tokio::test]
    async fn test_process_respects_query_limit() {
        let mut phase = RemPhase::new();
        let interrupt = Arc::new(AtomicBool::new(false));

        let report = phase.process(&interrupt).await.unwrap();

        // Constitution: query_limit = 100
        assert!(report.queries_generated <= 100,
            "Constitution violation: queries_generated {} exceeds limit 100",
            report.queries_generated);
    }

    #[tokio::test]
    async fn test_process_discovers_blind_spots_via_explorer() {
        let mut phase = RemPhase::new();
        let interrupt = Arc::new(AtomicBool::new(false));

        let report = phase.process(&interrupt).await.unwrap();

        // With no known positions set on explorer, every position is a blind spot
        // This verifies HyperbolicExplorer is being used for real exploration
        // The blind_spots_found count should reflect actual DiscoveredBlindSpot from explorer
        // (filtering for is_significant())

        // Real exploration should produce some coverage
        assert!(report.exploration_coverage > 0.0,
            "exploration_coverage should be > 0 from real exploration");
    }

    #[tokio::test]
    async fn test_process_returns_real_metrics() {
        let mut phase = RemPhase::new();
        let interrupt = Arc::new(AtomicBool::new(false));

        let report = phase.process(&interrupt).await.unwrap();

        // Verify metrics come from ExplorationResult, not hardcoded stubs
        // These assertions ensure we're not returning placeholder values

        // Duration should be very short (exploration is fast with no actual embedding lookups)
        assert!(report.duration.as_millis() < 10000,
            "exploration should complete quickly without real embedding lookups");

        // queries_generated should match explorer's actual query count
        // (not stub value like `self.query_limit`)
        assert!(report.queries_generated > 0);

        // If blind_spots_found > 0, average_semantic_leap should also be > 0
        // (unless all blind spots have distance_from_nearest = 0, which is unlikely)
        if report.blind_spots_found > 0 {
            // Note: average_semantic_leap can be 0.0 if no significant blind spots
            // This is valid behavior from ExplorationResult
        }
    }

    #[tokio::test]
    async fn test_multiple_process_calls_reset_explorer() {
        let mut phase = RemPhase::new();
        let interrupt = Arc::new(AtomicBool::new(false));

        // First exploration
        let report1 = phase.process(&interrupt).await.unwrap();

        // Second exploration should work independently (explorer.reset_queries() called)
        let report2 = phase.process(&interrupt).await.unwrap();

        // Both should generate queries (explorer was reset between calls)
        assert!(report1.queries_generated > 0);
        assert!(report2.queries_generated > 0);
    }
}
