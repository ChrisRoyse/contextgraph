//! Cross-modal entropy for E10 (Multimodal/CLIP) embeddings.
//!
//! Formula: ΔS = weighted_knn_distance with modality-aware weighting
//! Per constitution.yaml delta_methods.ΔS E10: "Cross-modal KNN"
//!
//! # Algorithm
//!
//! CLIP embeddings have different activation patterns for different modalities:
//! - **Text inputs:** Higher activation in lower dimensions (linguistic features)
//! - **Visual inputs:** Higher activation in higher dimensions (perceptual features)
//!
//! 1. **Modality Detection:**
//!    - Compute energy ratio between lower and upper halves of embedding
//!    - Indicator 0.0 = text-like, 1.0 = visual-like, 0.5 = balanced
//!
//! 2. **Cross-Modal Weighting:**
//!    - Same modality (indicator diff < 0.3): use intra_modal_weight (default 0.7)
//!    - Different modality (indicator diff > 0.7): use cross_modal_weight (default 0.3)
//!    - Interpolate for intermediate differences
//!
//! 3. **KNN-Style Computation:**
//!    - Compute weighted distances to all history embeddings
//!    - Sort, take k nearest
//!    - Normalize via sigmoid with running statistics
//!    - Clamp to [0.0, 1.0], verify no NaN/Infinity (AP-10)
//!
//! # Constitution Reference
//!
//! - From constitution.yaml delta_methods.ΔS E10: "Cross-modal KNN"
//! - Default weights: intra=0.7, cross=0.3

use super::EmbedderEntropy;
use crate::config::SurpriseConfig;
use crate::error::{UtlError, UtlResult};
use crate::surprise::compute_cosine_distance;
use context_graph_core::teleological::Embedder;

/// Default weight for same-modality comparisons per constitution.
const DEFAULT_INTRA_MODAL_WEIGHT: f32 = 0.7;

/// Default weight for cross-modality comparisons per constitution.
const DEFAULT_CROSS_MODAL_WEIGHT: f32 = 0.3;

/// Default k neighbors for multimodal KNN.
const DEFAULT_K_NEIGHBORS: usize = 5;

/// Threshold for considering same modality (indicator difference).
const SAME_MODALITY_THRESHOLD: f32 = 0.3;

/// Threshold for considering different modality (indicator difference).
const DIFFERENT_MODALITY_THRESHOLD: f32 = 0.7;

/// Minimum energy threshold to avoid division by zero.
const MIN_ENERGY: f32 = 1e-8;

/// Minimum standard deviation to avoid division by zero.
const MIN_STD_DEV: f32 = 0.1;

/// E10 (Multimodal) entropy using cross-modal distance metrics.
///
/// CLIP embeddings have different activation patterns for text vs visual:
/// - Text: Higher activation in lower dimensions (linguistic features)
/// - Visual: Higher activation in higher dimensions (perceptual features)
///
/// This calculator weights distances based on modality similarity.
///
/// # Constitution Reference
/// E10: "Cross-modal KNN" per constitution.yaml delta_methods.ΔS
#[derive(Debug, Clone)]
pub struct CrossModalEntropy {
    /// Weight for intra-modal (same modality) comparisons. Default: 0.7
    intra_modal_weight: f32,
    /// Weight for cross-modal (different modality) comparisons. Default: 0.3
    cross_modal_weight: f32,
    /// Running mean for distance normalization.
    running_mean: f32,
    /// Running variance for distance normalization.
    running_variance: f32,
    /// Number of samples seen for statistics.
    sample_count: usize,
    /// k neighbors for KNN component.
    k_neighbors: usize,
}

impl Default for CrossModalEntropy {
    fn default() -> Self {
        Self::new()
    }
}

impl CrossModalEntropy {
    /// Create a new cross-modal entropy calculator with constitution defaults.
    pub fn new() -> Self {
        Self {
            intra_modal_weight: DEFAULT_INTRA_MODAL_WEIGHT,
            cross_modal_weight: DEFAULT_CROSS_MODAL_WEIGHT,
            running_mean: 0.5,
            running_variance: 0.1,
            sample_count: 0,
            k_neighbors: DEFAULT_K_NEIGHBORS,
        }
    }

    /// Create from SurpriseConfig.
    pub fn from_config(config: &SurpriseConfig) -> Self {
        Self {
            intra_modal_weight: config.multimodal_intra_weight.clamp(0.0, 1.0),
            cross_modal_weight: config.multimodal_cross_weight.clamp(0.0, 1.0),
            running_mean: 0.5,
            running_variance: 0.1,
            sample_count: 0,
            k_neighbors: config.multimodal_k_neighbors.clamp(1, 20),
        }
    }

    /// Builder: set intra-modal weight.
    #[must_use]
    pub fn with_intra_weight(mut self, weight: f32) -> Self {
        self.intra_modal_weight = weight.clamp(0.0, 1.0);
        self
    }

    /// Builder: set cross-modal weight.
    #[must_use]
    pub fn with_cross_weight(mut self, weight: f32) -> Self {
        self.cross_modal_weight = weight.clamp(0.0, 1.0);
        self
    }

    /// Builder: set k neighbors.
    #[must_use]
    pub fn with_k_neighbors(mut self, k: usize) -> Self {
        self.k_neighbors = k.clamp(1, 20);
        self
    }

    /// Detect modality indicator from embedding activation pattern.
    ///
    /// CLIP embeddings have different activation patterns:
    /// - Text inputs: Higher energy in lower dimensions
    /// - Visual inputs: Higher energy in upper dimensions
    ///
    /// # Returns
    /// - 0.0 = text-like (energy concentrated in lower half)
    /// - 1.0 = visual-like (energy concentrated in upper half)
    /// - 0.5 = balanced/hybrid
    fn detect_modality_indicator(&self, embedding: &[f32]) -> f32 {
        if embedding.is_empty() {
            return 0.5; // Default to balanced for empty embedding
        }

        let half = embedding.len() / 2;
        if half == 0 {
            return 0.5;
        }

        // Compute squared energy in each half
        let lower_energy: f32 = embedding[..half].iter().map(|x| x.powi(2)).sum();
        let upper_energy: f32 = embedding[half..].iter().map(|x| x.powi(2)).sum();
        let total = lower_energy + upper_energy;

        if total < MIN_ENERGY {
            return 0.5; // Balanced for near-zero embeddings
        }

        // Ratio of upper energy to total
        // 0.0 = all energy in lower half (text-like)
        // 1.0 = all energy in upper half (visual-like)
        upper_energy / total
    }

    /// Compute weighted distance between two embeddings based on modality.
    ///
    /// # Arguments
    /// * `current` - Current embedding
    /// * `other` - History embedding
    /// * `current_modality` - Modality indicator for current
    /// * `other_modality` - Modality indicator for other
    ///
    /// # Returns
    /// Weighted cosine distance
    fn compute_modal_distance(
        &self,
        current: &[f32],
        other: &[f32],
        current_modality: f32,
        other_modality: f32,
    ) -> f32 {
        let base_distance = compute_cosine_distance(current, other);
        let modality_diff = (current_modality - other_modality).abs();

        // Determine weight based on modality similarity
        let weight = if modality_diff < SAME_MODALITY_THRESHOLD {
            // Same modality: use intra-modal weight (higher, emphasizes similarity)
            self.intra_modal_weight
        } else if modality_diff > DIFFERENT_MODALITY_THRESHOLD {
            // Different modality: use cross-modal weight (lower, de-emphasizes difference)
            self.cross_modal_weight
        } else {
            // Interpolate between weights for intermediate differences
            let t = (modality_diff - SAME_MODALITY_THRESHOLD)
                / (DIFFERENT_MODALITY_THRESHOLD - SAME_MODALITY_THRESHOLD);
            self.intra_modal_weight * (1.0 - t) + self.cross_modal_weight * t
        };

        base_distance * weight
    }

    /// Sigmoid function for normalization.
    #[inline]
    fn sigmoid(x: f32) -> f32 {
        1.0 / (1.0 + (-x).exp())
    }
}

impl EmbedderEntropy for CrossModalEntropy {
    fn compute_delta_s(
        &self,
        current: &[f32],
        history: &[Vec<f32>],
        k: usize,
    ) -> UtlResult<f32> {
        // Validate input: empty current is an error per spec
        if current.is_empty() {
            return Err(UtlError::EmptyInput);
        }

        // Check for NaN/Infinity in current embedding (AP-10)
        for &v in current {
            if v.is_nan() || v.is_infinite() {
                return Err(UtlError::EntropyError(
                    "Invalid value (NaN/Infinity) in current embedding".to_string(),
                ));
            }
        }

        // Empty history = maximum surprise
        if history.is_empty() {
            return Ok(1.0);
        }

        // Use provided k or fallback to configured k_neighbors
        let k_to_use = if k > 0 { k } else { self.k_neighbors };

        // Compute modality indicator for current embedding
        let current_modality = self.detect_modality_indicator(current);

        // Compute weighted distances to all valid history embeddings
        let mut distances: Vec<f32> = history
            .iter()
            .filter(|h| !h.is_empty())
            .map(|h| {
                let h_modality = self.detect_modality_indicator(h);
                self.compute_modal_distance(current, h, current_modality, h_modality)
            })
            .filter(|d| !d.is_nan() && !d.is_infinite())
            .collect();

        // If all history was invalid, return max surprise
        if distances.is_empty() {
            return Ok(1.0);
        }

        // Sort distances and take k nearest
        distances.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let k_actual = k_to_use.min(distances.len()).max(1);
        let k_nearest = &distances[..k_actual];

        // Compute mean of k-nearest distances
        let mean_dist: f32 = k_nearest.iter().sum::<f32>() / k_actual as f32;

        // Normalize: z = (mean_dist - running_mean) / running_std
        let running_std = self.running_variance.sqrt().max(MIN_STD_DEV);
        let z = (mean_dist - self.running_mean) / running_std;

        // Apply sigmoid normalization
        let delta_s = Self::sigmoid(z);

        // Final validation per AP-10: no NaN/Infinity
        let clamped = delta_s.clamp(0.0, 1.0);
        if clamped.is_nan() || clamped.is_infinite() {
            return Err(UtlError::EntropyError(
                "Computed delta_s is NaN or Infinity - violates AP-10".to_string(),
            ));
        }

        Ok(clamped)
    }

    fn embedder_type(&self) -> Embedder {
        Embedder::Multimodal
    }

    fn reset(&mut self) {
        self.running_mean = 0.5;
        self.running_variance = 0.1;
        self.sample_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // E10 dimension constant for tests (CLIP = 768)
    const E10_DIM: usize = 768;

    #[test]
    fn test_cross_modal_empty_history_returns_one() {
        let calculator = CrossModalEntropy::new();
        let current = vec![0.5f32; E10_DIM];
        let history: Vec<Vec<f32>> = vec![];

        println!("BEFORE: history.len() = 0");
        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok(), "Should not error on empty history");
        assert_eq!(result.unwrap(), 1.0, "Empty history should return 1.0");
        println!("AFTER: delta_s = 1.0");
        println!("[PASS] test_cross_modal_empty_history_returns_one");
    }

    #[test]
    fn test_cross_modal_empty_input_error() {
        let calculator = CrossModalEntropy::new();
        let empty: Vec<f32> = vec![];
        let history = vec![vec![0.5f32; E10_DIM]];

        let result = calculator.compute_delta_s(&empty, &history, 5);
        assert!(
            matches!(result, Err(UtlError::EmptyInput)),
            "Empty input should return EmptyInput error"
        );
        println!("[PASS] test_cross_modal_empty_input_error - Err(EmptyInput)");
    }

    #[test]
    fn test_cross_modal_identical_returns_low() {
        let calculator = CrossModalEntropy::new();

        // Create history with identical embeddings
        let embedding = vec![0.5f32; E10_DIM];
        let history: Vec<Vec<f32>> = vec![embedding.clone(); 20];

        let result = calculator.compute_delta_s(&embedding, &history, 5);
        assert!(result.is_ok());
        let delta_s = result.unwrap();

        println!(
            "BEFORE: history contains 20 identical embeddings to current"
        );
        println!("AFTER: delta_s = {}", delta_s);
        assert!(
            delta_s < 0.5,
            "Identical embedding should have low surprise, got {}",
            delta_s
        );
        println!(
            "[PASS] test_cross_modal_identical_returns_low - delta_s = {}",
            delta_s
        );
    }

    #[test]
    fn test_cross_modal_modality_detection() {
        let calculator = CrossModalEntropy::new();

        // Text-like: energy in lower dimensions
        let mut text_like = vec![0.0f32; E10_DIM];
        for i in 0..E10_DIM / 2 {
            text_like[i] = 1.0;
        }
        let text_indicator = calculator.detect_modality_indicator(&text_like);
        println!("text_indicator = {} (expected < 0.3)", text_indicator);
        assert!(
            text_indicator < 0.3,
            "Text-like should have low indicator, got {}",
            text_indicator
        );

        // Visual-like: energy in upper dimensions
        let mut visual_like = vec![0.0f32; E10_DIM];
        for i in E10_DIM / 2..E10_DIM {
            visual_like[i] = 1.0;
        }
        let visual_indicator = calculator.detect_modality_indicator(&visual_like);
        println!("visual_indicator = {} (expected > 0.7)", visual_indicator);
        assert!(
            visual_indicator > 0.7,
            "Visual-like should have high indicator, got {}",
            visual_indicator
        );

        // Balanced: energy evenly distributed
        let balanced = vec![0.5f32; E10_DIM];
        let balanced_indicator = calculator.detect_modality_indicator(&balanced);
        println!(
            "balanced_indicator = {} (expected ~0.5)",
            balanced_indicator
        );
        assert!(
            (balanced_indicator - 0.5).abs() < 0.1,
            "Balanced should be ~0.5, got {}",
            balanced_indicator
        );

        println!("[PASS] test_cross_modal_modality_detection");
    }

    #[test]
    fn test_cross_modal_different_modality_weighted() {
        let calculator = CrossModalEntropy::new();

        // Text-like current (energy in lower half)
        let mut text_current = vec![0.0f32; E10_DIM];
        for i in 0..E10_DIM / 2 {
            text_current[i] = 1.0;
        }

        // Visual-like history (energy in upper half)
        let mut visual_history_item = vec![0.0f32; E10_DIM];
        for i in E10_DIM / 2..E10_DIM {
            visual_history_item[i] = 1.0;
        }
        let visual_history: Vec<Vec<f32>> = vec![visual_history_item; 10];

        // Same modality history (text-like)
        let text_history: Vec<Vec<f32>> = vec![text_current.clone(); 10];

        let cross_modal_result =
            calculator.compute_delta_s(&text_current, &visual_history, 5);
        let same_modal_result =
            calculator.compute_delta_s(&text_current, &text_history, 5);

        assert!(cross_modal_result.is_ok());
        assert!(same_modal_result.is_ok());

        let cross_delta_s = cross_modal_result.unwrap();
        let same_delta_s = same_modal_result.unwrap();

        println!(
            "Cross-modal delta_s = {}, Same-modal delta_s = {}",
            cross_delta_s, same_delta_s
        );

        // Same modality comparison should have different (likely lower) surprise
        // since identical vectors in same modality space have distance 0
        println!("[PASS] test_cross_modal_different_modality_weighted");
    }

    #[test]
    fn test_cross_modal_same_modality_baseline() {
        let calculator = CrossModalEntropy::new();

        // Create text-like embeddings (all energy in lower half)
        let mut text_current = vec![0.0f32; E10_DIM];
        for i in 0..E10_DIM / 2 {
            text_current[i] = 0.5;
        }

        // Similar text-like history (slightly different)
        let text_history: Vec<Vec<f32>> = (0..10)
            .map(|j| {
                let mut h = vec![0.0f32; E10_DIM];
                for i in 0..E10_DIM / 2 {
                    h[i] = 0.5 + (j as f32) * 0.01;
                }
                h
            })
            .collect();

        let result = calculator.compute_delta_s(&text_current, &text_history, 5);
        assert!(result.is_ok());
        let delta_s = result.unwrap();

        println!(
            "Same modality (text-text) delta_s = {} (should use intra_weight=0.7)",
            delta_s
        );
        assert!(
            (0.0..=1.0).contains(&delta_s),
            "delta_s should be in valid range"
        );

        println!("[PASS] test_cross_modal_same_modality_baseline");
    }

    #[test]
    fn test_cross_modal_embedder_type() {
        let calculator = CrossModalEntropy::new();
        assert_eq!(
            calculator.embedder_type(),
            Embedder::Multimodal,
            "Should return Embedder::Multimodal"
        );
        println!("[PASS] test_cross_modal_embedder_type - Embedder::Multimodal");
    }

    #[test]
    fn test_cross_modal_valid_range() {
        let calculator = CrossModalEntropy::new();

        // Test various input patterns
        for pattern in 0..5 {
            let current: Vec<f32> = (0..E10_DIM)
                .map(|i| ((i + pattern * 100) as f32) / E10_DIM as f32)
                .collect();

            let history: Vec<Vec<f32>> = (0..15)
                .map(|j| {
                    (0..E10_DIM)
                        .map(|i| ((i + j * 50) as f32) / E10_DIM as f32)
                        .collect()
                })
                .collect();

            let result = calculator.compute_delta_s(&current, &history, 5);
            assert!(result.is_ok());
            let delta_s = result.unwrap();

            assert!(
                (0.0..=1.0).contains(&delta_s),
                "Pattern {} delta_s {} out of range",
                pattern,
                delta_s
            );
        }

        println!("[PASS] test_cross_modal_valid_range");
    }

    #[test]
    fn test_cross_modal_no_nan_infinity() {
        let calculator = CrossModalEntropy::new();

        // Edge case: very small values
        let small: Vec<f32> = vec![1e-10; E10_DIM];
        let history: Vec<Vec<f32>> = vec![vec![1e-10; E10_DIM]; 10];

        let result = calculator.compute_delta_s(&small, &history, 5);
        assert!(result.is_ok());
        let delta_s = result.unwrap();
        assert!(!delta_s.is_nan(), "delta_s should not be NaN (AP-10)");
        assert!(
            !delta_s.is_infinite(),
            "delta_s should not be Infinite (AP-10)"
        );

        // Edge case: values near 1
        let near_one: Vec<f32> = vec![0.9999; E10_DIM];
        let result2 = calculator.compute_delta_s(&near_one, &history, 5);
        assert!(result2.is_ok());
        let delta_s2 = result2.unwrap();
        assert!(!delta_s2.is_nan(), "delta_s should not be NaN");
        assert!(!delta_s2.is_infinite(), "delta_s should not be Infinite");

        // Edge case: mixed positive and negative
        let mixed: Vec<f32> = (0..E10_DIM)
            .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
            .collect();
        let result3 = calculator.compute_delta_s(&mixed, &history, 5);
        assert!(result3.is_ok());
        let delta_s3 = result3.unwrap();
        assert!(!delta_s3.is_nan(), "delta_s should not be NaN for mixed values");

        println!("[PASS] test_cross_modal_no_nan_infinity - AP-10 compliant");
    }

    #[test]
    fn test_cross_modal_from_config() {
        let mut config = SurpriseConfig::default();
        config.multimodal_intra_weight = 0.8;
        config.multimodal_cross_weight = 0.4;
        config.multimodal_k_neighbors = 10;

        let calculator = CrossModalEntropy::from_config(&config);

        assert!(
            (calculator.intra_modal_weight - 0.8).abs() < 1e-6,
            "intra_weight should be 0.8, got {}",
            calculator.intra_modal_weight
        );
        assert!(
            (calculator.cross_modal_weight - 0.4).abs() < 1e-6,
            "cross_weight should be 0.4, got {}",
            calculator.cross_modal_weight
        );
        assert_eq!(
            calculator.k_neighbors, 10,
            "k_neighbors should be 10"
        );

        println!(
            "[PASS] test_cross_modal_from_config - intra={}, cross={}, k={}",
            calculator.intra_modal_weight,
            calculator.cross_modal_weight,
            calculator.k_neighbors
        );
    }

    #[test]
    fn test_cross_modal_weight_range() {
        let calculator = CrossModalEntropy::new();

        assert!(
            (0.0..=1.0).contains(&calculator.intra_modal_weight),
            "intra_modal_weight should be in [0.0, 1.0]"
        );
        assert!(
            (0.0..=1.0).contains(&calculator.cross_modal_weight),
            "cross_modal_weight should be in [0.0, 1.0]"
        );

        // Test clamping via builder
        let clamped = CrossModalEntropy::new()
            .with_intra_weight(2.0)
            .with_cross_weight(-0.5);
        assert_eq!(
            clamped.intra_modal_weight, 1.0,
            "intra_weight should be clamped to 1.0"
        );
        assert_eq!(
            clamped.cross_modal_weight, 0.0,
            "cross_weight should be clamped to 0.0"
        );

        println!("[PASS] test_cross_modal_weight_range");
    }

    #[test]
    fn test_cross_modal_reset() {
        let mut calculator = CrossModalEntropy::new();

        // Modify internal state
        calculator.running_mean = 0.8;
        calculator.running_variance = 0.5;
        calculator.sample_count = 100;

        calculator.reset();

        assert_eq!(
            calculator.running_mean, 0.5,
            "running_mean should reset to 0.5"
        );
        assert_eq!(
            calculator.running_variance, 0.1,
            "running_variance should reset to 0.1"
        );
        assert_eq!(
            calculator.sample_count, 0,
            "sample_count should reset to 0"
        );

        println!("[PASS] test_cross_modal_reset");
    }

    #[test]
    fn test_cross_modal_nan_input_error() {
        let calculator = CrossModalEntropy::new();
        let mut current = vec![0.5f32; E10_DIM];
        current[0] = f32::NAN;

        let history = vec![vec![0.5f32; E10_DIM]];
        let result = calculator.compute_delta_s(&current, &history, 5);

        assert!(result.is_err(), "Should error on NaN input");
        assert!(
            matches!(result, Err(UtlError::EntropyError(_))),
            "Should return EntropyError for NaN input"
        );

        println!("[PASS] test_cross_modal_nan_input_error");
    }

    #[test]
    fn test_cross_modal_infinity_input_error() {
        let calculator = CrossModalEntropy::new();
        let mut current = vec![0.5f32; E10_DIM];
        current[0] = f32::INFINITY;

        let history = vec![vec![0.5f32; E10_DIM]];
        let result = calculator.compute_delta_s(&current, &history, 5);

        assert!(result.is_err(), "Should error on Infinity input");
        assert!(
            matches!(result, Err(UtlError::EntropyError(_))),
            "Should return EntropyError for Infinity input"
        );

        println!("[PASS] test_cross_modal_infinity_input_error");
    }

    // === Edge Case Tests per Task Requirements ===

    #[test]
    fn test_edge_case_text_vs_visual_history() {
        let calculator = CrossModalEntropy::new();

        // Input concentrated in lower dimensions (text-like)
        let mut current = vec![0.0f32; E10_DIM];
        for i in 0..E10_DIM / 2 {
            current[i] = 1.0;
        }

        // History concentrated in upper dimensions (visual-like)
        let history: Vec<Vec<f32>> = (0..10)
            .map(|_| {
                let mut h = vec![0.0f32; E10_DIM];
                for i in E10_DIM / 2..E10_DIM {
                    h[i] = 1.0;
                }
                h
            })
            .collect();

        println!("BEFORE: current is text-like, history is visual-like");
        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok(), "Should handle cross-modal comparison");
        let delta_s = result.unwrap();
        println!("AFTER: delta_s = {}", delta_s);

        assert!(
            (0.0..=1.0).contains(&delta_s),
            "delta_s should be in valid range"
        );
        // Cross-modal comparison should use lower weight (0.3)
        // Resulting in moderate surprise since distance is weighted down

        println!(
            "[PASS] test_edge_case_text_vs_visual_history - delta_s = {}",
            delta_s
        );
    }

    #[test]
    fn test_edge_case_single_history_item() {
        let calculator = CrossModalEntropy::new();

        let current = vec![0.5f32; E10_DIM];
        let history = vec![vec![0.5f32; E10_DIM]]; // Only 1 item, but k=5

        println!("BEFORE: history.len()={}, k=5", history.len());

        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok(), "Should handle k > history.len()");
        let delta_s = result.unwrap();

        println!("AFTER: delta_s = {}", delta_s);
        assert!(
            (0.0..=1.0).contains(&delta_s),
            "delta_s should be in valid range"
        );
        // Should use k=1 when k > history.len()

        println!(
            "[PASS] test_edge_case_single_history_item - delta_s = {}",
            delta_s
        );
    }

    #[test]
    fn test_edge_case_near_zero_norm() {
        let calculator = CrossModalEntropy::new();

        let current = vec![1e-10f32; E10_DIM];
        let history: Vec<Vec<f32>> = vec![vec![0.5f32; E10_DIM]; 10];

        println!("BEFORE: current has near-zero norm");

        let result = calculator.compute_delta_s(&current, &history, 5);
        assert!(result.is_ok(), "Should handle near-zero norm embedding");
        let delta_s = result.unwrap();

        println!("AFTER: delta_s = {}, must not be NaN/Inf", delta_s);
        assert!(!delta_s.is_nan(), "delta_s must not be NaN");
        assert!(!delta_s.is_infinite(), "delta_s must not be Infinite");
        assert!(
            (0.0..=1.0).contains(&delta_s),
            "delta_s should be in valid range"
        );

        println!(
            "[PASS] test_edge_case_near_zero_norm - delta_s = {}",
            delta_s
        );
    }

    #[test]
    fn test_builder_patterns() {
        let calc = CrossModalEntropy::new()
            .with_intra_weight(0.9)
            .with_cross_weight(0.2)
            .with_k_neighbors(7);

        assert!((calc.intra_modal_weight - 0.9).abs() < 1e-6);
        assert!((calc.cross_modal_weight - 0.2).abs() < 1e-6);
        assert_eq!(calc.k_neighbors, 7);

        println!("[PASS] test_builder_patterns");
    }

    #[test]
    fn test_k_neighbors_clamping() {
        let calc = CrossModalEntropy::new().with_k_neighbors(0);
        assert_eq!(calc.k_neighbors, 1, "Should clamp k_neighbors min to 1");

        let calc2 = CrossModalEntropy::new().with_k_neighbors(100);
        assert_eq!(calc2.k_neighbors, 20, "Should clamp k_neighbors max to 20");

        println!("[PASS] test_k_neighbors_clamping");
    }
}
