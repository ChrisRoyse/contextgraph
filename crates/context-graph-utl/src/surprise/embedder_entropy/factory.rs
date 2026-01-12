//! Factory for creating per-embedder entropy calculators.
//!
//! Routes each Embedder variant to its specialized ΔS calculation method
//! as specified in constitution.yaml delta_sc.ΔS_methods.
//!
//! # Constitution Reference
//!
//! From constitution.yaml delta_sc.ΔS_methods (lines 792-802):
//! - E1: "GMM+Mahalanobis: ΔS=√(e-μ)ᵀΣ⁻¹(e-μ)"
//! - E2-4,E8: "KNN: ΔS=σ((d_k-μ)/σ)"
//! - E5: "Asymmetric KNN: ΔS=d_k×direction_mod"
//! - E6,E13: "IDF/Jaccard: ΔS=IDF(dims) or 1-jaccard"
//! - E7: "GMM+KNN hybrid: ΔS=0.5×ΔS_GMM + 0.5×ΔS_KNN"
//! - E10-12: "KNN: ΔS=σ((d_k-μ)/σ)"
//! - E9: "Hamming: ΔS=min_hamming/dim"

use super::{
    AsymmetricKnnEntropy, CrossModalEntropy, DefaultKnnEntropy, EmbedderEntropy,
    GmmMahalanobisEntropy, HammingPrototypeEntropy, HybridGmmKnnEntropy, JaccardActiveEntropy,
};
use crate::config::SurpriseConfig;
use context_graph_core::teleological::Embedder;

/// Factory for creating per-embedder entropy calculators.
///
/// This factory routes each of the 13 embedder types to its specialized
/// entropy calculation method based on constitution.yaml specifications.
pub struct EmbedderEntropyFactory;

impl EmbedderEntropyFactory {
    /// Create an entropy calculator for a specific embedder type.
    ///
    /// # Arguments
    /// * `embedder` - The embedder type to create a calculator for
    /// * `config` - Surprise configuration
    ///
    /// # Returns
    /// A boxed trait object implementing EmbedderEntropy
    ///
    /// # Routing
    /// - E1 (Semantic) → GmmMahalanobisEntropy
    /// - E5 (Causal) → AsymmetricKnnEntropy
    /// - E7 (Code) → HybridGmmKnnEntropy (GMM+KNN hybrid per constitution)
    /// - E9 (Hdc) → HammingPrototypeEntropy
    /// - E10 (Multimodal) → CrossModalEntropy (Cross-modal KNN per constitution)
    /// - E13 (KeywordSplade) → JaccardActiveEntropy
    /// - All others → DefaultKnnEntropy
    pub fn create(embedder: Embedder, config: &SurpriseConfig) -> Box<dyn EmbedderEntropy> {
        match embedder {
            // E1: GMM + Mahalanobis distance
            Embedder::Semantic => Box::new(GmmMahalanobisEntropy::from_config(config)),

            // E5: Asymmetric KNN with direction modifiers
            Embedder::Causal => Box::new(
                AsymmetricKnnEntropy::new(config.k_neighbors)
                    .with_direction_modifiers(
                        config.causal_cause_to_effect_mod,
                        config.causal_effect_to_cause_mod,
                    ),
            ),

            // E9: Hamming distance to prototypes
            Embedder::Hdc => Box::new(
                HammingPrototypeEntropy::new(config.hdc_max_prototypes)
                    .with_threshold(config.hdc_binarization_threshold),
            ),

            // E13: Jaccard similarity of active dimensions
            Embedder::KeywordSplade => Box::new(
                JaccardActiveEntropy::new()
                    .with_threshold(config.splade_activation_threshold)
                    .with_smoothing(config.splade_smoothing),
            ),

            // E7 (Code): Hybrid GMM+KNN per constitution.yaml delta_methods.ΔS E7
            Embedder::Code => Box::new(HybridGmmKnnEntropy::from_config(config)),

            // E10 (Multimodal): Cross-modal KNN per constitution.yaml delta_methods.ΔS E10
            Embedder::Multimodal => Box::new(CrossModalEntropy::from_config(config)),

            // E2-E4, E6, E8, E11-E12: Default KNN-based entropy
            Embedder::TemporalRecent
            | Embedder::TemporalPeriodic
            | Embedder::TemporalPositional
            | Embedder::Sparse
            | Embedder::Graph
            | Embedder::Entity
            | Embedder::LateInteraction => {
                Box::new(DefaultKnnEntropy::from_config(embedder, config))
            }
        }
    }

    /// Create entropy calculators for all 13 embedder types.
    ///
    /// # Arguments
    /// * `config` - Surprise configuration
    ///
    /// # Returns
    /// An array of 13 boxed entropy calculators, indexed by embedder ordinal
    pub fn create_all(config: &SurpriseConfig) -> [Box<dyn EmbedderEntropy>; 13] {
        [
            Self::create(Embedder::Semantic, config),
            Self::create(Embedder::TemporalRecent, config),
            Self::create(Embedder::TemporalPeriodic, config),
            Self::create(Embedder::TemporalPositional, config),
            Self::create(Embedder::Causal, config),
            Self::create(Embedder::Sparse, config),
            Self::create(Embedder::Code, config),
            Self::create(Embedder::Graph, config),
            Self::create(Embedder::Hdc, config),
            Self::create(Embedder::Multimodal, config),
            Self::create(Embedder::Entity, config),
            Self::create(Embedder::LateInteraction, config),
            Self::create(Embedder::KeywordSplade, config),
        ]
    }

    /// Get a calculator by embedder index (0-12).
    ///
    /// # Arguments
    /// * `index` - Embedder ordinal (0-12)
    /// * `config` - Surprise configuration
    ///
    /// # Returns
    /// Some(calculator) if index is valid, None otherwise
    pub fn create_by_index(index: usize, config: &SurpriseConfig) -> Option<Box<dyn EmbedderEntropy>> {
        Embedder::from_index(index).map(|embedder| Self::create(embedder, config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_creates_correct_types() {
        let config = SurpriseConfig::default();

        // Test each specialized type
        let semantic = EmbedderEntropyFactory::create(Embedder::Semantic, &config);
        assert_eq!(semantic.embedder_type(), Embedder::Semantic);

        let causal = EmbedderEntropyFactory::create(Embedder::Causal, &config);
        assert_eq!(causal.embedder_type(), Embedder::Causal);

        let hdc = EmbedderEntropyFactory::create(Embedder::Hdc, &config);
        assert_eq!(hdc.embedder_type(), Embedder::Hdc);

        let splade = EmbedderEntropyFactory::create(Embedder::KeywordSplade, &config);
        assert_eq!(splade.embedder_type(), Embedder::KeywordSplade);

        println!("[PASS] factory_creates_correct_types");
    }

    #[test]
    fn test_factory_creates_fallback_types() {
        let config = SurpriseConfig::default();

        // Note: E7 (Code) and E10 (Multimodal) have specialized implementations
        // but should still return the correct embedder_type
        let fallback_embedders = [
            Embedder::TemporalRecent,
            Embedder::TemporalPeriodic,
            Embedder::TemporalPositional,
            Embedder::Sparse,
            Embedder::Code,       // Uses HybridGmmKnnEntropy
            Embedder::Graph,
            Embedder::Multimodal, // Uses CrossModalEntropy
            Embedder::Entity,
            Embedder::LateInteraction,
        ];

        for embedder in fallback_embedders {
            let calculator = EmbedderEntropyFactory::create(embedder, &config);
            assert_eq!(
                calculator.embedder_type(),
                embedder,
                "Factory should create calculator for {:?}",
                embedder
            );
        }

        println!("[PASS] factory_creates_fallback_types");
    }

    #[test]
    fn test_factory_routes_multimodal_to_cross_modal() {
        let config = SurpriseConfig::default();
        let calculator = EmbedderEntropyFactory::create(Embedder::Multimodal, &config);

        assert_eq!(
            calculator.embedder_type(),
            Embedder::Multimodal,
            "Factory should create CrossModalEntropy for Multimodal"
        );

        // Test that it computes correctly
        let current = vec![0.5f32; 768]; // E10 dimension is 768 (CLIP)
        let history: Vec<Vec<f32>> = vec![vec![0.5f32; 768]; 10];
        let result = calculator.compute_delta_s(&current, &history, 5);

        assert!(result.is_ok(), "CrossModalEntropy should compute successfully");
        let delta_s = result.unwrap();
        assert!(
            (0.0..=1.0).contains(&delta_s),
            "delta_s should be in valid range"
        );
        assert!(!delta_s.is_nan(), "delta_s should not be NaN");
        assert!(!delta_s.is_infinite(), "delta_s should not be Infinite");

        println!("[PASS] test_factory_routes_multimodal_to_cross_modal");
    }

    #[test]
    fn test_factory_create_all() {
        let config = SurpriseConfig::default();
        let calculators = EmbedderEntropyFactory::create_all(&config);

        assert_eq!(calculators.len(), 13);

        // Verify each calculator matches its index
        let expected_embedders = [
            Embedder::Semantic,
            Embedder::TemporalRecent,
            Embedder::TemporalPeriodic,
            Embedder::TemporalPositional,
            Embedder::Causal,
            Embedder::Sparse,
            Embedder::Code,
            Embedder::Graph,
            Embedder::Hdc,
            Embedder::Multimodal,
            Embedder::Entity,
            Embedder::LateInteraction,
            Embedder::KeywordSplade,
        ];

        for (i, calculator) in calculators.iter().enumerate() {
            assert_eq!(
                calculator.embedder_type(),
                expected_embedders[i],
                "Calculator at index {} should be {:?}",
                i,
                expected_embedders[i]
            );
        }

        println!("[PASS] factory_create_all");
    }

    #[test]
    fn test_factory_create_by_index() {
        let config = SurpriseConfig::default();

        // Valid indices
        for i in 0..13 {
            let calc = EmbedderEntropyFactory::create_by_index(i, &config);
            assert!(calc.is_some(), "Should create calculator for index {}", i);
        }

        // Invalid indices
        assert!(EmbedderEntropyFactory::create_by_index(13, &config).is_none());
        assert!(EmbedderEntropyFactory::create_by_index(100, &config).is_none());

        println!("[PASS] factory_create_by_index");
    }

    #[test]
    fn test_factory_calculators_compute_valid_results() {
        let config = SurpriseConfig::default();
        let calculators = EmbedderEntropyFactory::create_all(&config);

        let current = vec![0.5f32; 100];
        let history: Vec<Vec<f32>> = vec![vec![0.6f32; 100]; 10];

        for (i, calculator) in calculators.iter().enumerate() {
            let result = calculator.compute_delta_s(&current, &history, 5);
            assert!(
                result.is_ok(),
                "Calculator {} ({:?}) should compute successfully: {:?}",
                i,
                calculator.embedder_type(),
                result.err()
            );

            let delta_s = result.unwrap();
            assert!(
                (0.0..=1.0).contains(&delta_s),
                "Calculator {} ({:?}) delta_s {} out of range",
                i,
                calculator.embedder_type(),
                delta_s
            );
            assert!(
                !delta_s.is_nan(),
                "Calculator {} ({:?}) returned NaN",
                i,
                calculator.embedder_type()
            );
            assert!(
                !delta_s.is_infinite(),
                "Calculator {} ({:?}) returned Infinity",
                i,
                calculator.embedder_type()
            );
        }

        println!("[PASS] factory_calculators_compute_valid_results");
    }

    #[test]
    fn test_factory_config_propagation() {
        let mut config = SurpriseConfig::default();
        config.k_neighbors = 10;
        config.ema_alpha = 0.2;
        config.causal_cause_to_effect_mod = 1.5;
        config.causal_effect_to_cause_mod = 0.6;
        config.hdc_max_prototypes = 50;
        config.splade_activation_threshold = 0.02;

        // Create calculators - config should be applied
        let calculators = EmbedderEntropyFactory::create_all(&config);

        // Verify all 13 calculators exist and work
        assert_eq!(calculators.len(), 13);

        for calculator in &calculators {
            let current = vec![0.5f32; 50];
            let history: Vec<Vec<f32>> = vec![vec![0.6f32; 50]; 5];

            let result = calculator.compute_delta_s(&current, &history, 5);
            assert!(result.is_ok());
        }

        println!("[PASS] factory_config_propagation");
    }

    #[test]
    fn test_factory_empty_history_all_return_one() {
        let config = SurpriseConfig::default();
        let calculators = EmbedderEntropyFactory::create_all(&config);

        let current = vec![0.5f32; 100];
        let history: Vec<Vec<f32>> = vec![];

        for calculator in &calculators {
            let result = calculator.compute_delta_s(&current, &history, 5);
            assert!(result.is_ok());
            let delta_s = result.unwrap();
            assert_eq!(
                delta_s, 1.0,
                "{:?} should return 1.0 for empty history",
                calculator.embedder_type()
            );
        }

        println!("[PASS] factory_empty_history_all_return_one");
    }

    #[test]
    fn test_factory_send_sync() {
        let config = SurpriseConfig::default();
        let calculators = EmbedderEntropyFactory::create_all(&config);

        // Verify Send + Sync by moving to another thread
        std::thread::spawn(move || {
            for calculator in &calculators {
                let current = vec![0.5f32; 50];
                let history: Vec<Vec<f32>> = vec![vec![0.6f32; 50]];
                let _ = calculator.compute_delta_s(&current, &history, 3);
            }
        })
        .join()
        .expect("Thread should complete");

        println!("[PASS] factory_send_sync");
    }
}
