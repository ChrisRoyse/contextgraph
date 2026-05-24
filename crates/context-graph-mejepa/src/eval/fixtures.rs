use super::error::EvalError;
use super::graph::PatchEmbedding;
use super::store::RocksDbEvalStore;
use super::types::{HoldoutPanel, MutationCategory};
use crate::calibration::CalibrationStore;
use crate::compiler::TrainCertSummary;
use crate::conformal::CalibrationExample;
use crate::fixtures::fixture_patch_context;
use crate::gates::sha256_bytes;
use crate::types::{EmbedderId, FailureModeClass, Language, OracleOutcome, TaskId};
use crate::{
    build_fixture_deterministic_compiler, open_infer_rocksdb, MeJepaCompiler, MeJepaInferConfig,
    RocksDbInferStore,
};
use rocksdb::DB;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

pub fn build_eval_compiler(
    db_path: &Path,
    repo_root: &Path,
) -> Result<
    (
        Arc<DB>,
        Arc<MeJepaCompiler>,
        RocksDbEvalStore,
        CalibrationStore,
    ),
    EvalError,
> {
    let db = open_infer_rocksdb(db_path)?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    seed_calibration_and_train_certs(&db, &calibration)?;
    let store = Arc::new(RocksDbInferStore::new(db.clone()));
    let config = MeJepaInferConfig {
        ood_refuse_threshold: 1.0,
        outcome_set_max: 4,
        ..Default::default()
    };
    let compiler = Arc::new(build_fixture_deterministic_compiler(
        repo_root.to_path_buf(),
        store,
        calibration.clone(),
        config,
    )?);
    let eval_store = RocksDbEvalStore::new(db.clone())?;
    Ok((db, compiler, eval_store, calibration))
}

pub fn synthetic_holdout(repo_root: &Path) -> Result<Vec<HoldoutPanel>, EvalError> {
    let specs = [
        (
            "known_good_python",
            MutationCategory::KnownGood,
            OracleOutcome::Pass,
            vec![],
        ),
        (
            "predicted_failure_subtle_flip",
            MutationCategory::SubtleFlip,
            OracleOutcome::Fail,
            vec![FailureModeClass::WrongAlgorithm],
        ),
        (
            "predicted_failure_off_by_one",
            MutationCategory::OffByOne,
            OracleOutcome::Fail,
            vec![FailureModeClass::OffByOne],
        ),
        (
            "low_confidence_swap_variable",
            MutationCategory::SwapVariable,
            OracleOutcome::Pass,
            vec![],
        ),
        (
            "predicted_failure_delete_test_call",
            MutationCategory::DeleteTestCall,
            OracleOutcome::Fail,
            vec![FailureModeClass::AssertionViolation],
        ),
        (
            "ood_wrong_file",
            MutationCategory::WrongFile,
            OracleOutcome::OutOfDistribution,
            vec![],
        ),
        (
            "granger_rejection_over_engineer",
            MutationCategory::OverEngineer,
            OracleOutcome::Pass,
            vec![],
        ),
        (
            "predicted_failure_compile_error",
            MutationCategory::CompileError,
            OracleOutcome::Fail,
            vec![FailureModeClass::CompileError],
        ),
    ];
    let mut out = Vec::with_capacity(specs.len());
    for (scenario, category, actual, actual_failure_modes) in specs {
        let (patch, mut context) = fixture_patch_context(repo_root, scenario)?;
        context.task_id = TaskId(format!("phase8-{scenario}"));
        context.language = Language::Python;
        let panel_sha = sha256_bytes(format!("phase8-panel:{scenario}").as_bytes());
        out.push(HoldoutPanel {
            task_id: context.task_id.clone(),
            mutation_category: category,
            language: context.language,
            patch,
            context,
            actual_oracle: actual,
            actual_failure_modes,
            panel_sha,
        });
    }
    Ok(out)
}

pub fn synthetic_patch_embeddings() -> Vec<PatchEmbedding> {
    (0..8)
        .map(|idx| PatchEmbedding {
            task_id: TaskId(format!("phase8-embedding-{idx}")),
            vector: vec![
                1.0,
                idx as f32 / 10.0,
                if idx % 2 == 0 { 0.25 } else { 0.75 },
                (7 - idx) as f32 / 10.0,
            ],
        })
        .collect()
}

fn seed_calibration_and_train_certs(
    db: &Arc<DB>,
    calibration: &CalibrationStore,
) -> Result<(), EvalError> {
    let examples = (0..40)
        .map(|idx| CalibrationExample {
            language: Language::Python,
            predicted_test_pass: vec![0.50],
            actual_test_pass: vec![if idx % 2 == 0 { 1.0 } else { 0.0 }],
        })
        .collect::<Vec<_>>();
    calibration.calibrate(
        &examples,
        &[0.01; 40],
        0.10,
        30,
        0.30,
        [8; 32],
        BTreeMap::<EmbedderId, String>::new(),
    )?;
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)
        .ok_or_else(|| EvalError::new(super::error::EvalErrorCode::Store, "missing train CF"))?;
    let cert = TrainCertSummary {
        step: 1,
        delta_omega: 0.9,
        delta_xi: 0.9,
        witness_offset: 88,
        // #699: fixture proves "trained" path; bump to 1 so compute_train_health
        // treats this cert as Measured rather than filtering it out.
        predictor_parameter_update_count: 1,
    };
    db.put_cf(cf, b"cert:phase8-eval:0001", bincode::serialize(&cert)?)?;
    Ok(())
}
