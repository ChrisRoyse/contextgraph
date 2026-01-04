//! Stub implementation of UTL processor.

use async_trait::async_trait;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::error::CoreResult;
use crate::traits::UtlProcessor;
use crate::types::{MemoryNode, UtlContext, UtlMetrics};

/// Stub UTL processor for Ghost System phase.
///
/// Returns deterministic values based on input hashing.
#[derive(Debug, Clone, Default)]
pub struct StubUtlProcessor {
    consolidation_threshold: f32,
}

impl StubUtlProcessor {
    /// Create a new stub processor.
    pub fn new() -> Self {
        Self {
            consolidation_threshold: 0.7,
        }
    }

    /// Create with custom consolidation threshold.
    pub fn with_threshold(threshold: f32) -> Self {
        Self {
            consolidation_threshold: threshold,
        }
    }

    /// Generate a deterministic value from input.
    fn hash_to_float(input: &str, seed: u64) -> f32 {
        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        seed.hash(&mut hasher);
        let hash = hasher.finish();
        // Map to [0.0, 1.0]
        (hash as f64 / u64::MAX as f64) as f32
    }
}

#[async_trait]
impl UtlProcessor for StubUtlProcessor {
    async fn compute_learning_score(&self, input: &str, context: &UtlContext) -> CoreResult<f32> {
        let surprise = self.compute_surprise(input, context).await?;
        let coherence_change = self.compute_coherence_change(input, context).await?;
        let emotional_weight = self.compute_emotional_weight(input, context).await?;
        let alignment = self.compute_alignment(input, context).await?;

        // L = (ΔS × ΔC) · wₑ · cos φ
        let score = (surprise * coherence_change) * emotional_weight * alignment;
        Ok(score.clamp(0.0, 1.0))
    }

    async fn compute_surprise(&self, input: &str, _context: &UtlContext) -> CoreResult<f32> {
        Ok(Self::hash_to_float(input, 1))
    }

    async fn compute_coherence_change(
        &self,
        input: &str,
        _context: &UtlContext,
    ) -> CoreResult<f32> {
        Ok(Self::hash_to_float(input, 2))
    }

    async fn compute_emotional_weight(&self, input: &str, context: &UtlContext) -> CoreResult<f32> {
        let base = Self::hash_to_float(input, 3);
        // Apply emotional state modifier
        Ok((base * context.emotional_state.weight_modifier()).clamp(0.5, 1.5))
    }

    async fn compute_alignment(&self, input: &str, _context: &UtlContext) -> CoreResult<f32> {
        // Map to [-1.0, 1.0] range
        let base = Self::hash_to_float(input, 4);
        Ok(base * 2.0 - 1.0)
    }

    async fn should_consolidate(&self, node: &MemoryNode) -> CoreResult<bool> {
        Ok(node.importance >= self.consolidation_threshold)
    }

    async fn compute_metrics(&self, input: &str, context: &UtlContext) -> CoreResult<UtlMetrics> {
        let surprise = self.compute_surprise(input, context).await?;
        let coherence_change = self.compute_coherence_change(input, context).await?;
        let emotional_weight = self.compute_emotional_weight(input, context).await?;
        let alignment = self.compute_alignment(input, context).await?;
        let learning_score = self.compute_learning_score(input, context).await?;

        Ok(UtlMetrics {
            entropy: context.prior_entropy,
            coherence: context.current_coherence,
            learning_score,
            surprise,
            coherence_change,
            emotional_weight,
            alignment,
        })
    }

    fn get_status(&self) -> serde_json::Value {
        // Stub returns default/initial status
        serde_json::json!({
            "lifecycle_phase": "Infancy",
            "interaction_count": 0,
            "entropy": 0.0,
            "coherence": 0.0,
            "learning_score": 0.0,
            "johari_quadrant": "Hidden",
            "consolidation_phase": "Wake",
            "phase_angle": 0.0,
            "thresholds": {
                "entropy_trigger": 0.9,
                "coherence_trigger": 0.2,
                "min_importance_store": 0.1,
                "consolidation_threshold": 0.3
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::EmotionalState;

    #[tokio::test]
    async fn test_compute_learning_score() {
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();

        let score = processor
            .compute_learning_score("test input", &context)
            .await
            .unwrap();

        assert!((0.0..=1.0).contains(&score));
    }

    #[tokio::test]
    async fn test_deterministic_output() {
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();

        let score1 = processor
            .compute_surprise("same input", &context)
            .await
            .unwrap();
        let score2 = processor
            .compute_surprise("same input", &context)
            .await
            .unwrap();

        assert_eq!(score1, score2);
    }

    #[tokio::test]
    async fn test_different_inputs_different_outputs() {
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();

        let score1 = processor
            .compute_surprise("input one", &context)
            .await
            .unwrap();
        let score2 = processor
            .compute_surprise("input two", &context)
            .await
            .unwrap();

        assert_ne!(score1, score2);
    }

    #[tokio::test]
    async fn test_emotional_weight_modifier() {
        let processor = StubUtlProcessor::new();

        let neutral_ctx = UtlContext {
            emotional_state: EmotionalState::Neutral,
            ..Default::default()
        };
        let curious_ctx = UtlContext {
            emotional_state: EmotionalState::Curious,
            ..Default::default()
        };

        let neutral_weight = processor
            .compute_emotional_weight("test", &neutral_ctx)
            .await
            .unwrap();
        let curious_weight = processor
            .compute_emotional_weight("test", &curious_ctx)
            .await
            .unwrap();

        // Curious should have higher weight
        assert!(curious_weight >= neutral_weight);
    }

    // =========================================================================
    // TC-GHOST-001: UTL Equation Logic Tests
    // =========================================================================

    #[tokio::test]
    async fn test_utl_equation_formula_verification() {
        // TC-GHOST-001: UTL formula L = (surprise * coherence_change) * emotional_weight * alignment
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();

        let input = "test input for UTL verification";

        // Get individual components
        let surprise = processor.compute_surprise(input, &context).await.unwrap();
        let coherence_change = processor
            .compute_coherence_change(input, &context)
            .await
            .unwrap();
        let emotional_weight = processor
            .compute_emotional_weight(input, &context)
            .await
            .unwrap();
        let alignment = processor.compute_alignment(input, &context).await.unwrap();

        // Compute expected learning score using the formula
        let expected_raw = (surprise * coherence_change) * emotional_weight * alignment;
        let expected = expected_raw.clamp(0.0, 1.0);

        // Get actual learning score
        let actual = processor
            .compute_learning_score(input, &context)
            .await
            .unwrap();

        // Verify formula is correctly implemented
        assert!(
            (actual - expected).abs() < 0.0001,
            "UTL formula mismatch: expected {} = ({} * {}) * {} * {}, got {}",
            expected,
            surprise,
            coherence_change,
            emotional_weight,
            alignment,
            actual
        );
    }

    #[tokio::test]
    async fn test_utl_learning_score_in_valid_range() {
        // TC-GHOST-001: Learning score must always be in [0.0, 1.0]
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();

        for input in [
            "a",
            "test",
            "Neural Network",
            "complex input string with many words",
        ] {
            let score = processor
                .compute_learning_score(input, &context)
                .await
                .unwrap();
            assert!(
                score >= 0.0 && score <= 1.0,
                "Learning score {} for '{}' must be in [0.0, 1.0]",
                score,
                input
            );
        }
    }

    #[tokio::test]
    async fn test_utl_components_deterministic() {
        // TC-GHOST-001: All UTL components must be deterministic
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();
        let input = "determinism test input";

        // Compute twice
        let surprise1 = processor.compute_surprise(input, &context).await.unwrap();
        let surprise2 = processor.compute_surprise(input, &context).await.unwrap();

        let coherence1 = processor
            .compute_coherence_change(input, &context)
            .await
            .unwrap();
        let coherence2 = processor
            .compute_coherence_change(input, &context)
            .await
            .unwrap();

        let weight1 = processor
            .compute_emotional_weight(input, &context)
            .await
            .unwrap();
        let weight2 = processor
            .compute_emotional_weight(input, &context)
            .await
            .unwrap();

        let align1 = processor.compute_alignment(input, &context).await.unwrap();
        let align2 = processor.compute_alignment(input, &context).await.unwrap();

        // All must match
        assert_eq!(surprise1, surprise2, "Surprise must be deterministic");
        assert_eq!(
            coherence1, coherence2,
            "Coherence change must be deterministic"
        );
        assert_eq!(weight1, weight2, "Emotional weight must be deterministic");
        assert_eq!(align1, align2, "Alignment must be deterministic");
    }

    #[tokio::test]
    async fn test_utl_surprise_in_valid_range() {
        // TC-GHOST-001: Surprise component must be in [0.0, 1.0]
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();

        for input in [
            "",
            "x",
            "test phrase",
            "A very long input string for testing boundaries",
        ] {
            let surprise = processor.compute_surprise(input, &context).await.unwrap();
            assert!(
                surprise >= 0.0 && surprise <= 1.0,
                "Surprise {} for '{}' must be in [0.0, 1.0]",
                surprise,
                input
            );
        }
    }

    #[tokio::test]
    async fn test_utl_coherence_change_in_valid_range() {
        // TC-GHOST-001: Coherence change component must be in [0.0, 1.0]
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();

        for input in [
            "",
            "x",
            "test phrase",
            "A very long input string for testing boundaries",
        ] {
            let coherence = processor
                .compute_coherence_change(input, &context)
                .await
                .unwrap();
            assert!(
                coherence >= 0.0 && coherence <= 1.0,
                "Coherence change {} for '{}' must be in [0.0, 1.0]",
                coherence,
                input
            );
        }
    }

    #[tokio::test]
    async fn test_utl_alignment_in_valid_range() {
        // TC-GHOST-001: Alignment (cos phi) must be in [-1.0, 1.0]
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();

        for input in [
            "",
            "x",
            "test phrase",
            "A very long input string for testing boundaries",
        ] {
            let alignment = processor.compute_alignment(input, &context).await.unwrap();
            assert!(
                alignment >= -1.0 && alignment <= 1.0,
                "Alignment {} for '{}' must be in [-1.0, 1.0]",
                alignment,
                input
            );
        }
    }

    #[tokio::test]
    async fn test_utl_emotional_weight_in_valid_range() {
        // TC-GHOST-001: Emotional weight must be in [0.5, 1.5] after clamping
        let processor = StubUtlProcessor::new();

        let states = [
            EmotionalState::Neutral,
            EmotionalState::Curious,
            EmotionalState::Focused,
            EmotionalState::Stressed,
            EmotionalState::Fatigued,
            EmotionalState::Engaged,
            EmotionalState::Confused,
        ];

        for state in states {
            let context = UtlContext {
                emotional_state: state,
                ..Default::default()
            };
            let weight = processor
                .compute_emotional_weight("test", &context)
                .await
                .unwrap();
            assert!(
                weight >= 0.5 && weight <= 1.5,
                "Emotional weight {} for {:?} must be in [0.5, 1.5]",
                weight,
                state
            );
        }
    }

    #[tokio::test]
    async fn test_utl_metrics_contains_all_components() {
        // TC-GHOST-001: compute_metrics must return all UTL components
        let processor = StubUtlProcessor::new();
        let context = UtlContext {
            prior_entropy: 0.6,
            current_coherence: 0.7,
            ..Default::default()
        };

        let metrics = processor
            .compute_metrics("test input", &context)
            .await
            .unwrap();

        // Verify all fields are populated
        assert_eq!(
            metrics.entropy, context.prior_entropy,
            "Entropy must match context"
        );
        assert_eq!(
            metrics.coherence, context.current_coherence,
            "Coherence must match context"
        );

        // Verify components match individual computations
        let surprise = processor
            .compute_surprise("test input", &context)
            .await
            .unwrap();
        let coherence_change = processor
            .compute_coherence_change("test input", &context)
            .await
            .unwrap();
        let emotional_weight = processor
            .compute_emotional_weight("test input", &context)
            .await
            .unwrap();
        let alignment = processor
            .compute_alignment("test input", &context)
            .await
            .unwrap();
        let learning_score = processor
            .compute_learning_score("test input", &context)
            .await
            .unwrap();

        assert_eq!(metrics.surprise, surprise);
        assert_eq!(metrics.coherence_change, coherence_change);
        assert_eq!(metrics.emotional_weight, emotional_weight);
        assert_eq!(metrics.alignment, alignment);
        assert_eq!(metrics.learning_score, learning_score);
    }

    #[tokio::test]
    async fn test_utl_consolidation_threshold() {
        // TC-GHOST-001: Consolidation decision must respect threshold
        let processor = StubUtlProcessor::with_threshold(0.7);
        let embedding = vec![0.5; 1536];

        // Node below threshold
        let mut low_importance =
            crate::types::MemoryNode::new("low".to_string(), embedding.clone());
        low_importance.importance = 0.5;

        // Node at threshold
        let mut at_threshold = crate::types::MemoryNode::new("at".to_string(), embedding.clone());
        at_threshold.importance = 0.7;

        // Node above threshold
        let mut high_importance = crate::types::MemoryNode::new("high".to_string(), embedding);
        high_importance.importance = 0.9;

        assert!(
            !processor.should_consolidate(&low_importance).await.unwrap(),
            "Node below threshold should not consolidate"
        );
        assert!(
            processor.should_consolidate(&at_threshold).await.unwrap(),
            "Node at threshold should consolidate"
        );
        assert!(
            processor
                .should_consolidate(&high_importance)
                .await
                .unwrap(),
            "Node above threshold should consolidate"
        );
    }

    #[tokio::test]
    async fn test_utl_different_inputs_different_scores() {
        // TC-GHOST-001: Different inputs should produce different scores
        let processor = StubUtlProcessor::new();
        let context = UtlContext::default();

        let score1 = processor
            .compute_learning_score("input one", &context)
            .await
            .unwrap();
        let score2 = processor
            .compute_learning_score("input two", &context)
            .await
            .unwrap();

        assert_ne!(
            score1, score2,
            "Different inputs should produce different learning scores"
        );
    }
}
