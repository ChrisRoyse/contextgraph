//! Integration tests for UTL metrics module (M05-T24)
//!
//! These tests verify the integration between metrics types and other UTL components,
//! ensuring proper interaction between StageThresholds, QuadrantDistribution,
//! UtlComputationMetrics, and UtlStatus with lifecycle and phase components.

use context_graph_utl::{
    johari::JohariQuadrant,
    metrics::{
        QuadrantDistribution, StageThresholds, ThresholdsResponse, UtlComputationMetrics,
        UtlStatus, UtlStatusResponse,
    },
    phase::ConsolidationPhase,
    LifecycleLambdaWeights, LifecycleStage,
};

/// Test that StageThresholds factory methods produce correct stage-specific values
#[test]
fn test_stage_thresholds_factory_integration() {
    // Verify each stage has progressively changing thresholds
    let infancy = StageThresholds::infancy();
    let growth = StageThresholds::growth();
    let maturity = StageThresholds::maturity();

    // Entropy triggers should DECREASE with stage (less novelty-seeking over time)
    assert!(infancy.entropy_trigger > growth.entropy_trigger);
    assert!(growth.entropy_trigger > maturity.entropy_trigger);

    // Coherence triggers should increase (higher coherence standards in maturity)
    assert!(infancy.coherence_trigger < growth.coherence_trigger);
    assert!(growth.coherence_trigger < maturity.coherence_trigger);

    // Min importance should increase (more selective storage over time)
    assert!(infancy.min_importance_store < growth.min_importance_store);
    assert!(growth.min_importance_store < maturity.min_importance_store);

    // Consolidation thresholds should increase
    assert!(infancy.consolidation_threshold < growth.consolidation_threshold);
    assert!(growth.consolidation_threshold < maturity.consolidation_threshold);
}

/// Test for_stage factory matches individual factories
#[test]
fn test_for_stage_factory_consistency() {
    assert_eq!(
        StageThresholds::for_stage(LifecycleStage::Infancy),
        StageThresholds::infancy()
    );
    assert_eq!(
        StageThresholds::for_stage(LifecycleStage::Growth),
        StageThresholds::growth()
    );
    assert_eq!(
        StageThresholds::for_stage(LifecycleStage::Maturity),
        StageThresholds::maturity()
    );
}

/// Test QuadrantDistribution integration with JohariQuadrant
#[test]
fn test_quadrant_distribution_johari_integration() {
    let mut dist = QuadrantDistribution::default();

    // Increment each quadrant
    dist.increment(JohariQuadrant::Open);
    dist.increment(JohariQuadrant::Open);
    dist.increment(JohariQuadrant::Blind);
    dist.increment(JohariQuadrant::Hidden);
    dist.increment(JohariQuadrant::Unknown);

    assert_eq!(dist.open, 2);
    assert_eq!(dist.blind, 1);
    assert_eq!(dist.hidden, 1);
    assert_eq!(dist.unknown, 1);
    assert_eq!(dist.total(), 5);

    // Dominant should return Open (highest count)
    assert_eq!(dist.dominant(), JohariQuadrant::Open);
}

/// Test UtlComputationMetrics lifecycle stage integration
#[test]
fn test_computation_metrics_lifecycle_integration() {
    let mut metrics = UtlComputationMetrics::new();

    // Initially at default (Infancy)
    assert_eq!(metrics.lifecycle_stage, LifecycleStage::Infancy);

    // Record some computations
    metrics.record_computation(0.5, 0.3, 0.2, JohariQuadrant::Open, 100.0);
    metrics.record_computation(0.4, 0.2, 0.3, JohariQuadrant::Blind, 150.0);

    assert_eq!(metrics.computation_count, 2);
    assert!(metrics.avg_learning_magnitude > 0.0);
    assert_eq!(metrics.quadrant_distribution.open, 1);
    assert_eq!(metrics.quadrant_distribution.blind, 1);
}

/// Test UtlStatus full lifecycle with transitions
#[test]
fn test_utl_status_full_lifecycle() {
    // Create status at infancy
    let thresholds = StageThresholds::infancy();
    let weights = LifecycleLambdaWeights::for_stage(LifecycleStage::Infancy);
    let metrics = UtlComputationMetrics::new();

    let status = UtlStatus {
        lifecycle_stage: LifecycleStage::Infancy,
        interaction_count: 10,
        current_thresholds: thresholds,
        lambda_weights: weights,
        phase_angle: 0.5,
        consolidation_phase: ConsolidationPhase::Wake,
        metrics,
    };

    // Verify lifecycle helpers
    assert!(status.is_novelty_seeking());
    assert!(!status.is_consolidation_focused());
    assert!(status.is_wake());
    assert!(!status.is_encoding());
    assert!(!status.is_consolidating());

    // Verify MCP response conversion
    let response = status.to_mcp_response();
    assert_eq!(response.lifecycle_phase, "Infancy");
    assert_eq!(response.interaction_count, 10);
}

/// Test UtlStatus phase transitions
#[test]
fn test_utl_status_phase_transitions() {
    let create_status = |phase: ConsolidationPhase| -> UtlStatus {
        UtlStatus {
            lifecycle_stage: LifecycleStage::Growth,
            interaction_count: 100,
            current_thresholds: StageThresholds::growth(),
            lambda_weights: LifecycleLambdaWeights::for_stage(LifecycleStage::Growth),
            phase_angle: 1.0,
            consolidation_phase: phase,
            metrics: UtlComputationMetrics::new(),
        }
    };

    let wake_status = create_status(ConsolidationPhase::Wake);
    assert!(wake_status.is_wake());
    assert!(!wake_status.is_encoding());
    assert!(!wake_status.is_consolidating());

    let nrem_status = create_status(ConsolidationPhase::NREM);
    assert!(!nrem_status.is_wake());
    assert!(nrem_status.is_encoding());
    assert!(!nrem_status.is_consolidating());

    let rem_status = create_status(ConsolidationPhase::REM);
    assert!(!rem_status.is_wake());
    assert!(!rem_status.is_encoding());
    assert!(rem_status.is_consolidating());
}

/// Test MCP response types serialization roundtrip
#[test]
fn test_mcp_response_serialization() {
    let status = UtlStatus {
        lifecycle_stage: LifecycleStage::Maturity,
        interaction_count: 1000,
        current_thresholds: StageThresholds::maturity(),
        lambda_weights: LifecycleLambdaWeights::for_stage(LifecycleStage::Maturity),
        phase_angle: std::f32::consts::PI,
        consolidation_phase: ConsolidationPhase::REM,
        metrics: UtlComputationMetrics::new(),
    };

    let response = status.to_mcp_response();

    // Serialize to JSON
    let json = serde_json::to_string(&response).expect("serialization should succeed");

    // Deserialize back
    let deserialized: UtlStatusResponse =
        serde_json::from_str(&json).expect("deserialization should succeed");

    assert_eq!(deserialized.lifecycle_phase, "Maturity");
    assert_eq!(deserialized.interaction_count, 1000);
}

/// Test ThresholdsResponse serialization
#[test]
fn test_thresholds_response_serialization() {
    let response = ThresholdsResponse::from(&StageThresholds::growth());

    let json = serde_json::to_string(&response).expect("serialization should succeed");
    let deserialized: ThresholdsResponse =
        serde_json::from_str(&json).expect("deserialization should succeed");

    assert_eq!(deserialized.entropy_trigger, 0.7);
    assert_eq!(deserialized.coherence_trigger, 0.5);
}

/// Test metrics health indicators across stages
#[test]
fn test_metrics_health_across_stages() {
    for stage in [
        LifecycleStage::Infancy,
        LifecycleStage::Growth,
        LifecycleStage::Maturity,
    ] {
        let mut metrics = UtlComputationMetrics::new();
        metrics.lifecycle_stage = stage;

        // New metrics should be healthy (no computations yet means no issues)
        assert!(
            metrics.is_healthy(),
            "New metrics for {:?} should be healthy",
            stage
        );

        // Learning efficiency should be 0.0 for new metrics
        assert_eq!(
            metrics.learning_efficiency(),
            0.0,
            "New metrics for {:?} should have 0 efficiency",
            stage
        );
    }
}

/// Test quadrant distribution percentages with real data
#[test]
fn test_quadrant_percentages_real_distribution() {
    let mut dist = QuadrantDistribution::default();

    // Simulate a realistic learning session distribution
    // Open: 40%, Blind: 25%, Hidden: 20%, Unknown: 15%
    for _ in 0..40 {
        dist.increment(JohariQuadrant::Open);
    }
    for _ in 0..25 {
        dist.increment(JohariQuadrant::Blind);
    }
    for _ in 0..20 {
        dist.increment(JohariQuadrant::Hidden);
    }
    for _ in 0..15 {
        dist.increment(JohariQuadrant::Unknown);
    }

    let pcts = dist.percentages();

    // percentages() returns fractions [0,1], not percentages [0,100]
    assert!((pcts[0] - 0.40).abs() < 0.01); // open
    assert!((pcts[1] - 0.25).abs() < 0.01); // blind
    assert!((pcts[2] - 0.20).abs() < 0.01); // hidden
    assert!((pcts[3] - 0.15).abs() < 0.01); // unknown

    // Dominant should be Open
    assert_eq!(dist.dominant(), JohariQuadrant::Open);
}

/// Test computation metrics EMA behavior
#[test]
fn test_computation_metrics_ema_convergence() {
    let mut metrics = UtlComputationMetrics::new();

    // Record many computations with same values - EMA should converge
    for _ in 0..100 {
        metrics.record_computation(0.5, 0.3, 0.2, JohariQuadrant::Open, 1000.0);
    }

    // After 100 iterations, EMA should be very close to actual values
    assert!(
        (metrics.avg_learning_magnitude - 0.5).abs() < 0.01,
        "EMA should converge to 0.5, got {}",
        metrics.avg_learning_magnitude
    );
    assert!(
        (metrics.avg_delta_s - 0.3).abs() < 0.01,
        "EMA should converge to 0.3, got {}",
        metrics.avg_delta_s
    );
    assert!(
        (metrics.avg_delta_c - 0.2).abs() < 0.01,
        "EMA should converge to 0.2, got {}",
        metrics.avg_delta_c
    );
}

/// Test that UTL status summary is well-formed
#[test]
fn test_utl_status_summary_format() {
    let status = UtlStatus {
        lifecycle_stage: LifecycleStage::Growth,
        interaction_count: 250,
        current_thresholds: StageThresholds::growth(),
        lambda_weights: LifecycleLambdaWeights::for_stage(LifecycleStage::Growth),
        phase_angle: 1.5,
        consolidation_phase: ConsolidationPhase::Wake,
        metrics: UtlComputationMetrics::new(),
    };

    let summary = status.summary();

    assert!(summary.contains("UTL:"));
    assert!(summary.contains("Growth"));
    assert!(summary.contains("250"));
    assert!(summary.contains("Wake"));
}

/// Test UtlStatus new() creates consistent defaults
#[test]
fn test_utl_status_new_consistency() {
    let status1 = UtlStatus::new();
    let status2 = UtlStatus::default();

    assert_eq!(status1, status2);
    assert_eq!(status1.lifecycle_stage, LifecycleStage::Infancy);
    assert_eq!(status1.interaction_count, 0);
    assert_eq!(status1.consolidation_phase, ConsolidationPhase::Wake);
}

/// Test lifecycle stage behaviors
#[test]
fn test_lifecycle_stage_behaviors() {
    let infancy_status = UtlStatus {
        lifecycle_stage: LifecycleStage::Infancy,
        ..Default::default()
    };
    assert!(infancy_status.is_novelty_seeking());
    assert!(!infancy_status.is_balanced());
    assert!(!infancy_status.is_consolidation_focused());

    let growth_status = UtlStatus {
        lifecycle_stage: LifecycleStage::Growth,
        ..Default::default()
    };
    assert!(!growth_status.is_novelty_seeking());
    assert!(growth_status.is_balanced());
    assert!(!growth_status.is_consolidation_focused());

    let maturity_status = UtlStatus {
        lifecycle_stage: LifecycleStage::Maturity,
        ..Default::default()
    };
    assert!(!maturity_status.is_novelty_seeking());
    assert!(!maturity_status.is_balanced());
    assert!(maturity_status.is_consolidation_focused());
}

/// Test MCP response contains all required fields
#[test]
fn test_mcp_response_completeness() {
    let mut metrics = UtlComputationMetrics::new();
    metrics.record_computation(0.7, 0.4, 0.5, JohariQuadrant::Open, 500.0);

    let status = UtlStatus {
        lifecycle_stage: LifecycleStage::Growth,
        interaction_count: 150,
        current_thresholds: StageThresholds::growth(),
        lambda_weights: LifecycleLambdaWeights::for_stage(LifecycleStage::Growth),
        phase_angle: 1.0,
        consolidation_phase: ConsolidationPhase::Wake,
        metrics,
    };

    let response = status.to_mcp_response();

    // Verify all fields are populated
    assert!(!response.lifecycle_phase.is_empty());
    assert_eq!(response.interaction_count, 150);
    assert!(response.entropy >= 0.0 && response.entropy <= 1.0);
    assert!(response.coherence >= 0.0 && response.coherence <= 1.0);
    assert!(response.learning_score >= 0.0 && response.learning_score <= 1.0);
    assert!(!response.johari_quadrant.is_empty());
    assert!(!response.consolidation_phase.is_empty());
    assert!(response.phase_angle >= 0.0);

    // Verify thresholds are populated
    assert!(response.thresholds.entropy_trigger > 0.0);
    assert!(response.thresholds.coherence_trigger > 0.0);
}
