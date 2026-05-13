//! FSV for TASK-G-013 failing-cell root-cause classification.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mcp::daemon::{write_weekly_eval_markdown, WeeklyOperationalSummary};
use context_graph_mejepa::eval::report::write_json_0600;
use context_graph_mejepa::eval::{
    cell_key, ActiveLearningSummary, EvalProvenance, EvalReport, MutationCategory,
};
use context_graph_mejepa::heal::drift_attribution::{
    classify_failing_cell, FailingCellClassification, FailingCellRootCause, FailingCellSignature,
};
use context_graph_mejepa::Language;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DEFAULT_FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-g-failing-cell-root-cause-fsv";

fn main() -> Result<()> {
    let output_root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_FSV_ROOT));
    let report = run(&output_root)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
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

    let signatures = synthetic_signatures();
    let mut classifications = BTreeMap::new();
    for (expected, signature) in &signatures {
        let classification = classify_failing_cell(signature)
            .with_context(|| format!("classify {}", signature.cell))?;
        ensure!(
            classification.root_cause == *expected,
            "classification mismatch for {}",
            signature.cell
        );
        classifications.insert(signature.cell.clone(), classification);
    }

    let report = synthetic_report(classifications);
    report.validate().context("validate synthetic EvalReport")?;
    let markdown_path = run_root.join("weekly.md");
    write_weekly_eval_markdown(
        &markdown_path,
        &report,
        "synthetic-task-g-013-report-hash",
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
        "not every expected root-cause row rendered"
    );

    let boundary_cases = vec![
        boundary_empty_cell_signature(),
        boundary_invalid_distribution_shift_score(),
        boundary_mismatched_report_classification_key(),
        boundary_empty_weekly_root_cause_table(&run_root)?,
    ];
    let all_boundaries_pass = boundary_cases
        .iter()
        .all(|case| case["pass"].as_bool().unwrap_or(false));
    let evidence_path = output_root.join("failing_cell_root_cause_fsv.json");
    let evidence = json!({
        "fsv_root": output_root,
        "task_id": "TASK-G-013",
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_sha(),
        "happy_path": [{
            "case": "six_root_cause_classes_render_in_weekly_report",
            "sot": markdown_path,
            "before": null,
            "trigger": "cargo run --release -p context-graph-mcp --example phase_g_failing_cell_root_cause_fsv",
            "after": {
                "rendered_rows": rendered_rows,
                "markdown_sha256": sha256_bytes(markdown.as_bytes()),
                "classifications": report.failing_cell_classifications
            },
            "expected": {
                "classes": [
                    "insufficient_training_data",
                    "embedder_signal_gap",
                    "label_noise",
                    "oracle_flakiness",
                    "distribution_shift",
                    "unknown"
                ],
                "rows": expected_rows
            },
            "actual": {
                "rows_present": rendered_rows,
                "classification_count": report.failing_cell_classifications.len()
            },
            "pass": true,
            "evidence_path": markdown_path
        }],
        "boundary_cases": boundary_cases,
        "all_passed": all_boundaries_pass,
        "readback_equal": true,
        "physical_artifacts": {
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

fn synthetic_signatures() -> Vec<(FailingCellRootCause, FailingCellSignature)> {
    vec![
        (
            FailingCellRootCause::InsufficientTrainingData,
            signature(
                cell_key(MutationCategory::KnownGood, Language::Python),
                Some(0.70),
                3,
                10,
            ),
        ),
        (
            FailingCellRootCause::EmbedderSignalGap,
            FailingCellSignature {
                blind_spot_z: Some(2.4),
                embedder_pairwise_mi: Some(0.42),
                ..signature(
                    cell_key(MutationCategory::OffByOne, Language::Rust),
                    Some(0.72),
                    12,
                    10,
                )
            },
        ),
        (
            FailingCellRootCause::LabelNoise,
            FailingCellSignature {
                label_disagreement_rate: Some(0.21),
                ..signature(
                    cell_key(MutationCategory::SubtleFlip, Language::Go),
                    Some(0.73),
                    12,
                    10,
                )
            },
        ),
        (
            FailingCellRootCause::OracleFlakiness,
            FailingCellSignature {
                oracle_flake_rate: Some(0.12),
                label_disagreement_rate: Some(0.21),
                ..signature(
                    cell_key(MutationCategory::SwapVariable, Language::Java),
                    Some(0.74),
                    12,
                    10,
                )
            },
        ),
        (
            FailingCellRootCause::DistributionShift,
            FailingCellSignature {
                distribution_shift_score: Some(0.34),
                ..signature(
                    cell_key(MutationCategory::WrongFile, Language::Ruby),
                    Some(0.75),
                    12,
                    10,
                )
            },
        ),
        (
            FailingCellRootCause::Unknown,
            signature(
                cell_key(MutationCategory::CompileError, Language::Php),
                Some(0.76),
                12,
                10,
            ),
        ),
    ]
}

fn signature(
    cell: String,
    correlation: Option<f32>,
    holdout_count: usize,
    min_holdout_count: usize,
) -> FailingCellSignature {
    FailingCellSignature {
        cell,
        correlation,
        holdout_count,
        min_holdout_count,
        embedder_pairwise_mi: None,
        blind_spot_z: None,
        label_disagreement_rate: None,
        oracle_flake_rate: None,
        distribution_shift_score: None,
    }
}

fn synthetic_report(classifications: BTreeMap<String, FailingCellClassification>) -> EvalReport {
    let per_cell_correlation = classifications
        .keys()
        .map(|cell| (cell.clone(), Some(0.75)))
        .collect();
    EvalReport {
        report_date: "2026-05-13".to_string(),
        generated_at_unix_ms: 1_778_666_683_000,
        rolling_window_size: 4,
        holdout_count: 72,
        overall_correlation: Some(0.75),
        per_category_correlation: BTreeMap::new(),
        per_language_correlation: BTreeMap::new(),
        per_cell_correlation,
        cell_exemptions: BTreeMap::new(),
        bayesian_shrinkage: BTreeMap::new(),
        conformal_coverage_health: BTreeMap::new(),
        ood_calibration_health: BTreeMap::new(),
        gtau_pass_rate: BTreeMap::new(),
        per_prediction_class_calibration: BTreeMap::new(),
        active_learning: ActiveLearningSummary {
            queued_count: 0,
            evicted_count: 0,
            ood_escalation_count: 0,
        },
        state_transfer_diagnostic: None,
        per_cell_state_transfer: BTreeMap::new(),
        failing_cell_classifications: classifications,
        aux_head_distillation: None,
        regression_checks: Vec::new(),
        open_research_questions: Vec::new(),
        q1_pass_rate: 1.0,
        q2_report_correlation: Some(0.75),
        q3_side_effect_agreement: Some(1.0),
        ship_gate_passed: false,
        ship_gate_failures: vec!["synthetic failing cells".to_string()],
        provenance: EvalProvenance {
            corpus_sha: "phase-g-failing-cell-root-cause-fsv".to_string(),
            eval_code_version: "task-g-013-fsv".to_string(),
            calibration_version: "task-g-013-fsv".to_string(),
            generated_by: "phase_g_failing_cell_root_cause_fsv".to_string(),
        },
        wall_clock_seconds: 0.01,
    }
}

fn expected_markdown_rows() -> Vec<String> {
    vec![
        "| known_good::python | insufficient_training_data | 0.950000 | holdout_count < min_holdout_count | holdout_count=3; min_holdout_count=10 |".to_string(),
        "| off_by_one::rust | embedder_signal_gap | 0.820000 | blind_spot_z >= 2.0 or embedder_pairwise_mi <= 0.15 | blind_spot_z=2.400000; embedder_pairwise_mi=0.420000 |".to_string(),
        "| subtle_flip::go | label_noise | 0.860000 | label_disagreement_rate >= 0.15 | label_disagreement_rate=0.210000 |".to_string(),
        "| swap_variable::java | oracle_flakiness | 0.900000 | oracle_flake_rate >= 0.10 | oracle_flake_rate=0.120000 |".to_string(),
        "| wrong_file::ruby | distribution_shift | 0.840000 | distribution_shift_score >= 0.30 | distribution_shift_score=0.340000 |".to_string(),
        "| compile_error::php | unknown | 0.250000 | no configured root-cause heuristic threshold fired | holdout_count=12; correlation=0.760000 |".to_string(),
    ]
}

fn boundary_empty_cell_signature() -> Value {
    let err = classify_failing_cell(&signature("".to_string(), Some(0.70), 12, 10))
        .expect_err("empty cell must fail closed");
    json!({
        "case": "empty_cell_signature",
        "sot": "classify_failing_cell",
        "before": "cell=''",
        "after": err.code(),
        "expected": "MEJEPA_HEAL_INVALID_STATE",
        "actual": err.code(),
        "pass": err.code() == "MEJEPA_HEAL_INVALID_STATE"
    })
}

fn boundary_invalid_distribution_shift_score() -> Value {
    let err = classify_failing_cell(&FailingCellSignature {
        distribution_shift_score: Some(1.2),
        ..signature(
            cell_key(MutationCategory::KnownGood, Language::Rust),
            Some(0.70),
            12,
            10,
        )
    })
    .expect_err("invalid distribution shift score must fail closed");
    json!({
        "case": "invalid_distribution_shift_score",
        "sot": "classify_failing_cell",
        "before": "distribution_shift_score=1.2",
        "after": err.code(),
        "expected": "MEJEPA_HEAL_INVALID_STATE",
        "actual": err.code(),
        "pass": err.code() == "MEJEPA_HEAL_INVALID_STATE"
    })
}

fn boundary_mismatched_report_classification_key() -> Value {
    let mut report = synthetic_report(BTreeMap::new());
    report.per_cell_correlation.insert(
        cell_key(MutationCategory::KnownGood, Language::Python),
        Some(0.70),
    );
    report.failing_cell_classifications.insert(
        "wrong_file::python".to_string(),
        FailingCellClassification {
            cell: "known_good::python".to_string(),
            root_cause: FailingCellRootCause::Unknown,
            confidence: 0.25,
            heuristic: "synthetic".to_string(),
            evidence: vec!["synthetic".to_string()],
        },
    );
    let err = report
        .validate()
        .expect_err("classification key without correlation must fail closed");
    json!({
        "case": "classification_key_without_correlation",
        "sot": "EvalReport::validate",
        "before": "failing_cell_classifications key absent from per_cell_correlation",
        "after": err.code(),
        "expected": "MEJEPA_EVAL_INVALID_INPUT",
        "actual": err.code(),
        "pass": err.code() == "MEJEPA_EVAL_INVALID_INPUT"
    })
}

fn boundary_empty_weekly_root_cause_table(run_root: &Path) -> Result<Value> {
    let report = synthetic_report(BTreeMap::new());
    let markdown_path = run_root.join("weekly-empty-root-cause.md");
    write_weekly_eval_markdown(
        &markdown_path,
        &report,
        "synthetic-empty-root-cause-report-hash",
        &WeeklyOperationalSummary::new(0, 0, 0, 0, 0, 0),
    )?;
    let markdown = fs::read_to_string(&markdown_path)?;
    let row = "| none | unknown | 0.000000 | no_failing_cells_classified | none |";
    Ok(json!({
        "case": "empty_weekly_root_cause_table",
        "sot": markdown_path,
        "before": "failing_cell_classifications={}",
        "after": sha256_bytes(markdown.as_bytes()),
        "expected": row,
        "actual": markdown.contains(row),
        "pass": markdown.contains(row)
    }))
}

fn file_artifact(path: &Path) -> Result<Value> {
    let bytes = fs::read(path)?;
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
        .args(["rev-parse", "--short=8", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}
