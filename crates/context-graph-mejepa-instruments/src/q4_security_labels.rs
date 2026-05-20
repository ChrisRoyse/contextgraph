// TASK-RWD-204 / TASK-PY-G-046: durable Bandit + Semgrep Q4 security labels.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use context_graph_mejepa_cf::CF_MEJEPA_Q4_SECURITY_LABELS;
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::e_static_analysis::{Diagnostic, DiagnosticSeverity, StaticAnalysisInput};
use crate::features::fnv1a64;
use crate::{InstrumentError, InstrumentResult};

const Q4_SECURITY_SCHEMA_VERSION: u32 = 1;
const MAX_SOURCE_BYTES: usize = 10_000_000;
const MAX_LABELS: usize = 100_000;
pub const DEFAULT_Q4_SECURITY_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4SecurityDetector {
    Bandit,
    Semgrep,
}

impl Q4SecurityDetector {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bandit => "bandit",
            Self::Semgrep => "semgrep",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4SecurityScanPhase {
    PrePatch,
    PostPatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4SecuritySeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl Q4SecuritySeverity {
    fn as_diagnostic(self) -> DiagnosticSeverity {
        match self {
            Self::Critical | Self::High => DiagnosticSeverity::Error,
            Self::Medium => DiagnosticSeverity::Warning,
            Self::Low => DiagnosticSeverity::Info,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4SecurityClass {
    SqlInjection,
    CommandInjection,
    PathTraversal,
    Xss,
    Csrf,
    Ssrf,
    Deserialization,
    HardcodedSecret,
    LoggingSecret,
    InsecureCryptoAlgo,
    InsufficientCryptoKeyLength,
    MissingAuth,
    BrokenAccessControl,
    OpenRedirect,
    MissingTlsVerify,
    Other,
}

impl Q4SecurityClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SqlInjection => "sql_injection",
            Self::CommandInjection => "command_injection",
            Self::PathTraversal => "path_traversal",
            Self::Xss => "xss",
            Self::Csrf => "csrf",
            Self::Ssrf => "ssrf",
            Self::Deserialization => "deserialization",
            Self::HardcodedSecret => "hardcoded_secret",
            Self::LoggingSecret => "logging_secret",
            Self::InsecureCryptoAlgo => "insecure_crypto_algo",
            Self::InsufficientCryptoKeyLength => "insufficient_crypto_key_length",
            Self::MissingAuth => "missing_auth",
            Self::BrokenAccessControl => "broken_access_control",
            Self::OpenRedirect => "open_redirect",
            Self::MissingTlsVerify => "missing_tls_verify",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4SecurityLineRange {
    pub start_line: u32,
    pub end_line: u32,
    pub start_column: u32,
    pub end_column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4SecurityFinding {
    pub finding_id: String,
    pub rule_id: String,
    pub severity: Q4SecuritySeverity,
    pub class: Q4SecurityClass,
    pub file: String,
    pub line_range: Q4SecurityLineRange,
    pub detector: Q4SecurityDetector,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4SecurityLabel {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub finding_id: String,
    pub rule_id: String,
    pub severity: Q4SecuritySeverity,
    pub class: Q4SecurityClass,
    pub file: String,
    pub line_range: Q4SecurityLineRange,
    pub detector: Q4SecurityDetector,
    pub message: String,
    pub introduced_by_patch: bool,
    pub fixed_by_patch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4SecurityQuarantine {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub detector: Q4SecurityDetector,
    pub phase: Q4SecurityScanPhase,
    pub reason_code: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "record_kind", content = "record", rename_all = "snake_case")]
pub enum Q4SecuritySignalRecord {
    Label(Q4SecurityLabel),
    Quarantine(Q4SecurityQuarantine),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedQ4SecuritySignal {
    pub schema_version: u32,
    pub signal: Q4SecuritySignalRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4SecuritySource {
    pub corpus_row_id: String,
    pub chunk_id: String,
    pub logical_path: String,
    pub pre_patch_source: String,
    pub post_patch_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4SecurityToolOutput {
    pub detector: Q4SecurityDetector,
    pub phase: Q4SecurityScanPhase,
    pub command: Vec<String>,
    pub status_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub runtime_exceeded: bool,
    pub toolchain_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4SecurityRawOutputs {
    pub bandit_pre: Q4SecurityToolOutput,
    pub bandit_post: Q4SecurityToolOutput,
    pub semgrep_pre: Q4SecurityToolOutput,
    pub semgrep_post: Q4SecurityToolOutput,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4SecurityExtraction {
    pub labels: Vec<Q4SecurityLabel>,
    pub quarantines: Vec<Q4SecurityQuarantine>,
    pub static_analysis_input: StaticAnalysisInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4SecurityRawOutputPaths {
    pub row_id: String,
    pub root: PathBuf,
    pub bandit_json: PathBuf,
    pub semgrep_json: PathBuf,
}

pub fn run_q4_security_tools(
    pre_patch_file: impl AsRef<Path>,
    post_patch_file: impl AsRef<Path>,
) -> InstrumentResult<Q4SecurityRawOutputs> {
    run_q4_security_tools_with_timeout(
        pre_patch_file,
        post_patch_file,
        Duration::from_secs(DEFAULT_Q4_SECURITY_TIMEOUT_SECS),
    )
}

pub fn run_q4_security_tools_with_timeout(
    pre_patch_file: impl AsRef<Path>,
    post_patch_file: impl AsRef<Path>,
    timeout: Duration,
) -> InstrumentResult<Q4SecurityRawOutputs> {
    if timeout.is_zero() {
        return invalid(
            "q4_security.timeout",
            "Q4 security analyzer timeout is zero",
            "configure a positive caller-enforced timeout for Bandit/Semgrep",
        );
    }
    let pre = validate_file_path("q4_security.pre_patch_file", pre_patch_file.as_ref())?;
    let post = validate_file_path("q4_security.post_patch_file", post_patch_file.as_ref())?;
    Ok(Q4SecurityRawOutputs {
        bandit_pre: run_detector_command(
            Q4SecurityDetector::Bandit,
            Q4SecurityScanPhase::PrePatch,
            "bandit",
            &["-q", "-f", "json", &pre],
            timeout,
        )?,
        bandit_post: run_detector_command(
            Q4SecurityDetector::Bandit,
            Q4SecurityScanPhase::PostPatch,
            "bandit",
            &["-q", "-f", "json", &post],
            timeout,
        )?,
        semgrep_pre: run_detector_command(
            Q4SecurityDetector::Semgrep,
            Q4SecurityScanPhase::PrePatch,
            "semgrep",
            &[
                "scan",
                "--json",
                "--metrics=off",
                "--disable-version-check",
                "--config",
                bundled_semgrep_config(),
                &pre,
            ],
            timeout,
        )?,
        semgrep_post: run_detector_command(
            Q4SecurityDetector::Semgrep,
            Q4SecurityScanPhase::PostPatch,
            "semgrep",
            &[
                "scan",
                "--json",
                "--metrics=off",
                "--disable-version-check",
                "--config",
                bundled_semgrep_config(),
                &post,
            ],
            timeout,
        )?,
    })
}

pub fn extract_q4_security_labels(
    source: &Q4SecuritySource,
    outputs: &Q4SecurityRawOutputs,
) -> InstrumentResult<Q4SecurityExtraction> {
    validate_source(source)?;
    validate_tool_output(&outputs.bandit_pre, Q4SecurityDetector::Bandit)?;
    validate_tool_output(&outputs.bandit_post, Q4SecurityDetector::Bandit)?;
    validate_tool_output(&outputs.semgrep_pre, Q4SecurityDetector::Semgrep)?;
    validate_tool_output(&outputs.semgrep_post, Q4SecurityDetector::Semgrep)?;

    let mut quarantines = Vec::new();
    let mut pre_findings = Vec::new();
    let mut post_findings = Vec::new();
    collect_detector_output(
        source,
        &outputs.bandit_pre,
        &mut pre_findings,
        &mut quarantines,
    )?;
    collect_detector_output(
        source,
        &outputs.bandit_post,
        &mut post_findings,
        &mut quarantines,
    )?;
    collect_detector_output(
        source,
        &outputs.semgrep_pre,
        &mut pre_findings,
        &mut quarantines,
    )?;
    collect_detector_output(
        source,
        &outputs.semgrep_post,
        &mut post_findings,
        &mut quarantines,
    )?;

    let labels = diff_findings(source, pre_findings, post_findings)?;
    if labels.len() > MAX_LABELS {
        return invalid(
            "q4_security.labels",
            format!(
                "Q4 security label count {} exceeds {MAX_LABELS}",
                labels.len()
            ),
            "shard large analyzer outputs before persisting Q4 security labels",
        );
    }
    for label in &labels {
        validate_label(label, source)?;
    }
    for quarantine in &quarantines {
        validate_quarantine(quarantine, source)?;
    }
    let static_analysis_input = StaticAnalysisInput {
        source_text: source.post_patch_source.clone(),
        diagnostics: labels
            .iter()
            .map(|label| Diagnostic {
                tool: label.detector.as_str().to_string(),
                severity: label.severity.as_diagnostic(),
                category: format!("{}:{}", label.class.as_str(), label.rule_id),
                line: label.line_range.start_line,
                column: label.line_range.start_column,
            })
            .collect(),
        churn_30d: None,
        evidence_unavailable: false,
    };
    Ok(Q4SecurityExtraction {
        labels,
        quarantines,
        static_analysis_input,
    })
}

pub fn write_q4_security_raw_outputs(
    root: impl AsRef<Path>,
    row_id: &str,
    outputs: &Q4SecurityRawOutputs,
) -> InstrumentResult<Q4SecurityRawOutputPaths> {
    validate_path_component("row_id", row_id)?;
    let root = root.as_ref().join(row_id);
    fs::create_dir_all(&root).map_err(|err| {
        InstrumentError::store(
            "create_dir_all",
            "python-q4-security-labels-v1",
            err.to_string(),
            "ensure the D-root corpus raw-output directory is writable",
        )
    })?;
    let paths = Q4SecurityRawOutputPaths {
        row_id: row_id.to_string(),
        bandit_json: root.join("bandit.json"),
        semgrep_json: root.join("semgrep.json"),
        root,
    };
    write_raw_wrapper(
        &paths.bandit_json,
        &outputs.bandit_pre,
        &outputs.bandit_post,
    )?;
    write_raw_wrapper(
        &paths.semgrep_json,
        &outputs.semgrep_pre,
        &outputs.semgrep_post,
    )?;
    Ok(paths)
}

pub struct Q4SecurityLabelStore {
    db: DB,
}

impl Q4SecurityLabelStore {
    pub fn open(path: impl AsRef<Path>) -> InstrumentResult<Self> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_paranoid_checks(true);
        let descriptors = vec![
            ColumnFamilyDescriptor::new("default", Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_Q4_SECURITY_LABELS, cf_options()),
        ];
        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), descriptors).map_err(|err| {
            InstrumentError::store(
                "open",
                CF_MEJEPA_Q4_SECURITY_LABELS,
                err.to_string(),
                "inspect the RocksDB path, lock ownership, and column-family metadata",
            )
        })?;
        if db.cf_handle(CF_MEJEPA_Q4_SECURITY_LABELS).is_none() {
            return Err(InstrumentError::store(
                "cf_handle",
                CF_MEJEPA_Q4_SECURITY_LABELS,
                "column family missing after RocksDB open",
                "open Q4 security label storage with the canonical CF descriptor",
            ));
        }
        Ok(Self { db })
    }

    pub fn put_extraction(
        &self,
        extraction: &Q4SecurityExtraction,
    ) -> InstrumentResult<Vec<String>> {
        let mut keys = Vec::new();
        for label in &extraction.labels {
            let record = PersistedQ4SecuritySignal {
                schema_version: Q4_SECURITY_SCHEMA_VERSION,
                signal: Q4SecuritySignalRecord::Label(label.clone()),
            };
            keys.push(self.put_record(&record)?);
        }
        for quarantine in &extraction.quarantines {
            let record = PersistedQ4SecuritySignal {
                schema_version: Q4_SECURITY_SCHEMA_VERSION,
                signal: Q4SecuritySignalRecord::Quarantine(quarantine.clone()),
            };
            keys.push(self.put_record(&record)?);
        }
        Ok(keys)
    }

    pub fn scan_records(&self) -> InstrumentResult<Vec<(String, PersistedQ4SecuritySignal)>> {
        let cf = self.cf()?;
        let mut rows = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                InstrumentError::store(
                    "iterate",
                    CF_MEJEPA_Q4_SECURITY_LABELS,
                    err.to_string(),
                    "inspect RocksDB iterator state and Q4 security CF health",
                )
            })?;
            let key = String::from_utf8(key.to_vec()).map_err(|err| {
                InstrumentError::store(
                    "decode_key",
                    CF_MEJEPA_Q4_SECURITY_LABELS,
                    err.to_string(),
                    "Q4 security signal keys must be UTF-8",
                )
            })?;
            let record = serde_json::from_slice(&value).map_err(|err| {
                InstrumentError::store(
                    "deserialize",
                    CF_MEJEPA_Q4_SECURITY_LABELS,
                    err.to_string(),
                    "only mutate Q4 security rows through Q4SecurityLabelStore",
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
                CF_MEJEPA_Q4_SECURITY_LABELS,
                err.to_string(),
                "inspect RocksDB WAL and filesystem state",
            )
        })
    }

    fn put_record(&self, record: &PersistedQ4SecuritySignal) -> InstrumentResult<String> {
        let value = serde_json::to_vec(record).map_err(|err| {
            InstrumentError::store(
                "serialize",
                CF_MEJEPA_Q4_SECURITY_LABELS,
                err.to_string(),
                "ensure Q4 security records remain JSON-serializable",
            )
        })?;
        let key = q4_security_record_key(record, &value);
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(true);
        self.db
            .put_cf_opt(self.cf()?, key.as_bytes(), &value, &write_opts)
            .map_err(|err| {
                InstrumentError::store(
                    "put_cf",
                    CF_MEJEPA_Q4_SECURITY_LABELS,
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
                    CF_MEJEPA_Q4_SECURITY_LABELS,
                    err.to_string(),
                    "inspect RocksDB read permissions and column-family health",
                )
            })?
            .ok_or_else(|| {
                InstrumentError::store(
                    "read_after_write",
                    CF_MEJEPA_Q4_SECURITY_LABELS,
                    "Q4 security row missing after put_cf",
                    "do not advance reward-signal checkpoints until the CF row is readable",
                )
            })?;
        if readback != value {
            return Err(InstrumentError::store(
                "read_after_write",
                CF_MEJEPA_Q4_SECURITY_LABELS,
                "readback bytes differ from written Q4 security payload",
                "inspect RocksDB WAL/device integrity before continuing",
            ));
        }
        Ok(key)
    }

    fn cf(&self) -> InstrumentResult<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_MEJEPA_Q4_SECURITY_LABELS)
            .ok_or_else(|| {
                InstrumentError::store(
                    "cf_handle",
                    CF_MEJEPA_Q4_SECURITY_LABELS,
                    "column-family handle not found",
                    "open the store through Q4SecurityLabelStore::open",
                )
            })
    }
}

fn collect_detector_output(
    source: &Q4SecuritySource,
    output: &Q4SecurityToolOutput,
    findings: &mut Vec<Q4SecurityFinding>,
    quarantines: &mut Vec<Q4SecurityQuarantine>,
) -> InstrumentResult<()> {
    if output.runtime_exceeded {
        quarantines.push(quarantine(
            source,
            output.detector,
            output.phase,
            "Q4_SECURITY_RUNTIME_EXCEEDED",
            "analyzer exceeded the caller-enforced runtime limit",
        ));
        return Ok(());
    }
    if output.toolchain_missing {
        quarantines.push(quarantine(
            source,
            output.detector,
            output.phase,
            "Q4_SECURITY_TOOLCHAIN_MISSING",
            "required analyzer binary was unavailable",
        ));
        return Ok(());
    }
    let parsed = match output.detector {
        Q4SecurityDetector::Bandit => parse_bandit_output(source, output),
        Q4SecurityDetector::Semgrep => parse_semgrep_output(source, output),
    };
    match parsed {
        Ok(mut parsed_findings) => findings.append(&mut parsed_findings),
        Err(err) => quarantines.push(quarantine(
            source,
            output.detector,
            output.phase,
            "Q4_SECURITY_LABEL_PARSE_FAILURE",
            err,
        )),
    }
    Ok(())
}

fn parse_bandit_output(
    source: &Q4SecuritySource,
    output: &Q4SecurityToolOutput,
) -> Result<Vec<Q4SecurityFinding>, String> {
    let value: Value = serde_json::from_str(&output.stdout)
        .map_err(|err| format!("Bandit JSON parse failed: {err}"))?;
    reject_reported_errors("Bandit", &value)?;
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| "Bandit JSON missing results array".to_string())?;
    let mut findings = Vec::new();
    for result in results {
        let rule_id = string_field(result, "test_id").unwrap_or("bandit_unknown");
        let message = string_field(result, "issue_text").unwrap_or("Bandit finding");
        let severity = parse_bandit_severity(string_field(result, "issue_severity"));
        let start_line = u32_field(result, "line_number").unwrap_or(1).max(1);
        let end_line = result
            .get("line_range")
            .and_then(Value::as_array)
            .and_then(|lines| lines.iter().filter_map(value_to_u32).max())
            .unwrap_or(start_line)
            .max(start_line);
        let line_range = Q4SecurityLineRange {
            start_line,
            end_line,
            start_column: 1,
            end_column: 1,
        };
        findings.push(make_finding(
            source,
            Q4SecurityDetector::Bandit,
            rule_id,
            severity,
            line_range,
            message,
        ));
    }
    Ok(findings)
}

fn parse_semgrep_output(
    source: &Q4SecuritySource,
    output: &Q4SecurityToolOutput,
) -> Result<Vec<Q4SecurityFinding>, String> {
    let value: Value = serde_json::from_str(&output.stdout)
        .map_err(|err| format!("Semgrep JSON parse failed: {err}"))?;
    reject_reported_errors("Semgrep", &value)?;
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| "Semgrep JSON missing results array".to_string())?;
    let mut findings = Vec::new();
    for result in results {
        let rule_id = string_field(result, "check_id").unwrap_or("semgrep_unknown");
        let start = result.get("start").unwrap_or(&Value::Null);
        let end = result.get("end").unwrap_or(&Value::Null);
        let extra = result.get("extra").unwrap_or(&Value::Null);
        let message = extra
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Semgrep finding");
        let severity = parse_semgrep_severity(extra.get("severity").and_then(Value::as_str));
        let start_line = u32_field(start, "line").unwrap_or(1).max(1);
        let end_line = u32_field(end, "line").unwrap_or(start_line).max(start_line);
        let start_column = u32_field(start, "col").unwrap_or(1).max(1);
        let end_column = u32_field(end, "col")
            .unwrap_or(start_column)
            .max(start_column);
        let vuln_class = extra
            .get("metadata")
            .and_then(|metadata| metadata.get("vulnerability_class"))
            .map(vulnerability_class_text)
            .unwrap_or_default();
        let line_range = Q4SecurityLineRange {
            start_line,
            end_line,
            start_column,
            end_column,
        };
        findings.push(make_finding(
            source,
            Q4SecurityDetector::Semgrep,
            rule_id,
            severity,
            line_range,
            &format!("{message} {vuln_class}"),
        ));
    }
    Ok(findings)
}

fn diff_findings(
    source: &Q4SecuritySource,
    pre_findings: Vec<Q4SecurityFinding>,
    post_findings: Vec<Q4SecurityFinding>,
) -> InstrumentResult<Vec<Q4SecurityLabel>> {
    let pre = keyed_findings(pre_findings);
    let post = keyed_findings(post_findings);
    let mut keys = BTreeSet::new();
    keys.extend(pre.keys().cloned());
    keys.extend(post.keys().cloned());
    let mut labels = Vec::new();
    for key in keys {
        match (pre.get(&key), post.get(&key)) {
            (None, Some(finding)) => labels.push(label_from_finding(source, finding, true, false)),
            (Some(finding), None) => labels.push(label_from_finding(source, finding, false, true)),
            (Some(_), Some(_)) | (None, None) => {}
        }
    }
    labels.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line_range.cmp(&right.line_range))
            .then(left.rule_id.cmp(&right.rule_id))
            .then(left.detector.cmp(&right.detector))
    });
    Ok(labels)
}

fn keyed_findings(findings: Vec<Q4SecurityFinding>) -> BTreeMap<String, Q4SecurityFinding> {
    findings
        .into_iter()
        .map(|finding| (finding.finding_id.clone(), finding))
        .collect()
}

fn make_finding(
    source: &Q4SecuritySource,
    detector: Q4SecurityDetector,
    rule_id: &str,
    severity: Q4SecuritySeverity,
    line_range: Q4SecurityLineRange,
    message: &str,
) -> Q4SecurityFinding {
    let class = classify_security(rule_id, message);
    let finding_id = finding_id(
        detector,
        rule_id,
        &source.logical_path,
        class,
        &line_range,
        message,
    );
    Q4SecurityFinding {
        finding_id,
        rule_id: rule_id.to_string(),
        severity,
        class,
        file: source.logical_path.clone(),
        line_range,
        detector,
        message: compact_message(message),
    }
}

fn label_from_finding(
    source: &Q4SecuritySource,
    finding: &Q4SecurityFinding,
    introduced_by_patch: bool,
    fixed_by_patch: bool,
) -> Q4SecurityLabel {
    Q4SecurityLabel {
        corpus_row_id: source.corpus_row_id.clone(),
        chunk_id: source.chunk_id.clone(),
        finding_id: finding.finding_id.clone(),
        rule_id: finding.rule_id.clone(),
        severity: finding.severity,
        class: finding.class,
        file: finding.file.clone(),
        line_range: finding.line_range.clone(),
        detector: finding.detector,
        message: finding.message.clone(),
        introduced_by_patch,
        fixed_by_patch,
    }
}

fn classify_security(rule_id: &str, message: &str) -> Q4SecurityClass {
    let text = format!("{rule_id} {message}").to_ascii_lowercase();
    if text.contains("b602")
        || text.contains("b605")
        || text.contains("shell=true")
        || text.contains("command injection")
        || text.contains("os command")
    {
        Q4SecurityClass::CommandInjection
    } else if text.contains("b608") || text.contains("sql injection") {
        Q4SecurityClass::SqlInjection
    } else if text.contains("path traversal") || text.contains("directory traversal") {
        Q4SecurityClass::PathTraversal
    } else if text.contains("xss") || text.contains("cross-site scripting") {
        Q4SecurityClass::Xss
    } else if text.contains("csrf") {
        Q4SecurityClass::Csrf
    } else if text.contains("ssrf") {
        Q4SecurityClass::Ssrf
    } else if text.contains("pickle")
        || text.contains("yaml.load")
        || text.contains("deserial")
        || text.contains("b301")
    {
        Q4SecurityClass::Deserialization
    } else if text.contains("b105")
        || text.contains("b106")
        || text.contains("b107")
        || text.contains("hardcoded")
        || text.contains("secret")
        || text.contains("password")
        || text.contains("api key")
    {
        Q4SecurityClass::HardcodedSecret
    } else if text.contains("log") && text.contains("secret") {
        Q4SecurityClass::LoggingSecret
    } else if text.contains("md5")
        || text.contains("sha1")
        || text.contains("weak crypt")
        || text.contains("b324")
    {
        Q4SecurityClass::InsecureCryptoAlgo
    } else if text.contains("key length") || text.contains("short key") {
        Q4SecurityClass::InsufficientCryptoKeyLength
    } else if text.contains("missing auth") || text.contains("authentication") {
        Q4SecurityClass::MissingAuth
    } else if text.contains("access control") || text.contains("authorization") {
        Q4SecurityClass::BrokenAccessControl
    } else if text.contains("open redirect") {
        Q4SecurityClass::OpenRedirect
    } else if text.contains("tls") || text.contains("ssl") {
        Q4SecurityClass::MissingTlsVerify
    } else {
        Q4SecurityClass::Other
    }
}

fn run_detector_command(
    detector: Q4SecurityDetector,
    phase: Q4SecurityScanPhase,
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> InstrumentResult<Q4SecurityToolOutput> {
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
            return Ok(Q4SecurityToolOutput {
                detector,
                phase,
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
                "python-q4-security-labels-v1",
                err.to_string(),
                "inspect analyzer process state and caller timeout handling",
            )
        })? {
            Some(_) => {
                let output = child.wait_with_output().map_err(|err| {
                    InstrumentError::store(
                        "wait_with_output",
                        "python-q4-security-labels-v1",
                        err.to_string(),
                        "inspect analyzer process output handling",
                    )
                })?;
                return tool_output_from_process(detector, phase, command, output, false);
            }
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let output = child.wait_with_output().map_err(|err| {
                    InstrumentError::store(
                        "wait_timeout_output",
                        "python-q4-security-labels-v1",
                        err.to_string(),
                        "inspect analyzer timeout process cleanup",
                    )
                })?;
                return tool_output_from_process(detector, phase, command, output, true);
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn tool_output_from_process(
    detector: Q4SecurityDetector,
    phase: Q4SecurityScanPhase,
    command: Vec<String>,
    output: std::process::Output,
    runtime_exceeded: bool,
) -> InstrumentResult<Q4SecurityToolOutput> {
    Ok(Q4SecurityToolOutput {
        detector,
        phase,
        command,
        status_code: output.status.code(),
        stdout: String::from_utf8(output.stdout).map_err(|err| {
            InstrumentError::invalid(
                "q4_security.stdout",
                format!("analyzer stdout was not UTF-8: {err}"),
                "configure Bandit/Semgrep to emit UTF-8 JSON",
            )
        })?,
        stderr: String::from_utf8(output.stderr).map_err(|err| {
            InstrumentError::invalid(
                "q4_security.stderr",
                format!("analyzer stderr was not UTF-8: {err}"),
                "configure Bandit/Semgrep to emit UTF-8 diagnostics",
            )
        })?,
        runtime_exceeded,
        toolchain_missing: false,
    })
}

fn reject_reported_errors(tool: &str, value: &Value) -> Result<(), String> {
    if let Some(errors) = value.get("errors").and_then(Value::as_array) {
        if !errors.is_empty() {
            return Err(format!("{tool} reported analyzer errors: {errors:?}"));
        }
    }
    Ok(())
}

fn string_field<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn u32_field(value: &Value, field: &str) -> Option<u32> {
    value.get(field).and_then(value_to_u32)
}

fn value_to_u32(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|raw| u32::try_from(raw).ok())
}

fn parse_bandit_severity(raw: Option<&str>) -> Q4SecuritySeverity {
    match raw.unwrap_or("").to_ascii_uppercase().as_str() {
        "HIGH" => Q4SecuritySeverity::High,
        "MEDIUM" => Q4SecuritySeverity::Medium,
        "LOW" => Q4SecuritySeverity::Low,
        _ => Q4SecuritySeverity::Medium,
    }
}

fn parse_semgrep_severity(raw: Option<&str>) -> Q4SecuritySeverity {
    match raw.unwrap_or("").to_ascii_uppercase().as_str() {
        "ERROR" => Q4SecuritySeverity::High,
        "WARNING" => Q4SecuritySeverity::Medium,
        "INFO" | "INVENTORY" => Q4SecuritySeverity::Low,
        _ => Q4SecuritySeverity::Medium,
    }
}

fn vulnerability_class_text(value: &Value) -> String {
    match value {
        Value::String(raw) => raw.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

fn compact_message(message: &str) -> String {
    message
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(512)
        .collect()
}

fn finding_id(
    detector: Q4SecurityDetector,
    rule_id: &str,
    path: &str,
    class: Q4SecurityClass,
    line_range: &Q4SecurityLineRange,
    message: &str,
) -> String {
    let basis = format!(
        "{}:{rule_id}:{path}:{}:{}:{}:{}",
        detector.as_str(),
        line_range.start_line,
        line_range.end_line,
        class.as_str(),
        compact_message(message)
    );
    format!("{}:{:016x}", detector.as_str(), fnv1a64(basis.as_bytes()))
}

fn q4_security_record_key(record: &PersistedQ4SecuritySignal, value: &[u8]) -> String {
    match &record.signal {
        Q4SecuritySignalRecord::Label(label) => format!(
            "{}:label:{}:{}",
            label.corpus_row_id, label.finding_id, label.rule_id
        ),
        Q4SecuritySignalRecord::Quarantine(quarantine) => format!(
            "{}:quarantine:{}:{}:{:016x}",
            quarantine.corpus_row_id,
            quarantine.detector.as_str(),
            quarantine.reason_code,
            fnv1a64(value)
        ),
    }
}

fn quarantine(
    source: &Q4SecuritySource,
    detector: Q4SecurityDetector,
    phase: Q4SecurityScanPhase,
    reason_code: &str,
    detail: impl Into<String>,
) -> Q4SecurityQuarantine {
    Q4SecurityQuarantine {
        corpus_row_id: source.corpus_row_id.clone(),
        chunk_id: source.chunk_id.clone(),
        detector,
        phase,
        reason_code: reason_code.to_string(),
        detail: detail.into(),
    }
}

fn validate_source(source: &Q4SecuritySource) -> InstrumentResult<()> {
    validate_path_component("corpus_row_id", &source.corpus_row_id)?;
    validate_non_empty_single_line("chunk_id", &source.chunk_id)?;
    validate_non_empty_single_line("logical_path", &source.logical_path)?;
    validate_source_text("pre_patch_source", &source.pre_patch_source)?;
    validate_source_text("post_patch_source", &source.post_patch_source)?;
    Ok(())
}

fn validate_source_text(field: &'static str, text: &str) -> InstrumentResult<()> {
    if text.trim().is_empty() {
        return invalid(
            field,
            "source text is empty",
            "capture Python source text before SAST",
        );
    }
    if text.len() > MAX_SOURCE_BYTES {
        return invalid(
            field,
            format!(
                "source text length {} exceeds {MAX_SOURCE_BYTES}",
                text.len()
            ),
            "shard oversized Python source before SAST",
        );
    }
    if text.chars().any(|ch| ch == '\0') {
        return invalid(
            field,
            "source text contains a NUL byte",
            "store parser-facing Python source as valid UTF-8",
        );
    }
    Ok(())
}

fn validate_tool_output(
    output: &Q4SecurityToolOutput,
    expected: Q4SecurityDetector,
) -> InstrumentResult<()> {
    if output.detector != expected {
        return invalid(
            "q4_security.output.detector",
            format!(
                "tool output detector {:?} does not match expected {:?}",
                output.detector, expected
            ),
            "wire Bandit/Semgrep outputs to their matching parser",
        );
    }
    if output.command.is_empty() {
        return invalid(
            "q4_security.output.command",
            "tool output command provenance is empty",
            "persist analyzer command provenance with every raw output",
        );
    }
    Ok(())
}

fn validate_label(label: &Q4SecurityLabel, source: &Q4SecuritySource) -> InstrumentResult<()> {
    if label.corpus_row_id != source.corpus_row_id {
        return invalid(
            "q4_security.label.corpus_row_id",
            "label corpus_row_id does not match source",
            "persist Q4 security labels against the exact corpus row being scanned",
        );
    }
    validate_non_empty_single_line("q4_security.label.finding_id", &label.finding_id)?;
    validate_non_empty_single_line("q4_security.label.rule_id", &label.rule_id)?;
    validate_non_empty_single_line("q4_security.label.file", &label.file)?;
    validate_non_empty_single_line("q4_security.label.message", &label.message)?;
    if label.introduced_by_patch == label.fixed_by_patch {
        return invalid(
            "q4_security.label.delta_flags",
            "exactly one of introduced_by_patch or fixed_by_patch must be true",
            "drop unchanged SAST findings and preserve only patch deltas",
        );
    }
    if label.line_range.start_line == 0 || label.line_range.end_line < label.line_range.start_line {
        return invalid(
            "q4_security.label.line_range",
            "line range is not one-based and ordered",
            "normalize SAST source locations before persisting labels",
        );
    }
    Ok(())
}

fn validate_quarantine(
    quarantine: &Q4SecurityQuarantine,
    source: &Q4SecuritySource,
) -> InstrumentResult<()> {
    if quarantine.corpus_row_id != source.corpus_row_id {
        return invalid(
            "q4_security.quarantine.corpus_row_id",
            "quarantine corpus_row_id does not match source",
            "persist Q4 security quarantines against the exact corpus row being scanned",
        );
    }
    validate_non_empty_single_line(
        "q4_security.quarantine.reason_code",
        &quarantine.reason_code,
    )?;
    validate_non_empty_single_line("q4_security.quarantine.detail", &quarantine.detail)?;
    Ok(())
}

fn validate_file_path(field: &'static str, path: &Path) -> InstrumentResult<String> {
    if !path.is_file() {
        return invalid(
            field,
            format!("{} is not a readable file", path.display()),
            "materialize the Python corpus row file before running Q4 security analysis",
        );
    }
    let path_arg = path.to_str().ok_or_else(|| {
        InstrumentError::invalid(
            field,
            format!("{} is not valid UTF-8", path.display()),
            "security analyzer command paths must be valid UTF-8 for provenance",
        )
    })?;
    validate_no_nul(field, path_arg)?;
    Ok(path_arg.to_string())
}

fn validate_path_component(field: &'static str, value: &str) -> InstrumentResult<()> {
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

fn validate_non_empty_single_line(field: &'static str, value: &str) -> InstrumentResult<()> {
    if value.trim().is_empty() {
        return invalid(
            field,
            format!("{field} is empty or whitespace-only"),
            "persist non-empty UTF-8 provenance fields",
        );
    }
    validate_no_nul(field, value)?;
    if value.chars().any(|ch| ch.is_control()) {
        return invalid(
            field,
            format!("{field} contains a control character"),
            "persist single-line UTF-8 provenance fields",
        );
    }
    Ok(())
}

fn validate_no_nul(field: &'static str, value: &str) -> InstrumentResult<()> {
    if value.chars().any(|ch| ch == '\0') {
        return invalid(
            field,
            format!("{field} contains a NUL byte"),
            "reject NUL-containing analyzer provenance before spawning subprocesses",
        );
    }
    Ok(())
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
    opts.set_paranoid_checks(true);
    opts
}

fn write_raw_wrapper(
    path: &Path,
    pre: &Q4SecurityToolOutput,
    post: &Q4SecurityToolOutput,
) -> InstrumentResult<()> {
    let value = serde_json::json!({
        "schema_version": Q4_SECURITY_SCHEMA_VERSION,
        "detector": pre.detector,
        "pre_patch": pre,
        "post_patch": post,
    });
    let bytes = serde_json::to_vec_pretty(&value).map_err(|err| {
        InstrumentError::store(
            "serialize_raw_output",
            "python-q4-security-labels-v1",
            err.to_string(),
            "ensure raw Q4 security output wrappers remain JSON serializable",
        )
    })?;
    fs::write(path, &bytes).map_err(|err| {
        InstrumentError::store(
            "write_raw_output",
            "python-q4-security-labels-v1",
            err.to_string(),
            "ensure the D-root corpus raw-output directory is writable",
        )
    })?;
    let readback = fs::read(path).map_err(|err| {
        InstrumentError::store(
            "read_raw_output",
            "python-q4-security-labels-v1",
            err.to_string(),
            "read back raw analyzer output after writing it",
        )
    })?;
    if readback != bytes {
        return Err(InstrumentError::store(
            "read_after_write_raw_output",
            "python-q4-security-labels-v1",
            format!(
                "{} readback bytes differ from written bytes",
                path.display()
            ),
            "do not advance Q4 security label checkpoints until raw output is durable",
        ));
    }
    Ok(())
}

fn bundled_semgrep_config() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/q4_security_semgrep_python.yml"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn introduced_and_fixed_findings_are_deltas_unchanged_is_dropped() {
        let source = sample_source();
        let outputs = sample_outputs();
        let extraction = extract_q4_security_labels(&source, &outputs).unwrap();
        assert_eq!(extraction.labels.len(), 4);
        assert!(extraction.labels.iter().any(|label| {
            label.introduced_by_patch && label.class == Q4SecurityClass::CommandInjection
        }));
        assert!(extraction.labels.iter().any(|label| {
            label.fixed_by_patch && label.class == Q4SecurityClass::HardcodedSecret
        }));
        assert!(extraction.quarantines.is_empty());
        assert_eq!(extraction.static_analysis_input.diagnostics.len(), 4);
    }

    #[test]
    fn toolchain_missing_becomes_quarantine_not_empty_success() {
        let source = sample_source();
        let mut outputs = sample_outputs();
        outputs.bandit_post.toolchain_missing = true;
        outputs.bandit_post.stdout.clear();
        let extraction = extract_q4_security_labels(&source, &outputs).unwrap();
        assert!(extraction.quarantines.iter().any(|quarantine| {
            quarantine.reason_code == "Q4_SECURITY_TOOLCHAIN_MISSING"
                && quarantine.detector == Q4SecurityDetector::Bandit
        }));
    }

    #[test]
    fn analyzer_errors_become_parse_failure_quarantine() {
        let source = sample_source();
        let mut outputs = sample_outputs();
        outputs.semgrep_post.stdout =
            r#"{"results":[],"errors":[{"message":"parse failed"}]}"#.into();
        let extraction = extract_q4_security_labels(&source, &outputs).unwrap();
        assert!(extraction.quarantines.iter().any(|quarantine| {
            quarantine.reason_code == "Q4_SECURITY_LABEL_PARSE_FAILURE"
                && quarantine.detector == Q4SecurityDetector::Semgrep
        }));
    }

    fn sample_source() -> Q4SecuritySource {
        Q4SecuritySource {
            corpus_row_id: "row-security-001".into(),
            chunk_id: "chunk-security-001".into(),
            logical_path: "app/security.py".into(),
            pre_patch_source: "API_KEY = 'secret'\nprint('safe')\n".into(),
            post_patch_source: "import subprocess\nsubprocess.call(user_input, shell=True)\n"
                .into(),
        }
    }

    fn sample_outputs() -> Q4SecurityRawOutputs {
        Q4SecurityRawOutputs {
            bandit_pre: output(
                Q4SecurityDetector::Bandit,
                Q4SecurityScanPhase::PrePatch,
                bandit_json(&[("B105", "LOW", 1, "Possible hardcoded password: 'secret'")]),
            ),
            bandit_post: output(
                Q4SecurityDetector::Bandit,
                Q4SecurityScanPhase::PostPatch,
                bandit_json(&[(
                    "B602",
                    "HIGH",
                    2,
                    "subprocess call with shell=True identified, security issue.",
                )]),
            ),
            semgrep_pre: output(
                Q4SecurityDetector::Semgrep,
                Q4SecurityScanPhase::PrePatch,
                semgrep_json(&[(
                    "python.lang.security.audit.hardcoded-secret",
                    "WARNING",
                    1,
                    "Hardcoded secret",
                )]),
            ),
            semgrep_post: output(
                Q4SecurityDetector::Semgrep,
                Q4SecurityScanPhase::PostPatch,
                semgrep_json(&[(
                    "python.lang.security.audit.subprocess-shell-true",
                    "ERROR",
                    2,
                    "Possible command injection with shell=True",
                )]),
            ),
        }
    }

    fn output(
        detector: Q4SecurityDetector,
        phase: Q4SecurityScanPhase,
        stdout: String,
    ) -> Q4SecurityToolOutput {
        Q4SecurityToolOutput {
            detector,
            phase,
            command: vec![detector.as_str().into()],
            status_code: Some(0),
            stdout,
            stderr: String::new(),
            runtime_exceeded: false,
            toolchain_missing: false,
        }
    }

    fn bandit_json(items: &[(&str, &str, u32, &str)]) -> String {
        let results = items
            .iter()
            .map(|(rule, severity, line, message)| {
                serde_json::json!({
                    "filename": "app/security.py",
                    "issue_severity": severity,
                    "issue_text": message,
                    "line_number": line,
                    "line_range": [line],
                    "test_id": rule,
                    "test_name": "test"
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({"errors":[],"results":results}).to_string()
    }

    fn semgrep_json(items: &[(&str, &str, u32, &str)]) -> String {
        let results = items
            .iter()
            .map(|(rule, severity, line, message)| {
                serde_json::json!({
                    "check_id": rule,
                    "path": "app/security.py",
                    "start": {"line": line, "col": 1},
                    "end": {"line": line, "col": 44},
                    "extra": {
                        "message": message,
                        "severity": severity,
                        "metadata": {"vulnerability_class": ["Command Injection"]}
                    }
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({"errors":[],"results":results}).to_string()
    }
}
