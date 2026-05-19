#![allow(dead_code)]

use context_graph_mejepa::q4_trust_gate::{Q4EvidenceCatalog, Q4HeadEvidence, Q4HeadKind};
use context_graph_mejepa_instruments::{
    extract_q4_perf_labels, run_q4_perf_tools_with_timeout, Q4PerfCategory, Q4PerfCommandSpec,
    Q4PerfExtraction, Q4PerfLabelStore, Q4PerfRawOutputs, Q4PerfScanPhase, Q4PerfSource,
    Q4PerfToolKind, Q4PerfToolOutput,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(crate) type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize)]
pub(crate) struct EdgeCase {
    pub(crate) name: &'static str,
    pub(crate) passed: bool,
    pub(crate) detail: Value,
}

pub(crate) fn happy_source(row_id: &str) -> Q4PerfSource {
    Q4PerfSource {
        corpus_row_id: row_id.to_string(),
        chunk_id: "chunk-q4-perf-001".to_string(),
        logical_path: "app/perf.py".to_string(),
        benchmark_selector: "tests/test_perf.py::test_linear".to_string(),
    }
}

pub(crate) fn write_perf_fixture(root: &Path) -> FsvResult<PathBuf> {
    fs::create_dir_all(root)?;
    let script = root.join("q4_perf_fixture.py");
    let source = r#"
import argparse
import cProfile
import json
import pstats
import statistics
import time


def linear_workload() -> int:
    total = 0
    for i in range(360_000):
        total += (i * 17) & 31
    return total


def quadratic_workload() -> int:
    total = 0
    for i in range(1_050):
        for j in range(1_050):
            total += (i * j + i + j) & 7
    return total


def workload(phase: str) -> int:
    if phase == "pre":
        return linear_workload()
    if phase == "post":
        return quadratic_workload()
    raise ValueError(f"unknown phase: {phase}")


def emit_benchmark(phase: str) -> None:
    values = []
    for _ in range(5):
        start = time.perf_counter_ns()
        workload(phase)
        values.append(time.perf_counter_ns() - start)
    mean_ns = statistics.fmean(values)
    stddev_ns = statistics.pstdev(values)
    print(json.dumps({
        "benchmarks": [{
            "name": "test_linear",
            "stats": {
                "mean": mean_ns / 1_000_000_000.0,
                "stddev": stddev_ns / 1_000_000_000.0,
            },
        }],
    }))


def emit_cprofile(phase: str, profile_out: str) -> None:
    profiler = cProfile.Profile()
    profiler.enable()
    workload(phase)
    profiler.disable()
    profiler.dump_stats(profile_out)
    stats = pstats.Stats(profiler)
    print(json.dumps({"total_time_s": stats.total_tt}))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=["benchmark", "cprofile"])
    parser.add_argument("phase", choices=["pre", "post"])
    parser.add_argument("--profile-out", default="profile.prof")
    args = parser.parse_args()
    if args.mode == "benchmark":
        emit_benchmark(args.phase)
    else:
        emit_cprofile(args.phase, args.profile_out)


if __name__ == "__main__":
    main()
"#;
    fs::write(&script, source.trim_start())?;
    Ok(script)
}

pub(crate) fn run_fixture_outputs(
    fixture_root: &Path,
    fixture_script: &Path,
    row_id: &str,
) -> FsvResult<Q4PerfRawOutputs> {
    fs::create_dir_all(fixture_root.join("profiles").join(row_id))?;
    Ok(run_q4_perf_tools_with_timeout(
        &fixture_command(fixture_root, fixture_script, "benchmark", "pre", None),
        &fixture_command(fixture_root, fixture_script, "benchmark", "post", None),
        &fixture_command(
            fixture_root,
            fixture_script,
            "cprofile",
            "pre",
            Some(profile_path(fixture_root, row_id, "pre")),
        ),
        &fixture_command(
            fixture_root,
            fixture_script,
            "cprofile",
            "post",
            Some(profile_path(fixture_root, row_id, "post")),
        ),
        Duration::from_secs(30),
    )?)
}

pub(crate) fn profile_path(fixture_root: &Path, row_id: &str, phase: &str) -> PathBuf {
    fixture_root
        .join("profiles")
        .join(row_id)
        .join(format!("{phase}.prof"))
}

pub(crate) fn persist_extraction(
    store: &Q4PerfLabelStore,
    extraction: &Q4PerfExtraction,
) -> FsvResult<usize> {
    let keys = store.put_extraction(extraction)?;
    Ok(keys.len())
}

pub(crate) fn update_q4_catalog(
    path: &Path,
    producer_rows: u64,
    producer_fsv_root: &Path,
) -> FsvResult<Q4EvidenceCatalog> {
    let mut catalog = if path.exists() {
        serde_json::from_slice(&fs::read(path)?)?
    } else {
        Q4EvidenceCatalog::default()
    };
    let mut heads: BTreeMap<_, _> = std::mem::take(&mut catalog.heads);
    heads.insert(
        Q4HeadKind::Perf,
        Q4HeadEvidence {
            producer_fsv_root: Some(canonical_fsv_root(producer_fsv_root)),
            producer_rows,
            calibration_fsv_root: None,
            calibration_rows: 0,
            per_slot_evidence_root: None,
            slots_with_evidence: Vec::new(),
        },
    );
    catalog.heads = heads;
    write_json_checked(path, &catalog)?;
    Ok(catalog)
}

pub(crate) fn canonical_fsv_root(path: &Path) -> String {
    let mut raw = path.display().to_string();
    if !raw.ends_with('/') {
        raw.push('/');
    }
    raw
}

pub(crate) fn edge_unstable_benchmark_quarantined() -> FsvResult<EdgeCase> {
    let source = happy_source("edge-unstable");
    let outputs = happy_outputs(1_000_000.0, 1_500_000.0, 20_000.0, 800_000.0);
    let extraction = extract_q4_perf_labels(&source, &outputs)?;
    Ok(EdgeCase {
        name: "unstable_benchmark_quarantined",
        passed: extraction
            .quarantines
            .iter()
            .any(|quarantine| quarantine.reason_code == "Q4_PERF_LABEL_UNSTABLE_BENCHMARK")
            && !extraction
                .labels
                .iter()
                .any(|label| label.metric == "test_linear"),
        detail: json!({"labels": extraction.labels, "quarantines": extraction.quarantines}),
    })
}

pub(crate) fn edge_no_timing_tests_empty_success() -> FsvResult<EdgeCase> {
    let source = happy_source("edge-no-timing");
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
    let extraction = extract_q4_perf_labels(&source, &outputs)?;
    Ok(EdgeCase {
        name: "no_timing_relevant_tests_empty_success",
        passed: extraction.labels.is_empty() && extraction.quarantines.is_empty(),
        detail: json!({"labels": extraction.labels, "quarantines": extraction.quarantines}),
    })
}

pub(crate) fn edge_profiler_budget_exceeded() -> FsvResult<EdgeCase> {
    let source = happy_source("edge-budget");
    let mut outputs = happy_outputs(1_000_000.0, 1_100_000.0, 20_000.0, 20_000.0);
    outputs.cprofile_post.runtime_exceeded = true;
    outputs.cprofile_post.stdout.clear();
    let extraction = extract_q4_perf_labels(&source, &outputs)?;
    Ok(EdgeCase {
        name: "profiler_budget_exceeded_signal",
        passed: extraction.labels.iter().any(|label| {
            label.category == Q4PerfCategory::WallclockBudgetExceeded
                && label.baseline_ns.is_none()
                && label.after_ns.is_none()
                && label.delta_pct.is_none()
        }),
        detail: json!({"labels": extraction.labels, "quarantines": extraction.quarantines}),
    })
}

pub(crate) fn edge_negative_delta_improvement() -> FsvResult<EdgeCase> {
    let source = happy_source("edge-improvement");
    let outputs = happy_outputs(2_000_000.0, 1_000_000.0, 20_000.0, 20_000.0);
    let extraction = extract_q4_perf_labels(&source, &outputs)?;
    Ok(EdgeCase {
        name: "negative_delta_improvement_label",
        passed: extraction.labels.iter().any(|label| {
            label.metric == "test_linear"
                && label.category == Q4PerfCategory::Improvement
                && !label.regression
                && label.delta_pct.unwrap_or_default() < 0.0
        }),
        detail: json!({"labels": extraction.labels, "quarantines": extraction.quarantines}),
    })
}

pub(crate) fn write_json_checked(path: &Path, value: &impl Serialize) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let readback = fs::read(path)?;
    require("json readback matches write", readback == bytes)?;
    Ok(())
}

pub(crate) fn sha256_file(path: &Path) -> FsvResult<String> {
    let bytes = fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

pub(crate) fn require(name: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("requirement failed: {name}").into())
    }
}

fn fixture_command(
    cwd: &Path,
    script: &Path,
    mode: &str,
    phase: &str,
    profile_out: Option<PathBuf>,
) -> Q4PerfCommandSpec {
    let mut args = vec![
        script.display().to_string(),
        mode.to_string(),
        phase.to_string(),
    ];
    if let Some(profile_out) = profile_out {
        args.push("--profile-out".to_string());
        args.push(profile_out.display().to_string());
    }
    Q4PerfCommandSpec {
        program: "python3".to_string(),
        args,
        cwd: cwd.to_path_buf(),
    }
}

fn happy_outputs(
    baseline_ns: f64,
    after_ns: f64,
    baseline_stddev_ns: f64,
    after_stddev_ns: f64,
) -> Q4PerfRawOutputs {
    Q4PerfRawOutputs {
        benchmark_pre: output(
            Q4PerfToolKind::PytestBenchmark,
            Q4PerfScanPhase::PrePatch,
            metrics_json(&[("test_linear", baseline_ns, Some(baseline_stddev_ns))]).as_str(),
        ),
        benchmark_post: output(
            Q4PerfToolKind::PytestBenchmark,
            Q4PerfScanPhase::PostPatch,
            metrics_json(&[("test_linear", after_ns, Some(after_stddev_ns))]).as_str(),
        ),
        cprofile_pre: output(
            Q4PerfToolKind::CProfile,
            Q4PerfScanPhase::PrePatch,
            r#"{"total_time_s":0.021}"#,
        ),
        cprofile_post: output(
            Q4PerfToolKind::CProfile,
            Q4PerfScanPhase::PostPatch,
            r#"{"total_time_s":0.023}"#,
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
