//! Unit tests for the training-record types and computation helpers.
//!
//! Covers: cross-correlation count/ordering, group alignment mapping,
//! bincode roundtrip, and topic-profile derivation edge cases.

use super::*;
use crate::teleological::synergy_matrix::SynergyMatrix;

#[test]
fn cross_correlations_has_exactly_78_entries() {
    let profile = [1.0f32; NUM_EMBEDDERS];
    let synergy = SynergyMatrix::with_base_synergies();
    let cc = compute_cross_correlations(&profile, &synergy);
    assert_eq!(cc.len(), NUM_CROSS_CORRELATIONS, "expected C(13,2) = 78");
}

#[test]
fn cross_correlations_are_bounded_unit_interval() {
    let profile = [0.5f32; NUM_EMBEDDERS];
    let synergy = SynergyMatrix::with_base_synergies();
    let cc = compute_cross_correlations(&profile, &synergy);
    for (idx, &v) in cc.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(&v),
            "cross_correlation[{}] = {} outside [0, 1]",
            idx,
            v,
        );
    }
}

#[test]
fn cross_correlations_ordering_matches_pair_convention() {
    // Use sentinel profile: only E1 (idx 0) and E5 (idx 4) have nonzero alignment.
    let mut profile = [0.0f32; NUM_EMBEDDERS];
    profile[0] = 1.0;
    profile[4] = 1.0;
    let synergy = SynergyMatrix::with_base_synergies();
    let cc = compute_cross_correlations(&profile, &synergy);

    // Pair (0, 4) lives at index 0*12 - 0 + (4 - 0 - 1) = 3 in the flat layout
    // (pairs starting at i=0: 12 pairs — (0,1), (0,2), ..., (0,12) — index 3 is (0,4)).
    assert!(
        cc[3] > 0.0,
        "(0,4) pair at flat index 3 should be nonzero when both alignments are 1.0 (got {})",
        cc[3],
    );
    // All-zero pairs must be zero.
    assert_eq!(cc[0], 0.0, "pair (0,1) has alignment 1.0 * 0.0 * synergy = 0");
}

#[test]
fn group_alignments_match_groups_spec() {
    // Factual = E1(0) + E12(11) + E13(12) averaged.
    // Temporal = E2(1) + E3(2). Causal = E4(3) + E7(6).
    // Relational = E5(4) + E8(7) + E9(8). Qualitative = E10(9) + E11(10). Implementation = E6(5).
    let mut profile = [0.0f32; NUM_EMBEDDERS];
    profile[0] = 0.9;
    profile[11] = 0.6;
    profile[12] = 0.3; // Factual mean = 0.6
    profile[1] = 0.8;
    profile[2] = 0.4; // Temporal mean = 0.6
    profile[5] = 0.75; // Implementation = 0.75

    let groups = compute_group_alignments(&profile);
    assert!((groups[0] - 0.6).abs() < 1e-5, "Factual = {}", groups[0]);
    assert!((groups[1] - 0.6).abs() < 1e-5, "Temporal = {}", groups[1]);
    assert!((groups[5] - 0.75).abs() < 1e-5, "Implementation = {}", groups[5]);
    assert_eq!(groups.len(), NUM_GROUP_ALIGNMENTS);
}

#[test]
fn training_record_bincode_roundtrip() {
    let record = sample_record();
    let bytes = bincode::serialize(&record).expect("serialize");
    let back: TrainingRecord = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(back.memory_id, record.memory_id);
    assert_eq!(back.content, record.content);
    assert_eq!(back.cross_correlations.len(), NUM_CROSS_CORRELATIONS);
    assert_eq!(back.topic_profile, record.topic_profile);
    assert_eq!(back.outgoing_edges.len(), record.outgoing_edges.len());
    assert_eq!(back.causal_effects.len(), record.causal_effects.len());
    assert_eq!(back.knn_neighbors.len(), NUM_EMBEDDERS);
}

#[test]
fn topic_profile_from_empty_fingerprint_is_zero() {
    // All-empty vectors should yield all-zero profile (no panic, no NaN).
    let fp = SemanticFingerprint {
        e1_semantic: Vec::new(),
        e2_temporal_recent: Vec::new(),
        e3_temporal_periodic: Vec::new(),
        e4_temporal_positional: Vec::new(),
        e5_causal_as_cause: Vec::new(),
        e5_causal_as_effect: Vec::new(),
        e5_causal: Vec::new(),
        e6_sparse: SparseVector::empty(),
        e7_code: Vec::new(),
        e8_graph_as_source: Vec::new(),
        e8_graph_as_target: Vec::new(),
        e8_graph: Vec::new(),
        e9_hdc: Vec::new(),
        e10_multimodal_paraphrase: Vec::new(),
        e10_multimodal_as_context: Vec::new(),
        e11_entity: Vec::new(),
        e12_late_interaction: Vec::new(),
        e13_splade: SparseVector::empty(),
    };
    let profile = compute_topic_profile_from_fingerprint(&fp);
    for (idx, v) in profile.iter().enumerate() {
        assert!(v.abs() < 1e-6, "profile[{}] = {} should be zero", idx, v);
    }
}

#[test]
fn topic_profile_handles_unit_normalized_dense_vectors() {
    // A unit-normalized 4-element vector: [1, 0, 0, 0] has L2 norm 1.0 → alignment ≈ 1.0
    let unit = vec![1.0f32, 0.0, 0.0, 0.0];
    let fp = SemanticFingerprint {
        e1_semantic: unit.clone(),
        e2_temporal_recent: Vec::new(),
        e3_temporal_periodic: Vec::new(),
        e4_temporal_positional: Vec::new(),
        e5_causal_as_cause: unit.clone(),
        e5_causal_as_effect: Vec::new(),
        e5_causal: Vec::new(),
        e6_sparse: SparseVector::empty(),
        e7_code: Vec::new(),
        e8_graph_as_source: Vec::new(),
        e8_graph_as_target: Vec::new(),
        e8_graph: Vec::new(),
        e9_hdc: Vec::new(),
        e10_multimodal_paraphrase: Vec::new(),
        e10_multimodal_as_context: Vec::new(),
        e11_entity: Vec::new(),
        e12_late_interaction: Vec::new(),
        e13_splade: SparseVector::empty(),
    };
    let profile = compute_topic_profile_from_fingerprint(&fp);
    assert!((profile[0] - 1.0).abs() < 1e-5);
    assert!((profile[4] - 1.0).abs() < 1e-5);
    assert_eq!(profile[1], 0.0);
}

fn sample_record() -> TrainingRecord {
    let mut profile = [0.0f32; NUM_EMBEDDERS];
    profile[0] = 0.9;
    profile[6] = 0.8;
    let synergy = SynergyMatrix::with_base_synergies();
    let cc = compute_cross_correlations(&profile, &synergy);
    let groups = compute_group_alignments(&profile);

    TrainingRecord {
        memory_id: Uuid::new_v4(),
        content: "unit test content".into(),
        importance: 0.7,
        created_at: Utc::now(),
        session_id: Some("test-session".into()),
        source_type: Some("Manual".into()),
        source_path: None,
        e1_semantic: vec![0.1, 0.2, 0.3],
        e2_temporal_recent: Vec::new(),
        e3_temporal_periodic: Vec::new(),
        e4_temporal_positional: Vec::new(),
        e5_causal_cause: Vec::new(),
        e5_causal_effect: Vec::new(),
        e7_code: Vec::new(),
        e8_graph_source: Vec::new(),
        e8_graph_target: Vec::new(),
        e9_hdc: Vec::new(),
        e10_paraphrase: Vec::new(),
        e10_context: Vec::new(),
        e11_entity: Vec::new(),
        e6_sparse_indices: vec![1, 42, 100],
        e6_sparse_values: vec![0.5, 0.3, 0.2],
        e13_splade_indices: Vec::new(),
        e13_splade_values: Vec::new(),
        e12_token_embeddings: Vec::new(),
        topic_profile: profile,
        cross_correlations: cc,
        group_alignments: groups,
        outgoing_edges: vec![TrainingEdge {
            edge_type: 0,
            peer_id: Uuid::new_v4(),
            weight: 0.85,
            direction: 0,
            agreement_count: 4,
            embedder_scores: [0.1f32; NUM_EMBEDDERS],
        }],
        incoming_edges: Vec::new(),
        knn_neighbors: (0..NUM_EMBEDDERS).map(|_| Vec::new()).collect(),
        causal_effects: vec![CausalLabel {
            related_memory_id: Uuid::new_v4(),
            description: "A produces B".into(),
            direction: "cause".into(),
            confidence: 0.82,
        }],
        causal_causes: Vec::new(),
        topic_memberships: Vec::new(),
    }
}
