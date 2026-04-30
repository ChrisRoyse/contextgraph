//! Integration tests for `CF_CONSTELLATIONS` + `CF_CONSTELLATION_BY_SELECTOR`
//! (Phase 2).
//!
//! Every test opens a real RocksDB in a fresh temp directory, compiles or
//! hand-builds a [`Constellation`], runs it through the production
//! encode/decode path, and asserts field-by-field that the roundtrip is
//! lossless. No mocks.

use std::sync::Arc;

use chrono::{Duration, Utc};
use context_graph_core::constellation::types::{
    EmbedderStats, VectorKind, CROSS_CORRELATION_CENTROID_DIM, GROUP_ALIGNMENT_CENTROID_DIM,
    TOPIC_PROFILE_CENTROID_DIM,
};
use context_graph_core::constellation::{
    compile_constellation, score_memory_against_constellation, Constellation, ConstellationError,
    ConstellationSelector, MIN_CONSTELLATION_MEMBERS,
};
use context_graph_core::teleological::synergy_matrix::SynergyMatrix;
use context_graph_core::teleological::types::NUM_EMBEDDERS;
use context_graph_core::types::fingerprint::{SemanticFingerprint, SparseVector};
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use tempfile::TempDir;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn open_store() -> (TempDir, Arc<RocksDbTeleologicalStore>) {
    let td = TempDir::new().expect("tempdir");
    let path = td.path().join("store");
    let store =
        RocksDbTeleologicalStore::open_with_config(&path, TeleologicalStoreConfig::default())
            .expect("open store");
    (td, Arc::new(store))
}

/// Normalize a vector in place to unit L2 length (safe on zeros).
fn l2_normalize(v: &mut [f32]) {
    let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        for x in v {
            *x /= n;
        }
    }
}

/// Build a deterministic fingerprint with all 13 embedder slots populated.
fn synthetic_fp(seed: u32) -> SemanticFingerprint {
    fn dense(dim: usize, seed: u32) -> Vec<f32> {
        let mut v: Vec<f32> = (0..dim)
            .map(|i| ((i as u32 + seed) % 17) as f32 * 0.01 + 0.01)
            .collect();
        l2_normalize(&mut v);
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
    let e6 = SparseVector {
        indices: vec![seed as u16 % 1000, (seed + 7) as u16 % 1000, 42],
        values: vec![0.5, 0.3, 0.2],
    };
    let e13 = SparseVector {
        indices: vec![100, 200 + (seed as u16 % 50), 300],
        values: vec![0.4, 0.4, 0.2],
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
        e14_bge_m3_dense: dense(1024, seed.wrapping_add(13)),
    }
}

fn synergy() -> SynergyMatrix {
    SynergyMatrix::with_base_synergies()
}

/// Hand-build a Constellation without running the compiler, so roundtrip
/// tests can assert every field.
fn hand_built_constellation(selector: ConstellationSelector) -> Constellation {
    let per_embedder: Vec<EmbedderStats> = (0..NUM_EMBEDDERS)
        .map(|i| EmbedderStats {
            embedder_index: i as u8,
            dimension: 128,
            vector_kind: if i == 5 || i == 12 {
                VectorKind::Sparse
            } else if i == 11 {
                VectorKind::TokenLevel
            } else if i == 4 || i == 7 || i == 9 {
                VectorKind::Asymmetric
            } else {
                VectorKind::Dense
            },
            centroid: vec![0.1 + i as f32 * 0.01; 128],
            sparse_top_terms: if i == 5 {
                vec![(1, 0.9), (2, 0.5), (3, 0.1)]
            } else {
                Vec::new()
            },
            mean_token_count: if i == 11 { Some(5.5) } else { None },
            pooled_token_centroid: if i == 11 { vec![0.01; 128] } else { Vec::new() },
            mean_l2: 1.0,
            stddev_l2: 0.25,
            cosine_spread_p50: 0.85,
            cosine_spread_p95: 0.97,
            min_cosine: 0.7,
            max_cosine: 1.0,
            coverage: 0.8,
        })
        .collect();

    Constellation {
        id: Uuid::new_v4(),
        label: "hand-built".into(),
        created_at: Utc::now(),
        selector,
        member_ids: vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()],
        member_count: 3,
        per_embedder,
        topic_profile_centroid: [0.1; TOPIC_PROFILE_CENTROID_DIM],
        group_alignment_centroid: [0.2; GROUP_ALIGNMENT_CENTROID_DIM],
        cross_correlation_centroid: vec![0.05; CROSS_CORRELATION_CENTROID_DIM],
        coherence: 0.85,
        purity: Some(0.95),
    }
}

// ---------------------------------------------------------------------------
// 1. Roundtrip preserves all fields
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn roundtrip_constellation_preserves_all_fields() {
    let (_td, store) = open_store();
    let original = hand_built_constellation(ConstellationSelector::Tag {
        tag: "rt-all-fields".into(),
    });

    store.store_constellation(&original).await.expect("store");
    let back = store
        .get_constellation(original.id)
        .await
        .expect("get")
        .expect("exists");

    assert_eq!(back.id, original.id);
    assert_eq!(back.label, original.label);
    assert_eq!(back.selector, original.selector);
    assert_eq!(back.member_ids, original.member_ids);
    assert_eq!(back.member_count, original.member_count);
    assert_eq!(back.per_embedder.len(), original.per_embedder.len());
    for (a, b) in back.per_embedder.iter().zip(original.per_embedder.iter()) {
        assert_eq!(a.embedder_index, b.embedder_index);
        assert_eq!(a.dimension, b.dimension);
        assert_eq!(a.vector_kind, b.vector_kind);
        assert_eq!(a.centroid, b.centroid);
        assert_eq!(a.sparse_top_terms, b.sparse_top_terms);
        assert_eq!(a.mean_token_count, b.mean_token_count);
        assert_eq!(a.pooled_token_centroid, b.pooled_token_centroid);
        assert_eq!(a.mean_l2, b.mean_l2);
        assert_eq!(a.stddev_l2, b.stddev_l2);
        assert_eq!(a.cosine_spread_p50, b.cosine_spread_p50);
        assert_eq!(a.cosine_spread_p95, b.cosine_spread_p95);
        assert_eq!(a.min_cosine, b.min_cosine);
        assert_eq!(a.max_cosine, b.max_cosine);
        assert_eq!(a.coverage, b.coverage);
    }
    assert_eq!(back.topic_profile_centroid, original.topic_profile_centroid);
    assert_eq!(
        back.group_alignment_centroid,
        original.group_alignment_centroid
    );
    assert_eq!(
        back.cross_correlation_centroid,
        original.cross_correlation_centroid
    );
    assert_eq!(back.coherence, original.coherence);
    assert_eq!(back.purity, original.purity);
}

// ---------------------------------------------------------------------------
// 2. find_by_selector returns the stored id
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn find_by_selector_returns_stored_id() {
    let (_td, store) = open_store();
    let selector = ConstellationSelector::Topic {
        topic_id: "find-me".into(),
    };
    let c = hand_built_constellation(selector.clone());
    store.store_constellation(&c).await.expect("store");

    let found = store
        .find_constellation_by_selector(&selector)
        .await
        .expect("find");
    assert_eq!(found, Some(c.id));

    // Different selector — index miss.
    let other = ConstellationSelector::Topic {
        topic_id: "not-stored".into(),
    };
    assert_eq!(
        store.find_constellation_by_selector(&other).await.unwrap(),
        None
    );
}

// ---------------------------------------------------------------------------
// 3. Delete cleans both primary and secondary index
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn delete_cleans_primary_and_secondary() {
    let (_td, store) = open_store();
    let selector = ConstellationSelector::Session {
        session_id: "del-me".into(),
    };
    let c = hand_built_constellation(selector.clone());
    store.store_constellation(&c).await.unwrap();
    assert!(store.get_constellation(c.id).await.unwrap().is_some());
    assert!(store
        .find_constellation_by_selector(&selector)
        .await
        .unwrap()
        .is_some());

    let deleted = store.delete_constellation(c.id).await.unwrap();
    assert!(
        deleted,
        "delete_constellation should report true on first delete"
    );

    assert!(store.get_constellation(c.id).await.unwrap().is_none());
    assert!(store
        .find_constellation_by_selector(&selector)
        .await
        .unwrap()
        .is_none());

    // Idempotent: deleting again returns false.
    assert!(!store.delete_constellation(c.id).await.unwrap());
}

// ---------------------------------------------------------------------------
// 4. Duplicate members → coherence ≈ 1.0
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn compile_with_duplicate_members_yields_coherence_one() {
    let (_td, store) = open_store();
    let fp = synthetic_fp(42);
    let members: Vec<_> = (0..10)
        .map(|_| (Uuid::new_v4(), fp.clone(), None))
        .collect();
    let c = compile_constellation(
        ConstellationSelector::Tag {
            tag: "duplicates".into(),
        },
        "duplicates".into(),
        members,
        &synergy(),
    )
    .expect("compile");

    assert!(
        c.coherence >= 0.99,
        "coherence={} expected ≥ 0.99",
        c.coherence
    );
    assert!(
        c.per_embedder[0].cosine_spread_p50 >= 0.99,
        "E1 p50 = {} expected ≥ 0.99",
        c.per_embedder[0].cosine_spread_p50
    );

    // Persist + read back to prove storage layer preserves the math.
    store.store_constellation(&c).await.unwrap();
    let back = store
        .get_constellation(c.id)
        .await
        .unwrap()
        .expect("persisted");
    assert!((back.coherence - c.coherence).abs() < 1e-6);
}

// ---------------------------------------------------------------------------
// 5. Two clusters → intermediate coherence
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn compile_with_two_clusters_yields_mid_coherence() {
    let fp_a = synthetic_fp(1);
    let fp_b = synthetic_fp(9999);
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
        "two-clusters".into(),
        members,
        &synergy(),
    )
    .expect("compile");

    // The two fp seeds produce vectors that are similar in bulk (they share
    // structure) but not collinear. The permitted window is deliberately
    // wide: the point is that the median is NOT pinned to 1.0 (which would
    // indicate a degenerate centroid bug).
    let e1_p50 = c.per_embedder[0].cosine_spread_p50;
    assert!(
        (0.3..=0.9999).contains(&e1_p50),
        "E1 p50 = {} outside expected [0.3, 0.9999] window",
        e1_p50
    );
}

// ---------------------------------------------------------------------------
// 6. Too-few-members error
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn compile_rejects_too_few_members() {
    let fp = synthetic_fp(1);
    let members = vec![
        (Uuid::new_v4(), fp.clone(), None),
        (Uuid::new_v4(), fp.clone(), None),
    ];
    let res = compile_constellation(
        ConstellationSelector::Tag {
            tag: "too-small".into(),
        },
        "too small".into(),
        members,
        &synergy(),
    );
    match res {
        Err(ConstellationError::TooFewMembers { count, min }) => {
            assert_eq!(count, 2);
            assert_eq!(min, MIN_CONSTELLATION_MEMBERS);
        }
        other => panic!("expected TooFewMembers, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// 7. Own-member score
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn score_own_member_matches_centroid_highly() {
    let fp = synthetic_fp(7);
    let members: Vec<_> = (0..10)
        .map(|_| (Uuid::new_v4(), fp.clone(), None))
        .collect();
    let c = compile_constellation(
        ConstellationSelector::Session {
            session_id: "sess-own".into(),
        },
        "sess-own".into(),
        members,
        &synergy(),
    )
    .expect("compile");

    let result = score_memory_against_constellation(&c, Uuid::new_v4(), &fp);
    assert!(
        result.combined_score >= 0.99,
        "combined_score = {} expected ≥ 0.99",
        result.combined_score
    );
    assert!(
        result.in_spread_p95,
        "expected in_spread_p95=true for own-member score"
    );
}

// ---------------------------------------------------------------------------
// 8. Unrelated-memory score
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn score_unrelated_memory_low() {
    // Build a cluster from 10 identical copies of A, with vectors chosen to
    // be as far from the B query as synthetic data allows.
    let mut a_e1 = vec![0.0f32; 1024];
    a_e1[0] = 1.0;
    l2_normalize(&mut a_e1);

    let mut b_e1 = vec![0.0f32; 1024];
    b_e1[512] = 1.0;
    l2_normalize(&mut b_e1);

    fn with_only_e1(e1: Vec<f32>) -> SemanticFingerprint {
        let mut fp = SemanticFingerprint {
            e1_semantic: e1,
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
        };
        // Give the sparse vectors at least something structurally distinct
        // so Coverage stats on those embedders isn't all zeros — but keep
        // them identical between A and B so the only signal is E1.
        fp.e6_sparse = SparseVector {
            indices: vec![1, 2],
            values: vec![0.5, 0.5],
        };
        fp.e13_splade = SparseVector {
            indices: vec![3, 4],
            values: vec![0.5, 0.5],
        };
        fp
    }

    let fp_a = with_only_e1(a_e1);
    let fp_b = with_only_e1(b_e1);

    let members: Vec<_> = (0..10)
        .map(|_| (Uuid::new_v4(), fp_a.clone(), None))
        .collect();
    let c = compile_constellation(
        ConstellationSelector::Tag {
            tag: "a-only".into(),
        },
        "a-only".into(),
        members,
        &synergy(),
    )
    .expect("compile");

    let res = score_memory_against_constellation(&c, Uuid::new_v4(), &fp_b);
    // E1 cosine between orthogonal unit vectors is 0.
    let e1_cos = res.per_embedder_cosine[0];
    assert!(
        e1_cos.abs() < 0.01,
        "E1 cosine for orthogonal B ({}) should be ≈ 0",
        e1_cos
    );
    assert!(
        !res.in_spread_p95,
        "unrelated memory must not be inside p95 spread"
    );
}

// ---------------------------------------------------------------------------
// 9. list + count reflect writes
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn list_and_count_reflect_writes() {
    let (_td, store) = open_store();
    assert_eq!(store.count_constellations().await.unwrap(), 0);
    assert!(store.list_constellation_ids().await.unwrap().is_empty());

    let c1 = hand_built_constellation(ConstellationSelector::Tag { tag: "t1".into() });
    let c2 = hand_built_constellation(ConstellationSelector::Tag { tag: "t2".into() });
    let c3 = hand_built_constellation(ConstellationSelector::TimeRange {
        start: Utc::now() - Duration::hours(1),
        end: Utc::now(),
    });
    store.store_constellation(&c1).await.unwrap();
    store.store_constellation(&c2).await.unwrap();
    store.store_constellation(&c3).await.unwrap();

    assert_eq!(store.count_constellations().await.unwrap(), 3);
    let ids = store.list_constellation_ids().await.unwrap();
    assert_eq!(ids.len(), 3);
    for expected in [c1.id, c2.id, c3.id] {
        assert!(ids.contains(&expected), "missing id {}", expected);
    }
}

// ---------------------------------------------------------------------------
// 10. multi_get returns aligned slices
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn multi_get_parallel_slice() {
    let (_td, store) = open_store();
    let c1 = hand_built_constellation(ConstellationSelector::Tag {
        tag: "multi-1".into(),
    });
    let c2 = hand_built_constellation(ConstellationSelector::Tag {
        tag: "multi-2".into(),
    });
    store.store_constellation(&c1).await.unwrap();
    store.store_constellation(&c2).await.unwrap();

    // Include a middle-slot miss to validate index alignment.
    let absent = Uuid::new_v4();
    let results = store
        .multi_get_constellations(&[c1.id, absent, c2.id])
        .await
        .unwrap();
    assert_eq!(results.len(), 3);
    assert!(results[0].as_ref().is_some());
    assert!(results[1].is_none(), "missing slot must be None");
    assert!(results[2].as_ref().is_some());
    assert_eq!(results[0].as_ref().unwrap().id, c1.id);
    assert_eq!(results[2].as_ref().unwrap().id, c2.id);
}

// ---------------------------------------------------------------------------
// 11. store_constellation atomically swaps the primary row when the selector
//     index already points at a different UUID (rebuildIfExists=true path).
//     Regression test for the orphan-on-rebuild defect documented in
//     ./memory/discoveries/agent-08-code-simplifier--concern-2-rebuild-orphan.md.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn rebuild_for_same_selector_atomically_deletes_old_primary() {
    let (_td, store) = open_store();
    let selector = ConstellationSelector::Session {
        session_id: "rebuild-swap-test".into(),
    };

    // First store under selector S.
    let old = hand_built_constellation(selector.clone());
    let old_id = old.id;
    store.store_constellation(&old).await.unwrap();

    // Second store under the SAME selector but a fresh UUID — simulates
    // compile_constellation(..., rebuildIfExists=true) where the accumulator
    // generates a new Uuid::new_v4() inside `finalize`.
    let new = hand_built_constellation(selector.clone());
    let new_id = new.id;
    assert_ne!(
        old_id, new_id,
        "sanity: test fixtures must use different UUIDs"
    );
    store.store_constellation(&new).await.unwrap();

    // Primary CF must contain ONLY the new record — the old one must have
    // been deleted in the same batch, not orphaned.
    let ids: Vec<_> = store.list_constellation_ids().await.unwrap();
    assert!(
        ids.contains(&new_id),
        "new UUID must be present in primary CF (got {ids:?})"
    );
    assert!(
        !ids.contains(&old_id),
        "old UUID must be GONE from primary CF after rebuild — this is the \
         orphan-prevention invariant (got {ids:?})"
    );
    assert_eq!(
        ids.len(),
        1,
        "primary CF must have exactly one row for one selector; got {ids:?}"
    );

    // Secondary index must resolve to the new UUID.
    let by_sel = store
        .find_constellation_by_selector(&selector)
        .await
        .unwrap();
    assert_eq!(by_sel, Some(new_id));

    // Direct point-get on the old UUID returns None (confirms primary deletion).
    let gone = store.get_constellation(old_id).await.unwrap();
    assert!(
        gone.is_none(),
        "get_constellation(old_id) must return None after atomic swap"
    );
}

// ---------------------------------------------------------------------------
// 12. Re-storing the SAME UUID (idempotent case) must NOT delete the record
//     (it's the same primary key, and delete-then-put in the same batch
//     could surface ordering surprises). Covers the second branch of the
//     probe-index match in store_constellation.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn restore_same_uuid_is_idempotent_and_keeps_the_row() {
    let (_td, store) = open_store();
    let selector = ConstellationSelector::Tag {
        tag: "restore-same".into(),
    };
    let c = hand_built_constellation(selector.clone());
    let c_id = c.id;

    store.store_constellation(&c).await.unwrap();
    store.store_constellation(&c).await.unwrap(); // re-store same UUID

    let ids: Vec<_> = store.list_constellation_ids().await.unwrap();
    assert_eq!(ids, vec![c_id]);
    let back = store.get_constellation(c_id).await.unwrap();
    assert!(back.is_some());
    assert_eq!(back.unwrap().id, c_id);
}
