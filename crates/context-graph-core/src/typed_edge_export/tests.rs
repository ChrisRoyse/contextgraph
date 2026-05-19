//! Bincode roundtrip tests for the typed-edge export wire types.

use super::*;

#[test]
fn verdict_round_trips_through_bincode() {
    for v in [
        LLMVerdict::Valid,
        LLMVerdict::Invalid,
        LLMVerdict::Reclassify { new_edge_type: 3 },
    ] {
        let bytes = bincode::serialize(&v).expect("serialize verdict");
        let back: LLMVerdict = bincode::deserialize(&bytes).expect("deserialize verdict");
        assert_eq!(v, back);
    }
}

#[test]
fn record_round_trips_with_full_content() {
    let rec = TypedEdgeTrainingRecord {
        source_memory_id: Uuid::new_v4(),
        target_memory_id: Uuid::new_v4(),
        edge_type: 3,
        edge_type_name: "causal_chain".into(),
        weight: 0.72,
        direction: 1,
        embedder_scores: {
            let mut s = [0f32; NUM_EMBEDDERS];
            s[0] = 0.8;
            s[4] = 0.72;
            s
        },
        agreement_count: 1,
        agreeing_embedders: 1u16 << 4,
        source_content: "A causes B.".into(),
        target_content: "B happens after A.".into(),
        source_session_id: Some("sess-1".into()),
        target_session_id: Some("sess-1".into()),
        source_type: Some("HookDescription".into()),
        target_type: Some("HookDescription".into()),
        mechanism_type: Some("direct".into()),
        llm_validation: Some(LLMValidationSummary {
            validated_at: Utc::now(),
            verdict: LLMVerdict::Valid,
            confidence: 0.9,
            rationale: "Direct cause-effect stated.".into(),
            validator_version: "deterministic-validator-v1@2026-05".into(),
        }),
        exported_at: Utc::now(),
        exporter_version: "typed_edge_export_v1".into(),
    };
    let bytes = bincode::serialize(&rec).expect("serialize record");
    let back: TypedEdgeTrainingRecord = bincode::deserialize(&bytes).expect("deserialize record");
    assert_eq!(rec, back);
}

#[test]
fn record_round_trips_without_content_or_validation() {
    let rec = TypedEdgeTrainingRecord {
        source_memory_id: Uuid::new_v4(),
        target_memory_id: Uuid::new_v4(),
        edge_type: 0,
        edge_type_name: "semantic_similar".into(),
        weight: 0.8,
        direction: 0,
        embedder_scores: [0.0; NUM_EMBEDDERS],
        agreement_count: 1,
        agreeing_embedders: 1,
        source_content: String::new(),
        target_content: String::new(),
        source_session_id: None,
        target_session_id: None,
        source_type: None,
        target_type: None,
        mechanism_type: None,
        llm_validation: None,
        exported_at: Utc::now(),
        exporter_version: "typed_edge_export_v1".into(),
    };
    let bytes = bincode::serialize(&rec).expect("serialize record");
    let back: TypedEdgeTrainingRecord = bincode::deserialize(&bytes).expect("deserialize record");
    assert_eq!(rec, back);
}
