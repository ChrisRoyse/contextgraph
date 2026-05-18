use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::heal::{
    all_dynamic_embedder_promotions, apply_promotion_approval, gate_dynamic_embedder_promotion,
    pending_dynamic_embedder_promotions, DynamicEmbedderPromotionGateRequest, HealRocksStore,
    PendingPromotion, PendingPromotionKind, PromotionApprovalAction, PromotionApprovalRequest,
    PromotionApprovalState, CATASTROPHIC_REQUIRED_DISTINCT_APPROVALS,
};
use context_graph_mejepa::{current_unix_ms, write_json_0600};
use context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS;
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-ek-018h-operator-gates-fsv";

fn main() -> Result<()> {
    let started_at_unix_ms = current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let db_path = run_root.join("heal-rocksdb");
    let fsv_path = run_root.join("operator_gates_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-018H FSV run dir")?;
    let storage = HealRocksStore::open(&db_path).context("open TASK-EK-018H heal RocksDB")?;

    let first_promotion = gate_dynamic_embedder_promotion(
        storage.as_ref(),
        request("edynamic:1:corpus_transe_v1", true, false, false),
    )
    .context("gate first dynamic embedder promotion")?;
    let pending_after_first_queue =
        pending_dynamic_embedder_promotions(storage.as_ref()).context("pending first queue")?;
    let first_approval = apply_promotion_approval(
        storage.as_ref(),
        PromotionApprovalRequest {
            promotion_id: first_promotion
                .queued_promotion_id
                .clone()
                .context("first promotion missing queued id")?,
            operator_id: "operator-alpha".to_string(),
            action: PromotionApprovalAction::Approve,
            reason: "reviewed first dynamic embedder promotion".to_string(),
            two_person_rule: false,
        },
    )
    .context("approve first dynamic embedder promotion")?;

    let catastrophic = gate_dynamic_embedder_promotion(
        storage.as_ref(),
        request("edynamic:2:ship_gate_crossing_v1", false, true, true),
    )
    .context("gate catastrophic dynamic embedder promotion")?;
    let catastrophic_promotion_id = catastrophic
        .queued_promotion_id
        .clone()
        .context("catastrophic promotion missing queued id")?;
    let catastrophic_first_approval = apply_promotion_approval(
        storage.as_ref(),
        PromotionApprovalRequest {
            promotion_id: catastrophic_promotion_id.clone(),
            operator_id: "operator-alpha".to_string(),
            action: PromotionApprovalAction::Approve,
            reason: "reviewed ship-gate-crossing dynamic embedder".to_string(),
            two_person_rule: false,
        },
    )
    .context("first catastrophic approval")?;
    let catastrophic_second_approval = apply_promotion_approval(
        storage.as_ref(),
        PromotionApprovalRequest {
            promotion_id: catastrophic_promotion_id,
            operator_id: "operator-beta".to_string(),
            action: PromotionApprovalAction::Approve,
            reason: "independent second dynamic embedder approval".to_string(),
            two_person_rule: false,
        },
    )
    .context("second catastrophic approval")?;

    let auto_promotion = gate_dynamic_embedder_promotion(
        storage.as_ref(),
        request("edynamic:3:auto_non_catastrophic_v1", false, false, false),
    )
    .context("gate non-catastrophic non-first dynamic embedder promotion")?;
    let pending_after_approvals =
        pending_dynamic_embedder_promotions(storage.as_ref()).context("pending after approvals")?;
    let rows =
        all_dynamic_embedder_promotions(storage.as_ref()).context("all dynamic approval rows")?;

    drop(storage);
    let reopened = HealRocksStore::open(&db_path).context("reopen TASK-EK-018H heal RocksDB")?;
    let reopened_rows =
        all_dynamic_embedder_promotions(reopened.as_ref()).context("reopened approvals")?;

    let first_blocks_until_operator = !first_promotion.may_promote_now
        && first_promotion.approval_required
        && first_promotion.required_distinct_approvals == 1
        && pending_after_first_queue.len() == 1
        && first_approval.state_after == PromotionApprovalState::Approved
        && first_approval.distinct_approval_count == 1;
    let catastrophic_requires_two = !catastrophic.may_promote_now
        && catastrophic.approval_required
        && catastrophic.catastrophic_class
        && catastrophic.required_distinct_approvals == CATASTROPHIC_REQUIRED_DISTINCT_APPROVALS
        && catastrophic_first_approval.state_after == PromotionApprovalState::Pending
        && catastrophic_second_approval.state_after == PromotionApprovalState::Approved
        && catastrophic_second_approval.distinct_approval_count == 2;
    let non_catastrophic_auto_promotes = auto_promotion.may_promote_now
        && !auto_promotion.approval_required
        && auto_promotion.queued_promotion_id.is_none()
        && auto_promotion.required_distinct_approvals == 0;
    let approval_audit_preserved = rows.len() == 2
        && pending_after_approvals.is_empty()
        && rows.iter().all(dynamic_row_has_approval_audit)
        && reopened_rows == rows;
    let all_passed = first_blocks_until_operator
        && catastrophic_requires_two
        && non_catastrophic_auto_promotes
        && approval_audit_preserved;

    let evidence = json!({
        "task": "TASK-EK-018H",
        "issue": 350,
        "all_passed": all_passed,
        "sot": {
            "approval_cf": CF_MEJEPA_MODEL_PROMOTIONS,
            "db_path": db_path,
            "fsv_path": fsv_path,
            "inner_llm_invoked": false
        },
        "first_promotion": {
            "decision": first_promotion,
            "approval": first_approval,
            "pending_after_queue": pending_after_first_queue.len(),
            "blocks_until_operator": first_blocks_until_operator
        },
        "catastrophic_promotion": {
            "decision": catastrophic,
            "first_approval": catastrophic_first_approval,
            "second_approval": catastrophic_second_approval,
            "requires_two_distinct_operators": catastrophic_requires_two
        },
        "auto_promotion": {
            "decision": auto_promotion,
            "auto_promotes": non_catastrophic_auto_promotes
        },
        "audit": {
            "dynamic_rows": rows,
            "reopened_rows_equal": reopened_rows == rows,
            "pending_after_approvals": pending_after_approvals.len(),
            "approval_audit_preserved": approval_audit_preserved
        }
    });
    write_json_0600(&fsv_path, &evidence).context("write TASK-EK-018H FSV JSON")?;
    let fsv_sha256 = sha256_file(&fsv_path).context("hash TASK-EK-018H FSV JSON")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "all_passed": all_passed,
            "fsv_path": fsv_path,
            "fsv_sha256": fsv_sha256,
            "first_blocks_until_operator": first_blocks_until_operator,
            "catastrophic_requires_two": catastrophic_requires_two,
            "non_catastrophic_auto_promotes": non_catastrophic_auto_promotes,
            "approval_audit_preserved": approval_audit_preserved
        }))?
    );
    ensure!(all_passed, "TASK-EK-018H operator gate FSV failed");
    Ok(())
}

fn request(
    candidate_id_slug: &str,
    first_dynamic_promotion: bool,
    would_cross_ship_gate: bool,
    modifies_below_threshold_cell: bool,
) -> DynamicEmbedderPromotionGateRequest {
    DynamicEmbedderPromotionGateRequest {
        candidate_id_slug: candidate_id_slug.to_string(),
        candidate_name: candidate_id_slug.replace(':', "_"),
        proposal_id_hex: sha256_text(candidate_id_slug)[..32].to_string(),
        falsification_passed: true,
        redundant_with_existing: false,
        within_vram_budget: true,
        first_dynamic_promotion,
        adversarial_injection_risk: false,
        would_cross_ship_gate,
        modifies_below_threshold_cell,
        heldout_global_delta: 0.006,
        min_cell_delta: 0.0,
        reason: format!("operator gate FSV for {candidate_id_slug}"),
    }
}

fn dynamic_row_has_approval_audit(row: &PendingPromotion) -> bool {
    let PendingPromotionKind::DynamicEmbedderPromotion {
        candidate_id_slug,
        proposal_id_hex,
        required_distinct_approvals,
        ..
    } = &row.kind
    else {
        return false;
    };
    row.state == PromotionApprovalState::Approved
        && !row.promotion_id.is_empty()
        && !candidate_id_slug.is_empty()
        && proposal_id_hex.len() == 32
        && row.required_distinct_approvals == *required_distinct_approvals
        && row.approvals.len() >= usize::from(*required_distinct_approvals)
        && row
            .approvals
            .iter()
            .all(|event| !event.operator_id.is_empty() && !event.reason.is_empty())
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
