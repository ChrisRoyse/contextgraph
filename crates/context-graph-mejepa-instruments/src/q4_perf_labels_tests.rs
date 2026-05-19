use super::*;

#[test]
fn stable_regression_and_improvement_labels_are_emitted() {
    let source = sample_source("row-perf-001");
    let extraction = extract_q4_perf_labels(&source, &sample_outputs()).unwrap();
    assert!(extraction.quarantines.is_empty());
    assert!(extraction.labels.iter().any(|label| {
        label.metric == "test_linear"
            && label.category == Q4PerfCategory::WallclockMs
            && label.regression
            && label.delta_pct.unwrap() > 25.0
    }));
    assert!(extraction.labels.iter().any(|label| {
        label.metric == "cprofile_total_time"
            && label.category == Q4PerfCategory::Improvement
            && !label.regression
            && label.delta_pct.unwrap() < 0.0
    }));
}

#[test]
fn unstable_benchmark_quarantines_without_label() {
    let source = sample_source("row-perf-unstable");
    let mut outputs = sample_outputs();
    outputs.benchmark_post.stdout = metrics_json(&[("test_linear", 2_000_000.0, Some(900_000.0))]);
    let extraction = extract_q4_perf_labels(&source, &outputs).unwrap();
    assert!(extraction.quarantines.iter().any(|quarantine| {
        quarantine.reason_code == "Q4_PERF_LABEL_UNSTABLE_BENCHMARK"
            && quarantine.tool == Q4PerfToolKind::PytestBenchmark
    }));
    assert!(!extraction
        .labels
        .iter()
        .any(|label| label.metric == "test_linear"));
}

#[test]
fn no_timing_relevant_tests_is_empty_success() {
    let source = sample_source("row-perf-empty");
    let outputs = Q4PerfRawOutputs {
        benchmark_pre: output(
            Q4PerfToolKind::PytestBenchmark,
            Q4PerfScanPhase::PrePatch,
            "",
        ),
        benchmark_post: output(
            Q4PerfToolKind::PytestBenchmark,
            Q4PerfScanPhase::PostPatch,
            "",
        ),
        cprofile_pre: output(Q4PerfToolKind::CProfile, Q4PerfScanPhase::PrePatch, ""),
        cprofile_post: output(Q4PerfToolKind::CProfile, Q4PerfScanPhase::PostPatch, ""),
    };
    let extraction = extract_q4_perf_labels(&source, &outputs).unwrap();
    assert!(extraction.labels.is_empty());
    assert!(extraction.quarantines.is_empty());
}

#[test]
fn profiler_budget_exceeded_is_signal_without_fake_delta() {
    let source = sample_source("row-perf-budget");
    let mut outputs = sample_outputs();
    outputs.cprofile_post.runtime_exceeded = true;
    outputs.cprofile_post.stdout.clear();
    let extraction = extract_q4_perf_labels(&source, &outputs).unwrap();
    let budget = extraction
        .labels
        .iter()
        .find(|label| label.category == Q4PerfCategory::WallclockBudgetExceeded)
        .unwrap();
    assert_eq!(budget.baseline_ns, None);
    assert_eq!(budget.after_ns, None);
    assert_eq!(budget.delta_pct, None);
    assert!(budget.regression);
}

#[test]
fn pytest_benchmark_without_stability_is_quarantined() {
    let source = sample_source("row-perf-missing-stability");
    let mut outputs = sample_outputs();
    outputs.benchmark_post.stdout = metrics_json(&[("test_linear", 2_000_000.0, None)]);
    let extraction = extract_q4_perf_labels(&source, &outputs).unwrap();
    assert!(extraction.quarantines.iter().any(|quarantine| {
        quarantine.reason_code == "Q4_PERF_LABEL_MISSING_STABILITY"
            && quarantine.tool == Q4PerfToolKind::PytestBenchmark
    }));
    assert!(!extraction
        .labels
        .iter()
        .any(|label| label.metric == "test_linear"));
}

#[test]
fn swapped_phase_or_failed_command_fails_closed() {
    let source = sample_source("row-perf-bad-phase");
    let mut outputs = sample_outputs();
    outputs.benchmark_pre.phase = Q4PerfScanPhase::PostPatch;
    assert!(extract_q4_perf_labels(&source, &outputs).is_err());

    let mut outputs = sample_outputs();
    outputs.benchmark_post.status_code = Some(2);
    assert!(extract_q4_perf_labels(&source, &outputs).is_err());
}

fn sample_source(row_id: &str) -> Q4PerfSource {
    Q4PerfSource {
        corpus_row_id: row_id.to_string(),
        chunk_id: "chunk-perf-001".to_string(),
        logical_path: "app/perf.py".to_string(),
        benchmark_selector: "tests/test_perf.py::test_linear".to_string(),
    }
}

fn sample_outputs() -> Q4PerfRawOutputs {
    Q4PerfRawOutputs {
        benchmark_pre: output(
            Q4PerfToolKind::PytestBenchmark,
            Q4PerfScanPhase::PrePatch,
            &metrics_json(&[("test_linear", 1_000_000.0, Some(20_000.0))]),
        ),
        benchmark_post: output(
            Q4PerfToolKind::PytestBenchmark,
            Q4PerfScanPhase::PostPatch,
            &metrics_json(&[("test_linear", 1_420_000.0, Some(25_000.0))]),
        ),
        cprofile_pre: output(
            Q4PerfToolKind::CProfile,
            Q4PerfScanPhase::PrePatch,
            r#"{"total_time_s":0.020}"#,
        ),
        cprofile_post: output(
            Q4PerfToolKind::CProfile,
            Q4PerfScanPhase::PostPatch,
            r#"{"total_time_s":0.015}"#,
        ),
    }
}

fn output(tool: Q4PerfToolKind, phase: Q4PerfScanPhase, stdout: &str) -> Q4PerfToolOutput {
    Q4PerfToolOutput {
        tool,
        phase,
        command: vec![tool.as_str().to_string()],
        status_code: Some(0),
        stdout: stdout.to_string(),
        stderr: String::new(),
        runtime_exceeded: false,
        toolchain_missing: false,
    }
}

fn metrics_json(rows: &[(&str, f64, Option<f64>)]) -> String {
    serde_json::json!({
        "metrics": rows.iter().map(|(metric, mean_ns, stddev_ns)| {
            serde_json::json!({
                "metric": metric,
                "mean_ns": mean_ns,
                "stddev_ns": stddev_ns,
                "category": "wallclock_ms"
            })
        }).collect::<Vec<_>>()
    })
    .to_string()
}
