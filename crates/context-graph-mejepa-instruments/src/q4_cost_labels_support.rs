use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use rocksdb::Options;
use sha2::{Digest, Sha256};

use super::*;

pub(super) fn run_command(
    phase: Q4CostScanPhase,
    spec: &Q4CostCommandSpec,
    timeout: Duration,
) -> InstrumentResult<Q4CostToolOutput> {
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
            return Ok(Q4CostToolOutput {
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
                "python-q4-cost-labels-v1",
                err.to_string(),
                "inspect analyzer process state and caller timeout handling",
            )
        })? {
            Some(_) => {
                let output = child.wait_with_output().map_err(|err| {
                    InstrumentError::store(
                        "wait_with_output",
                        "python-q4-cost-labels-v1",
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
                        "python-q4-cost-labels-v1",
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

pub(super) fn emit_delta_labels(
    source: &Q4CostSource,
    pre: BTreeMap<Q4CostKind, CostMeasurement>,
    post: BTreeMap<Q4CostKind, CostMeasurement>,
    labels: &mut Vec<Q4CostLabel>,
    quarantines: &mut Vec<Q4CostQuarantine>,
) -> InstrumentResult<()> {
    if pre.is_empty() && post.is_empty() {
        return Ok(());
    }
    let mut keys = BTreeSet::new();
    keys.extend(pre.keys().copied());
    keys.extend(post.keys().copied());
    for kind in keys {
        if kind == Q4CostKind::DependencyCount && !dependency_manifest_change_tracked(source) {
            quarantines.push(quarantine(
                source,
                Q4CostScanPhase::PostPatch,
                "Q4_COST_LABEL_PRECONDITION_MISSING",
                "dependency_count measurement present but no requirements*.txt or pyproject.toml change was tracked",
            ));
            continue;
        }
        let Some(before) = pre.get(&kind) else {
            quarantines.push(quarantine(
                source,
                Q4CostScanPhase::PrePatch,
                "Q4_COST_LABEL_MISSING_BASELINE",
                format!("{} missing from pre-patch output", kind.as_str()),
            ));
            continue;
        };
        let Some(after) = post.get(&kind) else {
            quarantines.push(quarantine(
                source,
                Q4CostScanPhase::PostPatch,
                "Q4_COST_LABEL_MISSING_AFTER",
                format!("{} missing from post-patch output", kind.as_str()),
            ));
            continue;
        };
        if before.value < 0.0 || after.value < 0.0 {
            quarantines.push(quarantine(
                source,
                Q4CostScanPhase::PostPatch,
                "Q4_COST_LABEL_NEGATIVE_VALUE",
                format!(
                    "{} had negative baseline/after values: {} -> {}",
                    kind.as_str(),
                    before.value,
                    after.value
                ),
            ));
            continue;
        }
        if unstable(before) || unstable(after) {
            let reason = if kind == Q4CostKind::CiMinutes {
                "Q4_COST_LABEL_UNSTABLE_WALLTIME"
            } else {
                "Q4_COST_LABEL_UNSTABLE_MEASUREMENT"
            };
            quarantines.push(quarantine(
                source,
                Q4CostScanPhase::PostPatch,
                reason,
                format!(
                    "{} exceeded stddev/mean threshold {UNSTABLE_CV_THRESHOLD}",
                    kind.as_str()
                ),
            ));
            continue;
        }
        let delta = after.value - before.value;
        let label_kind = if delta > 0.0 {
            Q4CostLabelKind::Regression
        } else if delta < 0.0 {
            Q4CostLabelKind::Improvement
        } else {
            Q4CostLabelKind::Stable
        };
        labels.push(Q4CostLabel {
            corpus_row_id: source.corpus_row_id.clone(),
            chunk_id: source.chunk_id.clone(),
            logical_path: source.logical_path.clone(),
            cost_selector: source.cost_selector.clone(),
            kind,
            baseline: before.value,
            after: after.value,
            delta,
            regression: label_kind == Q4CostLabelKind::Regression,
            label_kind,
        });
    }
    Ok(())
}

pub(super) fn q4_cost_record_key(record: &PersistedQ4CostSignal, _value: &[u8]) -> String {
    match &record.signal {
        Q4CostSignalRecord::Label(label) => {
            q4_cost_label_key(&label.corpus_row_id, &label.chunk_id, label.kind)
        }
        Q4CostSignalRecord::Quarantine(quarantine) => format!(
            "{}:quarantine:{}:{}:{}:{}",
            quarantine.corpus_row_id,
            stable_hash(&quarantine.chunk_id),
            quarantine.phase.as_str(),
            quarantine.reason_code,
            stable_hash(&quarantine.detail),
        ),
    }
}

pub(super) fn q4_cost_label_key(corpus_row_id: &str, chunk_id: &str, kind: Q4CostKind) -> String {
    format!(
        "{corpus_row_id}:label:{}:{}",
        kind.as_str(),
        stable_hash(chunk_id)
    )
}

pub(super) fn quarantine(
    source: &Q4CostSource,
    phase: Q4CostScanPhase,
    reason_code: &str,
    detail: impl Into<String>,
) -> Q4CostQuarantine {
    Q4CostQuarantine {
        corpus_row_id: source.corpus_row_id.clone(),
        chunk_id: source.chunk_id.clone(),
        logical_path: source.logical_path.clone(),
        cost_selector: source.cost_selector.clone(),
        phase,
        reason_code: reason_code.to_string(),
        detail: detail.into(),
    }
}

pub(super) fn validate_source(source: &Q4CostSource) -> InstrumentResult<()> {
    validate_path_component("corpus_row_id", &source.corpus_row_id)?;
    validate_non_empty_single_line("chunk_id", &source.chunk_id)?;
    validate_non_empty_single_line("logical_path", &source.logical_path)?;
    validate_non_empty_single_line("cost_selector", &source.cost_selector)?;
    for changed_path in &source.changed_paths {
        validate_non_empty_single_line("q4_cost.source.changed_path", changed_path)?;
    }
    Ok(())
}

pub(super) fn validate_tool_output(
    output: &Q4CostToolOutput,
    expected_phase: Q4CostScanPhase,
) -> InstrumentResult<()> {
    if output.phase != expected_phase {
        return invalid(
            "q4_cost.output.phase",
            format!(
                "tool output phase {:?} does not match expected {:?}",
                output.phase, expected_phase
            ),
            "wire pre/post cost outputs to their matching parser",
        );
    }
    if output.command.is_empty() {
        return invalid(
            "q4_cost.output.command",
            "tool output command provenance is empty",
            "persist analyzer command provenance with every raw output",
        );
    }
    Ok(())
}

pub(super) fn validate_label(label: &Q4CostLabel, source: &Q4CostSource) -> InstrumentResult<()> {
    if label.corpus_row_id != source.corpus_row_id {
        return invalid(
            "q4_cost.label.corpus_row_id",
            "label corpus_row_id does not match source",
            "persist Q4 cost labels against the exact corpus row being measured",
        );
    }
    validate_label_shape(label)
}

pub(super) fn validate_quarantine(
    quarantine: &Q4CostQuarantine,
    source: &Q4CostSource,
) -> InstrumentResult<()> {
    if quarantine.corpus_row_id != source.corpus_row_id {
        return invalid(
            "q4_cost.quarantine.corpus_row_id",
            "quarantine corpus_row_id does not match source",
            "persist Q4 cost quarantines against the exact corpus row being measured",
        );
    }
    validate_quarantine_shape(quarantine)
}

pub(super) fn validate_signal_record(record: &PersistedQ4CostSignal) -> InstrumentResult<()> {
    if record.schema_version != Q4_COST_SCHEMA_VERSION {
        return invalid(
            "q4_cost.signal.schema_version",
            format!(
                "unsupported Q4 cost schema version {}",
                record.schema_version
            ),
            "write Q4 cost evidence through the current producer",
        );
    }
    match &record.signal {
        Q4CostSignalRecord::Label(label) => validate_label_shape(label),
        Q4CostSignalRecord::Quarantine(quarantine) => validate_quarantine_shape(quarantine),
    }
}

pub(super) fn validate_label_shape(label: &Q4CostLabel) -> InstrumentResult<()> {
    validate_path_component("q4_cost.label.corpus_row_id", &label.corpus_row_id)?;
    validate_non_empty_single_line("q4_cost.label.chunk_id", &label.chunk_id)?;
    validate_non_empty_single_line("q4_cost.label.logical_path", &label.logical_path)?;
    validate_non_empty_single_line("q4_cost.label.cost_selector", &label.cost_selector)?;
    validate_finite("q4_cost.label.baseline", label.baseline)?;
    validate_finite("q4_cost.label.after", label.after)?;
    validate_finite("q4_cost.label.delta", label.delta)?;
    if label.baseline < 0.0 || label.after < 0.0 {
        return invalid(
            "q4_cost.label.value",
            "baseline/after values must be non-negative",
            "persist raw cost values as non-negative CI minutes, dependency counts, or bytes",
        );
    }
    let expected_delta = label.after - label.baseline;
    if (label.delta - expected_delta).abs() > 1e-9_f64.max(expected_delta.abs() * 1e-9) {
        return invalid(
            "q4_cost.label.delta",
            format!(
                "delta {} does not match after-baseline {}",
                label.delta, expected_delta
            ),
            "derive Q4 cost delta from measured baseline and after values",
        );
    }
    let expected_kind = if label.delta > 0.0 {
        Q4CostLabelKind::Regression
    } else if label.delta < 0.0 {
        Q4CostLabelKind::Improvement
    } else {
        Q4CostLabelKind::Stable
    };
    if label.label_kind != expected_kind {
        return invalid(
            "q4_cost.label.label_kind",
            "label_kind disagrees with delta direction",
            "derive Q4 cost label_kind from the cost delta direction",
        );
    }
    if label.regression != (label.label_kind == Q4CostLabelKind::Regression) {
        return invalid(
            "q4_cost.label.regression",
            "regression boolean disagrees with label_kind",
            "derive Q4 cost regression from the cost delta direction",
        );
    }
    Ok(())
}

pub(super) fn validate_quarantine_shape(quarantine: &Q4CostQuarantine) -> InstrumentResult<()> {
    validate_path_component(
        "q4_cost.quarantine.corpus_row_id",
        &quarantine.corpus_row_id,
    )?;
    validate_non_empty_single_line("q4_cost.quarantine.chunk_id", &quarantine.chunk_id)?;
    validate_non_empty_single_line("q4_cost.quarantine.logical_path", &quarantine.logical_path)?;
    validate_non_empty_single_line(
        "q4_cost.quarantine.cost_selector",
        &quarantine.cost_selector,
    )?;
    validate_non_empty_single_line("q4_cost.quarantine.reason_code", &quarantine.reason_code)?;
    validate_non_empty_single_line("q4_cost.quarantine.detail", &quarantine.detail)
}

pub(super) fn validate_path_component(field: &'static str, value: &str) -> InstrumentResult<()> {
    validate_non_empty_single_line(field, value)?;
    if value.contains('/') || value.contains('\\') || value == "." || value == ".." {
        return invalid(
            field,
            format!("{field} must be a single path component"),
            "use stable corpus-row identifiers, not filesystem paths, for raw-output directories",
        );
    }
    Ok(())
}

pub(super) fn invalid<T>(
    field: &'static str,
    message: impl Into<String>,
    remediation: &'static str,
) -> InstrumentResult<T> {
    Err(InstrumentError::invalid(field, message, remediation))
}

pub(super) fn cf_options() -> Options {
    let mut opts = Options::default();
    opts.set_paranoid_checks(true);
    opts
}

pub(super) fn write_raw_wrapper(
    path: &Path,
    pre: &Q4CostToolOutput,
    post: &Q4CostToolOutput,
) -> InstrumentResult<()> {
    let value = serde_json::json!({
        "schema_version": Q4_COST_SCHEMA_VERSION,
        "pre_patch": pre,
        "post_patch": post,
    });
    let bytes = serde_json::to_vec_pretty(&value).map_err(|err| {
        InstrumentError::store(
            "serialize_raw_output",
            "python-q4-cost-labels-v1",
            err.to_string(),
            "ensure raw Q4 cost output wrappers remain JSON serializable",
        )
    })?;
    fs::write(path, &bytes).map_err(|err| {
        InstrumentError::store(
            "write_raw_output",
            "python-q4-cost-labels-v1",
            err.to_string(),
            "ensure the aiwonder Q4 cost raw-output directory is writable",
        )
    })?;
    let readback = fs::read(path).map_err(|err| {
        InstrumentError::store(
            "read_raw_output",
            "python-q4-cost-labels-v1",
            err.to_string(),
            "read back raw analyzer output after writing it",
        )
    })?;
    if readback != bytes {
        return Err(InstrumentError::store(
            "read_after_write_raw_output",
            "python-q4-cost-labels-v1",
            format!(
                "{} readback bytes differ from written bytes",
                path.display()
            ),
            "do not advance Q4 cost checkpoints until raw output is durable",
        ));
    }
    Ok(())
}

pub(super) fn tool_output_from_process(
    phase: Q4CostScanPhase,
    command: Vec<String>,
    output: std::process::Output,
    runtime_exceeded: bool,
) -> InstrumentResult<Q4CostToolOutput> {
    Ok(Q4CostToolOutput {
        phase,
        command,
        status_code: output.status.code(),
        stdout: String::from_utf8(output.stdout).map_err(|err| {
            InstrumentError::invalid(
                "q4_cost.stdout",
                format!("analyzer stdout was not UTF-8: {err}"),
                "configure Q4 cost analyzers to emit UTF-8 JSON summaries",
            )
        })?,
        stderr: String::from_utf8(output.stderr).map_err(|err| {
            InstrumentError::invalid(
                "q4_cost.stderr",
                format!("analyzer stderr was not UTF-8: {err}"),
                "configure Q4 cost analyzers to emit UTF-8 diagnostics",
            )
        })?,
        runtime_exceeded,
        toolchain_missing: false,
    })
}

pub(super) fn validate_command_spec(spec: &Q4CostCommandSpec) -> InstrumentResult<()> {
    validate_non_empty_single_line("q4_cost.command.program", &spec.program)?;
    if !spec.cwd.is_dir() {
        return invalid(
            "q4_cost.command.cwd",
            format!("{} is not a directory", spec.cwd.display()),
            "materialize the cost analyzer workspace before running Q4 cost analysis",
        );
    }
    Ok(())
}

fn unstable(measurement: &CostMeasurement) -> bool {
    measurement
        .stddev
        .map(|stddev| {
            measurement.value > 0.0 && (stddev.abs() / measurement.value) > UNSTABLE_CV_THRESHOLD
        })
        .unwrap_or(false)
}

fn validate_finite(field: &'static str, value: f64) -> InstrumentResult<()> {
    if !value.is_finite() {
        return invalid(
            field,
            "value is not finite",
            "persist finite Q4 cost metrics",
        );
    }
    Ok(())
}

fn stable_hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn dependency_manifest_change_tracked(source: &Q4CostSource) -> bool {
    std::iter::once(source.logical_path.as_str())
        .chain(source.changed_paths.iter().map(String::as_str))
        .any(is_dependency_manifest_path)
}

fn is_dependency_manifest_path(path: &str) -> bool {
    let path = path.trim().to_ascii_lowercase().replace('\\', "/");
    let name = path.rsplit('/').next().unwrap_or(path.as_str());
    name == "pyproject.toml"
        || name == "setup.py"
        || name == "setup.cfg"
        || (name.starts_with("requirements") && name.ends_with(".txt"))
}

pub(super) fn validate_non_empty_single_line(
    field: &'static str,
    value: &str,
) -> InstrumentResult<()> {
    if value.trim().is_empty() {
        return invalid(
            field,
            format!("{field} is empty or whitespace-only"),
            "persist non-empty UTF-8 provenance fields",
        );
    }
    if value.chars().any(|ch| ch == '\0' || ch.is_control()) {
        return invalid(
            field,
            format!("{field} contains a control character"),
            "persist single-line UTF-8 provenance fields",
        );
    }
    Ok(())
}
