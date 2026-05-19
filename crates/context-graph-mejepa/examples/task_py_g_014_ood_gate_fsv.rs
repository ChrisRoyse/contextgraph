use anyhow::{ensure, Context, Result};
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa::fixtures::fixture_patch_context;
use context_graph_mejepa::{
    apply_ood_gate_decision, compute_ood_calibration_report, count_cf, open_infer_rocksdb,
    persist_ood_harvest_row, read_latest_ood_calibration_report, CalibrationExample,
    CalibrationStore, ChunkId, ConstellationGuard, DeterministicConstellationGuard,
    DeterministicOracleHead, DeterministicPredictor, IdentityFrozenTarget, Language,
    MeJepaCompiler, MeJepaInferConfig, OodHarvestConfig, OodHarvestRow, OodHarvestStatus,
    OracleOutcome, PatchWitnessReader, PredictionId, RocksDbInferStore, SyntheticOodCalibrationRow,
    TaskId, TrainCertSummary, Verdict, VerifyVerdict, DEFAULT_OOD_HARVEST_THRESHOLD,
    OOD_CALIBRATOR_MISSING, OOD_CELL_AUC_REGRESSION, OOD_HARVEST_ACTIVE_LEARNING_WEIGHT,
    OOD_HARVEST_EMPTY, OOD_SCORE_ABOVE_THRESHOLD,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_OOD_CALIBRATIONS, CF_MEJEPA_OOD_HARVEST,
    CF_MEJEPA_TRAIN_CERTS,
};
use rocksdb::{WriteOptions, DB};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const REPORT_NAME: &str = "ood_gate_fsv.json";
const READBACK_NAME: &str = "independent_rocksdb_readback.json";
const FSV_SUBDIR: &str = "task-py-g-014-ood-gate-fsv";

fn main() -> Result<()> {
    let run_id = format!("run-{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ"));
    let run_dir = fsv_root()?.join(run_id);
    fs::create_dir_all(&run_dir).context("create TASK-PY-G-014 FSV run dir")?;
    let now = chrono::Utc::now().timestamp_millis();

    let happy_db_path = run_dir.join("happy-ood-gate.rocksdb");
    let empty_db_path = run_dir.join("empty-harvest.rocksdb");
    let regression_db_path = run_dir.join("cell-regression.rocksdb");
    let missing_db_path = run_dir.join("missing-calibrator.rocksdb");
    let corrupt_db_path = run_dir.join("corrupt-calibrator.rocksdb");
    let cold_db_path = run_dir.join("cold-cell-precedence.rocksdb");

    let happy = run_happy_serving_case(&happy_db_path, &run_dir.join("happy-repo"), now)
        .context("happy serving case")?;
    let empty_harvest =
        run_empty_harvest_case(&empty_db_path, now + 100_000).context("empty harvest case")?;
    let cell_regression = run_cell_regression_case(&regression_db_path, now + 200_000)
        .context("cell regression case")?;
    let missing = run_missing_calibrator_case(
        &missing_db_path,
        &run_dir.join("missing-repo"),
        false,
        now + 300_000,
    )
    .context("missing calibrator case")?;
    let corrupt = run_missing_calibrator_case(
        &corrupt_db_path,
        &run_dir.join("corrupt-repo"),
        true,
        now + 400_000,
    )
    .context("corrupt calibrator case")?;
    let cold = run_cold_cell_case(&cold_db_path, &run_dir.join("cold-repo"), now + 500_000)
        .context("cold-cell precedence case")?;

    let readback = independent_readback(&[
        (&happy_db_path, "happy"),
        (&empty_db_path, "emptyHarvest"),
        (&regression_db_path, "cellRegression"),
        (&missing_db_path, "missingCalibrator"),
        (&corrupt_db_path, "corruptCalibrator"),
        (&cold_db_path, "coldCell"),
    ])?;

    let all_passed = happy["passed"].as_bool() == Some(true)
        && empty_harvest["passed"].as_bool() == Some(true)
        && cell_regression["passed"].as_bool() == Some(true)
        && missing["passed"].as_bool() == Some(true)
        && corrupt["passed"].as_bool() == Some(true)
        && cold["passed"].as_bool() == Some(true)
        && readback["allPassed"].as_bool() == Some(true);

    let report = json!({
        "task": "TASK-PY-G-014",
        "issue": 129,
        "allPassed": all_passed,
        "sourceOfTruth": {
            "fsvRoot": run_dir,
            "oodHarvestCf": CF_MEJEPA_OOD_HARVEST,
            "oodCalibrationCf": CF_MEJEPA_OOD_CALIBRATIONS,
            "livePredictionCf": CF_MEJEPA_LIVE_PREDICTIONS,
            "canonicalCorpusIndex": "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json",
            "canonicalForwardCache": "/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z/",
            "perSlotCalibrationState": "/zfs/archive/contextgraph/state/calibration/per-slot-v1/",
        },
        "cases": {
            "happyPathCalibratedServing": happy,
            "emptyHarvestSyntheticOnly": empty_harvest,
            "perCellAucRegression": cell_regression,
            "missingCalibratorFailsClosed": missing,
            "corruptCalibratorFailsClosed": corrupt,
            "coldCellPrecedence": cold,
        },
        "independentReadback": readback,
    });
    let report_path = run_dir.join(REPORT_NAME);
    let readback_path = run_dir.join(READBACK_NAME);
    write_json_0600(&report_path, &report).context("write TASK-PY-G-014 FSV report")?;
    write_json_0600(&readback_path, &readback).context("write TASK-PY-G-014 readback report")?;

    let report_readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(
        report_readback.get("allPassed").and_then(Value::as_bool) == Some(true),
        "TASK-PY-G-014 FSV report allPassed must be true"
    );
    println!(
        "task-py-g-014 OOD gate FSV allPassed=true report={} report_sha256={} readback={} readback_sha256={}",
        report_path.display(),
        sha256_file(&report_path)?,
        readback_path.display(),
        sha256_file(&readback_path)?,
    );
    Ok(())
}

fn run_happy_serving_case(db_path: &Path, repo_root: &Path, now: i64) -> Result<Value> {
    let config = OodHarvestConfig {
        threshold: DEFAULT_OOD_HARVEST_THRESHOLD,
        retention_active_ms: 90_000,
        queue_capacity: 128,
    };
    let (normal, ood, report_json) = {
        let db = open_infer_rocksdb(db_path).context("open happy OOD DB")?;
        seed_harvest_rows(
            db.as_ref(),
            "language=python:mutation=bool_flip",
            50,
            50,
            now,
        )?;
        let report = compute_ood_calibration_report(
            db.as_ref(),
            &config,
            now + 1_000,
            now,
            now + 1_000,
            &[],
        )
        .context("compute happy OOD calibration")?;
        ensure!(report.selected_for_serving, "happy OOD report must serve");
        ensure!(
            report.global_auc.unwrap_or_default() >= 0.85,
            "happy OOD AUC must meet threshold"
        );
        let report_json = serde_json::to_value(&report)?;

        let calibration = CalibrationStore::new(db.clone(), 30)?;
        seed_infer_calibration(db.as_ref(), &calibration)?;
        let compiler = strict_compiler(db.clone(), repo_root, calibration, high_support_guard())?;

        let (normal_patch, normal_context) = fixture_patch_context(repo_root, "known_good")?;
        let normal = compiler.verify(&normal_patch, &normal_context)?;

        let (ood_patch, ood_context) = fixture_patch_context(repo_root, "ood")?;
        let ood = compiler.verify(&ood_patch, &ood_context)?;
        (verdict_summary(&normal), verdict_summary(&ood), report_json)
    };

    let monotonic = run_monotonic_harvest_check(db_path, &config, now + 10_000)?;
    let passed = normal["isOod"].as_bool() == Some(false)
        && ood["failedGate"].as_str() == Some("OutOfDistribution")
        && ood["reason"].as_str() == Some(OOD_SCORE_ABOVE_THRESHOLD)
        && monotonic["passed"].as_bool() == Some(true);
    Ok(json!({
        "passed": passed,
        "report": report_json,
        "normalPythonRow": normal,
        "freshSyntheticOodRow": ood,
        "monotonicHarvestAuc": monotonic,
    }))
}

fn run_empty_harvest_case(db_path: &Path, now: i64) -> Result<Value> {
    let db = open_infer_rocksdb(db_path).context("open empty-harvest DB")?;
    let report = compute_ood_calibration_report(
        db.as_ref(),
        &OodHarvestConfig::default(),
        now,
        now - 1_000,
        now,
        &[
            synthetic("empty-id-1", 0.10, false),
            synthetic("empty-id-2", 0.20, false),
            synthetic("empty-ood-1", 0.91, true),
            synthetic("empty-ood-2", 0.95, true),
        ],
    )?;
    let passed = report.flags == vec![OOD_HARVEST_EMPTY.to_string()]
        && report.min_required_auc == 0.75
        && report.selected_for_serving
        && report.global_auc.unwrap_or_default() >= 0.75;
    Ok(json!({
        "passed": passed,
        "flags": report.flags,
        "selectedForServing": report.selected_for_serving,
        "globalAuc": report.global_auc,
        "minRequiredAuc": report.min_required_auc,
    }))
}

fn run_cell_regression_case(db_path: &Path, now: i64) -> Result<Value> {
    let db = open_infer_rocksdb(db_path).context("open cell-regression DB")?;
    seed_harvest_rows(
        db.as_ref(),
        "language=python:mutation=bool_flip",
        10,
        10,
        now,
    )?;
    persist_harvest_row(
        db.as_ref(),
        "bad-cell-id",
        "language=python:mutation=boundary",
        0.95,
        OracleOutcome::Pass,
        now + 100,
    )?;
    persist_harvest_row(
        db.as_ref(),
        "bad-cell-ood",
        "language=python:mutation=boundary",
        0.10,
        OracleOutcome::OutOfDistribution,
        now + 101,
    )?;
    let report = compute_ood_calibration_report(
        db.as_ref(),
        &OodHarvestConfig::default(),
        now + 1_000,
        now,
        now + 1_000,
        &[],
    )?;
    let bad_cell = report
        .cell_reports
        .iter()
        .find(|cell| cell.cell_id == "language=python:mutation=boundary")
        .context("bad boundary cell missing")?;
    let passed = report.global_auc.unwrap_or_default() >= 0.85
        && report.flags.contains(&OOD_CELL_AUC_REGRESSION.to_string())
        && bad_cell
            .flags
            .contains(&OOD_CELL_AUC_REGRESSION.to_string())
        && !report.selected_for_serving;
    Ok(json!({
        "passed": passed,
        "globalAuc": report.global_auc,
        "flags": report.flags,
        "badCell": bad_cell,
    }))
}

fn run_missing_calibrator_case(
    db_path: &Path,
    repo_root: &Path,
    corrupt: bool,
    now: i64,
) -> Result<Value> {
    let outcome = {
        let db = open_infer_rocksdb(db_path).context("open missing/corrupt OOD DB")?;
        let calibration = CalibrationStore::new(db.clone(), 30)?;
        seed_infer_calibration(db.as_ref(), &calibration)?;
        if corrupt {
            write_corrupt_ood_calibration_row(db.as_ref(), now)?;
        }
        let compiler = strict_compiler(db.clone(), repo_root, calibration, high_support_guard())?;
        let (patch, context) = fixture_patch_context(repo_root, "ood")?;
        compiler.verify(&patch, &context)?
    };
    let summary = verdict_summary(&outcome);
    let passed = summary["failedGate"].as_str() == Some("OodGateRejected")
        && summary["reason"].as_str() == Some(OOD_CALIBRATOR_MISSING);
    Ok(json!({
        "passed": passed,
        "corruptRowInjected": corrupt,
        "verdict": summary,
    }))
}

fn run_cold_cell_case(db_path: &Path, repo_root: &Path, now: i64) -> Result<Value> {
    let outcome = {
        let db = open_infer_rocksdb(db_path).context("open cold-cell OOD DB")?;
        seed_harvest_rows(
            db.as_ref(),
            "language=python:mutation=bool_flip",
            50,
            50,
            now,
        )?;
        compute_ood_calibration_report(
            db.as_ref(),
            &OodHarvestConfig::default(),
            now + 1_000,
            now,
            now + 1_000,
            &[],
        )?;
        let calibration = CalibrationStore::new(db.clone(), 30)?;
        seed_infer_calibration(db.as_ref(), &calibration)?;
        let compiler = strict_compiler(db.clone(), repo_root, calibration, low_support_guard())?;
        let (patch, context) = fixture_patch_context(repo_root, "ood")?;
        compiler.verify(&patch, &context)?
    };
    let summary = verdict_summary(&outcome);
    let direct = apply_ood_gate_decision(Verdict::Pass, 0.99, None, Some((Some(10), 50)))?;
    let passed = summary["failedGate"].as_str() == Some("ColdCell")
        && direct.verdict == Verdict::Abstain
        && !direct.reason.is_empty();
    Ok(json!({
        "passed": passed,
        "verifyVerdict": summary,
        "directGateDecision": direct,
    }))
}

fn run_monotonic_harvest_check(
    db_path: &Path,
    config: &OodHarvestConfig,
    now: i64,
) -> Result<Value> {
    let db = open_infer_rocksdb(db_path).context("reopen happy DB for monotonic check")?;
    let before = read_latest_ood_calibration_report(db.as_ref())?
        .context("latest happy report missing before monotonic check")?;
    persist_harvest_row(
        db.as_ref(),
        "monotonic-extra-ood",
        "language=python:mutation=bool_flip",
        0.98,
        OracleOutcome::OutOfDistribution,
        now,
    )?;
    let after =
        compute_ood_calibration_report(db.as_ref(), config, now + 1_000, now, now + 1_000, &[])?;
    let before_auc = before.global_auc.context("before AUC missing")?;
    let after_auc = after.global_auc.context("after AUC missing")?;
    Ok(json!({
        "passed": after_auc >= before_auc,
        "beforeAuc": before_auc,
        "afterAuc": after_auc,
        "beforeHarvestRows": before.harvested_rows,
        "afterHarvestRows": after.harvested_rows,
    }))
}

fn strict_compiler(
    db: Arc<DB>,
    repo_root: &Path,
    calibration: CalibrationStore,
    guard: DeterministicConstellationGuard,
) -> Result<MeJepaCompiler> {
    let store = Arc::new(RocksDbInferStore::new(db));
    let config = MeJepaInferConfig {
        require_ood_calibrator: true,
        pause_state_path: None,
        outcome_set_max: 4,
        ..MeJepaInferConfig::default()
    };
    Ok(MeJepaCompiler::new(
        config,
        Arc::new(DeterministicPredictor),
        Arc::new(IdentityFrozenTarget),
        Arc::new(DeterministicOracleHead),
        Arc::new(guard) as Arc<dyn ConstellationGuard>,
        Arc::new(PatchWitnessReader),
        store,
        calibration,
        repo_root.to_path_buf(),
    )?)
}

fn high_support_guard() -> DeterministicConstellationGuard {
    DeterministicConstellationGuard {
        cell_id: "mutation=bool_flip:language=python:entity=function".to_string(),
        n_supporting: Some(100),
        ..DeterministicConstellationGuard::default()
    }
}

fn low_support_guard() -> DeterministicConstellationGuard {
    DeterministicConstellationGuard {
        cell_id: "mutation=bool_flip:language=python:entity=function".to_string(),
        n_supporting: Some(10),
        ..DeterministicConstellationGuard::default()
    }
}

fn seed_infer_calibration(db: &DB, calibration: &CalibrationStore) -> Result<()> {
    let examples = (0..40)
        .map(|idx| CalibrationExample {
            language: Language::Python,
            predicted_test_pass: vec![0.95 - (idx as f32 * 0.0001)],
            actual_test_pass: vec![1.0],
        })
        .collect::<Vec<_>>();
    calibration.calibrate(
        &examples,
        &[0.01; 40],
        0.10,
        30,
        0.30,
        [8; 32],
        BTreeMap::new(),
    )?;
    let cf = db
        .cf_handle(CF_MEJEPA_TRAIN_CERTS)
        .context("CF_MEJEPA_TRAIN_CERTS missing")?;
    let cert = TrainCertSummary {
        step: 1,
        delta_omega: 0.9,
        delta_xi: 0.9,
        witness_offset: 88,
    };
    db.put_cf(cf, b"cert:task-py-g-014:0001", bincode::serialize(&cert)?)?;
    Ok(())
}

fn seed_harvest_rows(
    db: &DB,
    cell: &str,
    id_count: usize,
    ood_count: usize,
    now: i64,
) -> Result<()> {
    for idx in 0..id_count {
        persist_harvest_row(
            db,
            &format!("id-{cell}-{idx:03}"),
            cell,
            0.05 + idx as f32 * 0.001,
            OracleOutcome::Pass,
            now + idx as i64,
        )?;
    }
    for idx in 0..ood_count {
        persist_harvest_row(
            db,
            &format!("ood-{cell}-{idx:03}"),
            cell,
            0.91 + idx as f32 * 0.0005,
            OracleOutcome::OutOfDistribution,
            now + 10_000 + idx as i64,
        )?;
    }
    Ok(())
}

fn persist_harvest_row(
    db: &DB,
    seed: &str,
    cell: &str,
    ood_score: f32,
    outcome: OracleOutcome,
    now: i64,
) -> Result<()> {
    let row = OodHarvestRow {
        schema_version: context_graph_mejepa::OOD_HARVEST_SCHEMA_VERSION,
        prediction_id: PredictionId(id16(&format!("prediction-{seed}"))),
        task_id: TaskId(format!("task-py-g-014-{seed}")),
        session_id: id16("task-py-g-014-session"),
        panel_id: context_graph_mejepa::PanelId(id32(&format!("panel-{seed}"))),
        calibration_cell: cell.to_string(),
        affected_chunk_ids: vec![ChunkId(format!("chunk-{seed}"))],
        agent_prose: "TASK-PY-G-014 FSV synthetic OOD calibration row".to_string(),
        verdict: if outcome == OracleOutcome::OutOfDistribution {
            Verdict::OutOfDistribution
        } else {
            Verdict::Pass
        },
        ood_score,
        created_at_unix_ms: now,
        harvested_at_unix_ms: now + 1,
        oracle_outcome: Some(outcome),
        status: OodHarvestStatus::Active,
        priority_weight: OOD_HARVEST_ACTIVE_LEARNING_WEIGHT,
        source_live_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
    };
    persist_ood_harvest_row(db, &row)?;
    Ok(())
}

fn synthetic(row_id: &str, ood_score: f32, actual_ood: bool) -> SyntheticOodCalibrationRow {
    SyntheticOodCalibrationRow {
        row_id: row_id.to_string(),
        calibration_cell: "language=python:mutation=synthetic".to_string(),
        ood_score,
        predicted_ood: ood_score > DEFAULT_OOD_HARVEST_THRESHOLD,
        actual_ood,
    }
}

fn write_corrupt_ood_calibration_row(db: &DB, now: i64) -> Result<()> {
    let cf = db
        .cf_handle(CF_MEJEPA_OOD_CALIBRATIONS)
        .context("CF_MEJEPA_OOD_CALIBRATIONS missing")?;
    let mut key = now.to_be_bytes().to_vec();
    key.extend_from_slice(b"corrupt");
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, key, b"not-a-valid-bincode-report", &opts)?;
    Ok(())
}

fn verdict_summary(verdict: &VerifyVerdict) -> Value {
    match verdict {
        VerifyVerdict::Approve {
            reality_prediction,
            gates_passed,
        } => json!({
            "kind": "Approve",
            "failedGate": null,
            "reason": null,
            "isOod": reality_prediction.verdict == Verdict::OutOfDistribution,
            "predictionVerdict": reality_prediction.verdict,
            "oodScore": reality_prediction.ood_score,
            "gatesPassed": gates_passed,
        }),
        VerifyVerdict::EscalateToHuman {
            reality_prediction,
            failed_gate,
            gates_passed,
        } => {
            let (failed_gate_name, reason, threshold, ood_score) = match failed_gate {
                context_graph_mejepa::FailedGate::OutOfDistribution {
                    ood_score,
                    threshold,
                } => (
                    "OutOfDistribution",
                    OOD_SCORE_ABOVE_THRESHOLD.to_string(),
                    Some(*threshold),
                    Some(*ood_score),
                ),
                context_graph_mejepa::FailedGate::OodGateRejected {
                    reason,
                    ood_score,
                    threshold,
                } => (
                    "OodGateRejected",
                    reason.clone(),
                    *threshold,
                    Some(*ood_score),
                ),
                context_graph_mejepa::FailedGate::ColdCell { reason, .. } => {
                    ("ColdCell", reason.clone(), None, None)
                }
                other => (
                    std::any::type_name_of_val(other),
                    format!("{other:?}"),
                    None,
                    None,
                ),
            };
            json!({
                "kind": "EscalateToHuman",
                "failedGate": failed_gate_name,
                "reason": reason,
                "threshold": threshold,
                "oodScore": ood_score,
                "isOod": matches!(failed_gate, context_graph_mejepa::FailedGate::OutOfDistribution { .. }),
                "predictionVerdict": reality_prediction.as_ref().map(|prediction| prediction.verdict),
                "gatesPassed": gates_passed,
            })
        }
    }
}

fn independent_readback(paths: &[(&Path, &str)]) -> Result<Value> {
    let mut rows = Vec::new();
    for (path, label) in paths {
        let db = open_infer_rocksdb(path).with_context(|| format!("reopen {}", path.display()))?;
        let latest = read_latest_ood_calibration_report(db.as_ref())
            .ok()
            .flatten();
        let row = json!({
            "label": label,
            "path": path,
            "oodHarvestRows": count_cf(db.as_ref(), CF_MEJEPA_OOD_HARVEST)?,
            "oodCalibrationRows": count_cf(db.as_ref(), CF_MEJEPA_OOD_CALIBRATIONS)?,
            "livePredictionRows": count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?,
            "latestSelectedForServing": latest.as_ref().map(|report| report.selected_for_serving),
            "latestFlags": latest.as_ref().map(|report| report.flags.clone()),
        });
        rows.push(row);
    }
    let all_passed = rows.iter().all(|row| {
        row.get("oodCalibrationRows")
            .and_then(Value::as_u64)
            .unwrap_or_default()
            >= if row.get("label").and_then(Value::as_str) == Some("missingCalibrator") {
                0
            } else {
                1
            }
    });
    Ok(json!({
        "allPassed": all_passed,
        "databases": rows,
    }))
}

fn fsv_root() -> Result<PathBuf> {
    if let Ok(root) = std::env::var("CG_TASK_PY_G_014_FSV_ROOT") {
        return Ok(PathBuf::from(root));
    }
    Ok(PathBuf::from("/tmp/contextgraph").join(FSV_SUBDIR))
}

fn id16(seed: &str) -> [u8; 16] {
    let digest = Sha256::digest(seed.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn id32(seed: &str) -> [u8; 32] {
    let digest = Sha256::digest(seed.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}
