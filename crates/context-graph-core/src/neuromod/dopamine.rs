//! Dopamine (DA) - Reward/Salience Modulator
//!
//! Range: [1, 5]
//! Parameter: hopfield.beta
//! Trigger: memory_enters_workspace (GWT event)
//!
//! ## Constitution Reference: neuromod.Dopamine (lines 162-170)
//!
//! Dopamine modulates Hopfield network retrieval sharpness:
//! - High DA (5): Sharp, focused retrieval
//! - Low DA (1): Diffuse, exploratory retrieval
//!
//! ## Homeostatic Regulation
//!
//! After each trigger, DA decays exponentially toward baseline (3.0).
//! This implements the "phasic burst" pattern seen in biological systems.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Dopamine baseline (center of range)
pub const DA_BASELINE: f32 = 3.0;

/// Dopamine minimum value
pub const DA_MIN: f32 = 1.0;

/// Dopamine maximum value
pub const DA_MAX: f32 = 5.0;

/// Dopamine decay rate per second (exponential decay toward baseline)
pub const DA_DECAY_RATE: f32 = 0.05;

/// Dopamine increase per workspace entry event
pub const DA_WORKSPACE_INCREMENT: f32 = 0.2;

/// Dopamine level state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DopamineLevel {
    /// Current dopamine value in range [DA_MIN, DA_MAX]
    pub value: f32,
    /// Timestamp of last trigger event
    pub last_trigger: Option<DateTime<Utc>>,
}

impl Default for DopamineLevel {
    fn default() -> Self {
        Self {
            value: DA_BASELINE,
            last_trigger: None,
        }
    }
}

/// Dopamine modulator - controls Hopfield beta parameter
#[derive(Debug, Clone)]
pub struct DopamineModulator {
    level: DopamineLevel,
    decay_rate: f32,
}

impl DopamineModulator {
    /// Create a new dopamine modulator at baseline
    pub fn new() -> Self {
        Self {
            level: DopamineLevel::default(),
            decay_rate: DA_DECAY_RATE,
        }
    }

    /// Create a modulator with custom decay rate
    pub fn with_decay_rate(decay_rate: f32) -> Self {
        Self {
            level: DopamineLevel::default(),
            decay_rate: decay_rate.clamp(0.0, 1.0),
        }
    }

    /// Get current dopamine level
    pub fn level(&self) -> &DopamineLevel {
        &self.level
    }

    /// Get current dopamine value
    pub fn value(&self) -> f32 {
        self.level.value
    }

    /// Increase dopamine (called on memory_enters_workspace)
    /// Constitution: Dopamine += 0.2 per workspace entry
    pub fn on_workspace_entry(&mut self) {
        self.level.value = (self.level.value + DA_WORKSPACE_INCREMENT).clamp(DA_MIN, DA_MAX);
        self.level.last_trigger = Some(Utc::now());
        tracing::debug!(
            "Dopamine increased on workspace entry: value={:.3}",
            self.level.value
        );
    }

    /// Decrease dopamine (for negative reinforcement)
    pub fn on_negative_event(&mut self, magnitude: f32) {
        let delta = magnitude.abs() * 0.1;
        self.level.value = (self.level.value - delta).clamp(DA_MIN, DA_MAX);
        tracing::debug!(
            "Dopamine decreased on negative event: value={:.3}",
            self.level.value
        );
    }

    /// Get current hopfield.beta value
    /// This is the primary parameter controlled by dopamine
    pub fn get_hopfield_beta(&self) -> f32 {
        self.level.value
    }

    /// Apply homeostatic decay toward baseline
    ///
    /// Implements exponential decay: value += (baseline - value) * rate * dt
    /// This ensures smooth convergence without overshooting.
    pub fn decay(&mut self, delta_t: Duration) {
        let dt_secs = delta_t.as_secs_f32();
        // Scale decay by time elapsed
        let effective_rate = (self.decay_rate * dt_secs).clamp(0.0, 1.0);

        // Exponential decay toward baseline
        self.level.value += (DA_BASELINE - self.level.value) * effective_rate;
        self.level.value = self.level.value.clamp(DA_MIN, DA_MAX);
    }

    /// Reset to baseline (used in testing or forced reset)
    pub fn reset(&mut self) {
        self.level.value = DA_BASELINE;
        self.level.last_trigger = None;
    }

    /// Set dopamine value directly (for testing or initialization)
    pub fn set_value(&mut self, value: f32) {
        self.level.value = value.clamp(DA_MIN, DA_MAX);
    }
}

impl Default for DopamineModulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dopamine_initial_baseline() {
        let modulator = DopamineModulator::new();
        assert!((modulator.value() - DA_BASELINE).abs() < f32::EPSILON);
    }

    #[test]
    fn test_dopamine_workspace_entry() {
        let mut modulator = DopamineModulator::new();
        let initial = modulator.value();

        modulator.on_workspace_entry();

        assert!(modulator.value() > initial);
        assert!((modulator.value() - (initial + DA_WORKSPACE_INCREMENT)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_dopamine_range_clamping_max() {
        let mut modulator = DopamineModulator::new();

        // Trigger many workspace entries to hit max
        for _ in 0..100 {
            modulator.on_workspace_entry();
        }

        assert!(modulator.value() <= DA_MAX);
        assert!((modulator.value() - DA_MAX).abs() < f32::EPSILON);
    }

    #[test]
    fn test_dopamine_range_clamping_min() {
        let mut modulator = DopamineModulator::new();
        modulator.set_value(DA_MIN);

        // Strong negative event
        modulator.on_negative_event(100.0);

        assert!(modulator.value() >= DA_MIN);
    }

    #[test]
    fn test_dopamine_decay_toward_baseline() {
        let mut modulator = DopamineModulator::with_decay_rate(0.2); // Use faster decay for test
        modulator.set_value(DA_MAX);

        // Decay over several seconds
        for _ in 0..100 {
            modulator.decay(Duration::from_millis(100));
        }

        // Should approach baseline (with 0.2 rate, 10 seconds of decay should get close)
        let diff = (modulator.value() - DA_BASELINE).abs();
        assert!(
            diff < 0.5,
            "Expected value near baseline, got: {}",
            modulator.value()
        );
    }

    #[test]
    fn test_dopamine_decay_from_min() {
        let mut modulator = DopamineModulator::new();
        modulator.set_value(DA_MIN);

        // Decay should increase toward baseline
        for _ in 0..100 {
            modulator.decay(Duration::from_millis(100));
        }

        assert!(
            modulator.value() > DA_MIN,
            "Expected value above min, got: {}",
            modulator.value()
        );
    }

    #[test]
    fn test_dopamine_hopfield_beta() {
        let mut modulator = DopamineModulator::new();
        modulator.set_value(4.5);

        let beta = modulator.get_hopfield_beta();
        assert!((beta - 4.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_dopamine_reset() {
        let mut modulator = DopamineModulator::new();
        modulator.set_value(DA_MAX);
        modulator.reset();

        assert!((modulator.value() - DA_BASELINE).abs() < f32::EPSILON);
        assert!(modulator.level().last_trigger.is_none());
    }
}
