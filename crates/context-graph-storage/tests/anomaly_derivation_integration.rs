//! Integration test for F2: derive anomaly pairs from typed edges.
//!
//! Opens a real RocksDB in a fresh [`TempDir`], seeds 6 synthetic
//! [`TypedEdge`] values (one per expressible anomaly pattern + one negative
//! case) via [`EdgeRepository::store_typed_edges_batch`], seeds content for
//! every anchor/negative so the classifier's `get_content` lookup returns
//! non-empty strings, runs [`RocksDbTeleologicalStore::derive_anomalies_from_edges`],
//! and asserts that five of the six edges produce a correctly classified
//! [`ContrastivePair`] written into `CF_CONTRASTIVE_PAIRS`. No mocks.

use std::sync::Arc;

use context_graph_core::contrastive::types::AnomalyKind;
use context_graph_core::graph_linking::{DirectedRelation, GraphLinkEdgeType, TypedEdge};
use context_graph_core::traits::TeleologicalMemoryStore;
use context_graph_storage::graph_edges::EdgeRepository;
use context_graph_storage::teleological::rocksdb_store::AnomalyDerivationConfig;
use context_graph_storage::teleological::{RocksDbTeleologicalStore, TeleologicalStoreConfig};
use tempfile::TempDir;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

fn open_store() -> (TempDir, Arc<RocksDbTeleologicalStore>) {
    let td = TempDir::new().expect("tempdir");
    let path = td.path().join("store");
    let store =
        RocksDbTeleologicalStore::open_with_config(&path, TeleologicalStoreConfig::default())
            .expect("open store");
    (td, Arc::new(store))
}

/// Build a `TypedEdge` with the given scores for an edge type.
///
/// - Primary embedder slot gets bumped to `>=0.5` if below, so the derived
///   weight is a valid agreement.
/// - Agreement bitset/count are computed from `scores >= 0.5` (excluding
///   temporal slots 1..=3 per AP-60 convention).
fn synthetic_edge(
    source: Uuid,
    target: Uuid,
    et: GraphLinkEdgeType,
    scores_in: [f32; 14],
) -> TypedEdge {
    let dir = if et.is_asymmetric() {
        DirectedRelation::Forward
    } else {
        DirectedRelation::Symmetric
    };
    let mut s = scores_in;
    if let Some(i) = et.primary_embedder_index() {
        if s[i] < 0.5 {
            s[i] = 0.8;
        }
    }
    let mut bits = 0u16;
    let mut count = 0u8;
    for (i, x) in s.iter().enumerate() {
        if matches!(i, 1..=3) {
            continue;
        }
        if *x >= 0.5 {
            bits |= 1 << i;
            count += 1;
        }
    }
    let weight = if let Some(i) = et.primary_embedder_index() {
        s[i]
    } else {
        0.8
    };
    TypedEdge::new(source, target, et, weight, dir, s, count, bits).expect("build edge")
}

// ---------------------------------------------------------------------------
// The test
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn derives_five_anomaly_patterns_from_six_edges() {
    let (_td, store) = open_store();
    let db_arc = store.db_arc();
    let repo = EdgeRepository::new(db_arc);

    // Build six source/target pairs. Each pair has its own anchor+target
    // content so the classifier's `get_content` lookup returns non-empty
    // strings (matches production semantics).
    let mut sources = Vec::new();
    let mut targets = Vec::new();
    for i in 0..6 {
        let s = Uuid::new_v4();
        let t = Uuid::new_v4();
        store
            .store_content(s, &format!("anchor content {}", i))
            .await
            .expect("store anchor content");
        store
            .store_content(t, &format!("negative content {}", i))
            .await
            .expect("store negative content");
        sources.push(s);
        targets.push(t);
    }

    // Edge 0: SemanticButNotCausal — E1 high, E5 low.
    let mut e0_scores = [0f32; 14];
    e0_scores[0] = 0.92;
    e0_scores[4] = 0.10;
    let e0 = synthetic_edge(
        sources[0],
        targets[0],
        GraphLinkEdgeType::SemanticSimilar,
        e0_scores,
    );

    // Edge 1: KeywordButNotParaphrase — E6 high, E10 low.
    let mut e1_scores = [0f32; 14];
    e1_scores[5] = 0.88;
    e1_scores[9] = 0.15;
    let e1 = synthetic_edge(
        sources[1],
        targets[1],
        GraphLinkEdgeType::KeywordOverlap,
        e1_scores,
    );

    // Edge 2: CodeShapeButDifferentIntent — E7 high, E1 low.
    let mut e2_scores = [0f32; 14];
    e2_scores[6] = 0.85;
    e2_scores[0] = 0.05;
    let e2 = synthetic_edge(
        sources[2],
        targets[2],
        GraphLinkEdgeType::CodeRelated,
        e2_scores,
    );

    // Edge 3: EntitySharedButDifferentStructure — E11 high, E8 low.
    let mut e3_scores = [0f32; 14];
    e3_scores[10] = 0.82;
    e3_scores[7] = 0.05;
    let e3 = synthetic_edge(
        sources[3],
        targets[3],
        GraphLinkEdgeType::EntityShared,
        e3_scores,
    );

    // Edge 4: HdcRobustButSemanticDifferent — E9 high, E1 low on a
    // ParaphraseAligned edge (cross-type fallback).
    let mut e4_scores = [0f32; 14];
    e4_scores[9] = 0.8; // keep primary above threshold
    e4_scores[8] = 0.96; // E9 high
    e4_scores[0] = 0.09; // E1 low
    let e4 = synthetic_edge(
        sources[4],
        targets[4],
        GraphLinkEdgeType::ParaphraseAligned,
        e4_scores,
    );

    // Edge 5: no classification — all scores ~0.4 on a SemanticSimilar edge.
    // Primary embedder will be bumped to 0.8 but E5 is also 0.4 (above the
    // `low_threshold=0.30` cutoff), so the pattern doesn't match.
    let mut e5_scores = [0f32; 14];
    e5_scores.fill(0.4);
    e5_scores[4] = 0.4; // still above low_threshold → no match
    let e5 = synthetic_edge(
        sources[5],
        targets[5],
        GraphLinkEdgeType::SemanticSimilar,
        e5_scores,
    );

    repo.store_typed_edges_batch(&[e0, e1, e2, e3, e4, e5])
        .expect("seed typed edges");

    // Derive.
    let summary = store
        .derive_anomalies_from_edges(&repo, &AnomalyDerivationConfig::default())
        .await
        .expect("derivation must succeed");

    assert_eq!(summary.edges_scanned, 6, "all 6 edges must be scanned");
    assert_eq!(
        summary.pairs_written, 5,
        "5 edges should classify, 1 should not; got {} writes",
        summary.pairs_written
    );

    // Per-kind counts must match exactly.
    assert_eq!(
        summary
            .per_kind_counts
            .get(&AnomalyKind::SemanticButNotCausal),
        Some(&1),
        "missing SemanticButNotCausal in {:?}",
        summary.per_kind_counts
    );
    assert_eq!(
        summary
            .per_kind_counts
            .get(&AnomalyKind::KeywordButNotParaphrase),
        Some(&1),
        "missing KeywordButNotParaphrase"
    );
    assert_eq!(
        summary
            .per_kind_counts
            .get(&AnomalyKind::CodeShapeButDifferentIntent),
        Some(&1),
        "missing CodeShapeButDifferentIntent"
    );
    assert_eq!(
        summary
            .per_kind_counts
            .get(&AnomalyKind::EntitySharedButDifferentStructure),
        Some(&1),
        "missing EntitySharedButDifferentStructure"
    );
    assert_eq!(
        summary
            .per_kind_counts
            .get(&AnomalyKind::HdcRobustButSemanticDifferent),
        Some(&1),
        "missing HdcRobustButSemanticDifferent"
    );
    assert_eq!(
        summary.per_kind_counts.len(),
        5,
        "exactly 5 named kinds expected, got {:?}",
        summary.per_kind_counts
    );

    // Total pairs in CF_CONTRASTIVE_PAIRS must equal pairs_written.
    assert_eq!(store.count_contrastive_pairs().await.unwrap(), 5);

    // Each written pair must carry the derivation generator tag so origin is
    // traceable.
    let pair = store
        .get_contrastive_pair(sources[0], targets[0])
        .await
        .unwrap()
        .expect("SemanticButNotCausal pair must exist");
    assert_eq!(pair.generator, "typed_edge_anomaly_derivation_v1");
    assert_eq!(pair.anomaly_kind, AnomalyKind::SemanticButNotCausal);
    assert_eq!(pair.anchor_id, sources[0]);
    assert_eq!(pair.negative_id, targets[0]);
    // Anchor/negative content must be populated from CF_CONTENT.
    assert!(pair.anchor_text.starts_with("anchor content"));
    assert!(pair.negative_text.starts_with("negative content"));

    // Spot-check the HDC cross-type pair (edge type ParaphraseAligned, but
    // classified as HdcRobustButSemanticDifferent via the cross-type fallback).
    let hdc_pair = store
        .get_contrastive_pair(sources[4], targets[4])
        .await
        .unwrap()
        .expect("HDC pair must exist");
    assert_eq!(
        hdc_pair.anomaly_kind,
        AnomalyKind::HdcRobustButSemanticDifferent
    );
    assert_eq!(hdc_pair.generator, "typed_edge_anomaly_derivation_v1");

    // The non-matching edge must not have produced any pair.
    let miss = store
        .get_contrastive_pair(sources[5], targets[5])
        .await
        .unwrap();
    assert!(miss.is_none(), "no-match edge must not produce a pair");
}
