use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::heal::promote::{HoldoutEval, ModeWinner};
use context_graph_mejepa::{
    count_cf, current_unix_ms, learned_head_registry_kind, open_infer_rocksdb,
    read_dynamic_embedder_provenance, read_dynamic_embedder_registry,
    read_embedder_proposal_rejections, synthesize_learned_head_candidate,
    train_and_evaluate_learned_head_synthesis, write_dynamic_embedder_provenance_sync_readback,
    write_dynamic_embedder_registry_sync_readback, write_embedder_proposals_sync_readback,
    write_json_0600, AbsenceShapeDescriptor, DynamicEmbedderProvenanceRecord,
    DynamicEmbedderRegistryRecord, EmbedderAbsenceSignalKind, EmbedderCandidateHoldoutComparison,
    EmbedderFalsificationGate, EmbedderProposalSourceEvidence, FrozenBackboneExample,
    LearnedHeadCandidate, LearnedHeadSynthesisMode, LearnedHeadTrainingConfig,
    PendingEmbedderProposal, EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
    EMBEDDER_PROPOSAL_SCHEMA_VERSION, MEJEPA_LEARNED_HEAD_COLLAPSED,
    MEJEPA_LEARNED_HEAD_TRAINING_DIVERGED,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE, CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY,
    CF_MEJEPA_EMBEDDER_PROPOSALS, CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS,
};
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-ek-018e-learned-head-synthesis-fsv";

fn main() -> Result<()> {
    let started_at = current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at}"));
    let infer_db_path = run_root.join("infer-rocksdb");
    let artifact_root = PathBuf::from(format!(
        "/mnt/d/contextgraph/models/dynamic/task-ek-018e/run-{started_at}"
    ));
    let fsv_path = run_root.join("learned_head_synthesis_fsv.json");
    let canonical_path = PathBuf::from(FSV_ROOT).join("learned_head_synthesis_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-018E FSV run root")?;

    let db = open_infer_rocksdb(&infer_db_path).context("open TASK-EK-018E infer DB")?;
    let config = LearnedHeadTrainingConfig {
        input_dim: 6,
        output_dim: 8,
        epochs: 220,
        artifact_root,
        ..LearnedHeadTrainingConfig::default()
    };
    let proposal = proposal_fixture(started_at);
    let proposal_write =
        write_embedder_proposals_sync_readback(db.as_ref(), std::slice::from_ref(&proposal))
            .context("write/readback learned-head proposal")?;
    let proposal_count =
        count_cf(db.as_ref(), CF_MEJEPA_EMBEDDER_PROPOSALS).context("count proposals")?;
    let examples = examples();

    let accepted_candidate = synthesize_learned_head_candidate(
        &proposal,
        41,
        "E7",
        LearnedHeadSynthesisMode::ContrastiveTrain,
        &config,
        started_at,
    )
    .context("synthesise accepted learned-head candidate")?;
    let accepted_report = train_and_evaluate_learned_head_synthesis(
        db.as_ref(),
        accepted_candidate.clone(),
        &examples,
        &config,
        Some(comparison_fixture(&accepted_candidate, started_at + 1)?),
        EmbedderFalsificationGate::default(),
    )
    .context("train/evaluate accepted learned-head candidate")?;
    let accepted_artifact = accepted_report
        .artifact
        .as_ref()
        .context("accepted artifact missing")?;
    let accepted_artifact_file_sha256 =
        sha256_file(&accepted_artifact.artifact_path).context("hash accepted artifact")?;
    ensure!(
        accepted_artifact_file_sha256 == accepted_artifact.artifact_sha256,
        "accepted learned-head artifact sha mismatch"
    );

    let before_registry_count =
        count_cf(db.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY).context("registry before")?;
    let before_provenance_count = count_cf(db.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE)
        .context("provenance before")?;
    let registry_record = DynamicEmbedderRegistryRecord {
        id: accepted_candidate.candidate_id.clone(),
        registry_version: 2,
        kind: learned_head_registry_kind(),
        dimension: accepted_candidate.output_dim,
        route_languages: vec!["python".to_string()],
        route_entity_types: vec!["Function".to_string(), "Method".to_string()],
        forward_artifact_path: accepted_artifact.artifact_path.display().to_string(),
        forward_artifact_sha256: accepted_artifact.artifact_sha256.clone(),
        required_vram_bytes: accepted_artifact.artifact_bytes,
        active: true,
        promoted_at_unix_ms: started_at,
    };
    registry_record
        .validate()
        .context("validate learned-head registry record")?;
    write_dynamic_embedder_registry_sync_readback(db.as_ref(), &registry_record)
        .context("write/readback learned-head registry")?;
    let accepted_decision = accepted_report
        .falsification_decision
        .as_ref()
        .context("accepted falsification decision missing")?;
    let provenance_record = DynamicEmbedderProvenanceRecord {
        id: accepted_candidate.candidate_id.clone(),
        registry_version: registry_record.registry_version,
        residual_signal_ref: accepted_candidate.proposal_source_refs.join(","),
        architecture_generator: "task-ek-018e-deterministic-frozen-e7-projection-head-v1"
            .to_string(),
        training_cert_chain_hash: accepted_report.training_cert_chain_hash.clone(),
        heldout_global_delta: accepted_decision.global_delta,
        heldout_min_cell_delta: accepted_decision.min_cell_delta,
        operator_approval_id: None,
        forward_pass_artifact_sha256: accepted_artifact.artifact_sha256.clone(),
        created_at_unix_ms: started_at + 2,
    };
    provenance_record
        .validate()
        .context("validate learned-head provenance record")?;
    write_dynamic_embedder_provenance_sync_readback(db.as_ref(), &provenance_record)
        .context("write/readback learned-head provenance")?;
    let direct_registry =
        read_dynamic_embedder_registry(db.as_ref(), &accepted_candidate.candidate_id)
            .context("direct registry read")?
            .context("missing direct registry row")?;
    let direct_provenance = read_dynamic_embedder_provenance(
        db.as_ref(),
        &accepted_candidate.candidate_id,
        registry_record.registry_version,
    )
    .context("direct provenance read")?
    .context("missing direct provenance row")?;
    let after_registry_count =
        count_cf(db.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY).context("registry after")?;
    let after_provenance_count =
        count_cf(db.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE).context("provenance after")?;

    let divergence_candidate = synthesize_learned_head_candidate(
        &proposal,
        42,
        "E7",
        LearnedHeadSynthesisMode::DivergenceFixture,
        &config,
        started_at + 3,
    )
    .context("synthesise divergent learned-head candidate")?;
    let divergence_report = train_and_evaluate_learned_head_synthesis(
        db.as_ref(),
        divergence_candidate.clone(),
        &examples,
        &config,
        None,
        EmbedderFalsificationGate::default(),
    )
    .context("train/evaluate divergent learned-head candidate")?;

    let constant_candidate = synthesize_learned_head_candidate(
        &proposal,
        43,
        "E7",
        LearnedHeadSynthesisMode::ConstantCollapseFixture,
        &config,
        started_at + 4,
    )
    .context("synthesise constant learned-head candidate")?;
    let constant_report = train_and_evaluate_learned_head_synthesis(
        db.as_ref(),
        constant_candidate.clone(),
        &examples,
        &config,
        None,
        EmbedderFalsificationGate::default(),
    )
    .context("train/evaluate constant learned-head candidate")?;
    let direct_rejections =
        read_embedder_proposal_rejections(db.as_ref()).context("read direct rejections")?;

    drop(db);
    let reopened = open_infer_rocksdb(&infer_db_path).context("reopen TASK-EK-018E infer DB")?;
    let reopened_registry =
        read_dynamic_embedder_registry(reopened.as_ref(), &accepted_candidate.candidate_id)
            .context("reopened registry read")?
            .context("missing reopened registry row")?;
    let reopened_provenance = read_dynamic_embedder_provenance(
        reopened.as_ref(),
        &accepted_candidate.candidate_id,
        registry_record.registry_version,
    )
    .context("reopened provenance read")?
    .context("missing reopened provenance row")?;
    let reopened_rejections =
        read_embedder_proposal_rejections(reopened.as_ref()).context("reopened rejections")?;

    let accepted_case_passed = accepted_report.accepted
        && accepted_report.training.converged
        && accepted_report.training.final_loss < accepted_report.training.initial_loss
        && accepted_report.training.final_contrastive_loss
            < accepted_report.training.initial_contrastive_loss
        && accepted_report
            .collapse
            .as_ref()
            .map(|collapse| !collapse.collapsed)
            .unwrap_or(false)
        && accepted_decision.accepted
        && accepted_decision.winner == ModeWinner::B
        && accepted_decision.global_delta >= EmbedderFalsificationGate::default().delta_global
        && accepted_decision.regressing_cells.is_empty()
        && accepted_artifact_file_sha256 == registry_record.forward_artifact_sha256;
    let registry_case_passed = before_registry_count == 0
        && before_provenance_count == 0
        && after_registry_count == 1
        && after_provenance_count == 1
        && direct_registry == registry_record
        && direct_provenance == provenance_record
        && reopened_registry == registry_record
        && reopened_provenance == provenance_record;
    let divergence_logged = !divergence_report.accepted
        && divergence_report.reason_code.as_deref() == Some(MEJEPA_LEARNED_HEAD_TRAINING_DIVERGED)
        && divergence_report.training.diverged
        && divergence_report.rejection_key_hex.is_some();
    let constant_collapse_logged = !constant_report.accepted
        && constant_report.reason_code.as_deref() == Some(MEJEPA_LEARNED_HEAD_COLLAPSED)
        && constant_report
            .collapse
            .as_ref()
            .map(|collapse| collapse.collapsed)
            .unwrap_or(false)
        && constant_report.rejection_key_hex.is_some();
    let rejections_reopened = has_rejection(
        &reopened_rejections,
        &divergence_candidate.architecture_signature,
        MEJEPA_LEARNED_HEAD_TRAINING_DIVERGED,
    ) && has_rejection(
        &reopened_rejections,
        &constant_candidate.architecture_signature,
        MEJEPA_LEARNED_HEAD_COLLAPSED,
    );
    let all_passed = proposal_write.byte_identical_readback
        && proposal_count == 1
        && accepted_case_passed
        && registry_case_passed
        && divergence_logged
        && constant_collapse_logged
        && rejections_reopened
        && direct_rejections.len() == reopened_rejections.len();

    let evidence = json!({
        "task": "TASK-EK-018E",
        "issue": 347,
        "all_passed": all_passed,
        "sot": {
            "proposal_cf": CF_MEJEPA_EMBEDDER_PROPOSALS,
            "rejection_cf": CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS,
            "registry_cf": CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY,
            "provenance_cf": CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE,
            "infer_db_path": infer_db_path,
            "run_fsv_path": fsv_path,
            "canonical_path": canonical_path,
            "frozen_backbone_fixture": "E7",
            "gpu_required_for_fsv": false
        },
        "proposal": {
            "write_summary": proposal_write,
            "count": proposal_count,
            "record": proposal
        },
        "accepted_learned_head": {
            "candidate": accepted_candidate,
            "report": accepted_report,
            "artifact_file_sha256": accepted_artifact_file_sha256,
            "case_passed": accepted_case_passed
        },
        "registry": {
            "record": registry_record,
            "before_count": before_registry_count,
            "after_count": after_registry_count,
            "direct_readback_equal": direct_registry == registry_record,
            "reopened_readback_equal": reopened_registry == registry_record
        },
        "provenance": {
            "record": provenance_record,
            "before_count": before_provenance_count,
            "after_count": after_provenance_count,
            "direct_readback_equal": direct_provenance == provenance_record,
            "reopened_readback_equal": reopened_provenance == provenance_record
        },
        "divergence": {
            "candidate": divergence_candidate,
            "report": divergence_report,
            "case_passed": divergence_logged
        },
        "constant_collapse": {
            "candidate": constant_candidate,
            "report": constant_report,
            "case_passed": constant_collapse_logged
        },
        "rejections": {
            "direct": direct_rejections,
            "reopened": reopened_rejections,
            "reopened_contains_divergence_and_constant": rejections_reopened
        }
    });

    write_json_0600(&fsv_path, &evidence).context("write TASK-EK-018E run FSV")?;
    write_json_0600(&canonical_path, &evidence).context("write TASK-EK-018E canonical FSV")?;
    let fsv_sha256 = sha256_file(&fsv_path).context("hash TASK-EK-018E FSV")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "all_passed": all_passed,
            "fsv_path": fsv_path,
            "canonical_path": canonical_path,
            "fsv_sha256": fsv_sha256,
            "accepted_case_passed": accepted_case_passed,
            "registry_case_passed": registry_case_passed,
            "divergence_logged": divergence_logged,
            "constant_collapse_logged": constant_collapse_logged,
            "rejections_reopened": rejections_reopened
        }))?
    );
    ensure!(all_passed, "TASK-EK-018E learned-head synthesis FSV failed");
    Ok(())
}

fn proposal_fixture(created_at_unix_ms: i64) -> PendingEmbedderProposal {
    PendingEmbedderProposal {
        schema_version: EMBEDDER_PROPOSAL_SCHEMA_VERSION,
        proposal_id: proposal_id_for("task-ek-018e-e7-blind-spot"),
        candidate_name: "e7_blind_spot".to_string(),
        descriptor: AbsenceShapeDescriptor {
            input_modality: "e7_frozen_content_vector".to_string(),
            suggested_dim: 8,
            suggested_objective: "contrastive blind-spot projection".to_string(),
            reality_channel_for_falsification: "heldout-window:e7-blind-spot".to_string(),
            signal_magnitude: 0.44,
        },
        composite_score: 0.84,
        predicted_delta_cp_phi: 0.08,
        novelty_vs_existing_proposals: 0.90,
        source_signals: vec![EmbedderProposalSourceEvidence {
            kind: EmbedderAbsenceSignalKind::UnknownFingerprintCluster,
            source_ref: "unknown-cluster:e7-blind-spot".to_string(),
            signal_magnitude: 0.44,
            predicted_delta_cp_phi: 0.08,
            used_window_ids: vec!["proposal-window:e7:0".to_string()],
        }],
        created_at_unix_ms,
        source_of_truth_cf: CF_MEJEPA_EMBEDDER_PROPOSALS.to_string(),
    }
}

fn examples() -> Vec<FrozenBackboneExample> {
    vec![
        example("a0", 0, [0.90, 0.10, 0.20, 0.00, 0.15, 0.30]),
        example("a1", 0, [0.86, 0.12, 0.22, 0.02, 0.12, 0.28]),
        example("a2", 0, [0.88, 0.09, 0.19, 0.01, 0.16, 0.31]),
        example("b0", 1, [0.05, 0.82, 0.15, 0.76, 0.20, 0.12]),
        example("b1", 1, [0.04, 0.86, 0.14, 0.73, 0.18, 0.10]),
        example("b2", 1, [0.06, 0.80, 0.18, 0.78, 0.21, 0.11]),
    ]
}

fn example(id: &str, class_id: u32, values: [f32; 6]) -> FrozenBackboneExample {
    FrozenBackboneExample {
        example_id: id.to_string(),
        backbone_embedder_id: "E7".to_string(),
        vector: values.to_vec(),
        class_id,
        source_ref: format!("fixture:{id}"),
    }
}

fn comparison_fixture(
    candidate: &LearnedHeadCandidate,
    created_at_unix_ms: i64,
) -> Result<EmbedderCandidateHoldoutComparison> {
    let comparison = EmbedderCandidateHoldoutComparison {
        schema_version: EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
        proposal_id: candidate.proposal_id,
        candidate_id: candidate.candidate_id.clone(),
        candidate_name: candidate.candidate_name.clone(),
        candidate_architecture_signature: candidate.architecture_signature.clone(),
        candidate_artifact_sha256:
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        training_cert_chain_hash:
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        proposal_source_refs: candidate.proposal_source_refs.clone(),
        proposer_used_window_ids: candidate.proposer_used_window_ids.clone(),
        heldout_window_ids: vec!["heldout-window:e7:1".to_string()],
        mode_a: eval(
            0.900,
            BTreeMap::from([
                ("e7_blind_spot:python".to_string(), 0.848),
                ("mutation:python".to_string(), 0.862),
                ("typing:python".to_string(), 0.855),
            ]),
            "task-ek-018e-mode-a-baseline",
        )?,
        mode_b: eval(
            0.908,
            BTreeMap::from([
                ("e7_blind_spot:python".to_string(), 0.858),
                ("mutation:python".to_string(), 0.869),
                ("typing:python".to_string(), 0.862),
            ]),
            "task-ek-018e-mode-b-learned-head",
        )?,
        mode_c: eval(
            0.899,
            BTreeMap::from([
                ("e7_blind_spot:python".to_string(), 0.847),
                ("mutation:python".to_string(), 0.861),
                ("typing:python".to_string(), 0.854),
            ]),
            "task-ek-018e-mode-c-control",
        )?,
        created_at_unix_ms,
    };
    comparison.validate()?;
    Ok(comparison)
}

fn eval(global: f32, cells: BTreeMap<String, f32>, label: &str) -> Result<HoldoutEval> {
    Ok(HoldoutEval::try_new_with_cells(
        0.95,
        global,
        0.01,
        512,
        sha256_array(label.as_bytes()),
        cells,
    )?)
}

fn has_rejection(
    rows: &[context_graph_mejepa::EmbedderProposalRejectionRecord],
    architecture_signature: &str,
    reason_code: &str,
) -> bool {
    rows.iter().any(|row| {
        row.candidate_architecture_signature == architecture_signature
            && row.reason_code == reason_code
    })
}

fn proposal_id_for(label: &str) -> [u8; 16] {
    let digest = sha256_array(label.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(sha256_array(&bytes)))
}

fn sha256_array(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}
