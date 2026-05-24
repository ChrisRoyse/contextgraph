use std::collections::BTreeMap;

use super::*;

#[test]
fn missing_support_fails_closed() {
    let report = evaluate_q4_trust_gate(&Q4EvidenceCatalog::default()).unwrap();
    assert!(!report.q4_head_ready);
    assert!(!report.q4_trust_decision_eligible);
    assert!(report.heads.iter().all(|head| !head.trusted_in_decision));
}

#[test]
fn complete_single_head_support_stays_display_only_under_freeze() {
    let mut catalog = Q4EvidenceCatalog::default();
    catalog.heads.insert(
        Q4HeadKind::Perf,
        complete_evidence_for(Q4HeadKind::Perf, Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT),
    );
    let report = evaluate_q4_trust_gate(&catalog).unwrap();
    let perf = report.head(Q4HeadKind::Perf).unwrap();
    assert!(!perf.q4_head_ready);
    assert!(!perf.trusted_in_decision);
    assert!(perf
        .missing_requirements
        .iter()
        .any(|item| item == Q4_DOCTRINE_FREEZE_REASON));
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

#[test]
fn trusted_q4_consequences_omits_all_heads_under_freeze() {
    let catalog = Q4EvidenceCatalog {
        heads: default_q4_requirements()
            .into_iter()
            .map(|requirement| {
                (
                    requirement.head,
                    complete_evidence_for(requirement.head, Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT),
                )
            })
            .collect(),
    };
    let report = evaluate_q4_trust_gate(&catalog).unwrap();
    let prediction = crate::RealityPredictionBuilder::from_parts(
        crate::TaskId("q4-freeze-unit".to_string()),
        [0x14; 16],
        crate::Language::Python,
        crate::ConformalSet::try_new(vec![crate::OracleOutcome::Pass], 0.1, 0.2).unwrap(),
    )
    .prediction_id([0x15; 16])
    .witness_hash(crate::WitnessHash([0x16; 32]))
    .verdict(crate::Verdict::Pass)
    .confidence_interval(crate::ConformalInterval {
        lower: 0.7,
        upper: 0.9,
        ..crate::ConformalInterval::default()
    })
    .predicted_oracle_pass(0.8)
    .predicted_test_pass(vec![0.8])
    .ood_score(0.1)
    .calibrated_confidence(0.8)
    .provenance(crate::PredictionProvenance {
        predictor_version: "q4-freeze-unit".to_string(),
        constellation_version: "q4-freeze-unit".to_string(),
        calibration_version: "q4-freeze-unit".to_string(),
        active_pointer: "q4-freeze-unit".to_string(),
        train_health_source: String::new(),
    })
    .source_panel_sha([0x17; 32])
    .calibration_version("q4-freeze-unit")
    .build()
    .unwrap();
    let trusted = trusted_q4_consequences(&prediction, &report);

    assert!(!trusted.q4_trust_decision_eligible);
    assert!(trusted.perf_regressions.is_empty());
    assert!(trusted.accuracy_degradations.is_empty());
    assert!(trusted.cost_regressions.is_empty());
    assert_eq!(trusted.reasoning_class, None);
    assert_eq!(trusted.omitted_heads.len(), default_q4_requirements().len());
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
