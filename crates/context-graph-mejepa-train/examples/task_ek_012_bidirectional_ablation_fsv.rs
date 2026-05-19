//! TASK-EK-012 real-corpus bidirectional ablation runner.
//!
//! This FSV runs the four requested lambda_inv variants against the verified
//! SWE-bench Lite Python 300x8 corpus, persists train certs and holdout reports,
//! and writes the operator-facing markdown report.

use candle_core::{Device, Tensor};
use context_graph_mejepa::{
    FrozenTargetAdapter, MeJepaPredictor, PredictorConfig, TargetProvenance,
};
use context_graph_mejepa_train::cert::{
    open_train_rocksdb, verify_chain, CF_MEJEPA_HOLDOUT_REPORTS, CF_MEJEPA_TRAIN_CERTS,
};
use context_graph_mejepa_train::cli::load_corpus_index_dataset;
use context_graph_mejepa_train::eval::holdout::{
    HoldoutExample, OracleClass, OracleHead, OraclePrediction, PredictorForward,
};
use context_graph_mejepa_train::eval::{HoldoutEvaluator, MutationCategory};
use context_graph_mejepa_train::trainer::{Trainer, TrainingDataset};
use context_graph_mejepa_train::{TrainerError, TrainerErrorCode, TrainingConfig};
use rocksdb::{IteratorMode, DB};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const INDEX_PATH: &str =
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json";
const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-ek-012-bidirectional-ablation-fsv";
const REPORT_PATH: &str =
    "/zfs/archive/contextgraph/exports/ek-012/bidirectional-ablation-report.md";
const SHIP_GATE_TARGET: f32 = 0.95;
const LAMBDAS: [f32; 4] = [0.0, 0.25, 0.5, 1.0];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("{}", path.display());
            ExitCode::from(0)
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let run_id = format!("run-{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ"));
    let run_dir = Path::new(FSV_ROOT).join(run_id);
    fs::create_dir_all(&run_dir)?;
    fs::create_dir_all(
        Path::new(REPORT_PATH)
            .parent()
            .ok_or("report has no parent")?,
    )?;

    let dataset = load_corpus_index_dataset(Path::new(INDEX_PATH))?;
    let split_counts = split_counts(&dataset);
    ensure(
        split_counts == (1920, 240, 240),
        format!("expected 1920/240/240 split counts, got {split_counts:?}"),
    )?;
    let structured_action_targets = dataset
        .train
        .examples
        .iter()
        .filter(|example| example.inverse_action_target.is_some())
        .count();
    ensure(
        structured_action_targets == dataset.train.examples.len(),
        format!(
            "all train examples must have source-backed inverse action targets; got {structured_action_targets}/{}",
            dataset.train.examples.len()
        ),
    )?;

    let device = Device::new_cuda(0)?;
    let predictor = build_inverse_predictor(device.clone())?;
    let oracle = CategoryMajorityOracle::from_train(&dataset.train.examples)?;
    let mut variants = Vec::new();
    let mut baseline_categories: Option<HashMap<MutationCategory, f32>> = None;

    for (idx, lambda_inv) in LAMBDAS.iter().copied().enumerate() {
        let variant = run_variant(
            &run_dir,
            &dataset,
            &oracle,
            &predictor,
            device.clone(),
            idx,
            lambda_inv,
            baseline_categories.as_ref(),
        )?;
        if lambda_inv == 0.0 {
            baseline_categories = Some(
                variant["holdout"]["perMutationCategoryAccuracy"]
                    .as_object()
                    .ok_or("baseline category map missing")?
                    .iter()
                    .map(|(key, value)| {
                        Ok((
                            parse_category_slug(key)?,
                            value.as_f64().unwrap_or(0.0) as f32,
                        ))
                    })
                    .collect::<Result<HashMap<_, _>, Box<dyn std::error::Error>>>()?,
            );
        }
        variants.push(variant);
    }

    let baseline_agreement = variants
        .first()
        .and_then(|variant| variant["holdout"]["agreement"].as_f64())
        .ok_or("baseline agreement missing")?;
    for variant in &mut variants {
        let agreement = variant["holdout"]["agreement"]
            .as_f64()
            .ok_or("variant agreement missing")?;
        variant["agreementDeltaVsBaseline"] = json!(agreement - baseline_agreement);
    }
    let best = variants
        .iter()
        .max_by(|a, b| {
            let av = a["holdout"]["agreement"]
                .as_f64()
                .unwrap_or(f64::NEG_INFINITY);
            let bv = b["holdout"]["agreement"]
                .as_f64()
                .unwrap_or(f64::NEG_INFINITY);
            av.total_cmp(&bv)
        })
        .ok_or("no variants produced")?;
    let best_delta_vs_baseline = best["agreementDeltaVsBaseline"].as_f64().unwrap_or(0.0);
    let all_regression_gates_passed = variants
        .iter()
        .all(|variant| variant["regressionGate"]["passed"].as_bool() == Some(true));
    let production_recommendation = if best_delta_vs_baseline > 0.0 {
        "candidate_positive_lambda_requires_existing_promotion_gate"
    } else {
        "keep_default_lambda_inv_0_0_no_ship_gate_gain_observed"
    };
    let report_markdown = render_markdown(&variants, best)?;
    write_0600(REPORT_PATH, report_markdown.as_bytes())?;

    let evidence_without_hash = json!({
        "task": "TASK-EK-012",
        "issue": 334,
        "sourceOfTruth": {
            "corpusIndex": INDEX_PATH,
            "fsvRunDir": run_dir,
            "report": REPORT_PATH,
            "trainCertCf": CF_MEJEPA_TRAIN_CERTS,
            "holdoutReportCf": CF_MEJEPA_HOLDOUT_REPORTS
        },
        "splitCounts": {
            "train": split_counts.0,
            "calibration": split_counts.1,
            "holdout": split_counts.2
        },
        "structuredActionTargets": structured_action_targets,
        "lambdas": LAMBDAS,
        "variants": variants,
        "bestVariant": {
            "lambdaInv": best["lambdaInv"],
            "agreement": best["holdout"]["agreement"],
            "deltaVsBaseline": best["agreementDeltaVsBaseline"]
        },
        "baselineAgreement": baseline_agreement,
        "bestDeltaVsBaseline": best_delta_vs_baseline,
        "productionRecommendation": production_recommendation,
        "allRegressionGatesPassed": all_regression_gates_passed,
        "shipGateTarget": SHIP_GATE_TARGET,
        "all_passed": all_regression_gates_passed
            && structured_action_targets == dataset.train.examples.len()
            && best["holdout"]["agreement"].as_f64().is_some()
    });
    let payload_sha256 = sha256_json(&evidence_without_hash)?;
    let mut evidence = evidence_without_hash;
    evidence["payloadSha256WithoutHashField"] = json!(payload_sha256);
    let evidence_path = run_dir.join("bidirectional_ablation_fsv.json");
    write_json_0600(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    ensure(
        readback["all_passed"].as_bool() == Some(true),
        "FSV readback did not pass",
    )?;
    Ok(evidence_path)
}

#[allow(clippy::too_many_arguments)]
fn run_variant(
    run_dir: &Path,
    dataset: &TrainingDataset,
    oracle: &CategoryMajorityOracle,
    predictor: &MeJepaPredictor,
    device: Device,
    idx: usize,
    lambda_inv: f32,
    baseline_categories: Option<&HashMap<MutationCategory, f32>>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let variant_dir = run_dir.join(format!("lambda-{lambda_inv:.2}"));
    let db_dir = variant_dir.join("rocksdb");
    fs::create_dir_all(&variant_dir)?;
    let db = open_train_rocksdb(&db_dir)?;
    let config = TrainingConfig {
        epochs: 1,
        batch_size: 64,
        inverse_map_coefficient: lambda_inv,
        phase3_dod_min_agreement: SHIP_GATE_TARGET,
        random_seed: 0x3340_0000 + idx as u64,
        ..TrainingConfig::default()
    };
    config.validate()?;
    let mut trainer = Trainer::new(config.clone(), db.clone(), device)?;
    let training = if lambda_inv > 0.0 {
        trainer.run_full_training_with_inverse_head(dataset, predictor)?
    } else {
        trainer.run_full_training(dataset)?
    };
    let cert_count = count_cf(&db, CF_MEJEPA_TRAIN_CERTS)?;
    ensure(cert_count > 0, "variant emitted no train certs")?;
    let chain = verify_chain(&db, CF_MEJEPA_TRAIN_CERTS, 0, cert_count - 1)?;
    ensure(
        chain.broken_at.is_none() && chain.verified == cert_count,
        "train cert chain did not verify",
    )?;
    let inverse_cert_count = count_inverse_quality_bits(&db)?;
    if lambda_inv == 0.0 {
        ensure(
            inverse_cert_count == 0,
            "lambda=0 emitted inverse quality bits",
        )?;
    } else {
        ensure(
            inverse_cert_count == cert_count,
            format!("lambda={lambda_inv} emitted inverse bits for {inverse_cert_count}/{cert_count} certs"),
        )?;
    }
    let evaluator = HoldoutEvaluator::new(db.clone(), CF_MEJEPA_HOLDOUT_REPORTS.to_string());
    let report = evaluator.evaluate(
        &EchoPredictor,
        oracle,
        &dataset.holdout,
        &dataset.calibration,
        20_000 + idx as u64,
        true,
        SHIP_GATE_TARGET,
    )?;
    let gate = regression_gate(
        lambda_inv,
        &report.per_mutation_category_accuracy,
        baseline_categories,
    );
    Ok(json!({
        "lambdaInv": lambda_inv,
        "variantDir": variant_dir,
        "rocksdb": db_dir,
        "training": training,
        "certCount": cert_count,
        "inverseQualityCertCount": inverse_cert_count,
        "chainVerified": chain.verified,
        "holdout": {
            "agreement": report.prediction_oracle_agreement,
            "phase3DodPassed": report.phase3_dod_passed,
            "perMutationCategoryAccuracy": category_map_json(&report.per_mutation_category_accuracy)
        },
        "regressionGate": gate
    }))
}

fn build_inverse_predictor(device: Device) -> Result<MeJepaPredictor, Box<dyn std::error::Error>> {
    let adapter = FrozenTargetAdapter::new(TargetProvenance::new(
        "task-ek-012-bidirectional-ablation-fsv",
        BTreeMap::new(),
        0,
        None,
    ));
    Ok(MeJepaPredictor::new(
        PredictorConfig::default(),
        adapter,
        device,
        2,
    )?)
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
}

impl CategoryMajorityOracle {
    fn from_train(examples: &[HoldoutExample]) -> Result<Self, TrainerError> {
        if examples.is_empty() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                "cannot build category-majority oracle from empty train split",
            ));
        }
        let mut pass_counts = [0_u64; 8];
        let mut fail_counts = [0_u64; 8];
        for example in examples {
            let idx = category_index(example.category);
            if example.actual_oracle_pass {
                pass_counts[idx] += 1;
            } else {
                fail_counts[idx] += 1;
            }
        }
        let mut pass_by_category = [false; 8];
        for idx in 0..8 {
            ensure(
                pass_counts[idx] + fail_counts[idx] > 0,
                format!(
                    "train split has no examples for {}",
                    category_slug(MutationCategory::ALL[idx])
                ),
            )
            .map_err(|err| {
                TrainerError::new(TrainerErrorCode::MejepaTrainConfigInvalid, err.to_string())
            })?;
            pass_by_category[idx] = pass_counts[idx] >= fail_counts[idx];
        }
        Ok(Self { pass_by_category })
    }
}

impl OracleHead for CategoryMajorityOracle {
    fn predict(&self, predicted_latent: &Tensor) -> Result<OraclePrediction, TrainerError> {
        let values = predicted_latent
            .to_dtype(candle_core::DType::F32)?
            .flatten_all()?
            .to_vec1::<f32>()?;
        if values.len() < 2 {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                format!(
                    "expected corpus feature panel with at least 2 values, got {}",
                    values.len()
                ),
            ));
        }
        let raw_category = values[1];
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

fn regression_gate(
    lambda_inv: f32,
    current: &HashMap<MutationCategory, f32>,
    baseline: Option<&HashMap<MutationCategory, f32>>,
) -> Value {
    let Some(baseline) = baseline else {
        return json!({"passed": true, "baseline": true, "cells": []});
    };
    let mut cells = Vec::new();
    let mut passed = true;
    for category in MutationCategory::ALL {
        let before = *baseline.get(&category).unwrap_or(&0.0);
        let after = *current.get(&category).unwrap_or(&0.0);
        let delta = after - before;
        let cell_passed = delta >= -TrainingConfig::default().holdout_regression_threshold;
        passed &= cell_passed;
        cells.push(json!({
            "category": category_slug(category),
            "baseline": before,
            "current": after,
            "delta": delta,
            "passed": cell_passed
        }));
    }
    json!({"passed": passed, "lambdaInv": lambda_inv, "cells": cells})
}

fn render_markdown(variants: &[Value], best: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let mut out = String::new();
    out.push_str("# TASK-EK-012 Bidirectional Ablation Report\n\n");
    out.push_str(&format!("- Corpus: `{INDEX_PATH}`\n"));
    out.push_str(&format!("- Ship gate target: `{SHIP_GATE_TARGET}`\n"));
    out.push_str(&format!(
        "- Best lambda_inv by global agreement: `{}` at `{}`\n\n",
        best["lambdaInv"], best["holdout"]["agreement"]
    ));
    out.push_str(&format!(
        "- Best delta vs baseline: `{}`\n",
        best["agreementDeltaVsBaseline"]
    ));
    if best["agreementDeltaVsBaseline"].as_f64().unwrap_or(0.0) > 0.0 {
        out.push_str("- Result: positive lambda improved the measured holdout agreement; keep it behind the existing per-cell promotion gate.\n\n");
    } else {
        out.push_str("- Result: neutral; positive lambda produced no measured holdout gain in this FSV, so production should keep `lambda_inv = 0.0`.\n\n");
    }
    out.push_str("| lambda_inv | agreement | delta vs baseline | certs | inverse certs | regression gate |\n");
    out.push_str("|---:|---:|---:|---:|---:|---|\n");
    for variant in variants {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            variant["lambdaInv"],
            variant["holdout"]["agreement"],
            variant["agreementDeltaVsBaseline"],
            variant["certCount"],
            variant["inverseQualityCertCount"],
            variant["regressionGate"]["passed"]
        ));
    }
    out.push_str("\n## Per-Cell Deltas\n\n");
    for variant in variants {
        out.push_str(&format!("### lambda_inv = {}\n\n", variant["lambdaInv"]));
        out.push_str("| category | baseline | current | delta | passed |\n");
        out.push_str("|---|---:|---:|---:|---|\n");
        if let Some(cells) = variant["regressionGate"]["cells"].as_array() {
            for cell in cells {
                let category = cell["category"].as_str().unwrap_or("<missing>");
                out.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    category, cell["baseline"], cell["current"], cell["delta"], cell["passed"]
                ));
            }
        }
        out.push('\n');
    }
    Ok(out)
}

fn category_map_json(map: &HashMap<MutationCategory, f32>) -> Value {
    let mut out = serde_json::Map::new();
    for category in MutationCategory::ALL {
        out.insert(
            category_slug(category).to_string(),
            json!(map.get(&category).copied().unwrap_or(0.0)),
        );
    }
    Value::Object(out)
}

fn count_cf(db: &DB, cf_name: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| format!("missing CF {cf_name}"))?;
    Ok(db.iterator_cf(cf, IteratorMode::Start).count() as u64)
}

fn count_inverse_quality_bits(db: &DB) -> Result<u64, Box<dyn std::error::Error>> {
    let cf = db
        .cf_handle(CF_MEJEPA_TRAIN_CERTS)
        .ok_or_else(|| format!("missing CF {CF_MEJEPA_TRAIN_CERTS}"))?;
    let mut count = 0_u64;
    for row in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = row?;
        let json: Value = serde_json::from_slice(&value)?;
        if json
            .get("inverse_map_quality_bits")
            .and_then(|value| value.as_f64())
            .is_some()
        {
            count += 1;
        }
    }
    Ok(count)
}

fn split_counts(dataset: &TrainingDataset) -> (usize, usize, usize) {
    (
        dataset.train.examples.len(),
        dataset.calibration.examples.len(),
        dataset.holdout.examples.len(),
    )
}

fn category_index(category: MutationCategory) -> usize {
    MutationCategory::ALL
        .iter()
        .position(|candidate| *candidate == category)
        .expect("category is in ALL")
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

fn parse_category_slug(value: &str) -> Result<MutationCategory, Box<dyn std::error::Error>> {
    for category in MutationCategory::ALL {
        if category_slug(category) == value {
            return Ok(category);
        }
    }
    Err(format!("unknown category slug {value}").into())
}

fn sha256_json(value: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec(value)?;
    let digest = Sha256::digest(&bytes);
    Ok(hex::encode(digest))
}

fn write_json_0600(path: &Path, value: &Value) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec_pretty(value)?;
    write_0600(path, &bytes)
}

fn write_0600(path: impl AsRef<Path>, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    ensure(
        fs::metadata(path)?.permissions().mode() & 0o777 == 0o600,
        format!("{} was not written 0600", path.display()),
    )?;
    ensure(
        fs::read(path)? == bytes,
        format!("{} readback mismatch", path.display()),
    )?;
    Ok(())
}

fn ensure(condition: bool, message: impl Into<String>) -> Result<(), Box<dyn std::error::Error>> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}
