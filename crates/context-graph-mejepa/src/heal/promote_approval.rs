use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::dynamic_embedder_vram::MEJEPA_DYNAMIC_EMBEDDER_VRAM_BUDGET_EXCEEDED;
use crate::heal::errors::HealError;
use crate::heal::policy::{
    load_policy_record, persist_policy_record, policy_key, scan_policy_records,
};
use crate::heal::promote::TriggerReason;
use crate::heal::store::HealRocksStore;

const PENDING_PROMOTION_PREFIX: &[u8] = b"phase_e/pending-promotion/";
pub const CATASTROPHIC_REQUIRED_DISTINCT_APPROVALS: u8 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PendingPromotionKind {
    CatastrophicFullRetrainRequired {
        cell_key: String,
        metric_value: f32,
    },
    CatastrophicAbcCandidate {
        candidate_sha_hex: String,
        eval_report_key_hex: String,
    },
    WitnessChainRepairRequired {
        chain_path: String,
        broken_at_offset: Option<usize>,
        quarantine_recorded_at_unix_ms: i64,
    },
    DynamicEmbedderPromotion {
        candidate_id_slug: String,
        candidate_name: String,
        proposal_id_hex: String,
        first_dynamic_promotion: bool,
        #[serde(default)]
        adversarial_injection_risk: bool,
        would_cross_ship_gate: bool,
        modifies_below_threshold_cell: bool,
        heldout_global_delta: f32,
        min_cell_delta: f32,
        required_distinct_approvals: u8,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromotionApprovalState {
    Pending,
    Approved,
    Rejected,
    Executed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PromotionApprovalAction {
    Approve,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PromotionApprovalEvent {
    pub operator_id: String,
    pub action: PromotionApprovalAction,
    pub reason: String,
    pub recorded_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PendingPromotion {
    pub promotion_id: String,
    pub kind: PendingPromotionKind,
    pub trigger_reason: TriggerReason,
    pub reason: String,
    pub state: PromotionApprovalState,
    pub created_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
    pub required_distinct_approvals: u8,
    pub approvals: Vec<PromotionApprovalEvent>,
    pub rejections: Vec<PromotionApprovalEvent>,
}

impl PendingPromotion {
    fn validate(&self) -> Result<(), HealError> {
        if self.promotion_id.trim().is_empty() {
            return Err(HealError::invalid(
                "promote_approval.promotion_id",
                "promotion id must be non-empty",
            ));
        }
        if self.reason.trim().is_empty() {
            return Err(HealError::invalid(
                "promote_approval.reason",
                "promotion reason must be non-empty",
            ));
        }
        if self.required_distinct_approvals == 0 {
            return Err(HealError::invalid(
                "promote_approval.required_distinct_approvals",
                "required approvals must be greater than zero",
            ));
        }
        validate_pending_promotion_kind(&self.kind)?;
        Ok(())
    }

    pub fn key(&self) -> Result<Vec<u8>, HealError> {
        promotion_key(&self.promotion_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PromotionApprovalRequest {
    pub promotion_id: String,
    pub operator_id: String,
    pub action: PromotionApprovalAction,
    pub reason: String,
    pub two_person_rule: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PromotionApprovalResponse {
    pub promotion_id: String,
    pub state_before: PromotionApprovalState,
    pub state_after: PromotionApprovalState,
    pub required_distinct_approvals: u8,
    pub distinct_approval_count: usize,
    pub source_of_truth_cf: String,
    pub source_of_truth_key_hex: String,
    pub readback_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DynamicEmbedderPromotionGateRequest {
    pub candidate_id_slug: String,
    pub candidate_name: String,
    pub proposal_id_hex: String,
    pub falsification_passed: bool,
    pub redundant_with_existing: bool,
    pub within_vram_budget: bool,
    pub first_dynamic_promotion: bool,
    #[serde(default)]
    pub adversarial_injection_risk: bool,
    pub would_cross_ship_gate: bool,
    pub modifies_below_threshold_cell: bool,
    pub heldout_global_delta: f32,
    pub min_cell_delta: f32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DynamicEmbedderPromotionGateDecision {
    pub candidate_id_slug: String,
    pub candidate_name: String,
    pub may_promote_now: bool,
    pub approval_required: bool,
    pub queued_promotion_id: Option<String>,
    pub required_distinct_approvals: u8,
    pub catastrophic_class: bool,
    #[serde(default)]
    pub adversarial_injection_risk: bool,
    pub block_reasons: Vec<String>,
    pub source_of_truth_cf: String,
    pub source_of_truth_key_hex: Option<String>,
    pub readback_verified: bool,
}

pub fn queue_pending_retrain_request(
    storage: &HealRocksStore,
    kind: PendingPromotionKind,
    trigger_reason: TriggerReason,
    reason: impl Into<String>,
) -> Result<String, HealError> {
    let reason = reason.into();
    let created_at = chrono::Utc::now().timestamp_millis();
    let promotion_id = promotion_id(&kind, trigger_reason, created_at, &reason)?;
    let required_distinct_approvals = required_approvals_for_kind(&kind);
    let promotion = PendingPromotion {
        promotion_id: promotion_id.clone(),
        kind,
        trigger_reason,
        reason,
        state: PromotionApprovalState::Pending,
        created_at_unix_ms: created_at,
        updated_at_unix_ms: created_at,
        required_distinct_approvals,
        approvals: Vec::new(),
        rejections: Vec::new(),
    };
    promotion.validate()?;
    persist_policy_record(storage, &promotion.key()?, &promotion)?;
    Ok(promotion_id)
}

pub fn gate_dynamic_embedder_promotion(
    storage: &HealRocksStore,
    request: DynamicEmbedderPromotionGateRequest,
) -> Result<DynamicEmbedderPromotionGateDecision, HealError> {
    validate_dynamic_embedder_gate_request(&request)?;
    let block_reasons = dynamic_embedder_promotion_block_reasons(&request);
    let catastrophic_class = request.would_cross_ship_gate
        || request.modifies_below_threshold_cell
        || request.adversarial_injection_risk;
    let required_distinct_approvals = if !block_reasons.is_empty() {
        0
    } else {
        required_dynamic_embedder_approvals(
            request.first_dynamic_promotion,
            request.would_cross_ship_gate,
            request.modifies_below_threshold_cell,
            request.adversarial_injection_risk,
        )
    };
    if !block_reasons.is_empty() {
        return Ok(DynamicEmbedderPromotionGateDecision {
            candidate_id_slug: request.candidate_id_slug,
            candidate_name: request.candidate_name,
            may_promote_now: false,
            approval_required: false,
            queued_promotion_id: None,
            required_distinct_approvals,
            catastrophic_class,
            adversarial_injection_risk: request.adversarial_injection_risk,
            block_reasons,
            source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
            source_of_truth_key_hex: None,
            readback_verified: true,
        });
    }
    if required_distinct_approvals == 0 {
        return Ok(DynamicEmbedderPromotionGateDecision {
            candidate_id_slug: request.candidate_id_slug,
            candidate_name: request.candidate_name,
            may_promote_now: true,
            approval_required: false,
            queued_promotion_id: None,
            required_distinct_approvals,
            catastrophic_class,
            adversarial_injection_risk: request.adversarial_injection_risk,
            block_reasons,
            source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
            source_of_truth_key_hex: None,
            readback_verified: true,
        });
    }
    let kind = PendingPromotionKind::DynamicEmbedderPromotion {
        candidate_id_slug: request.candidate_id_slug.clone(),
        candidate_name: request.candidate_name.clone(),
        proposal_id_hex: request.proposal_id_hex,
        first_dynamic_promotion: request.first_dynamic_promotion,
        adversarial_injection_risk: request.adversarial_injection_risk,
        would_cross_ship_gate: request.would_cross_ship_gate,
        modifies_below_threshold_cell: request.modifies_below_threshold_cell,
        heldout_global_delta: request.heldout_global_delta,
        min_cell_delta: request.min_cell_delta,
        required_distinct_approvals,
    };
    let promotion_id = queue_pending_retrain_request(
        storage,
        kind,
        TriggerReason::OperatorTriggered,
        request.reason,
    )?;
    let key = promotion_key(&promotion_id)?;
    let readback: PendingPromotion = load_policy_record(storage, &key)?.ok_or_else(|| {
        HealError::invalid(
            "dynamic_embedder_approval.readback",
            "pending dynamic embedder promotion missing after queue write",
        )
    })?;
    readback.validate()?;
    Ok(DynamicEmbedderPromotionGateDecision {
        candidate_id_slug: request.candidate_id_slug,
        candidate_name: request.candidate_name,
        may_promote_now: false,
        approval_required: true,
        queued_promotion_id: Some(promotion_id),
        required_distinct_approvals,
        catastrophic_class,
        adversarial_injection_risk: request.adversarial_injection_risk,
        block_reasons,
        source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
        source_of_truth_key_hex: Some(hex::encode(key)),
        readback_verified: true,
    })
}

pub fn apply_promotion_approval(
    storage: &HealRocksStore,
    request: PromotionApprovalRequest,
) -> Result<PromotionApprovalResponse, HealError> {
    validate_operator(&request.operator_id)?;
    if request.reason.trim().is_empty()
        || request.reason.len() > 4096
        || request
            .reason
            .bytes()
            .any(|byte| byte == 0 || byte == b'\n')
    {
        return Err(HealError::invalid(
            "promote_approval.reason",
            "approval reason must be non-empty single-line text up to 4096 bytes",
        ));
    }
    let key = promotion_key(&request.promotion_id)?;
    let mut promotion: PendingPromotion = load_policy_record(storage, &key)?.ok_or_else(|| {
        HealError::invalid(
            "promote_approval.promotion_id",
            format!("pending promotion {} not found", request.promotion_id),
        )
    })?;
    promotion.validate()?;
    let state_before = promotion.state;
    if promotion.state != PromotionApprovalState::Pending {
        return Err(HealError::invalid(
            "promote_approval.state",
            format!(
                "promotion {} is {:?}, not pending",
                request.promotion_id, promotion.state
            ),
        ));
    }
    let requested_required = if request.two_person_rule { 2 } else { 1 };
    let policy_required = required_approvals_for_kind(&promotion.kind);
    promotion.required_distinct_approvals = promotion
        .required_distinct_approvals
        .max(policy_required)
        .max(requested_required);
    let event = PromotionApprovalEvent {
        operator_id: request.operator_id,
        action: request.action,
        reason: request.reason,
        recorded_at_unix_ms: chrono::Utc::now().timestamp_millis(),
    };
    match event.action {
        PromotionApprovalAction::Reject => {
            promotion.state = PromotionApprovalState::Rejected;
            promotion.rejections.push(event);
        }
        PromotionApprovalAction::Approve => {
            if promotion
                .approvals
                .iter()
                .any(|prior| prior.operator_id == event.operator_id)
            {
                return Err(HealError::invalid(
                    "promote_approval.operator_id",
                    "same operator cannot approve the same promotion twice",
                ));
            }
            promotion.approvals.push(event);
            if distinct_approvals(&promotion) >= promotion.required_distinct_approvals as usize {
                promotion.state = PromotionApprovalState::Approved;
            }
        }
    }
    promotion.updated_at_unix_ms = chrono::Utc::now().timestamp_millis();
    persist_policy_record(storage, &key, &promotion)?;
    let readback: PendingPromotion = load_policy_record(storage, &key)?.ok_or_else(|| {
        HealError::invalid(
            "promote_approval.readback",
            "pending promotion missing after approval write",
        )
    })?;
    if readback != promotion {
        return Err(HealError::invalid(
            "promote_approval.readback",
            "pending promotion readback differs after approval write",
        ));
    }
    Ok(PromotionApprovalResponse {
        promotion_id: promotion.promotion_id.clone(),
        state_before,
        state_after: promotion.state,
        required_distinct_approvals: promotion.required_distinct_approvals,
        distinct_approval_count: distinct_approvals(&promotion),
        source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
        source_of_truth_key_hex: hex::encode(key),
        readback_verified: true,
    })
}

pub fn pending_promotions(storage: &HealRocksStore) -> Result<Vec<PendingPromotion>, HealError> {
    Ok(
        scan_policy_records::<PendingPromotion>(storage, PENDING_PROMOTION_PREFIX)?
            .into_iter()
            .map(|(_, value)| value)
            .filter(|value| value.state == PromotionApprovalState::Pending)
            .collect(),
    )
}

pub fn pending_dynamic_embedder_promotions(
    storage: &HealRocksStore,
) -> Result<Vec<PendingPromotion>, HealError> {
    Ok(all_dynamic_embedder_promotions(storage)?
        .into_iter()
        .filter(|value| value.state == PromotionApprovalState::Pending)
        .collect())
}

pub fn all_dynamic_embedder_promotions(
    storage: &HealRocksStore,
) -> Result<Vec<PendingPromotion>, HealError> {
    let mut rows = Vec::new();
    for (_, value) in scan_policy_records::<PendingPromotion>(storage, PENDING_PROMOTION_PREFIX)? {
        if matches!(
            value.kind,
            PendingPromotionKind::DynamicEmbedderPromotion { .. }
        ) {
            value.validate()?;
            rows.push(value);
        }
    }
    rows.sort_by(|left, right| left.promotion_id.cmp(&right.promotion_id));
    Ok(rows)
}

pub fn approved_promotions(storage: &HealRocksStore) -> Result<Vec<PendingPromotion>, HealError> {
    Ok(
        scan_policy_records::<PendingPromotion>(storage, PENDING_PROMOTION_PREFIX)?
            .into_iter()
            .map(|(_, value)| value)
            .filter(|value| value.state == PromotionApprovalState::Approved)
            .collect(),
    )
}

pub fn mark_promotion_executed(
    storage: &HealRocksStore,
    promotion_id: &str,
) -> Result<PendingPromotion, HealError> {
    let key = promotion_key(promotion_id)?;
    let mut promotion: PendingPromotion = load_policy_record(storage, &key)?.ok_or_else(|| {
        HealError::invalid(
            "promote_approval.promotion_id",
            format!("pending promotion {promotion_id} not found"),
        )
    })?;
    if promotion.state != PromotionApprovalState::Approved {
        return Err(HealError::invalid(
            "promote_approval.state",
            format!(
                "promotion {promotion_id} is {:?}, not approved",
                promotion.state
            ),
        ));
    }
    promotion.state = PromotionApprovalState::Executed;
    promotion.updated_at_unix_ms = chrono::Utc::now().timestamp_millis();
    persist_policy_record(storage, &key, &promotion)?;
    let readback: PendingPromotion = load_policy_record(storage, &key)?.ok_or_else(|| {
        HealError::invalid(
            "promote_approval.readback",
            "pending promotion missing after execution write",
        )
    })?;
    if readback != promotion {
        return Err(HealError::invalid(
            "promote_approval.readback",
            "pending promotion readback differs after execution write",
        ));
    }
    Ok(readback)
}

fn distinct_approvals(promotion: &PendingPromotion) -> usize {
    let mut ids = promotion
        .approvals
        .iter()
        .map(|event| event.operator_id.as_str())
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    ids.len()
}

fn promotion_key(promotion_id: &str) -> Result<Vec<u8>, HealError> {
    policy_key(&["phase_e", "pending-promotion", promotion_id])
}

fn required_approvals_for_kind(kind: &PendingPromotionKind) -> u8 {
    match kind {
        PendingPromotionKind::CatastrophicFullRetrainRequired { .. }
        | PendingPromotionKind::CatastrophicAbcCandidate { .. } => {
            CATASTROPHIC_REQUIRED_DISTINCT_APPROVALS
        }
        PendingPromotionKind::WitnessChainRepairRequired { .. } => 1,
        PendingPromotionKind::DynamicEmbedderPromotion {
            required_distinct_approvals,
            ..
        } => *required_distinct_approvals,
    }
}

fn required_dynamic_embedder_approvals(
    first_dynamic_promotion: bool,
    would_cross_ship_gate: bool,
    modifies_below_threshold_cell: bool,
    adversarial_injection_risk: bool,
) -> u8 {
    if would_cross_ship_gate || modifies_below_threshold_cell || adversarial_injection_risk {
        CATASTROPHIC_REQUIRED_DISTINCT_APPROVALS
    } else if first_dynamic_promotion {
        1
    } else {
        0
    }
}

fn validate_operator(operator_id: &str) -> Result<(), HealError> {
    if operator_id.trim().is_empty()
        || operator_id.len() > 256
        || operator_id.bytes().any(|byte| byte == 0 || byte == b'\n')
    {
        return Err(HealError::invalid(
            "promote_approval.operator_id",
            "operator id must be non-empty single-line text up to 256 bytes",
        ));
    }
    Ok(())
}

fn validate_pending_promotion_kind(kind: &PendingPromotionKind) -> Result<(), HealError> {
    match kind {
        PendingPromotionKind::CatastrophicFullRetrainRequired {
            cell_key,
            metric_value,
        } => {
            validate_single_line("promote_approval.cell_key", cell_key, 256)?;
            if !metric_value.is_finite() || *metric_value < 0.0 {
                return Err(HealError::invalid(
                    "promote_approval.metric_value",
                    "metric value must be finite and non-negative",
                ));
            }
        }
        PendingPromotionKind::CatastrophicAbcCandidate {
            candidate_sha_hex,
            eval_report_key_hex,
        } => {
            validate_single_line("promote_approval.candidate_sha_hex", candidate_sha_hex, 64)?;
            validate_single_line(
                "promote_approval.eval_report_key_hex",
                eval_report_key_hex,
                4096,
            )?;
        }
        PendingPromotionKind::WitnessChainRepairRequired {
            chain_path,
            quarantine_recorded_at_unix_ms,
            ..
        } => {
            validate_single_line("promote_approval.chain_path", chain_path, 4096)?;
            if *quarantine_recorded_at_unix_ms <= 0 {
                return Err(HealError::invalid(
                    "promote_approval.quarantine_recorded_at_unix_ms",
                    "must be positive",
                ));
            }
        }
        PendingPromotionKind::DynamicEmbedderPromotion {
            candidate_id_slug,
            candidate_name,
            proposal_id_hex,
            heldout_global_delta,
            min_cell_delta,
            required_distinct_approvals,
            ..
        } => {
            validate_single_line(
                "dynamic_embedder_approval.candidate_id_slug",
                candidate_id_slug,
                256,
            )?;
            validate_single_line(
                "dynamic_embedder_approval.candidate_name",
                candidate_name,
                256,
            )?;
            validate_single_line(
                "dynamic_embedder_approval.proposal_id_hex",
                proposal_id_hex,
                64,
            )?;
            validate_unit_delta(
                "dynamic_embedder_approval.heldout_global_delta",
                *heldout_global_delta,
            )?;
            validate_unit_delta("dynamic_embedder_approval.min_cell_delta", *min_cell_delta)?;
            if *required_distinct_approvals == 0 {
                return Err(HealError::invalid(
                    "dynamic_embedder_approval.required_distinct_approvals",
                    "queued dynamic embedder approval must require at least one approval",
                ));
            }
        }
    }
    Ok(())
}

fn validate_dynamic_embedder_gate_request(
    request: &DynamicEmbedderPromotionGateRequest,
) -> Result<(), HealError> {
    validate_single_line(
        "dynamic_embedder_approval.candidate_id_slug",
        &request.candidate_id_slug,
        256,
    )?;
    validate_single_line(
        "dynamic_embedder_approval.candidate_name",
        &request.candidate_name,
        256,
    )?;
    validate_single_line(
        "dynamic_embedder_approval.proposal_id_hex",
        &request.proposal_id_hex,
        64,
    )?;
    if request.proposal_id_hex.len() != 32
        || !request
            .proposal_id_hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(HealError::invalid(
            "dynamic_embedder_approval.proposal_id_hex",
            "proposal id must be 16-byte lowercase-or-uppercase hex",
        ));
    }
    validate_unit_delta(
        "dynamic_embedder_approval.heldout_global_delta",
        request.heldout_global_delta,
    )?;
    validate_unit_delta(
        "dynamic_embedder_approval.min_cell_delta",
        request.min_cell_delta,
    )?;
    validate_single_line("dynamic_embedder_approval.reason", &request.reason, 4096)?;
    Ok(())
}

fn dynamic_embedder_promotion_block_reasons(
    request: &DynamicEmbedderPromotionGateRequest,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if !request.falsification_passed {
        reasons.push("falsification_gate_failed".to_string());
    }
    if request.redundant_with_existing {
        reasons.push("redundant_with_existing_embedder".to_string());
    }
    if !request.within_vram_budget {
        reasons.push(MEJEPA_DYNAMIC_EMBEDDER_VRAM_BUDGET_EXCEEDED.to_string());
    }
    reasons
}

fn validate_unit_delta(field: &str, value: f32) -> Result<(), HealError> {
    if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
        return Err(HealError::invalid(field, "delta must be finite in [-1, 1]"));
    }
    Ok(())
}

fn validate_single_line(field: &str, value: &str, max_len: usize) -> Result<(), HealError> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.len() > max_len
        || value.bytes().any(|byte| byte == 0 || byte == b'\n')
    {
        return Err(HealError::invalid(
            field,
            format!("must be non-empty trimmed single-line text up to {max_len} bytes"),
        ));
    }
    Ok(())
}

fn promotion_id(
    kind: &PendingPromotionKind,
    trigger_reason: TriggerReason,
    created_at: i64,
    reason: &str,
) -> Result<String, HealError> {
    let mut hasher = Sha256::new();
    hasher.update(serde_json::to_vec(kind)?);
    hasher.update(format!("{trigger_reason:?}"));
    hasher.update(created_at.to_be_bytes());
    hasher.update(reason.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heal::store::HealRocksStore;

    #[test]
    fn two_person_rule_rejects_duplicate_operator() {
        let temp = tempfile::tempdir().unwrap();
        let storage = HealRocksStore::open(temp.path().join("db")).unwrap();
        let promotion_id = queue_pending_retrain_request(
            storage.as_ref(),
            PendingPromotionKind::CatastrophicFullRetrainRequired {
                cell_key: "compile_error::rust".to_string(),
                metric_value: 0.72,
            },
            TriggerReason::DriftCatastrophic,
            "catastrophic drift",
        )
        .unwrap();
        let first = apply_promotion_approval(
            storage.as_ref(),
            PromotionApprovalRequest {
                promotion_id: promotion_id.clone(),
                operator_id: "operator-a".to_string(),
                action: PromotionApprovalAction::Approve,
                reason: "reviewed eval".to_string(),
                two_person_rule: true,
            },
        )
        .unwrap();
        assert_eq!(first.state_after, PromotionApprovalState::Pending);
        assert_eq!(
            first.required_distinct_approvals,
            CATASTROPHIC_REQUIRED_DISTINCT_APPROVALS
        );
        let duplicate = apply_promotion_approval(
            storage.as_ref(),
            PromotionApprovalRequest {
                promotion_id,
                operator_id: "operator-a".to_string(),
                action: PromotionApprovalAction::Approve,
                reason: "same operator second approval".to_string(),
                two_person_rule: true,
            },
        )
        .unwrap_err();
        assert_eq!(duplicate.code(), "MEJEPA_HEAL_INVALID_STATE");
    }

    #[test]
    fn catastrophic_policy_cannot_be_downgraded_to_one_person() {
        let temp = tempfile::tempdir().unwrap();
        let storage = HealRocksStore::open(temp.path().join("db")).unwrap();
        let promotion_id = queue_pending_retrain_request(
            storage.as_ref(),
            PendingPromotionKind::CatastrophicFullRetrainRequired {
                cell_key: "global_or_per_cell".to_string(),
                metric_value: 0.75,
            },
            TriggerReason::DriftCatastrophic,
            "catastrophic drift",
        )
        .unwrap();
        let first = apply_promotion_approval(
            storage.as_ref(),
            PromotionApprovalRequest {
                promotion_id: promotion_id.clone(),
                operator_id: "operator-a".to_string(),
                action: PromotionApprovalAction::Approve,
                reason: "reviewed catastrophic eval".to_string(),
                two_person_rule: false,
            },
        )
        .unwrap();
        assert_eq!(first.state_after, PromotionApprovalState::Pending);
        assert_eq!(
            first.required_distinct_approvals,
            CATASTROPHIC_REQUIRED_DISTINCT_APPROVALS
        );
        let second = apply_promotion_approval(
            storage.as_ref(),
            PromotionApprovalRequest {
                promotion_id,
                operator_id: "operator-b".to_string(),
                action: PromotionApprovalAction::Approve,
                reason: "second operator approval".to_string(),
                two_person_rule: false,
            },
        )
        .unwrap();
        assert_eq!(second.state_after, PromotionApprovalState::Approved);
        assert_eq!(second.distinct_approval_count, 2);
    }

    #[test]
    fn first_dynamic_embedder_promotion_requires_single_operator_approval() {
        let temp = tempfile::tempdir().unwrap();
        let storage = HealRocksStore::open(temp.path().join("db")).unwrap();
        let decision = gate_dynamic_embedder_promotion(
            storage.as_ref(),
            dynamic_request("edynamic:1:corpus_transe_v1", true, false, false),
        )
        .unwrap();

        assert!(!decision.may_promote_now);
        assert!(decision.approval_required);
        assert_eq!(decision.required_distinct_approvals, 1);
        assert_eq!(
            pending_dynamic_embedder_promotions(storage.as_ref())
                .unwrap()
                .len(),
            1
        );

        let approval = apply_promotion_approval(
            storage.as_ref(),
            PromotionApprovalRequest {
                promotion_id: decision.queued_promotion_id.unwrap(),
                operator_id: "operator-a".to_string(),
                action: PromotionApprovalAction::Approve,
                reason: "reviewed first dynamic embedder".to_string(),
                two_person_rule: false,
            },
        )
        .unwrap();
        assert_eq!(approval.state_after, PromotionApprovalState::Approved);
        assert_eq!(approval.distinct_approval_count, 1);
    }

    #[test]
    fn catastrophic_dynamic_embedder_promotion_requires_two_distinct_operators() {
        let temp = tempfile::tempdir().unwrap();
        let storage = HealRocksStore::open(temp.path().join("db")).unwrap();
        let decision = gate_dynamic_embedder_promotion(
            storage.as_ref(),
            dynamic_request("edynamic:2:ship_gate_crossing_v1", false, true, false),
        )
        .unwrap();

        assert!(!decision.may_promote_now);
        assert!(decision.approval_required);
        assert!(decision.catastrophic_class);
        assert_eq!(
            decision.required_distinct_approvals,
            CATASTROPHIC_REQUIRED_DISTINCT_APPROVALS
        );
        let promotion_id = decision.queued_promotion_id.unwrap();
        let first = apply_promotion_approval(
            storage.as_ref(),
            PromotionApprovalRequest {
                promotion_id: promotion_id.clone(),
                operator_id: "operator-a".to_string(),
                action: PromotionApprovalAction::Approve,
                reason: "reviewed catastrophic dynamic embedder".to_string(),
                two_person_rule: false,
            },
        )
        .unwrap();
        assert_eq!(first.state_after, PromotionApprovalState::Pending);
        let second = apply_promotion_approval(
            storage.as_ref(),
            PromotionApprovalRequest {
                promotion_id,
                operator_id: "operator-b".to_string(),
                action: PromotionApprovalAction::Approve,
                reason: "second dynamic embedder approval".to_string(),
                two_person_rule: false,
            },
        )
        .unwrap();
        assert_eq!(second.state_after, PromotionApprovalState::Approved);
        assert_eq!(second.distinct_approval_count, 2);
    }

    #[test]
    fn non_catastrophic_non_first_dynamic_embedder_auto_promotes() {
        let temp = tempfile::tempdir().unwrap();
        let storage = HealRocksStore::open(temp.path().join("db")).unwrap();
        let decision = gate_dynamic_embedder_promotion(
            storage.as_ref(),
            dynamic_request("edynamic:3:auto_v1", false, false, false),
        )
        .unwrap();

        assert!(decision.may_promote_now);
        assert!(!decision.approval_required);
        assert!(decision.queued_promotion_id.is_none());
        assert_eq!(
            pending_dynamic_embedder_promotions(storage.as_ref())
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn adversarial_dynamic_embedder_risk_requires_operator_gate() {
        let temp = tempfile::tempdir().unwrap();
        let storage = HealRocksStore::open(temp.path().join("db")).unwrap();
        let mut request = dynamic_request("edynamic:4:adversarial_risk_v1", false, false, false);
        request.adversarial_injection_risk = true;
        let decision = gate_dynamic_embedder_promotion(storage.as_ref(), request).unwrap();

        assert!(!decision.may_promote_now);
        assert!(decision.approval_required);
        assert!(decision.adversarial_injection_risk);
        assert!(decision.catastrophic_class);
        assert_eq!(
            decision.required_distinct_approvals,
            CATASTROPHIC_REQUIRED_DISTINCT_APPROVALS
        );
        assert!(decision.queued_promotion_id.is_some());
        let rows = pending_dynamic_embedder_promotions(storage.as_ref()).unwrap();
        assert_eq!(rows.len(), 1);
        let PendingPromotionKind::DynamicEmbedderPromotion {
            adversarial_injection_risk,
            required_distinct_approvals,
            ..
        } = &rows[0].kind
        else {
            panic!("expected dynamic embedder promotion row");
        };
        assert!(*adversarial_injection_risk);
        assert_eq!(
            *required_distinct_approvals,
            CATASTROPHIC_REQUIRED_DISTINCT_APPROVALS
        );
    }

    fn dynamic_request(
        candidate_id_slug: &str,
        first_dynamic_promotion: bool,
        would_cross_ship_gate: bool,
        modifies_below_threshold_cell: bool,
    ) -> DynamicEmbedderPromotionGateRequest {
        DynamicEmbedderPromotionGateRequest {
            candidate_id_slug: candidate_id_slug.to_string(),
            candidate_name: candidate_id_slug.replace(':', "_"),
            proposal_id_hex: "01010101010101010101010101010101".to_string(),
            falsification_passed: true,
            redundant_with_existing: false,
            within_vram_budget: true,
            first_dynamic_promotion,
            adversarial_injection_risk: false,
            would_cross_ship_gate,
            modifies_below_threshold_cell,
            heldout_global_delta: 0.006,
            min_cell_delta: 0.0,
            reason: "dynamic embedder promotion gate fixture".to_string(),
        }
    }
}
