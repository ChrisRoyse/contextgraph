//! Cognitive pulse action matrix tests.
//!
//! Tests the decision logic for cognitive state management based on entropy and coherence.

use context_graph_core::types::{CognitivePulse, SuggestedAction};

#[test]
fn test_cognitive_pulse_action_matrix() {
    println!("=== COGNITIVE PULSE ACTION MATRIX ===");

    // Test decision paths from constitution.yaml and PRD
    // entropy > 0.7, coherence < 0.4 -> Stabilize
    let stabilize = CognitivePulse::from_values(0.8, 0.3);
    println!(
        "entropy=0.8, coherence=0.3 => {:?}",
        stabilize.suggested_action
    );
    assert_eq!(
        stabilize.suggested_action,
        SuggestedAction::Stabilize,
        "High entropy + low coherence should suggest Stabilize"
    );

    // entropy < 0.4, coherence > 0.6 -> Ready
    let ready = CognitivePulse::from_values(0.3, 0.8);
    println!("entropy=0.3, coherence=0.8 => {:?}", ready.suggested_action);
    assert_eq!(
        ready.suggested_action,
        SuggestedAction::Ready,
        "Low entropy + high coherence should suggest Ready"
    );

    // entropy > 0.6, coherence > 0.5 -> Explore
    let explore = CognitivePulse::from_values(0.7, 0.6);
    println!(
        "entropy=0.7, coherence=0.6 => {:?}",
        explore.suggested_action
    );
    assert_eq!(
        explore.suggested_action,
        SuggestedAction::Explore,
        "High entropy + moderate coherence should suggest Explore"
    );

    // Default case -> Continue
    let continue_action = CognitivePulse::from_values(0.5, 0.5);
    println!(
        "entropy=0.5, coherence=0.5 => {:?}",
        continue_action.suggested_action
    );
    assert_eq!(
        continue_action.suggested_action,
        SuggestedAction::Continue,
        "Balanced state should suggest Continue"
    );

    // Low coherence -> Consolidate
    let consolidate = CognitivePulse::from_values(0.4, 0.3);
    println!(
        "entropy=0.4, coherence=0.3 => {:?}",
        consolidate.suggested_action
    );
    assert_eq!(
        consolidate.suggested_action,
        SuggestedAction::Consolidate,
        "Low coherence should suggest Consolidate"
    );

    // Very high entropy -> Prune
    let prune = CognitivePulse::from_values(0.85, 0.5);
    println!(
        "entropy=0.85, coherence=0.5 => {:?}",
        prune.suggested_action
    );
    assert_eq!(
        prune.suggested_action,
        SuggestedAction::Prune,
        "Very high entropy should suggest Prune"
    );

    println!("RESULT: PASSED");
}

#[test]
fn test_cognitive_pulse_is_healthy() {
    println!("=== COGNITIVE PULSE IS_HEALTHY TEST ===");

    let healthy = CognitivePulse::from_values(0.5, 0.5);
    println!(
        "BEFORE: entropy=0.5, coherence=0.5, is_healthy={}",
        healthy.is_healthy()
    );
    assert!(healthy.is_healthy(), "balanced pulse should be healthy");

    let unhealthy_high_entropy = CognitivePulse::from_values(0.9, 0.5);
    println!(
        "VERIFY: entropy=0.9, coherence=0.5, is_healthy={}",
        unhealthy_high_entropy.is_healthy()
    );
    assert!(
        !unhealthy_high_entropy.is_healthy(),
        "high entropy should be unhealthy"
    );

    let unhealthy_low_coherence = CognitivePulse::from_values(0.5, 0.2);
    println!(
        "VERIFY: entropy=0.5, coherence=0.2, is_healthy={}",
        unhealthy_low_coherence.is_healthy()
    );
    assert!(
        !unhealthy_low_coherence.is_healthy(),
        "low coherence should be unhealthy"
    );

    println!("RESULT: PASSED");
}
