//! Level 3: Thompson Sampling Bandit Threshold Selector
//!
//! Session-level multi-armed bandit for threshold selection.
//! Uses Thompson sampling to balance exploration and exploitation.
//!
//! # Algorithms
//! 1. Thompson Sampling: For each threshold candidate, sample reward ~ Beta(α, β)
//! 2. UCB (Upper Confidence Bound): θ = argmax[μ(θ) + c√(ln(N)/n(θ))]
//! 3. Budgeted UCB: violation_budget(t) = B_0 × exp(-λ×t)

use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Arm of the bandit (threshold candidate)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThresholdArm {
    pub value: f32,
}

/// Statistics for one arm
#[derive(Debug, Clone, Copy)]
pub struct ArmStats {
    /// Number of successes
    pub successes: u32,
    /// Number of failures
    pub failures: u32,
    /// Total pulls
    pub pulls: u32,
    /// Empirical mean reward
    pub mean_reward: f32,
}

impl ArmStats {
    pub fn new() -> Self {
        Self {
            successes: 0,
            failures: 0,
            pulls: 0,
            mean_reward: 0.0,
        }
    }

    pub fn record_success(&mut self) {
        self.successes += 1;
        self.pulls += 1;
        self.update_mean();
    }

    pub fn record_failure(&mut self) {
        self.failures += 1;
        self.pulls += 1;
        self.update_mean();
    }

    fn update_mean(&mut self) {
        if self.pulls > 0 {
            self.mean_reward = self.successes as f32 / self.pulls as f32;
        }
    }

    /// Get Beta distribution parameters (α, β) for Thompson sampling
    pub fn get_beta_params(&self) -> (f32, f32) {
        // Add pseudo-counts for regularization
        let alpha = self.successes as f32 + 1.0;
        let beta = self.failures as f32 + 1.0;
        (alpha, beta)
    }
}

/// Thompson Sampling bandit for threshold selection
#[derive(Debug)]
pub struct ThresholdBandit {
    /// Candidate threshold values
    arms: Vec<ThresholdArm>,
    /// Statistics for each arm
    stats: HashMap<u32, ArmStats>, // arm_index -> stats
    /// Total number of pulls across all arms
    total_pulls: u32,
    /// Exploration coefficient for UCB
    ucb_c: f32,
    /// Initial violation budget
    budget_b0: f32,
    /// Decay rate λ for budget
    budget_lambda: f32,
    /// Creation time
    created_at: DateTime<Utc>,
}

impl ThresholdBandit {
    /// Create new bandit with threshold candidates
    pub fn new(arms: Vec<ThresholdArm>, ucb_c: f32) -> Self {
        let mut stats = HashMap::new();
        for i in 0..arms.len() {
            stats.insert(i as u32, ArmStats::new());
        }

        Self {
            arms,
            stats,
            total_pulls: 0,
            ucb_c,
            budget_b0: 100.0,
            budget_lambda: 0.01,
            created_at: Utc::now(),
        }
    }

    /// Select arm using Thompson sampling
    /// (Simplified: uses Beta mean instead of sampling for determinism)
    pub fn select_thompson(&self) -> Option<ThresholdArm> {
        if self.arms.is_empty() {
            return None;
        }

        let mut best_arm_idx = 0;
        let mut best_score = -1.0f32;

        for (idx, _) in self.arms.iter().enumerate() {
            if let Some(stats) = self.stats.get(&(idx as u32)) {
                let (alpha, beta) = stats.get_beta_params();

                // Use Beta mean: α/(α+β) as proxy for sampling
                let mean = alpha / (alpha + beta);
                if mean > best_score {
                    best_score = mean;
                    best_arm_idx = idx;
                }
            }
        }

        Some(self.arms[best_arm_idx])
    }

    /// Select arm using Upper Confidence Bound
    pub fn select_ucb(&self) -> Option<ThresholdArm> {
        if self.arms.is_empty() {
            return None;
        }

        let mut best_arm_idx = 0;
        let mut best_ucb = f32::NEG_INFINITY;
        let ln_n = (self.total_pulls as f32 + 1.0).ln();

        for (idx, _) in self.arms.iter().enumerate() {
            if let Some(stats) = self.stats.get(&(idx as u32)) {
                let exploration = if stats.pulls > 0 {
                    self.ucb_c * (ln_n / stats.pulls as f32).sqrt()
                } else {
                    f32::INFINITY // Unplayed arms get priority
                };

                let ucb = stats.mean_reward + exploration;
                if ucb > best_ucb {
                    best_ucb = ucb;
                    best_arm_idx = idx;
                }
            }
        }

        Some(self.arms[best_arm_idx])
    }

    /// Get remaining violation budget
    pub fn get_violation_budget(&self) -> f32 {
        let age_secs = Utc::now()
            .signed_duration_since(self.created_at)
            .num_seconds() as f32;

        self.budget_b0 * (-self.budget_lambda * age_secs).exp()
    }

    /// Check if we can still violate constraints (for exploration)
    pub fn can_violate_constraints(&self) -> bool {
        self.get_violation_budget() > 1.0
    }

    /// Record outcome of pulling an arm
    pub fn record_outcome(&mut self, arm: ThresholdArm, success: bool) {
        // Find arm index
        if let Some(idx) = self.arms.iter().position(|a| a.value == arm.value) {
            if let Some(stats) = self.stats.get_mut(&(idx as u32)) {
                if success {
                    stats.record_success();
                } else {
                    stats.record_failure();
                }
            }
        }
        self.total_pulls += 1;
    }

    /// Get statistics for an arm
    pub fn get_arm_stats(&self, arm: ThresholdArm) -> Option<ArmStats> {
        if let Some(idx) = self.arms.iter().position(|a| a.value == arm.value) {
            self.stats.get(&(idx as u32)).copied()
        } else {
            None
        }
    }

    /// Get best arm by empirical mean
    pub fn get_best_arm(&self) -> Option<(ThresholdArm, f32)> {
        let mut best_idx = 0;
        let mut best_mean = -1.0f32;

        for (idx, _) in self.arms.iter().enumerate() {
            if let Some(stats) = self.stats.get(&(idx as u32)) {
                if stats.mean_reward > best_mean {
                    best_mean = stats.mean_reward;
                    best_idx = idx;
                }
            }
        }

        Some((self.arms[best_idx], best_mean))
    }

    /// Get all arm statistics (for monitoring)
    pub fn get_all_stats(&self) -> Vec<(f32, ArmStats)> {
        let mut result = Vec::new();
        for (idx, arm) in self.arms.iter().enumerate() {
            if let Some(stats) = self.stats.get(&(idx as u32)) {
                result.push((arm.value, *stats));
            }
        }
        result
    }

    /// Reset bandit for new session
    pub fn reset(&mut self) {
        self.total_pulls = 0;
        for stats in self.stats.values_mut() {
            *stats = ArmStats::new();
        }
        self.created_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arm_stats() {
        let mut stats = ArmStats::new();
        assert_eq!(stats.mean_reward, 0.0);

        stats.record_success();
        assert_eq!(stats.successes, 1);
        assert_eq!(stats.pulls, 1);
        assert_eq!(stats.mean_reward, 1.0);

        stats.record_failure();
        assert_eq!(stats.failures, 1);
        assert_eq!(stats.pulls, 2);
        assert_eq!(stats.mean_reward, 0.5);
    }

    #[test]
    fn test_beta_params() {
        let mut stats = ArmStats::new();
        stats.record_success();
        stats.record_success();

        let (alpha, beta) = stats.get_beta_params();
        assert_eq!(alpha, 3.0); // 2 + 1
        assert_eq!(beta, 1.0);  // 0 + 1
    }

    #[test]
    fn test_bandit_creation() {
        let arms = vec![
            ThresholdArm { value: 0.70 },
            ThresholdArm { value: 0.75 },
            ThresholdArm { value: 0.80 },
        ];
        let bandit = ThresholdBandit::new(arms, 1.5);
        assert_eq!(bandit.arms.len(), 3);
        assert_eq!(bandit.total_pulls, 0);
    }

    #[test]
    fn test_ucb_selection() {
        let arms = vec![
            ThresholdArm { value: 0.70 },
            ThresholdArm { value: 0.75 },
            ThresholdArm { value: 0.80 },
        ];
        let bandit = ThresholdBandit::new(arms, 1.5);

        // Unplayed arms should get selected (UCB = infinity)
        let selected = bandit.select_ucb();
        assert!(selected.is_some());
    }

    #[test]
    fn test_record_outcomes() {
        let arms = vec![
            ThresholdArm { value: 0.70 },
            ThresholdArm { value: 0.75 },
        ];
        let mut bandit = ThresholdBandit::new(arms, 1.5);

        bandit.record_outcome(ThresholdArm { value: 0.70 }, true);
        bandit.record_outcome(ThresholdArm { value: 0.70 }, true);
        bandit.record_outcome(ThresholdArm { value: 0.75 }, false);

        let stats_70 = bandit.get_arm_stats(ThresholdArm { value: 0.70 }).unwrap();
        assert_eq!(stats_70.successes, 2);
        assert_eq!(stats_70.failures, 0);

        let stats_75 = bandit.get_arm_stats(ThresholdArm { value: 0.75 }).unwrap();
        assert_eq!(stats_75.successes, 0);
        assert_eq!(stats_75.failures, 1);
    }

    #[test]
    fn test_violation_budget() {
        let arms = vec![ThresholdArm { value: 0.75 }];
        let bandit = ThresholdBandit::new(arms, 1.5);

        let budget = bandit.get_violation_budget();
        // Just created, budget should be close to B_0 (100)
        assert!(budget > 99.0 && budget <= 100.0);
    }

    #[test]
    fn test_best_arm() {
        let arms = vec![
            ThresholdArm { value: 0.70 },
            ThresholdArm { value: 0.75 },
            ThresholdArm { value: 0.80 },
        ];
        let mut bandit = ThresholdBandit::new(arms, 1.5);

        // Make 0.80 the best arm
        for _ in 0..10 {
            bandit.record_outcome(ThresholdArm { value: 0.80 }, true);
        }
        for _ in 0..5 {
            bandit.record_outcome(ThresholdArm { value: 0.70 }, false);
        }

        let (best, mean) = bandit.get_best_arm().unwrap();
        assert_eq!(best.value, 0.80);
        assert_eq!(mean, 1.0);
    }
}
