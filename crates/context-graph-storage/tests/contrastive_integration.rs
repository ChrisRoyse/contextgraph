//! Integration tests for `CF_CONTRASTIVE_PAIRS` + `CF_CONTRASTIVE_BY_KIND` +
//! `CF_CONTRASTIVE_BY_ANCHOR` (Phase 3).
//!
//! Every test opens a real RocksDB in a fresh temp directory, builds
//! [`ContrastivePair`]s (hand-constructed for lifecycle tests; mined via the
//! pure functional path for similarity-profile assertions), runs them through
//! the production encode/decode path, and asserts field-by-field that the
//! roundtrip is lossless. No mocks.

use std::sync::Arc;

use chrono::Utc;
use context_graph_core::contrastive::{
    classify_anomaly, mine_pair_from_candidate, similarity_profile, AnomalyKind, ContrastivePair,
    MiningConfig,
};
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

fn l2_normalize(v: &mut [f32]) {
    let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        for x in v {
            *x /= n;
        }
    }
}

fn synthetic_fp(seed: u32) -> SemanticFingerprint {
    fn dense(dim: usize, seed: u32) -> Vec<f32> {
        let mut v: Vec<f32> = (0..dim)
            .map(|i| ((i as u32 + seed) % 17) as f32 * 0.01 + 0.01)
            .collect();
        l2_normalize(&mut v);
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
            indices: vec![1, 2, 3],
            values: vec![0.5, 0.3, 0.2],
        },
        e7_code: dense(1536, seed.wrapping_add(6)),
        e8_graph_as_source: dense(1024, seed.wrapping_add(7)),
        e8_graph_as_target: Vec::new(),
        e8_graph: Vec::new(),
        e9_hdc: dense(1024, seed.wrapping_add(8)),
        e10_multimodal_paraphrase: dense(768, seed.wrapping_add(9)),
        e10_multimodal_as_context: Vec::new(),
        e11_entity: dense(768, seed.wrapping_add(10)),
        e14_bge_m3_dense: dense(1024, seed.wrapping_add(13)),
        e12_late_interaction: (0..3)
            .map(|t| dense(128, seed.wrapping_add(100 + t as u32)))
            .collect(),
        e13_splade: SparseVector {
            indices: vec![10, 20, 30],
            values: vec![0.4, 0.4, 0.2],
        },
    }
}

fn hand_built_pair(anchor: Uuid, negative: Uuid, kind: AnomalyKind) -> ContrastivePair {
    let mut profile = [0.5f32; NUM_EMBEDDERS];
    profile[0] = 0.9; // high E1
    profile[4] = 0.05; // low E5
    ContrastivePair {
        anchor_id: anchor,
        negative_id: negative,
        anchor_text: format!("anchor-{}", anchor),
        negative_text: format!("neg-{}", negative),
        similarity_profile: profile,
        high_embedders: vec![0],
        low_embedders: vec![4],
        disagreement_magnitude: 0.85,
        anomaly_kind: kind,
        mined_at: Utc::now(),
        generator: "cross_embedder_anomaly_v1".into(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn roundtrip_pair_preserves_all_fields() {
    let (_td, store) = open_store();
    let anchor = Uuid::new_v4();
    let negative = Uuid::new_v4();
    let original = hand_built_pair(anchor, negative, AnomalyKind::SemanticButNotCausal);

    store.store_contrastive_pair(&original).await.unwrap();
    let back = store
        .get_contrastive_pair(anchor, negative)
        .await
        .unwrap()
        .expect("stored pair must come back");

    assert_eq!(back.anchor_id, original.anchor_id);
    assert_eq!(back.negative_id, original.negative_id);
    assert_eq!(back.anchor_text, original.anchor_text);
    assert_eq!(back.negative_text, original.negative_text);
    assert_eq!(back.similarity_profile, original.similarity_profile);
    assert_eq!(back.high_embedders, original.high_embedders);
    assert_eq!(back.low_embedders, original.low_embedders);
    assert!((back.disagreement_magnitude - original.disagreement_magnitude).abs() < 1e-6);
    assert_eq!(back.anomaly_kind, original.anomaly_kind);
    assert_eq!(back.generator, original.generator);
    // mined_at round-trips exactly through bincode (i64 + u32 nanos).
    assert_eq!(back.mined_at, original.mined_at);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_populates_all_three_cfs_atomically() {
    let (_td, store) = open_store();
    let anchor = Uuid::new_v4();
    let negative = Uuid::new_v4();
    let pair = hand_built_pair(anchor, negative, AnomalyKind::CodeShapeButDifferentIntent);

    store.store_contrastive_pair(&pair).await.unwrap();

    // Primary CF.
    assert_eq!(store.count_contrastive_pairs().await.unwrap(), 1);
    // By-anchor.
    let negs = store.pairs_for_anchor(anchor).await.unwrap();
    assert_eq!(negs, vec![negative]);
    // By-kind.
    let by_kind = store
        .list_pairs_by_kind(AnomalyKind::CodeShapeButDifferentIntent, 10)
        .await
        .unwrap();
    assert_eq!(by_kind, vec![(anchor, negative)]);
    // And a different kind returns nothing.
    let other = store
        .list_pairs_by_kind(AnomalyKind::SemanticButNotCausal, 10)
        .await
        .unwrap();
    assert!(other.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_removes_from_all_three_cfs() {
    let (_td, store) = open_store();
    let anchor = Uuid::new_v4();
    let negative = Uuid::new_v4();
    let pair = hand_built_pair(anchor, negative, AnomalyKind::HdcRobustButSemanticDifferent);
    store.store_contrastive_pair(&pair).await.unwrap();
    assert_eq!(store.count_contrastive_pairs().await.unwrap(), 1);

    let ok = store
        .delete_contrastive_pair(anchor, negative)
        .await
        .unwrap();
    assert!(ok);

    // All three CFs must be empty for this pair.
    assert_eq!(store.count_contrastive_pairs().await.unwrap(), 0);
    assert!(store
        .get_contrastive_pair(anchor, negative)
        .await
        .unwrap()
        .is_none());
    assert!(store.pairs_for_anchor(anchor).await.unwrap().is_empty());
    assert!(store
        .list_pairs_by_kind(AnomalyKind::HdcRobustButSemanticDifferent, 10)
        .await
        .unwrap()
        .is_empty());

    // Second delete is a no-op that returns false.
    let ok2 = store
        .delete_contrastive_pair(anchor, negative)
        .await
        .unwrap();
    assert!(!ok2);
}

#[tokio::test(flavor = "multi_thread")]
async fn clear_all_contrastive_pairs_empties_all_three() {
    let (_td, store) = open_store();
    let a1 = Uuid::new_v4();
    let a2 = Uuid::new_v4();
    for _ in 0..3 {
        let pair = hand_built_pair(a1, Uuid::new_v4(), AnomalyKind::SemanticButNotCausal);
        store.store_contrastive_pair(&pair).await.unwrap();
    }
    for _ in 0..2 {
        let pair = hand_built_pair(a2, Uuid::new_v4(), AnomalyKind::KeywordButNotParaphrase);
        store.store_contrastive_pair(&pair).await.unwrap();
    }
    assert_eq!(store.count_contrastive_pairs().await.unwrap(), 5);

    let cleared = store.clear_all_contrastive_pairs().await.unwrap();
    assert_eq!(cleared, 5);
    assert_eq!(store.count_contrastive_pairs().await.unwrap(), 0);
    assert_eq!(
        store
            .count_contrastive_pairs_by_kind(AnomalyKind::SemanticButNotCausal)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .count_contrastive_pairs_by_kind(AnomalyKind::KeywordButNotParaphrase)
            .await
            .unwrap(),
        0
    );
    assert!(store.pairs_for_anchor(a1).await.unwrap().is_empty());
    assert!(store.pairs_for_anchor(a2).await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn list_pairs_by_kind_filters_correctly() {
    let (_td, store) = open_store();
    // 3 pairs of kind A (SemanticButNotCausal) + 2 of kind B (CodeShape).
    for _ in 0..3 {
        let p = hand_built_pair(
            Uuid::new_v4(),
            Uuid::new_v4(),
            AnomalyKind::SemanticButNotCausal,
        );
        store.store_contrastive_pair(&p).await.unwrap();
    }
    for _ in 0..2 {
        let p = hand_built_pair(
            Uuid::new_v4(),
            Uuid::new_v4(),
            AnomalyKind::CodeShapeButDifferentIntent,
        );
        store.store_contrastive_pair(&p).await.unwrap();
    }

    assert_eq!(
        store
            .list_pairs_by_kind(AnomalyKind::SemanticButNotCausal, 100)
            .await
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        store
            .list_pairs_by_kind(AnomalyKind::CodeShapeButDifferentIntent, 100)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        store
            .count_contrastive_pairs_by_kind(AnomalyKind::SemanticButNotCausal)
            .await
            .unwrap(),
        3
    );
    assert_eq!(
        store
            .count_contrastive_pairs_by_kind(AnomalyKind::CodeShapeButDifferentIntent)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        store
            .count_contrastive_pairs_by_kind(AnomalyKind::Other)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn pairs_for_anchor_returns_all_negatives() {
    let (_td, store) = open_store();
    let anchor = Uuid::new_v4();
    let mut expected = Vec::new();
    for _ in 0..4 {
        let neg = Uuid::new_v4();
        expected.push(neg);
        let pair = hand_built_pair(anchor, neg, AnomalyKind::Other);
        store.store_contrastive_pair(&pair).await.unwrap();
    }

    let mut got = store.pairs_for_anchor(anchor).await.unwrap();
    got.sort();
    let mut expected_sorted = expected.clone();
    expected_sorted.sort();
    assert_eq!(got, expected_sorted);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_is_idempotent_for_same_key() {
    let (_td, store) = open_store();
    let anchor = Uuid::new_v4();
    let negative = Uuid::new_v4();
    let pair = hand_built_pair(anchor, negative, AnomalyKind::Other);
    store.store_contrastive_pair(&pair).await.unwrap();
    store.store_contrastive_pair(&pair).await.unwrap();
    store.store_contrastive_pair(&pair).await.unwrap();
    assert_eq!(store.count_contrastive_pairs().await.unwrap(), 1);
    let negs = store.pairs_for_anchor(anchor).await.unwrap();
    assert_eq!(negs.len(), 1, "anchor list must deduplicate");
    assert_eq!(
        store
            .count_contrastive_pairs_by_kind(AnomalyKind::Other)
            .await
            .unwrap(),
        1
    );
}

#[test]
fn similarity_profile_for_identical_fingerprints_is_all_ones() {
    let fp = synthetic_fp(17);
    let prof = similarity_profile(&fp, &fp);
    for (i, v) in prof.iter().enumerate() {
        assert!(
            *v >= 0.95,
            "embedder {} expected >= 0.95 on identical fp, got {}",
            i,
            v
        );
    }
}

#[test]
fn similarity_profile_for_orthogonal_e1_respects_bounds() {
    let mut a = synthetic_fp(1);
    let mut b = synthetic_fp(1);
    let dim = a.e1_semantic.len();
    let mut va = vec![0.0f32; dim];
    let mut vb = vec![0.0f32; dim];
    va[0] = 1.0;
    vb[1] = 1.0;
    a.e1_semantic = va;
    b.e1_semantic = vb;
    let prof = similarity_profile(&a, &b);
    // Orthogonal dense cosine = 0 → SRC-3 midpoint = 0.5.
    assert!(
        (prof[0] - 0.5).abs() < 1e-3,
        "E1 orthogonal should map to ~0.5 under SRC-3, got {}",
        prof[0]
    );
    for (i, v) in prof.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(v) && v.is_finite(),
            "embedder {} out of bounds: {}",
            i,
            v
        );
    }
}

#[test]
fn classify_high_e1_low_e5_is_semantic_but_not_causal() {
    let mut profile = [0.0f32; NUM_EMBEDDERS];
    profile[0] = 0.8;
    profile[4] = 0.1;
    let kind = classify_anomaly(&profile, 0.6, 0.3);
    assert_eq!(kind, AnomalyKind::SemanticButNotCausal);
}

#[test]
fn mine_pair_from_candidate_returns_none_below_min_disagreement() {
    // Identical fingerprints → all-ones profile → no low embedders → None.
    let fp = synthetic_fp(5);
    let cfg = MiningConfig::default();
    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();
    let got = mine_pair_from_candidate(id_a, "a", &fp, id_b, "b", &fp, &cfg);
    assert!(got.is_none());

    // Set min_disagreement above the profile's natural range so even a
    // genuine high/low pair fails. We construct a pair with E1 ~= 1.0 and
    // E5 = 0.0 (zero-norm → 0.0 sim); disagreement = 1.0. Then set
    // min_disagreement = 1.01 to reject.
    let a = synthetic_fp(1);
    let mut b = synthetic_fp(1);
    let dim5 = a.e5_causal_as_cause.len();
    b.e5_causal_as_cause = vec![0.0; dim5];
    // Upper bound of f32 similarity is 1.0; 1.5 is strictly unreachable.
    // MiningConfig::validate() rejects values > 1.0, so call the miner
    // directly without running validate() — the function itself must still
    // respect min_disagreement even when it's been set beyond the valid
    // range (defensive check against bad callers).
    let strict = MiningConfig {
        min_disagreement: 1.5,
        ..Default::default()
    };
    let got2 = mine_pair_from_candidate(id_a, "a", &a, id_b, "b", &b, &strict);
    assert!(got2.is_none(), "expected None when min_disagreement > 1.0");
}
