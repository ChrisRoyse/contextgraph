// TASK-PY-G-047: durable pytest-benchmark + cProfile Q4 perf labels.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{InstrumentError, InstrumentResult};

const Q4_PERF_SCHEMA_VERSION: u32 = 1;
const MAX_LABELS: usize = 100_000;
const MAX_OUTPUT_BYTES: usize = 20_000_000;
const UNSTABLE_CV_THRESHOLD: f64 = 0.15;
const REGRESSION_DELTA_THRESHOLD_PCT: f64 = 25.0;
pub const DEFAULT_Q4_PERF_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4PerfToolKind {
    PytestBenchmark,
    CProfile,
}

impl Q4PerfToolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PytestBenchmark => "pytest_benchmark",
            Self::CProfile => "cprofile",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4PerfScanPhase {
    PrePatch,
    PostPatch,
}

impl Q4PerfScanPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PrePatch => "pre_patch",
            Self::PostPatch => "post_patch",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4PerfCategory {
    CpuMs,
    WallclockMs,
    AllocCount,
    RssKb,
    WallclockBudgetExceeded,
    Improvement,
}

impl Q4PerfCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CpuMs => "cpu_ms",
            Self::WallclockMs => "wallclock_ms",
            Self::AllocCount => "alloc_count",
            Self::RssKb => "rss_kb",
            Self::WallclockBudgetExceeded => "wallclock_budget_exceeded",
            Self::Improvement => "improvement",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4PerfSource {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub logical_path: String,
    pub benchmark_selector: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4PerfCommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4PerfToolOutput {
    pub tool: Q4PerfToolKind,
    pub phase: Q4PerfScanPhase,
    pub command: Vec<String>,
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub runtime_exceeded: bool,
    pub toolchain_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4PerfRawOutputs {
    pub benchmark_pre: Q4PerfToolOutput,
    pub benchmark_post: Q4PerfToolOutput,
    pub cprofile_pre: Q4PerfToolOutput,
    pub cprofile_post: Q4PerfToolOutput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4PerfLabel {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub metric: String,
    pub category: Q4PerfCategory,
    pub baseline_ns: Option<f64>,
    pub after_ns: Option<f64>,
    pub delta_pct: Option<f64>,
    pub regression: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4PerfQuarantine {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub tool: Q4PerfToolKind,
    pub phase: Q4PerfScanPhase,
    pub reason_code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record_kind", content = "record", rename_all = "snake_case")]
pub enum Q4PerfSignalRecord {
    Label(Q4PerfLabel),
    Quarantine(Q4PerfQuarantine),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedQ4PerfSignal {
    pub schema_version: u32,
    pub signal: Q4PerfSignalRecord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4PerfExtraction {
    pub labels: Vec<Q4PerfLabel>,
    pub quarantines: Vec<Q4PerfQuarantine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4PerfRawOutputPaths {
    pub row_id: String,
    pub root: PathBuf,
    pub benchmark_json: PathBuf,
    pub cprofile_json: PathBuf,
}

#[derive(Debug, Clone, Copy)]
struct PerfMeasurement {
    value_ns: f64,
    stddev_ns: Option<f64>,
    category: Q4PerfCategory,
}

pub fn run_q4_perf_tools_with_timeout(
    benchmark_pre: &Q4PerfCommandSpec,
    benchmark_post: &Q4PerfCommandSpec,
    cprofile_pre: &Q4PerfCommandSpec,
    cprofile_post: &Q4PerfCommandSpec,
    timeout: Duration,
) -> InstrumentResult<Q4PerfRawOutputs> {
    if timeout.is_zero() {
        return invalid(
            "q4_perf.timeout",
            "Q4 perf analyzer timeout is zero",
            "configure a positive caller-enforced timeout for benchmark/profile commands",
        );
    }
    Ok(Q4PerfRawOutputs {
        benchmark_pre: run_command(
            Q4PerfToolKind::PytestBenchmark,
            Q4PerfScanPhase::PrePatch,
            benchmark_pre,
            timeout,
        )?,
        benchmark_post: run_command(
            Q4PerfToolKind::PytestBenchmark,
            Q4PerfScanPhase::PostPatch,
            benchmark_post,
            timeout,
        )?,
        cprofile_pre: run_command(
            Q4PerfToolKind::CProfile,
            Q4PerfScanPhase::PrePatch,
            cprofile_pre,
            timeout,
        )?,
        cprofile_post: run_command(
            Q4PerfToolKind::CProfile,
            Q4PerfScanPhase::PostPatch,
            cprofile_post,
            timeout,
        )?,
    })
}

pub fn extract_q4_perf_labels(
    source: &Q4PerfSource,
    outputs: &Q4PerfRawOutputs,
) -> InstrumentResult<Q4PerfExtraction> {
    validate_source(source)?;
    validate_tool_output(
        &outputs.benchmark_pre,
        Q4PerfToolKind::PytestBenchmark,
        Q4PerfScanPhase::PrePatch,
    )?;
    validate_tool_output(
        &outputs.benchmark_post,
        Q4PerfToolKind::PytestBenchmark,
        Q4PerfScanPhase::PostPatch,
    )?;
    validate_tool_output(
        &outputs.cprofile_pre,
        Q4PerfToolKind::CProfile,
        Q4PerfScanPhase::PrePatch,
    )?;
    validate_tool_output(
        &outputs.cprofile_post,
        Q4PerfToolKind::CProfile,
        Q4PerfScanPhase::PostPatch,
    )?;

    let mut labels = Vec::new();
    let mut quarantines = Vec::new();
    collect_benchmark_labels(source, outputs, &mut labels, &mut quarantines)?;
    collect_cprofile_labels(source, outputs, &mut labels, &mut quarantines)?;
    if labels.len() > MAX_LABELS {
        return invalid(
            "q4_perf.labels",
            format!("Q4 perf label count {} exceeds {MAX_LABELS}", labels.len()),
            "shard large perf analyzer outputs before persisting labels",
        );
    }
    labels.sort_by(|left, right| {
        left.metric
            .cmp(&right.metric)
            .then(left.category.cmp(&right.category))
    });
    for label in &labels {
        validate_label(label, source)?;
    }
    for quarantine in &quarantines {
        validate_quarantine(quarantine, source)?;
    }
    Ok(Q4PerfExtraction {
        labels,
        quarantines,
    })
}

pub fn write_q4_perf_raw_outputs(
    root: impl AsRef<Path>,
    row_id: &str,
    outputs: &Q4PerfRawOutputs,
) -> InstrumentResult<Q4PerfRawOutputPaths> {
    validate_path_component("row_id", row_id)?;
    let root = root.as_ref().join(row_id);
    fs::create_dir_all(&root).map_err(|err| {
        InstrumentError::store(
            "create_dir_all",
            "python-q4-perf-labels-v1",
            err.to_string(),
            "ensure the aiwonder Q4 perf raw-output directory is writable",
        )
    })?;
    let paths = Q4PerfRawOutputPaths {
        row_id: row_id.to_string(),
        benchmark_json: root.join("benchmark.json"),
        cprofile_json: root.join("cprofile.json"),
        root,
    };
    write_raw_wrapper(
        &paths.benchmark_json,
        &outputs.benchmark_pre,
        &outputs.benchmark_post,
    )?;
    write_raw_wrapper(
        &paths.cprofile_json,
        &outputs.cprofile_pre,
        &outputs.cprofile_post,
    )?;
    Ok(paths)
}

fn collect_benchmark_labels(
    source: &Q4PerfSource,
    outputs: &Q4PerfRawOutputs,
    labels: &mut Vec<Q4PerfLabel>,
    quarantines: &mut Vec<Q4PerfQuarantine>,
) -> InstrumentResult<()> {
    let pre = parse_measurements(source, &outputs.benchmark_pre, quarantines)?;
    let post = parse_measurements(source, &outputs.benchmark_post, quarantines)?;
    emit_delta_labels(
        source,
        Q4PerfToolKind::PytestBenchmark,
        Q4PerfScanPhase::PostPatch,
        pre,
        post,
        labels,
        quarantines,
    )
}

fn collect_cprofile_labels(
    source: &Q4PerfSource,
    outputs: &Q4PerfRawOutputs,
    labels: &mut Vec<Q4PerfLabel>,
    quarantines: &mut Vec<Q4PerfQuarantine>,
) -> InstrumentResult<()> {
    if outputs.cprofile_pre.runtime_exceeded || outputs.cprofile_post.runtime_exceeded {
        labels.push(Q4PerfLabel {
            corpus_row_id: source.corpus_row_id.clone(),
            chunk_id: source.chunk_id.clone(),
            metric: "cprofile_walltime_budget".to_string(),
            category: Q4PerfCategory::WallclockBudgetExceeded,
            baseline_ns: None,
            after_ns: None,
            delta_pct: None,
            regression: true,
        });
        return Ok(());
    }
    let pre = parse_measurements(source, &outputs.cprofile_pre, quarantines)?;
    let post = parse_measurements(source, &outputs.cprofile_post, quarantines)?;
    emit_delta_labels(
        source,
        Q4PerfToolKind::CProfile,
        Q4PerfScanPhase::PostPatch,
        pre,
        post,
        labels,
        quarantines,
    )
}

fn parse_measurements(
    source: &Q4PerfSource,
    output: &Q4PerfToolOutput,
    quarantines: &mut Vec<Q4PerfQuarantine>,
) -> InstrumentResult<BTreeMap<String, PerfMeasurement>> {
    if output.toolchain_missing {
        quarantines.push(quarantine(
            source,
            output.tool,
            output.phase,
            "Q4_PERF_TOOLCHAIN_MISSING",
            "required benchmark/profiler binary was unavailable",
        ));
        return Ok(BTreeMap::new());
    }
    if output.runtime_exceeded {
        quarantines.push(quarantine(
            source,
            output.tool,
            output.phase,
            "Q4_PERF_RUNTIME_EXCEEDED",
            "analyzer exceeded the caller-enforced runtime limit",
        ));
        return Ok(BTreeMap::new());
    }
    let stdout = output.stdout.trim();
    if stdout.is_empty() {
        return Ok(BTreeMap::new());
    }
    if stdout.len() > MAX_OUTPUT_BYTES {
        return invalid(
            "q4_perf.stdout",
            format!(
                "analyzer stdout length {} exceeds {MAX_OUTPUT_BYTES}",
                stdout.len()
            ),
            "persist large analyzer outputs as raw artifacts and pass bounded summaries",
        );
    }
    match parse_measurement_json(stdout, output.tool) {
        Ok(rows) => Ok(rows),
        Err(err) => {
            quarantines.push(quarantine(
                source,
                output.tool,
                output.phase,
                "Q4_PERF_LABEL_PARSE_FAILURE",
                err,
            ));
            Ok(BTreeMap::new())
        }
    }
}

#[path = "q4_perf_labels_support.rs"]
mod q4_perf_labels_support;

use q4_perf_labels_support::*;

#[path = "q4_perf_labels_parse.rs"]
mod q4_perf_labels_parse;
use q4_perf_labels_parse::parse_measurement_json;

#[path = "q4_perf_labels_store.rs"]
mod q4_perf_labels_store;
pub use q4_perf_labels_store::Q4PerfLabelStore;

#[cfg(test)]
#[path = "q4_perf_labels_tests.rs"]
mod q4_perf_labels_tests;
