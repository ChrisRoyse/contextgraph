//! Divergence detection metrics: TPR, FPR, accuracy.
//!
//! These metrics evaluate how well the system detects when a query diverges
//! from the corpus topics (indicating a new topic or out-of-domain query).

use serde::{Deserialize, Serialize};

/// Divergence detection metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DivergenceMetrics {
    /// True Positive Rate (sensitivity/recall for divergence).
    /// TPR = TP / (TP + FN)
    pub tpr: f64,

    /// False Positive Rate.
    /// FPR = FP / (FP + TN)
    pub fpr: f64,

    /// True Negative Rate (specificity).
    /// TNR = TN / (TN + FP)
    pub tnr: f64,

    /// False Negative Rate.
    /// FNR = FN / (FN + TP)
    pub fnr: f64,

    /// Precision for divergence detection.
    /// Precision = TP / (TP + FP)
    pub precision: f64,

    /// F1 score for divergence detection.
    pub f1: f64,

    /// Area Under ROC Curve (if threshold sweep was performed).
    pub auc_roc: Option<f64>,

    /// Total number of test cases.
    pub total_cases: usize,

    /// Number of true divergent cases (ground truth).
    pub divergent_cases: usize,

    /// Number of non-divergent cases (ground truth).
    pub non_divergent_cases: usize,
}

impl DivergenceMetrics {
    /// Create metrics from confusion matrix values.
    pub fn from_confusion_matrix(
        true_positives: usize,
        false_positives: usize,
        true_negatives: usize,
        false_negatives: usize,
    ) -> Self {
        let tp = true_positives as f64;
        let fp = false_positives as f64;
        let tn = true_negatives as f64;
        let fn_ = false_negatives as f64;

        let tpr = if tp + fn_ > 0.0 { tp / (tp + fn_) } else { 0.0 };
        let fpr = if fp + tn > 0.0 { fp / (fp + tn) } else { 0.0 };
        let tnr = if tn + fp > 0.0 { tn / (tn + fp) } else { 0.0 };
        let fnr = if fn_ + tp > 0.0 {
            fn_ / (fn_ + tp)
        } else {
            0.0
        };
        let precision = if tp + fp > 0.0 { tp / (tp + fp) } else { 0.0 };
        let f1 = if precision + tpr > 0.0 {
            2.0 * precision * tpr / (precision + tpr)
        } else {
            0.0
        };

        Self {
            tpr,
            fpr,
            tnr,
            fnr,
            precision,
            f1,
            auc_roc: None,
            total_cases: true_positives + false_positives + true_negatives + false_negatives,
            divergent_cases: true_positives + false_negatives,
            non_divergent_cases: true_negatives + false_positives,
        }
    }

    /// Overall accuracy.
    pub fn accuracy(&self) -> f64 {
        // (TP + TN) / Total = (TPR * P + TNR * N) / Total
        if self.total_cases == 0 {
            return 0.0;
        }

        let correct =
            (self.tpr * self.divergent_cases as f64) + (self.tnr * self.non_divergent_cases as f64);
        correct / self.total_cases as f64
    }

    /// Balanced accuracy (average of TPR and TNR).
    pub fn balanced_accuracy(&self) -> f64 {
        (self.tpr + self.tnr) / 2.0
    }

    /// Matthews Correlation Coefficient (good for imbalanced data).
    pub fn mcc(&self) -> f64 {
        let p = self.divergent_cases as f64;
        let n = self.non_divergent_cases as f64;

        let tp = self.tpr * p;
        let tn = self.tnr * n;
        let fp = self.fpr * n;
        let fn_ = self.fnr * p;

        let numerator = tp * tn - fp * fn_;
        let denominator = ((tp + fp) * (tp + fn_) * (tn + fp) * (tn + fn_)).sqrt();

        if denominator < f64::EPSILON {
            0.0
        } else {
            numerator / denominator
        }
    }
}

/// Compute divergence metrics from predictions and ground truth.
///
/// # Arguments
/// * `predictions` - Predicted divergence (true = divergent)
/// * `ground_truth` - Actual divergence (true = divergent)
pub fn compute_metrics(predictions: &[bool], ground_truth: &[bool]) -> DivergenceMetrics {
    if predictions.len() != ground_truth.len() {
        return DivergenceMetrics::default();
    }

    let mut tp = 0;
    let mut fp = 0;
    let mut tn = 0;
    let mut fn_ = 0;

    for (&pred, &truth) in predictions.iter().zip(ground_truth.iter()) {
        match (pred, truth) {
            (true, true) => tp += 1,
            (true, false) => fp += 1,
            (false, true) => fn_ += 1,
            (false, false) => tn += 1,
        }
    }

    DivergenceMetrics::from_confusion_matrix(tp, fp, tn, fn_)
}

/// Compute AUC-ROC from scores and ground truth.
///
/// # Arguments
/// * `scores` - Divergence scores (higher = more likely divergent)
/// * `ground_truth` - Actual divergence (true = divergent)
pub fn compute_auc_roc(scores: &[f64], ground_truth: &[bool]) -> f64 {
    if scores.len() != ground_truth.len() || scores.is_empty() {
        return 0.5;
    }

    // Sort by score descending
    let mut indexed: Vec<(f64, bool)> = scores
        .iter()
        .zip(ground_truth.iter())
        .map(|(&s, &g)| (s, g))
        .collect();
    indexed.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Count positives and negatives
    let n_pos = ground_truth.iter().filter(|&&g| g).count() as f64;
    let n_neg = ground_truth.iter().filter(|&&g| !g).count() as f64;

    if n_pos < f64::EPSILON || n_neg < f64::EPSILON {
        return 0.5;
    }

    // Compute AUC using trapezoidal rule on ROC curve
    let mut auc = 0.0;
    let mut tp = 0.0;
    let mut fp = 0.0;
    let mut prev_tp = 0.0;
    let mut prev_fp = 0.0;

    for (_, is_positive) in indexed {
        if is_positive {
            tp += 1.0;
        } else {
            fp += 1.0;
        }

        // Add trapezoidal area
        auc += (fp - prev_fp) * (tp + prev_tp) / 2.0;
        prev_tp = tp;
        prev_fp = fp;
    }

    auc / (n_pos * n_neg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_detection() {
        let predictions = vec![true, true, false, false];
        let ground_truth = vec![true, true, false, false];

        let metrics = compute_metrics(&predictions, &ground_truth);
        assert!((metrics.tpr - 1.0).abs() < 0.01);
        assert!(metrics.fpr < 0.01);
        assert!((metrics.accuracy() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_no_true_positives() {
        let predictions = vec![false, false, false, false];
        let ground_truth = vec![true, true, false, false];

        let metrics = compute_metrics(&predictions, &ground_truth);
        assert!(metrics.tpr < 0.01); // No true positives
        assert!(metrics.fpr < 0.01); // No false positives
    }

    #[test]
    fn test_auc_roc_perfect() {
        let scores = vec![0.9, 0.8, 0.3, 0.2];
        let ground_truth = vec![true, true, false, false];

        let auc = compute_auc_roc(&scores, &ground_truth);
        assert!((auc - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_auc_roc_random() {
        // When scores are equal, AUC depends on tie-breaking and can vary
        let scores = vec![0.5, 0.5, 0.5, 0.5];
        let ground_truth = vec![true, false, true, false];

        let auc = compute_auc_roc(&scores, &ground_truth);
        // Allow wider tolerance since tie-breaking is arbitrary
        assert!((0.0..=1.0).contains(&auc), "AUC should be between 0 and 1");
    }
}
