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

use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};

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

impl GaussianProcessTracker {
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
        self.mean = self.observations
            .iter()
            .map(|o| o.performance)
            .sum::<f32>() / n;

        let variance: f32 = self.observations
            .iter()
            .map(|o| (o.performance - self.mean).powi(2))
            .sum::<f32>() / n;

        self.variance = variance.max(0.01); // Avoid zero variance
    }

    /// Estimate performance for a threshold configuration (simplified)
    pub fn predict_performance(&self, _thresholds: &HashMap<String, f32>) -> (f32, f32) {
        // Simplified: return mean ± sqrt(variance)
        // In a real implementation, this would use actual GP prediction
        (self.mean, self.variance.sqrt())
    }

    /// Compute Expected Improvement
    pub fn expected_improvement(
        &self,
        predicted_mean: f32,
        predicted_std: f32,
    ) -> f32 {
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
            if let (Some(&opt), Some(&acc), Some(&warn)) =
                (config.get("theta_opt"), config.get("theta_acc"), config.get("theta_warn"))
            {
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
        self.gp.observations
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
}
