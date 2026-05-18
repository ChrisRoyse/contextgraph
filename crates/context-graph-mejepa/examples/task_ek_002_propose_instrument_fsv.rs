//! TASK-EK-002 instrument-proposal FSV.
//!
//! Proves that Unknown-fingerprint residual clusters, mincut structural-hole
//! reports, and pairwise-MI health compose into auditable frozen-instrument
//! proposals with deterministic promotion / falsification decisions.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use context_graph_mejepa::eval::report::write_json_0600;
use context_graph_mejepa::{
    count_cf, mejepa_mincut_panel, open_infer_rocksdb, promote_instrument_proposal,
    propose_instruments, propose_instruments_from_db, read_instrument_proposals,
    write_instrument_proposals_sync_readback, write_mincut_report_sync_readback,
    write_pairwise_mi_matrix_sync_readback, ActiveLearningKind, ActiveLearningQueueEntry,
    ActiveLearningQueueState, EmbedderId, FailureModeClass, FingerprintCalibrationState,
    FingerprintCandidateScore, FingerprintEmbedderScore, FingerprintId, FingerprintKind,
    InstrumentProposalConfig, InstrumentProposalDecision, InstrumentProposalStatus, MincutOptions,
    MutationCategory, OracleOutcome, PanelGraphSource, PredictionId, RocksDbEvalStore, TaskId,
    UnknownFingerprintCandidate, MEJEPA_PROPOSE_INSTRUMENT_EMPTY_QUEUE,
    MEJEPA_PROPOSE_INSTRUMENT_NO_SURVIVING_CLUSTER, PAIRWISE_MI_ESTIMATOR_PARTITIONED_NMI,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE, CF_MEJEPA_INSTRUMENT_PROPOSALS, CF_MEJEPA_MINCUT_REPORTS,
    CF_MEJEPA_PAIRWISE_MI,
};
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-ek-002-propose-instrument-fsv";
const PROPOSE_TOOL: &str = "mcp__cgreality__mejepa_propose_instrument";
const PROMOTE_TOOL: &str = "mcp__cgreality__mejepa_promote_instrument_proposal";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EdgeCaseEvidence {
    name: String,
    passed: bool,
    returned_proposals: usize,
    empty_reason: Option<String>,
    detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskEk002Fsv {
    output_path: String,
    canonical_path: String,
    db_path: String,
    started_at_unix_ms: i64,
    mcp_tool_names: Vec<String>,
    source_cfs: Vec<String>,
    queue_entries_written: usize,
    mincut_report_rows: u64,
    pairwise_mi_rows_written: usize,
    proposal_count: usize,
    proposal_cf_rows_after_write: u64,
    proposal_cf_rows_after_reopen: u64,
    residual_cosine_to_injected: f32,
    accepted_status: String,
    accepted_delta_cleared: bool,
    rejected_status: String,
    rejected_falsification_recorded: bool,
    under_review_second_call_error_code: String,
    edge_cases: Vec<EdgeCaseEvidence>,
    json_readback_equal: bool,
    passes: bool,
}

fn main() -> Result<()> {
    let started_at_unix_ms = unix_ms()?;
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let db_path = run_root.join("rocksdb");
    let output_path = run_root.join("propose_instrument_fsv.json");
    let canonical_path = PathBuf::from(FSV_ROOT).join("propose_instrument_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-002 FSV root")?;

    let (
        queue_entries_written,
        mincut_report_rows,
        pairwise_mi_rows_written,
        proposal_count,
        proposal_cf_rows_after_write,
        proposal_cf_rows_after_reopen,
        residual_cosine_to_injected,
        accepted_status,
        accepted_delta_cleared,
        rejected_status,
        rejected_falsification_recorded,
        under_review_second_call_error_code,
    ) = {
        let db = open_infer_rocksdb(&db_path).context("open TASK-EK-002 FSV RocksDB")?;
        let queue_entries_written = seed_unknown_queue(db.clone(), started_at_unix_ms)?;
        let mincut_report = seed_mincut(db.as_ref(), started_at_unix_ms + 10)?;
        let mincut_report_rows = count_cf(db.as_ref(), CF_MEJEPA_MINCUT_REPORTS)?;
        let pairwise_summary = seed_pairwise_mi(db.as_ref(), started_at_unix_ms + 20)?;

        let config = fsv_config(3);
        let proposal_report =
            propose_instruments_from_db(db.as_ref(), config, started_at_unix_ms + 30)
                .context("compose instrument proposals from CF inputs")?;
        if proposal_report.proposals.is_empty() {
            bail!("TASK-EK-002 proposal report unexpectedly empty");
        }
        let first = proposal_report.proposals[0].clone();
        let residual_cosine_to_injected =
            cosine(&first.residual_direction, &injected_residual_direction())?;
        let cluster_justified = first.justifying_unknown_cluster.is_some();
        let mi_justified = first.justifying_pairwise_mi_signature.is_some();
        let mincut_justified = first.structural_hole_partition == Some(mincut_report.partition);
        if !cluster_justified || !mi_justified || !mincut_justified {
            bail!("proposal did not retain cluster + MI + mincut justifications");
        }

        write_instrument_proposals_sync_readback(db.as_ref(), &proposal_report.proposals)
            .context("persist proposed instruments")?;
        let mut non_improving = first.clone();
        non_improving.proposal_id = [0x72; 16];
        non_improving.candidate_name = "instrument_known_non_improving".to_string();
        non_improving.status = InstrumentProposalStatus::Candidate;
        non_improving.decision = None;
        let mut under_review = first.clone();
        under_review.proposal_id = [0x73; 16];
        under_review.candidate_name = "instrument_under_review_concurrency".to_string();
        under_review.status = InstrumentProposalStatus::Candidate;
        under_review.decision = None;
        write_instrument_proposals_sync_readback(
            db.as_ref(),
            &[non_improving.clone(), under_review.clone()],
        )
        .context("persist promotion edge proposals")?;
        let proposal_cf_rows_after_write = count_cf(db.as_ref(), CF_MEJEPA_INSTRUMENT_PROPOSALS)?;

        let accepted = promote_instrument_proposal(
            db.as_ref(),
            first.proposal_id,
            InstrumentProposalDecision::Accept,
            0.025,
            0.01,
            started_at_unix_ms + 40,
        )
        .context("accept improving proposal")?;
        let rejected = promote_instrument_proposal(
            db.as_ref(),
            non_improving.proposal_id,
            InstrumentProposalDecision::Accept,
            0.0,
            0.01,
            started_at_unix_ms + 41,
        )
        .context("falsify non-improving proposal")?;
        let _under_review_once = promote_instrument_proposal(
            db.as_ref(),
            under_review.proposal_id,
            InstrumentProposalDecision::MarkUnderReview,
            0.0,
            0.01,
            started_at_unix_ms + 42,
        )
        .context("mark proposal under review")?;
        let under_review_second_call_error_code = promote_instrument_proposal(
            db.as_ref(),
            under_review.proposal_id,
            InstrumentProposalDecision::MarkUnderReview,
            0.0,
            0.01,
            started_at_unix_ms + 43,
        )
        .err()
        .map(|err| err.code().to_string())
        .unwrap_or_else(|| "NO_ERROR".to_string());

        drop(db);
        let reopened = open_infer_rocksdb(&db_path).context("reopen TASK-EK-002 FSV RocksDB")?;
        let reopened_proposals =
            read_instrument_proposals(reopened.as_ref()).context("read reopened proposals")?;
        let proposal_cf_rows_after_reopen =
            count_cf(reopened.as_ref(), CF_MEJEPA_INSTRUMENT_PROPOSALS)?;
        if proposal_cf_rows_after_reopen != reopened_proposals.len() as u64 {
            bail!("instrument proposal row count/readback mismatch after reopen");
        }

        (
            queue_entries_written,
            mincut_report_rows,
            pairwise_summary.rows_written,
            proposal_report.proposals.len(),
            proposal_cf_rows_after_write,
            proposal_cf_rows_after_reopen,
            residual_cosine_to_injected,
            status_slug(accepted.status).to_string(),
            accepted
                .decision
                .as_ref()
                .map(|decision| decision.accepted && decision.observed_holdout_delta >= 0.01)
                .unwrap_or(false),
            status_slug(rejected.status).to_string(),
            rejected
                .decision
                .as_ref()
                .map(|decision| {
                    !decision.accepted
                        && decision.reason.contains("rejected_by_falsification")
                        && rejected.status == InstrumentProposalStatus::RejectedByFalsification
                })
                .unwrap_or(false),
            under_review_second_call_error_code,
        )
    };

    let mut edge_cases = edge_case_evidence(started_at_unix_ms + 100)?;
    edge_cases.push(edge(
        "concurrent_promotion_under_review",
        under_review_second_call_error_code == "MEJEPA_INSTRUMENT_PROPOSAL_UNDER_REVIEW",
        0,
        None,
        "second mark_under_review call fails closed while the proposal is already under review",
    ));
    let passes = queue_entries_written == 3
        && mincut_report_rows == 1
        && pairwise_mi_rows_written == 3
        && proposal_count >= 1
        && proposal_cf_rows_after_write == 3
        && proposal_cf_rows_after_reopen == 3
        && residual_cosine_to_injected >= 0.95
        && accepted_status == "accepted"
        && accepted_delta_cleared
        && rejected_status == "rejected_by_falsification"
        && rejected_falsification_recorded
        && under_review_second_call_error_code == "MEJEPA_INSTRUMENT_PROPOSAL_UNDER_REVIEW"
        && edge_cases.iter().all(|edge| edge.passed);

    let mut output = TaskEk002Fsv {
        output_path: output_path.display().to_string(),
        canonical_path: canonical_path.display().to_string(),
        db_path: db_path.display().to_string(),
        started_at_unix_ms,
        mcp_tool_names: vec![PROPOSE_TOOL.to_string(), PROMOTE_TOOL.to_string()],
        source_cfs: vec![
            CF_MEJEPA_ACTIVE_LEARNING_QUEUE.to_string(),
            CF_MEJEPA_MINCUT_REPORTS.to_string(),
            CF_MEJEPA_PAIRWISE_MI.to_string(),
            CF_MEJEPA_INSTRUMENT_PROPOSALS.to_string(),
        ],
        queue_entries_written,
        mincut_report_rows,
        pairwise_mi_rows_written,
        proposal_count,
        proposal_cf_rows_after_write,
        proposal_cf_rows_after_reopen,
        residual_cosine_to_injected,
        accepted_status,
        accepted_delta_cleared,
        rejected_status,
        rejected_falsification_recorded,
        under_review_second_call_error_code,
        edge_cases,
        json_readback_equal: false,
        passes,
    };
    if !output.passes {
        bail!("TASK-EK-002 FSV assertions failed: {output:#?}");
    }
    write_json_0600(&output_path, &output).context("write run FSV output")?;
    let decoded: TaskEk002Fsv =
        serde_json::from_slice(&fs::read(&output_path).context("read run FSV output")?)
            .context("decode run FSV output")?;
    output.json_readback_equal = decoded == output;
    if !output.json_readback_equal {
        bail!("TASK-EK-002 FSV JSON readback mismatch");
    }
    write_json_0600(&output_path, &output).context("write final run FSV output")?;
    write_json_0600(&canonical_path, &output).context("write canonical FSV output")?;
    let canonical_readback: TaskEk002Fsv =
        serde_json::from_slice(&fs::read(&canonical_path).context("read canonical FSV output")?)
            .context("decode canonical FSV output")?;
    if canonical_readback != output {
        bail!("TASK-EK-002 canonical FSV readback mismatch");
    }
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn seed_unknown_queue(db: std::sync::Arc<DB>, ts: i64) -> Result<usize> {
    let store = RocksDbEvalStore::new(db).context("open eval store")?;
    let mut queue = ActiveLearningQueueState::new(8).context("create queue")?;
    for candidate in [
        unknown_candidate(0x11, 1.00, 0.05, None, ts + 1),
        unknown_candidate(0x12, 0.99, 0.04, None, ts + 2),
        unknown_candidate(0x13, 1.01, 0.06, None, ts + 3),
    ] {
        insert_candidate(&mut queue, candidate);
    }
    let len = queue.entries.len();
    store
        .persist_queue(&queue)
        .context("persist active-learning queue")?;
    Ok(len)
}

fn seed_mincut(db: &DB, ts: i64) -> Result<context_graph_mejepa::MincutReport> {
    let report = mejepa_mincut_panel(
        Some(db),
        PanelGraphSource::InlineWeightedGraph {
            graph_id: "task-ek-002-structural-hole".to_string(),
            node_ids: vec![
                "E_AST".to_string(),
                "E_CFG".to_string(),
                "E_TypeGraph".to_string(),
                "E_Test".to_string(),
            ],
            weights: vec![
                vec![0.0, 2.0, 0.1, 0.1],
                vec![2.0, 0.0, 0.1, 0.1],
                vec![0.1, 0.1, 0.0, 2.0],
                vec![0.1, 0.1, 2.0, 0.0],
            ],
        },
        MincutOptions {
            return_top_k_candidate_directions: 2,
            ..MincutOptions::default()
        },
        ts,
    )
    .context("run synthetic mincut panel")?;
    write_mincut_report_sync_readback(db, &report).context("persist mincut report")?;
    Ok(report)
}

fn seed_pairwise_mi(db: &DB, ts: i64) -> Result<context_graph_mejepa::PairwiseMiCfWriteSummary> {
    let corpus_hash = hex::encode(Sha256::digest(b"task-ek-002-pairwise-mi"));
    let slots = vec![
        "E_AST".to_string(),
        "E_CFG".to_string(),
        "E_TypeGraph".to_string(),
    ];
    let values = vec![
        vec![1.0, 0.10, 0.12],
        vec![0.10, 1.0, 0.08],
        vec![0.12, 0.08, 1.0],
    ];
    write_pairwise_mi_matrix_sync_readback(
        db,
        &corpus_hash,
        &slots,
        &values,
        7,
        64,
        ts,
        PAIRWISE_MI_ESTIMATOR_PARTITIONED_NMI,
    )
    .context("persist pairwise MI matrix")
}

fn edge_case_evidence(ts: i64) -> Result<Vec<EdgeCaseEvidence>> {
    let empty_queue = ActiveLearningQueueState::new(8).context("empty queue")?;
    let empty_report = propose_instruments(&empty_queue, &[], None, fsv_config(3), ts)?;

    let mut below_k = ActiveLearningQueueState::new(8).context("below-k queue")?;
    insert_candidate(
        &mut below_k,
        unknown_candidate(0x21, 1.0, 0.0, None, ts + 1),
    );
    insert_candidate(
        &mut below_k,
        unknown_candidate(0x22, 0.99, 0.01, None, ts + 2),
    );
    let below_k_report = propose_instruments(&below_k, &[], None, fsv_config(3), ts + 3)?;

    let mut low_intra = ActiveLearningQueueState::new(8).context("low-intra queue")?;
    insert_candidate(
        &mut low_intra,
        unknown_candidate(0x31, 1.0, 0.0, None, ts + 4),
    );
    insert_candidate(
        &mut low_intra,
        unknown_candidate(0x32, 0.0, 1.0, None, ts + 5),
    );
    insert_candidate(
        &mut low_intra,
        unknown_candidate(0x33, -1.0, 0.0, None, ts + 6),
    );
    let low_intra_report = propose_instruments(
        &low_intra,
        &[],
        None,
        InstrumentProposalConfig {
            tau_intra: 0.99,
            ..fsv_config(3)
        },
        ts + 7,
    )?;

    let mut too_similar = ActiveLearningQueueState::new(8).context("too-similar queue")?;
    insert_candidate(
        &mut too_similar,
        unknown_candidate(0x41, 1.0, 0.0, Some(0.95), ts + 8),
    );
    insert_candidate(
        &mut too_similar,
        unknown_candidate(0x42, 0.99, 0.01, Some(0.95), ts + 9),
    );
    insert_candidate(
        &mut too_similar,
        unknown_candidate(0x43, 0.98, 0.02, Some(0.95), ts + 10),
    );
    let too_similar_report = propose_instruments(&too_similar, &[], None, fsv_config(3), ts + 11)?;

    Ok(vec![
        edge(
            "empty_active_learning_queue",
            empty_report.proposals.is_empty()
                && empty_report.empty_reason.as_deref()
                    == Some(MEJEPA_PROPOSE_INSTRUMENT_EMPTY_QUEUE),
            empty_report.proposals.len(),
            empty_report.empty_reason,
            "empty queue returns no proposals with explicit empty reason",
        ),
        edge(
            "cluster_below_k_min",
            below_k_report.proposals.is_empty()
                && below_k_report.empty_reason.as_deref()
                    == Some(MEJEPA_PROPOSE_INSTRUMENT_NO_SURVIVING_CLUSTER),
            below_k_report.proposals.len(),
            below_k_report.empty_reason,
            "K_min - 1 coherent Unknown candidates are filtered",
        ),
        edge(
            "low_intra_cluster_cosine",
            low_intra_report.proposals.is_empty()
                && low_intra_report.empty_reason.as_deref()
                    == Some(MEJEPA_PROPOSE_INSTRUMENT_NO_SURVIVING_CLUSTER),
            low_intra_report.proposals.len(),
            low_intra_report.empty_reason,
            "incoherent Unknown candidates do not form a novel cluster",
        ),
        edge(
            "too_similar_to_existing_centroid",
            too_similar_report.proposals.is_empty()
                && too_similar_report.empty_reason.as_deref()
                    == Some(MEJEPA_PROPOSE_INSTRUMENT_NO_SURVIVING_CLUSTER),
            too_similar_report.proposals.len(),
            too_similar_report.empty_reason,
            "distance below tau_far is treated as a duplicate centroid",
        ),
    ])
}

fn edge(
    name: &str,
    passed: bool,
    returned_proposals: usize,
    empty_reason: Option<String>,
    detail: &str,
) -> EdgeCaseEvidence {
    EdgeCaseEvidence {
        name: name.to_string(),
        passed,
        returned_proposals,
        empty_reason,
        detail: detail.to_string(),
    }
}

fn fsv_config(min_cluster_size: usize) -> InstrumentProposalConfig {
    InstrumentProposalConfig {
        min_cluster_size,
        tau_intra: 0.85,
        tau_far: 0.40,
        min_expected_holdout_improvement: 0.01,
        max_proposals: 16,
        pairwise_mi_max_rows: 1024,
    }
}

fn unknown_candidate(
    id: u8,
    x: f32,
    y: f32,
    nearest_mean_cosine: Option<f32>,
    observed_at_unix_ms: i64,
) -> UnknownFingerprintCandidate {
    let task_id = TaskId(format!("task-ek-002-candidate-{id:02x}"));
    UnknownFingerprintCandidate {
        candidate_id: [id; 16],
        prediction_id: PredictionId([id; 16]),
        task_id,
        session_id: [id.wrapping_add(0x30); 16],
        observed_at_unix_ms,
        ood_score: 0.90,
        embedder_disagreement_score: 0.80,
        active_learning_priority: 5,
        observation_by_embedder: BTreeMap::from([
            (EmbedderId("E_AST".to_string()), vec![x, y]),
            (EmbedderId("E_CFG".to_string()), vec![x, y]),
        ]),
        nearest_fingerprints: nearest_mean_cosine
            .map(|mean| vec![nearest_fingerprint_score(id, mean)])
            .unwrap_or_default(),
    }
}

fn insert_candidate(queue: &mut ActiveLearningQueueState, candidate: UnknownFingerprintCandidate) {
    let task_id = candidate.task_id.clone();
    let entry = ActiveLearningQueueEntry {
        task_id: task_id.clone(),
        score: 1.0,
        outcome_set_len: 1,
        ood_score: candidate.ood_score,
        curiosity_score: 0.0,
        reason: "fingerprint_unknown_ood".to_string(),
        kind: ActiveLearningKind::UnknownFingerprint {
            candidate: Box::new(candidate),
        },
    };
    queue.entries.insert(task_id, entry);
}

fn nearest_fingerprint_score(id: u8, mean_cosine: f32) -> FingerprintCandidateScore {
    let matched = mean_cosine >= 0.90;
    FingerprintCandidateScore {
        fingerprint_id: FingerprintId([id.wrapping_add(0x50); 32]),
        name: format!("known-centroid-{id:02x}"),
        kind: FingerprintKind::KnownBad {
            repo: "django__django".to_string(),
            mutation_category: MutationCategory::OffByOne,
            failure_mode: FailureModeClass::OffByOne,
            exception_class: None,
        },
        oracle_outcome: Some(OracleOutcome::Fail),
        matched,
        mean_cosine,
        min_margin: mean_cosine - 0.90,
        embedder_scores: vec![
            FingerprintEmbedderScore {
                embedder: EmbedderId("E_AST".to_string()),
                cosine: mean_cosine,
                tau: 0.90,
                margin: mean_cosine - 0.90,
                matched,
            },
            FingerprintEmbedderScore {
                embedder: EmbedderId("E_CFG".to_string()),
                cosine: mean_cosine,
                tau: 0.90,
                margin: mean_cosine - 0.90,
                matched,
            },
        ],
        n_references: 4,
        calibration_state: FingerprintCalibrationState::Calibrated,
    }
}

fn injected_residual_direction() -> Vec<f32> {
    normalize(&[1.0, 0.05, 1.0, 0.05])
}

fn cosine(left: &[f32], right: &[f32]) -> Result<f32> {
    if left.len() != right.len() {
        bail!(
            "cosine dimension mismatch: left={} right={}",
            left.len(),
            right.len()
        );
    }
    let left = normalize(left);
    let right = normalize(right);
    Ok(left
        .iter()
        .zip(right.iter())
        .map(|(lhs, rhs)| lhs * rhs)
        .sum::<f32>())
}

fn normalize(values: &[f32]) -> Vec<f32> {
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    values.iter().map(|value| value / norm).collect()
}

fn status_slug(status: InstrumentProposalStatus) -> &'static str {
    match status {
        InstrumentProposalStatus::Candidate => "candidate",
        InstrumentProposalStatus::UnderReview => "under_review",
        InstrumentProposalStatus::Accepted => "accepted",
        InstrumentProposalStatus::RejectedByFalsification => "rejected_by_falsification",
        InstrumentProposalStatus::RejectedByOperator => "rejected_by_operator",
    }
}

fn unix_ms() -> Result<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before UNIX epoch")?
        .as_millis()
        .try_into()
        .context("unix millis overflow")
}

#[allow(dead_code)]
fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
