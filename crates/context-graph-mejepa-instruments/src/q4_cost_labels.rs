// TASK-PY-G-049: durable Q4 CI/dependency/wheel-size cost labels.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{InstrumentError, InstrumentResult};

const Q4_COST_SCHEMA_VERSION: u32 = 1;
const MAX_LABELS: usize = 100_000;
const MAX_OUTPUT_BYTES: usize = 20_000_000;
const UNSTABLE_CV_THRESHOLD: f64 = 0.20;
pub const DEFAULT_Q4_COST_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4CostScanPhase {
    PrePatch,
    PostPatch,
}

impl Q4CostScanPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PrePatch => "pre_patch",
            Self::PostPatch => "post_patch",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4CostKind {
    CiMinutes,
    DependencyCount,
    WheelBytes,
}

impl Q4CostKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CiMinutes => "ci_minutes",
            Self::DependencyCount => "dependency_count",
            Self::WheelBytes => "wheel_bytes",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4CostLabelKind {
    Regression,
    Improvement,
    Stable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4CostSource {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub logical_path: String,
    pub cost_selector: String,
    pub changed_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4CostCommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4CostToolOutput {
    pub phase: Q4CostScanPhase,
    pub command: Vec<String>,
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub runtime_exceeded: bool,
    pub toolchain_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4CostRawOutputs {
    pub pre_patch: Q4CostToolOutput,
    pub post_patch: Q4CostToolOutput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4CostLabel {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub logical_path: String,
    pub cost_selector: String,
    pub kind: Q4CostKind,
    pub baseline: f64,
    pub after: f64,
    pub delta: f64,
    pub regression: bool,
    pub label_kind: Q4CostLabelKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4CostQuarantine {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub logical_path: String,
    pub cost_selector: String,
    pub phase: Q4CostScanPhase,
    pub reason_code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record_kind", content = "record", rename_all = "snake_case")]
pub enum Q4CostSignalRecord {
    Label(Q4CostLabel),
    Quarantine(Q4CostQuarantine),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedQ4CostSignal {
    pub schema_version: u32,
    pub signal: Q4CostSignalRecord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4CostExtraction {
    pub labels: Vec<Q4CostLabel>,
    pub quarantines: Vec<Q4CostQuarantine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4CostRawOutputPaths {
    pub row_id: String,
    pub root: PathBuf,
    pub cost_json: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct CostMeasurement {
    pub kind: Q4CostKind,
    pub value: f64,
    pub stddev: Option<f64>,
}

pub fn run_q4_cost_tools_with_timeout(
    pre_patch: &Q4CostCommandSpec,
    post_patch: &Q4CostCommandSpec,
    timeout: Duration,
) -> InstrumentResult<Q4CostRawOutputs> {
    if timeout.is_zero() {
        return invalid(
            "q4_cost.timeout",
            "Q4 cost analyzer timeout is zero",
            "configure a positive caller-enforced timeout for cost analyzer commands",
        );
    }
    Ok(Q4CostRawOutputs {
        pre_patch: run_command(Q4CostScanPhase::PrePatch, pre_patch, timeout)?,
        post_patch: run_command(Q4CostScanPhase::PostPatch, post_patch, timeout)?,
    })
}

pub fn extract_q4_cost_labels(
    source: &Q4CostSource,
    outputs: &Q4CostRawOutputs,
) -> InstrumentResult<Q4CostExtraction> {
    validate_source(source)?;
    validate_tool_output(&outputs.pre_patch, Q4CostScanPhase::PrePatch)?;
    validate_tool_output(&outputs.post_patch, Q4CostScanPhase::PostPatch)?;

    let mut quarantines = Vec::new();
    let pre = parse_measurements(source, &outputs.pre_patch, &mut quarantines)?;
    let post = parse_measurements(source, &outputs.post_patch, &mut quarantines)?;
    let mut labels = Vec::new();
    emit_delta_labels(source, pre, post, &mut labels, &mut quarantines)?;
    if labels.len() > MAX_LABELS {
        return invalid(
            "q4_cost.labels",
            format!("Q4 cost label count {} exceeds {MAX_LABELS}", labels.len()),
            "shard large cost analyzer outputs before persisting labels",
        );
    }
    labels.sort_by_key(|label| label.kind);
    for label in &labels {
        validate_label(label, source)?;
    }
    for quarantine in &quarantines {
        validate_quarantine(quarantine, source)?;
    }
    Ok(Q4CostExtraction {
        labels,
        quarantines,
    })
}

pub fn write_q4_cost_raw_outputs(
    root: impl AsRef<Path>,
    row_id: &str,
    outputs: &Q4CostRawOutputs,
) -> InstrumentResult<Q4CostRawOutputPaths> {
    validate_path_component("row_id", row_id)?;
    let root = root.as_ref().join(row_id);
    fs::create_dir_all(&root).map_err(|err| {
        InstrumentError::store(
            "create_dir_all",
            "python-q4-cost-labels-v1",
            err.to_string(),
            "ensure the aiwonder Q4 cost raw-output directory is writable",
        )
    })?;
    let paths = Q4CostRawOutputPaths {
        row_id: row_id.to_string(),
        cost_json: root.join("cost.json"),
        root,
    };
    write_raw_wrapper(&paths.cost_json, &outputs.pre_patch, &outputs.post_patch)?;
    Ok(paths)
}

fn parse_measurements(
    source: &Q4CostSource,
    output: &Q4CostToolOutput,
    quarantines: &mut Vec<Q4CostQuarantine>,
) -> InstrumentResult<BTreeMap<Q4CostKind, CostMeasurement>> {
    if output.toolchain_missing {
        quarantines.push(quarantine(
            source,
            output.phase,
            "Q4_COST_TOOLCHAIN_MISSING",
            "required cost analyzer binary was unavailable",
        ));
        return Ok(BTreeMap::new());
    }
    if output.runtime_exceeded {
        quarantines.push(quarantine(
            source,
            output.phase,
            "Q4_COST_RUNTIME_EXCEEDED",
            "cost analyzer exceeded the caller-enforced runtime limit",
        ));
        return Ok(BTreeMap::new());
    }
    if output.status_code != Some(0) {
        quarantines.push(quarantine(
            source,
            output.phase,
            "Q4_COST_LABEL_BUILD_FAILED",
            format!(
                "cost analyzer exited with status {:?}: {}",
                output.status_code,
                output.stderr.trim()
            ),
        ));
        return Ok(BTreeMap::new());
    }
    let stdout = output.stdout.trim();
    if stdout.is_empty() {
        return Ok(BTreeMap::new());
    }
    if stdout.len() > MAX_OUTPUT_BYTES {
        return invalid(
            "q4_cost.stdout",
            format!(
                "analyzer stdout length {} exceeds {MAX_OUTPUT_BYTES}",
                stdout.len()
            ),
            "persist large analyzer outputs as raw artifacts and pass bounded summaries",
        );
    }
    match parse_cost_measurements(stdout, &output.command) {
        Ok(rows) => Ok(rows),
        Err(err) => {
            quarantines.push(quarantine(
                source,
                output.phase,
                "Q4_COST_LABEL_PARSE_FAILURE",
                err,
            ));
            Ok(BTreeMap::new())
        }
    }
}

#[path = "q4_cost_labels_support.rs"]
mod q4_cost_labels_support;

use q4_cost_labels_support::*;

#[path = "q4_cost_labels_parse.rs"]
mod q4_cost_labels_parse;
use q4_cost_labels_parse::parse_cost_measurements;

#[path = "q4_cost_labels_store.rs"]
mod q4_cost_labels_store;
pub use q4_cost_labels_store::Q4CostLabelStore;

#[cfg(test)]
#[path = "q4_cost_labels_tests.rs"]
mod q4_cost_labels_tests;
