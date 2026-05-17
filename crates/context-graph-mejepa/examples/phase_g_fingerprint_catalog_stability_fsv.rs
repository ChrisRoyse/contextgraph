//! TASK-FP-008 fingerprint catalog stability FSV.
//!
//! Proves per-fingerprint EWC Fisher and CBP dormancy snapshots are durable
//! source-of-truth records and that their compute helpers fail closed.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use context_graph_mejepa::{
    require_fingerprint_fisher, sanitize_fingerprint_dormancy_ema,
    write_fingerprint_dormancy_sync_readback, write_fingerprint_fisher_sync_readback,
    write_fingerprint_sync_readback, ChunkId, EmbedderId, FailureModeClass,
    FailureShapeFingerprint, FingerprintCbpReset, FingerprintDormancySnapshot,
    FingerprintEwcPenalty, FingerprintFisherSnapshot, FingerprintKind, FingerprintPairMetric,
    MutationCategory, OracleOutcome, DEFAULT_FINGERPRINT_CBP_DORMANCY_THRESHOLD,
    DEFAULT_FINGERPRINT_EWC_LAMBDA, FAILURE_FINGERPRINT_SCHEMA_VERSION,
    FINGERPRINT_DORMANCY_RESET_CODE,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_FAILURE_FINGERPRINTS, CF_MEJEPA_FINGERPRINT_DORMANCY, CF_MEJEPA_FINGERPRINT_FISHER,
    FINGERPRINT_CFS,
};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use serde_json::{json, Value};

const DEFAULT_FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-fp-008-fingerprint-stability-fsv";

fn main() -> Result<()> {
    let output_root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_FSV_ROOT));
    let report = run(&output_root)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn run(output_root: &Path) -> Result<Value> {
    if output_root.exists() {
        fs::remove_dir_all(output_root)
            .with_context(|| format!("remove {}", output_root.display()))?;
    }
    fs::create_dir_all(output_root).with_context(|| format!("create {}", output_root.display()))?;
    let db_path = output_root.join("fingerprint-stability-rocksdb");
    let db = open_db(&db_path)?;

    let now_ms = chrono::Utc::now().timestamp_millis();
    let fingerprint = synthetic_fingerprint(now_ms)?;
    write_fingerprint_sync_readback(&db, &fingerprint)?;

    let fisher = FingerprintFisherSnapshot {
        fingerprint_id: fingerprint.fingerprint_id,
        calibrated_at_unix_ms: now_ms,
        sample_count: 64,
        theta_star_by_dimension: BTreeMap::from([(0, 0.10), (1, -0.20), (2, 0.30)]),
        fisher_by_dimension: BTreeMap::from([(0, 0.80), (1, 1.20), (2, 0.40)]),
    };
    write_fingerprint_fisher_sync_readback(&db, &fisher)?;
    let zero_penalty = FingerprintEwcPenalty::compute(
        &[0.10, -0.20, 0.30],
        &fisher,
        DEFAULT_FINGERPRINT_EWC_LAMBDA,
    )?;
    let shifted_penalty = FingerprintEwcPenalty::compute(
        &[0.15, -0.10, 0.25],
        &fisher,
        DEFAULT_FINGERPRINT_EWC_LAMBDA,
    )?;

    let raw_dormancy = BTreeMap::from([(0, 0.02), (1, f32::NAN), (2, 0.80)]);
    let sanitized = sanitize_fingerprint_dormancy_ema(&raw_dormancy)?;
    let dormancy = FingerprintDormancySnapshot {
        fingerprint_id: fingerprint.fingerprint_id,
        recorded_at_unix_ms: now_ms + 1,
        window_steps: 128,
        dormancy_ema_by_dimension: sanitized.sanitized_ema_by_dimension.clone(),
        dormancy_reset_count: sanitized.reset_count,
    };
    write_fingerprint_dormancy_sync_readback(&db, &dormancy)?;
    let cbp_plan =
        FingerprintCbpReset::plan(&dormancy, DEFAULT_FINGERPRINT_CBP_DORMANCY_THRESHOLD)?;

    let missing_fingerprint = synthetic_unknown_fingerprint(now_ms + 2)?;
    write_fingerprint_sync_readback(&db, &missing_fingerprint)?;
    let boundary_cases = vec![
        boundary(
            "all_zero_fisher_rejected",
            FingerprintFisherSnapshot {
                fisher_by_dimension: BTreeMap::from([(0, 0.0), (1, 0.0)]),
                theta_star_by_dimension: BTreeMap::from([(0, 0.0), (1, 0.0)]),
                ..fisher.clone()
            }
            .validate()
            .map(|_| ()),
            "MEJEPA_INFER_INVALID_INPUT",
        ),
        boundary(
            "missing_fisher_fails_closed",
            require_fingerprint_fisher(&db, missing_fingerprint.fingerprint_id).map(|_| ()),
            "MEJEPA_FINGERPRINT_FISHER_MISSING",
        ),
        boundary(
            "direct_nan_dormancy_rejected",
            FingerprintDormancySnapshot {
                dormancy_ema_by_dimension: BTreeMap::from([(0, f32::NAN)]),
                ..dormancy.clone()
            }
            .validate()
            .map(|_| ()),
            "MEJEPA_INFER_INVALID_INPUT",
        ),
        boundary(
            "parameter_dim_mismatch_rejected",
            FingerprintEwcPenalty::compute(&[0.0], &fisher, DEFAULT_FINGERPRINT_EWC_LAMBDA)
                .map(|_| ()),
            "MEJEPA_INFER_DIM_MISMATCH",
        ),
    ];
    drop(db);

    let reopened = open_db(&db_path)?;
    let fisher_readback =
        context_graph_mejepa::read_fingerprint_fisher(&reopened, fingerprint.fingerprint_id)?
            .context("missing fisher readback after reopen")?;
    let dormancy_readback =
        context_graph_mejepa::read_fingerprint_dormancy(&reopened, fingerprint.fingerprint_id)?
            .context("missing dormancy readback after reopen")?;
    let counts = counts_json(&reopened)?;

    let assertions = json!({
        "fisher_readback_equal": fisher_readback == fisher,
        "dormancy_readback_equal": dormancy_readback == dormancy,
        "zero_penalty_for_anchor_params": zero_penalty.scaled_penalty == 0.0,
        "shifted_penalty_positive": shifted_penalty.scaled_penalty > 0.0,
        "nan_dormancy_preserved_by_default": sanitized.reset_code.as_deref() == Some(FINGERPRINT_DORMANCY_RESET_CODE)
            && sanitized.reset_dimensions == vec![1]
            && dormancy.dormancy_ema_by_dimension.get(&1) == Some(&1.0),
        "cbp_resets_only_low_utility_dim": cbp_plan.reset_dimensions == vec![0]
            && cbp_plan.protected_dimensions == vec![1, 2],
        "fisher_cf_count": counts[CF_MEJEPA_FINGERPRINT_FISHER] == 1,
        "dormancy_cf_count": counts[CF_MEJEPA_FINGERPRINT_DORMANCY] == 1,
        "boundary_cases_passed": boundary_cases.iter().all(|case| case["passed"] == true),
    });
    let all_passed = assertions
        .as_object()
        .expect("assertions is object")
        .values()
        .all(|value| value.as_bool() == Some(true));
    if !all_passed {
        bail!("TASK-FP-008 FSV assertions failed: {assertions}");
    }

    let report_path = output_root.join("fingerprint_catalog_stability_fsv.json");
    let output = json!({
        "task_id": "TASK-FP-008",
        "fsv": "phase_g_fingerprint_catalog_stability",
        "all_passed": true,
        "artifact_path": report_path,
        "source_of_truth": {
            "catalog_cf": CF_MEJEPA_FAILURE_FINGERPRINTS,
            "fingerprint_fisher_cf": CF_MEJEPA_FINGERPRINT_FISHER,
            "fingerprint_dormancy_cf": CF_MEJEPA_FINGERPRINT_DORMANCY
        },
        "ewc": {
            "zero_penalty": zero_penalty,
            "shifted_penalty": shifted_penalty
        },
        "cbp": {
            "sanitization": sanitized,
            "reset_plan": cbp_plan
        },
        "counts_after_reopen": counts,
        "boundary_cases": boundary_cases,
        "assertions": assertions
    });
    let bytes = serde_json::to_vec_pretty(&output)?;
    fs::write(&report_path, &bytes).with_context(|| format!("write {}", report_path.display()))?;
    let readback =
        fs::read(&report_path).with_context(|| format!("read {}", report_path.display()))?;
    if readback != bytes {
        bail!("FSV report readback bytes differ from written bytes");
    }
    Ok(output)
}

fn synthetic_fingerprint(now_ms: i64) -> Result<FailureShapeFingerprint> {
    let kind = FingerprintKind::KnownBad {
        repo: "django/django".to_string(),
        mutation_category: MutationCategory::OffByOne,
        failure_mode: FailureModeClass::OffByOne,
        exception_class: None,
    };
    build_fingerprint(kind, now_ms)
}

fn synthetic_unknown_fingerprint(now_ms: i64) -> Result<FailureShapeFingerprint> {
    let kind = FingerprintKind::Unknown {
        observed_at_unix_ms: now_ms,
        observed_by_session: [0x22; 16],
        ood_score: 0.42,
        embedder_disagreement_score: 0.21,
        active_learning_priority: 2,
    };
    build_fingerprint(kind, now_ms)
}

fn build_fingerprint(kind: FingerprintKind, now_ms: i64) -> Result<FailureShapeFingerprint> {
    let source_corpus = "task-fp-008-fsv";
    let fingerprint_id = FailureShapeFingerprint::canonical_id(&kind, source_corpus)?;
    let oracle_outcome = match &kind {
        FingerprintKind::KnownGood { .. } => Some(OracleOutcome::Pass),
        FingerprintKind::KnownBad { .. } => Some(OracleOutcome::Fail),
        FingerprintKind::Unknown { .. } => None,
    };
    let embedders = [EmbedderId("e1".to_string()), EmbedderId("e2".to_string())];
    let centroid_by_embedder = BTreeMap::from([
        (embedders[0].clone(), vec![0.2, 0.4, 0.6]),
        (embedders[1].clone(), vec![0.3, 0.5, 0.7]),
    ]);
    let variance_by_embedder =
        BTreeMap::from([(embedders[0].clone(), 0.01), (embedders[1].clone(), 0.02)]);
    let tau_by_embedder =
        BTreeMap::from([(embedders[0].clone(), 0.70), (embedders[1].clone(), 0.72)]);
    let fingerprint = FailureShapeFingerprint {
        schema_version: FAILURE_FINGERPRINT_SCHEMA_VERSION,
        fingerprint_id,
        name: kind.class_slug(),
        kind,
        source_corpus: source_corpus.to_string(),
        source_manifest_sha256: Some([0x44; 32]),
        centroid_by_embedder,
        variance_by_embedder,
        tau_by_embedder,
        pairwise_cosine: vec![FingerprintPairMetric {
            left_embedder: embedders[0].clone(),
            right_embedder: embedders[1].clone(),
            value: 0.33,
        }],
        pairwise_mutual_information: vec![FingerprintPairMetric {
            left_embedder: embedders[0].clone(),
            right_embedder: embedders[1].clone(),
            value: 0.12,
        }],
        reference_chunks: vec![ChunkId(format!("{}:chunk-0", fingerprint_id.hex()))],
        n_references: 1,
        oracle_outcome,
        is_canonical: true,
        frozen_at_unix_ms: now_ms,
        confidence: context_graph_mejepa::FingerprintConfidence {
            classification_accuracy: Some(0.97),
            classification_precision: Some(0.96),
            unknown_recall: Some(0.92),
            calibration_observations: 1,
            calibration_state: context_graph_mejepa::FingerprintCalibrationState::Calibrated,
        },
    };
    fingerprint.validate()?;
    Ok(fingerprint)
}

fn boundary(
    case: &'static str,
    result: std::result::Result<(), context_graph_mejepa::MejepaInferError>,
    expected_code: &'static str,
) -> Value {
    match result {
        Ok(()) => json!({
            "case": case,
            "expected_code": expected_code,
            "actual_code": "none",
            "passed": false
        }),
        Err(err) => json!({
            "case": case,
            "expected_code": expected_code,
            "actual_code": err.code(),
            "error": err.to_string(),
            "passed": err.code() == expected_code
        }),
    }
}

fn open_db(path: &Path) -> Result<DB> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    opts.set_paranoid_checks(true);
    let mut descriptors: Vec<ColumnFamilyDescriptor> = FINGERPRINT_CFS
        .iter()
        .map(|cf| ColumnFamilyDescriptor::new(*cf, Options::default()))
        .collect();
    descriptors.push(ColumnFamilyDescriptor::new("default", Options::default()));
    DB::open_cf_descriptors(&opts, path, descriptors).context("open fingerprint RocksDB")
}

fn counts_json(db: &DB) -> Result<Value> {
    Ok(json!({
        CF_MEJEPA_FAILURE_FINGERPRINTS: count_cf(db, CF_MEJEPA_FAILURE_FINGERPRINTS)?,
        CF_MEJEPA_FINGERPRINT_FISHER: count_cf(db, CF_MEJEPA_FINGERPRINT_FISHER)?,
        CF_MEJEPA_FINGERPRINT_DORMANCY: count_cf(db, CF_MEJEPA_FINGERPRINT_DORMANCY)?,
    }))
}

fn count_cf(db: &DB, cf_name: &str) -> Result<usize> {
    let cf = db
        .cf_handle(cf_name)
        .with_context(|| format!("missing CF {cf_name}"))?;
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        item.with_context(|| format!("iterate {cf_name}"))?;
        count += 1;
    }
    Ok(count)
}
