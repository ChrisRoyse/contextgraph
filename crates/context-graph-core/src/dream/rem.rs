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
//! 1. **Synthetic Query Generation**: Generate diverse queries via random walk
//! 2. **High-Temperature Search**: Explore with softmax temp=2.0
//! 3. **Semantic Leap Discovery**: Find connections with distance >= 0.7
//! 4. **Blind Spot Detection**: Identify unexplored graph regions
//!
//! Agent 2 will implement the actual exploration logic.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use uuid::Uuid;

use super::constants;
use crate::error::CoreResult;

/// REM phase handler for attractor exploration
#[derive(Debug, Clone)]
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
    pub fn new() -> Self {
        Self {
            duration: constants::REM_DURATION,
            temperature: constants::REM_TEMPERATURE,
            min_semantic_leap: constants::MIN_SEMANTIC_LEAP,
            query_limit: constants::MAX_REM_QUERIES,
            new_edge_weight: 0.3,
            new_edge_confidence: 0.5,
            exploration_bias: 0.7,
            walk_step_size: 0.3,
        }
    }

    /// Execute the REM phase
    ///
    /// Note: This is a stub implementation. Agent 2 will implement the full
    /// exploration logic with actual graph integration.
    ///
    /// # Arguments
    ///
    /// * `interrupt_flag` - Flag to check for abort requests
    ///
    /// # Returns
    ///
    /// Report containing REM phase metrics
    pub async fn process(&mut self, interrupt_flag: &Arc<AtomicBool>) -> CoreResult<RemReport> {
        let start = Instant::now();
        let deadline = start + self.duration;

        info!(
            "Starting REM phase: temp={}, semantic_leap={}, query_limit={}",
            self.temperature, self.min_semantic_leap, self.query_limit
        );

        let mut report = RemReport {
            queries_generated: 0,
            blind_spots_found: 0,
            new_edges_created: 0,
            average_semantic_leap: 0.0,
            exploration_coverage: 0.0,
            duration: Duration::ZERO,
            completed: false,
            unique_nodes_visited: 0,
        };

        // Check for interrupt
        if interrupt_flag.load(Ordering::SeqCst) {
            debug!("REM phase interrupted at start");
            report.duration = start.elapsed();
            return Ok(report);
        }

        // TODO: Agent 2 will implement actual processing:
        // 1. Generate synthetic queries via random walk
        // 2. Search with high temperature (2.0)
        // 3. Filter for semantic leap >= 0.7
        // 4. Create new edges for discovered connections
        // 5. Track blind spots

        // Placeholder: Simulate REM phase processing
        let mut queries_generated = 0;
        let mut blind_spots_found = 0;
        let mut semantic_leaps = Vec::new();

        // Simulate query generation up to limit
        while queries_generated < self.query_limit {
            // Check for interrupt periodically
            if interrupt_flag.load(Ordering::SeqCst) {
                debug!("REM phase interrupted during exploration");
                break;
            }

            // Simulate query and discovery
            queries_generated += 1;

            // Simulate occasional blind spot discovery
            if queries_generated % 10 == 0 {
                blind_spots_found += 1;
                semantic_leaps.push(0.75 + (queries_generated as f32 * 0.001));
            }

            // Check deadline
            if Instant::now() >= deadline {
                break;
            }
        }

        report.queries_generated = queries_generated;
        report.blind_spots_found = blind_spots_found;
        report.new_edges_created = blind_spots_found;
        report.average_semantic_leap = if semantic_leaps.is_empty() {
            0.0
        } else {
            semantic_leaps.iter().sum::<f32>() / semantic_leaps.len() as f32
        };
        report.exploration_coverage = (queries_generated as f32 / self.query_limit as f32).min(1.0);
        report.unique_nodes_visited = queries_generated * 3; // Estimate
        report.duration = start.elapsed();
        report.completed = queries_generated >= self.query_limit || Instant::now() >= deadline;

        info!(
            "REM phase completed: {} queries, {} blind spots in {:?}",
            report.queries_generated, report.blind_spots_found, report.duration
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rem_phase_creation() {
        let phase = RemPhase::new();

        assert_eq!(phase.duration.as_secs(), 120); // 2 minutes
        assert_eq!(phase.temperature, 2.0);
        assert_eq!(phase.min_semantic_leap, 0.7);
        assert_eq!(phase.query_limit, 100);
    }

    #[test]
    fn test_constitution_compliance() {
        let phase = RemPhase::new();

        // Constitution mandates: 2 min, temp=2.0, semantic_leap=0.7, queries=100
        assert_eq!(phase.duration, constants::REM_DURATION);
        assert_eq!(phase.temperature, constants::REM_TEMPERATURE);
        assert_eq!(phase.min_semantic_leap, constants::MIN_SEMANTIC_LEAP);
        assert_eq!(phase.query_limit, constants::MAX_REM_QUERIES);
    }

    #[test]
    fn test_semantic_leap_check() {
        let phase = RemPhase::new();

        assert!(!phase.meets_semantic_leap(0.5));
        assert!(!phase.meets_semantic_leap(0.69));
        assert!(phase.meets_semantic_leap(0.7));
        assert!(phase.meets_semantic_leap(0.8));
        assert!(phase.meets_semantic_leap(0.99));
    }

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

        // With temp=2.0, distribution should be more uniform than temp=1.0
        // The ratio between max and min should be smaller
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

    #[tokio::test]
    async fn test_process_with_interrupt() {
        let mut phase = RemPhase::new();
        let interrupt = Arc::new(AtomicBool::new(true)); // Set interrupt immediately

        let report = phase.process(&interrupt).await.unwrap();

        // Should return quickly due to interrupt
        assert!(!report.completed);
        assert_eq!(report.queries_generated, 0);
    }

    #[tokio::test]
    async fn test_process_without_interrupt() {
        let mut phase = RemPhase::new();
        let interrupt = Arc::new(AtomicBool::new(false));

        let report = phase.process(&interrupt).await.unwrap();

        // Should complete with queries generated
        assert!(report.completed);
        assert!(report.queries_generated > 0);
        assert!(report.queries_generated <= 100); // Query limit
    }
}
