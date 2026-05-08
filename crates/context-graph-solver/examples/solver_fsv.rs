// Inspired by ruvnet/RuVector crates/ruvector-solver/src/{forward_push,cg}.rs at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.

use context_graph_solver::{
    ConjugateGradientConfig, ConjugateGradientSolver, CsrMatrix, ForwardPushConfig,
    ForwardPushSolver, MatrixKind, SolverError,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
struct CaseEvidence {
    name: String,
    trigger: serde_json::Value,
    before_state: serde_json::Value,
    after_state: serde_json::Value,
    expected: serde_json::Value,
    passed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Evidence {
    operation: String,
    status: String,
    source_of_truth: String,
    cases: Vec<CaseEvidence>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("SOLVER_FSV_FAILED: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let output = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/solver-fsv-evidence.json"));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let cases = vec![
        happy_path_ppr()?,
        happy_path_cg()?,
        edge_empty_graph(),
        edge_invalid_tolerance(),
        edge_unsymmetric_cg()?,
    ];
    let passed = cases.iter().all(|case| case.passed);
    let evidence = Evidence {
        operation: "context_graph_solver_full_state_verification".to_string(),
        status: if passed { "ok" } else { "failed" }.to_string(),
        source_of_truth: output.display().to_string(),
        cases,
    };
    let bytes = serde_json::to_vec_pretty(&evidence)?;
    fs::write(&output, bytes)?;
    let readback = fs::read(&output)?;
    let stored: Evidence = serde_json::from_slice(&readback)?;
    println!(
        "SOLVER_FSV_SOURCE_OF_TRUTH path={} status={} cases={}",
        output.display(),
        stored.status,
        stored.cases.len()
    );
    println!("{}", serde_json::to_string_pretty(&stored)?);
    if stored.status != "ok" {
        return Err("one or more FSV cases failed".into());
    }
    Ok(())
}

fn happy_path_ppr() -> Result<CaseEvidence, Box<dyn std::error::Error>> {
    let graph = CsrMatrix::from_edges(
        2,
        2,
        MatrixKind::NonNegativeAdjacency,
        &[(0, 1, 1.0), (1, 0, 1.0)],
    )?;
    let solver = ForwardPushSolver::new(ForwardPushConfig {
        alpha: 0.5,
        tolerance: 1e-12,
        max_pushes: 10_000,
    })?;
    let report = solver.solve_from_seed(&graph, 0)?;
    let expected_0 = 2.0 / 3.0;
    let expected_1 = 1.0 / 3.0;
    let passed = (report.estimate[0] - expected_0).abs() < 1e-9
        && (report.estimate[1] - expected_1).abs() < 1e-9
        && (report.total_mass - 1.0).abs() < 1e-9;
    Ok(CaseEvidence {
        name: "happy_path_ppr_two_node_closed_form".to_string(),
        trigger: json!({"algorithm": "forward_push_ppr", "seed": 0}),
        before_state: json!({"rows": graph.rows, "cols": graph.cols, "edges": [[0, 1, 1.0], [1, 0, 1.0]], "alpha": 0.5}),
        after_state: json!({"estimate": report.estimate, "residual_l1": report.residual_l1, "total_mass": report.total_mass, "pushes": report.pushes}),
        expected: json!({"estimate[0]": expected_0, "estimate[1]": expected_1, "total_mass": 1.0}),
        passed,
    })
}

fn happy_path_cg() -> Result<CaseEvidence, Box<dyn std::error::Error>> {
    let matrix = CsrMatrix::from_edges(
        2,
        2,
        MatrixKind::SymmetricPositiveDefinite,
        &[(0, 0, 4.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, 3.0)],
    )?;
    let solver = ConjugateGradientSolver::new(ConjugateGradientConfig::default())?;
    let report = solver.solve(&matrix, &[1.0, 2.0])?;
    let expected_x0 = 1.0 / 11.0;
    let expected_x1 = 7.0 / 11.0;
    let passed = (report.solution[0] - expected_x0).abs() < 1e-9
        && (report.solution[1] - expected_x1).abs() < 1e-9
        && report.converged;
    Ok(CaseEvidence {
        name: "happy_path_cg_two_by_two_closed_form".to_string(),
        trigger: json!({"algorithm": "conjugate_gradient", "rhs": [1.0, 2.0]}),
        before_state: json!({"matrix": [[4.0, 1.0], [1.0, 3.0]], "rhs": [1.0, 2.0]}),
        after_state: json!({"solution": report.solution, "residual_norm": report.residual_norm, "iterations": report.iterations, "converged": report.converged}),
        expected: json!({"solution": [expected_x0, expected_x1]}),
        passed,
    })
}

fn edge_empty_graph() -> CaseEvidence {
    let result = CsrMatrix::from_edges(0, 0, MatrixKind::NonNegativeAdjacency, &[]);
    let passed = matches!(result, Err(SolverError::InvalidInput { .. }));
    CaseEvidence {
        name: "edge_empty_graph_fails_closed".to_string(),
        trigger: json!({"algorithm": "csr_from_edges", "rows": 0, "cols": 0}),
        before_state: json!({"rows": 0, "cols": 0, "edges": []}),
        after_state: json!({"error_code": result.expect_err("empty graph must fail").code()}),
        expected: json!({"error_code": "CGSOLVER_INVALID_INPUT"}),
        passed,
    }
}

fn edge_invalid_tolerance() -> CaseEvidence {
    let result = ForwardPushSolver::new(ForwardPushConfig {
        alpha: 0.15,
        tolerance: 0.0,
        max_pushes: 10,
    });
    let passed = matches!(result, Err(SolverError::InvalidInput { .. }));
    CaseEvidence {
        name: "edge_zero_tolerance_fails_closed".to_string(),
        trigger: json!({"algorithm": "forward_push_ppr", "tolerance": 0.0}),
        before_state: json!({"alpha": 0.15, "tolerance": 0.0, "max_pushes": 10}),
        after_state: json!({"error_code": result.expect_err("zero tolerance must fail").code()}),
        expected: json!({"error_code": "CGSOLVER_INVALID_INPUT"}),
        passed,
    }
}

fn edge_unsymmetric_cg() -> Result<CaseEvidence, Box<dyn std::error::Error>> {
    let matrix = CsrMatrix::from_edges(
        2,
        2,
        MatrixKind::SymmetricPositiveDefinite,
        &[(0, 0, 2.0), (0, 1, 1.0), (1, 1, 2.0)],
    )?;
    let solver = ConjugateGradientSolver::new(ConjugateGradientConfig::default())?;
    let result = solver.solve(&matrix, &[1.0, 1.0]);
    let passed = matches!(result, Err(SolverError::InvalidInput { .. }));
    Ok(CaseEvidence {
        name: "edge_unsymmetric_cg_fails_closed".to_string(),
        trigger: json!({"algorithm": "conjugate_gradient", "rhs": [1.0, 1.0]}),
        before_state: json!({"matrix_edges": [[0, 0, 2.0], [0, 1, 1.0], [1, 1, 2.0]], "claimed_kind": "SymmetricPositiveDefinite"}),
        after_state: json!({"error_code": result.expect_err("unsymmetric CG must fail").code()}),
        expected: json!({"error_code": "CGSOLVER_INVALID_INPUT"}),
        passed,
    })
}
