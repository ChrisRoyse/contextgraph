//! TASK-FP-001/002 failure-shape fingerprint source-of-truth FSV.
//!
//! This harness proves the new fingerprint catalog has a real RocksDB storage
//! path before the aiwonder seed materializer writes the 97 production
//! fingerprints. It writes KnownGood and KnownBad records, references,
//! calibration rows, audit rows, drops the DB handle, reopens it, and verifies
//! field-equal readback from the canonical fingerprint CFs.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use context_graph_mejepa::EmbedderId;
use context_graph_mejepa::MutationCategory;
use context_graph_mejepa::{
    write_fingerprint_audit_sync_readback, write_fingerprint_calibration_sync_readback,
    write_fingerprint_reference_sync_readback, write_fingerprint_sync_readback, ChunkId,
    FailureModeClass, FailureShapeFingerprint, FingerprintAuditAction, FingerprintAuditEntry,
    FingerprintCalibrationRecord, FingerprintCalibrationState, FingerprintConfidence,
    FingerprintId, FingerprintKind, FingerprintPairMetric, FingerprintReference, OracleOutcome,
    TaskId, FAILURE_FINGERPRINT_SCHEMA_VERSION, FORWARD_CACHE_EMBEDDER_COUNT,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_FAILURE_FINGERPRINTS, CF_MEJEPA_FINGERPRINT_AUDIT, CF_MEJEPA_FINGERPRINT_CALIBRATION,
    CF_MEJEPA_FINGERPRINT_REFERENCES, CF_MEJEPA_FINGERPRINT_REVERSE_INDEX, FINGERPRINT_CFS,
};
use context_graph_mejepa_instruments::ExceptionClass;
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use serde::Serialize;
use serde_json::json;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-g-failure-fingerprint-fsv";

#[derive(Debug, Serialize)]
struct ReadbackCase {
    case: &'static str,
    cf: &'static str,
    key_hex: String,
    value_bytes: usize,
    readback_equal: bool,
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
    expected_embedder_count: usize,
    cf_counts_before: serde_json::Value,
    cf_counts_after_reopen: serde_json::Value,
    readback_cases: Vec<ReadbackCase>,
    boundary_cases: Vec<BoundaryCase>,
    all_passed: bool,
}

fn main() -> Result<()> {
    let started_at_unix_ms = unix_ms()?;
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    fs::create_dir_all(&run_root).context("create fsv run root")?;
    let db_path = run_root.join("rocks");
    let output_path = run_root.join("failure_fingerprint_fsv.json");

    let db = open_db(&db_path)?;
    let cf_counts_before = counts_json(&db)?;

    let source_corpus = "python-forward-cache-v1-aiwonder-prod-20260515-191116Z";
    let manifest_sha = [0x5a; 32];
    let embedders = embedder_ids();
    let known_good = build_fingerprint(
        FingerprintKind::KnownGood {
            repo: Some("django__django".to_string()),
            gold_patch_count: 2,
        },
        source_corpus,
        Some(manifest_sha),
        2,
        started_at_unix_ms,
        &embedders,
    )?;
    let known_bad = build_fingerprint(
        FingerprintKind::KnownBad {
            repo: "django/django".to_string(),
            mutation_category: MutationCategory::OffByOne,
            failure_mode: FailureModeClass::OffByOne,
            exception_class: Some(ExceptionClass::AssertionError),
        },
        source_corpus,
        Some(manifest_sha),
        2,
        started_at_unix_ms,
        &embedders,
    )?;

    write_fingerprint_sync_readback(&db, &known_good)?;
    write_fingerprint_sync_readback(&db, &known_bad)?;
    let good_ref = build_reference(
        known_good.fingerprint_id,
        "good-ref-1",
        "django__django",
        MutationCategory::KnownGood,
        OracleOutcome::Pass,
        known_good.reference_chunks[0].clone(),
        &embedders,
    );
    let bad_ref = build_reference(
        known_bad.fingerprint_id,
        "bad-ref-1",
        "django__django",
        MutationCategory::OffByOne,
        OracleOutcome::Fail,
        known_bad.reference_chunks[0].clone(),
        &embedders,
    );
    write_fingerprint_reference_sync_readback(&db, &good_ref)?;
    write_fingerprint_reference_sync_readback(&db, &bad_ref)?;
    let calibration = FingerprintCalibrationRecord {
        fingerprint_id: known_bad.fingerprint_id,
        calibrated_at_unix_ms: started_at_unix_ms,
        tau_by_embedder: known_bad.tau_by_embedder.clone(),
        same_session_band_percentile: 0.10,
        sample_count: 2,
    };
    write_fingerprint_calibration_sync_readback(&db, &calibration)?;
    let audit = FingerprintAuditEntry {
        fingerprint_id: known_bad.fingerprint_id,
        action: FingerprintAuditAction::Created,
        actor: "phase_g_failure_fingerprint_fsv".to_string(),
        created_at_unix_ms: started_at_unix_ms,
        detail: "synthetic source-of-truth readback row".to_string(),
    };
    write_fingerprint_audit_sync_readback(&db, &audit)?;
    drop(db);

    let reopened = open_db(&db_path)?;
    let readback_cases = vec![
        readback_case(
            &reopened,
            CF_MEJEPA_FAILURE_FINGERPRINTS,
            &known_good.fingerprint_id.key_bytes(),
            &known_good,
            "known_good_fingerprint",
        )?,
        readback_case(
            &reopened,
            CF_MEJEPA_FAILURE_FINGERPRINTS,
            &known_bad.fingerprint_id.key_bytes(),
            &known_bad,
            "known_bad_fingerprint",
        )?,
        readback_case(
            &reopened,
            CF_MEJEPA_FINGERPRINT_REFERENCES,
            &context_graph_mejepa::fingerprint_reference_key(
                good_ref.fingerprint_id,
                &good_ref.reference_id,
            )?,
            &good_ref,
            "known_good_reference",
        )?,
        readback_case(
            &reopened,
            CF_MEJEPA_FINGERPRINT_REFERENCES,
            &context_graph_mejepa::fingerprint_reference_key(
                bad_ref.fingerprint_id,
                &bad_ref.reference_id,
            )?,
            &bad_ref,
            "known_bad_reference",
        )?,
        readback_case(
            &reopened,
            CF_MEJEPA_FINGERPRINT_CALIBRATION,
            &context_graph_mejepa::fingerprint_calibration_key(
                calibration.fingerprint_id,
                calibration.calibrated_at_unix_ms,
            )?,
            &calibration,
            "calibration",
        )?,
        readback_case(
            &reopened,
            CF_MEJEPA_FINGERPRINT_AUDIT,
            &context_graph_mejepa::fingerprint_audit_key(
                audit.fingerprint_id,
                audit.created_at_unix_ms,
                &audit.actor,
            )?,
            &audit,
            "audit",
        )?,
    ];

    let boundary_cases = boundary_cases(&known_bad, &good_ref)?;
    let cf_counts_after_reopen = counts_json(&reopened)?;
    let all_passed = readback_cases.iter().all(|case| case.readback_equal)
        && boundary_cases.iter().all(|case| case.pass)
        && count_cf(&reopened, CF_MEJEPA_FAILURE_FINGERPRINTS)? == 2
        && count_cf(&reopened, CF_MEJEPA_FINGERPRINT_REFERENCES)? == 2
        && count_cf(&reopened, CF_MEJEPA_FINGERPRINT_REVERSE_INDEX)? == 2
        && count_cf(&reopened, CF_MEJEPA_FINGERPRINT_CALIBRATION)? == 1
        && count_cf(&reopened, CF_MEJEPA_FINGERPRINT_AUDIT)? == 1;
    if !all_passed {
        bail!("failure fingerprint FSV did not pass all readback and boundary checks");
    }

    let report = FsvReport {
        fsv_root: FSV_ROOT.to_string(),
        db_path: db_path.display().to_string(),
        output_path: output_path.display().to_string(),
        started_at_unix_ms,
        expected_embedder_count: FORWARD_CACHE_EMBEDDER_COUNT,
        cf_counts_before,
        cf_counts_after_reopen,
        readback_cases,
        boundary_cases,
        all_passed,
    };
    let bytes = serde_json::to_vec_pretty(&report).context("serialize fsv report")?;
    fs::write(&output_path, &bytes).context("write fsv report")?;
    let readback = fs::read(&output_path).context("read fsv report")?;
    if readback != bytes {
        bail!("FSV report readback bytes differ from written bytes");
    }
    println!("{}", output_path.display());
    Ok(())
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

fn embedder_ids() -> Vec<EmbedderId> {
    (1..=FORWARD_CACHE_EMBEDDER_COUNT)
        .map(|idx| EmbedderId(format!("e{idx}")))
        .collect()
}

fn build_fingerprint(
    kind: FingerprintKind,
    source_corpus: &str,
    source_manifest_sha256: Option<[u8; 32]>,
    n_references: usize,
    frozen_at_unix_ms: i64,
    embedders: &[EmbedderId],
) -> Result<FailureShapeFingerprint> {
    let fingerprint_id = FailureShapeFingerprint::canonical_id(&kind, source_corpus)?;
    let name = kind.class_slug();
    let reference_chunks = (0..n_references)
        .map(|idx| ChunkId(format!("{}:reference-{idx}", fingerprint_id.hex())))
        .collect::<Vec<_>>();
    let oracle_outcome = match &kind {
        FingerprintKind::KnownGood { .. } => Some(OracleOutcome::Pass),
        FingerprintKind::KnownBad { .. } => Some(OracleOutcome::Fail),
        FingerprintKind::Unknown { .. } => None,
    };
    let mut centroid_by_embedder = BTreeMap::new();
    let mut variance_by_embedder = BTreeMap::new();
    let mut tau_by_embedder = BTreeMap::new();
    for (idx, embedder) in embedders.iter().enumerate() {
        let base = idx as f32 + 1.0;
        centroid_by_embedder.insert(
            embedder.clone(),
            vec![base / 10.0, base / 20.0, base / 30.0],
        );
        variance_by_embedder.insert(embedder.clone(), base / 1_000.0);
        tau_by_embedder.insert(embedder.clone(), 0.75 - (idx as f32 * 0.001));
    }
    let mut pair_order = embedders.to_vec();
    pair_order.sort();
    let mut pairwise_cosine = Vec::new();
    let mut pairwise_mutual_information = Vec::new();
    for left_idx in 0..pair_order.len() {
        for right_idx in (left_idx + 1)..pair_order.len() {
            pairwise_cosine.push(FingerprintPairMetric {
                left_embedder: pair_order[left_idx].clone(),
                right_embedder: pair_order[right_idx].clone(),
                value: 0.25 + (left_idx as f32 * 0.001) + (right_idx as f32 * 0.0001),
            });
            pairwise_mutual_information.push(FingerprintPairMetric {
                left_embedder: pair_order[left_idx].clone(),
                right_embedder: pair_order[right_idx].clone(),
                value: 0.01 + ((left_idx + right_idx) as f32 * 0.001),
            });
        }
    }
    let fingerprint = FailureShapeFingerprint {
        schema_version: FAILURE_FINGERPRINT_SCHEMA_VERSION,
        fingerprint_id,
        kind,
        name,
        source_corpus: source_corpus.to_string(),
        source_manifest_sha256,
        centroid_by_embedder,
        variance_by_embedder,
        tau_by_embedder,
        pairwise_cosine,
        pairwise_mutual_information,
        reference_chunks,
        n_references,
        oracle_outcome,
        is_canonical: true,
        frozen_at_unix_ms,
        confidence: FingerprintConfidence {
            classification_accuracy: Some(0.96),
            classification_precision: Some(0.97),
            unknown_recall: Some(0.91),
            calibration_observations: n_references,
            calibration_state: FingerprintCalibrationState::Calibrated,
        },
    };
    fingerprint.validate()?;
    Ok(fingerprint)
}

fn build_reference(
    fingerprint_id: FingerprintId,
    reference_id: &str,
    repo: &str,
    mutation_category: MutationCategory,
    oracle_outcome: OracleOutcome,
    chunk_id: ChunkId,
    embedders: &[EmbedderId],
) -> FingerprintReference {
    FingerprintReference {
        fingerprint_id,
        reference_id: reference_id.to_string(),
        task_id: TaskId(format!("{repo}-synthetic-task")),
        repo: repo.to_string(),
        mutation_category,
        chunk_id,
        embedder_ids: embedders.to_vec(),
        oracle_outcome,
        witness_hash: [0x33; 32],
        source_manifest_sha256: [0x5a; 32],
    }
}

fn readback_case<T>(
    db: &DB,
    cf_name: &'static str,
    key: &[u8],
    expected: &T,
    case: &'static str,
) -> Result<ReadbackCase>
where
    T: serde::de::DeserializeOwned + Serialize + PartialEq + std::fmt::Debug,
{
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| anyhow!("missing CF {cf_name}"))?;
    let bytes = db
        .get_cf(cf, key)
        .with_context(|| format!("read {cf_name}"))?
        .ok_or_else(|| anyhow!("missing row for {case} in {cf_name}"))?;
    let recovered: T = bincode::deserialize(&bytes).context("decode readback row")?;
    if &recovered != expected {
        bail!("{case} mismatch: expected {expected:?}, recovered {recovered:?}");
    }
    Ok(ReadbackCase {
        case,
        cf: cf_name,
        key_hex: hex::encode(key),
        value_bytes: bytes.len(),
        readback_equal: true,
    })
}

fn boundary_cases(
    valid: &FailureShapeFingerprint,
    good_ref: &FingerprintReference,
) -> Result<Vec<BoundaryCase>> {
    let mut empty_centroid = valid.clone();
    empty_centroid.centroid_by_embedder.clear();
    let mut bad_tau = valid.clone();
    let first_embedder = bad_tau
        .tau_by_embedder
        .keys()
        .next()
        .cloned()
        .ok_or_else(|| anyhow!("valid fingerprint missing tau rows"))?;
    bad_tau.tau_by_embedder.insert(first_embedder, 1.25);
    let unknown = FingerprintKind::Unknown {
        observed_at_unix_ms: 0,
        observed_by_session: [0; 16],
        ood_score: 0.4,
        embedder_disagreement_score: 0.2,
        active_learning_priority: 0,
    };
    let mut bad_good_ref = good_ref.clone();
    bad_good_ref.oracle_outcome = OracleOutcome::Fail;

    Ok(vec![
        expect_invalid("reject_empty_centroid", empty_centroid.validate()),
        expect_invalid("reject_tau_out_of_range", bad_tau.validate()),
        expect_invalid("reject_malformed_unknown_kind", unknown.validate()),
        expect_invalid(
            "reject_known_good_reference_with_fail",
            bad_good_ref.validate(),
        ),
    ])
}

fn expect_invalid(
    case: &'static str,
    result: Result<(), context_graph_mejepa::MejepaInferError>,
) -> BoundaryCase {
    match result {
        Ok(()) => BoundaryCase {
            case,
            error_code: "none".to_string(),
            pass: false,
        },
        Err(err) => BoundaryCase {
            case,
            error_code: err.code().to_string(),
            pass: err.code() == "MEJEPA_INFER_INVALID_INPUT",
        },
    }
}

fn counts_json(db: &DB) -> Result<serde_json::Value> {
    Ok(json!({
        CF_MEJEPA_FAILURE_FINGERPRINTS: count_cf(db, CF_MEJEPA_FAILURE_FINGERPRINTS)?,
        CF_MEJEPA_FINGERPRINT_REFERENCES: count_cf(db, CF_MEJEPA_FINGERPRINT_REFERENCES)?,
        CF_MEJEPA_FINGERPRINT_REVERSE_INDEX: count_cf(db, CF_MEJEPA_FINGERPRINT_REVERSE_INDEX)?,
        CF_MEJEPA_FINGERPRINT_CALIBRATION: count_cf(db, CF_MEJEPA_FINGERPRINT_CALIBRATION)?,
        CF_MEJEPA_FINGERPRINT_AUDIT: count_cf(db, CF_MEJEPA_FINGERPRINT_AUDIT)?,
    }))
}

fn count_cf(db: &DB, cf_name: &str) -> Result<usize> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| anyhow!("missing CF {cf_name}"))?;
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        item.with_context(|| format!("iterate {cf_name}"))?;
        count += 1;
    }
    Ok(count)
}

fn unix_ms() -> Result<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before unix epoch")?
        .as_millis()
        .try_into()
        .context("unix millis overflow i64")
}
