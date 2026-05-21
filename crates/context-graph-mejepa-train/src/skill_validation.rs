use crate::error::{TrainerError, TrainerErrorCode};
use serde_json::json;
use std::collections::BTreeSet;

const MAX_ID_BYTES: usize = 512;

pub(crate) fn validate_id(
    source_file: &'static str,
    remediation: &'static str,
    field: &str,
    value: &str,
) -> Result<(), TrainerError> {
    if value.trim().is_empty() {
        return Err(invalid(
            source_file,
            remediation,
            field,
            "must be non-empty",
        ));
    }
    if value.len() > MAX_ID_BYTES || value.chars().any(char::is_control) {
        return Err(invalid(
            source_file,
            remediation,
            field,
            "must be single-line text up to 512 bytes",
        ));
    }
    Ok(())
}

pub(crate) fn validate_id_list(
    source_file: &'static str,
    remediation: &'static str,
    field: &str,
    values: &[String],
    max_items: usize,
) -> Result<(), TrainerError> {
    if values.len() > max_items {
        return Err(invalid(
            source_file,
            remediation,
            field,
            format!("too many ids: {}", values.len()),
        ));
    }
    let mut seen = BTreeSet::new();
    for value in values {
        validate_id(source_file, remediation, field, value)?;
        if !seen.insert(value) {
            return Err(invalid(
                source_file,
                remediation,
                field,
                format!("duplicate id {value}"),
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_live_id_list(
    source_file: &'static str,
    remediation: &'static str,
    field: &str,
    values: &[String],
    max_items: usize,
) -> Result<(), TrainerError> {
    validate_id_list(source_file, remediation, field, values, max_items)?;
    for value in values {
        if is_target_only_live_label(value) {
            return Err(invalid(
                source_file,
                remediation,
                field,
                format!("{value} is target-side supervision and cannot identify a live skill"),
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_project_relative_path(
    source_file: &'static str,
    remediation: &'static str,
    field: &str,
    value: &str,
) -> Result<(), TrainerError> {
    validate_id(source_file, remediation, field, value)?;
    if value.starts_with('/') || value.split('/').any(|part| part == "..") {
        return Err(invalid(
            source_file,
            remediation,
            field,
            "must be a project-relative path",
        ));
    }
    Ok(())
}

pub(crate) fn validate_path_list(
    source_file: &'static str,
    remediation: &'static str,
    field: &str,
    values: &[String],
    max_items: usize,
) -> Result<(), TrainerError> {
    if values.len() > max_items {
        return Err(invalid(
            source_file,
            remediation,
            field,
            format!("too many paths: {}", values.len()),
        ));
    }
    let mut seen = BTreeSet::new();
    for value in values {
        validate_project_relative_path(source_file, remediation, field, value)?;
        if !seen.insert(value) {
            return Err(invalid(
                source_file,
                remediation,
                field,
                format!("duplicate path {value}"),
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_finite_unit(
    source_file: &'static str,
    remediation: &'static str,
    field: &str,
    value: f64,
) -> Result<(), TrainerError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(invalid(
            source_file,
            remediation,
            field,
            "must be finite and within [0, 1]",
        ));
    }
    Ok(())
}

pub fn is_target_only_live_label(label: &str) -> bool {
    matches!(
        label.split_once(':').map(|(family, _)| family),
        Some(
            "oracle"
                | "oracle_exception"
                | "docker"
                | "test_phase"
                | "test_result"
                | "code_state_outcome"
                | "partition"
                | "leakage_policy"
                | "code_state_identity"
                | "mutation"
                | "mutation_micro"
                | "mutation_mechanism"
                | "ground_truth"
                | "truth"
                | "post_tool_use"
        )
    )
}

pub(crate) fn invalid(
    source_file: &'static str,
    remediation: &'static str,
    field: impl Into<String>,
    message: impl Into<String>,
) -> TrainerError {
    TrainerError::new(TrainerErrorCode::MejepaTrainConfigInvalid, message).with_context(json!({
        "field": field.into(),
        "file": source_file,
        "remediation": remediation
    }))
}
