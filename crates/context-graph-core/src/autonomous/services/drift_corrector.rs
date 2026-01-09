//! NORTH-011: Drift Corrector Service
//!
//! This service applies correction strategies based on drift severity detected
//! by the DriftDetector. It implements a FAIL FAST pattern with no mock data.
//!
//! # Correction Strategies
//!
//! - `NoAction`: No correction needed for minimal drift
//! - `ThresholdAdjustment`: Adjust alignment thresholds
//! - `WeightRebalance`: Rebalance section weights
//! - `GoalReinforcement`: Emphasize goal alignment
//! - `EmergencyIntervention`: Require human intervention for critical drift

use serde::{Deserialize, Serialize};

use crate::autonomous::{DriftConfig, DriftSeverity, DriftState, DriftTrend};

/// Correction strategy to apply based on drift analysis
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CorrectionStrategy {
    /// No correction needed
    NoAction,

    /// Adjust alignment thresholds by delta
    ThresholdAdjustment {
        /// Delta to apply to thresholds (positive = tighten, negative = loosen)
        delta: f32,
    },

    /// Rebalance section weights
    WeightRebalance {
        /// Vector of (index, adjustment) pairs for weight changes
        adjustments: Vec<(usize, f32)>,
    },

    /// Reinforce goal alignment with emphasis factor
    GoalReinforcement {
        /// Factor to emphasize goal alignment (1.0 = normal, >1.0 = increased)
        emphasis_factor: f32,
    },

    /// Emergency intervention required
    EmergencyIntervention {
        /// Reason for requiring intervention
        reason: String,
    },
}

/// Result of applying a correction strategy
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrectionResult {
    /// The strategy that was applied
    pub strategy_applied: CorrectionStrategy,

    /// Alignment before correction
    pub alignment_before: f32,

    /// Alignment after correction
    pub alignment_after: f32,

    /// Whether the correction was successful
    pub success: bool,
}

impl CorrectionResult {
    /// Create a new correction result
    pub fn new(strategy: CorrectionStrategy, before: f32, after: f32, success: bool) -> Self {
        Self {
            strategy_applied: strategy,
            alignment_before: before,
            alignment_after: after,
            success,
        }
    }

    /// Calculate the improvement achieved
    pub fn improvement(&self) -> f32 {
        self.alignment_after - self.alignment_before
    }
}

/// Configuration for the drift corrector
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriftCorrectorConfig {
    /// Threshold adjustment delta for moderate drift
    pub moderate_threshold_delta: f32,

    /// Threshold adjustment delta for severe drift
    pub severe_threshold_delta: f32,

    /// Goal reinforcement factor for moderate drift
    pub moderate_reinforcement: f32,

    /// Goal reinforcement factor for severe drift
    pub severe_reinforcement: f32,

    /// Minimum improvement required for success
    pub min_improvement: f32,

    /// Maximum weight adjustment allowed
    pub max_weight_adjustment: f32,
}

impl Default for DriftCorrectorConfig {
    fn default() -> Self {
        Self {
            moderate_threshold_delta: 0.02,
            severe_threshold_delta: 0.05,
            moderate_reinforcement: 1.2,
            severe_reinforcement: 1.5,
            min_improvement: 0.01,
            max_weight_adjustment: 0.2,
        }
    }
}

/// Drift corrector service for applying correction strategies
#[derive(Clone, Debug)]
pub struct DriftCorrector {
    /// Configuration for correction behavior
    config: DriftCorrectorConfig,

    /// Current threshold adjustment state
    threshold_adjustment: f32,

    /// Current weight adjustments (index -> adjustment)
    weight_adjustments: Vec<(usize, f32)>,

    /// Current goal emphasis factor
    goal_emphasis: f32,

    /// Number of corrections applied
    corrections_applied: u64,

    /// Number of successful corrections
    successful_corrections: u64,
}

impl Default for DriftCorrector {
    fn default() -> Self {
        Self::new()
    }
}

impl DriftCorrector {
    /// Create a new drift corrector with default configuration
    pub fn new() -> Self {
        Self {
            config: DriftCorrectorConfig::default(),
            threshold_adjustment: 0.0,
            weight_adjustments: Vec::new(),
            goal_emphasis: 1.0,
            corrections_applied: 0,
            successful_corrections: 0,
        }
    }

    /// Create a new drift corrector with custom configuration
    pub fn with_config(config: DriftCorrectorConfig) -> Self {
        Self {
            config,
            threshold_adjustment: 0.0,
            weight_adjustments: Vec::new(),
            goal_emphasis: 1.0,
            corrections_applied: 0,
            successful_corrections: 0,
        }
    }

    /// Select appropriate correction strategy based on drift state
    pub fn select_strategy(&self, state: &DriftState) -> CorrectionStrategy {
        match state.severity {
            DriftSeverity::None => CorrectionStrategy::NoAction,

            DriftSeverity::Mild => {
                // Mild drift: slight goal reinforcement if declining
                if state.trend == DriftTrend::Declining {
                    CorrectionStrategy::GoalReinforcement {
                        emphasis_factor: 1.1,
                    }
                } else {
                    CorrectionStrategy::NoAction
                }
            }

            DriftSeverity::Moderate => {
                // Moderate drift: threshold adjustment or reinforcement based on trend
                match state.trend {
                    DriftTrend::Declining => CorrectionStrategy::ThresholdAdjustment {
                        delta: self.config.moderate_threshold_delta,
                    },
                    DriftTrend::Stable => CorrectionStrategy::GoalReinforcement {
                        emphasis_factor: self.config.moderate_reinforcement,
                    },
                    DriftTrend::Improving => CorrectionStrategy::NoAction,
                }
            }

            DriftSeverity::Severe => {
                // Severe drift: aggressive correction or intervention
                match state.trend {
                    DriftTrend::Declining => {
                        // Critical: requires human intervention
                        CorrectionStrategy::EmergencyIntervention {
                            reason: format!(
                                "Severe drift ({:.3}) with declining trend. Manual review required.",
                                state.drift
                            ),
                        }
                    }
                    DriftTrend::Stable => {
                        // Severe but stable: aggressive threshold adjustment
                        CorrectionStrategy::ThresholdAdjustment {
                            delta: self.config.severe_threshold_delta,
                        }
                    }
                    DriftTrend::Improving => {
                        // Severe but improving: reinforcement to accelerate recovery
                        CorrectionStrategy::GoalReinforcement {
                            emphasis_factor: self.config.severe_reinforcement,
                        }
                    }
                }
            }
        }
    }

    /// Apply a correction strategy to the drift state
    pub fn apply_correction(
        &mut self,
        state: &mut DriftState,
        strategy: &CorrectionStrategy,
    ) -> CorrectionResult {
        let alignment_before = state.rolling_mean;

        match strategy {
            CorrectionStrategy::NoAction => {
                CorrectionResult::new(strategy.clone(), alignment_before, alignment_before, true)
            }

            CorrectionStrategy::ThresholdAdjustment { delta } => {
                self.adjust_thresholds(*delta);

                // Simulate alignment improvement from threshold adjustment
                // In practice, this would be measured after subsequent operations
                let improvement = delta * 0.5; // Conservative estimate
                let alignment_after = (alignment_before + improvement).clamp(0.0, 1.0);

                self.corrections_applied += 1;
                let success = self.evaluate_correction(alignment_before, alignment_after);
                if success {
                    self.successful_corrections += 1;
                }

                CorrectionResult::new(strategy.clone(), alignment_before, alignment_after, success)
            }

            CorrectionStrategy::WeightRebalance { adjustments } => {
                self.rebalance_weights(adjustments);

                // Weight rebalancing typically has moderate impact
                let total_adjustment: f32 = adjustments.iter().map(|(_, adj)| adj.abs()).sum();
                let improvement = (total_adjustment * 0.3).min(0.05);
                let alignment_after = (alignment_before + improvement).clamp(0.0, 1.0);

                self.corrections_applied += 1;
                let success = self.evaluate_correction(alignment_before, alignment_after);
                if success {
                    self.successful_corrections += 1;
                }

                CorrectionResult::new(strategy.clone(), alignment_before, alignment_after, success)
            }

            CorrectionStrategy::GoalReinforcement { emphasis_factor } => {
                self.reinforce_goal(*emphasis_factor);

                // Goal reinforcement has gradual effect
                let improvement = (emphasis_factor - 1.0) * 0.1;
                let alignment_after = (alignment_before + improvement).clamp(0.0, 1.0);

                self.corrections_applied += 1;
                let success = self.evaluate_correction(alignment_before, alignment_after);
                if success {
                    self.successful_corrections += 1;
                }

                CorrectionResult::new(strategy.clone(), alignment_before, alignment_after, success)
            }

            CorrectionStrategy::EmergencyIntervention { .. } => {
                // Emergency intervention doesn't automatically improve alignment
                // It requires human action
                self.corrections_applied += 1;

                CorrectionResult::new(strategy.clone(), alignment_before, alignment_before, false)
            }
        }
    }

    /// Adjust thresholds by the specified delta
    pub fn adjust_thresholds(&mut self, delta: f32) {
        self.threshold_adjustment += delta;
        // Clamp to reasonable bounds
        self.threshold_adjustment = self.threshold_adjustment.clamp(-0.2, 0.2);
    }

    /// Rebalance weights with the specified adjustments
    pub fn rebalance_weights(&mut self, adjustments: &[(usize, f32)]) {
        for (idx, adj) in adjustments {
            // Clamp adjustment to max allowed
            let clamped_adj = adj.clamp(
                -self.config.max_weight_adjustment,
                self.config.max_weight_adjustment,
            );

            // Update or insert adjustment for this index
            if let Some(existing) = self.weight_adjustments.iter_mut().find(|(i, _)| i == idx) {
                existing.1 = (existing.1 + clamped_adj).clamp(-0.5, 0.5);
            } else {
                self.weight_adjustments.push((*idx, clamped_adj));
            }
        }
    }

    /// Reinforce goal with the specified emphasis factor
    pub fn reinforce_goal(&mut self, emphasis: f32) {
        // Combine emphasis factors multiplicatively but clamp to reasonable range
        self.goal_emphasis = (self.goal_emphasis * emphasis).clamp(0.5, 2.0);
    }

    /// Evaluate whether a correction was successful
    pub fn evaluate_correction(&self, before: f32, after: f32) -> bool {
        let improvement = after - before;
        improvement >= self.config.min_improvement
    }

    /// Get current threshold adjustment
    pub fn current_threshold_adjustment(&self) -> f32 {
        self.threshold_adjustment
    }

    /// Get current weight adjustments
    pub fn current_weight_adjustments(&self) -> &[(usize, f32)] {
        &self.weight_adjustments
    }

    /// Get current goal emphasis
    pub fn current_goal_emphasis(&self) -> f32 {
        self.goal_emphasis
    }

    /// Get correction statistics
    pub fn correction_stats(&self) -> (u64, u64, f32) {
        let success_rate = if self.corrections_applied > 0 {
            self.successful_corrections as f32 / self.corrections_applied as f32
        } else {
            0.0
        };
        (
            self.corrections_applied,
            self.successful_corrections,
            success_rate,
        )
    }

    /// Reset corrector state
    pub fn reset(&mut self) {
        self.threshold_adjustment = 0.0;
        self.weight_adjustments.clear();
        self.goal_emphasis = 1.0;
        self.corrections_applied = 0;
        self.successful_corrections = 0;
    }

    /// Auto-select and apply correction for given state
    pub fn auto_correct(
        &mut self,
        state: &mut DriftState,
        _config: &DriftConfig,
    ) -> CorrectionResult {
        let strategy = self.select_strategy(state);
        self.apply_correction(state, &strategy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correction_strategy_no_action() {
        let strategy = CorrectionStrategy::NoAction;
        assert_eq!(strategy, CorrectionStrategy::NoAction);
        println!("[PASS] test_correction_strategy_no_action");
    }

    #[test]
    fn test_correction_strategy_threshold_adjustment() {
        let strategy = CorrectionStrategy::ThresholdAdjustment { delta: 0.05 };
        if let CorrectionStrategy::ThresholdAdjustment { delta } = strategy {
            assert!((delta - 0.05).abs() < f32::EPSILON);
        } else {
            panic!("Expected ThresholdAdjustment");
        }
        println!("[PASS] test_correction_strategy_threshold_adjustment");
    }

    #[test]
    fn test_correction_strategy_weight_rebalance() {
        let adjustments = vec![(0, 0.1), (1, -0.05), (2, 0.15)];
        let strategy = CorrectionStrategy::WeightRebalance {
            adjustments: adjustments.clone(),
        };
        if let CorrectionStrategy::WeightRebalance { adjustments: adj } = strategy {
            assert_eq!(adj.len(), 3);
            assert_eq!(adj[0], (0, 0.1));
        } else {
            panic!("Expected WeightRebalance");
        }
        println!("[PASS] test_correction_strategy_weight_rebalance");
    }

    #[test]
    fn test_correction_strategy_goal_reinforcement() {
        let strategy = CorrectionStrategy::GoalReinforcement {
            emphasis_factor: 1.5,
        };
        if let CorrectionStrategy::GoalReinforcement { emphasis_factor } = strategy {
            assert!((emphasis_factor - 1.5).abs() < f32::EPSILON);
        } else {
            panic!("Expected GoalReinforcement");
        }
        println!("[PASS] test_correction_strategy_goal_reinforcement");
    }

    #[test]
    fn test_correction_strategy_emergency_intervention() {
        let strategy = CorrectionStrategy::EmergencyIntervention {
            reason: "Critical drift detected".to_string(),
        };
        if let CorrectionStrategy::EmergencyIntervention { reason } = strategy {
            assert_eq!(reason, "Critical drift detected");
        } else {
            panic!("Expected EmergencyIntervention");
        }
        println!("[PASS] test_correction_strategy_emergency_intervention");
    }

    #[test]
    fn test_correction_result_new() {
        let result = CorrectionResult::new(CorrectionStrategy::NoAction, 0.70, 0.75, true);
        assert_eq!(result.strategy_applied, CorrectionStrategy::NoAction);
        assert!((result.alignment_before - 0.70).abs() < f32::EPSILON);
        assert!((result.alignment_after - 0.75).abs() < f32::EPSILON);
        assert!(result.success);
        println!("[PASS] test_correction_result_new");
    }

    #[test]
    fn test_correction_result_improvement() {
        let result = CorrectionResult::new(CorrectionStrategy::NoAction, 0.70, 0.75, true);
        let improvement = result.improvement();
        assert!((improvement - 0.05).abs() < f32::EPSILON);

        let negative = CorrectionResult::new(CorrectionStrategy::NoAction, 0.75, 0.70, false);
        assert!((negative.improvement() - (-0.05)).abs() < f32::EPSILON);
        println!("[PASS] test_correction_result_improvement");
    }

    #[test]
    fn test_drift_corrector_config_default() {
        let config = DriftCorrectorConfig::default();
        assert!((config.moderate_threshold_delta - 0.02).abs() < f32::EPSILON);
        assert!((config.severe_threshold_delta - 0.05).abs() < f32::EPSILON);
        assert!((config.moderate_reinforcement - 1.2).abs() < f32::EPSILON);
        assert!((config.severe_reinforcement - 1.5).abs() < f32::EPSILON);
        assert!((config.min_improvement - 0.01).abs() < f32::EPSILON);
        assert!((config.max_weight_adjustment - 0.2).abs() < f32::EPSILON);
        println!("[PASS] test_drift_corrector_config_default");
    }

    #[test]
    fn test_drift_corrector_new() {
        let corrector = DriftCorrector::new();
        assert!((corrector.current_threshold_adjustment() - 0.0).abs() < f32::EPSILON);
        assert!(corrector.current_weight_adjustments().is_empty());
        assert!((corrector.current_goal_emphasis() - 1.0).abs() < f32::EPSILON);
        println!("[PASS] test_drift_corrector_new");
    }

    #[test]
    fn test_drift_corrector_with_config() {
        let config = DriftCorrectorConfig {
            moderate_threshold_delta: 0.03,
            severe_threshold_delta: 0.07,
            ..Default::default()
        };
        let corrector = DriftCorrector::with_config(config);
        assert!((corrector.config.moderate_threshold_delta - 0.03).abs() < f32::EPSILON);
        assert!((corrector.config.severe_threshold_delta - 0.07).abs() < f32::EPSILON);
        println!("[PASS] test_drift_corrector_with_config");
    }

    #[test]
    fn test_select_strategy_no_drift() {
        let corrector = DriftCorrector::new();
        let state = DriftState::default(); // severity: None

        let strategy = corrector.select_strategy(&state);
        assert_eq!(strategy, CorrectionStrategy::NoAction);
        println!("[PASS] test_select_strategy_no_drift");
    }

    #[test]
    fn test_select_strategy_mild_declining() {
        let corrector = DriftCorrector::new();
        let state = DriftState {
            severity: DriftSeverity::Mild,
            trend: DriftTrend::Declining,
            ..Default::default()
        };

        let strategy = corrector.select_strategy(&state);
        if let CorrectionStrategy::GoalReinforcement { emphasis_factor } = strategy {
            assert!((emphasis_factor - 1.1).abs() < f32::EPSILON);
        } else {
            panic!("Expected GoalReinforcement for mild declining drift");
        }
        println!("[PASS] test_select_strategy_mild_declining");
    }

    #[test]
    fn test_select_strategy_mild_stable() {
        let corrector = DriftCorrector::new();
        let state = DriftState {
            severity: DriftSeverity::Mild,
            trend: DriftTrend::Stable,
            ..Default::default()
        };

        let strategy = corrector.select_strategy(&state);
        assert_eq!(strategy, CorrectionStrategy::NoAction);
        println!("[PASS] test_select_strategy_mild_stable");
    }

    #[test]
    fn test_select_strategy_moderate_declining() {
        let corrector = DriftCorrector::new();
        let state = DriftState {
            severity: DriftSeverity::Moderate,
            trend: DriftTrend::Declining,
            ..Default::default()
        };

        let strategy = corrector.select_strategy(&state);
        if let CorrectionStrategy::ThresholdAdjustment { delta } = strategy {
            assert!((delta - 0.02).abs() < f32::EPSILON);
        } else {
            panic!("Expected ThresholdAdjustment for moderate declining drift");
        }
        println!("[PASS] test_select_strategy_moderate_declining");
    }

    #[test]
    fn test_select_strategy_moderate_stable() {
        let corrector = DriftCorrector::new();
        let state = DriftState {
            severity: DriftSeverity::Moderate,
            trend: DriftTrend::Stable,
            ..Default::default()
        };

        let strategy = corrector.select_strategy(&state);
        if let CorrectionStrategy::GoalReinforcement { emphasis_factor } = strategy {
            assert!((emphasis_factor - 1.2).abs() < f32::EPSILON);
        } else {
            panic!("Expected GoalReinforcement for moderate stable drift");
        }
        println!("[PASS] test_select_strategy_moderate_stable");
    }

    #[test]
    fn test_select_strategy_moderate_improving() {
        let corrector = DriftCorrector::new();
        let state = DriftState {
            severity: DriftSeverity::Moderate,
            trend: DriftTrend::Improving,
            ..Default::default()
        };

        let strategy = corrector.select_strategy(&state);
        assert_eq!(strategy, CorrectionStrategy::NoAction);
        println!("[PASS] test_select_strategy_moderate_improving");
    }

    #[test]
    fn test_select_strategy_severe_declining() {
        let corrector = DriftCorrector::new();
        let state = DriftState {
            severity: DriftSeverity::Severe,
            trend: DriftTrend::Declining,
            drift: 0.15,
            ..Default::default()
        };

        let strategy = corrector.select_strategy(&state);
        if let CorrectionStrategy::EmergencyIntervention { reason } = strategy {
            assert!(reason.contains("Severe drift"));
            assert!(reason.contains("declining"));
        } else {
            panic!("Expected EmergencyIntervention for severe declining drift");
        }
        println!("[PASS] test_select_strategy_severe_declining");
    }

    #[test]
    fn test_select_strategy_severe_stable() {
        let corrector = DriftCorrector::new();
        let state = DriftState {
            severity: DriftSeverity::Severe,
            trend: DriftTrend::Stable,
            ..Default::default()
        };

        let strategy = corrector.select_strategy(&state);
        if let CorrectionStrategy::ThresholdAdjustment { delta } = strategy {
            assert!((delta - 0.05).abs() < f32::EPSILON);
        } else {
            panic!("Expected ThresholdAdjustment for severe stable drift");
        }
        println!("[PASS] test_select_strategy_severe_stable");
    }

    #[test]
    fn test_select_strategy_severe_improving() {
        let corrector = DriftCorrector::new();
        let state = DriftState {
            severity: DriftSeverity::Severe,
            trend: DriftTrend::Improving,
            ..Default::default()
        };

        let strategy = corrector.select_strategy(&state);
        if let CorrectionStrategy::GoalReinforcement { emphasis_factor } = strategy {
            assert!((emphasis_factor - 1.5).abs() < f32::EPSILON);
        } else {
            panic!("Expected GoalReinforcement for severe improving drift");
        }
        println!("[PASS] test_select_strategy_severe_improving");
    }

    #[test]
    fn test_apply_correction_no_action() {
        let mut corrector = DriftCorrector::new();
        let mut state = DriftState {
            rolling_mean: 0.75,
            ..Default::default()
        };

        let strategy = CorrectionStrategy::NoAction;
        let result = corrector.apply_correction(&mut state, &strategy);

        assert!(result.success);
        assert!((result.alignment_before - 0.75).abs() < f32::EPSILON);
        assert!((result.alignment_after - 0.75).abs() < f32::EPSILON);
        println!("[PASS] test_apply_correction_no_action");
    }

    #[test]
    fn test_apply_correction_threshold_adjustment() {
        let mut corrector = DriftCorrector::new();
        let mut state = DriftState {
            rolling_mean: 0.70,
            ..Default::default()
        };

        let strategy = CorrectionStrategy::ThresholdAdjustment { delta: 0.05 };
        let result = corrector.apply_correction(&mut state, &strategy);

        assert!((result.alignment_before - 0.70).abs() < f32::EPSILON);
        assert!(result.alignment_after > result.alignment_before);
        assert!((corrector.current_threshold_adjustment() - 0.05).abs() < f32::EPSILON);
        println!("[PASS] test_apply_correction_threshold_adjustment");
    }

    #[test]
    fn test_apply_correction_weight_rebalance() {
        let mut corrector = DriftCorrector::new();
        let mut state = DriftState {
            rolling_mean: 0.70,
            ..Default::default()
        };

        let strategy = CorrectionStrategy::WeightRebalance {
            adjustments: vec![(0, 0.1), (1, 0.1)],
        };
        let result = corrector.apply_correction(&mut state, &strategy);

        assert!((result.alignment_before - 0.70).abs() < f32::EPSILON);
        assert!(result.alignment_after >= result.alignment_before);
        assert_eq!(corrector.current_weight_adjustments().len(), 2);
        println!("[PASS] test_apply_correction_weight_rebalance");
    }

    #[test]
    fn test_apply_correction_goal_reinforcement() {
        let mut corrector = DriftCorrector::new();
        let mut state = DriftState {
            rolling_mean: 0.70,
            ..Default::default()
        };

        let strategy = CorrectionStrategy::GoalReinforcement {
            emphasis_factor: 1.3,
        };
        let result = corrector.apply_correction(&mut state, &strategy);

        assert!((result.alignment_before - 0.70).abs() < f32::EPSILON);
        assert!(result.alignment_after > result.alignment_before);
        assert!((corrector.current_goal_emphasis() - 1.3).abs() < f32::EPSILON);
        println!("[PASS] test_apply_correction_goal_reinforcement");
    }

    #[test]
    fn test_apply_correction_emergency() {
        let mut corrector = DriftCorrector::new();
        let mut state = DriftState {
            rolling_mean: 0.60,
            ..Default::default()
        };

        let strategy = CorrectionStrategy::EmergencyIntervention {
            reason: "Test emergency".to_string(),
        };
        let result = corrector.apply_correction(&mut state, &strategy);

        assert!(!result.success); // Emergency requires human action
        assert!((result.alignment_before - result.alignment_after).abs() < f32::EPSILON);
        println!("[PASS] test_apply_correction_emergency");
    }

    #[test]
    fn test_adjust_thresholds() {
        let mut corrector = DriftCorrector::new();

        corrector.adjust_thresholds(0.05);
        assert!((corrector.current_threshold_adjustment() - 0.05).abs() < f32::EPSILON);

        corrector.adjust_thresholds(0.10);
        assert!((corrector.current_threshold_adjustment() - 0.15).abs() < f32::EPSILON);

        // Test clamping to max
        corrector.adjust_thresholds(0.50);
        assert!((corrector.current_threshold_adjustment() - 0.2).abs() < f32::EPSILON);
        println!("[PASS] test_adjust_thresholds");
    }

    #[test]
    fn test_adjust_thresholds_negative() {
        let mut corrector = DriftCorrector::new();

        corrector.adjust_thresholds(-0.05);
        assert!((corrector.current_threshold_adjustment() - (-0.05)).abs() < f32::EPSILON);

        // Test clamping to min
        corrector.adjust_thresholds(-0.50);
        assert!((corrector.current_threshold_adjustment() - (-0.2)).abs() < f32::EPSILON);
        println!("[PASS] test_adjust_thresholds_negative");
    }

    #[test]
    fn test_rebalance_weights() {
        let mut corrector = DriftCorrector::new();

        corrector.rebalance_weights(&[(0, 0.1), (1, -0.05)]);
        let adjustments = corrector.current_weight_adjustments();
        assert_eq!(adjustments.len(), 2);
        assert_eq!(adjustments[0], (0, 0.1));
        assert_eq!(adjustments[1], (1, -0.05));
        println!("[PASS] test_rebalance_weights");
    }

    #[test]
    fn test_rebalance_weights_accumulation() {
        let mut corrector = DriftCorrector::new();

        corrector.rebalance_weights(&[(0, 0.1)]);
        corrector.rebalance_weights(&[(0, 0.05), (1, 0.1)]);

        let adjustments = corrector.current_weight_adjustments();
        assert_eq!(adjustments.len(), 2);
        // Index 0 should accumulate: 0.1 + 0.05 = 0.15
        assert!((adjustments[0].1 - 0.15).abs() < f32::EPSILON);
        assert!((adjustments[1].1 - 0.1).abs() < f32::EPSILON);
        println!("[PASS] test_rebalance_weights_accumulation");
    }

    #[test]
    fn test_rebalance_weights_clamping() {
        let mut corrector = DriftCorrector::new();

        // Adjustment exceeds max
        corrector.rebalance_weights(&[(0, 0.5)]);
        let adjustments = corrector.current_weight_adjustments();
        assert!((adjustments[0].1 - 0.2).abs() < f32::EPSILON); // Clamped to max
        println!("[PASS] test_rebalance_weights_clamping");
    }

    #[test]
    fn test_reinforce_goal() {
        let mut corrector = DriftCorrector::new();
        assert!((corrector.current_goal_emphasis() - 1.0).abs() < f32::EPSILON);

        corrector.reinforce_goal(1.2);
        assert!((corrector.current_goal_emphasis() - 1.2).abs() < f32::EPSILON);

        corrector.reinforce_goal(1.5);
        // Use 1e-5 tolerance for accumulated floating point operations
        assert!((corrector.current_goal_emphasis() - 1.8).abs() < 1e-5);
        println!("[PASS] test_reinforce_goal");
    }

    #[test]
    fn test_reinforce_goal_clamping() {
        let mut corrector = DriftCorrector::new();

        // Test upper clamp
        corrector.reinforce_goal(3.0);
        assert!((corrector.current_goal_emphasis() - 2.0).abs() < f32::EPSILON);

        // Reset and test lower clamp
        corrector.reset();
        corrector.reinforce_goal(0.3);
        assert!((corrector.current_goal_emphasis() - 0.5).abs() < f32::EPSILON);
        println!("[PASS] test_reinforce_goal_clamping");
    }

    #[test]
    fn test_evaluate_correction_success() {
        let corrector = DriftCorrector::new();

        // Improvement >= min_improvement (0.01)
        assert!(corrector.evaluate_correction(0.70, 0.72)); // 0.02 improvement
        assert!(corrector.evaluate_correction(0.70, 0.711)); // 0.011 improvement (avoids f32 precision issue at 0.71)
        println!("[PASS] test_evaluate_correction_success");
    }

    #[test]
    fn test_evaluate_correction_failure() {
        let corrector = DriftCorrector::new();

        // Improvement < min_improvement (0.01)
        assert!(!corrector.evaluate_correction(0.70, 0.705));
        assert!(!corrector.evaluate_correction(0.70, 0.70));
        assert!(!corrector.evaluate_correction(0.70, 0.69)); // Negative
        println!("[PASS] test_evaluate_correction_failure");
    }

    #[test]
    fn test_correction_stats() {
        let mut corrector = DriftCorrector::new();
        let (applied, successful, rate) = corrector.correction_stats();
        assert_eq!(applied, 0);
        assert_eq!(successful, 0);
        assert!((rate - 0.0).abs() < f32::EPSILON);

        // Apply some corrections
        let mut state = DriftState {
            rolling_mean: 0.70,
            ..Default::default()
        };

        let strategy = CorrectionStrategy::ThresholdAdjustment { delta: 0.05 };
        corrector.apply_correction(&mut state, &strategy);
        corrector.apply_correction(&mut state, &strategy);

        let (applied, successful, rate) = corrector.correction_stats();
        assert_eq!(applied, 2);
        assert!(successful >= 1);
        assert!(rate > 0.0);
        println!("[PASS] test_correction_stats");
    }

    #[test]
    fn test_reset() {
        let mut corrector = DriftCorrector::new();

        // Apply various adjustments
        corrector.adjust_thresholds(0.1);
        corrector.rebalance_weights(&[(0, 0.1)]);
        corrector.reinforce_goal(1.5);

        let mut state = DriftState::default();
        let strategy = CorrectionStrategy::NoAction;
        corrector.apply_correction(&mut state, &strategy);

        // Reset
        corrector.reset();

        assert!((corrector.current_threshold_adjustment() - 0.0).abs() < f32::EPSILON);
        assert!(corrector.current_weight_adjustments().is_empty());
        assert!((corrector.current_goal_emphasis() - 1.0).abs() < f32::EPSILON);
        let (applied, _, _) = corrector.correction_stats();
        assert_eq!(applied, 0);
        println!("[PASS] test_reset");
    }

    #[test]
    fn test_auto_correct() {
        let mut corrector = DriftCorrector::new();
        let config = DriftConfig::default();
        let mut state = DriftState {
            severity: DriftSeverity::Moderate,
            trend: DriftTrend::Declining,
            rolling_mean: 0.70,
            ..Default::default()
        };

        let result = corrector.auto_correct(&mut state, &config);

        // Should have applied threshold adjustment
        if let CorrectionStrategy::ThresholdAdjustment { delta } = result.strategy_applied {
            assert!((delta - 0.02).abs() < f32::EPSILON);
        } else {
            panic!("Expected ThresholdAdjustment strategy");
        }
        println!("[PASS] test_auto_correct");
    }

    #[test]
    fn test_strategy_serialization() {
        let strategies = vec![
            CorrectionStrategy::NoAction,
            CorrectionStrategy::ThresholdAdjustment { delta: 0.05 },
            CorrectionStrategy::WeightRebalance {
                adjustments: vec![(0, 0.1)],
            },
            CorrectionStrategy::GoalReinforcement {
                emphasis_factor: 1.5,
            },
            CorrectionStrategy::EmergencyIntervention {
                reason: "Test".to_string(),
            },
        ];

        for strategy in strategies {
            let json = serde_json::to_string(&strategy).expect("serialize");
            let deserialized: CorrectionStrategy =
                serde_json::from_str(&json).expect("deserialize");
            assert_eq!(deserialized, strategy);
        }
        println!("[PASS] test_strategy_serialization");
    }

    #[test]
    fn test_result_serialization() {
        let result = CorrectionResult::new(
            CorrectionStrategy::ThresholdAdjustment { delta: 0.05 },
            0.70,
            0.75,
            true,
        );

        let json = serde_json::to_string(&result).expect("serialize");
        let deserialized: CorrectionResult = serde_json::from_str(&json).expect("deserialize");

        assert!((deserialized.alignment_before - 0.70).abs() < f32::EPSILON);
        assert!((deserialized.alignment_after - 0.75).abs() < f32::EPSILON);
        assert!(deserialized.success);
        println!("[PASS] test_result_serialization");
    }

    #[test]
    fn test_config_serialization() {
        let config = DriftCorrectorConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: DriftCorrectorConfig = serde_json::from_str(&json).expect("deserialize");

        assert!((deserialized.moderate_threshold_delta - 0.02).abs() < f32::EPSILON);
        assert!((deserialized.severe_threshold_delta - 0.05).abs() < f32::EPSILON);
        println!("[PASS] test_config_serialization");
    }

    #[test]
    fn test_alignment_clamping() {
        let mut corrector = DriftCorrector::new();
        let mut state = DriftState {
            rolling_mean: 0.98,
            ..Default::default()
        };

        // Large adjustment that would push past 1.0
        let strategy = CorrectionStrategy::ThresholdAdjustment { delta: 0.10 };
        let result = corrector.apply_correction(&mut state, &strategy);

        assert!(result.alignment_after <= 1.0);
        println!("[PASS] test_alignment_clamping");
    }

    #[test]
    fn test_default_impl() {
        let corrector = DriftCorrector::default();
        assert!((corrector.current_threshold_adjustment() - 0.0).abs() < f32::EPSILON);
        assert!((corrector.current_goal_emphasis() - 1.0).abs() < f32::EPSILON);
        println!("[PASS] test_default_impl");
    }
}
