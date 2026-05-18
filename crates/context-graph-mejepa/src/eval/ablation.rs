use super::error::{EvalError, EvalErrorCode};
use super::types::validate_optional_correlation;
use context_graph_mejepa_cf::CF_MEJEPA_ABLATION_REPORTS;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const NEGATIVE_ACTION_ABLATION_SCHEMA_VERSION: u32 = 1;
pub const NEGATIVE_ACTION_ABLATION_GLOBAL_DROP_THRESHOLD_PCT: f32 = 5.0;
pub const NEGATIVE_ACTION_ABLATION_CELL_DROP_THRESHOLD_PCT: f32 = 3.0;
pub const NEGATIVE_ACTION_ABLATION_BLOCKER: &str =
    "MEJEPA_EVAL_NEGATIVE_ACTION_ABLATION_DEGENERATE";
pub const NEGATIVE_ACTION_ABLATION_WARNING: &str = "MEJEPA_EVAL_NEGATIVE_ACTION_ABLATION_WARNING";
pub const ABLATION_INCOMPLETE: &str = "ABLATION_INCOMPLETE";
pub const ABLATION_NUMERICAL_INSTABILITY: &str = "ABLATION_NUMERICAL_INSTABILITY";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AblationVerdict {
    Healthy,
    Degenerate,
    Incomplete,
    NumericalInstability,
}

impl AblationVerdict {
    pub fn blocks_ship_gate(self) -> bool {
        matches!(self, Self::Degenerate | Self::NumericalInstability)
    }

    pub fn is_incomplete(self) -> bool {
        matches!(self, Self::Incomplete)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AblationCellDrop {
    pub cell_id: String,
    pub action_enabled_correlation: Option<f32>,
    pub action_disabled_correlation: Option<f32>,
    pub score_drop_pct: Option<f32>,
    pub degenerate: bool,
}

impl AblationCellDrop {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.cell_id.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "ablation cell_id must be non-empty",
            ));
        }
        validate_optional_correlation(
            "ablation_cell.action_enabled_correlation",
            self.action_enabled_correlation,
        )?;
        validate_optional_correlation(
            "ablation_cell.action_disabled_correlation",
            self.action_disabled_correlation,
        )?;
        validate_optional_drop_pct("ablation_cell.score_drop_pct", self.score_drop_pct)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AblationReport {
    pub schema_version: u32,
    pub report_id: String,
    pub report_date: String,
    pub generated_at_unix_ms: i64,
    pub action_enabled_correlation: Option<f32>,
    pub action_disabled_correlation: Option<f32>,
    pub score_drop_pct: Option<f32>,
    pub global_drop_threshold_pct: f32,
    pub cell_drop_threshold_pct: f32,
    pub per_cell_drop: BTreeMap<String, AblationCellDrop>,
    pub verdict: AblationVerdict,
    pub status_code: Option<String>,
    pub warning: Option<String>,
    pub source_of_truth_cf: String,
}

impl AblationReport {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.schema_version != NEGATIVE_ACTION_ABLATION_SCHEMA_VERSION {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "ablation schema_version {} != {}",
                    self.schema_version, NEGATIVE_ACTION_ABLATION_SCHEMA_VERSION
                ),
            ));
        }
        if self.report_id.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "ablation report_id must be non-empty",
            ));
        }
        if self.report_date.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "ablation report_date must be non-empty",
            ));
        }
        if self.generated_at_unix_ms < 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "ablation generated_at_unix_ms must be non-negative",
            ));
        }
        if self.source_of_truth_cf != CF_MEJEPA_ABLATION_REPORTS {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "ablation source_of_truth_cf {} != {CF_MEJEPA_ABLATION_REPORTS}",
                    self.source_of_truth_cf
                ),
            ));
        }
        validate_optional_correlation(
            "ablation.action_enabled_correlation",
            self.action_enabled_correlation,
        )?;
        validate_optional_correlation(
            "ablation.action_disabled_correlation",
            self.action_disabled_correlation,
        )?;
        validate_optional_drop_pct("ablation.score_drop_pct", self.score_drop_pct)?;
        validate_threshold(
            "ablation.global_drop_threshold_pct",
            self.global_drop_threshold_pct,
        )?;
        validate_threshold(
            "ablation.cell_drop_threshold_pct",
            self.cell_drop_threshold_pct,
        )?;
        for (cell, drop) in &self.per_cell_drop {
            if cell != &drop.cell_id {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!("ablation per_cell_drop key {cell} does not match payload cell_id"),
                ));
            }
            drop.validate()?;
        }
        match self.verdict {
            AblationVerdict::Healthy | AblationVerdict::Degenerate => {
                if self.action_enabled_correlation.is_none()
                    || self.action_disabled_correlation.is_none()
                    || self.score_drop_pct.is_none()
                {
                    return Err(EvalError::new(
                        EvalErrorCode::InvalidInput,
                        "complete ablation report requires enabled, disabled, and drop scores",
                    ));
                }
            }
            AblationVerdict::Incomplete => {
                if self.status_code.as_deref() != Some(ABLATION_INCOMPLETE) {
                    return Err(EvalError::new(
                        EvalErrorCode::InvalidInput,
                        "incomplete ablation report must use ABLATION_INCOMPLETE",
                    ));
                }
                if self.warning.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(EvalError::new(
                        EvalErrorCode::InvalidInput,
                        "incomplete ablation report requires warning",
                    ));
                }
            }
            AblationVerdict::NumericalInstability => {
                if self.status_code.as_deref() != Some(ABLATION_NUMERICAL_INSTABILITY) {
                    return Err(EvalError::new(
                        EvalErrorCode::InvalidInput,
                        "numerical-instability ablation report must use ABLATION_NUMERICAL_INSTABILITY",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AblationRunInput {
    pub report_id: String,
    pub report_date: String,
    pub generated_at_unix_ms: i64,
    pub action_enabled_correlation: f32,
    pub action_disabled_correlation: f32,
    pub per_cell_enabled_correlation: BTreeMap<String, f32>,
    pub per_cell_disabled_correlation: BTreeMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NegativeActionAblationGateStatus {
    pub source_of_truth_cf: String,
    pub ready: bool,
    pub latest_report_id: Option<String>,
    pub latest_report_date: Option<String>,
    pub latest_verdict: Option<AblationVerdict>,
    pub effective_report_id: Option<String>,
    pub effective_report_date: Option<String>,
    pub effective_verdict: Option<AblationVerdict>,
    pub effective_score_drop_pct: Option<f32>,
    pub blocker: Option<String>,
    pub warning: Option<String>,
    pub incomplete_warning_count: usize,
}

pub fn build_negative_action_ablation_report(
    input: AblationRunInput,
) -> Result<AblationReport, EvalError> {
    validate_report_identity(
        &input.report_id,
        &input.report_date,
        input.generated_at_unix_ms,
    )?;
    if input
        .per_cell_enabled_correlation
        .keys()
        .collect::<Vec<_>>()
        != input
            .per_cell_disabled_correlation
            .keys()
            .collect::<Vec<_>>()
    {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "ablation enabled/disabled per-cell key sets differ",
        ));
    }

    let global_drop = match score_drop_pct(
        input.action_enabled_correlation,
        input.action_disabled_correlation,
    ) {
        Some(drop) => drop,
        None => return numerical_instability_report(input, "global score was non-finite"),
    };

    let mut per_cell_drop = BTreeMap::new();
    for (cell, enabled) in &input.per_cell_enabled_correlation {
        let disabled = *input
            .per_cell_disabled_correlation
            .get(cell)
            .ok_or_else(|| {
                EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!("ablation disabled cell missing for {cell}"),
                )
            })?;
        let Some(drop) = score_drop_pct(*enabled, disabled) else {
            return numerical_instability_report(
                input.clone(),
                format!("cell {cell} score was non-finite"),
            );
        };
        per_cell_drop.insert(
            cell.clone(),
            AblationCellDrop {
                cell_id: cell.clone(),
                action_enabled_correlation: Some(*enabled),
                action_disabled_correlation: Some(disabled),
                score_drop_pct: Some(drop),
                degenerate: drop < NEGATIVE_ACTION_ABLATION_CELL_DROP_THRESHOLD_PCT,
            },
        );
    }

    let verdict = if global_drop >= NEGATIVE_ACTION_ABLATION_GLOBAL_DROP_THRESHOLD_PCT {
        AblationVerdict::Healthy
    } else {
        AblationVerdict::Degenerate
    };
    let status_code = if verdict.blocks_ship_gate() {
        Some(NEGATIVE_ACTION_ABLATION_BLOCKER.to_string())
    } else {
        None
    };
    let report = AblationReport {
        schema_version: NEGATIVE_ACTION_ABLATION_SCHEMA_VERSION,
        report_id: input.report_id,
        report_date: input.report_date,
        generated_at_unix_ms: input.generated_at_unix_ms,
        action_enabled_correlation: Some(input.action_enabled_correlation),
        action_disabled_correlation: Some(input.action_disabled_correlation),
        score_drop_pct: Some(global_drop),
        global_drop_threshold_pct: NEGATIVE_ACTION_ABLATION_GLOBAL_DROP_THRESHOLD_PCT,
        cell_drop_threshold_pct: NEGATIVE_ACTION_ABLATION_CELL_DROP_THRESHOLD_PCT,
        per_cell_drop,
        verdict,
        status_code,
        warning: None,
        source_of_truth_cf: CF_MEJEPA_ABLATION_REPORTS.to_string(),
    };
    report.validate()?;
    Ok(report)
}

pub fn incomplete_negative_action_ablation_report(
    report_id: impl Into<String>,
    report_date: impl Into<String>,
    generated_at_unix_ms: i64,
    warning: impl Into<String>,
) -> Result<AblationReport, EvalError> {
    let report_id = report_id.into();
    let report_date = report_date.into();
    let warning = warning.into();
    validate_report_identity(&report_id, &report_date, generated_at_unix_ms)?;
    if warning.trim().is_empty() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "incomplete ablation warning must be non-empty",
        ));
    }
    let report = AblationReport {
        schema_version: NEGATIVE_ACTION_ABLATION_SCHEMA_VERSION,
        report_id,
        report_date,
        generated_at_unix_ms,
        action_enabled_correlation: None,
        action_disabled_correlation: None,
        score_drop_pct: None,
        global_drop_threshold_pct: NEGATIVE_ACTION_ABLATION_GLOBAL_DROP_THRESHOLD_PCT,
        cell_drop_threshold_pct: NEGATIVE_ACTION_ABLATION_CELL_DROP_THRESHOLD_PCT,
        per_cell_drop: BTreeMap::new(),
        verdict: AblationVerdict::Incomplete,
        status_code: Some(ABLATION_INCOMPLETE.to_string()),
        warning: Some(format!("{ABLATION_INCOMPLETE}: {warning}")),
        source_of_truth_cf: CF_MEJEPA_ABLATION_REPORTS.to_string(),
    };
    report.validate()?;
    Ok(report)
}

pub fn negative_action_ablation_gate_status(
    reports: &[AblationReport],
) -> Result<NegativeActionAblationGateStatus, EvalError> {
    for report in reports {
        report.validate()?;
    }
    let latest = reports.last();
    let effective = reports
        .iter()
        .rev()
        .find(|report| !report.verdict.is_incomplete());
    let incomplete_warning_count = reports
        .iter()
        .filter(|report| report.verdict.is_incomplete())
        .count();
    let warning = match latest {
        Some(report) if report.verdict.is_incomplete() => report.warning.clone(),
        None => Some(format!(
            "{NEGATIVE_ACTION_ABLATION_WARNING}: no rows in {CF_MEJEPA_ABLATION_REPORTS}"
        )),
        _ => None,
    };
    let blocker = effective.and_then(|report| {
        if report.verdict.blocks_ship_gate() {
            Some(format!(
                "{NEGATIVE_ACTION_ABLATION_BLOCKER}: report_id={} verdict={:?} score_drop_pct={:?} threshold_pct={:.6}",
                report.report_id,
                report.verdict,
                report.score_drop_pct,
                report.global_drop_threshold_pct
            ))
        } else {
            None
        }
    });
    Ok(NegativeActionAblationGateStatus {
        source_of_truth_cf: CF_MEJEPA_ABLATION_REPORTS.to_string(),
        ready: blocker.is_none(),
        latest_report_id: latest.map(|report| report.report_id.clone()),
        latest_report_date: latest.map(|report| report.report_date.clone()),
        latest_verdict: latest.map(|report| report.verdict),
        effective_report_id: effective.map(|report| report.report_id.clone()),
        effective_report_date: effective.map(|report| report.report_date.clone()),
        effective_verdict: effective.map(|report| report.verdict),
        effective_score_drop_pct: effective.and_then(|report| report.score_drop_pct),
        blocker,
        warning,
        incomplete_warning_count,
    })
}

pub fn ablation_report_key(report: &AblationReport) -> Vec<u8> {
    format!(
        "{}::{:020}::{}",
        report.report_date, report.generated_at_unix_ms, report.report_id
    )
    .into_bytes()
}

pub fn render_ablation_weekly_markdown(report: &AblationReport) -> Result<String, EvalError> {
    report.validate()?;
    let rows = if report.per_cell_drop.is_empty() {
        "| cell | enabled | disabled | drop_pct | degenerate |\n| --- | ---: | ---: | ---: | --- |\n| none | n/a | n/a | n/a | false |".to_string()
    } else {
        let mut rows =
            "| cell | enabled | disabled | drop_pct | degenerate |\n| --- | ---: | ---: | ---: | --- |"
                .to_string();
        for cell in report.per_cell_drop.values() {
            rows.push_str(&format!(
                "\n| {} | {} | {} | {} | {} |",
                cell.cell_id,
                render_optional_f32(cell.action_enabled_correlation),
                render_optional_f32(cell.action_disabled_correlation),
                render_optional_f32(cell.score_drop_pct),
                cell.degenerate
            ));
        }
        rows
    };
    Ok(format!(
        "# ME-JEPA Negative-Action Ablation Report\n\n\
         - report_id: {}\n\
         - report_date: {}\n\
         - generated_at_unix_ms: {}\n\
         - verdict: {:?}\n\
         - status_code: {}\n\
         - warning: {}\n\
         - action_enabled_correlation: {}\n\
         - action_disabled_correlation: {}\n\
         - score_drop_pct: {}\n\
         - global_drop_threshold_pct: {:.6}\n\
         - cell_drop_threshold_pct: {:.6}\n\
         - source_of_truth: {}\n\n\
         ## Per-Cell Drop\n\n{}\n",
        report.report_id,
        report.report_date,
        report.generated_at_unix_ms,
        report.verdict,
        report.status_code.as_deref().unwrap_or("none"),
        report.warning.as_deref().unwrap_or("none"),
        render_optional_f32(report.action_enabled_correlation),
        render_optional_f32(report.action_disabled_correlation),
        render_optional_f32(report.score_drop_pct),
        report.global_drop_threshold_pct,
        report.cell_drop_threshold_pct,
        report.source_of_truth_cf,
        rows
    ))
}

pub fn write_ablation_weekly_markdown(
    weekly_root: impl AsRef<Path>,
    report: &AblationReport,
) -> Result<PathBuf, EvalError> {
    let path = weekly_root
        .as_ref()
        .join(&report.report_date)
        .join("ablation_report.md");
    write_markdown_0600(&path, &render_ablation_weekly_markdown(report)?)?;
    Ok(path)
}

fn numerical_instability_report(
    input: AblationRunInput,
    warning: impl Into<String>,
) -> Result<AblationReport, EvalError> {
    let report = AblationReport {
        schema_version: NEGATIVE_ACTION_ABLATION_SCHEMA_VERSION,
        report_id: input.report_id,
        report_date: input.report_date,
        generated_at_unix_ms: input.generated_at_unix_ms,
        action_enabled_correlation: finite_option(input.action_enabled_correlation),
        action_disabled_correlation: finite_option(input.action_disabled_correlation),
        score_drop_pct: None,
        global_drop_threshold_pct: NEGATIVE_ACTION_ABLATION_GLOBAL_DROP_THRESHOLD_PCT,
        cell_drop_threshold_pct: NEGATIVE_ACTION_ABLATION_CELL_DROP_THRESHOLD_PCT,
        per_cell_drop: BTreeMap::new(),
        verdict: AblationVerdict::NumericalInstability,
        status_code: Some(ABLATION_NUMERICAL_INSTABILITY.to_string()),
        warning: Some(format!(
            "{}: {}",
            ABLATION_NUMERICAL_INSTABILITY,
            warning.into()
        )),
        source_of_truth_cf: CF_MEJEPA_ABLATION_REPORTS.to_string(),
    };
    report.validate()?;
    Ok(report)
}

fn score_drop_pct(enabled: f32, disabled: f32) -> Option<f32> {
    if !enabled.is_finite() || !disabled.is_finite() || enabled.abs() < 1e-6 {
        return None;
    }
    Some(((enabled - disabled) / enabled.abs()) * 100.0)
}

fn validate_optional_drop_pct(name: &str, value: Option<f32>) -> Result<(), EvalError> {
    if let Some(value) = value {
        if !value.is_finite() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("{name} must be finite; got {value}"),
            ));
        }
    }
    Ok(())
}

fn validate_threshold(name: &str, value: f32) -> Result<(), EvalError> {
    if !value.is_finite() || value < 0.0 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{name} must be finite and non-negative; got {value}"),
        ));
    }
    Ok(())
}

fn validate_report_identity(
    report_id: &str,
    report_date: &str,
    generated_at_unix_ms: i64,
) -> Result<(), EvalError> {
    if report_id.trim().is_empty() || report_date.trim().is_empty() {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "ablation report_id and report_date must be non-empty",
        ));
    }
    if generated_at_unix_ms < 0 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "ablation generated_at_unix_ms must be non-negative",
        ));
    }
    Ok(())
}

fn finite_option(value: f32) -> Option<f32> {
    value.is_finite().then_some(value)
}

fn render_optional_f32(value: Option<f32>) -> String {
    value
        .map(|value| format!("{value:.6}"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn write_markdown_0600(path: &Path, contents: &str) -> Result<(), EvalError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)?
    };
    #[cfg(not(unix))]
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    let readback = fs::read_to_string(path)?;
    if readback != contents {
        return Err(EvalError::new(
            EvalErrorCode::ReadbackMismatch,
            format!("{} markdown readback differs", path.display()),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)?.permissions().mode() & 0o777;
        if mode != 0o600 {
            return Err(EvalError::new(
                EvalErrorCode::ReadbackMismatch,
                format!("{} mode {mode:o} != 600", path.display()),
            ));
        }
    }
    Ok(())
}
