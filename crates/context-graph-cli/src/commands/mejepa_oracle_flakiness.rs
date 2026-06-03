//! Phase G oracle flakiness audit and corpus quarantine support.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, ensure, Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::json;

pub const DEFAULT_CORPUS_QUARANTINE_PATH: &str = "config/corpus_quarantine.toml";
const MIN_REQUIRED_ORACLE_RUNS: usize = 2;
const DEFAULT_REQUIRED_ORACLE_RUNS: usize = 3;
const DEFAULT_FLAKINESS_THRESHOLD: f32 = 0.05;

#[derive(Args, Debug, Clone)]
pub struct OracleFlakinessAuditArgs {
    /// JSON file containing repeated oracle observations per task.
    #[arg(long)]
    pub input: PathBuf,

    /// TOML quarantine output path consumed by `mejepa train`.
    #[arg(
        long,
        env = "CONTEXTGRAPH_CORPUS_QUARANTINE",
        default_value = DEFAULT_CORPUS_QUARANTINE_PATH
    )]
    pub output: PathBuf,

    /// Minimum repeated oracle runs required per task.
    #[arg(long, default_value_t = DEFAULT_REQUIRED_ORACLE_RUNS)]
    pub required_runs: usize,

    /// Quarantine tasks whose measured flakiness rate is above this threshold.
    #[arg(long, default_value_t = DEFAULT_FLAKINESS_THRESHOLD)]
    pub threshold: f32,

    /// Operator or automation id recorded in the quarantine file.
    #[arg(long, default_value = "oracle-flakiness-audit")]
    pub operator_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OracleFlakinessAuditOutput {
    pub input_path: String,
    pub output_path: String,
    pub required_runs: usize,
    pub threshold: f32,
    pub total_tasks: usize,
    pub quarantined_task_count: usize,
    pub reports: Vec<OracleFlakinessTaskReport>,
    pub readback_equal: bool,
    pub source_of_truth: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OracleFlakinessTaskReport {
    pub audit_key: String,
    pub task_id: String,
    pub repo: Option<String>,
    pub category: Option<String>,
    pub run_count: usize,
    pub pass_count: usize,
    pub fail_count: usize,
    pub unique_verdict_count: usize,
    pub outcome_flakiness_rate: f32,
    pub verdict_hash_flakiness_rate: f32,
    pub flakiness_rate: f32,
    pub quarantined: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleFlakinessAuditInput {
    pub tasks: Vec<OracleTaskRuns>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleTaskRuns {
    pub task_id: String,
    #[serde(default)]
    pub repo: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    pub runs: Vec<OracleRunObservation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleRunObservation {
    pub run_id: String,
    pub oracle_all_passed: bool,
    pub oracle_verdict_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusQuarantineConfig {
    pub quarantined_tasks: Vec<CorpusQuarantineEntry>,
}

impl CorpusQuarantineConfig {
    pub fn validate(&self) -> Result<()> {
        let mut seen = BTreeSet::new();
        for entry in &self.quarantined_tasks {
            entry.validate()?;
            ensure!(
                seen.insert(entry.task_id.clone()),
                "MEJEPA_CORPUS_QUARANTINE_DUPLICATE_TASK: duplicate task_id {}",
                entry.task_id
            );
        }
        Ok(())
    }

    pub fn task_map(&self) -> Result<BTreeMap<String, CorpusQuarantineEntry>> {
        self.validate()?;
        Ok(self
            .quarantined_tasks
            .iter()
            .map(|entry| (entry.task_id.clone(), entry.clone()))
            .collect())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusQuarantineEntry {
    pub task_id: String,
    pub reason: String,
    pub flakiness_rate: f32,
    pub oracle_runs: usize,
    pub observed_outcomes: Vec<bool>,
    pub observed_verdict_sha256: Vec<String>,
    pub operator_id: String,
    pub created_unix_ms: i64,
}

impl CorpusQuarantineEntry {
    fn validate(&self) -> Result<()> {
        validate_task_id(&self.task_id)?;
        ensure!(
            !self.reason.trim().is_empty(),
            "MEJEPA_CORPUS_QUARANTINE_INVALID_REASON: {}",
            self.task_id
        );
        ensure!(
            self.flakiness_rate.is_finite() && (0.0..=1.0).contains(&self.flakiness_rate),
            "MEJEPA_CORPUS_QUARANTINE_INVALID_RATE: {}",
            self.task_id
        );
        ensure!(
            self.oracle_runs >= MIN_REQUIRED_ORACLE_RUNS,
            "MEJEPA_CORPUS_QUARANTINE_INSUFFICIENT_RUNS: {} has {}",
            self.task_id,
            self.oracle_runs
        );
        ensure!(
            self.observed_outcomes.len() == self.oracle_runs,
            "MEJEPA_CORPUS_QUARANTINE_OUTCOME_COUNT_MISMATCH: {}",
            self.task_id
        );
        ensure!(
            self.observed_verdict_sha256.len() == self.oracle_runs,
            "MEJEPA_CORPUS_QUARANTINE_HASH_COUNT_MISMATCH: {}",
            self.task_id
        );
        for hash in &self.observed_verdict_sha256 {
            validate_sha256(hash)?;
        }
        ensure!(
            !self.operator_id.trim().is_empty(),
            "MEJEPA_CORPUS_QUARANTINE_OPERATOR_MISSING: {}",
            self.task_id
        );
        ensure!(
            self.created_unix_ms > 0,
            "MEJEPA_CORPUS_QUARANTINE_CREATED_AT_INVALID: {}",
            self.task_id
        );
        Ok(())
    }
}

pub fn run_oracle_flakiness_audit(
    args: OracleFlakinessAuditArgs,
) -> Result<OracleFlakinessAuditOutput> {
    validate_audit_args(&args)?;
    let raw = fs::read_to_string(&args.input)
        .with_context(|| format!("read oracle flakiness input {}", args.input.display()))?;
    let input: OracleFlakinessAuditInput = serde_json::from_str(&raw)
        .with_context(|| format!("parse oracle flakiness input {}", args.input.display()))?;
    let created_unix_ms = chrono::Utc::now().timestamp_millis();
    let (reports, config) = build_quarantine_from_audit(
        &input,
        args.required_runs,
        args.threshold,
        &args.operator_id,
        created_unix_ms,
    )?;
    write_quarantine_config(&args.output, &config)?;
    let readback = load_corpus_quarantine(&args.output)?;
    let expected = config.task_map()?;
    Ok(OracleFlakinessAuditOutput {
        input_path: args.input.display().to_string(),
        output_path: args.output.display().to_string(),
        required_runs: args.required_runs,
        threshold: args.threshold,
        total_tasks: reports.len(),
        quarantined_task_count: config.quarantined_tasks.len(),
        reports,
        readback_equal: readback == expected,
        source_of_truth: json!({
            "quarantine_config": args.output,
            "input_observation_file": args.input,
            "format": "CorpusQuarantineConfig"
        }),
    })
}

pub fn load_corpus_quarantine(
    path: impl AsRef<Path>,
) -> Result<BTreeMap<String, CorpusQuarantineEntry>> {
    let path = path.as_ref();
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let raw = fs::read_to_string(path)
        .with_context(|| format!("read corpus quarantine config {}", path.display()))?;
    let config: CorpusQuarantineConfig = toml::from_str(&raw)
        .with_context(|| format!("parse corpus quarantine config {}", path.display()))?;
    config.task_map()
}

pub fn write_quarantine_config(path: &Path, config: &CorpusQuarantineConfig) -> Result<()> {
    config.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create quarantine parent {}", parent.display()))?;
    }
    let bytes = toml::to_string_pretty(config).context("serialize corpus quarantine config")?;
    fs::write(path, bytes.as_bytes())
        .with_context(|| format!("write corpus quarantine config {}", path.display()))?;
    let readback = fs::read(path)
        .with_context(|| format!("read back corpus quarantine config {}", path.display()))?;
    ensure!(
        readback == bytes.as_bytes(),
        "MEJEPA_CORPUS_QUARANTINE_READBACK_MISMATCH: {}",
        path.display()
    );
    Ok(())
}

pub fn build_quarantine_from_audit(
    input: &OracleFlakinessAuditInput,
    required_runs: usize,
    threshold: f32,
    operator_id: &str,
    created_unix_ms: i64,
) -> Result<(Vec<OracleFlakinessTaskReport>, CorpusQuarantineConfig)> {
    ensure!(
        required_runs >= MIN_REQUIRED_ORACLE_RUNS,
        "MEJEPA_ORACLE_FLAKINESS_REQUIRED_RUNS_INVALID: required_runs must be >= 2"
    );
    ensure!(
        threshold.is_finite() && (0.0..1.0).contains(&threshold),
        "MEJEPA_ORACLE_FLAKINESS_THRESHOLD_INVALID: threshold must be finite in [0,1)"
    );
    ensure!(
        !operator_id.trim().is_empty(),
        "MEJEPA_ORACLE_FLAKINESS_OPERATOR_MISSING"
    );
    ensure!(
        created_unix_ms > 0,
        "MEJEPA_ORACLE_FLAKINESS_CREATED_AT_INVALID"
    );
    let mut seen_rows = BTreeSet::new();
    let mut reports = Vec::new();
    let mut quarantined_by_task = BTreeMap::new();
    for task in &input.tasks {
        validate_task_id(&task.task_id)?;
        validate_optional_audit_field("repo", task.repo.as_deref())?;
        validate_optional_audit_field("category", task.category.as_deref())?;
        let audit_key = oracle_task_audit_key(task)?;
        ensure!(
            seen_rows.insert(audit_key.clone()),
            "MEJEPA_ORACLE_FLAKINESS_DUPLICATE_ROW: {}",
            audit_key
        );
        let report = classify_task(task, required_runs, threshold)?;
        if report.quarantined {
            let entry = CorpusQuarantineEntry {
                task_id: task.task_id.clone(),
                reason: report.reason.clone(),
                flakiness_rate: report.flakiness_rate,
                oracle_runs: report.run_count,
                observed_outcomes: task.runs.iter().map(|run| run.oracle_all_passed).collect(),
                observed_verdict_sha256: task
                    .runs
                    .iter()
                    .map(|run| normalize_sha256(&run.oracle_verdict_sha256))
                    .collect::<Result<Vec<_>>>()?,
                operator_id: operator_id.to_string(),
                created_unix_ms,
            };
            quarantined_by_task
                .entry(task.task_id.clone())
                .and_modify(|existing: &mut CorpusQuarantineEntry| {
                    if entry.flakiness_rate > existing.flakiness_rate {
                        *existing = entry.clone();
                    }
                })
                .or_insert(entry);
        }
        reports.push(report);
    }
    let config = CorpusQuarantineConfig {
        quarantined_tasks: quarantined_by_task.into_values().collect(),
    };
    config.validate()?;
    Ok((reports, config))
}

fn classify_task(
    task: &OracleTaskRuns,
    required_runs: usize,
    threshold: f32,
) -> Result<OracleFlakinessTaskReport> {
    let audit_key = oracle_task_audit_key(task)?;
    ensure!(
        task.runs.len() >= required_runs,
        "MEJEPA_ORACLE_FLAKINESS_INSUFFICIENT_RUNS: {} has {} runs, expected at least {}",
        audit_key,
        task.runs.len(),
        required_runs
    );
    let mut run_ids = BTreeSet::new();
    let mut pass_count = 0usize;
    let mut fail_count = 0usize;
    let mut verdict_counts = BTreeMap::<String, usize>::new();
    for run in &task.runs {
        ensure!(
            !run.run_id.trim().is_empty(),
            "MEJEPA_ORACLE_FLAKINESS_RUN_ID_EMPTY: {}",
            audit_key
        );
        ensure!(
            run_ids.insert(run.run_id.clone()),
            "MEJEPA_ORACLE_FLAKINESS_DUPLICATE_RUN: {} {}",
            audit_key,
            run.run_id
        );
        if run.oracle_all_passed {
            pass_count += 1;
        } else {
            fail_count += 1;
        }
        let verdict_hash = normalize_sha256(&run.oracle_verdict_sha256)?;
        *verdict_counts.entry(verdict_hash).or_insert(0) += 1;
    }
    let run_count = task.runs.len();
    let outcome_majority = pass_count.max(fail_count);
    let verdict_majority = verdict_counts.values().copied().max().unwrap_or(0);
    let outcome_flakiness_rate = (run_count - outcome_majority) as f32 / run_count as f32;
    let verdict_hash_flakiness_rate = (run_count - verdict_majority) as f32 / run_count as f32;
    let flakiness_rate = outcome_flakiness_rate.max(verdict_hash_flakiness_rate);
    let quarantined = flakiness_rate > threshold;
    let reason = if quarantined {
        format!(
            "oracle flakiness {:.6} > threshold {:.6} over {} runs for {}",
            flakiness_rate, threshold, run_count, audit_key
        )
    } else {
        format!(
            "oracle flakiness {:.6} <= threshold {:.6} over {} runs for {}",
            flakiness_rate, threshold, run_count, audit_key
        )
    };
    Ok(OracleFlakinessTaskReport {
        audit_key,
        task_id: task.task_id.clone(),
        repo: task.repo.clone(),
        category: task.category.clone(),
        run_count,
        pass_count,
        fail_count,
        unique_verdict_count: verdict_counts.len(),
        outcome_flakiness_rate,
        verdict_hash_flakiness_rate,
        flakiness_rate,
        quarantined,
        reason,
    })
}

fn validate_audit_args(args: &OracleFlakinessAuditArgs) -> Result<()> {
    ensure!(
        args.input.exists(),
        "MEJEPA_ORACLE_FLAKINESS_INPUT_MISSING: {}",
        args.input.display()
    );
    ensure!(
        args.required_runs >= MIN_REQUIRED_ORACLE_RUNS,
        "MEJEPA_ORACLE_FLAKINESS_REQUIRED_RUNS_INVALID"
    );
    ensure!(
        args.threshold.is_finite() && (0.0..1.0).contains(&args.threshold),
        "MEJEPA_ORACLE_FLAKINESS_THRESHOLD_INVALID"
    );
    ensure!(
        !args.operator_id.trim().is_empty(),
        "MEJEPA_ORACLE_FLAKINESS_OPERATOR_MISSING"
    );
    Ok(())
}

fn validate_task_id(task_id: &str) -> Result<()> {
    ensure!(
        !task_id.trim().is_empty() && task_id.trim() == task_id,
        "MEJEPA_CORPUS_TASK_ID_INVALID: task_id must be non-empty with no surrounding whitespace"
    );
    ensure!(
        !task_id
            .bytes()
            .any(|byte| byte == 0 || byte == b'\n' || byte == b'\r'),
        "MEJEPA_CORPUS_TASK_ID_INVALID: task_id contains control characters"
    );
    Ok(())
}

fn validate_optional_audit_field(field_name: &str, value: Option<&str>) -> Result<()> {
    let Some(value) = value else {
        return Ok(());
    };
    ensure!(
        !value.trim().is_empty() && value.trim() == value,
        "MEJEPA_ORACLE_FLAKINESS_INVALID_FIELD: {field_name} must be non-empty with no surrounding whitespace when present"
    );
    ensure!(
        !value
            .bytes()
            .any(|byte| byte == 0 || byte == b'\n' || byte == b'\r'),
        "MEJEPA_ORACLE_FLAKINESS_INVALID_FIELD: {field_name} contains control characters"
    );
    Ok(())
}

fn oracle_task_audit_key(task: &OracleTaskRuns) -> Result<String> {
    validate_task_id(&task.task_id)?;
    validate_optional_audit_field("category", task.category.as_deref())?;
    Ok(match task.category.as_deref() {
        Some(category) => format!("{}|{}", task.task_id, category),
        None => task.task_id.clone(),
    })
}

fn validate_sha256(value: &str) -> Result<()> {
    normalize_sha256(value).map(|_| ())
}

fn normalize_sha256(value: &str) -> Result<String> {
    let stripped = value.strip_prefix("sha256:").unwrap_or(value);
    ensure!(
        stripped.len() == 64 && stripped.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "MEJEPA_ORACLE_FLAKINESS_VERDICT_HASH_INVALID: {value}"
    );
    Ok(stripped.to_ascii_lowercase())
}

pub fn quarantine_load_error(path: &Path, err: anyhow::Error) -> anyhow::Error {
    anyhow!(
        "MEJEPA_CORPUS_QUARANTINE_LOAD_FAILED: path={} error={err:#}",
        path.display()
    )
}

#[cfg(test)]
mod tests;
