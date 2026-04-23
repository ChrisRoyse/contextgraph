//! Weight projection trait for learned edge weight computation.
//!
//! This trait allows external crates (like context-graph-embeddings) to provide
//! learned weight projection models without creating circular dependencies.
//!
//! # Architecture
//!
//! The EdgeBuilder uses this trait to optionally apply learned projections:
//! 1. If no projector is configured, falls back to constitution-based weighted agreement
//! 2. If a projector is provided, uses learned model for weight computation
//!
//! # Implementation
//!
//! The `LearnedWeightProjection` in context-graph-embeddings implements this trait.

use std::sync::Arc;

/// Number of embedders in the system.
pub const NUM_EMBEDDERS: usize = 14;

/// Trait for projecting embedder similarity scores to edge weights.
///
/// Implementors receive 13 similarity scores (one per embedder) and return
/// a single edge weight in [0, 1].
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to allow concurrent access from
/// the EdgeBuilder.
pub trait WeightProjector: Send + Sync {
    /// Project embedder similarity scores to an edge weight.
    ///
    /// # Arguments
    ///
    /// * `scores` - Array of 13 similarity scores (E1-E13)
    ///
    /// # Returns
    ///
    /// Edge weight in [0, 1] range.
    fn project(&self, scores: &[f32; NUM_EMBEDDERS]) -> f32;

    /// Batch projection for multiple score sets.
    ///
    /// Default implementation calls `project` for each set.
    fn project_batch(&self, batch_scores: &[[f32; NUM_EMBEDDERS]]) -> Vec<f32> {
        batch_scores.iter().map(|s| self.project(s)).collect()
    }

    /// Check if the projector is using learned weights (vs fallback).
    fn is_learned(&self) -> bool;
}

/// Type alias for optional weight projector.
pub type OptionalProjector = Option<Arc<dyn WeightProjector>>;

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock projector for testing
    struct MockProjector {
        fixed_weight: f32,
    }

    impl WeightProjector for MockProjector {
        fn project(&self, _scores: &[f32; NUM_EMBEDDERS]) -> f32 {
            self.fixed_weight
        }

        fn is_learned(&self) -> bool {
            true
        }
    }

    #[test]
    fn test_mock_projector() {
        let projector = MockProjector { fixed_weight: 0.75 };
        let scores = [0.5; NUM_EMBEDDERS];
        assert!((projector.project(&scores) - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_batch_projection() {
        let projector = MockProjector { fixed_weight: 0.6 };
        let batch = vec![[0.5; NUM_EMBEDDERS], [0.7; NUM_EMBEDDERS]];
        let results = projector.project_batch(&batch);
        assert_eq!(results.len(), 2);
        assert!((results[0] - 0.6).abs() < 0.001);
        assert!((results[1] - 0.6).abs() < 0.001);
    }
}
