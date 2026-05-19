use super::*;

#[test]
fn cost_regression_labels_are_emitted() {
    let source = sample_source("row-cost-001");
    let extraction = extract_q4_cost_labels(&source, &sample_outputs()).unwrap();
    assert!(extraction.quarantines.is_empty());
    assert!(extraction.labels.iter().any(|label| {
        label.kind == Q4CostKind::DependencyCount
            && label.baseline == 12.0
            && label.after == 14.0
            && label.delta == 2.0
            && label.regression
            && label.label_kind == Q4CostLabelKind::Regression
    }));
    assert!(extraction.labels.iter().any(|label| {
        label.kind == Q4CostKind::WheelBytes && label.delta == 2048.0 && label.regression
    }));
}

#[test]
fn raw_pytest_requirements_and_wheel_outputs_are_parsed() {
    let source = sample_source("row-cost-raw");
    let outputs = Q4CostRawOutputs {
        pre_patch: Q4CostToolOutput {
            phase: Q4CostScanPhase::PrePatch,
            command: vec!["python3".to_string(), "cost-analyzer.py".to_string()],
            status_code: Some(0),
            stdout: raw_cost_output(600.0, 10_000, 12),
            stderr: String::new(),
            runtime_exceeded: false,
            toolchain_missing: false,
        },
        post_patch: Q4CostToolOutput {
            phase: Q4CostScanPhase::PostPatch,
            command: vec!["python3".to_string(), "cost-analyzer.py".to_string()],
            status_code: Some(0),
            stdout: raw_cost_output(750.0, 12_048, 14),
            stderr: String::new(),
            runtime_exceeded: false,
            toolchain_missing: false,
        },
    };
    let extraction = extract_q4_cost_labels(&source, &outputs).unwrap();
    assert!(extraction.quarantines.is_empty());
    assert_eq!(extraction.labels.len(), 3);
    assert!(extraction
        .labels
        .iter()
        .any(|label| label.kind == Q4CostKind::CiMinutes && label.delta == 2.5));
    assert!(extraction
        .labels
        .iter()
        .any(|label| label.kind == Q4CostKind::DependencyCount && label.delta == 2.0));
    assert!(extraction
        .labels
        .iter()
        .any(|label| label.kind == Q4CostKind::WheelBytes && label.delta == 2048.0));
}

#[test]
fn row_without_dependency_manifest_does_not_synthesize_zero() {
    let mut source = sample_source("row-cost-no-deps");
    source.logical_path = "app/module.py".to_string();
    source.changed_paths = vec!["app/module.py".to_string()];
    let outputs = raw(
        r#"{"ci_minutes": 2.0, "wheel_bytes": 1024}"#,
        r#"{"ci_minutes": 2.5, "wheel_bytes": 2048}"#,
    );
    let extraction = extract_q4_cost_labels(&source, &outputs).unwrap();
    assert!(!extraction
        .labels
        .iter()
        .any(|label| label.kind == Q4CostKind::DependencyCount));
    assert!(extraction.quarantines.is_empty());
}

#[test]
fn dependency_count_without_manifest_change_quarantines_precondition() {
    let mut source = sample_source("row-cost-untracked-deps");
    source.logical_path = "app/module.py".to_string();
    source.changed_paths = vec!["app/module.py".to_string()];
    let outputs = raw(r#"{"dependency_count": 12}"#, r#"{"dependency_count": 14}"#);
    let extraction = extract_q4_cost_labels(&source, &outputs).unwrap();
    assert!(extraction.labels.is_empty());
    assert!(extraction
        .quarantines
        .iter()
        .any(|quarantine| { quarantine.reason_code == "Q4_COST_LABEL_PRECONDITION_MISSING" }));
}

#[test]
fn wheel_build_failure_is_quarantined() {
    let source = sample_source("row-cost-build-fail");
    let mut outputs = sample_outputs();
    outputs.post_patch.status_code = Some(1);
    outputs.post_patch.stdout.clear();
    outputs.post_patch.stderr = "building wheel failed".to_string();
    let extraction = extract_q4_cost_labels(&source, &outputs).unwrap();
    assert!(extraction.labels.is_empty());
    assert!(extraction.quarantines.iter().any(|quarantine| {
        quarantine.reason_code == "Q4_COST_LABEL_BUILD_FAILED"
            && quarantine.phase == Q4CostScanPhase::PostPatch
    }));
}

#[test]
fn removed_dependency_is_improvement_not_regression() {
    let source = sample_source("row-cost-improvement");
    let outputs = raw(r#"{"dependency_count": 14}"#, r#"{"dependency_count": 12}"#);
    let extraction = extract_q4_cost_labels(&source, &outputs).unwrap();
    let label = extraction.labels.first().unwrap();
    assert_eq!(label.kind, Q4CostKind::DependencyCount);
    assert_eq!(label.delta, -2.0);
    assert!(!label.regression);
    assert_eq!(label.label_kind, Q4CostLabelKind::Improvement);
}

#[test]
fn noisy_walltime_quarantines_only_ci_minutes() {
    let source = sample_source("row-cost-noisy-ci");
    let outputs = raw(
        r#"{"metrics":[
            {"kind":"ci_minutes","value":10.0,"stddev":3.0},
            {"kind":"dependency_count","value":12}
        ]}"#,
        r#"{"metrics":[
            {"kind":"ci_minutes","value":12.0,"stddev":0.4},
            {"kind":"dependency_count","value":14}
        ]}"#,
    );
    let extraction = extract_q4_cost_labels(&source, &outputs).unwrap();
    assert!(extraction
        .quarantines
        .iter()
        .any(|quarantine| { quarantine.reason_code == "Q4_COST_LABEL_UNSTABLE_WALLTIME" }));
    assert!(!extraction
        .labels
        .iter()
        .any(|label| label.kind == Q4CostKind::CiMinutes));
    assert!(extraction
        .labels
        .iter()
        .any(|label| label.kind == Q4CostKind::DependencyCount && label.regression));
}

#[test]
fn swapped_phase_fails_closed() {
    let source = sample_source("row-cost-bad-phase");
    let mut outputs = sample_outputs();
    outputs.pre_patch.phase = Q4CostScanPhase::PostPatch;
    assert!(extract_q4_cost_labels(&source, &outputs).is_err());
}

#[test]
fn inconsistent_cost_label_shape_fails_closed() {
    let label = Q4CostLabel {
        corpus_row_id: "row-cost-inconsistent".to_string(),
        chunk_id: "chunk-cost-001".to_string(),
        logical_path: "requirements.txt".to_string(),
        cost_selector: "pytest-and-build-wheel".to_string(),
        kind: Q4CostKind::DependencyCount,
        baseline: 14.0,
        after: 12.0,
        delta: 2.0,
        regression: true,
        label_kind: Q4CostLabelKind::Regression,
    };
    assert!(validate_label_shape(&label).is_err());
}

#[test]
fn label_key_keeps_same_row_different_chunks_separate() {
    let left = q4_cost_label_key(
        "row-cost-same-row",
        "chunk-cost-left",
        Q4CostKind::DependencyCount,
    );
    let right = q4_cost_label_key(
        "row-cost-same-row",
        "chunk-cost-right",
        Q4CostKind::DependencyCount,
    );
    assert_ne!(left, right);
    assert!(left.starts_with("row-cost-same-row:label:dependency_count:"));
    assert!(right.starts_with("row-cost-same-row:label:dependency_count:"));
}

fn sample_source(row_id: &str) -> Q4CostSource {
    Q4CostSource {
        corpus_row_id: row_id.to_string(),
        chunk_id: "chunk-cost-001".to_string(),
        logical_path: "app/requirements.txt".to_string(),
        cost_selector: "tests".to_string(),
        changed_paths: vec!["app/requirements.txt".to_string()],
    }
}

fn sample_outputs() -> Q4CostRawOutputs {
    raw(
        r#"{"ci_minutes": 10.0, "ci_minutes_stddev": 0.3, "dependencies": ["a","b","c","d","e","f","g","h","i","j","k","l"], "wheel_bytes": 10000}"#,
        r#"{"ci_minutes": 12.5, "ci_minutes_stddev": 0.4, "dependencies": ["a","b","c","d","e","f","g","h","i","j","k","l","numpy","scipy"], "wheel_bytes": 12048}"#,
    )
}

fn raw_cost_output(seconds: f64, wheel_bytes: u64, dependency_count: usize) -> String {
    let requirements = (0..dependency_count)
        .map(|idx| format!("dep{idx}==1.0"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "============================= test session starts =============================\n\
         tests/test_cost.py::test_cost PASSED\n\
         ============================== 1 passed in {seconds:.2}s ==============================\n\
         REQUIREMENTS_START\n{requirements}\nREQUIREMENTS_END\n\
         WHEEL_START\n\
         dist/demo-0.1.0-py3-none-any.whl {wheel_bytes}\n\
         WHEEL_END\n"
    )
}

fn raw(pre_stdout: &str, post_stdout: &str) -> Q4CostRawOutputs {
    Q4CostRawOutputs {
        pre_patch: output(Q4CostScanPhase::PrePatch, pre_stdout),
        post_patch: output(Q4CostScanPhase::PostPatch, post_stdout),
    }
}

fn output(phase: Q4CostScanPhase, stdout: &str) -> Q4CostToolOutput {
    Q4CostToolOutput {
        phase,
        command: vec!["python3".to_string()],
        status_code: Some(0),
        stdout: stdout.to_string(),
        stderr: String::new(),
        runtime_exceeded: false,
        toolchain_missing: false,
    }
}
