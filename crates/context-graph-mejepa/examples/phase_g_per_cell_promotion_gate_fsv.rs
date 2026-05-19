use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use clap::Parser;
use context_graph_mejepa::heal::{
    write_json_atomic_readback, AbcPromoter, HealError, HoldoutEval, ModeWinner, PromotionGate,
};
use serde_json::{json, Value};

#[derive(Parser, Debug)]
struct Args {
    #[arg(
        long,
        default_value = "/zfs/archive/contextgraph/fsv/phase-g-per-cell-promotion-gate-fsv"
    )]
    output_root: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let report = run(&args.output_root)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn run(output_root: &Path) -> Result<Value> {
    if output_root.exists() {
        fs::remove_dir_all(output_root)
            .with_context(|| format!("remove {}", output_root.display()))?;
    }
    fs::create_dir_all(output_root).with_context(|| format!("create {}", output_root.display()))?;

    let report_path = output_root.join("per_cell_promotion_gate_fsv.json");
    let promoter = AbcPromoter::try_new(0.1, PromotionGate::default())?;

    let happy_path = exact_tolerance_allows(&promoter)?;
    let boundary_cases = vec![
        regressing_cell_blocks(&promoter)?,
        missing_after_cell_blocks(&promoter)?,
        fallback_to_non_regressing_candidate(&promoter)?,
    ];

    let report = json!({
        "fsv": "phase_g_per_cell_promotion_gate",
        "task_id": "TASK-G-016",
        "all_passed": true,
        "artifact_path": report_path,
        "build_release_sha": git_sha(),
        "source_of_truth": report_path,
        "trigger": "cargo run -p context-graph-mejepa --example phase_g_per_cell_promotion_gate_fsv",
        "expected": "A/B/C promotion blocks any per-cell correlation regression greater than 0.005 and allows all hold-or-improve candidates",
        "happy_path": [happy_path],
        "boundary_cases": boundary_cases,
        "readback_equal": true,
    });
    write_json_atomic_readback(&report_path, &report)?;
    Ok(report)
}

fn regressing_cell_blocks(promoter: &AbcPromoter) -> Result<Value> {
    let mode_a = eval_with_cells(
        0.90,
        [("compile_error::rust", 0.90), ("off_by_one::python", 0.80)],
        0xA1,
    )?;
    let mode_b = eval_with_cells(
        0.94,
        [("compile_error::rust", 0.91), ("off_by_one::python", 0.794)],
        0xB1,
    )?;
    let mode_c = eval_with_cells(
        0.89,
        [("compile_error::rust", 0.90), ("off_by_one::python", 0.80)],
        0xC1,
    )?;
    let result = promoter.evaluate(&mode_a, &mode_b, &mode_c);
    let (error_code, regressing_cells) = regression_error_details(result)?;
    ensure!(
        error_code == "MEJEPA_PROMOTION_CELL_REGRESSION",
        "wrong error code for regressing cell: {error_code}"
    );
    ensure!(
        regressing_cells == vec!["off_by_one::python".to_string()],
        "unexpected regressing cells: {regressing_cells:?}"
    );
    Ok(json!({
        "case": "regressing_cell_blocks",
        "before": mode_a.per_cell_correlation,
        "after": mode_b.per_cell_correlation,
        "expected": "MEJEPA_PROMOTION_CELL_REGRESSION for off_by_one::python",
        "actual_error_code": error_code,
        "actual_regressing_cells": regressing_cells,
        "pass": true,
    }))
}

fn exact_tolerance_allows(promoter: &AbcPromoter) -> Result<Value> {
    let mode_a = eval_with_cells(
        0.90,
        [("compile_error::rust", 0.90), ("off_by_one::python", 0.80)],
        0xA2,
    )?;
    let mode_b = eval_with_cells(
        0.94,
        [("compile_error::rust", 0.91), ("off_by_one::python", 0.795)],
        0xB2,
    )?;
    let mode_c = eval_with_cells(
        0.89,
        [("compile_error::rust", 0.90), ("off_by_one::python", 0.80)],
        0xC2,
    )?;
    let winner = promoter.evaluate(&mode_a, &mode_b, &mode_c)?;
    ensure!(winner == ModeWinner::B, "expected B winner, got {winner:?}");
    Ok(json!({
        "case": "exact_tolerance_allows",
        "before": mode_a.per_cell_correlation,
        "after": mode_b.per_cell_correlation,
        "expected": "ModeWinner::B because 0.795 >= 0.800 - 0.005",
        "actual_winner": winner,
        "pass": true,
    }))
}

fn missing_after_cell_blocks(promoter: &AbcPromoter) -> Result<Value> {
    let mode_a = eval_with_cells(
        0.90,
        [("compile_error::rust", 0.90), ("off_by_one::python", 0.80)],
        0xA3,
    )?;
    let mode_b = eval_with_cells(0.94, [("compile_error::rust", 0.91)], 0xB3)?;
    let mode_c = eval_with_cells(
        0.89,
        [("compile_error::rust", 0.90), ("off_by_one::python", 0.80)],
        0xC3,
    )?;
    let result = promoter.evaluate(&mode_a, &mode_b, &mode_c);
    let (error_code, regressing_cells) = regression_error_details(result)?;
    ensure!(
        regressing_cells == vec!["off_by_one::python".to_string()],
        "missing-cell gate reported wrong cells: {regressing_cells:?}"
    );
    Ok(json!({
        "case": "missing_after_cell_blocks",
        "before": mode_a.per_cell_correlation,
        "after": mode_b.per_cell_correlation,
        "expected": "missing after-cell is fail-closed",
        "actual_error_code": error_code,
        "actual_regressing_cells": regressing_cells,
        "pass": true,
    }))
}

fn fallback_to_non_regressing_candidate(promoter: &AbcPromoter) -> Result<Value> {
    let mode_a = eval_with_cells(
        0.90,
        [("compile_error::rust", 0.90), ("off_by_one::python", 0.80)],
        0xA4,
    )?;
    let mode_b = eval_with_cells(
        0.95,
        [("compile_error::rust", 0.91), ("off_by_one::python", 0.79)],
        0xB4,
    )?;
    let mode_c = eval_with_cells(
        0.94,
        [("compile_error::rust", 0.91), ("off_by_one::python", 0.80)],
        0xC4,
    )?;
    let winner = promoter.evaluate(&mode_a, &mode_b, &mode_c)?;
    ensure!(
        winner == ModeWinner::C,
        "expected C fallback, got {winner:?}"
    );
    Ok(json!({
        "case": "fallback_to_non_regressing_candidate",
        "mode_b_after": mode_b.per_cell_correlation,
        "mode_c_after": mode_c.per_cell_correlation,
        "expected": "Mode B aggregate winner is filtered and Mode C wins because all cells hold",
        "actual_winner": winner,
        "pass": true,
    }))
}

fn eval_with_cells<const N: usize>(
    aggregate: f32,
    cells: [(&str, f32); N],
    sha_byte: u8,
) -> Result<HoldoutEval> {
    let per_cell_correlation = cells
        .into_iter()
        .map(|(cell, value)| (cell.to_string(), value))
        .collect::<BTreeMap<_, _>>();
    Ok(HoldoutEval::try_new_with_cells(
        aggregate,
        aggregate,
        0.05,
        100,
        [sha_byte; 32],
        per_cell_correlation,
    )?)
}

fn regression_error_details(
    result: Result<ModeWinner, HealError>,
) -> Result<(String, Vec<String>)> {
    let err = result.expect_err("expected promotion cell regression");
    let code = err.code().to_string();
    let HealError::PromotionCellRegression { regressions } = err else {
        anyhow::bail!("expected PromotionCellRegression, got {code}");
    };
    let cells = regressions
        .into_iter()
        .map(|regression| regression.cell_key)
        .collect::<Vec<_>>();
    Ok((code, cells))
}

fn git_sha() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
