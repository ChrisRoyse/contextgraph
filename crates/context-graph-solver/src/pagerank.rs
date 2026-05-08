// Inspired by ruvnet/RuVector crates/ruvector-solver/src/forward_push.rs at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.
//
// Algorithm reference used for the clean-room implementation:
// Andersen, Chung, and Lang, "Local Graph Partitioning using PageRank Vectors".

use crate::csr::{CsrMatrix, MatrixKind};
use crate::error::{SolverError, SolverResult};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ForwardPushConfig {
    pub alpha: f64,
    pub tolerance: f64,
    pub max_pushes: usize,
}

impl Default for ForwardPushConfig {
    fn default() -> Self {
        Self {
            alpha: 0.15,
            tolerance: 1e-8,
            max_pushes: 1_000_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankedNode {
    pub node: usize,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForwardPushReport {
    pub estimate: Vec<f64>,
    pub residual: Vec<f64>,
    pub ranked: Vec<RankedNode>,
    pub pushes: usize,
    pub residual_l1: f64,
    pub estimate_l1: f64,
    pub total_mass: f64,
}

#[derive(Debug, Clone)]
pub struct ForwardPushSolver {
    config: ForwardPushConfig,
}

impl ForwardPushSolver {
    pub fn new(config: ForwardPushConfig) -> SolverResult<Self> {
        validate_config(config)?;
        Ok(Self { config })
    }

    pub fn config(&self) -> ForwardPushConfig {
        self.config
    }

    pub fn solve_from_seed(
        &self,
        graph: &CsrMatrix,
        seed: usize,
    ) -> SolverResult<ForwardPushReport> {
        self.solve_from_distribution(graph, &[(seed, 1.0)])
    }

    pub fn top_k(&self, graph: &CsrMatrix, seed: usize, k: usize) -> SolverResult<Vec<RankedNode>> {
        if k == 0 {
            return Err(SolverError::invalid(
                "k",
                "top_k requires k > 0",
                "request at least one ranked node",
            ));
        }
        let mut ranked = self.solve_from_seed(graph, seed)?.ranked;
        ranked.truncate(k);
        Ok(ranked)
    }

    pub fn solve_from_distribution(
        &self,
        graph: &CsrMatrix,
        seeds: &[(usize, f64)],
    ) -> SolverResult<ForwardPushReport> {
        graph.validate()?;
        if graph.rows != graph.cols {
            return Err(SolverError::invalid(
                "graph",
                format!(
                    "PPR requires a square adjacency matrix, got {}x{}",
                    graph.rows, graph.cols
                ),
                "build the influence graph with one row and column per node",
            ));
        }
        if !matches!(graph.kind, MatrixKind::NonNegativeAdjacency) {
            return Err(SolverError::invalid(
                "graph.kind",
                "Forward-Push PPR requires MatrixKind::NonNegativeAdjacency",
                "construct the graph with non-negative adjacency semantics",
            ));
        }
        if seeds.is_empty() {
            return Err(SolverError::invalid(
                "seeds",
                "seed distribution is empty",
                "provide at least one seed vertex with positive mass",
            ));
        }

        let n = graph.rows;
        let mut residual = vec![0.0; n];
        let mut estimate = vec![0.0; n];
        let mut total_seed_mass = 0.0;
        for (idx, &(node, mass)) in seeds.iter().enumerate() {
            if node >= n {
                return Err(SolverError::invalid(
                    "seeds.node",
                    format!("seed {idx} node {node} is outside graph node count {n}"),
                    "fix seed selection before invoking PPR",
                ));
            }
            if !mass.is_finite() || mass <= 0.0 {
                return Err(SolverError::invalid(
                    "seeds.mass",
                    format!("seed {idx} mass must be finite and > 0, got {mass}"),
                    "normalize seed weights into a positive distribution",
                ));
            }
            residual[node] += mass;
            total_seed_mass += mass;
        }
        if total_seed_mass <= 0.0 || !total_seed_mass.is_finite() {
            return Err(SolverError::invalid(
                "seeds",
                format!("total seed mass must be finite and > 0, got {total_seed_mass}"),
                "normalize seed weights into a positive distribution",
            ));
        }
        for value in &mut residual {
            *value /= total_seed_mass;
        }
        total_seed_mass = 1.0;

        let row_sums = (0..n).map(|row| graph.row_sum(row)).collect::<Vec<_>>();
        let mut in_queue = vec![false; n];
        let mut queue = VecDeque::new();
        for node in 0..n {
            if should_push(residual[node], row_sums[node], self.config.tolerance) {
                queue.push_back(node);
                in_queue[node] = true;
            }
        }

        let mut pushes = 0usize;
        while let Some(node) = queue.pop_front() {
            in_queue[node] = false;
            if !should_push(residual[node], row_sums[node], self.config.tolerance) {
                continue;
            }
            if pushes >= self.config.max_pushes {
                return Err(SolverError::DidNotConverge {
                    message: format!(
                        "Forward-Push exceeded max_pushes={} with residual_l1={}",
                        self.config.max_pushes,
                        residual.iter().sum::<f64>()
                    ),
                    remediation: "raise max_pushes only after inspecting graph degree distribution and tolerance",
                });
            }
            pushes += 1;
            let mass = residual[node];
            residual[node] = 0.0;
            estimate[node] += self.config.alpha * mass;

            let distributable = (1.0 - self.config.alpha) * mass;
            if row_sums[node] == 0.0 {
                estimate[node] += distributable;
                continue;
            }
            for (neighbor, weight) in graph.row_entries(node) {
                let delta = distributable * (weight / row_sums[node]);
                residual[neighbor] += delta;
                if !in_queue[neighbor]
                    && should_push(
                        residual[neighbor],
                        row_sums[neighbor],
                        self.config.tolerance,
                    )
                {
                    queue.push_back(neighbor);
                    in_queue[neighbor] = true;
                }
            }
        }

        let estimate_l1 = estimate.iter().sum::<f64>();
        let residual_l1 = residual.iter().sum::<f64>();
        let total_mass = estimate_l1 + residual_l1;
        let allowed_drift = self.config.tolerance.max(1e-10) * (n as f64).max(1.0) * 8.0;
        if (total_mass - total_seed_mass).abs() > allowed_drift {
            return Err(SolverError::invariant(
                "mass",
                format!(
                    "PPR mass drifted: estimate_l1={estimate_l1} residual_l1={residual_l1} total={total_mass}"
                ),
                "inspect graph weights and dangling-node policy before trusting influence scores",
            ));
        }

        let mut ranked = estimate
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, score)| *score > 0.0)
            .map(|(node, score)| RankedNode { node, score })
            .collect::<Vec<_>>();
        ranked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.node.cmp(&b.node))
        });

        Ok(ForwardPushReport {
            estimate,
            residual,
            ranked,
            pushes,
            residual_l1,
            estimate_l1,
            total_mass,
        })
    }
}

fn validate_config(config: ForwardPushConfig) -> SolverResult<()> {
    if !config.alpha.is_finite() || !(0.0..1.0).contains(&config.alpha) {
        return Err(SolverError::invalid(
            "alpha",
            format!("alpha must be finite and in (0, 1), got {}", config.alpha),
            "use the standard teleport probability range; 0.15 is the default",
        ));
    }
    if !config.tolerance.is_finite() || config.tolerance <= 0.0 {
        return Err(SolverError::invalid(
            "tolerance",
            format!("tolerance must be finite and > 0, got {}", config.tolerance),
            "use a positive residual threshold such as 1e-8",
        ));
    }
    if config.max_pushes == 0 {
        return Err(SolverError::invalid(
            "max_pushes",
            "max_pushes must be greater than zero",
            "set an explicit positive compute budget",
        ));
    }
    Ok(())
}

fn should_push(residual: f64, row_sum: f64, tolerance: f64) -> bool {
    let scale = row_sum.max(1.0);
    residual > tolerance * scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_node_symmetric_graph_matches_closed_form() {
        let graph = CsrMatrix::from_edges(
            2,
            2,
            MatrixKind::NonNegativeAdjacency,
            &[(0, 1, 1.0), (1, 0, 1.0)],
        )
        .expect("graph");
        let solver = ForwardPushSolver::new(ForwardPushConfig {
            alpha: 0.5,
            tolerance: 1e-12,
            max_pushes: 10_000,
        })
        .expect("solver");
        let report = solver.solve_from_seed(&graph, 0).expect("ppr");
        println!(
            "PPR_SOURCE_OF_TRUTH estimate={:?} residual_l1={} total_mass={}",
            report.estimate, report.residual_l1, report.total_mass
        );
        assert!((report.estimate[0] - (2.0 / 3.0)).abs() < 1e-9);
        assert!((report.estimate[1] - (1.0 / 3.0)).abs() < 1e-9);
        assert!((report.total_mass - 1.0).abs() < 1e-9);
    }

    #[test]
    fn invalid_negative_adjacency_fails_closed() {
        let err = CsrMatrix::from_edges(2, 2, MatrixKind::NonNegativeAdjacency, &[(0, 1, -1.0)])
            .expect_err("negative adjacency must fail");
        println!(
            "PPR_EDGE_CASE_NEGATIVE before=negative_edge after={}",
            err.code()
        );
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }
}
