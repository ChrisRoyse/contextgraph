//! NORTH-016: Weight Adjuster Service
//!
//! Optimizes section weights based on performance feedback using gradient descent
//! with momentum. This service learns optimal goal weights by adjusting them
//! based on alignment performance metrics.
//!
//! # Algorithm
//!
//! Uses momentum-based gradient descent:
//! - velocity[t] = momentum * velocity[t-1] + lr * gradient
//! - weight[t] = weight[t-1] - velocity[t]
//! - Weights are clamped to configured bounds

use std::collections::HashMap;

use super::super::bootstrap::GoalId;
use super::super::evolution::{GoalEvolutionConfig, WeightAdjustment};

/// Reason for weight adjustment with extended variants for the service
#[derive(Clone, Debug, PartialEq)]
pub enum AdjustmentReason {
    /// Adjustment based on alignment performance metrics
    PerformanceBased { performance_delta: f32 },
    /// Correcting drift from target alignment
    DriftCorrection { drift_magnitude: f32 },
    /// User explicitly adjusted the weight
    UserFeedback { magnitude: f32 },
    /// Weight evolved based on goal hierarchy changes
    EvolutionBased { evolution_score: f32 },
    /// Scheduled periodic weight rebalancing
    Scheduled { cycle_id: u32 },
}

impl AdjustmentReason {
    /// Convert to a simple description string
    pub fn description(&self) -> String {
        match self {
            Self::PerformanceBased { performance_delta } => {
                format!("Performance-based (delta: {:.4})", performance_delta)
            }
            Self::DriftCorrection { drift_magnitude } => {
                format!("Drift correction (magnitude: {:.4})", drift_magnitude)
            }
            Self::UserFeedback { magnitude } => {
                format!("User feedback (magnitude: {:.4})", magnitude)
            }
            Self::EvolutionBased { evolution_score } => {
                format!("Evolution-based (score: {:.4})", evolution_score)
            }
            Self::Scheduled { cycle_id } => {
                format!("Scheduled (cycle: {})", cycle_id)
            }
        }
    }
}

/// Configuration for the weight adjuster service
#[derive(Clone, Debug)]
pub struct WeightAdjusterConfig {
    /// Learning rate for gradient updates (default from GoalEvolutionConfig: 0.05)
    pub learning_rate: f32,
    /// Minimum allowed weight (default: 0.3)
    pub min_weight: f32,
    /// Maximum allowed weight (default: 0.95)
    pub max_weight: f32,
    /// Momentum coefficient for gradient updates (default: 0.9)
    pub momentum: f32,
    /// Minimum performance delta to trigger adjustment
    pub adjustment_threshold: f32,
}

impl Default for WeightAdjusterConfig {
    fn default() -> Self {
        let evolution_config = GoalEvolutionConfig::default();
        Self {
            learning_rate: evolution_config.weight_lr,
            min_weight: evolution_config.weight_bounds.0,
            max_weight: evolution_config.weight_bounds.1,
            momentum: 0.9,
            adjustment_threshold: 0.01,
        }
    }
}

impl WeightAdjusterConfig {
    /// Validate configuration values
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.learning_rate <= 0.0 || self.learning_rate > 1.0 {
            return Err("learning_rate must be in (0.0, 1.0]");
        }
        if self.min_weight < 0.0 {
            return Err("min_weight must be non-negative");
        }
        if self.max_weight > 1.0 {
            return Err("max_weight must not exceed 1.0");
        }
        if self.min_weight >= self.max_weight {
            return Err("min_weight must be less than max_weight");
        }
        if self.momentum < 0.0 || self.momentum >= 1.0 {
            return Err("momentum must be in [0.0, 1.0)");
        }
        if self.adjustment_threshold < 0.0 {
            return Err("adjustment_threshold must be non-negative");
        }
        Ok(())
    }
}

/// Report summarizing weight adjustments applied
#[derive(Clone, Debug)]
pub struct AdjustmentReport {
    /// Number of adjustments applied
    pub adjustments_applied: usize,
    /// Number of adjustments skipped (failed validation)
    pub adjustments_skipped: usize,
    /// Total weight delta (sum of absolute deltas)
    pub total_delta: f32,
    /// Average delta per adjustment
    pub avg_delta: f32,
    /// Maximum absolute delta
    pub max_delta: f32,
    /// Goals that were adjusted
    pub adjusted_goals: Vec<GoalId>,
    /// Goals that were skipped with reasons
    pub skipped_goals: Vec<(GoalId, String)>,
}

impl AdjustmentReport {
    /// Create a new empty report
    pub fn new() -> Self {
        Self {
            adjustments_applied: 0,
            adjustments_skipped: 0,
            total_delta: 0.0,
            avg_delta: 0.0,
            max_delta: 0.0,
            adjusted_goals: Vec::new(),
            skipped_goals: Vec::new(),
        }
    }

    /// Check if any adjustments were made
    pub fn has_adjustments(&self) -> bool {
        self.adjustments_applied > 0
    }

    /// Check if all adjustments were successful
    pub fn all_successful(&self) -> bool {
        self.adjustments_skipped == 0
    }
}

impl Default for AdjustmentReport {
    fn default() -> Self {
        Self::new()
    }
}

/// Service for optimizing goal section weights based on performance feedback
#[derive(Debug)]
pub struct WeightAdjuster {
    /// Configuration for the adjuster
    config: WeightAdjusterConfig,
    /// Momentum velocities per goal for gradient descent
    velocities: HashMap<GoalId, f32>,
}

impl WeightAdjuster {
    /// Create a new weight adjuster with default configuration
    pub fn new() -> Self {
        Self {
            config: WeightAdjusterConfig::default(),
            velocities: HashMap::new(),
        }
    }

    /// Create a weight adjuster with custom configuration
    ///
    /// # Errors
    /// Returns error if configuration is invalid
    pub fn with_config(config: WeightAdjusterConfig) -> Result<Self, &'static str> {
        config.validate()?;
        Ok(Self {
            config,
            velocities: HashMap::new(),
        })
    }

    /// Get the current configuration
    pub fn config(&self) -> &WeightAdjusterConfig {
        &self.config
    }

    /// Compute a weight adjustment for a goal based on performance
    ///
    /// Performance is expected to be in [0.0, 1.0] where:
    /// - 1.0 = perfect alignment, weight should increase
    /// - 0.5 = neutral, no change needed
    /// - 0.0 = poor alignment, weight should decrease
    pub fn compute_adjustment(
        &self,
        goal_id: &GoalId,
        performance: f32,
        current_weight: f32,
    ) -> WeightAdjustment {
        // Target weight is current weight adjusted by performance delta from neutral (0.5)
        let performance_delta = performance - 0.5;
        let target_direction = if performance_delta > 0.0 { 1.0 } else { -1.0 };

        // Scale the adjustment by learning rate and performance magnitude
        let adjustment_magnitude = performance_delta.abs() * self.config.learning_rate;
        let raw_new_weight = current_weight + (target_direction * adjustment_magnitude);

        // Clamp to bounds
        let new_weight = self.clamp_weight(raw_new_weight);

        WeightAdjustment {
            goal_id: goal_id.clone(),
            old_weight: current_weight,
            new_weight,
            reason: super::super::evolution::AdjustmentReason::HighRetrievalActivity,
        }
    }

    /// Apply a batch of weight adjustments and return a report
    pub fn apply_adjustments(&mut self, adjustments: &[WeightAdjustment]) -> AdjustmentReport {
        let mut report = AdjustmentReport::new();

        for adjustment in adjustments {
            if self.validate_adjustment(adjustment) {
                let delta = (adjustment.new_weight - adjustment.old_weight).abs();
                report.total_delta += delta;
                report.max_delta = report.max_delta.max(delta);
                report.adjusted_goals.push(adjustment.goal_id.clone());
                report.adjustments_applied += 1;
            } else {
                let reason = self.validation_failure_reason(adjustment);
                report
                    .skipped_goals
                    .push((adjustment.goal_id.clone(), reason));
                report.adjustments_skipped += 1;
            }
        }

        // Calculate average delta
        if report.adjustments_applied > 0 {
            report.avg_delta = report.total_delta / report.adjustments_applied as f32;
        }

        report
    }

    /// Perform a single gradient step from current weight towards target
    ///
    /// Uses the formula: new_weight = current + lr * (target - current)
    pub fn gradient_step(&self, current: f32, target: f32, lr: f32) -> f32 {
        let gradient = target - current;
        current + lr * gradient
    }

    /// Clamp a weight value to the configured bounds
    pub fn clamp_weight(&self, weight: f32) -> f32 {
        weight.clamp(self.config.min_weight, self.config.max_weight)
    }

    /// Check if a performance delta is significant enough to trigger adjustment
    pub fn should_adjust(&self, performance_delta: f32) -> bool {
        performance_delta.abs() >= self.config.adjustment_threshold
    }

    /// Compute momentum-adjusted gradient for a goal
    ///
    /// Uses exponential moving average: v[t] = momentum * v[t-1] + (1 - momentum) * gradient
    pub fn compute_momentum(&mut self, goal_id: &GoalId, gradient: f32) -> f32 {
        let prev_velocity = self.velocities.get(goal_id).copied().unwrap_or(0.0);
        let new_velocity =
            self.config.momentum * prev_velocity + (1.0 - self.config.momentum) * gradient;
        self.velocities.insert(goal_id.clone(), new_velocity);
        new_velocity
    }

    /// Validate that an adjustment is within acceptable bounds
    pub fn validate_adjustment(&self, adjustment: &WeightAdjustment) -> bool {
        // Check new weight is within bounds
        if adjustment.new_weight < self.config.min_weight
            || adjustment.new_weight > self.config.max_weight
        {
            return false;
        }

        // Check old weight is valid (non-negative)
        if adjustment.old_weight < 0.0 {
            return false;
        }

        // Check for NaN values
        if adjustment.new_weight.is_nan() || adjustment.old_weight.is_nan() {
            return false;
        }

        true
    }

    /// Get the reason why an adjustment failed validation
    fn validation_failure_reason(&self, adjustment: &WeightAdjustment) -> String {
        if adjustment.new_weight.is_nan() {
            return "new_weight is NaN".to_string();
        }
        if adjustment.old_weight.is_nan() {
            return "old_weight is NaN".to_string();
        }
        if adjustment.new_weight < self.config.min_weight {
            return format!(
                "new_weight {} below min {}",
                adjustment.new_weight, self.config.min_weight
            );
        }
        if adjustment.new_weight > self.config.max_weight {
            return format!(
                "new_weight {} above max {}",
                adjustment.new_weight, self.config.max_weight
            );
        }
        if adjustment.old_weight < 0.0 {
            return format!("old_weight {} is negative", adjustment.old_weight);
        }
        "unknown validation failure".to_string()
    }

    /// Reset momentum for a specific goal
    pub fn reset_momentum(&mut self, goal_id: &GoalId) {
        self.velocities.remove(goal_id);
    }

    /// Reset all momentum values
    pub fn reset_all_momentum(&mut self) {
        self.velocities.clear();
    }

    /// Get current velocity for a goal (for debugging/monitoring)
    pub fn get_velocity(&self, goal_id: &GoalId) -> Option<f32> {
        self.velocities.get(goal_id).copied()
    }
}

impl Default for WeightAdjuster {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === WeightAdjusterConfig Tests ===

    #[test]
    fn test_config_default_values() {
        let config = WeightAdjusterConfig::default();

        assert!((config.learning_rate - 0.05).abs() < f32::EPSILON);
        assert!((config.min_weight - 0.3).abs() < f32::EPSILON);
        assert!((config.max_weight - 0.95).abs() < f32::EPSILON);
        assert!((config.momentum - 0.9).abs() < f32::EPSILON);
        assert!((config.adjustment_threshold - 0.01).abs() < f32::EPSILON);

        println!("[PASS] test_config_default_values");
    }

    #[test]
    fn test_config_validation_valid() {
        let config = WeightAdjusterConfig {
            learning_rate: 0.1,
            min_weight: 0.1,
            max_weight: 0.9,
            momentum: 0.5,
            adjustment_threshold: 0.05,
        };

        assert!(config.validate().is_ok());
        println!("[PASS] test_config_validation_valid");
    }

    #[test]
    fn test_config_validation_invalid_learning_rate() {
        let config = WeightAdjusterConfig {
            learning_rate: 0.0,
            ..WeightAdjusterConfig::default()
        };
        assert!(config.validate().is_err());

        let config = WeightAdjusterConfig {
            learning_rate: 1.5,
            ..WeightAdjusterConfig::default()
        };
        assert!(config.validate().is_err());

        let config = WeightAdjusterConfig {
            learning_rate: -0.1,
            ..WeightAdjusterConfig::default()
        };
        assert!(config.validate().is_err());

        println!("[PASS] test_config_validation_invalid_learning_rate");
    }

    #[test]
    fn test_config_validation_invalid_weight_bounds() {
        // min_weight negative
        let config = WeightAdjusterConfig {
            min_weight: -0.1,
            ..WeightAdjusterConfig::default()
        };
        assert!(config.validate().is_err());

        // max_weight > 1.0
        let config = WeightAdjusterConfig {
            max_weight: 1.1,
            ..WeightAdjusterConfig::default()
        };
        assert!(config.validate().is_err());

        // min >= max
        let config = WeightAdjusterConfig {
            min_weight: 0.8,
            max_weight: 0.5,
            ..WeightAdjusterConfig::default()
        };
        assert!(config.validate().is_err());

        println!("[PASS] test_config_validation_invalid_weight_bounds");
    }

    #[test]
    fn test_config_validation_invalid_momentum() {
        let config = WeightAdjusterConfig {
            momentum: 1.0,
            ..WeightAdjusterConfig::default()
        };
        assert!(config.validate().is_err());

        let config = WeightAdjusterConfig {
            momentum: -0.1,
            ..WeightAdjusterConfig::default()
        };
        assert!(config.validate().is_err());

        println!("[PASS] test_config_validation_invalid_momentum");
    }

    // === WeightAdjuster Construction Tests ===

    #[test]
    fn test_new_default() {
        let adjuster = WeightAdjuster::new();

        assert!((adjuster.config().learning_rate - 0.05).abs() < f32::EPSILON);
        assert!(adjuster.velocities.is_empty());

        println!("[PASS] test_new_default");
    }

    #[test]
    fn test_with_config_valid() {
        let config = WeightAdjusterConfig {
            learning_rate: 0.1,
            min_weight: 0.2,
            max_weight: 0.8,
            momentum: 0.85,
            adjustment_threshold: 0.02,
        };

        let adjuster = WeightAdjuster::with_config(config.clone());
        assert!(adjuster.is_ok());

        let adjuster = adjuster.unwrap();
        assert!((adjuster.config().learning_rate - 0.1).abs() < f32::EPSILON);
        assert!((adjuster.config().min_weight - 0.2).abs() < f32::EPSILON);

        println!("[PASS] test_with_config_valid");
    }

    #[test]
    fn test_with_config_invalid() {
        let config = WeightAdjusterConfig {
            learning_rate: 0.0, // Invalid
            ..WeightAdjusterConfig::default()
        };

        let result = WeightAdjuster::with_config(config);
        assert!(result.is_err());

        println!("[PASS] test_with_config_invalid");
    }

    // === Gradient Step Tests ===

    #[test]
    fn test_gradient_step_towards_target() {
        let adjuster = WeightAdjuster::new();

        // Moving from 0.5 towards 0.8 with lr=0.1
        // Expected: 0.5 + 0.1 * (0.8 - 0.5) = 0.5 + 0.03 = 0.53
        let result = adjuster.gradient_step(0.5, 0.8, 0.1);
        assert!((result - 0.53).abs() < 1e-6);

        // Moving from 0.8 towards 0.5 with lr=0.1
        // Expected: 0.8 + 0.1 * (0.5 - 0.8) = 0.8 - 0.03 = 0.77
        let result = adjuster.gradient_step(0.8, 0.5, 0.1);
        assert!((result - 0.77).abs() < 1e-6);

        println!("[PASS] test_gradient_step_towards_target");
    }

    #[test]
    fn test_gradient_step_at_target() {
        let adjuster = WeightAdjuster::new();

        // Already at target, no change
        let result = adjuster.gradient_step(0.6, 0.6, 0.1);
        assert!((result - 0.6).abs() < f32::EPSILON);

        println!("[PASS] test_gradient_step_at_target");
    }

    #[test]
    fn test_gradient_step_zero_learning_rate() {
        let adjuster = WeightAdjuster::new();

        // Zero lr means no change
        let result = adjuster.gradient_step(0.5, 0.9, 0.0);
        assert!((result - 0.5).abs() < f32::EPSILON);

        println!("[PASS] test_gradient_step_zero_learning_rate");
    }

    #[test]
    fn test_gradient_step_full_learning_rate() {
        let adjuster = WeightAdjuster::new();

        // lr=1.0 means jump directly to target
        let result = adjuster.gradient_step(0.5, 0.9, 1.0);
        assert!((result - 0.9).abs() < 1e-6);

        println!("[PASS] test_gradient_step_full_learning_rate");
    }

    // === Clamp Weight Tests ===

    #[test]
    fn test_clamp_weight_within_bounds() {
        let adjuster = WeightAdjuster::new();

        let result = adjuster.clamp_weight(0.6);
        assert!((result - 0.6).abs() < f32::EPSILON);

        println!("[PASS] test_clamp_weight_within_bounds");
    }

    #[test]
    fn test_clamp_weight_below_min() {
        let adjuster = WeightAdjuster::new();

        let result = adjuster.clamp_weight(0.1);
        assert!((result - 0.3).abs() < f32::EPSILON); // min is 0.3

        let result = adjuster.clamp_weight(-0.5);
        assert!((result - 0.3).abs() < f32::EPSILON);

        println!("[PASS] test_clamp_weight_below_min");
    }

    #[test]
    fn test_clamp_weight_above_max() {
        let adjuster = WeightAdjuster::new();

        let result = adjuster.clamp_weight(0.99);
        assert!((result - 0.95).abs() < f32::EPSILON); // max is 0.95

        let result = adjuster.clamp_weight(1.5);
        assert!((result - 0.95).abs() < f32::EPSILON);

        println!("[PASS] test_clamp_weight_above_max");
    }

    #[test]
    fn test_clamp_weight_at_boundaries() {
        let adjuster = WeightAdjuster::new();

        let result = adjuster.clamp_weight(0.3);
        assert!((result - 0.3).abs() < f32::EPSILON);

        let result = adjuster.clamp_weight(0.95);
        assert!((result - 0.95).abs() < f32::EPSILON);

        println!("[PASS] test_clamp_weight_at_boundaries");
    }

    // === Should Adjust Tests ===

    #[test]
    fn test_should_adjust_above_threshold() {
        let adjuster = WeightAdjuster::new();

        assert!(adjuster.should_adjust(0.05)); // Above 0.01 threshold
        assert!(adjuster.should_adjust(-0.05)); // Negative also above threshold
        assert!(adjuster.should_adjust(0.01)); // Exactly at threshold

        println!("[PASS] test_should_adjust_above_threshold");
    }

    #[test]
    fn test_should_adjust_below_threshold() {
        let adjuster = WeightAdjuster::new();

        assert!(!adjuster.should_adjust(0.005)); // Below 0.01 threshold
        assert!(!adjuster.should_adjust(-0.005));
        assert!(!adjuster.should_adjust(0.0));

        println!("[PASS] test_should_adjust_below_threshold");
    }

    // === Compute Momentum Tests ===

    #[test]
    fn test_compute_momentum_first_call() {
        let mut adjuster = WeightAdjuster::new();
        let goal_id = GoalId::new();

        // First call: v = 0.9 * 0 + 0.1 * 0.5 = 0.05
        let velocity = adjuster.compute_momentum(&goal_id, 0.5);
        assert!((velocity - 0.05).abs() < 1e-6);

        println!("[PASS] test_compute_momentum_first_call");
    }

    #[test]
    fn test_compute_momentum_accumulation() {
        let mut adjuster = WeightAdjuster::new();
        let goal_id = GoalId::new();

        // First call with gradient 1.0
        // v1 = 0.9 * 0 + 0.1 * 1.0 = 0.1
        let v1 = adjuster.compute_momentum(&goal_id, 1.0);
        assert!((v1 - 0.1).abs() < 1e-6);

        // Second call with gradient 1.0
        // v2 = 0.9 * 0.1 + 0.1 * 1.0 = 0.09 + 0.1 = 0.19
        let v2 = adjuster.compute_momentum(&goal_id, 1.0);
        assert!((v2 - 0.19).abs() < 1e-6);

        // Third call with gradient 1.0
        // v3 = 0.9 * 0.19 + 0.1 * 1.0 = 0.171 + 0.1 = 0.271
        let v3 = adjuster.compute_momentum(&goal_id, 1.0);
        assert!((v3 - 0.271).abs() < 1e-6);

        println!("[PASS] test_compute_momentum_accumulation");
    }

    #[test]
    fn test_compute_momentum_different_goals() {
        let mut adjuster = WeightAdjuster::new();
        let goal1 = GoalId::new();
        let goal2 = GoalId::new();

        let v1 = adjuster.compute_momentum(&goal1, 1.0);
        let v2 = adjuster.compute_momentum(&goal2, -1.0);

        assert!((v1 - 0.1).abs() < 1e-6);
        assert!((v2 - (-0.1)).abs() < 1e-6);

        // Check they're stored independently
        assert!(adjuster.get_velocity(&goal1).is_some());
        assert!(adjuster.get_velocity(&goal2).is_some());

        println!("[PASS] test_compute_momentum_different_goals");
    }

    #[test]
    fn test_reset_momentum() {
        let mut adjuster = WeightAdjuster::new();
        let goal_id = GoalId::new();

        adjuster.compute_momentum(&goal_id, 1.0);
        assert!(adjuster.get_velocity(&goal_id).is_some());

        adjuster.reset_momentum(&goal_id);
        assert!(adjuster.get_velocity(&goal_id).is_none());

        println!("[PASS] test_reset_momentum");
    }

    #[test]
    fn test_reset_all_momentum() {
        let mut adjuster = WeightAdjuster::new();
        let goal1 = GoalId::new();
        let goal2 = GoalId::new();

        adjuster.compute_momentum(&goal1, 1.0);
        adjuster.compute_momentum(&goal2, 1.0);

        adjuster.reset_all_momentum();

        assert!(adjuster.get_velocity(&goal1).is_none());
        assert!(adjuster.get_velocity(&goal2).is_none());

        println!("[PASS] test_reset_all_momentum");
    }

    // === Validate Adjustment Tests ===

    #[test]
    fn test_validate_adjustment_valid() {
        let adjuster = WeightAdjuster::new();

        let adjustment = WeightAdjustment {
            goal_id: GoalId::new(),
            old_weight: 0.5,
            new_weight: 0.6,
            reason: super::super::super::evolution::AdjustmentReason::HighRetrievalActivity,
        };

        assert!(adjuster.validate_adjustment(&adjustment));

        println!("[PASS] test_validate_adjustment_valid");
    }

    #[test]
    fn test_validate_adjustment_below_min() {
        let adjuster = WeightAdjuster::new();

        let adjustment = WeightAdjustment {
            goal_id: GoalId::new(),
            old_weight: 0.5,
            new_weight: 0.2, // Below min 0.3
            reason: super::super::super::evolution::AdjustmentReason::LowActivity,
        };

        assert!(!adjuster.validate_adjustment(&adjustment));

        println!("[PASS] test_validate_adjustment_below_min");
    }

    #[test]
    fn test_validate_adjustment_above_max() {
        let adjuster = WeightAdjuster::new();

        let adjustment = WeightAdjustment {
            goal_id: GoalId::new(),
            old_weight: 0.9,
            new_weight: 0.98, // Above max 0.95
            reason: super::super::super::evolution::AdjustmentReason::HighRetrievalActivity,
        };

        assert!(!adjuster.validate_adjustment(&adjustment));

        println!("[PASS] test_validate_adjustment_above_max");
    }

    #[test]
    fn test_validate_adjustment_negative_old_weight() {
        let adjuster = WeightAdjuster::new();

        let adjustment = WeightAdjustment {
            goal_id: GoalId::new(),
            old_weight: -0.1,
            new_weight: 0.5,
            reason: super::super::super::evolution::AdjustmentReason::LowActivity,
        };

        assert!(!adjuster.validate_adjustment(&adjustment));

        println!("[PASS] test_validate_adjustment_negative_old_weight");
    }

    #[test]
    fn test_validate_adjustment_nan() {
        let adjuster = WeightAdjuster::new();

        let adjustment = WeightAdjustment {
            goal_id: GoalId::new(),
            old_weight: 0.5,
            new_weight: f32::NAN,
            reason: super::super::super::evolution::AdjustmentReason::HighRetrievalActivity,
        };

        assert!(!adjuster.validate_adjustment(&adjustment));

        let adjustment = WeightAdjustment {
            goal_id: GoalId::new(),
            old_weight: f32::NAN,
            new_weight: 0.5,
            reason: super::super::super::evolution::AdjustmentReason::HighRetrievalActivity,
        };

        assert!(!adjuster.validate_adjustment(&adjustment));

        println!("[PASS] test_validate_adjustment_nan");
    }

    // === Compute Adjustment Tests ===

    #[test]
    fn test_compute_adjustment_high_performance() {
        let adjuster = WeightAdjuster::new();
        let goal_id = GoalId::new();

        // Performance 0.9 (0.4 above neutral 0.5) with lr=0.05
        // Expected delta: 0.4 * 0.05 = 0.02 increase
        let adjustment = adjuster.compute_adjustment(&goal_id, 0.9, 0.5);

        assert_eq!(adjustment.goal_id, goal_id);
        assert!((adjustment.old_weight - 0.5).abs() < f32::EPSILON);
        assert!((adjustment.new_weight - 0.52).abs() < 1e-6);

        println!("[PASS] test_compute_adjustment_high_performance");
    }

    #[test]
    fn test_compute_adjustment_low_performance() {
        let adjuster = WeightAdjuster::new();
        let goal_id = GoalId::new();

        // Performance 0.1 (0.4 below neutral 0.5) with lr=0.05
        // Expected delta: 0.4 * 0.05 = 0.02 decrease
        let adjustment = adjuster.compute_adjustment(&goal_id, 0.1, 0.5);

        assert!((adjustment.old_weight - 0.5).abs() < f32::EPSILON);
        assert!((adjustment.new_weight - 0.48).abs() < 1e-6);

        println!("[PASS] test_compute_adjustment_low_performance");
    }

    #[test]
    fn test_compute_adjustment_neutral_performance() {
        let adjuster = WeightAdjuster::new();
        let goal_id = GoalId::new();

        // Performance at neutral 0.5, no change
        let adjustment = adjuster.compute_adjustment(&goal_id, 0.5, 0.6);

        assert!((adjustment.new_weight - 0.6).abs() < f32::EPSILON);

        println!("[PASS] test_compute_adjustment_neutral_performance");
    }

    #[test]
    fn test_compute_adjustment_clamps_to_bounds() {
        let adjuster = WeightAdjuster::new();
        let goal_id = GoalId::new();

        // Very low weight with low performance should clamp to min
        let adjustment = adjuster.compute_adjustment(&goal_id, 0.0, 0.31);
        assert!(adjustment.new_weight >= 0.3);

        // Very high weight with high performance should clamp to max
        let adjustment = adjuster.compute_adjustment(&goal_id, 1.0, 0.94);
        assert!(adjustment.new_weight <= 0.95);

        println!("[PASS] test_compute_adjustment_clamps_to_bounds");
    }

    // === Apply Adjustments Tests ===

    #[test]
    fn test_apply_adjustments_empty() {
        let mut adjuster = WeightAdjuster::new();

        let report = adjuster.apply_adjustments(&[]);

        assert_eq!(report.adjustments_applied, 0);
        assert_eq!(report.adjustments_skipped, 0);
        assert!((report.total_delta - 0.0).abs() < f32::EPSILON);
        assert!(!report.has_adjustments());
        assert!(report.all_successful());

        println!("[PASS] test_apply_adjustments_empty");
    }

    #[test]
    fn test_apply_adjustments_all_valid() {
        let mut adjuster = WeightAdjuster::new();

        let adjustments = vec![
            WeightAdjustment {
                goal_id: GoalId::new(),
                old_weight: 0.5,
                new_weight: 0.6,
                reason: super::super::super::evolution::AdjustmentReason::HighRetrievalActivity,
            },
            WeightAdjustment {
                goal_id: GoalId::new(),
                old_weight: 0.7,
                new_weight: 0.8,
                reason: super::super::super::evolution::AdjustmentReason::HighRetrievalActivity,
            },
        ];

        let report = adjuster.apply_adjustments(&adjustments);

        assert_eq!(report.adjustments_applied, 2);
        assert_eq!(report.adjustments_skipped, 0);
        assert!((report.total_delta - 0.2).abs() < 1e-6); // 0.1 + 0.1
        assert!((report.avg_delta - 0.1).abs() < 1e-6);
        assert!((report.max_delta - 0.1).abs() < 1e-6);
        assert!(report.has_adjustments());
        assert!(report.all_successful());

        println!("[PASS] test_apply_adjustments_all_valid");
    }

    #[test]
    fn test_apply_adjustments_some_invalid() {
        let mut adjuster = WeightAdjuster::new();

        let adjustments = vec![
            WeightAdjustment {
                goal_id: GoalId::new(),
                old_weight: 0.5,
                new_weight: 0.6,
                reason: super::super::super::evolution::AdjustmentReason::HighRetrievalActivity,
            },
            WeightAdjustment {
                goal_id: GoalId::new(),
                old_weight: 0.5,
                new_weight: 0.1, // Below min 0.3 - invalid
                reason: super::super::super::evolution::AdjustmentReason::LowActivity,
            },
            WeightAdjustment {
                goal_id: GoalId::new(),
                old_weight: 0.8,
                new_weight: 0.85,
                reason: super::super::super::evolution::AdjustmentReason::HighRetrievalActivity,
            },
        ];

        let report = adjuster.apply_adjustments(&adjustments);

        assert_eq!(report.adjustments_applied, 2);
        assert_eq!(report.adjustments_skipped, 1);
        assert!((report.total_delta - 0.15).abs() < 1e-6); // 0.1 + 0.05
        assert!(report.has_adjustments());
        assert!(!report.all_successful());
        assert_eq!(report.skipped_goals.len(), 1);

        println!("[PASS] test_apply_adjustments_some_invalid");
    }

    #[test]
    fn test_apply_adjustments_varying_deltas() {
        let mut adjuster = WeightAdjuster::new();

        let adjustments = vec![
            WeightAdjustment {
                goal_id: GoalId::new(),
                old_weight: 0.5,
                new_weight: 0.55, // delta = 0.05
                reason: super::super::super::evolution::AdjustmentReason::HighRetrievalActivity,
            },
            WeightAdjustment {
                goal_id: GoalId::new(),
                old_weight: 0.6,
                new_weight: 0.8, // delta = 0.20
                reason: super::super::super::evolution::AdjustmentReason::HighRetrievalActivity,
            },
            WeightAdjustment {
                goal_id: GoalId::new(),
                old_weight: 0.7,
                new_weight: 0.65, // delta = 0.05
                reason: super::super::super::evolution::AdjustmentReason::LowActivity,
            },
        ];

        let report = adjuster.apply_adjustments(&adjustments);

        assert_eq!(report.adjustments_applied, 3);
        assert!((report.total_delta - 0.30).abs() < 1e-6);
        assert!((report.avg_delta - 0.10).abs() < 1e-6);
        assert!((report.max_delta - 0.20).abs() < 1e-6);

        println!("[PASS] test_apply_adjustments_varying_deltas");
    }

    // === AdjustmentReport Tests ===

    #[test]
    fn test_adjustment_report_new() {
        let report = AdjustmentReport::new();

        assert_eq!(report.adjustments_applied, 0);
        assert_eq!(report.adjustments_skipped, 0);
        assert!((report.total_delta - 0.0).abs() < f32::EPSILON);
        assert!((report.avg_delta - 0.0).abs() < f32::EPSILON);
        assert!((report.max_delta - 0.0).abs() < f32::EPSILON);
        assert!(report.adjusted_goals.is_empty());
        assert!(report.skipped_goals.is_empty());

        println!("[PASS] test_adjustment_report_new");
    }

    #[test]
    fn test_adjustment_report_default() {
        let report = AdjustmentReport::default();

        assert!(!report.has_adjustments());
        assert!(report.all_successful());

        println!("[PASS] test_adjustment_report_default");
    }

    // === AdjustmentReason Tests ===

    #[test]
    fn test_adjustment_reason_equality() {
        let r1 = AdjustmentReason::PerformanceBased {
            performance_delta: 0.5,
        };
        let r2 = AdjustmentReason::PerformanceBased {
            performance_delta: 0.5,
        };
        let r3 = AdjustmentReason::PerformanceBased {
            performance_delta: 0.3,
        };

        assert_eq!(r1, r2);
        assert_ne!(r1, r3);

        assert_eq!(
            AdjustmentReason::Scheduled { cycle_id: 1 },
            AdjustmentReason::Scheduled { cycle_id: 1 }
        );
        assert_ne!(
            AdjustmentReason::Scheduled { cycle_id: 1 },
            AdjustmentReason::Scheduled { cycle_id: 2 }
        );

        println!("[PASS] test_adjustment_reason_equality");
    }

    #[test]
    fn test_adjustment_reason_description() {
        let r1 = AdjustmentReason::PerformanceBased {
            performance_delta: 0.15,
        };
        assert!(r1.description().contains("Performance-based"));
        assert!(r1.description().contains("0.15"));

        let r2 = AdjustmentReason::DriftCorrection {
            drift_magnitude: 0.25,
        };
        assert!(r2.description().contains("Drift correction"));
        assert!(r2.description().contains("0.25"));

        let r3 = AdjustmentReason::UserFeedback { magnitude: 0.8 };
        assert!(r3.description().contains("User feedback"));
        assert!(r3.description().contains("0.8"));

        let r4 = AdjustmentReason::EvolutionBased {
            evolution_score: 0.6,
        };
        assert!(r4.description().contains("Evolution-based"));
        assert!(r4.description().contains("0.6"));

        let r5 = AdjustmentReason::Scheduled { cycle_id: 42 };
        assert!(r5.description().contains("Scheduled"));
        assert!(r5.description().contains("42"));

        println!("[PASS] test_adjustment_reason_description");
    }

    // === Integration Tests ===

    #[test]
    fn test_gradient_descent_convergence() {
        let config = WeightAdjusterConfig {
            learning_rate: 0.1,
            min_weight: 0.0,
            max_weight: 1.0,
            momentum: 0.0, // No momentum for simple convergence test
            adjustment_threshold: 0.001,
        };
        let adjuster = WeightAdjuster::with_config(config).unwrap();

        let target = 0.8;
        let mut current = 0.2;

        // Run gradient descent for 50 iterations
        for _ in 0..50 {
            current = adjuster.gradient_step(current, target, 0.1);
        }

        // Should converge close to target
        assert!((current - target).abs() < 0.01);

        println!("[PASS] test_gradient_descent_convergence");
    }

    #[test]
    fn test_momentum_accumulates_velocity() {
        // With high momentum (0.9), velocity accumulates over iterations
        let config = WeightAdjusterConfig {
            momentum: 0.9,
            ..WeightAdjusterConfig::default()
        };
        let mut adjuster = WeightAdjuster::with_config(config).unwrap();

        let goal_id = GoalId::new();
        let gradient = 1.0;

        // First call: v1 = 0.9 * 0 + 0.1 * 1.0 = 0.1
        let v1 = adjuster.compute_momentum(&goal_id, gradient);
        assert!((v1 - 0.1).abs() < 1e-6);

        // Second call: v2 = 0.9 * 0.1 + 0.1 * 1.0 = 0.19
        let v2 = adjuster.compute_momentum(&goal_id, gradient);
        assert!((v2 - 0.19).abs() < 1e-6);
        assert!(v2 > v1); // Velocity increased

        // After many iterations, velocity should approach gradient / (1 - momentum) = 1.0
        // i.e., asymptote towards 1.0 for gradient=1.0, momentum=0.9
        let mut v_final = v2;
        for _ in 0..50 {
            v_final = adjuster.compute_momentum(&goal_id, gradient);
        }

        // Should approach 1.0 (exact limit is gradient when sum converges)
        // With momentum=0.9, the geometric series converges to ~1.0
        assert!(v_final > 0.9);
        assert!(v_final < 1.01);

        println!("[PASS] test_momentum_accumulates_velocity");
    }

    #[test]
    fn test_full_adjustment_workflow() {
        let mut adjuster = WeightAdjuster::new();
        let goal1 = GoalId::new();
        let goal2 = GoalId::new();

        // Simulate performance feedback
        let adj1 = adjuster.compute_adjustment(&goal1, 0.8, 0.5); // High performance
        let adj2 = adjuster.compute_adjustment(&goal2, 0.3, 0.6); // Low performance

        // Validate performance-based direction
        assert!(adj1.new_weight > adj1.old_weight); // Should increase
        assert!(adj2.new_weight < adj2.old_weight); // Should decrease

        // Apply adjustments
        let report = adjuster.apply_adjustments(&[adj1.clone(), adj2.clone()]);

        assert_eq!(report.adjustments_applied, 2);
        assert!(report.has_adjustments());
        assert!(report.all_successful());

        println!("[PASS] test_full_adjustment_workflow");
    }

    #[test]
    fn test_weight_bounds_respected_throughout() {
        let config = WeightAdjusterConfig {
            learning_rate: 0.5, // Large LR to test bounds
            min_weight: 0.3,
            max_weight: 0.7,
            momentum: 0.0,
            adjustment_threshold: 0.001,
        };
        let adjuster = WeightAdjuster::with_config(config).unwrap();

        // Extreme high performance on already high weight
        let adj = adjuster.compute_adjustment(&GoalId::new(), 1.0, 0.65);
        assert!(adj.new_weight <= 0.7);

        // Extreme low performance on already low weight
        let adj = adjuster.compute_adjustment(&GoalId::new(), 0.0, 0.35);
        assert!(adj.new_weight >= 0.3);

        println!("[PASS] test_weight_bounds_respected_throughout");
    }
}
