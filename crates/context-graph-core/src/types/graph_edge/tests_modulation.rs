//! Unit tests for GraphEdge modulation methods (weight, steering, decay).

use super::*;
use crate::marblestone::{Domain, EdgeType};
use uuid::Uuid;

// =========================================================================
// get_modulated_weight() Tests
// =========================================================================

#[test]
fn test_get_modulated_weight_applies_nt() {
    let edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    // General NT: excitatory=0.5, inhibitory=0.2, modulatory=0.3
    // Base weight for Semantic: 0.5
    let modulated = edge.get_modulated_weight();
    assert!(
        (modulated - 0.138).abs() < 0.001,
        "Expected ~0.138, got {}",
        modulated
    );
}

#[test]
fn test_get_modulated_weight_applies_steering_positive() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.steering_reward = 1.0;
    let modulated = edge.get_modulated_weight();
    assert!(
        modulated > 0.138,
        "Positive steering should increase weight"
    );
}

#[test]
fn test_get_modulated_weight_applies_steering_negative() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.steering_reward = -1.0;
    let modulated = edge.get_modulated_weight();
    assert!(
        modulated < 0.138,
        "Negative steering should decrease weight"
    );
}

#[test]
fn test_get_modulated_weight_always_in_range() {
    for domain in Domain::all() {
        for edge_type in EdgeType::all() {
            for sr in [-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
                let mut edge = GraphEdge::new(Uuid::new_v4(), Uuid::new_v4(), edge_type, domain);
                edge.steering_reward = sr;
                let modulated = edge.get_modulated_weight();
                assert!(
                    (0.0..=1.0).contains(&modulated),
                    "Out of range: domain={:?}, edge_type={:?}, sr={}, modulated={}",
                    domain,
                    edge_type,
                    sr,
                    modulated
                );
            }
        }
    }
}

#[test]
fn test_get_modulated_weight_no_nan() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.weight = 0.0;
    edge.steering_reward = 0.0;
    let modulated = edge.get_modulated_weight();
    assert!(!modulated.is_nan(), "Should not produce NaN");
}

// =========================================================================
// apply_steering_reward() Tests
// =========================================================================

#[test]
fn test_apply_steering_reward_adds() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.apply_steering_reward(0.3);
    assert_eq!(edge.steering_reward, 0.3);
    edge.apply_steering_reward(0.2);
    assert_eq!(edge.steering_reward, 0.5);
}

#[test]
fn test_apply_steering_reward_clamps_positive() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.apply_steering_reward(0.8);
    edge.apply_steering_reward(0.5);
    assert_eq!(edge.steering_reward, 1.0);
}

#[test]
fn test_apply_steering_reward_clamps_negative() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.apply_steering_reward(-0.8);
    edge.apply_steering_reward(-0.5);
    assert_eq!(edge.steering_reward, -1.0);
}

#[test]
fn test_apply_steering_reward_handles_negative() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.apply_steering_reward(-0.5);
    assert_eq!(edge.steering_reward, -0.5);
}

#[test]
fn test_apply_steering_reward_mixed() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.apply_steering_reward(0.5);
    edge.apply_steering_reward(-0.3);
    assert!((edge.steering_reward - 0.2).abs() < 0.001);
}

// =========================================================================
// decay_steering() Tests
// =========================================================================

#[test]
fn test_decay_steering_multiplies() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.steering_reward = 1.0;
    edge.decay_steering(0.5);
    assert_eq!(edge.steering_reward, 0.5);
}

#[test]
fn test_decay_steering_to_zero() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.steering_reward = 0.5;
    edge.decay_steering(0.0);
    assert_eq!(edge.steering_reward, 0.0);
}

#[test]
fn test_decay_steering_negative() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.steering_reward = -1.0;
    edge.decay_steering(0.5);
    assert_eq!(edge.steering_reward, -0.5);
}

#[test]
fn test_decay_steering_preserves_sign() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.steering_reward = -0.8;
    edge.decay_steering(0.5);
    assert!(edge.steering_reward < 0.0);
}

#[test]
fn test_decay_steering_multiple_times() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.steering_reward = 1.0;
    edge.decay_steering(0.5);
    edge.decay_steering(0.5);
    assert_eq!(edge.steering_reward, 0.25);
}

#[test]
fn test_decay_steering_factor_one_preserves() {
    let mut edge = GraphEdge::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        EdgeType::Semantic,
        Domain::General,
    );
    edge.steering_reward = 0.7;
    edge.decay_steering(1.0);
    assert_eq!(edge.steering_reward, 0.7);
}
