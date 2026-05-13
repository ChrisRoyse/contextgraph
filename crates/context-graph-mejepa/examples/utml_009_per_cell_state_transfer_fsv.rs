use context_graph_mejepa::eval::report::write_json_0600;
use context_graph_mejepa::eval::{
    build_eval_compiler, cell_key, corpus_sha_from_holdout, synthetic_holdout, EvalConfig,
    EvalProvenance, EvalReport, EvalRunner, MutationCategory, StateTransferDiagnostic,
};
use context_graph_mejepa::{open_infer_rocksdb, Language, RocksDbEvalStore, TaskId};
use rocksdb::IteratorMode;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/utml-009-fsv";
const REPORT_FILE: &str = "per_cell_state_transfer_fsv.json";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let started_at_unix_ms = chrono::Utc::now().timestamp_millis();
    let fsv_root = PathBuf::from(FSV_ROOT);
    fs::create_dir_all(&fsv_root)?;
    let run_root = fsv_root.join(format!("run-{}-{}", started_at_unix_ms, std::process::id()));
    fs::create_dir_all(&run_root)?;

    let db_path = run_root.join("rocksdb");
    let repo_root = run_root.join("repo");
    let (db, compiler, eval_store, calibration) = build_eval_compiler(&db_path, &repo_root)?;
    let mut holdout = synthetic_holdout(&repo_root)?;
    duplicate_known_good_cell(&mut holdout)?;
    let config = EvalConfig::default();
    let cf_counts_before = cf_counts(db.as_ref())?;
    let provenance = EvalProvenance {
        corpus_sha: corpus_sha_from_holdout(&holdout),
        eval_code_version: env!("CARGO_PKG_VERSION").to_string(),
        calibration_version: calibration.load_active()?.version.clone(),
        generated_by: "utml_009_per_cell_state_transfer_fsv".to_string(),
    };
    let runner = EvalRunner::new(compiler, eval_store.clone(), config)?;
    let report = runner.run_holdout_eval(&holdout, &[], "2026-05-13-utml-009", provenance)?;
    let cell = cell_key(MutationCategory::KnownGood, Language::Python);
    let diagnostic = report
        .per_cell_state_transfer
        .get(&cell)
        .and_then(|value| value.as_ref())
        .ok_or("known_good::python state transfer missing")?;
    ensure_unit_diagnostic(diagnostic)?;

    let latest = eval_store
        .load_latest_report()?
        .ok_or("latest eval report missing")?;
    let report_hash = report.determinism_hash()?;
    let latest_hash = latest.determinism_hash()?;
    if latest_hash != report_hash {
        return Err("latest persisted report hash differs".into());
    }
    write_json_0600(&run_root.join("weekly-report.json"), &report)?;
    flush_eval_cfs(db.as_ref())?;
    let cf_counts_after = cf_counts(db.as_ref())?;
    drop(runner);
    drop(eval_store);
    drop(calibration);
    drop(db);

    let reopened_db = open_infer_rocksdb(&db_path)?;
    let reopened_store = RocksDbEvalStore::new(reopened_db.clone())?;
    let reopened = reopened_store
        .load_latest_report()?
        .ok_or("reopened latest eval report missing")?;
    let reopened_hash = reopened.determinism_hash()?;
    let readback_equal = reopened_hash == report_hash;
    if !readback_equal {
        return Err("reopened eval report hash differs".into());
    }

    let boundary_cases = vec![
        boundary_empty_holdout(&runnerless_eval(&run_root, "empty")?)?,
        boundary_single_sample_cells(&run_root)?,
        boundary_invalid_extra_cell_key()?,
        boundary_nonfinite_diagnostic()?,
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
        "task_id": "TASK-UTML-009",
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": git_head_short()?,
        "run_root": run_root,
        "happy_path": [{
            "case": "per_cell_state_transfer_persisted",
            "sot": context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS,
            "before": cf_counts_before[context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS],
            "trigger": "cargo run -p context-graph-mejepa --example utml_009_per_cell_state_transfer_fsv",
            "after": cf_counts_after[context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS],
            "expected": {
                "cell": cell,
                "diagnostic_present": true,
                "readback_equal": true
            },
            "actual": {
                "cell": cell,
                "diagnostic": diagnostic,
                "report_hash": report_hash,
                "reopened_hash": reopened_hash
            },
            "pass": true,
            "evidence_path": run_root.join("weekly-report.json")
        }],
        "boundary_cases": boundary_cases,
        "all_passed": all_boundaries_pass && readback_equal,
        "cf_counts_before": cf_counts_before,
        "cf_counts_after": cf_counts_after,
        "readback_equal": readback_equal,
        "physical_artifacts": {
            "db_path": db_path,
            "sst_files": sst_files
        }
    });
    if !evidence["all_passed"].as_bool().unwrap_or(false) {
        return Err("TASK-UTML-009 FSV evidence did not pass".into());
    }
    let evidence_path = fsv_root.join(REPORT_FILE);
    write_json_0600(&evidence_path, &evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn duplicate_known_good_cell(
    holdout: &mut Vec<context_graph_mejepa::HoldoutPanel>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut duplicate = holdout
        .iter()
        .find(|panel| panel.mutation_category == MutationCategory::KnownGood)
        .cloned()
        .ok_or("synthetic holdout missing known-good panel")?;
    duplicate.task_id = TaskId("utml-009-known-good-duplicate".to_string());
    duplicate.context.task_id = duplicate.task_id.clone();
    holdout.push(duplicate);
    Ok(())
}

fn runnerless_eval(run_root: &Path, name: &str) -> Result<EvalRunner, Box<dyn std::error::Error>> {
    let db_path = run_root.join(format!("{name}-rocksdb"));
    let repo_root = run_root.join(format!("{name}-repo"));
    let (_db, compiler, eval_store, _calibration) = build_eval_compiler(&db_path, &repo_root)?;
    Ok(EvalRunner::new(
        compiler,
        eval_store,
        EvalConfig::default(),
    )?)
}

fn boundary_empty_holdout(
    runner: &EvalRunner,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let err = runner
        .run_holdout_eval(
            &[],
            &[],
            "2026-05-13-utml-009-empty",
            edge_provenance("empty"),
        )
        .expect_err("empty holdout must fail closed");
    Ok(json!({
        "case": "empty_holdout",
        "sot": context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS,
        "expected": "MEJEPA_EVAL_EMPTY_HOLDOUT",
        "actual": err.code(),
        "pass": err.code() == "MEJEPA_EVAL_EMPTY_HOLDOUT"
    }))
}

fn boundary_single_sample_cells(
    run_root: &Path,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let db_path = run_root.join("single-sample-rocksdb");
    let repo_root = run_root.join("single-sample-repo");
    let (_db, compiler, eval_store, calibration) = build_eval_compiler(&db_path, &repo_root)?;
    let holdout = synthetic_holdout(&repo_root)?;
    let runner = EvalRunner::new(compiler, eval_store, EvalConfig::default())?;
    let report = runner.run_holdout_eval(
        &holdout,
        &[],
        "2026-05-13-utml-009-single",
        EvalProvenance {
            corpus_sha: corpus_sha_from_holdout(&holdout),
            eval_code_version: env!("CARGO_PKG_VERSION").to_string(),
            calibration_version: calibration.load_active()?.version.clone(),
            generated_by: "utml_009_single_sample_boundary".to_string(),
        },
    )?;
    let none_count = report
        .per_cell_state_transfer
        .values()
        .filter(|diagnostic| diagnostic.is_none())
        .count();
    Ok(json!({
        "case": "single_sample_cells",
        "sot": context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS,
        "expected": "all per-cell transfer values are unavailable with one sample per cell",
        "actual": {
            "cell_count": report.per_cell_state_transfer.len(),
            "none_count": none_count
        },
        "pass": none_count == report.per_cell_state_transfer.len()
    }))
}

fn boundary_invalid_extra_cell_key() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut report = minimal_report()?;
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
        "expected": "MEJEPA_EVAL_INVALID_INPUT",
        "actual": err.code(),
        "pass": err.code() == "MEJEPA_EVAL_INVALID_INPUT"
    }))
}

fn boundary_nonfinite_diagnostic() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut report = minimal_report()?;
    let cell = cell_key(MutationCategory::KnownGood, Language::Python);
    report.per_cell_state_transfer.insert(
        cell,
        Some(StateTransferDiagnostic {
            wasserstein_1: f32::NAN,
            transfer_score: 0.8,
            performance_deploy: 0.7,
        }),
    );
    let err = report
        .validate()
        .expect_err("nonfinite diagnostic must fail closed");
    Ok(json!({
        "case": "nonfinite_state_transfer",
        "sot": "EvalReport::validate",
        "expected": "MEJEPA_EVAL_INVALID_INPUT",
        "actual": err.code(),
        "pass": err.code() == "MEJEPA_EVAL_INVALID_INPUT"
    }))
}

fn minimal_report() -> Result<EvalReport, Box<dyn std::error::Error>> {
    let cell = cell_key(MutationCategory::KnownGood, Language::Python);
    let per_cell_correlation = BTreeMap::from([(cell.clone(), Some(1.0))]);
    let per_cell_convergence_eta =
        context_graph_mejepa::baseline_convergence_eta_for_cells(&per_cell_correlation, 0.95);
    let mut per_cell_state_transfer = BTreeMap::new();
    per_cell_state_transfer.insert(cell.clone(), None);
    Ok(EvalReport {
        report_date: "2026-05-13-utml-009-minimal".to_string(),
        generated_at_unix_ms: chrono::Utc::now().timestamp_millis(),
        rolling_window_size: 2,
        holdout_count: 2,
        overall_correlation: Some(1.0),
        per_category_correlation: BTreeMap::from([(MutationCategory::KnownGood, Some(1.0))]),
        per_language_correlation: BTreeMap::from([(Language::Python, Some(1.0))]),
        per_cell_correlation,
        cell_exemptions: BTreeMap::new(),
        bayesian_shrinkage: BTreeMap::from([("known_good::python".to_string(), 1.0)]),
        conformal_coverage_health: BTreeMap::new(),
        ood_calibration_health: BTreeMap::new(),
        gtau_pass_rate: BTreeMap::new(),
        per_prediction_class_calibration: BTreeMap::new(),
        per_failure_mode_class: context_graph_mejepa::empty_failure_mode_class_metrics(
            2,
            &context_graph_mejepa::EvalConfig::default(),
        ),
        per_cell_convergence_eta,
        active_learning: context_graph_mejepa::ActiveLearningSummary {
            queued_count: 0,
            evicted_count: 0,
            ood_escalation_count: 0,
        },
        state_transfer_diagnostic: None,
        per_cell_state_transfer,
        failing_cell_classifications: BTreeMap::new(),
        aux_head_distillation: None,
        regression_checks: Vec::new(),
        open_research_questions: Vec::new(),
        q1_pass_rate: 1.0,
        q2_report_correlation: Some(1.0),
        q3_side_effect_agreement: Some(1.0),
        ship_gate_passed: true,
        ship_gate_failures: Vec::new(),
        provenance: edge_provenance("minimal"),
        wall_clock_seconds: 0.0,
    })
}

fn edge_provenance(name: &str) -> EvalProvenance {
    EvalProvenance {
        corpus_sha: format!("utml-009-{name}"),
        eval_code_version: env!("CARGO_PKG_VERSION").to_string(),
        calibration_version: "utml-009-boundary".to_string(),
        generated_by: format!("utml_009_{name}_boundary"),
    }
}

fn ensure_unit_diagnostic(
    diagnostic: &StateTransferDiagnostic,
) -> Result<(), Box<dyn std::error::Error>> {
    if !(0.0..=1.0).contains(&diagnostic.wasserstein_1)
        || !(0.0..=1.0).contains(&diagnostic.transfer_score)
        || !(0.0..=1.0).contains(&diagnostic.performance_deploy)
    {
        return Err("state-transfer diagnostic outside [0,1]".into());
    }
    Ok(())
}

fn cf_counts(db: &rocksdb::DB) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut counts = serde_json::Map::new();
    for cf_name in context_graph_mejepa_cf::EVAL_CFS {
        let cf = db.cf_handle(cf_name).ok_or(format!("missing {cf_name}"))?;
        counts.insert(
            cf_name.to_string(),
            json!(db.iterator_cf(cf, IteratorMode::Start).count()),
        );
    }
    Ok(serde_json::Value::Object(counts))
}

fn flush_eval_cfs(db: &rocksdb::DB) -> Result<(), Box<dyn std::error::Error>> {
    for cf_name in context_graph_mejepa_cf::EVAL_CFS {
        let cf = db.cf_handle(cf_name).ok_or(format!("missing {cf_name}"))?;
        db.flush_cf(cf)?;
    }
    Ok(())
}

fn list_sst_files(root: &Path) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    collect_sst_files(root, &mut out)?;
    Ok(out)
}

fn collect_sst_files(
    root: &Path,
    out: &mut Vec<serde_json::Value>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_sst_files(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("sst") {
            let bytes = fs::metadata(&path)?.len();
            if bytes == 0 {
                return Err(format!("{} is empty", path.display()).into());
            }
            out.push(json!({
                "path": path,
                "bytes": bytes,
                "sha256": sha256_file(&path)?
            }));
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}

fn git_head_short() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()?;
    if !output.status.success() {
        return Err("git rev-parse --short HEAD failed".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
