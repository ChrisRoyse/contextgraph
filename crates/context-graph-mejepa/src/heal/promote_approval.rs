use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::heal::errors::HealError;
use crate::heal::policy::{
    load_policy_record, persist_policy_record, policy_key, scan_policy_records,
};
use crate::heal::promote::TriggerReason;
use crate::heal::store::HealRocksStore;

const PENDING_PROMOTION_PREFIX: &[u8] = b"phase_e/pending-promotion/";
pub const CATASTROPHIC_REQUIRED_DISTINCT_APPROVALS: u8 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PendingPromotionKind {
    CatastrophicFullRetrainRequired {
        cell_key: String,
        metric_value: f32,
    },
    CatastrophicAbcCandidate {
        candidate_sha_hex: String,
        eval_report_key_hex: String,
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
}
