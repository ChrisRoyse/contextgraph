use candle_core::{DType, Device, Tensor};
use context_graph_mejepa::eval::report::write_json_0600;
use context_graph_mejepa_train::cert::{open_train_rocksdb, verify_chain, CF_MEJEPA_TRAIN_CERTS};
use context_graph_mejepa_train::eval::holdout::{
    CalibrationDataset, HoldoutDataset, HoldoutExample, TrainSplit,
};
use context_graph_mejepa_train::eval::{Lang, MutationCategory};
use context_graph_mejepa_train::loss::entropy::{
    estimate_latent_entropy_nats, latent_entropy_loss, EntropyLambdaScheduler, LatentEntropyConfig,
    UTML_LATENT_ENTROPY_DEGENERATE,
};
use context_graph_mejepa_train::loss::{compose_l_full, AuxOutputs, BatchMetadata};
use context_graph_mejepa_train::trainer::{Trainer, TrainingDataset};
use context_graph_mejepa_train::{LossCoefficients, TrainingCertificate, TrainingConfig};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fsv_root = env_path(
        "CG_TASK_IP_001_ENTROPY_LOSS_FSV_ROOT",
        "/zfs/archive/contextgraph/fsv/task-ip-001-entropy-loss-fsv",
    );
    let run_dir = fsv_root.join(format!(
        "run-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
    ));
    fs::create_dir_all(&run_dir)?;

    let lambda_zero = lambda_zero_equivalence()?;
    let positive_loss = positive_entropy_loss()?;
    let monotonic = monotonic_entropy_window()?;
    let degenerate = degenerate_latent_case()?;
    let brier = brier_scheduler_case()?;
    let cert = training_certificate_case(&run_dir)?;

    let all_passed = lambda_zero["passed"] == true
        && positive_loss["passed"] == true
        && monotonic["passed"] == true
        && degenerate["passed"] == true
        && brier["passed"] == true
        && cert["passed"] == true;

    let report = json!({
        "artifactKind": "task_ip_001_entropy_loss_fsv",
        "task": "TASK-IP-001",
        "coveredGithubIssues": [511],
        "allPassed": all_passed,
        "sourceOfTruth": {
            "fsvRoot": run_dir,
            "trainCertCf": CF_MEJEPA_TRAIN_CERTS
        },
        "cases": {
            "lambdaZeroEquivalence": lambda_zero,
            "positiveEntropyLoss": positive_loss,
            "monotonicEntropyWindow": monotonic,
            "degenerateLatentFailsClosed": degenerate,
            "brierSchedulerNoRegression": brier,
            "trainCertReadback": cert
        },
        "activePathPolicy": {
            "mntDActiveReadsOrWrites": false,
            "flatRawPanelEntropyUsed": false,
            "predictionSurfaceAdded": false
        },
        "scopeBoundary": {
            "provesTrainingSignalAndFailClosedGuards": true,
            "claimsOracleCorrelationLift": false
        }
    });
    let report_path = run_dir.join("task_ip_001_entropy_loss_fsv.json");
    write_json_0600(&report_path, &report)?;
    let readback: Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    if readback["allPassed"] != true {
        return Err(format!("TASK-IP-001 FSV failed at {}", report_path.display()).into());
    }
    println!(
        "task-ip-001 entropy loss FSV allPassed=true report={} sha256={}",
        report_path.display(),
        sha256_file(&report_path)?
    );
    Ok(())
}

fn lambda_zero_equivalence() -> Result<Value, Box<dyn std::error::Error>> {
    let device = Device::Cpu;
    let latent = noncollinear_latent(&device, 1.0)?;
    let (tensor, report) = latent_entropy_loss(
        &latent,
        LatentEntropyConfig {
            lambda: 0.0,
            ..LatentEntropyConfig::default()
        },
    )?;
    let scalar = tensor.to_scalar::<f32>()?;
    let base = compose_loss_for_latent(&latent, 0.0)?;
    let repeated = compose_loss_for_latent(&latent, 0.0)?;
    Ok(json!({
        "passed": scalar == 0.0
            && !report.enabled
            && base.total == repeated.total
            && base.components.get("entropy_weighted") == Some(&0.0),
        "entropyTensorScalar": scalar,
        "composeTotal": base.total,
        "entropyComponent": base.components.get("entropy"),
        "entropyWeighted": base.components.get("entropy_weighted")
    }))
}

fn positive_entropy_loss() -> Result<Value, Box<dyn std::error::Error>> {
    let device = Device::Cpu;
    let latent = noncollinear_latent(&device, 1.0)?;
    let (tensor, report) = latent_entropy_loss(
        &latent,
        LatentEntropyConfig {
            lambda: 1.0e-4,
            ..LatentEntropyConfig::default()
        },
    )?;
    let loss = compose_loss_for_latent(&latent, 1.0e-4)?;
    Ok(json!({
        "passed": report.enabled
            && report.entropy_nats > 0.0
            && report.weighted_loss > 0.0
            && tensor.to_scalar::<f32>()? == report.entropy_nats
            && loss.components.get("entropy").copied().unwrap_or(0.0) > 0.0
            && loss.components.get("entropy_weighted").copied().unwrap_or(0.0) > 0.0,
        "report": report,
        "composeEntropy": loss.components.get("entropy"),
        "composeEntropyWeighted": loss.components.get("entropy_weighted")
    }))
}

fn monotonic_entropy_window() -> Result<Value, Box<dyn std::error::Error>> {
    let device = Device::Cpu;
    let wide = estimate_latent_entropy_nats(&noncollinear_latent(&device, 4.0)?, 3, 1e-6)?;
    let mid = estimate_latent_entropy_nats(&noncollinear_latent(&device, 2.0)?, 3, 1e-6)?;
    let tight = estimate_latent_entropy_nats(&noncollinear_latent(&device, 0.5)?, 3, 1e-6)?;
    Ok(json!({
        "passed": wide.entropy_nats > mid.entropy_nats && mid.entropy_nats > tight.entropy_nats,
        "wide": wide,
        "mid": mid,
        "tight": tight
    }))
}

fn degenerate_latent_case() -> Result<Value, Box<dyn std::error::Error>> {
    let device = Device::Cpu;
    let constant = Tensor::zeros((4, 2), DType::F32, &device)?;
    let err = estimate_latent_entropy_nats(&constant, 3, 1e-6).unwrap_err();
    Ok(json!({
        "passed": err.code() == UTML_LATENT_ENTROPY_DEGENERATE,
        "errorCode": err.code(),
        "error": err.to_string(),
        "context": err.context
    }))
}

fn brier_scheduler_case() -> Result<Value, Box<dyn std::error::Error>> {
    let scheduler = EntropyLambdaScheduler::default();
    let promoted = scheduler.decide(0.10, 0.09)?;
    let regressed = scheduler.decide(0.10, 0.11)?;
    Ok(json!({
        "passed": promoted.effective_lambda == scheduler.base_lambda
            && !promoted.regressed
            && regressed.effective_lambda == 0.0
            && regressed.regressed,
        "promoted": promoted,
        "regressed": regressed
    }))
}

fn training_certificate_case(run_dir: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let db_path = run_dir.join("entropy-train-certs.rocksdb");
    let db = open_train_rocksdb(&db_path)?;
    let cfg = TrainingConfig {
        epochs: 1,
        batch_size: 4,
        random_seed: 0x511511,
        cross_task_transfer_probability: 0.0,
        adversarial_mix_ratio: 0.0,
        loss_coefficients: LossCoefficients {
            lambda_entropy: 1.0e-4,
            ..LossCoefficients::default()
        },
        ..TrainingConfig::default()
    };
    let mut trainer = Trainer::new(cfg, db.clone(), Device::Cpu)?;
    let dataset = TrainingDataset {
        train: TrainSplit {
            examples: synthetic_examples(8)?,
        },
        calibration: CalibrationDataset { examples: vec![] },
        holdout: HoldoutDataset { examples: vec![] },
    };
    trainer.train_one_epoch(&dataset, 0)?;
    let cert_count = count_cf(&db, CF_MEJEPA_TRAIN_CERTS)?;
    let chain = verify_chain(&db, CF_MEJEPA_TRAIN_CERTS, 0, cert_count - 1)?;
    let first = read_cert(&db, 0)?;
    drop(trainer);
    drop(db);
    let reopened = open_train_rocksdb(&db_path)?;
    let readback = read_cert(&reopened, 0)?;
    let first_value = serde_json::to_value(&first)?;
    let readback_value = serde_json::to_value(&readback)?;
    Ok(json!({
        "passed": cert_count == 2
            && chain.verified == cert_count
            && chain.broken_at.is_none()
            && first_value == readback_value
            && readback.l_entropy_lambda == Some(1.0e-4)
            && readback.l_entropy_nats.unwrap_or(0.0) > 0.0
            && readback.l_entropy_weighted.unwrap_or(0.0) > 0.0
            && readback.loss_components.get("entropy").copied().unwrap_or(0.0) > 0.0
            && readback.loss_components.get("entropy_weighted").copied().unwrap_or(0.0) > 0.0,
        "dbPath": db_path,
        "certCount": cert_count,
        "chain": chain,
        "firstCertEntropy": {
            "lEntropyNats": readback.l_entropy_nats,
            "lEntropyWeighted": readback.l_entropy_weighted,
            "lEntropyLambda": readback.l_entropy_lambda,
            "lEntropySampleCount": readback.l_entropy_sample_count,
            "lEntropyEstimator": readback.l_entropy_estimator,
            "lossComponents": readback.loss_components
        }
    }))
}

fn compose_loss_for_latent(
    latent: &Tensor,
    lambda_entropy: f32,
) -> Result<context_graph_mejepa_train::loss::LossBreakdown, Box<dyn std::error::Error>> {
    let device = Device::Cpu;
    let batch = latent.dims()[0];
    let aux = AuxOutputs {
        warnings: Tensor::zeros((batch, 1), DType::F32, &device)?,
        runtime_log: Tensor::zeros((batch, 1), DType::F32, &device)?,
        rss: Tensor::zeros((batch, 1), DType::F32, &device)?,
        traceback_cluster_logits: Tensor::zeros((batch, 2), DType::F32, &device)?,
        hunk_verdicts: Tensor::zeros((batch, 1), DType::F32, &device)?,
        adjacent_outcomes: Tensor::zeros((batch, 1), DType::F32, &device)?,
        reasoning_embedding: latent.clone(),
        operator_match: Tensor::zeros((batch, 2), DType::F32, &device)?,
    };
    let meta = BatchMetadata {
        actual_warnings: aux.warnings.clone(),
        actual_runtime_log: aux.runtime_log.clone(),
        actual_rss: aux.rss.clone(),
        actual_traceback_cluster_id: Tensor::from_slice(&vec![0u32; batch], batch, &device)?,
        actual_hunk_verdicts: aux.hunk_verdicts.clone(),
        actual_adjacent_outcomes: aux.adjacent_outcomes.clone(),
        actual_cot_cluster_id: Tensor::from_slice(&vec![0u32; batch], batch, &device)?,
        override_flags: vec![false; batch],
        override_gold_labels: Tensor::zeros((batch, 2), DType::F32, &device)?,
        current_task_centroids: Some(vec![latent.clone()]),
    };
    Ok(compose_l_full(
        latent,
        latent,
        &aux,
        &meta,
        &LossCoefficients {
            lambda_entropy,
            ..LossCoefficients::default()
        },
    )?)
}

fn noncollinear_latent(device: &Device, scale: f32) -> Result<Tensor, candle_core::Error> {
    Tensor::from_slice(
        &[
            0.5f32 * scale,
            0.25 * scale,
            1.0 * scale,
            0.5 * scale,
            2.0 * scale,
            2.0 * scale,
            3.0 * scale,
            4.5 * scale,
        ],
        (4, 2),
        device,
    )
}

fn synthetic_examples(n: usize) -> Result<Vec<HoldoutExample>, candle_core::Error> {
    let mut examples = Vec::with_capacity(n);
    for i in 0..n {
        let x = i as f32;
        let y = ((i * i + 3) % 11) as f32;
        let panel = Tensor::from_slice(&[x, y], 2, &Device::Cpu)?;
        examples.push(HoldoutExample {
            task_id: format!("entropy-{i:03}"),
            category: MutationCategory::ALL[i % MutationCategory::ALL.len()],
            language: Lang::Python,
            panel_t01: panel.clone(),
            panel_t2: panel,
            inverse_action_target: None,
            actual_oracle_pass: i % 2 == 0,
            adversarial: false,
            foundationality_score: 0.0,
        });
    }
    Ok(examples)
}

fn read_cert(
    db: &rocksdb::DB,
    step: u64,
) -> Result<TrainingCertificate, Box<dyn std::error::Error>> {
    let cf = db
        .cf_handle(CF_MEJEPA_TRAIN_CERTS)
        .ok_or("missing train cert CF")?;
    let bytes = db
        .get_cf(cf, step.to_be_bytes())?
        .ok_or_else(|| format!("missing train cert at step {step}"))?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn count_cf(db: &rocksdb::DB, cf_name: &str) -> Result<u64, rocksdb::Error> {
    let cf = db.cf_handle(cf_name).expect("cf exists");
    let mut count = 0;
    for item in db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
        item?;
        count += 1;
    }
    Ok(count)
}

fn env_path(name: &str, default: &str) -> PathBuf {
    env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(hex::encode(Sha256::digest(fs::read(path)?)))
}
