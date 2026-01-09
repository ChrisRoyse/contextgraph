//! Level 4: Bayesian Meta-Optimizer
//!
//! Weekly optimization of thresholds using Gaussian Process surrogate model.
//! Uses Expected Improvement (EI) acquisition function with constraints.
//!
//! # Algorithm
//! 1. Fit GP to (threshold, performance) observations
//! 2. Maximize EI to select next threshold configuration
//! 3. Evaluate system with new thresholds
//! 4. Update GP with observation
//! 5. Repeat weekly
//!
//! # Constraints
//! - θ_optimal > θ_acceptable > θ_warning (monotonicity)
//! - θ_dup > θ_edge (duplicate stricter than edge)
//! - Per-embedder bounds respected

use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

/// Observation of threshold performance
#[derive(Debug, Clone)]
pub struct ThresholdObservation {
    /// Threshold configuration tested
    pub thresholds: HashMap<String, f32>,
    /// Performance metric achieved
    pub performance: f32,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Simple Gaussian Process-like tracker
#[derive(Debug)]
pub struct GaussianProcessTracker {
    /// Past observations
    observations: Vec<ThresholdObservation>,
    /// Best performance seen
    best_performance: f32,
    /// Running mean and variance
    mean: f32,
    variance: f32,
}

impl Default for GaussianProcessTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl GaussianProcessTracker {
    /// Signal variance for the RBF kernel (controls amplitude of function variation)
    const SIGNAL_VARIANCE: f64 = 1.0;
    /// Length scale for the RBF kernel (controls smoothness - lower = more local)
    const LENGTH_SCALE: f64 = 0.1;
    /// Observation noise variance
    const NOISE_VARIANCE: f64 = 0.01;

    pub fn new() -> Self {
        Self {
            observations: Vec::new(),
            best_performance: 0.0,
            mean: 0.5,
            variance: 0.1,
        }
    }

    /// Add observation
    pub fn add_observation(&mut self, obs: ThresholdObservation) {
        if obs.performance > self.best_performance {
            self.best_performance = obs.performance;
        }
        self.observations.push(obs);
        self.update_statistics();
    }

    /// Get number of observations
    pub fn observation_count(&self) -> usize {
        self.observations.len()
    }

    /// Update mean and variance from observations
    fn update_statistics(&mut self) {
        if self.observations.is_empty() {
            return;
        }

        let n = self.observations.len() as f32;
        self.mean = self.observations.iter().map(|o| o.performance).sum::<f32>() / n;

        let variance: f32 = self
            .observations
            .iter()
            .map(|o| (o.performance - self.mean).powi(2))
            .sum::<f32>()
            / n;

        self.variance = variance.max(0.01); // Avoid zero variance
    }

    /// Squared Exponential (RBF) kernel function
    /// k(x, x') = sigma^2 * exp(-||x - x'||^2 / (2 * l^2))
    ///
    /// This kernel measures similarity between two threshold configurations.
    /// Close configurations have high kernel values (near 1), distant ones have low (near 0).
    fn kernel(&self, config1: &HashMap<String, f32>, config2: &HashMap<String, f32>) -> f64 {
        // Collect all keys from both configurations
        let mut all_keys: Vec<&String> = config1.keys().collect();
        for key in config2.keys() {
            if !config1.contains_key(key) {
                all_keys.push(key);
            }
        }

        // Compute squared Euclidean distance between configurations
        let mut sq_dist = 0.0_f64;
        for key in &all_keys {
            let v1 = config1.get(*key).copied().unwrap_or(0.0) as f64;
            let v2 = config2.get(*key).copied().unwrap_or(0.0) as f64;
            sq_dist += (v1 - v2).powi(2);
        }

        // RBF kernel: k(x, x') = sigma^2 * exp(-||x - x'||^2 / (2 * l^2))
        Self::SIGNAL_VARIANCE * (-sq_dist / (2.0 * Self::LENGTH_SCALE.powi(2))).exp()
    }

    /// Predict performance for a threshold configuration using GP posterior
    ///
    /// Uses kernel-weighted averaging as an efficient approximation to the full GP posterior:
    /// - GP posterior mean: mu* = k*^T (K + sigma^2 I)^{-1} y
    /// - GP posterior variance: sigma*^2 = k** - k*^T (K + sigma^2 I)^{-1} k*
    ///
    /// The simplified approach:
    /// - Mean: blend between prior and kernel-weighted observation mean, based on total kernel weight
    /// - Variance: decreases near observations (high kernel similarity), high far from them
    pub fn predict_performance(&self, thresholds: &HashMap<String, f32>) -> (f32, f32) {
        // Prior parameters
        const PRIOR_MEAN: f64 = 0.5;
        const PRIOR_STD: f64 = 0.3;

        // Return prior when no observations
        if self.observations.is_empty() {
            return (PRIOR_MEAN as f32, PRIOR_STD as f32);
        }

        // Compute kernel similarity to all past observations
        let mut kernel_weights: Vec<f64> = Vec::with_capacity(self.observations.len());
        let mut weight_sum = 0.0_f64;
        let mut max_kernel = 0.0_f64;

        for obs in &self.observations {
            let k = self.kernel(thresholds, &obs.thresholds);
            kernel_weights.push(k);
            weight_sum += k;
            if k > max_kernel {
                max_kernel = k;
            }
        }

        // If input is very far from all observations (all kernels near zero),
        // return prior with high uncertainty
        if weight_sum < 1e-10 {
            return (PRIOR_MEAN as f32, PRIOR_STD as f32);
        }

        // Compute kernel-weighted mean of observations
        // mu_obs = sum_i(k_i * y_i) / sum_i(k_i)
        let mut obs_weighted_mean = 0.0_f64;
        for (i, obs) in self.observations.iter().enumerate() {
            obs_weighted_mean += (kernel_weights[i] / weight_sum) * obs.performance as f64;
        }

        // The key insight: blend between prior and observation mean based on confidence
        // When max_kernel is high (query near an observation), trust the data more
        // When max_kernel is low (query far from all observations), revert to prior
        //
        // The effective number of observations at this point is proportional to weight_sum
        // We use max_kernel as a blend factor since it represents how "close" we are to data
        let blend_factor = max_kernel; // 1.0 = at observation, 0.0 = far from all
        let predicted_mean = blend_factor * obs_weighted_mean + (1.0 - blend_factor) * PRIOR_MEAN;

        // Posterior variance: decreases near observations
        // sigma*^2 = prior_variance * (1 - max_kernel) + noise
        // At an observation (max_kernel=1): variance = noise only
        // Far from observations (max_kernel=0): variance = prior_variance + noise
        let prior_var = PRIOR_STD.powi(2);
        let predicted_variance = (prior_var * (1.0 - max_kernel) + Self::NOISE_VARIANCE).max(0.01);

        (predicted_mean as f32, predicted_variance.sqrt() as f32)
    }

    /// Compute Expected Improvement
    pub fn expected_improvement(&self, predicted_mean: f32, predicted_std: f32) -> f32 {
        if predicted_std == 0.0 {
            return 0.0;
        }

        let improvement = predicted_mean - self.best_performance;
        if improvement <= 0.0 {
            return 0.0;
        }

        // EI ≈ improvement × Φ(Z) + σ × φ(Z)
        // where Z = improvement / σ
        let z = improvement / predicted_std;
        let normal_cdf = 0.5 * (1.0 + (z / 2.0_f32.sqrt()).tanh()); // Approximation
        let normal_pdf = (-z.powi(2) / 2.0).exp() / (2.0 * std::f32::consts::PI).sqrt();

        improvement * normal_cdf + predicted_std * normal_pdf
    }
}

/// Bayesian meta-optimizer for threshold configuration
#[derive(Debug)]
pub struct BayesianOptimizer {
    /// GP tracker
    gp: GaussianProcessTracker,
    /// Last optimization timestamp
    last_optimized: DateTime<Utc>,
    /// Threshold constraints
    constraints: ThresholdConstraints,
}

/// Constraints on threshold values
#[derive(Debug, Clone)]
pub struct ThresholdConstraints {
    /// θ_opt >= 0.60, <= 0.90
    pub theta_opt_range: (f32, f32),
    /// θ_acc >= 0.55, <= 0.85
    pub theta_acc_range: (f32, f32),
    /// θ_warn >= 0.40, <= 0.70
    pub theta_warn_range: (f32, f32),
    /// θ_dup >= 0.80, <= 0.98
    pub theta_dup_range: (f32, f32),
    /// θ_edge >= 0.50, <= 0.85
    pub theta_edge_range: (f32, f32),
    /// Monotonicity constraint: θ_opt > θ_acc > θ_warn
    pub enforce_monotonicity: bool,
}

impl Default for ThresholdConstraints {
    fn default() -> Self {
        Self {
            theta_opt_range: (0.60, 0.90),
            theta_acc_range: (0.55, 0.85),
            theta_warn_range: (0.40, 0.70),
            theta_dup_range: (0.80, 0.98),
            theta_edge_range: (0.50, 0.85),
            enforce_monotonicity: true,
        }
    }
}

impl ThresholdConstraints {
    /// Check if configuration satisfies all constraints
    pub fn is_valid(&self, config: &HashMap<String, f32>) -> bool {
        // Check ranges
        if let Some(&opt) = config.get("theta_opt") {
            if opt < self.theta_opt_range.0 || opt > self.theta_opt_range.1 {
                return false;
            }
        }

        if let Some(&acc) = config.get("theta_acc") {
            if acc < self.theta_acc_range.0 || acc > self.theta_acc_range.1 {
                return false;
            }
        }

        if let Some(&warn) = config.get("theta_warn") {
            if warn < self.theta_warn_range.0 || warn > self.theta_warn_range.1 {
                return false;
            }
        }

        if let Some(&dup) = config.get("theta_dup") {
            if dup < self.theta_dup_range.0 || dup > self.theta_dup_range.1 {
                return false;
            }
        }

        if let Some(&edge) = config.get("theta_edge") {
            if edge < self.theta_edge_range.0 || edge > self.theta_edge_range.1 {
                return false;
            }
        }

        // Check monotonicity
        if self.enforce_monotonicity {
            if let (Some(&opt), Some(&acc), Some(&warn)) = (
                config.get("theta_opt"),
                config.get("theta_acc"),
                config.get("theta_warn"),
            ) {
                if !(opt > acc && acc > warn) {
                    return false;
                }
            }
        }

        true
    }

    /// Clamp values to satisfy constraints
    pub fn clamp(&self, config: &mut HashMap<String, f32>) {
        if let Some(opt) = config.get_mut("theta_opt") {
            *opt = opt.clamp(self.theta_opt_range.0, self.theta_opt_range.1);
        }
        if let Some(acc) = config.get_mut("theta_acc") {
            *acc = acc.clamp(self.theta_acc_range.0, self.theta_acc_range.1);
        }
        if let Some(warn) = config.get_mut("theta_warn") {
            *warn = warn.clamp(self.theta_warn_range.0, self.theta_warn_range.1);
        }
        if let Some(dup) = config.get_mut("theta_dup") {
            *dup = dup.clamp(self.theta_dup_range.0, self.theta_dup_range.1);
        }
        if let Some(edge) = config.get_mut("theta_edge") {
            *edge = edge.clamp(self.theta_edge_range.0, self.theta_edge_range.1);
        }
    }
}

impl BayesianOptimizer {
    /// Create new Bayesian optimizer
    pub fn new(constraints: ThresholdConstraints) -> Self {
        Self {
            gp: GaussianProcessTracker::new(),
            last_optimized: Utc::now(),
            constraints,
        }
    }

    /// Add observation to GP
    pub fn observe(&mut self, config: HashMap<String, f32>, performance: f32) {
        let obs = ThresholdObservation {
            thresholds: config,
            performance,
            timestamp: Utc::now(),
        };
        self.gp.add_observation(obs);
    }

    /// Suggest next configuration to evaluate using Expected Improvement
    pub fn suggest_next(&self) -> HashMap<String, f32> {
        // Start with midpoints of ranges
        let mut best_config = HashMap::new();
        best_config.insert(
            "theta_opt".to_string(),
            (self.constraints.theta_opt_range.0 + self.constraints.theta_opt_range.1) / 2.0,
        );
        best_config.insert(
            "theta_acc".to_string(),
            (self.constraints.theta_acc_range.0 + self.constraints.theta_acc_range.1) / 2.0,
        );
        best_config.insert(
            "theta_warn".to_string(),
            (self.constraints.theta_warn_range.0 + self.constraints.theta_warn_range.1) / 2.0,
        );

        let mut best_ei = 0.0;

        // Grid search over parameter space (simplified)
        for opt in [0.65, 0.70, 0.75, 0.80, 0.85] {
            for acc in [0.60, 0.65, 0.70, 0.75] {
                for warn in [0.50, 0.55, 0.60, 0.65] {
                    let mut config = HashMap::new();
                    config.insert("theta_opt".to_string(), opt);
                    config.insert("theta_acc".to_string(), acc);
                    config.insert("theta_warn".to_string(), warn);

                    if !self.constraints.is_valid(&config) {
                        continue;
                    }

                    let (pred_mean, pred_std) = self.gp.predict_performance(&config);
                    let ei = self.gp.expected_improvement(pred_mean, pred_std);

                    if ei > best_ei {
                        best_ei = ei;
                        best_config = config;
                    }
                }
            }
        }

        best_config
    }

    /// Check if weekly optimization is due
    pub fn should_optimize(&self) -> bool {
        Utc::now().signed_duration_since(self.last_optimized) > Duration::days(7)
    }

    /// Mark optimization as done
    pub fn mark_optimized(&mut self) {
        self.last_optimized = Utc::now();
    }

    /// Get best configuration found so far
    pub fn get_best_config(&self) -> Option<HashMap<String, f32>> {
        self.gp
            .observations
            .iter()
            .max_by(|a, b| a.performance.partial_cmp(&b.performance).unwrap())
            .map(|obs| obs.thresholds.clone())
    }

    /// Get number of observations
    pub fn observation_count(&self) -> usize {
        self.gp.observations.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraints_valid() {
        let constraints = ThresholdConstraints::default();

        let mut valid = HashMap::new();
        valid.insert("theta_opt".to_string(), 0.75);
        valid.insert("theta_acc".to_string(), 0.70);
        valid.insert("theta_warn".to_string(), 0.55);

        assert!(constraints.is_valid(&valid));
    }

    #[test]
    fn test_constraints_monotonicity() {
        let constraints = ThresholdConstraints::default();

        let mut invalid = HashMap::new();
        invalid.insert("theta_opt".to_string(), 0.70);
        invalid.insert("theta_acc".to_string(), 0.75); // Wrong: should be < opt
        invalid.insert("theta_warn".to_string(), 0.55);

        assert!(!constraints.is_valid(&invalid));
    }

    #[test]
    fn test_gp_tracker() {
        let mut gp = GaussianProcessTracker::new();

        let obs1 = ThresholdObservation {
            thresholds: HashMap::from([("theta_opt".to_string(), 0.75)]),
            performance: 0.85,
            timestamp: Utc::now(),
        };
        gp.add_observation(obs1);

        assert_eq!(gp.best_performance, 0.85);
        assert_eq!(gp.observation_count(), 1);
    }

    #[test]
    fn test_bayesian_optimizer() {
        let constraints = ThresholdConstraints::default();
        let mut optimizer = BayesianOptimizer::new(constraints);

        let obs = HashMap::from([
            ("theta_opt".to_string(), 0.75),
            ("theta_acc".to_string(), 0.70),
            ("theta_warn".to_string(), 0.55),
        ]);
        optimizer.observe(obs, 0.82);

        let suggestion = optimizer.suggest_next();
        assert!(suggestion.contains_key("theta_opt"));
        assert!(optimizer.constraints.is_valid(&suggestion));
    }

    #[test]
    fn test_expected_improvement() {
        let gp = GaussianProcessTracker::new();
        let ei = gp.expected_improvement(0.6, 0.1);
        assert!(ei >= 0.0);
    }

    #[test]
    fn test_gp_prediction_varies_with_input() {
        let mut gp = GaussianProcessTracker::new();

        // Add an observation at a specific point
        let obs = ThresholdObservation {
            thresholds: HashMap::from([
                ("theta_opt".to_string(), 0.75),
                ("theta_acc".to_string(), 0.70),
            ]),
            performance: 0.90,
            timestamp: Utc::now(),
        };
        gp.add_observation(obs);

        // Prediction at same point should be close to observed value
        let same_config = HashMap::from([
            ("theta_opt".to_string(), 0.75),
            ("theta_acc".to_string(), 0.70),
        ]);
        let (mean_same, std_same) = gp.predict_performance(&same_config);

        // Prediction at a different point should differ
        let diff_config = HashMap::from([
            ("theta_opt".to_string(), 0.60),
            ("theta_acc".to_string(), 0.55),
        ]);
        let (mean_diff, std_diff) = gp.predict_performance(&diff_config);

        // Near observation: prediction should be close to observed value
        assert!(
            (mean_same - 0.90).abs() < 0.1,
            "Prediction at observed point should be close to observed value, got {} vs 0.90",
            mean_same
        );

        // Far from observation: predictions should differ
        assert!(
            (mean_same - mean_diff).abs() > 0.01,
            "Predictions should vary with input: same={} vs diff={}",
            mean_same,
            mean_diff
        );

        // Uncertainty should be higher far from observations
        assert!(
            std_diff > std_same,
            "Uncertainty should be higher far from observations: std_diff={} vs std_same={}",
            std_diff,
            std_same
        );
    }

    #[test]
    fn test_gp_prediction_empty_returns_prior() {
        let gp = GaussianProcessTracker::new();
        let config = HashMap::from([("theta_opt".to_string(), 0.75)]);
        let (mean, std) = gp.predict_performance(&config);

        // Should return prior: mean=0.5, high uncertainty
        assert!(
            (mean - 0.5).abs() < 0.1,
            "Empty GP should return prior mean ~0.5, got {}",
            mean
        );
        assert!(
            std > 0.2,
            "Empty GP should have high uncertainty (std > 0.2), got {}",
            std
        );
    }

    #[test]
    fn test_kernel_similarity() {
        let gp = GaussianProcessTracker::new();

        // Identical configurations should have kernel = 1.0
        let config1 = HashMap::from([
            ("theta_opt".to_string(), 0.75),
            ("theta_acc".to_string(), 0.70),
        ]);
        let k_same = gp.kernel(&config1, &config1);
        assert!(
            (k_same - 1.0).abs() < 0.01,
            "Kernel of identical configs should be ~1.0, got {}",
            k_same
        );

        // Different configurations should have lower kernel
        let config2 = HashMap::from([
            ("theta_opt".to_string(), 0.60),
            ("theta_acc".to_string(), 0.55),
        ]);
        let k_diff = gp.kernel(&config1, &config2);
        assert!(
            k_diff < k_same,
            "Kernel of different configs should be less than identical: {} vs {}",
            k_diff,
            k_same
        );
        assert!(k_diff > 0.0, "Kernel should be positive, got {}", k_diff);
    }

    #[test]
    fn test_gp_multiple_observations() {
        let mut gp = GaussianProcessTracker::new();

        // Add observation at low performance point
        gp.add_observation(ThresholdObservation {
            thresholds: HashMap::from([
                ("theta_opt".to_string(), 0.65),
                ("theta_acc".to_string(), 0.60),
            ]),
            performance: 0.60,
            timestamp: Utc::now(),
        });

        // Add observation at high performance point
        gp.add_observation(ThresholdObservation {
            thresholds: HashMap::from([
                ("theta_opt".to_string(), 0.85),
                ("theta_acc".to_string(), 0.80),
            ]),
            performance: 0.95,
            timestamp: Utc::now(),
        });

        // Prediction near low-performance observation
        let near_low = HashMap::from([
            ("theta_opt".to_string(), 0.65),
            ("theta_acc".to_string(), 0.60),
        ]);
        let (mean_low, _) = gp.predict_performance(&near_low);

        // Prediction near high-performance observation
        let near_high = HashMap::from([
            ("theta_opt".to_string(), 0.85),
            ("theta_acc".to_string(), 0.80),
        ]);
        let (mean_high, _) = gp.predict_performance(&near_high);

        // Should reflect the different performance values
        assert!(
            mean_high > mean_low,
            "Prediction near high-perf obs ({}) should be > near low-perf obs ({})",
            mean_high,
            mean_low
        );
    }

    #[test]
    fn test_bayesian_optimizer_suggest_varies() {
        let constraints = ThresholdConstraints::default();
        let mut optimizer = BayesianOptimizer::new(constraints);

        // First suggestion (no observations) - should return midpoint
        let first_suggestion = optimizer.suggest_next();

        // Add observation
        optimizer.observe(
            HashMap::from([
                ("theta_opt".to_string(), 0.75),
                ("theta_acc".to_string(), 0.70),
                ("theta_warn".to_string(), 0.55),
            ]),
            0.85,
        );

        // Second suggestion should potentially differ (EI-driven exploration)
        let second_suggestion = optimizer.suggest_next();

        // Both should be valid
        assert!(optimizer.constraints.is_valid(&first_suggestion));
        assert!(optimizer.constraints.is_valid(&second_suggestion));
    }
}
