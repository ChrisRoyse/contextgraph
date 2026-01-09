//! Serotonin (5HT) - Mood/Space Weight Modulator
//!
//! Range: [0, 1]
//! Parameter: space_weights (E1-E13)
//! Effect: Scales embedding space weights
//!
//! ## Constitution Reference: neuromod.Serotonin (lines 171-180)
//!
//! Serotonin modulates how many embedding spaces are considered:
//! - High 5HT (1.0): All spaces equally weighted (broad consideration)
//! - Low 5HT (0.0): Only strongest spaces considered (narrow focus)
//!
//! ## Embedding Spaces (E1-E13)
//!
//! The system uses 13 embedding spaces for different semantic dimensions.
//! Serotonin scales the weight of each space, affecting retrieval diversity.
//!
//! ## Homeostatic Regulation
//!
//! Serotonin decays slowly toward baseline (0.5), representing mood stability.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Number of embedding spaces (E1-E13)
pub const NUM_EMBEDDING_SPACES: usize = 13;

/// Serotonin baseline (center of range)
pub const SEROTONIN_BASELINE: f32 = 0.5;

/// Serotonin minimum value
pub const SEROTONIN_MIN: f32 = 0.0;

/// Serotonin maximum value
pub const SEROTONIN_MAX: f32 = 1.0;

/// Serotonin decay rate per second (slow decay for mood stability)
pub const SEROTONIN_DECAY_RATE: f32 = 0.02;

/// Serotonin level state with per-space weights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerotoninLevel {
    /// Current global serotonin value in range [0, 1]
    pub value: f32,
    /// Per-space weight modulation [0,1] for each of E1-E13
    /// These are base weights before serotonin scaling
    pub space_weights: [f32; NUM_EMBEDDING_SPACES],
}

impl Default for SerotoninLevel {
    fn default() -> Self {
        // Initialize all space weights to 1.0 (equal consideration)
        Self {
            value: SEROTONIN_BASELINE,
            space_weights: [1.0; NUM_EMBEDDING_SPACES],
        }
    }
}

/// Serotonin modulator - controls embedding space weight scaling
#[derive(Debug, Clone)]
pub struct SerotoninModulator {
    level: SerotoninLevel,
    decay_rate: f32,
}

impl SerotoninModulator {
    /// Create a new serotonin modulator at baseline
    pub fn new() -> Self {
        Self {
            level: SerotoninLevel::default(),
            decay_rate: SEROTONIN_DECAY_RATE,
        }
    }

    /// Create a modulator with custom decay rate
    pub fn with_decay_rate(decay_rate: f32) -> Self {
        Self {
            level: SerotoninLevel::default(),
            decay_rate: decay_rate.clamp(0.0, 1.0),
        }
    }

    /// Get current serotonin level
    pub fn level(&self) -> &SerotoninLevel {
        &self.level
    }

    /// Get current serotonin value
    pub fn value(&self) -> f32 {
        self.level.value
    }

    /// Get scaled weight for a specific embedding space
    ///
    /// The effective weight is: base_weight * (0.5 + 0.5 * serotonin)
    /// This ensures:
    /// - At serotonin=0: weight = 0.5 * base_weight (narrow focus)
    /// - At serotonin=0.5: weight = 0.75 * base_weight (normal)
    /// - At serotonin=1: weight = 1.0 * base_weight (broad consideration)
    pub fn get_space_weight(&self, space_index: usize) -> f32 {
        if space_index >= NUM_EMBEDDING_SPACES {
            tracing::error!(
                "Invalid embedding space index: {} (max: {})",
                space_index,
                NUM_EMBEDDING_SPACES - 1
            );
            return 0.0;
        }

        let base_weight = self.level.space_weights[space_index];
        let scaling = 0.5 + 0.5 * self.level.value;
        base_weight * scaling
    }

    /// Get all scaled space weights as an array
    pub fn get_all_space_weights(&self) -> [f32; NUM_EMBEDDING_SPACES] {
        let scaling = 0.5 + 0.5 * self.level.value;
        let mut weights = [0.0; NUM_EMBEDDING_SPACES];
        for (i, weight) in weights.iter_mut().enumerate() {
            *weight = self.level.space_weights[i] * scaling;
        }
        weights
    }

    /// Set base weight for a specific embedding space
    pub fn set_space_weight(&mut self, space_index: usize, weight: f32) {
        if space_index >= NUM_EMBEDDING_SPACES {
            tracing::error!(
                "Invalid embedding space index: {} (max: {})",
                space_index,
                NUM_EMBEDDING_SPACES - 1
            );
            return;
        }
        self.level.space_weights[space_index] = weight.clamp(0.0, 1.0);
    }

    /// Adjust serotonin level
    pub fn adjust(&mut self, delta: f32) {
        self.level.value = (self.level.value + delta).clamp(SEROTONIN_MIN, SEROTONIN_MAX);
        tracing::debug!(
            "Serotonin adjusted by {:.3}: new value={:.3}",
            delta,
            self.level.value
        );
    }

    /// Set serotonin value directly
    pub fn set_value(&mut self, value: f32) {
        self.level.value = value.clamp(SEROTONIN_MIN, SEROTONIN_MAX);
    }

    /// Apply homeostatic decay toward baseline
    pub fn decay(&mut self, delta_t: Duration) {
        let dt_secs = delta_t.as_secs_f32();
        let effective_rate = (self.decay_rate * dt_secs).clamp(0.0, 1.0);

        // Exponential decay toward baseline
        self.level.value += (SEROTONIN_BASELINE - self.level.value) * effective_rate;
        self.level.value = self.level.value.clamp(SEROTONIN_MIN, SEROTONIN_MAX);
    }

    /// Reset to baseline
    pub fn reset(&mut self) {
        self.level.value = SEROTONIN_BASELINE;
        self.level.space_weights = [1.0; NUM_EMBEDDING_SPACES];
    }

    /// Positive mood event (increases serotonin)
    pub fn on_positive_event(&mut self, magnitude: f32) {
        let delta = magnitude.abs() * 0.1;
        self.adjust(delta);
    }

    /// Negative mood event (decreases serotonin)
    pub fn on_negative_event(&mut self, magnitude: f32) {
        let delta = -(magnitude.abs() * 0.1);
        self.adjust(delta);
    }
}

impl Default for SerotoninModulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serotonin_initial_baseline() {
        let modulator = SerotoninModulator::new();
        assert!((modulator.value() - SEROTONIN_BASELINE).abs() < f32::EPSILON);
    }

    #[test]
    fn test_serotonin_range_clamping_max() {
        let mut modulator = SerotoninModulator::new();

        // Multiple positive events to hit max
        for _ in 0..100 {
            modulator.on_positive_event(1.0);
        }

        assert!(modulator.value() <= SEROTONIN_MAX);
        assert!((modulator.value() - SEROTONIN_MAX).abs() < f32::EPSILON);
    }

    #[test]
    fn test_serotonin_range_clamping_min() {
        let mut modulator = SerotoninModulator::new();

        // Multiple negative events to hit min
        for _ in 0..100 {
            modulator.on_negative_event(1.0);
        }

        assert!(modulator.value() >= SEROTONIN_MIN);
        assert!((modulator.value() - SEROTONIN_MIN).abs() < f32::EPSILON);
    }

    #[test]
    fn test_serotonin_decay_toward_baseline() {
        let mut modulator = SerotoninModulator::with_decay_rate(0.2); // Use faster decay for test
        modulator.set_value(SEROTONIN_MAX);

        // Decay over time
        for _ in 0..200 {
            modulator.decay(Duration::from_millis(100));
        }

        let diff = (modulator.value() - SEROTONIN_BASELINE).abs();
        assert!(diff < 0.1, "Expected value near baseline, got: {}", modulator.value());
    }

    #[test]
    fn test_serotonin_space_weight_scaling() {
        let mut modulator = SerotoninModulator::new();

        // At baseline (0.5), scaling = 0.5 + 0.5 * 0.5 = 0.75
        let weight_at_baseline = modulator.get_space_weight(0);
        assert!((weight_at_baseline - 0.75).abs() < f32::EPSILON);

        // At max (1.0), scaling = 0.5 + 0.5 * 1.0 = 1.0
        modulator.set_value(SEROTONIN_MAX);
        let weight_at_max = modulator.get_space_weight(0);
        assert!((weight_at_max - 1.0).abs() < f32::EPSILON);

        // At min (0.0), scaling = 0.5 + 0.5 * 0.0 = 0.5
        modulator.set_value(SEROTONIN_MIN);
        let weight_at_min = modulator.get_space_weight(0);
        assert!((weight_at_min - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_serotonin_all_space_weights() {
        let modulator = SerotoninModulator::new();
        let weights = modulator.get_all_space_weights();

        assert_eq!(weights.len(), NUM_EMBEDDING_SPACES);
        for weight in weights.iter() {
            // At baseline, all weights should be 0.75
            assert!((*weight - 0.75).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_serotonin_set_space_weight() {
        let mut modulator = SerotoninModulator::new();
        modulator.set_space_weight(5, 0.5);
        modulator.set_value(SEROTONIN_MAX); // scaling = 1.0

        let weight = modulator.get_space_weight(5);
        assert!((weight - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_serotonin_invalid_space_index() {
        let modulator = SerotoninModulator::new();
        let weight = modulator.get_space_weight(100);
        assert!((weight - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_serotonin_reset() {
        let mut modulator = SerotoninModulator::new();
        modulator.set_value(SEROTONIN_MAX);
        modulator.set_space_weight(0, 0.1);
        modulator.reset();

        assert!((modulator.value() - SEROTONIN_BASELINE).abs() < f32::EPSILON);
        assert!((modulator.level().space_weights[0] - 1.0).abs() < f32::EPSILON);
    }
}
