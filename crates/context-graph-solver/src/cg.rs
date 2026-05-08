// Inspired by ruvnet/RuVector crates/ruvector-solver/src/cg.rs at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.

use crate::csr::{CsrMatrix, MatrixKind};
use crate::error::{SolverError, SolverResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ConjugateGradientConfig {
    pub tolerance: f64,
    pub max_iterations: usize,
    pub validate_symmetry: bool,
    pub symmetry_tolerance: f64,
}

impl Default for ConjugateGradientConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-10,
            max_iterations: 1_000,
            validate_symmetry: true,
            symmetry_tolerance: 1e-10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConjugateGradientReport {
    pub solution: Vec<f64>,
    pub residual_norm: f64,
    pub rhs_norm: f64,
    pub iterations: usize,
    pub converged: bool,
}

#[derive(Debug, Clone)]
pub struct ConjugateGradientSolver {
    config: ConjugateGradientConfig,
}

impl ConjugateGradientSolver {
    pub fn new(config: ConjugateGradientConfig) -> SolverResult<Self> {
        validate_config(config)?;
        Ok(Self { config })
    }

    pub fn config(&self) -> ConjugateGradientConfig {
        self.config
    }

    pub fn solve(&self, matrix: &CsrMatrix, rhs: &[f64]) -> SolverResult<ConjugateGradientReport> {
        self.solve_with_initial(matrix, rhs, None)
    }

    pub fn solve_with_initial(
        &self,
        matrix: &CsrMatrix,
        rhs: &[f64],
        initial: Option<&[f64]>,
    ) -> SolverResult<ConjugateGradientReport> {
        validate_system(matrix, rhs, initial, self.config)?;
        let n = matrix.rows;
        if rhs.iter().all(|value| *value == 0.0) {
            return Ok(ConjugateGradientReport {
                solution: vec![0.0; n],
                residual_norm: 0.0,
                rhs_norm: 0.0,
                iterations: 0,
                converged: true,
            });
        }

        let mut x = initial.map_or_else(|| vec![0.0; n], |value| value.to_vec());
        let ax = matrix.spmv(&x)?;
        let mut residual = rhs
            .iter()
            .zip(ax.iter())
            .map(|(b, ax)| b - ax)
            .collect::<Vec<_>>();
        let mut direction = residual.clone();
        let mut residual_dot = dot(&residual, &residual);
        let rhs_norm = dot(rhs, rhs).sqrt();
        let target = self.config.tolerance * rhs_norm.max(1.0);
        let initial_residual_norm = residual_dot.sqrt();
        if initial_residual_norm <= target {
            return Ok(ConjugateGradientReport {
                solution: x,
                residual_norm: initial_residual_norm,
                rhs_norm,
                iterations: 0,
                converged: true,
            });
        }

        for iteration in 1..=self.config.max_iterations {
            let ad = matrix.spmv(&direction)?;
            let denom = dot(&direction, &ad);
            if !denom.is_finite() || denom <= 0.0 {
                return Err(SolverError::invariant(
                    "direction_dot_A_direction",
                    format!("CG encountered non-positive curvature {denom} at iteration {iteration}"),
                    "verify the matrix is symmetric positive-definite; CG is not valid for indefinite systems",
                ));
            }
            let step = residual_dot / denom;
            if !step.is_finite() {
                return Err(SolverError::invariant(
                    "step",
                    format!("CG step became non-finite at iteration {iteration}"),
                    "inspect matrix conditioning and RHS magnitude",
                ));
            }
            for idx in 0..n {
                x[idx] += step * direction[idx];
                residual[idx] -= step * ad[idx];
            }
            let next_residual_dot = dot(&residual, &residual);
            let residual_norm = next_residual_dot.sqrt();
            if residual_norm <= target {
                return Ok(ConjugateGradientReport {
                    solution: x,
                    residual_norm,
                    rhs_norm,
                    iterations: iteration,
                    converged: true,
                });
            }
            let beta = next_residual_dot / residual_dot;
            if !beta.is_finite() {
                return Err(SolverError::invariant(
                    "beta",
                    format!("CG beta became non-finite at iteration {iteration}"),
                    "inspect matrix conditioning and RHS magnitude",
                ));
            }
            for idx in 0..n {
                direction[idx] = residual[idx] + beta * direction[idx];
            }
            residual_dot = next_residual_dot;
        }

        Err(SolverError::DidNotConverge {
            message: format!(
                "CG did not reach residual target {target} after {} iterations; final residual={}",
                self.config.max_iterations,
                residual_dot.sqrt()
            ),
            remediation: "increase max_iterations only after inspecting conditioning; otherwise use a better-conditioned SPD system",
        })
    }
}

fn validate_config(config: ConjugateGradientConfig) -> SolverResult<()> {
    if !config.tolerance.is_finite() || config.tolerance <= 0.0 {
        return Err(SolverError::invalid(
            "tolerance",
            format!("tolerance must be finite and > 0, got {}", config.tolerance),
            "use a positive residual tolerance such as 1e-10",
        ));
    }
    if config.max_iterations == 0 {
        return Err(SolverError::invalid(
            "max_iterations",
            "max_iterations must be greater than zero",
            "set an explicit positive compute budget",
        ));
    }
    if !config.symmetry_tolerance.is_finite() || config.symmetry_tolerance < 0.0 {
        return Err(SolverError::invalid(
            "symmetry_tolerance",
            format!(
                "symmetry_tolerance must be finite and non-negative, got {}",
                config.symmetry_tolerance
            ),
            "use a non-negative validation tolerance",
        ));
    }
    Ok(())
}

fn validate_system(
    matrix: &CsrMatrix,
    rhs: &[f64],
    initial: Option<&[f64]>,
    config: ConjugateGradientConfig,
) -> SolverResult<()> {
    matrix.validate()?;
    if !matches!(matrix.kind, MatrixKind::SymmetricPositiveDefinite) {
        return Err(SolverError::invalid(
            "matrix.kind",
            "CG requires MatrixKind::SymmetricPositiveDefinite",
            "only call CG on systems that were constructed and validated as SPD",
        ));
    }
    if matrix.rows != matrix.cols {
        return Err(SolverError::invalid(
            "matrix.dimensions",
            format!(
                "CG requires a square matrix, got {}x{}",
                matrix.rows, matrix.cols
            ),
            "construct a square SPD matrix",
        ));
    }
    if rhs.len() != matrix.rows {
        return Err(SolverError::invalid(
            "rhs",
            format!(
                "rhs length {} does not equal matrix rows {}",
                rhs.len(),
                matrix.rows
            ),
            "pass a RHS vector with one value per matrix row",
        ));
    }
    for (idx, value) in rhs.iter().copied().enumerate() {
        if !value.is_finite() {
            return Err(SolverError::invalid(
                "rhs",
                format!("rhs[{idx}] is non-finite: {value}"),
                "remove NaN/Inf at the caller boundary",
            ));
        }
    }
    if let Some(initial) = initial {
        if initial.len() != matrix.rows {
            return Err(SolverError::invalid(
                "initial",
                format!(
                    "initial solution length {} does not equal matrix rows {}",
                    initial.len(),
                    matrix.rows
                ),
                "pass one initial value per matrix row",
            ));
        }
        for (idx, value) in initial.iter().copied().enumerate() {
            if !value.is_finite() {
                return Err(SolverError::invalid(
                    "initial",
                    format!("initial[{idx}] is non-finite: {value}"),
                    "remove NaN/Inf at the caller boundary",
                ));
            }
        }
    }
    let diagonal = matrix.diagonal();
    for (idx, value) in diagonal.iter().copied().enumerate() {
        if value <= 0.0 {
            return Err(SolverError::invalid(
                "matrix.diagonal",
                format!("diagonal entry {idx} is not positive: {value}"),
                "CG requires a positive diagonal in the SPD system",
            ));
        }
    }
    if config.validate_symmetry {
        validate_symmetry(matrix, config.symmetry_tolerance)?;
    }
    Ok(())
}

fn validate_symmetry(matrix: &CsrMatrix, tolerance: f64) -> SolverResult<()> {
    let mut entries = BTreeMap::new();
    for row in 0..matrix.rows {
        for (col, value) in matrix.row_entries(row) {
            entries.insert((row, col), value);
        }
    }
    for row in 0..matrix.rows {
        for (col, value) in matrix.row_entries(row) {
            let mirror = entries.get(&(col, row)).copied().unwrap_or(0.0);
            if (value - mirror).abs() > tolerance {
                return Err(SolverError::invalid(
                    "matrix.symmetry",
                    format!(
                        "matrix[{row},{col}]={value} does not match matrix[{col},{row}]={mirror}"
                    ),
                    "CG requires symmetric input; fix graph-to-matrix construction or use a non-CG solver",
                ));
            }
        }
    }
    Ok(())
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    debug_assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b.iter())
        .map(|(left, right)| left * right)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solves_known_two_by_two_spd_system() {
        let matrix = CsrMatrix::from_edges(
            2,
            2,
            MatrixKind::SymmetricPositiveDefinite,
            &[(0, 0, 4.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, 3.0)],
        )
        .expect("matrix");
        let solver = ConjugateGradientSolver::new(ConjugateGradientConfig::default()).expect("cg");
        let report = solver.solve(&matrix, &[1.0, 2.0]).expect("solve");
        println!(
            "CG_SOURCE_OF_TRUTH solution={:?} residual_norm={}",
            report.solution, report.residual_norm
        );
        assert!((report.solution[0] - (1.0 / 11.0)).abs() < 1e-9);
        assert!((report.solution[1] - (7.0 / 11.0)).abs() < 1e-9);
        assert!(report.converged);
    }

    #[test]
    fn unsymmetric_spd_claim_fails_closed() {
        let matrix = CsrMatrix::from_edges(
            2,
            2,
            MatrixKind::SymmetricPositiveDefinite,
            &[(0, 0, 2.0), (0, 1, 1.0), (1, 1, 2.0)],
        )
        .expect("matrix");
        let solver = ConjugateGradientSolver::new(ConjugateGradientConfig::default()).expect("cg");
        let err = solver
            .solve(&matrix, &[1.0, 1.0])
            .expect_err("unsymmetric must fail");
        println!(
            "CG_EDGE_CASE_UNSYMMETRIC before=unsymmetric_spd_claim after={}",
            err.code()
        );
        assert_eq!(err.code(), "CGSOLVER_INVALID_INPUT");
    }
}
