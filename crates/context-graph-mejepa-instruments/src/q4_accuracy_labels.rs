// TASK-PY-G-048: durable Q4 accuracy/data-metric labels.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::{InstrumentError, InstrumentResult};

const Q4_ACCURACY_SCHEMA_VERSION: u32 = 1;
const MAX_LABELS: usize = 100_000;
const MAX_OUTPUT_BYTES: usize = 20_000_000;
const UNSTABLE_VALUE_THRESHOLD: f64 = 0.0001;
const REGRESSION_DELTA_THRESHOLD_PCT: f64 = 1.0;
pub const DEFAULT_Q4_ACCURACY_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4AccuracyScanPhase {
    PrePatch,
    PostPatch,
}

impl Q4AccuracyScanPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PrePatch => "pre_patch",
            Self::PostPatch => "post_patch",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4AccuracyMetricKind {
    Accuracy,
    F1,
    Precision,
    Recall,
    Auc,
    MeanAbsoluteError,
    MeanSquaredError,
    R2,
    Rouge,
    Loss,
    LogLoss,
    CrossEntropy,
    Perplexity,
    CalibrationError,
    BrierScore,
    Other,
}

impl Q4AccuracyMetricKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accuracy => "accuracy",
            Self::F1 => "f1",
            Self::Precision => "precision",
            Self::Recall => "recall",
            Self::Auc => "auc",
            Self::MeanAbsoluteError => "mae",
            Self::MeanSquaredError => "mse",
            Self::R2 => "r2",
            Self::Rouge => "rouge",
            Self::Loss => "loss",
            Self::LogLoss => "log_loss",
            Self::CrossEntropy => "cross_entropy",
            Self::Perplexity => "perplexity",
            Self::CalibrationError => "calibration_error",
            Self::BrierScore => "brier_score",
            Self::Other => "other",
        }
    }

    pub fn lower_is_better(self) -> bool {
        matches!(
            self,
            Self::MeanAbsoluteError
                | Self::MeanSquaredError
                | Self::Loss
                | Self::LogLoss
                | Self::CrossEntropy
                | Self::Perplexity
                | Self::CalibrationError
                | Self::BrierScore
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4AccuracyLabelKind {
    Regression,
    Fix,
    Stable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4AccuracySource {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub logical_path: String,
    pub source_test: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4AccuracyCommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4AccuracyToolOutput {
    pub phase: Q4AccuracyScanPhase,
    pub command: Vec<String>,
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub runtime_exceeded: bool,
    pub toolchain_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4AccuracyRawOutputs {
    pub pre_patch: Q4AccuracyToolOutput,
    pub post_patch: Q4AccuracyToolOutput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4AccuracyLabel {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub metric_name: String,
    pub metric_kind: Q4AccuracyMetricKind,
    pub baseline_value: f64,
    pub after_value: f64,
    pub delta_pct: f64,
    pub regression: bool,
    pub kind: Q4AccuracyLabelKind,
    pub source_test: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4AccuracyQuarantine {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub phase: Q4AccuracyScanPhase,
    pub reason_code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record_kind", content = "record", rename_all = "snake_case")]
pub enum Q4AccuracySignalRecord {
    Label(Q4AccuracyLabel),
    Quarantine(Q4AccuracyQuarantine),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedQ4AccuracySignal {
    pub schema_version: u32,
    pub signal: Q4AccuracySignalRecord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4AccuracyExtraction {
    pub labels: Vec<Q4AccuracyLabel>,
    pub quarantines: Vec<Q4AccuracyQuarantine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4AccuracyRawOutputPaths {
    pub row_id: String,
    pub root: PathBuf,
    pub metrics_json: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct AccuracyMeasurement {
    pub metric_name: String,
    pub value: f64,
    pub metric_kind: Q4AccuracyMetricKind,
    pub source_test: Option<String>,
    pub stddev: Option<f64>,
}

pub fn run_q4_accuracy_tools_with_timeout(
    pre_patch: &Q4AccuracyCommandSpec,
    post_patch: &Q4AccuracyCommandSpec,
    timeout: Duration,
) -> InstrumentResult<Q4AccuracyRawOutputs> {
    if timeout.is_zero() {
        return invalid(
            "q4_accuracy.timeout",
            "Q4 accuracy analyzer timeout is zero",
            "configure a positive caller-enforced timeout for metric commands",
        );
    }
    Ok(Q4AccuracyRawOutputs {
        pre_patch: run_command(Q4AccuracyScanPhase::PrePatch, pre_patch, timeout)?,
        post_patch: run_command(Q4AccuracyScanPhase::PostPatch, post_patch, timeout)?,
    })
}

pub fn extract_q4_accuracy_labels(
    source: &Q4AccuracySource,
    outputs: &Q4AccuracyRawOutputs,
) -> InstrumentResult<Q4AccuracyExtraction> {
    validate_source(source)?;
    validate_tool_output(&outputs.pre_patch, Q4AccuracyScanPhase::PrePatch)?;
    validate_tool_output(&outputs.post_patch, Q4AccuracyScanPhase::PostPatch)?;

    let mut quarantines = Vec::new();
    let pre = parse_measurements(source, &outputs.pre_patch, &mut quarantines)?;
    let post = parse_measurements(source, &outputs.post_patch, &mut quarantines)?;
    let mut labels = Vec::new();
    emit_delta_labels(source, pre, post, &mut labels, &mut quarantines)?;
    if labels.len() > MAX_LABELS {
        return invalid(
            "q4_accuracy.labels",
            format!(
                "Q4 accuracy label count {} exceeds {MAX_LABELS}",
                labels.len()
            ),
            "shard large accuracy analyzer outputs before persisting labels",
        );
    }
    labels.sort_by(|left, right| left.metric_name.cmp(&right.metric_name));
    for label in &labels {
        validate_label(label, source)?;
    }
    for quarantine in &quarantines {
        validate_quarantine(quarantine, source)?;
    }
    Ok(Q4AccuracyExtraction {
        labels,
        quarantines,
    })
}

pub fn write_q4_accuracy_raw_outputs(
    root: impl AsRef<Path>,
    row_id: &str,
    outputs: &Q4AccuracyRawOutputs,
) -> InstrumentResult<Q4AccuracyRawOutputPaths> {
    validate_path_component("row_id", row_id)?;
    let root = root.as_ref().join(row_id);
    fs::create_dir_all(&root).map_err(|err| {
        InstrumentError::store(
            "create_dir_all",
            "python-q4-accuracy-labels-v1",
            err.to_string(),
            "ensure the aiwonder Q4 accuracy raw-output directory is writable",
        )
    })?;
    let paths = Q4AccuracyRawOutputPaths {
        row_id: row_id.to_string(),
        metrics_json: root.join("metrics.json"),
        root,
    };
    write_raw_wrapper(&paths.metrics_json, &outputs.pre_patch, &outputs.post_patch)?;
    Ok(paths)
}

fn run_command(
    phase: Q4AccuracyScanPhase,
    spec: &Q4AccuracyCommandSpec,
    timeout: Duration,
) -> InstrumentResult<Q4AccuracyToolOutput> {
    validate_command_spec(spec)?;
    let command = std::iter::once(spec.program.clone())
        .chain(spec.args.clone())
        .collect::<Vec<_>>();
    let mut child = match Command::new(&spec.program)
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            return Ok(Q4AccuracyToolOutput {
                phase,
                command,
                status_code: None,
                stdout: String::new(),
                stderr: format!("failed to spawn {}: {err}", spec.program),
                runtime_exceeded: false,
                toolchain_missing: true,
            });
        }
    };
    let started = Instant::now();
    loop {
        match child.try_wait().map_err(|err| {
            InstrumentError::store(
                "try_wait",
                "python-q4-accuracy-labels-v1",
                err.to_string(),
                "inspect analyzer process state and caller timeout handling",
            )
        })? {
            Some(_) => {
                let output = child.wait_with_output().map_err(|err| {
                    InstrumentError::store(
                        "wait_with_output",
                        "python-q4-accuracy-labels-v1",
                        err.to_string(),
                        "inspect analyzer process output handling",
                    )
                })?;
                return tool_output_from_process(phase, command, output, false);
            }
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let output = child.wait_with_output().map_err(|err| {
                    InstrumentError::store(
                        "wait_timeout_output",
                        "python-q4-accuracy-labels-v1",
                        err.to_string(),
                        "inspect analyzer timeout process cleanup",
                    )
                })?;
                return tool_output_from_process(phase, command, output, true);
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn parse_measurements(
    source: &Q4AccuracySource,
    output: &Q4AccuracyToolOutput,
    quarantines: &mut Vec<Q4AccuracyQuarantine>,
) -> InstrumentResult<BTreeMap<String, AccuracyMeasurement>> {
    if output.toolchain_missing {
        quarantines.push(quarantine(
            source,
            output.phase,
            "Q4_ACCURACY_TOOLCHAIN_MISSING",
            "required metric extractor binary was unavailable",
        ));
        return Ok(BTreeMap::new());
    }
    if output.runtime_exceeded {
        quarantines.push(quarantine(
            source,
            output.phase,
            "Q4_ACCURACY_RUNTIME_EXCEEDED",
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
            "q4_accuracy.stdout",
            format!(
                "analyzer stdout length {} exceeds {MAX_OUTPUT_BYTES}",
                stdout.len()
            ),
            "persist large analyzer outputs as raw artifacts and pass bounded summaries",
        );
    }
    match parse_accuracy_measurement_output(stdout) {
        Ok(rows) => Ok(rows),
        Err(err) => {
            quarantines.push(quarantine(
                source,
                output.phase,
                "Q4_ACCURACY_LABEL_PARSE_FAILURE",
                err,
            ));
            Ok(BTreeMap::new())
        }
    }
}

#[path = "q4_accuracy_labels_parse.rs"]
mod q4_accuracy_labels_parse;
use q4_accuracy_labels_parse::parse_accuracy_measurement_output;

#[path = "q4_accuracy_labels_support.rs"]
mod q4_accuracy_labels_support;
use q4_accuracy_labels_support::*;

#[path = "q4_accuracy_labels_store.rs"]
mod q4_accuracy_labels_store;
pub use q4_accuracy_labels_store::Q4AccuracyLabelStore;

#[cfg(test)]
#[path = "q4_accuracy_labels_tests.rs"]
mod q4_accuracy_labels_tests;
