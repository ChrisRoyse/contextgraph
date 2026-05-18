use candle_core::{Device, Tensor};
use context_graph_mejepa::HeadId;
use context_graph_mejepa_train::cert::{
    open_train_rocksdb, verify_chain, TrainCertWriter, TrainingCertificate, CF_MEJEPA_TRAIN_CERTS,
};
use context_graph_mejepa_train::learning_signal::{
    compute_l_step, DeltaKComponents, DeltaOmegaComponents, DeltaPAggregator, DeltaPComponents,
    DeltaXiComponents, LearningSignal,
};
use context_graph_mejepa_train::loss::LossBreakdown;
use context_graph_mejepa_train::{
    compose_bidirectional_l_full, inverse_map_loss, InverseMapOutputs, InverseMapTargets,
    TrainingConfig,
};
use rocksdb::{IteratorMode, DB};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-ek-012-bidirectional-corpus-fsv";

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
    let run_id = format!("run-{}", chrono::Utc::now().timestamp_millis());
    let run_dir = Path::new(FSV_ROOT).join(run_id);
    let db_dir = run_dir.join("train-certs-rocksdb");
    let output_path = run_dir.join("bidirectional_corpus_fsv.json");
    fs::create_dir_all(&run_dir)?;

    let config_default = TrainingConfig::default();
    let config_inverse = TrainingConfig {
        inverse_map_coefficient: 1.0,
        ..TrainingConfig::default()
    };
    config_inverse.validate()?;
    let invalid_config = TrainingConfig {
        inverse_map_coefficient: 1.25,
        ..TrainingConfig::default()
    }
    .validate()
    .expect_err("inverse coefficient >1 must fail");

    let device = Device::Cpu;
    let inverse = synthetic_inverse_loss(&device)?;
    let forward = LossBreakdown {
        total: 2.0,
        total_tensor: Tensor::new(2.0f32, &device)?,
        components: HashMap::from([("predict".to_string(), 2.0)]),
    };
    let lambda_zero = compose_bidirectional_l_full(&forward, Some(&inverse), 0.0)?;
    let lambda_one = compose_bidirectional_l_full(&forward, Some(&inverse), 1.0)?;
    let missing_inverse_error = compose_bidirectional_l_full(&forward, None, 0.25)
        .expect_err("positive coefficient without inverse outputs must fail");

    let db = open_train_rocksdb(&db_dir)?;
    let mut writer = TrainCertWriter::new(
        db.clone(),
        CF_MEJEPA_TRAIN_CERTS.to_string(),
        "task-ek-012-fsv".to_string(),
        "synthetic-bidirectional-corpus".to_string(),
        (1..=21)
            .map(|i| (format!("E{i}"), "task-ek-012-fsv".to_string()))
            .collect(),
        "2026-05-18T00:00:00Z".to_string(),
    )?;

    let mut baseline = training_cert(0, None)?;
    writer.emit(&mut baseline)?;
    let mut inverse_cert = training_cert(1, Some(inverse.quality_bits))?;
    inverse_cert
        .loss_components
        .insert("inverse_map".to_string(), inverse.loss);
    inverse_cert.loss_components.insert(
        "inverse_map_weighted".to_string(),
        inverse.loss * config_inverse.inverse_map_coefficient,
    );
    writer.emit(&mut inverse_cert)?;
    db.flush()?;

    let baseline_readback = read_cert(&db, 0)?;
    let inverse_readback = read_cert(&db, 1)?;
    let chain = verify_chain(&db, CF_MEJEPA_TRAIN_CERTS, 0, 1)?;
    let cert_count = count_cf(&db, CF_MEJEPA_TRAIN_CERTS)?;

    let mut historical_json = serde_json::to_value(&inverse_readback)?;
    historical_json
        .as_object_mut()
        .ok_or("cert JSON is not an object")?
        .remove("inverse_map_quality_bits");
    let historical: TrainingCertificate = serde_json::from_value(historical_json)?;
    historical.validate_phase3()?;

    let mut invalid_cert = inverse_readback.clone();
    invalid_cert.inverse_map_quality_bits = Some(-1.0);
    let invalid_cert_error = invalid_cert
        .validate_phase3()
        .expect_err("negative inverse bits must fail");

    let zero_exact = (lambda_zero.total - forward.total).abs() < 1e-6
        && !lambda_zero.components.contains_key("inverse_map");
    let one_composed = (lambda_one.total - (forward.total + inverse.loss)).abs() < 1e-6
        && lambda_one.components.contains_key("inverse_map_weighted");
    let readback_passes = baseline_readback.inverse_map_quality_bits.is_none()
        && inverse_readback.inverse_map_quality_bits == Some(inverse.quality_bits)
        && inverse_readback
            .loss_components
            .contains_key("inverse_map_weighted");
    let all_passed = config_default.inverse_map_coefficient == 0.0
        && invalid_config.code() == "MEJEPA_TRAIN_CONFIG_INVALID"
        && missing_inverse_error.code() == "MEJEPA_TRAIN_CONFIG_INVALID"
        && invalid_cert_error.code() == "MEJEPA_TRAIN_CERT_CHAIN_BROKEN"
        && inverse.quality_bits.is_finite()
        && inverse.quality_bits >= 0.0
        && zero_exact
        && one_composed
        && readback_passes
        && historical.inverse_map_quality_bits.is_none()
        && chain.verified == 2
        && chain.broken_at.is_none()
        && cert_count == 2;

    let evidence_without_hash = json!({
        "task": "TASK-EK-012",
        "issue": 334,
        "sourceOfTruth": {
            "artifact": output_path.display().to_string(),
            "trainCertCf": CF_MEJEPA_TRAIN_CERTS,
            "rocksdb": db_dir.display().to_string(),
            "implementation": [
                "crates/context-graph-mejepa-train/src/config.rs",
                "crates/context-graph-mejepa-train/src/cert/mod.rs",
                "crates/context-graph-mejepa-train/src/loss/inverse.rs",
                "crates/context-graph-mejepa-train/src/trainer/mod.rs"
            ]
        },
        "scope": "first implementation slice: config/schema/loss/cert readback for bidirectional training; real 300x8 four-variant ablation remains open",
        "config": {
            "default_inverse_map_coefficient": config_default.inverse_map_coefficient,
            "enabled_inverse_map_coefficient": config_inverse.inverse_map_coefficient,
            "invalidCoefficientError": invalid_config.code()
        },
        "inverseLoss": {
            "loss": inverse.loss,
            "inputPanelNllBits": inverse.input_panel_nll_bits,
            "actionNllBits": inverse.action_nll_bits,
            "qualityBits": inverse.quality_bits,
            "components": inverse.components
        },
        "composition": {
            "lambdaZeroTotal": lambda_zero.total,
            "lambdaOneTotal": lambda_one.total,
            "forwardTotal": forward.total,
            "missingInverseError": missing_inverse_error.code(),
            "zeroExact": zero_exact,
            "oneComposed": one_composed
        },
        "certReadback": {
            "baselineInverseBits": baseline_readback.inverse_map_quality_bits,
            "inverseBits": inverse_readback.inverse_map_quality_bits,
            "inverseLossComponent": inverse_readback.loss_components.get("inverse_map"),
            "certCount": cert_count,
            "chain": chain,
            "historicalMissingFieldCompat": historical.inverse_map_quality_bits.is_none(),
            "invalidCertError": invalid_cert_error.code(),
            "passes": readback_passes
        },
        "remainingIssueWork": [
            "wire real inverse-head outputs from the shared trunk instead of the synthetic cert harness",
            "run lambda in {0,0.25,0.5,1.0} on the real 300x8 Python corpus",
            "publish per-cell/global ablation report at /mnt/d/contextgraph/exports/ek-012/bidirectional-ablation-report.md",
            "apply per-cell regression-prevention gate to every ablation variant"
        ],
        "all_passed": all_passed
    });
    assert_eq!(evidence_without_hash["all_passed"], true);

    let payload_sha256 = sha256_json(&evidence_without_hash)?;
    let mut evidence = evidence_without_hash;
    evidence["payloadSha256WithoutHashField"] = json!(payload_sha256);
    write_json_0600(&output_path, &evidence)?;
    let readback: serde_json::Value = serde_json::from_slice(&fs::read(&output_path)?)?;
    assert_eq!(readback["all_passed"], true);
    assert_eq!(
        readback["payloadSha256WithoutHashField"],
        evidence["payloadSha256WithoutHashField"]
    );
    Ok(output_path)
}

fn synthetic_inverse_loss(
    device: &Device,
) -> Result<context_graph_mejepa_train::InverseMapLossBreakdown, Box<dyn std::error::Error>> {
    let outputs = InverseMapOutputs {
        predicted_input_panel: Tensor::from_slice(&[0f32, 1., 2., 3., 4., 5.], (2, 3), device)?,
        predicted_action: Tensor::from_slice(&[0.0f32, 1.0, 1.0, 0.0], (2, 2), device)?,
    };
    let targets = InverseMapTargets {
        input_panel: Tensor::from_slice(&[0f32, 1., 2.5, 3., 3.5, 5.], (2, 3), device)?,
        action: Tensor::from_slice(&[0.0f32, 1.0, 0.0, 1.0], (2, 2), device)?,
    };
    Ok(inverse_map_loss(&outputs, &targets)?)
}

fn training_cert(
    step: u64,
    inverse_map_quality_bits: Option<f64>,
) -> Result<TrainingCertificate, Box<dyn std::error::Error>> {
    let signal = signal(step)?;
    let per_head_l_step = HeadId::ALL
        .into_iter()
        .map(|head| (head.as_str().to_string(), signal.l_step))
        .collect();
    let delta_xi_global_min = signal.delta_xi;
    Ok(TrainingCertificate {
        step,
        epoch: 0,
        signal,
        per_head_l_step,
        delta_xi_global_min,
        loss_components: HashMap::from([("predict".to_string(), 0.1)]),
        conditional_description_length_bits: Some(
            context_graph_mejepa_train::conditional_description_length_bits_from_probability(0.7),
        ),
        inverse_map_quality_bits,
        training_mode: "task_ek_012_bidirectional_fsv".to_string(),
        distillation_cycle_id: None,
        distillation_loss_mean: None,
        distillation_skipped_count: None,
        counterfactual_smoothness: None,
        counterfactual_smoothness_anomaly: None,
        adversarial_mix_target_ratio: 0.20,
        adversarial_mix_count: 0,
        adversarial_mix_example_indices: Vec::new(),
        adversarial_mix_fallback_count: 0,
        cross_task_transfer_indices: vec![],
        cross_task_transfer_fallback_count: 0,
        predictor_redundancy_pairwise_mi: 0.01,
        predictor_redundancy_pairwise_mi_source: "task_ek_012_synthetic_signal".to_string(),
        holdout_promotion: None,
        generic_only_warning: None,
        phase3_dod_passed: None,
        grad_norm_running_mean: 0.0,
        parent_witness_hash: String::new(),
        self_hash: String::new(),
        merkle_root: String::new(),
        code_version: String::new(),
        corpus_sha: String::new(),
        embedder_versions: HashMap::new(),
        frozen_at: String::new(),
    })
}

fn signal(step: u64) -> Result<LearningSignal, Box<dyn std::error::Error>> {
    Ok(compute_l_step(
        DeltaPComponents {
            delta_p_real: 0.7 + step as f32 * 0.01,
            delta_p_imagined: None,
            snr: 1.0,
            exploration_bonus: 0.0,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Mean,
            per_chunk_values: vec![0.7 + step as f32 * 0.01],
        },
        DeltaKComponents::default(),
        DeltaOmegaComponents::default(),
        DeltaXiComponents {
            target_collapse: 0.0,
            predictor_redundancy: 0.01,
            constellation_violation_rate: 0.01,
        },
    )?)
}

fn read_cert(db: &DB, step: u64) -> Result<TrainingCertificate, Box<dyn std::error::Error>> {
    let cf = db
        .cf_handle(CF_MEJEPA_TRAIN_CERTS)
        .ok_or("missing CF_MEJEPA_TRAIN_CERTS")?;
    let bytes = db
        .get_cf(cf, step.to_be_bytes())?
        .ok_or_else(|| format!("missing training cert step {step}"))?;
    let cert: TrainingCertificate = serde_json::from_slice(&bytes)?;
    cert.validate_phase3()?;
    Ok(cert)
}

fn count_cf(db: &DB, cf_name: &str) -> Result<u64, rocksdb::Error> {
    let cf = db.cf_handle(cf_name).expect("cf");
    let mut count = 0;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        item?;
        count += 1;
    }
    Ok(count)
}

fn write_json_0600(
    path: &Path,
    value: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    assert_eq!(fs::metadata(path)?.permissions().mode() & 0o777, 0o600);
    let readback: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    assert_eq!(&readback, value);
    Ok(())
}

fn sha256_json(value: &serde_json::Value) -> Result<String, Box<dyn std::error::Error>> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
