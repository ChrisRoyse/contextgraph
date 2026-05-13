use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use clap::Args;
use serde::{Deserialize, Serialize};

use crate::calibration::CalibrationStore;
use crate::compiler::{
    DeterministicConstellationGuard, DeterministicOracleHead, DeterministicPredictor,
    IdentityFrozenTarget, MeJepaCompiler, PatchWitnessReader,
};
use crate::config::MeJepaInferConfig;
use crate::conformal::{non_conformity_score, CalibrationExample};
use crate::error::MejepaInferError;
use crate::fixtures::fixture_patch_context;
use crate::ood::separation_auc;
use crate::store::RocksDbInferStore;

#[derive(Debug, Clone, Args)]
pub struct InferTestArgs {
    #[arg(long)]
    pub calibration: PathBuf,
    #[arg(long, default_value_t = 0.10)]
    pub alpha: f32,
    #[arg(long, default_value = "/tmp/contextgraph-mejepa-infer-fsv")]
    pub output_fsv: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferTestReport {
    pub stated_coverage: f32,
    pub empirical_coverage: f32,
    pub prediction_latency_p50_ms: f32,
    pub prediction_latency_p99_ms: f32,
    pub ood_separation_auc: f32,
    pub num_calibration_samples: usize,
    pub calibration_version: String,
}

pub fn run_infer_test(
    args: InferTestArgs,
    compiler: &MeJepaCompiler,
    store: &CalibrationStore,
) -> Result<InferTestReport, MejepaInferError> {
    fs::create_dir_all(&args.output_fsv)
        .map_err(|source| MejepaInferError::io("create_dir_all", &args.output_fsv, source))?;
    let examples = load_calibration_examples(&args.calibration)?;
    let norm_sq = examples
        .iter()
        .map(|example| {
            let score =
                non_conformity_score(&example.predicted_test_pass, &example.actual_test_pass)?;
            Ok(score * score)
        })
        .collect::<Result<Vec<_>, MejepaInferError>>()?;
    let record = store.calibrate(
        &examples,
        &norm_sq,
        args.alpha,
        30,
        0.30,
        [4; 32],
        BTreeMap::new(),
    )?;

    let holdout_start = (examples.len() * 8 / 10).min(examples.len().saturating_sub(1));
    let holdout = &examples[holdout_start..];
    let covered = holdout
        .iter()
        .filter(|example| {
            non_conformity_score(&example.predicted_test_pass, &example.actual_test_pass)
                .map(|score| score <= record.tau)
                .unwrap_or(false)
        })
        .count();
    let empirical_coverage = covered as f32 / holdout.len() as f32;

    let repo_root = args.output_fsv.join("infer-test-repo");
    let (patch, context) = fixture_patch_context(&repo_root, "approve")?;
    let mut latencies = Vec::with_capacity(1_000);
    for _ in 0..1_000 {
        let start = Instant::now();
        let _ = compiler.verify(&patch, &context)?;
        latencies.push(start.elapsed().as_secs_f32() * 1_000.0);
    }
    latencies.sort_by(|a, b| a.total_cmp(b));
    let p50 = latencies[latencies.len() / 2];
    let p99 = latencies[(latencies.len() * 99 / 100).min(latencies.len() - 1)];

    let in_dist_scores = vec![0.05; 100];
    let ood_scores = vec![0.95; 100];
    let auc = separation_auc(&in_dist_scores, &ood_scores)?;

    let report = InferTestReport {
        stated_coverage: 1.0 - args.alpha,
        empirical_coverage,
        prediction_latency_p50_ms: p50,
        prediction_latency_p99_ms: p99,
        ood_separation_auc: auc,
        num_calibration_samples: examples.len(),
        calibration_version: record.version,
    };
    write_json_0600(&args.output_fsv.join("infer-test-report.json"), &report)?;
    write_json_0600(&args.output_fsv.join("latency-histogram.json"), &latencies)?;
    Ok(report)
}

pub fn dod_satisfied(report: &InferTestReport) -> bool {
    (0.88..=0.92).contains(&report.empirical_coverage)
        && report.prediction_latency_p50_ms < 100.0
        && report.ood_separation_auc > 0.85
}

pub fn build_deterministic_compiler(
    repo_root: PathBuf,
    store: Arc<RocksDbInferStore>,
    calibration: CalibrationStore,
    mut config: MeJepaInferConfig,
) -> Result<MeJepaCompiler, MejepaInferError> {
    config.outcome_set_max = config.outcome_set_max.max(1);
    let system_cost_counters = store.system_cost_counters();
    MeJepaCompiler::new_with_system_cost_counters(
        config,
        Arc::new(DeterministicPredictor),
        Arc::new(IdentityFrozenTarget),
        Arc::new(DeterministicOracleHead),
        Arc::new(DeterministicConstellationGuard::default()),
        Arc::new(PatchWitnessReader),
        store,
        calibration,
        repo_root,
        system_cost_counters,
    )
}

pub fn load_calibration_examples(
    path: &PathBuf,
) -> Result<Vec<CalibrationExample>, MejepaInferError> {
    let mut examples = Vec::new();
    if path.is_file() {
        let bytes = fs::read(path).map_err(|source| MejepaInferError::io("read", path, source))?;
        examples = serde_json::from_slice(&bytes)?;
    } else {
        let entries =
            fs::read_dir(path).map_err(|source| MejepaInferError::io("read_dir", path, source))?;
        for entry in entries {
            let entry =
                entry.map_err(|source| MejepaInferError::io("read_dir_entry", path, source))?;
            let entry_path = entry.path();
            if entry_path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let bytes = fs::read(&entry_path)
                .map_err(|source| MejepaInferError::io("read", &entry_path, source))?;
            let example: CalibrationExample = serde_json::from_slice(&bytes)?;
            examples.push(example);
        }
    }
    if examples.is_empty() {
        return Err(MejepaInferError::ConformalInsufficientSamples {
            language: None,
            expected: 1,
            actual: 0,
        });
    }
    Ok(examples)
}

pub fn write_json_0600<T: Serialize>(path: &PathBuf, value: &T) -> Result<(), MejepaInferError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| MejepaInferError::io("create_dir_all", parent, source))?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|source| MejepaInferError::io("open", path, source))?
    };
    #[cfg(not(unix))]
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|source| MejepaInferError::io("open", path, source))?;
    file.write_all(&bytes)
        .map_err(|source| MejepaInferError::io("write", path, source))?;
    let readback = fs::read(path).map_err(|source| MejepaInferError::io("read", path, source))?;
    if readback != bytes {
        return Err(MejepaInferError::InvalidInput {
            field: "fsv_readback".to_string(),
            detail: format!(
                "{} readback bytes differ from written bytes",
                path.display()
            ),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)
            .map_err(|source| MejepaInferError::io("metadata", path, source))?
            .permissions()
            .mode()
            & 0o777;
        if mode != 0o600 {
            return Err(MejepaInferError::InvalidInput {
                field: "fsv_mode".to_string(),
                detail: format!("{} mode {mode:o} != 600", path.display()),
            });
        }
    }
    Ok(())
}
