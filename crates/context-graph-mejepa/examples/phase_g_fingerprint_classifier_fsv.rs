//! TASK-FP-004 live failure-fingerprint classifier FSV.
//!
//! Proves the calibrated catalog can be read from RocksDB and consumed through
//! the predictor-head compiler surface. The cases cover KnownGood, KnownBad,
//! Unknown/OOD, conflict abstain, and fail-closed malformed observations.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use context_graph_mejepa::{
    classify_failure_fingerprint_observation, write_fingerprint_sync_readback, CalibrationStore,
    DeterministicConstellationGuard, DeterministicOracleHead, DeterministicPredictor, EmbedderId,
    FailureModeClass, FailureShapeFingerprint, FingerprintCalibrationState,
    FingerprintClassifierConfig, FingerprintConfidence, FingerprintDecisionReason, FingerprintKind,
    IdentityFrozenTarget, MeJepaCompiler, MeJepaInferConfig, MutationCategory, OracleOutcome,
    PatchWitnessReader, RocksDbInferStore, Verdict, FAILURE_FINGERPRINT_SCHEMA_VERSION,
};
use context_graph_mejepa_instruments::ExceptionClass;
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde::Serialize;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-g-fingerprint-classifier-fsv";

#[derive(Debug, Serialize)]
struct ClassificationCase {
    case: &'static str,
    verdict: Verdict,
    reason: FingerprintDecisionReason,
    primary: Option<String>,
    ranked_len: usize,
    pass: bool,
}

#[derive(Debug, Serialize)]
struct BoundaryCase {
    case: &'static str,
    error_code: String,
    pass: bool,
}

#[derive(Debug, Serialize)]
struct FsvReport {
    fsv_root: String,
    db_path: String,
    output_path: String,
    started_at_unix_ms: i64,
    catalog_rows: usize,
    classification_cases: Vec<ClassificationCase>,
    boundary_cases: Vec<BoundaryCase>,
    all_passed: bool,
}

fn main() -> Result<()> {
    let started_at_unix_ms = unix_ms()?;
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    fs::create_dir_all(&run_root).context("create FSV root")?;
    let db_path = run_root.join("rocks");
    let output_path = run_root.join("fingerprint_classifier_fsv.json");

    let db = open_full_db(&db_path)?;
    let good = fingerprint(
        FingerprintKind::KnownGood {
            repo: Some("django__django".to_string()),
            gold_patch_count: 2,
        },
        vec![1.0, 0.0],
    )?;
    let bad = fingerprint(
        FingerprintKind::KnownBad {
            repo: "django__django".to_string(),
            mutation_category: MutationCategory::OffByOne,
            failure_mode: FailureModeClass::OffByOne,
            exception_class: Some(ExceptionClass::AssertionError),
        },
        vec![0.0, 1.0],
    )?;
    write_fingerprint_sync_readback(db.as_ref(), &good)?;
    write_fingerprint_sync_readback(db.as_ref(), &bad)?;

    let compiler = compiler_for_store(&run_root, db.clone())?;
    let classification_cases = vec![
        classify_case(
            "known_good_only",
            compiler.classify_failure_fingerprints(&vectors([("e1", vec![1.0, 0.0])]))?,
            Verdict::Pass,
            FingerprintDecisionReason::KnownGoodOnly,
        ),
        classify_case(
            "known_bad_only",
            compiler.classify_failure_fingerprints(&vectors([("e1", vec![0.0, 1.0])]))?,
            Verdict::Fail,
            FingerprintDecisionReason::KnownBadOnly,
        ),
        classify_case(
            "unknown_ood",
            compiler.classify_failure_fingerprints(&vectors([("e1", vec![-1.0, 0.0])]))?,
            Verdict::OutOfDistribution,
            FingerprintDecisionReason::NoKnownMatch,
        ),
    ];

    let conflict_bad = fingerprint(
        FingerprintKind::KnownBad {
            repo: "django__django".to_string(),
            mutation_category: MutationCategory::SubtleFlip,
            failure_mode: FailureModeClass::WrongAlgorithm,
            exception_class: Some(ExceptionClass::AssertionError),
        },
        vec![1.0, 0.0],
    )?;
    let conflict = classify_failure_fingerprint_observation(
        &[good.clone(), conflict_bad],
        &vectors([("e1", vec![1.0, 0.0])]),
        FingerprintClassifierConfig::default(),
    )?;
    let mut classification_cases = classification_cases;
    classification_cases.push(classify_case(
        "known_good_known_bad_conflict",
        conflict,
        Verdict::Abstain,
        FingerprintDecisionReason::KnownGoodKnownBadConflict,
    ));

    let boundary_cases = vec![
        boundary_case(
            "missing_embedder_fails_closed",
            compiler.classify_failure_fingerprints(&BTreeMap::new()),
            "MEJEPA_INFER_INVALID_INPUT",
        ),
        boundary_case(
            "dimension_mismatch_fails_closed",
            compiler.classify_failure_fingerprints(&vectors([("e1", vec![1.0, 0.0, 0.0])])),
            "MEJEPA_INFER_DIM_MISMATCH",
        ),
        boundary_case(
            "nonfinite_vector_fails_closed",
            compiler.classify_failure_fingerprints(&vectors([("e1", vec![f32::NAN, 0.0])])),
            "MEJEPA_INFER_NAN_DETECTED",
        ),
    ];

    let catalog_rows = compiler.store().read_failure_fingerprint_catalog()?.len();
    let all_passed = catalog_rows == 2
        && classification_cases.iter().all(|case| case.pass)
        && boundary_cases.iter().all(|case| case.pass);
    if !all_passed {
        bail!("fingerprint classifier FSV did not pass all checks");
    }

    let report = FsvReport {
        fsv_root: FSV_ROOT.to_string(),
        db_path: db_path.display().to_string(),
        output_path: output_path.display().to_string(),
        started_at_unix_ms,
        catalog_rows,
        classification_cases,
        boundary_cases,
        all_passed,
    };
    let bytes = serde_json::to_vec_pretty(&report).context("serialize report")?;
    fs::write(&output_path, &bytes).context("write report")?;
    let readback = fs::read(&output_path).context("read report")?;
    if readback != bytes {
        bail!("FSV report readback bytes differ from written bytes");
    }
    println!("{}", output_path.display());
    Ok(())
}

fn compiler_for_store(repo_root: &Path, db: Arc<DB>) -> Result<MeJepaCompiler> {
    let store = Arc::new(RocksDbInferStore::new(db.clone()));
    let calibration = CalibrationStore::new(db, 30)?;
    Ok(MeJepaCompiler::new(
        MeJepaInferConfig::default(),
        Arc::new(DeterministicPredictor),
        Arc::new(IdentityFrozenTarget),
        Arc::new(DeterministicOracleHead),
        Arc::new(DeterministicConstellationGuard::default()),
        Arc::new(PatchWitnessReader),
        store,
        calibration,
        repo_root.to_path_buf(),
    )?)
}

fn classify_case(
    case: &'static str,
    classification: context_graph_mejepa::FingerprintClassification,
    expected_verdict: Verdict,
    expected_reason: FingerprintDecisionReason,
) -> ClassificationCase {
    let primary = classification
        .primary_match
        .as_ref()
        .map(|candidate| candidate.name.clone());
    ClassificationCase {
        case,
        verdict: classification.verdict,
        reason: classification.reason,
        primary,
        ranked_len: classification.ranked_matches.len(),
        pass: classification.verdict == expected_verdict
            && classification.reason == expected_reason
            && !classification.ranked_matches.is_empty(),
    }
}

fn boundary_case<T>(
    case: &'static str,
    result: Result<T, context_graph_mejepa::MejepaInferError>,
    expected_code: &'static str,
) -> BoundaryCase {
    match result {
        Ok(_) => BoundaryCase {
            case,
            error_code: "unexpected_ok".to_string(),
            pass: false,
        },
        Err(err) => BoundaryCase {
            case,
            error_code: err.code().to_string(),
            pass: err.code() == expected_code,
        },
    }
}

fn fingerprint(kind: FingerprintKind, centroid: Vec<f32>) -> Result<FailureShapeFingerprint> {
    let source_corpus = "phase-g-fingerprint-classifier-fsv";
    let fingerprint_id = FailureShapeFingerprint::canonical_id(&kind, source_corpus)?;
    let embedder = EmbedderId("e1".to_string());
    let oracle_outcome = match kind {
        FingerprintKind::KnownGood { .. } => Some(OracleOutcome::Pass),
        FingerprintKind::KnownBad { .. } => Some(OracleOutcome::Fail),
        FingerprintKind::Unknown { .. } => None,
    };
    let fingerprint = FailureShapeFingerprint {
        schema_version: FAILURE_FINGERPRINT_SCHEMA_VERSION,
        fingerprint_id,
        kind,
        name: "classifier_fsv_fingerprint".to_string(),
        source_corpus: source_corpus.to_string(),
        source_manifest_sha256: Some([0x5a; 32]),
        centroid_by_embedder: BTreeMap::from([(embedder.clone(), centroid)]),
        variance_by_embedder: BTreeMap::from([(embedder.clone(), 0.01)]),
        tau_by_embedder: BTreeMap::from([(embedder, 0.90)]),
        pairwise_cosine: Vec::new(),
        pairwise_mutual_information: Vec::new(),
        reference_chunks: vec![context_graph_mejepa::ChunkId("chunk-1".to_string())],
        n_references: 1,
        oracle_outcome,
        is_canonical: true,
        frozen_at_unix_ms: 1,
        confidence: FingerprintConfidence {
            classification_accuracy: Some(1.0),
            classification_precision: Some(1.0),
            unknown_recall: None,
            calibration_observations: 2,
            calibration_state: FingerprintCalibrationState::Calibrated,
        },
    };
    fingerprint.validate()?;
    Ok(fingerprint)
}

fn vectors<const N: usize>(items: [(&str, Vec<f32>); N]) -> BTreeMap<EmbedderId, Vec<f32>> {
    items
        .into_iter()
        .map(|(embedder, vector)| (EmbedderId(embedder.to_string()), vector))
        .collect()
}

fn open_full_db(path: &Path) -> Result<Arc<DB>> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_paranoid_checks(true);
    let descriptors = context_graph_mejepa_cf::all_hygiene_referenced_cfs()
        .into_iter()
        .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default()))
        .collect::<Vec<_>>();
    Ok(Arc::new(DB::open_cf_descriptors(&opts, path, descriptors)?))
}

fn unix_ms() -> Result<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before unix epoch")?
        .as_millis()
        .try_into()
        .context("unix millis exceeds i64")
}
