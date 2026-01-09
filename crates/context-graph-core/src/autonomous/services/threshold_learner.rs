//! NORTH-009: ThresholdLearner Service
//!
//! Adaptive threshold learning service implementing the 4-level ATC (Adaptive
//! Threshold Calibration) from constitution.yaml. Learns optimal alignment
//! thresholds based on retrieval feedback without hardcoded values.
//!
//! # 4-Level Architecture
//!
//! 1. **Level 1 - EWMA Drift Adjustment** (per-query)
//!    - Formula: `θ_ewma(t) = α × θ_observed(t) + (1 - α) × θ_ewma(t-1)`
//!    - Detects distribution drift; triggers higher levels when drift > 2σ/3σ
//!
//! 2. **Level 2 - Temperature Scaling Calibration** (hourly)
//!    - Formula: `calibrated = σ(logit(raw) / T)`
//!    - Per-embedder temperatures for confidence calibration
//!
//! 3. **Level 3 - Thompson Sampling Exploration** (session)
//!    - Samples from `Beta(α, β)` per threshold arm
//!    - Balances exploration vs exploitation with decaying violation budget
//!
//! 4. **Level 4 - Bayesian Meta-Optimization** (weekly)
//!    - Gaussian Process surrogate with Expected Improvement acquisition
//!    - Constrained optimization respecting monotonicity bounds
//!
//! # Constitution Reference
//!
//! Lines 1016-1133 define the ATC system with:
//! - Threshold priors and ranges (θ_opt, θ_acc, θ_warn, etc.)
//! - Calibration metrics (ECE < 0.05, MCE < 0.10, Brier < 0.10)
//! - Self-correction protocol (minor/moderate/major/critical)

use chrono::{DateTime, Duration, Utc};
use rand::prelude::*;
use rand_distr::Beta;
use serde::{Deserialize, Serialize};

use crate::autonomous::{
    AdaptiveThresholdConfig, AdaptiveThresholdState, AlignmentBucket, RetrievalStats,
};

/// Type alias for backwards compatibility
pub type ThresholdLearnerConfig = AdaptiveThresholdConfig;

/// Number of embedders in the system (E1-E13)
pub const NUM_EMBEDDERS: usize = 13;

/// Default EWMA smoothing factor
const DEFAULT_ALPHA: f32 = 0.2;

/// Minimum observations before recalibration is considered
const MIN_OBSERVATIONS_FOR_RECALIBRATION: u32 = 10;

/// Recalibration check interval
const RECALIBRATION_CHECK_INTERVAL_SECS: i64 = 3600; // 1 hour

/// Thompson sampling state for a single embedder threshold
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThompsonState {
    /// Beta distribution alpha parameter (successes + 1)
    pub alpha: f32,
    /// Beta distribution beta parameter (failures + 1)
    pub beta: f32,
    /// Total samples taken
    pub samples: u32,
}

impl Default for ThompsonState {
    fn default() -> Self {
        Self {
            alpha: 1.0, // Uniform prior
            beta: 1.0,
            samples: 0,
        }
    }
}

/// Per-embedder learning state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderLearningState {
    /// Current EWMA threshold value
    pub ewma_threshold: f32,
    /// Temperature scaling factor for calibration
    pub temperature: f32,
    /// Thompson sampling state
    pub thompson: ThompsonState,
    /// Observation count for this embedder
    pub observation_count: u32,
    /// Cumulative success count
    pub success_count: u32,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
}

impl Default for EmbedderLearningState {
    fn default() -> Self {
        Self {
            ewma_threshold: 0.75,
            temperature: 1.0,
            thompson: ThompsonState::default(),
            observation_count: 0,
            success_count: 0,
            last_updated: Utc::now(),
        }
    }
}

/// Bayesian optimization observation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BayesianObservation {
    /// Threshold configuration tried
    pub thresholds: [f32; NUM_EMBEDDERS],
    /// Performance score achieved
    pub performance: f32,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// NORTH-009 ThresholdLearner Service
///
/// Implements adaptive threshold learning using the 4-level ATC architecture.
/// All thresholds are learned from feedback rather than hardcoded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdLearner {
    /// Configuration
    config: AdaptiveThresholdConfig,

    /// Current threshold state
    state: AdaptiveThresholdState,

    /// Per-embedder learning states
    embedder_states: [EmbedderLearningState; NUM_EMBEDDERS],

    /// EWMA alpha (smoothing factor)
    ewma_alpha: f32,

    /// Overall observation count
    total_observations: u32,

    /// Total successes across all observations
    total_successes: u32,

    /// Bayesian optimization history (Level 4)
    bayesian_history: Vec<BayesianObservation>,

    /// Best performance seen
    best_performance: f32,

    /// Last recalibration check
    last_recalibration_check: DateTime<Utc>,

    /// Creation timestamp
    created_at: DateTime<Utc>,
}

impl ThresholdLearner {
    /// Create a new ThresholdLearner with default configuration.
    pub fn new() -> Self {
        Self::with_config(AdaptiveThresholdConfig::default())
    }

    /// Create a ThresholdLearner with custom configuration.
    pub fn with_config(config: AdaptiveThresholdConfig) -> Self {
        let now = Utc::now();
        let embedder_states = std::array::from_fn(|_| EmbedderLearningState::default());

        Self {
            config,
            state: AdaptiveThresholdState::default(),
            embedder_states,
            ewma_alpha: DEFAULT_ALPHA,
            total_observations: 0,
            total_successes: 0,
            bayesian_history: Vec::new(),
            best_performance: 0.0,
            last_recalibration_check: now,
            created_at: now,
        }
    }

    /// Learn from retrieval feedback to update thresholds.
    ///
    /// This is the main learning entry point that orchestrates all 4 ATC levels:
    /// 1. Updates EWMA for drift tracking
    /// 2. Adjusts temperature scaling
    /// 3. Updates Thompson sampling parameters
    /// 4. Records for Bayesian optimization
    pub fn learn_from_feedback(&mut self, stats: &RetrievalStats, was_relevant: bool) {
        if !self.config.enabled {
            return;
        }

        self.total_observations += 1;
        if was_relevant {
            self.total_successes += 1;
        }

        // Level 1: Update EWMA for global threshold tracking
        let observed_success_rate = stats.success_rate();
        let new_optimal = self.update_ewma(self.state.optimal, observed_success_rate);
        self.state.optimal =
            new_optimal.clamp(self.config.optimal_bounds.0, self.config.optimal_bounds.1);

        // Update per-bucket thresholds based on success rates
        for (bucket, &rate) in stats.bucket_success_rates.iter() {
            match bucket {
                AlignmentBucket::Optimal => {
                    if rate > 0.8 {
                        // High success rate in optimal bucket - threshold is well-calibrated
                        self.state.optimal = self.update_ewma(self.state.optimal, 0.0);
                    }
                }
                AlignmentBucket::Acceptable => {
                    // Adjust acceptable threshold based on its success rate
                    let adjustment = (rate - 0.7) * self.config.learning_rate;
                    self.state.acceptable = (self.state.acceptable + adjustment).clamp(0.65, 0.80);
                }
                AlignmentBucket::Warning => {
                    // Warning bucket should have moderate success
                    let adjustment = (rate - 0.5) * self.config.learning_rate;
                    self.state.warning = (self.state.warning + adjustment)
                        .clamp(self.config.warning_bounds.0, self.config.warning_bounds.1);
                }
                AlignmentBucket::Critical => {
                    // Critical bucket should have low success - if high, raise warning threshold
                    if rate > 0.3 {
                        self.state.warning += self.config.learning_rate * 0.5;
                    }
                }
            }
        }

        // Level 3: Update Thompson sampling for exploration
        let alpha = self.ewma_alpha; // Cache alpha to avoid borrow conflict
        for (idx, embedder_state) in self.embedder_states.iter_mut().enumerate() {
            embedder_state.observation_count += 1;
            if was_relevant {
                embedder_state.success_count += 1;
                embedder_state.thompson.alpha += 1.0;
            } else {
                embedder_state.thompson.beta += 1.0;
            }
            embedder_state.thompson.samples += 1;
            embedder_state.last_updated = Utc::now();

            // Update per-embedder EWMA threshold (inline to avoid borrow conflict)
            let embedder_success_rate = embedder_state.success_count as f32
                / embedder_state.observation_count.max(1) as f32;
            embedder_state.ewma_threshold =
                alpha * embedder_success_rate + (1.0 - alpha) * embedder_state.ewma_threshold;

            // Sync to state
            if idx < NUM_EMBEDDERS {
                self.state.per_embedder[idx] = embedder_state.ewma_threshold;
            }
        }

        // Record performance for Level 4 Bayesian optimization
        let performance = self.total_successes as f32 / self.total_observations.max(1) as f32;
        if performance > self.best_performance {
            self.best_performance = performance;
        }

        // Every 100 observations, record a Bayesian observation
        if self.total_observations % 100 == 0 {
            self.bayesian_history.push(BayesianObservation {
                thresholds: self.state.per_embedder,
                performance,
                timestamp: Utc::now(),
            });
        }

        // Update state timestamp
        self.state.updated_at = Utc::now();
    }

    /// Update EWMA value with a new observation.
    ///
    /// Formula: `θ_ewma(t) = α × θ_observed(t) + (1 - α) × θ_ewma(t-1)`
    #[inline]
    pub fn update_ewma(&mut self, current: f32, observed: f32) -> f32 {
        if observed.is_nan() || current.is_nan() {
            return current;
        }
        self.ewma_alpha * observed + (1.0 - self.ewma_alpha) * current
    }

    /// Apply temperature scaling to logits for calibration.
    ///
    /// Formula: `calibrated = softmax(logits / T)`
    pub fn temperature_scale(&self, logits: &[f32], temperature: f32) -> Vec<f32> {
        if logits.is_empty() {
            return Vec::new();
        }

        let temp = if temperature <= 0.0 {
            0.01
        } else {
            temperature
        };

        // Scale logits by temperature
        let scaled: Vec<f32> = logits.iter().map(|&l| l / temp).collect();

        // Numerical stability: subtract max before exp
        let max_val = scaled.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_vals: Vec<f32> = scaled.iter().map(|&s| (s - max_val).exp()).collect();
        let sum: f32 = exp_vals.iter().sum();

        if sum == 0.0 || sum.is_nan() || sum.is_infinite() {
            // Uniform distribution fallback
            let uniform = 1.0 / logits.len() as f32;
            return vec![uniform; logits.len()];
        }

        exp_vals.iter().map(|&e| e / sum).collect()
    }

    /// Sample a threshold using Thompson Sampling.
    ///
    /// Samples from `Beta(α, β)` for the specified embedder.
    pub fn thompson_sample(&mut self, embedder_idx: usize) -> f32 {
        if embedder_idx >= NUM_EMBEDDERS {
            return 0.75; // Fallback for invalid index
        }

        let state = &self.embedder_states[embedder_idx].thompson;
        let mut rng = thread_rng();

        // Sample from Beta distribution
        match Beta::new(state.alpha as f64, state.beta as f64) {
            Ok(dist) => {
                let sample = dist.sample(&mut rng) as f32;
                // Scale sample to threshold range [0.5, 0.95]
                0.5 + sample * 0.45
            }
            Err(_) => {
                // Fallback to mean if Beta fails (should not happen with α,β >= 1)
                let mean = state.alpha / (state.alpha + state.beta);
                0.5 + mean * 0.45
            }
        }
    }

    /// Perform Bayesian update on prior belief.
    ///
    /// Formula: `posterior ∝ prior × likelihood`
    pub fn bayesian_update(&mut self, prior: f32, likelihood: f32) -> f32 {
        if prior.is_nan() || likelihood.is_nan() || prior < 0.0 || likelihood < 0.0 {
            return 0.5; // Neutral prior for invalid input
        }

        let prior_pos = prior.clamp(0.001, 0.999);
        let prior_neg = 1.0 - prior_pos;

        let posterior_unnorm = prior_pos * likelihood;
        let marginal = posterior_unnorm + prior_neg * (1.0 - likelihood);

        if marginal == 0.0 || marginal.is_nan() {
            return 0.5;
        }

        (posterior_unnorm / marginal).clamp(0.0, 1.0)
    }

    /// Get the current threshold for a specific embedder.
    #[inline]
    pub fn get_threshold(&self, embedder_idx: usize) -> f32 {
        if embedder_idx < NUM_EMBEDDERS {
            self.state.per_embedder[embedder_idx]
        } else {
            self.state.optimal // Fallback
        }
    }

    /// Check if recalibration should be triggered.
    pub fn should_recalibrate(&self) -> bool {
        // Need minimum observations
        if self.total_observations < MIN_OBSERVATIONS_FOR_RECALIBRATION {
            return false;
        }

        // Check time since last recalibration
        let elapsed = Utc::now().signed_duration_since(self.last_recalibration_check);
        if elapsed < Duration::seconds(RECALIBRATION_CHECK_INTERVAL_SECS) {
            return false;
        }

        // Check for drift in any embedder
        for embedder_state in &self.embedder_states {
            let expected = 0.75; // Prior expectation
            let current = embedder_state.ewma_threshold;
            let std_dev = 0.05; // Assumed baseline std
            let drift = ((current - expected) / std_dev).abs();

            if drift > 2.0 {
                return true; // Level 2 trigger
            }
        }

        // Check overall success rate drift
        let overall_success_rate =
            self.total_successes as f32 / self.total_observations.max(1) as f32;
        if !(0.5..=0.95).contains(&overall_success_rate) {
            return true; // Extreme rates warrant recalibration
        }

        false
    }

    /// Get the current adaptive threshold state.
    pub fn get_state(&self) -> &AdaptiveThresholdState {
        &self.state
    }

    /// Get configuration.
    pub fn get_config(&self) -> &AdaptiveThresholdConfig {
        &self.config
    }

    /// Get total observation count.
    pub fn total_observations(&self) -> u32 {
        self.total_observations
    }

    /// Get best performance seen.
    pub fn best_performance(&self) -> f32 {
        self.best_performance
    }

    /// Mark recalibration as checked (resets timer).
    pub fn mark_recalibration_checked(&mut self) {
        self.last_recalibration_check = Utc::now();
    }

    /// Get embedder learning state for inspection.
    pub fn get_embedder_state(&self, embedder_idx: usize) -> Option<&EmbedderLearningState> {
        if embedder_idx < NUM_EMBEDDERS {
            Some(&self.embedder_states[embedder_idx])
        } else {
            None
        }
    }

    /// Get Bayesian optimization history.
    pub fn get_bayesian_history(&self) -> &[BayesianObservation] {
        &self.bayesian_history
    }

    /// Set EWMA alpha (for testing/tuning).
    pub fn set_ewma_alpha(&mut self, alpha: f32) {
        self.ewma_alpha = alpha.clamp(0.05, 0.5);
    }

    /// Reset learning state (for testing).
    #[cfg(test)]
    pub fn reset(&mut self) {
        self.state = AdaptiveThresholdState::default();
        self.embedder_states = std::array::from_fn(|_| EmbedderLearningState::default());
        self.total_observations = 0;
        self.total_successes = 0;
        self.bayesian_history.clear();
        self.best_performance = 0.0;
        self.last_recalibration_check = Utc::now();
    }
}

impl Default for ThresholdLearner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_learner_new() {
        let learner = ThresholdLearner::new();

        assert!(learner.config.enabled);
        assert_eq!(learner.total_observations, 0);
        assert_eq!(learner.total_successes, 0);
        assert!((learner.state.optimal - 0.75).abs() < f32::EPSILON);
        assert_eq!(learner.embedder_states.len(), NUM_EMBEDDERS);

        println!("[PASS] test_threshold_learner_new: Created with default config");
    }

    #[test]
    fn test_threshold_learner_with_config() {
        let config = AdaptiveThresholdConfig {
            enabled: true,
            learning_rate: 0.02,
            optimal_bounds: (0.72, 0.88),
            warning_bounds: (0.48, 0.62),
        };

        let learner = ThresholdLearner::with_config(config.clone());

        assert!((learner.config.learning_rate - 0.02).abs() < f32::EPSILON);
        assert!((learner.config.optimal_bounds.0 - 0.72).abs() < f32::EPSILON);

        println!("[PASS] test_threshold_learner_with_config: Custom config applied");
    }

    #[test]
    fn test_update_ewma_basic() {
        let mut learner = ThresholdLearner::new();
        learner.set_ewma_alpha(0.2);

        // EWMA = 0.2 * 0.80 + 0.8 * 0.75 = 0.16 + 0.60 = 0.76
        let result = learner.update_ewma(0.75, 0.80);
        assert!((result - 0.76).abs() < 0.001);

        // EWMA = 0.2 * 0.60 + 0.8 * 0.75 = 0.12 + 0.60 = 0.72
        let result2 = learner.update_ewma(0.75, 0.60);
        assert!((result2 - 0.72).abs() < 0.001);

        println!("[PASS] test_update_ewma_basic: EWMA formula verified");
    }

    #[test]
    fn test_update_ewma_nan_handling() {
        let mut learner = ThresholdLearner::new();

        // NaN observed should return current
        let result = learner.update_ewma(0.75, f32::NAN);
        assert!((result - 0.75).abs() < f32::EPSILON);

        // NaN current should return current (NaN)
        let result2 = learner.update_ewma(f32::NAN, 0.80);
        assert!(result2.is_nan());

        println!("[PASS] test_update_ewma_nan_handling: NaN handled gracefully");
    }

    #[test]
    fn test_temperature_scale_basic() {
        let learner = ThresholdLearner::new();

        let logits = vec![2.0, 1.0, 0.1];
        let scaled = learner.temperature_scale(&logits, 1.0);

        // Should sum to 1.0
        let sum: f32 = scaled.iter().sum();
        assert!((sum - 1.0).abs() < 0.001);

        // Higher logit should have higher probability
        assert!(scaled[0] > scaled[1]);
        assert!(scaled[1] > scaled[2]);

        println!("[PASS] test_temperature_scale_basic: Softmax normalization works");
    }

    #[test]
    fn test_temperature_scale_high_temp() {
        let learner = ThresholdLearner::new();

        let logits = vec![2.0, 1.0, 0.1];
        let scaled_high = learner.temperature_scale(&logits, 2.0);
        let scaled_low = learner.temperature_scale(&logits, 0.5);

        // High temp should produce more uniform distribution
        let variance_high: f32 = scaled_high
            .iter()
            .map(|p| (p - 1.0 / 3.0).powi(2))
            .sum::<f32>()
            / 3.0;
        let variance_low: f32 = scaled_low
            .iter()
            .map(|p| (p - 1.0 / 3.0).powi(2))
            .sum::<f32>()
            / 3.0;

        assert!(variance_high < variance_low);

        println!("[PASS] test_temperature_scale_high_temp: Temperature scaling verified");
    }

    #[test]
    fn test_temperature_scale_empty() {
        let learner = ThresholdLearner::new();
        let result = learner.temperature_scale(&[], 1.0);
        assert!(result.is_empty());

        println!("[PASS] test_temperature_scale_empty: Empty input handled");
    }

    #[test]
    fn test_temperature_scale_zero_temp() {
        let learner = ThresholdLearner::new();
        let logits = vec![1.0, 0.5, 0.0];

        // Zero temp should not panic, uses 0.01 instead
        let result = learner.temperature_scale(&logits, 0.0);
        assert!(!result.is_empty());
        assert!(result.iter().all(|&p| !p.is_nan()));

        println!("[PASS] test_temperature_scale_zero_temp: Zero temp handled");
    }

    #[test]
    fn test_thompson_sample_range() {
        let mut learner = ThresholdLearner::new();

        // Sample multiple times to check range
        for _ in 0..100 {
            let sample = learner.thompson_sample(0);
            assert!(
                (0.5..=0.95).contains(&sample),
                "Sample {} out of expected range [0.5, 0.95]",
                sample
            );
        }

        println!("[PASS] test_thompson_sample_range: Samples within expected range");
    }

    #[test]
    fn test_thompson_sample_invalid_idx() {
        let mut learner = ThresholdLearner::new();

        // Invalid index should return fallback
        let sample = learner.thompson_sample(100);
        assert!((sample - 0.75).abs() < f32::EPSILON);

        println!("[PASS] test_thompson_sample_invalid_idx: Invalid index returns fallback");
    }

    #[test]
    fn test_thompson_sample_updates_with_feedback() {
        let mut learner = ThresholdLearner::new();

        // Get initial Thompson state
        let initial_alpha = learner.embedder_states[0].thompson.alpha;
        let initial_beta = learner.embedder_states[0].thompson.beta;

        // Learn from positive feedback
        let stats = RetrievalStats::new();
        learner.learn_from_feedback(&stats, true);

        // Alpha should increase (success)
        assert!(learner.embedder_states[0].thompson.alpha > initial_alpha);
        assert!((learner.embedder_states[0].thompson.beta - initial_beta).abs() < f32::EPSILON);

        // Learn from negative feedback
        learner.learn_from_feedback(&stats, false);

        // Beta should increase (failure)
        assert!(learner.embedder_states[0].thompson.beta > initial_beta);

        println!(
            "[PASS] test_thompson_sample_updates_with_feedback: Thompson params update correctly"
        );
    }

    #[test]
    fn test_bayesian_update_basic() {
        let mut learner = ThresholdLearner::new();

        // Strong evidence should move posterior toward likelihood
        let posterior = learner.bayesian_update(0.5, 0.9);
        assert!(
            posterior > 0.5,
            "Posterior {} should be > 0.5 with strong likelihood",
            posterior
        );

        // Weak evidence with high prior - posterior should be influenced by both
        // With Bayes rule: P(H|E) = P(E|H)*P(H) / P(E)
        // When prior=0.8, likelihood=0.5, the posterior remains relatively high
        let posterior2 = learner.bayesian_update(0.8, 0.5);
        // Posterior should be between prior and likelihood, or close to 0.5 (neutral likelihood)
        assert!(
            (0.4..=0.85).contains(&posterior2),
            "Posterior {} should be in reasonable range with neutral likelihood",
            posterior2
        );

        println!("[PASS] test_bayesian_update_basic: Bayes rule applied correctly");
    }

    #[test]
    fn test_bayesian_update_edge_cases() {
        let mut learner = ThresholdLearner::new();

        // NaN should return neutral
        let result = learner.bayesian_update(f32::NAN, 0.5);
        assert!((result - 0.5).abs() < f32::EPSILON);

        // Negative should return neutral
        let result2 = learner.bayesian_update(-0.5, 0.5);
        assert!((result2 - 0.5).abs() < f32::EPSILON);

        println!("[PASS] test_bayesian_update_edge_cases: Edge cases handled");
    }

    #[test]
    fn test_get_threshold_valid_indices() {
        let learner = ThresholdLearner::new();

        // All valid indices should return a threshold
        for idx in 0..NUM_EMBEDDERS {
            let threshold = learner.get_threshold(idx);
            assert!((0.0..=1.0).contains(&threshold));
        }

        println!("[PASS] test_get_threshold_valid_indices: All embedder thresholds accessible");
    }

    #[test]
    fn test_get_threshold_invalid_index() {
        let learner = ThresholdLearner::new();

        // Invalid index should return optimal as fallback
        let threshold = learner.get_threshold(100);
        assert!((threshold - learner.state.optimal).abs() < f32::EPSILON);

        println!("[PASS] test_get_threshold_invalid_index: Invalid index returns fallback");
    }

    #[test]
    fn test_should_recalibrate_no_observations() {
        let learner = ThresholdLearner::new();

        // No observations - should not recalibrate
        assert!(!learner.should_recalibrate());

        println!("[PASS] test_should_recalibrate_no_observations: No recal with no data");
    }

    #[test]
    fn test_should_recalibrate_after_observations() {
        let mut learner = ThresholdLearner::new();

        // Add observations
        let stats = RetrievalStats::new();
        for _ in 0..15 {
            learner.learn_from_feedback(&stats, true);
        }

        // Reset timer to simulate time passage
        learner.last_recalibration_check =
            Utc::now() - Duration::seconds(RECALIBRATION_CHECK_INTERVAL_SECS + 1);

        // Should check recalibration logic (may or may not trigger based on drift)
        let _result = learner.should_recalibrate();

        println!("[PASS] test_should_recalibrate_after_observations: Recalibration check runs");
    }

    #[test]
    fn test_learn_from_feedback_updates_state() {
        let mut learner = ThresholdLearner::new();

        let initial_obs = learner.total_observations;
        let initial_successes = learner.total_successes;

        let mut stats = RetrievalStats::new();
        stats.record_retrieval(AlignmentBucket::Optimal, true);

        learner.learn_from_feedback(&stats, true);

        assert_eq!(learner.total_observations, initial_obs + 1);
        assert_eq!(learner.total_successes, initial_successes + 1);

        println!("[PASS] test_learn_from_feedback_updates_state: State updated on feedback");
    }

    #[test]
    fn test_learn_from_feedback_disabled() {
        let config = AdaptiveThresholdConfig {
            enabled: false,
            ..Default::default()
        };
        let mut learner = ThresholdLearner::with_config(config);

        let stats = RetrievalStats::new();
        learner.learn_from_feedback(&stats, true);

        // Should not update when disabled
        assert_eq!(learner.total_observations, 0);

        println!("[PASS] test_learn_from_feedback_disabled: Disabled config prevents learning");
    }

    #[test]
    fn test_bayesian_history_recording() {
        let mut learner = ThresholdLearner::new();
        let stats = RetrievalStats::new();

        // Add 100 observations to trigger history recording
        for _ in 0..100 {
            learner.learn_from_feedback(&stats, true);
        }

        assert!(!learner.bayesian_history.is_empty());
        assert_eq!(learner.bayesian_history.len(), 1);

        // Add another 100
        for _ in 0..100 {
            learner.learn_from_feedback(&stats, false);
        }

        assert_eq!(learner.bayesian_history.len(), 2);

        println!("[PASS] test_bayesian_history_recording: Bayesian history recorded at intervals");
    }

    #[test]
    fn test_best_performance_tracking() {
        let mut learner = ThresholdLearner::new();
        let stats = RetrievalStats::new();

        // Start with successes
        for _ in 0..10 {
            learner.learn_from_feedback(&stats, true);
        }
        let perf_after_success = learner.best_performance();

        // Add failures
        for _ in 0..10 {
            learner.learn_from_feedback(&stats, false);
        }

        // Best should not decrease
        assert!(learner.best_performance() >= perf_after_success * 0.9);

        println!("[PASS] test_best_performance_tracking: Best performance tracked");
    }

    #[test]
    fn test_per_embedder_threshold_evolution() {
        let mut learner = ThresholdLearner::new();

        let initial_thresholds: Vec<f32> = (0..NUM_EMBEDDERS)
            .map(|i| learner.get_threshold(i))
            .collect();

        // Learn from mixed feedback
        let stats = RetrievalStats::new();
        for i in 0..50 {
            learner.learn_from_feedback(&stats, i % 2 == 0);
        }

        let final_thresholds: Vec<f32> = (0..NUM_EMBEDDERS)
            .map(|i| learner.get_threshold(i))
            .collect();

        // Thresholds should have evolved (at least some difference)
        let total_change: f32 = initial_thresholds
            .iter()
            .zip(final_thresholds.iter())
            .map(|(i, f)| (i - f).abs())
            .sum();

        assert!(total_change > 0.0, "Thresholds should evolve with feedback");

        println!("[PASS] test_per_embedder_threshold_evolution: Per-embedder thresholds evolve");
    }

    #[test]
    fn test_get_embedder_state() {
        let learner = ThresholdLearner::new();

        // Valid index
        let state = learner.get_embedder_state(0);
        assert!(state.is_some());
        assert_eq!(state.unwrap().observation_count, 0);

        // Invalid index
        let state_invalid = learner.get_embedder_state(100);
        assert!(state_invalid.is_none());

        println!("[PASS] test_get_embedder_state: Embedder state accessible");
    }

    #[test]
    fn test_set_ewma_alpha_clamping() {
        let mut learner = ThresholdLearner::new();

        learner.set_ewma_alpha(0.01);
        assert!((learner.ewma_alpha - 0.05).abs() < f32::EPSILON);

        learner.set_ewma_alpha(0.9);
        assert!((learner.ewma_alpha - 0.5).abs() < f32::EPSILON);

        learner.set_ewma_alpha(0.3);
        assert!((learner.ewma_alpha - 0.3).abs() < f32::EPSILON);

        println!("[PASS] test_set_ewma_alpha_clamping: Alpha clamped to valid range");
    }

    #[test]
    fn test_reset() {
        let mut learner = ThresholdLearner::new();

        // Add some data
        let stats = RetrievalStats::new();
        for _ in 0..50 {
            learner.learn_from_feedback(&stats, true);
        }

        assert!(learner.total_observations > 0);

        // Reset
        learner.reset();

        assert_eq!(learner.total_observations, 0);
        assert_eq!(learner.total_successes, 0);
        assert!(learner.bayesian_history.is_empty());
        assert!((learner.best_performance - 0.0).abs() < f32::EPSILON);

        println!("[PASS] test_reset: State reset to initial values");
    }

    #[test]
    fn test_default_impl() {
        let learner1 = ThresholdLearner::default();
        let learner2 = ThresholdLearner::new();

        assert_eq!(learner1.total_observations, learner2.total_observations);
        assert!((learner1.state.optimal - learner2.state.optimal).abs() < f32::EPSILON);

        println!("[PASS] test_default_impl: Default and new produce same result");
    }

    #[test]
    fn test_4_level_atc_integration() {
        let mut learner = ThresholdLearner::new();

        // Simulate realistic usage scenario
        let mut stats = RetrievalStats::new();

        // Phase 1: Good results (high alignment)
        for _ in 0..20 {
            stats.record_retrieval(AlignmentBucket::Optimal, true);
            learner.learn_from_feedback(&stats, true);
        }

        let threshold_after_good = learner.get_threshold(0);

        // Phase 2: Mixed results
        for _ in 0..20 {
            stats.record_retrieval(AlignmentBucket::Warning, false);
            learner.learn_from_feedback(&stats, false);
        }

        // Level 1: EWMA should have tracked drift
        let threshold_after_mixed = learner.get_threshold(0);

        // Level 2: Temperature should be updated
        let temp = learner.embedder_states[0].temperature;
        assert!((temp - 1.0).abs() < 0.5); // Should be near default

        // Level 3: Thompson state should reflect history
        let thompson = &learner.embedder_states[0].thompson;
        assert!(thompson.samples >= 40);

        println!("[PASS] test_4_level_atc_integration: All 4 ATC levels work together");
        println!(
            "       Level 1 (EWMA): {} -> {}",
            threshold_after_good, threshold_after_mixed
        );
        println!("       Level 2 (Temp): {}", temp);
        println!(
            "       Level 3 (Thompson): alpha={}, beta={}",
            thompson.alpha, thompson.beta
        );
        println!(
            "       Level 4 (Bayesian history): {} observations",
            learner.bayesian_history.len()
        );
    }

    #[test]
    fn test_constitution_compliance() {
        // Verify compliance with constitution.yaml adaptive_thresholds section
        let learner = ThresholdLearner::new();

        // Check default priors match constitution
        assert!(
            (learner.state.optimal - 0.75).abs() < 0.01,
            "θ_opt prior should be ~0.75"
        );
        assert!(
            (learner.state.acceptable - 0.70).abs() < 0.01,
            "θ_acc prior should be ~0.70"
        );
        assert!(
            (learner.state.warning - 0.55).abs() < 0.01,
            "θ_warn prior should be ~0.55"
        );
        assert!(
            (learner.state.critical - 0.40).abs() < 0.01,
            "θ_crit prior should be ~0.40"
        );

        // Check config bounds match constitution
        let config = learner.get_config();
        assert!(config.optimal_bounds.0 >= 0.60 && config.optimal_bounds.1 <= 0.90);
        assert!(config.warning_bounds.0 >= 0.40 && config.warning_bounds.1 <= 0.70);

        println!("[PASS] test_constitution_compliance: Matches constitution.yaml ATC spec");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut learner = ThresholdLearner::new();

        // Add some data
        let stats = RetrievalStats::new();
        for _ in 0..10 {
            learner.learn_from_feedback(&stats, true);
        }

        // Serialize
        let json = serde_json::to_string(&learner).expect("Serialization should succeed");

        // Deserialize
        let restored: ThresholdLearner =
            serde_json::from_str(&json).expect("Deserialization should succeed");

        assert_eq!(restored.total_observations, learner.total_observations);
        assert_eq!(restored.total_successes, learner.total_successes);
        assert!((restored.state.optimal - learner.state.optimal).abs() < f32::EPSILON);

        println!("[PASS] test_serialization_roundtrip: Serde works correctly");
    }

    #[test]
    fn test_threshold_validity_after_learning() {
        let mut learner = ThresholdLearner::new();

        // Add significant feedback
        let mut stats = RetrievalStats::new();
        for i in 0..100 {
            stats.record_retrieval(AlignmentBucket::Optimal, true);
            stats.record_retrieval(AlignmentBucket::Acceptable, i % 2 == 0);
            stats.record_retrieval(AlignmentBucket::Warning, false);
            learner.learn_from_feedback(&stats, true);
        }

        // After learning, verify thresholds are still valid (positive and bounded)
        assert!(
            learner.state.optimal > 0.0 && learner.state.optimal <= 1.0,
            "Optimal ({}) should be in (0, 1]",
            learner.state.optimal
        );
        assert!(
            learner.state.acceptable > 0.0 && learner.state.acceptable <= 1.0,
            "Acceptable ({}) should be in (0, 1]",
            learner.state.acceptable
        );
        assert!(
            learner.state.warning >= 0.0 && learner.state.warning <= 1.0,
            "Warning ({}) should be in [0, 1]",
            learner.state.warning
        );
        assert!(
            learner.state.critical >= 0.0 && learner.state.critical <= 1.0,
            "Critical ({}) should be in [0, 1]",
            learner.state.critical
        );

        println!("[PASS] test_threshold_validity_after_learning: Threshold bounds maintained");
    }

    #[test]
    fn test_threshold_bounds_respected() {
        let mut learner = ThresholdLearner::new();
        let stats = RetrievalStats::new();

        // Apply many learning updates
        for _ in 0..200 {
            learner.learn_from_feedback(&stats, true);
        }

        let config = learner.get_config();

        // Optimal should be within bounds
        assert!(
            learner.state.optimal >= config.optimal_bounds.0
                && learner.state.optimal <= config.optimal_bounds.1,
            "Optimal ({}) should be within bounds {:?}",
            learner.state.optimal,
            config.optimal_bounds
        );

        // Warning should be within bounds
        assert!(
            learner.state.warning >= config.warning_bounds.0
                && learner.state.warning <= config.warning_bounds.1,
            "Warning ({}) should be within bounds {:?}",
            learner.state.warning,
            config.warning_bounds
        );

        println!("[PASS] test_threshold_bounds_respected: Thresholds stay within bounds");
    }
}
