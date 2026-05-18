use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::heal::promote::{HoldoutEval, ModeWinner};
use context_graph_mejepa::{
    count_cf, current_unix_ms, embedder_architecture_has_rejection,
    evaluate_and_persist_embedder_falsification, open_infer_rocksdb,
    read_embedder_proposal_rejection, read_embedder_proposal_rejections, write_json_0600,
    EmbedderCandidateHoldoutComparison, EmbedderFalsificationGate, RuntimeEmbedderId,
    EMBEDDER_FALSIFICATION_SCHEMA_VERSION, MEJEPA_EMBEDDER_FALSIFICATION_CELL_REGRESSION,
    MEJEPA_EMBEDDER_FALSIFICATION_GLOBAL_DELTA_TOO_SMALL,
};
use context_graph_mejepa_cf::CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS;
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-ek-018c-falsification-gate-fsv";

fn main() -> Result<()> {
    let started_at_unix_ms = current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let db_path = run_root.join("infer-rocksdb");
    let fsv_path = run_root.join("falsification_gate_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-018C FSV run dir")?;
    let db = open_infer_rocksdb(&db_path).context("open TASK-EK-018C inference RocksDB")?;
    let gate = EmbedderFalsificationGate::default();

    let before_rejection_count = count_cf(db.as_ref(), CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS)
        .context("count rejections before")?;

    let positive = comparison_fixture(
        "known_positive",
        2,
        0.906,
        BTreeMap::from([
            ("mutation:python".to_string(), 0.866),
            ("typing:python".to_string(), 0.856),
        ]),
        started_at_unix_ms,
    )?;
    let regression = comparison_fixture(
        "cell_regression",
        3,
        0.908,
        BTreeMap::from([
            ("mutation:python".to_string(), 0.866),
            ("typing:python".to_string(), 0.844),
        ]),
        started_at_unix_ms + 1,
    )?;
    let zero_delta = comparison_fixture(
        "zero_delta",
        4,
        0.900,
        BTreeMap::from([
            ("mutation:python".to_string(), 0.860),
            ("typing:python".to_string(), 0.850),
        ]),
        started_at_unix_ms + 2,
    )?;

    let positive_decision =
        evaluate_and_persist_embedder_falsification(db.as_ref(), &positive, gate)
            .context("positive candidate falsification gate")?;
    let regression_decision =
        evaluate_and_persist_embedder_falsification(db.as_ref(), &regression, gate)
            .context("regression candidate falsification gate")?;
    let zero_delta_decision =
        evaluate_and_persist_embedder_falsification(db.as_ref(), &zero_delta, gate)
            .context("zero-delta candidate falsification gate")?;

    let rows = read_embedder_proposal_rejections(db.as_ref()).context("rejection readback")?;
    let direct_rejection_reads = rows
        .iter()
        .map(|row| {
            read_embedder_proposal_rejection(db.as_ref(), row.rejection_id)
                .context("direct rejection read")
                .and_then(|record| record.context("missing direct rejection row"))
        })
        .collect::<Result<Vec<_>>>()?;
    let regression_architecture_blocked = embedder_architecture_has_rejection(
        db.as_ref(),
        &regression.candidate_architecture_signature,
    )
    .context("regression architecture rejection lookup")?;
    let zero_architecture_blocked = embedder_architecture_has_rejection(
        db.as_ref(),
        &zero_delta.candidate_architecture_signature,
    )
    .context("zero-delta architecture rejection lookup")?;
    let positive_architecture_unblocked = !embedder_architecture_has_rejection(
        db.as_ref(),
        &positive.candidate_architecture_signature,
    )
    .context("positive architecture rejection lookup")?;
    let after_rejection_count = count_cf(db.as_ref(), CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS)
        .context("count rejections after")?;

    drop(db);
    let reopened = open_infer_rocksdb(&db_path).context("reopen TASK-EK-018C RocksDB")?;
    let reopened_rows =
        read_embedder_proposal_rejections(reopened.as_ref()).context("reopened rejection rows")?;
    let reopened_count = count_cf(reopened.as_ref(), CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS)
        .context("count reopened rejections")?;

    let positive_promoted = positive_decision.accepted
        && positive_decision.winner == ModeWinner::B
        && positive_decision.global_delta >= gate.delta_global
        && positive_decision.regressing_cells.is_empty()
        && positive_decision.source_of_truth_key_hex.is_none();
    let regression_rejected = !regression_decision.accepted
        && regression_decision.reason_code.as_deref()
            == Some(MEJEPA_EMBEDDER_FALSIFICATION_CELL_REGRESSION)
        && regression_decision
            .regressing_cells
            .contains(&"typing:python".to_string())
        && regression_decision.source_of_truth_key_hex.is_some();
    let zero_delta_rejected = !zero_delta_decision.accepted
        && zero_delta_decision.reason_code.as_deref()
            == Some(MEJEPA_EMBEDDER_FALSIFICATION_GLOBAL_DELTA_TOO_SMALL)
        && zero_delta_decision.source_of_truth_key_hex.is_some();
    let rejection_rows_preserve_full_provenance = rows.len() == 2
        && direct_rejection_reads == rows
        && rows.iter().all(rejection_has_full_provenance);
    let persisted_across_reopen = reopened_rows == rows && reopened_count == 2;

    let all_passed = before_rejection_count == 0
        && after_rejection_count == 2
        && positive_promoted
        && regression_rejected
        && zero_delta_rejected
        && regression_architecture_blocked
        && zero_architecture_blocked
        && positive_architecture_unblocked
        && rejection_rows_preserve_full_provenance
        && persisted_across_reopen;

    let evidence = json!({
        "task": "TASK-EK-018C",
        "issue": 343,
        "all_passed": all_passed,
        "sot": {
            "rejection_cf": CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS,
            "db_path": db_path,
            "fsv_path": fsv_path,
            "inner_llm_invoked": false
        },
        "gate": gate,
        "positive_candidate": {
            "accepted": positive_decision.accepted,
            "winner": positive_decision.winner,
            "global_delta": positive_decision.global_delta,
            "min_cell_delta": positive_decision.min_cell_delta,
            "source_of_truth_key_hex": positive_decision.source_of_truth_key_hex,
            "promoted": positive_promoted
        },
        "cell_regression_candidate": {
            "accepted": regression_decision.accepted,
            "reason_code": regression_decision.reason_code,
            "global_delta": regression_decision.global_delta,
            "min_cell_delta": regression_decision.min_cell_delta,
            "regressing_cells": regression_decision.regressing_cells,
            "source_of_truth_key_hex": regression_decision.source_of_truth_key_hex,
            "architecture_blocked": regression_architecture_blocked
        },
        "zero_delta_candidate": {
            "accepted": zero_delta_decision.accepted,
            "reason_code": zero_delta_decision.reason_code,
            "global_delta": zero_delta_decision.global_delta,
            "min_cell_delta": zero_delta_decision.min_cell_delta,
            "source_of_truth_key_hex": zero_delta_decision.source_of_truth_key_hex,
            "architecture_blocked": zero_architecture_blocked
        },
        "persistence": {
            "before_rejection_count": before_rejection_count,
            "after_rejection_count": after_rejection_count,
            "reopened_count": reopened_count,
            "rows_preserve_full_provenance": rejection_rows_preserve_full_provenance,
            "persisted_across_reopen": persisted_across_reopen,
            "rows": rows
        }
    });

    write_json_0600(&fsv_path, &evidence).context("write TASK-EK-018C FSV JSON")?;
    let fsv_sha256 = sha256_file(&fsv_path).context("hash TASK-EK-018C FSV JSON")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "all_passed": all_passed,
            "fsv_path": fsv_path,
            "fsv_sha256": fsv_sha256,
            "positive_promoted": positive_promoted,
            "regression_rejected": regression_rejected,
            "zero_delta_rejected": zero_delta_rejected,
            "rejection_rows": rows.len(),
            "persisted_across_reopen": persisted_across_reopen,
        }))?
    );
    ensure!(all_passed, "TASK-EK-018C falsification gate FSV failed");
    Ok(())
}

fn comparison_fixture(
    suffix: &str,
    sequence: u32,
    mode_b_global: f32,
    mode_b_cells: BTreeMap<String, f32>,
    created_at_unix_ms: i64,
) -> Result<EmbedderCandidateHoldoutComparison> {
    let proposal_id = proposal_id_for(suffix);
    let comparison = EmbedderCandidateHoldoutComparison {
        schema_version: EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
        proposal_id,
        candidate_id: RuntimeEmbedderId::dynamic(sequence, format!("{suffix}_candidate"))?,
        candidate_name: format!("fixture_{suffix}_candidate"),
        candidate_architecture_signature: format!("fixture-architecture-{suffix}"),
        candidate_artifact_sha256: sha256_text(&format!("artifact:{suffix}")),
        training_cert_chain_hash: sha256_text(&format!("train-cert:{suffix}")),
        proposal_source_refs: vec![
            format!("unknown_cluster:{suffix}"),
            format!("mincut_window:{suffix}"),
        ],
        proposer_used_window_ids: vec![
            format!("proposal-window-{suffix}-a"),
            format!("proposal-window-{suffix}-b"),
        ],
        heldout_window_ids: vec![format!("unseen-heldout-window-{suffix}")],
        mode_a: eval(
            0.900,
            BTreeMap::from([
                ("mutation:python".to_string(), 0.860),
                ("typing:python".to_string(), 0.850),
            ]),
        )?,
        mode_b: eval(mode_b_global, mode_b_cells)?,
        mode_c: eval(
            0.898,
            BTreeMap::from([
                ("mutation:python".to_string(), 0.858),
                ("typing:python".to_string(), 0.848),
            ]),
        )?,
        created_at_unix_ms,
    };
    comparison.validate()?;
    Ok(comparison)
}

fn eval(global: f32, cells: BTreeMap<String, f32>) -> Result<HoldoutEval> {
    Ok(HoldoutEval::try_new_with_cells(
        0.95, global, 0.01, 256, [9u8; 32], cells,
    )?)
}

fn rejection_has_full_provenance(
    record: &context_graph_mejepa::EmbedderProposalRejectionRecord,
) -> bool {
    record.source_of_truth_cf == CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS
        && record.schema_version == EMBEDDER_FALSIFICATION_SCHEMA_VERSION
        && !record.candidate_architecture_signature.is_empty()
        && record.candidate_artifact_sha256.len() == 64
        && record.training_cert_chain_hash.len() == 64
        && !record.proposal_source_refs.is_empty()
        && !record.proposer_used_window_ids.is_empty()
        && !record.heldout_window_ids.is_empty()
        && !record.mode_a_score.score.is_nan()
        && !record.mode_b_score.score.is_nan()
        && !record.mode_c_score.score.is_nan()
        && !record.compared_cells.is_empty()
        && record.created_at_unix_ms > 0
}

fn proposal_id_for(suffix: &str) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"TASK_EK_018C_PROPOSAL");
    hasher.update(suffix.as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn sha256_text(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex::encode(hasher.finalize()))
}
