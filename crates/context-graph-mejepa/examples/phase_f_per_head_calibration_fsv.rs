use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use context_graph_mejepa::eval::report::write_json_0600;
use context_graph_mejepa::eval::{
    cell_key, compute_calibration_for_samples, ActiveLearningSummary, CalibrationSample,
    ConformalHealthEntry, EvalProvenance, EvalReport, MutationCategory, PredictionClassCalibration,
    RegressionCheck, RocksDbEvalStore, StateTransferDiagnostic,
};
use context_graph_mejepa::{open_infer_rocksdb, Language};
use rocksdb::{IteratorMode, DB};
use serde_json::json;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-f-per-head-calibration-fsv";
const REPORT_FILE: &str = "per_head_calibration_fsv.json";
const EXPECTED_ECE: f32 = 0.25;
const ECE_TOLERANCE: f32 = 0.02;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let fsv_root = PathBuf::from(FSV_ROOT);
    fs::create_dir_all(&fsv_root)?;
    let run_root = fsv_root.join(format!("run-{}-{}", started_at_unix_ms, std::process::id()));
    fs::create_dir_all(&run_root)?;

    let db_path = run_root.join("rocksdb");
    let db = open_infer_rocksdb(&db_path)?;
    let eval_store = RocksDbEvalStore::new(db.clone())?;
    let cf_counts_before = cf_counts(db.as_ref())?;

    let calibration = happy_calibration("q2_oracle_pass")?;
    let happy_ece_error = (calibration.expected_calibration_error - EXPECTED_ECE).abs();
    let mut per_prediction_class_calibration = BTreeMap::new();
    per_prediction_class_calibration.insert(calibration.class_name.clone(), calibration.clone());
    per_prediction_class_calibration.insert(
        "q2_oracle_fail".to_string(),
        happy_calibration("q2_oracle_fail")?,
    );
    per_prediction_class_calibration.insert(
        "ood_rejection".to_string(),
        compute_calibration_for_samples(
            "ood_rejection",
            &[
                CalibrationSample::try_new(0.05, false)?,
                CalibrationSample::try_new(0.05, false)?,
                CalibrationSample::try_new(0.95, true)?,
                CalibrationSample::try_new(0.95, true)?,
            ],
            2,
            ECE_TOLERANCE,
        )?,
    );

    let report = synthetic_report(per_prediction_class_calibration)?;
    eval_store.persist_report(&report)?;
    let latest = eval_store
        .load_latest_report()?
        .ok_or("latest eval report missing after persist")?;
    let report_hash = report.determinism_hash()?;
    let latest_hash = latest.determinism_hash()?;
    let readback_equal = latest_hash == report_hash
        && latest.per_prediction_class_calibration == report.per_prediction_class_calibration;
    if !readback_equal {
        return Err("latest report readback differs from calibration report".into());
    }
    let report_artifact = run_root.join("calibration-report.json");
    write_json_0600(&report_artifact, &report)?;
    flush_eval_cfs(db.as_ref())?;
    let cf_counts_after = cf_counts(db.as_ref())?;
    drop(eval_store);
    drop(db);

    let reopened_db = open_infer_rocksdb(&db_path)?;
    let reopened_store = RocksDbEvalStore::new(reopened_db.clone())?;
    let reopened = reopened_store
        .load_latest_report()?
        .ok_or("reopened eval report missing")?;
    let reopened_hash = reopened.determinism_hash()?;
    let reopened_equal = reopened_hash == report_hash
        && reopened
            .per_prediction_class_calibration
            .contains_key("q2_oracle_pass");
    if !reopened_equal {
        return Err("reopened report calibration readback differs".into());
    }
    drop(reopened_store);
    drop(reopened_db);

    let boundary_cases = vec![
        boundary_empty_samples()?,
        boundary_zero_bins()?,
        boundary_out_of_range_confidence()?,
        boundary_nonfinite_confidence()?,
    ];
    let all_boundaries_pass = boundary_cases
        .iter()
        .all(|case| case["pass"].as_bool().unwrap_or(false));
    let sst_files = list_sst_files(&db_path)?;
    if sst_files.is_empty() {
        return Err("no RocksDB SST files found for physical artifact verification".into());
    }

    let evidence = json!({
        "fsv_root": FSV_ROOT,
        "task_id": "TASK-OBS-004",
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_head_short()?,
        "run_root": run_root,
        "happy_path": [{
            "case": "per_prediction_class_ece_persisted",
            "sot": context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS,
            "before": cf_counts_before[context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS],
            "trigger": "cargo run --release -p context-graph-mejepa --example phase_f_per_head_calibration_fsv",
            "after": cf_counts_after[context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS],
            "expected": {
                "class": "q2_oracle_pass",
                "expected_calibration_error": EXPECTED_ECE,
                "max_abs_error": ECE_TOLERANCE,
                "report_hash": report_hash
            },
            "actual": {
                "class": calibration.class_name,
                "expected_calibration_error": calibration.expected_calibration_error,
                "abs_error": happy_ece_error,
                "latest_hash": latest_hash,
                "reopened_hash": reopened_hash
            },
            "pass": happy_ece_error <= ECE_TOLERANCE && readback_equal && reopened_equal,
            "evidence_path": report_artifact
        }],
        "boundary_cases": boundary_cases,
        "all_passed": happy_ece_error <= ECE_TOLERANCE
            && readback_equal
            && reopened_equal
            && all_boundaries_pass,
        "cf_counts_before": cf_counts_before,
        "cf_counts_after": cf_counts_after,
        "readback_equal": readback_equal && reopened_equal,
        "physical_artifacts": {
            "db_path": db_path,
            "sst_files": sst_files
        }
    });
    if !evidence["all_passed"].as_bool().unwrap_or(false) {
        return Err("TASK-OBS-004 FSV evidence did not pass".into());
    }
    let evidence_path = fsv_root.join(REPORT_FILE);
    write_json_0600(&evidence_path, &evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn happy_calibration(
    class_name: &str,
) -> Result<PredictionClassCalibration, Box<dyn std::error::Error>> {
    Ok(compute_calibration_for_samples(
        class_name,
        &[
            CalibrationSample::try_new(0.25, false)?,
            CalibrationSample::try_new(0.25, false)?,
            CalibrationSample::try_new(0.75, true)?,
            CalibrationSample::try_new(0.75, true)?,
        ],
        2,
        ECE_TOLERANCE,
    )?)
}

fn synthetic_report(
    per_prediction_class_calibration: BTreeMap<String, PredictionClassCalibration>,
) -> Result<EvalReport, Box<dyn std::error::Error>> {
    let cell = cell_key(MutationCategory::KnownGood, Language::Rust);
    let mut per_cell_state_transfer = BTreeMap::new();
    per_cell_state_transfer.insert(
        cell.clone(),
        Some(StateTransferDiagnostic {
            wasserstein_1: 0.05,
            transfer_score: 0.92,
            performance_deploy: 0.91,
        }),
    );
    Ok(EvalReport {
        report_date: "2026-05-13-obs-004".to_string(),
        generated_at_unix_ms: chrono::Utc::now().timestamp_millis(),
        rolling_window_size: 4,
        holdout_count: 4,
        overall_correlation: Some(0.96),
        per_category_correlation: BTreeMap::from([(MutationCategory::KnownGood, Some(0.96))]),
        per_language_correlation: BTreeMap::from([(Language::Rust, Some(0.96))]),
        per_cell_correlation: BTreeMap::from([(cell, Some(0.96))]),
        cell_exemptions: BTreeMap::new(),
        bayesian_shrinkage: BTreeMap::from([("known_good::rust".to_string(), 0.95)]),
        conformal_coverage_health: BTreeMap::from([(
            Language::Rust,
            ConformalHealthEntry {
                expected_coverage: 0.90,
                empirical_coverage: 0.90,
                sample_count: 4,
                within_band: true,
            },
        )]),
        ood_calibration_health: BTreeMap::from([(Language::Rust, Some(0.88))]),
        gtau_pass_rate: BTreeMap::from([(Language::Rust, 0.98)]),
        per_prediction_class_calibration,
        active_learning: ActiveLearningSummary {
            queued_count: 0,
            evicted_count: 0,
            ood_escalation_count: 0,
        },
        state_transfer_diagnostic: Some(StateTransferDiagnostic {
            wasserstein_1: 0.05,
            transfer_score: 0.92,
            performance_deploy: 0.91,
        }),
        per_cell_state_transfer,
        aux_head_distillation: None,
        regression_checks: vec![RegressionCheck {
            name: "overall_correlation".to_string(),
            previous: 0.95,
            current: 0.96,
            drop: 0.0,
            passed: true,
        }],
        open_research_questions: Vec::new(),
        q1_pass_rate: 1.0,
        q2_report_correlation: Some(0.96),
        q3_side_effect_agreement: Some(1.0),
        ship_gate_passed: true,
        ship_gate_failures: Vec::new(),
        provenance: EvalProvenance {
            corpus_sha: "phase-f-per-head-calibration".to_string(),
            eval_code_version: env!("CARGO_PKG_VERSION").to_string(),
            calibration_version: "phase-f-calibration-tracker".to_string(),
            generated_by: "phase_f_per_head_calibration_fsv".to_string(),
        },
        wall_clock_seconds: 0.1,
    })
}

fn boundary_empty_samples() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let err = compute_calibration_for_samples("empty_samples", &[], 10, ECE_TOLERANCE).unwrap_err();
    Ok(json!({
        "case": "empty_samples",
        "before": {"samples": 0},
        "after": {"error": err.code()},
        "expected": "MEJEPA_EVAL_EMPTY_HOLDOUT",
        "actual": err.code(),
        "pass": err.code() == "MEJEPA_EVAL_EMPTY_HOLDOUT"
    }))
}

fn boundary_zero_bins() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let samples = [CalibrationSample::try_new(0.5, true)?];
    let err = compute_calibration_for_samples("zero_bins", &samples, 0, ECE_TOLERANCE).unwrap_err();
    Ok(json!({
        "case": "zero_bins",
        "before": {"bin_count": 0},
        "after": {"error": err.code()},
        "expected": "MEJEPA_EVAL_INVALID_INPUT",
        "actual": err.code(),
        "pass": err.code() == "MEJEPA_EVAL_INVALID_INPUT"
    }))
}

fn boundary_out_of_range_confidence() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let samples = [CalibrationSample {
        confidence: 1.25,
        actual: true,
    }];
    let err =
        compute_calibration_for_samples("bad_confidence", &samples, 10, ECE_TOLERANCE).unwrap_err();
    Ok(json!({
        "case": "confidence_greater_than_one",
        "before": {"confidence": 1.25},
        "after": {"error": err.code()},
        "expected": "MEJEPA_EVAL_INVALID_INPUT",
        "actual": err.code(),
        "pass": err.code() == "MEJEPA_EVAL_INVALID_INPUT"
    }))
}

fn boundary_nonfinite_confidence() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let samples = [CalibrationSample {
        confidence: f32::NAN,
        actual: true,
    }];
    let err =
        compute_calibration_for_samples("nan_confidence", &samples, 10, ECE_TOLERANCE).unwrap_err();
    Ok(json!({
        "case": "nonfinite_confidence",
        "before": {"confidence": "NaN"},
        "after": {"error": err.code()},
        "expected": "MEJEPA_EVAL_INVALID_INPUT",
        "actual": err.code(),
        "pass": err.code() == "MEJEPA_EVAL_INVALID_INPUT"
    }))
}

fn cf_counts(db: &DB) -> Result<BTreeMap<String, usize>, Box<dyn std::error::Error>> {
    let mut out = BTreeMap::new();
    for cf_name in context_graph_mejepa_cf::EVAL_CFS {
        let cf = db
            .cf_handle(cf_name)
            .ok_or_else(|| format!("missing column family {cf_name}"))?;
        let mut count = 0usize;
        for item in db.iterator_cf(cf, IteratorMode::Start) {
            item?;
            count += 1;
        }
        out.insert((*cf_name).to_string(), count);
    }
    Ok(out)
}

fn flush_eval_cfs(db: &DB) -> Result<(), Box<dyn std::error::Error>> {
    for cf_name in context_graph_mejepa_cf::EVAL_CFS {
        let cf = db
            .cf_handle(cf_name)
            .ok_or_else(|| format!("missing column family {cf_name}"))?;
        db.flush_cf(cf)?;
    }
    Ok(())
}

fn list_sst_files(db_path: &Path) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    let mut stack = vec![db_path.to_path_buf()];
    let mut out = Vec::new();
    while let Some(path) = stack.pop() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "sst") {
                let size = fs::metadata(&path)?.len();
                out.push(json!({
                    "path": path,
                    "bytes": size
                }));
            }
        }
    }
    out.sort_by_key(|value| value["path"].to_string());
    Ok(out)
}

fn git_head_short() -> Result<String, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err("git rev-parse --short HEAD failed".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
