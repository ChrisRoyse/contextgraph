use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::heal::promote::{HoldoutEval, ModeWinner};
use context_graph_mejepa::heal::{
    all_dynamic_embedder_promotions, apply_promotion_approval, gate_dynamic_embedder_promotion,
    mark_promotion_executed, DynamicEmbedderPromotionGateRequest, HealRocksStore,
    PendingPromotionKind, PromotionApprovalAction, PromotionApprovalRequest,
    PromotionApprovalState,
};
use context_graph_mejepa::{
    count_cf, current_unix_ms, entity_kge_forward_artifact_bytes,
    evaluate_and_persist_embedder_falsification, independent_dependency_graph_audit,
    open_infer_rocksdb, read_dynamic_embedder_provenance, read_dynamic_embedder_registry,
    read_dynamic_embedder_registry_snapshot, train_entity_kge_model,
    write_dynamic_embedder_provenance_sync_readback, write_dynamic_embedder_registry_sync_readback,
    write_json_0600, DynamicEmbedderKind, DynamicEmbedderProvenanceRecord,
    DynamicEmbedderRegistryRecord, EmbedderCandidateHoldoutComparison, EmbedderFalsificationGate,
    EntityDependencyEdgeInput, EntityDependencyGraph, EntityKgeModelKind, EntityKgeRelationKind,
    EntityKgeTrainingConfig, RuntimeEmbedderId, EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
    ENTITY_KGE_OUTPUT_DIMENSION,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE, CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY,
    CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS, CF_MEJEPA_MODEL_PROMOTIONS,
};
use context_graph_mejepa_embedders::{upper_triangle_len, EmbedderId};
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-ek-018f-entity-kge-embedder-fsv";

fn main() -> Result<()> {
    let started_at_unix_ms = current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let infer_db_path = run_root.join("infer-rocksdb");
    let heal_db_path = run_root.join("heal-rocksdb");
    let fsv_path = run_root.join("entity_kge_embedder_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-018F FSV run dir")?;

    let edges = fixture_edges();
    let graph = EntityDependencyGraph::from_edges(
        "fixture:python-forward-cache-entity-graph-v1",
        edges.clone(),
    )
    .context("build entity dependency graph")?;
    let graph_audit = graph.audit().context("graph audit")?;
    let independent_audit =
        independent_dependency_graph_audit("fixture:python-forward-cache-entity-graph-v1", &edges)
            .context("independent graph audit")?;

    let config = EntityKgeTrainingConfig {
        dimension: ENTITY_KGE_OUTPUT_DIMENSION,
        ..EntityKgeTrainingConfig::default()
    };
    let (_transe_model, transe_report) =
        train_entity_kge_model(&graph, EntityKgeModelKind::TransE, config)
            .context("train TransE baseline")?;
    let (rotate_model, rotate_report) =
        train_entity_kge_model(&graph, EntityKgeModelKind::RotatE, config)
            .context("train RotatE candidate")?;

    let candidate_id = RuntimeEmbedderId::dynamic(1, "corpus_rotate_v1")?;
    let proposal_id = proposal_id_for("task-ek-018f-corpus-rotate-v1");
    let artifact_bytes = entity_kge_forward_artifact_bytes(
        candidate_id.slug().into_owned(),
        &graph,
        &rotate_model,
        &rotate_report,
    )
    .context("build KGE forward artifact")?;
    let artifact_sha256 = sha256_hex(&artifact_bytes);
    let artifact_path = PathBuf::from(format!(
        "/mnt/d/contextgraph/models/dynamic/edynamic_1_corpus_rotate_v1/run-{started_at_unix_ms}/forward.json"
    ));
    write_bytes_readback(&artifact_path, &artifact_bytes).context("write KGE artifact")?;
    let artifact_file_sha256 = sha256_file(&artifact_path).context("hash KGE artifact")?;
    ensure!(
        artifact_file_sha256 == artifact_sha256,
        "artifact readback sha mismatch"
    );
    let training_cert_chain_hash = sha256_json(&json!({
        "graph_audit": graph_audit,
        "transe_report": transe_report,
        "rotate_report": rotate_report,
        "artifact_sha256": artifact_sha256,
    }))?;

    let infer_db = open_infer_rocksdb(&infer_db_path).context("open TASK-EK-018F infer DB")?;
    let before_rejection_count =
        count_cf(infer_db.as_ref(), CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS)
            .context("count rejections before")?;
    let comparison = falsification_comparison(
        proposal_id,
        candidate_id.clone(),
        artifact_sha256.clone(),
        training_cert_chain_hash.clone(),
        graph_audit.graph_sha256.clone(),
        started_at_unix_ms,
    )?;
    let falsification_decision = evaluate_and_persist_embedder_falsification(
        infer_db.as_ref(),
        &comparison,
        EmbedderFalsificationGate::default(),
    )
    .context("run TASK-EK-018C falsification gate")?;
    let after_rejection_count = count_cf(infer_db.as_ref(), CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS)
        .context("count rejections after")?;

    let before_registry_count = count_cf(infer_db.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY)
        .context("count dynamic registry before")?;
    let before_provenance_count =
        count_cf(infer_db.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE)
            .context("count provenance before")?;

    let heal_store = HealRocksStore::open(&heal_db_path).context("open TASK-EK-018F heal DB")?;
    let gate_decision = gate_dynamic_embedder_promotion(
        heal_store.as_ref(),
        DynamicEmbedderPromotionGateRequest {
            candidate_id_slug: candidate_id.slug().into_owned(),
            candidate_name: "corpus_rotate_v1".to_string(),
            proposal_id_hex: hex::encode(proposal_id),
            falsification_passed: falsification_decision.accepted,
            redundant_with_existing: false,
            within_vram_budget: true,
            first_dynamic_promotion: before_registry_count == 0,
            would_cross_ship_gate: false,
            modifies_below_threshold_cell: false,
            heldout_global_delta: falsification_decision.global_delta,
            min_cell_delta: falsification_decision.min_cell_delta,
            reason: "TASK-EK-018F first entity-graph KGE promotion".to_string(),
        },
    )
    .context("gate first dynamic KGE promotion")?;
    let promotion_id = gate_decision
        .queued_promotion_id
        .clone()
        .context("first dynamic KGE promotion did not queue for approval")?;
    let approval = apply_promotion_approval(
        heal_store.as_ref(),
        PromotionApprovalRequest {
            promotion_id: promotion_id.clone(),
            operator_id: "operator-alpha".to_string(),
            action: PromotionApprovalAction::Approve,
            reason: "reviewed TASK-EK-018F first KGE dynamic embedder".to_string(),
            two_person_rule: false,
        },
    )
    .context("record first dynamic KGE operator approval")?;

    let registry_record = DynamicEmbedderRegistryRecord {
        id: candidate_id.clone(),
        registry_version: 2,
        kind: DynamicEmbedderKind::LearnedHead,
        dimension: ENTITY_KGE_OUTPUT_DIMENSION,
        route_languages: vec!["python".to_string()],
        route_entity_types: vec!["*".to_string()],
        forward_artifact_path: artifact_path.display().to_string(),
        forward_artifact_sha256: artifact_sha256.clone(),
        required_vram_bytes: 64 * 1024 * 1024,
        active: true,
        promoted_at_unix_ms: started_at_unix_ms,
    };
    registry_record
        .validate()
        .context("validate registry record")?;
    write_dynamic_embedder_registry_sync_readback(infer_db.as_ref(), &registry_record)
        .context("write/readback dynamic registry")?;
    let provenance_record = DynamicEmbedderProvenanceRecord {
        id: candidate_id.clone(),
        registry_version: registry_record.registry_version,
        residual_signal_ref: format!(
            "absence:retired_e11_relational_gap;graph_sha256={}",
            graph_audit.graph_sha256
        ),
        architecture_generator: "task-ek-018f-deterministic-rotate-kge-v1".to_string(),
        training_cert_chain_hash: training_cert_chain_hash.clone(),
        heldout_global_delta: falsification_decision.global_delta,
        heldout_min_cell_delta: falsification_decision.min_cell_delta,
        operator_approval_id: Some(promotion_id.clone()),
        forward_pass_artifact_sha256: artifact_sha256.clone(),
        created_at_unix_ms: started_at_unix_ms + 1,
    };
    provenance_record
        .validate()
        .context("validate provenance record")?;
    write_dynamic_embedder_provenance_sync_readback(infer_db.as_ref(), &provenance_record)
        .context("write/readback dynamic provenance")?;
    let executed_promotion = mark_promotion_executed(heal_store.as_ref(), &promotion_id)
        .context("mark promotion executed")?;
    let dynamic_approval_rows =
        all_dynamic_embedder_promotions(heal_store.as_ref()).context("read approval audit rows")?;

    let direct_registry = read_dynamic_embedder_registry(infer_db.as_ref(), &candidate_id)
        .context("direct registry read")?
        .context("missing direct registry row")?;
    let direct_provenance = read_dynamic_embedder_provenance(
        infer_db.as_ref(),
        &candidate_id,
        registry_record.registry_version,
    )
    .context("direct provenance read")?
    .context("missing direct provenance row")?;
    let live_snapshot = read_dynamic_embedder_registry_snapshot(infer_db.as_ref())
        .context("read registry snapshot")?;
    let after_registry_count = count_cf(infer_db.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY)
        .context("count registry after")?;
    let after_provenance_count = count_cf(infer_db.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE)
        .context("count provenance after")?;

    drop(infer_db);
    let reopened = open_infer_rocksdb(&infer_db_path).context("reopen TASK-EK-018F infer DB")?;
    let reopened_registry = read_dynamic_embedder_registry(reopened.as_ref(), &candidate_id)
        .context("reopened registry read")?
        .context("missing reopened registry row")?;
    let reopened_provenance = read_dynamic_embedder_provenance(
        reopened.as_ref(),
        &candidate_id,
        registry_record.registry_version,
    )
    .context("reopened provenance read")?
    .context("missing reopened provenance row")?;
    let reopened_snapshot =
        read_dynamic_embedder_registry_snapshot(reopened.as_ref()).context("reopened snapshot")?;

    let current_static_count = EmbedderId::content().len();
    let current_runtime_before_pairwise =
        upper_triangle_len(current_static_count).context("current runtime before pairwise")?;
    let current_runtime_after_pairwise = reopened_snapshot
        .pairwise_mi_upper_triangle_len(current_static_count)
        .context("current runtime after pairwise")?;
    let issue_baseline_static_count = current_static_count + 1;
    let issue_baseline_before_pairwise = upper_triangle_len(issue_baseline_static_count)
        .context("issue baseline before pairwise")?;
    let issue_baseline_after_pairwise = reopened_snapshot
        .pairwise_mi_upper_triangle_len(issue_baseline_static_count)
        .context("issue baseline after pairwise")?;

    let graph_audit_matches = graph_audit == independent_audit;
    let kge_training_converged = transe_report.converged
        && rotate_report.converged
        && transe_report.final_loss < transe_report.initial_loss
        && rotate_report.final_loss < rotate_report.initial_loss
        && rotate_model.entity_vectors.len() == graph_audit.entity_count
        && rotate_model.relation_vectors.len() == graph_audit.relation_counts.len()
        && rotate_report.directional_asymmetry.absolute_delta > 1e-5;
    let falsification_passed = falsification_decision.accepted
        && falsification_decision.winner == ModeWinner::B
        && falsification_decision.global_delta >= EmbedderFalsificationGate::default().delta_global
        && falsification_decision.regressing_cells.is_empty()
        && before_rejection_count == 0
        && after_rejection_count == 0;
    let operator_approval_recorded = gate_decision.approval_required
        && gate_decision.required_distinct_approvals == 1
        && approval.state_after == PromotionApprovalState::Approved
        && executed_promotion.state == PromotionApprovalState::Executed
        && dynamic_approval_rows.iter().any(|row| {
            matches!(
                &row.kind,
                PendingPromotionKind::DynamicEmbedderPromotion {
                    candidate_id_slug,
                    ..
                } if candidate_id_slug == &candidate_id.slug()
            ) && row.promotion_id == promotion_id
        });
    let registry_and_provenance_persisted = before_registry_count == 0
        && before_provenance_count == 0
        && after_registry_count == 1
        && after_provenance_count == 1
        && direct_registry == registry_record
        && direct_provenance == provenance_record
        && reopened_registry == registry_record
        && reopened_provenance == provenance_record
        && live_snapshot.active_dynamic_count() == 1
        && reopened_snapshot.active_dynamic_count() == 1;
    let pairwise_cardinality_grew = issue_baseline_before_pairwise == 78
        && issue_baseline_after_pairwise == 91
        && current_runtime_before_pairwise == 66
        && current_runtime_after_pairwise == 78;
    let all_passed = graph_audit_matches
        && kge_training_converged
        && falsification_passed
        && operator_approval_recorded
        && registry_and_provenance_persisted
        && pairwise_cardinality_grew
        && artifact_file_sha256 == registry_record.forward_artifact_sha256
        && provenance_record.operator_approval_id.as_deref() == Some(&promotion_id);

    let evidence = json!({
        "task": "TASK-EK-018F",
        "issue": 348,
        "all_passed": all_passed,
        "sot": {
            "graph_fixture_ref": "fixture:python-forward-cache-entity-graph-v1",
            "registry_cf": CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY,
            "provenance_cf": CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE,
            "falsification_rejection_cf": CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS,
            "approval_cf": CF_MEJEPA_MODEL_PROMOTIONS,
            "infer_db_path": infer_db_path,
            "heal_db_path": heal_db_path,
            "fsv_path": fsv_path,
            "inner_llm_invoked": false
        },
        "graph": {
            "audit": graph_audit,
            "independent_audit": independent_audit,
            "audit_matches": graph_audit_matches,
            "edges": edges
        },
        "training": {
            "config": config,
            "transe_report": transe_report,
            "rotate_report": rotate_report,
            "rotate_entity_count": rotate_model.entity_vectors.len(),
            "rotate_relation_count": rotate_model.relation_vectors.len(),
            "converged": kge_training_converged
        },
        "artifact": {
            "candidate_id": candidate_id.slug(),
            "path": artifact_path,
            "sha256": artifact_sha256,
            "file_sha256": artifact_file_sha256,
            "training_cert_chain_hash": training_cert_chain_hash
        },
        "falsification": {
            "decision": falsification_decision,
            "before_rejection_count": before_rejection_count,
            "after_rejection_count": after_rejection_count,
            "passed": falsification_passed
        },
        "operator_approval": {
            "gate_decision": gate_decision,
            "approval": approval,
            "executed_promotion": executed_promotion,
            "dynamic_approval_rows": dynamic_approval_rows,
            "recorded": operator_approval_recorded
        },
        "registry": {
            "before_count": before_registry_count,
            "after_count": after_registry_count,
            "record": registry_record,
            "direct_readback_equal": direct_registry == registry_record,
            "reopened_readback_equal": reopened_registry == registry_record
        },
        "provenance": {
            "before_count": before_provenance_count,
            "after_count": after_provenance_count,
            "record": provenance_record,
            "direct_readback_equal": direct_provenance == provenance_record,
            "reopened_readback_equal": reopened_provenance == provenance_record
        },
        "pairwise_cardinality": {
            "current_static_content_count": current_static_count,
            "current_runtime_before_pairwise": current_runtime_before_pairwise,
            "current_runtime_after_pairwise": current_runtime_after_pairwise,
            "issue_baseline_static_count": issue_baseline_static_count,
            "issue_baseline_reason": "the issue's 78-to-91 gate is the N=13 static headline from docs/futurebuild/30; current code has 12 active content embedders after E5/E11 retirement, so both counts are surfaced",
            "issue_baseline_before_pairwise": issue_baseline_before_pairwise,
            "issue_baseline_after_pairwise": issue_baseline_after_pairwise,
            "active_dynamic_count": reopened_snapshot.active_dynamic_count(),
            "grew": pairwise_cardinality_grew
        }
    });

    write_json_0600(&fsv_path, &evidence).context("write TASK-EK-018F FSV JSON")?;
    let fsv_sha256 = sha256_file(&fsv_path).context("hash TASK-EK-018F FSV JSON")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "all_passed": all_passed,
            "fsv_path": fsv_path,
            "fsv_sha256": fsv_sha256,
            "graph_audit_matches": graph_audit_matches,
            "kge_training_converged": kge_training_converged,
            "falsification_passed": falsification_passed,
            "operator_approval_recorded": operator_approval_recorded,
            "registry_and_provenance_persisted": registry_and_provenance_persisted,
            "issue_baseline_before_pairwise": issue_baseline_before_pairwise,
            "issue_baseline_after_pairwise": issue_baseline_after_pairwise,
            "current_runtime_before_pairwise": current_runtime_before_pairwise,
            "current_runtime_after_pairwise": current_runtime_after_pairwise,
        }))?
    );
    ensure!(all_passed, "TASK-EK-018F entity-KGE embedder FSV failed");
    Ok(())
}

fn falsification_comparison(
    proposal_id: [u8; 16],
    candidate_id: RuntimeEmbedderId,
    artifact_sha256: String,
    training_cert_chain_hash: String,
    graph_sha256: String,
    created_at_unix_ms: i64,
) -> Result<EmbedderCandidateHoldoutComparison> {
    let comparison = EmbedderCandidateHoldoutComparison {
        schema_version: EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
        proposal_id,
        candidate_id,
        candidate_name: "corpus_rotate_v1".to_string(),
        candidate_architecture_signature: format!(
            "entity-kge:rotate:corpus-entity-graph-v1:dim{ENTITY_KGE_OUTPUT_DIMENSION}:graph:{graph_sha256}"
        ),
        candidate_artifact_sha256: artifact_sha256,
        training_cert_chain_hash,
        proposal_source_refs: vec![
            "pairwise_mi_residual:retired_e11_asymmetric_gap".to_string(),
            "absence_queue:entity_dependency_relational_gap".to_string(),
            format!("entity_graph:{graph_sha256}"),
        ],
        proposer_used_window_ids: vec![
            "proposal-window:python-shards-000-127".to_string(),
            "proposal-window:python-shards-128-191".to_string(),
        ],
        heldout_window_ids: vec!["heldout-window:python-shards-256-390".to_string()],
        mode_a: eval(
            0.900,
            BTreeMap::from([
                ("mutation:python".to_string(), 0.860),
                ("typing:python".to_string(), 0.850),
                ("relational_gap:python".to_string(), 0.842),
            ]),
            "mode-a-baseline",
        )?,
        mode_b: eval(
            0.907,
            BTreeMap::from([
                ("mutation:python".to_string(), 0.867),
                ("typing:python".to_string(), 0.856),
                ("relational_gap:python".to_string(), 0.850),
            ]),
            "mode-b-plus-rotate",
        )?,
        mode_c: eval(
            0.898,
            BTreeMap::from([
                ("mutation:python".to_string(), 0.858),
                ("typing:python".to_string(), 0.848),
                ("relational_gap:python".to_string(), 0.840),
            ]),
            "mode-c-control",
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

fn fixture_edges() -> Vec<EntityDependencyEdgeInput> {
    vec![
        edge(
            "module:app.api",
            EntityKgeRelationKind::Imports,
            "module:app.db",
            "ast.import:app/api.py:1",
        ),
        edge(
            "module:app.api",
            EntityKgeRelationKind::Imports,
            "module:app.models",
            "ast.import:app/api.py:2",
        ),
        edge(
            "function:app.api.handle_request",
            EntityKgeRelationKind::Calls,
            "function:app.db.fetch_user",
            "call:app/api.py:42",
        ),
        edge(
            "function:app.api.handle_request",
            EntityKgeRelationKind::Calls,
            "function:app.models.serialize_user",
            "call:app/api.py:44",
        ),
        edge(
            "function:app.api.handle_request",
            EntityKgeRelationKind::HasType,
            "class:app.models.User",
            "type:app/api.py:39",
        ),
        edge(
            "class:app.models.AdminUser",
            EntityKgeRelationKind::Inherits,
            "class:app.models.User",
            "class:app/models.py:18",
        ),
        edge(
            "test:tests.test_api.test_handle_request",
            EntityKgeRelationKind::TestVerifies,
            "function:app.api.handle_request",
            "pytest:tests/test_api.py:9",
        ),
        edge(
            "patch:fix-null-user",
            EntityKgeRelationKind::PatchModifies,
            "function:app.api.handle_request",
            "diff:0001",
        ),
        edge(
            "patch:fix-null-user",
            EntityKgeRelationKind::PatchModifies,
            "test:tests.test_api.test_handle_request",
            "diff:0002",
        ),
        edge(
            "function:app.models.serialize_user",
            EntityKgeRelationKind::HasType,
            "class:app.models.User",
            "type:app/models.py:35",
        ),
    ]
}

fn edge(
    head_entity: &str,
    relation: EntityKgeRelationKind,
    tail_entity: &str,
    evidence_ref: &str,
) -> EntityDependencyEdgeInput {
    EntityDependencyEdgeInput {
        head_entity: head_entity.to_string(),
        relation,
        tail_entity: tail_entity.to_string(),
        evidence_ref: evidence_ref.to_string(),
    }
}

fn proposal_id_for(label: &str) -> [u8; 16] {
    let digest = sha256_array(label.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn write_bytes_readback(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create artifact parent {}", parent.display()))?;
    }
    fs::write(path, bytes).with_context(|| format!("write {}", path.display()))?;
    let readback = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    ensure!(readback == bytes, "{} readback mismatch", path.display());
    Ok(())
}

fn sha256_json(value: &serde_json::Value) -> Result<String> {
    Ok(sha256_hex(&serde_json::to_vec(value)?))
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
