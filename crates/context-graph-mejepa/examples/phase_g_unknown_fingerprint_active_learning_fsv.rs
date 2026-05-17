//! TASK-FP-005 Unknown/OOD fingerprint active-learning FSV.
//!
//! Proves that a fingerprint classifier OOD/no-known-match decision becomes a
//! durable active-learning queue entry and that similar Unknown candidates
//! produce a deterministic new-fingerprint cluster suggestion.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use context_graph_mejepa::{
    ActiveLearningKind, ActiveLearningQueueState, ConformalSet, EmbedderId,
    FingerprintCalibrationState, FingerprintCandidateScore, FingerprintClassification,
    FingerprintDecisionReason, FingerprintEmbedderScore, FingerprintId, FingerprintKind, Language,
    OracleOutcome, RealityPrediction, RealityPredictionBuilder, RocksDbEvalStore, TaskId,
    UnknownFingerprintClusterSuggestion, Verdict,
};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde::Serialize;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-g-unknown-fingerprint-active-learning-fsv";

#[derive(Debug, Serialize)]
struct BoundaryCase {
    case: &'static str,
    observed: String,
    pass: bool,
}

#[derive(Debug, Serialize)]
struct FsvReport {
    fsv_root: String,
    db_path: String,
    output_path: String,
    started_at_unix_ms: i64,
    queue_entries_before_persist: usize,
    queue_entries_after_reopen: usize,
    unknown_candidate_count_after_reopen: usize,
    cluster_suggestions: Vec<UnknownFingerprintClusterSuggestion>,
    boundary_cases: Vec<BoundaryCase>,
    all_passed: bool,
}

fn main() -> Result<()> {
    let started_at_unix_ms = unix_ms()?;
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    fs::create_dir_all(&run_root).context("create FSV root")?;
    let db_path = run_root.join("rocks");
    let output_path = run_root.join("unknown_fingerprint_active_learning_fsv.json");

    let db = open_full_db(&db_path)?;
    let store = RocksDbEvalStore::new(db.clone()).context("open eval store")?;
    let mut queue = ActiveLearningQueueState::new(8).context("queue")?;

    for (idx, vector) in [vec![1.0, 0.0], vec![0.999, 0.010], vec![0.998, 0.020]]
        .into_iter()
        .enumerate()
    {
        let prediction = prediction(&format!("unknown-fp-task-{idx}"), 0x30 + idx as u8, 0.91);
        queue
            .enqueue_unknown_fingerprint_candidate(
                &prediction,
                &ood_classification(),
                vectors([("e1", vector), ("e2", vec![0.0, 1.0])]),
                started_at_unix_ms + idx as i64,
            )
            .context("enqueue unknown fingerprint")?;
    }
    let queue_entries_before_persist = queue.entries.len();
    store.persist_queue(&queue).context("persist queue")?;
    drop(store);
    drop(db);

    let reopened = open_full_db(&db_path)?;
    let reopened_store = RocksDbEvalStore::new(reopened).context("open reopened eval store")?;
    let reopened_queue = reopened_store
        .load_queue()
        .context("load reopened queue")?
        .context("queue missing after persist")?;
    let queue_entries_after_reopen = reopened_queue.entries.len();
    let unknown_candidate_count_after_reopen = reopened_queue
        .entries
        .values()
        .filter(|entry| matches!(entry.kind, ActiveLearningKind::UnknownFingerprint { .. }))
        .count();
    let cluster_suggestions = reopened_queue
        .suggest_unknown_fingerprint_clusters(3, 0.98)
        .context("suggest unknown clusters")?;
    for suggestion in &cluster_suggestions {
        suggestion
            .validate()
            .context("cluster suggestion validates")?;
    }

    let boundary_cases = boundary_cases(started_at_unix_ms)?;
    let all_passed = queue_entries_before_persist == 3
        && queue_entries_after_reopen == 3
        && unknown_candidate_count_after_reopen == 3
        && cluster_suggestions.len() == 1
        && cluster_suggestions[0].member_candidate_ids.len() == 3
        && boundary_cases.iter().all(|case| case.pass);
    if !all_passed {
        bail!("unknown fingerprint active-learning FSV did not pass all checks");
    }

    let report = FsvReport {
        fsv_root: FSV_ROOT.to_string(),
        db_path: db_path.display().to_string(),
        output_path: output_path.display().to_string(),
        started_at_unix_ms,
        queue_entries_before_persist,
        queue_entries_after_reopen,
        unknown_candidate_count_after_reopen,
        cluster_suggestions,
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

fn boundary_cases(started_at_unix_ms: i64) -> Result<Vec<BoundaryCase>> {
    let mut queue = ActiveLearningQueueState::new(2).context("queue")?;
    let known_good_prediction = prediction("known-good-boundary", 0x40, 0.10);
    let non_ood = queue
        .enqueue_unknown_fingerprint_candidate(
            &known_good_prediction,
            &known_good_classification(),
            vectors([("e1", vec![1.0, 0.0])]),
            started_at_unix_ms,
        )
        .context("non-ood enqueue")?;

    let empty_observation = queue.enqueue_unknown_fingerprint_candidate(
        &prediction("empty-observation-boundary", 0x41, 0.91),
        &ood_classification(),
        BTreeMap::new(),
        started_at_unix_ms,
    );
    let bad_min_cluster = queue.suggest_unknown_fingerprint_clusters(0, 0.98);
    let bad_threshold = queue.suggest_unknown_fingerprint_clusters(2, 1.5);
    let empty_observation_code = error_code(empty_observation);
    let bad_min_cluster_code = error_code(bad_min_cluster);
    let bad_threshold_code = error_code(bad_threshold);

    Ok(vec![
        BoundaryCase {
            case: "non_ood_classification_not_enqueued",
            observed: non_ood.to_string(),
            pass: !non_ood && queue.entries.is_empty(),
        },
        BoundaryCase {
            case: "empty_observation_fails_closed",
            observed: empty_observation_code.clone(),
            pass: empty_observation_code == "MEJEPA_EVAL_INVALID_INPUT",
        },
        BoundaryCase {
            case: "zero_min_cluster_fails_closed",
            observed: bad_min_cluster_code.clone(),
            pass: bad_min_cluster_code == "MEJEPA_EVAL_INVALID_CONFIG",
        },
        BoundaryCase {
            case: "bad_threshold_fails_closed",
            observed: bad_threshold_code.clone(),
            pass: bad_threshold_code == "MEJEPA_EVAL_INVALID_INPUT",
        },
    ])
}

fn error_code<T>(result: std::result::Result<T, context_graph_mejepa::EvalError>) -> String {
    match result {
        Ok(_) => "unexpected_ok".to_string(),
        Err(err) => err.code().to_string(),
    }
}

fn prediction(task_id: &str, prediction_id: u8, ood_score: f32) -> RealityPrediction {
    let outcome_set = ConformalSet::try_new(vec![OracleOutcome::OutOfDistribution], 0.1, 0.5)
        .expect("outcome set");
    RealityPredictionBuilder::from_parts(
        TaskId(task_id.to_string()),
        [0x7a; 16],
        Language::Python,
        outcome_set,
    )
    .prediction_id([prediction_id; 16])
    .verdict(Verdict::OutOfDistribution)
    .predicted_oracle_pass(0.5)
    .predicted_test_pass(vec![0.5])
    .ood_score(ood_score)
    .calibrated_confidence(0.5)
    .calibration_version("unknown-fingerprint-fsv")
    .build()
    .expect("prediction")
}

fn ood_classification() -> FingerprintClassification {
    let classification = FingerprintClassification {
        verdict: Verdict::OutOfDistribution,
        reason: FingerprintDecisionReason::NoKnownMatch,
        primary_match: None,
        ranked_matches: vec![candidate_score(false, 0x77, -0.40)],
        matched_known_good_count: 0,
        matched_known_bad_count: 0,
        matched_unknown_count: 0,
        scored_fingerprint_count: 1,
    };
    classification.validate().expect("ood classification");
    classification
}

fn known_good_classification() -> FingerprintClassification {
    let primary = candidate_score(true, 0x78, 0.10);
    let classification = FingerprintClassification {
        verdict: Verdict::Pass,
        reason: FingerprintDecisionReason::KnownGoodOnly,
        primary_match: Some(primary.clone()),
        ranked_matches: vec![primary],
        matched_known_good_count: 1,
        matched_known_bad_count: 0,
        matched_unknown_count: 0,
        scored_fingerprint_count: 1,
    };
    classification
        .validate()
        .expect("known-good classification");
    classification
}

fn candidate_score(matched: bool, id: u8, min_margin: f32) -> FingerprintCandidateScore {
    let kind = if matched {
        FingerprintKind::KnownGood {
            repo: Some("django__django".to_string()),
            gold_patch_count: 2,
        }
    } else {
        FingerprintKind::KnownBad {
            repo: "django__django".to_string(),
            mutation_category: context_graph_mejepa::MutationCategory::OffByOne,
            failure_mode: context_graph_mejepa::FailureModeClass::OffByOne,
            exception_class: None,
        }
    };
    let score = FingerprintCandidateScore {
        fingerprint_id: FingerprintId([id; 32]),
        name: format!("candidate-{id}"),
        kind,
        oracle_outcome: if matched {
            Some(OracleOutcome::Pass)
        } else {
            Some(OracleOutcome::Fail)
        },
        matched,
        mean_cosine: if matched { 0.95 } else { 0.50 },
        min_margin,
        embedder_scores: vec![
            FingerprintEmbedderScore {
                embedder: EmbedderId("e1".to_string()),
                cosine: if matched { 0.95 } else { 0.20 },
                tau: 0.90,
                margin: if matched { 0.05 } else { -0.70 },
                matched,
            },
            FingerprintEmbedderScore {
                embedder: EmbedderId("e2".to_string()),
                cosine: if matched { 0.95 } else { 0.80 },
                tau: 0.90,
                margin: if matched { 0.05 } else { -0.10 },
                matched,
            },
        ],
        n_references: 1,
        calibration_state: FingerprintCalibrationState::Calibrated,
    };
    score.validate().expect("candidate score");
    score
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
