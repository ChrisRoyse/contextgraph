use std::collections::BTreeMap;

use context_graph_mejepa_cf::{CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS, CF_MEJEPA_MODEL_PROMOTIONS};
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::count_cf;
use crate::embedder_falsification::{
    evaluate_and_persist_embedder_falsification, read_embedder_proposal_rejections,
    EmbedderCandidateHoldoutComparison, EmbedderFalsificationDecision, EmbedderFalsificationGate,
    EMBEDDER_FALSIFICATION_SCHEMA_VERSION, MEJEPA_EMBEDDER_FALSIFICATION_CELL_REGRESSION,
    MEJEPA_EMBEDDER_FALSIFICATION_GLOBAL_DELTA_TOO_SMALL,
    MEJEPA_EMBEDDER_FALSIFICATION_HOLDOUT_OVERLAP,
};
use crate::error::MejepaInferError;
use crate::heal::errors::HealError;
use crate::heal::promote::HoldoutEval;
use crate::heal::promote_approval::{
    all_dynamic_embedder_promotions, gate_dynamic_embedder_promotion,
    DynamicEmbedderPromotionGateDecision, DynamicEmbedderPromotionGateRequest,
    PendingPromotionKind, PromotionApprovalState, CATASTROPHIC_REQUIRED_DISTINCT_APPROVALS,
};
use crate::heal::store::HealRocksStore;
use crate::RuntimeEmbedderId;

pub const ADVERSARIAL_EMBEDDER_INJECTION_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdversarialEmbedderInjectionKind {
    SyntheticOodCluster,
    CentroidPoisoning,
    HoldoutLeakage,
    OperatorApprovalBypass,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdversarialEmbedderInjectionCase {
    pub schema_version: u32,
    pub case_id: String,
    pub kind: AdversarialEmbedderInjectionKind,
    pub threat_vector: String,
    pub expected_falsification_reason_code: Option<String>,
    pub comparison: EmbedderCandidateHoldoutComparison,
    pub promotion_request_if_accepted: Option<DynamicEmbedderPromotionGateRequest>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdversarialEmbedderInjectionCaseResult {
    pub case_id: String,
    pub kind: AdversarialEmbedderInjectionKind,
    pub threat_vector: String,
    pub falsification_decision: EmbedderFalsificationDecision,
    pub falsification_rejected_as_expected: bool,
    pub operator_gate_decision: Option<DynamicEmbedderPromotionGateDecision>,
    pub operator_gate_caught_survivor: bool,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdversarialEmbedderInjectionReport {
    pub schema_version: u32,
    pub all_passed: bool,
    pub cases_total: usize,
    pub falsification_rejected_count: usize,
    pub operator_gate_caught_count: usize,
    pub unexpected_survivor_count: usize,
    pub rejection_rows_before: u64,
    pub rejection_rows_after: u64,
    pub promotion_rows_after: usize,
    pub source_of_truth_rejection_cf: String,
    pub source_of_truth_approval_cf: String,
    pub cases: Vec<AdversarialEmbedderInjectionCaseResult>,
}

pub fn adversarial_embedder_injection_cases(
    created_at_unix_ms: i64,
) -> Result<Vec<AdversarialEmbedderInjectionCase>, MejepaInferError> {
    if created_at_unix_ms <= 0 {
        return invalid(
            "adversarial_embedder_injection.created_at_unix_ms",
            "must be positive",
        );
    }
    Ok(vec![
        adversarial_case(AdversarialCaseSpec {
            case_id: "synthetic-ood-cluster",
            kind: AdversarialEmbedderInjectionKind::SyntheticOodCluster,
            threat_vector:
                "synthetic Unknown cluster appears coherent but has no held-out global gain",
            sequence: 31,
            mode_b_global: 0.903,
            mode_b_cells: BTreeMap::from([
                ("auth:python".to_string(), 0.860),
                ("io:python".to_string(), 0.850),
            ]),
            proposer_used_window_ids: vec!["ood-proposal-window-a".to_string()],
            heldout_window_ids: vec!["ood-heldout-window-a".to_string()],
            expected_reason: Some(MEJEPA_EMBEDDER_FALSIFICATION_GLOBAL_DELTA_TOO_SMALL),
            promotion_request_if_accepted: None,
            created_at_unix_ms,
        })?,
        adversarial_case(AdversarialCaseSpec {
            case_id: "centroid-poisoning",
            kind: AdversarialEmbedderInjectionKind::CentroidPoisoning,
            threat_vector:
                "poisoned centroid improves aggregate score while degrading a security cell",
            sequence: 32,
            mode_b_global: 0.909,
            mode_b_cells: BTreeMap::from([
                ("auth:python".to_string(), 0.872),
                ("io:python".to_string(), 0.842),
            ]),
            proposer_used_window_ids: vec!["centroid-proposal-window-a".to_string()],
            heldout_window_ids: vec!["centroid-heldout-window-a".to_string()],
            expected_reason: Some(MEJEPA_EMBEDDER_FALSIFICATION_CELL_REGRESSION),
            promotion_request_if_accepted: None,
            created_at_unix_ms: created_at_unix_ms + 1,
        })?,
        adversarial_case(AdversarialCaseSpec {
            case_id: "holdout-leakage",
            kind: AdversarialEmbedderInjectionKind::HoldoutLeakage,
            threat_vector: "proposer saw the held-out window and attempts to replay its shape",
            sequence: 33,
            mode_b_global: 0.912,
            mode_b_cells: BTreeMap::from([
                ("auth:python".to_string(), 0.873),
                ("io:python".to_string(), 0.864),
            ]),
            proposer_used_window_ids: vec![
                "leak-proposal-window-a".to_string(),
                "leak-heldout-window-a".to_string(),
            ],
            heldout_window_ids: vec!["leak-heldout-window-a".to_string()],
            expected_reason: Some(MEJEPA_EMBEDDER_FALSIFICATION_HOLDOUT_OVERLAP),
            promotion_request_if_accepted: None,
            created_at_unix_ms: created_at_unix_ms + 2,
        })?,
        adversarial_case(AdversarialCaseSpec {
            case_id: "operator-approval-bypass",
            kind: AdversarialEmbedderInjectionKind::OperatorApprovalBypass,
            threat_vector:
                "candidate survives hold-out scoring but is flagged as adversarial-injection risk",
            sequence: 34,
            mode_b_global: 0.914,
            mode_b_cells: BTreeMap::from([
                ("auth:python".to_string(), 0.874),
                ("io:python".to_string(), 0.864),
            ]),
            proposer_used_window_ids: vec!["bypass-proposal-window-a".to_string()],
            heldout_window_ids: vec!["bypass-heldout-window-a".to_string()],
            expected_reason: None,
            promotion_request_if_accepted: Some(promotion_request("operator-approval-bypass", 34)?),
            created_at_unix_ms: created_at_unix_ms + 3,
        })?,
    ])
}

pub fn evaluate_adversarial_embedder_injection(
    infer_db: &DB,
    heal_store: &HealRocksStore,
    cases: &[AdversarialEmbedderInjectionCase],
    gate: EmbedderFalsificationGate,
) -> Result<AdversarialEmbedderInjectionReport, MejepaInferError> {
    if cases.is_empty() {
        return invalid("adversarial_embedder_injection.cases", "must be non-empty");
    }
    let rejection_rows_before = count_cf(infer_db, CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS)?;
    let mut results = Vec::with_capacity(cases.len());
    for case in cases {
        validate_case(case)?;
        let falsification_decision =
            evaluate_and_persist_embedder_falsification(infer_db, &case.comparison, gate)?;
        let falsification_rejected_as_expected = match &case.expected_falsification_reason_code {
            Some(expected) => {
                !falsification_decision.accepted
                    && falsification_decision.reason_code.as_deref() == Some(expected.as_str())
            }
            None => falsification_decision.accepted,
        };
        let operator_gate_decision = if falsification_decision.accepted {
            let mut request = case.promotion_request_if_accepted.clone().ok_or_else(|| {
                MejepaInferError::InvalidInput {
                    field: "adversarial_embedder_injection.promotion_request".to_string(),
                    detail: format!("accepted case {} has no promotion request", case.case_id),
                }
            })?;
            request.falsification_passed = falsification_decision.accepted;
            request.heldout_global_delta = falsification_decision.global_delta;
            request.min_cell_delta = falsification_decision.min_cell_delta;
            Some(gate_dynamic_embedder_promotion(heal_store, request).map_err(map_heal_error)?)
        } else {
            None
        };
        let operator_gate_caught_survivor = operator_gate_decision
            .as_ref()
            .map(|decision| {
                !decision.may_promote_now
                    && decision.approval_required
                    && decision.adversarial_injection_risk
                    && decision.catastrophic_class
                    && decision.required_distinct_approvals
                        >= CATASTROPHIC_REQUIRED_DISTINCT_APPROVALS
                    && decision.queued_promotion_id.is_some()
            })
            .unwrap_or(false);
        let passed = if case.expected_falsification_reason_code.is_some() {
            falsification_rejected_as_expected && operator_gate_decision.is_none()
        } else {
            falsification_rejected_as_expected && operator_gate_caught_survivor
        };
        results.push(AdversarialEmbedderInjectionCaseResult {
            case_id: case.case_id.clone(),
            kind: case.kind,
            threat_vector: case.threat_vector.clone(),
            falsification_decision,
            falsification_rejected_as_expected,
            operator_gate_decision,
            operator_gate_caught_survivor,
            passed,
        });
    }

    let rejection_rows_after = count_cf(infer_db, CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS)?;
    let rejection_rows = read_embedder_proposal_rejections(infer_db)?;
    let promotion_rows = all_dynamic_embedder_promotions(heal_store).map_err(map_heal_error)?;
    let promotion_rows_after = promotion_rows.len();
    let falsification_rejected_count = results
        .iter()
        .filter(|result| !result.falsification_decision.accepted)
        .count();
    let operator_gate_caught_count = results
        .iter()
        .filter(|result| result.operator_gate_caught_survivor)
        .count();
    let unexpected_survivor_count = results
        .iter()
        .filter(|result| {
            result.falsification_decision.accepted && !result.operator_gate_caught_survivor
        })
        .count();
    let rejected_case_count = cases
        .iter()
        .filter(|case| case.expected_falsification_reason_code.is_some())
        .count();
    let approval_case_count = cases.len() - rejected_case_count;
    let rejection_row_delta = rejection_rows_after.saturating_sub(rejection_rows_before);
    let approval_rows_valid = promotion_rows.iter().all(|row| {
        matches!(
            &row.kind,
            PendingPromotionKind::DynamicEmbedderPromotion {
                adversarial_injection_risk: true,
                required_distinct_approvals,
                ..
            } if *required_distinct_approvals >= CATASTROPHIC_REQUIRED_DISTINCT_APPROVALS
        ) && row.state == PromotionApprovalState::Pending
    });
    let all_passed = results.iter().all(|result| result.passed)
        && falsification_rejected_count == rejected_case_count
        && operator_gate_caught_count == approval_case_count
        && unexpected_survivor_count == 0
        && rejection_row_delta as usize == rejected_case_count
        && rejection_rows.len() as u64 == rejection_rows_after
        && promotion_rows_after == approval_case_count
        && approval_rows_valid;

    Ok(AdversarialEmbedderInjectionReport {
        schema_version: ADVERSARIAL_EMBEDDER_INJECTION_SCHEMA_VERSION,
        all_passed,
        cases_total: cases.len(),
        falsification_rejected_count,
        operator_gate_caught_count,
        unexpected_survivor_count,
        rejection_rows_before,
        rejection_rows_after,
        promotion_rows_after,
        source_of_truth_rejection_cf: CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS.to_string(),
        source_of_truth_approval_cf: CF_MEJEPA_MODEL_PROMOTIONS.to_string(),
        cases: results,
    })
}

struct AdversarialCaseSpec {
    case_id: &'static str,
    kind: AdversarialEmbedderInjectionKind,
    threat_vector: &'static str,
    sequence: u32,
    mode_b_global: f32,
    mode_b_cells: BTreeMap<String, f32>,
    proposer_used_window_ids: Vec<String>,
    heldout_window_ids: Vec<String>,
    expected_reason: Option<&'static str>,
    promotion_request_if_accepted: Option<DynamicEmbedderPromotionGateRequest>,
    created_at_unix_ms: i64,
}

fn adversarial_case(
    spec: AdversarialCaseSpec,
) -> Result<AdversarialEmbedderInjectionCase, MejepaInferError> {
    let AdversarialCaseSpec {
        case_id,
        kind,
        threat_vector,
        sequence,
        mode_b_global,
        mode_b_cells,
        proposer_used_window_ids,
        heldout_window_ids,
        expected_reason,
        promotion_request_if_accepted,
        created_at_unix_ms,
    } = spec;
    let proposal_id = proposal_id_for(case_id);
    let comparison = EmbedderCandidateHoldoutComparison {
        schema_version: EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
        proposal_id,
        candidate_id: RuntimeEmbedderId::dynamic(sequence, dynamic_id_name(case_id))
            .map_err(embed_error)?,
        candidate_name: format!("adversarial_{case_id}_candidate"),
        candidate_architecture_signature: format!(
            "adversarial-injection:{case_id}:synthetic-absence-signal"
        ),
        candidate_artifact_sha256: sha256_text(&format!("artifact:{case_id}")),
        training_cert_chain_hash: sha256_text(&format!("train-cert:{case_id}")),
        proposal_source_refs: vec![
            format!("adversarial_unknown_cluster:{case_id}"),
            format!("threat_vector:{kind:?}"),
        ],
        proposer_used_window_ids,
        heldout_window_ids,
        mode_a: eval(
            0.900,
            BTreeMap::from([
                ("auth:python".to_string(), 0.860),
                ("io:python".to_string(), 0.850),
            ]),
        )?,
        mode_b: eval(mode_b_global, mode_b_cells)?,
        mode_c: eval(
            0.898,
            BTreeMap::from([
                ("auth:python".to_string(), 0.858),
                ("io:python".to_string(), 0.848),
            ]),
        )?,
        created_at_unix_ms,
    };
    let case = AdversarialEmbedderInjectionCase {
        schema_version: ADVERSARIAL_EMBEDDER_INJECTION_SCHEMA_VERSION,
        case_id: case_id.to_string(),
        kind,
        threat_vector: threat_vector.to_string(),
        expected_falsification_reason_code: expected_reason.map(ToOwned::to_owned),
        comparison,
        promotion_request_if_accepted,
    };
    validate_case(&case)?;
    Ok(case)
}

fn promotion_request(
    case_id: &str,
    sequence: u32,
) -> Result<DynamicEmbedderPromotionGateRequest, MejepaInferError> {
    let candidate_id =
        RuntimeEmbedderId::dynamic(sequence, dynamic_id_name(case_id)).map_err(embed_error)?;
    Ok(DynamicEmbedderPromotionGateRequest {
        candidate_id_slug: candidate_id.slug().into_owned(),
        candidate_name: format!("adversarial_{case_id}_candidate"),
        proposal_id_hex: hex::encode(proposal_id_for(case_id)),
        falsification_passed: true,
        redundant_with_existing: false,
        within_vram_budget: true,
        first_dynamic_promotion: false,
        adversarial_injection_risk: true,
        would_cross_ship_gate: false,
        modifies_below_threshold_cell: false,
        heldout_global_delta: 0.0,
        min_cell_delta: 0.0,
        reason: format!("TASK-EK-018J adversarial injection survivor: {case_id}"),
    })
}

fn validate_case(case: &AdversarialEmbedderInjectionCase) -> Result<(), MejepaInferError> {
    if case.schema_version != ADVERSARIAL_EMBEDDER_INJECTION_SCHEMA_VERSION {
        return invalid(
            "adversarial_embedder_injection.schema_version",
            format!(
                "expected {ADVERSARIAL_EMBEDDER_INJECTION_SCHEMA_VERSION}, got {}",
                case.schema_version
            ),
        );
    }
    validate_single_line("adversarial_embedder_injection.case_id", &case.case_id, 128)?;
    validate_single_line(
        "adversarial_embedder_injection.threat_vector",
        &case.threat_vector,
        512,
    )?;
    case.comparison.validate()?;
    if case.expected_falsification_reason_code.is_none()
        && !case
            .promotion_request_if_accepted
            .as_ref()
            .map(|request| request.adversarial_injection_risk)
            .unwrap_or(false)
    {
        return invalid(
            "adversarial_embedder_injection.promotion_request",
            "accepted adversarial cases must be marked adversarial_injection_risk",
        );
    }
    Ok(())
}

fn eval(global: f32, cells: BTreeMap<String, f32>) -> Result<HoldoutEval, MejepaInferError> {
    HoldoutEval::try_new_with_cells(0.95, global, 0.01, 256, [17u8; 32], cells)
        .map_err(map_heal_error)
}

fn proposal_id_for(case_id: &str) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"TASK_EK_018J_ADVERSARIAL_EMBEDDER_INJECTION");
    hasher.update(case_id.as_bytes());
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

fn dynamic_id_name(case_id: &str) -> String {
    format!("{}_candidate", case_id.replace('-', "_"))
}

fn validate_single_line(field: &str, value: &str, max_len: usize) -> Result<(), MejepaInferError> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.len() > max_len
        || value.bytes().any(|byte| byte == 0 || byte == b'\n')
    {
        return invalid(
            field,
            format!("must be non-empty trimmed single-line text up to {max_len} bytes"),
        );
    }
    Ok(())
}

fn invalid<T>(field: impl Into<String>, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.into(),
        detail: detail.into(),
    })
}

fn map_heal_error(err: HealError) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: "adversarial_embedder_injection.heal_gate".to_string(),
        detail: err.to_string(),
    }
}

fn embed_error(err: context_graph_mejepa_embedders::EmbedError) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: "adversarial_embedder_injection.candidate_id".to_string(),
        detail: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::heal::promote::ModeWinner;
    use crate::open_infer_rocksdb;

    #[test]
    fn adversarial_injection_cases_are_rejected_or_operator_gated() {
        let temp = tempfile::tempdir().unwrap();
        let infer_db = open_infer_rocksdb(temp.path().join("infer")).unwrap();
        let heal_store = HealRocksStore::open(temp.path().join("heal")).unwrap();
        let cases = adversarial_embedder_injection_cases(1_779_100_000_000).unwrap();
        let report = evaluate_adversarial_embedder_injection(
            infer_db.as_ref(),
            heal_store.as_ref(),
            &cases,
            EmbedderFalsificationGate::default(),
        )
        .unwrap();

        assert!(report.all_passed);
        assert_eq!(report.cases_total, 4);
        assert_eq!(report.falsification_rejected_count, 3);
        assert_eq!(report.operator_gate_caught_count, 1);
        assert_eq!(report.unexpected_survivor_count, 0);
        assert!(report.cases.iter().any(|case| {
            case.falsification_decision.winner == ModeWinner::B
                && case.operator_gate_caught_survivor
        }));
    }
}
