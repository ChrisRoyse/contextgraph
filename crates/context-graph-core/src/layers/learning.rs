//! L4 Learning Layer - UTL-driven weight optimization.
//!
//! The Learning layer implements the Unified Theory of Learning (UTL) weight
//! optimization formula: W' = W + η*(S⊗C_w)
//!
//! # Constitution Compliance
//!
//! - Latency budget: <10ms
//! - Frequency: 100Hz
//! - Gradient clipping: 1.0
//! - Components: UTL optimizer, neuromod controller
//! - UTL: L optimization (weight updates based on surprise × coherence)
//!
//! # Critical Rules
//!
//! - NO BACKWARDS COMPATIBILITY: System works or fails fast
//! - NO MOCK DATA: Returns real weight updates or proper errors
//! - NO FALLBACKS: If UTL computation fails, ERROR OUT
//!
//! # UTL Weight Update Formula
//!
//! The canonical weight update: W' = W + η*(S⊗C_w)
//! Where:
//! - W = current weight
//! - η = learning rate (0.0005 from constitution)
//! - S = surprise signal (from L1 delta_s × L3 novelty)
//! - C_w = weighted coherence (from pulse)
//! - ⊗ = element-wise product (Hadamard, scalar for global signal)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::error::{CoreError, CoreResult};
use crate::traits::NervousLayer;
use crate::types::{LayerId, LayerInput, LayerOutput, LayerResult};

// ============================================================
// Constants from Constitution
// ============================================================

/// Default learning rate (η) from constitution utl.constants.eta
pub const DEFAULT_LEARNING_RATE: f32 = 0.0005;

/// Consolidation threshold - trigger when weight delta exceeds this
pub const DEFAULT_CONSOLIDATION_THRESHOLD: f32 = 0.1;

/// Gradient clipping value from constitution (L4_Learning.grad_clip)
pub const GRADIENT_CLIP: f32 = 1.0;

/// Target frequency in Hz (100Hz = 10ms period)
pub const TARGET_FREQUENCY_HZ: u32 = 100;

// ============================================================
// Weight Delta - Result of UTL computation
// ============================================================

/// Weight delta from UTL computation: Δw = η*(S⊗C_w)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightDelta {
    /// The computed weight change: η*(S⊗C_w)
    pub value: f32,

    /// Surprise component (S) used in computation
    pub surprise: f32,

    /// Weighted coherence component (C_w) used in computation
    pub coherence_w: f32,

    /// Learning rate (η) used in computation
    pub learning_rate: f32,

    /// Whether gradient clipping was applied
    pub was_clipped: bool,
}

impl WeightDelta {
    /// Get the absolute magnitude of the weight delta.
    pub fn magnitude(&self) -> f32 {
        self.value.abs()
    }

    /// Check if this delta should trigger consolidation.
    pub fn should_consolidate(&self, threshold: f32) -> bool {
        self.magnitude() > threshold
    }
}

// ============================================================
// UTL Weight Computer - Core computation engine
// ============================================================

/// UTL Weight Computer - implements W' = W + η*(S⊗C_w)
///
/// This is the core computation engine for L4 Learning.
/// It computes weight updates based on surprise and coherence signals.
///
/// # Formula
///
/// W' = W + η*(S⊗C_w)
///
/// Where:
/// - η = learning rate (default 0.0005)
/// - S = surprise signal [0, 1]
/// - C_w = weighted coherence [0, 1]
/// - ⊗ = element-wise product (scalar here for global signal)
///
/// # Gradient Clipping
///
/// The delta is clipped to [-1.0, 1.0] per constitution.
#[derive(Debug, Clone)]
pub struct UtlWeightComputer {
    /// Learning rate (η)
    learning_rate: f32,

    /// Gradient clipping bound
    grad_clip: f32,
}

impl UtlWeightComputer {
    /// Create a new UTL weight computer with specified learning rate.
    pub fn new(learning_rate: f32) -> Self {
        Self {
            learning_rate,
            grad_clip: GRADIENT_CLIP,
        }
    }

    /// Create with custom gradient clipping bound.
    pub fn with_grad_clip(mut self, clip: f32) -> Self {
        self.grad_clip = clip.abs().max(0.01); // Ensure positive minimum
        self
    }

    /// Compute weight update: Δw = η*(S⊗C_w)
    ///
    /// # Arguments
    ///
    /// * `surprise` - Surprise signal S from sensing/memory layers
    /// * `coherence_w` - Weighted coherence C_w from pulse
    ///
    /// # Returns
    ///
    /// WeightDelta containing the computed delta and metadata.
    ///
    /// # Errors
    ///
    /// Returns error for invalid (NaN/Infinity) inputs per AP-009.
    pub fn compute_update(&self, surprise: f32, coherence_w: f32) -> CoreResult<WeightDelta> {
        // Validate inputs - NO silent fallbacks per AP-009
        if surprise.is_nan() || surprise.is_infinite() {
            return Err(CoreError::UtlError(format!(
                "Invalid surprise value: {} - NaN/Infinity not allowed per AP-009",
                surprise
            )));
        }
        if coherence_w.is_nan() || coherence_w.is_infinite() {
            return Err(CoreError::UtlError(format!(
                "Invalid coherence value: {} - NaN/Infinity not allowed per AP-009",
                coherence_w
            )));
        }

        // Clamp inputs to valid range
        let s = surprise.clamp(0.0, 1.0);
        let c = coherence_w.clamp(0.0, 1.0);

        // S⊗C_w (element-wise product, scalar for global signal)
        let learning_signal = s * c;

        // η*(S⊗C_w)
        let raw_delta = self.learning_rate * learning_signal;

        // Apply gradient clipping
        let (delta, was_clipped) = if raw_delta.abs() > self.grad_clip {
            (raw_delta.signum() * self.grad_clip, true)
        } else {
            (raw_delta, false)
        };

        Ok(WeightDelta {
            value: delta,
            surprise: s,
            coherence_w: c,
            learning_rate: self.learning_rate,
            was_clipped,
        })
    }

    /// Get the current learning rate.
    pub fn learning_rate(&self) -> f32 {
        self.learning_rate
    }

    /// Get the gradient clip bound.
    pub fn grad_clip(&self) -> f32 {
        self.grad_clip
    }
}

impl Default for UtlWeightComputer {
    fn default() -> Self {
        Self::new(DEFAULT_LEARNING_RATE)
    }
}

// ============================================================
// L4 Learning Layer
// ============================================================

/// L4 Learning Layer - UTL-driven weight optimization.
///
/// This layer computes weight updates based on surprise and coherence signals
/// following the UTL formula: W' = W + η*(S⊗C_w)
///
/// # Constitution Compliance
///
/// - Latency: <10ms (CRITICAL)
/// - Frequency: 100Hz
/// - Grad clip: 1.0
/// - Components: UTL optimizer (this), neuromod controller (external)
///
/// # No Fallbacks
///
/// Per AP-007: If UTL computation fails, this layer returns an error.
/// Invalid inputs (NaN/Infinity) are rejected per AP-009.
///
/// # Consolidation Trigger
///
/// When weight delta magnitude exceeds the consolidation threshold,
/// the layer signals that consolidation should occur (for L5/dream).
#[derive(Debug)]
pub struct LearningLayer {
    /// UTL weight computation engine
    weight_computer: UtlWeightComputer,

    /// Consolidation threshold - signal consolidation when exceeded
    consolidation_threshold: f32,

    /// Total layer processing time in microseconds
    total_processing_us: AtomicU64,

    /// Total layer invocations
    invocation_count: AtomicU64,

    /// Total consolidation triggers
    consolidation_triggers: AtomicU64,
}

impl LearningLayer {
    /// Create a new Learning layer with default configuration.
    pub fn new() -> Self {
        Self {
            weight_computer: UtlWeightComputer::default(),
            consolidation_threshold: DEFAULT_CONSOLIDATION_THRESHOLD,
            total_processing_us: AtomicU64::new(0),
            invocation_count: AtomicU64::new(0),
            consolidation_triggers: AtomicU64::new(0),
        }
    }

    /// Create with custom learning rate.
    pub fn with_learning_rate(mut self, rate: f32) -> Self {
        self.weight_computer = UtlWeightComputer::new(rate);
        self
    }

    /// Create with custom consolidation threshold.
    pub fn with_consolidation_threshold(mut self, threshold: f32) -> Self {
        self.consolidation_threshold = threshold.clamp(0.01, 1.0);
        self
    }

    /// Get the current learning rate.
    pub fn learning_rate(&self) -> f32 {
        self.weight_computer.learning_rate()
    }

    /// Get the consolidation threshold.
    pub fn consolidation_threshold(&self) -> f32 {
        self.consolidation_threshold
    }

    /// Get the number of consolidation triggers.
    pub fn consolidation_trigger_count(&self) -> u64 {
        self.consolidation_triggers.load(Ordering::Relaxed)
    }

    /// Get average processing time in microseconds.
    pub fn avg_processing_us(&self) -> f64 {
        let count = self.invocation_count.load(Ordering::Relaxed);
        let total = self.total_processing_us.load(Ordering::Relaxed);
        if count > 0 {
            total as f64 / count as f64
        } else {
            0.0
        }
    }

    /// Extract surprise signal from layer context.
    ///
    /// Computes surprise from:
    /// - L1 Sensing: delta_s (entropy change)
    /// - L3 Memory: novelty (if available)
    ///
    /// Combined: S = delta_s × novelty
    fn compute_surprise(&self, context: &crate::types::LayerContext) -> CoreResult<f32> {
        // Get delta_s from L1 Sensing result
        let delta_s = context
            .layer_results
            .iter()
            .find(|r| r.layer == LayerId::Sensing)
            .and_then(|r| r.data.get("delta_s"))
            .and_then(|v| v.as_f64())
            .map(|v| v as f32);

        // Get novelty from L3 Memory if available
        let memory_novelty = context
            .layer_results
            .iter()
            .find(|r| r.layer == LayerId::Memory)
            .and_then(|r| r.data.get("novelty"))
            .and_then(|v| v.as_f64())
            .map(|v| v as f32);

        // If we have retrieval count from memory, compute novelty from that
        let retrieval_novelty = context
            .layer_results
            .iter()
            .find(|r| r.layer == LayerId::Memory)
            .and_then(|r| r.data.get("retrieval_count"))
            .and_then(|v| v.as_u64())
            .map(|count| {
                // More retrievals = less novel (inverse relationship)
                // 0 retrievals = high novelty (1.0)
                // 10+ retrievals = low novelty (~0.1)
                1.0 / (1.0 + count as f32 * 0.1)
            });

        // Combine available signals
        let surprise = match (delta_s, memory_novelty.or(retrieval_novelty)) {
            (Some(ds), Some(nov)) => ds * nov,
            (Some(ds), None) => ds,
            (None, Some(nov)) => nov,
            (None, None) => {
                // Use pulse entropy as fallback surprise indicator
                context.pulse.entropy
            }
        };

        Ok(surprise.clamp(0.0, 1.0))
    }
}

impl Default for LearningLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NervousLayer for LearningLayer {
    async fn process(&self, input: LayerInput) -> CoreResult<LayerOutput> {
        let start = Instant::now();

        // Compute surprise from context
        let surprise = self.compute_surprise(&input.context)?;

        // Get coherence from pulse (weighted coherence C_w)
        let coherence_w = input.context.pulse.coherence;

        // Compute weight update: Δw = η*(S⊗C_w)
        let weight_delta = self.weight_computer.compute_update(surprise, coherence_w)?;

        // Check consolidation trigger
        let should_consolidate = weight_delta.should_consolidate(self.consolidation_threshold);

        if should_consolidate {
            self.consolidation_triggers.fetch_add(1, Ordering::Relaxed);
        }

        let duration = start.elapsed();
        let duration_us = duration.as_micros() as u64;

        // Record metrics
        self.total_processing_us
            .fetch_add(duration_us, Ordering::Relaxed);
        self.invocation_count.fetch_add(1, Ordering::Relaxed);

        // Check latency budget
        let budget = self.latency_budget();
        if duration > budget {
            tracing::warn!(
                "LearningLayer exceeded latency budget: {:?} > {:?}",
                duration,
                budget
            );
        }

        // Update pulse with learning information
        let mut updated_pulse = input.context.pulse.clone();
        // Coherence improves slightly with positive learning signal
        if weight_delta.value > 0.0 {
            updated_pulse.coherence =
                (updated_pulse.coherence + weight_delta.value * 0.1).clamp(0.0, 1.0);
        }
        // Update coherence delta to reflect the learning
        updated_pulse.coherence_delta = weight_delta.value;

        // Build result data
        let result_data = serde_json::json!({
            "weight_delta": weight_delta.value,
            "surprise": weight_delta.surprise,
            "coherence_w": weight_delta.coherence_w,
            "learning_rate": weight_delta.learning_rate,
            "was_clipped": weight_delta.was_clipped,
            "should_consolidate": should_consolidate,
            "consolidation_threshold": self.consolidation_threshold,
            "duration_us": duration_us,
            "within_budget": duration <= budget,
        });

        Ok(LayerOutput {
            layer: LayerId::Learning,
            result: LayerResult::success(LayerId::Learning, result_data),
            pulse: updated_pulse,
            duration_us,
        })
    }

    fn latency_budget(&self) -> Duration {
        Duration::from_millis(10) // 10ms budget per constitution
    }

    fn layer_id(&self) -> LayerId {
        LayerId::Learning
    }

    fn layer_name(&self) -> &'static str {
        "Learning Layer"
    }

    async fn health_check(&self) -> CoreResult<bool> {
        // Verify UTL computation works with valid inputs
        let test = self.weight_computer.compute_update(0.5, 0.5);
        if test.is_err() {
            return Ok(false);
        }

        // Verify invalid inputs are rejected
        let nan_test = self.weight_computer.compute_update(f32::NAN, 0.5);
        if nan_test.is_ok() {
            return Ok(false); // Should have failed
        }

        Ok(true)
    }
}

// ============================================================
// Tests - REAL implementations, NO MOCKS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // UtlWeightComputer Tests
    // ============================================================

    #[test]
    fn test_utl_weight_computation() {
        let computer = UtlWeightComputer::new(0.0005);
        let result = computer.compute_update(0.8, 0.6).unwrap();

        // η*(S⊗C_w) = 0.0005 * (0.8 * 0.6) = 0.0005 * 0.48 = 0.00024
        let expected = 0.0005 * 0.8 * 0.6;
        assert!(
            (result.value - expected).abs() < 1e-9,
            "Expected {}, got {}",
            expected,
            result.value
        );
        assert!(!result.was_clipped);
        println!(
            "[VERIFIED] UTL weight computation: η*(S⊗C_w) = {}",
            result.value
        );
    }

    #[test]
    fn test_utl_zero_surprise() {
        let computer = UtlWeightComputer::new(0.0005);
        let result = computer.compute_update(0.0, 0.9).unwrap();

        // Zero surprise = zero delta
        assert!(result.value.abs() < 1e-9);
        println!("[VERIFIED] Zero surprise produces zero delta");
    }

    #[test]
    fn test_utl_zero_coherence() {
        let computer = UtlWeightComputer::new(0.0005);
        let result = computer.compute_update(0.9, 0.0).unwrap();

        // Zero coherence = zero delta
        assert!(result.value.abs() < 1e-9);
        println!("[VERIFIED] Zero coherence produces zero delta");
    }

    #[test]
    fn test_utl_max_values() {
        let computer = UtlWeightComputer::new(0.0005);
        let result = computer.compute_update(1.0, 1.0).unwrap();

        // η*(1.0*1.0) = 0.0005
        assert!((result.value - 0.0005).abs() < 1e-9);
        println!("[VERIFIED] Max inputs: δ = {}", result.value);
    }

    #[test]
    fn test_utl_gradient_clipping() {
        // Use a very high learning rate to trigger clipping
        let computer = UtlWeightComputer::new(10.0); // Way too high
        let result = computer.compute_update(1.0, 1.0).unwrap();

        // Should be clipped to GRADIENT_CLIP (1.0)
        assert!(
            result.value.abs() <= GRADIENT_CLIP + 1e-6,
            "Should be clipped, got {}",
            result.value
        );
        assert!(result.was_clipped, "Should have been clipped");
        println!("[VERIFIED] Gradient clipping works: {} -> 1.0", 10.0);
    }

    #[test]
    fn test_utl_nan_surprise_rejected() {
        let computer = UtlWeightComputer::new(0.0005);
        let result = computer.compute_update(f32::NAN, 0.5);

        assert!(result.is_err(), "NaN surprise should be rejected");
        println!("[VERIFIED] NaN surprise rejected per AP-009");
    }

    #[test]
    fn test_utl_nan_coherence_rejected() {
        let computer = UtlWeightComputer::new(0.0005);
        let result = computer.compute_update(0.5, f32::NAN);

        assert!(result.is_err(), "NaN coherence should be rejected");
        println!("[VERIFIED] NaN coherence rejected per AP-009");
    }

    #[test]
    fn test_utl_infinity_rejected() {
        let computer = UtlWeightComputer::new(0.0005);

        assert!(computer.compute_update(f32::INFINITY, 0.5).is_err());
        assert!(computer.compute_update(f32::NEG_INFINITY, 0.5).is_err());
        assert!(computer.compute_update(0.5, f32::INFINITY).is_err());
        assert!(computer.compute_update(0.5, f32::NEG_INFINITY).is_err());

        println!("[VERIFIED] Infinity values rejected per AP-009");
    }

    #[test]
    fn test_utl_input_clamping() {
        let computer = UtlWeightComputer::new(0.0005);

        // Values > 1.0 should be clamped
        let result = computer.compute_update(2.0, 1.5).unwrap();
        assert_eq!(result.surprise, 1.0, "Surprise should be clamped to 1.0");
        assert_eq!(result.coherence_w, 1.0, "Coherence should be clamped to 1.0");

        // Values < 0.0 should be clamped
        let result = computer.compute_update(-0.5, -0.3).unwrap();
        assert_eq!(result.surprise, 0.0, "Surprise should be clamped to 0.0");
        assert_eq!(result.coherence_w, 0.0, "Coherence should be clamped to 0.0");

        println!("[VERIFIED] Input values clamped to [0, 1]");
    }

    // ============================================================
    // WeightDelta Tests
    // ============================================================

    #[test]
    fn test_weight_delta_magnitude() {
        let delta = WeightDelta {
            value: -0.5,
            surprise: 0.8,
            coherence_w: 0.6,
            learning_rate: 0.0005,
            was_clipped: false,
        };

        assert!((delta.magnitude() - 0.5).abs() < 1e-6);
        println!("[VERIFIED] WeightDelta.magnitude() = |value|");
    }

    #[test]
    fn test_weight_delta_consolidation() {
        let delta = WeightDelta {
            value: 0.15,
            surprise: 0.8,
            coherence_w: 0.9,
            learning_rate: 0.5,
            was_clipped: false,
        };

        assert!(delta.should_consolidate(0.1));
        assert!(!delta.should_consolidate(0.2));
        println!("[VERIFIED] WeightDelta.should_consolidate() checks threshold");
    }

    // ============================================================
    // LearningLayer Tests
    // ============================================================

    #[tokio::test]
    async fn test_learning_layer_process() {
        let layer = LearningLayer::new();
        let input = LayerInput::new("test-123".to_string(), "test content".to_string());

        let result = layer.process(input).await.unwrap();

        assert_eq!(result.layer, LayerId::Learning);
        assert!(result.result.success);
        assert!(result.result.data.get("weight_delta").is_some());
        assert!(result.result.data.get("surprise").is_some());
        assert!(result.result.data.get("coherence_w").is_some());

        println!("[VERIFIED] LearningLayer.process() returns valid output");
    }

    #[tokio::test]
    async fn test_learning_layer_with_context() {
        let layer = LearningLayer::new();

        // Create input with L1 and L3 context
        let mut input = LayerInput::new("test-456".to_string(), "test content".to_string());
        input.context.layer_results.push(LayerResult::success(
            LayerId::Sensing,
            serde_json::json!({
                "delta_s": 0.7,
                "scrubbed": false,
            }),
        ));
        input.context.layer_results.push(LayerResult::success(
            LayerId::Memory,
            serde_json::json!({
                "retrieval_count": 2,
            }),
        ));
        input.context.pulse.coherence = 0.8;

        let result = layer.process(input).await.unwrap();

        assert!(result.result.success);

        let delta = result.result.data["weight_delta"].as_f64().unwrap();
        assert!(delta > 0.0, "Should have positive delta with high surprise/coherence");

        println!(
            "[VERIFIED] LearningLayer uses L1/L3 context: δ = {}",
            delta
        );
    }

    #[tokio::test]
    async fn test_learning_layer_properties() {
        let layer = LearningLayer::new();

        assert_eq!(layer.layer_id(), LayerId::Learning);
        assert_eq!(layer.latency_budget(), Duration::from_millis(10));
        assert_eq!(layer.layer_name(), "Learning Layer");
        assert!((layer.learning_rate() - DEFAULT_LEARNING_RATE).abs() < 1e-9);

        println!("[VERIFIED] LearningLayer properties correct");
    }

    #[tokio::test]
    async fn test_learning_layer_health_check() {
        let layer = LearningLayer::new();
        let healthy = layer.health_check().await.unwrap();

        assert!(healthy, "LearningLayer should be healthy");
        println!("[VERIFIED] health_check passes");
    }

    #[tokio::test]
    async fn test_learning_layer_custom_config() {
        let layer = LearningLayer::new()
            .with_learning_rate(0.001)
            .with_consolidation_threshold(0.05);

        assert!((layer.learning_rate() - 0.001).abs() < 1e-9);
        assert!((layer.consolidation_threshold() - 0.05).abs() < 1e-9);

        println!("[VERIFIED] Custom configuration works");
    }

    #[tokio::test]
    async fn test_consolidation_trigger() {
        // Use high learning rate to trigger consolidation
        let layer = LearningLayer::new()
            .with_learning_rate(1.0)
            .with_consolidation_threshold(0.01);

        let mut input = LayerInput::new("test-789".to_string(), "trigger consolidation".to_string());
        input.context.pulse.entropy = 0.9; // High surprise
        input.context.pulse.coherence = 0.9; // High coherence

        let result = layer.process(input).await.unwrap();

        let should_consolidate = result.result.data["should_consolidate"]
            .as_bool()
            .unwrap();
        assert!(
            should_consolidate,
            "Should trigger consolidation with high delta"
        );
        assert!(layer.consolidation_trigger_count() > 0);

        println!("[VERIFIED] Consolidation triggers correctly");
    }

    // ============================================================
    // Performance Benchmark - CRITICAL <10ms
    // ============================================================

    #[tokio::test]
    async fn test_learning_layer_latency_benchmark() {
        let layer = LearningLayer::new();

        let iterations = 10_000;
        let mut total_us: u64 = 0;
        let mut max_us: u64 = 0;

        for i in 0..iterations {
            let mut input = LayerInput::new(
                format!("bench-{}", i),
                format!("Benchmark content {}", i),
            );
            input.context.pulse.entropy = (i as f32 / iterations as f32).clamp(0.0, 1.0);
            input.context.pulse.coherence = 0.5 + (i as f32 / iterations as f32 * 0.5);

            let start = Instant::now();
            let _ = layer.process(input).await;
            let elapsed = start.elapsed().as_micros() as u64;

            total_us += elapsed;
            max_us = max_us.max(elapsed);
        }

        let avg_us = total_us / iterations as u64;

        println!("Learning Layer Benchmark Results:");
        println!("  Iterations: {}", iterations);
        println!("  Avg latency: {} us", avg_us);
        println!("  Max latency: {} us", max_us);
        println!("  Budget: 10000 us (10ms)");

        // Average should be well under budget
        assert!(
            avg_us < 10_000,
            "Average latency {} us exceeds 10ms budget",
            avg_us
        );

        // Even max should be under budget for a well-behaved layer
        assert!(
            max_us < 10_000,
            "Max latency {} us exceeds 10ms budget",
            max_us
        );

        println!("[VERIFIED] Average latency {} us < 10000 us budget", avg_us);
    }

    #[test]
    fn test_utl_computation_benchmark() {
        let computer = UtlWeightComputer::new(DEFAULT_LEARNING_RATE);

        let iterations = 100_000;
        let start = Instant::now();

        for i in 0..iterations {
            let s = (i as f32 / iterations as f32).clamp(0.0, 1.0);
            let c = 1.0 - s;
            let _ = computer.compute_update(s, c);
        }

        let total_us = start.elapsed().as_micros();
        let avg_ns = (total_us * 1000) / iterations as u128;

        println!("UTL Computation Benchmark:");
        println!("  Iterations: {}", iterations);
        println!("  Total time: {} us", total_us);
        println!("  Avg per op: {} ns", avg_ns);

        // Each computation should be sub-microsecond
        assert!(avg_ns < 1000, "UTL computation too slow: {} ns", avg_ns);

        println!("[VERIFIED] UTL computation avg {} ns < 1000 ns", avg_ns);
    }

    // ============================================================
    // Integration Tests
    // ============================================================

    #[tokio::test]
    async fn test_learning_layer_full_pipeline_context() {
        let layer = LearningLayer::new();

        // Simulate full L1 -> L2 -> L3 -> L4 pipeline context
        let mut input = LayerInput::new("pipeline-test".to_string(), "Full pipeline test".to_string());

        // L1 Sensing result
        input.context.layer_results.push(LayerResult::success(
            LayerId::Sensing,
            serde_json::json!({
                "delta_s": 0.6,
                "scrubbed_content": "Full pipeline test",
                "pii_found": false,
                "duration_us": 100,
            }),
        ));

        // L2 Reflex result (cache miss)
        input.context.layer_results.push(LayerResult::success(
            LayerId::Reflex,
            serde_json::json!({
                "cache_hit": false,
                "query_norm": 1.0,
            }),
        ));

        // L3 Memory result
        input.context.layer_results.push(LayerResult::success(
            LayerId::Memory,
            serde_json::json!({
                "retrieval_count": 3,
                "memories": [],
                "duration_us": 500,
            }),
        ));

        // Set pulse state
        input.context.pulse.coherence = 0.75;
        input.context.pulse.entropy = 0.55;

        let result = layer.process(input).await.unwrap();

        assert!(result.result.success);

        // Verify all expected fields
        let data = &result.result.data;
        assert!(data.get("weight_delta").is_some());
        assert!(data.get("surprise").is_some());
        assert!(data.get("coherence_w").is_some());
        assert!(data.get("learning_rate").is_some());
        assert!(data.get("should_consolidate").is_some());
        assert!(data.get("duration_us").is_some());
        assert!(data.get("within_budget").is_some());

        // Verify reasonable values
        let delta = data["weight_delta"].as_f64().unwrap() as f32;
        let surprise = data["surprise"].as_f64().unwrap() as f32;
        let coherence = data["coherence_w"].as_f64().unwrap() as f32;

        assert!((0.0..=1.0).contains(&surprise));
        assert!((0.0..=1.0).contains(&coherence));
        assert!(delta.abs() <= GRADIENT_CLIP);

        println!("[VERIFIED] Full pipeline context processed correctly");
        println!("  Surprise: {}", surprise);
        println!("  Coherence: {}", coherence);
        println!("  Delta: {}", delta);
    }

    #[tokio::test]
    async fn test_pulse_update_on_positive_learning() {
        let layer = LearningLayer::new().with_learning_rate(0.1); // Higher rate for visible effect

        let mut input = LayerInput::new("pulse-test".to_string(), "Test pulse update".to_string());
        input.context.pulse.coherence = 0.5;
        input.context.pulse.entropy = 0.8; // High surprise

        let initial_coherence = input.context.pulse.coherence;
        let result = layer.process(input).await.unwrap();

        // Positive learning should slightly increase coherence
        let weight_delta = result.result.data["weight_delta"].as_f64().unwrap() as f32;
        if weight_delta > 0.0 {
            // Coherence should increase (or stay same if at max)
            assert!(
                result.pulse.coherence >= initial_coherence || result.pulse.coherence >= 0.99,
                "Coherence should increase with positive learning"
            );
        }

        // coherence_delta should reflect the weight delta
        assert!(
            (result.pulse.coherence_delta - weight_delta).abs() < 1e-6,
            "coherence_delta should equal weight_delta"
        );

        println!("[VERIFIED] Pulse updated correctly on positive learning");
    }
}
