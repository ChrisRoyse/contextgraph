//! UtlProcessor - Main UTL computation orchestrator.

use std::time::Instant;

use crate::coherence::CoherenceTracker;
use crate::config::UtlConfig;
use crate::emotional::EmotionalWeightCalculator;
use crate::error::{UtlError, UtlResult};
use crate::johari::JohariClassifier;
use crate::lifecycle::LifecycleManager;
use crate::phase::PhaseOscillator;
use crate::surprise::SurpriseCalculator;
use crate::{
    compute_learning_magnitude_validated, JohariQuadrant, LearningSignal, LifecycleLambdaWeights,
    LifecycleStage, SuggestedAction,
};
use context_graph_core::types::EmotionalState;

/// Main UTL computation orchestrator integrating all 6 UTL components:
/// surprise (delta_s), coherence (delta_c), emotional weight (w_e),
/// phase angle (phi), lifecycle lambda weights, and Johari classification.
///
/// # Performance Budget
/// Full `compute_learning()`: < 10ms (constitution perf.latency)
///
/// # Example
/// ```
/// use context_graph_utl::processor::UtlProcessor;
/// use context_graph_utl::config::UtlConfig;
///
/// let config = UtlConfig::default();
/// let mut processor = UtlProcessor::new(config);
///
/// let content = "New information about machine learning";
/// let embedding = vec![0.1; 1536];
/// let context = vec![vec![0.15; 1536], vec![0.12; 1536]];
///
/// let signal = processor.compute_learning(content, &embedding, &context)
///     .expect("Valid computation");
/// assert!(signal.magnitude >= 0.0 && signal.magnitude <= 1.0);
/// ```
#[derive(Debug)]
pub struct UtlProcessor {
    surprise_calculator: SurpriseCalculator,
    coherence_tracker: CoherenceTracker,
    emotional_calculator: EmotionalWeightCalculator,
    phase_oscillator: PhaseOscillator,
    johari_classifier: JohariClassifier,
    lifecycle_manager: LifecycleManager,
    config: UtlConfig,
    computation_count: u64,
}

impl UtlProcessor {
    /// Create a new UtlProcessor with the given configuration.
    /// Panics if config validation fails. Use `try_new()` for fallible construction.
    pub fn new(config: UtlConfig) -> Self {
        config.validate().expect("UtlConfig validation failed");
        Self::from_config(config)
    }

    /// Try to create a new UtlProcessor, returning an error if config is invalid.
    pub fn try_new(config: UtlConfig) -> UtlResult<Self> {
        config.validate().map_err(UtlError::ConfigError)?;
        Ok(Self::from_config(config))
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(UtlConfig::default())
    }

    fn from_config(config: UtlConfig) -> Self {
        Self {
            surprise_calculator: SurpriseCalculator::new(&config.surprise),
            coherence_tracker: CoherenceTracker::new(&config.coherence),
            emotional_calculator: EmotionalWeightCalculator::new(&config.emotional),
            phase_oscillator: PhaseOscillator::new(&config.phase),
            johari_classifier: JohariClassifier::new(&config.johari),
            lifecycle_manager: LifecycleManager::new(&config.lifecycle),
            config,
            computation_count: 0,
        }
    }

    /// Compute full UTL learning signal. Main entry point for UTL computation.
    ///
    /// # Arguments
    /// * `content` - Text content for emotional analysis
    /// * `embedding` - Vector embedding of the content (typically 1536D)
    /// * `context_embeddings` - Recent context embeddings for comparison
    ///
    /// # Returns
    /// `Ok(LearningSignal)` with all computed values, or `Err(UtlError)` on failure.
    ///
    /// # Performance
    /// Target: < 10ms (per constitution perf.latency.inject_context)
    pub fn compute_learning(
        &mut self,
        content: &str,
        embedding: &[f32],
        context_embeddings: &[Vec<f32>],
    ) -> UtlResult<LearningSignal> {
        self.compute_learning_internal(
            content,
            embedding,
            context_embeddings,
            EmotionalState::Neutral,
        )
    }

    /// Compute learning with explicit emotional state.
    pub fn compute_learning_with_state(
        &mut self,
        content: &str,
        embedding: &[f32],
        context_embeddings: &[Vec<f32>],
        emotional_state: EmotionalState,
    ) -> UtlResult<LearningSignal> {
        self.compute_learning_internal(content, embedding, context_embeddings, emotional_state)
    }

    fn compute_learning_internal(
        &mut self,
        content: &str,
        embedding: &[f32],
        context_embeddings: &[Vec<f32>],
        emotional_state: EmotionalState,
    ) -> UtlResult<LearningSignal> {
        let start = Instant::now();

        // Record interaction for lifecycle tracking
        let _transitioned = self.lifecycle_manager.increment();

        // Compute UTL components
        let delta_s = self
            .surprise_calculator
            .compute_surprise(embedding, context_embeddings);
        let delta_c = self
            .coherence_tracker
            .compute_coherence(embedding, context_embeddings);
        let w_e = self
            .emotional_calculator
            .compute_emotional_weight(content, emotional_state);
        let phi = self.phase_oscillator.phase();
        let lambda_weights = self.lifecycle_manager.current_weights();

        // Apply Marblestone lambda weights
        let weighted_delta_s = delta_s * lambda_weights.lambda_s();
        let weighted_delta_c = delta_c * lambda_weights.lambda_c();

        // Compute magnitude with validated inputs
        let magnitude = compute_learning_magnitude_validated(
            weighted_delta_s.clamp(0.0, 1.0),
            weighted_delta_c.clamp(0.0, 1.0),
            w_e.clamp(0.5, 1.5),
            phi.clamp(0.0, std::f32::consts::PI),
        )?;

        // Classify Johari quadrant using raw (unweighted) values
        let quadrant = self.johari_classifier.classify(delta_s, delta_c);
        let suggested_action = suggested_action_for_quadrant(quadrant);

        // Determine consolidation and storage decisions
        let should_consolidate = magnitude > self.config.thresholds.high_quality;
        let should_store = magnitude > self.config.thresholds.low_quality;
        let latency_us = start.elapsed().as_micros() as u64;

        self.computation_count += 1;

        LearningSignal::new(
            magnitude,
            delta_s,
            delta_c,
            w_e,
            phi,
            Some(lambda_weights),
            quadrant,
            suggested_action,
            should_consolidate,
            should_store,
            latency_us,
        )
    }

    /// Get current lifecycle stage.
    #[inline]
    pub fn lifecycle_stage(&self) -> LifecycleStage {
        self.lifecycle_manager.current_stage()
    }

    /// Get current lifecycle lambda weights.
    #[inline]
    pub fn lambda_weights(&self) -> LifecycleLambdaWeights {
        self.lifecycle_manager.current_weights()
    }

    /// Get total interaction count.
    #[inline]
    pub fn interaction_count(&self) -> u64 {
        self.lifecycle_manager.interaction_count()
    }

    /// Get computation count.
    #[inline]
    pub fn computation_count(&self) -> u64 {
        self.computation_count
    }

    /// Get current phase angle.
    #[inline]
    pub fn current_phase(&self) -> f32 {
        self.phase_oscillator.phase()
    }

    /// Update phase oscillator with elapsed time.
    pub fn update_phase(&mut self, elapsed: std::time::Duration) {
        self.phase_oscillator.update(elapsed);
    }

    /// Reset the processor to initial state.
    pub fn reset(&mut self) {
        self.lifecycle_manager.reset();
        self.coherence_tracker.clear();
        self.phase_oscillator.reset();
        self.computation_count = 0;
    }

    /// Restore lifecycle state from persistence.
    pub fn restore_lifecycle(&mut self, interaction_count: u64) {
        self.lifecycle_manager.increment_by(interaction_count);
    }
}

/// Map Johari quadrant to suggested action.
/// Per constitution.yaml:159-163: Open->DirectRecall, Blind->TriggerDream,
/// Hidden->GetNeighborhood, Unknown->EpistemicAction
fn suggested_action_for_quadrant(quadrant: JohariQuadrant) -> SuggestedAction {
    match quadrant {
        JohariQuadrant::Open => SuggestedAction::DirectRecall,
        JohariQuadrant::Blind => SuggestedAction::TriggerDream,
        JohariQuadrant::Hidden => SuggestedAction::GetNeighborhood,
        JohariQuadrant::Unknown => SuggestedAction::EpistemicAction,
    }
}
