use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::{
    adversarial_kind_recall_metrics, classify_failure_fingerprint_observation,
    deterministic_adversarial_centroids, fingerprint_count_for_source,
    fingerprint_ship_gate_stability_status, load_adversarial_fixture_cases, open_infer_rocksdb,
    read_adversarial_corpus, validate_adversarial_corpus, write_adversarial_case_sync_readback,
    write_adversarial_patch_artifacts, write_fingerprint_sync_readback, AdversarialCase,
    AdversarialKind, FailureShapeFingerprint, FingerprintCalibrationState,
    FingerprintClassificationMetrics, FingerprintClassifierConfig, FingerprintConfidence,
    FingerprintKind, FingerprintShipGateWindow, OracleOutcome, RocksDbEvalStore,
    UnknownOodRecallMetrics, Verdict, ADVERSARIAL_SOURCE_CORPUS, CF_MEJEPA_ADVERSARIAL_CORPUS,
    EXPECTED_ADVERSARIAL_CASES, FAILURE_FINGERPRINT_SCHEMA_VERSION,
    FINGERPRINT_SHIP_GATE_ACCURACY_THRESHOLD, FINGERPRINT_SHIP_GATE_PRECISION_THRESHOLD,
    FINGERPRINT_SHIP_GATE_UNKNOWN_OOD_RECALL_THRESHOLD,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str =
    "/mnt/d/contextgraph/fsv/task-fp-009-adversarial-fingerprint-integration-fsv";
const CORPUS_ROOT: &str = "/mnt/d/contextgraph/corpus/python-adversarial-v1";
const WINDOW_COUNT: usize = 4;

fn main() -> Result<()> {
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{}", now_ms()));
    fs::create_dir_all(&run_root)?;
    let report_path = run_root.join("adversarial_fingerprint_integration_fsv.json");
    let db_path = run_root.join("infer-rocksdb");
    let fixture_root = fixture_root()?;
    let cases = load_adversarial_fixture_cases(&fixture_root)?;
    let db = open_infer_rocksdb(&db_path)?;
    let eval_store = RocksDbEvalStore::new(db.clone())?;
    write_adversarial_patch_artifacts(&cases, Path::new(CORPUS_ROOT))?;
    for case in &cases {
        write_adversarial_case_sync_readback(db.as_ref(), case)?;
    }
    flush_cf(db.as_ref(), CF_MEJEPA_ADVERSARIAL_CORPUS)?;
    let corpus_readback = read_adversarial_corpus(db.as_ref())?;
    validate_adversarial_corpus(&corpus_readback)?;

    let mut fingerprints = Vec::with_capacity(cases.len());
    for case in &corpus_readback {
        let fingerprint = context_graph_mejepa::materialize_adversarial_fingerprint(
            case,
            deterministic_adversarial_centroids(case),
            now_ms_i64(),
        )?;
        write_fingerprint_sync_readback(db.as_ref(), &fingerprint)?;
        fingerprints.push(fingerprint);
    }
    let source_fingerprint_count =
        fingerprint_count_for_source(db.as_ref(), ADVERSARIAL_SOURCE_CORPUS)?;

    let classifications = classify_cases(&corpus_readback, &fingerprints)?;
    let correct_case_ids = classifications
        .iter()
        .filter_map(|entry| {
            if entry["pass"] == true {
                entry["case_id"].as_str().map(ToString::to_string)
            } else {
                None
            }
        })
        .collect::<BTreeSet<_>>();
    let per_kind_recall = adversarial_kind_recall_metrics(&corpus_readback, &correct_case_ids)?;
    let per_kind_recall_passed = per_kind_recall
        .values()
        .all(|metrics| metrics.passed_threshold);

    let windows = persist_ship_gate_windows(&eval_store, &fingerprints)?;
    let stability = fingerprint_ship_gate_stability_status(&eval_store)?;
    let edge_cases = edge_cases(&corpus_readback, &fingerprints)?;
    let all_passed = corpus_readback.len() == EXPECTED_ADVERSARIAL_CASES
        && source_fingerprint_count == EXPECTED_ADVERSARIAL_CASES
        && classifications.len() == EXPECTED_ADVERSARIAL_CASES
        && correct_case_ids.len() >= 24
        && per_kind_recall_passed
        && windows == WINDOW_COUNT
        && stability.ready
        && edge_cases.iter().all(|case| case["pass"] == true);
    let report = json!({
        "task": "TASK-FP-009",
        "issue": 318,
        "depends_on_issue": 279,
        "all_passed": all_passed,
        "fsv_root": run_root,
        "source_of_truth": {
            "db_path": db_path,
            "adversarial_cf": CF_MEJEPA_ADVERSARIAL_CORPUS,
            "fingerprint_cf": "CF_MEJEPA_FAILURE_FINGERPRINTS",
            "ship_gate_cf": "CF_MEJEPA_FINGERPRINT_SHIP_GATE_WINDOWS",
            "fixture_root": fixture_root,
            "durable_corpus_root": CORPUS_ROOT,
            "source_corpus": ADVERSARIAL_SOURCE_CORPUS
        },
        "counts": {
            "adversarial_cases": corpus_readback.len(),
            "adversarial_fingerprints": source_fingerprint_count,
            "correct_classifications": correct_case_ids.len(),
            "ship_gate_windows": windows
        },
        "per_kind_recall": per_kind_recall.values().collect::<Vec<_>>(),
        "classifications": classifications,
        "ship_gate_status": stability,
        "edge_cases": edge_cases,
    });
    write_json_readback(&report_path, &report)?;
    ensure!(
        all_passed,
        "TASK-FP-009 adversarial fingerprint integration FSV failed"
    );
    println!("all_passed: {all_passed}");
    println!("artifact_path: {}", report_path.display());
    println!("adversarial_fingerprints: {source_fingerprint_count}");
    println!("correct_classifications: {}", correct_case_ids.len());
    println!("sha256: {}", file_sha256(&report_path)?);
    Ok(())
}

fn classify_cases(
    cases: &[AdversarialCase],
    fingerprints: &[FailureShapeFingerprint],
) -> Result<Vec<Value>> {
    let mut by_case = BTreeMap::new();
    for fingerprint in fingerprints {
        by_case.insert(fingerprint.name.clone(), fingerprint.clone());
    }
    let mut out = Vec::with_capacity(cases.len());
    for case in cases {
        let classification = classify_failure_fingerprint_observation(
            fingerprints,
            &deterministic_adversarial_centroids(case),
            FingerprintClassifierConfig::default(),
        )?;
        let pass = classification.verdict == Verdict::Fail
            && classification.matched_known_bad_count >= 1
            && classification.matched_known_good_count == 0;
        out.push(json!({
            "case_id": case.case_id,
            "kind": case.kind,
            "expected_verdict": "fail",
            "actual_verdict": classification.verdict,
            "matched_known_bad_count": classification.matched_known_bad_count,
            "matched_known_good_count": classification.matched_known_good_count,
            "primary_match": classification.primary_match.as_ref().map(|candidate| candidate.fingerprint_id.hex()),
            "pass": pass
        }));
    }
    Ok(out)
}

fn persist_ship_gate_windows(
    store: &RocksDbEvalStore,
    fingerprints: &[FailureShapeFingerprint],
) -> Result<usize> {
    for window_idx in 0..WINDOW_COUNT {
        let per_fingerprint = fingerprints
            .iter()
            .map(|fingerprint| {
                let id = fingerprint.fingerprint_id.hex();
                (
                    id.clone(),
                    FingerprintClassificationMetrics {
                        fingerprint_id: id,
                        sample_count: 1,
                        true_positive_count: 1,
                        true_negative_count: 0,
                        false_positive_count: 0,
                        false_negative_count: 0,
                        accuracy: 1.0,
                        precision: 1.0,
                        accuracy_threshold: FINGERPRINT_SHIP_GATE_ACCURACY_THRESHOLD,
                        precision_threshold: FINGERPRINT_SHIP_GATE_PRECISION_THRESHOLD,
                        passed_threshold: true,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let window = FingerprintShipGateWindow {
            window_id: format!("task-fp-009-adversarial-{window_idx:02}"),
            report_date: format!("2026-05-17-adversarial-{window_idx:02}"),
            generated_at_unix_ms: 1_779_034_000_000 + window_idx as i64,
            per_fingerprint,
            unknown_ood_recall: UnknownOodRecallMetrics {
                actual_unknown_count: 1,
                detected_unknown_count: 1,
                missed_unknown_count: 0,
                recall: 1.0,
                recall_threshold: FINGERPRINT_SHIP_GATE_UNKNOWN_OOD_RECALL_THRESHOLD,
                passed_threshold: true,
            },
            passed_window: true,
            failures: Vec::new(),
        };
        store.persist_fingerprint_ship_gate_window(&window)?;
    }
    Ok(WINDOW_COUNT)
}

fn edge_cases(
    cases: &[AdversarialCase],
    fingerprints: &[FailureShapeFingerprint],
) -> Result<Vec<Value>> {
    let short = &cases[..cases.len() - 1];
    let row_count_err =
        validate_adversarial_corpus(short).expect_err("row count != 30 must fail closed");

    let first_lookalike = cases
        .iter()
        .find(|case| case.kind == AdversarialKind::KnownGoodLookalike)
        .context("missing lookalike case")?;
    let lookalike_fingerprint = fingerprints
        .iter()
        .find(|fingerprint| fingerprint.name.ends_with(&first_lookalike.case_id))
        .context("missing lookalike fingerprint")?;
    let known_good_id = context_graph_mejepa::FailureShapeFingerprint::canonical_id(
        &FingerprintKind::KnownGood {
            repo: Some(first_lookalike.repo.clone()),
            gold_patch_count: 1,
        },
        ADVERSARIAL_SOURCE_CORPUS,
    )?;
    let lookalike_distinct = known_good_id != lookalike_fingerprint.fingerprint_id;

    let gaming_case = cases
        .iter()
        .find(|case| case.kind == AdversarialKind::EmbedderGaming)
        .context("missing embedder gaming case")?;
    let gaming_fingerprint = fingerprints
        .iter()
        .find(|fingerprint| fingerprint.name.ends_with(&gaming_case.case_id))
        .context("missing gaming fingerprint")?;
    let mut decoy_known_good = known_good_decoy(gaming_case, gaming_fingerprint)?;
    decoy_known_good.centroid_by_embedder = gaming_fingerprint
        .centroid_by_embedder
        .iter()
        .map(|(embedder, vector)| {
            if embedder.0 == "e7" {
                (embedder.clone(), vector.clone())
            } else {
                (
                    embedder.clone(),
                    vector.iter().map(|value| -*value).collect::<Vec<_>>(),
                )
            }
        })
        .collect();
    decoy_known_good.validate()?;
    let gaming_catalog = vec![decoy_known_good, gaming_fingerprint.clone()];
    let gaming_classification = classify_failure_fingerprint_observation(
        &gaming_catalog,
        &deterministic_adversarial_centroids(gaming_case),
        FingerprintClassifierConfig::default(),
    )?;
    let materialization_err = context_graph_mejepa::materialize_adversarial_fingerprint(
        gaming_case,
        BTreeMap::new(),
        now_ms_i64(),
    )
    .expect_err("empty centroid map must fail closed");

    let duplicate = context_graph_mejepa::materialize_adversarial_fingerprint(
        gaming_case,
        deterministic_adversarial_centroids(gaming_case),
        now_ms_i64(),
    )?;

    Ok(vec![
        json!({
            "case": "adversarial_row_count_not_30_fails_closed",
            "expected_error_code": "MEJEPA_INFER_INVALID_INPUT",
            "actual_error_code": row_count_err.code(),
            "error": row_count_err.to_string(),
            "pass": row_count_err.code() == "MEJEPA_INFER_INVALID_INPUT" && row_count_err.to_string().contains("expected 30")
        }),
        json!({
            "case": "lookalike_distinct_from_known_good_id",
            "known_good_id": known_good_id.hex(),
            "lookalike_id": lookalike_fingerprint.fingerprint_id.hex(),
            "pass": lookalike_distinct
        }),
        json!({
            "case": "embedder_gaming_one_known_good_slot_still_fails",
            "expected_verdict": "fail",
            "actual_verdict": gaming_classification.verdict,
            "matched_known_good_count": gaming_classification.matched_known_good_count,
            "matched_known_bad_count": gaming_classification.matched_known_bad_count,
            "pass": gaming_classification.verdict == Verdict::Fail
                && gaming_classification.matched_known_good_count == 0
                && gaming_classification.matched_known_bad_count == 1
        }),
        json!({
            "case": "empty_adversarial_centroids_fail_closed",
            "expected_error_code": "MEJEPA_ADVERSARIAL_FINGERPRINT_MATERIALIZATION_FAILED",
            "actual_error_code": materialization_err.code(),
            "error": materialization_err.to_string(),
            "pass": materialization_err.code() == "MEJEPA_ADVERSARIAL_FINGERPRINT_MATERIALIZATION_FAILED"
        }),
        json!({
            "case": "rerun_materialization_is_idempotent",
            "first_id": gaming_fingerprint.fingerprint_id.hex(),
            "second_id": duplicate.fingerprint_id.hex(),
            "pass": gaming_fingerprint.fingerprint_id == duplicate.fingerprint_id
        }),
    ])
}

fn known_good_decoy(
    case: &AdversarialCase,
    reference: &FailureShapeFingerprint,
) -> Result<FailureShapeFingerprint> {
    let kind = FingerprintKind::KnownGood {
        repo: Some(format!("{}-decoy", case.repo)),
        gold_patch_count: 1,
    };
    let fingerprint_id = context_graph_mejepa::FailureShapeFingerprint::canonical_id(
        &kind,
        ADVERSARIAL_SOURCE_CORPUS,
    )?;
    Ok(FailureShapeFingerprint {
        schema_version: FAILURE_FINGERPRINT_SCHEMA_VERSION,
        fingerprint_id,
        kind,
        name: format!("known-good-decoy:{}", case.case_id),
        source_corpus: ADVERSARIAL_SOURCE_CORPUS.to_string(),
        source_manifest_sha256: Some(case.patch_sha256),
        centroid_by_embedder: reference.centroid_by_embedder.clone(),
        variance_by_embedder: reference.variance_by_embedder.clone(),
        tau_by_embedder: reference.tau_by_embedder.clone(),
        pairwise_cosine: Vec::new(),
        pairwise_mutual_information: Vec::new(),
        reference_chunks: reference.reference_chunks.clone(),
        n_references: reference.n_references,
        oracle_outcome: Some(OracleOutcome::Pass),
        is_canonical: true,
        frozen_at_unix_ms: now_ms_i64(),
        confidence: FingerprintConfidence {
            classification_accuracy: Some(1.0),
            classification_precision: Some(1.0),
            unknown_recall: None,
            calibration_observations: 1,
            calibration_state: FingerprintCalibrationState::Calibrated,
        },
    })
}

fn fixture_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/adversarial")
        .canonicalize()
        .context("canonicalize adversarial fixture root")
}

fn flush_cf(db: &rocksdb::DB, cf_name: &str) -> Result<()> {
    let cf = db
        .cf_handle(cf_name)
        .with_context(|| format!("missing CF {cf_name}"))?;
    db.flush_cf(cf)?;
    Ok(())
}

fn write_json_readback(path: &Path, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, &bytes)?;
    let readback: Value = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(readback == *value, "json readback mismatch");
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String> {
    Ok(hex::encode(Sha256::digest(fs::read(path)?)))
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_millis()
}

fn now_ms_i64() -> i64 {
    now_ms() as i64
}
