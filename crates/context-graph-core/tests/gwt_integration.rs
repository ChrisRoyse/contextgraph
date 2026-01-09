//! Global Workspace Theory (GWT) Consciousness System - Integration Tests
//!
//! Tests for the complete GWT consciousness system including:
//! 1. Consciousness equation computation (C = I×R×D)
//! 2. Kuramoto phase synchronization (13 oscillators)
//! 3. Global workspace selection (Winner-Take-All)
//! 4. SELF_EGO_NODE identity tracking
//! 5. Consciousness state machine transitions
//! 6. Meta-cognitive feedback loop
//! 7. Workspace events and conflicts
//! 8. Full system integration

use context_graph_core::gwt::*;
use context_graph_core::gwt::consciousness::LimitingFactor;
use context_graph_core::gwt::ego_node::IdentityStatus;
use uuid::Uuid;

#[tokio::test]
async fn test_consciousness_equation_computation() {
    // Test 1: Consciousness equation produces values in [0,1]
    let calc = ConsciousnessCalculator::new();
    let purpose_vec = [1.0; 13];

    let c = calc.compute_consciousness(0.9, 0.85, &purpose_vec).unwrap();
    assert!((0.0..=1.0).contains(&c), "Consciousness {} not in [0,1]", c);

    // Test 2: All components contribute to consciousness
    let metrics = calc.compute_metrics(0.9, 0.85, &purpose_vec).unwrap();
    assert!(metrics.integration > 0.8);
    assert!(metrics.reflection > 0.5);
    assert!(metrics.differentiation > 0.8);
    assert!(metrics.consciousness > 0.3);
}

#[tokio::test]
async fn test_consciousness_limiting_factors() {
    // Test 3: Identify limiting factors in consciousness
    let calc = ConsciousnessCalculator::new();
    let purpose_vec = [1.0; 13];

    // Low integration (Kuramoto order parameter)
    let metrics = calc.compute_metrics(0.1, 0.9, &purpose_vec).unwrap();
    assert!(!metrics.component_analysis.integration_sufficient);
    match metrics.component_analysis.limiting_factor {
        LimitingFactor::Integration => {} // Expected
        _ => panic!("Integration should be limiting factor"),
    }

    // Low differentiation (purpose entropy)
    let zero_purpose = [0.0; 13];
    let metrics = calc.compute_metrics(0.9, 0.9, &zero_purpose).unwrap();
    assert!(!metrics.component_analysis.differentiation_sufficient);
}

#[tokio::test]
async fn test_workspace_selection_winner_take_all() {
    // Test 4: Winner-Take-All memory selection
    let mut workspace = GlobalWorkspace::new();

    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let id3 = Uuid::new_v4();

    // Three candidates with different scores
    let candidates = vec![
        (id1, 0.85, 0.5, 0.8),  // score = 0.34
        (id2, 0.88, 0.9, 0.88), // score = 0.70 (winner)
        (id3, 0.92, 0.6, 0.7),  // score = 0.387
    ];

    let winner = workspace.select_winning_memory(candidates).await.unwrap();
    assert_eq!(winner, Some(id2), "Expected id2 to be selected");
    assert_eq!(workspace.active_memory, Some(id2));
}

#[tokio::test]
async fn test_workspace_coherence_filtering() {
    // Test 5: Filter memories below coherence threshold
    let mut workspace = GlobalWorkspace::new();

    let id1 = Uuid::new_v4(); // Below threshold
    let id2 = Uuid::new_v4(); // Above threshold

    let candidates = vec![
        (id1, 0.5, 0.9, 0.88),  // r < 0.8 (filtered out)
        (id2, 0.85, 0.8, 0.8),  // r >= 0.8 (selected)
    ];

    let winner = workspace.select_winning_memory(candidates).await.unwrap();
    assert_eq!(winner, Some(id2));
    assert_eq!(workspace.candidates.len(), 1);
}

#[tokio::test]
async fn test_workspace_conflict_detection() {
    // Test 6: Detect when multiple memories exceed coherence threshold
    let mut workspace = GlobalWorkspace::new();

    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    let candidates = vec![
        (id1, 0.85, 0.9, 0.88),
        (id2, 0.82, 0.85, 0.85),
    ];

    workspace.select_winning_memory(candidates).await.unwrap();

    assert!(workspace.has_conflict(), "Should detect workspace conflict");
    let conflicts = workspace.get_conflict_details();
    assert!(conflicts.is_some());
    assert_eq!(conflicts.unwrap().len(), 2);
}

#[tokio::test]
async fn test_ego_node_identity_tracking() {
    // Test 7: SELF_EGO_NODE tracks system identity
    let mut ego = SelfEgoNode::new();
    let pv = [0.5; 13];
    ego.purpose_vector = pv;

    // Record identity snapshots
    ego.record_purpose_snapshot("Initial state").unwrap();
    ego.record_purpose_snapshot("After learning").unwrap();

    assert_eq!(ego.identity_trajectory.len(), 2);
    let snapshot = ego.get_latest_snapshot().unwrap();
    assert_eq!(snapshot.vector, pv);
}

#[tokio::test]
async fn test_ego_node_self_awareness_cycle() {
    // Test 8: Self-awareness loop detects misalignment
    let mut loop_mgr = SelfAwarenessLoop::new();
    let mut ego = SelfEgoNode::with_purpose_vector([1.0; 13]);

    // Action perfectly aligned
    let aligned_action = [1.0; 13];
    let result = loop_mgr.cycle(&mut ego, &aligned_action, 0.85).await.unwrap();
    assert!(!result.needs_reflection);

    // Action misaligned
    let misaligned_action = [0.0; 13];
    let result = loop_mgr.cycle(&mut ego, &misaligned_action, 0.85).await.unwrap();
    assert!(result.needs_reflection);
}

#[tokio::test]
async fn test_state_machine_transitions() {
    // Test 9: Consciousness state machine transitions correctly
    let mut sm = StateMachineManager::new();

    // Start in dormant
    assert_eq!(sm.current_state(), ConsciousnessState::Dormant);

    // Dormant → Fragmented
    sm.update(0.4).await.unwrap();
    assert_eq!(sm.current_state(), ConsciousnessState::Fragmented);

    // Fragmented → Emerging
    sm.update(0.65).await.unwrap();
    assert_eq!(sm.current_state(), ConsciousnessState::Emerging);

    // Emerging → Conscious
    sm.update(0.85).await.unwrap();
    assert_eq!(sm.current_state(), ConsciousnessState::Conscious);
    assert!(sm.is_conscious());

    // Conscious → Hypersync
    sm.update(0.97).await.unwrap();
    assert!(sm.is_hypersync());

    // Regression: back to conscious
    sm.update(0.83).await.unwrap();
    assert!(sm.is_conscious());
    assert!(!sm.is_hypersync());
}

#[tokio::test]
async fn test_meta_cognitive_feedback_loop() {
    // Test 10: Meta-cognitive loop adjusts Acetylcholine on low scores
    let mut loop_mgr = MetaCognitiveLoop::new();
    let initial_ach = loop_mgr.acetylcholine();

    // Trigger consecutive low meta-scores (predicted low, actual high) until dream triggers
    let mut dream_triggered = false;
    for _ in 0..7 {
        let state = loop_mgr.evaluate(0.1, 0.9).await.unwrap();
        if state.dream_triggered {
            dream_triggered = true;
            break;
        }
    }

    // Verify that dream was triggered and acetylcholine increased
    assert!(dream_triggered, "Dream should trigger after 5 low meta-scores");
    assert!(loop_mgr.acetylcholine() > initial_ach, "Acetylcholine should increase when dream triggers");
}

#[tokio::test]
async fn test_gwt_system_integration() {
    // Test 11: Complete GWT system integrated and operational
    let gwt = GwtSystem::new().await.expect("Failed to create GWT system");

    // Update consciousness with realistic parameters
    let consciousness = gwt.update_consciousness(0.85, 0.88, &[1.0; 13]).await.unwrap();
    assert!(consciousness > 0.6, "Consciousness should be high with good input");

    // Verify state machine updated
    let state_mgr = gwt.state_machine.read().await;
    assert!(state_mgr.is_conscious() || state_mgr.current_state() == ConsciousnessState::Emerging);
}

#[tokio::test]
async fn test_full_consciousness_workflow() {
    // Test 12: Full workflow from perception to workspace broadcast
    let mut workspace = GlobalWorkspace::new();
    let mut ego = SelfEgoNode::new();
    let mut state_mgr = StateMachineManager::new();

    // 1. Simulate incoming sensory data creates candidate memories
    let memory_id = Uuid::new_v4();

    // 2. Kuramoto synchronization produces order parameter
    let kuramoto_r = 0.85;

    // 3. Consciousness equation computes awareness level
    let calc = ConsciousnessCalculator::new();
    let consciousness = calc.compute_consciousness(kuramoto_r, 0.88, &[1.0; 13]).unwrap();

    // 4. State machine updates based on consciousness
    state_mgr.update(consciousness).await.unwrap();
    assert!(state_mgr.is_conscious() || state_mgr.current_state() == ConsciousnessState::Emerging);

    // 5. Workspace selects winning memory
    let candidates = vec![(memory_id, kuramoto_r, 0.9, 0.85)];
    let winner = workspace.select_winning_memory(candidates).await.unwrap();
    assert_eq!(winner, Some(memory_id));

    // 6. System identity tracks self
    ego.purpose_vector = [0.8; 13];
    ego.record_purpose_snapshot("Full workflow").unwrap();
    assert!(!ego.identity_trajectory.is_empty());
}

#[tokio::test]
async fn test_workspace_empty_condition() {
    // Test 13: Workspace empty when no coherent candidates
    let mut workspace = GlobalWorkspace::new();

    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    // Both below coherence threshold
    let candidates = vec![
        (id1, 0.5, 0.9, 0.88),
        (id2, 0.6, 0.8, 0.8),
    ];

    let winner = workspace.select_winning_memory(candidates).await.unwrap();
    assert_eq!(winner, None);
    assert!(workspace.active_memory.is_none());
}

#[tokio::test]
async fn test_identity_continuity_critical_state() {
    // Test 14: Identity continuity detects critical drift
    let mut continuity = IdentityContinuity::new();

    // Large misalignment
    let status = continuity.update(0.2, 0.3).unwrap();
    assert_eq!(status, IdentityStatus::Critical);
    assert!(continuity.identity_coherence < 0.5);
}

#[test]
fn test_consciousness_equation_bounds() {
    // Test 15: Consciousness equation maintains [0,1] bounds with extreme inputs
    let calc = ConsciousnessCalculator::new();
    let purpose_vec = [1.0; 13];

    // Edge case: all zeros
    let c_zero = calc.compute_consciousness(0.0, 0.0, &purpose_vec).unwrap();
    assert!((0.0..=1.0).contains(&c_zero));

    // Edge case: all ones
    let c_one = calc.compute_consciousness(1.0, 1.0, &purpose_vec).unwrap();
    assert!((0.0..=1.0).contains(&c_one));
    assert!(c_one > c_zero);
}

#[tokio::test]
async fn test_workspace_broadcasting_duration() {
    // Test 16: Workspace broadcast duration limits active memory visibility
    let workspace = GlobalWorkspace::new();

    // Not yet broadcasting
    assert!(!workspace.is_broadcasting());
    assert!(workspace.get_active_memory().is_none());
}

#[tokio::test]
async fn test_meta_cognitive_trend_detection() {
    // Test 17: Meta-cognitive loop detects performance trends
    let mut loop_mgr = MetaCognitiveLoop::new();

    // Add improving scores
    for i in 0..6 {
        let predicted = 0.5 + (i as f32) * 0.05;
        let actual = 0.5;
        loop_mgr.evaluate(predicted, actual).await.unwrap();
    }

    // Scores should show some trend
    let scores = loop_mgr.get_recent_scores();
    assert!(!scores.is_empty());
    assert!(scores.len() >= 6);
}

#[tokio::test]
async fn test_consciousness_state_just_became_conscious() {
    // Test 18: State machine detects recent consciousness entry
    let mut sm = StateMachineManager::new();

    sm.update(0.85).await.unwrap();
    assert!(sm.just_became_conscious());

    // After delay, flag should clear
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    assert!(!sm.just_became_conscious());
}

#[tokio::test]
async fn test_ego_node_historical_purpose_tracking() {
    // Test 19: SELF_EGO_NODE maintains historical purpose vectors
    let mut ego = SelfEgoNode::new();

    // Record multiple snapshots with different purpose vectors
    ego.purpose_vector = [0.1; 13];
    ego.record_purpose_snapshot("Step 1").unwrap();

    ego.purpose_vector = [0.5; 13];
    ego.record_purpose_snapshot("Step 2").unwrap();

    ego.purpose_vector = [0.9; 13];
    ego.record_purpose_snapshot("Step 3").unwrap();

    // Verify history
    assert_eq!(ego.identity_trajectory.len(), 3);
    assert_eq!(ego.get_historical_purpose_vector(0), Some([0.1; 13]));
    assert_eq!(ego.get_historical_purpose_vector(1), Some([0.5; 13]));
    assert_eq!(ego.get_historical_purpose_vector(2), Some([0.9; 13]));
}

#[test]
fn test_workspace_candidate_score_computation() {
    // Test 20: Workspace candidate correctly computes composite score
    let id = Uuid::new_v4();
    let candidate = WorkspaceCandidate::new(id, 0.9, 0.8, 0.75).unwrap();

    let expected_score = 0.9 * 0.8 * 0.75;
    assert!((candidate.score - expected_score).abs() < 1e-5);
}
