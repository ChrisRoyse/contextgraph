//! Pure unit tests for the contrastive pair miner (Phase 3).
//!
//! These tests build synthetic `SemanticFingerprint` instances directly and
//! run the pure-functional miner. Storage-level roundtrip tests live in
//! `context-graph-storage/tests/contrastive_integration.rs`.

use uuid::Uuid;

use crate::contrastive::types::{
    AnomalyKind, ContrastiveError, MiningConfig, DEFAULT_HIGH_THRESHOLD, DEFAULT_LOW_THRESHOLD,
    NUM_ANOMALY_KINDS,
};
use crate::contrastive::{classify_anomaly, mine_pair_from_candidate, similarity_profile};
use crate::teleological::types::NUM_EMBEDDERS;
use crate::types::fingerprint::{SemanticFingerprint, SparseVector};

/// Build a fully-populated fingerprint using the same deterministic seeding
/// pattern the constellation tests use. All dense vectors are L2-normalized.
fn synthetic_fp(seed: u32) -> SemanticFingerprint {
    fn dense(dim: usize, seed: u32) -> Vec<f32> {
        let mut v: Vec<f32> = (0..dim)
            .map(|i| ((i as u32 + seed) % 17) as f32 * 0.01 + 0.01)
            .collect();
        let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if n > 0.0 {
            for x in &mut v {
                *x /= n;
            }
        }
        v
    }
    SemanticFingerprint {
        e1_semantic: dense(1024, seed),
        e2_temporal_recent: dense(512, seed.wrapping_add(1)),
        e3_temporal_periodic: dense(512, seed.wrapping_add(2)),
        e4_temporal_positional: dense(512, seed.wrapping_add(3)),
        e5_causal_as_cause: dense(768, seed.wrapping_add(4)),
        e5_causal_as_effect: Vec::new(),
        e5_causal: Vec::new(),
        e6_sparse: SparseVector {
            indices: vec![1, 2, 3, 4, 5],
            values: vec![0.1, 0.2, 0.3, 0.2, 0.1],
        },
        e7_code: dense(1536, seed.wrapping_add(6)),
        e8_graph_as_source: dense(1024, seed.wrapping_add(7)),
        e8_graph_as_target: Vec::new(),
        e8_graph: Vec::new(),
        e9_hdc: dense(1024, seed.wrapping_add(8)),
        e10_multimodal_paraphrase: dense(768, seed.wrapping_add(9)),
        e10_multimodal_as_context: Vec::new(),
        e11_entity: dense(768, seed.wrapping_add(10)),
        e12_late_interaction: (0..3)
            .map(|t| dense(128, seed.wrapping_add(100 + t as u32)))
            .collect(),
        e13_splade: SparseVector {
            indices: vec![10, 20, 30],
            values: vec![0.5, 0.3, 0.2],
        },
        e14_bge_m3_dense: dense(1024, seed.wrapping_add(13)),
    }
}

fn cfg_default() -> MiningConfig {
    MiningConfig::default()
}

// ---------------------------------------------------------------------------
// Similarity profile
// ---------------------------------------------------------------------------

#[test]
fn identical_fingerprints_yield_all_ones() {
    let fp = synthetic_fp(42);
    let prof = similarity_profile(&fp, &fp);
    assert_eq!(prof.len(), NUM_EMBEDDERS);
    for (i, v) in prof.iter().enumerate() {
        assert!(
            (*v - 1.0).abs() < 1e-3,
            "embedder {} expected ~1.0 on identical fingerprints, got {}",
            i,
            v
        );
    }
}

#[test]
fn identical_fingerprints_give_near_zero_disagreement_and_classify_other() {
    let fp = synthetic_fp(42);
    let prof = similarity_profile(&fp, &fp);
    // There are no "low" entries, so classification must fall through to Other.
    let k = classify_anomaly(&prof, DEFAULT_HIGH_THRESHOLD, DEFAULT_LOW_THRESHOLD);
    assert_eq!(k, AnomalyKind::Other);
    let max_high: f32 = prof
        .iter()
        .filter(|&&v| v > DEFAULT_HIGH_THRESHOLD)
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let min_low: f32 = prof
        .iter()
        .filter(|&&v| v < DEFAULT_LOW_THRESHOLD)
        .copied()
        .fold(f32::INFINITY, f32::min);
    // Either no low entries (min_low is +inf) or disagreement is tiny.
    assert!(min_low.is_infinite() || (max_high - min_low) < 0.1);
}

#[test]
fn profile_entries_are_bounded_in_zero_one() {
    // Unrelated seeds with opposite dense content should still stay in [0, 1].
    let a = synthetic_fp(1);
    let b = synthetic_fp(999);
    let prof = similarity_profile(&a, &b);
    for (i, v) in prof.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(v) && v.is_finite(),
            "embedder {} out of range [0, 1]: got {}",
            i,
            v
        );
    }
}

#[test]
fn orthogonal_e1_yields_src3_midpoint() {
    // Construct two fingerprints whose E1 vectors are orthogonal; everything
    // else gets reused from the same seed so we're only shifting E1.
    let mut a = synthetic_fp(7);
    let mut b = synthetic_fp(7);
    let dim = a.e1_semantic.len();
    let mut va = vec![0.0f32; dim];
    let mut vb = vec![0.0f32; dim];
    va[0] = 1.0;
    vb[1] = 1.0;
    a.e1_semantic = va;
    b.e1_semantic = vb;
    let prof = similarity_profile(&a, &b);
    // Orthogonal → raw cos = 0 → SRC-3 = 0.5.
    assert!(
        (prof[0] - 0.5).abs() < 1e-3,
        "E1 orthogonal should map to ~0.5 under SRC-3, got {}",
        prof[0]
    );
}

// ---------------------------------------------------------------------------
// classify_anomaly
// ---------------------------------------------------------------------------

fn blank_profile() -> [f32; NUM_EMBEDDERS] {
    [0.0; NUM_EMBEDDERS]
}

#[test]
fn high_e1_low_e5_classifies_as_semantic_but_not_causal() {
    let mut p = blank_profile();
    p[0] = 0.8;
    p[4] = 0.1;
    let k = classify_anomaly(&p, 0.6, 0.3);
    assert_eq!(k, AnomalyKind::SemanticButNotCausal);
}

#[test]
fn high_e7_low_e1_classifies_as_code_shape() {
    let mut p = blank_profile();
    p[6] = 0.85;
    p[0] = 0.1;
    let k = classify_anomaly(&p, 0.6, 0.3);
    assert_eq!(k, AnomalyKind::CodeShapeButDifferentIntent);
}

#[test]
fn high_e11_low_e8_classifies_as_entity_shared_but_different_structure() {
    let mut p = blank_profile();
    p[10] = 0.9;
    p[7] = 0.05;
    let k = classify_anomaly(&p, 0.6, 0.3);
    assert_eq!(k, AnomalyKind::EntitySharedButDifferentStructure);
}

#[test]
fn high_e9_low_e1_classifies_as_hdc_robust() {
    let mut p = blank_profile();
    p[8] = 0.9;
    p[0] = 0.1;
    let k = classify_anomaly(&p, 0.6, 0.3);
    assert_eq!(k, AnomalyKind::HdcRobustButSemanticDifferent);
}

#[test]
fn high_e6_low_e10_classifies_as_keyword_but_not_paraphrase() {
    let mut p = blank_profile();
    p[5] = 0.9;
    p[9] = 0.05;
    let k = classify_anomaly(&p, 0.6, 0.3);
    assert_eq!(k, AnomalyKind::KeywordButNotParaphrase);
}

#[test]
fn high_e13_low_e10_also_classifies_as_keyword_but_not_paraphrase() {
    let mut p = blank_profile();
    p[12] = 0.9;
    p[9] = 0.05;
    let k = classify_anomaly(&p, 0.6, 0.3);
    assert_eq!(k, AnomalyKind::KeywordButNotParaphrase);
}

#[test]
fn unnamed_high_low_falls_through_to_other() {
    // Only E2 (temporal) high + E3 low — not covered by a named axis.
    let mut p = blank_profile();
    p[1] = 0.9;
    p[2] = 0.05;
    let k = classify_anomaly(&p, 0.6, 0.3);
    assert_eq!(k, AnomalyKind::Other);
}

// ---------------------------------------------------------------------------
// mine_pair_from_candidate
// ---------------------------------------------------------------------------

#[test]
fn mine_returns_none_below_min_disagreement() {
    let a = synthetic_fp(1);
    let b = synthetic_fp(1);
    let mut cfg = cfg_default();
    // Set an impossibly high bar.
    cfg.min_disagreement = 0.95;
    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();
    let got = mine_pair_from_candidate(id_a, "a", &a, id_b, "b", &b, &cfg);
    assert!(got.is_none());
}

#[test]
fn mine_returns_none_when_kind_filter_excludes() {
    // Construct a forced CodeShape pair: manually set profile through actual
    // fingerprints. We reuse the SemanticButNotCausal trick by constructing a
    // pair whose E1 is nearly identical and E5 is orthogonal; that drops into
    // SemanticButNotCausal. We then request only CodeShape.
    let mut a = synthetic_fp(2);
    let mut b = synthetic_fp(2);
    let dim5 = a.e5_causal_as_cause.len();
    let mut va = vec![0.0f32; dim5];
    let mut vb = vec![0.0f32; dim5];
    va[0] = 1.0;
    vb[1] = 1.0;
    a.e5_causal_as_cause = va;
    b.e5_causal_as_cause = vb;
    // E1 is identical (inherited from seed 2), so E1 sim ~ 1.0.

    // Ensure baseline kind would be SemanticButNotCausal.
    let baseline_profile = similarity_profile(&a, &b);
    assert!(baseline_profile[0] > 0.9); // high E1
                                        // E5 orthogonal dims → SRC-3 = 0.5, but we want low; push via zeroing one side.
    a.e5_causal_as_cause = vec![0.0; dim5];
    b.e5_causal_as_cause = vec![0.0; dim5];
    // Zero-norm → similarity 0.0 (our contract).
    let profile = similarity_profile(&a, &b);
    assert!(profile[4] < 0.1);
    let baseline_kind = classify_anomaly(&profile, 0.6, 0.3);
    assert_eq!(baseline_kind, AnomalyKind::SemanticButNotCausal);

    // Now ask only for CodeShapeButDifferentIntent — mining must return None.
    let mut cfg = cfg_default();
    cfg.kinds = Some(vec![AnomalyKind::CodeShapeButDifferentIntent]);
    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();
    let got = mine_pair_from_candidate(id_a, "a", &a, id_b, "b", &b, &cfg);
    assert!(got.is_none());

    // Sanity: asking for SemanticButNotCausal yields a pair.
    let mut cfg2 = cfg_default();
    cfg2.kinds = Some(vec![AnomalyKind::SemanticButNotCausal]);
    let got2 = mine_pair_from_candidate(id_a, "a", &a, id_b, "b", &b, &cfg2);
    assert!(
        got2.is_some(),
        "expected SemanticButNotCausal pair, got none"
    );
}

#[test]
fn mine_returns_none_when_anchor_equals_negative() {
    let fp = synthetic_fp(4);
    let cfg = cfg_default();
    let id = Uuid::new_v4();
    let got = mine_pair_from_candidate(id, "same", &fp, id, "same", &fp, &cfg);
    assert!(got.is_none());
}

#[test]
fn mine_populates_full_pair_on_success() {
    // Force a SemanticButNotCausal pair: identical E1, zeroed-out E5 on both.
    let mut a = synthetic_fp(3);
    let mut b = synthetic_fp(3);
    let dim5 = a.e5_causal_as_cause.len();
    a.e5_causal_as_cause = vec![0.0; dim5];
    b.e5_causal_as_cause = vec![0.0; dim5];
    // Also force E8 and E10 onto orthogonal enough territory so they drop below
    // low; the synthetic fp has all-same vectors otherwise so they'd sit at 1.0.
    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();
    let cfg = cfg_default();
    let pair = mine_pair_from_candidate(id_a, "anchor text", &a, id_b, "negative text", &b, &cfg)
        .expect("expected a pair");
    assert_eq!(pair.anchor_id, id_a);
    assert_eq!(pair.negative_id, id_b);
    assert_eq!(pair.anchor_text, "anchor text");
    assert_eq!(pair.negative_text, "negative text");
    assert_eq!(pair.similarity_profile.len(), NUM_EMBEDDERS);
    assert!(!pair.high_embedders.is_empty());
    assert!(!pair.low_embedders.is_empty());
    assert!(pair.disagreement_magnitude >= cfg.min_disagreement);
    assert!(pair.disagreement_magnitude.is_finite());
    assert_eq!(pair.generator, super::mining::GENERATOR_TAG);
}

// ---------------------------------------------------------------------------
// AnomalyKind round-trip + parse
// ---------------------------------------------------------------------------

#[test]
fn anomaly_kind_as_u8_round_trips() {
    for (i, v) in AnomalyKind::all().iter().enumerate() {
        assert_eq!(v.as_u8(), i as u8);
        let back = AnomalyKind::from_u8(v.as_u8()).expect("round-trip");
        assert_eq!(&back, v);
    }
    // Guard: count matches advertised NUM_ANOMALY_KINDS.
    assert_eq!(AnomalyKind::all().len() as u8, NUM_ANOMALY_KINDS);
}

#[test]
fn anomaly_kind_from_u8_rejects_unknown() {
    assert!(AnomalyKind::from_u8(99).is_none());
}

#[test]
fn anomaly_kind_parse_round_trips_snake_case() {
    for v in AnomalyKind::all() {
        let parsed = AnomalyKind::parse(v.as_str()).expect("parse");
        assert_eq!(parsed, v);
    }
    assert!(AnomalyKind::parse("not_a_real_kind").is_none());
}

// ---------------------------------------------------------------------------
// MiningConfig validation
// ---------------------------------------------------------------------------

#[test]
fn mining_config_default_validates() {
    assert!(MiningConfig::default().validate().is_ok());
}

#[test]
fn mining_config_rejects_inverted_thresholds() {
    let mut cfg = MiningConfig::default();
    cfg.high_threshold = 0.3;
    cfg.low_threshold = 0.5;
    match cfg.validate() {
        Err(ContrastiveError::InvalidThresholds(lo, hi)) => {
            assert!((lo - 0.5).abs() < 1e-6);
            assert!((hi - 0.3).abs() < 1e-6);
        }
        other => panic!("expected InvalidThresholds, got {:?}", other),
    }
}

#[test]
fn mining_config_rejects_out_of_range_disagreement() {
    let mut cfg = MiningConfig::default();
    cfg.min_disagreement = 1.5;
    assert!(matches!(
        cfg.validate(),
        Err(ContrastiveError::InvalidDisagreement(_))
    ));
}
