//! Surprise (delta-S) computation settings.
//!
//! Controls how surprise/entropy is computed for knowledge items.
//! Surprise measures the novelty or unexpectedness of information.

use serde::{Deserialize, Serialize};

/// Surprise (delta-S) computation settings.
///
/// Controls how surprise/entropy is computed for knowledge items.
/// Surprise measures the novelty or unexpectedness of information.
///
/// # Constitution Reference
///
/// - `delta-S` range: `[0, 1]` representing entropy/novelty
/// - Higher values indicate more surprising/novel information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurpriseConfig {
    /// Weight applied to entropy component.
    /// Range: `[0.0, 1.0]`
    pub entropy_weight: f32,

    /// Boost factor for novel items.
    /// Range: `[0.5, 2.0]`
    pub novelty_boost: f32,

    /// Decay rate for repeated exposure.
    /// Range: `[0.0, 1.0]`
    pub repetition_decay: f32,

    /// Minimum surprise threshold below which items are considered familiar.
    /// Range: `[0.0, 0.5]`
    pub min_threshold: f32,

    /// Maximum surprise value (for clamping).
    /// Range: `[0.5, 1.0]`
    pub max_value: f32,

    /// Number of samples for entropy estimation.
    pub sample_count: usize,

    /// Use exponential moving average for smoothing.
    pub use_ema: bool,

    /// EMA alpha (smoothing factor).
    /// Range: `[0.0, 1.0]`
    pub ema_alpha: f32,

    // === Per-Embedder Entropy Configuration ===
    // These fields configure specialized entropy methods per constitution.yaml delta_sc.ΔS_methods

    /// Number of nearest neighbors for KNN-based entropy methods.
    /// Used by: DefaultKnnEntropy, AsymmetricKnnEntropy
    /// Range: `[1, 100]`
    pub k_neighbors: usize,

    // --- GMM Mahalanobis (E1 Semantic) ---

    /// Number of GMM components for semantic entropy.
    /// Range: `[1, 20]`
    pub gmm_n_components: usize,

    /// Regularization term for covariance matrix inversion.
    /// Range: `[1e-8, 1e-2]`
    pub gmm_regularization: f32,

    // --- Asymmetric KNN (E5 Causal) ---

    /// Direction modifier for cause→effect relationships.
    /// Higher values increase surprise for forward causal queries.
    /// Range: `[0.5, 2.0]`
    pub causal_cause_to_effect_mod: f32,

    /// Direction modifier for effect→cause relationships.
    /// Lower values decrease surprise for backward causal queries.
    /// Range: `[0.5, 2.0]`
    pub causal_effect_to_cause_mod: f32,

    // --- Hamming Prototype (E9 HDC) ---

    /// Maximum number of prototypes for HDC entropy.
    /// Range: `[10, 1000]`
    pub hdc_max_prototypes: usize,

    /// Threshold for binarizing float embeddings in HDC.
    /// Range: `[0.0, 1.0]`
    pub hdc_binarization_threshold: f32,

    // --- Jaccard Active (E13 SPLADE) ---

    /// Activation threshold for SPLADE active dimensions.
    /// Values above this threshold are considered "active".
    /// Range: `[0.0, 0.1]`
    pub splade_activation_threshold: f32,

    /// Smoothing factor for empty union handling in Jaccard.
    /// Range: `[0.001, 0.1]`
    pub splade_smoothing: f32,

    // --- Hybrid GMM+KNN (E7 Code) ---

    /// GMM component weight for Code entropy.
    /// Constitution: 0.5 (GMM+KNN hybrid uses equal weights)
    /// Range: `[0.0, 1.0]`
    pub code_gmm_weight: f32,

    /// KNN component weight for Code entropy.
    /// Constitution: 0.5 (GMM+KNN hybrid uses equal weights)
    /// Range: `[0.0, 1.0]`
    pub code_knn_weight: f32,

    /// Number of GMM components for Code entropy.
    /// Range: `[2, 10]`
    pub code_n_components: usize,

    /// k for KNN component in Code entropy.
    /// Range: `[1, 20]`
    pub code_k_neighbors: usize,

    // --- Cross-Modal (E10 Multimodal) ---

    /// Weight for intra-modal comparisons (same modality).
    /// Constitution: 0.7 (prefer same-modality similarity)
    /// Range: `[0.0, 1.0]`
    pub multimodal_intra_weight: f32,

    /// Weight for cross-modal comparisons (different modality).
    /// Constitution: 0.3 (still consider cross-modal alignment)
    /// Range: `[0.0, 1.0]`
    pub multimodal_cross_weight: f32,

    /// k neighbors for multimodal KNN entropy.
    /// Range: `[1, 20]`
    pub multimodal_k_neighbors: usize,
}

impl Default for SurpriseConfig {
    fn default() -> Self {
        Self {
            entropy_weight: 0.6,
            novelty_boost: 1.0,
            repetition_decay: 0.1,
            min_threshold: 0.05,
            max_value: 1.0,
            sample_count: 100,
            use_ema: true,
            ema_alpha: 0.3,

            // Per-embedder entropy defaults (constitution.yaml compliant)
            k_neighbors: 5,
            gmm_n_components: 3,
            gmm_regularization: 1e-6,
            causal_cause_to_effect_mod: 1.2,  // constitution.yaml: cause_to_effect=1.2
            causal_effect_to_cause_mod: 0.8,  // constitution.yaml: effect_to_cause=0.8
            hdc_max_prototypes: 100,
            hdc_binarization_threshold: 0.5,
            splade_activation_threshold: 0.0,
            splade_smoothing: 0.01,

            // Hybrid GMM+KNN (E7 Code) - per constitution.yaml delta_methods.ΔS E7
            code_gmm_weight: 0.5,
            code_knn_weight: 0.5,
            code_n_components: 5,
            code_k_neighbors: 5,

            // Cross-Modal (E10 Multimodal) - per constitution.yaml delta_methods.ΔS E10
            multimodal_intra_weight: 0.7,
            multimodal_cross_weight: 0.3,
            multimodal_k_neighbors: 5,
        }
    }
}

impl SurpriseConfig {
    /// Validate the surprise configuration.
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.entropy_weight) {
            return Err(format!(
                "entropy_weight must be in [0, 1], got {}",
                self.entropy_weight
            ));
        }
        if !(0.5..=2.0).contains(&self.novelty_boost) {
            return Err(format!(
                "novelty_boost must be in [0.5, 2.0], got {}",
                self.novelty_boost
            ));
        }
        if !(0.0..=1.0).contains(&self.repetition_decay) {
            return Err(format!(
                "repetition_decay must be in [0, 1], got {}",
                self.repetition_decay
            ));
        }
        if !(0.0..=0.5).contains(&self.min_threshold) {
            return Err(format!(
                "min_threshold must be in [0, 0.5], got {}",
                self.min_threshold
            ));
        }
        if !(0.5..=1.0).contains(&self.max_value) {
            return Err(format!(
                "max_value must be in [0.5, 1.0], got {}",
                self.max_value
            ));
        }
        if self.sample_count == 0 {
            return Err("sample_count must be > 0".to_string());
        }
        if !(0.0..=1.0).contains(&self.ema_alpha) {
            return Err(format!(
                "ema_alpha must be in [0, 1], got {}",
                self.ema_alpha
            ));
        }
        Ok(())
    }
}
