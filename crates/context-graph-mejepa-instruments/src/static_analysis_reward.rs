// TASK-RWD-208 / TASK-PY-G-074: durable mypy + pyright + ruff reward labels.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use context_graph_mejepa_cf::CF_MEJEPA_STATIC_ANALYSIS_SIGNALS;
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::e_static_analysis::{Diagnostic, DiagnosticSeverity, StaticAnalysisInput};
use crate::features::fnv1a64;
use crate::{InstrumentError, InstrumentResult};

const STATIC_ANALYSIS_SCHEMA_VERSION: u32 = 1;
const MAX_SOURCE_BYTES: usize = 10_000_000;
const MAX_LABELS: usize = 100_000;
pub const DEFAULT_STATIC_ANALYSIS_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaticAnalysisDetector {
    Mypy,
    Pyright,
    Ruff,
}

impl StaticAnalysisDetector {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mypy => "mypy",
            Self::Pyright => "pyright",
            Self::Ruff => "ruff",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaticAnalysisSignalKind {
    TypeError,
    LintError,
    ConventionViolation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticAnalysisLineRange {
    pub start_line: u32,
    pub end_line: u32,
    pub start_column: u32,
    pub end_column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticAnalysisLabel {
    pub chunk_id: String,
    pub path: String,
    pub signal_kind: StaticAnalysisSignalKind,
    pub rule_id: String,
    pub severity: DiagnosticSeverity,
    pub line_range: StaticAnalysisLineRange,
    pub detector: StaticAnalysisDetector,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticAnalysisQuarantine {
    pub chunk_id: String,
    pub path: String,
    pub detector: StaticAnalysisDetector,
    pub reason_code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "record_kind", content = "record", rename_all = "snake_case")]
pub enum StaticAnalysisSignalRecord {
    Label(StaticAnalysisLabel),
    Quarantine(StaticAnalysisQuarantine),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedStaticAnalysisSignal {
    pub schema_version: u32,
    pub signal: StaticAnalysisSignalRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticAnalysisSource {
    pub chunk_id: String,
    pub path: String,
    pub source_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticAnalysisToolOutput {
    pub detector: StaticAnalysisDetector,
    pub command: Vec<String>,
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub runtime_exceeded: bool,
    pub toolchain_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticAnalysisRawOutputs {
    pub mypy: StaticAnalysisToolOutput,
    pub pyright: StaticAnalysisToolOutput,
    pub ruff: StaticAnalysisToolOutput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticAnalysisExtraction {
    pub labels: Vec<StaticAnalysisLabel>,
    pub quarantines: Vec<StaticAnalysisQuarantine>,
    pub static_analysis_input: StaticAnalysisInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StaticAnalysisRawOutputPaths {
    pub row_id: String,
    pub root: PathBuf,
    pub mypy_json: PathBuf,
    pub pyright_json: PathBuf,
    pub ruff_json: PathBuf,
}

pub fn run_static_analysis_tools(
    file_path: impl AsRef<Path>,
) -> InstrumentResult<StaticAnalysisRawOutputs> {
    run_static_analysis_tools_with_timeout(
        file_path,
        Duration::from_secs(DEFAULT_STATIC_ANALYSIS_TIMEOUT_SECS),
    )
}

pub fn run_static_analysis_tools_with_timeout(
    file_path: impl AsRef<Path>,
    timeout: Duration,
) -> InstrumentResult<StaticAnalysisRawOutputs> {
    let file_path = file_path.as_ref();
    if timeout.is_zero() {
        return invalid(
            "static_analysis.timeout",
            "static-analysis timeout is zero",
            "configure a positive caller-enforced timeout for mypy/pyright/ruff",
        );
    }
    if !file_path.is_file() {
        return invalid(
            "static_analysis.file_path",
            format!("{} is not a readable file", file_path.display()),
            "materialize the Python corpus/project file before running static analysis",
        );
    }
    let path_arg = file_path.to_str().ok_or_else(|| {
        InstrumentError::invalid(
            "static_analysis.file_path",
            format!("{} is not valid UTF-8", file_path.display()),
            "static-analysis command paths must be valid UTF-8 for raw-output provenance",
        )
    })?;
    validate_no_nul("static_analysis.file_path", path_arg)?;

    Ok(StaticAnalysisRawOutputs {
        mypy: run_detector_command(
            StaticAnalysisDetector::Mypy,
            "mypy",
            &["--strict", "--no-error-summary", path_arg],
            timeout,
        )?,
        pyright: run_detector_command(
            StaticAnalysisDetector::Pyright,
            "pyright",
            &["--outputjson", path_arg],
            timeout,
        )?,
        ruff: run_detector_command(
            StaticAnalysisDetector::Ruff,
            "ruff",
            &["check", "--output-format", "json", path_arg],
            timeout,
        )?,
    })
}

pub fn extract_static_analysis_labels(
    source: &StaticAnalysisSource,
    outputs: &StaticAnalysisRawOutputs,
) -> InstrumentResult<StaticAnalysisExtraction> {
    validate_source(source)?;
    validate_tool_output(&outputs.mypy, StaticAnalysisDetector::Mypy)?;
    validate_tool_output(&outputs.pyright, StaticAnalysisDetector::Pyright)?;
    validate_tool_output(&outputs.ruff, StaticAnalysisDetector::Ruff)?;

    let mut labels = Vec::new();
    let mut quarantines = Vec::new();
    collect_detector_output(source, &outputs.mypy, &mut labels, &mut quarantines)?;
    collect_detector_output(source, &outputs.pyright, &mut labels, &mut quarantines)?;
    collect_detector_output(source, &outputs.ruff, &mut labels, &mut quarantines)?;
    if labels.len() > MAX_LABELS {
        return invalid(
            "labels",
            format!(
                "static-analysis label count {} exceeds {MAX_LABELS}",
                labels.len()
            ),
            "shard large analyzer outputs before persisting static-analysis labels",
        );
    }
    for label in &labels {
        validate_label(label, source)?;
    }
    for quarantine in &quarantines {
        validate_quarantine(quarantine)?;
    }
    let static_analysis_input = StaticAnalysisInput {
        source_text: source.source_text.clone(),
        diagnostics: labels
            .iter()
            .map(|label| Diagnostic {
                tool: label.detector.as_str().to_string(),
                severity: label.severity,
                category: label.rule_id.clone(),
                line: label.line_range.start_line,
                column: label.line_range.start_column,
            })
            .collect(),
        churn_30d: None,
    };
    Ok(StaticAnalysisExtraction {
        labels,
        quarantines,
        static_analysis_input,
    })
}

pub fn write_static_analysis_raw_outputs(
    root: impl AsRef<Path>,
    row_id: &str,
    outputs: &StaticAnalysisRawOutputs,
) -> InstrumentResult<StaticAnalysisRawOutputPaths> {
    validate_path_component("row_id", row_id)?;
    let root = root.as_ref().join(row_id);
    fs::create_dir_all(&root).map_err(|err| {
        InstrumentError::store(
            "create_dir_all",
            "python-static-analysis-v1",
            err.to_string(),
            "ensure the D-root corpus raw-output directory is writable",
        )
    })?;
    let paths = StaticAnalysisRawOutputPaths {
        row_id: row_id.to_string(),
        root: root.clone(),
        mypy_json: root.join("mypy.json"),
        pyright_json: root.join("pyright.json"),
        ruff_json: root.join("ruff.json"),
    };
    write_json_readback(&paths.mypy_json, &outputs.mypy)?;
    write_json_readback(&paths.pyright_json, &outputs.pyright)?;
    write_json_readback(&paths.ruff_json, &outputs.ruff)?;
    Ok(paths)
}

pub struct StaticAnalysisSignalStore {
    db: DB,
}

impl StaticAnalysisSignalStore {
    pub fn open(path: impl AsRef<Path>) -> InstrumentResult<Self> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_paranoid_checks(true);
        let descriptors = vec![
            ColumnFamilyDescriptor::new("default", Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_STATIC_ANALYSIS_SIGNALS, cf_options()),
        ];
        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), descriptors).map_err(|err| {
            InstrumentError::store(
                "open",
                CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
                err.to_string(),
                "inspect the RocksDB path, lock ownership, and column-family metadata",
            )
        })?;
        if db.cf_handle(CF_MEJEPA_STATIC_ANALYSIS_SIGNALS).is_none() {
            return Err(InstrumentError::store(
                "cf_handle",
                CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
                "column family missing after RocksDB open",
                "open static-analysis signal storage with the canonical CF descriptor",
            ));
        }
        Ok(Self { db })
    }

    pub fn put_extraction(
        &self,
        extraction: &StaticAnalysisExtraction,
    ) -> InstrumentResult<Vec<String>> {
        let mut keys = Vec::new();
        for label in &extraction.labels {
            let record = PersistedStaticAnalysisSignal {
                schema_version: STATIC_ANALYSIS_SCHEMA_VERSION,
                signal: StaticAnalysisSignalRecord::Label(label.clone()),
            };
            keys.push(self.put_record(&record)?);
        }
        for quarantine in &extraction.quarantines {
            let record = PersistedStaticAnalysisSignal {
                schema_version: STATIC_ANALYSIS_SCHEMA_VERSION,
                signal: StaticAnalysisSignalRecord::Quarantine(quarantine.clone()),
            };
            keys.push(self.put_record(&record)?);
        }
        Ok(keys)
    }

    pub fn scan_records(&self) -> InstrumentResult<Vec<(String, PersistedStaticAnalysisSignal)>> {
        let cf = self.cf()?;
        let mut rows = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                InstrumentError::store(
                    "iterate",
                    CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
                    err.to_string(),
                    "inspect RocksDB iterator state and static-analysis CF health",
                )
            })?;
            let key = String::from_utf8(key.to_vec()).map_err(|err| {
                InstrumentError::store(
                    "decode_key",
                    CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
                    err.to_string(),
                    "static-analysis signal keys must be UTF-8",
                )
            })?;
            let record = serde_json::from_slice(&value).map_err(|err| {
                InstrumentError::store(
                    "deserialize",
                    CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
                    err.to_string(),
                    "only mutate static-analysis signal rows through StaticAnalysisSignalStore",
                )
            })?;
            rows.push((key, record));
        }
        Ok(rows)
    }

    pub fn count_records(&self) -> InstrumentResult<usize> {
        Ok(self.scan_records()?.len())
    }

    pub fn flush(&self) -> InstrumentResult<()> {
        self.db.flush_cf(self.cf()?).map_err(|err| {
            InstrumentError::store(
                "flush",
                CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
                err.to_string(),
                "inspect RocksDB WAL and filesystem state",
            )
        })
    }

    fn put_record(&self, record: &PersistedStaticAnalysisSignal) -> InstrumentResult<String> {
        let value = serde_json::to_vec(record).map_err(|err| {
            InstrumentError::store(
                "serialize",
                CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
                err.to_string(),
                "ensure static-analysis signal records remain JSON-serializable",
            )
        })?;
        let key = static_analysis_record_key(record, &value);
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(true);
        self.db
            .put_cf_opt(self.cf()?, key.as_bytes(), &value, &write_opts)
            .map_err(|err| {
                InstrumentError::store(
                    "put_cf",
                    CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
                    err.to_string(),
                    "inspect RocksDB write permissions, WAL state, and disk capacity",
                )
            })?;
        let readback = self
            .db
            .get_cf(self.cf()?, key.as_bytes())
            .map_err(|err| {
                InstrumentError::store(
                    "get_cf",
                    CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
                    err.to_string(),
                    "inspect RocksDB read permissions and column-family health",
                )
            })?
            .ok_or_else(|| {
                InstrumentError::store(
                    "read_after_write",
                    CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
                    "static-analysis row missing after put_cf",
                    "do not advance reward-signal checkpoints until the CF row is readable",
                )
            })?;
        if readback != value {
            return Err(InstrumentError::store(
                "read_after_write",
                CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
                "readback bytes differ from written static-analysis payload",
                "inspect RocksDB WAL/device integrity before continuing",
            ));
        }
        Ok(key)
    }

    fn cf(&self) -> InstrumentResult<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_MEJEPA_STATIC_ANALYSIS_SIGNALS)
            .ok_or_else(|| {
                InstrumentError::store(
                    "cf_handle",
                    CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
                    "column-family handle not found",
                    "open the store through StaticAnalysisSignalStore::open",
                )
            })
    }
}

fn collect_detector_output(
    source: &StaticAnalysisSource,
    output: &StaticAnalysisToolOutput,
    labels: &mut Vec<StaticAnalysisLabel>,
    quarantines: &mut Vec<StaticAnalysisQuarantine>,
) -> InstrumentResult<()> {
    if output.runtime_exceeded {
        quarantines.push(quarantine(
            source,
            output.detector,
            "STATIC_ANALYSIS_RUNTIME_EXCEEDED",
            "analyzer exceeded the caller-enforced runtime limit",
        ));
        return Ok(());
    }
    if output.toolchain_missing {
        quarantines.push(quarantine(
            source,
            output.detector,
            "STATIC_ANALYSIS_TOOLCHAIN_MISSING",
            "required analyzer binary was unavailable",
        ));
        return Ok(());
    }
    match output.detector {
        StaticAnalysisDetector::Mypy => parse_mypy_output(source, &output.stdout, labels),
        StaticAnalysisDetector::Pyright => parse_pyright_output(source, &output.stdout, labels),
        StaticAnalysisDetector::Ruff => parse_ruff_output(source, &output.stdout, labels),
    }
}

fn run_detector_command(
    detector: StaticAnalysisDetector,
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> InstrumentResult<StaticAnalysisToolOutput> {
    let command = std::iter::once(program.to_string())
        .chain(args.iter().map(|arg| (*arg).to_string()))
        .collect::<Vec<_>>();
    let mut child = match Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            return Ok(StaticAnalysisToolOutput {
                detector,
                command,
                status_code: None,
                stdout: String::new(),
                stderr: format!("failed to spawn {program}: {err}"),
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
                "python-static-analysis-v1",
                err.to_string(),
                "inspect analyzer process state and caller timeout handling",
            )
        })? {
            Some(_) => {
                let output = child.wait_with_output().map_err(|err| {
                    InstrumentError::store(
                        "wait_with_output",
                        "python-static-analysis-v1",
                        err.to_string(),
                        "capture static-analysis stdout/stderr before extracting labels",
                    )
                })?;
                return Ok(tool_output_from_process(
                    detector, command, output, false, false,
                ));
            }
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let output = child.wait_with_output().map_err(|err| {
                    InstrumentError::store(
                        "wait_with_output",
                        "python-static-analysis-v1",
                        err.to_string(),
                        "capture timed-out static-analysis stdout/stderr for quarantine evidence",
                    )
                })?;
                return Ok(tool_output_from_process(
                    detector, command, output, true, false,
                ));
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn tool_output_from_process(
    detector: StaticAnalysisDetector,
    command: Vec<String>,
    output: Output,
    runtime_exceeded: bool,
    toolchain_missing: bool,
) -> StaticAnalysisToolOutput {
    StaticAnalysisToolOutput {
        detector,
        command,
        status_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        runtime_exceeded,
        toolchain_missing,
    }
}

fn parse_mypy_output(
    source: &StaticAnalysisSource,
    stdout: &str,
    labels: &mut Vec<StaticAnalysisLabel>,
) -> InstrumentResult<()> {
    for raw_line in stdout.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("Success: ") || line.starts_with("Found ") {
            continue;
        }
        let Some((marker, severity)) = mypy_marker(line) else {
            return invalid(
                "mypy.stdout",
                format!("unrecognized mypy diagnostic line: {line}"),
                "run mypy with --no-error-summary and persist raw stdout for parser readback",
            );
        };
        let marker_idx = line.find(marker).ok_or_else(|| {
            InstrumentError::invalid(
                "mypy.stdout",
                "mypy marker disappeared during parsing",
                "inspect TASK-RWD-208 mypy parser state",
            )
        })?;
        let prefix = &line[..marker_idx];
        let message = line[marker_idx + marker.len()..].trim();
        let (line_no, column) = parse_mypy_location(prefix)?;
        if has_line_marker(&source.source_text, line_no, "# type: ignore") {
            continue;
        }
        labels.push(StaticAnalysisLabel {
            chunk_id: source.chunk_id.clone(),
            path: source.path.clone(),
            signal_kind: StaticAnalysisSignalKind::TypeError,
            rule_id: bracket_rule_id(message).unwrap_or_else(|| "mypy".to_string()),
            severity,
            line_range: StaticAnalysisLineRange {
                start_line: line_no,
                end_line: line_no,
                start_column: column,
                end_column: column,
            },
            detector: StaticAnalysisDetector::Mypy,
            message: message.to_string(),
        });
    }
    Ok(())
}

fn parse_pyright_output(
    source: &StaticAnalysisSource,
    stdout: &str,
    labels: &mut Vec<StaticAnalysisLabel>,
) -> InstrumentResult<()> {
    let value = parse_json("pyright.stdout", stdout)?;
    let diagnostics = value
        .get("generalDiagnostics")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            InstrumentError::invalid(
                "pyright.generalDiagnostics",
                "pyright JSON missing generalDiagnostics array",
                "persist pyright --outputjson stdout without truncation",
            )
        })?;
    for diagnostic in diagnostics {
        let severity_text = string_field(diagnostic, "severity", "pyright.severity")?;
        let severity = match severity_text {
            "error" => DiagnosticSeverity::Error,
            "warning" => DiagnosticSeverity::Warning,
            "information" => DiagnosticSeverity::Info,
            _ => DiagnosticSeverity::Hint,
        };
        let message = string_field(diagnostic, "message", "pyright.message")?;
        let range = diagnostic.get("range").ok_or_else(|| {
            InstrumentError::invalid(
                "pyright.range",
                "pyright diagnostic missing range",
                "persist pyright --outputjson diagnostics with source ranges",
            )
        })?;
        let start = range.get("start").ok_or_else(|| {
            InstrumentError::invalid(
                "pyright.range.start",
                "pyright diagnostic missing range.start",
                "persist pyright --outputjson diagnostics with source ranges",
            )
        })?;
        let end = range.get("end").unwrap_or(start);
        let start_line = u32_field(start, "line", "pyright.range.start.line")? + 1;
        if has_line_marker(&source.source_text, start_line, "# pyright: ignore") {
            continue;
        }
        let start_column = u32_field(start, "character", "pyright.range.start.character")? + 1;
        let end_line =
            u32_field(end, "line", "pyright.range.end.line").unwrap_or(start_line - 1) + 1;
        let end_column = u32_field(end, "character", "pyright.range.end.character")
            .unwrap_or(start_column - 1)
            + 1;
        let rule_id = diagnostic
            .get("rule")
            .and_then(Value::as_str)
            .filter(|rule| !rule.trim().is_empty())
            .unwrap_or(severity_text)
            .to_string();
        labels.push(StaticAnalysisLabel {
            chunk_id: source.chunk_id.clone(),
            path: source.path.clone(),
            signal_kind: StaticAnalysisSignalKind::TypeError,
            rule_id,
            severity,
            line_range: StaticAnalysisLineRange {
                start_line,
                end_line,
                start_column,
                end_column,
            },
            detector: StaticAnalysisDetector::Pyright,
            message: message.to_string(),
        });
    }
    Ok(())
}

fn parse_ruff_output(
    source: &StaticAnalysisSource,
    stdout: &str,
    labels: &mut Vec<StaticAnalysisLabel>,
) -> InstrumentResult<()> {
    let value = parse_json("ruff.stdout", stdout)?;
    let diagnostics = value.as_array().ok_or_else(|| {
        InstrumentError::invalid(
            "ruff.stdout",
            "ruff JSON output must be an array",
            "run ruff check --output-format json and persist raw stdout",
        )
    })?;
    for diagnostic in diagnostics {
        let code = string_field(diagnostic, "code", "ruff.code")?;
        let message = string_field(diagnostic, "message", "ruff.message")?;
        let location = diagnostic.get("location").ok_or_else(|| {
            InstrumentError::invalid(
                "ruff.location",
                "ruff diagnostic missing location",
                "persist ruff JSON diagnostics with location rows",
            )
        })?;
        let start_line = u32_field(location, "row", "ruff.location.row")?;
        let start_column = u32_field(location, "column", "ruff.location.column")?;
        let end_location = diagnostic.get("end_location").unwrap_or(location);
        let end_line =
            u32_field(end_location, "row", "ruff.end_location.row").unwrap_or(start_line);
        let end_column =
            u32_field(end_location, "column", "ruff.end_location.column").unwrap_or(start_column);
        labels.push(StaticAnalysisLabel {
            chunk_id: source.chunk_id.clone(),
            path: source.path.clone(),
            signal_kind: ruff_signal_kind(code),
            rule_id: code.to_string(),
            severity: ruff_severity(code),
            line_range: StaticAnalysisLineRange {
                start_line,
                end_line,
                start_column,
                end_column,
            },
            detector: StaticAnalysisDetector::Ruff,
            message: message.to_string(),
        });
    }
    Ok(())
}

fn validate_source(source: &StaticAnalysisSource) -> InstrumentResult<()> {
    validate_single_line("source.chunk_id", &source.chunk_id)?;
    validate_no_nul("source.path", &source.path)?;
    if source.path.trim().is_empty() {
        return invalid(
            "source.path",
            "static-analysis source path is empty",
            "persist the analyzed Python file path with every static-analysis label",
        );
    }
    if source.source_text.trim().is_empty() {
        return invalid(
            "source.source_text",
            "static-analysis source text is empty",
            "capture source text before normalizing analyzer labels",
        );
    }
    if source.source_text.len() > MAX_SOURCE_BYTES {
        return invalid(
            "source.source_text",
            format!(
                "source text is {} bytes, exceeding max {MAX_SOURCE_BYTES}",
                source.source_text.len()
            ),
            "shard very large Python files before static-analysis label extraction",
        );
    }
    if source.source_text.chars().any(|ch| ch == '\0') {
        return invalid(
            "source.source_text",
            "source text contains a NUL byte",
            "persist parser-facing UTF-8 source without embedded NUL bytes",
        );
    }
    Ok(())
}

fn validate_tool_output(
    output: &StaticAnalysisToolOutput,
    expected: StaticAnalysisDetector,
) -> InstrumentResult<()> {
    if output.detector != expected {
        return invalid(
            "tool_output.detector",
            format!(
                "expected {:?} output but found {:?}",
                expected, output.detector
            ),
            "bind mypy/pyright/ruff raw outputs to the matching detector slot",
        );
    }
    if output.command.is_empty() {
        return invalid(
            "tool_output.command",
            "tool output command is empty",
            "persist the command used to produce each static-analysis raw output",
        );
    }
    for (idx, part) in output.command.iter().enumerate() {
        validate_single_line_indexed("tool_output.command", idx, part)?;
    }
    validate_no_nul("tool_output.stdout", &output.stdout)?;
    validate_no_nul("tool_output.stderr", &output.stderr)?;
    Ok(())
}

fn validate_label(
    label: &StaticAnalysisLabel,
    source: &StaticAnalysisSource,
) -> InstrumentResult<()> {
    validate_single_line("label.chunk_id", &label.chunk_id)?;
    validate_no_nul("label.path", &label.path)?;
    validate_single_line("label.rule_id", &label.rule_id)?;
    validate_no_nul("label.message", &label.message)?;
    if label.chunk_id != source.chunk_id || label.path != source.path {
        return invalid(
            "label.source_identity",
            "label chunk_id/path does not match extraction source",
            "do not mix analyzer outputs across chunks before persistence",
        );
    }
    let line_count = source.source_text.lines().count() as u32;
    if label.line_range.start_line == 0 || label.line_range.start_line > line_count {
        return invalid(
            "label.line_range.start_line",
            format!(
                "start_line {} is outside source line count {line_count}",
                label.line_range.start_line
            ),
            "normalize analyzer locations to one-based source lines",
        );
    }
    if label.line_range.end_line < label.line_range.start_line
        || label.line_range.end_line > line_count
    {
        return invalid(
            "label.line_range.end_line",
            format!(
                "end_line {} is invalid for start_line {} and source line count {line_count}",
                label.line_range.end_line, label.line_range.start_line
            ),
            "normalize analyzer end locations to valid one-based source lines",
        );
    }
    if label.line_range.start_column == 0 || label.line_range.end_column == 0 {
        return invalid(
            "label.line_range.column",
            "columns must be one-based and non-zero",
            "normalize analyzer columns to one-based source columns",
        );
    }
    Ok(())
}

fn validate_quarantine(quarantine: &StaticAnalysisQuarantine) -> InstrumentResult<()> {
    validate_single_line("quarantine.chunk_id", &quarantine.chunk_id)?;
    validate_no_nul("quarantine.path", &quarantine.path)?;
    validate_single_line("quarantine.reason_code", &quarantine.reason_code)?;
    validate_no_nul("quarantine.detail", &quarantine.detail)
}

fn quarantine(
    source: &StaticAnalysisSource,
    detector: StaticAnalysisDetector,
    reason_code: &str,
    detail: &str,
) -> StaticAnalysisQuarantine {
    StaticAnalysisQuarantine {
        chunk_id: source.chunk_id.clone(),
        path: source.path.clone(),
        detector,
        reason_code: reason_code.to_string(),
        detail: detail.to_string(),
    }
}

fn mypy_marker(line: &str) -> Option<(&'static str, DiagnosticSeverity)> {
    for (marker, severity) in [
        (": error: ", DiagnosticSeverity::Error),
        (": warning: ", DiagnosticSeverity::Warning),
        (": note: ", DiagnosticSeverity::Info),
    ] {
        if line.contains(marker) {
            return Some((marker, severity));
        }
    }
    None
}

fn parse_mypy_location(prefix: &str) -> InstrumentResult<(u32, u32)> {
    let mut parts = prefix.rsplitn(3, ':');
    let right = parts.next().unwrap_or_default();
    let middle = parts.next().ok_or_else(|| {
        InstrumentError::invalid(
            "mypy.location",
            format!("mypy location {prefix:?} does not include a line number"),
            "persist standard mypy diagnostics in path:line[:column] form",
        )
    })?;
    let right_num = parse_u32("mypy.location", right)?;
    if let Ok(middle_num) = middle.parse::<u32>() {
        Ok((middle_num, right_num))
    } else {
        Ok((right_num, 1))
    }
}

fn bracket_rule_id(message: &str) -> Option<String> {
    let trimmed = message.trim_end();
    if !trimmed.ends_with(']') {
        return None;
    }
    let start = trimmed.rfind('[')?;
    let rule = &trimmed[start + 1..trimmed.len() - 1];
    if rule.trim().is_empty() {
        None
    } else {
        Some(rule.to_string())
    }
}

fn has_line_marker(source: &str, one_based_line: u32, marker: &str) -> bool {
    source
        .lines()
        .nth(one_based_line.saturating_sub(1) as usize)
        .is_some_and(|line| line.contains(marker))
}

fn parse_json(field: &'static str, text: &str) -> InstrumentResult<Value> {
    serde_json::from_str(text).map_err(|err| {
        InstrumentError::invalid(
            field,
            format!("invalid JSON: {err}"),
            "persist analyzer JSON stdout without truncation or stderr interleaving",
        )
    })
}

fn string_field<'a>(
    value: &'a Value,
    key: &'static str,
    field: &'static str,
) -> InstrumentResult<&'a str> {
    let value = value.get(key).and_then(Value::as_str).ok_or_else(|| {
        InstrumentError::invalid(
            field,
            format!("missing or non-string field {key}"),
            "persist analyzer diagnostics with the expected JSON schema",
        )
    })?;
    if value.trim().is_empty() {
        return invalid(
            field,
            format!("{field} is empty"),
            "persist analyzer diagnostics with non-empty string fields",
        );
    }
    Ok(value)
}

fn u32_field(value: &Value, key: &'static str, field: &'static str) -> InstrumentResult<u32> {
    let raw = value.get(key).and_then(Value::as_u64).ok_or_else(|| {
        InstrumentError::invalid(
            field,
            format!("missing or non-integer field {key}"),
            "persist analyzer diagnostic locations as unsigned integer fields",
        )
    })?;
    u32::try_from(raw).map_err(|_| {
        InstrumentError::invalid(
            field,
            format!("{field} value {raw} exceeds u32"),
            "shard impossible analyzer locations before static-analysis persistence",
        )
    })
}

fn ruff_signal_kind(code: &str) -> StaticAnalysisSignalKind {
    if code.starts_with('F')
        || code.starts_with('E')
        || code.starts_with('B')
        || code.starts_with('S')
    {
        StaticAnalysisSignalKind::LintError
    } else {
        StaticAnalysisSignalKind::ConventionViolation
    }
}

fn ruff_severity(code: &str) -> DiagnosticSeverity {
    if code.starts_with('F') || code.starts_with('E') {
        DiagnosticSeverity::Error
    } else if code.starts_with('W') || code.starts_with('B') || code.starts_with('S') {
        DiagnosticSeverity::Warning
    } else {
        DiagnosticSeverity::Info
    }
}

fn static_analysis_record_key(record: &PersistedStaticAnalysisSignal, value: &[u8]) -> String {
    let (chunk_id, detector, start_line) = match &record.signal {
        StaticAnalysisSignalRecord::Label(label) => (
            label.chunk_id.as_str(),
            label.detector.as_str(),
            label.line_range.start_line,
        ),
        StaticAnalysisSignalRecord::Quarantine(quarantine) => (
            quarantine.chunk_id.as_str(),
            quarantine.detector.as_str(),
            0,
        ),
    };
    format!(
        "static-analysis/{chunk_id}/{detector}/{start_line:08}/{:016x}",
        fnv1a64(value)
    )
}

fn write_json_readback<T: Serialize>(path: &Path, value: &T) -> InstrumentResult<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|err| {
        InstrumentError::store(
            "serialize",
            "python-static-analysis-v1",
            err.to_string(),
            "ensure static-analysis raw output wrappers remain JSON-serializable",
        )
    })?;
    fs::write(path, &bytes).map_err(|err| {
        InstrumentError::store(
            "write",
            "python-static-analysis-v1",
            err.to_string(),
            "ensure the D-root corpus raw-output directory is writable",
        )
    })?;
    let readback = fs::read(path).map_err(|err| {
        InstrumentError::store(
            "read",
            "python-static-analysis-v1",
            err.to_string(),
            "read back raw static-analysis output immediately after writing",
        )
    })?;
    if readback != bytes {
        return Err(InstrumentError::store(
            "read_after_write",
            "python-static-analysis-v1",
            "raw output readback bytes differ from written payload",
            "do not advance static-analysis checkpoints until raw outputs are durable",
        ));
    }
    Ok(())
}

fn validate_path_component(field: &'static str, value: &str) -> InstrumentResult<()> {
    validate_single_line(field, value)?;
    if value == "." || value == ".." || value.contains('/') || value.contains('\\') {
        return invalid(
            field,
            format!("{field} must be a single path component, got {value:?}"),
            "use a slash-free row_id when writing static-analysis raw outputs",
        );
    }
    Ok(())
}

fn validate_single_line(field: &'static str, value: &str) -> InstrumentResult<()> {
    if value.trim().is_empty() {
        return invalid(
            field,
            format!("{field} is empty or whitespace-only"),
            "persist static-analysis fields as non-empty UTF-8 strings",
        );
    }
    if value.chars().any(char::is_control) {
        return invalid(
            field,
            format!("{field} contains a control character"),
            "strip control characters before persisting static-analysis fields",
        );
    }
    Ok(())
}

fn validate_single_line_indexed(
    field: &'static str,
    idx: usize,
    value: &str,
) -> InstrumentResult<()> {
    if value.trim().is_empty() {
        return invalid(
            field,
            format!("{field}[{idx}] is empty or whitespace-only"),
            "persist static-analysis commands as non-empty UTF-8 strings",
        );
    }
    if value.chars().any(char::is_control) {
        return invalid(
            field,
            format!("{field}[{idx}] contains a control character"),
            "strip control characters before persisting static-analysis commands",
        );
    }
    Ok(())
}

fn validate_no_nul(field: &'static str, value: &str) -> InstrumentResult<()> {
    if value.chars().any(|ch| ch == '\0') {
        return invalid(
            field,
            format!("{field} contains a NUL byte"),
            "persist static-analysis fields as valid UTF-8 without embedded NUL bytes",
        );
    }
    Ok(())
}

fn parse_u32(field: &'static str, value: &str) -> InstrumentResult<u32> {
    value.parse::<u32>().map_err(|err| {
        InstrumentError::invalid(
            field,
            format!("could not parse {value:?} as u32: {err}"),
            "persist analyzer locations as unsigned integer fields",
        )
    })
}

fn invalid<T>(
    field: &'static str,
    message: impl Into<String>,
    remediation: &'static str,
) -> InstrumentResult<T> {
    Err(InstrumentError::invalid(field, message, remediation))
}

fn cf_options() -> Options {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EStaticAnalysisInstrument, Instrument};

    fn source() -> StaticAnalysisSource {
        StaticAnalysisSource {
            chunk_id: "chunk-1".to_string(),
            path: "sample.py".to_string(),
            source_text: "from typing import Any\n\ndef f(x: int) -> str:\n    return x\n"
                .to_string(),
        }
    }

    fn outputs() -> StaticAnalysisRawOutputs {
        StaticAnalysisRawOutputs {
            mypy: StaticAnalysisToolOutput {
                detector: StaticAnalysisDetector::Mypy,
                command: vec!["mypy".into(), "--strict".into(), "sample.py".into()],
                status_code: Some(1),
                stdout: "sample.py:4: error: Incompatible return value type (got \"int\", expected \"str\")  [return-value]\n".into(),
                stderr: String::new(),
                runtime_exceeded: false,
                toolchain_missing: false,
            },
            pyright: StaticAnalysisToolOutput {
                detector: StaticAnalysisDetector::Pyright,
                command: vec!["pyright".into(), "--outputjson".into(), "sample.py".into()],
                status_code: Some(1),
                stdout: r#"{"generalDiagnostics":[{"file":"sample.py","severity":"error","message":"Type \"int\" is not assignable to return type \"str\"","range":{"start":{"line":3,"character":11},"end":{"line":3,"character":12}},"rule":"reportReturnType"}]}"#.into(),
                stderr: String::new(),
                runtime_exceeded: false,
                toolchain_missing: false,
            },
            ruff: StaticAnalysisToolOutput {
                detector: StaticAnalysisDetector::Ruff,
                command: vec!["ruff".into(), "check".into(), "--output-format".into(), "json".into(), "sample.py".into()],
                status_code: Some(1),
                stdout: r#"[{"cell":null,"code":"F401","end_location":{"column":29,"row":1},"filename":"sample.py","fix":null,"location":{"column":20,"row":1},"message":"`typing.Any` imported but unused","noqa_row":1,"url":"https://docs.astral.sh/ruff/rules/unused-import/"}]"#.into(),
                stderr: String::new(),
                runtime_exceeded: false,
                toolchain_missing: false,
            },
        }
    }

    #[test]
    fn extracts_three_detector_labels_and_static_input() {
        let extraction = extract_static_analysis_labels(&source(), &outputs()).unwrap();
        assert_eq!(extraction.labels.len(), 3);
        assert_eq!(extraction.quarantines.len(), 0);
        assert_eq!(extraction.static_analysis_input.diagnostics.len(), 3);
        assert!(EStaticAnalysisInstrument
            .encode(&extraction.static_analysis_input)
            .unwrap()
            .iter()
            .any(|value| *value != 0.0));
    }

    #[test]
    fn respects_mypy_and_pyright_line_ignores() {
        let mut source = source();
        source.source_text =
            "from typing import Any\n\ndef f(x: int) -> str:\n    return x  # type: ignore # pyright: ignore\n"
                .to_string();
        let extraction = extract_static_analysis_labels(&source, &outputs()).unwrap();
        assert_eq!(
            extraction
                .labels
                .iter()
                .filter(|label| label.detector != StaticAnalysisDetector::Ruff)
                .count(),
            0
        );
        assert_eq!(extraction.labels.len(), 1);
    }

    #[test]
    fn runtime_exceeded_becomes_quarantine_record() {
        let mut outputs = outputs();
        outputs.mypy.runtime_exceeded = true;
        outputs.mypy.stdout.clear();
        let extraction = extract_static_analysis_labels(&source(), &outputs).unwrap();
        assert_eq!(extraction.quarantines.len(), 1);
        assert_eq!(
            extraction.quarantines[0].reason_code,
            "STATIC_ANALYSIS_RUNTIME_EXCEEDED"
        );
    }

    #[test]
    fn runner_marks_unavailable_toolchain_fail_closed() {
        let output = run_detector_command(
            StaticAnalysisDetector::Mypy,
            "__contextgraph_missing_static_analysis_tool_293__",
            &["sample.py"],
            Duration::from_millis(1),
        )
        .unwrap();
        assert_eq!(output.detector, StaticAnalysisDetector::Mypy);
        assert!(output.toolchain_missing);
        assert!(!output.runtime_exceeded);
        assert!(output.status_code.is_none());
    }

    #[test]
    fn malformed_pyright_json_fails_closed() {
        let mut outputs = outputs();
        outputs.pyright.stdout = "{not-json".to_string();
        assert_eq!(
            extract_static_analysis_labels(&source(), &outputs)
                .unwrap_err()
                .code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }
}
