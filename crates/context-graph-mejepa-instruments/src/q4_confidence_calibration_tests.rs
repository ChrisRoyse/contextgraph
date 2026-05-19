use super::*;

#[test]
fn calibrated_head_meets_quality_gate() {
    let examples = calibration_examples(240, 1, Q4CalibrationHead::Reasoning, "overclaiming");
    let report =
        calibrate_q4_binary_class(Q4CalibrationHead::Reasoning, "overclaiming", &examples, 1)
            .unwrap();
    assert!(report.trust_supported());
    assert_eq!(report.status, Q4CalibrationStatus::Calibrated);
    assert!(report.ece.unwrap() <= Q4_CONFIDENCE_TARGET_ECE);
    assert!(report.precision_at_tau.unwrap() >= Q4_CONFIDENCE_TARGET_PRECISION);
    assert!((Q4_CONFIDENCE_COVERAGE_LOW..=Q4_CONFIDENCE_COVERAGE_HIGH)
        .contains(&report.empirical_coverage.unwrap()));
}

#[test]
fn insufficient_labels_fail_closed() {
    let examples = calibration_examples(60, 1, Q4CalibrationHead::Perf, "wallclock_ms");
    let report =
        calibrate_q4_binary_class(Q4CalibrationHead::Perf, "wallclock_ms", &examples, 1).unwrap();
    assert_eq!(
        report.status,
        Q4CalibrationStatus::InsufficientLabelsForCalibration
    );
    assert!(!report.trust_supported());
    assert!(report.fail_closed_reason.unwrap().contains("INSUFFICIENT"));
}

#[test]
fn stale_labels_fail_closed_before_fit() {
    let examples = calibration_examples(120, 0, Q4CalibrationHead::Accuracy, "accuracy");
    let report =
        calibrate_q4_binary_class(Q4CalibrationHead::Accuracy, "accuracy", &examples, 1).unwrap();
    assert_eq!(report.status, Q4CalibrationStatus::StaleLabels);
    assert!(!report.trust_supported());
    assert!(report
        .fail_closed_reason
        .unwrap()
        .contains("Q4_CALIBRATION_STALE_LABELS"));
}

#[test]
fn severe_class_imbalance_is_flagged() {
    let mut examples =
        calibration_examples(200, 1, Q4CalibrationHead::Security, "command_injection");
    for (idx, example) in examples.iter_mut().enumerate() {
        example.actual = idx < 4;
        example.raw_confidence = if example.actual { 0.91 } else { 0.08 };
    }
    let report = calibrate_q4_binary_class(
        Q4CalibrationHead::Security,
        "command_injection",
        &examples,
        1,
    )
    .unwrap();
    assert!(report.severe_class_imbalance);
}

#[test]
fn store_filters_q4cal_records_by_prefix() {
    let temp = tempfile::tempdir().unwrap();
    let store = Q4HeadCalibrationStore::open(temp.path()).unwrap();
    let examples = calibration_examples(140, 1, Q4CalibrationHead::Reasoning, "hedging");
    let report =
        calibrate_q4_binary_class(Q4CalibrationHead::Reasoning, "hedging", &examples, 1).unwrap();
    let keys = store.put_reports(std::slice::from_ref(&report)).unwrap();
    store.flush().unwrap();
    assert_eq!(keys, vec!["q4cal::reasoning::hedging".to_string()]);
    assert_eq!(store.count_reports().unwrap(), 1);
    let readback = store
        .get_report(Q4CalibrationHead::Reasoning, "hedging")
        .unwrap()
        .unwrap();
    assert_eq!(readback.report.head, report.head);
    assert_eq!(readback.report.class_name, report.class_name);
    assert_eq!(readback.report.status, report.status);
    assert_eq!(readback.report.label_count, report.label_count);
    assert!(readback.report.trust_supported());
}

fn calibration_examples(
    count: usize,
    schema_version: u32,
    head: Q4CalibrationHead,
    class_name: &str,
) -> Vec<Q4CalibrationExample> {
    (0..count)
        .map(|idx| {
            let actual = idx % 4 != 0;
            Q4CalibrationExample {
                row_id: format!("row-{idx:03}"),
                cell: if idx % 2 == 0 {
                    "subtle_flip::python".to_string()
                } else {
                    "known_good::python".to_string()
                },
                head,
                class_name: class_name.to_string(),
                raw_confidence: if actual {
                    0.82 + ((idx % 5) as f64 * 0.01)
                } else {
                    0.08 + ((idx % 5) as f64 * 0.01)
                },
                actual,
                label_schema_version: schema_version,
                source_artifact_sha256: "fixture-sha256".to_string(),
            }
        })
        .collect()
}
