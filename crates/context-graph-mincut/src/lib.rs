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
    #[error("weight matrix size overflow for n={n}")]
    MatrixSizeOverflow { n: usize },
    #[error("symmetry tolerance must be finite and non-negative, got {tolerance}")]
    InvalidSymmetryTolerance { tolerance: f64 },
    #[error("edge endpoint out of bounds: edge=({i},{j}) n={n}")]
    EdgeEndpointOutOfBounds { i: usize, j: usize, n: usize },
    #[error("self-loop edge rejected at vertex {v}: weight={weight}")]
    SelfLoopEdge { v: usize, weight: f64 },
    #[error("weight matrix has non-finite or negative entry at ({i},{j}): {value}")]
    NegativeOrNonFiniteWeight { i: usize, j: usize, value: f64 },
    #[error("weight matrix is not symmetric at ({i},{j}): w[{i},{j}]={a} != w[{j},{i}]={b}")]
    AsymmetricEdge { i: usize, j: usize, a: f64, b: f64 },
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
            Self::MatrixSizeOverflow { .. } => "MINCUT_MATRIX_SIZE_OVERFLOW",
            Self::InvalidSymmetryTolerance { .. } => "MINCUT_INVALID_SYMMETRY_TOLERANCE",
            Self::EdgeEndpointOutOfBounds { .. } => "MINCUT_EDGE_ENDPOINT_OUT_OF_BOUNDS",
            Self::SelfLoopEdge { .. } => "MINCUT_SELF_LOOP_EDGE",
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
    if !config.symmetry_tolerance.is_finite() || config.symmetry_tolerance < 0.0 {
        return Err(MincutError::InvalidSymmetryTolerance {
            tolerance: config.symmetry_tolerance,
        });
    }
    let expected_len = n
        .checked_mul(n)
        .ok_or(MincutError::MatrixSizeOverflow { n })?;
    if weights.len() != expected_len {
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
        let mut cut_weight_phase = 0.0;

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
            last = chosen;
            cut_weight_phase = best_w;
            in_set[chosen] = true;
            for j in 0..n {
                if active[j] && !in_set[j] {
                    weights_to_set[j] += mat[chosen * n + j];
                }
            }
        }

        // Cut-of-the-phase: at the moment `last` was selected, `best_w`
        // was the sum of edges from `last` to S \ {last}.

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

/// Build a symmetric weight matrix from `(i, j, weight)` edges over `n`
/// vertices. Caller-provided parallel edges accumulate. Invalid endpoints,
/// self-loops, negative weights, non-finite weights, and accumulation
/// overflow are rejected instead of silently dropping graph structure.
pub fn weights_from_edges<I>(n: usize, edges: I) -> Result<Vec<f64>, MincutError>
where
    I: IntoIterator<Item = (usize, usize, f64)>,
{
    if n < 2 {
        return Err(MincutError::GraphTooSmall { got: n });
    }
    let len = n
        .checked_mul(n)
        .ok_or(MincutError::MatrixSizeOverflow { n })?;
    let mut w = vec![0.0; len];
    for (i, j, weight) in edges {
        if i >= n || j >= n {
            return Err(MincutError::EdgeEndpointOutOfBounds { i, j, n });
        }
        if i == j {
            return Err(MincutError::SelfLoopEdge { v: i, weight });
        }
        if !weight.is_finite() || weight < 0.0 {
            return Err(MincutError::NegativeOrNonFiniteWeight {
                i,
                j,
                value: weight,
            });
        }
        let forward = w[i * n + j] + weight;
        let reverse = w[j * n + i] + weight;
        if !forward.is_finite() {
            return Err(MincutError::NegativeOrNonFiniteWeight {
                i,
                j,
                value: forward,
            });
        }
        if !reverse.is_finite() {
            return Err(MincutError::NegativeOrNonFiniteWeight {
                i: j,
                j: i,
                value: reverse,
            });
        }
        w[i * n + j] = forward;
        w[j * n + i] = reverse;
    }
    Ok(w)
}

#[cfg(test)]
mod tests;
