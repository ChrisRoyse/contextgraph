// Inspired by ruvnet/RuVector crates/ruvector-graph-transformer/src/temporal.rs
// at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.

use super::*;

#[test]
fn strict_mask_allows_only_past_tokens() {
    let mask = build_causal_mask(&[10, 20, 30], MaskStrategy::Strict).unwrap();
    assert_eq!(
        mask,
        vec![
            false, false, false, //
            true, false, false, //
            true, true, false,
        ]
    );
}

#[test]
fn causal_with_self_allows_past_and_self() {
    let mask = build_causal_mask(&[10, 20, 30], MaskStrategy::CausalWithSelf).unwrap();
    assert_eq!(
        mask,
        vec![
            true, false, false, //
            true, true, false, //
            true, true, true,
        ]
    );
}

#[test]
fn time_window_limits_attention_cone() {
    let mask = build_causal_mask(
        &[10, 20, 35, 50],
        MaskStrategy::TimeWindow { window_size: 15 },
    )
    .unwrap();
    assert_eq!(
        mask,
        vec![
            true, false, false, false, //
            true, true, false, false, //
            false, true, true, false, //
            false, false, true, true,
        ]
    );
}

#[test]
fn apply_mask_sets_forbidden_scores_to_negative_infinity() {
    let mask = build_causal_mask(&[10, 20], MaskStrategy::CausalWithSelf).unwrap();
    let mut scores = vec![0.1, 0.2, 0.3, 0.4];
    apply_mask_to_scores(&mut scores, &mask, 2).unwrap();
    assert_eq!(scores, vec![0.1, f32::NEG_INFINITY, 0.3, 0.4]);
}

#[test]
fn apply_mask_rejects_non_finite_scores() {
    let mask = build_causal_mask(&[10, 20], MaskStrategy::CausalWithSelf).unwrap();
    let mut scores = vec![0.1, f32::INFINITY, 0.3, 0.4];
    let before = scores.clone();
    let err = apply_mask_to_scores(&mut scores, &mask, 2).expect_err("+inf must reject");
    assert_eq!(err.code(), "CGSOLVER_NUMERICAL_INVARIANT");
    assert_eq!(
        scores, before,
        "non-finite rejection must not mutate scores"
    );
}

#[test]
fn apply_mask_rejects_shape_mismatch() {
    let mask = build_causal_mask(&[10, 20], MaskStrategy::CausalWithSelf).unwrap();
    let mut scores = vec![0.1, 0.2, 0.3];
    let err = apply_mask_to_scores(&mut scores, &mask, 2).expect_err("short scores reject");
    assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
}

#[test]
fn retrocausal_safety_rejects_future_attention() {
    let err = retrocausal_safety_check(&[10, 20, 30], 1, 2, MaskStrategy::CausalWithSelf)
        .expect_err("query at t=20 cannot attend to key at t=30");
    assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
}

#[test]
fn retrocausal_safety_accepts_allowed_attention() {
    retrocausal_safety_check(&[10, 20, 30], 2, 1, MaskStrategy::CausalWithSelf).unwrap();
    retrocausal_safety_check(&[10, 20, 30], 2, 2, MaskStrategy::CausalWithSelf).unwrap();
}

#[test]
fn strict_retrocausal_safety_rejects_self_attention() {
    let err = retrocausal_safety_check(&[10, 20, 30], 2, 2, MaskStrategy::Strict)
        .expect_err("strict mask forbids self-attention");
    assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
}

#[test]
fn empty_timestamps_reject() {
    let err = build_causal_mask(&[], MaskStrategy::CausalWithSelf)
        .expect_err("empty mask input must reject");
    assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
}

#[test]
fn softmax_assigns_zero_to_masked_negative_infinity() {
    let probs = softmax_row(&[1.0, f32::NEG_INFINITY, 2.0]).unwrap();
    assert_eq!(probs[1], 0.0);
    assert!((probs[0] + probs[2] - 1.0).abs() < 1e-6);
}

#[test]
fn softmax_rejects_fully_masked_row() {
    let err = softmax_row(&[f32::NEG_INFINITY, f32::NEG_INFINITY])
        .expect_err("fully masked row must reject");
    assert_eq!(err.code(), "CGSOLVER_NUMERICAL_INVARIANT");
}

#[test]
fn softmax_rejects_positive_infinity() {
    let err = softmax_row(&[0.0, f32::INFINITY]).expect_err("+inf must reject");
    assert_eq!(err.code(), "CGSOLVER_NUMERICAL_INVARIANT");
}
