//! Compute the 8-dim `edge_type_distribution` for a memory from an iterator
//! of its outgoing (or combined in+out) [`TypedEdge`]s.
//!
//! The distribution is the per-`GraphLinkEdgeType` count of edges and is
//! stored on [`crate::training::TrainingRecord`] (v2). Indexing matches
//! [`crate::graph_linking::GraphLinkEdgeType::as_u8`].

use crate::graph_linking::{GraphLinkEdgeType, TypedEdge};

/// Length of the edge-type distribution vector
/// (matches [`GraphLinkEdgeType::COUNT`]).
pub const NUM_EDGE_TYPE_DISTRIBUTION: usize = GraphLinkEdgeType::COUNT;

/// Count each edge by its `edge_type`. Returns an 8-vector indexed by
/// [`GraphLinkEdgeType::as_u8`].
///
/// Saturates at `u32::MAX` per slot (the panic-free upper bound is far
/// beyond any realistic outgoing-edges-per-memory count).
pub fn compute_edge_type_distribution<'a, I>(edges: I) -> [u32; NUM_EDGE_TYPE_DISTRIBUTION]
where
    I: IntoIterator<Item = &'a TypedEdge>,
{
    let mut dist = [0u32; NUM_EDGE_TYPE_DISTRIBUTION];
    for edge in edges {
        let idx = edge.edge_type().as_u8() as usize;
        debug_assert!(
            idx < NUM_EDGE_TYPE_DISTRIBUTION,
            "edge_type u8 must be 0..{}",
            NUM_EDGE_TYPE_DISTRIBUTION
        );
        dist[idx] = dist[idx].saturating_add(1);
    }
    dist
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_linking::{DirectedRelation, TypedEdge};
    use uuid::Uuid;

    fn mk_edge(source: Uuid, target: Uuid, et: GraphLinkEdgeType, weight: f32) -> TypedEdge {
        let mut scores = [0f32; 14];
        if let Some(i) = et.primary_embedder_index() {
            scores[i] = weight;
        } else {
            // MultiAgreement — set 3 arbitrary slots to satisfy the bitset/count
            // invariant enforced by `TypedEdge::new`.
            scores[0] = weight;
            scores[5] = weight;
            scores[9] = weight;
        }
        let dir = if et.is_asymmetric() {
            DirectedRelation::Forward
        } else {
            DirectedRelation::Symmetric
        };
        let (count, bits) = if matches!(et, GraphLinkEdgeType::MultiAgreement) {
            // E1 (bit 0) + E6 (bit 5) + E10 (bit 9): three agreeing embedders.
            (3u8, (1u16 << 0) | (1u16 << 5) | (1u16 << 9))
        } else if let Some(i) = et.primary_embedder_index() {
            (1u8, 1u16 << i)
        } else {
            (0u8, 0u16)
        };
        TypedEdge::new(source, target, et, weight, dir, scores, count, bits).expect("build edge")
    }

    #[test]
    fn empty_iterator_yields_zero_vector() {
        let edges: Vec<TypedEdge> = Vec::new();
        assert_eq!(
            compute_edge_type_distribution(&edges),
            [0u32; NUM_EDGE_TYPE_DISTRIBUTION]
        );
    }

    #[test]
    fn single_edge_single_slot() {
        let s = Uuid::new_v4();
        let t = Uuid::new_v4();
        let edges = vec![mk_edge(s, t, GraphLinkEdgeType::CausalChain, 0.7)];
        let dist = compute_edge_type_distribution(&edges);
        assert_eq!(dist[GraphLinkEdgeType::CausalChain.as_u8() as usize], 1);
        assert_eq!(dist.iter().sum::<u32>(), 1);
    }

    #[test]
    fn mixed_distribution_sums_correctly() {
        let s = Uuid::new_v4();
        let edges = vec![
            mk_edge(s, Uuid::new_v4(), GraphLinkEdgeType::SemanticSimilar, 0.9),
            mk_edge(s, Uuid::new_v4(), GraphLinkEdgeType::SemanticSimilar, 0.85),
            mk_edge(s, Uuid::new_v4(), GraphLinkEdgeType::CodeRelated, 0.75),
            mk_edge(s, Uuid::new_v4(), GraphLinkEdgeType::CodeRelated, 0.72),
            mk_edge(s, Uuid::new_v4(), GraphLinkEdgeType::CausalChain, 0.65),
        ];
        let dist = compute_edge_type_distribution(&edges);
        let mut expected = [0u32; NUM_EDGE_TYPE_DISTRIBUTION];
        expected[GraphLinkEdgeType::SemanticSimilar.as_u8() as usize] = 2;
        expected[GraphLinkEdgeType::CodeRelated.as_u8() as usize] = 2;
        expected[GraphLinkEdgeType::CausalChain.as_u8() as usize] = 1;
        assert_eq!(dist, expected);
        assert_eq!(dist.iter().sum::<u32>(), 5);
    }
}
