use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use rocksdb::Options;

use super::*;

pub(super) fn run_command(
    tool: Q4PerfToolKind,
    phase: Q4PerfScanPhase,
    spec: &Q4PerfCommandSpec,
    timeout: Duration,
) -> InstrumentResult<Q4PerfToolOutput> {
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
            return Ok(Q4PerfToolOutput {
                tool,
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
                "python-q4-perf-labels-v1",
                err.to_string(),
                "inspect analyzer process state and caller timeout handling",
            )
        })? {
            Some(_) => {
                let output = child.wait_with_output().map_err(|err| {
                    InstrumentError::store(
                        "wait_with_output",
                        "python-q4-perf-labels-v1",
                        err.to_string(),
                        "inspect analyzer process output handling",
                    )
                })?;
                return tool_output_from_process(tool, phase, command, output, false);
            }
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let output = child.wait_with_output().map_err(|err| {
                    InstrumentError::store(
                        "wait_timeout_output",
                        "python-q4-perf-labels-v1",
                        err.to_string(),
                        "inspect analyzer timeout process cleanup",
                    )
                })?;
                return tool_output_from_process(tool, phase, command, output, true);
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    }
}

pub(super) fn emit_delta_labels(
    source: &Q4PerfSource,
    tool: Q4PerfToolKind,
    phase: Q4PerfScanPhase,
    pre: BTreeMap<String, PerfMeasurement>,
    post: BTreeMap<String, PerfMeasurement>,
    labels: &mut Vec<Q4PerfLabel>,
    quarantines: &mut Vec<Q4PerfQuarantine>,
) -> InstrumentResult<()> {
    if pre.is_empty() && post.is_empty() {
        return Ok(());
    }
    let mut keys = BTreeSet::new();
    keys.extend(pre.keys().cloned());
    keys.extend(post.keys().cloned());
    for metric in keys {
        let Some(before) = pre.get(&metric) else {
            quarantines.push(quarantine(
                source,
                tool,
                phase,
                "Q4_PERF_LABEL_MISSING_BASELINE",
                format!("metric {metric} missing from pre-patch output"),
            ));
            continue;
        };
        let Some(after) = post.get(&metric) else {
            quarantines.push(quarantine(
                source,
                tool,
                phase,
                "Q4_PERF_LABEL_MISSING_AFTER",
                format!("metric {metric} missing from post-patch output"),
            ));
            continue;
        };
        if unstable(before) || unstable(after) {
            quarantines.push(quarantine(
                source,
                tool,
                phase,
                "Q4_PERF_LABEL_UNSTABLE_BENCHMARK",
                format!("metric {metric} exceeded stddev/mean threshold {UNSTABLE_CV_THRESHOLD}"),
            ));
            continue;
        }
        if before.value_ns <= 0.0 {
            quarantines.push(quarantine(
                source,
                tool,
                phase,
                "Q4_PERF_LABEL_ZERO_BASELINE",
                format!("metric {metric} baseline was not positive"),
            ));
            continue;
        }
        if tool == Q4PerfToolKind::PytestBenchmark
            && (before.stddev_ns.is_none() || after.stddev_ns.is_none())
        {
            quarantines.push(quarantine(
                source,
                tool,
                phase,
                "Q4_PERF_LABEL_MISSING_STABILITY",
                format!("metric {metric} missing stddev needed for pytest-benchmark CV gate"),
            ));
            continue;
        }
        let delta_pct = ((after.value_ns - before.value_ns) / before.value_ns) * 100.0;
        labels.push(Q4PerfLabel {
            corpus_row_id: source.corpus_row_id.clone(),
            chunk_id: source.chunk_id.clone(),
            metric,
            category: if delta_pct < 0.0 {
                Q4PerfCategory::Improvement
            } else {
                after.category
            },
            baseline_ns: Some(before.value_ns),
            after_ns: Some(after.value_ns),
            delta_pct: Some(delta_pct),
            regression: delta_pct > REGRESSION_DELTA_THRESHOLD_PCT,
        });
    }
    Ok(())
}

pub(super) fn q4_perf_record_key(record: &PersistedQ4PerfSignal, _value: &[u8]) -> String {
    match &record.signal {
        Q4PerfSignalRecord::Label(label) => q4_perf_label_key(&label.corpus_row_id, &label.metric),
        Q4PerfSignalRecord::Quarantine(quarantine) => format!(
            "{}:quarantine:{}:{}:{}",
            quarantine.corpus_row_id,
            quarantine.tool.as_str(),
            quarantine.phase.as_str(),
            quarantine.reason_code,
        ),
    }
}

pub(super) fn q4_perf_label_key(corpus_row_id: &str, metric: &str) -> String {
    format!("{corpus_row_id}:label:{metric}")
}

pub(super) fn quarantine(
    source: &Q4PerfSource,
    tool: Q4PerfToolKind,
    phase: Q4PerfScanPhase,
    reason_code: &str,
    detail: impl Into<String>,
) -> Q4PerfQuarantine {
    Q4PerfQuarantine {
        corpus_row_id: source.corpus_row_id.clone(),
        chunk_id: source.chunk_id.clone(),
        tool,
        phase,
        reason_code: reason_code.to_string(),
        detail: detail.into(),
    }
}

pub(super) fn validate_source(source: &Q4PerfSource) -> InstrumentResult<()> {
    validate_path_component("corpus_row_id", &source.corpus_row_id)?;
    validate_non_empty_single_line("chunk_id", &source.chunk_id)?;
    validate_non_empty_single_line("logical_path", &source.logical_path)?;
    validate_non_empty_single_line("benchmark_selector", &source.benchmark_selector)
}

pub(super) fn validate_tool_output(
    output: &Q4PerfToolOutput,
    expected: Q4PerfToolKind,
    expected_phase: Q4PerfScanPhase,
) -> InstrumentResult<()> {
    if output.tool != expected {
        return invalid(
            "q4_perf.output.tool",
            format!(
                "tool output {:?} does not match expected {:?}",
                output.tool, expected
            ),
            "wire benchmark/profile outputs to their matching parser",
        );
    }
    if output.phase != expected_phase {
        return invalid(
            "q4_perf.output.phase",
            format!(
                "tool output phase {:?} does not match expected {:?}",
                output.phase, expected_phase
            ),
            "wire pre/post benchmark/profile outputs to their matching parser",
        );
    }
    if output.command.is_empty() {
        return invalid(
            "q4_perf.output.command",
            "tool output command provenance is empty",
            "persist analyzer command provenance with every raw output",
        );
    }
    if !output.runtime_exceeded && !output.toolchain_missing && output.status_code != Some(0) {
        return invalid(
            "q4_perf.output.status_code",
            format!(
                "analyzer exited with status {:?}; stdout must not become labels",
                output.status_code
            ),
            "only parse Q4 perf labels from successful analyzer commands",
        );
    }
    Ok(())
}

pub(super) fn validate_label(label: &Q4PerfLabel, source: &Q4PerfSource) -> InstrumentResult<()> {
    if label.corpus_row_id != source.corpus_row_id {
        return invalid(
            "q4_perf.label.corpus_row_id",
            "label corpus_row_id does not match source",
            "persist Q4 perf labels against the exact corpus row being measured",
        );
    }
    validate_non_empty_single_line("q4_perf.label.metric", &label.metric)?;
    match label.category {
        Q4PerfCategory::WallclockBudgetExceeded => {
            if label.baseline_ns.is_some() || label.after_ns.is_some() || label.delta_pct.is_some()
            {
                return invalid(
                    "q4_perf.label.budget_exceeded",
                    "budget-exceeded labels must not carry fake deltas",
                    "persist timeout as its own Q4 perf signal",
                );
            }
        }
        _ => {
            validate_optional_f64("q4_perf.label.baseline_ns", label.baseline_ns)?;
            validate_optional_f64("q4_perf.label.after_ns", label.after_ns)?;
            validate_optional_f64("q4_perf.label.delta_pct", label.delta_pct)?;
            if label.baseline_ns.is_none() || label.after_ns.is_none() || label.delta_pct.is_none()
            {
                return invalid(
                    "q4_perf.label.delta",
                    "non-budget perf labels require baseline/after/delta",
                    "do not persist partial perf deltas outside quarantine",
                );
            }
        }
    }
    Ok(())
}

pub(super) fn validate_quarantine(
    quarantine: &Q4PerfQuarantine,
    source: &Q4PerfSource,
) -> InstrumentResult<()> {
    if quarantine.corpus_row_id != source.corpus_row_id {
        return invalid(
            "q4_perf.quarantine.corpus_row_id",
            "quarantine corpus_row_id does not match source",
            "persist Q4 perf quarantines against the exact corpus row being measured",
        );
    }
    validate_non_empty_single_line("q4_perf.quarantine.reason_code", &quarantine.reason_code)?;
    validate_non_empty_single_line("q4_perf.quarantine.detail", &quarantine.detail)
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
    pre: &Q4PerfToolOutput,
    post: &Q4PerfToolOutput,
) -> InstrumentResult<()> {
    let value = serde_json::json!({
        "schema_version": Q4_PERF_SCHEMA_VERSION,
        "tool": pre.tool,
        "pre_patch": pre,
        "post_patch": post,
    });
    let bytes = serde_json::to_vec_pretty(&value).map_err(|err| {
        InstrumentError::store(
            "serialize_raw_output",
            "python-q4-perf-labels-v1",
            err.to_string(),
            "ensure raw Q4 perf output wrappers remain JSON serializable",
        )
    })?;
    fs::write(path, &bytes).map_err(|err| {
        InstrumentError::store(
            "write_raw_output",
            "python-q4-perf-labels-v1",
            err.to_string(),
            "ensure the aiwonder Q4 perf raw-output directory is writable",
        )
    })?;
    let readback = fs::read(path).map_err(|err| {
        InstrumentError::store(
            "read_raw_output",
            "python-q4-perf-labels-v1",
            err.to_string(),
            "read back raw analyzer output after writing it",
        )
    })?;
    if readback != bytes {
        return Err(InstrumentError::store(
            "read_after_write_raw_output",
            "python-q4-perf-labels-v1",
            format!(
                "{} readback bytes differ from written bytes",
                path.display()
            ),
            "do not advance Q4 perf checkpoints until raw output is durable",
        ));
    }
    Ok(())
}

fn unstable(measurement: &PerfMeasurement) -> bool {
    measurement
        .stddev_ns
        .map(|stddev| {
            measurement.value_ns > 0.0 && stddev / measurement.value_ns > UNSTABLE_CV_THRESHOLD
        })
        .unwrap_or(false)
}

fn tool_output_from_process(
    tool: Q4PerfToolKind,
    phase: Q4PerfScanPhase,
    command: Vec<String>,
    output: std::process::Output,
    runtime_exceeded: bool,
) -> InstrumentResult<Q4PerfToolOutput> {
    Ok(Q4PerfToolOutput {
        tool,
        phase,
        command,
        status_code: output.status.code(),
        stdout: String::from_utf8(output.stdout).map_err(|err| {
            InstrumentError::invalid(
                "q4_perf.stdout",
                format!("analyzer stdout was not UTF-8: {err}"),
                "configure Q4 perf analyzers to emit UTF-8 JSON summaries",
            )
        })?,
        stderr: String::from_utf8(output.stderr).map_err(|err| {
            InstrumentError::invalid(
                "q4_perf.stderr",
                format!("analyzer stderr was not UTF-8: {err}"),
                "configure Q4 perf analyzers to emit UTF-8 diagnostics",
            )
        })?,
        runtime_exceeded,
        toolchain_missing: false,
    })
}

fn validate_command_spec(spec: &Q4PerfCommandSpec) -> InstrumentResult<()> {
    validate_non_empty_single_line("q4_perf.command.program", &spec.program)?;
    if !spec.cwd.is_dir() {
        return invalid(
            "q4_perf.command.cwd",
            format!("{} is not a directory", spec.cwd.display()),
            "materialize the benchmark workspace before running Q4 perf analysis",
        );
    }
    Ok(())
}

fn validate_optional_f64(field: &'static str, value: Option<f64>) -> InstrumentResult<()> {
    if let Some(value) = value {
        if !value.is_finite() {
            return invalid(
                field,
                "value is not finite",
                "persist finite Q4 perf metrics",
            );
        }
    }
    Ok(())
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
