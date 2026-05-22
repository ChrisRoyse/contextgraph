//! Phase 0 oracle execution and physical RocksDB source-of-truth storage.
//!
//! The oracle contract is intentionally strict:
//! - runner success is not inferred from an exit code alone;
//! - a JSON report file must be created by the runner and parsed separately;
//! - mutation outcomes and oracle verdicts are persisted in named RocksDB
//!   column families and can be read back independently for FSV.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use context_graph_mejepa_instruments::{
    ExceptionClass, OracleVerdict, PerTestOutcome, TestOutcome,
};
use rocksdb::{
    ColumnFamily, ColumnFamilyDescriptor, Direction, IteratorMode, Options, WriteBatch, DB,
};
use serde_json::Value;
use thiserror::Error;

use crate::{CorpusProvenance, MutationCategory, MutationOutcome};

pub type OracleResult<T> = Result<T, OracleError>;

pub const CF_MEJEPA_CORPUS_MUTATION_OUTCOMES: &str = "mejepa_mutation_corpus";
pub const CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON: &str = "mejepa_oracle_verdicts";
pub const CF_MEJEPA_CORPUS_PROVENANCE_JSON: &str = "mejepa_corpus_provenance";
pub const MEJEPA_ORACLE_CFS: &[&str] = &[
    CF_MEJEPA_CORPUS_MUTATION_OUTCOMES,
    CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON,
    CF_MEJEPA_CORPUS_PROVENANCE_JSON,
];

#[derive(Debug, Error)]
pub enum OracleError {
    #[error("oracle input invalid at {field}: {message}; remediation: {remediation}")]
    InvalidInput {
        field: &'static str,
        message: String,
        remediation: &'static str,
    },
    #[error("oracle I/O failed at {path}: {message}; remediation: {remediation}")]
    Io {
        path: String,
        message: String,
        remediation: &'static str,
    },
    #[error("oracle command failed for {program}: {message}; remediation: {remediation}")]
    CommandFailed {
        program: String,
        message: String,
        remediation: &'static str,
    },
    #[error("oracle command timed out for {program}: {message}; remediation: {remediation}")]
    Timeout {
        program: String,
        message: String,
        remediation: &'static str,
    },
    #[error("oracle command interrupted for {program}: {message}; remediation: {remediation}")]
    Interrupted {
        program: String,
        message: String,
        remediation: &'static str,
    },
    #[error("oracle JSON parse failed at {path}: {message}; remediation: {remediation}")]
    JsonParse {
        path: String,
        message: String,
        remediation: &'static str,
    },
    #[error("oracle report invalid at {field}: {message}; remediation: {remediation}")]
    ReportInvalid {
        field: &'static str,
        message: String,
        remediation: &'static str,
    },
    #[error(
        "oracle RocksDB store failed at {operation}/{cf}: {message}; remediation: {remediation}"
    )]
    Store {
        operation: &'static str,
        cf: &'static str,
        message: String,
        remediation: &'static str,
    },
    #[error(
        "oracle Docker run_id is unsafe at position {position}: {run_id:?} contains {character:?}; remediation: {remediation}"
    )]
    DockerRunIdUnsafe {
        run_id: String,
        character: char,
        position: usize,
        remediation: &'static str,
    },
}

impl OracleError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "MEJEPA_ORACLE_INVALID_INPUT",
            Self::Io { .. } => "MEJEPA_ORACLE_IO",
            Self::CommandFailed { .. } => "MEJEPA_ORACLE_COMMAND_FAILED",
            Self::Timeout { .. } => "MEJEPA_ORACLE_TIMEOUT",
            Self::Interrupted { .. } => "MEJEPA_ORACLE_INTERRUPTED",
            Self::JsonParse { .. } => "MEJEPA_ORACLE_JSON_PARSE",
            Self::ReportInvalid { .. } => "MEJEPA_ORACLE_REPORT_INVALID",
            Self::Store { .. } => "MEJEPA_ORACLE_STORE",
            Self::DockerRunIdUnsafe { .. } => "MEJEPA_CORPUS_DOCKER_RUN_ID_UNSAFE",
        }
    }

    pub(crate) fn invalid(
        field: &'static str,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::InvalidInput {
            field,
            message: message.into(),
            remediation,
        }
    }

    pub(crate) fn io(
        path: impl AsRef<Path>,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::Io {
            path: path.as_ref().display().to_string(),
            message: message.into(),
            remediation,
        }
    }

    pub(crate) fn command_failed(
        program: impl Into<String>,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::CommandFailed {
            program: program.into(),
            message: message.into(),
            remediation,
        }
    }

    pub(crate) fn timeout(
        program: impl Into<String>,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::Timeout {
            program: program.into(),
            message: message.into(),
            remediation,
        }
    }

    pub(crate) fn interrupted(
        program: impl Into<String>,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::Interrupted {
            program: program.into(),
            message: message.into(),
            remediation,
        }
    }

    pub(crate) fn json_parse(
        path: impl Into<String>,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::JsonParse {
            path: path.into(),
            message: message.into(),
            remediation,
        }
    }

    pub(crate) fn report_invalid(
        field: &'static str,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::ReportInvalid {
            field,
            message: message.into(),
            remediation,
        }
    }

    pub(crate) fn store(
        operation: &'static str,
        cf: &'static str,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::Store {
            operation,
            cf,
            message: message.into(),
            remediation,
        }
    }

    pub(crate) fn docker_run_id_unsafe(
        run_id: impl Into<String>,
        character: char,
        position: usize,
        remediation: &'static str,
    ) -> Self {
        Self::DockerRunIdUnsafe {
            run_id: run_id.into(),
            character,
            position,
            remediation,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandOracleConfig {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub report_path: PathBuf,
    pub timeout: Duration,
    pub accepted_exit_codes: Vec<i32>,
}

#[derive(Debug, Clone)]
pub struct DockerMount {
    pub host_path: PathBuf,
    pub container_path: String,
    pub readonly: bool,
}

#[derive(Debug, Clone)]
pub struct DockerOracleConfig {
    pub image: String,
    pub mounts: Vec<DockerMount>,
    pub command: Vec<String>,
    pub report_path_host: PathBuf,
    pub timeout: Duration,
    pub accepted_exit_codes: Vec<i32>,
    pub network_none: bool,
    pub read_only_root: bool,
}

/// Parse a pytest-json-report shaped JSON document into the canonical
/// E_Oracle source record. Summary totals are cross-checked against per-test
/// rows so stale or truncated reports fail closed.
pub fn parse_pytest_json_report(report_text: &str) -> OracleResult<OracleVerdict> {
    if report_text.trim().is_empty() {
        return Err(OracleError::json_parse(
            "<inline-report>",
            "report text is empty",
            "configure the runner to write a non-empty JSON report before parsing",
        ));
    }
    let root: Value = serde_json::from_str(report_text).map_err(|err| {
        OracleError::json_parse(
            "<inline-report>",
            err.to_string(),
            "write valid JSON from the test runner and preserve the report file for inspection",
        )
    })?;
    parse_pytest_json_value(&root)
}

/// Execute a command, then read and parse the report file it was configured to
/// write. The report file must not exist before execution; this prevents stale
/// report reuse from masking runner failures.
pub fn run_command_json_oracle(config: &CommandOracleConfig) -> OracleResult<OracleVerdict> {
    validate_command_config(config)?;
    let mut command = Command::new(&config.program);
    command
        .args(&config.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(cwd) = &config.cwd {
        command.current_dir(cwd);
    }
    for (key, value) in &config.env {
        validate_env_pair(key, value)?;
        command.env(key, value);
    }
    configure_child_process_group(&mut command);

    let child = command.spawn().map_err(|err| {
        OracleError::command_failed(
            config.program.clone(),
            format!("failed to spawn command: {err}"),
            "verify the oracle executable exists and is executable in this environment",
        )
    })?;
    let output = wait_with_timeout(child, config.timeout, &config.program)?;

    let exit_code = output.status.code().ok_or_else(|| {
        OracleError::command_failed(
            config.program.clone(),
            format!(
                "process terminated by signal; stderr_tail={}",
                tail_text(&output.stderr)
            ),
            "fix the runner crash before accepting oracle output",
        )
    })?;
    if !config.accepted_exit_codes.contains(&exit_code) {
        return Err(OracleError::command_failed(
            config.program.clone(),
            format!(
                "exit_code={exit_code}, accepted={:?}, stdout_tail={}, stderr_tail={}",
                config.accepted_exit_codes,
                tail_text(&output.stdout),
                tail_text(&output.stderr),
            ),
            "include expected test-failure exit codes explicitly or fix the runner failure",
        ));
    }

    read_report_path(&config.report_path)
}

/// Run a Dockerized oracle command. Docker availability is checked first and
/// the final verdict still comes only from the host-side report file.
pub fn run_docker_json_oracle(config: &DockerOracleConfig) -> OracleResult<OracleVerdict> {
    validate_docker_config(config)?;
    verify_docker_available(config.timeout)?;

    let mut args = vec!["run".to_string(), "--rm".to_string()];
    if config.network_none {
        args.push("--network".to_string());
        args.push("none".to_string());
    }
    if config.read_only_root {
        args.push("--read-only".to_string());
        args.push("--tmpfs".to_string());
        args.push("/tmp:rw,noexec,nosuid,size=1g".to_string());
    }
    for mount in &config.mounts {
        args.push("--mount".to_string());
        args.push(docker_mount_arg(mount)?);
    }
    args.push(config.image.clone());
    args.extend(config.command.iter().cloned());

    let command_config = CommandOracleConfig {
        program: "docker".to_string(),
        args,
        cwd: None,
        env: vec![],
        report_path: config.report_path_host.clone(),
        timeout: config.timeout,
        accepted_exit_codes: config.accepted_exit_codes.clone(),
    };
    run_command_json_oracle(&command_config)
}

pub struct OracleStore {
    db: DB,
}

impl OracleStore {
    pub fn open(path: impl AsRef<Path>) -> OracleResult<Self> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_paranoid_checks(true);

        let descriptors = MEJEPA_ORACLE_CFS
            .iter()
            .map(|name| ColumnFamilyDescriptor::new(*name, cf_options()))
            .collect::<Vec<_>>();
        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), descriptors).map_err(|err| {
            OracleError::store(
                "open",
                "<all>",
                err.to_string(),
                "inspect the RocksDB path, lock ownership, and column-family metadata",
            )
        })?;
        for cf in MEJEPA_ORACLE_CFS {
            if db.cf_handle(cf).is_none() {
                return Err(OracleError::store(
                    "open",
                    cf,
                    "column family missing after RocksDB open",
                    "open the ME-JEPA oracle store with the canonical column-family descriptors",
                ));
            }
        }
        Ok(Self { db })
    }

    pub fn put_corpus_row(
        &self,
        task_id: &str,
        category: MutationCategory,
        outcome: &MutationOutcome,
        verdict: &OracleVerdict,
    ) -> OracleResult<()> {
        let key = corpus_key(task_id, category)?;
        let mutation_bytes = serde_json::to_vec(outcome).map_err(|err| {
            OracleError::store(
                "serialize",
                CF_MEJEPA_CORPUS_MUTATION_OUTCOMES,
                err.to_string(),
                "ensure MutationOutcome remains JSON-serializable",
            )
        })?;
        let verdict_bytes = serde_json::to_vec(verdict).map_err(|err| {
            OracleError::store(
                "serialize",
                CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON,
                err.to_string(),
                "ensure OracleVerdict remains JSON-serializable",
            )
        })?;
        let mut batch = WriteBatch::default();
        batch.put_cf(
            self.cf(CF_MEJEPA_CORPUS_MUTATION_OUTCOMES)?,
            key.as_bytes(),
            mutation_bytes,
        );
        batch.put_cf(
            self.cf(CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON)?,
            key.as_bytes(),
            verdict_bytes,
        );
        self.db.write(batch).map_err(|err| {
            OracleError::store(
                "write_batch",
                "<corpus_row>",
                err.to_string(),
                "inspect RocksDB write permissions, WAL state, and disk capacity",
            )
        })
    }

    pub fn get_mutation(
        &self,
        task_id: &str,
        category: MutationCategory,
    ) -> OracleResult<Option<MutationOutcome>> {
        let key = corpus_key(task_id, category)?;
        let Some(bytes) = self
            .db
            .get_cf(self.cf(CF_MEJEPA_CORPUS_MUTATION_OUTCOMES)?, key.as_bytes())
            .map_err(|err| {
                OracleError::store(
                    "get",
                    CF_MEJEPA_CORPUS_MUTATION_OUTCOMES,
                    err.to_string(),
                    "inspect RocksDB read permissions and column-family health",
                )
            })?
        else {
            return Ok(None);
        };
        let outcome = serde_json::from_slice(&bytes).map_err(|err| {
            OracleError::store(
                "deserialize",
                CF_MEJEPA_CORPUS_MUTATION_OUTCOMES,
                err.to_string(),
                "do not mutate persisted ME-JEPA mutation records outside this API",
            )
        })?;
        Ok(Some(outcome))
    }

    pub fn get_corpus_row(
        &self,
        task_id: &str,
        category: MutationCategory,
    ) -> OracleResult<Option<(MutationOutcome, OracleVerdict)>> {
        let mutation = self.get_mutation(task_id, category)?;
        let verdict = self.get_verdict(task_id, category)?;
        match (mutation, verdict) {
            (Some(mutation), Some(verdict)) => Ok(Some((mutation, verdict))),
            (None, None) => Ok(None),
            (Some(_), None) => Err(OracleError::store(
                "get_corpus_row",
                "<corpus_row>",
                format!(
                    "partial corpus row for task_id={task_id} category={}; mutation exists but verdict is missing",
                    category.slug()
                ),
                "never write mutation/verdict separately; rebuild this corpus with put_corpus_row",
            )),
            (None, Some(_)) => Err(OracleError::store(
                "get_corpus_row",
                "<corpus_row>",
                format!(
                    "partial corpus row for task_id={task_id} category={}; verdict exists but mutation is missing",
                    category.slug()
                ),
                "never write mutation/verdict separately; rebuild this corpus with put_corpus_row",
            )),
        }
    }

    pub fn get_verdict(
        &self,
        task_id: &str,
        category: MutationCategory,
    ) -> OracleResult<Option<OracleVerdict>> {
        let key = corpus_key(task_id, category)?;
        let Some(bytes) = self
            .db
            .get_cf(
                self.cf(CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON)?,
                key.as_bytes(),
            )
            .map_err(|err| {
                OracleError::store(
                    "get",
                    CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON,
                    err.to_string(),
                    "inspect RocksDB read permissions and column-family health",
                )
            })?
        else {
            return Ok(None);
        };
        let verdict = serde_json::from_slice(&bytes).map_err(|err| {
            OracleError::store(
                "deserialize",
                CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON,
                err.to_string(),
                "do not mutate persisted ME-JEPA oracle records outside this API",
            )
        })?;
        Ok(Some(verdict))
    }

    pub fn iter_corpus_rows(
        &self,
    ) -> OracleResult<Vec<(String, MutationCategory, MutationOutcome, OracleVerdict)>> {
        let cf = self.cf(CF_MEJEPA_CORPUS_MUTATION_OUTCOMES)?;
        let mut rows = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, mutation_bytes) = item.map_err(|err| {
                OracleError::store(
                    "iterate",
                    CF_MEJEPA_CORPUS_MUTATION_OUTCOMES,
                    err.to_string(),
                    "inspect RocksDB iterator state and column-family health",
                )
            })?;
            let key_text = String::from_utf8(key.to_vec()).map_err(|err| {
                OracleError::store(
                    "decode_key",
                    CF_MEJEPA_CORPUS_MUTATION_OUTCOMES,
                    err.to_string(),
                    "only use UTF-8 ME-JEPA corpus keys",
                )
            })?;
            let (task_id, category) = parse_corpus_key(&key_text)?;
            let mutation = serde_json::from_slice(&mutation_bytes).map_err(|err| {
                OracleError::store(
                    "deserialize",
                    CF_MEJEPA_CORPUS_MUTATION_OUTCOMES,
                    err.to_string(),
                    "do not mutate persisted ME-JEPA mutation records outside this API",
                )
            })?;
            let verdict = self.get_verdict(&task_id, category)?.ok_or_else(|| {
                OracleError::store(
                    "iter_corpus_rows",
                    "<corpus_row>",
                    format!(
                        "verdict row missing for mutation key task_id={task_id} category={}",
                        category.slug()
                    ),
                    "rebuild this corpus because mutation/verdict rows must be atomic",
                )
            })?;
            rows.push((task_id, category, mutation, verdict));
        }
        Ok(rows)
    }

    pub fn put_provenance(&self, provenance: &CorpusProvenance) -> OracleResult<()> {
        provenance.validate().map_err(|err| {
            OracleError::store(
                "validate",
                CF_MEJEPA_CORPUS_PROVENANCE_JSON,
                format!("{err} ({})", err.code()),
                "compute complete corpus provenance before writing it",
            )
        })?;
        if self
            .get_provenance(&provenance.corpus_version, &provenance.embedder_version)?
            .is_some()
        {
            return Err(OracleError::store(
                "put",
                CF_MEJEPA_CORPUS_PROVENANCE_JSON,
                format!(
                    "provenance already exists for corpus_version={} embedder_version={}",
                    provenance.corpus_version, provenance.embedder_version
                ),
                "write provenance exactly once after successful corpus generation",
            ));
        }
        let key = provenance_key(&provenance.corpus_version, &provenance.embedder_version)?;
        let bytes = serde_json::to_vec(provenance).map_err(|err| {
            OracleError::store(
                "serialize",
                CF_MEJEPA_CORPUS_PROVENANCE_JSON,
                err.to_string(),
                "ensure CorpusProvenance remains JSON-serializable",
            )
        })?;
        self.db
            .put_cf(
                self.cf(CF_MEJEPA_CORPUS_PROVENANCE_JSON)?,
                key.as_bytes(),
                bytes,
            )
            .map_err(|err| {
                OracleError::store(
                    "put",
                    CF_MEJEPA_CORPUS_PROVENANCE_JSON,
                    err.to_string(),
                    "inspect RocksDB write permissions and disk capacity",
                )
            })
    }

    pub fn get_provenance(
        &self,
        corpus_version: &str,
        embedder_version: &str,
    ) -> OracleResult<Option<CorpusProvenance>> {
        let key = provenance_key(corpus_version, embedder_version)?;
        let Some(bytes) = self
            .db
            .get_cf(self.cf(CF_MEJEPA_CORPUS_PROVENANCE_JSON)?, key.as_bytes())
            .map_err(|err| {
                OracleError::store(
                    "get",
                    CF_MEJEPA_CORPUS_PROVENANCE_JSON,
                    err.to_string(),
                    "inspect RocksDB read permissions and column-family health",
                )
            })?
        else {
            return Ok(None);
        };
        let provenance = serde_json::from_slice(&bytes).map_err(|err| {
            OracleError::store(
                "deserialize",
                CF_MEJEPA_CORPUS_PROVENANCE_JSON,
                err.to_string(),
                "do not mutate persisted ME-JEPA provenance records outside this API",
            )
        })?;
        Ok(Some(provenance))
    }

    pub fn count_cf(&self, cf_name: &'static str) -> OracleResult<usize> {
        let cf = self.cf(cf_name)?;
        let mut count = 0usize;
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            item.map_err(|err| {
                OracleError::store(
                    "iterate",
                    cf_name,
                    err.to_string(),
                    "inspect RocksDB iterator state and column-family health",
                )
            })?;
            count += 1;
        }
        Ok(count)
    }

    pub fn scan_cf_json(&self, cf_name: &'static str) -> OracleResult<Vec<(String, Value)>> {
        let cf = self.cf(cf_name)?;
        let mut rows = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                OracleError::store(
                    "iterate",
                    cf_name,
                    err.to_string(),
                    "inspect RocksDB iterator state and column-family health",
                )
            })?;
            let key = String::from_utf8(key.to_vec()).map_err(|err| {
                OracleError::store(
                    "decode_key",
                    cf_name,
                    err.to_string(),
                    "only use UTF-8 ME-JEPA oracle keys",
                )
            })?;
            let value = serde_json::from_slice(&value).map_err(|err| {
                OracleError::store(
                    "decode_value",
                    cf_name,
                    err.to_string(),
                    "only persist JSON ME-JEPA oracle records through this API",
                )
            })?;
            rows.push((key, value));
        }
        Ok(rows)
    }

    pub fn flush(&self) -> OracleResult<()> {
        self.db.flush().map_err(|err| {
            OracleError::store(
                "flush",
                "<all>",
                err.to_string(),
                "inspect RocksDB WAL and filesystem state",
            )
        })
    }

    fn cf(&self, name: &'static str) -> OracleResult<&ColumnFamily> {
        self.db.cf_handle(name).ok_or_else(|| {
            OracleError::store(
                "cf_handle",
                name,
                "column family handle not found",
                "open the store through OracleStore::open so all required CFs exist",
            )
        })
    }
}

fn parse_pytest_json_value(root: &Value) -> OracleResult<OracleVerdict> {
    let summary = root.get("summary").ok_or_else(|| {
        OracleError::report_invalid(
            "summary",
            "missing pytest summary object",
            "run pytest with a JSON-report producer that includes summary totals",
        )
    })?;
    if !summary.is_object() {
        return Err(OracleError::report_invalid(
            "summary",
            "summary must be a JSON object",
            "write pytest summary totals as an object",
        ));
    }
    let tests = root.get("tests").and_then(Value::as_array).ok_or_else(|| {
        OracleError::report_invalid(
            "tests",
            "missing tests array",
            "write every pytest node outcome into a tests array",
        )
    })?;

    let mut per_test = Vec::with_capacity(tests.len());
    let mut raw_counts = [0usize; 6];
    let mut exception = top_level_exception(root);
    for (idx, test) in tests.iter().enumerate() {
        let test_obj = test.as_object().ok_or_else(|| {
            OracleError::report_invalid(
                "tests[i]",
                format!("tests[{idx}] must be an object"),
                "write each pytest result row as an object",
            )
        })?;
        let test_id = test_obj
            .get("nodeid")
            .or_else(|| test_obj.get("test_id"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                OracleError::report_invalid(
                    "tests[i].nodeid",
                    format!("tests[{idx}] is missing nodeid/test_id"),
                    "preserve pytest node ids in the JSON report",
                )
            })?;
        validate_single_line_text("tests[i].nodeid", test_id)?;

        let outcome_text = test_obj
            .get("outcome")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                OracleError::report_invalid(
                    "tests[i].outcome",
                    format!("tests[{idx}] is missing outcome"),
                    "write pytest outcome strings for every node",
                )
            })?;
        let (outcome, raw_kind) = parse_outcome(outcome_text, idx)?;
        raw_counts[raw_kind.index()] += 1;

        if outcome == TestOutcome::Error && exception.is_none() {
            exception = exception_from_json_fragment(test.get("longrepr"))
                .or_else(|| exception_from_json_fragment(test.get("setup")))
                .or_else(|| exception_from_json_fragment(test.get("call")))
                .or(Some(ExceptionClass::Other));
        }
        per_test.push(PerTestOutcome {
            test_id: test_id.to_string(),
            outcome,
            runtime_ms: test_runtime_ms(test, idx)?,
        });
    }

    let summary_total = required_summary_usize(summary, "total")?;
    if summary_total != per_test.len() {
        return Err(OracleError::report_invalid(
            "summary.total",
            format!(
                "summary.total={summary_total} but tests array contains {} rows",
                per_test.len()
            ),
            "write a complete report and parse only after the runner closes the file",
        ));
    }
    validate_summary_count(
        summary,
        "passed",
        raw_counts[PytestOutcomeKind::Passed.index()],
    )?;
    validate_summary_count(
        summary,
        "failed",
        raw_counts[PytestOutcomeKind::Failed.index()],
    )?;
    validate_summary_count(
        summary,
        "skipped",
        raw_counts[PytestOutcomeKind::Skipped.index()],
    )?;
    validate_error_summary_count(summary, raw_counts[PytestOutcomeKind::Error.index()])?;
    validate_summary_count(
        summary,
        "xfailed",
        raw_counts[PytestOutcomeKind::XFailed.index()],
    )?;
    validate_summary_count(
        summary,
        "xpassed",
        raw_counts[PytestOutcomeKind::XPassed.index()],
    )?;

    let summary_errors = optional_summary_usize(summary, "errors")?
        .or(optional_summary_usize(summary, "error")?)
        .unwrap_or(0);
    if summary_errors > 0 && exception.is_none() {
        exception = top_level_exception(root).or(Some(ExceptionClass::Other));
    }

    if per_test.is_empty() && exception.is_none() {
        return Err(OracleError::report_invalid(
            "tests",
            "report contains no tests and no detectable exception",
            "record no-tests-collected as exception=Other or fix the report producer",
        ));
    }

    Ok(OracleVerdict {
        per_test,
        exception,
        evidence_unavailable: false,
    })
}

fn validate_command_config(config: &CommandOracleConfig) -> OracleResult<()> {
    validate_program_text("program", &config.program)?;
    if config.report_path.as_os_str().is_empty() {
        return Err(OracleError::invalid(
            "report_path",
            "report_path is empty",
            "pass a unique report path for this oracle run",
        ));
    }
    if config.report_path.exists() {
        return Err(OracleError::invalid(
            "report_path",
            format!(
                "report_path already exists before runner execution: {}",
                config.report_path.display()
            ),
            "delete stale reports and use a unique report path per run",
        ));
    }
    if config.timeout.is_zero() {
        return Err(OracleError::invalid(
            "timeout",
            "timeout must be greater than zero",
            "set an explicit finite oracle timeout",
        ));
    }
    if config.accepted_exit_codes.is_empty() {
        return Err(OracleError::invalid(
            "accepted_exit_codes",
            "accepted_exit_codes must be non-empty",
            "explicitly include every runner exit code that still produces a valid report",
        ));
    }
    if let Some(cwd) = &config.cwd {
        if !cwd.is_dir() {
            return Err(OracleError::invalid(
                "cwd",
                format!(
                    "cwd does not exist or is not a directory: {}",
                    cwd.display()
                ),
                "create the oracle working directory before running the command",
            ));
        }
    }
    Ok(())
}

fn validate_docker_config(config: &DockerOracleConfig) -> OracleResult<()> {
    validate_program_text("image", &config.image)?;
    if config.command.is_empty() {
        return Err(OracleError::invalid(
            "command",
            "docker oracle command is empty",
            "pass the exact command that writes the JSON report inside the container",
        ));
    }
    for (idx, part) in config.command.iter().enumerate() {
        validate_program_text("command[i]", part).map_err(|err| {
            OracleError::invalid(
                "command[i]",
                format!("command[{idx}] invalid: {err}"),
                "pass single-line Docker command arguments",
            )
        })?;
    }
    if config.report_path_host.as_os_str().is_empty() {
        return Err(OracleError::invalid(
            "report_path_host",
            "host report path is empty",
            "bind-mount a host directory and point report_path_host at the expected report",
        ));
    }
    if config.report_path_host.exists() {
        return Err(OracleError::invalid(
            "report_path_host",
            format!(
                "host report path already exists before Docker execution: {}",
                config.report_path_host.display()
            ),
            "delete stale reports and use a unique report path per run",
        ));
    }
    if config.timeout.is_zero() {
        return Err(OracleError::invalid(
            "timeout",
            "timeout must be greater than zero",
            "set an explicit finite Docker oracle timeout",
        ));
    }
    if config.accepted_exit_codes.is_empty() {
        return Err(OracleError::invalid(
            "accepted_exit_codes",
            "accepted_exit_codes must be non-empty",
            "explicitly include every Docker exit code that still produces a valid report",
        ));
    }
    for (idx, mount) in config.mounts.iter().enumerate() {
        if !mount.host_path.exists() {
            return Err(OracleError::invalid(
                "mounts[i].host_path",
                format!(
                    "mounts[{idx}].host_path does not exist: {}",
                    mount.host_path.display()
                ),
                "create the host mount before running the Docker oracle",
            ));
        }
        if mount.container_path.trim().is_empty()
            || !mount.container_path.starts_with('/')
            || mount.container_path.contains('\0')
        {
            return Err(OracleError::invalid(
                "mounts[i].container_path",
                format!("mounts[{idx}].container_path is invalid"),
                "use an absolute single-line container path",
            ));
        }
    }
    Ok(())
}

pub fn configure_child_process_group(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
}

pub fn wait_with_timeout(child: Child, timeout: Duration, program: &str) -> OracleResult<Output> {
    wait_with_timeout_interruptible(child, timeout, program, None)
}

pub fn wait_with_timeout_interruptible(
    mut child: Child,
    timeout: Duration,
    program: &str,
    interrupted: Option<&AtomicBool>,
) -> OracleResult<Output> {
    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .map_err(|err| {
                OracleError::command_failed(
                    program,
                    format!("failed while polling process status: {err}"),
                    "inspect OS process state and rerun with a clean oracle process",
                )
            })?
            .is_some()
        {
            return child.wait_with_output().map_err(|err| {
                OracleError::command_failed(
                    program,
                    format!("failed to collect process output: {err}"),
                    "inspect OS process state and stdout/stderr pipes",
                )
            });
        }
        if interrupted
            .map(|flag| flag.load(Ordering::SeqCst))
            .unwrap_or(false)
        {
            terminate_child_process_group(&mut child, program)?;
            let output = child.wait_with_output().map_err(|err| {
                OracleError::interrupted(
                    program,
                    format!("interrupted and failed to collect output: {err}"),
                    "rerun the corpus command after confirming no stale SWE-bench containers remain",
                )
            })?;
            return Err(OracleError::interrupted(
                program,
                format!(
                    "interrupted after {:?}; stdout_tail={}; stderr_tail={}",
                    started.elapsed(),
                    tail_text(&output.stdout),
                    tail_text(&output.stderr),
                ),
                "rerun the corpus command with --resume-incomplete or use a fresh shard output after inspecting the partial index",
            ));
        }
        if started.elapsed() >= timeout {
            terminate_child_process_group(&mut child, program)?;
            let output = child.wait_with_output().map_err(|err| {
                OracleError::timeout(
                    program,
                    format!("timed out after {timeout:?} and failed to collect output: {err}"),
                    "increase the timeout only after profiling or fix the hung runner",
                )
            })?;
            return Err(OracleError::timeout(
                program,
                format!(
                    "timed out after {timeout:?}; stdout_tail={}; stderr_tail={}",
                    tail_text(&output.stdout),
                    tail_text(&output.stderr),
                ),
                "fix the hung runner, stuck tests, or Docker resource starvation before retrying",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn terminate_child_process_group(child: &mut Child, program: &str) -> OracleResult<()> {
    #[cfg(unix)]
    {
        let process_group = format!("-{}", child.id());
        let term_status = Command::new("kill")
            .args(["-TERM", "--", &process_group])
            .status()
            .map_err(|err| {
                OracleError::command_failed(
                    program,
                    format!("failed to send SIGTERM to process group {process_group}: {err}"),
                    "inspect OS process state; retry after cleaning stale oracle subprocesses",
                )
            })?;
        if !term_status.success() {
            let _ = child.kill();
        }
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(2) {
            if child
                .try_wait()
                .map_err(|err| {
                    OracleError::command_failed(
                        program,
                        format!("failed while polling terminated process group: {err}"),
                        "inspect OS process state; retry after cleaning stale oracle subprocesses",
                    )
                })?
                .is_some()
            {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(25));
        }
        let kill_status = Command::new("kill")
            .args(["-KILL", "--", &process_group])
            .status()
            .map_err(|err| {
                OracleError::command_failed(
                    program,
                    format!("failed to send SIGKILL to process group {process_group}: {err}"),
                    "inspect OS process state; retry after cleaning stale oracle subprocesses",
                )
            })?;
        if !kill_status.success() {
            let _ = child.kill();
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        child.kill().map_err(|err| {
            OracleError::command_failed(
                program,
                format!("failed to terminate child process: {err}"),
                "inspect OS process state; retry after cleaning stale oracle subprocesses",
            )
        })
    }
}

fn read_report_path(report_path: &Path) -> OracleResult<OracleVerdict> {
    let report_text = fs::read_to_string(report_path).map_err(|err| {
        OracleError::io(
            report_path,
            err.to_string(),
            "ensure the oracle runner writes the configured report path",
        )
    })?;
    let root: Value = serde_json::from_str(&report_text).map_err(|err| {
        OracleError::json_parse(
            report_path.display().to_string(),
            err.to_string(),
            "write valid JSON from the test runner and keep the raw report for inspection",
        )
    })?;
    parse_pytest_json_value(&root)
}

fn verify_docker_available(timeout: Duration) -> OracleResult<()> {
    let mut command = Command::new("docker");
    command
        .args(["version", "--format", "{{.Server.Version}}"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_child_process_group(&mut command);
    let child = command.spawn().map_err(|err| {
        OracleError::command_failed(
            "docker",
            format!("failed to spawn docker version: {err}"),
            "install Docker and ensure the daemon socket is accessible before running Docker oracle",
        )
    })?;
    let output = wait_with_timeout(child, timeout.min(Duration::from_secs(30)), "docker")?;
    if !output.status.success() {
        return Err(OracleError::command_failed(
            "docker",
            format!(
                "docker version failed; stdout_tail={}; stderr_tail={}",
                tail_text(&output.stdout),
                tail_text(&output.stderr),
            ),
            "start Docker and resolve daemon access before running the oracle",
        ));
    }
    if String::from_utf8_lossy(&output.stdout).trim().is_empty() {
        return Err(OracleError::command_failed(
            "docker",
            "docker version returned an empty server version",
            "ensure the Docker client can reach a running daemon",
        ));
    }
    Ok(())
}

fn docker_mount_arg(mount: &DockerMount) -> OracleResult<String> {
    let source = mount.host_path.as_os_str().to_string_lossy();
    if source.contains(',') || source.contains('=') {
        return Err(OracleError::invalid(
            "mounts[i].host_path",
            format!("host path cannot be encoded in Docker --mount syntax: {source}"),
            "use a host path without comma or equals characters",
        ));
    }
    if mount.container_path.contains(',') || mount.container_path.contains('=') {
        return Err(OracleError::invalid(
            "mounts[i].container_path",
            format!(
                "container path cannot be encoded in Docker --mount syntax: {}",
                mount.container_path
            ),
            "use a container path without comma or equals characters",
        ));
    }
    let mut arg = format!("type=bind,source={source},target={}", mount.container_path);
    if mount.readonly {
        arg.push_str(",readonly");
    }
    Ok(arg)
}

#[derive(Clone, Copy)]
enum PytestOutcomeKind {
    Passed,
    Failed,
    Skipped,
    Error,
    XFailed,
    XPassed,
}

impl PytestOutcomeKind {
    fn index(self) -> usize {
        match self {
            Self::Passed => 0,
            Self::Failed => 1,
            Self::Skipped => 2,
            Self::Error => 3,
            Self::XFailed => 4,
            Self::XPassed => 5,
        }
    }
}

fn parse_outcome(text: &str, idx: usize) -> OracleResult<(TestOutcome, PytestOutcomeKind)> {
    match text {
        "passed" | "pass" => Ok((TestOutcome::Pass, PytestOutcomeKind::Passed)),
        "failed" | "fail" => Ok((TestOutcome::Fail, PytestOutcomeKind::Failed)),
        "skipped" | "skip" => Ok((TestOutcome::Skip, PytestOutcomeKind::Skipped)),
        "xfailed" => Ok((TestOutcome::Skip, PytestOutcomeKind::XFailed)),
        "xpassed" => Ok((TestOutcome::Fail, PytestOutcomeKind::XPassed)),
        "error" | "errored" => Ok((TestOutcome::Error, PytestOutcomeKind::Error)),
        other => Err(OracleError::report_invalid(
            "tests[i].outcome",
            format!("tests[{idx}] has unsupported outcome {other:?}"),
            "normalize runner outcomes to passed/failed/skipped/error before parsing",
        )),
    }
}

fn test_runtime_ms(test: &Value, idx: usize) -> OracleResult<i64> {
    let mut duration_secs = None;
    for field in ["setup", "call", "teardown"] {
        if let Some(duration) = test
            .get(field)
            .and_then(|v| v.get("duration"))
            .and_then(Value::as_f64)
        {
            if !duration.is_finite() || duration < 0.0 {
                return Err(OracleError::report_invalid(
                    "tests[i].duration",
                    format!("tests[{idx}].{field}.duration is invalid: {duration}"),
                    "write finite non-negative duration seconds",
                ));
            }
            duration_secs = Some(duration_secs.unwrap_or(0.0) + duration);
        }
    }
    if duration_secs.is_none() {
        duration_secs = test.get("duration").and_then(Value::as_f64);
    }
    match duration_secs {
        Some(duration) if duration.is_finite() && duration >= 0.0 => {
            Ok((duration * 1000.0).round() as i64)
        }
        Some(duration) => Err(OracleError::report_invalid(
            "tests[i].duration",
            format!("tests[{idx}].duration is invalid: {duration}"),
            "write finite non-negative duration seconds",
        )),
        None => Ok(-1),
    }
}

fn required_summary_usize(summary: &Value, key: &'static str) -> OracleResult<usize> {
    optional_summary_usize(summary, key)?.ok_or_else(|| {
        OracleError::report_invalid(
            "summary",
            format!("summary.{key} is missing"),
            "write pytest summary totals in the JSON report",
        )
    })
}

fn optional_summary_usize(summary: &Value, key: &'static str) -> OracleResult<Option<usize>> {
    let Some(value) = summary.get(key) else {
        return Ok(None);
    };
    let Some(n) = value.as_u64() else {
        return Err(OracleError::report_invalid(
            "summary",
            format!("summary.{key} must be an unsigned integer"),
            "write summary counts as non-negative integers",
        ));
    };
    usize::try_from(n).map(Some).map_err(|_| {
        OracleError::report_invalid(
            "summary",
            format!("summary.{key}={n} does not fit usize"),
            "shard pathological test reports before oracle parsing",
        )
    })
}

fn validate_summary_count(summary: &Value, key: &'static str, actual: usize) -> OracleResult<()> {
    if let Some(expected) = optional_summary_usize(summary, key)? {
        if expected != actual {
            return Err(OracleError::report_invalid(
                "summary",
                format!("summary.{key}={expected} but tests array counted {actual}"),
                "fix the report producer or parse only complete runner output",
            ));
        }
    } else if actual > 0 {
        return Err(OracleError::report_invalid(
            "summary",
            format!("summary.{key} is missing but tests array counted {actual}"),
            "write complete per-outcome summary counts for every non-zero pytest outcome",
        ));
    }
    Ok(())
}

fn validate_error_summary_count(summary: &Value, actual: usize) -> OracleResult<()> {
    let errors = optional_summary_usize(summary, "errors")?;
    let error = optional_summary_usize(summary, "error")?;
    if errors.is_none() && error.is_none() {
        if actual > 0 {
            return Err(OracleError::report_invalid(
                "summary",
                format!("summary.error/errors is missing but tests array counted {actual}"),
                "write the pytest collection/error count under error or errors",
            ));
        }
        return Ok(());
    }
    if let Some(expected) = errors {
        if expected != actual {
            return Err(OracleError::report_invalid(
                "summary",
                format!("summary.errors={expected} but tests array counted {actual}"),
                "fix the report producer or parse only complete runner output",
            ));
        }
    }
    if let Some(expected) = error {
        if expected != actual {
            return Err(OracleError::report_invalid(
                "summary",
                format!("summary.error={expected} but tests array counted {actual}"),
                "fix the report producer or parse only complete runner output",
            ));
        }
    }
    Ok(())
}

fn top_level_exception(root: &Value) -> Option<ExceptionClass> {
    exception_from_json_fragment(root.get("exception"))
        .or_else(|| exception_from_json_fragment(root.get("error")))
        .or_else(|| exception_from_json_fragment(root.get("longrepr")))
        .or_else(|| exception_from_json_fragment(root.get("collectors")))
}

fn exception_from_json_fragment(value: Option<&Value>) -> Option<ExceptionClass> {
    let value = value?;
    let text = match value {
        Value::String(s) => s.clone(),
        other => serde_json::to_string(other).ok()?,
    };
    exception_from_text(&text)
}

fn exception_from_text(text: &str) -> Option<ExceptionClass> {
    for (needle, class) in [
        ("AssertionError", ExceptionClass::AssertionError),
        ("TypeError", ExceptionClass::TypeError),
        ("ValueError", ExceptionClass::ValueError),
        ("ImportError", ExceptionClass::ImportError),
        ("ModuleNotFoundError", ExceptionClass::ImportError),
        ("AttributeError", ExceptionClass::AttributeError),
        ("KeyError", ExceptionClass::KeyError),
        ("IndexError", ExceptionClass::IndexError),
        ("NameError", ExceptionClass::NameError),
        ("RuntimeError", ExceptionClass::RuntimeError),
    ] {
        if text.contains(needle) {
            return Some(class);
        }
    }
    None
}

fn validate_single_line_text(field: &'static str, value: &str) -> OracleResult<()> {
    if value.trim().is_empty() {
        return Err(OracleError::report_invalid(
            field,
            "value is empty or whitespace-only",
            "write non-empty identifiers into oracle reports",
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(OracleError::report_invalid(
            field,
            "value contains control characters",
            "write single-line UTF-8 identifiers into oracle reports",
        ));
    }
    Ok(())
}

fn validate_program_text(field: &'static str, value: &str) -> OracleResult<()> {
    if value.trim().is_empty() {
        return Err(OracleError::invalid(
            field,
            "value is empty or whitespace-only",
            "pass a non-empty single-line command value",
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(OracleError::invalid(
            field,
            "value contains control characters",
            "pass single-line UTF-8 command values",
        ));
    }
    Ok(())
}

fn validate_env_pair(key: &str, _value: &str) -> OracleResult<()> {
    if key.trim().is_empty()
        || key.contains('=')
        || key.chars().any(char::is_control)
        || key.as_bytes().contains(&0)
    {
        return Err(OracleError::invalid(
            "env",
            format!("invalid environment key {key:?}"),
            "use non-empty environment keys without '=' or control characters",
        ));
    }
    Ok(())
}

fn cf_options() -> Options {
    let mut opts = Options::default();
    opts.set_write_buffer_size(16 * 1024 * 1024);
    opts.set_max_write_buffer_number(2);
    opts.set_target_file_size_base(32 * 1024 * 1024);
    opts
}

fn corpus_key(task_id: &str, category: MutationCategory) -> OracleResult<String> {
    validate_key_part("task_id", task_id)?;
    Ok(format!("{}|{}", task_id, category.slug()))
}

fn parse_corpus_key(key: &str) -> OracleResult<(String, MutationCategory)> {
    let mut parts = key.split('|');
    let task_id = parts.next().ok_or_else(|| {
        OracleError::store(
            "decode_key",
            "<corpus_row>",
            "corpus key missing task id",
            "only use corpus keys generated by OracleStore",
        )
    })?;
    let category_slug = parts.next().ok_or_else(|| {
        OracleError::store(
            "decode_key",
            "<corpus_row>",
            "corpus key missing category",
            "only use corpus keys generated by OracleStore",
        )
    })?;
    if parts.next().is_some() {
        return Err(OracleError::store(
            "decode_key",
            "<corpus_row>",
            format!("corpus key contains too many separators: {key:?}"),
            "only use corpus keys generated by OracleStore",
        ));
    }
    validate_key_part("task_id", task_id)?;
    let category = MutationCategory::all()
        .into_iter()
        .find(|category| category.slug() == category_slug)
        .ok_or_else(|| {
            OracleError::store(
                "decode_key",
                "<corpus_row>",
                format!("unknown mutation category in corpus key: {category_slug:?}"),
                "only use canonical MutationCategory slugs in corpus keys",
            )
        })?;
    Ok((task_id.to_string(), category))
}

fn provenance_key(corpus_version: &str, embedder_version: &str) -> OracleResult<String> {
    validate_key_part("corpus_version", corpus_version)?;
    validate_key_part("embedder_version", embedder_version)?;
    Ok(format!("{corpus_version}|{embedder_version}"))
}

fn validate_key_part(field: &'static str, value: &str) -> OracleResult<()> {
    if value.trim().is_empty() {
        return Err(OracleError::invalid(
            field,
            "key component is empty or whitespace-only",
            "use a stable non-empty task id",
        ));
    }
    if value.contains('|') || value.chars().any(char::is_control) {
        return Err(OracleError::invalid(
            field,
            "key component contains a separator or control character",
            "use single-line task ids without the '|' separator",
        ));
    }
    Ok(())
}

pub(crate) fn tail_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut chars = text.chars().rev().take(2048).collect::<Vec<_>>();
    chars.reverse();
    chars.into_iter().collect::<String>().replace('\n', "\\n")
}

#[allow(dead_code)]
fn _iterator_mode_from_prefix(prefix: &[u8]) -> IteratorMode<'_> {
    IteratorMode::From(prefix, Direction::Forward)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        apply_mutation, compute_corpus_sha256, CorpusProvenance, CorpusProvenanceRow, Language,
        MutationConfig, MEJEPA_CORPUS_VERSION_V1, MEJEPA_EMBEDDER_VERSION_V1,
    };
    use std::ffi::OsStr;

    const REPORT: &str = r#"{
      "summary": {"passed": 1, "failed": 1, "skipped": 1, "errors": 1, "total": 4},
      "tests": [
        {"nodeid": "tests/test_calc.py::test_add", "outcome": "passed", "call": {"duration": 0.010}},
        {"nodeid": "tests/test_calc.py::test_subtract", "outcome": "failed", "call": {"duration": 0.020}, "longrepr": "AssertionError: expected 3"},
        {"nodeid": "tests/test_calc.py::test_optional", "outcome": "skipped", "call": {"duration": 0.000}},
        {"nodeid": "tests/test_calc.py::test_import", "outcome": "error", "setup": {"duration": 0.001}, "longrepr": "ImportError: missing package"}
      ]
    }"#;

    #[test]
    fn parses_pytest_json_report_and_exception_class() {
        let verdict = parse_pytest_json_report(REPORT).unwrap();
        assert_eq!(verdict.per_test.len(), 4);
        assert_eq!(verdict.per_test[0].outcome, TestOutcome::Pass);
        assert_eq!(verdict.per_test[0].runtime_ms, 10);
        assert_eq!(verdict.exception, Some(ExceptionClass::ImportError));
        assert!(!verdict.all_passed());
    }

    #[test]
    fn rejects_mismatched_summary_counts() {
        let bad = REPORT.replace("\"passed\": 1", "\"passed\": 2");
        let err = parse_pytest_json_report(&bad).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_REPORT_INVALID");
    }

    #[test]
    fn validates_raw_pytest_xfail_xpass_summary_counts() {
        let report = r#"{
          "summary": {"passed": 1, "xfailed": 1, "xpassed": 1, "total": 3},
          "tests": [
            {"nodeid": "tests/test_x.py::test_pass", "outcome": "passed", "call": {"duration": 0.001}},
            {"nodeid": "tests/test_x.py::test_xfail", "outcome": "xfailed", "call": {"duration": 0.001}},
            {"nodeid": "tests/test_x.py::test_xpass", "outcome": "xpassed", "call": {"duration": 0.001}}
          ]
        }"#;
        let verdict = parse_pytest_json_report(report).unwrap();
        assert_eq!(verdict.per_test[0].outcome, TestOutcome::Pass);
        assert_eq!(verdict.per_test[1].outcome, TestOutcome::Skip);
        assert_eq!(verdict.per_test[2].outcome, TestOutcome::Fail);

        let bad = report.replace("\"xfailed\": 1", "\"xfailed\": 2");
        let err = parse_pytest_json_report(&bad).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_REPORT_INVALID");
    }

    #[test]
    fn rejects_missing_nonzero_summary_bucket() {
        let report = r#"{
          "summary": {"total": 1},
          "tests": [
            {"nodeid": "tests/test_x.py::test_pass", "outcome": "passed", "call": {"duration": 0.001}}
          ]
        }"#;
        let err = parse_pytest_json_report(report).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_REPORT_INVALID");
    }

    #[test]
    fn command_oracle_reads_report_from_source_of_truth() {
        let temp = tempfile::tempdir().unwrap();
        let report_path = temp.path().join("report.json");
        let config = CommandOracleConfig {
            program: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "printf '%s' \"$MEJEPA_REPORT_JSON\" > \"$MEJEPA_REPORT_PATH\"".to_string(),
            ],
            cwd: None,
            env: vec![
                (
                    "MEJEPA_REPORT_PATH".to_string(),
                    report_path.display().to_string(),
                ),
                ("MEJEPA_REPORT_JSON".to_string(), REPORT.to_string()),
            ],
            report_path: report_path.clone(),
            timeout: Duration::from_secs(5),
            accepted_exit_codes: vec![0],
        };
        let verdict = run_command_json_oracle(&config).unwrap();
        assert_eq!(verdict.per_test.len(), 4);
        assert!(report_path.exists());
    }

    #[test]
    fn command_oracle_rejects_stale_report_path() {
        let temp = tempfile::tempdir().unwrap();
        let report_path = temp.path().join("report.json");
        fs::write(&report_path, REPORT).unwrap();
        let config = CommandOracleConfig {
            program: "sh".to_string(),
            args: vec!["-c".to_string(), "true".to_string()],
            cwd: None,
            env: vec![],
            report_path,
            timeout: Duration::from_secs(5),
            accepted_exit_codes: vec![0],
        };
        let err = run_command_json_oracle(&config).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_INVALID_INPUT");
    }

    #[test]
    fn oracle_store_persists_atomic_corpus_row_and_provenance_cf() {
        let temp = tempfile::tempdir().unwrap();
        let store = OracleStore::open(temp.path()).unwrap();
        let outcome = apply_mutation(
            MutationCategory::KnownGood,
            "def add(a, b):\n    return a + b\n",
            MutationConfig::default(),
        )
        .unwrap();
        let verdict = OracleVerdict {
            per_test: vec![PerTestOutcome {
                test_id: "tests/test_calc.py::test_add".to_string(),
                outcome: TestOutcome::Pass,
                runtime_ms: 3,
            }],
            exception: None,
            evidence_unavailable: false,
        };
        let patch_sha256 =
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let verdict_sha256 =
            "sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        let corpus_sha256 = compute_corpus_sha256(&[CorpusProvenanceRow {
            task_id: "synthetic-task-001".to_string(),
            category: MutationCategory::KnownGood,
            patch_sha256: patch_sha256.to_string(),
            oracle_verdict_sha256: verdict_sha256.to_string(),
        }])
        .unwrap();
        let provenance = CorpusProvenance {
            corpus_version: MEJEPA_CORPUS_VERSION_V1.to_string(),
            embedder_version: MEJEPA_EMBEDDER_VERSION_V1.to_string(),
            generated_at_unix_ms: 1,
            generator_version: "test".to_string(),
            seed: 0,
            languages: vec![Language::Python],
            task_manifest_count: 1,
            mutation_count: 1,
            mutation_categories: vec![MutationCategory::KnownGood],
            source_patch_mode: "source-backed".to_string(),
            split_mode: "instance_atomic".to_string(),
            corpus_sha256,
            complete: true,
        };
        store
            .put_corpus_row(
                "synthetic-task-001",
                MutationCategory::KnownGood,
                &outcome,
                &verdict,
            )
            .unwrap();
        store.put_provenance(&provenance).unwrap();
        store.flush().unwrap();

        assert_eq!(
            store.count_cf(CF_MEJEPA_CORPUS_MUTATION_OUTCOMES).unwrap(),
            1
        );
        assert_eq!(
            store
                .count_cf(CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON)
                .unwrap(),
            1
        );
        assert_eq!(store.count_cf(CF_MEJEPA_CORPUS_PROVENANCE_JSON).unwrap(), 1);
        assert_eq!(
            store
                .get_mutation("synthetic-task-001", MutationCategory::KnownGood)
                .unwrap(),
            Some(outcome)
        );
        assert_eq!(
            store
                .get_verdict("synthetic-task-001", MutationCategory::KnownGood)
                .unwrap(),
            Some(verdict)
        );
        assert_eq!(
            store
                .get_provenance(MEJEPA_CORPUS_VERSION_V1, MEJEPA_EMBEDDER_VERSION_V1)
                .unwrap(),
            Some(provenance)
        );
        assert_eq!(store.iter_corpus_rows().unwrap().len(), 1);
    }

    #[test]
    fn oracle_store_rejects_ambiguous_key_parts() {
        let temp = tempfile::tempdir().unwrap();
        let store = OracleStore::open(temp.path()).unwrap();
        let verdict = OracleVerdict {
            per_test: vec![PerTestOutcome {
                test_id: "tests/test_calc.py::test_add".to_string(),
                outcome: TestOutcome::Pass,
                runtime_ms: 3,
            }],
            exception: None,
            evidence_unavailable: false,
        };
        let outcome = apply_mutation(
            MutationCategory::KnownGood,
            "def add(a, b):\n    return a + b\n",
            MutationConfig::default(),
        )
        .unwrap();
        let err = store
            .put_corpus_row("bad|task", MutationCategory::KnownGood, &outcome, &verdict)
            .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_INVALID_INPUT");
    }

    #[test]
    fn command_oracle_times_out_hung_runner() {
        let temp = tempfile::tempdir().unwrap();
        let report_path = temp.path().join("report.json");
        let config = CommandOracleConfig {
            program: "sh".to_string(),
            args: vec!["-c".to_string(), "sleep 2".to_string()],
            cwd: None,
            env: vec![],
            report_path,
            timeout: Duration::from_millis(50),
            accepted_exit_codes: vec![0],
        };
        let err = run_command_json_oracle(&config).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_TIMEOUT");
    }

    #[cfg(unix)]
    #[test]
    fn timeout_kills_child_process_group() {
        let marker = format!("contextgraph_timeout_child_{}", std::process::id());
        let mut command = Command::new("bash");
        command
            .args(["-c", &format!("exec -a {marker} sleep 30")])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_child_process_group(&mut command);
        let child = command.spawn().unwrap();
        let err = wait_with_timeout(child, Duration::from_millis(50), "timeout-test").unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_TIMEOUT");
        thread::sleep(Duration::from_millis(100));
        let output = Command::new("pgrep")
            .args(["-f", &format!("^{marker}")])
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).trim().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn interrupt_kills_child_process_group() {
        let marker = format!("contextgraph_interrupt_child_{}", std::process::id());
        let interrupted = AtomicBool::new(true);
        let mut command = Command::new("bash");
        command
            .args(["-c", &format!("exec -a {marker} sleep 30")])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_child_process_group(&mut command);
        let child = command.spawn().unwrap();
        let err = wait_with_timeout_interruptible(
            child,
            Duration::from_secs(30),
            "interrupt-test",
            Some(&interrupted),
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_ORACLE_INTERRUPTED");
        thread::sleep(Duration::from_millis(100));
        let output = Command::new("pgrep")
            .args(["-f", &format!("^{marker}")])
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).trim().is_empty());
    }

    fn _assert_os_str_send_sync(_: &OsStr) {}
}
