use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::heal::promote::{HoldoutEval, ModeWinner};
use context_graph_mejepa::{
    algorithmic_registry_kind, count_cf, current_unix_ms, evaluate_algorithmic_embedder_synthesis,
    open_infer_rocksdb, read_dynamic_embedder_provenance, read_dynamic_embedder_registry,
    read_embedder_proposal_rejections, synthesize_algorithmic_embedder_candidate,
    write_dynamic_embedder_provenance_sync_readback, write_dynamic_embedder_registry_sync_readback,
    write_embedder_proposals_sync_readback, write_json_0600, AbsenceShapeDescriptor,
    AlgorithmicEmbedderCandidate, AlgorithmicEmbedderSynthesisConfig, AlgorithmicKernelTemplate,
    DynamicEmbedderProvenanceRecord, DynamicEmbedderRegistryRecord, EmbedderAbsenceSignalKind,
    EmbedderCandidateHoldoutComparison, EmbedderFalsificationGate, EmbedderProposalSourceEvidence,
    PendingEmbedderProposal, EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
    EMBEDDER_PROPOSAL_SCHEMA_VERSION, MEJEPA_ALGORITHMIC_EMBEDDER_COLLAPSED,
    MEJEPA_ALGORITHMIC_EMBEDDER_COMPILE_FAILED,
    MEJEPA_ALGORITHMIC_EMBEDDER_REJECTION_REPLAY_BLOCKED,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE, CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY,
    CF_MEJEPA_EMBEDDER_PROPOSALS, CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS,
};
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-ek-018d-algorithmic-embedder-synthesis-fsv";

fn main() -> Result<()> {
    let started_at = current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at}"));
    let infer_db_path = run_root.join("infer-rocksdb");
    let work_root = run_root.join("sandbox-work");
    let artifact_root = PathBuf::from(format!(
        "/zfs/archive/contextgraph/models/dynamic/task-ek-018d/run-{started_at}"
    ));
    let fsv_path = run_root.join("algorithmic_embedder_synthesis_fsv.json");
    let canonical_path = PathBuf::from(FSV_ROOT).join("algorithmic_embedder_synthesis_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-018D FSV run root")?;

    let db = open_infer_rocksdb(&infer_db_path).context("open TASK-EK-018D infer DB")?;
    let proposal = proposal_fixture(started_at);
    let proposal_write =
        write_embedder_proposals_sync_readback(db.as_ref(), std::slice::from_ref(&proposal))
            .context("write/readback pending embedder proposal")?;
    let proposal_count =
        count_cf(db.as_ref(), CF_MEJEPA_EMBEDDER_PROPOSALS).context("count proposals")?;

    let config = AlgorithmicEmbedderSynthesisConfig {
        work_root,
        artifact_root,
        ..AlgorithmicEmbedderSynthesisConfig::default()
    };

    let accepted_candidate = synthesize_algorithmic_embedder_candidate(
        &proposal,
        21,
        AlgorithmicKernelTemplate::IdentifierLength,
        started_at,
    )
    .context("synthesise identifier-length algorithmic candidate")?;
    let accepted_report = evaluate_algorithmic_embedder_synthesis(
        db.as_ref(),
        accepted_candidate.clone(),
        &config,
        Some(comparison_fixture(&accepted_candidate, started_at + 1)?),
        EmbedderFalsificationGate::default(),
    )
    .context("evaluate accepted algorithmic candidate")?;
    let accepted_artifact_sha256 = accepted_report
        .artifact_sha256()
        .context("accepted artifact sha missing")?
        .to_string();
    let accepted_artifact_file_sha256 =
        sha256_file(&accepted_report.sandbox.artifact_path).context("hash accepted artifact")?;
    ensure!(
        accepted_artifact_file_sha256 == accepted_artifact_sha256,
        "accepted artifact file sha mismatch"
    );

    let before_registry_count =
        count_cf(db.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY).context("registry before")?;
    let before_provenance_count = count_cf(db.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE)
        .context("provenance before")?;
    let registry_record = DynamicEmbedderRegistryRecord {
        id: accepted_candidate.candidate_id.clone(),
        registry_version: 2,
        kind: algorithmic_registry_kind(),
        dimension: accepted_candidate.dimension,
        route_languages: vec!["rust".to_string(), "python".to_string()],
        route_entity_types: vec!["Function".to_string(), "Method".to_string()],
        forward_artifact_path: accepted_report.sandbox.artifact_path.display().to_string(),
        forward_artifact_sha256: accepted_artifact_sha256.clone(),
        required_vram_bytes: fs::metadata(&accepted_report.sandbox.artifact_path)
            .context("accepted artifact metadata")?
            .len(),
        active: true,
        promoted_at_unix_ms: started_at,
    };
    registry_record
        .validate()
        .context("validate algorithmic registry record")?;
    write_dynamic_embedder_registry_sync_readback(db.as_ref(), &registry_record)
        .context("write/readback algorithmic registry")?;
    let accepted_decision = accepted_report
        .falsification_decision
        .as_ref()
        .context("accepted falsification decision missing")?;
    let provenance_record = DynamicEmbedderProvenanceRecord {
        id: accepted_candidate.candidate_id.clone(),
        registry_version: registry_record.registry_version,
        residual_signal_ref: accepted_candidate.proposal_source_refs.join(","),
        architecture_generator: format!(
            "task-ek-018d-alphaevolve-pattern-prompt-sha256:{}",
            sha256_hex(accepted_candidate.architecture_prompt.as_bytes())
        ),
        training_cert_chain_hash: accepted_report.training_cert_chain_hash.clone(),
        heldout_global_delta: accepted_decision.global_delta,
        heldout_min_cell_delta: accepted_decision.min_cell_delta,
        operator_approval_id: None,
        forward_pass_artifact_sha256: accepted_artifact_sha256.clone(),
        created_at_unix_ms: started_at + 2,
    };
    provenance_record
        .validate()
        .context("validate algorithmic provenance record")?;
    write_dynamic_embedder_provenance_sync_readback(db.as_ref(), &provenance_record)
        .context("write/readback algorithmic provenance")?;
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

    let compile_fail_candidate = synthesize_algorithmic_embedder_candidate(
        &proposal,
        22,
        AlgorithmicKernelTemplate::CompileFailureFixture,
        started_at + 3,
    )
    .context("synthesise compile-fail algorithmic candidate")?;
    let compile_fail_report = evaluate_algorithmic_embedder_synthesis(
        db.as_ref(),
        compile_fail_candidate.clone(),
        &config,
        None,
        EmbedderFalsificationGate::default(),
    )
    .context("evaluate compile-fail candidate")?;
    let compile_fail_replay_report = evaluate_algorithmic_embedder_synthesis(
        db.as_ref(),
        compile_fail_candidate.clone(),
        &config,
        None,
        EmbedderFalsificationGate::default(),
    )
    .context("evaluate compile-fail replay block")?;

    let constant_candidate = synthesize_algorithmic_embedder_candidate(
        &proposal,
        23,
        AlgorithmicKernelTemplate::ConstantCollapseFixture,
        started_at + 4,
    )
    .context("synthesise constant-collapse algorithmic candidate")?;
    let constant_report = evaluate_algorithmic_embedder_synthesis(
        db.as_ref(),
        constant_candidate.clone(),
        &config,
        None,
        EmbedderFalsificationGate::default(),
    )
    .context("evaluate constant-collapse candidate")?;
    let direct_rejections =
        read_embedder_proposal_rejections(db.as_ref()).context("read direct rejections")?;

    drop(db);
    let reopened = open_infer_rocksdb(&infer_db_path).context("reopen TASK-EK-018D infer DB")?;
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
        && accepted_report.sandbox.passed()
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
    let compile_failure_logged = !compile_fail_report.accepted
        && compile_fail_report.reason_code.as_deref()
            == Some(MEJEPA_ALGORITHMIC_EMBEDDER_COMPILE_FAILED)
        && compile_fail_report.rejection_key_hex.is_some()
        && compile_fail_report.sandbox.rustc_test_status != Some(0)
        && compile_fail_replay_report.reason_code.as_deref()
            == Some(MEJEPA_ALGORITHMIC_EMBEDDER_REJECTION_REPLAY_BLOCKED);
    let constant_collapse_logged = !constant_report.accepted
        && constant_report.reason_code.as_deref() == Some(MEJEPA_ALGORITHMIC_EMBEDDER_COLLAPSED)
        && constant_report
            .collapse
            .as_ref()
            .map(|collapse| collapse.collapsed)
            .unwrap_or(false)
        && constant_report.rejection_key_hex.is_some();
    let rejections_reopened = has_rejection(
        &reopened_rejections,
        &compile_fail_candidate.architecture_signature,
        MEJEPA_ALGORITHMIC_EMBEDDER_COMPILE_FAILED,
    ) && has_rejection(
        &reopened_rejections,
        &constant_candidate.architecture_signature,
        MEJEPA_ALGORITHMIC_EMBEDDER_COLLAPSED,
    );
    let all_passed = proposal_write.byte_identical_readback
        && proposal_count == 1
        && accepted_case_passed
        && registry_case_passed
        && compile_failure_logged
        && constant_collapse_logged
        && rejections_reopened
        && direct_rejections.len() == reopened_rejections.len();

    let evidence = json!({
        "task": "TASK-EK-018D",
        "issue": 345,
        "all_passed": all_passed,
        "sot": {
            "proposal_cf": CF_MEJEPA_EMBEDDER_PROPOSALS,
            "rejection_cf": CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS,
            "registry_cf": CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY,
            "provenance_cf": CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE,
            "infer_db_path": infer_db_path,
            "run_fsv_path": fsv_path,
            "canonical_path": canonical_path,
            "inner_llm_invoked": false,
            "system1_prompt_persisted": true
        },
        "proposal": {
            "write_summary": proposal_write,
            "count": proposal_count,
            "record": proposal
        },
        "accepted_identifier_length": {
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
        "compile_failure": {
            "candidate": compile_fail_candidate,
            "report": compile_fail_report,
            "replay_report": compile_fail_replay_report,
            "case_passed": compile_failure_logged
        },
        "constant_collapse": {
            "candidate": constant_candidate,
            "report": constant_report,
            "case_passed": constant_collapse_logged
        },
        "rejections": {
            "direct": direct_rejections,
            "reopened": reopened_rejections,
            "reopened_contains_compile_and_constant": rejections_reopened
        }
    });

    write_json_0600(&fsv_path, &evidence).context("write TASK-EK-018D run FSV")?;
    write_json_0600(&canonical_path, &evidence).context("write TASK-EK-018D canonical FSV")?;
    let fsv_sha256 = sha256_file(&fsv_path).context("hash TASK-EK-018D FSV")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "all_passed": all_passed,
            "fsv_path": fsv_path,
            "canonical_path": canonical_path,
            "fsv_sha256": fsv_sha256,
            "accepted_case_passed": accepted_case_passed,
            "registry_case_passed": registry_case_passed,
            "compile_failure_logged": compile_failure_logged,
            "constant_collapse_logged": constant_collapse_logged,
            "rejections_reopened": rejections_reopened
        }))?
    );
    ensure!(all_passed, "TASK-EK-018D algorithmic synthesis FSV failed");
    Ok(())
}

fn proposal_fixture(created_at_unix_ms: i64) -> PendingEmbedderProposal {
    PendingEmbedderProposal {
        schema_version: EMBEDDER_PROPOSAL_SCHEMA_VERSION,
        proposal_id: proposal_id_for("task-ek-018d-identifier-length-gap"),
        candidate_name: "identifier_length_gap".to_string(),
        descriptor: AbsenceShapeDescriptor {
            input_modality: "rust_identifier_tokens".to_string(),
            suggested_dim: 8,
            suggested_objective: "separate identifier-length-sensitive failures".to_string(),
            reality_channel_for_falsification: "heldout-window:identifier-length".to_string(),
            signal_magnitude: 0.42,
        },
        composite_score: 0.82,
        predicted_delta_cp_phi: 0.07,
        novelty_vs_existing_proposals: 0.91,
        source_signals: vec![EmbedderProposalSourceEvidence {
            kind: EmbedderAbsenceSignalKind::MincutStructuralHole,
            source_ref: "mincut:identifier-length-structural-hole".to_string(),
            signal_magnitude: 0.42,
            predicted_delta_cp_phi: 0.07,
            used_window_ids: vec!["proposal-window:python-forward-cache:000-127".to_string()],
        }],
        created_at_unix_ms,
        source_of_truth_cf: CF_MEJEPA_EMBEDDER_PROPOSALS.to_string(),
    }
}

fn comparison_fixture(
    candidate: &AlgorithmicEmbedderCandidate,
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
        heldout_window_ids: vec!["heldout-window:python-forward-cache:256-390".to_string()],
        mode_a: eval(
            0.900,
            BTreeMap::from([
                ("identifier_length:python".to_string(), 0.850),
                ("mutation:python".to_string(), 0.861),
                ("typing:python".to_string(), 0.855),
            ]),
            "task-ek-018d-mode-a-baseline",
        )?,
        mode_b: eval(
            0.907,
            BTreeMap::from([
                ("identifier_length:python".to_string(), 0.859),
                ("mutation:python".to_string(), 0.868),
                ("typing:python".to_string(), 0.861),
            ]),
            "task-ek-018d-mode-b-plus-algorithmic",
        )?,
        mode_c: eval(
            0.899,
            BTreeMap::from([
                ("identifier_length:python".to_string(), 0.848),
                ("mutation:python".to_string(), 0.860),
                ("typing:python".to_string(), 0.854),
            ]),
            "task-ek-018d-mode-c-control",
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
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(sha256_array(bytes))
}

fn sha256_array(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}
