use super::*;

#[test]
fn pass_rows_compute_nontrivial_rate_and_exclude_known_good() {
    let annotations = sample_annotations(120);
    let predictions = annotations
        .iter()
        .filter(|annotation| annotation.annotation_kind == Q4PassAnnotationKind::Concern)
        .take(80)
        .map(|annotation| Q4PassPredictedConcern {
            row_id: annotation.row_id.clone(),
            cell: annotation.cell.clone(),
            head_kind: annotation.head_kind,
            concern: annotation.concern.clone(),
            confidence: 0.91,
        })
        .collect::<Vec<_>>();
    let report = evaluate_q4_pass_nontrivial(&annotations, &predictions, None).unwrap();
    assert_eq!(report.annotation_count, 120);
    assert_eq!(report.known_good_excluded_rows, 12);
    assert_eq!(report.denominator_rows, 108);
    assert_eq!(report.matched_rows, 80);
    assert_eq!(report.status, Q4PassMetricStatus::Pass);
    assert!(report.q4_pass_nontrivial_rate >= Q4_PASS_TARGET_NONTRIVIAL_RATE);
}

#[test]
fn insufficient_annotations_fail_closed() {
    let annotations = sample_annotations(50);
    let predictions = Vec::new();
    let report = evaluate_q4_pass_nontrivial(&annotations, &predictions, None).unwrap();
    assert_eq!(
        report.status,
        Q4PassMetricStatus::InsufficientPassAnnotations
    );
    assert!(!report.per_cell.iter().any(|cell| cell.passed_threshold));
}

#[test]
fn operator_novel_concerns_are_surfaced() {
    let annotations = sample_annotations(120);
    let predictions = vec![Q4PassPredictedConcern {
        row_id: annotations[1].row_id.clone(),
        cell: annotations[1].cell.clone(),
        head_kind: Q4PassHeadKind::Security,
        concern: "new unchecked deserialization boundary".to_string(),
        confidence: 0.88,
    }];
    let report = evaluate_q4_pass_nontrivial(&annotations, &predictions, None).unwrap();
    assert_eq!(report.operator_novel_concerns.len(), 1);
    assert_eq!(
        report.operator_novel_concerns[0].concern,
        "new unchecked deserialization boundary"
    );
}

#[test]
fn regression_drop_is_flagged() {
    let annotations = sample_annotations(120);
    let predictions = annotations
        .iter()
        .filter(|annotation| annotation.annotation_kind == Q4PassAnnotationKind::Concern)
        .take(70)
        .map(|annotation| Q4PassPredictedConcern {
            row_id: annotation.row_id.clone(),
            cell: annotation.cell.clone(),
            head_kind: annotation.head_kind,
            concern: annotation.concern.clone(),
            confidence: 0.90,
        })
        .collect::<Vec<_>>();
    let report = evaluate_q4_pass_nontrivial(&annotations, &predictions, Some(0.90)).unwrap();
    assert!(report
        .regression_alert
        .as_deref()
        .unwrap()
        .contains("Q4_PASS_NONTRIVIAL_REGRESSION"));
}

#[test]
fn canonical_key_uses_head_and_concern_hash() {
    let left = q4_pass_annotation_key("row-pass-001", Q4PassHeadKind::Perf, "slow import path");
    let right = q4_pass_annotation_key("row-pass-001", Q4PassHeadKind::Cost, "slow import path");
    assert_ne!(left, right);
    assert!(left.starts_with("row-pass-001:perf:"));
    assert!(right.starts_with("row-pass-001:cost:"));
}

fn sample_annotations(count: usize) -> Vec<Q4PassAnnotation> {
    (0..count)
        .map(|idx| {
            let no_concern = idx % 10 == 0;
            Q4PassAnnotation {
                row_id: format!("row-pass-{idx:03}"),
                cell: if idx % 2 == 0 {
                    "known_good::python".to_string()
                } else {
                    "subtle_flip::python".to_string()
                },
                head_kind: if idx % 3 == 0 {
                    Q4PassHeadKind::Perf
                } else {
                    Q4PassHeadKind::TechDebt
                },
                concern: if no_concern {
                    "no plausible concern exists".to_string()
                } else {
                    format!("latent pass concern {}", idx % 7)
                },
                severity: Q4PassSeverity::Low,
                justification: "rule-derived pass-row review annotation".to_string(),
                annotation_kind: if no_concern {
                    Q4PassAnnotationKind::KnownGoodNoConcern
                } else {
                    Q4PassAnnotationKind::Concern
                },
            }
        })
        .collect()
}
