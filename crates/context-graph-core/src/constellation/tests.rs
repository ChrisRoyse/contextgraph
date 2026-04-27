//! Pure unit tests for the constellation compiler.
//!
//! These tests build synthetic `SemanticFingerprint` instances directly and
//! run the compiler without going through RocksDB. Storage-level roundtrip
//! tests live in `context-graph-storage/tests/constellation_integration.rs`.

use uuid::Uuid;

use crate::constellation::{
    compile_constellation, score_memory_against_constellation, ConstellationError,
    ConstellationSelector, VectorKind, MIN_CONSTELLATION_MEMBERS,
};
use crate::teleological::synergy_matrix::SynergyMatrix;
use crate::teleological::types::NUM_EMBEDDERS;
use crate::types::fingerprint::{SemanticFingerprint, SparseVector};

/// Build a SemanticFingerprint with all 13 embedder slots populated by a
/// simple pattern keyed on `seed`. Dense vectors are L2-normalized. Sparse
/// and token-level slots get non-empty content. Asymmetric embedders fill
/// the production side used by the constellation compiler (cause / source /
/// paraphrase).
fn synthetic_fp(seed: u32) -> SemanticFingerprint {
    fn dense(dim: usize, seed: u32) -> Vec<f32> {
        let mut v: Vec<f32> = (0..dim)
            .map(|i| ((i as u32 + seed) % 17) as f32 * 0.01)
            .collect();
        let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if n > 0.0 {
            for x in &mut v {
                *x /= n;
            }
        }
        v
    }
    let e1 = dense(1024, seed);
    let e2 = dense(512, seed.wrapping_add(1));
    let e3 = dense(512, seed.wrapping_add(2));
    let e4 = dense(512, seed.wrapping_add(3));
    let e5c = dense(768, seed.wrapping_add(4));
    let e7 = dense(1536, seed.wrapping_add(6));
    let e8s = dense(1024, seed.wrapping_add(7));
    let e9 = dense(1024, seed.wrapping_add(8));
    let e10p = dense(768, seed.wrapping_add(9));
    let e11 = dense(768, seed.wrapping_add(10));
    let e14 = dense(1024, seed.wrapping_add(13));
    let e6 = SparseVector {
        indices: vec![1, 2, 3, 4, 5],
        values: vec![0.1, 0.2, 0.3, 0.2, 0.1],
    };
    let e13 = SparseVector {
        indices: vec![10, 20, 30],
        values: vec![0.5, 0.3, 0.2],
    };
    let tokens: Vec<Vec<f32>> = (0..3)
        .map(|t| dense(128, seed.wrapping_add(100 + t as u32)))
        .collect();

    SemanticFingerprint {
        e1_semantic: e1,
        e2_temporal_recent: e2,
        e3_temporal_periodic: e3,
        e4_temporal_positional: e4,
        e5_causal_as_cause: e5c,
        e5_causal_as_effect: Vec::new(),
        e5_causal: Vec::new(),
        e6_sparse: e6,
        e7_code: e7,
        e8_graph_as_source: e8s,
        e8_graph_as_target: Vec::new(),
        e8_graph: Vec::new(),
        e9_hdc: e9,
        e10_multimodal_paraphrase: e10p,
        e10_multimodal_as_context: Vec::new(),
        e11_entity: e11,
        e12_late_interaction: tokens,
        e13_splade: e13,
        e14_bge_m3_dense: e14,
    }
}

fn synergy() -> SynergyMatrix {
    SynergyMatrix::with_base_synergies()
}

#[test]
fn compiler_rejects_too_few_members() {
    let members = (0..MIN_CONSTELLATION_MEMBERS - 1)
        .map(|i| (Uuid::new_v4(), synthetic_fp(i as u32), None))
        .collect::<Vec<_>>();
    let res = compile_constellation(
        ConstellationSelector::Tag {
            tag: "too-small".into(),
        },
        "too small".into(),
        members,
        &synergy(),
    );
    assert!(matches!(res, Err(ConstellationError::TooFewMembers { .. })));
}

#[test]
fn identical_members_give_coherence_near_one() {
    let fp = synthetic_fp(42);
    let members: Vec<_> = (0..10)
        .map(|_| (Uuid::new_v4(), fp.clone(), None))
        .collect();
    let c = compile_constellation(
        ConstellationSelector::Tag {
            tag: "identical".into(),
        },
        "identical".into(),
        members,
        &synergy(),
    )
    .expect("compile");
    assert_eq!(c.member_count, 10);
    assert_eq!(c.per_embedder.len(), NUM_EMBEDDERS);
    // Every embedder populated → every coverage==1.0
    for s in &c.per_embedder {
        assert!(
            (s.coverage - 1.0).abs() < 1e-6,
            "embedder {} coverage={} expected 1.0",
            s.embedder_index,
            s.coverage,
        );
        // Identical members → median cosine to centroid is 1.0
        assert!(
            s.cosine_spread_p50 > 0.99,
            "embedder {} p50={} expected >0.99",
            s.embedder_index,
            s.cosine_spread_p50,
        );
    }
    // Coherence (p50 of E1 cosines) is ≈1.0
    assert!(
        c.coherence > 0.99,
        "coherence={} expected >0.99",
        c.coherence
    );
}

#[test]
fn two_clusters_give_intermediate_e1_coherence() {
    let fp_a = synthetic_fp(1);
    let fp_b = synthetic_fp(9999);
    // Sanity: the two fingerprints should NOT be collinear on E1.
    let raw_cos: f32 = fp_a
        .e1_semantic
        .iter()
        .zip(fp_b.e1_semantic.iter())
        .map(|(a, b)| a * b)
        .sum();
    assert!(
        raw_cos.abs() < 0.99,
        "fp_a and fp_b too close on E1 ({}) — pick a different seed",
        raw_cos
    );
    let mut members = Vec::new();
    for _ in 0..5 {
        members.push((Uuid::new_v4(), fp_a.clone(), None));
    }
    for _ in 0..5 {
        members.push((Uuid::new_v4(), fp_b.clone(), None));
    }
    let c = compile_constellation(
        ConstellationSelector::Tag {
            tag: "two-clusters".into(),
        },
        "two clusters".into(),
        members,
        &synergy(),
    )
    .expect("compile");
    // Synthetic fingerprints are fairly similar by construction; the check is
    // that E1 p50 is below the degenerate 1.0 that would indicate a bug in
    // the cosine-vs-centroid accumulator.
    let e1 = &c.per_embedder[0];
    assert!(
        e1.cosine_spread_p50 < 0.999,
        "E1 p50 ({}) indicates degenerate centroid",
        e1.cosine_spread_p50
    );
    assert!(
        e1.cosine_spread_p50 > 0.0,
        "E1 p50 ({}) should still be positive",
        e1.cosine_spread_p50
    );
}

#[test]
fn score_own_member_matches_centroid_highly() {
    let fp = synthetic_fp(7);
    let members: Vec<_> = (0..10)
        .map(|_| (Uuid::new_v4(), fp.clone(), None))
        .collect();
    let c = compile_constellation(
        ConstellationSelector::Session {
            session_id: "sess".into(),
        },
        "sess".into(),
        members,
        &synergy(),
    )
    .expect("compile");
    let result = score_memory_against_constellation(&c, Uuid::new_v4(), &fp);
    assert!(
        result.combined_score > 0.99,
        "combined_score={} expected >0.99",
        result.combined_score
    );
    assert!(
        result.in_spread_p95,
        "expected in_spread_p95=true for own-member score"
    );
    // Every active embedder reports near-1 cosine.
    for (i, c_val) in result.per_embedder_cosine.iter().enumerate() {
        if c.per_embedder[i].coverage > 0.0 {
            assert!(
                *c_val > 0.95,
                "embedder {} cosine={} expected >0.95",
                i,
                c_val
            );
        }
    }
}

#[test]
fn score_unrelated_memory_is_low() {
    let a = synthetic_fp(1);
    let b = synthetic_fp(9999);
    let members: Vec<_> = (0..10).map(|_| (Uuid::new_v4(), a.clone(), None)).collect();
    let c = compile_constellation(
        ConstellationSelector::Tag {
            tag: "a-cluster".into(),
        },
        "a-cluster".into(),
        members,
        &synergy(),
    )
    .expect("compile");
    let result = score_memory_against_constellation(&c, Uuid::new_v4(), &b);
    assert!(
        result.combined_score < 0.99,
        "combined_score={} — unrelated memory should not perfectly match",
        result.combined_score
    );
    assert!(
        !result.in_spread_p95,
        "unrelated memory should not be inside p95 spread",
    );
}

#[test]
fn per_embedder_includes_all_13() {
    let fp = synthetic_fp(2);
    let members: Vec<_> = (0..5).map(|_| (Uuid::new_v4(), fp.clone(), None)).collect();
    let c = compile_constellation(
        ConstellationSelector::Tag {
            tag: "shape-test".into(),
        },
        "shape".into(),
        members,
        &synergy(),
    )
    .expect("compile");
    assert_eq!(c.per_embedder.len(), NUM_EMBEDDERS);
    // Embedder 11 (E12) is token-level.
    assert!(matches!(
        c.per_embedder[11].vector_kind,
        VectorKind::TokenLevel
    ));
    // E6 and E13 are sparse.
    assert!(matches!(c.per_embedder[5].vector_kind, VectorKind::Sparse));
    assert!(matches!(c.per_embedder[12].vector_kind, VectorKind::Sparse));
    // E5, E8, E10 are asymmetric.
    assert!(matches!(
        c.per_embedder[4].vector_kind,
        VectorKind::Asymmetric
    ));
    assert!(matches!(
        c.per_embedder[7].vector_kind,
        VectorKind::Asymmetric
    ));
    assert!(matches!(
        c.per_embedder[9].vector_kind,
        VectorKind::Asymmetric
    ));
    // E6 / E13 produce top-term lists.
    assert!(!c.per_embedder[5].sparse_top_terms.is_empty());
    assert!(!c.per_embedder[12].sparse_top_terms.is_empty());
    // E12 reports a pooled centroid and mean token count.
    assert_eq!(c.per_embedder[11].pooled_token_centroid.len(), 128);
    assert!(c.per_embedder[11].mean_token_count.is_some());
}

#[test]
fn topic_selector_populates_purity() {
    let fp = synthetic_fp(3);
    use crate::constellation::ConstellationAccumulator;
    let mut acc = ConstellationAccumulator::new(
        ConstellationSelector::Topic {
            topic_id: "topic-X".into(),
        },
        "topic-X".into(),
    );
    let syn = synergy();
    for i in 0..10 {
        acc.observe(Uuid::new_v4(), &fp, None, &syn).unwrap();
        acc.observe_topic_match(i < 6); // 60% match rate
    }
    let c = acc.finalize().expect("finalize");
    assert!(c.purity.is_some());
    let purity = c.purity.unwrap();
    assert!(
        (purity - 0.6).abs() < 1e-5,
        "expected purity ≈ 0.6, got {}",
        purity
    );
}

#[test]
fn non_topic_selector_leaves_purity_none() {
    let fp = synthetic_fp(4);
    let members: Vec<_> = (0..5).map(|_| (Uuid::new_v4(), fp.clone(), None)).collect();
    let c = compile_constellation(
        ConstellationSelector::Tag {
            tag: "not-topic".into(),
        },
        "not-topic".into(),
        members,
        &synergy(),
    )
    .expect("compile");
    assert!(c.purity.is_none());
}

#[test]
fn selector_canonical_form_stable() {
    // The hash key used for CF_CONSTELLATION_BY_SELECTOR relies on this
    // being deterministic: identical selectors → identical bytes.
    let a = ConstellationSelector::Topic {
        topic_id: "xyz".into(),
    };
    let b = ConstellationSelector::Topic {
        topic_id: "xyz".into(),
    };
    assert_eq!(a.canonical_form(), b.canonical_form());
    let ids = vec![
        Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
        Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
    ];
    let rev = vec![
        Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(),
        Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
    ];
    let e1 = ConstellationSelector::ExplicitIds {
        rationale: "a".into(),
        ids: ids.clone(),
    };
    let e2 = ConstellationSelector::ExplicitIds {
        rationale: "b".into(),
        ids: rev,
    };
    // Rationale is NOT hashed; id set IS hashed and must be order-insensitive.
    assert_eq!(e1.canonical_form(), e2.canonical_form());
}
