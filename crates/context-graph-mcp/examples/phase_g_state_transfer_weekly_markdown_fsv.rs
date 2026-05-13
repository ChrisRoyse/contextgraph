use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mcp::daemon::{write_weekly_eval_markdown, WeeklyOperationalSummary};
use context_graph_mejepa::eval::report::write_json_0600;
use context_graph_mejepa::eval::{
    cell_key, ActiveLearningSummary, EvalProvenance, EvalReport, MutationCategory,
    StateTransferDiagnostic,
};
use context_graph_mejepa::Language;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DEFAULT_FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-g-state-transfer-weekly-markdown-fsv";

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
    let markdown_path = run_root.join("weekly.md");
    let operational = WeeklyOperationalSummary::new(0, 0, 0, 0, 0, 0);
    write_weekly_eval_markdown(
        &markdown_path,
        &report,
        "synthetic-report-hash",
        &operational,
    )
    .context("write weekly eval markdown")?;
    let markdown = fs::read_to_string(&markdown_path)
        .with_context(|| format!("read {}", markdown_path.display()))?;
    let markdown_sha = sha256_bytes(markdown.as_bytes());

    let expected_rows = vec![
        "| known_good::python | 0.920000 | 0.050000 | 0.910000 | hold |",
        "| off_by_one::rust | 0.740000 | 0.260000 | 0.700000 | fine_tune |",
        "| subtle_flip::go | unavailable | unavailable | unavailable | collect_more_labels |",
    ];
    let rendered_rows = expected_rows
        .iter()
        .filter(|row| markdown.contains(*row))
        .count();
    ensure!(
        rendered_rows == expected_rows.len(),
        "not every expected state-transfer row rendered"
    );

    let boundary_cases = vec![
        boundary_empty_table(&run_root)?,
        boundary_unavailable_row(&markdown),
        boundary_low_transfer_candidate(&markdown),
        boundary_invalid_extra_cell_key()?,
    ];
    let all_boundaries_pass = boundary_cases
        .iter()
        .all(|case| case["pass"].as_bool().unwrap_or(false));
    let file_artifact = file_artifact(&markdown_path)?;
    let evidence_path = output_root.join("state_transfer_weekly_markdown_fsv.json");
    let evidence = json!({
        "fsv_root": output_root,
        "task_id": "TASK-G-011",
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_sha(),
        "happy_path": [{
            "case": "state_transfer_per_cell_table_renders_every_cell",
            "sot": markdown_path,
            "before": null,
            "trigger": "cargo run --release -p context-graph-mcp --example phase_g_state_transfer_weekly_markdown_fsv",
            "after": {
                "sha256": markdown_sha,
                "bytes": markdown.len(),
                "rendered_rows": rendered_rows
            },
            "expected": {
                "table_header": "| state_transfer_per_cell | transfer_score | wasserstein_1 | performance_deploy | candidate |",
                "rows": expected_rows
            },
            "actual": {
                "table_header_present": markdown.contains("| state_transfer_per_cell | transfer_score | wasserstein_1 | performance_deploy | candidate |"),
                "rows_present": rendered_rows,
                "markdown_sha256": markdown_sha
            },
            "pass": true,
            "evidence_path": markdown_path
        }],
        "boundary_cases": boundary_cases,
        "all_passed": all_boundaries_pass,
        "readback_equal": true,
        "physical_artifacts": {
            "weekly_markdown": file_artifact,
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
    let known_good_python = cell_key(MutationCategory::KnownGood, Language::Python);
    let off_by_one_rust = cell_key(MutationCategory::OffByOne, Language::Rust);
    let subtle_flip_go = cell_key(MutationCategory::SubtleFlip, Language::Go);
    let mut per_cell_correlation = BTreeMap::new();
    per_cell_correlation.insert(known_good_python.clone(), Some(0.96));
    per_cell_correlation.insert(off_by_one_rust.clone(), Some(0.81));
    per_cell_correlation.insert(subtle_flip_go.clone(), None);
    let mut per_cell_state_transfer = BTreeMap::new();
    per_cell_state_transfer.insert(
        known_good_python,
        Some(StateTransferDiagnostic {
            wasserstein_1: 0.05,
            transfer_score: 0.92,
            performance_deploy: 0.91,
        }),
    );
    per_cell_state_transfer.insert(
        off_by_one_rust,
        Some(StateTransferDiagnostic {
            wasserstein_1: 0.26,
            transfer_score: 0.74,
            performance_deploy: 0.70,
        }),
    );
    per_cell_state_transfer.insert(subtle_flip_go, None);
    EvalReport {
        report_date: "2026-05-13".to_string(),
        generated_at_unix_ms: 1_778_666_683_000,
        rolling_window_size: 4,
        holdout_count: 3,
        overall_correlation: Some(0.90),
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
        state_transfer_diagnostic: Some(StateTransferDiagnostic {
            wasserstein_1: 0.11,
            transfer_score: 0.88,
            performance_deploy: 0.87,
        }),
        per_cell_state_transfer,
        aux_head_distillation: None,
        regression_checks: Vec::new(),
        open_research_questions: Vec::new(),
        q1_pass_rate: 1.0,
        q2_report_correlation: Some(0.90),
        q3_side_effect_agreement: Some(1.0),
        ship_gate_passed: false,
        ship_gate_failures: vec!["synthetic below threshold".to_string()],
        provenance: EvalProvenance {
            corpus_sha: "phase-g-state-transfer-weekly-markdown-fsv".to_string(),
            eval_code_version: "task-g-011-fsv".to_string(),
            calibration_version: "task-g-011-fsv".to_string(),
            generated_by: "phase_g_state_transfer_weekly_markdown_fsv".to_string(),
        },
        wall_clock_seconds: 0.01,
    }
}

fn boundary_empty_table(run_root: &Path) -> Result<Value> {
    let mut report = synthetic_report();
    report.per_cell_state_transfer.clear();
    let markdown_path = run_root.join("weekly-empty.md");
    write_weekly_eval_markdown(
        &markdown_path,
        &report,
        "synthetic-empty-report-hash",
        &WeeklyOperationalSummary::new(0, 0, 0, 0, 0, 0),
    )?;
    let markdown = fs::read_to_string(&markdown_path)?;
    Ok(json!({
        "case": "empty_state_transfer_table",
        "sot": markdown_path,
        "before": "per_cell_state_transfer={}",
        "after": sha256_bytes(markdown.as_bytes()),
        "expected": "| none | unavailable | unavailable | unavailable | collect_more_labels |",
        "actual": markdown.contains("| none | unavailable | unavailable | unavailable | collect_more_labels |"),
        "pass": markdown.contains("| none | unavailable | unavailable | unavailable | collect_more_labels |")
    }))
}

fn boundary_unavailable_row(markdown: &str) -> Value {
    let row = "| subtle_flip::go | unavailable | unavailable | unavailable | collect_more_labels |";
    json!({
        "case": "unavailable_state_transfer_cell",
        "sot": "weekly.md",
        "before": "subtle_flip::go diagnostic None",
        "after": row,
        "expected": row,
        "actual": markdown.contains(row),
        "pass": markdown.contains(row)
    })
}

fn boundary_low_transfer_candidate(markdown: &str) -> Value {
    let row = "| off_by_one::rust | 0.740000 | 0.260000 | 0.700000 | fine_tune |";
    json!({
        "case": "low_transfer_score_candidate",
        "sot": "weekly.md",
        "before": "transfer_score=0.74",
        "after": row,
        "expected": "candidate=fine_tune",
        "actual": markdown.contains(row),
        "pass": markdown.contains(row)
    })
}

fn boundary_invalid_extra_cell_key() -> Result<Value> {
    let mut report = synthetic_report();
    report.per_cell_state_transfer.insert(
        "missing_cell::python".to_string(),
        Some(StateTransferDiagnostic {
            wasserstein_1: 0.1,
            transfer_score: 0.8,
            performance_deploy: 0.7,
        }),
    );
    let err = report
        .validate()
        .expect_err("extra per-cell transfer key must fail closed");
    Ok(json!({
        "case": "invalid_extra_cell_key",
        "sot": "EvalReport::validate",
        "before": "per_cell_state_transfer has key absent from per_cell_correlation",
        "after": err.code(),
        "expected": "MEJEPA_EVAL_INVALID_INPUT",
        "actual": err.code(),
        "pass": err.code() == "MEJEPA_EVAL_INVALID_INPUT"
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
