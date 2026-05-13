use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

const DEFAULT_QUERY: &str = "name,memory.total,memory.free,memory.used";
const MAX_ATTEMPTS: usize = 3;
const FIRST_BACKOFF_MS: u64 = 200;
const COMMAND_TIMEOUT_MS: u64 = 2_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GpuTelemetry {
    pub query: String,
    pub attempt_count: usize,
    pub wsl_detected: bool,
    pub fields: BTreeMap<String, GpuTelemetryField>,
    pub unavailable_fields: Vec<String>,
    pub telemetry_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GpuTelemetryField {
    pub raw: String,
    pub value: Option<f64>,
    pub unavailable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GpuTelemetryFault {
    pub code: &'static str,
    pub message: String,
    pub attempt_count: usize,
}

impl std::fmt::Display for GpuTelemetryFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} after {} attempt(s): {}",
            self.code, self.attempt_count, self.message
        )
    }
}

impl std::error::Error for GpuTelemetryFault {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpuError {
    EmptyQuery,
    Timeout,
    CommandFailed { stderr: String },
    InvalidUtf8 { message: String },
    MalformedOutput { message: String },
    InvalidNumericField { field: String, raw: String },
}

impl GpuError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::EmptyQuery => "MEJEPA_GPU_WSL_EMPTY_QUERY",
            Self::Timeout => "MEJEPA_GPU_WSL_TIMEOUT",
            Self::CommandFailed { .. } => "MEJEPA_GPU_WSL_COMMAND_FAILED",
            Self::InvalidUtf8 { .. } => "MEJEPA_GPU_WSL_INVALID_UTF8",
            Self::MalformedOutput { .. } => "MEJEPA_GPU_WSL_MALFORMED_OUTPUT",
            Self::InvalidNumericField { .. } => "MEJEPA_GPU_WSL_INVALID_NUMERIC_FIELD",
        }
    }
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyQuery => write!(f, "empty nvidia-smi query"),
            Self::Timeout => write!(f, "nvidia-smi command timed out"),
            Self::CommandFailed { stderr } => write!(f, "nvidia-smi command failed: {stderr}"),
            Self::InvalidUtf8 { message } => {
                write!(f, "nvidia-smi stdout was not UTF-8: {message}")
            }
            Self::MalformedOutput { message } => {
                write!(f, "malformed nvidia-smi output: {message}")
            }
            Self::InvalidNumericField { field, raw } => {
                write!(f, "nvidia-smi field {field} had non-numeric value {raw:?}")
            }
        }
    }
}

impl std::error::Error for GpuError {}

pub fn default_query() -> &'static str {
    DEFAULT_QUERY
}

pub async fn nvidia_smi_query(query: &str) -> Result<GpuTelemetry, GpuTelemetryFault> {
    nvidia_smi_query_with_command("nvidia-smi", query).await
}

pub async fn nvidia_smi_query_with_command(
    command: impl AsRef<Path>,
    query: &str,
) -> Result<GpuTelemetry, GpuTelemetryFault> {
    let command = command.as_ref().to_path_buf();
    nvidia_smi_query_with_runner(query, || {
        let command = command.clone();
        async move { nvidia_smi_once(&command, query).await }
    })
    .await
}

pub async fn nvidia_smi_query_with_runner<F, Fut>(
    query: &str,
    mut runner: F,
) -> Result<GpuTelemetry, GpuTelemetryFault>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<Vec<u8>, GpuError>>,
{
    let mut last_error = None;
    for attempt in 1..=MAX_ATTEMPTS {
        match runner().await {
            Ok(stdout) => {
                let mut telemetry = parse_nvidia_smi_output(&stdout, query)
                    .map_err(|err| fault_from_error(err, attempt))?;
                telemetry.attempt_count = attempt;
                return Ok(telemetry);
            }
            Err(GpuError::Timeout) if attempt < MAX_ATTEMPTS => {
                last_error = Some(GpuError::Timeout);
                tokio::time::sleep(Duration::from_millis(
                    FIRST_BACKOFF_MS * (1 << (attempt - 1)),
                ))
                .await;
            }
            Err(err) => return Err(fault_from_error(err, attempt)),
        }
    }
    Err(fault_from_error(
        last_error.unwrap_or(GpuError::Timeout),
        MAX_ATTEMPTS,
    ))
}

async fn nvidia_smi_once(command: &Path, query: &str) -> Result<Vec<u8>, GpuError> {
    validate_query(query)?;
    let query_arg = format!("--query-gpu={query}");
    let mut process = Command::new(command);
    process
        .kill_on_drop(true)
        .args([query_arg.as_str(), "--format=csv,noheader,nounits"]);
    let output = tokio::time::timeout(Duration::from_millis(COMMAND_TIMEOUT_MS), process.output())
        .await
        .map_err(|_| GpuError::Timeout)?
        .map_err(|err| GpuError::CommandFailed {
            stderr: format!("failed to execute {}: {err}", command.display()),
        })?;
    if !output.status.success() {
        return Err(GpuError::CommandFailed {
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(output.stdout)
}

pub fn parse_nvidia_smi_output(stdout: &[u8], query: &str) -> Result<GpuTelemetry, GpuError> {
    let columns = parse_query_columns(query)?;
    let text = std::str::from_utf8(stdout).map_err(|err| GpuError::InvalidUtf8 {
        message: err.to_string(),
    })?;
    let first_line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| GpuError::MalformedOutput {
            message: "nvidia-smi returned no rows".to_string(),
        })?;
    let raw_values = first_line.split(',').map(str::trim).collect::<Vec<_>>();
    if raw_values.len() != columns.len() {
        return Err(GpuError::MalformedOutput {
            message: format!(
                "query expected {} columns but nvidia-smi returned {}",
                columns.len(),
                raw_values.len()
            ),
        });
    }

    let mut fields = BTreeMap::new();
    let mut unavailable_fields = Vec::new();
    for (field, raw) in columns.iter().zip(raw_values) {
        let normalized = normalize_sentinel(raw);
        let unavailable = matches!(
            normalized.as_str(),
            "" | "n/a" | "na" | "not supported" | "unsupported"
        );
        if unavailable {
            unavailable_fields.push(field.clone());
            fields.insert(
                field.clone(),
                GpuTelemetryField {
                    raw: raw.to_string(),
                    value: None,
                    unavailable: true,
                },
            );
            continue;
        }
        let value = if field_requires_number(field) {
            Some(
                raw.parse::<f64>()
                    .map_err(|_| GpuError::InvalidNumericField {
                        field: field.clone(),
                        raw: raw.to_string(),
                    })?,
            )
        } else {
            None
        };
        fields.insert(
            field.clone(),
            GpuTelemetryField {
                raw: raw.to_string(),
                value,
                unavailable: false,
            },
        );
    }

    Ok(GpuTelemetry {
        query: columns.join(","),
        attempt_count: 1,
        wsl_detected: is_wsl(),
        fields,
        unavailable_fields,
        telemetry_source: "nvidia_smi_wsl_diagnostic_non_authoritative".to_string(),
    })
}

pub fn fault_from_error(err: GpuError, attempt_count: usize) -> GpuTelemetryFault {
    GpuTelemetryFault {
        code: err.code(),
        message: err.to_string(),
        attempt_count,
    }
}

fn parse_query_columns(query: &str) -> Result<Vec<String>, GpuError> {
    validate_query(query)?;
    Ok(query
        .split(',')
        .map(str::trim)
        .map(str::to_string)
        .collect())
}

fn validate_query(query: &str) -> Result<(), GpuError> {
    if query.trim().is_empty() {
        return Err(GpuError::EmptyQuery);
    }
    if query
        .split(',')
        .map(str::trim)
        .any(|field| field.is_empty() || field.chars().any(char::is_control))
    {
        return Err(GpuError::MalformedOutput {
            message: "query contains an empty or control-character field".to_string(),
        });
    }
    Ok(())
}

fn normalize_sentinel(raw: &str) -> String {
    raw.trim()
        .trim_matches(['[', ']'])
        .trim()
        .to_ascii_lowercase()
        .replace('-', " ")
}

fn field_requires_number(field: &str) -> bool {
    field.starts_with("memory.")
        || field.starts_with("utilization.")
        || field.starts_with("temperature.")
        || field.ends_with(".count")
        || field.ends_with(".limit")
}

fn is_wsl() -> bool {
    std::fs::read_to_string("/proc/version")
        .map(|value| {
            let lower = value.to_ascii_lowercase();
            lower.contains("microsoft") || lower.contains("wsl")
        })
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "gpu_wsl_tests.rs"]
mod tests;
