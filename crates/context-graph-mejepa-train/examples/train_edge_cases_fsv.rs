//! Manual edge-case FSV: before/after physical state for fail-closed paths.

use context_graph_mejepa_train::cert::{
    open_train_rocksdb, verify_chain, TrainCertWriter, TrainingCertificate,
    CF_MEJEPA_HOLDOUT_REPORTS, CF_MEJEPA_TRAIN_CERTS,
};
use context_graph_mejepa_train::eval::holdout::{CalibrationDataset, HoldoutDataset};
use context_graph_mejepa_train::eval::HoldoutEvaluator;
use context_graph_mejepa_train::learning_signal::{
    compute_l_step, DeltaKComponents, DeltaOmegaComponents, DeltaPComponents, DeltaXiComponents,
};
use context_graph_mejepa_train::TrainerErrorCode;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::ExitCode;

const FSV_DIR: &str = "/tmp/contextgraph-mejepa-train-fsv";
const DB_DIR: &str = "/tmp/contextgraph-mejepa-train-fsv/edge-rocksdb";

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
        "edge-fsv".to_string(),
        "edge-corpus".to_string(),
        (1..=21)
            .map(|i| (format!("E{i}"), "edge".to_string()))
            .collect(),
        "2026-05-10T00:00:00Z".to_string(),
    )?;

    let before_leak = count_cf(&db, CF_MEJEPA_TRAIN_CERTS)?;
    let leak_err = compute_l_step(
        DeltaPComponents {
            oracle_disagreement: 0.1,
            prediction_residual: 0.1,
            uncertainty: 0.1,
        },
        DeltaKComponents::default(),
        DeltaOmegaComponents::default(),
        DeltaXiComponents {
            target_collapse: 0.5,
            predictor_collapse: 0.0,
            redundancy_mi: 0.0,
        },
    )
    .expect_err("target collapse must fail before cert construction");
    let after_leak = count_cf(&db, CF_MEJEPA_TRAIN_CERTS)?;

    let before_chain_count = after_leak;
    for step in 0..3 {
        let mut ok = cert(step, 0.0)?;
        writer.emit(&mut ok)?;
    }
    let before_tamper = verify_chain(&db, CF_MEJEPA_TRAIN_CERTS, 0, 2)?;
    let cf = db.cf_handle(CF_MEJEPA_TRAIN_CERTS).expect("cf");
    let mut tampered: serde_json::Value =
        serde_json::from_slice(&db.get_cf(cf, 1u64.to_be_bytes())?.expect("step 1"))?;
    tampered["parent_witness_hash"] = json!("bad-parent");
    db.put_cf(cf, 1u64.to_be_bytes(), serde_json::to_vec(&tampered)?)?;
    let after_tamper = verify_chain(&db, CF_MEJEPA_TRAIN_CERTS, 0, 2)?;

    let before_holdout = count_cf(&db, CF_MEJEPA_HOLDOUT_REPORTS)?;
    let evaluator = HoldoutEvaluator::new(db.clone(), CF_MEJEPA_HOLDOUT_REPORTS.to_string());
    let empty_err = evaluator
        .evaluate(
            &NoPredictor,
            &NoOracle,
            &HoldoutDataset { examples: vec![] },
            &CalibrationDataset { examples: vec![] },
            7,
            false,
            0.75,
        )
        .expect_err("empty holdout must fail");
    let after_holdout = count_cf(&db, CF_MEJEPA_HOLDOUT_REPORTS)?;

    let evidence = json!({
        "source_of_truth": DB_DIR,
        "edge_cases": [
            {
                "name": "target_collapse_rejected_before_write",
                "before": {"train_cert_count": before_leak},
                "action": "compute_l_step with target_collapse=0.5",
                "error_code": leak_err.code(),
                "after": {"train_cert_count": after_leak}
            },
            {
                "name": "tampered_chain_detected_by_readback",
                "before": {"train_cert_count": before_chain_count, "verify": before_tamper},
                "action": "overwrite persisted step 1 parent_witness_hash",
                "after": {"verify": after_tamper}
            },
            {
                "name": "empty_holdout_rejected_before_report_write",
                "before": {"holdout_report_count": before_holdout},
                "action": "evaluate empty holdout dataset",
                "error_code": empty_err.code(),
                "after": {"holdout_report_count": after_holdout}
            }
        ],
        "passes": leak_err.code() == TrainerErrorCode::MejepaInstrumentGradientLeak.as_screaming_snake()
            && before_leak == after_leak
            && before_tamper.broken_at.is_none()
            && after_tamper.broken_at == Some(1)
            && before_holdout == after_holdout
    });
    assert_eq!(evidence["passes"], true);
    write_0600(
        Path::new(FSV_DIR).join("edge-case-evidence.json"),
        &evidence,
    )?;
    Ok(())
}

struct NoPredictor;
impl context_graph_mejepa_train::eval::holdout::PredictorForward for NoPredictor {
    fn forward(
        &self,
        _panel_t01: &candle_core::Tensor,
    ) -> Result<candle_core::Tensor, context_graph_mejepa_train::TrainerError> {
        unreachable!("empty holdout should fail before predictor call")
    }
}

struct NoOracle;
impl context_graph_mejepa_train::eval::holdout::OracleHead for NoOracle {
    fn predict(
        &self,
        _predicted_latent: &candle_core::Tensor,
    ) -> Result<
        context_graph_mejepa_train::eval::holdout::OraclePrediction,
        context_graph_mejepa_train::TrainerError,
    > {
        unreachable!("empty holdout should fail before oracle call")
    }
}

fn cert(
    step: u64,
    target_collapse: f32,
) -> Result<TrainingCertificate, Box<dyn std::error::Error>> {
    let signal = compute_l_step(
        DeltaPComponents {
            oracle_disagreement: 0.1,
            prediction_residual: 0.1,
            uncertainty: 0.1,
        },
        DeltaKComponents::default(),
        DeltaOmegaComponents::default(),
        DeltaXiComponents {
            target_collapse,
            predictor_collapse: 0.0,
            redundancy_mi: 0.0,
        },
    )?;
    Ok(TrainingCertificate {
        step,
        epoch: 0,
        signal,
        loss_components: HashMap::from([("predict".to_string(), 0.1)]),
        training_mode: "edge_case".to_string(),
        distillation_cycle_id: None,
        distillation_loss_mean: None,
        distillation_skipped_count: None,
        counterfactual_smoothness: None,
        counterfactual_smoothness_anomaly: None,
        cross_task_transfer_indices: vec![],
        cross_task_transfer_fallback_count: 0,
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

fn count_cf(db: &rocksdb::DB, cf_name: &str) -> Result<u64, rocksdb::Error> {
    let cf = db.cf_handle(cf_name).expect("cf");
    let mut count = 0;
    for item in db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
        item?;
        count += 1;
    }
    Ok(count)
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
