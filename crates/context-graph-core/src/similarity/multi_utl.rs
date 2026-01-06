//! Multi-UTL formula implementation for advanced semantic learning.
//!
//! This module implements the Multi-Embedding UTL formula from constitution.yaml:
//!
//! ```text
//! L_multi = sigmoid(2.0 * (SUM_i tau_i * lambda_S * Delta_S_i) *
//!                          (SUM_j tau_j * lambda_C * Delta_C_j) *
//!                          w_e * cos(phi))
//! ```
//!
//! # Parameters
//!
//! - `Delta_S_i`: Per-space semantic entropy deltas (13D)
//! - `Delta_C_j`: Per-space coherence deltas (13D)
//! - `tau_i`: Per-space teleological weights from purpose vector (13D)
//! - `lambda_S`: Lambda for semantic term
//! - `lambda_C`: Lambda for coherence term
//! - `w_e`: Emotional weight
//! - `phi`: Phase angle (goal alignment)

use crate::types::fingerprint::NUM_EMBEDDERS;
use serde::{Deserialize, Serialize};

/// Compute sigmoid function.
///
/// `sigmoid(x) = 1 / (1 + exp(-x))`
///
/// # Arguments
/// - `x`: Input value (any f32)
///
/// # Returns
/// Value in range (0.0, 1.0)
///
/// # Example
/// ```
/// use context_graph_core::similarity::sigmoid;
///
/// assert!((sigmoid(0.0) - 0.5).abs() < 1e-6);
/// assert!(sigmoid(100.0) > 0.999);
/// assert!(sigmoid(-100.0) < 0.001);
/// ```
#[inline]
pub fn sigmoid(x: f32) -> f32 {
    // Handle overflow: for very large |x|, sigmoid approaches 0 or 1
    if x > 88.0 {
        return 1.0;
    }
    if x < -88.0 {
        return 0.0;
    }
    1.0 / (1.0 + (-x).exp())
}

/// Parameters for Multi-UTL formula (13 embedding spaces).
///
/// # Formula
///
/// ```text
/// L_multi = sigmoid(2.0 * (SUM_i tau_i * lambda_S * Delta_S_i) *
///                          (SUM_j tau_j * lambda_C * Delta_C_j) *
///                          w_e * cos(phi))
/// ```
///
/// # Usage
///
/// ```rust,ignore
/// use context_graph_core::similarity::{MultiUtlParams, sigmoid};
///
/// let params = MultiUtlParams {
///     semantic_deltas: [0.1; 13],
///     coherence_deltas: [0.2; 13],
///     tau_weights: [1.0; 13],
///     lambda_s: 1.0,
///     lambda_c: 1.0,
///     w_e: 1.0,
///     phi: 0.0,  // cos(0) = 1
/// };
///
/// let l_multi = params.compute();
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MultiUtlParams {
    /// Per-space semantic entropy deltas (Delta_S_i).
    ///
    /// Measures novelty/surprise in each embedding space.
    /// Range: [0.0, 1.0] per space.
    pub semantic_deltas: [f32; NUM_EMBEDDERS],

    /// Per-space coherence deltas (Delta_C_j).
    ///
    /// Measures integration/understanding in each embedding space.
    /// Range: [0.0, 1.0] per space.
    pub coherence_deltas: [f32; NUM_EMBEDDERS],

    /// Per-space tau weights from purpose vector.
    ///
    /// These are the teleological alignment values that weight
    /// each embedding space's contribution.
    /// Range: [-1.0, 1.0] per space (from purpose vector).
    pub tau_weights: [f32; NUM_EMBEDDERS],

    /// Lambda for semantic term.
    ///
    /// Controls the importance of the entropy (novelty) term.
    /// Default: 1.0
    pub lambda_s: f32,

    /// Lambda for coherence term.
    ///
    /// Controls the importance of the coherence (understanding) term.
    /// Default: 1.0
    pub lambda_c: f32,

    /// Emotional weight (w_e).
    ///
    /// Scales how efficiently energy transfers (attention, motivation).
    /// Range: [0.5, 1.5] per constitution.yaml.
    /// Default: 1.0
    pub w_e: f32,

    /// Phase angle phi (goal alignment).
    ///
    /// Models synchronization vs. desynchronization.
    /// Range: [0, pi].
    /// cos(0) = 1 (perfect sync), cos(pi) = -1 (anti-sync).
    /// Default: 0.0
    pub phi: f32,
}

impl Default for MultiUtlParams {
    fn default() -> Self {
        Self {
            semantic_deltas: [0.0; NUM_EMBEDDERS],
            coherence_deltas: [0.0; NUM_EMBEDDERS],
            tau_weights: [1.0 / NUM_EMBEDDERS as f32; NUM_EMBEDDERS],
            lambda_s: 1.0,
            lambda_c: 1.0,
            w_e: 1.0,
            phi: 0.0,
        }
    }
}

impl MultiUtlParams {
    /// Create params with uniform tau weights.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create params from a purpose vector's alignments.
    pub fn from_purpose_alignments(alignments: [f32; NUM_EMBEDDERS]) -> Self {
        Self {
            tau_weights: alignments,
            ..Default::default()
        }
    }

    /// Compute the Multi-UTL score.
    ///
    /// # Formula
    /// ```text
    /// L_multi = sigmoid(2.0 * semantic_sum * coherence_sum * w_e * cos(phi))
    /// ```
    /// where:
    /// - `semantic_sum = SUM_i (tau_i * lambda_S * Delta_S_i)`
    /// - `coherence_sum = SUM_j (tau_j * lambda_C * Delta_C_j)`
    ///
    /// # Returns
    /// Learning score in range (0.0, 1.0)
    pub fn compute(&self) -> f32 {
        // Compute semantic sum: SUM_i (tau_i * lambda_S * Delta_S_i)
        let semantic_sum: f32 = self
            .semantic_deltas
            .iter()
            .zip(self.tau_weights.iter())
            .map(|(delta, tau)| tau * self.lambda_s * delta)
            .sum();

        // Compute coherence sum: SUM_j (tau_j * lambda_C * Delta_C_j)
        let coherence_sum: f32 = self
            .coherence_deltas
            .iter()
            .zip(self.tau_weights.iter())
            .map(|(delta, tau)| tau * self.lambda_c * delta)
            .sum();

        // Compute raw value: 2.0 * semantic * coherence * w_e * cos(phi)
        let raw = 2.0 * semantic_sum * coherence_sum * self.w_e * self.phi.cos();

        // Apply sigmoid
        sigmoid(raw)
    }

    /// Compute with detailed breakdown for debugging.
    ///
    /// Returns (L_multi, semantic_sum, coherence_sum, raw_value).
    pub fn compute_detailed(&self) -> (f32, f32, f32, f32) {
        let semantic_sum: f32 = self
            .semantic_deltas
            .iter()
            .zip(self.tau_weights.iter())
            .map(|(delta, tau)| tau * self.lambda_s * delta)
            .sum();

        let coherence_sum: f32 = self
            .coherence_deltas
            .iter()
            .zip(self.tau_weights.iter())
            .map(|(delta, tau)| tau * self.lambda_c * delta)
            .sum();

        let raw = 2.0 * semantic_sum * coherence_sum * self.w_e * self.phi.cos();
        let l_multi = sigmoid(raw);

        (l_multi, semantic_sum, coherence_sum, raw)
    }

    /// Set semantic deltas for all spaces.
    pub fn with_semantic_deltas(mut self, deltas: [f32; NUM_EMBEDDERS]) -> Self {
        self.semantic_deltas = deltas;
        self
    }

    /// Set coherence deltas for all spaces.
    pub fn with_coherence_deltas(mut self, deltas: [f32; NUM_EMBEDDERS]) -> Self {
        self.coherence_deltas = deltas;
        self
    }

    /// Set tau weights from purpose vector.
    pub fn with_tau_weights(mut self, weights: [f32; NUM_EMBEDDERS]) -> Self {
        self.tau_weights = weights;
        self
    }

    /// Set lambda_s (semantic term weight).
    pub fn with_lambda_s(mut self, lambda: f32) -> Self {
        self.lambda_s = lambda;
        self
    }

    /// Set lambda_c (coherence term weight).
    pub fn with_lambda_c(mut self, lambda: f32) -> Self {
        self.lambda_c = lambda;
        self
    }

    /// Set emotional weight.
    pub fn with_w_e(mut self, w_e: f32) -> Self {
        self.w_e = w_e;
        self
    }

    /// Set phase angle.
    pub fn with_phi(mut self, phi: f32) -> Self {
        self.phi = phi;
        self
    }

    /// Validate parameters are in expected ranges.
    ///
    /// Returns error message if invalid, None if valid.
    pub fn validate(&self) -> Option<String> {
        // Check w_e range [0.5, 1.5]
        if self.w_e < 0.5 || self.w_e > 1.5 {
            return Some(format!(
                "w_e ({}) out of range [0.5, 1.5]",
                self.w_e
            ));
        }

        // Check phi range [0, pi]
        if self.phi < 0.0 || self.phi > std::f32::consts::PI {
            return Some(format!(
                "phi ({}) out of range [0, pi]",
                self.phi
            ));
        }

        // Check for NaN/Infinity
        for (i, &delta) in self.semantic_deltas.iter().enumerate() {
            if delta.is_nan() || delta.is_infinite() {
                return Some(format!(
                    "semantic_deltas[{}] is NaN or Infinite",
                    i
                ));
            }
        }

        for (i, &delta) in self.coherence_deltas.iter().enumerate() {
            if delta.is_nan() || delta.is_infinite() {
                return Some(format!(
                    "coherence_deltas[{}] is NaN or Infinite",
                    i
                ));
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sigmoid_bounds() {
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-6);
        assert!(sigmoid(100.0) > 0.999);
        assert!(sigmoid(-100.0) < 0.001);
        assert!(sigmoid(5.0) > 0.99);
        assert!(sigmoid(-5.0) < 0.01);

        println!(
            "[PASS] Sigmoid bounds: sigmoid(0)={:.4}, sigmoid(100)={:.6}, sigmoid(-100)={:.6}",
            sigmoid(0.0),
            sigmoid(100.0),
            sigmoid(-100.0)
        );
    }

    #[test]
    fn test_sigmoid_overflow_handling() {
        // These should not overflow or produce NaN
        let large = sigmoid(1000.0);
        let small = sigmoid(-1000.0);

        assert!(!large.is_nan());
        assert!(!small.is_nan());
        assert!((large - 1.0).abs() < 1e-6);
        assert!(small.abs() < 1e-6);

        println!("[PASS] Sigmoid handles overflow: large={}, small={}", large, small);
    }

    #[test]
    fn test_multi_utl_default() {
        let params = MultiUtlParams::default();
        let score = params.compute();

        // With all zeros, semantic_sum * coherence_sum = 0, so sigmoid(0) = 0.5
        assert!(
            (score - 0.5).abs() < 1e-6,
            "Default params should give 0.5, got {}",
            score
        );

        println!("[PASS] Default MultiUtlParams gives score={}", score);
    }

    #[test]
    fn test_multi_utl_high_learning() {
        let params = MultiUtlParams {
            semantic_deltas: [0.1; NUM_EMBEDDERS],
            coherence_deltas: [0.1; NUM_EMBEDDERS],
            tau_weights: [1.0; NUM_EMBEDDERS],
            lambda_s: 1.0,
            lambda_c: 1.0,
            w_e: 1.0,
            phi: 0.0, // cos(0) = 1
        };

        let score = params.compute();

        // semantic_sum = 13 * 1.0 * 1.0 * 0.1 = 1.3
        // coherence_sum = 13 * 1.0 * 1.0 * 0.1 = 1.3
        // raw = 2.0 * 1.3 * 1.3 * 1.0 * 1.0 = 3.38
        // sigmoid(3.38) ≈ 0.967

        let expected_raw = 2.0 * 1.3 * 1.3 * 1.0 * 1.0;
        let expected = sigmoid(expected_raw);

        assert!(
            (score - expected).abs() < 1e-4,
            "Expected {}, got {}",
            expected,
            score
        );

        println!(
            "[PASS] High learning scenario: score={:.4} (expected {:.4})",
            score, expected
        );
    }

    #[test]
    fn test_multi_utl_phase_impact() {
        let base = MultiUtlParams {
            semantic_deltas: [0.5; NUM_EMBEDDERS],
            coherence_deltas: [0.5; NUM_EMBEDDERS],
            tau_weights: [1.0; NUM_EMBEDDERS],
            lambda_s: 1.0,
            lambda_c: 1.0,
            w_e: 1.0,
            phi: 0.0,
        };

        let score_phi_0 = base.clone().with_phi(0.0).compute(); // cos(0) = 1
        let score_phi_pi2 = base.clone().with_phi(std::f32::consts::FRAC_PI_2).compute(); // cos(pi/2) = 0
        let score_phi_pi = base.clone().with_phi(std::f32::consts::PI).compute(); // cos(pi) = -1

        // phi=0 should give highest score (positive)
        // phi=pi/2 should give ~0.5 (zero contribution)
        // phi=pi should give lowest score (negative contribution)

        assert!(score_phi_0 > score_phi_pi2);
        assert!(score_phi_pi2 > score_phi_pi);
        assert!((score_phi_pi2 - 0.5).abs() < 1e-4);

        println!(
            "[PASS] Phase impact: phi=0 -> {:.4}, phi=pi/2 -> {:.4}, phi=pi -> {:.4}",
            score_phi_0, score_phi_pi2, score_phi_pi
        );
    }

    #[test]
    fn test_compute_detailed() {
        let params = MultiUtlParams {
            semantic_deltas: [0.1; NUM_EMBEDDERS],
            coherence_deltas: [0.2; NUM_EMBEDDERS],
            tau_weights: [1.0; NUM_EMBEDDERS],
            lambda_s: 1.0,
            lambda_c: 1.0,
            w_e: 1.0,
            phi: 0.0,
        };

        let (l_multi, sem_sum, coh_sum, raw) = params.compute_detailed();

        // Verify sums
        let expected_sem = 13.0 * 1.0 * 1.0 * 0.1; // 1.3
        let expected_coh = 13.0 * 1.0 * 1.0 * 0.2; // 2.6

        assert!((sem_sum - expected_sem).abs() < 1e-4);
        assert!((coh_sum - expected_coh).abs() < 1e-4);

        println!(
            "[PASS] Detailed: L={:.4}, sem_sum={:.4}, coh_sum={:.4}, raw={:.4}",
            l_multi, sem_sum, coh_sum, raw
        );
    }

    #[test]
    fn test_validation() {
        let valid = MultiUtlParams::default();
        assert!(valid.validate().is_none());

        let invalid_w_e = MultiUtlParams::default().with_w_e(2.0);
        assert!(invalid_w_e.validate().is_some());

        let invalid_phi = MultiUtlParams::default().with_phi(-1.0);
        assert!(invalid_phi.validate().is_some());

        let mut invalid_nan = MultiUtlParams::default();
        invalid_nan.semantic_deltas[0] = f32::NAN;
        assert!(invalid_nan.validate().is_some());

        println!("[PASS] Validation catches invalid parameters");
    }

    #[test]
    fn test_builder_pattern() {
        let params = MultiUtlParams::new()
            .with_semantic_deltas([0.5; NUM_EMBEDDERS])
            .with_coherence_deltas([0.3; NUM_EMBEDDERS])
            .with_lambda_s(0.8)
            .with_lambda_c(1.2)
            .with_w_e(1.0)
            .with_phi(0.1);

        assert_eq!(params.semantic_deltas[0], 0.5);
        assert_eq!(params.coherence_deltas[0], 0.3);
        assert!((params.lambda_s - 0.8).abs() < 1e-6);
        assert!((params.lambda_c - 1.2).abs() < 1e-6);
        assert!((params.phi - 0.1).abs() < 1e-6);

        println!("[PASS] Builder pattern creates correct params");
    }

    #[test]
    fn test_from_purpose_alignments() {
        let alignments = [0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.0, -0.1, -0.2, -0.3];
        let params = MultiUtlParams::from_purpose_alignments(alignments);

        for i in 0..NUM_EMBEDDERS {
            assert!((params.tau_weights[i] - alignments[i]).abs() < 1e-6);
        }

        println!("[PASS] from_purpose_alignments sets tau_weights correctly");
    }
}
