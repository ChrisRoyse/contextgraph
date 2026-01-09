//! Calibration Quality Metrics
//!
//! Computes Expected Calibration Error (ECE), Maximum Calibration Error (MCE),
//! and Brier Score to monitor threshold calibration quality.
//!
//! # Targets (from constitution)
//! - ECE < 0.05 (good), < 0.10 (acceptable), > 0.15 (trigger recalibration)
//! - MCE < 0.10 (good), < 0.20 (acceptable)
//! - Brier < 0.10 (good), < 0.15 (acceptable)

use std::collections::HashMap;

/// Single prediction with confidence and outcome
#[derive(Debug, Clone, Copy)]
pub struct Prediction {
    pub confidence: f32,
    pub is_correct: bool,
}

/// Calibration metrics report
#[derive(Debug, Clone)]
pub struct CalibrationMetrics {
    /// Expected Calibration Error
    pub ece: f32,
    /// Maximum Calibration Error
    pub mce: f32,
    /// Brier Score
    pub brier: f32,
    /// Number of predictions
    pub sample_count: usize,
    /// Calibration quality status
    pub quality_status: CalibrationStatus,
}

/// Calibration quality status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationStatus {
    /// ECE < 0.05
    Excellent,
    /// 0.05 <= ECE < 0.10
    Good,
    /// 0.10 <= ECE < 0.15
    Acceptable,
    /// 0.15 <= ECE < 0.25
    Poor,
    /// ECE >= 0.25
    Critical,
}

impl CalibrationStatus {
    pub fn from_ece(ece: f32) -> Self {
        match ece {
            e if e < 0.05 => CalibrationStatus::Excellent,
            e if e < 0.10 => CalibrationStatus::Good,
            e if e < 0.15 => CalibrationStatus::Acceptable,
            e if e < 0.25 => CalibrationStatus::Poor,
            _ => CalibrationStatus::Critical,
        }
    }

    pub fn should_recalibrate(&self) -> bool {
        matches!(self, CalibrationStatus::Poor | CalibrationStatus::Critical)
    }
}

/// Calibration metric computer
#[derive(Debug)]
pub struct CalibrationComputer {
    predictions: Vec<Prediction>,
    num_bins: usize,
}

impl CalibrationComputer {
    /// Create new calibration computer
    pub fn new(num_bins: usize) -> Self {
        Self {
            predictions: Vec::new(),
            num_bins: num_bins.clamp(5, 20),
        }
    }

    /// Add a prediction
    pub fn add_prediction(&mut self, confidence: f32, is_correct: bool) {
        self.predictions.push(Prediction {
            confidence: confidence.clamp(0.0, 1.0),
            is_correct,
        });
    }

    /// Add multiple predictions
    pub fn add_predictions(&mut self, predictions: Vec<Prediction>) {
        self.predictions.extend(predictions);
    }

    /// Compute Brier Score: (1/N) × Σᵢ (confidenceᵢ - correctᵢ)²
    pub fn compute_brier(&self) -> f32 {
        if self.predictions.is_empty() {
            return 0.0;
        }

        let sum: f32 = self
            .predictions
            .iter()
            .map(|p| {
                let actual = if p.is_correct { 1.0 } else { 0.0 };
                (p.confidence - actual).powi(2)
            })
            .sum();

        sum / self.predictions.len() as f32
    }

    /// Compute Expected Calibration Error (ECE)
    /// ECE = Σ (count_bin / total) × |avg_confidence_bin - avg_accuracy_bin|
    pub fn compute_ece(&self) -> f32 {
        if self.predictions.is_empty() {
            return 0.0;
        }

        // Create bins
        let mut bins: Vec<Vec<Prediction>> = vec![Vec::new(); self.num_bins];

        for pred in &self.predictions {
            let bin_idx = (pred.confidence * self.num_bins as f32).floor() as usize;
            let bin_idx = bin_idx.min(self.num_bins - 1);
            bins[bin_idx].push(*pred);
        }

        let total = self.predictions.len() as f32;
        let mut ece = 0.0;

        for bin in bins {
            if bin.is_empty() {
                continue;
            }

            let bin_size = bin.len() as f32;
            let avg_confidence = bin.iter().map(|p| p.confidence).sum::<f32>() / bin_size;
            let avg_accuracy = bin.iter().filter(|p| p.is_correct).count() as f32 / bin_size;

            let contribution = (bin_size / total) * (avg_confidence - avg_accuracy).abs();
            ece += contribution;
        }

        ece
    }

    /// Compute Maximum Calibration Error (MCE)
    /// MCE = max over bins of |avg_confidence_bin - avg_accuracy_bin|
    pub fn compute_mce(&self) -> f32 {
        if self.predictions.is_empty() {
            return 0.0;
        }

        // Create bins
        let mut bins: Vec<Vec<Prediction>> = vec![Vec::new(); self.num_bins];

        for pred in &self.predictions {
            let bin_idx = (pred.confidence * self.num_bins as f32).floor() as usize;
            let bin_idx = bin_idx.min(self.num_bins - 1);
            bins[bin_idx].push(*pred);
        }

        let mut mce: f32 = 0.0;

        for bin in bins {
            if bin.is_empty() {
                continue;
            }

            let bin_size = bin.len() as f32;
            let avg_confidence = bin.iter().map(|p| p.confidence).sum::<f32>() / bin_size;
            let avg_accuracy = bin.iter().filter(|p| p.is_correct).count() as f32 / bin_size;

            let error = (avg_confidence - avg_accuracy).abs();
            mce = mce.max(error);
        }

        mce
    }

    /// Compute all metrics
    pub fn compute_all(&self) -> CalibrationMetrics {
        let ece = self.compute_ece();
        let mce = self.compute_mce();
        let brier = self.compute_brier();

        CalibrationMetrics {
            ece,
            mce,
            brier,
            sample_count: self.predictions.len(),
            quality_status: CalibrationStatus::from_ece(ece),
        }
    }

    /// Clear predictions
    pub fn clear(&mut self) {
        self.predictions.clear();
    }

    /// Get confidence distribution info
    pub fn get_distribution_info(&self) -> CalibrationDistribution {
        if self.predictions.is_empty() {
            return CalibrationDistribution::default();
        }

        let mut bins: HashMap<u32, Vec<Prediction>> = HashMap::new();

        for pred in &self.predictions {
            let bin_idx = (pred.confidence * self.num_bins as f32).floor() as u32;
            let bin_idx = bin_idx.min(self.num_bins as u32 - 1);
            bins.entry(bin_idx).or_default().push(*pred);
        }

        let mut bin_stats = Vec::new();
        for i in 0..self.num_bins {
            if let Some(preds) = bins.get(&(i as u32)) {
                if !preds.is_empty() {
                    let avg_conf =
                        preds.iter().map(|p| p.confidence).sum::<f32>() / preds.len() as f32;
                    let accuracy =
                        preds.iter().filter(|p| p.is_correct).count() as f32 / preds.len() as f32;

                    bin_stats.push(BinStatistics {
                        bin_index: i,
                        sample_count: preds.len(),
                        avg_confidence: avg_conf,
                        avg_accuracy: accuracy,
                        calibration_gap: (avg_conf - accuracy).abs(),
                    });
                }
            }
        }

        CalibrationDistribution {
            bins: bin_stats,
            total_samples: self.predictions.len(),
        }
    }
}

/// Statistics for one confidence bin
#[derive(Debug, Clone)]
pub struct BinStatistics {
    pub bin_index: usize,
    pub sample_count: usize,
    pub avg_confidence: f32,
    pub avg_accuracy: f32,
    pub calibration_gap: f32,
}

/// Distribution of predictions across confidence bins
#[derive(Debug, Clone, Default)]
pub struct CalibrationDistribution {
    pub bins: Vec<BinStatistics>,
    pub total_samples: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brier_score_perfect() {
        let mut computer = CalibrationComputer::new(10);

        // Perfect predictions at extreme confidence
        for _ in 0..10 {
            computer.add_prediction(1.0, true);
        }

        let brier = computer.compute_brier();
        assert!(
            brier < 0.001,
            "Perfect 1.0 confidence with correct = perfect score"
        );
    }

    #[test]
    fn test_brier_score_bad() {
        let mut computer = CalibrationComputer::new(10);

        // Terribly overconfident
        for _ in 0..10 {
            computer.add_prediction(0.9, false);
        }

        let brier = computer.compute_brier();
        assert!(brier > 0.8);
    }

    #[test]
    fn test_ece_well_calibrated() {
        let mut computer = CalibrationComputer::new(10);

        // Well-calibrated: 80% confidence -> 80% accuracy
        // 40 correct, 10 incorrect = 80% accuracy
        for _ in 0..40 {
            computer.add_prediction(0.8, true);
        }
        for _ in 0..10 {
            computer.add_prediction(0.8, false);
        }

        let ece = computer.compute_ece();
        assert!(ece < 0.2, "Well-calibrated predictions should have low ECE");
    }

    #[test]
    fn test_ece_poorly_calibrated() {
        let mut computer = CalibrationComputer::new(10);

        // Overconfident: 90% confidence but only 50% accuracy
        for _ in 0..50 {
            computer.add_prediction(0.9, false);
        }
        for _ in 0..50 {
            computer.add_prediction(0.9, true);
        }

        let ece = computer.compute_ece();
        assert!(ece > 0.2);
    }

    #[test]
    fn test_calibration_status() {
        assert_eq!(
            CalibrationStatus::from_ece(0.02),
            CalibrationStatus::Excellent
        );
        assert_eq!(CalibrationStatus::from_ece(0.08), CalibrationStatus::Good);
        assert_eq!(
            CalibrationStatus::from_ece(0.12),
            CalibrationStatus::Acceptable
        );
        assert_eq!(CalibrationStatus::from_ece(0.20), CalibrationStatus::Poor);
        assert_eq!(
            CalibrationStatus::from_ece(0.30),
            CalibrationStatus::Critical
        );
    }

    #[test]
    fn test_should_recalibrate() {
        assert!(!CalibrationStatus::Excellent.should_recalibrate());
        assert!(!CalibrationStatus::Good.should_recalibrate());
        assert!(!CalibrationStatus::Acceptable.should_recalibrate());
        assert!(CalibrationStatus::Poor.should_recalibrate());
        assert!(CalibrationStatus::Critical.should_recalibrate());
    }

    #[test]
    fn test_compute_all() {
        let mut computer = CalibrationComputer::new(10);

        // Add well-calibrated predictions
        for _ in 0..24 {
            computer.add_prediction(0.8, true);
        }
        for _ in 0..6 {
            computer.add_prediction(0.8, false);
        }

        let metrics = computer.compute_all();
        assert!(
            metrics.ece < 0.25,
            "ECE should be reasonable for well-calibrated data"
        );
        assert!(
            metrics.brier < 0.25,
            "Brier should be low for well-calibrated data"
        );
        assert_eq!(metrics.sample_count, 30);
    }

    #[test]
    fn test_mce() {
        let mut computer = CalibrationComputer::new(5);

        // Bin 0: 10% confidence, 100% accuracy
        for _ in 0..10 {
            computer.add_prediction(0.1, true);
        }
        // Bin 1: 50% confidence, 0% accuracy
        for _ in 0..10 {
            computer.add_prediction(0.5, false);
        }

        let mce = computer.compute_mce();
        assert!(mce > 0.4); // At least one bin has large error
    }
}
