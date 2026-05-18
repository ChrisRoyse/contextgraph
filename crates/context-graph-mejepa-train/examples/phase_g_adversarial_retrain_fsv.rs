#[path = "../../../tests/e2e/adversarial_corpus.rs"]
#[allow(dead_code)]
mod adversarial_corpus;

use anyhow::{anyhow, ensure, Context, Result};
use candle_core::{Device, Tensor};
use context_graph_mejepa::gates::valid_witness_segment;
use context_graph_mejepa::{
    build_deterministic_compiler, fixture_patch_context, open_infer_rocksdb, sha256_bytes, AstDiff,
    CalibrationExample, CalibrationStore, DiffHunk, Language, MeJepaInferConfig, PatchBundle,
    RocksDbInferStore, TaskContext, TaskEnvironment, TaskId, TestId, VerifyVerdict,
};
use context_graph_mejepa_train::cert::{open_train_rocksdb, CF_MEJEPA_TRAIN_CERTS};
use context_graph_mejepa_train::eval::holdout::{
    CalibrationDataset, HoldoutDataset, HoldoutExample, TrainSplit,
};
use context_graph_mejepa_train::eval::{Lang, MutationCategory};
use context_graph_mejepa_train::sampler::{BatchSampler, PatchSimilarityGraph};
use context_graph_mejepa_train::trainer::{Trainer, TrainingDataset};
use context_graph_mejepa_train::{TrainingCertificate, TrainingConfig};
use rocksdb::{IteratorMode, DB};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-g-adversarial-retrain-fsv";
const REPORT_NAME: &str = "phase_g_adversarial_retrain_fsv.json";
const TASK_ID: &str = "TASK-G-010";
const TRAIN_EXAMPLES: usize = 50;
const ADVERSARIAL_EXAMPLES: usize = 10;
const BATCH_SIZE: usize = 10;
const EXPECTED_ADVERSARIAL_PER_BATCH: usize = 2;

#[derive(Debug, Clone, PartialEq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FileArtifact {
    path: String,
    bytes: u64,
    sha256: String,
}

fn main() -> Result<()> {
    let fsv_root = PathBuf::from(FSV_ROOT);
    if fsv_root.exists() {
        fs::remove_dir_all(&fsv_root)
            .with_context(|| format!("remove stale FSV root {}", fsv_root.display()))?;
    }
    fs::create_dir_all(&fsv_root)?;

    let train_db_path = fsv_root.join("train-rocksdb");
    let dataset = synthetic_training_dataset()?;
    let config = TrainingConfig {
        batch_size: BATCH_SIZE,
        epochs: 1,
        random_seed: 106,
        adversarial_mix_ratio: 0.20,
        ..TrainingConfig::default()
    };

    let db = open_train_rocksdb(&train_db_path)?;
    let cert_count_before = count_train_certs(db.as_ref())?;
    let mut trainer = Trainer::new(config.clone(), db.clone(), Device::Cpu)?;
    let epoch_report = trainer.train_one_epoch(&dataset, 0)?;
    flush_train_certs(db.as_ref())?;
    let cert_count_after = count_train_certs(db.as_ref())?;
    drop(trainer);
    drop(db);

    let reopened = open_train_rocksdb(&train_db_path)?;
    let certs = read_train_certs(reopened.as_ref())?;
    let cert_mix_pass = certs.len() == TRAIN_EXAMPLES / BATCH_SIZE
        && certs.iter().all(|cert| {
            cert.adversarial_mix_target_ratio == 0.20
                && cert.adversarial_mix_count == EXPECTED_ADVERSARIAL_PER_BATCH
                && cert.adversarial_mix_fallback_count == 0
        })
        && adversarial_indices_seen(&certs) == (40..50).collect::<BTreeSet<_>>();

    let prediction_cases = run_synthetic_prediction_cases(&fsv_root)?;
    let corpus_report_root = fsv_root.join("adversarial-corpus-readback");
    let corpus_report = adversarial_corpus::run_adversarial_corpus_fsv(&corpus_report_root)?;
    let corpus_report_path = adversarial_corpus::default_report_path(&corpus_report_root);
    adversarial_corpus::write_report_with_readback(&corpus_report, &corpus_report_path)?;
    let adversarial_set_pass = corpus_report.aggregate.non_pass_rate >= 0.95
        && corpus_report.aggregate.high_confidence_false_passes == 0
        && corpus_report.readback_equal
        && corpus_report.all_passed;

    let boundary_cases = boundary_cases()?;
    let physical_artifacts = collect_file_artifacts(&fsv_root)?;
    let report_path = fsv_root.join(REPORT_NAME);
    let happy_path = vec![
        json!({
            "case": "trainer_persists_twenty_percent_adversarial_mix",
            "sot": CF_MEJEPA_TRAIN_CERTS,
            "before": {"train_cert_count": cert_count_before},
            "trigger": "Trainer::train_one_epoch with 10 adversarial examples in a 50-row train split",
            "expected": {
                "cert_count": 5,
                "adversarial_mix_target_ratio": 0.20,
                "adversarial_examples_per_batch": EXPECTED_ADVERSARIAL_PER_BATCH,
                "adversarial_indices_seen": (40..50).collect::<Vec<_>>()
            },
            "actual": {
                "cert_count_after": cert_count_after,
                "certs": certs.iter().map(cert_summary).collect::<Vec<_>>(),
                "epoch_total_steps": epoch_report.summary.total_steps_this_epoch
            },
            "pass": cert_mix_pass
        }),
        json!({
            "case": "adversarial_corpus_non_pass_rate_after_retrain_cycle",
            "sot": corpus_report_path.display().to_string(),
            "trigger": "TASK-TEST-010 adversarial corpus verify pass after adversarial-mix retrain cycle",
            "expected": {"non_pass_rate_min": 0.95, "high_confidence_false_passes": 0},
            "actual": {
                "total_cases": corpus_report.aggregate.total_cases,
                "non_pass_rate": corpus_report.aggregate.non_pass_rate,
                "high_confidence_false_passes": corpus_report.aggregate.high_confidence_false_passes
            },
            "pass": adversarial_set_pass
        }),
        prediction_cases,
    ];
    let all_passed = cert_mix_pass
        && adversarial_set_pass
        && happy_path
            .iter()
            .all(|case| case["pass"].as_bool() == Some(true))
        && boundary_cases
            .iter()
            .all(|case| case["pass"].as_bool() == Some(true))
        && !physical_artifacts.is_empty();
    let report = json!({
        "fsv_root": fsv_root.display().to_string(),
        "task_id": TASK_ID,
        "build_release_sha": git_head_short(),
        "source_of_truth": {
            "train_db_path": train_db_path.display().to_string(),
            "train_cert_cf": CF_MEJEPA_TRAIN_CERTS,
            "adversarial_fixture_root": "tests/fixtures/adversarial",
            "adversarial_corpus_report": corpus_report_path.display().to_string()
        },
        "happy_path": happy_path,
        "boundary_cases": boundary_cases,
        "cf_counts_before": {CF_MEJEPA_TRAIN_CERTS: cert_count_before},
        "cf_counts_after": {CF_MEJEPA_TRAIN_CERTS: cert_count_after},
        "physical_artifacts": physical_artifacts,
        "readback_equal": true,
        "all_passed": all_passed
    });
    write_json_readback(&report_path, &report)?;
    let readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(
        readback == report,
        "MEJEPA_PHASE_G_ADVERSARIAL_RETRAIN_FSV_READBACK_MISMATCH"
    );
    ensure!(all_passed, "MEJEPA_PHASE_G_ADVERSARIAL_RETRAIN_FSV_FAILED");
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn synthetic_training_dataset() -> Result<TrainingDataset> {
    let mut examples = Vec::with_capacity(TRAIN_EXAMPLES);
    for idx in 0..TRAIN_EXAMPLES {
        let adversarial = idx >= TRAIN_EXAMPLES - ADVERSARIAL_EXAMPLES;
        let panel = Tensor::from_slice(
            &[
                idx as f32 / TRAIN_EXAMPLES as f32,
                if adversarial { 1.0 } else { 0.0 },
            ],
            2,
            &Device::Cpu,
        )?;
        examples.push(HoldoutExample {
            task_id: if adversarial {
                format!("TASK-TEST-010-adversarial-{idx:03}")
            } else {
                format!("phase-g-gold-{idx:03}")
            },
            category: if adversarial {
                MutationCategory::SubtleFlip
            } else {
                MutationCategory::KnownGood
            },
            language: Lang::Python,
            panel_t01: panel.clone(),
            panel_t2: panel,
            actual_oracle_pass: !adversarial,
            adversarial,
            foundationality_score: 0.0,
        });
    }
    Ok(TrainingDataset {
        train: TrainSplit { examples },
        calibration: CalibrationDataset {
            examples: Vec::new(),
        },
        holdout: HoldoutDataset {
            examples: Vec::new(),
        },
    })
}

fn run_synthetic_prediction_cases(fsv_root: &Path) -> Result<Value> {
    let infer_db_path = fsv_root.join("infer-rocksdb");
    let repo_root = fsv_root.join("predictor-repo");
    let db = open_infer_rocksdb(&infer_db_path)?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    let examples = (0..40)
        .map(|idx| {
            let failing = idx % 10 == 0;
            CalibrationExample {
                language: Language::Python,
                predicted_test_pass: vec![if failing { 0.05 } else { 0.95 }],
                actual_test_pass: vec![if failing { 0.0 } else { 1.0 }],
            }
        })
        .collect::<Vec<_>>();
    calibration.calibrate(
        &examples,
        &[0.0; 40],
        0.10,
        30,
        0.30,
        [9; 32],
        BTreeMap::new(),
    )?;
    let store = Arc::new(RocksDbInferStore::new(db));
    let compiler = build_deterministic_compiler(
        repo_root.clone(),
        store,
        calibration,
        MeJepaInferConfig {
            outcome_set_max: 4,
            bootstrap_delta_omega: 1.0,
            bootstrap_delta_xi: 1.0,
            ..MeJepaInferConfig::default()
        },
    )?;
    let (adv_patch, adv_context) = fixture_patch_context(&repo_root, "predicted_failure")?;
    let adversarial = classify_verdict(compiler.verify(&adv_patch, &adv_context)?);
    let (gold_patch, gold_context) = gold_patch_context(&repo_root)?;
    let gold = classify_verdict(compiler.verify(&gold_patch, &gold_context)?);
    let adversarial_fail_closed = adversarial
        .final_classification
        .starts_with("PredictedFailure")
        || adversarial
            .final_classification
            .starts_with("OutOfDistribution")
        || matches!(
            adversarial.predicted_verdict.as_deref(),
            Some("Fail") | Some("OutOfDistribution")
        );
    let pass = adversarial.final_classification != "Pass"
        && adversarial_fail_closed
        && gold.final_classification == "Pass"
        && gold.predicted_verdict.as_deref() == Some("Pass");
    Ok(json!({
        "case": "synthetic_adversarial_and_gold_predictor_cases",
        "sot": infer_db_path.display().to_string(),
        "trigger": "MeJepaCompiler::verify on scenario: predicted_failure and scenario: approve",
        "expected": {
            "adversarial": "final classification PredictedFailure/OutOfDistribution or persisted RealityPrediction Fail/OOD",
            "gold": "final classification Pass with persisted RealityPrediction Pass"
        },
        "actual": {
            "adversarial": adversarial,
            "gold": gold
        },
        "pass": pass
    }))
}

fn gold_patch_context(repo_root: &Path) -> Result<(PatchBundle, TaskContext)> {
    fs::create_dir_all(repo_root.join("gold"))?;
    let mut hunks = Vec::new();
    for idx in 0..4 {
        let rel_path = PathBuf::from(format!("gold/gold_{idx}.py"));
        let content = format!("def stable_{idx}():\n    return {}\n", idx + 1);
        fs::write(repo_root.join(&rel_path), content.as_bytes())?;
        let post_sha = sha256_bytes(content.as_bytes());
        hunks.push(DiffHunk {
            path: rel_path,
            pre_sha: post_sha,
            post_sha,
            before: String::new(),
            after: content,
        });
    }
    let patch = PatchBundle::try_new(
        AstDiff { hunks },
        valid_witness_segment(),
        "Phase G adversarial retrain gold patch".to_string(),
        sha256_bytes(b"phase-g-adversarial-retrain-gold"),
    )?;
    let context = TaskContext {
        task_id: TaskId("phase-g-adversarial-retrain-gold".to_string()),
        session_id: [0x6a; 16],
        language: Language::Python,
        problem_statement: "scenario: approve; Phase G gold patch".to_string(),
        tests: vec![
            TestId("test_gold_0".to_string()),
            TestId("test_gold_1".to_string()),
            TestId("test_gold_2".to_string()),
            TestId("test_gold_3".to_string()),
        ],
        environment: TaskEnvironment {
            repo_root: repo_root.to_path_buf(),
            python_version: Some("3.11".to_string()),
            os: std::env::consts::OS.to_string(),
        },
        claim_graph: None,
        skill_citations: Vec::new(),
    };
    Ok((patch, context))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PredictionClassification {
    final_classification: String,
    predicted_verdict: Option<String>,
}

fn classify_verdict(verdict: VerifyVerdict) -> PredictionClassification {
    match verdict {
        VerifyVerdict::Approve {
            reality_prediction, ..
        } => PredictionClassification {
            final_classification: "Pass".to_string(),
            predicted_verdict: Some(format!("{:?}", reality_prediction.verdict)),
        },
        VerifyVerdict::EscalateToHuman {
            failed_gate,
            reality_prediction,
            ..
        } => PredictionClassification {
            final_classification: format!("{failed_gate:?}"),
            predicted_verdict: reality_prediction
                .map(|prediction| format!("{:?}", prediction.verdict)),
        },
    }
}

fn boundary_cases() -> Result<Vec<Value>> {
    let zero_mix = {
        let cfg = TrainingConfig {
            batch_size: 10,
            adversarial_mix_ratio: 0.0,
            random_seed: 106,
            ..TrainingConfig::default()
        };
        let mut sampler = sampler_with_adversarial(cfg, vec![8, 9])?;
        let plan = sampler.next_batch(None, 10)?;
        json!({
            "case": "zero_mix_ratio_disables_adversarial_reservation",
            "expected": {"adversarial_count": 0},
            "actual": {"adversarial_count": plan.adversarial_count, "indices": plan.indices},
            "pass": plan.adversarial_count == 0
        })
    };
    let duplicate_index = {
        let err = sampler_with_adversarial(TrainingConfig::default(), vec![1, 1])
            .expect_err("duplicate adversarial index must fail closed");
        json!({
            "case": "duplicate_adversarial_index_rejected",
            "expected_error_code": "MEJEPA_TRAIN_CONFIG_INVALID",
            "actual_error_code": err.code(),
            "pass": err.code() == "MEJEPA_TRAIN_CONFIG_INVALID"
                && err.to_string().contains("duplicate adversarial index")
        })
    };
    let out_of_range_index = {
        let err = sampler_with_adversarial(TrainingConfig::default(), vec![10])
            .expect_err("out-of-range adversarial index must fail closed");
        json!({
            "case": "out_of_range_adversarial_index_rejected",
            "expected_error_code": "MEJEPA_TRAIN_CONFIG_INVALID",
            "actual_error_code": err.code(),
            "pass": err.code() == "MEJEPA_TRAIN_CONFIG_INVALID"
                && err.to_string().contains("outside sampler length")
        })
    };
    let invalid_ratio = {
        let cfg = TrainingConfig {
            adversarial_mix_ratio: 1.01,
            ..TrainingConfig::default()
        };
        let err = cfg
            .validate()
            .expect_err("adversarial_mix_ratio > 1 must fail closed");
        json!({
            "case": "adversarial_mix_ratio_above_one_rejected",
            "expected_error_code": "MEJEPA_TRAIN_CONFIG_INVALID",
            "actual_error_code": err.code(),
            "pass": err.code() == "MEJEPA_TRAIN_CONFIG_INVALID"
                && err.to_string().contains("adversarial_mix_ratio")
        })
    };
    Ok(vec![
        zero_mix,
        duplicate_index,
        out_of_range_index,
        invalid_ratio,
    ])
}

fn sampler_with_adversarial(
    config: TrainingConfig,
    adversarial_indices: Vec<usize>,
) -> Result<BatchSampler, context_graph_mejepa_train::TrainerError> {
    BatchSampler::new_with_adversarial_indices(
        config,
        vec![0.2; 10],
        vec![false; 10],
        vec![0.0; 10],
        (0..10).map(|idx| format!("boundary-{idx}")).collect(),
        adversarial_indices,
        PatchSimilarityGraph {
            neighbors: vec![Vec::new(); 10],
        },
    )
}

fn cert_summary(cert: &TrainingCertificate) -> Value {
    json!({
        "step": cert.step,
        "adversarialMixTargetRatio": cert.adversarial_mix_target_ratio,
        "adversarialMixCount": cert.adversarial_mix_count,
        "adversarialMixExampleIndices": cert.adversarial_mix_example_indices,
        "adversarialMixFallbackCount": cert.adversarial_mix_fallback_count
    })
}

fn adversarial_indices_seen(certs: &[TrainingCertificate]) -> BTreeSet<usize> {
    certs
        .iter()
        .flat_map(|cert| cert.adversarial_mix_example_indices.iter().copied())
        .collect()
}

fn count_train_certs(db: &DB) -> Result<u64> {
    let cf = db
        .cf_handle(CF_MEJEPA_TRAIN_CERTS)
        .ok_or_else(|| anyhow!("missing {CF_MEJEPA_TRAIN_CERTS}"))?;
    let mut count = 0u64;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        item?;
        count += 1;
    }
    Ok(count)
}

fn read_train_certs(db: &DB) -> Result<Vec<TrainingCertificate>> {
    let cf = db
        .cf_handle(CF_MEJEPA_TRAIN_CERTS)
        .ok_or_else(|| anyhow!("missing {CF_MEJEPA_TRAIN_CERTS}"))?;
    let mut out = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        out.push(serde_json::from_slice(&value)?);
    }
    out.sort_by_key(|cert: &TrainingCertificate| cert.step);
    Ok(out)
}

fn flush_train_certs(db: &DB) -> Result<()> {
    let cf = db
        .cf_handle(CF_MEJEPA_TRAIN_CERTS)
        .ok_or_else(|| anyhow!("missing {CF_MEJEPA_TRAIN_CERTS}"))?;
    db.flush_cf(cf)?;
    Ok(())
}

fn collect_file_artifacts(root: &Path) -> Result<Vec<FileArtifact>> {
    let mut out = Vec::new();
    collect_file_artifacts_inner(root, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn collect_file_artifacts_inner(root: &Path, out: &mut Vec<FileArtifact>) -> Result<()> {
    for entry in fs::read_dir(root).with_context(|| format!("read {}", root.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect_file_artifacts_inner(&path, out)?;
            continue;
        }
        let bytes = fs::read(&path)?;
        out.push(FileArtifact {
            path: path.display().to_string(),
            bytes: bytes.len() as u64,
            sha256: hex::encode(Sha256::digest(&bytes)),
        });
    }
    Ok(())
}

fn write_json_readback(path: &Path, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    let readback = fs::read(path)?;
    ensure!(readback == bytes, "FSV report bytes changed on readback");
    Ok(())
}

fn git_head_short() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
