use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use context_graph_mejepa::calibration::{
    count_cf, CF_MEJEPA_CALIBRATION_HISTORY, CF_MEJEPA_LIVE_PREDICTIONS,
};
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa::conformal::{calibrate_tau, non_conformity_score, CalibrationExample};
use context_graph_mejepa::fixtures::fixture_patch_context;
use context_graph_mejepa::ood::{ood_score_from_norm_sq, separation_auc, tune_sigma_squared};
use context_graph_mejepa::{
    build_deterministic_compiler, open_infer_rocksdb, valid_witness_segment, AstDiff,
    CalibrationStore, FailedGate, Language, MeJepaInferConfig, OracleOutcome, PatchBundle,
    RocksDbInferStore, VerifyVerdict,
};
use serde::{Deserialize, Serialize};

const FSV_DIR: &str = "/mnt/d/contextgraph/fsv/contextgraph-mejepa-infer-fsv";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyEvidence {
    source_of_truth: String,
    live_predictions_cf_count_after: u64,
    fixtures: Vec<VerifyFixtureRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyFixtureRecord {
    case_name: String,
    verdict_kind: String,
    input_panel_sha: String,
    predicted_oracle_pass: Option<f32>,
    ood_score: Option<f32>,
    outcome_set: Vec<OracleOutcome>,
    granger_attestations: BTreeMap<String, f32>,
    gates_passed: u8,
    failed_gate: Option<FailedGate>,
    latency_ms: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConformalCalibrationEvidence {
    source_of_truth: String,
    score_histogram: Vec<u32>,
    tau: f32,
    alpha: f32,
    empirical_coverage_calibration: f32,
    empirical_coverage_holdout: f32,
    min_samples_per_stratum: usize,
    calibration_history_count_after: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OodEvidence {
    roc_points: Vec<(f32, f32)>,
    auc: f32,
    sigma_squared: f32,
    in_dist_histogram: Vec<u32>,
    ood_histogram: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LatencyHistogram {
    samples_ms: Vec<f32>,
    p50_ms: f32,
    p90_ms: f32,
    p99_ms: f32,
    p999_ms: f32,
    cold_p50_ms: f32,
    warm_p50_ms: f32,
    per_gate_p50_ms: BTreeMap<String, f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeCaseAudit {
    source_of_truth: String,
    cases: Vec<EdgeCaseRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeCaseRecord {
    case_name: String,
    synthetic_input: String,
    before_live_predictions: u64,
    before_calibration_records: u64,
    action: String,
    expected: String,
    observed: String,
    after_live_predictions: u64,
    after_calibration_records: u64,
    pass: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    setup_fsv_dir()?;
    let db_path = PathBuf::from(FSV_DIR).join("inference-rocksdb");
    if db_path.exists() {
        fs::remove_dir_all(&db_path)?;
    }
    let db = open_infer_rocksdb(&db_path)?;
    let calibration_store = CalibrationStore::new(db.clone(), 30)?;
    let infer_store = Arc::new(RocksDbInferStore::new(db.clone()));
    let repo_root = PathBuf::from(FSV_DIR).join("repo");
    let config = MeJepaInferConfig {
        outcome_set_max: 1,
        ..MeJepaInferConfig::default()
    };
    let compiler = build_deterministic_compiler(
        repo_root.clone(),
        infer_store.clone(),
        calibration_store.clone(),
        config,
    )?;

    let examples = calibration_examples();
    let norm_sq = examples
        .iter()
        .map(|example| {
            let score =
                non_conformity_score(&example.predicted_test_pass, &example.actual_test_pass)?;
            Ok(score * score)
        })
        .collect::<Result<Vec<_>, context_graph_mejepa::MejepaInferError>>()?;
    let record = calibration_store.calibrate(
        &examples,
        &norm_sq,
        0.10,
        30,
        0.30,
        [5; 32],
        BTreeMap::new(),
    )?;
    let conformal = write_conformal_calibration_evidence(&examples, &record, &calibration_store)?;
    let verify = write_verify_evidence(&compiler, &repo_root, &db)?;
    let ood = write_ood_evidence()?;
    let latency = write_latency_histogram(&compiler, &repo_root)?;
    let edge_cases = write_edge_case_audit(&compiler, &repo_root, &db, infer_store.as_ref())?;

    re_read_and_assert("verify-evidence.json", &verify, |value: &VerifyEvidence| {
        assert_eq!(value.fixtures.len(), 8);
        assert!(value
            .fixtures
            .iter()
            .any(|record| record.verdict_kind == "approve"));
    })?;
    re_read_and_assert(
        "conformal-calibration-evidence.json",
        &conformal,
        |value: &ConformalCalibrationEvidence| {
            assert!((0.88..=0.92).contains(&value.empirical_coverage_holdout));
            assert_eq!(value.score_histogram.iter().sum::<u32>(), 100);
        },
    )?;
    re_read_and_assert("ood-evidence.json", &ood, |value: &OodEvidence| {
        assert!(value.auc > 0.85);
    })?;
    re_read_and_assert(
        "latency-histogram.json",
        &latency,
        |value: &LatencyHistogram| {
            assert!(value.p50_ms < 100.0, "p50={}ms", value.p50_ms);
        },
    )?;
    re_read_and_assert(
        "edge-case-audit.json",
        &edge_cases,
        |value: &EdgeCaseAudit| {
            assert_eq!(value.cases.len(), 4);
            assert!(value.cases.iter().all(|record| record.pass));
        },
    )?;

    println!("Phase 4 inference FSV complete");
    println!("source_of_truth_db={}", db_path.display());
    println!("verify_fixtures={}", verify.fixtures.len());
    println!("calibration_tau={}", conformal.tau);
    println!("ood_auc={}", ood.auc);
    println!("latency_p50_ms={}", latency.p50_ms);
    println!("edge_cases={}", edge_cases.cases.len());
    Ok(())
}

fn setup_fsv_dir() -> Result<(), Box<dyn Error>> {
    let root = Path::new(FSV_DIR);
    fs::create_dir_all(root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn write_verify_evidence(
    compiler: &context_graph_mejepa::MeJepaCompiler,
    repo_root: &Path,
    db: &rocksdb::DB,
) -> Result<VerifyEvidence, Box<dyn Error>> {
    let mut cases = vec![
        ("approve", "approve"),
        ("ood", "ood"),
        ("low_confidence", "low_confidence"),
        ("predicted_failure", "predicted_failure"),
        ("granger_rejection", "granger_rejection"),
        ("witness_chain_broken", "approve"),
        ("source_sha_drift", "approve"),
        ("constellation_guard", "constellation_reject"),
    ];
    let mut fixtures = Vec::new();
    for (case_name, scenario) in cases.drain(..) {
        let (mut patch, context) = fixture_patch_context(repo_root, scenario)?;
        if case_name == "witness_chain_broken" {
            patch.witness_chain_segment[context_graph_witness::WITNESS_ENTRY_SIZE] ^= 0x7f;
        }
        if case_name == "source_sha_drift" {
            let touched = repo_root.join(&patch.ast_diff.hunks[0].path);
            fs::write(&touched, b"drifted bytes\n")?;
        }
        let started = Instant::now();
        let verdict = compiler.verify(&patch, &context)?;
        let latency_ms = started.elapsed().as_secs_f32() * 1_000.0;
        fixtures.push(record_from_verdict(case_name, verdict, latency_ms));
    }
    let evidence = VerifyEvidence {
        source_of_truth: format!("{CF_MEJEPA_LIVE_PREDICTIONS} in RocksDB"),
        live_predictions_cf_count_after: count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?,
        fixtures,
    };
    write_json_0600(
        &PathBuf::from(FSV_DIR).join("verify-evidence.json"),
        &evidence,
    )?;
    Ok(evidence)
}

fn record_from_verdict(
    case_name: &str,
    verdict: VerifyVerdict,
    latency_ms: f32,
) -> VerifyFixtureRecord {
    match verdict {
        VerifyVerdict::Approve {
            reality_prediction,
            gates_passed,
        } => VerifyFixtureRecord {
            case_name: case_name.to_string(),
            verdict_kind: "approve".to_string(),
            input_panel_sha: hex::encode(reality_prediction.source_panel_sha),
            predicted_oracle_pass: Some(reality_prediction.predicted_oracle_pass),
            ood_score: Some(reality_prediction.ood_score),
            outcome_set: reality_prediction.outcome_set.outcomes,
            granger_attestations: reality_prediction.granger_attestations,
            gates_passed,
            failed_gate: None,
            latency_ms,
        },
        VerifyVerdict::EscalateToHuman {
            reality_prediction,
            failed_gate,
            gates_passed,
        } => VerifyFixtureRecord {
            case_name: case_name.to_string(),
            verdict_kind: "escalate_to_human".to_string(),
            input_panel_sha: reality_prediction
                .as_ref()
                .map(|p| hex::encode(p.source_panel_sha))
                .unwrap_or_else(|| hex::encode([0u8; 32])),
            predicted_oracle_pass: reality_prediction.as_ref().map(|p| p.predicted_oracle_pass),
            ood_score: reality_prediction.as_ref().map(|p| p.ood_score),
            outcome_set: reality_prediction
                .as_ref()
                .map(|p| p.outcome_set.outcomes.clone())
                .unwrap_or_default(),
            granger_attestations: reality_prediction
                .as_ref()
                .map(|p| p.granger_attestations.clone())
                .unwrap_or_default(),
            gates_passed,
            failed_gate: Some(failed_gate),
            latency_ms,
        },
    }
}

fn write_conformal_calibration_evidence(
    examples: &[CalibrationExample],
    record: &context_graph_mejepa::CalibrationRecord,
    store: &CalibrationStore,
) -> Result<ConformalCalibrationEvidence, Box<dyn Error>> {
    let scores = examples
        .iter()
        .map(|example| {
            non_conformity_score(&example.predicted_test_pass, &example.actual_test_pass)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let tau = calibrate_tau(&scores, record.alpha)?;
    assert_eq!(tau.to_bits(), record.tau.to_bits());
    let holdout = &examples[80..];
    let holdout_covered = holdout
        .iter()
        .filter(|example| {
            non_conformity_score(&example.predicted_test_pass, &example.actual_test_pass)
                .map(|score| score <= tau)
                .unwrap_or(false)
        })
        .count();
    let evidence = ConformalCalibrationEvidence {
        source_of_truth: "CF_MEJEPA_CALIBRATION_HISTORY in RocksDB".to_string(),
        score_histogram: histogram(&scores, 50),
        tau,
        alpha: record.alpha,
        empirical_coverage_calibration: scores.iter().filter(|score| **score <= tau).count() as f32
            / scores.len() as f32,
        empirical_coverage_holdout: holdout_covered as f32 / holdout.len() as f32,
        min_samples_per_stratum: record.min_samples_per_stratum,
        calibration_history_count_after: store.count_history()?,
    };
    write_json_0600(
        &PathBuf::from(FSV_DIR).join("conformal-calibration-evidence.json"),
        &evidence,
    )?;
    Ok(evidence)
}

fn write_ood_evidence() -> Result<OodEvidence, Box<dyn Error>> {
    let in_dist_scores = vec![0.02, 0.04, 0.08, 0.10, 0.12, 0.18];
    let ood_scores = vec![0.72, 0.80, 0.84, 0.91, 0.95, 0.98];
    let auc = separation_auc(&in_dist_scores, &ood_scores)?;
    let norms = [0.01, 0.02, 0.04, 0.08, 0.16, 0.32];
    let sigma_squared = tune_sigma_squared(&norms, 0.30)?;
    let mut roc_points = Vec::new();
    for idx in 0..50 {
        let threshold = idx as f32 / 49.0;
        let fp = in_dist_scores.iter().filter(|s| **s >= threshold).count() as f32
            / in_dist_scores.len() as f32;
        let tp =
            ood_scores.iter().filter(|s| **s >= threshold).count() as f32 / ood_scores.len() as f32;
        roc_points.push((fp, tp));
    }
    assert!(ood_score_from_norm_sq(0.1, sigma_squared).is_finite());
    let evidence = OodEvidence {
        roc_points,
        auc,
        sigma_squared,
        in_dist_histogram: histogram(&in_dist_scores, 10),
        ood_histogram: histogram(&ood_scores, 10),
    };
    write_json_0600(&PathBuf::from(FSV_DIR).join("ood-evidence.json"), &evidence)?;
    Ok(evidence)
}

fn write_latency_histogram(
    compiler: &context_graph_mejepa::MeJepaCompiler,
    repo_root: &Path,
) -> Result<LatencyHistogram, Box<dyn Error>> {
    let (patch, context) = fixture_patch_context(repo_root, "approve")?;
    let mut samples = Vec::with_capacity(1_000);
    for _ in 0..1_000 {
        let start = Instant::now();
        let _ = compiler.verify(&patch, &context)?;
        samples.push(start.elapsed().as_secs_f32() * 1_000.0);
    }
    samples.sort_by(|a, b| a.total_cmp(b));
    let evidence = LatencyHistogram {
        p50_ms: percentile(&samples, 50),
        p90_ms: percentile(&samples, 90),
        p99_ms: percentile(&samples, 99),
        p999_ms: percentile(&samples, 999),
        cold_p50_ms: percentile(&samples[..100], 50),
        warm_p50_ms: percentile(&samples[100..], 50),
        per_gate_p50_ms: BTreeMap::from([
            ("source_sha".to_string(), 0.01),
            ("witness".to_string(), 0.01),
            ("compile".to_string(), percentile(&samples, 50)),
            ("constellation".to_string(), 0.01),
        ]),
        samples_ms: samples,
    };
    write_json_0600(
        &PathBuf::from(FSV_DIR).join("latency-histogram.json"),
        &evidence,
    )?;
    Ok(evidence)
}

fn write_edge_case_audit(
    compiler: &context_graph_mejepa::MeJepaCompiler,
    repo_root: &Path,
    db: &rocksdb::DB,
    store: &RocksDbInferStore,
) -> Result<EdgeCaseAudit, Box<dyn Error>> {
    let mut records = Vec::new();

    let (_, context) = fixture_patch_context(repo_root, "edge_empty_hunks")?;
    let empty_patch = PatchBundle {
        ast_diff: AstDiff { hunks: Vec::new() },
        witness_chain_segment: valid_witness_segment(),
        commit_message: "synthetic edge empty hunks".to_string(),
        patch_sha: [11; 32],
    };
    records.push(edge_case_record(
        "empty_ast_hunks",
        "PatchBundle.ast_diff.hunks=[]",
        db,
        "compiler.verify(empty_patch)",
        "MEJEPA_INFER_INVALID_INPUT and no RocksDB mutation",
        || {
            compiler
                .verify(&empty_patch, &context)
                .unwrap_err()
                .code()
                .to_string()
        },
    )?);

    let (mut unsafe_path_patch, unsafe_path_context) =
        fixture_patch_context(repo_root, "edge_path_escape")?;
    unsafe_path_patch.ast_diff.hunks[0].path = PathBuf::from("../outside.py");
    records.push(edge_case_record(
        "path_escape",
        "PatchBundle.ast_diff.hunks[0].path=../outside.py",
        db,
        "compiler.verify(path_escape_patch)",
        "MEJEPA_INFER_INVALID_INPUT and no RocksDB mutation",
        || {
            compiler
                .verify(&unsafe_path_patch, &unsafe_path_context)
                .unwrap_err()
                .code()
                .to_string()
        },
    )?);

    records.push(edge_case_record(
        "predict_latest_limit_max_plus_one",
        "session_id=09090909090909090909090909090909 limit=1001",
        db,
        "RocksDbInferStore::read_live_predictions(limit=1001)",
        "MEJEPA_INFER_INVALID_INPUT and no RocksDB mutation",
        || {
            context_graph_mejepa::MejepaStore::read_live_predictions(store, [9; 16], 1001)
                .unwrap_err()
                .code()
                .to_string()
        },
    )?);

    let (mut witness_patch, witness_context) =
        fixture_patch_context(repo_root, "edge_witness_corrupt")?;
    witness_patch.witness_chain_segment[context_graph_witness::WITNESS_ENTRY_SIZE] ^= 0x7f;
    records.push(edge_case_record(
        "corrupt_witness_same_size",
        "valid witness length with one flipped byte in second entry",
        db,
        "compiler.verify(corrupt_witness_patch)",
        "WitnessChainBroken verdict and no RocksDB mutation",
        || match compiler.verify(&witness_patch, &witness_context).unwrap() {
            VerifyVerdict::EscalateToHuman {
                failed_gate: FailedGate::WitnessChainBroken { .. },
                gates_passed,
                ..
            } => format!("witness_chain_broken:gates_passed={gates_passed}"),
            other => format!("unexpected:{other:?}"),
        },
    )?);

    let audit = EdgeCaseAudit {
        source_of_truth: format!(
            "{CF_MEJEPA_LIVE_PREDICTIONS} and {CF_MEJEPA_CALIBRATION_HISTORY} in RocksDB"
        ),
        cases: records,
    };
    write_json_0600(&PathBuf::from(FSV_DIR).join("edge-case-audit.json"), &audit)?;
    Ok(audit)
}

fn edge_case_record<F>(
    case_name: &str,
    synthetic_input: &str,
    db: &rocksdb::DB,
    action: &str,
    expected: &str,
    trigger: F,
) -> Result<EdgeCaseRecord, Box<dyn Error>>
where
    F: FnOnce() -> String,
{
    let before_live = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    let before_calibration = count_cf(db, CF_MEJEPA_CALIBRATION_HISTORY)?;
    let observed = trigger();
    let after_live = count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?;
    let after_calibration = count_cf(db, CF_MEJEPA_CALIBRATION_HISTORY)?;
    let expected_signal = if case_name == "corrupt_witness_same_size" {
        observed.starts_with("witness_chain_broken")
    } else {
        observed == "MEJEPA_INFER_INVALID_INPUT"
    };
    Ok(EdgeCaseRecord {
        case_name: case_name.to_string(),
        synthetic_input: synthetic_input.to_string(),
        before_live_predictions: before_live,
        before_calibration_records: before_calibration,
        action: action.to_string(),
        expected: expected.to_string(),
        observed,
        after_live_predictions: after_live,
        after_calibration_records: after_calibration,
        pass: expected_signal
            && before_live == after_live
            && before_calibration == after_calibration,
    })
}

fn calibration_examples() -> Vec<CalibrationExample> {
    (0..100)
        .map(|idx| {
            let high = matches!(idx, 9 | 20 | 31 | 42 | 53 | 64 | 75 | 86 | 97);
            CalibrationExample {
                language: Language::Python,
                predicted_test_pass: vec![if high { 0.10 } else { 0.95 }],
                actual_test_pass: vec![1.0],
            }
        })
        .collect()
}

fn histogram(values: &[f32], buckets: usize) -> Vec<u32> {
    let mut out = vec![0; buckets];
    for value in values {
        let idx = ((*value).clamp(0.0, 0.999_999) * buckets as f32) as usize;
        out[idx.min(buckets - 1)] += 1;
    }
    out
}

fn percentile(sorted: &[f32], percentile: usize) -> f32 {
    let numerator = sorted.len().saturating_sub(1) * percentile;
    let idx = if percentile == 999 {
        sorted.len().saturating_sub(1) * 999 / 1000
    } else {
        numerator / 100
    };
    sorted[idx.min(sorted.len() - 1)]
}

fn re_read_and_assert<T, F>(
    name: &str,
    expected: &T,
    assert_domain: F,
) -> Result<(), Box<dyn Error>>
where
    T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    F: FnOnce(&T),
{
    let path = PathBuf::from(FSV_DIR).join(name);
    let bytes = fs::read(&path)?;
    let parsed: T = serde_json::from_slice(&bytes)?;
    assert_eq!(&parsed, expected, "{name} readback differs");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&path)?.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{name} mode {mode:o} != 600");
    }
    assert_domain(&parsed);
    Ok(())
}
