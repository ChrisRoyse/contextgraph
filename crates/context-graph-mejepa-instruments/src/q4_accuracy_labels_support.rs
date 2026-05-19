use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use rocksdb::Options;
use sha2::{Digest, Sha256};

use super::*;

pub(super) fn emit_delta_labels(
    source: &Q4AccuracySource,
    pre: BTreeMap<String, AccuracyMeasurement>,
    post: BTreeMap<String, AccuracyMeasurement>,
    labels: &mut Vec<Q4AccuracyLabel>,
    quarantines: &mut Vec<Q4AccuracyQuarantine>,
) -> InstrumentResult<()> {
    if pre.is_empty() && post.is_empty() {
        return Ok(());
    }
    let mut keys = BTreeSet::new();
    keys.extend(pre.keys().cloned());
    keys.extend(post.keys().cloned());
    for measurement_key in keys {
        let Some(before) = pre.get(&measurement_key) else {
            quarantines.push(quarantine(
                source,
                Q4AccuracyScanPhase::PrePatch,
                "Q4_ACCURACY_LABEL_MISSING_BASELINE",
                format!("metric key {measurement_key} missing from pre-patch output"),
            ));
            continue;
        };
        let Some(after) = post.get(&measurement_key) else {
            quarantines.push(quarantine(
                source,
                Q4AccuracyScanPhase::PostPatch,
                "Q4_ACCURACY_LABEL_MISSING_AFTER",
                format!("metric key {measurement_key} missing from post-patch output"),
            ));
            continue;
        };
        if unstable(before) || unstable(after) {
            quarantines.push(quarantine(
                source,
                Q4AccuracyScanPhase::PostPatch,
                "Q4_ACCURACY_LABEL_UNSTABLE_SEED",
                format!("metric key {measurement_key} changed across fixed-seed samples"),
            ));
            continue;
        }
        if before.value == 0.0 {
            quarantines.push(quarantine(
                source,
                Q4AccuracyScanPhase::PostPatch,
                "Q4_ACCURACY_LABEL_ZERO_BASELINE",
                format!("metric key {measurement_key} baseline was zero"),
            ));
            continue;
        }
        let delta_pct = ((after.value - before.value) / before.value.abs()) * 100.0;
        let regression = if before.metric_kind.lower_is_better() {
            delta_pct > REGRESSION_DELTA_THRESHOLD_PCT
        } else {
            delta_pct < -REGRESSION_DELTA_THRESHOLD_PCT
        };
        let kind = if regression {
            Q4AccuracyLabelKind::Regression
        } else if meaningful_fix(before.metric_kind, delta_pct) {
            Q4AccuracyLabelKind::Fix
        } else {
            Q4AccuracyLabelKind::Stable
        };
        labels.push(Q4AccuracyLabel {
            corpus_row_id: source.corpus_row_id.clone(),
            chunk_id: source.chunk_id.clone(),
            metric_name: before.metric_name.clone(),
            metric_kind: before.metric_kind,
            baseline_value: before.value,
            after_value: after.value,
            delta_pct,
            regression,
            kind,
            source_test: after
                .source_test
                .clone()
                .or_else(|| before.source_test.clone())
                .unwrap_or_else(|| source.source_test.clone()),
        });
    }
    Ok(())
}

pub(super) fn q4_accuracy_record_key(record: &PersistedQ4AccuracySignal, _value: &[u8]) -> String {
    match &record.signal {
        Q4AccuracySignalRecord::Label(label) => q4_accuracy_label_key_with_source(
            &label.corpus_row_id,
            &label.metric_name,
            &label.source_test,
        ),
        Q4AccuracySignalRecord::Quarantine(quarantine) => format!(
            "{}:quarantine:{}:{}:{}",
            quarantine.corpus_row_id,
            quarantine.phase.as_str(),
            quarantine.reason_code,
            stable_hash(&quarantine.detail),
        ),
    }
}

pub(super) fn q4_accuracy_label_key(corpus_row_id: &str, metric_name: &str) -> String {
    format!("{corpus_row_id}:label:{metric_name}")
}

pub(super) fn q4_accuracy_label_key_with_source(
    corpus_row_id: &str,
    metric_name: &str,
    source_test: &str,
) -> String {
    format!(
        "{}:{}",
        q4_accuracy_label_key(corpus_row_id, metric_name),
        stable_hash(source_test)
    )
}

pub(super) fn quarantine(
    source: &Q4AccuracySource,
    phase: Q4AccuracyScanPhase,
    reason_code: &str,
    detail: impl Into<String>,
) -> Q4AccuracyQuarantine {
    Q4AccuracyQuarantine {
        corpus_row_id: source.corpus_row_id.clone(),
        chunk_id: source.chunk_id.clone(),
        phase,
        reason_code: reason_code.to_string(),
        detail: detail.into(),
    }
}

pub(super) fn validate_source(source: &Q4AccuracySource) -> InstrumentResult<()> {
    validate_path_component("corpus_row_id", &source.corpus_row_id)?;
    validate_non_empty_single_line("chunk_id", &source.chunk_id)?;
    validate_non_empty_single_line("logical_path", &source.logical_path)?;
    validate_non_empty_single_line("source_test", &source.source_test)
}

pub(super) fn validate_tool_output(
    output: &Q4AccuracyToolOutput,
    expected_phase: Q4AccuracyScanPhase,
) -> InstrumentResult<()> {
    if output.phase != expected_phase {
        return invalid(
            "q4_accuracy.output.phase",
            format!(
                "tool output phase {:?} does not match expected {:?}",
                output.phase, expected_phase
            ),
            "wire pre/post metric outputs to their matching parser",
        );
    }
    if output.command.is_empty() {
        return invalid(
            "q4_accuracy.output.command",
            "tool output command provenance is empty",
            "persist analyzer command provenance with every raw output",
        );
    }
    if !output.runtime_exceeded && !output.toolchain_missing && output.status_code != Some(0) {
        return invalid(
            "q4_accuracy.output.status_code",
            format!(
                "analyzer exited with status {:?}; stdout must not become labels",
                output.status_code
            ),
            "only parse Q4 accuracy labels from successful analyzer commands",
        );
    }
    Ok(())
}

pub(super) fn validate_label(
    label: &Q4AccuracyLabel,
    source: &Q4AccuracySource,
) -> InstrumentResult<()> {
    if label.corpus_row_id != source.corpus_row_id {
        return invalid(
            "q4_accuracy.label.corpus_row_id",
            "label corpus_row_id does not match source",
            "persist Q4 accuracy labels against the exact corpus row being measured",
        );
    }
    validate_non_empty_single_line("q4_accuracy.label.metric_name", &label.metric_name)?;
    validate_non_empty_single_line("q4_accuracy.label.source_test", &label.source_test)?;
    validate_finite("q4_accuracy.label.baseline_value", label.baseline_value)?;
    validate_finite("q4_accuracy.label.after_value", label.after_value)?;
    validate_finite("q4_accuracy.label.delta_pct", label.delta_pct)
}

pub(super) fn validate_quarantine(
    quarantine: &Q4AccuracyQuarantine,
    source: &Q4AccuracySource,
) -> InstrumentResult<()> {
    if quarantine.corpus_row_id != source.corpus_row_id {
        return invalid(
            "q4_accuracy.quarantine.corpus_row_id",
            "quarantine corpus_row_id does not match source",
            "persist Q4 accuracy quarantines against the exact corpus row being measured",
        );
    }
    validate_non_empty_single_line(
        "q4_accuracy.quarantine.reason_code",
        &quarantine.reason_code,
    )?;
    validate_non_empty_single_line("q4_accuracy.quarantine.detail", &quarantine.detail)
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
    pre: &Q4AccuracyToolOutput,
    post: &Q4AccuracyToolOutput,
) -> InstrumentResult<()> {
    let value = serde_json::json!({
        "schema_version": Q4_ACCURACY_SCHEMA_VERSION,
        "pre_patch": pre,
        "post_patch": post,
    });
    let bytes = serde_json::to_vec_pretty(&value).map_err(|err| {
        InstrumentError::store(
            "serialize_raw_output",
            "python-q4-accuracy-labels-v1",
            err.to_string(),
            "ensure raw Q4 accuracy output wrappers remain JSON serializable",
        )
    })?;
    fs::write(path, &bytes).map_err(|err| {
        InstrumentError::store(
            "write_raw_output",
            "python-q4-accuracy-labels-v1",
            err.to_string(),
            "ensure the aiwonder Q4 accuracy raw-output directory is writable",
        )
    })?;
    let readback = fs::read(path).map_err(|err| {
        InstrumentError::store(
            "read_raw_output",
            "python-q4-accuracy-labels-v1",
            err.to_string(),
            "read back raw analyzer output after writing it",
        )
    })?;
    if readback != bytes {
        return Err(InstrumentError::store(
            "read_after_write_raw_output",
            "python-q4-accuracy-labels-v1",
            format!(
                "{} readback bytes differ from written bytes",
                path.display()
            ),
            "do not advance Q4 accuracy checkpoints until raw output is durable",
        ));
    }
    Ok(())
}

fn unstable(measurement: &AccuracyMeasurement) -> bool {
    measurement
        .stddev
        .map(|stddev| stddev.abs() > UNSTABLE_VALUE_THRESHOLD)
        .unwrap_or(false)
}

fn meaningful_fix(metric: Q4AccuracyMetricKind, delta_pct: f64) -> bool {
    if metric.lower_is_better() {
        delta_pct < -REGRESSION_DELTA_THRESHOLD_PCT
    } else {
        delta_pct > REGRESSION_DELTA_THRESHOLD_PCT
    }
}

pub(super) fn tool_output_from_process(
    phase: Q4AccuracyScanPhase,
    command: Vec<String>,
    output: std::process::Output,
    runtime_exceeded: bool,
) -> InstrumentResult<Q4AccuracyToolOutput> {
    Ok(Q4AccuracyToolOutput {
        phase,
        command,
        status_code: output.status.code(),
        stdout: String::from_utf8(output.stdout).map_err(|err| {
            InstrumentError::invalid(
                "q4_accuracy.stdout",
                format!("analyzer stdout was not UTF-8: {err}"),
                "configure Q4 accuracy analyzers to emit UTF-8 JSON summaries",
            )
        })?,
        stderr: String::from_utf8(output.stderr).map_err(|err| {
            InstrumentError::invalid(
                "q4_accuracy.stderr",
                format!("analyzer stderr was not UTF-8: {err}"),
                "configure Q4 accuracy analyzers to emit UTF-8 diagnostics",
            )
        })?,
        runtime_exceeded,
        toolchain_missing: false,
    })
}

pub(super) fn validate_command_spec(spec: &Q4AccuracyCommandSpec) -> InstrumentResult<()> {
    validate_non_empty_single_line("q4_accuracy.command.program", &spec.program)?;
    if !spec.cwd.is_dir() {
        return invalid(
            "q4_accuracy.command.cwd",
            format!("{} is not a directory", spec.cwd.display()),
            "materialize the metric workspace before running Q4 accuracy analysis",
        );
    }
    Ok(())
}

fn validate_finite(field: &'static str, value: f64) -> InstrumentResult<()> {
    if !value.is_finite() {
        return invalid(
            field,
            "value is not finite",
            "persist finite Q4 accuracy metrics",
        );
    }
    Ok(())
}

fn stable_hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
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
