use std::collections::BTreeMap;

use super::*;

#[test]
fn missing_support_fails_closed() {
    let report = default_q4_trust_gate_report().unwrap();
    assert!(!report.q4_head_ready);
    assert!(!report.q4_trust_decision_eligible);
    assert!(report.heads.iter().all(|head| !head.trusted_in_decision));
}

#[test]
fn complete_single_head_support_flips_only_that_head() {
    let mut catalog = Q4EvidenceCatalog::default();
    catalog.heads.insert(
        Q4HeadKind::Perf,
        complete_evidence_for(Q4HeadKind::Perf, Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT),
    );
    let report = evaluate_q4_trust_gate(&catalog).unwrap();
    assert!(report.head(Q4HeadKind::Perf).unwrap().q4_head_ready);
    assert!(!report.q4_head_ready);
    assert!(!report.head(Q4HeadKind::Accuracy).unwrap().q4_head_ready);
}

#[test]
fn producer_without_calibration_is_untrusted() {
    let requirement = default_q4_requirements()
        .into_iter()
        .find(|item| item.head == Q4HeadKind::Cost)
        .unwrap();
    let evidence = Q4HeadEvidence {
        producer_fsv_root: Some(requirement.producer_fsv_root.clone()),
        producer_rows: Q4_DEFAULT_MIN_PRODUCER_ROWS,
        per_slot_evidence_root: Some(Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT.to_string()),
        slots_with_evidence: slots(),
        ..Q4HeadEvidence::default()
    };
    let readiness = evaluate_head(&requirement, &evidence).unwrap();
    assert!(!readiness.q4_head_ready);
    assert!(readiness.producer_supported);
    assert!(!readiness.calibration_supported);
}

#[test]
fn wrong_slot_names_fail_closed() {
    let mut evidence =
        complete_evidence_for(Q4HeadKind::Reasoning, Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT);
    evidence.slots_with_evidence = (0..Q4_DEFAULT_REQUIRED_SLOT_COUNT)
        .map(|idx| format!("wrong_slot_{idx}"))
        .collect();
    let catalog = Q4EvidenceCatalog {
        heads: BTreeMap::from([(Q4HeadKind::Reasoning, evidence)]),
    };
    let report = evaluate_q4_trust_gate(&catalog).unwrap();
    let reasoning = report.head(Q4HeadKind::Reasoning).unwrap();
    assert!(!reasoning.q4_head_ready);
    assert!(!reasoning.per_slot_supported);
    assert_eq!(
        reasoning.unexpected_slots.len(),
        Q4_DEFAULT_REQUIRED_SLOT_COUNT
    );
}

fn complete_evidence_for(head: Q4HeadKind, slot_root: &str) -> Q4HeadEvidence {
    let requirement = default_q4_requirements()
        .into_iter()
        .find(|item| item.head == head)
        .unwrap();
    Q4HeadEvidence {
        producer_fsv_root: Some(requirement.producer_fsv_root),
        producer_rows: Q4_DEFAULT_MIN_PRODUCER_ROWS,
        calibration_fsv_root: Some(requirement.calibration_fsv_root),
        calibration_rows: Q4_DEFAULT_MIN_CALIBRATION_ROWS,
        per_slot_evidence_root: Some(slot_root.to_string()),
        slots_with_evidence: slots(),
    }
}

fn slots() -> Vec<String> {
    q4_trust_gate_support::active_slots()
}
