// Inspired by ruvnet/RuVector crates/ruvector-mincut/src/canonical/* at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.
//
// Stoer-Wagner global minimum-cut algorithm.
//
// Algorithm reference:
//   Stoer & Wagner, "A Simple Min-Cut Algorithm", Journal of the ACM 44 (4),
//   1997. Doi 10.1145/263867.263872.
//
// Approach:
//   For an undirected, non-negatively-weighted graph G with n vertices, run
//   n-1 "phases." In each phase grow a Maximum Adjacency (MA) ordering by
//   repeatedly choosing the vertex u with the highest sum of edge weights
//   to the already-selected set S; the last vertex t added to S yields a
//   "cut-of-the-phase" w(t) equal to the sum of edges between t and S\{t}.
//   This cut separates t from the rest. Track the minimum over all phases
//   as the global mincut. Then merge t with the second-to-last added vertex,
//   reducing |V| by 1, and repeat.
//
//   Total complexity: O(V * E + V^2 * log V) with a binary heap, or
//   O(V^3) with the dense-graph linear-scan variant. We implement the
//   dense O(V^3) variant — adequate for the partition sizes ContextGraph
//   uses (typically tens to a few hundred nodes for A/B splits).
//
// Use cases (per `docs/ruvectorfindings/03 §6` and `04 §6`):
//   - ccreality A/B treatment/control split with minimum cross-set edge
//     weight (where edge weight = "tasks share failure mode").
//   - ME-JEPA train/test/calibration split with no shared-ancestor leakage.
//
// Fail-closed: graphs with negative weights are rejected; graphs with NaN
// weights are rejected; graphs with fewer than 2 vertices are rejected.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum MincutError {
    #[error("graph must have at least 2 vertices, got {got}")]
    GraphTooSmall { got: usize },
    #[error("weight matrix is not square: rows={rows} cols={cols}")]
    NonSquareMatrix { rows: usize, cols: usize },
    #[error("weight matrix has non-finite or negative entry at ({i},{j}): {value}")]
    NegativeOrNonFiniteWeight { i: usize, j: usize, value: f64 },
    #[error("weight matrix is not symmetric at ({i},{j}): w[{i},{j}]={a} != w[{j},{i}]={b}")]
    AsymmetricEdge {
        i: usize,
        j: usize,
        a: f64,
        b: f64,
    },
    #[error("self-loop weight at vertex {v} must be 0, got {weight}")]
    SelfLoopNonZero { v: usize, weight: f64 },
    #[error("graph is disconnected: best phase cut had zero weight at iteration {phase}")]
    GraphDisconnected { phase: usize },
}

impl MincutError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::GraphTooSmall { .. } => "MINCUT_GRAPH_TOO_SMALL",
            Self::NonSquareMatrix { .. } => "MINCUT_NON_SQUARE_MATRIX",
            Self::NegativeOrNonFiniteWeight { .. } => "MINCUT_WEIGHT_INVALID",
            Self::AsymmetricEdge { .. } => "MINCUT_ASYMMETRIC_EDGE",
            Self::SelfLoopNonZero { .. } => "MINCUT_SELF_LOOP_NONZERO",
            Self::GraphDisconnected { .. } => "MINCUT_GRAPH_DISCONNECTED",
        }
    }
}

/// One side of a global min-cut.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CutPartition {
    /// Original-indexed vertices on the "small" side of the cut (the side
    /// with the singleton seed vertex t at termination).
    pub small_side: Vec<usize>,
    /// Original-indexed vertices on the "large" side (the merged complement).
    pub large_side: Vec<usize>,
    /// Total weight of edges crossing the cut.
    pub cut_weight: f64,
    /// Phase index at which this cut was found (0-based).
    pub phase: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StoerWagnerConfig {
    /// Tolerance for symmetry checks. Default 1e-9.
    pub symmetry_tolerance: f64,
}

impl Default for StoerWagnerConfig {
    fn default() -> Self {
        Self {
            symmetry_tolerance: 1e-9,
        }
    }
}

/// Find the global minimum cut of an undirected, non-negatively-weighted
/// graph represented as a dense V×V symmetric weight matrix in row-major
/// layout (`weights[i*n + j]`).
///
/// Returns `Ok(CutPartition)` with the cut weight, the small side, and the
/// large side. Returns a structured error on any input violation.
pub fn stoer_wagner(
    weights: &[f64],
    n: usize,
    config: StoerWagnerConfig,
) -> Result<CutPartition, MincutError> {
    if n < 2 {
        return Err(MincutError::GraphTooSmall { got: n });
    }
    if weights.len() != n * n {
        return Err(MincutError::NonSquareMatrix {
            rows: weights.len() / n.max(1),
            cols: n,
        });
    }
    // Validate weights: finite, non-negative, symmetric, no self-loops.
    for i in 0..n {
        if weights[i * n + i] != 0.0 {
            return Err(MincutError::SelfLoopNonZero {
                v: i,
                weight: weights[i * n + i],
            });
        }
        for j in 0..n {
            let w = weights[i * n + j];
            if !w.is_finite() || w < 0.0 {
                return Err(MincutError::NegativeOrNonFiniteWeight { i, j, value: w });
            }
            let w_ji = weights[j * n + i];
            if (w - w_ji).abs() > config.symmetry_tolerance {
                return Err(MincutError::AsymmetricEdge {
                    i,
                    j,
                    a: w,
                    b: w_ji,
                });
            }
        }
    }

    // Active vertex tracking. `active[v]` is true while v has not been merged.
    // `members[v]` collects original-indexed members merged into v.
    // `mat` is the current dense weight matrix (mutated by merges).
    let mut active: Vec<bool> = vec![true; n];
    let mut members: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    let mut mat: Vec<f64> = weights.to_vec();

    let mut best_cut_weight = f64::INFINITY;
    let mut best_small_side: Vec<usize> = Vec::new();
    let mut best_large_side: Vec<usize> = Vec::new();
    let mut best_phase: usize = 0;

    // n - 1 phases. Each phase computes one cut-of-the-phase and merges
    // the last two vertices added in MA order.
    for phase in 0..(n - 1) {
        // Compute Maximum Adjacency ordering on the active set.
        // weights_to_set[v] = sum of edge weights between v and S so far.
        let mut weights_to_set = vec![0.0f64; n];
        let mut in_set = vec![false; n];

        // Pick a deterministic seed (lowest active index) to start S.
        let start = (0..n)
            .find(|&v| active[v])
            .expect("at least one active vertex");
        in_set[start] = true;
        for j in 0..n {
            if active[j] && !in_set[j] {
                weights_to_set[j] = mat[start * n + j];
            }
        }

        // Track last two vertices added in MA order: prev_last, last.
        let mut prev_last = start;
        let mut last = start;
        let mut prev_last_set = false;

        // Number of active vertices remaining is n - phase, we already
        // placed `start`, so we add (n - phase - 1) more.
        let active_count = active.iter().filter(|x| **x).count();
        for _ in 0..(active_count - 1) {
            // Pick the active, not-in-set vertex with the highest
            // weights_to_set. Deterministic tiebreak: lowest index.
            let mut best_v: Option<usize> = None;
            let mut best_w = f64::NEG_INFINITY;
            for v in 0..n {
                if active[v] && !in_set[v] && weights_to_set[v] > best_w {
                    best_w = weights_to_set[v];
                    best_v = Some(v);
                }
            }
            let chosen = best_v.expect("at least one candidate before final vertex");
            prev_last = last;
            prev_last_set = true;
            last = chosen;
            in_set[chosen] = true;
            for j in 0..n {
                if active[j] && !in_set[j] {
                    weights_to_set[j] += mat[chosen * n + j];
                }
            }
        }

        // Cut-of-the-phase: weights_to_set[last] is the sum of edges
        // between last and S \ {last} (S grew until last was added).
        // But weights_to_set[last] was overwritten when last entered S;
        // we must recompute the sum from the matrix instead.
        // Edges from `last` to all other active vertices (which are
        // S \ {last}) sum to the cut weight separating last from rest.
        let mut cut_weight_phase = 0.0;
        for j in 0..n {
            if active[j] && j != last {
                cut_weight_phase += mat[last * n + j];
            }
        }

        // The "small side" of THIS phase's cut is the singleton {last},
        // expanded to all original vertices merged into `last`.
        // The "large side" is everything else active in this phase.
        if cut_weight_phase < best_cut_weight {
            best_cut_weight = cut_weight_phase;
            best_small_side = members[last].clone();
            let mut large: Vec<usize> = Vec::new();
            for v in 0..n {
                if active[v] && v != last {
                    large.extend_from_slice(&members[v]);
                }
            }
            large.sort_unstable();
            best_large_side = large;
            best_small_side.sort_unstable();
            best_phase = phase;
        }

        if !prev_last_set {
            // Only one active vertex was present besides start (n=2 phase=0).
            // prev_last == last == start; just merge `last` into `start`'s
            // partner. With n=2 the "merge" is a no-op since the next loop
            // exits.
            prev_last = start;
        }

        // Merge `last` into `prev_last`: combine member lists, sum row+col,
        // mark `last` inactive.
        let last_members = std::mem::take(&mut members[last]);
        members[prev_last].extend(last_members);
        for j in 0..n {
            if j != prev_last && j != last {
                let combined = mat[prev_last * n + j] + mat[last * n + j];
                mat[prev_last * n + j] = combined;
                mat[j * n + prev_last] = combined;
            }
        }
        // Self-loop stays 0 by construction.
        mat[prev_last * n + last] = 0.0;
        mat[last * n + prev_last] = 0.0;
        for j in 0..n {
            mat[last * n + j] = 0.0;
            mat[j * n + last] = 0.0;
        }
        active[last] = false;
    }

    if best_cut_weight == 0.0 {
        return Err(MincutError::GraphDisconnected { phase: best_phase });
    }
    if !best_cut_weight.is_finite() {
        // Should be impossible after phase loop unless n was 1, which
        // we rejected. Defense in depth.
        return Err(MincutError::GraphTooSmall { got: n });
    }

    Ok(CutPartition {
        small_side: best_small_side,
        large_side: best_large_side,
        cut_weight: best_cut_weight,
        phase: best_phase,
    })
}

/// Convenience: build a symmetric weight matrix from an iterator of edges
/// `(i, j, weight)` over `n` vertices. Caller-provided edges accumulate
/// (parallel edges' weights sum).
pub fn weights_from_edges<I>(n: usize, edges: I) -> Vec<f64>
where
    I: IntoIterator<Item = (usize, usize, f64)>,
{
    let mut w = vec![0.0; n * n];
    for (i, j, weight) in edges {
        if i < n && j < n && i != j {
            w[i * n + j] += weight;
            w[j * n + i] += weight;
        }
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_too_small_graph() {
        let err = stoer_wagner(&[0.0], 1, StoerWagnerConfig::default())
            .expect_err("n=1 must error");
        assert_eq!(err.code(), "MINCUT_GRAPH_TOO_SMALL");
    }

    #[test]
    fn rejects_non_square_matrix() {
        let err = stoer_wagner(&[0.0, 1.0, 1.0], 2, StoerWagnerConfig::default())
            .expect_err("3 entries for n=2 must error");
        assert_eq!(err.code(), "MINCUT_NON_SQUARE_MATRIX");
    }

    #[test]
    fn rejects_negative_weight() {
        let weights = vec![0.0, -1.0, -1.0, 0.0];
        let err = stoer_wagner(&weights, 2, StoerWagnerConfig::default())
            .expect_err("negative weight must error");
        assert_eq!(err.code(), "MINCUT_WEIGHT_INVALID");
    }

    #[test]
    fn rejects_nan_weight() {
        let weights = vec![0.0, f64::NAN, f64::NAN, 0.0];
        let err = stoer_wagner(&weights, 2, StoerWagnerConfig::default())
            .expect_err("NaN weight must error");
        assert_eq!(err.code(), "MINCUT_WEIGHT_INVALID");
    }

    #[test]
    fn rejects_asymmetric_matrix() {
        let weights = vec![0.0, 1.0, 2.0, 0.0];
        let err = stoer_wagner(&weights, 2, StoerWagnerConfig::default())
            .expect_err("asymmetric must error");
        assert_eq!(err.code(), "MINCUT_ASYMMETRIC_EDGE");
    }

    #[test]
    fn rejects_self_loop() {
        let weights = vec![1.0, 1.0, 1.0, 0.0];
        let err = stoer_wagner(&weights, 2, StoerWagnerConfig::default())
            .expect_err("self-loop must error");
        assert_eq!(err.code(), "MINCUT_SELF_LOOP_NONZERO");
    }

    #[test]
    fn two_vertex_graph_cuts_at_single_edge() {
        // Two vertices with one edge of weight 5.
        let weights = vec![0.0, 5.0, 5.0, 0.0];
        let cut = stoer_wagner(&weights, 2, StoerWagnerConfig::default()).unwrap();
        assert_eq!(cut.cut_weight, 5.0);
        assert_eq!(cut.small_side.len() + cut.large_side.len(), 2);
    }

    #[test]
    fn known_example_from_stoer_wagner_paper() {
        // The 8-vertex example graph from Figure 1 of the original paper.
        // Vertices 1..=8 in the paper become 0..=7 here.
        // Expected global mincut weight: 4.
        let edges = [
            (0, 1, 2.0),
            (0, 4, 3.0),
            (1, 2, 3.0),
            (1, 4, 2.0),
            (1, 5, 2.0),
            (2, 3, 4.0),
            (2, 6, 2.0),
            (3, 6, 2.0),
            (3, 7, 2.0),
            (4, 5, 3.0),
            (5, 6, 1.0),
            (6, 7, 3.0),
        ];
        let weights = weights_from_edges(8, edges.iter().copied());
        let cut = stoer_wagner(&weights, 8, StoerWagnerConfig::default()).unwrap();
        assert_eq!(cut.cut_weight, 4.0);
        assert_eq!(cut.small_side.len() + cut.large_side.len(), 8);
    }

    #[test]
    fn dumbbell_graph_cuts_at_bridge() {
        // Two K3 cliques joined by a single bridge of weight 1.
        // Cliques: {0,1,2} and {3,4,5} with intra-clique weights 10.
        // Bridge: (2,3) with weight 1. Mincut should be 1, separating
        // {0,1,2} from {3,4,5}.
        let edges = [
            (0, 1, 10.0),
            (0, 2, 10.0),
            (1, 2, 10.0),
            (2, 3, 1.0),
            (3, 4, 10.0),
            (3, 5, 10.0),
            (4, 5, 10.0),
        ];
        let weights = weights_from_edges(6, edges.iter().copied());
        let cut = stoer_wagner(&weights, 6, StoerWagnerConfig::default()).unwrap();
        assert_eq!(cut.cut_weight, 1.0);
        let mut combined: Vec<usize> = cut
            .small_side
            .iter()
            .chain(cut.large_side.iter())
            .copied()
            .collect();
        combined.sort();
        assert_eq!(combined, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn complete_graph_mincut_isolates_one_vertex() {
        // K4 with all unit weights. Min cut isolates one vertex with
        // weight 3 (3 edges to the rest, each weight 1).
        let mut weights = vec![1.0; 16];
        for i in 0..4 {
            weights[i * 4 + i] = 0.0;
        }
        let cut = stoer_wagner(&weights, 4, StoerWagnerConfig::default()).unwrap();
        assert_eq!(cut.cut_weight, 3.0);
        let small = cut.small_side.len();
        let large = cut.large_side.len();
        assert!(
            (small == 1 && large == 3) || (small == 3 && large == 1),
            "K4 mincut isolates a single vertex: got small={small}, large={large}"
        );
    }

    #[test]
    fn rejects_disconnected_graph() {
        // Two components: {0,1} and {2,3}. Mincut weight = 0.
        let edges = [(0, 1, 1.0), (2, 3, 1.0)];
        let weights = weights_from_edges(4, edges.iter().copied());
        let err = stoer_wagner(&weights, 4, StoerWagnerConfig::default())
            .expect_err("disconnected graph must error");
        assert_eq!(err.code(), "MINCUT_GRAPH_DISCONNECTED");
    }

    #[test]
    fn deterministic_across_runs() {
        let edges = [
            (0, 1, 1.0),
            (1, 2, 1.0),
            (2, 0, 1.0),
            (2, 3, 0.5),
            (3, 4, 1.0),
            (3, 5, 1.0),
            (4, 5, 1.0),
        ];
        let weights = weights_from_edges(6, edges.iter().copied());
        let a = stoer_wagner(&weights, 6, StoerWagnerConfig::default()).unwrap();
        let b = stoer_wagner(&weights, 6, StoerWagnerConfig::default()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn weights_from_edges_accumulates_parallel() {
        let w = weights_from_edges(3, [(0, 1, 1.0), (0, 1, 2.5), (1, 2, 0.5)].iter().copied());
        assert_eq!(w[0 * 3 + 1], 3.5);
        assert_eq!(w[1 * 3 + 0], 3.5);
        assert_eq!(w[1 * 3 + 2], 0.5);
    }

    #[test]
    fn small_balanced_cluster_split() {
        // 6 nodes: cluster A {0,1,2} densely connected; cluster B {3,4,5}
        // densely connected; weak edges between clusters.
        // Mincut should reveal {0,1,2} vs {3,4,5} with weight =
        // sum of inter-cluster weights.
        let edges = [
            (0, 1, 5.0),
            (0, 2, 5.0),
            (1, 2, 5.0),
            (3, 4, 5.0),
            (3, 5, 5.0),
            (4, 5, 5.0),
            (0, 3, 1.0),
            (1, 4, 0.5),
            (2, 5, 0.5),
        ];
        let weights = weights_from_edges(6, edges.iter().copied());
        let cut = stoer_wagner(&weights, 6, StoerWagnerConfig::default()).unwrap();
        // The minimum cut weight should be 2.0 (1 + 0.5 + 0.5).
        assert!(
            (cut.cut_weight - 2.0).abs() < 1e-9,
            "expected cut weight ~2.0, got {}",
            cut.cut_weight
        );
    }
}
