//! TASK-RWD-211 / TASK-PY-G-067 novel-pattern ontology-growth FSV.
//!
//! Proves the complete source-of-truth lifecycle for novel-pattern rows:
//! active-learning candidate queue, cluster admission, held-out acceptance,
//! held-out falsification rejection, and TASK-EK-002 proposal-surface input.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use context_graph_mejepa::{
    candidate_audit_entry, evaluate_novel_pattern_promotion, proposal_inputs_from_ontology_audit,
    write_fingerprint_sync_readback, ActiveLearningKind, ActiveLearningQueueState, ConformalSet,
    ConstellationCentroid, EmbedderId, HeldoutImprovementEvidence, Language,
    NovelInstrumentProposalInput, NovelPatternClusterAdmission, NovelPatternDetectorConfig,
    OntologyGrowthAuditEntry, OntologyGrowthAuditOutcome, OracleOutcome, RealityPrediction,
    RealityPredictionBuilder, RocksDbEvalStore, TaskId, Verdict,
};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use serde::Serialize;

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-py-g-067-novel-pattern-fsv";

#[derive(Debug, Serialize)]
struct BoundaryCase {
    case: &'static str,
    observed: String,
    pass: bool,
}

#[derive(Debug, Serialize)]
struct SourceOfTruthCounts {
    active_learning_queue_cf_rows: usize,
    ontology_growth_audit_cf_rows: usize,
    failure_fingerprint_cf_rows: usize,
}

#[derive(Debug, Serialize)]
struct FsvReport {
    fsv_root: String,
    db_path: String,
    output_path: String,
    started_at_unix_ms: i64,
    queue_entries_before_persist: usize,
    queue_entries_after_reopen: usize,
    novel_candidates_after_reopen: usize,
    candidate_admissions: Vec<NovelPatternClusterAdmission>,
    audit_entries_after_reopen: Vec<OntologyGrowthAuditEntry>,
    proposal_inputs: Vec<NovelInstrumentProposalInput>,
    source_of_truth_counts: SourceOfTruthCounts,
    boundary_cases: Vec<BoundaryCase>,
    all_passed: bool,
}

fn main() -> Result<()> {
    let started_at_unix_ms = unix_ms()?;
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    fs::create_dir_all(&run_root).context("create FSV root")?;
    let db_path = run_root.join("rocks");
    let output_path = run_root.join("novel_pattern_fsv.json");

    let config = NovelPatternDetectorConfig::default();
    let existing_cells = existing_constellation_cells();
    let db = open_full_db(&db_path)?;
    let store = RocksDbEvalStore::new(db.clone()).context("open eval store")?;
    let mut queue = ActiveLearningQueueState::new(8).context("queue")?;

    for idx in 0..3 {
        let prediction = prediction(&format!("novel-pattern-task-{idx}"), 0x40 + idx as u8);
        let enqueued = queue
            .enqueue_novel_pattern_candidate(
                &prediction,
                novel_observation(),
                &existing_cells,
                started_at_unix_ms + idx as i64,
                config,
            )
            .context("enqueue novel-pattern candidate")?;
        if !enqueued {
            bail!("synthetic novel pattern was not enqueued");
        }
    }
    let queue_entries_before_persist = queue.entries.len();
    store.persist_queue(&queue).context("persist queue")?;
    drop(store);
    drop(db);

    let reopened = open_full_db(&db_path)?;
    let reopened_store = RocksDbEvalStore::new(reopened.clone()).context("reopen eval store")?;
    let reopened_queue = reopened_store
        .load_queue()
        .context("load active-learning queue")?
        .context("active-learning queue missing after persist")?;
    let queue_entries_after_reopen = reopened_queue.entries.len();
    let novel_candidates_after_reopen = reopened_queue
        .entries
        .values()
        .filter(|entry| matches!(entry.kind, ActiveLearningKind::NovelCluster { .. }))
        .count();
    let candidate_admissions = reopened_queue
        .admit_novel_pattern_clusters(&existing_cells, config)
        .context("admit novel-pattern cluster")?;
    let admission = candidate_admissions
        .first()
        .context("missing novel-pattern candidate admission")?;

    let candidate_entry =
        candidate_audit_entry(admission, started_at_unix_ms + 100, config).context("candidate")?;
    reopened_store
        .persist_ontology_growth_audit(&candidate_entry)
        .context("persist candidate audit")?;
    let accepted_entry = evaluate_novel_pattern_promotion(
        admission,
        &accepted_heldout(),
        started_at_unix_ms + 200,
        config,
    )
    .context("accepted held-out gate")?;
    reopened_store
        .persist_ontology_growth_audit(&accepted_entry)
        .context("persist accepted audit")?;
    let accepted_fingerprint = admission
        .to_unknown_fingerprint(started_at_unix_ms + 250)
        .context("materialize accepted fingerprint")?;
    write_fingerprint_sync_readback(reopened.as_ref(), &accepted_fingerprint)
        .context("write accepted fingerprint")?;

    let rejected_entry = evaluate_novel_pattern_promotion(
        admission,
        &rejected_heldout(),
        started_at_unix_ms + 300,
        config,
    )
    .context("rejected held-out gate")?;
    reopened_store
        .persist_ontology_growth_audit(&rejected_entry)
        .context("persist rejected audit")?;
    drop(reopened_store);
    drop(reopened);

    let final_db = open_full_db(&db_path)?;
    let final_store = RocksDbEvalStore::new(final_db.clone()).context("final store")?;
    let audit_entries_after_reopen = final_store
        .load_ontology_growth_audit_entries()
        .context("load ontology audit entries")?;
    let proposal_inputs =
        proposal_inputs_from_ontology_audit(&audit_entries_after_reopen).context("proposals")?;
    let source_of_truth_counts = SourceOfTruthCounts {
        active_learning_queue_cf_rows: count_cf(
            final_db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_ACTIVE_LEARNING_QUEUE,
        )?,
        ontology_growth_audit_cf_rows: count_cf(
            final_db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_ONTOLOGY_GROWTH_AUDIT,
        )?,
        failure_fingerprint_cf_rows: count_cf(
            final_db.as_ref(),
            context_graph_mejepa_cf::CF_MEJEPA_FAILURE_FINGERPRINTS,
        )?,
    };
    let boundary_cases = boundary_cases(started_at_unix_ms, config)?;
    let all_passed = queue_entries_before_persist == 3
        && queue_entries_after_reopen == 3
        && novel_candidates_after_reopen == 3
        && candidate_admissions.len() == 1
        && candidate_admissions[0].member_candidate_ids.len() == 3
        && audit_entries_after_reopen.len() == 3
        && audit_entries_after_reopen
            .iter()
            .any(|entry| entry.outcome == OntologyGrowthAuditOutcome::Candidate)
        && audit_entries_after_reopen
            .iter()
            .any(|entry| entry.outcome == OntologyGrowthAuditOutcome::Accepted)
        && audit_entries_after_reopen.iter().any(|entry| {
            entry.outcome == OntologyGrowthAuditOutcome::RejectedByFalsification
                && entry.reason == "rejected_by_falsification"
        })
        && source_of_truth_counts.active_learning_queue_cf_rows == 1
        && source_of_truth_counts.ontology_growth_audit_cf_rows == 3
        && source_of_truth_counts.failure_fingerprint_cf_rows == 1
        && proposal_inputs.len() == 2
        && proposal_inputs.iter().any(|input| {
            input.audit_outcome == OntologyGrowthAuditOutcome::Candidate
                && input.cluster_id == admission.cluster_id
        })
        && proposal_inputs.iter().any(|input| {
            input.audit_outcome == OntologyGrowthAuditOutcome::Accepted
                && input.cluster_id == admission.cluster_id
        })
        && boundary_cases.iter().all(|case| case.pass);
    if !all_passed {
        bail!("novel-pattern ontology-growth FSV did not pass all checks");
    }

    let report = FsvReport {
        fsv_root: FSV_ROOT.to_string(),
        db_path: db_path.display().to_string(),
        output_path: output_path.display().to_string(),
        started_at_unix_ms,
        queue_entries_before_persist,
        queue_entries_after_reopen,
        novel_candidates_after_reopen,
        candidate_admissions,
        audit_entries_after_reopen,
        proposal_inputs,
        source_of_truth_counts,
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

fn boundary_cases(
    started_at_unix_ms: i64,
    config: NovelPatternDetectorConfig,
) -> Result<Vec<BoundaryCase>> {
    let existing_cells = existing_constellation_cells();
    let mut queue = ActiveLearningQueueState::new(8).context("boundary queue")?;
    let low_novelty = queue
        .enqueue_novel_pattern_candidate(
            &prediction("low-novelty-boundary", 0x71),
            vectors([("e1", vec![1.0, 0.001]), ("e2", vec![1.0, 0.001])]),
            &existing_cells,
            started_at_unix_ms + 1_000,
            config,
        )
        .context("low novelty boundary")?;

    queue
        .enqueue_novel_pattern_candidate(
            &prediction("small-cluster-boundary-1", 0x72),
            novel_observation(),
            &existing_cells,
            started_at_unix_ms + 1_001,
            config,
        )
        .context("small cluster one")?;
    queue
        .enqueue_novel_pattern_candidate(
            &prediction("small-cluster-boundary-2", 0x73),
            novel_observation(),
            &existing_cells,
            started_at_unix_ms + 1_002,
            config,
        )
        .context("small cluster two")?;
    let insufficient_cluster = queue
        .admit_novel_pattern_clusters(&existing_cells, config)
        .context("insufficient cluster admission")?;

    let missing_cells = queue.enqueue_novel_pattern_candidate(
        &prediction("empty-cell-boundary", 0x74),
        novel_observation(),
        &[],
        started_at_unix_ms + 1_003,
        config,
    );
    let bad_heldout = evaluate_novel_pattern_promotion(
        &single_admission_for_boundary()?,
        &HeldoutImprovementEvidence {
            baseline_per_cell_correlation: BTreeMap::from([("cell-a".to_string(), 0.8)]),
            candidate_per_cell_correlation: BTreeMap::from([("cell-b".to_string(), 0.9)]),
        },
        started_at_unix_ms + 1_004,
        config,
    );

    let proposal_inputs = proposal_inputs_from_ontology_audit(&[evaluate_novel_pattern_promotion(
        &single_admission_for_boundary()?,
        &rejected_heldout(),
        started_at_unix_ms + 1_005,
        config,
    )
    .context("rejected boundary audit")?])
    .context("rejected proposal surface")?;

    let missing_cells_code = error_code(missing_cells);
    let bad_heldout_code = error_code(bad_heldout);
    Ok(vec![
        BoundaryCase {
            case: "low_novelty_not_enqueued",
            observed: low_novelty.to_string(),
            pass: !low_novelty,
        },
        BoundaryCase {
            case: "insufficient_cluster_not_admitted",
            observed: insufficient_cluster.len().to_string(),
            pass: insufficient_cluster.is_empty(),
        },
        BoundaryCase {
            case: "missing_existing_cells_fail_closed",
            observed: missing_cells_code.clone(),
            pass: missing_cells_code == "MEJEPA_EVAL_INVALID_INPUT",
        },
        BoundaryCase {
            case: "heldout_cell_mismatch_fails_closed",
            observed: bad_heldout_code.clone(),
            pass: bad_heldout_code == "MEJEPA_EVAL_INVALID_INPUT",
        },
        BoundaryCase {
            case: "rejected_proposal_not_surfaced",
            observed: proposal_inputs.len().to_string(),
            pass: proposal_inputs.is_empty(),
        },
    ])
}

fn accepted_heldout() -> HeldoutImprovementEvidence {
    HeldoutImprovementEvidence {
        baseline_per_cell_correlation: BTreeMap::from([
            ("off_by_one::python".to_string(), 0.80),
            ("swap_variable::python".to_string(), 0.81),
        ]),
        candidate_per_cell_correlation: BTreeMap::from([
            ("off_by_one::python".to_string(), 0.82),
            ("swap_variable::python".to_string(), 0.83),
        ]),
    }
}

fn rejected_heldout() -> HeldoutImprovementEvidence {
    HeldoutImprovementEvidence {
        baseline_per_cell_correlation: BTreeMap::from([
            ("off_by_one::python".to_string(), 0.80),
            ("swap_variable::python".to_string(), 0.81),
        ]),
        candidate_per_cell_correlation: BTreeMap::from([
            ("off_by_one::python".to_string(), 0.805),
            ("swap_variable::python".to_string(), 0.83),
        ]),
    }
}

fn existing_constellation_cells() -> Vec<ConstellationCentroid> {
    vec![ConstellationCentroid {
        cell_id: "known-good-python-cell".to_string(),
        centroid_by_embedder: vectors([("e1", vec![1.0, 0.0]), ("e2", vec![1.0, 0.0])]),
        variance_by_embedder: BTreeMap::from([
            (EmbedderId("e1".to_string()), 0.01),
            (EmbedderId("e2".to_string()), 0.01),
        ]),
    }]
}

fn novel_observation() -> BTreeMap<EmbedderId, Vec<f32>> {
    vectors([("e1", vec![0.0, 1.0]), ("e2", vec![0.0, 1.0])])
}

fn single_admission_for_boundary() -> Result<NovelPatternClusterAdmission> {
    let config = NovelPatternDetectorConfig::default();
    let existing_cells = existing_constellation_cells();
    let mut queue = ActiveLearningQueueState::new(8).context("boundary admission queue")?;
    for idx in 0..3 {
        queue.enqueue_novel_pattern_candidate(
            &prediction(&format!("boundary-admission-{idx}"), 0x80 + idx as u8),
            novel_observation(),
            &existing_cells,
            2_000 + idx,
            config,
        )?;
    }
    queue
        .admit_novel_pattern_clusters(&existing_cells, config)?
        .into_iter()
        .next()
        .context("boundary admission missing")
}

fn prediction(task_id: &str, prediction_id: u8) -> RealityPrediction {
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
    .ood_score(0.91)
    .calibrated_confidence(0.5)
    .calibration_version("task-rwd-211-novel-pattern-fsv")
    .build()
    .expect("prediction")
}

fn vectors<const N: usize>(items: [(&str, Vec<f32>); N]) -> BTreeMap<EmbedderId, Vec<f32>> {
    items
        .into_iter()
        .map(|(embedder, vector)| (EmbedderId(embedder.to_string()), vector))
        .collect()
}

fn error_code<T>(result: std::result::Result<T, context_graph_mejepa::EvalError>) -> String {
    match result {
        Ok(_) => "unexpected_ok".to_string(),
        Err(err) => err.code().to_string(),
    }
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

fn count_cf(db: &DB, cf_name: &str) -> Result<usize> {
    let cf = db
        .cf_handle(cf_name)
        .with_context(|| format!("missing CF {cf_name}"))?;
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        item?;
        count += 1;
    }
    Ok(count)
}

fn unix_ms() -> Result<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before unix epoch")?
        .as_millis()
        .try_into()
        .context("unix millis exceeds i64")
}
