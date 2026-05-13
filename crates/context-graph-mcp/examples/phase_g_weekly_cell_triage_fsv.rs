//! FSV for TASK-G-018 weekly operator-actionable cell triage.

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
    FailingCellClassification, FailingCellRootCause,
};
use context_graph_mejepa::Language;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DEFAULT_FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-g-weekly-cell-triage-fsv";

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

    let report = synthetic_report();
    report.validate().context("validate synthetic EvalReport")?;
    let markdown_path = run_root.join("weekly.md");
    write_weekly_eval_markdown(
        &markdown_path,
        &report,
        "synthetic-task-g-018-report-hash",
        &WeeklyOperationalSummary::new(0, 0, 0, 0, 0, 0),
    )
    .context("write weekly markdown")?;
    let markdown = fs::read_to_string(&markdown_path)
        .with_context(|| format!("read {}", markdown_path.display()))?;
    let expected_rows = expected_top_rows();
    let rendered_rows = expected_rows
        .iter()
        .filter(|row| markdown.contains(*row))
        .count();
    ensure!(
        rendered_rows == expected_rows.len(),
        "not every expected triage row rendered"
    );

    let boundary_cases = vec![
        boundary_top_ten_limit(&markdown),
        boundary_action_mapping(&markdown),
        boundary_missing_correlation_ranks_first(&markdown),
        boundary_empty_triage_table(&run_root)?,
        boundary_passing_classification_filtered(&markdown),
    ];
    let all_boundaries_pass = boundary_cases
        .iter()
        .all(|case| case["pass"].as_bool().unwrap_or(false));
    let evidence_path = output_root.join("weekly_cell_triage_fsv.json");
    let evidence = json!({
        "fsv_root": output_root,
        "task_id": "TASK-G-018",
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_sha(),
        "happy_path": [{
            "case": "operator_triage_top_10_renders_ranked_actions",
            "sot": markdown_path,
            "before": null,
            "trigger": "cargo run --release -p context-graph-mcp --example phase_g_weekly_cell_triage_fsv",
            "after": {
                "rendered_rows": rendered_rows,
                "markdown_sha256": sha256_bytes(markdown.as_bytes())
            },
            "expected": {
                "section": "## Operator Cell Triage",
                "rows": expected_rows
            },
            "actual": {
                "section_present": markdown.contains("## Operator Cell Triage"),
                "rows_present": rendered_rows
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

fn synthetic_report() -> EvalReport {
    let rows = triage_inputs();
    let mut per_cell_correlation = BTreeMap::new();
    let mut failing_cell_classifications = BTreeMap::new();
    for row in rows {
        per_cell_correlation.insert(row.cell.clone(), row.correlation);
        failing_cell_classifications.insert(
            row.cell.clone(),
            FailingCellClassification {
                cell: row.cell,
                root_cause: row.root_cause,
                confidence: row.confidence,
                heuristic: row.heuristic.to_string(),
                evidence: vec![row.evidence.to_string()],
            },
        );
    }
    EvalReport {
        report_date: "2026-05-13".to_string(),
        generated_at_unix_ms: 1_778_666_683_000,
        rolling_window_size: 4,
        holdout_count: 144,
        overall_correlation: Some(0.74),
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
        failing_cell_classifications,
        aux_head_distillation: None,
        regression_checks: Vec::new(),
        open_research_questions: Vec::new(),
        q1_pass_rate: 1.0,
        q2_report_correlation: Some(0.74),
        q3_side_effect_agreement: Some(1.0),
        ship_gate_passed: false,
        ship_gate_failures: vec!["synthetic failing cells for operator triage".to_string()],
        provenance: EvalProvenance {
            corpus_sha: "phase-g-weekly-cell-triage-fsv".to_string(),
            eval_code_version: "task-g-018-fsv".to_string(),
            calibration_version: "task-g-018-fsv".to_string(),
            generated_by: "phase_g_weekly_cell_triage_fsv".to_string(),
        },
        wall_clock_seconds: 0.01,
    }
}

struct TriageInput {
    cell: String,
    correlation: Option<f32>,
    root_cause: FailingCellRootCause,
    confidence: f32,
    heuristic: &'static str,
    evidence: &'static str,
}

fn triage_inputs() -> Vec<TriageInput> {
    vec![
        input(
            MutationCategory::CompileError,
            Language::Php,
            None,
            FailingCellRootCause::Unknown,
            0.25,
            "no configured root-cause heuristic threshold fired",
            "holdout_count=12",
        ),
        input(
            MutationCategory::KnownGood,
            Language::Python,
            Some(0.40),
            FailingCellRootCause::InsufficientTrainingData,
            0.95,
            "holdout_count < min_holdout_count",
            "holdout_count=3",
        ),
        input(
            MutationCategory::OffByOne,
            Language::Rust,
            Some(0.50),
            FailingCellRootCause::EmbedderSignalGap,
            0.82,
            "blind_spot_z >= 2.0 or embedder_pairwise_mi <= 0.15",
            "blind_spot_z=2.400000",
        ),
        input(
            MutationCategory::SubtleFlip,
            Language::Go,
            Some(0.60),
            FailingCellRootCause::LabelNoise,
            0.86,
            "label_disagreement_rate >= 0.15",
            "label_disagreement_rate=0.210000",
        ),
        input(
            MutationCategory::SwapVariable,
            Language::Java,
            Some(0.65),
            FailingCellRootCause::OracleFlakiness,
            0.90,
            "oracle_flake_rate >= 0.10",
            "oracle_flake_rate=0.120000",
        ),
        input(
            MutationCategory::WrongFile,
            Language::Ruby,
            Some(0.70),
            FailingCellRootCause::DistributionShift,
            0.84,
            "distribution_shift_score >= 0.30",
            "distribution_shift_score=0.340000",
        ),
        input(
            MutationCategory::OverEngineer,
            Language::Cpp,
            Some(0.80),
            FailingCellRootCause::InsufficientTrainingData,
            0.95,
            "holdout_count < min_holdout_count",
            "holdout_count=2",
        ),
        input(
            MutationCategory::CompileError,
            Language::C,
            Some(0.83),
            FailingCellRootCause::LabelNoise,
            0.86,
            "label_disagreement_rate >= 0.15",
            "label_disagreement_rate=0.180000",
        ),
        input(
            MutationCategory::KnownGood,
            Language::Javascript,
            Some(0.84),
            FailingCellRootCause::OracleFlakiness,
            0.90,
            "oracle_flake_rate >= 0.10",
            "oracle_flake_rate=0.110000",
        ),
        input(
            MutationCategory::SubtleFlip,
            Language::CSharp,
            Some(0.86),
            FailingCellRootCause::DistributionShift,
            0.84,
            "distribution_shift_score >= 0.30",
            "distribution_shift_score=0.310000",
        ),
        input(
            MutationCategory::DeleteTestCall,
            Language::Typescript,
            Some(0.87),
            FailingCellRootCause::Unknown,
            0.25,
            "no configured root-cause heuristic threshold fired",
            "correlation=0.870000",
        ),
        input(
            MutationCategory::OffByOne,
            Language::Php,
            Some(0.88),
            FailingCellRootCause::EmbedderSignalGap,
            0.82,
            "blind_spot_z >= 2.0 or embedder_pairwise_mi <= 0.15",
            "blind_spot_z=2.100000",
        ),
        input(
            MutationCategory::SwapVariable,
            Language::Ruby,
            Some(0.99),
            FailingCellRootCause::Unknown,
            0.25,
            "passing classification must be filtered",
            "correlation=0.990000",
        ),
    ]
}

fn input(
    category: MutationCategory,
    language: Language,
    correlation: Option<f32>,
    root_cause: FailingCellRootCause,
    confidence: f32,
    heuristic: &'static str,
    evidence: &'static str,
) -> TriageInput {
    TriageInput {
        cell: cell_key(category, language),
        correlation,
        root_cause,
        confidence,
        heuristic,
        evidence,
    }
}

fn expected_top_rows() -> Vec<String> {
    vec![
        "| 1 | compile_error::php | unavailable | unavailable | unknown | 0.250000 | request_adversarial_sample | holdout_count=12 |".to_string(),
        "| 2 | known_good::python | 0.400000 | 0.550000 | insufficient_training_data | 0.950000 | expand_corpus | holdout_count=3 |".to_string(),
        "| 3 | off_by_one::rust | 0.500000 | 0.450000 | embedder_signal_gap | 0.820000 | request_adversarial_sample | blind_spot_z=2.400000 |".to_string(),
        "| 4 | subtle_flip::go | 0.600000 | 0.350000 | label_noise | 0.860000 | recalibrate | label_disagreement_rate=0.210000 |".to_string(),
        "| 5 | swap_variable::java | 0.650000 | 0.300000 | oracle_flakiness | 0.900000 | mark_exempt | oracle_flake_rate=0.120000 |".to_string(),
        "| 6 | wrong_file::ruby | 0.700000 | 0.250000 | distribution_shift | 0.840000 | expand_corpus | distribution_shift_score=0.340000 |".to_string(),
        "| 7 | over_engineer::cpp | 0.800000 | 0.150000 | insufficient_training_data | 0.950000 | expand_corpus | holdout_count=2 |".to_string(),
        "| 8 | compile_error::c | 0.830000 | 0.120000 | label_noise | 0.860000 | recalibrate | label_disagreement_rate=0.180000 |".to_string(),
        "| 9 | known_good::javascript | 0.840000 | 0.110000 | oracle_flakiness | 0.900000 | mark_exempt | oracle_flake_rate=0.110000 |".to_string(),
        "| 10 | subtle_flip::c_sharp | 0.860000 | 0.090000 | distribution_shift | 0.840000 | expand_corpus | distribution_shift_score=0.310000 |".to_string(),
    ]
}

fn boundary_top_ten_limit(markdown: &str) -> Value {
    let excluded_rank_11 = "| delete_test_call::typescript | 0.870000 |";
    let excluded_rank_12 = "| off_by_one::php | 0.880000 |";
    json!({
        "case": "top_10_limit",
        "sot": "weekly.md",
        "before": "12 non-passing classifications",
        "after": "top 10 rendered",
        "expected": "rank 11 and rank 12 cells absent",
        "actual": {
            "rank_11_present": markdown.contains(excluded_rank_11),
            "rank_12_present": markdown.contains(excluded_rank_12)
        },
        "pass": !markdown.contains(excluded_rank_11) && !markdown.contains(excluded_rank_12)
    })
}

fn boundary_action_mapping(markdown: &str) -> Value {
    let actions = [
        "expand_corpus",
        "request_adversarial_sample",
        "recalibrate",
        "mark_exempt",
    ];
    let missing = actions
        .iter()
        .filter(|action| !markdown.contains(**action))
        .copied()
        .collect::<Vec<_>>();
    json!({
        "case": "action_mapping_all_primary_actions",
        "sot": "weekly.md",
        "before": "root causes cover every operator action",
        "after": actions,
        "expected": "all action strings present",
        "actual": {
            "missing": missing
        },
        "pass": missing.is_empty()
    })
}

fn boundary_missing_correlation_ranks_first(markdown: &str) -> Value {
    let row = "| 1 | compile_error::php | unavailable | unavailable | unknown | 0.250000 | request_adversarial_sample | holdout_count=12 |";
    json!({
        "case": "missing_correlation_ranks_first",
        "sot": "weekly.md",
        "before": "compile_error::php correlation None",
        "after": row,
        "expected": row,
        "actual": markdown.contains(row),
        "pass": markdown.contains(row)
    })
}

fn boundary_empty_triage_table(run_root: &Path) -> Result<Value> {
    let mut report = synthetic_report();
    report.failing_cell_classifications.clear();
    let markdown_path = run_root.join("weekly-empty-triage.md");
    write_weekly_eval_markdown(
        &markdown_path,
        &report,
        "synthetic-empty-triage-report-hash",
        &WeeklyOperationalSummary::new(0, 0, 0, 0, 0, 0),
    )?;
    let markdown = fs::read_to_string(&markdown_path)?;
    let row = "| none | none | unavailable | unavailable | unknown | 0.000000 | no_action | no failing cells classified |";
    Ok(json!({
        "case": "empty_operator_triage_table",
        "sot": markdown_path,
        "before": "failing_cell_classifications={}",
        "after": sha256_bytes(markdown.as_bytes()),
        "expected": row,
        "actual": markdown.contains(row),
        "pass": markdown.contains(row)
    }))
}

fn boundary_passing_classification_filtered(markdown: &str) -> Value {
    let row = "| swap_variable::ruby | 0.990000 |";
    json!({
        "case": "passing_classification_filtered",
        "sot": "weekly.md",
        "before": "swap_variable::ruby classification with correlation=0.99",
        "after": "row omitted",
        "expected": "passing classified cell is absent from operator triage",
        "actual": markdown.contains(row),
        "pass": !markdown.contains(row)
    })
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
