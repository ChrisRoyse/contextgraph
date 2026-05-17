//! 100,000-step synthetic cert chain replay FSV (NFR-TRAIN-02).

use context_graph_mejepa::HeadId;
use context_graph_mejepa_train::cert::{
    open_train_rocksdb, scan_target_collapse_invariant, verify_chain, TrainCertWriter,
    TrainingCertificate, CF_MEJEPA_TRAIN_CERTS,
};
use context_graph_mejepa_train::learning_signal::{
    compute_l_step, DeltaKComponents, DeltaOmegaComponents, DeltaPAggregator, DeltaPComponents,
    DeltaXiComponents,
};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

const FSV_DIR: &str = "/mnt/d/contextgraph/fsv/contextgraph-mejepa-train-fsv";
const DB_DIR: &str = "/mnt/d/contextgraph/fsv/contextgraph-mejepa-train-fsv/chain-rocksdb";
const STEPS: u64 = 100_000;

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
    let mut writer = TrainCertWriter::new(
        db.clone(),
        CF_MEJEPA_TRAIN_CERTS.to_string(),
        "phase3-fsv".to_string(),
        "chain-fsv-corpus".to_string(),
        (1..=21)
            .map(|i| (format!("E{i}"), "fsv".to_string()))
            .collect(),
        "2026-05-10T00:00:00Z".to_string(),
    )?;
    let setup_start = Instant::now();
    for step in 0..STEPS {
        let signal = compute_l_step(
            DeltaPComponents {
                delta_p_real: (step % 10) as f32 / 10.0,
                delta_p_imagined: None,
                snr: 1.0,
                exploration_bonus: 0.0,
                gamma: 0.7,
                aggregator: DeltaPAggregator::Mean,
                per_chunk_values: vec![(step % 10) as f32 / 10.0],
            },
            DeltaKComponents::default(),
            DeltaOmegaComponents::default(),
            DeltaXiComponents {
                target_collapse: 0.0,
                predictor_redundancy: 0.01,
                constellation_violation_rate: 0.01,
            },
        )?;
        let per_head_l_step = HeadId::ALL
            .into_iter()
            .map(|head| (head.as_str().to_string(), signal.l_step))
            .collect();
        let delta_xi_global_min = signal.delta_xi;
        let conditional_description_length_bits =
            context_graph_mejepa_train::conditional_description_length_bits_from_probability(
                signal.delta_p,
            );
        let mut cert = TrainingCertificate {
            step,
            epoch: 0,
            signal,
            per_head_l_step,
            delta_xi_global_min,
            loss_components: HashMap::from([("predict".to_string(), 0.1)]),
            conditional_description_length_bits: Some(conditional_description_length_bits),
            training_mode: "fsv_chain_stress".to_string(),
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
            predictor_redundancy_pairwise_mi_source: "synthetic_cert_signal".to_string(),
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
        };
        writer.emit(&mut cert)?;
    }
    db.flush()?;
    let verify_start = Instant::now();
    let report = verify_chain(&db, CF_MEJEPA_TRAIN_CERTS, 0, STEPS - 1)?;
    let replay_seconds = verify_start.elapsed().as_secs_f64();
    let violations = scan_target_collapse_invariant(&db, CF_MEJEPA_TRAIN_CERTS, 0, STEPS - 1)?;
    assert_eq!(report.verified, STEPS);
    assert!(report.broken_at.is_none());
    assert!(replay_seconds < 60.0, "replay took {replay_seconds}s");
    assert!(violations.is_empty());
    let evidence = json!({
        "source_of_truth": DB_DIR,
        "steps": STEPS,
        "setup_seconds": setup_start.elapsed().as_secs_f64(),
        "replay_seconds": replay_seconds,
        "chain_verification": report,
        "target_collapse_violations": violations
    });
    write_0600(
        Path::new(FSV_DIR).join("training-cert-chain.json"),
        &evidence,
    )?;
    Ok(())
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
