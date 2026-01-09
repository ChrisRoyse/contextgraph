//! Noradrenaline (NE) - Alertness/Attention Temperature Modulator
//!
//! Range: [0.5, 2]
//! Parameter: attention.temp
//! Trigger: threat_detection
//!
//! ## Constitution Reference: neuromod.Noradrenaline (lines 181-190)
//!
//! Noradrenaline modulates attention temperature:
//! - High NE (2.0): Flat attention (high alertness, broad vigilance)
//! - Low NE (0.5): Sharp attention (focused, calm)
//!
//! ## Threat Detection
//!
//! NE spikes on threat detection events, broadening attention to scan
//! for related threats. It then decays back to baseline.
//!
//! ## Homeostatic Regulation
//!
//! Noradrenaline decays quickly toward baseline (1.0), modeling the
//! transient nature of threat response.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Noradrenaline baseline (center of range)
pub const NE_BASELINE: f32 = 1.0;

/// Noradrenaline minimum value
pub const NE_MIN: f32 = 0.5;

/// Noradrenaline maximum value
pub const NE_MAX: f32 = 2.0;

/// Noradrenaline decay rate per second (fast decay for transient response)
pub const NE_DECAY_RATE: f32 = 0.1;

/// Noradrenaline spike on threat detection
pub const NE_THREAT_SPIKE: f32 = 0.5;

/// Noradrenaline level state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoradrenalineLevel {
    /// Current noradrenaline value in range [NE_MIN, NE_MAX]
    pub value: f32,
    /// Timestamp of last threat detection
    pub last_threat: Option<DateTime<Utc>>,
    /// Count of recent threats (for sustained alertness)
    pub threat_count: u32,
}

impl Default for NoradrenalineLevel {
    fn default() -> Self {
        Self {
            value: NE_BASELINE,
            last_threat: None,
            threat_count: 0,
        }
    }
}

/// Noradrenaline modulator - controls attention temperature
#[derive(Debug, Clone)]
pub struct NoradrenalineModulator {
    level: NoradrenalineLevel,
    decay_rate: f32,
}

impl NoradrenalineModulator {
    /// Create a new noradrenaline modulator at baseline
    pub fn new() -> Self {
        Self {
            level: NoradrenalineLevel::default(),
            decay_rate: NE_DECAY_RATE,
        }
    }

    /// Create a modulator with custom decay rate
    pub fn with_decay_rate(decay_rate: f32) -> Self {
        Self {
            level: NoradrenalineLevel::default(),
            decay_rate: decay_rate.clamp(0.0, 1.0),
        }
    }

    /// Get current noradrenaline level
    pub fn level(&self) -> &NoradrenalineLevel {
        &self.level
    }

    /// Get current noradrenaline value
    pub fn value(&self) -> f32 {
        self.level.value
    }

    /// Spike on threat detection
    ///
    /// Each threat detection increases NE toward max, triggering
    /// broadened attention for threat scanning.
    pub fn on_threat_detected(&mut self) {
        self.level.value = (self.level.value + NE_THREAT_SPIKE).clamp(NE_MIN, NE_MAX);
        self.level.last_threat = Some(Utc::now());
        self.level.threat_count += 1;
        tracing::warn!(
            "Noradrenaline spiked on threat detection: value={:.3}, count={}",
            self.level.value,
            self.level.threat_count
        );
    }

    /// Graded threat response based on severity
    pub fn on_threat_detected_with_severity(&mut self, severity: f32) {
        let spike = NE_THREAT_SPIKE * severity.clamp(0.0, 2.0);
        self.level.value = (self.level.value + spike).clamp(NE_MIN, NE_MAX);
        self.level.last_threat = Some(Utc::now());
        self.level.threat_count += 1;
        tracing::warn!(
            "Noradrenaline spiked on threat (severity={:.2}): value={:.3}",
            severity,
            self.level.value
        );
    }

    /// Calm down (intentional relaxation, e.g., after confirmed safety)
    pub fn on_safety_confirmed(&mut self) {
        let calm_factor = 0.3;
        self.level.value = (self.level.value - calm_factor).clamp(NE_MIN, NE_MAX);
        self.level.threat_count = 0;
        tracing::debug!(
            "Noradrenaline decreased on safety confirmation: value={:.3}",
            self.level.value
        );
    }

    /// Get attention temperature
    /// This is the primary parameter controlled by noradrenaline
    pub fn get_attention_temp(&self) -> f32 {
        self.level.value
    }

    /// Apply homeostatic decay toward baseline
    pub fn decay(&mut self, delta_t: Duration) {
        let dt_secs = delta_t.as_secs_f32();
        let effective_rate = (self.decay_rate * dt_secs).clamp(0.0, 1.0);

        // Exponential decay toward baseline
        self.level.value += (NE_BASELINE - self.level.value) * effective_rate;
        self.level.value = self.level.value.clamp(NE_MIN, NE_MAX);

        // Decay threat count over time (reset if no recent threats)
        if let Some(last_threat) = self.level.last_threat {
            let elapsed = Utc::now() - last_threat;
            if elapsed.num_seconds() > 60 {
                // 1 minute without threat
                self.level.threat_count = 0;
            }
        }
    }

    /// Reset to baseline
    pub fn reset(&mut self) {
        self.level.value = NE_BASELINE;
        self.level.last_threat = None;
        self.level.threat_count = 0;
    }

    /// Set noradrenaline value directly (for testing or initialization)
    pub fn set_value(&mut self, value: f32) {
        self.level.value = value.clamp(NE_MIN, NE_MAX);
    }

    /// Check if system is in heightened alertness state
    pub fn is_alert(&self) -> bool {
        self.level.value > NE_BASELINE + 0.2
    }

    /// Check if system is in high threat response state
    pub fn is_high_alert(&self) -> bool {
        self.level.value > NE_BASELINE + 0.5
    }
}

impl Default for NoradrenalineModulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noradrenaline_initial_baseline() {
        let modulator = NoradrenalineModulator::new();
        assert!((modulator.value() - NE_BASELINE).abs() < f32::EPSILON);
    }

    #[test]
    fn test_noradrenaline_threat_detection() {
        let mut modulator = NoradrenalineModulator::new();
        let initial = modulator.value();

        modulator.on_threat_detected();

        assert!(modulator.value() > initial);
        assert!((modulator.value() - (initial + NE_THREAT_SPIKE)).abs() < f32::EPSILON);
        assert!(modulator.level().last_threat.is_some());
        assert_eq!(modulator.level().threat_count, 1);
    }

    #[test]
    fn test_noradrenaline_range_clamping_max() {
        let mut modulator = NoradrenalineModulator::new();

        // Multiple threats to hit max
        for _ in 0..20 {
            modulator.on_threat_detected();
        }

        assert!(modulator.value() <= NE_MAX);
        assert!((modulator.value() - NE_MAX).abs() < f32::EPSILON);
    }

    #[test]
    fn test_noradrenaline_range_clamping_min() {
        let mut modulator = NoradrenalineModulator::new();
        modulator.set_value(NE_MIN);

        // Safety confirmation shouldn't go below min
        modulator.on_safety_confirmed();

        assert!(modulator.value() >= NE_MIN);
    }

    #[test]
    fn test_noradrenaline_decay_toward_baseline() {
        let mut modulator = NoradrenalineModulator::with_decay_rate(0.2); // Use faster decay for test
        modulator.set_value(NE_MAX);

        // Decay over time
        for _ in 0..100 {
            modulator.decay(Duration::from_millis(100));
        }

        let diff = (modulator.value() - NE_BASELINE).abs();
        assert!(
            diff < 0.2,
            "Expected value near baseline, got: {}",
            modulator.value()
        );
    }

    #[test]
    fn test_noradrenaline_decay_from_min() {
        let mut modulator = NoradrenalineModulator::new();
        modulator.set_value(NE_MIN);

        // Decay should increase toward baseline
        for _ in 0..100 {
            modulator.decay(Duration::from_millis(100));
        }

        assert!(
            modulator.value() > NE_MIN,
            "Expected value above min, got: {}",
            modulator.value()
        );
    }

    #[test]
    fn test_noradrenaline_attention_temp() {
        let mut modulator = NoradrenalineModulator::new();
        modulator.set_value(1.8);

        let temp = modulator.get_attention_temp();
        assert!((temp - 1.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_noradrenaline_threat_severity() {
        let mut modulator = NoradrenalineModulator::new();
        let initial = modulator.value();

        // High severity threat
        modulator.on_threat_detected_with_severity(2.0);

        // Should spike by NE_THREAT_SPIKE * 2.0
        let expected = (initial + NE_THREAT_SPIKE * 2.0).clamp(NE_MIN, NE_MAX);
        assert!((modulator.value() - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn test_noradrenaline_alert_states() {
        let mut modulator = NoradrenalineModulator::new();

        assert!(!modulator.is_alert());
        assert!(!modulator.is_high_alert());

        modulator.on_threat_detected();
        assert!(modulator.is_alert());

        modulator.on_threat_detected();
        assert!(modulator.is_high_alert());
    }

    #[test]
    fn test_noradrenaline_reset() {
        let mut modulator = NoradrenalineModulator::new();
        modulator.on_threat_detected();
        modulator.on_threat_detected();
        modulator.reset();

        assert!((modulator.value() - NE_BASELINE).abs() < f32::EPSILON);
        assert!(modulator.level().last_threat.is_none());
        assert_eq!(modulator.level().threat_count, 0);
    }
}
