use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use context_graph_mejepa::{
    open_infer_rocksdb, propose_embedder_proposals, propose_embedder_proposals_from_db,
    read_embedder_proposals, write_embedder_proposals_sync_readback, AbsenceShapeDescriptor,
    EmbedderAbsenceSignal, EmbedderAbsenceSignalKind, EmbedderProposalConfig,
};
use context_graph_mejepa_cf::CF_MEJEPA_EMBEDDER_PROPOSALS;
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-ek-018b-absence-queue-fsv";

fn main() -> Result<()> {
    let ts = now_unix_ms()?;
    let run_root = Path::new(FSV_ROOT).join(format!("run-{ts}"));
    fs::create_dir_all(&run_root).context("create FSV run dir")?;
    let db_path = run_root.join("infer-rocksdb");
    let fsv_path = run_root.join("absence_queue_fsv.json");
    let db = open_infer_rocksdb(&db_path).context("open FSV inference RocksDB")?;

    let empty_db_report =
        propose_embedder_proposals_from_db(db.as_ref(), EmbedderProposalConfig::default(), ts)
            .context("empty DB report")?;

    let mut signals = vec![
        synthetic_signal(
            EmbedderAbsenceSignalKind::MincutStructuralHole,
            "mincut:structural-gap-a",
            "panel_graph:pairwise_mi_matrix",
            "targeted_blind_spot_contrastive",
            "held_out_mincut_window_per_cell_correlation",
            12,
            0.90,
            0.80,
        ),
        synthetic_signal(
            EmbedderAbsenceSignalKind::UnknownFingerprintCluster,
            "unknown:cluster-a",
            "unknown_fingerprint_cluster",
            "contrast_unknown_cluster_against_known_fingerprints",
            "held_out_unknown_fingerprint_ood_recall",
            48,
            0.70,
            0.75,
        ),
        synthetic_signal(
            EmbedderAbsenceSignalKind::PairwiseMiResidual,
            "pairwise-mi:residual-a",
            "embedder_pairwise_mi_residual",
            "reduce_redundant_panel_signal_and_expose_missing_axis",
            "held_out_pairwise_mi_residual_and_global_correlation",
            12,
            0.55,
            0.65,
        ),
        synthetic_signal(
            EmbedderAbsenceSignalKind::OodScore,
            "ood:prediction-a",
            "prediction_ood_surface",
            "reduce_out_of_distribution_prediction_mass",
            "held_out_ood_recall_and_per_cell_correlation",
            64,
            0.50,
            0.45,
        ),
        synthetic_signal(
            EmbedderAbsenceSignalKind::Curiosity,
            "curiosity:window-a",
            "compression_progress_gap",
            "maximize_expected_compression_progress",
            "held_out_predicted_vs_actual_cp_phi_calibration",
            64,
            0.42,
            0.90,
        ),
        synthetic_signal(
            EmbedderAbsenceSignalKind::FoundationalityVariance,
            "foundationality:cell-a",
            "chunk_foundationality_variance",
            "separate_foundational_from_surface_level_chunk_structure",
            "held_out_foundationality_weighted_cell_correlation",
            64,
            0.40,
            0.50,
        ),
    ];
    signals.push(EmbedderAbsenceSignal {
        source_ref: "mincut:structural-gap-a-duplicate".to_string(),
        ..signals[0].clone()
    });

    let report = propose_embedder_proposals(&signals, &[], EmbedderProposalConfig::default(), ts)
        .context("synthetic absence proposal report")?;
    let write_summary = write_embedder_proposals_sync_readback(db.as_ref(), &report.proposals)
        .context("write proposal queue")?;
    let readback = read_embedder_proposals(db.as_ref()).context("proposal readback")?;
    let existing_duplicate_report = propose_embedder_proposals(
        &signals,
        &readback,
        EmbedderProposalConfig::default(),
        ts + 1,
    )
    .context("existing duplicate report")?;

    let ranking_descends = report
        .proposals
        .windows(2)
        .all(|pair| pair[0].composite_score >= pair[1].composite_score);
    let all_six_sources_present = [
        "mincut_structural_hole",
        "unknown_fingerprint_cluster",
        "pairwise_mi_residual",
        "ood_score",
        "curiosity",
        "foundationality_variance",
    ]
    .iter()
    .all(|kind| report.source_counts.contains_key(*kind));
    let readback_equal = readback == report.proposals;
    let empty_fail_closed = empty_db_report.proposals.is_empty()
        && empty_db_report.empty_reason.as_deref()
            == Some(context_graph_mejepa::MEJEPA_PENDING_EMBEDDER_PROPOSALS_EMPTY_SUBSTRATE);
    let duplicate_deduped = report.duplicates_deduped == 1 && report.proposals.len() == 6;
    let existing_duplicate_blocked = existing_duplicate_report.proposals.is_empty()
        && existing_duplicate_report.existing_proposal_matches == signals.len();
    let all_passed = ranking_descends
        && all_six_sources_present
        && write_summary.byte_identical_readback
        && write_summary.rows_written == 6
        && readback_equal
        && empty_fail_closed
        && duplicate_deduped
        && existing_duplicate_blocked;

    let evidence = json!({
        "task": "TASK-EK-018B",
        "issue": 344,
        "all_passed": all_passed,
        "sot": {
            "queue_cf": CF_MEJEPA_EMBEDDER_PROPOSALS,
            "db_path": db_path.display().to_string(),
            "inner_llm_invoked": false
        },
        "synthetic_injection": {
            "signals_seen": report.signals_seen,
            "source_counts": report.source_counts,
            "proposal_count": report.proposals.len(),
            "ranking_descends": ranking_descends,
            "all_six_sources_present": all_six_sources_present,
            "top_candidate": report.proposals.first().map(|proposal| proposal.candidate_name.clone()),
            "top_score": report.proposals.first().map(|proposal| proposal.composite_score)
        },
        "dedupe": {
            "duplicates_deduped": report.duplicates_deduped,
            "existing_proposal_matches": existing_duplicate_report.existing_proposal_matches,
            "existing_duplicate_blocked": existing_duplicate_blocked
        },
        "persistence": {
            "rows_written": write_summary.rows_written,
            "byte_identical_readback": write_summary.byte_identical_readback,
            "readback_equal": readback_equal,
            "readback_count": readback.len()
        },
        "empty_substrate": {
            "proposal_count": empty_db_report.proposals.len(),
            "empty_reason": empty_db_report.empty_reason,
            "fail_closed": empty_fail_closed
        }
    });
    fs::write(&fsv_path, serde_json::to_vec_pretty(&evidence)?).context("write FSV json")?;
    let fsv_sha256 = sha256_file(&fsv_path).context("hash FSV json")?;
    let mut stdout = evidence;
    stdout["fsv_path"] = json!(fsv_path.display().to_string());
    stdout["fsv_sha256"] = json!(fsv_sha256);
    println!("{}", serde_json::to_string_pretty(&stdout)?);
    if !all_passed {
        bail!("TASK-EK-018B absence queue FSV failed");
    }
    Ok(())
}

fn synthetic_signal(
    kind: EmbedderAbsenceSignalKind,
    source_ref: &str,
    input_modality: &str,
    suggested_objective: &str,
    reality_channel_for_falsification: &str,
    suggested_dim: usize,
    signal_magnitude: f32,
    predicted_delta_cp_phi: f32,
) -> EmbedderAbsenceSignal {
    EmbedderAbsenceSignal {
        kind,
        source_ref: source_ref.to_string(),
        descriptor: AbsenceShapeDescriptor {
            input_modality: input_modality.to_string(),
            suggested_dim,
            suggested_objective: suggested_objective.to_string(),
            reality_channel_for_falsification: reality_channel_for_falsification.to_string(),
            signal_magnitude,
        },
        predicted_delta_cp_phi,
        used_window_ids: vec!["synthetic-heldout-window".to_string()],
    }
}

fn now_unix_ms() -> Result<i64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before unix epoch")?;
    i64::try_from(elapsed.as_millis()).context("millis overflowed i64")
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}
