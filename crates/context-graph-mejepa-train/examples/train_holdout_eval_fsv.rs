//! 240-example holdout eval FSV with synthetic per-category / per-language distribution.

use candle_core::{Device, Tensor};
use context_graph_mejepa_train::cert::{open_train_rocksdb, CF_MEJEPA_HOLDOUT_REPORTS};
use context_graph_mejepa_train::eval::holdout::{
    CalibrationDataset, HoldoutDataset, HoldoutExample, OracleClass, OracleHead, OraclePrediction,
    PredictorForward,
};
use context_graph_mejepa_train::eval::regression::promote_best;
use context_graph_mejepa_train::eval::{HoldoutEvaluator, Lang, MutationCategory};
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::ExitCode;

const FSV_DIR: &str = "/mnt/d/contextgraph/fsv/contextgraph-mejepa-train-fsv";
const DB_DIR: &str = "/mnt/d/contextgraph/fsv/contextgraph-mejepa-train-fsv/holdout-rocksdb";

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
    let _ = fs::remove_dir_all(DB_DIR);
    fs::create_dir_all(FSV_DIR)?;
    let db = open_train_rocksdb(DB_DIR)?;
    let evaluator = HoldoutEvaluator::new(db.clone(), CF_MEJEPA_HOLDOUT_REPORTS.to_string());
    let dataset = HoldoutDataset {
        examples: synthetic_examples(240)?,
    };
    let calibration = CalibrationDataset {
        examples: synthetic_examples(24)?,
    };
    let report = evaluator.evaluate(
        &EchoPredictor,
        &SkewedOracle,
        &dataset,
        &calibration,
        1000,
        true,
        0.75,
    )?;
    let stored = db
        .get_cf(
            db.cf_handle(CF_MEJEPA_HOLDOUT_REPORTS).expect("cf"),
            1000u64.to_be_bytes(),
        )?
        .expect("holdout report persisted");
    let stored_report: context_graph_mejepa_train::eval::HoldoutReport =
        serde_json::from_slice(&stored)?;
    assert_eq!(stored_report.step, report.step);
    assert_eq!(report.per_mutation_category_accuracy.len(), 8);
    assert_eq!(report.per_language_accuracy.len(), 11);
    assert!(report.generic_only_warning.is_some());
    assert_eq!(report.phase3_dod_passed, Some(true));
    let best = promote_best(
        Path::new(FSV_DIR),
        1000,
        report.prediction_oracle_agreement,
        0.0,
        0.005,
    )?;
    let evidence = json!({
        "source_of_truth": DB_DIR,
        "cf": CF_MEJEPA_HOLDOUT_REPORTS,
        "stored_report_step": stored_report.step,
        "agreement": report.prediction_oracle_agreement,
        "per_category_keys": report.per_mutation_category_accuracy.len(),
        "per_language_keys": report.per_language_accuracy.len(),
        "generic_only_warning": report.generic_only_warning,
        "best_safetensors_path": best
    });
    write_0600(
        Path::new(FSV_DIR).join("holdout-eval-evidence.json"),
        &evidence,
    )?;
    Ok(())
}

struct EchoPredictor;
impl PredictorForward for EchoPredictor {
    fn forward(
        &self,
        panel_t01: &Tensor,
    ) -> Result<Tensor, context_graph_mejepa_train::TrainerError> {
        Ok(panel_t01.clone())
    }
}

struct SkewedOracle;
impl OracleHead for SkewedOracle {
    fn predict(
        &self,
        predicted_latent: &Tensor,
    ) -> Result<OraclePrediction, context_graph_mejepa_train::TrainerError> {
        let vals = predicted_latent
            .to_dtype(candle_core::DType::F32)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        let idx = vals[0] as usize;
        let actual = vals[1] >= 0.5;
        let compile_error_slice = idx % MutationCategory::ALL.len() == 7;
        let predicted = if compile_error_slice { !actual } else { actual };
        Ok(OraclePrediction {
            pass_to_pass_resolved: predicted,
            conformal_set: vec![OracleClass::Pass, OracleClass::Fail],
            class_logits: if predicted {
                vec![0.0, 3.0]
            } else {
                vec![3.0, 0.0]
            },
        })
    }
}

fn synthetic_examples(n: usize) -> Result<Vec<HoldoutExample>, candle_core::Error> {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let panel = Tensor::from_slice(&[i as f32, (i % 2) as f32], 2, &Device::Cpu)?;
        out.push(HoldoutExample {
            task_id: format!("holdout-{i:03}"),
            category: MutationCategory::ALL[i % MutationCategory::ALL.len()],
            language: Lang::ALL[i % Lang::ALL.len()],
            panel_t01: panel.clone(),
            panel_t2: panel,
            inverse_action_target: None,
            actual_oracle_pass: i % 2 == 1,
            adversarial: false,
            foundationality_score: 0.0,
        });
    }
    Ok(out)
}

fn write_0600(
    path: impl AsRef<Path>,
    value: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.as_ref();
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    assert_eq!(fs::metadata(path)?.permissions().mode() & 0o777, 0o600);
    let reread: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    assert_eq!(&reread, value);
    Ok(())
}
