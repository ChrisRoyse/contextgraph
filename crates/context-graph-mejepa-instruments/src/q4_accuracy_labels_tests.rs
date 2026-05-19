use super::*;

fn source(row: &str) -> Q4AccuracySource {
    Q4AccuracySource {
        corpus_row_id: row.to_string(),
        chunk_id: "chunk-q4-accuracy".to_string(),
        logical_path: "app/model.py".to_string(),
        source_test: "tests/test_model.py::test_quality".to_string(),
    }
}

fn output(phase: Q4AccuracyScanPhase, stdout: &str) -> Q4AccuracyToolOutput {
    Q4AccuracyToolOutput {
        phase,
        command: vec!["python3".to_string()],
        status_code: Some(0),
        stdout: stdout.to_string(),
        stderr: String::new(),
        runtime_exceeded: false,
        toolchain_missing: false,
    }
}

fn raw(pre: &str, post: &str) -> Q4AccuracyRawOutputs {
    Q4AccuracyRawOutputs {
        pre_patch: output(Q4AccuracyScanPhase::PrePatch, pre),
        post_patch: output(Q4AccuracyScanPhase::PostPatch, post),
    }
}

#[test]
fn accuracy_drop_emits_regression_label() {
    let extraction = extract_q4_accuracy_labels(
        &source("row-accuracy-drop"),
        &raw(
            r#"{"metrics":[{"metric":"accuracy","value":0.91,"source_test":"tests/test_model.py::test_quality"}]}"#,
            r#"{"metrics":[{"metric":"accuracy","value":0.83,"source_test":"tests/test_model.py::test_quality"}]}"#,
        ),
    )
    .unwrap();
    assert!(extraction.quarantines.is_empty());
    let label = &extraction.labels[0];
    assert_eq!(label.metric_name, "accuracy");
    assert!(label.regression);
    assert_eq!(label.kind, Q4AccuracyLabelKind::Regression);
    assert!((label.delta_pct + 8.7912087912).abs() < 0.01);
}

#[test]
fn loss_increase_emits_regression_label() {
    let extraction = extract_q4_accuracy_labels(
        &source("row-mse-regression"),
        &raw(
            r#"{"metrics":[{"metric":"mse","value":0.10}]}"#,
            r#"{"metrics":[{"metric":"mse","value":0.14}]}"#,
        ),
    )
    .unwrap();
    assert!(extraction.labels[0].regression);
    assert_eq!(
        extraction.labels[0].metric_kind,
        Q4AccuracyMetricKind::MeanSquaredError
    );
}

#[test]
fn no_accuracy_metrics_is_empty_success() {
    let extraction = extract_q4_accuracy_labels(
        &source("row-no-metrics"),
        &raw("3 passed in 0.02s", "3 passed in 0.02s"),
    )
    .unwrap();
    assert!(extraction.labels.is_empty());
    assert!(extraction.quarantines.is_empty());
}

#[test]
fn free_form_metric_text_quarantines_parse_failure() {
    let extraction = extract_q4_accuracy_labels(
        &source("row-prose"),
        &raw(
            "accuracy improved but not numeric",
            "accuracy dropped badly",
        ),
    )
    .unwrap();
    assert!(extraction.labels.is_empty());
    assert!(extraction
        .quarantines
        .iter()
        .all(|q| q.reason_code == "Q4_ACCURACY_LABEL_PARSE_FAILURE"));
}

#[test]
fn unstable_seed_quarantines_metric() {
    let extraction = extract_q4_accuracy_labels(
        &source("row-unstable"),
        &raw(
            r#"{"metrics":[{"metric":"accuracy","value":0.91,"stddev":0.02}]}"#,
            r#"{"metrics":[{"metric":"accuracy","value":0.83,"stddev":0.0}]}"#,
        ),
    )
    .unwrap();
    assert!(extraction.labels.is_empty());
    assert!(extraction
        .quarantines
        .iter()
        .any(|q| q.reason_code == "Q4_ACCURACY_LABEL_UNSTABLE_SEED"));
}

#[test]
fn fix_direction_is_preserved() {
    let extraction = extract_q4_accuracy_labels(
        &source("row-fix"),
        &raw(
            r#"{"metrics":[{"metric":"accuracy","value":0.83}]}"#,
            r#"{"metrics":[{"metric":"accuracy","value":0.91}]}"#,
        ),
    )
    .unwrap();
    assert!(!extraction.labels[0].regression);
    assert_eq!(extraction.labels[0].kind, Q4AccuracyLabelKind::Fix);
}

#[test]
fn duplicate_metric_different_source_tests_do_not_collapse() {
    let extraction = extract_q4_accuracy_labels(
        &source("row-two-accuracy-tests"),
        &raw(
            r#"{"metrics":[{"metric":"accuracy","value":0.91,"source_test":"tests/test_a.py::test_quality"},{"metric":"accuracy","value":0.77,"source_test":"tests/test_b.py::test_quality"}]}"#,
            r#"{"metrics":[{"metric":"accuracy","value":0.83,"source_test":"tests/test_a.py::test_quality"},{"metric":"accuracy","value":0.79,"source_test":"tests/test_b.py::test_quality"}]}"#,
        ),
    )
    .unwrap();
    assert_eq!(extraction.labels.len(), 2);
    assert_eq!(
        extraction
            .labels
            .iter()
            .filter(|label| label.metric_name == "accuracy")
            .count(),
        2
    );
    assert!(extraction
        .labels
        .iter()
        .any(|label| { label.source_test == "tests/test_a.py::test_quality" && label.regression }));
    assert!(extraction.labels.iter().any(|label| {
        label.source_test == "tests/test_b.py::test_quality" && !label.regression
    }));
}

#[test]
fn loss_alias_is_lower_is_better() {
    let extraction = extract_q4_accuracy_labels(
        &source("row-loss-regression"),
        &raw(
            r#"{"metrics":[{"metric":"cross_entropy","value":0.20}]}"#,
            r#"{"metrics":[{"metric":"cross_entropy","value":0.25}]}"#,
        ),
    )
    .unwrap();
    assert!(extraction.labels[0].regression);
    assert_eq!(
        extraction.labels[0].metric_kind,
        Q4AccuracyMetricKind::CrossEntropy
    );
}
