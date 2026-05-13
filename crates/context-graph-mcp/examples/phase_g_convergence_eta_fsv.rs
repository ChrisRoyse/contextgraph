//! FSV for TASK-G-021 per-cell convergence ETA tracking.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mcp::daemon::{write_weekly_eval_markdown, WeeklyOperationalSummary};
use context_graph_mejepa::eval::report::write_json_0600;
use context_graph_mejepa::{
    baseline_convergence_eta_for_cells, cell_key, compute_convergence_eta_from_reports,
    ActiveLearningSummary, EvalConfig, EvalProvenance, EvalReport, Language, MutationCategory,
    OpenResearchQuestionStatus,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DEFAULT_FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-g-convergence-eta-fsv";
const TASK_ID: &str = "TASK-G-021";

fn main() -> Result<()> {
    let output_root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_FSV_ROOT));
    let evidence = run(&output_root)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn run(output_root: &Path) -> Result<Value> {
    if output_root.exists() {
        fs::remove_dir_all(output_root)
            .with_context(|| format!("remove {}", output_root.display()))?;
    }
    fs::create_dir_all(output_root).with_context(|| format!("create {}", output_root.display()))?;
    let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let run_root = output_root.join(format!("run-{started_at_unix_ms}-{}", std::process::id()));
    fs::create_dir_all(&run_root).with_context(|| format!("create {}", run_root.display()))?;

    let config = EvalConfig {
        convergence_history_windows: 4,
        convergence_eta_min_points: 3,
        ..Default::default()
    };
    let mut reports = synthetic_series()?;
    let convergence_eta = compute_convergence_eta_from_reports(
        &reports,
        config.convergence_target_correlation,
        config.convergence_history_windows,
        config.convergence_eta_min_points,
    )
    .context("compute convergence ETA")?;
    reports
        .last_mut()
        .expect("synthetic series is non-empty")
        .per_cell_convergence_eta = convergence_eta.clone();
    let latest_report = reports.last().expect("latest report").clone();
    latest_report.validate().context("validate latest report")?;

    let report_json_path = run_root.join("weekly.json");
    write_json_0600(&report_json_path, &latest_report)
        .with_context(|| format!("write {}", report_json_path.display()))?;
    let report_readback: EvalReport = serde_json::from_slice(
        &fs::read(&report_json_path)
            .with_context(|| format!("read {}", report_json_path.display()))?,
    )
    .context("deserialize report readback")?;
    ensure!(
        report_readback.determinism_hash()? == latest_report.determinism_hash()?,
        "weekly.json readback hash mismatch"
    );

    let report_hash = latest_report.determinism_hash()?;
    let markdown_path = run_root.join("weekly.md");
    write_weekly_eval_markdown(
        &markdown_path,
        &latest_report,
        &report_hash,
        &WeeklyOperationalSummary::new(0, 0, 0, 0, 0, 0),
    )
    .context("write weekly markdown")?;
    let markdown = fs::read_to_string(&markdown_path)
        .with_context(|| format!("read {}", markdown_path.display()))?;
    let expected_rows = expected_markdown_rows();
    let rendered_rows = expected_rows
        .iter()
        .filter(|row| markdown.contains(*row))
        .count();
    ensure!(
        rendered_rows == expected_rows.len(),
        "not every expected convergence ETA row rendered"
    );

    let boundary_cases = vec![
        boundary_trending_eta(&latest_report),
        boundary_already_passing(&latest_report),
        boundary_not_converging(&latest_report),
        boundary_insufficient_history(&latest_report),
        boundary_insufficient_samples(&latest_report),
        boundary_missing_eta_rejected(&latest_report),
        boundary_invalid_target_rejected(&reports),
    ];
    let all_boundaries_pass = boundary_cases
        .iter()
        .all(|case| case["pass"].as_bool().unwrap_or(false));
    let evidence_path = output_root.join("convergence_eta_fsv.json");
    let evidence = json!({
        "fsv_root": output_root,
        "task_id": TASK_ID,
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_sha(),
        "run_root": run_root,
        "happy_path": [{
            "case": "synthetic_trending_series_renders_convergence_eta",
            "sot": {
                "json_report": report_json_path,
                "weekly_markdown": markdown_path
            },
            "before": null,
            "trigger": "cargo run --release -p context-graph-mcp --example phase_g_convergence_eta_fsv",
            "after": {
                "report_hash": report_hash,
                "markdown_sha256": sha256_bytes(markdown.as_bytes()),
                "eta_cells": latest_report.per_cell_convergence_eta.len(),
                "rendered_rows": rendered_rows,
                "trending_eta": latest_report
                    .per_cell_convergence_eta
                    .get(&cell_key(MutationCategory::OffByOne, Language::Rust))
            },
            "expected": {
                "eta_cells": 5,
                "rows": expected_rows
            },
            "actual": {
                "eta_cells": latest_report.per_cell_convergence_eta.len(),
                "rows_present": rendered_rows
            },
            "pass": true,
            "evidence_path": markdown_path
        }],
        "boundary_cases": boundary_cases,
        "all_passed": all_boundaries_pass,
        "readback_equal": true,
        "physical_artifacts": {
            "weekly_json": file_artifact(&report_json_path)?,
            "weekly_markdown": file_artifact(&markdown_path)?,
            "fsv_json": evidence_path
        }
    });
    ensure!(
        evidence["all_passed"].as_bool() == Some(true),
        "FSV evidence did not pass"
    );
    write_json_0600(&evidence_path, &evidence)
        .with_context(|| format!("write {}", evidence_path.display()))?;
    Ok(evidence)
}

fn synthetic_series() -> Result<Vec<EvalReport>> {
    let passing = cell_key(MutationCategory::KnownGood, Language::Python);
    let trending = cell_key(MutationCategory::OffByOne, Language::Rust);
    let flat = cell_key(MutationCategory::SubtleFlip, Language::Go);
    let sparse = cell_key(MutationCategory::SwapVariable, Language::Java);
    let missing = cell_key(MutationCategory::CompileError, Language::Ruby);

    let windows = [
        vec![
            (passing.clone(), Some(0.96)),
            (trending.clone(), Some(0.80)),
            (flat.clone(), Some(0.70)),
            (sparse.clone(), None),
            (missing.clone(), None),
        ],
        vec![
            (passing.clone(), Some(0.97)),
            (trending.clone(), Some(0.84)),
            (flat.clone(), Some(0.70)),
            (sparse.clone(), None),
            (missing.clone(), None),
        ],
        vec![
            (passing.clone(), Some(0.98)),
            (trending.clone(), Some(0.88)),
            (flat.clone(), Some(0.69)),
            (sparse.clone(), Some(0.78)),
            (missing.clone(), None),
        ],
        vec![
            (passing, Some(0.99)),
            (trending, Some(0.92)),
            (flat, Some(0.69)),
            (sparse, Some(0.80)),
            (missing, None),
        ],
    ];
    windows
        .into_iter()
        .enumerate()
        .map(|(index, cells)| {
            let per_cell = cells.into_iter().collect::<BTreeMap<_, _>>();
            synthetic_report(index, per_cell)
        })
        .collect()
}

fn synthetic_report(
    index: usize,
    per_cell_correlation: BTreeMap<String, Option<f32>>,
) -> Result<EvalReport> {
    let latest_average = average_present(&per_cell_correlation).unwrap_or(0.0);
    let per_cell_convergence_eta = baseline_convergence_eta_for_cells(&per_cell_correlation, 0.95);
    let report = EvalReport {
        report_date: format!("2026-05-{}", 10 + index),
        generated_at_unix_ms: 1_778_640_000_000 + ((index as i64) * 86_400_000),
        rolling_window_size: 5,
        holdout_count: 5,
        overall_correlation: Some(latest_average),
        per_category_correlation: BTreeMap::new(),
        per_language_correlation: BTreeMap::new(),
        per_cell_correlation,
        cell_exemptions: BTreeMap::new(),
        bayesian_shrinkage: BTreeMap::new(),
        conformal_coverage_health: BTreeMap::new(),
        ood_calibration_health: BTreeMap::new(),
        gtau_pass_rate: BTreeMap::new(),
        per_prediction_class_calibration: BTreeMap::new(),
        per_failure_mode_class: context_graph_mejepa::empty_failure_mode_class_metrics(
            5,
            &EvalConfig::default(),
        ),
        per_cell_convergence_eta,
        active_learning: ActiveLearningSummary {
            queued_count: 0,
            evicted_count: 0,
            ood_escalation_count: 0,
        },
        state_transfer_diagnostic: None,
        per_cell_state_transfer: BTreeMap::new(),
        failing_cell_classifications: BTreeMap::new(),
        aux_head_distillation: None,
        regression_checks: Vec::new(),
        open_research_questions: vec![OpenResearchQuestionStatus {
            id: TASK_ID.to_string(),
            question: "At current rate, when will each cell reach 0.95?".to_string(),
            status: "closed_by_fsv".to_string(),
        }],
        q1_pass_rate: 1.0,
        q2_report_correlation: Some(latest_average),
        q3_side_effect_agreement: Some(1.0),
        ship_gate_passed: false,
        ship_gate_failures: vec!["synthetic convergence ETA below target".to_string()],
        provenance: EvalProvenance {
            corpus_sha: "task-g-021-synthetic-trending-series".to_string(),
            eval_code_version: "task-g-021-fsv".to_string(),
            calibration_version: "task-g-021-fsv".to_string(),
            generated_by: "phase_g_convergence_eta_fsv".to_string(),
        },
        wall_clock_seconds: 0.01,
    };
    report.validate()?;
    Ok(report)
}

fn average_present(per_cell: &BTreeMap<String, Option<f32>>) -> Option<f32> {
    let values = per_cell.values().flatten().copied().collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f32>() / values.len() as f32)
    }
}

fn expected_markdown_rows() -> Vec<String> {
    vec![
        "| known_good::python | 0.990000 | already_passing | 0 | 0..0 | unavailable | unavailable | 4 | 4 |".to_string(),
        "| off_by_one::rust | 0.920000 | trending_to_target | 1 | 1..1 | 0.040000 | 1.000000 | 4 | 4 |".to_string(),
        "| subtle_flip::go | 0.690000 | not_converging | unavailable | unavailable | -0.004000 | 0.800000 | 4 | 4 |".to_string(),
        "| swap_variable::java | 0.800000 | insufficient_history | unavailable | unavailable | unavailable | unavailable | 4 | 2 |".to_string(),
        "| compile_error::ruby | unavailable | insufficient_samples | unavailable | unavailable | unavailable | unavailable | 4 | 0 |".to_string(),
    ]
}

fn boundary_trending_eta(report: &EvalReport) -> Value {
    let cell = cell_key(MutationCategory::OffByOne, Language::Rust);
    let eta = report
        .per_cell_convergence_eta
        .get(&cell)
        .expect("trending eta");
    json!({
        "case": "trending_linear_eta",
        "expected": {
            "status": "trending_to_target",
            "estimated_passing_window": 1,
            "confidence_interval": "1..1"
        },
        "actual": eta,
        "pass": eta.status == context_graph_mejepa::ConvergenceEtaStatus::TrendingToTarget
            && eta.estimated_passing_window == Some(1)
            && eta
                .confidence_interval
                .as_ref()
                .map(|interval| (interval.lower_window, interval.upper_window))
                == Some((1, 1))
    })
}

fn boundary_already_passing(report: &EvalReport) -> Value {
    let cell = cell_key(MutationCategory::KnownGood, Language::Python);
    let eta = report
        .per_cell_convergence_eta
        .get(&cell)
        .expect("passing eta");
    json!({
        "case": "already_passing_eta_zero",
        "expected": 0,
        "actual": eta.estimated_passing_window,
        "pass": eta.status == context_graph_mejepa::ConvergenceEtaStatus::AlreadyPassing
            && eta.estimated_passing_window == Some(0)
    })
}

fn boundary_not_converging(report: &EvalReport) -> Value {
    let cell = cell_key(MutationCategory::SubtleFlip, Language::Go);
    let eta = report
        .per_cell_convergence_eta
        .get(&cell)
        .expect("flat eta");
    json!({
        "case": "non_positive_slope_has_no_eta",
        "expected": "not_converging",
        "actual": eta,
        "pass": eta.status == context_graph_mejepa::ConvergenceEtaStatus::NotConverging
            && eta.estimated_passing_window.is_none()
    })
}

fn boundary_insufficient_history(report: &EvalReport) -> Value {
    let cell = cell_key(MutationCategory::SwapVariable, Language::Java);
    let eta = report
        .per_cell_convergence_eta
        .get(&cell)
        .expect("sparse eta");
    json!({
        "case": "insufficient_history_blocks_eta",
        "expected": "insufficient_history",
        "actual": eta,
        "pass": eta.status == context_graph_mejepa::ConvergenceEtaStatus::InsufficientHistory
            && eta.valid_observation_count == 2
            && eta.estimated_passing_window.is_none()
    })
}

fn boundary_insufficient_samples(report: &EvalReport) -> Value {
    let cell = cell_key(MutationCategory::CompileError, Language::Ruby);
    let eta = report
        .per_cell_convergence_eta
        .get(&cell)
        .expect("missing eta");
    json!({
        "case": "latest_missing_blocks_eta",
        "expected": "insufficient_samples",
        "actual": eta,
        "pass": eta.status == context_graph_mejepa::ConvergenceEtaStatus::InsufficientSamples
            && eta.latest_correlation.is_none()
    })
}

fn boundary_missing_eta_rejected(report: &EvalReport) -> Value {
    let mut invalid = report.clone();
    invalid
        .per_cell_convergence_eta
        .remove(&cell_key(MutationCategory::OffByOne, Language::Rust));
    let err = invalid.validate().expect_err("missing ETA should fail");
    json!({
        "case": "missing_eta_rejected",
        "expected": "MEJEPA_EVAL_INVALID_INPUT",
        "actual": err.code(),
        "pass": err.code() == "MEJEPA_EVAL_INVALID_INPUT"
    })
}

fn boundary_invalid_target_rejected(reports: &[EvalReport]) -> Value {
    let err = compute_convergence_eta_from_reports(reports, 2.0, 4, 3)
        .expect_err("invalid target should fail");
    json!({
        "case": "invalid_target_rejected",
        "expected": "MEJEPA_EVAL_INVALID_INPUT",
        "actual": err.code(),
        "pass": err.code() == "MEJEPA_EVAL_INVALID_INPUT"
    })
}

fn file_artifact(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(json!({
        "path": path,
        "bytes": bytes.len(),
        "sha256": sha256_bytes(&bytes)
    }))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn git_sha() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
