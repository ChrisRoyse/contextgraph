//! Bincode roundtrip tests for [`LLMEdgeValidation`].
//!
//! Covers the happy path (full payload roundtrip equality) and a malformed-
//! payload edge case (truncated bincode bytes must surface a deserialize
//! error rather than silently decoding garbage).

use super::*;
use crate::typed_edge_export::LLMVerdict;

fn sample_validation(verdict: LLMVerdict) -> LLMEdgeValidation {
    LLMEdgeValidation {
        validated_at: Utc::now(),
        verdict,
        confidence: 0.87,
        rationale: "LLM determined the link is direct cause-effect.".into(),
        auto_derived_weight: 0.55,
        validator_version: "external-validator@2026-04-14".into(),
        prompt_hash: [0xABu8; 32],
    }
}

#[test]
fn validation_round_trips_with_valid_verdict() {
    let v = sample_validation(LLMVerdict::Valid);
    let bytes = bincode::serialize(&v).expect("serialize");
    let back: LLMEdgeValidation = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(v, back);
    assert_eq!(back.verdict.as_str(), "valid");
    assert_eq!(back.prompt_hash.len(), 32);
    assert!((back.confidence - 0.87).abs() < f32::EPSILON);
}

#[test]
fn validation_round_trips_with_invalid_verdict() {
    let v = sample_validation(LLMVerdict::Invalid);
    let bytes = bincode::serialize(&v).expect("serialize");
    let back: LLMEdgeValidation = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(v, back);
    assert_eq!(back.verdict.as_str(), "invalid");
}

#[test]
fn validation_round_trips_with_reclassify_verdict() {
    let v = sample_validation(LLMVerdict::Reclassify { new_edge_type: 5 });
    let bytes = bincode::serialize(&v).expect("serialize");
    let back: LLMEdgeValidation = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(v, back);
    assert_eq!(back.verdict.as_str(), "reclassify");
    if let LLMVerdict::Reclassify { new_edge_type } = back.verdict {
        assert_eq!(new_edge_type, 5);
    } else {
        panic!("reclassify variant lost on roundtrip");
    }
}

#[test]
fn validation_round_trips_zero_confidence_and_zero_weight_boundary() {
    let mut v = sample_validation(LLMVerdict::Invalid);
    v.confidence = 0.0;
    v.auto_derived_weight = 0.0;
    let bytes = bincode::serialize(&v).expect("serialize");
    let back: LLMEdgeValidation = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(v, back);
    assert_eq!(back.confidence, 0.0);
    assert_eq!(back.auto_derived_weight, 0.0);
}

/// Truncating the bincode-encoded payload must surface a deserialization
/// error — not silently decode partial garbage.
#[test]
fn malformed_payload_truncated_fails_to_deserialize() {
    let v = sample_validation(LLMVerdict::Valid);
    let bytes = bincode::serialize(&v).expect("serialize");
    assert!(bytes.len() > 16, "sample must produce non-trivial bytes");
    let truncated = &bytes[..bytes.len() / 2];
    let result: Result<LLMEdgeValidation, _> = bincode::deserialize(truncated);
    assert!(
        result.is_err(),
        "truncated bincode must fail to deserialize, got Ok"
    );
}
