//! Lifecycle Transition Tests
//!
//! Tests for lifecycle stage transitions at 50/500 thresholds per Marblestone (2016)

use context_graph_utl::{
    lifecycle::{LifecycleLambdaWeights, LifecycleStage},
    metrics::StageThresholds,
    processor::UtlProcessor,
};

use super::helpers::uniform_embedding;

// =============================================================================
// LIFECYCLE TRANSITION TESTS
// =============================================================================

#[test]
fn test_lifecycle_transitions_at_thresholds() {
    let mut processor = UtlProcessor::with_defaults();

    // Start at Infancy
    assert_eq!(
        processor.lifecycle_stage(),
        LifecycleStage::Infancy,
        "Must start at Infancy"
    );

    // Progress through interactions - compute_learning increments interaction count
    let embedding = uniform_embedding(128, 0.5);
    let content = "test";

    // Infancy: 0-49 interactions (compute_learning records each as an interaction)
    for i in 0..49 {
        let _ = processor.compute_learning(content, &embedding, &[]);
        assert_eq!(
            processor.lifecycle_stage(),
            LifecycleStage::Infancy,
            "should stay Infancy at {} interactions",
            i + 1
        );
    }

    // 50th interaction -> Growth
    let _ = processor.compute_learning(content, &embedding, &[]);
    assert_eq!(
        processor.lifecycle_stage(),
        LifecycleStage::Growth,
        "must transition to Growth at 50 interactions, got {:?} at {}",
        processor.lifecycle_stage(),
        processor.interaction_count()
    );

    // Continue to 499 (staying in Growth)
    for i in 50..499 {
        let _ = processor.compute_learning(content, &embedding, &[]);
        assert_eq!(
            processor.lifecycle_stage(),
            LifecycleStage::Growth,
            "should stay Growth at {} interactions",
            i + 1
        );
    }

    // 500th interaction -> Maturity
    let _ = processor.compute_learning(content, &embedding, &[]);
    assert_eq!(
        processor.lifecycle_stage(),
        LifecycleStage::Maturity,
        "must transition to Maturity at 500 interactions, got {:?} at {}",
        processor.lifecycle_stage(),
        processor.interaction_count()
    );
}

#[test]
fn test_lifecycle_boundary_exact_at_50() {
    let mut processor = UtlProcessor::with_defaults();
    let embedding = uniform_embedding(128, 0.5);

    // 49 interactions
    for _ in 0..49 {
        let _ = processor.compute_learning("test", &embedding, &[]);
    }
    assert_eq!(processor.lifecycle_stage(), LifecycleStage::Infancy);
    assert_eq!(processor.interaction_count(), 49);

    // 50th interaction triggers transition
    let _ = processor.compute_learning("test", &embedding, &[]);
    assert_eq!(processor.lifecycle_stage(), LifecycleStage::Growth);
    assert_eq!(processor.interaction_count(), 50);
}

#[test]
fn test_lifecycle_boundary_exact_at_500() {
    // Per LifecycleConfig default: transition_hysteresis = 10
    // Hysteresis prevents rapid oscillation at boundaries by requiring
    // N interactions since last transition before allowing stage change.
    //
    // When using restore_lifecycle(count), last_transition_count is set to count,
    // meaning hysteresis applies from the restored position.
    let mut processor = UtlProcessor::with_defaults();
    let embedding = uniform_embedding(128, 0.5);

    // Restore to 490 (10 interactions before maturity threshold)
    // This allows exactly 10 interactions (matching hysteresis) to reach Maturity
    processor.restore_lifecycle(490);
    assert_eq!(processor.lifecycle_stage(), LifecycleStage::Growth);
    assert_eq!(processor.interaction_count(), 490);

    // 10 interactions: 491, 492, 493, 494, 495, 496, 497, 498, 499, 500
    // After 10 interactions (>=hysteresis), we reach 500 and can transition
    for i in 0..9 {
        let _ = processor.compute_learning("test", &embedding, &[]);
        assert_eq!(
            processor.lifecycle_stage(),
            LifecycleStage::Growth,
            "should stay Growth at {} interactions (hysteresis not satisfied)",
            491 + i
        );
    }

    // 10th interaction (count=500): hysteresis satisfied, transition allowed
    let _ = processor.compute_learning("test", &embedding, &[]);
    assert_eq!(processor.interaction_count(), 500);
    assert_eq!(
        processor.lifecycle_stage(),
        LifecycleStage::Maturity,
        "must transition to Maturity at 500 after hysteresis (10 interactions) is satisfied"
    );
}

#[test]
fn test_lambda_weights_per_lifecycle_stage() {
    // Per Marblestone (2016): Infancy prioritizes novelty, Maturity prioritizes consolidation
    let infancy = LifecycleLambdaWeights::for_stage(LifecycleStage::Infancy);
    assert!(
        (infancy.lambda_s() - 0.7).abs() < 0.001,
        "Infancy lambda_s should be 0.7, got {}",
        infancy.lambda_s()
    );
    assert!(
        (infancy.lambda_c() - 0.3).abs() < 0.001,
        "Infancy lambda_c should be 0.3, got {}",
        infancy.lambda_c()
    );

    let growth = LifecycleLambdaWeights::for_stage(LifecycleStage::Growth);
    assert!(
        (growth.lambda_s() - 0.5).abs() < 0.001,
        "Growth lambda_s should be 0.5, got {}",
        growth.lambda_s()
    );
    assert!(
        (growth.lambda_c() - 0.5).abs() < 0.001,
        "Growth lambda_c should be 0.5, got {}",
        growth.lambda_c()
    );

    let maturity = LifecycleLambdaWeights::for_stage(LifecycleStage::Maturity);
    assert!(
        (maturity.lambda_s() - 0.3).abs() < 0.001,
        "Maturity lambda_s should be 0.3, got {}",
        maturity.lambda_s()
    );
    assert!(
        (maturity.lambda_c() - 0.7).abs() < 0.001,
        "Maturity lambda_c should be 0.7, got {}",
        maturity.lambda_c()
    );
}

#[test]
fn test_lambda_weights_sum_to_one() {
    for stage in [
        LifecycleStage::Infancy,
        LifecycleStage::Growth,
        LifecycleStage::Maturity,
    ] {
        let weights = LifecycleLambdaWeights::for_stage(stage);
        let sum = weights.lambda_s() + weights.lambda_c();
        assert!(
            (sum - 1.0).abs() < 0.001,
            "Lambda weights for {:?} must sum to 1.0, got {}",
            stage,
            sum
        );
    }
}

// =============================================================================
// STAGE THRESHOLDS INTEGRATION TESTS
// =============================================================================

#[test]
fn test_stage_thresholds_progression() {
    let infancy = StageThresholds::infancy();
    let growth = StageThresholds::growth();
    let maturity = StageThresholds::maturity();

    // Entropy triggers should decrease (less novelty-seeking over time)
    assert!(infancy.entropy_trigger > growth.entropy_trigger);
    assert!(growth.entropy_trigger > maturity.entropy_trigger);

    // Coherence triggers should increase
    assert!(infancy.coherence_trigger < growth.coherence_trigger);
    assert!(growth.coherence_trigger < maturity.coherence_trigger);

    // Min importance should increase (more selective)
    assert!(infancy.min_importance_store < growth.min_importance_store);
    assert!(growth.min_importance_store < maturity.min_importance_store);
}
