//! Level 1: EWMA Drift Tracker
//!
//! Per-query drift detection using Exponentially Weighted Moving Average.
//! Detects when observed thresholds deviate significantly from baseline,
//! triggering higher-level recalibration.
//!
//! # Formula
//! θ_ewma(t) = α × θ_observed(t) + (1 - α) × θ_ewma(t-1)
//! drift_score = |θ_ewma(t) - θ_baseline| / σ_baseline
//!
//! # Triggers
//! - drift_score > 2.0: Trigger Level 2 recalibration
//! - drift_score > 3.0: Trigger Level 3 exploration

use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Threshold observation from a single query
#[derive(Debug, Clone, Copy)]
pub struct ThresholdObservation {
    /// The observed threshold value
    pub value: f32,
    /// Whether this observation resulted in a successful match
    pub success: bool,
    /// Timestamp of observation
    pub timestamp: DateTime<Utc>,
}

/// EWMA tracker state for a specific threshold type
#[derive(Debug, Clone)]
pub struct EwmaState {
    /// Current EWMA value
    pub ewma_value: f32,
    /// Baseline (prior) threshold value
    pub baseline: f32,
    /// Estimated standard deviation of baseline
    pub baseline_std: f32,
    /// Smoothing factor α ∈ [0.1, 0.3]
    pub alpha: f32,
    /// Number of observations incorporated
    pub observation_count: u32,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
}

impl EwmaState {
    /// Create new EWMA tracker with given baseline
    pub fn new(baseline: f32, baseline_std: f32, alpha: f32) -> Self {
        Self {
            ewma_value: baseline,
            baseline,
            baseline_std,
            alpha: alpha.clamp(0.1, 0.3),
            observation_count: 0,
            last_updated: Utc::now(),
        }
    }

    /// Update EWMA with a new observation
    pub fn update(&mut self, observed: f32) {
        self.ewma_value = self.alpha * observed + (1.0 - self.alpha) * self.ewma_value;
        self.observation_count += 1;
        self.last_updated = Utc::now();
    }

    /// Compute drift score normalized by baseline std
    pub fn compute_drift_score(&self) -> f32 {
        if self.baseline_std == 0.0 {
            0.0
        } else {
            ((self.ewma_value - self.baseline).abs()) / self.baseline_std
        }
    }

    /// Check if drift warrants Level 2 recalibration (drift > 2.0σ)
    pub fn should_trigger_level2(&self) -> bool {
        self.compute_drift_score() > 2.0
    }

    /// Check if drift warrants Level 3 exploration (drift > 3.0σ)
    pub fn should_trigger_level3(&self) -> bool {
        self.compute_drift_score() > 3.0
    }
}

/// Per-threshold EWMA tracker
#[derive(Debug)]
pub struct DriftTracker {
    /// EWMA states keyed by threshold type (e.g., "theta_opt", "theta_dup")
    states: HashMap<String, EwmaState>,
}

impl Default for DriftTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl DriftTracker {
    /// Create new drift tracker
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    /// Register a new threshold to track
    pub fn register_threshold(
        &mut self,
        threshold_name: &str,
        baseline: f32,
        baseline_std: f32,
        alpha: f32,
    ) {
        self.states.insert(
            threshold_name.to_string(),
            EwmaState::new(baseline, baseline_std, alpha),
        );
    }

    /// Record an observation for a threshold
    pub fn observe(&mut self, threshold_name: &str, observed: f32) {
        if let Some(state) = self.states.get_mut(threshold_name) {
            state.update(observed);
        }
    }

    /// Get current EWMA value for a threshold
    pub fn get_ewma(&self, threshold_name: &str) -> Option<f32> {
        self.states.get(threshold_name).map(|s| s.ewma_value)
    }

    /// Get drift score for a threshold
    pub fn get_drift_score(&self, threshold_name: &str) -> Option<f32> {
        self.states
            .get(threshold_name)
            .map(|s| s.compute_drift_score())
    }

    /// Get all thresholds that should trigger Level 2 recalibration
    pub fn get_level2_triggers(&self) -> Vec<String> {
        self.states
            .iter()
            .filter(|(_, state)| state.should_trigger_level2())
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Get all thresholds that should trigger Level 3 exploration
    pub fn get_level3_triggers(&self) -> Vec<String> {
        self.states
            .iter()
            .filter(|(_, state)| state.should_trigger_level3())
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Get complete state for a threshold (for debugging)
    pub fn get_state(&self, threshold_name: &str) -> Option<&EwmaState> {
        self.states.get(threshold_name)
    }

    /// Get all tracked thresholds
    pub fn get_all_thresholds(&self) -> Vec<&str> {
        self.states.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ewma_update() {
        let mut state = EwmaState::new(0.75, 0.05, 0.2);
        assert_eq!(state.ewma_value, 0.75);

        // First observation: 0.80 (above baseline)
        state.update(0.80);
        // EWMA = 0.2 * 0.80 + 0.8 * 0.75 = 0.16 + 0.6 = 0.76
        assert!((state.ewma_value - 0.76).abs() < 0.001);

        // Second observation: 0.85
        state.update(0.85);
        // EWMA = 0.2 * 0.85 + 0.8 * 0.76 = 0.17 + 0.608 = 0.778
        assert!((state.ewma_value - 0.778).abs() < 0.001);
    }

    #[test]
    fn test_drift_score_calculation() {
        let state = EwmaState::new(0.75, 0.05, 0.2);
        let mut state = state;

        // No drift initially
        assert!((state.compute_drift_score() - 0.0).abs() < 0.001);

        // Drift by 0.10 (2 std deviations)
        state.ewma_value = 0.85;
        assert!((state.compute_drift_score() - 2.0).abs() < 0.001);

        // Drift by 0.15 (3 std deviations)
        state.ewma_value = 0.90;
        assert!((state.compute_drift_score() - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_level_triggers() {
        let mut state = EwmaState::new(0.75, 0.05, 0.2);

        // No trigger at baseline
        assert!(!state.should_trigger_level2());
        assert!(!state.should_trigger_level3());

        // Trigger Level 2 at 2.1σ drift
        // drift = |0.755 - 0.75| / 0.05 = 0.005 / 0.05 = 0.1 (no trigger)
        // Need bigger drift: 2σ = 0.1 drift
        state.ewma_value = 0.85;
        assert!(state.should_trigger_level2(), "Should trigger at 2σ drift");
        assert!(
            !state.should_trigger_level3(),
            "Should not trigger Level 3 at 2σ"
        );

        // Trigger Level 3 at 3.1σ drift
        state.ewma_value = 0.905;
        assert!(
            state.should_trigger_level2(),
            "Should trigger at 3.1σ drift"
        );
        assert!(
            state.should_trigger_level3(),
            "Should trigger Level 3 at 3.1σ"
        );
    }

    #[test]
    fn test_drift_tracker_multi_threshold() {
        let mut tracker = DriftTracker::new();
        tracker.register_threshold("theta_opt", 0.75, 0.05, 0.2);
        tracker.register_threshold("theta_dup", 0.90, 0.03, 0.2);

        // No triggers initially
        assert!(tracker.get_level2_triggers().is_empty());
        assert!(tracker.get_level3_triggers().is_empty());

        // Observe theta_opt drifting
        for _ in 0..10 {
            tracker.observe("theta_opt", 0.82);
        }

        let drift = tracker.get_drift_score("theta_opt").unwrap();
        assert!(drift > 1.0);

        // Observe theta_dup staying stable
        tracker.observe("theta_dup", 0.90);
        let drift2 = tracker.get_drift_score("theta_dup").unwrap();
        assert!(drift2 < 0.1);
    }

    #[test]
    fn test_alpha_clamping() {
        let state1 = EwmaState::new(0.75, 0.05, 0.05);
        assert_eq!(state1.alpha, 0.1); // Clamped up

        let state2 = EwmaState::new(0.75, 0.05, 0.5);
        assert_eq!(state2.alpha, 0.3); // Clamped down

        let state3 = EwmaState::new(0.75, 0.05, 0.2);
        assert_eq!(state3.alpha, 0.2); // Within range
    }
}
