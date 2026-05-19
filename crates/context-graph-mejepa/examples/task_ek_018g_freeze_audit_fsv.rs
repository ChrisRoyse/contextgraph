use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::heal::promote::{HoldoutEval, ModeWinner};
use context_graph_mejepa::heal::{HealRocksStore, WitnessChainAppender};
use context_graph_mejepa::{
    count_cf, current_unix_ms, dynamic_embedder_record_sha256,
    dynamic_embedder_replay_decision_sha256, evaluate_embedder_candidate_falsification,
    frozen_adapter_for_dynamic_embedders, open_infer_rocksdb, read_dynamic_embedder_freeze_audit,
    read_dynamic_embedder_provenance, replay_dynamic_embedder_promotion, run_gradient_hook,
    write_dynamic_embedder_freeze_audit_sync_readback,
    write_dynamic_embedder_provenance_sync_readback, write_dynamic_embedder_registry_sync_readback,
    write_json_0600, DynamicEmbedderFreezeAuditRecord, DynamicEmbedderKind,
    DynamicEmbedderPromotionReplayRecord, DynamicEmbedderProvenanceRecord,
    DynamicEmbedderRegistryRecord, EmbedderCandidateHoldoutComparison, EmbedderFalsificationGate,
    GradStore, RuntimeEmbedderId, RuntimeRoutingTable, TargetProvenance, TensorId,
    DYNAMIC_EMBEDDER_FREEZE_AUDIT_SCHEMA_VERSION, DYNAMIC_EMBEDDER_PROMOTION_WITNESS_OP,
    EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
};
use context_graph_mejepa_cf::{
    CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE, CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY,
};
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-ek-018g-freeze-audit-fsv";

fn main() -> Result<()> {
    let started_at_unix_ms = current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let infer_db_path = run_root.join("infer-rocksdb");
    let heal_db_path = run_root.join("heal-rocksdb");
    let witness_chain_path = run_root.join("dynamic-embedder-witness-chain.bin");
    let fsv_path = run_root.join("freeze_audit_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-018G FSV run dir")?;

    let db = open_infer_rocksdb(&infer_db_path).context("open TASK-EK-018G infer DB")?;
    let storage = HealRocksStore::open(&heal_db_path).context("open TASK-EK-018G heal store")?;
    let mut witness_chain =
        WitnessChainAppender::new(witness_chain_path.clone()).context("open witness appender")?;
    let candidate_id = RuntimeEmbedderId::dynamic(1, "freeze_guarded_v1")?;
    let promotion_id = "promotion-freeze-guarded-v1".to_string();
    let comparison = comparison_fixture(candidate_id.clone(), started_at_unix_ms)?;
    let gate = EmbedderFalsificationGate::default();
    let decision = evaluate_embedder_candidate_falsification(&comparison, gate)
        .context("evaluate held-out falsification decision")?;
    ensure!(
        decision.accepted,
        "fixture promotion decision must be accepted"
    );
    let decision_sha256 = dynamic_embedder_replay_decision_sha256(&decision)?;
    let replay_record = DynamicEmbedderPromotionReplayRecord {
        schema_version: DYNAMIC_EMBEDDER_FREEZE_AUDIT_SCHEMA_VERSION,
        promotion_id: promotion_id.clone(),
        comparison: comparison.clone(),
        gate,
        expected_decision_sha256: decision_sha256.clone(),
    };
    replay_record.validate().context("validate replay record")?;
    let replay_record_sha256 = dynamic_embedder_record_sha256(&replay_record)?;
    let replay_report = replay_dynamic_embedder_promotion(&replay_record)
        .context("replay dynamic embedder promotion")?;

    let registry_record = DynamicEmbedderRegistryRecord {
        id: candidate_id.clone(),
        registry_version: 2,
        kind: DynamicEmbedderKind::LearnedHead,
        dimension: 256,
        route_languages: vec!["python".to_string()],
        route_entity_types: vec!["*".to_string()],
        forward_artifact_path:
            "/zfs/archive/contextgraph/models/dynamic/edynamic_1_freeze_guarded_v1/forward.bin"
                .to_string(),
        forward_artifact_sha256: comparison.candidate_artifact_sha256.clone(),
        required_vram_bytes: 128 * 1024 * 1024,
        active: true,
        promoted_at_unix_ms: started_at_unix_ms,
    };
    registry_record
        .validate()
        .context("validate registry record")?;
    let provenance_record = DynamicEmbedderProvenanceRecord {
        id: candidate_id.clone(),
        registry_version: registry_record.registry_version,
        residual_signal_ref: "absence:freeze-audit-fsv-relational-gap".to_string(),
        architecture_generator: "task-ek-018g-freeze-audit-fsv-generator".to_string(),
        training_cert_chain_hash: comparison.training_cert_chain_hash.clone(),
        heldout_global_delta: decision.global_delta,
        heldout_min_cell_delta: decision.min_cell_delta,
        operator_approval_id: Some(promotion_id.clone()),
        forward_pass_artifact_sha256: comparison.candidate_artifact_sha256.clone(),
        created_at_unix_ms: started_at_unix_ms + 1,
    };
    provenance_record
        .validate()
        .context("validate provenance record")?;

    let before_registry_count = count_cf(db.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY)
        .context("count registry before")?;
    let before_provenance_count = count_cf(db.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE)
        .context("count provenance before")?;
    write_dynamic_embedder_registry_sync_readback(db.as_ref(), &registry_record)
        .context("write registry")?;
    write_dynamic_embedder_provenance_sync_readback(db.as_ref(), &provenance_record)
        .context("write provenance")?;
    let direct_provenance = read_dynamic_embedder_provenance(
        db.as_ref(),
        &candidate_id,
        provenance_record.registry_version,
    )
    .context("read provenance")?
    .context("missing provenance row")?;
    let provenance_record_sha256 = dynamic_embedder_record_sha256(&direct_provenance)?;

    let weights_sha = hex_to_array(&comparison.candidate_artifact_sha256)?;
    let evaluation_sha = hex_to_array(&decision_sha256)?;
    let witness_offset = witness_chain
        .append_model_event_checked(
            storage.as_ref(),
            DYNAMIC_EMBEDDER_PROMOTION_WITNESS_OP,
            weights_sha,
            evaluation_sha,
        )
        .context("append dynamic promotion witness")?;
    let witness_bytes = fs::read(&witness_chain_path).context("read witness chain")?;
    let witness_verification = context_graph_witness::verify_chain_bytes(&witness_bytes)
        .context("verify witness chain")?;
    let witness_chain_sha256 = sha256_hex(&witness_bytes);

    let adapter = frozen_adapter_for_dynamic_embedders(
        target_provenance(),
        std::slice::from_ref(&candidate_id),
    )
    .context("build dynamic freeze adapter")?;
    let clean_guard =
        run_gradient_hook(&adapter, &GradStore::default()).context("clean dynamic freeze guard")?;
    let mut leaking_grads = GradStore::default();
    leaking_grads
        .insert_norm(
            TensorId(format!("{}:frozen_forward_parameters", candidate_id.slug())),
            1.0,
        )
        .context("insert leaking dynamic grad")?;
    let leak_error =
        run_gradient_hook(&adapter, &leaking_grads).expect_err("dynamic gradient leak must fail");

    let audit_record = DynamicEmbedderFreezeAuditRecord {
        schema_version: DYNAMIC_EMBEDDER_FREEZE_AUDIT_SCHEMA_VERSION,
        id: candidate_id.clone(),
        registry_version: provenance_record.registry_version,
        promotion_id: promotion_id.clone(),
        provenance_record_sha256: provenance_record_sha256.clone(),
        witness_chain_path: witness_chain_path.display().to_string(),
        witness_offset,
        witness_chain_sha256: witness_chain_sha256.clone(),
        replay_record_sha256: replay_record_sha256.clone(),
        replay_decision_sha256: replay_report.replayed_decision_sha256.clone(),
        gradient_guard_clean_passed: clean_guard.passes,
        gradient_guard_leak_code: leak_error.code().to_string(),
        created_at_unix_ms: started_at_unix_ms + 2,
        source_of_truth_cf: CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE.to_string(),
    };
    audit_record.validate().context("validate freeze audit")?;
    let audit_key = write_dynamic_embedder_freeze_audit_sync_readback(db.as_ref(), &audit_record)
        .context("write freeze audit")?;
    let direct_audit = read_dynamic_embedder_freeze_audit(
        db.as_ref(),
        &candidate_id,
        audit_record.registry_version,
        &promotion_id,
    )
    .context("read freeze audit")?
    .context("missing freeze audit row")?;
    let after_provenance_count = count_cf(db.as_ref(), CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE)
        .context("count provenance after")?;
    let table = RuntimeRoutingTable::new(2, vec![registry_record.clone()])
        .context("construct runtime table")?;

    drop(db);
    let reopened = open_infer_rocksdb(&infer_db_path).context("reopen infer DB")?;
    let reopened_provenance = read_dynamic_embedder_provenance(
        reopened.as_ref(),
        &candidate_id,
        provenance_record.registry_version,
    )
    .context("reopened provenance")?
    .context("missing reopened provenance")?;
    let reopened_audit = read_dynamic_embedder_freeze_audit(
        reopened.as_ref(),
        &candidate_id,
        audit_record.registry_version,
        &promotion_id,
    )
    .context("reopened freeze audit")?
    .context("missing reopened freeze audit")?;

    let provenance_has_required_fields = direct_provenance.residual_signal_ref.len() > 8
        && direct_provenance.architecture_generator.len() > 8
        && direct_provenance.training_cert_chain_hash.len() == 64
        && direct_provenance.heldout_global_delta >= gate.delta_global
        && direct_provenance.operator_approval_id.as_deref() == Some(&promotion_id)
        && direct_provenance.forward_pass_artifact_sha256.len() == 64;
    let witness_entry_written = witness_offset == 0
        && witness_verification.entries == 1
        && witness_verification
            .last_entry
            .as_ref()
            .map(|entry| {
                entry.action_hash
                    == action_hash(
                        DYNAMIC_EMBEDDER_PROMOTION_WITNESS_OP,
                        weights_sha,
                        evaluation_sha,
                    )
            })
            .unwrap_or(false);
    let guard_fires = clean_guard.passes
        && adapter.instrument_count() == 1
        && leak_error.code() == "MEJEPA_PRED_FROZEN_TARGET_GRAD";
    let replay_reproduces = replay_report.reproduced
        && replay_report.decision.accepted
        && replay_report.decision.winner == ModeWinner::B
        && replay_report.replayed_decision_sha256 == decision_sha256;
    let persisted_audit = direct_audit == audit_record
        && reopened_audit == audit_record
        && reopened_provenance == provenance_record
        && after_provenance_count == before_provenance_count + 2;
    let all_passed = before_registry_count == 0
        && provenance_has_required_fields
        && witness_entry_written
        && guard_fires
        && replay_reproduces
        && persisted_audit
        && table.active_dynamic_count() == 1;

    let evidence = json!({
        "task": "TASK-EK-018G",
        "issue": 346,
        "all_passed": all_passed,
        "sot": {
            "registry_cf": CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY,
            "provenance_cf": CF_MEJEPA_DYNAMIC_EMBEDDER_PROVENANCE,
            "infer_db_path": infer_db_path,
            "heal_db_path": heal_db_path,
            "witness_chain_path": witness_chain_path,
            "fsv_path": fsv_path,
            "inner_llm_invoked": false
        },
        "provenance": {
            "before_count": before_provenance_count,
            "after_count": after_provenance_count,
            "record": provenance_record,
            "record_sha256": provenance_record_sha256,
            "has_required_fields": provenance_has_required_fields,
            "reopened_equal": reopened_provenance == direct_provenance
        },
        "witness": {
            "op": DYNAMIC_EMBEDDER_PROMOTION_WITNESS_OP,
            "offset": witness_offset,
            "entries": witness_verification.entries,
            "last_chain_hash": context_graph_witness::hex_hash(&witness_verification.last_chain_hash),
            "witness_chain_sha256": witness_chain_sha256,
            "entry_written": witness_entry_written
        },
        "gradient_guard": {
            "clean_report": clean_guard,
            "leak_error_code": leak_error.code(),
            "guard_fires": guard_fires
        },
        "replay": {
            "record": replay_record,
            "record_sha256": replay_record_sha256,
            "report": replay_report,
            "reproduces": replay_reproduces
        },
        "freeze_audit": {
            "key_hex": hex::encode(audit_key),
            "record": audit_record,
            "direct_equal": direct_audit == audit_record,
            "reopened_equal": reopened_audit == audit_record,
            "persisted": persisted_audit
        }
    });
    write_json_0600(&fsv_path, &evidence).context("write TASK-EK-018G FSV JSON")?;
    let fsv_sha256 = sha256_file(&fsv_path).context("hash TASK-EK-018G FSV JSON")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "all_passed": all_passed,
            "fsv_path": fsv_path,
            "fsv_sha256": fsv_sha256,
            "provenance_has_required_fields": provenance_has_required_fields,
            "witness_entry_written": witness_entry_written,
            "guard_fires": guard_fires,
            "replay_reproduces": replay_reproduces,
            "persisted_audit": persisted_audit,
        }))?
    );
    ensure!(all_passed, "TASK-EK-018G freeze/audit FSV failed");
    Ok(())
}

fn comparison_fixture(
    candidate_id: RuntimeEmbedderId,
    created_at_unix_ms: i64,
) -> Result<EmbedderCandidateHoldoutComparison> {
    let comparison = EmbedderCandidateHoldoutComparison {
        schema_version: EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
        proposal_id: proposal_id_for("task-ek-018g-freeze-guarded-v1"),
        candidate_id,
        candidate_name: "freeze_guarded_v1".to_string(),
        candidate_architecture_signature: "freeze-audit:dynamic-learned-head:v1".to_string(),
        candidate_artifact_sha256: sha256_hex(b"freeze-audit-forward-artifact"),
        training_cert_chain_hash: sha256_hex(b"freeze-audit-training-cert-chain"),
        proposal_source_refs: vec!["absence:freeze-audit-fsv".to_string()],
        proposer_used_window_ids: vec!["proposal-window:freeze-audit-a".to_string()],
        heldout_window_ids: vec!["heldout-window:freeze-audit-b".to_string()],
        mode_a: eval(
            0.900,
            BTreeMap::from([
                ("mutation:python".to_string(), 0.860),
                ("relational_gap:python".to_string(), 0.840),
            ]),
            "freeze-mode-a",
        )?,
        mode_b: eval(
            0.907,
            BTreeMap::from([
                ("mutation:python".to_string(), 0.866),
                ("relational_gap:python".to_string(), 0.848),
            ]),
            "freeze-mode-b",
        )?,
        mode_c: eval(
            0.898,
            BTreeMap::from([
                ("mutation:python".to_string(), 0.858),
                ("relational_gap:python".to_string(), 0.838),
            ]),
            "freeze-mode-c",
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
        256,
        hex_to_array(&sha256_hex(label.as_bytes()))?,
        cells,
    )?)
}

fn target_provenance() -> TargetProvenance {
    TargetProvenance::new(
        "dynamic-embedder-freeze-fsv",
        BTreeMap::from([(
            "edynamic:1:freeze_guarded_v1".to_string(),
            "frozen".to_string(),
        )]),
        0,
        None,
    )
}

fn action_hash(op_type: &str, weights_sha: [u8; 32], evaluation_summary_sha: [u8; 32]) -> [u8; 32] {
    let mut action = Sha256::new();
    action.update(op_type.as_bytes());
    action.update(weights_sha);
    action.update(evaluation_summary_sha);
    action.finalize().into()
}

fn proposal_id_for(label: &str) -> [u8; 16] {
    let digest = Sha256::digest(label.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn hex_to_array(value: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(value).with_context(|| format!("decode sha256 {value}"))?;
    ensure!(bytes.len() == 32, "sha256 must decode to 32 bytes");
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}
