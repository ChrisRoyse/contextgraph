//! Per-embedder entropy computation.
//!
//! Each embedder type has a specialized ΔS calculation per constitution.yaml delta_sc.ΔS_methods:
//! - E1 (Semantic): GMM + Mahalanobis distance
//! - E2-E4, E8 (Temporal, Graph): Normalized KNN
//! - E5 (Causal): Asymmetric KNN with direction modifiers
//! - E6 (Sparse): IDF/Jaccard
//! - E7 (Code): GMM+KNN hybrid (uses DefaultKnn as fallback)
//! - E9 (HDC): Hamming distance to learned prototypes
//! - E10-E12: Specialized methods (uses DefaultKnn as fallback)
//! - E13 (SPLADE): 1 - Jaccard(active_dims)
//!
//! # Architecture Reference
//!
//! From constitution.yaml ARCH-02: "Compare Only Compatible Embedding Types"
//! From constitution.yaml delta_sc.ΔS_methods (lines 792-802)
//!
//! # Example
//!
//! ```ignore
//! use context_graph_utl::surprise::embedder_entropy::{EmbedderEntropy, EmbedderEntropyFactory};
//! use context_graph_utl::config::SurpriseConfig;
//! use context_graph_core::teleological::Embedder;
//!
//! let config = SurpriseConfig::default();
//! let calculator = EmbedderEntropyFactory::create(Embedder::Semantic, &config);
//!
//! let current = vec![0.5; 1024];
//! let history = vec![vec![0.5; 1024]; 10];
//!
//! let delta_s = calculator.compute_delta_s(&current, &history, 5).unwrap();
//! assert!(delta_s >= 0.0 && delta_s <= 1.0);
//! ```

mod asymmetric_knn;
mod cross_modal;
mod default_knn;
mod factory;
mod gmm_mahalanobis;
mod hamming_prototype;
mod hybrid_gmm_knn;
mod jaccard_active;
mod maxsim_token;
mod transe;

pub use asymmetric_knn::AsymmetricKnnEntropy;
pub use cross_modal::CrossModalEntropy;
pub use default_knn::DefaultKnnEntropy;
pub use factory::EmbedderEntropyFactory;
pub use gmm_mahalanobis::GmmMahalanobisEntropy;
pub use hamming_prototype::HammingPrototypeEntropy;
pub use hybrid_gmm_knn::HybridGmmKnnEntropy;
pub use jaccard_active::JaccardActiveEntropy;
pub use maxsim_token::MaxSimTokenEntropy;
pub use transe::TransEEntropy;

use crate::error::UtlResult;
use context_graph_core::teleological::Embedder;

/// Per-embedder entropy computation trait.
///
/// Implementors compute ΔS (surprise/entropy) using methods semantically appropriate
/// for each embedding space. Per AP-10, all outputs MUST be clamped to [0.0, 1.0]
/// with no NaN or Infinity values.
///
/// # Thread Safety
///
/// All implementations MUST be Send + Sync for use in async contexts.
pub trait EmbedderEntropy: Send + Sync {
    /// Compute ΔS for this embedder type.
    ///
    /// # Arguments
    /// * `current` - The current embedding vector
    /// * `history` - Recent embedding vectors (most recent first)
    /// * `k` - Number of neighbors for KNN-based methods
    ///
    /// # Returns
    /// ΔS value in [0.0, 1.0], or error if computation fails.
    /// Returns 1.0 for empty history (maximum surprise).
    ///
    /// # Errors
    /// - `UtlError::EmptyInput` if current embedding is empty
    /// - `UtlError::EntropyError` for computation failures
    fn compute_delta_s(
        &self,
        current: &[f32],
        history: &[Vec<f32>],
        k: usize,
    ) -> UtlResult<f32>;

    /// Get the embedder type this calculator handles.
    fn embedder_type(&self) -> Embedder;

    /// Reset any internal state (e.g., running statistics, learned prototypes).
    ///
    /// Called when the system needs to clear learned patterns.
    fn reset(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SurpriseConfig;

    #[test]
    fn test_factory_creates_correct_type_for_each_embedder() {
        let config = SurpriseConfig::default();

        for embedder in Embedder::all() {
            let calculator = EmbedderEntropyFactory::create(embedder, &config);
            assert_eq!(
                calculator.embedder_type(),
                embedder,
                "Factory created wrong type for {:?}",
                embedder
            );
        }
        println!("[PASS] Factory creates correct type for each of 13 embedders");
    }

    #[test]
    fn test_factory_create_all_returns_13_calculators() {
        let config = SurpriseConfig::default();
        let calculators = EmbedderEntropyFactory::create_all(&config);

        assert_eq!(calculators.len(), 13);

        // Verify each calculator matches the expected embedder
        for (idx, calculator) in calculators.iter().enumerate() {
            let expected = Embedder::from_index(idx).unwrap();
            assert_eq!(
                calculator.embedder_type(),
                expected,
                "Calculator at index {} has wrong embedder type",
                idx
            );
        }
        println!("[PASS] factory_create_all returns 13 calculators");
    }

    #[test]
    fn test_all_calculators_handle_empty_history() {
        let config = SurpriseConfig::default();

        for embedder in Embedder::all() {
            let calculator = EmbedderEntropyFactory::create(embedder, &config);

            // Create appropriate test vector based on embedder type
            let current = match embedder {
                Embedder::Semantic | Embedder::Hdc => vec![0.5f32; 1024],
                Embedder::TemporalRecent
                | Embedder::TemporalPeriodic
                | Embedder::TemporalPositional => vec![0.5f32; 512],
                Embedder::Causal | Embedder::Multimodal => vec![0.5f32; 768],
                Embedder::Code => vec![0.5f32; 1536],
                Embedder::Graph | Embedder::Entity => vec![0.5f32; 384],
                Embedder::Sparse | Embedder::KeywordSplade => vec![0.5f32; 100], // Sparse test
                Embedder::LateInteraction => vec![0.5f32; 128], // Token-level
            };

            let history: Vec<Vec<f32>> = vec![];
            let result = calculator.compute_delta_s(&current, &history, 5);

            assert!(
                result.is_ok(),
                "Calculator for {:?} failed on empty history: {:?}",
                embedder,
                result
            );
            assert_eq!(
                result.unwrap(),
                1.0,
                "Empty history should return 1.0 for {:?}",
                embedder
            );
        }
        println!("[PASS] All calculators return 1.0 for empty history");
    }

    #[test]
    fn test_all_calculators_handle_empty_input() {
        let config = SurpriseConfig::default();

        for embedder in Embedder::all() {
            let calculator = EmbedderEntropyFactory::create(embedder, &config);

            let empty: Vec<f32> = vec![];
            let history = vec![vec![0.5f32; 100]];

            let result = calculator.compute_delta_s(&empty, &history, 5);
            assert!(
                result.is_err(),
                "Calculator for {:?} should error on empty input",
                embedder
            );
        }
        println!("[PASS] All calculators error on empty input");
    }

    #[test]
    fn test_all_outputs_in_valid_range() {
        let config = SurpriseConfig::default();

        for embedder in Embedder::all() {
            let calculator = EmbedderEntropyFactory::create(embedder, &config);

            // Create test data appropriate for embedder
            let dim = match embedder {
                Embedder::Semantic | Embedder::Hdc => 1024,
                Embedder::TemporalRecent
                | Embedder::TemporalPeriodic
                | Embedder::TemporalPositional => 512,
                Embedder::Causal | Embedder::Multimodal => 768,
                Embedder::Code => 1536,
                Embedder::Graph | Embedder::Entity => 384,
                Embedder::Sparse | Embedder::KeywordSplade => 100,
                Embedder::LateInteraction => 128,
            };

            let current: Vec<f32> = (0..dim).map(|i| (i as f32) / (dim as f32)).collect();
            let history: Vec<Vec<f32>> = (0..10)
                .map(|j| {
                    (0..dim)
                        .map(|i| ((i + j * 10) as f32) / (dim as f32))
                        .collect()
                })
                .collect();

            let result = calculator.compute_delta_s(&current, &history, 5);

            if let Ok(delta_s) = result {
                assert!(
                    (0.0..=1.0).contains(&delta_s),
                    "ΔS for {:?} out of range: {}",
                    embedder,
                    delta_s
                );
                assert!(
                    !delta_s.is_nan(),
                    "ΔS for {:?} is NaN (violates AP-10)",
                    embedder
                );
                assert!(
                    !delta_s.is_infinite(),
                    "ΔS for {:?} is infinite (violates AP-10)",
                    embedder
                );
            }
        }
        println!("[PASS] All calculator outputs in [0.0, 1.0] range, no NaN/Infinity");
    }
}
