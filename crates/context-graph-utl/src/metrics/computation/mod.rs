//! Accumulated statistics from UTL computations.

use serde::{Deserialize, Serialize};

use crate::johari::JohariQuadrant;
use crate::lifecycle::{LifecycleLambdaWeights, LifecycleStage};

use super::QuadrantDistribution;

/// Accumulated statistics from UTL computations.
///
/// **DISTINCT FROM `UtlMetrics`**: This struct tracks AGGREGATE statistics
/// across multiple computations. `UtlMetrics` (in context-graph-core) captures
/// the values for a SINGLE UTL computation.
///
/// # Example
///
/// ```
/// use context_graph_utl::metrics::UtlComputationMetrics;
/// use context_graph_utl::johari::JohariQuadrant;
///
/// let mut metrics = UtlComputationMetrics::new();
/// assert_eq!(metrics.computation_count, 0);
/// assert!(metrics.is_healthy());
///
/// metrics.record_computation(0.7, 0.5, 0.6, JohariQuadrant::Open, 1000.0);
/// assert_eq!(metrics.computation_count, 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UtlComputationMetrics {
    /// Total number of UTL computations performed
    pub computation_count: u64,

    /// Running average of learning magnitude L [0.0, 1.0]
    pub avg_learning_magnitude: f32,

    /// Running average of surprise (delta_s) [0.0, 1.0]
    pub avg_delta_s: f32,

    /// Running average of coherence change (delta_c) [0.0, 1.0]
    pub avg_delta_c: f32,

    /// Distribution of Johari quadrant classifications
    pub quadrant_distribution: QuadrantDistribution,

    /// Current lifecycle stage (Infancy, Growth, Maturity)
    pub lifecycle_stage: LifecycleStage,

    /// Current Marblestone lambda weights
    pub lambda_weights: LifecycleLambdaWeights,

    /// Average computation latency in microseconds
    pub avg_latency_us: f64,

    /// 99th percentile latency in microseconds
    pub p99_latency_us: u64,
}

impl Default for UtlComputationMetrics {
    fn default() -> Self {
        Self {
            computation_count: 0,
            avg_learning_magnitude: 0.0,
            avg_delta_s: 0.0,
            avg_delta_c: 0.0,
            quadrant_distribution: QuadrantDistribution::default(),
            lifecycle_stage: LifecycleStage::default(),
            lambda_weights: LifecycleLambdaWeights::default(),
            avg_latency_us: 0.0,
            p99_latency_us: 0,
        }
    }
}

impl UtlComputationMetrics {
    /// Create new empty metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all metrics to initial state.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Get the dominant quadrant (most frequent classification).
    pub fn dominant_quadrant(&self) -> JohariQuadrant {
        self.quadrant_distribution.dominant()
    }

    /// Calculate learning efficiency (magnitude per microsecond x 1000).
    ///
    /// Returns 0.0 if no latency data or if latency is zero.
    pub fn learning_efficiency(&self) -> f64 {
        if self.avg_latency_us > 0.0 && !self.avg_latency_us.is_nan() {
            (self.avg_learning_magnitude as f64) / self.avg_latency_us * 1000.0
        } else {
            0.0
        }
    }

    /// Check if metrics indicate healthy operation.
    ///
    /// Healthy when:
    /// - Average latency < 10ms (10,000 us)
    /// - P99 latency < 50ms (50,000 us)
    /// - Learning magnitude is a valid number
    pub fn is_healthy(&self) -> bool {
        self.avg_latency_us < 10_000.0
            && self.p99_latency_us < 50_000
            && !self.avg_learning_magnitude.is_nan()
            && !self.avg_learning_magnitude.is_infinite()
    }

    /// Update running averages with a new computation result.
    ///
    /// Uses exponential moving average with alpha = 0.1 for smooth updates.
    pub fn record_computation(
        &mut self,
        learning_magnitude: f32,
        delta_s: f32,
        delta_c: f32,
        quadrant: JohariQuadrant,
        latency_us: f64,
    ) {
        const ALPHA: f32 = 0.1;
        const ALPHA_F64: f64 = 0.1;

        self.computation_count = self.computation_count.saturating_add(1);

        // Exponential moving average for smooth updates
        if self.computation_count == 1 {
            self.avg_learning_magnitude = learning_magnitude;
            self.avg_delta_s = delta_s;
            self.avg_delta_c = delta_c;
            self.avg_latency_us = latency_us;
        } else {
            self.avg_learning_magnitude =
                ALPHA * learning_magnitude + (1.0 - ALPHA) * self.avg_learning_magnitude;
            self.avg_delta_s = ALPHA * delta_s + (1.0 - ALPHA) * self.avg_delta_s;
            self.avg_delta_c = ALPHA * delta_c + (1.0 - ALPHA) * self.avg_delta_c;
            self.avg_latency_us = ALPHA_F64 * latency_us + (1.0 - ALPHA_F64) * self.avg_latency_us;
        }

        self.quadrant_distribution.increment(quadrant);

        // Update p99 (simplified: track max as approximation)
        let latency_u64 = latency_us as u64;
        if latency_u64 > self.p99_latency_us {
            self.p99_latency_us = latency_u64;
        }
    }
}

#[cfg(test)]
mod tests;
