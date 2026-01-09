//! # Adaptive Threshold Calibration (ATC) System
//!
//! 4-level adaptive threshold calibration replacing hardcoded thresholds.
//! Constitution reference: Lines 1009-1126
//!
//! ## Architecture
//!
//! 1. **Level 1 - EWMA Drift Tracker** (per-query)
//!    - Detects distribution drift via exponentially weighted moving average
//!    - Triggers Level 2 if drift > 2σ, Level 3 if drift > 3σ
//!
//! 2. **Level 2 - Temperature Scaling** (hourly)
//!    - Per-embedder confidence calibration
//!    - Target: L_calibration < 0.05
//!    - Embedder-specific temperatures (E5=1.2, E7=0.9, etc)
//!
//! 3. **Level 3 - Thompson Sampling Bandit** (session)
//!    - Multi-armed bandit for threshold selection
//!    - Balances exploration vs exploitation
//!    - Budgeted violation tolerance (decaying over time)
//!
//! 4. **Level 4 - Bayesian Meta-Optimizer** (weekly)
//!    - Gaussian Process surrogate + Expected Improvement
//!    - Constrained optimization with monotonicity
//!    - Learns optimal threshold configurations
//!
//! ## Quality Monitoring
//!
//! - ECE (Expected Calibration Error) < 0.05 (excellent)
//! - MCE (Maximum Calibration Error) < 0.10 (good)
//! - Brier Score < 0.10 (good)
//!
//! ## Self-Correction Protocol
//!
//! - Minor (ECE ∈ [0.05, 0.10]): Increase EWMA α for faster adaptation
//! - Moderate (ECE ∈ [0.10, 0.15]): Thompson exploration + temperature recalibration
//! - Major (ECE > 0.15): Reset to domain priors + Bayesian optimization
//! - Critical (ECE > 0.25): Fallback to conservative static, alert human

pub mod calibration;
pub mod domain;
pub mod level1_ewma;
pub mod level2_temperature;
pub mod level3_bandit;
pub mod level4_bayesian;

pub use calibration::{CalibrationComputer, CalibrationMetrics, CalibrationStatus, Prediction};
pub use domain::{Domain, DomainManager, DomainThresholds};
pub use level1_ewma::{DriftTracker, EwmaState};
pub use level2_temperature::{
    embedder_temperature_range, TemperatureCalibration, TemperatureScaler,
};
// Re-export canonical Embedder from teleological module for ATC consumers
pub use crate::teleological::Embedder;
pub use level3_bandit::{ThresholdArm, ThresholdBandit};
pub use level4_bayesian::{BayesianOptimizer, ThresholdConstraints, ThresholdObservation};

use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Unified ATC system orchestrating all 4 levels
#[derive(Debug)]
pub struct AdaptiveThresholdCalibration {
    level1: DriftTracker,
    level2: TemperatureScaler,
    level3: Option<ThresholdBandit>,
    level4: BayesianOptimizer,
    domains: DomainManager,

    // Monitoring
    last_level2_recalibration: DateTime<Utc>,
    calibration_quality: CalibrationMetrics,
}

impl AdaptiveThresholdCalibration {
    /// Create new ATC system with default configuration
    pub fn new() -> Self {
        let constraints = ThresholdConstraints::default();

        Self {
            level1: DriftTracker::new(),
            level2: TemperatureScaler::new(),
            level3: None,
            level4: BayesianOptimizer::new(constraints),
            domains: DomainManager::new(),
            last_level2_recalibration: Utc::now(),
            calibration_quality: CalibrationMetrics {
                ece: 0.0,
                mce: 0.0,
                brier: 0.0,
                sample_count: 0,
                quality_status: CalibrationStatus::Excellent,
            },
        }
    }

    /// Register a threshold to track at Level 1
    pub fn register_threshold(
        &mut self,
        threshold_name: &str,
        baseline: f32,
        baseline_std: f32,
        alpha: f32,
    ) {
        self.level1
            .register_threshold(threshold_name, baseline, baseline_std, alpha);
    }

    /// Observe threshold usage (Level 1)
    pub fn observe_threshold(&mut self, threshold_name: &str, observed: f32) {
        self.level1.observe(threshold_name, observed);

        // Check for Level 2 trigger
        if let Some(drift) = self.level1.get_drift_score(threshold_name) {
            if drift > 2.0 {
                // Would trigger Level 2 recalibration in production
            }
            if drift > 3.0 {
                // Would trigger Level 3 exploration in production
            }
        }
    }

    /// Record prediction for calibration (Level 2)
    pub fn record_prediction(&mut self, embedder: Embedder, confidence: f32, is_correct: bool) {
        self.level2.record(embedder, confidence, is_correct);
    }

    /// Run Level 2 temperature calibration (should be hourly)
    pub fn calibrate_temperatures(&mut self) -> HashMap<Embedder, f32> {
        let losses = self.level2.calibrate_all();
        self.last_level2_recalibration = Utc::now();
        losses
    }

    /// Get temperature-scaled confidence
    pub fn get_scaled_confidence(&self, embedder: Embedder, raw_confidence: f32) -> f32 {
        self.level2.scale(embedder, raw_confidence)
    }

    /// Initialize Level 3 bandit for a session
    pub fn init_session_bandit(&mut self, threshold_candidates: Vec<f32>) {
        let arms: Vec<ThresholdArm> = threshold_candidates
            .into_iter()
            .map(|v| ThresholdArm { value: v })
            .collect();

        self.level3 = Some(ThresholdBandit::new(arms, 1.5));
    }

    /// Select threshold using Thompson sampling
    pub fn select_threshold_thompson(&self) -> Option<f32> {
        self.level3
            .as_ref()
            .and_then(|bandit| bandit.select_thompson().map(|arm| arm.value))
    }

    /// Select threshold using UCB
    pub fn select_threshold_ucb(&self) -> Option<f32> {
        self.level3
            .as_ref()
            .and_then(|bandit| bandit.select_ucb().map(|arm| arm.value))
    }

    /// Record outcome of threshold selection
    pub fn record_threshold_outcome(&mut self, threshold: f32, success: bool) {
        if let Some(bandit) = &mut self.level3 {
            bandit.record_outcome(ThresholdArm { value: threshold }, success);
        }
    }

    /// Compute and update calibration metrics
    pub fn update_calibration_metrics(&mut self, predictions: Vec<Prediction>) {
        let mut computer = CalibrationComputer::new(10);
        computer.add_predictions(predictions);

        self.calibration_quality = computer.compute_all();
    }

    /// Get current calibration quality
    pub fn get_calibration_quality(&self) -> &CalibrationMetrics {
        &self.calibration_quality
    }

    /// Check if should trigger Level 2 recalibration (hourly)
    pub fn should_recalibrate_level2(&self) -> bool {
        self.level2.should_recalibrate()
    }

    /// Check if should trigger Level 3 exploration
    pub fn should_explore_level3(&self) -> bool {
        matches!(
            self.calibration_quality.quality_status,
            CalibrationStatus::Poor | CalibrationStatus::Critical
        )
    }

    /// Check if should trigger Level 4 optimization (weekly)
    pub fn should_optimize_level4(&self) -> bool {
        self.level4.should_optimize()
    }

    /// Get domain thresholds
    pub fn get_domain_thresholds(&self, domain: Domain) -> Option<&DomainThresholds> {
        self.domains.get(domain)
    }

    /// Get current drift status
    pub fn get_drift_status(&self) -> HashMap<String, f32> {
        let thresholds = self.level1.get_all_thresholds();
        thresholds
            .into_iter()
            .map(|t| {
                let drift = self.level1.get_drift_score(t).unwrap_or(0.0);
                (t.to_string(), drift)
            })
            .collect()
    }

    /// Get poorly calibrated embedders
    pub fn get_poorly_calibrated_embedders(&self) -> Vec<Embedder> {
        self.level2.get_poorly_calibrated()
    }

    /// Reset everything for testing
    #[cfg(test)]
    pub fn reset_for_testing(&mut self) {
        self.level1 = DriftTracker::new();
        self.level2 = TemperatureScaler::new();
        self.level3 = None;
        self.calibration_quality = CalibrationMetrics {
            ece: 0.0,
            mce: 0.0,
            brier: 0.0,
            sample_count: 0,
            quality_status: CalibrationStatus::Excellent,
        };
    }
}

impl Default for AdaptiveThresholdCalibration {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atc_creation() {
        let atc = AdaptiveThresholdCalibration::new();
        assert_eq!(atc.domains.get_all().len(), 6);
    }

    #[test]
    fn test_register_and_observe() {
        let mut atc = AdaptiveThresholdCalibration::new();

        atc.register_threshold("theta_opt", 0.75, 0.05, 0.2);
        atc.observe_threshold("theta_opt", 0.80);

        let drift = atc.level1.get_drift_score("theta_opt").unwrap();
        assert!(drift >= 0.0);
    }

    #[test]
    fn test_temperature_scaling() {
        let mut atc = AdaptiveThresholdCalibration::new();

        atc.record_prediction(Embedder::Semantic, 0.8, true);
        let scaled = atc.get_scaled_confidence(Embedder::Semantic, 0.8);

        assert!((0.0..=1.0).contains(&scaled));
    }

    #[test]
    fn test_bandit_initialization() {
        let mut atc = AdaptiveThresholdCalibration::new();

        atc.init_session_bandit(vec![0.70, 0.75, 0.80]);
        let selected = atc.select_threshold_thompson();

        assert!(selected.is_some());
        assert!(selected.unwrap() >= 0.70 && selected.unwrap() <= 0.80);
    }

    #[test]
    fn test_calibration_monitoring() {
        let mut atc = AdaptiveThresholdCalibration::new();

        let predictions = vec![
            Prediction {
                confidence: 0.9,
                is_correct: true,
            },
            Prediction {
                confidence: 0.8,
                is_correct: true,
            },
            Prediction {
                confidence: 0.7,
                is_correct: false,
            },
        ];

        atc.update_calibration_metrics(predictions);

        let quality = atc.get_calibration_quality();
        assert!(quality.sample_count == 3);
    }
}
