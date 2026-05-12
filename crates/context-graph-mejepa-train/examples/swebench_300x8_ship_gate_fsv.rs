//! Real SWE-bench Lite Python 300x8 corpus train/holdout ship-gate FSV.
//!
//! Source of truth:
//! - D-drive corpus index/stats/verify evidence.
//! - RocksDB CF_MEJEPA_TRAIN_CERTS for training certificates.
//! - RocksDB CF_MEJEPA_HOLDOUT_REPORTS for rolling holdout reports.
//! - The final JSON ledger written here and immediately read back.

use candle_core::{DType, Device, Tensor};
use context_graph_mejepa_train::cert::{
    open_train_rocksdb, verify_chain, CF_MEJEPA_HOLDOUT_REPORTS, CF_MEJEPA_TRAIN_CERTS,
};
use context_graph_mejepa_train::cli::load_corpus_index_dataset;
use context_graph_mejepa_train::eval::holdout::{
    HoldoutDataset, HoldoutExample, OracleClass, OracleHead, OraclePrediction, PredictorForward,
};
use context_graph_mejepa_train::eval::{HoldoutEvaluator, HoldoutReport, MutationCategory};
use context_graph_mejepa_train::trainer::Trainer;
use context_graph_mejepa_train::{TrainerError, TrainerErrorCode, TrainingConfig};
use rocksdb::{IteratorMode, DB};
use serde_json::{json, Value};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::ExitCode;

const CORPUS_DIR: &str = "/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511";
const INDEX_PATH: &str =
    "/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json";
const STATS_PATH: &str =
    "/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/stats.json";
const VERIFY_PATH: &str =
    "/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/verify-evidence.json";
const FSV_DIR: &str = "/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv";
const DB_DIR: &str = "/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv/train-eval-rocksdb";
const EVIDENCE_PATH: &str =
    "/mnt/d/contextgraph/fsv/swebench-300x8-ship-gate-fsv/ship-gate-fsv.json";
const SHIP_GATE_TARGET: f32 = 0.95;
const FINAL_HOLDOUT_STEP: u64 = 20_000;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::from(0),
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let fsv_dir = Path::new(FSV_DIR);
    let _ = fs::remove_dir_all(fsv_dir);
    fs::create_dir_all(fsv_dir)?;

    let verify_evidence = read_json(VERIFY_PATH)?;
    ensure(
        verify_evidence["passes"].as_bool() == Some(true),
        "corpus verify-evidence.json must pass",
    )?;
    ensure(
        verify_evidence["entry_count"].as_u64() == Some(2400),
        "corpus verify-evidence.json must contain 2400 entries",
    )?;
    let stats = read_json(STATS_PATH)?;
    ensure(
        stats["full_ship_gate_300x8"]["passes"].as_bool() == Some(true),
        "corpus stats full_ship_gate_300x8 must pass",
    )?;
    ensure(
        stats["leakage_check"]["passes"].as_bool() == Some(true),
        "corpus stats leakage_check must pass",
    )?;

    let dataset = load_corpus_index_dataset(Path::new(INDEX_PATH))?;
    let split_counts = split_counts_json(&dataset);
    ensure(
        split_counts["train"].as_u64() == Some(1920)
            && split_counts["calibration"].as_u64() == Some(240)
            && split_counts["holdout"].as_u64() == Some(240),
        "loaded corpus split counts must match the verified 1920/240/240 source of truth",
    )?;

    let db = open_train_rocksdb(DB_DIR)?;
    let before_db_counts = db_counts(&db)?;
    let mut config = TrainingConfig {
        epochs: 1,
        batch_size: 64,
        random_seed: 0x2026_0512,
        ..TrainingConfig::default()
    };
    config.phase3_dod_min_agreement = SHIP_GATE_TARGET;
    config.validate()?;
    let mut trainer = Trainer::new(config.clone(), db.clone(), Device::Cpu)?;
    let training_result = trainer.run_full_training(&dataset)?;

    let expected_cert_count = dataset.train.examples.len().div_ceil(config.batch_size) as u64;
    let cert_count = count_cf(&db, CF_MEJEPA_TRAIN_CERTS)?;
    ensure(
        cert_count == expected_cert_count,
        format!("expected {expected_cert_count} train certs, found {cert_count}"),
    )?;
    let chain_report = verify_chain(&db, CF_MEJEPA_TRAIN_CERTS, 0, expected_cert_count - 1)?;
    ensure(
        chain_report.broken_at.is_none() && chain_report.verified == expected_cert_count,
        "training certificate chain must verify from RocksDB readback",
    )?;

    let majority_oracle = CategoryMajorityOracle::from_train(&dataset.train.examples)?;
    let evaluator = HoldoutEvaluator::new(db.clone(), CF_MEJEPA_HOLDOUT_REPORTS.to_string());
    let predictor = EchoPredictor;
    let rolling_reports = evaluate_rolling_windows(
        &evaluator,
        &predictor,
        &majority_oracle,
        &dataset.holdout.examples,
        &dataset.calibration,
        &db,
    )?;
    let final_report = evaluator.evaluate(
        &predictor,
        &majority_oracle,
        &dataset.holdout,
        &dataset.calibration,
        FINAL_HOLDOUT_STEP,
        true,
        SHIP_GATE_TARGET,
    )?;
    let final_readback = read_holdout_report(&db, FINAL_HOLDOUT_STEP)?;
    ensure(
        final_readback.prediction_oracle_agreement == final_report.prediction_oracle_agreement
            && final_readback.phase3_dod_passed == final_report.phase3_dod_passed,
        "final holdout report readback must match the emitted report",
    )?;
    let holdout_report_count = count_cf(&db, CF_MEJEPA_HOLDOUT_REPORTS)?;
    ensure(
        holdout_report_count == rolling_reports.len() as u64 + 1,
        "holdout report CF count must equal rolling windows plus final report",
    )?;

    let edge_cases = run_edge_cases(fsv_dir, &db)?;
    let after_db_counts = db_counts(&db)?;
    let evidence = json!({
        "source_of_truth": {
            "corpus_dir": CORPUS_DIR,
            "index": INDEX_PATH,
            "stats": STATS_PATH,
            "verify_evidence": VERIFY_PATH,
            "rocksdb": DB_DIR,
            "train_cf": CF_MEJEPA_TRAIN_CERTS,
            "holdout_cf": CF_MEJEPA_HOLDOUT_REPORTS,
            "evidence": EVIDENCE_PATH
        },
        "best_practice_research": [
            {
                "source": "Google Cloud ML quality guidelines",
                "url": "https://docs.cloud.google.com/architecture/guidelines-for-developing-high-quality-ml-solutions",
                "applied": "kept train, calibration, and holdout splits separate; final scoring uses holdout only"
            },
            {
                "source": "AWS Prescriptive Guidance on splits and leakage",
                "url": "https://docs.aws.amazon.com/prescriptive-guidance/latest/ml-operations-planning/splits-leakage.html",
                "applied": "removed oracle label from feature tensor and fail-closed on missing/unknown split buckets"
            },
            {
                "source": "Google production ML monitoring",
                "url": "https://developers.google.com/machine-learning/crash-course/production-ml-systems/monitoring",
                "applied": "persisted rolling-window reports and physical RocksDB readback counts"
            }
        ],
        "pre_state": {
            "corpus_verify": verify_evidence,
            "corpus_stats": {
                "total_entries": stats["total_entries"],
                "by_bucket": stats["by_bucket"],
                "selected_task_count": stats["selected_task_count"],
                "task_manifest_count": stats["task_manifest_count"],
                "leakage_check": stats["leakage_check"],
                "full_ship_gate_300x8": stats["full_ship_gate_300x8"]
            },
            "rocksdb_counts": before_db_counts
        },
        "execute_and_inspect": {
            "loaded_split_counts": split_counts,
            "training_result": training_result,
            "expected_train_cert_count": expected_cert_count,
            "physical_train_cert_count": cert_count,
            "chain_report": chain_report,
            "category_majority_model": majority_oracle.summary_json(),
            "ship_gate_target": SHIP_GATE_TARGET,
            "rolling_reports": rolling_reports,
            "final_report": final_report,
            "final_report_readback": final_readback,
            "physical_holdout_report_count": holdout_report_count,
            "ship_gate_passed": final_report.phase3_dod_passed == Some(true),
            "readiness_blockers": readiness_blockers(&final_report)
        },
        "edge_case_audit": edge_cases,
        "post_state": {
            "rocksdb_counts": after_db_counts
        }
    });
    write_0600(EVIDENCE_PATH, &evidence)?;
    let readback = read_json(EVIDENCE_PATH)?;
    ensure(
        readback["execute_and_inspect"]["physical_train_cert_count"].as_u64()
            == Some(expected_cert_count),
        "evidence readback must contain expected physical train cert count",
    )?;
    ensure(
        readback["execute_and_inspect"]["physical_holdout_report_count"].as_u64()
            == Some(holdout_report_count),
        "evidence readback must contain expected physical holdout report count",
    )?;
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn evaluate_rolling_windows(
    evaluator: &HoldoutEvaluator,
    predictor: &dyn PredictorForward,
    oracle: &dyn OracleHead,
    holdout: &[HoldoutExample],
    calibration: &context_graph_mejepa_train::eval::holdout::CalibrationDataset,
    db: &DB,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    ensure(!holdout.is_empty(), "holdout split must not be empty")?;
    let window_size = holdout.len() / 3;
    ensure(
        window_size > 0 && holdout.len().is_multiple_of(window_size),
        "holdout split must divide cleanly into rolling windows",
    )?;
    let mut out = Vec::new();
    for (idx, window) in holdout.chunks(window_size).enumerate() {
        let step = 10_000 + idx as u64;
        let report = evaluator.evaluate(
            predictor,
            oracle,
            &HoldoutDataset {
                examples: window.to_vec(),
            },
            calibration,
            step,
            false,
            SHIP_GATE_TARGET,
        )?;
        let readback = read_holdout_report(db, step)?;
        ensure(
            readback.prediction_oracle_agreement == report.prediction_oracle_agreement,
            format!("rolling window {idx} report readback mismatch"),
        )?;
        out.push(json!({
            "window_index": idx,
            "step": step,
            "before": {"holdout_report_count": idx},
            "after": {
                "readback_step": readback.step,
                "sample_count": window.len(),
                "agreement": readback.prediction_oracle_agreement,
                "phase3_dod_passed": readback.phase3_dod_passed
            }
        }));
    }
    Ok(out)
}

fn run_edge_cases(fsv_dir: &Path, db: &DB) -> Result<Value, Box<dyn std::error::Error>> {
    let before_counts = db_counts(db)?;
    let missing_bucket_path = fsv_dir.join("edge-missing-bucket-index.json");
    write_json_raw(
        &missing_bucket_path,
        &json!({
            "entries": [
                {
                    "task_id": "edge-missing-bucket",
                    "category": "known_good",
                    "oracle_all_passed": true
                }
            ]
        }),
    )?;
    let missing_bucket = load_corpus_index_dataset(&missing_bucket_path).unwrap_err();
    let after_missing_counts = db_counts(db)?;

    let invalid_bucket_path = fsv_dir.join("edge-invalid-bucket-index.json");
    write_json_raw(
        &invalid_bucket_path,
        &json!({
            "entries": [
                {
                    "task_id": "edge-invalid-bucket",
                    "category": "known_good",
                    "bucket": "validation",
                    "oracle_all_passed": true
                }
            ]
        }),
    )?;
    let invalid_bucket = load_corpus_index_dataset(&invalid_bucket_path).unwrap_err();
    let after_invalid_counts = db_counts(db)?;

    let leakage_audit_path = fsv_dir.join("edge-label-leakage-index.json");
    write_json_raw(
        &leakage_audit_path,
        &json!({
            "entries": [
                {
                    "task_id": "edge-same-task",
                    "category": "known_good",
                    "bucket": "train",
                    "oracle_all_passed": true
                },
                {
                    "task_id": "edge-same-task",
                    "category": "known_good",
                    "bucket": "train",
                    "oracle_all_passed": false
                },
                {
                    "task_id": "edge-calibration",
                    "category": "compile_error",
                    "bucket": "calibration",
                    "oracle_all_passed": false
                },
                {
                    "task_id": "edge-holdout",
                    "category": "subtle_flip",
                    "bucket": "holdout",
                    "oracle_all_passed": true
                }
            ]
        }),
    )?;
    let leakage_dataset = load_corpus_index_dataset(&leakage_audit_path)?;
    let first = &leakage_dataset.train.examples[0];
    let second = &leakage_dataset.train.examples[1];
    let first_panel = tensor_values(&first.panel_t01)?;
    let second_panel = tensor_values(&second.panel_t01)?;
    ensure(
        first.actual_oracle_pass != second.actual_oracle_pass && first_panel == second_panel,
        "label leakage audit requires identical features when only oracle_all_passed changes",
    )?;
    let after_leakage_counts = db_counts(db)?;

    Ok(json!([
        {
            "name": "missing_bucket_fails_closed",
            "before": {
                "index_path": missing_bucket_path,
                "rocksdb_counts": before_counts
            },
            "after": {
                "error": missing_bucket,
                "rocksdb_counts": after_missing_counts
            }
        },
        {
            "name": "invalid_bucket_fails_closed",
            "before": {
                "index_path": invalid_bucket_path,
                "bucket": "validation",
                "rocksdb_counts": after_missing_counts
            },
            "after": {
                "error": invalid_bucket,
                "rocksdb_counts": after_invalid_counts
            }
        },
        {
            "name": "oracle_label_not_encoded_in_features",
            "before": {
                "index_path": leakage_audit_path,
                "first_actual_oracle_pass": first.actual_oracle_pass,
                "second_actual_oracle_pass": second.actual_oracle_pass
            },
            "after": {
                "features_equal": first_panel == second_panel,
                "first_panel": first_panel,
                "second_panel": second_panel,
                "split_counts": split_counts_json(&leakage_dataset),
                "rocksdb_counts": after_leakage_counts
            }
        }
    ]))
}

struct EchoPredictor;

impl PredictorForward for EchoPredictor {
    fn forward(&self, panel_t01: &Tensor) -> Result<Tensor, TrainerError> {
        Ok(panel_t01.clone())
    }
}

#[derive(Debug, Clone)]
struct CategoryMajorityOracle {
    pass_by_category: [bool; 8],
    counts: [CategoryCount; 8],
}

#[derive(Debug, Clone, Copy, Default)]
struct CategoryCount {
    pass: u64,
    fail: u64,
}

impl CategoryMajorityOracle {
    fn from_train(examples: &[HoldoutExample]) -> Result<Self, TrainerError> {
        if examples.is_empty() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                "cannot build category-majority oracle from empty train split",
            ));
        }
        let mut counts = [CategoryCount::default(); 8];
        for example in examples {
            let idx = category_index(example.category);
            if example.actual_oracle_pass {
                counts[idx].pass += 1;
            } else {
                counts[idx].fail += 1;
            }
        }
        let mut pass_by_category = [false; 8];
        for (idx, count) in counts.iter().enumerate() {
            if count.pass + count.fail == 0 {
                return Err(TrainerError::new(
                    TrainerErrorCode::MejepaTrainConfigInvalid,
                    format!(
                        "train split has no examples for category {}",
                        category_slug(MutationCategory::ALL[idx])
                    ),
                ));
            }
            pass_by_category[idx] = count.pass >= count.fail;
        }
        Ok(Self {
            pass_by_category,
            counts,
        })
    }

    fn summary_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        for category in MutationCategory::ALL {
            let idx = category_index(category);
            map.insert(
                category_slug(category).to_string(),
                json!({
                    "train_pass": self.counts[idx].pass,
                    "train_fail": self.counts[idx].fail,
                    "majority_prediction": self.pass_by_category[idx]
                }),
            );
        }
        Value::Object(map)
    }
}

impl OracleHead for CategoryMajorityOracle {
    fn predict(&self, predicted_latent: &Tensor) -> Result<OraclePrediction, TrainerError> {
        let values = tensor_values(predicted_latent)?;
        if values.len() != 2 {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!(
                    "expected 2-value corpus feature panel, got {}",
                    values.len()
                ),
            ));
        }
        let raw_category = values[1];
        if !raw_category.is_finite() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainLossNan,
                format!("category feature is non-finite: {raw_category}"),
            ));
        }
        let idx = (raw_category * (MutationCategory::ALL.len() - 1) as f32).round() as isize;
        if !(0..MutationCategory::ALL.len() as isize).contains(&idx) {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!("decoded category feature {raw_category} outside known category range"),
            ));
        }
        let pass = self.pass_by_category[idx as usize];
        Ok(OraclePrediction {
            pass_to_pass_resolved: pass,
            conformal_set: vec![OracleClass::Pass, OracleClass::Fail],
            class_logits: if pass { vec![0.0, 4.0] } else { vec![4.0, 0.0] },
        })
    }
}

fn split_counts_json(dataset: &context_graph_mejepa_train::trainer::TrainingDataset) -> Value {
    json!({
        "train": dataset.train.examples.len(),
        "calibration": dataset.calibration.examples.len(),
        "holdout": dataset.holdout.examples.len()
    })
}

fn readiness_blockers(report: &HoldoutReport) -> Vec<String> {
    let mut blockers = Vec::new();
    if report.prediction_oracle_agreement < SHIP_GATE_TARGET {
        blockers.push(format!(
            "prediction_oracle_agreement {:.6} < ship_gate_target {:.6}",
            report.prediction_oracle_agreement, SHIP_GATE_TARGET
        ));
    }
    for (category, accuracy) in &report.per_mutation_category_accuracy {
        if *accuracy < SHIP_GATE_TARGET {
            blockers.push(format!(
                "per_category {} {:.6} < {:.6}",
                category_slug(*category),
                accuracy,
                SHIP_GATE_TARGET
            ));
        }
    }
    blockers
}

fn db_counts(db: &DB) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(json!({
        CF_MEJEPA_TRAIN_CERTS: count_cf(db, CF_MEJEPA_TRAIN_CERTS)?,
        CF_MEJEPA_HOLDOUT_REPORTS: count_cf(db, CF_MEJEPA_HOLDOUT_REPORTS)?
    }))
}

fn count_cf(db: &DB, cf_name: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| format!("missing column family {cf_name}"))?;
    let mut count = 0;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        item?;
        count += 1;
    }
    Ok(count)
}

fn read_holdout_report(db: &DB, step: u64) -> Result<HoldoutReport, Box<dyn std::error::Error>> {
    let cf = db
        .cf_handle(CF_MEJEPA_HOLDOUT_REPORTS)
        .ok_or("missing holdout report CF")?;
    let bytes = db
        .get_cf(cf, step.to_be_bytes())?
        .ok_or_else(|| format!("missing holdout report at step {step}"))?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn tensor_values(tensor: &Tensor) -> Result<Vec<f32>, TrainerError> {
    Ok(tensor
        .to_dtype(DType::F32)?
        .flatten_all()?
        .to_vec1::<f32>()?)
}

fn read_json(path: impl AsRef<Path>) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&fs::read(path.as_ref())?)?)
}

fn write_0600(path: impl AsRef<Path>, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, &bytes)?;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    ensure(
        fs::metadata(path)?.permissions().mode() & 0o777 == 0o600,
        "evidence file permissions must be 0600",
    )?;
    let readback_bytes = fs::read(path)?;
    ensure(
        readback_bytes == bytes,
        "evidence file byte readback must match write",
    )?;
    let _: Value = serde_json::from_slice(&readback_bytes)?;
    Ok(())
}

fn write_json_raw(path: impl AsRef<Path>, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn category_index(category: MutationCategory) -> usize {
    match category {
        MutationCategory::KnownGood => 0,
        MutationCategory::SubtleFlip => 1,
        MutationCategory::OffByOne => 2,
        MutationCategory::SwapVariable => 3,
        MutationCategory::DeleteTestCall => 4,
        MutationCategory::WrongFile => 5,
        MutationCategory::OverEngineer => 6,
        MutationCategory::CompileError => 7,
    }
}

fn category_slug(category: MutationCategory) -> &'static str {
    match category {
        MutationCategory::KnownGood => "known_good",
        MutationCategory::SubtleFlip => "subtle_flip",
        MutationCategory::OffByOne => "off_by_one",
        MutationCategory::SwapVariable => "swap_variable",
        MutationCategory::DeleteTestCall => "delete_test_call",
        MutationCategory::WrongFile => "wrong_file",
        MutationCategory::OverEngineer => "over_engineer",
        MutationCategory::CompileError => "compile_error",
    }
}

fn ensure(condition: bool, message: impl Into<String>) -> Result<(), Box<dyn std::error::Error>> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}
