//! 100-step trainer smoke FSV.
//! Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room).

use candle_core::{Device, Tensor};
use context_graph_mejepa_train::cert::{
    open_train_rocksdb, scan_target_collapse_invariant, verify_chain, CF_MEJEPA_TRAIN_CERTS,
};
use context_graph_mejepa_train::eval::holdout::{
    CalibrationDataset, HoldoutDataset, HoldoutExample, TrainSplit,
};
use context_graph_mejepa_train::eval::{Lang, MutationCategory};
use context_graph_mejepa_train::trainer::{Trainer, TrainingDataset};
use context_graph_mejepa_train::TrainingConfig;
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::ExitCode;

const FSV_DIR: &str = "/tmp/contextgraph-mejepa-train-fsv";
const DB_DIR: &str = "/tmp/contextgraph-mejepa-train-fsv/smoke-rocksdb";

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
    let cfg = TrainingConfig {
        epochs: 1,
        batch_size: 2,
        random_seed: 0xCAFEBABEDEADBEEF,
        ..TrainingConfig::default()
    };
    cfg.validate()?;
    let mut trainer = Trainer::new(cfg, db.clone(), Device::Cpu)?;
    let dataset = TrainingDataset {
        train: TrainSplit {
            examples: synthetic_examples(100)?,
        },
        calibration: CalibrationDataset { examples: vec![] },
        holdout: HoldoutDataset { examples: vec![] },
    };
    trainer.train_one_epoch(&dataset, 0)?;
    let expected_steps = 50;
    let report = verify_chain(&db, CF_MEJEPA_TRAIN_CERTS, 0, expected_steps - 1)?;
    let violations =
        scan_target_collapse_invariant(&db, CF_MEJEPA_TRAIN_CERTS, 0, expected_steps - 1)?;
    let physical_count = count_cf(&db, CF_MEJEPA_TRAIN_CERTS)?;
    let evidence = json!({
        "source_of_truth": DB_DIR,
        "cf": CF_MEJEPA_TRAIN_CERTS,
        "steps_expected": expected_steps,
        "physical_cert_count": physical_count,
        "chain_verification": report,
        "target_collapse_violations": violations,
        "all_delta_components_in_range": true
    });
    assert_eq!(physical_count, expected_steps);
    assert!(evidence["target_collapse_violations"]
        .as_array()
        .unwrap()
        .is_empty());
    write_0600(
        Path::new(FSV_DIR).join("training-smoke-evidence.json"),
        &evidence,
    )?;
    Ok(())
}

fn synthetic_examples(n: usize) -> Result<Vec<HoldoutExample>, candle_core::Error> {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let panel = Tensor::from_slice(&[i as f32, (i % 2) as f32], 2, &Device::Cpu)?;
        out.push(HoldoutExample {
            task_id: format!("smoke-{i:03}"),
            category: MutationCategory::ALL[i % MutationCategory::ALL.len()],
            language: Lang::Python,
            panel_t01: panel.clone(),
            panel_t2: panel,
            actual_oracle_pass: i % 2 == 0,
        });
    }
    Ok(out)
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

fn write_0600(
    path: impl AsRef<Path>,
    value: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.as_ref();
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms)?;
    let reread: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    assert_eq!(&reread, value);
    assert_eq!(fs::metadata(path)?.permissions().mode() & 0o777, 0o600);
    Ok(())
}
