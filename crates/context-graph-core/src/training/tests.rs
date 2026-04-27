//! Unit tests for the training-record types and computation helpers.
//!
//! Covers:
//! - Cross-correlation count, ordering, bounds
//! - Group alignment mapping (with explicit math identities per spec)
//! - Bincode roundtrip of the full TrainingRecord
//! - Topic-profile fallback derivation
//! - Temporal label extraction (known date → known bucket)

use chrono::{DateTime, TimeZone, Utc};
use uuid::Uuid;

use super::*;
use crate::teleological::synergy_matrix::SynergyMatrix;
use crate::teleological::types::NUM_EMBEDDERS;
use crate::types::fingerprint::{SemanticFingerprint, SparseVector, TeleologicalFingerprint};

// =============================================================================
// Cross-correlation tests
// =============================================================================

#[test]
fn cross_correlations_has_exactly_91_entries() {
    let profile = [1.0f32; NUM_EMBEDDERS];
    let synergy = SynergyMatrix::with_base_synergies();
    let cc = compute_cross_correlations(&profile, &synergy);
    assert_eq!(cc.len(), NUM_CROSS_CORRELATIONS, "expected C(14,2) = 91");
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
    // Only idx 0 and idx 4 are non-zero. All non-(0,4) pairs must be zero
    // because their product contains at least one zero. Pair (0,4) must be
    // nonzero.
    //
    // Flat-index layout: for i in 0..NUM_EMBEDDERS, for j in i+1..NUM_EMBEDDERS.
    // For i=0: 13 pairs (j=1..=13). Offset of (0, j) = j - 1.
    // Pair (0,4) therefore lives at flat index 3.
    let mut profile = [0.0f32; NUM_EMBEDDERS];
    profile[0] = 1.0;
    profile[4] = 1.0;
    let synergy = SynergyMatrix::with_base_synergies();
    let cc = compute_cross_correlations(&profile, &synergy);

    // Every flat index we know does NOT cross (0,4) must be zero.
    for flat_idx in 0..NUM_CROSS_CORRELATIONS {
        let (i, j) = flat_index_to_pair(flat_idx);
        let is_pair_0_4 = (i == 0 && j == 4) || (i == 4 && j == 0);
        if !is_pair_0_4 {
            assert_eq!(
                cc[flat_idx], 0.0,
                "flat {} = pair ({},{}) should be zero, got {}",
                flat_idx, i, j, cc[flat_idx],
            );
        }
    }
    assert!(
        cc[3] > 0.0,
        "pair (0,4) at flat index 3 must be nonzero (got {})",
        cc[3],
    );
}

/// Invert the canonical flat layout:
/// for i in 0..NUM_EMBEDDERS, for j in (i+1)..NUM_EMBEDDERS.
fn flat_index_to_pair(flat: usize) -> (usize, usize) {
    let mut remaining = flat;
    for i in 0..NUM_EMBEDDERS {
        let pairs_at_i = NUM_EMBEDDERS - i - 1;
        if remaining < pairs_at_i {
            return (i, i + 1 + remaining);
        }
        remaining -= pairs_at_i;
    }
    panic!("flat index {} out of range", flat);
}

// =============================================================================
// Group alignment math identities
// =============================================================================

#[test]
fn group_alignments_length_is_six() {
    let profile = [0.0f32; NUM_EMBEDDERS];
    let ga = compute_group_alignments(&profile);
    assert_eq!(ga.len(), NUM_GROUP_ALIGNMENTS);
}

/// Implementation group = E6 (index 5) only. So the group value equals the
/// single topic-profile slot verbatim.
#[test]
fn math_identity_implementation_equals_topic_profile_slot_5() {
    let mut profile = [0.0f32; NUM_EMBEDDERS];
    profile[5] = 0.73;
    let ga = compute_group_alignments(&profile);
    assert!(
        (ga[5] - 0.73).abs() < 1e-5,
        "Implementation should equal topic_profile[5]=0.73, got {}",
        ga[5],
    );
}

/// Qualitative = (E10 + E11) / 2. With E11 disabled (index 10 = 0.0), a
/// non-zero E10=0.8 (index 9) yields Qualitative=0.4.
#[test]
fn math_identity_qualitative_halves_e10_when_e11_disabled() {
    let mut profile = [0.0f32; NUM_EMBEDDERS];
    profile[9] = 0.8; // E10
    profile[10] = 0.0; // E11 disabled
    let ga = compute_group_alignments(&profile);
    assert!(
        (ga[4] - 0.4).abs() < 1e-5,
        "Qualitative should be (E10+E11)/2 = 0.4, got {}",
        ga[4],
    );
}

#[test]
fn math_identity_factual_averages_e1_e12_e13() {
    // Factual = (E1 + E12 + E13 + E14) / 4 = (0.9 + 0.6 + 0.3 + 0.6) / 4 = 0.6
    // E14 was added to the factual group in parallel with E1/E12/E13.
    let mut profile = [0.0f32; NUM_EMBEDDERS];
    profile[0] = 0.9;
    profile[11] = 0.6;
    profile[12] = 0.3;
    profile[13] = 0.6;
    let ga = compute_group_alignments(&profile);
    assert!(
        (ga[0] - 0.6).abs() < 1e-5,
        "Factual should be (0.9+0.6+0.3+0.6)/4 = 0.6, got {}",
        ga[0],
    );
}

// =============================================================================
// Topic profile fallback
// =============================================================================

#[test]
fn topic_profile_stored_is_returned_verbatim() {
    let fp = SemanticFingerprint::stub();
    let mut stored = [0.0f32; NUM_EMBEDDERS];
    for (i, v) in stored.iter_mut().enumerate() {
        *v = i as f32 * 0.05;
    }
    let got = topic_profile_or_fallback(Some(stored), &fp);
    assert_eq!(got, stored);
}

#[test]
fn topic_profile_fallback_from_empty_fingerprint_is_all_zero() {
    let fp = empty_semantic_fingerprint();
    let got = topic_profile_or_fallback(None, &fp);
    for (idx, v) in got.iter().enumerate() {
        assert!(v.abs() < 1e-6, "profile[{}] = {} should be zero", idx, v);
    }
}

#[test]
fn topic_profile_fallback_with_unit_vectors_peaks_at_used_slots() {
    let unit: Vec<f32> = vec![1.0, 0.0, 0.0, 0.0];
    let mut fp = empty_semantic_fingerprint();
    fp.e1_semantic = unit.clone();
    fp.e5_causal_as_cause = unit.clone();
    fp.e14_bge_m3_dense = unit;
    let got = topic_profile_or_fallback(None, &fp);
    assert!((got[0] - 1.0).abs() < 1e-5);
    assert!((got[4] - 1.0).abs() < 1e-5);
    assert!((got[13] - 1.0).abs() < 1e-5);
    assert_eq!(got[1], 0.0);
    assert_eq!(got[11], 0.0);
}

// =============================================================================
// Bincode roundtrip
// =============================================================================

#[test]
fn training_record_bincode_roundtrip_full() {
    let record = sample_record();
    let bytes = bincode::serialize(&record).expect("serialize");
    let back: TrainingRecord = bincode::deserialize(&bytes).expect("deserialize");
    assert_eq!(back.memory_id, record.memory_id);
    assert_eq!(back.content, record.content);
    assert_eq!(back.cross_correlations.len(), NUM_CROSS_CORRELATIONS);
    assert_eq!(back.cross_correlations, record.cross_correlations);
    assert_eq!(back.topic_profile, record.topic_profile);
    assert_eq!(back.group_alignments, record.group_alignments);
    assert_eq!(back.outgoing_edges.len(), record.outgoing_edges.len());
    assert_eq!(back.incoming_edges.len(), record.incoming_edges.len());
    assert_eq!(back.causal_effects.len(), record.causal_effects.len());
    assert_eq!(back.causal_causes.len(), record.causal_causes.len());
    assert_eq!(back.knn_neighbors.len(), NUM_EMBEDDERS);
    assert_eq!(back.content_hash, record.content_hash);
    assert_eq!(
        back.temporal_labels.is_some(),
        record.temporal_labels.is_some()
    );
    if let (Some(a), Some(b)) = (&back.temporal_labels, &record.temporal_labels) {
        assert_eq!(a.periodic_bucket, b.periodic_bucket);
        assert_eq!(a.stored_hour_utc, b.stored_hour_utc);
    }
    assert!(back.tucker_core.is_none());
    assert_eq!(back.edge_type_distribution, record.edge_type_distribution);
    // CausalLabel fields
    assert_eq!(
        back.causal_effects[0].rel_id,
        record.causal_effects[0].rel_id
    );
    assert_eq!(
        back.causal_effects[0].mechanism_type,
        record.causal_effects[0].mechanism_type
    );
}

// =============================================================================
// v2: edge_type_distribution
// =============================================================================

/// Build a `TrainingRecord` with 5 outgoing edges (2 SemSim + 2 CodeRelated +
/// 1 CausalChain) and verify the 8-dim distribution lines up exactly.
#[test]
fn edge_type_distribution_sum_matches_outgoing_edges() {
    use crate::graph_linking::GraphLinkEdgeType;

    let mut record = sample_record();

    let mk_outgoing = |edge_type_u8: u8| TrainingEdge {
        edge_type: edge_type_u8,
        peer_id: Uuid::new_v4(),
        weight: 0.7,
        direction: 0,
        agreement_count: 1,
        embedder_scores: [0.0f32; NUM_EMBEDDERS],
    };

    record.outgoing_edges = vec![
        mk_outgoing(GraphLinkEdgeType::SemanticSimilar.as_u8()),
        mk_outgoing(GraphLinkEdgeType::SemanticSimilar.as_u8()),
        mk_outgoing(GraphLinkEdgeType::CodeRelated.as_u8()),
        mk_outgoing(GraphLinkEdgeType::CodeRelated.as_u8()),
        mk_outgoing(GraphLinkEdgeType::CausalChain.as_u8()),
    ];

    // Build distribution manually from the TrainingEdge u8 indices.
    let mut dist = [0u32; NUM_EDGE_TYPE_DISTRIBUTION];
    for e in &record.outgoing_edges {
        dist[e.edge_type as usize] = dist[e.edge_type as usize].saturating_add(1);
    }
    record.edge_type_distribution = dist;

    let expected: [u32; NUM_EDGE_TYPE_DISTRIBUTION] = [2, 2, 0, 1, 0, 0, 0, 0];
    assert_eq!(
        record.edge_type_distribution, expected,
        "distribution must be [2,2,0,1,0,0,0,0] for 2 SemSim + 2 CodeRelated + 1 CausalChain"
    );
    assert_eq!(
        record.edge_type_distribution.iter().sum::<u32>(),
        5,
        "distribution sum must equal the number of outgoing edges"
    );
    assert_eq!(
        record.edge_type_distribution.iter().sum::<u32>() as usize,
        record.outgoing_edges.len(),
        "sum must equal outgoing_edges.len()"
    );
}

/// Bincode roundtrip a record with non-zero distribution, asserting field
/// equality post-roundtrip.
#[test]
fn edge_type_distribution_bincode_roundtrip() {
    let mut record = sample_record();
    record.edge_type_distribution = [2, 2, 0, 1, 0, 0, 0, 0];

    let bytes = bincode::serialize(&record).expect("serialize");
    let back: TrainingRecord = bincode::deserialize(&bytes).expect("deserialize");

    assert_eq!(
        back.edge_type_distribution, record.edge_type_distribution,
        "edge_type_distribution must roundtrip exactly through bincode"
    );
    assert_eq!(back.edge_type_distribution[0], 2);
    assert_eq!(back.edge_type_distribution[1], 2);
    assert_eq!(back.edge_type_distribution[3], 1);
    assert_eq!(back.edge_type_distribution.iter().sum::<u32>(), 5);
}

// =============================================================================
// Temporal tests (Phase 5)
// =============================================================================

#[test]
fn temporal_labels_saturday_morning_yields_weekend_morning_bucket() {
    // 2026-02-28 is a Saturday. 09:00 UTC → Morning window.
    let fp = fingerprint_with_created_at(Utc.with_ymd_and_hms(2026, 2, 28, 9, 0, 0).unwrap());
    let labels = extract_temporal_labels(&fp, None, None, None, Utc::now());
    assert_eq!(labels.periodic_bucket, PeriodicBucket::WeekendMorning);
    assert_eq!(labels.stored_hour_utc, 9);
    assert_eq!(labels.stored_day_of_week, 6); // Saturday = 6 under Monday-first
    assert_eq!(labels.stored_month, 2);
}

#[test]
fn temporal_labels_tuesday_afternoon_yields_weekday_afternoon_bucket() {
    // 2026-03-03 is a Tuesday. 14:30 UTC → Weekday afternoon.
    let fp = fingerprint_with_created_at(Utc.with_ymd_and_hms(2026, 3, 3, 14, 30, 0).unwrap());
    let labels = extract_temporal_labels(&fp, None, None, None, Utc::now());
    assert_eq!(labels.periodic_bucket, PeriodicBucket::WeekdayAfternoon);
    assert_eq!(labels.stored_day_of_week, 2);
}

#[test]
fn temporal_labels_age_matches_export_now_difference() {
    let stored = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let export_now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 1, 30).unwrap(); // +90s
    let fp = fingerprint_with_created_at(stored);
    let labels = extract_temporal_labels(&fp, None, None, None, export_now);
    assert_eq!(labels.age_seconds_at_export, 90);
}

#[test]
fn temporal_labels_relative_position_from_session_numbers() {
    let fp = fingerprint_with_created_at(Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap());
    let labels = extract_temporal_labels(&fp, None, Some(3), Some(10), Utc::now());
    assert_eq!(labels.session_sequence, Some(3));
    assert_eq!(labels.session_total, Some(10));
    let rel = labels.relative_position.expect("should be Some");
    assert!((rel - 0.3).abs() < 1e-5);
}

#[test]
fn temporal_labels_relative_position_none_when_session_total_missing() {
    let fp = fingerprint_with_created_at(Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap());
    let labels = extract_temporal_labels(&fp, None, Some(3), None, Utc::now());
    assert!(labels.relative_position.is_none());
}

#[test]
fn temporal_labels_norms_reflect_populated_vectors() {
    let mut fp = fingerprint_with_created_at(Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap());
    // Give E2 a known-norm vector: sqrt(4 * 0.25) = 1.0
    fp.semantic.e2_temporal_recent = vec![0.5, 0.5, 0.5, 0.5];
    let labels = extract_temporal_labels(&fp, None, None, None, Utc::now());
    assert!((labels.e2_recency_norm - 1.0).abs() < 1e-5);
    assert_eq!(labels.e3_periodic_norm, 0.0);
    assert_eq!(labels.e4_positional_norm, 0.0);
}

#[test]
fn periodic_bucket_all_eight_variants_reachable() {
    // Smoke-check each hour boundary against a known weekday/weekend.
    let weekday = 3u8; // Wednesday
    let weekend = 7u8; // Sunday
    assert_eq!(
        PeriodicBucket::from_dow_hour(weekday, 6),
        PeriodicBucket::WeekdayMorning
    );
    assert_eq!(
        PeriodicBucket::from_dow_hour(weekday, 13),
        PeriodicBucket::WeekdayAfternoon
    );
    assert_eq!(
        PeriodicBucket::from_dow_hour(weekday, 19),
        PeriodicBucket::WeekdayEvening
    );
    assert_eq!(
        PeriodicBucket::from_dow_hour(weekday, 2),
        PeriodicBucket::WeekdayNight
    );
    assert_eq!(
        PeriodicBucket::from_dow_hour(weekend, 7),
        PeriodicBucket::WeekendMorning
    );
    assert_eq!(
        PeriodicBucket::from_dow_hour(weekend, 15),
        PeriodicBucket::WeekendAfternoon
    );
    assert_eq!(
        PeriodicBucket::from_dow_hour(weekend, 20),
        PeriodicBucket::WeekendEvening
    );
    assert_eq!(
        PeriodicBucket::from_dow_hour(weekend, 23),
        PeriodicBucket::WeekendNight
    );
}

// =============================================================================
// Test fixtures
// =============================================================================

fn sample_record() -> TrainingRecord {
    let mut profile = [0.0f32; NUM_EMBEDDERS];
    profile[0] = 0.9;
    profile[6] = 0.8;
    let synergy = SynergyMatrix::with_base_synergies();
    let cc = compute_cross_correlations(&profile, &synergy);
    let groups = compute_group_alignments(&profile);

    let stored_at = Utc.with_ymd_and_hms(2026, 2, 28, 9, 0, 0).unwrap();
    let temporal = Some(TemporalLabels {
        stored_at,
        stored_hour_utc: 9,
        stored_day_of_week: 6,
        stored_month: 2,
        age_seconds_at_export: 42,
        session_sequence: Some(3),
        session_total: Some(10),
        relative_position: Some(0.3),
        periodic_bucket: PeriodicBucket::WeekendMorning,
        e2_recency_norm: 1.0,
        e3_periodic_norm: 0.5,
        e4_positional_norm: 0.7,
    });

    TrainingRecord {
        memory_id: Uuid::new_v4(),
        content: "unit test content".into(),
        importance: 0.7,
        created_at: stored_at,
        session_id: Some("test-session".into()),
        source_type: Some("Manual".into()),
        source_path: None,
        content_hash: Some([7u8; 32]),
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
        e14_bge_m3_dense: Vec::new(),
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
            related_memory_id: Uuid::nil(),
            rel_id: Uuid::new_v4(),
            description: "A -> B. mediated via C".into(),
            direction: "cause".into(),
            confidence: 0.82,
            mechanism_type: Some("mediated".into()),
        }],
        causal_causes: Vec::new(),
        topic_memberships: Vec::new(),
        temporal_labels: temporal,
        tucker_core: None,
        edge_type_distribution: [0u32; NUM_EDGE_TYPE_DISTRIBUTION],
    }
}

fn empty_semantic_fingerprint() -> SemanticFingerprint {
    SemanticFingerprint {
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
        e14_bge_m3_dense: Vec::new(),
    }
}

fn fingerprint_with_created_at(created_at: DateTime<Utc>) -> TeleologicalFingerprint {
    TeleologicalFingerprint {
        id: Uuid::new_v4(),
        semantic: empty_semantic_fingerprint(),
        content_hash: [0u8; 32],
        created_at,
        last_updated: created_at,
        access_count: 0,
        importance: 0.5,
        last_accessed_at: created_at,
        e6_sparse: None,
    }
}
