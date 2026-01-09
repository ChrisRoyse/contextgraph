//! Level 2: Temperature Scaling Calibration
//!
//! Hourly calibration of per-embedder temperature parameters.
//! Temperature scaling adjusts confidence values to match actual accuracy:
//! calibrated_confidence = σ(logit(raw_confidence) / T)
//!
//! # Per-Embedder Temperatures (from constitution)
//! - E1_Semantic: T=1.0 (baseline)
//! - E5_Causal: T=1.2 (overconfident)
//! - E7_Code: T=0.9 (needs precision)
//! - E9_HDC: T=1.5 (noisy)
//! - E13_SPLADE: T=1.1 (sparse = variable)
//!
//! # Target
//! L_calibration < 0.05

use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

// Import canonical Embedder from teleological module
use crate::teleological::Embedder;

/// Get valid temperature range for this embedder (ATC-specific).
///
/// This is separate from the Embedder enum because temperature ranges
/// are specific to the ATC calibration system, not general embedder properties.
pub fn embedder_temperature_range(embedder: Embedder) -> (f32, f32) {
    match embedder {
        Embedder::Causal => (0.8, 2.5),
        Embedder::Code => (0.5, 1.5),
        Embedder::Hdc => (1.0, 3.0),
        Embedder::KeywordSplade => (0.7, 2.0),
        _ => (0.5, 2.0),
    }
}

/// Calibration sample: (confidence, actual_correctness)
#[derive(Debug, Clone, Copy)]
pub struct CalibrationSample {
    pub confidence: f32,
    pub is_correct: bool,
}

/// Temperature calibration state for one embedder
#[derive(Debug, Clone)]
pub struct TemperatureCalibration {
    pub embedder: Embedder,
    pub temperature: f32,
    pub samples: Vec<CalibrationSample>,
    pub calibration_loss: f32,
    pub last_updated: DateTime<Utc>,
}

impl TemperatureCalibration {
    /// Create new temperature calibration state
    pub fn new(embedder: Embedder) -> Self {
        Self {
            embedder,
            temperature: embedder.default_temperature(),
            samples: Vec::new(),
            calibration_loss: 0.0,
            last_updated: Utc::now(),
        }
    }

    /// Add a calibration sample
    pub fn add_sample(&mut self, confidence: f32, is_correct: bool) {
        self.samples.push(CalibrationSample {
            confidence: confidence.clamp(0.0, 1.0),
            is_correct,
        });
    }

    /// Compute Brier score (Expected Calibration Error approximation)
    pub fn compute_brier_score(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }

        let sum: f32 = self.samples
            .iter()
            .map(|s| {
                let pred = s.confidence;
                let actual = if s.is_correct { 1.0 } else { 0.0 };
                (pred - actual).powi(2)
            })
            .sum();

        sum / self.samples.len() as f32
    }

    /// Calibrate temperature to minimize Brier score
    /// Uses simple gradient descent on log-space temperature
    pub fn calibrate(&mut self) -> f32 {
        if self.samples.len() < 10 {
            return self.calibration_loss; // Not enough samples
        }

        // Try a few temperature values and pick the best
        let (min_t, max_t) = embedder_temperature_range(self.embedder);
        let mut best_temp = self.temperature;
        let mut best_loss = self.compute_brier_score();

        // Grid search over temperature range
        for steps in 0..20 {
            let t = min_t + (max_t - min_t) * (steps as f32 / 19.0);
            let loss = self.compute_scaled_loss(t);
            if loss < best_loss {
                best_loss = loss;
                best_temp = t;
            }
        }

        self.temperature = best_temp;
        self.calibration_loss = best_loss;
        self.last_updated = Utc::now();

        best_loss
    }

    /// Compute loss with a specific temperature (for calibration search)
    fn compute_scaled_loss(&self, temperature: f32) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }

        let sum: f32 = self.samples
            .iter()
            .map(|s| {
                let scaled = self.scale_confidence(s.confidence, temperature);
                let actual = if s.is_correct { 1.0 } else { 0.0 };
                (scaled - actual).powi(2)
            })
            .sum();

        sum / self.samples.len() as f32
    }

    /// Apply temperature scaling to confidence
    fn scale_confidence(&self, raw_confidence: f32, temperature: f32) -> f32 {
        if raw_confidence <= 0.0 || raw_confidence >= 1.0 {
            return raw_confidence;
        }

        // logit(p) = log(p / (1-p))
        let logit = (raw_confidence / (1.0 - raw_confidence)).ln();
        let scaled_logit = logit / temperature;

        // sigmoid(x) = 1 / (1 + exp(-x))
        1.0 / (1.0 + (-scaled_logit).exp())
    }

    /// Apply temperature scaling to confidence
    pub fn scale(&self, raw_confidence: f32) -> f32 {
        self.scale_confidence(raw_confidence, self.temperature)
    }

    /// Check if calibration is good (loss < 0.05)
    pub fn is_well_calibrated(&self) -> bool {
        self.calibration_loss < 0.05
    }

    /// Clear samples and reset for next calibration window
    pub fn reset_samples(&mut self) {
        self.samples.clear();
    }
}

/// Multi-embedder temperature calibrator
#[derive(Debug)]
pub struct TemperatureScaler {
    calibrations: HashMap<Embedder, TemperatureCalibration>,
    last_calibration: DateTime<Utc>,
}

impl Default for TemperatureScaler {
    fn default() -> Self {
        Self::new()
    }
}

impl TemperatureScaler {
    /// Create new temperature scaler with all embedders
    pub fn new() -> Self {
        let mut calibrations = HashMap::new();
        for embedder in Embedder::all() {
            calibrations.insert(embedder, TemperatureCalibration::new(embedder));
        }

        Self {
            calibrations,
            last_calibration: Utc::now(),
        }
    }

    /// Record a prediction for calibration
    pub fn record(&mut self, embedder: Embedder, confidence: f32, is_correct: bool) {
        if let Some(cal) = self.calibrations.get_mut(&embedder) {
            cal.add_sample(confidence, is_correct);
        }
    }

    /// Get current scaled confidence for an embedder
    pub fn scale(&self, embedder: Embedder, raw_confidence: f32) -> f32 {
        self.calibrations
            .get(&embedder)
            .map(|cal| cal.scale(raw_confidence))
            .unwrap_or(raw_confidence)
    }

    /// Calibrate all embedders (should run hourly)
    pub fn calibrate_all(&mut self) -> HashMap<Embedder, f32> {
        let mut losses = HashMap::new();
        for (embedder, cal) in self.calibrations.iter_mut() {
            let loss = cal.calibrate();
            losses.insert(*embedder, loss);
        }
        self.last_calibration = Utc::now();
        losses
    }

    /// Get calibration loss for an embedder
    pub fn get_calibration_loss(&self, embedder: Embedder) -> Option<f32> {
        self.calibrations
            .get(&embedder)
            .map(|cal| cal.calibration_loss)
    }

    /// Get temperature for an embedder
    pub fn get_temperature(&self, embedder: Embedder) -> Option<f32> {
        self.calibrations
            .get(&embedder)
            .map(|cal| cal.temperature)
    }

    /// Get embedders that need recalibration (loss > 0.05)
    pub fn get_poorly_calibrated(&self) -> Vec<Embedder> {
        self.calibrations
            .iter()
            .filter(|(_, cal)| !cal.is_well_calibrated() && cal.samples.len() >= 10)
            .map(|(embedder, _)| *embedder)
            .collect()
    }

    /// Check if it's time for hourly recalibration
    pub fn should_recalibrate(&self) -> bool {
        Utc::now().signed_duration_since(self.last_calibration) > Duration::hours(1)
    }

    /// Clear all samples for next calibration window
    pub fn reset_window(&mut self) {
        for cal in self.calibrations.values_mut() {
            cal.reset_samples();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temperature_defaults() {
        assert_eq!(Embedder::Semantic.default_temperature(), 1.0);
        assert_eq!(Embedder::Causal.default_temperature(), 1.2);
        assert_eq!(Embedder::Code.default_temperature(), 0.9);
        assert_eq!(Embedder::Hdc.default_temperature(), 1.5);
    }

    #[test]
    fn test_calibration_sample() {
        let mut cal = TemperatureCalibration::new(Embedder::Causal);
        assert_eq!(cal.temperature, 1.2);

        // Add perfectly calibrated samples
        for _ in 0..10 {
            cal.add_sample(0.8, true);
        }

        let brier = cal.compute_brier_score();
        assert!(brier < 0.05);
    }

    #[test]
    fn test_temperature_scaling() {
        let mut cal = TemperatureCalibration::new(Embedder::Semantic);
        cal.temperature = 1.0;

        // At temperature 1.0, confidence should stay the same (approximately)
        let scaled = cal.scale(0.8);
        assert!((scaled - 0.8).abs() < 0.1);
    }

    #[test]
    fn test_multi_embedder_scaler() {
        let mut scaler = TemperatureScaler::new();

        // Record some predictions
        scaler.record(Embedder::Causal, 0.9, true);
        scaler.record(Embedder::Causal, 0.85, true);
        scaler.record(Embedder::Code, 0.7, true);

        // Scale a confidence value
        let scaled = scaler.scale(Embedder::Causal, 0.8);
        assert!((0.0..=1.0).contains(&scaled));
    }

    #[test]
    fn test_should_recalibrate() {
        let scaler = TemperatureScaler::new();
        // Just created, should not need recalibration
        assert!(!scaler.should_recalibrate());
    }

    #[test]
    fn test_embedder_temperature_range() {
        let (min, max) = embedder_temperature_range(Embedder::Causal);
        assert_eq!(min, 0.8);
        assert_eq!(max, 2.5);

        let (min, max) = embedder_temperature_range(Embedder::Code);
        assert_eq!(min, 0.5);
        assert_eq!(max, 1.5);

        let (min, max) = embedder_temperature_range(Embedder::Semantic);
        assert_eq!(min, 0.5);
        assert_eq!(max, 2.0); // Default range
    }
}
