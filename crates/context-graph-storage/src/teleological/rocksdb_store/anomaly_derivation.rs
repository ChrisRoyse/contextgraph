//! F2: derive anomaly pairs directly from typed-edge combinations.
//!
//! Rather than re-scanning every memory and re-computing similarities, we
//! walk `CF_TYPED_EDGES` (already the output of the embedder ensemble) and
//! classify distinctive (source, target) pairs into five of the six
//! [`AnomalyKind`] variants. The sixth kind, `Other`, remains the
//! responsibility of the offline miner because it requires full per-embedder
//! re-scoring across the whole candidate pool.
//!
//! Each classified edge is written into `CF_CONTRASTIVE_PAIRS` + the two
//! secondary indexes via the existing [`RocksDbTeleologicalStore::store_contrastive_pair`]
//! write path — no schema changes.

use std::collections::HashMap;

use chrono::Utc;
use context_graph_core::contrastive::types::{AnomalyKind, ContrastivePair};
use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::graph_linking::{GraphLinkEdgeType, TypedEdge};
use tracing::{debug, info, warn};

use crate::graph_edges::EdgeRepository;

use super::store::RocksDbTeleologicalStore;

/// Derivation-time tunables.
///
/// Defaults mirror the offline miner's thresholds so the two pipelines produce
/// comparable pairs: `high_threshold=0.60`, `low_threshold=0.30`,
/// `min_disagreement=0.30`, `max_pairs=10_000`.
#[derive(Debug, Clone)]
pub struct AnomalyDerivationConfig {
    /// Embedder score at or above which an embedder counts as "high" for the
    /// anomaly classifier.
    pub high_threshold: f32,
    /// Embedder score at or below which the opposing-embedder slot counts as
    /// "low" for the anomaly classifier.
    pub low_threshold: f32,
    /// Hard cap on total pairs written in one derivation run.
    pub max_pairs: usize,
    /// Minimum `high - low` gap required to keep a pair (mirrors the offline
    /// miner's `min_disagreement`).
    pub min_disagreement: f32,
}

impl Default for AnomalyDerivationConfig {
    fn default() -> Self {
        Self {
            high_threshold: 0.60,
            low_threshold: 0.30,
            max_pairs: 10_000,
            min_disagreement: 0.30,
        }
    }
}

/// Summary returned by one call to
/// [`RocksDbTeleologicalStore::derive_anomalies_from_edges`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnomalyDerivationSummary {
    /// Number of edges walked from `CF_TYPED_EDGES`.
    pub edges_scanned: usize,
    /// Number of `ContrastivePair` rows written to `CF_CONTRASTIVE_PAIRS`.
    pub pairs_written: usize,
    /// Edges classified into an anomaly kind but whose disagreement gap was
    /// below `min_disagreement`.
    pub skipped_below_threshold: usize,
    /// Edges classified into an anomaly kind but whose source or target
    /// content could not be resolved. Skipping these keeps the contrastive
    /// corpus free of empty-text anchors / negatives that would be useless
    /// for training.
    pub skipped_missing_content: usize,
    /// Per-kind counts of written pairs.
    pub per_kind_counts: HashMap<AnomalyKind, usize>,
    /// Wall-clock duration of the derivation run in milliseconds.
    pub duration_ms: u64,
}

impl RocksDbTeleologicalStore {
    /// Walk `CF_TYPED_EDGES` once, classify each edge against the five
    /// expressible [`AnomalyKind`] patterns, and atomically write each hit
    /// into `CF_CONTRASTIVE_PAIRS` (plus the two secondary indexes).
    ///
    /// Per-pair writes go through
    /// [`RocksDbTeleologicalStore::store_contrastive_pair`] to keep
    /// `CF_CONTRASTIVE_BY_KIND` and `CF_CONTRASTIVE_BY_ANCHOR` in sync. Runs
    /// are idempotent on the composite `(anchor, negative)` primary key —
    /// re-derivation overwrites the prior row for the same pair.
    pub async fn derive_anomalies_from_edges(
        &self,
        edge_repository: &EdgeRepository,
        cfg: &AnomalyDerivationConfig,
    ) -> CoreResult<AnomalyDerivationSummary> {
        let t0 = std::time::Instant::now();

        let edges = edge_repository
            .iter_all_typed_edges()
            .map_err(|e| CoreError::StorageError(format!("iter_all_typed_edges failed: {}", e)))?;

        let mut summary = AnomalyDerivationSummary {
            edges_scanned: edges.len(),
            ..Default::default()
        };

        for edge in edges.iter() {
            if summary.pairs_written >= cfg.max_pairs {
                info!(
                    cap = cfg.max_pairs,
                    "derive_anomalies_from_edges: max_pairs reached; stopping"
                );
                break;
            }

            let Some((kind, high_embedders, low_embedders, disagreement)) =
                classify_edge_as_anomaly(edge, cfg)
            else {
                continue;
            };

            if disagreement < cfg.min_disagreement {
                summary.skipped_below_threshold += 1;
                continue;
            }

            let anchor_text = match self.get_content_async(edge.source()).await? {
                Some(s) => s,
                None => {
                    warn!(
                        src = %edge.source(),
                        tgt = %edge.target(),
                        kind = kind.as_str(),
                        "derive_anomalies_from_edges: source content missing; skipping pair"
                    );
                    summary.skipped_missing_content += 1;
                    continue;
                }
            };
            let negative_text = match self.get_content_async(edge.target()).await? {
                Some(s) => s,
                None => {
                    warn!(
                        src = %edge.source(),
                        tgt = %edge.target(),
                        kind = kind.as_str(),
                        "derive_anomalies_from_edges: target content missing; skipping pair"
                    );
                    summary.skipped_missing_content += 1;
                    continue;
                }
            };

            let pair = ContrastivePair {
                anchor_id: edge.source(),
                negative_id: edge.target(),
                anchor_text,
                negative_text,
                similarity_profile: *edge.embedder_scores(),
                high_embedders,
                low_embedders,
                disagreement_magnitude: disagreement,
                anomaly_kind: kind,
                mined_at: Utc::now(),
                generator: "typed_edge_anomaly_derivation_v1".to_string(),
            };

            self.store_contrastive_pair(&pair).await?;
            summary.pairs_written += 1;
            *summary.per_kind_counts.entry(kind).or_insert(0) += 1;
        }

        summary.duration_ms = t0.elapsed().as_millis() as u64;
        debug!(
            scanned = summary.edges_scanned,
            written = summary.pairs_written,
            skipped_below_threshold = summary.skipped_below_threshold,
            skipped_missing_content = summary.skipped_missing_content,
            duration_ms = summary.duration_ms,
            "derive_anomalies_from_edges complete"
        );
        Ok(summary)
    }
}

/// Pure-function classifier used by
/// [`RocksDbTeleologicalStore::derive_anomalies_from_edges`].
///
/// Returns `Some((kind, high_embedders, low_embedders, disagreement_gap))`
/// when the edge matches one of the five expressible anomaly patterns:
///
/// - `SemanticButNotCausal`: E1 high, E5 low on a `SemanticSimilar` edge.
/// - `KeywordButNotParaphrase`: E6 **or** E13 high, E10 low on a
///   `KeywordOverlap` edge.
/// - `CodeShapeButDifferentIntent`: E7 high, E1 low on a `CodeRelated` edge.
/// - `EntitySharedButDifferentStructure`: E11 high, E8 low on an
///   `EntityShared` edge.
/// - `HdcRobustButSemanticDifferent`: E9 high, E1 low on **any other** edge
///   type (cross-type fallback).
///
/// Returns `None` when no pattern matches.
///
/// Embedder index map (SRC-3 normalized scores in `[0, 1]`):
/// E1=0, E5=4, E6=5, E7=6, E8=7, E9=8, E10=9, E11=10, E13=12.
pub fn classify_edge_as_anomaly(
    edge: &TypedEdge,
    cfg: &AnomalyDerivationConfig,
) -> Option<(AnomalyKind, Vec<u8>, Vec<u8>, f32)> {
    let scores = edge.embedder_scores();
    let is_high = |i: usize| scores[i] >= cfg.high_threshold;
    let is_low = |i: usize| scores[i] <= cfg.low_threshold;

    match edge.edge_type() {
        GraphLinkEdgeType::SemanticSimilar => {
            // SemanticButNotCausal: E1 high, E5 low.
            if is_high(0) && is_low(4) {
                return Some((
                    AnomalyKind::SemanticButNotCausal,
                    vec![0],
                    vec![4],
                    scores[0] - scores[4],
                ));
            }
        }
        GraphLinkEdgeType::KeywordOverlap => {
            // KeywordButNotParaphrase: E6 or E13 high, E10 low.
            let hi_idx = if is_high(5) {
                Some(5usize)
            } else if is_high(12) {
                Some(12usize)
            } else {
                None
            };
            if let Some(hi) = hi_idx {
                if is_low(9) {
                    return Some((
                        AnomalyKind::KeywordButNotParaphrase,
                        vec![hi as u8],
                        vec![9],
                        scores[hi] - scores[9],
                    ));
                }
            }
        }
        GraphLinkEdgeType::CodeRelated => {
            // CodeShapeButDifferentIntent: E7 high, E1 low.
            if is_high(6) && is_low(0) {
                return Some((
                    AnomalyKind::CodeShapeButDifferentIntent,
                    vec![6],
                    vec![0],
                    scores[6] - scores[0],
                ));
            }
        }
        GraphLinkEdgeType::EntityShared => {
            // EntitySharedButDifferentStructure: E11 high, E8 low.
            if is_high(10) && is_low(7) {
                return Some((
                    AnomalyKind::EntitySharedButDifferentStructure,
                    vec![10],
                    vec![7],
                    scores[10] - scores[7],
                ));
            }
        }
        _ => {
            // HdcRobustButSemanticDifferent: E9 high, E1 low — applies on any
            // other edge type (cross-type fallback).
            if is_high(8) && is_low(0) {
                return Some((
                    AnomalyKind::HdcRobustButSemanticDifferent,
                    vec![8],
                    vec![0],
                    scores[8] - scores[0],
                ));
            }
        }
    }
    None
}

// ============================================================================
// Unit tests (classifier only — the derivation run is exercised by the
// integration test `tests/anomaly_derivation_integration.rs` with a real
// RocksDB TempDir)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::graph_linking::DirectedRelation;
    use uuid::Uuid;

    /// Build a `TypedEdge` with the given per-embedder scores and edge type.
    /// Agreement bits are derived from `scores >= 0.5` (excluding temporal
    /// slots 1..=3). The primary embedder slot is bumped to 0.8 if it would
    /// otherwise fall below 0.5, so the edge's `weight` is always valid.
    fn edge_with(scores: [f32; 14], et: GraphLinkEdgeType) -> TypedEdge {
        let dir = if et.is_asymmetric() {
            DirectedRelation::Forward
        } else {
            DirectedRelation::Symmetric
        };
        let mut s = scores;
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
        TypedEdge::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            et,
            weight,
            dir,
            s,
            count,
            bits,
        )
        .expect("build edge")
    }

    #[test]
    fn classifies_semantic_but_not_causal() {
        let mut s = [0f32; 14];
        s[0] = 0.9;
        s[4] = 0.1;
        let e = edge_with(s, GraphLinkEdgeType::SemanticSimilar);
        let cfg = AnomalyDerivationConfig::default();
        let got = classify_edge_as_anomaly(&e, &cfg).expect("pattern must match");
        assert_eq!(got.0, AnomalyKind::SemanticButNotCausal);
        assert_eq!(got.1, vec![0]);
        assert_eq!(got.2, vec![4]);
        // Agreement bump preserves E1=0.9; disagreement = 0.9 - 0.1 = 0.8
        assert!((got.3 - 0.8).abs() < 1e-3, "expected 0.8, got {}", got.3);
    }

    #[test]
    fn classifies_code_shape_but_different_intent() {
        let mut s = [0f32; 14];
        s[6] = 0.85;
        s[0] = 0.05;
        let e = edge_with(s, GraphLinkEdgeType::CodeRelated);
        let got = classify_edge_as_anomaly(&e, &AnomalyDerivationConfig::default())
            .expect("pattern must match");
        assert_eq!(got.0, AnomalyKind::CodeShapeButDifferentIntent);
        assert_eq!(got.1, vec![6]);
        assert_eq!(got.2, vec![0]);
    }

    #[test]
    fn classifies_hdc_cross_type() {
        // Edge type is ParaphraseAligned but pattern matches HDC anomaly.
        let mut s = [0f32; 14];
        s[9] = 0.8;
        s[8] = 0.95;
        s[0] = 0.10;
        let e = edge_with(s, GraphLinkEdgeType::ParaphraseAligned);
        let got = classify_edge_as_anomaly(&e, &AnomalyDerivationConfig::default())
            .expect("pattern must match");
        assert_eq!(got.0, AnomalyKind::HdcRobustButSemanticDifferent);
        assert_eq!(got.1, vec![8]);
        assert_eq!(got.2, vec![0]);
    }

    #[test]
    fn no_classification_when_thresholds_not_met() {
        let s = [0.4f32; 14];
        let e = edge_with(s, GraphLinkEdgeType::SemanticSimilar);
        assert!(classify_edge_as_anomaly(&e, &AnomalyDerivationConfig::default()).is_none());
    }

    #[test]
    fn config_defaults_match_offline_miner() {
        let cfg = AnomalyDerivationConfig::default();
        assert!((cfg.high_threshold - 0.60).abs() < 1e-6);
        assert!((cfg.low_threshold - 0.30).abs() < 1e-6);
        assert!((cfg.min_disagreement - 0.30).abs() < 1e-6);
        assert_eq!(cfg.max_pairs, 10_000);
    }
}
